// Execute the actual generated integration in a sandbox; never launch an agent or touch user config.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const deadline=setTimeout(()=>{console.error('Generated integration test timed out');process.exit(1);},10000);
(async () => {
  const {agent, source} = JSON.parse(fs.readFileSync(0, 'utf8'));
  const calls = [], handlers = new Map(), timers = [];
  let clock=Date.now();
  class Clock extends Date { static now(){return clock;} }
  const sandbox = {
    spawnSync: (_exe,args,opts) => { calls.push({event:args[2], ...JSON.parse(opts.input)}); return {status:0}; },
    process: {pid:1234,cwd:()=> 'C:/project',once:()=>{}},
    setInterval: fn => {timers.push(fn);return {unref(){}};}, clearInterval(){},
    AbortController, setTimeout, clearTimeout, console, Date:Clock,
  };
  let code=source.replace(/import \{ spawnSync \} from "node:child_process";/,'');
  code=code.replace('export default function emberExtension','function emberExtension')
    .replace('export const EmberHooks','const EmberHooks')
    .replace('export default { id: "ember-hooks", server: EmberHooks, setup };','');
  code += agent==='opencode' ? '\nglobalThis.factory=EmberHooks;globalThis.setup=setup;' : '\nglobalThis.factory=emberExtension;';
  vm.runInNewContext(code,sandbox);
  const last=()=>calls.at(-1)?.event;
  if(agent==='opencode'){
    const plugin=await sandbox.factory({directory:'C:/project'});
    const emit=async(type,properties={})=> plugin.event({event:{type,properties:{sessionID:'one',...properties}}});
    await emit('session.status',{status:{type:'busy'}});assert.equal(last(),'working');
    await emit('session.status',{status:{type:'idle'}});assert.equal(last(),'idle');
    const count=calls.length;timers.forEach(fn=>fn());assert.equal(calls.length,count,'idle cannot heartbeat');
    await emit('session.status',{status:{type:'retry'}});assert.equal(last(),'working');
    await emit('session.idle');assert.equal(last(),'idle');
    await emit('session.status',{status:{type:'busy'}});
    await emit('session.error',{error:{name:'MessageAbortedError'}});assert.equal(last(),'idle');
    await emit('session.deleted');assert.equal(last(),'end');
    await emit('session.status',{status:{type:'busy'}});
    await emit('question.asked',{id:'q1'});assert.equal(last(),'idle');
    const waitingCount=calls.length;clock+=60001;timers.forEach(fn=>fn());
    await emit('session.status',{status:{type:'busy'}});assert.equal(calls.length,waitingCount,'busy and timers cannot override a pending question');
    await emit('question.replied',{requestID:'q1'});assert.equal(last(),'working');
    await plugin['tool.execute.after']({sessionID:'one',tool:'bash'});assert.equal(last(),'working','ordinary tool completion does not idle');
    await emit('session.error',{error:{name:'AnyTerminalError'}});assert.equal(last(),'idle');
    await emit('session.execution.started');assert.equal(last(),'working');
    const v1Owned=calls.length;
    await emit('session.idle');await emit('session.status',{status:{type:'idle'}});
    await emit('session.error');assert.equal(calls.length,v1Owned,'v2 execution also owns the v1 adapter until terminal');
    clock+=60*60*1000;timers.forEach(fn=>fn());assert.equal(last(),'working','one hour of reasoning does not time out');
    await emit('session.execution.interrupted');assert.equal(last(),'idle');
    // V2 idle-looking step events must not terminate the execution lifecycle.
    let complete;
    const completed=new Promise(resolve=>complete=resolve);
    async function* events(){
      const event=(type,extra={})=>({type,properties:{sessionID:'v2',...extra}});
      yield event('session.execution.started');assert.equal(last(),'working');
      const count=calls.length;
      yield event('session.status',{status:{type:'idle'}});assert.equal(calls.length,count);
      clock+=60001;timers.forEach(fn=>fn());assert.equal(last(),'working','silent reasoning retains its turn');
      yield event('session.reasoning.delta');assert.equal(last(),'working');
      yield event('session.tool.success');assert.equal(last(),'working','tool completion is not turn completion');
      yield event('form.created',{form:{id:'f1',sessionID:'v2'}});assert.equal(last(),'idle');
      const waitingCount=calls.length;clock+=60001;timers.forEach(fn=>fn());
      yield event('session.status',{status:{type:'busy'}});assert.equal(calls.length,waitingCount);
      yield event('permission.asked',{id:'p1'});assert.equal(last(),'idle');
      yield event('form.replied',{id:'f1'});assert.equal(last(),'idle','other pending request must keep waiting');
      yield event('permission.replied',{id:'p1'});assert.equal(last(),'working');
      yield event('session.execution.succeeded');assert.equal(last(),'idle');
      const terminalCount=calls.length;
      yield event('session.reasoning.delta');assert.equal(calls.length,terminalCount,'late reasoning cannot undo terminal completion');
      yield event('session.execution.started');assert.equal(last(),'working');
      yield event('form.created',{form:{id:'f2',sessionID:'v2'}});assert.equal(last(),'idle');
      yield event('session.execution.interrupted');assert.equal(last(),'idle');
      const cancelledCount=calls.length;
      yield event('form.replied',{id:'f2'});assert.equal(calls.length,cancelledCount,'late answer cannot undo Escape');
      yield event('session.deleted');assert.equal(last(),'end');
      complete();
    }
    const cleanup=await sandbox.setup({event:{subscribe:()=>events()},location:{directory:'C:/project'}});
    await completed;await cleanup();

    // A transport failure is not completion. Reconnect and reconcile a missed
    // terminal event through metadata rather than timing out silent reasoning.
    let subscriptions=0, reconnected, recoveredIdle=100;
    const recovery=new Promise(resolve=>reconnected=resolve);
    const resilient=await sandbox.setup({
      session:{get:async()=>({time:{busy:{epochMillis:200},idle:{epochMillis:recoveredIdle}}})},
      event:{subscribe:({signal})=>(async function*(){
        subscriptions++;
        if(subscriptions===1){
          yield {type:'session.execution.started',properties:{sessionID:'reconnect'}};
          assert.equal(last(),'working');
          throw new Error('transport closed');
        }
        if(subscriptions===2){
          assert.equal(last(),'working','disconnect does not terminate ongoing silent work');
          recoveredIdle=300;return;
        }
        assert.equal(last(),'idle','metadata recovers the terminal event lost during disconnect');
        reconnected();
        await new Promise(resolve=>{signal.addEventListener('abort',resolve,{once:true});if(signal.aborted)resolve();});
      })()},
    });
    await recovery;assert.ok(subscriptions>=2);await resilient();

    // A slow metadata response from the previous turn cannot idle a newer turn,
    // even when both transitions happen within the same clock millisecond.
    let answerOld, finishRace, reads=0;
    const raceDone=new Promise(resolve=>finishRace=resolve);
    const racing=await sandbox.setup({
      session:{get:()=>++reads===1?new Promise(resolve=>answerOld=resolve):Promise.resolve({time:{busy:500,idle:200}})},
      event:{subscribe:({signal})=>(async function*(){
        const event=type=>({type,properties:{sessionID:'race'}});
        yield event('session.execution.started');
        timers.forEach(fn=>fn());
        assert.equal(typeof answerOld,'function');
        yield event('session.execution.interrupted');
        yield event('session.execution.started');
        const newer=calls.length;
        answerOld({time:{busy:100,idle:200}});
        await Promise.resolve();await Promise.resolve();
        assert.equal(calls.length,newer,'old metadata cannot override the newer execution');
        assert.equal(last(),'working');finishRace();
        await new Promise(resolve=>{signal.addEventListener('abort',resolve,{once:true});if(signal.aborted)resolve();});
      })()},
    });
    await raceDone;await racing();

  }else{
    sandbox.factory({on:(name,handler)=>handlers.set(name,handler)});
    const ctx={sessionManager:{getSessionId:()=> 'one'}};
    const emit=(name,event={})=>handlers.get(name)?.(event,ctx);
    emit('session_start');assert.equal(last(),'start');
    emit('agent_start');assert.equal(last(),'working');
    if(agent==='pi'){
      assert.equal(handlers.has('agent_end'),false,'Pi low-level end must not release during retry');
      timers.forEach(fn=>fn());assert.equal(last(),'working','long thinking refreshes state');
      emit('ui_prompt_start');assert.equal(last(),'waiting');
      const waitingCount=calls.length;timers.forEach(fn=>fn());assert.equal(calls.length,waitingCount);
      emit('tool_execution_start',{toolName:'bash'});assert.equal(calls.length,waitingCount,'a tool callback cannot override pending user input');
      emit('ui_prompt_end');assert.equal(last(),'resume');
      emit('ui_prompt_start');emit('agent_settled',{aborted:true});assert.equal(last(),'idle');
      const stopped=calls.length;emit('ui_prompt_end');assert.equal(calls.length,stopped,'cancelled prompt cannot resume');
    }else{
      const count=calls.length;emit('agent_end',{willContinue:true});assert.equal(calls.length,count);
      timers.forEach(fn=>fn());assert.equal(last(),'working');
      emit('tool_call',{toolName:'ask'});assert.equal(last(),'waiting');
      const questionCount=calls.length;
      emit('tool_execution_start',{toolName:'ask'});emit('tool_call',{toolName:'bash'});
      assert.equal(calls.length,questionCount,'question tool startup and parallel callbacks cannot clear its wait');
      emit('tool_execution_end',{toolName:'bash'});assert.equal(last(),'waiting');
      emit('tool_execution_end',{toolName:'ask'});assert.equal(last(),'resume');
      emit('agent_end',{willContinue:false});assert.equal(last(),'idle');
    }
    const count=calls.length;timers.forEach(fn=>fn());assert.equal(calls.length,count,'no idle resurrection');
    emit('session_shutdown');assert.equal(last(),'end');
  }
})().catch(e=>{console.error(e);process.exitCode=1;}).finally(()=>clearTimeout(deadline));
