import puppeteer from 'puppeteer-core';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});
const page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));
if(!page)throw Error('native page missing');
const invoke=(command,args={})=>page.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),command,args);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const initial=await invoke('get_snapshot');
try{
 await invoke('update_settings',{settings:{...initial.settings,notifications:{...initial.settings.notifications,mute:true}}});
 for(const agent of initial.agents.filter(a=>a.installed&&a.needs_repair)){
  const preview=await invoke('preview_install',{agent:agent.id,op:'install'});
  assert(preview.can_install||preview.can_repair);
  const result=await invoke('apply_install',{agent:agent.id,op:'install'});
  assert(result.ok); console.log('REPAIRED',agent.id,'backup',Boolean(result.backup),'needsRepair',result.needs_repair);
 }
 await invoke('reset_window');await sleep(400);
 const compact=await page.evaluate(()=>({w:innerWidth,h:innerHeight}));assert(compact.w<=1120&&compact.h<=720);
 await invoke('window_control',{action:'toggle_maximize'});await sleep(350);assert(await invoke('window_control',{action:'state'}));
 const maximized=await page.evaluate(()=>({w:innerWidth,h:innerHeight}));assert(maximized.w>compact.w);
 await invoke('window_control',{action:'toggle_maximize'});await sleep(350);assert(!(await invoke('window_control',{action:'state'})));
 const restored=await page.evaluate(()=>({w:innerWidth,h:innerHeight}));assert(restored.w<=1120&&restored.h<=720);
 await invoke('window_control',{action:'minimize'});await sleep(150);await invoke('show_window');
 console.log('WINDOW PASS',JSON.stringify({compact,restored}));
 const exe=process.env.EMBER_TEST_EXE || 'C:/Users/sanir/Windows Tool/src-tauri/target/release/ember.exe';
 for(const agent of ['claude-code','codex','opencode','pi','oh-my-pi','antigravity','cline']){
  const sid=`ember-cancel-check-${agent}`;
  const post=event=>{const result=spawnSync(exe,['hook',agent,event],{input:JSON.stringify({session_id:sid,cwd:'C:/ember-regression',pid:process.pid}),timeout:2000,windowsHide:true});assert.equal(result.status,0)};
  const state=async()=> (await invoke('get_snapshot')).sessions.find(s=>s.agent===agent&&s.session_id===sid)?.state;
  post('working');await sleep(600);assert.equal(await state(),'working',agent);
  post('waiting');await sleep(400);assert.equal(await state(),'idle',agent);
  post('resume');await sleep(400);assert.equal(await state(),'working',agent);
  post('idle');post('resume');post('heartbeat');await sleep(600);assert.equal(await state(),'idle',agent);
  post('end');post('heartbeat');post('resume');await sleep(400);assert.equal(await state(),undefined,agent);console.log('CANCEL PIPE PASS',agent);
 }
}finally{
 await invoke('update_settings',{settings:initial.settings});
 browser.disconnect();
}
