// Run against a packaged Ember with a local WebView CDP port; no mock backend.
import puppeteer from 'puppeteer-core';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});
const page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));
assert(page,'Packaged window must exist');
const invoke=(c,a={})=>page.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),c,a);
const initial=await invoke('get_snapshot');
assert.equal(initial.agents.length,7);assert(initial.agents.every(a=>a.supported),'All seven current integrations must expose hook support');
const version=await invoke('app_version');assert.equal(version,process.env.EMBER_EXPECTED_VERSION||'1.2.0');
const exe=process.env.EMBER_TEST_EXE;assert(exe,'Set EMBER_TEST_EXE to the packaged runtime');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(predicate)=>{for(let n=0;n<40;n++){const s=await invoke('get_snapshot');if(predicate(s))return s;await sleep(100);}throw Error('Native transition timed out');};
assert(!initial.manual.active&&!initial.paused,'Do not interrupt a user manual hold or pause');
try {
 await invoke('update_settings',{settings:{...initial.settings,watching:false,lid_proof:false,notifications:{...initial.settings.notifications,mute:true}}});
 await until(s=>!s.engaged);
 for(const seconds of [1800,3600,7200]){
  await invoke('start_manual',{seconds});
  const started=await until(s=>s.engaged&&s.manual.active);assert(started.want_awake);assert(!started.blocker);
  await invoke('stop_manual');await until(s=>!s.engaged&&!s.manual.active);
 }
 const target=new Date(Date.now()+10*60*1000);
 await invoke('start_manual_until',{hour:target.getHours(),minute:target.getMinutes()});await until(s=>s.engaged&&s.manual.mode_label==='Until');
 await invoke('stop_manual');await until(s=>!s.engaged);
 await invoke('start_manual_unlimited');await until(s=>s.engaged&&s.manual.unlimited);
 await invoke('stop_manual');await until(s=>!s.engaged);
 console.log('PASS packaged manual holds, real power engagement and immediate stop:',version);
 for(const agent of ['claude-code','codex','opencode','pi','oh-my-pi','antigravity','cline']){
  const session=`release-proof-${agent}`;
  const post=event=>assert.equal(spawnSync(exe,['hook',agent,event],{input:JSON.stringify({session_id:session,pid:process.pid,cwd:'C:/ember-release-proof'}),windowsHide:true,timeout:2000}).status,0);
  const state=s=>s.sessions.find(r=>r.agent===agent&&r.session_id===session)?.state;
  try {
   post('working');await until(s=>state(s)==='working');
   post('waiting');await until(s=>state(s)==='idle');
   post('resume');await until(s=>state(s)==='working');
   post('idle');post('heartbeat');post('resume');await until(s=>state(s)==='idle');
   post('end');post('heartbeat');post('resume');await until(s=>state(s)===undefined);
   console.log('PASS packaged lifecycle pipe:',agent);
  }finally{post('end');}
 }
 await page.evaluate(()=>document.fonts.ready);
 assert(await page.evaluate(()=>document.fonts.check('14px Archivo')));
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 console.log('PASS packaged Archivo and layout; user geometry preserved');
}finally{
 await invoke('stop_manual');await invoke('update_settings',{settings:initial.settings});browser.disconnect();
}
