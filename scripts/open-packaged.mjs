// Keep a packaged runtime open for release verification; no Vite interception.
import puppeteer from 'puppeteer-core';import {spawn} from 'node:child_process';import fs from 'node:fs';import path from 'node:path';
const exe=path.resolve(process.env.EMBER_TEST_EXE||'src-tauri/target/release/ember.exe');
const child=spawn(exe,[],{windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9333'}});child.unref();
fs.mkdirSync('research.local',{recursive:true});fs.writeFileSync('research.local/packaged-runtime.json',JSON.stringify({pid:child.pid,exe}));
let browser,page;
for(let n=0;n<100;n++){
 try{browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));if(page)break;browser.disconnect();}catch{}
 await new Promise(r=>setTimeout(r,100));
}
if(!page)throw Error('Packaged native window did not become ready');
console.log('Packaged runtime:',await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('app_version')));
await new Promise(resolve=>browser.once('disconnected',resolve));
