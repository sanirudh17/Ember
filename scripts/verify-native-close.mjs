// Requires a packaged runtime on local CDP. Saves and restores user preferences.
import puppeteer from 'puppeteer-core';import fs from 'node:fs';import path from 'node:path';import assert from 'node:assert/strict';
const settingsPath=path.join(process.env.APPDATA,'Ember/settings.json');const original=fs.readFileSync(settingsPath);
const statePath=path.join(process.env.LOCALAPPDATA,'Ember/session-state.json');
const b=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});const p=(await b.pages()).find(p=>p.url().includes('tauri.localhost'));
const invoke=(c,a={})=>p.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),c,a);const initial=await invoke('get_snapshot');assert(!initial.manual.active&&!initial.paused);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(fn)=>{for(let n=0;n<60;n++){const s=await invoke('get_snapshot');if(fn(s))return s;await sleep(100);}throw Error('Close/lid state timed out');};
let exited=false;
try{
 await invoke('update_settings',{settings:{...initial.settings,watching:false,lid_proof:true,require_charger_for_lid:true,notifications:{...initial.settings.notifications,mute:true}}});
 await invoke('start_manual',{seconds:1800});const held=await until(s=>s.engaged);
 if(!held.battery.plugged_in){
  assert(!held.lid.held,'Require charger must prevent the lid override on battery');
  console.log('PASS actual battery/require-charger lid safeguard');
  await invoke('update_settings',{settings:{...initial.settings,watching:false,lid_proof:true,require_charger_for_lid:false,notifications:{...initial.settings.notifications,mute:true}}});
 }
 await until(s=>s.engaged&&s.lid.held);assert(fs.existsSync(statePath));
 await invoke('window_control',{action:'close'});await p.waitForSelector('[role=alertdialog]');
 assert((await invoke('get_snapshot')).engaged);await p.evaluate(()=>[...document.querySelectorAll('[role=alertdialog] button')].find(e=>e.textContent.trim()==='Keep running').click());
 await p.waitForSelector('[role=alertdialog]',{hidden:true});assert((await invoke('get_snapshot')).engaged);
 await invoke('window_control',{action:'close'});await p.waitForSelector('[role=alertdialog]');await p.keyboard.press('Escape');
 await p.waitForSelector('[role=alertdialog]',{hidden:true});assert((await invoke('get_snapshot')).engaged);
 await invoke('hide_window');await sleep(200);assert((await invoke('get_snapshot')).engaged);await invoke('show_window');
 console.log('PASS Keep running, Escape and hidden-window hold preservation');
 await invoke('window_control',{action:'close'});await p.waitForSelector('[role=alertdialog]');
 const gone=new Promise(resolve=>b.once('disconnected',resolve));
 await p.evaluate(()=>[...document.querySelectorAll('[role=alertdialog] button')].find(e=>e.textContent.trim()==='Quit Ember').click()).catch(()=>{});
 await Promise.race([gone,sleep(8000).then(()=>{throw Error('Quit did not close the runtime');})]);exited=true;
 for(let n=0;n<40&&fs.existsSync(statePath);n++)await sleep(100);
 assert(!fs.existsSync(statePath),'Quit must restore the real lid policy and remove its undo record');
 const {pid}=JSON.parse(fs.readFileSync('research.local/packaged-runtime.json','utf8'));
 let alive=true;for(let n=0;n<60;n++){try{process.kill(pid,0);}catch{alive=false;break;}await sleep(100);}
 assert(!alive,'Confirmed Quit must terminate Ember after restoring the lid policy');
 console.log('PASS active Quit restores lid policy; no sleep/shutdown requested');
}finally{
 if(!exited){await invoke('stop_manual').catch(()=>{});await invoke('update_settings',{settings:initial.settings}).catch(()=>{});b.disconnect();}
 else{fs.writeFileSync(settingsPath,original);}
}
