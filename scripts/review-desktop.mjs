import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import puppeteer from 'puppeteer-core';
const server=http.createServer((req,res)=>{let file=path.join('dist',decodeURIComponent(new URL(req.url,'http://localhost').pathname));if(!fs.existsSync(file)||fs.statSync(file).isDirectory())file='dist/index.html';res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':file.endsWith('.png')?'image/png':'text/html');res.end(fs.readFileSync(file));});
await new Promise(r=>server.listen(4175,'127.0.0.1',r));
const browser=await puppeteer.launch({executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true});
const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(String(e)));
fs.mkdirSync('screenshots/desktop-review',{recursive:true});
for(const theme of ['dark','light'])for(const [width,height] of [[860,620],[760,520],[1920,1080]])for(const route of ['awake','agents','settings']){
 await page.setViewport({width,height});await page.emulateMediaFeatures([{name:'prefers-color-scheme',value:theme}]);await page.goto(`http://127.0.0.1:4175/?route=${route}&state=idle`,{waitUntil:'networkidle0'});
 await page.screenshot({path:`screenshots/desktop-review/${route}-${theme}-${width}.png`});
 const measure=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth,heading:document.querySelector('h1')?.textContent,content:document.querySelector('.content')?.scrollHeight}));console.log(theme,width,route,JSON.stringify(measure));
}
for (const state of ['default','manual','blocked','off']) {
 await page.setViewport({width:860,height:620});
 await page.emulateMediaFeatures([{name:'prefers-color-scheme',value:'dark'}]);
 await page.goto(`http://127.0.0.1:4175/?state=${state}`,{waitUntil:'networkidle0'});
 await page.screenshot({path:`screenshots/desktop-review/awake-state-${state}.png`});
 console.log('STATE',state,await page.$eval('h1',el=>el.textContent));
}
console.log('PAGE_ERRORS',JSON.stringify(errors));await browser.close();server.close();
