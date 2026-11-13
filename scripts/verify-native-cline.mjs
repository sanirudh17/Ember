// Execute the installed Windows PowerShell hooks against the real packaged app.
import puppeteer from 'puppeteer-core';import {spawnSync} from 'node:child_process';import path from 'node:path';import assert from 'node:assert/strict';
const b=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});const p=(await b.pages()).find(p=>p.url().includes('tauri.localhost'));
const invoke=(c,a={})=>p.evaluate((c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),c,a);
const install=await invoke('apply_install',{agent:'cline',op:'install'});assert(install.ok&&!install.needs_repair);const folder=install.file;
const sid='cline-powershell-release-proof';
const fire=(hook,data={})=>{const r=spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',path.join(folder,hook+'.ps1')],{input:JSON.stringify({taskId:sid,workspaceRoots:['C:/ember-release-proof'],...data}),windowsHide:true,timeout:5000,encoding:'utf8'});assert.equal(r.status,0,r.stderr);assert.deepEqual(JSON.parse(r.stdout.trim()),{cancel:false});};
const until=async(expected)=>{for(let n=0;n<30;n++){const s=(await invoke('get_snapshot')).sessions.find(s=>s.agent==='cline'&&s.session_id===sid);if(s?.state===expected)return s;await new Promise(r=>setTimeout(r,100));}throw Error('Cline state did not become '+expected);};
try{
 fire('TaskStart');const live=await until('working');assert(!live.approximate);
 fire('PreToolUse',{preToolUse:{toolName:'ask_followup_question'}});await until('idle');
 fire('PostToolUse',{postToolUse:{toolName:'bash'}});await until('idle');
 fire('PostToolUse',{postToolUse:{toolName:'ask_followup_question'}});await until('working');
 fire('Notification',{notification:{waitingForUserInput:true}});await until('idle');
 fire('UserPromptSubmit',{userPromptSubmit:{prompt:''}});await until('working');
 fire('TaskCancel');fire('PostToolUse',{postToolUse:{toolName:'ask_followup_question'}});await until('idle');
 fire('TaskResume');await until('working');fire('TaskComplete');await until('idle');
 fire('SessionShutdown');await until(undefined);
 console.log('PASS actual Cline .ps1 -> helper -> pipe: start, tools, questions, reply, cancel, resume, completion, shutdown and nonblocking JSON');
}finally{fire('SessionShutdown');b.disconnect();}
