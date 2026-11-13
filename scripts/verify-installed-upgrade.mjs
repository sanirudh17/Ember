// Start the preserved public v1.1.0 runtime and use its real updater.
// Leave the resulting installed application open for the user's review.
import puppeteer from 'puppeteer-core';import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';import assert from 'node:assert/strict';import {spawn} from 'node:child_process';
const old=path.resolve('research.local/update-source-v1.1.0/ember.exe');
const installed=path.join(process.env.LOCALAPPDATA,'Ember/ember.exe');
const settingsPath=path.join(process.env.APPDATA,'Ember/settings.json');
const hash=f=>crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex');
const oldHash=hash(old);assert.equal(oldHash,'616e6aaf186ec3825fb139dacfa5e8ce8602b6ee1cc3905cf367bce05c1f2321');
// Tauri patches its three-byte UNK bundle marker to NSS while creating NSIS,
// then restores the standalone executable. Compare the installed NSS payload.
const payload=fs.readFileSync('src-tauri/target/release/ember.exe');
const sentinel=Buffer.from([0x55,0x4e,0x4b,0xc0,0x02,0x3a,0x20,0xc0,0x00]);
const offset=payload.indexOf(sentinel);assert(offset>=0,'Expected Tauri bundle marker');
assert.equal(payload.indexOf(sentinel,offset+1),-1,'Bundle marker must be unique');
payload.write('NSS',offset,'ascii');
const expected=crypto.createHash('sha256').update(payload).digest('hex');const before=JSON.parse(fs.readFileSync(settingsPath,'utf8'));
const child=spawn(old,[],{windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9333'}});child.unref();
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function connect(){let b;try{b=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});const p=(await b.pages()).find(p=>p.url().includes('tauri.localhost'));if(p)return{b,p};b.disconnect();}catch{b?.disconnect();}return null;}
let session;for(let n=0;n<120&&!session;n++){session=await connect();if(!session)await sleep(100);}
assert(session,'Old native app did not start');
const invoke=(c,a={})=>session.p.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),c,a);
assert.equal(await invoke('app_version'),'1.1.0');await invoke('hide_window');
let update;
for(let n=0;n<90;n++){update=await invoke('check_update');if(update?.version==='1.2.0')break;await sleep(2000);}
assert.equal(update?.current_version,'1.1.0');assert.equal(update?.version,'1.2.0');console.log('PASS actual v1.1.0 discovered v1.2.0 through its unchanged updater endpoint');
void invoke('install_update').catch(error=>{if(!String(error).match(/closed|disconnected|detached/i))console.error('Updater returned:',String(error));});
// A previous manual install may already have the destination bytes. Wait for
// this old runtime to actually exit before treating that hash as update proof.
let oldExited=false;
for(let n=0;n<360;n++){try{process.kill(child.pid,0);}catch{oldExited=true;break;}await sleep(500);}
assert(oldExited,'Old runtime did not exit through its updater');
let matched=false;
for(let n=0;n<240;n++){try{if(hash(installed)===expected){matched=true;break;}}catch{}await sleep(500);}
assert(matched,'The actual updater did not install the expected release bytes');
session.b.disconnect();session=null;
for(let n=0;n<480&&!session;n++){const candidate=await connect();if(candidate){try{const v=await candidate.p.evaluate(()=>window.__TAURI_INTERNALS__.invoke('app_version'));if(v==='1.2.0')session=candidate;else candidate.b.disconnect();}catch{candidate.b.disconnect();}}if(!session)await sleep(250);}
assert(session,'Updated installed application did not reconnect');
const after=JSON.parse(fs.readFileSync(settingsPath,'utf8'));delete before.window;delete after.window;assert.deepEqual(after,before,'User preferences must survive the actual update');
const assets=await session.p.$$eval('script[src]',els=>els.map(e=>e.src));assert(!assets.some(src=>src.includes(':1420')),'Verify the bundled frontend, not Vite');
assert.equal(await session.p.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue('--chrome').trim()),'#171717');
console.log('PASS actual signed update, installed binary hash, packaged graphite UI and preserved preferences');
fs.writeFileSync('research.local/installed-upgrade-proof.json',JSON.stringify({from:'1.1.0',to:'1.2.0',oldHash,installedHash:hash(installed),settingsPreserved:true,bundledFrontend:true},null,2));
await invoke('focus_window');
// Keep the verified runtime alive under the terminal job until the user closes it.
await new Promise(resolve=>session.b.once('disconnected',resolve));
