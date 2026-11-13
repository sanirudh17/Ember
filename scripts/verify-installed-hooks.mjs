// Repair temporary release-test paths using the installed application's normal
// backup-and-merge installer, then inspect metadata without logging user config.
import puppeteer from 'puppeteer-core';
import assert from 'node:assert/strict';
import fs from 'node:fs';
const browser=await puppeteer.connect({browserURL:'http://127.0.0.1:9333',defaultViewport:null});
const page=(await browser.pages()).find(p=>p.url().includes('tauri.localhost'));
const invoke=(command,args={})=>page.evaluate((command,args)=>window.__TAURI_INTERNALS__.invoke(command,args),command,args);
try {
 assert.equal(await invoke('app_version'),'1.2.0');
 const snapshot=await invoke('get_snapshot');
 assert.equal(snapshot.agents.length,7);
 const results=[];
 for(const agent of snapshot.agents){
  assert(agent.supported,agent.id+' must expose lifecycle hooks');
  if(!agent.detected&&!agent.installed){results.push({agent:agent.id,status:'not installed on this device; integration available'});continue;}
  const preview=await invoke('preview_install',{agent:agent.id,op:'install'});
  assert(preview.can_install||preview.can_repair||preview.installed,agent.id+' policy conflict: '+preview.note);
  const result=await invoke('apply_install',{agent:agent.id,op:'install'});
  assert(result.ok&&result.installed&&!result.needs_repair,agent.id+': '+result.message);
  const check=await invoke('preview_install',{agent:agent.id,op:'install'});
  assert(check.installed&&!check.needs_repair,agent.id+' must pass installed-runtime readback');
  results.push({agent:agent.id,status:'installed and verified',backupCreated:!!result.backup});
 }
 fs.writeFileSync('research.local/installed-hooks-proof.json',JSON.stringify(results,null,2));
 console.log('PASS installed-runtime hook configuration:',results);
 await invoke('show_window');
}finally{browser.disconnect();}
