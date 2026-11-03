// Open the existing desktop runtime against Vite without compiling Rust or bundling assets.
// Keep the local app origin for IPC; resolve every development asset against Vite.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import puppeteer from 'puppeteer-core';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const dev='http://localhost:1420/';
await fetch(dev); // Start npm run dev separately if the server is not running.
let browser;
try { browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null}); }
catch {
 const child=spawn(path.join(root,'src-tauri/target/debug/ember.exe'),[],{cwd:root,windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9333'}});
 child.unref();
 for(let attempt=0;attempt<40;attempt++) {
  await new Promise(r=>setTimeout(r,250));
  try { browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});break; } catch {}
 }
 if(!browser)throw new Error('The existing Ember runtime did not open its development connection.');
}
const page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));
if(!page)throw new Error('No native Ember window found.');
const client=await page.createCDPSession();
await client.send('Fetch.enable',{patterns:[{urlPattern:'http://tauri.localhost/',resourceType:'Document',requestStage:'Request'}]});
client.on('Fetch.requestPaused',async event=>{
 try {
  const html=(await(await fetch(dev)).text()).replace('<head>',`<head><base href="${dev}">`);
  await client.send('Fetch.fulfillRequest',{requestId:event.requestId,responseCode:200,responseHeaders:[{name:'Content-Type',value:'text/html'}],body:Buffer.from(html).toString('base64')});
 }catch(error){console.error(String(error));await client.send('Fetch.failRequest',{requestId:event.requestId,errorReason:'Failed'}).catch(()=>{});}
});
await page.goto('http://tauri.localhost/',{waitUntil:'networkidle0'});
await page.evaluate(()=>document.fonts.ready);
console.log('Ember desktop is using live Vite modules and native IPC. No build ran.');
// Retain document interception for reloads as well as Vite HMR.
await new Promise(resolve=>browser.once('disconnected',resolve));
