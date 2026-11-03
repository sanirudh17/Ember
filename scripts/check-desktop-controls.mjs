import puppeteer from 'puppeteer-core';
import assert from 'node:assert/strict';
const browser=await puppeteer.launch({executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true});
try {
 const page=await browser.newPage();await page.setViewport({width:920,height:640});
 await page.goto('http://localhost:1420/?state=idle',{waitUntil:'networkidle0'});
 assert(await page.evaluate(()=>getComputedStyle(document.querySelector('.topbar')).backgroundColor===getComputedStyle(document.querySelector('.sidebar')).backgroundColor),'titlebar and sidebar must share a background');
 const buttonCenters=await page.$$eval('.btn',buttons=>buttons.map(b=>{const a=b.getBoundingClientRect(),s=b.querySelector('.control-label')?.getBoundingClientRect();return s?Math.abs((a.top+a.bottom-s.top-s.bottom)/2):999;}));
 assert(buttonCenters.every(d=>d<=1),'button label boxes must be centered');
 await page.click('.sidebar button:nth-child(3)');await page.waitForSelector('.page--settings');
 assert.equal(await page.$eval('.navlink.is-active',el=>getComputedStyle(el).boxShadow),'none');
 const before=await page.$eval('[role=switch][aria-label="Launch at login"]',el=>el.getAttribute('aria-checked'));
 await page.click('[role=switch][aria-label="Launch at login"]');
 await page.waitForFunction(old=>document.querySelector('[aria-label="Launch at login"]').getAttribute('aria-checked')!==old,{},before);
 await page.evaluate(()=>[...document.querySelectorAll('[role=radiogroup][aria-label="Appearance"] button')].find(b=>b.textContent.trim()==='Dark').click());
 await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');
 await page.click('.sidebar button:first-child');await page.waitForSelector('.page--awake');
 await page.evaluate(()=>[...document.querySelectorAll('.actions button')].find(b=>b.textContent.trim()==='30m').click());
 await page.waitForFunction(()=>document.querySelector('.actions')?.textContent.includes('End hold'));
 await page.evaluate(()=>[...document.querySelectorAll('.actions button')].find(b=>b.textContent.trim()==='End hold').click());
 await page.waitForFunction(()=>document.querySelector('.actions')?.textContent.includes('30m'));
 await page.evaluate(()=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='View agents').click());await page.waitForFunction(()=>document.querySelector('h1')?.textContent.trim()==='Agents');
 console.log('PASS: centered button boxes, no tab underline, sidebar routing, setting toggle, manual start/end');
} finally {await browser.close();}
