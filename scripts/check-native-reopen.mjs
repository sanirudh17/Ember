// Exercise the actual helper while Ember is closed, then inspect its first snapshot.
// Run after a native debug build with Ember's local CDP connection enabled.
import puppeteer from 'puppeteer-core';
import {spawn,spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const exe=process.env.EMBER_TEST_EXE || 'C:/Users/sanir/Windows Tool/src-tauri/target/debug/ember.exe';
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const connect=async()=>{
 for(let n=0;n<80;n++){
  let browser;
  try{
   browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});
   const page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));
   if(page){await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__);return {browser,page};}
   browser.disconnect();
  }catch{browser?.disconnect();}
  await sleep(100);
 }
 throw Error('Native window did not become ready');
};
let {browser,page}=await connect();
const invoke=(command,args={})=>page.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),command,args);
await invoke('quit_app');browser.disconnect();await sleep(700);
const session='ember-reopen-check';
const post=event=>assert.equal(spawnSync(exe,['hook','opencode',event],{
 input:JSON.stringify({session_id:session,pid:process.pid,cwd:'C:/ember-reopen-check'}),
 windowsHide:true,timeout:2000,
}).status,0);
post('working'); // No live pipe: this phase must survive through the local cache.
const child=spawn(exe,[],{windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9333'}});child.unref();
({browser,page}=await connect());
try{
 const snapshot=await invoke('get_snapshot');
 assert.equal(snapshot.sessions.find(s=>s.session_id===session)?.state,'working');
 console.log('REOPEN PASS: first snapshot recovered work without a new host event');
 const start=performance.now();post('idle');
 let ended=false;
 while(performance.now()-start<1500){
  if((await invoke('get_snapshot')).sessions.find(s=>s.session_id===session)?.state==='idle'){ended=true;break;}
  await sleep(40);
 }
 assert(ended);console.log('IDLE PASS: observed in',Math.round(performance.now()-start),'ms including helper launch');
 post('end');
 await invoke('reset_window');
}finally{post('end');browser.disconnect();}
