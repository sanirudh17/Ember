import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
const {version}=JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json','utf8'));
const local='src-tauri/target/release/bundle/nsis';
const destination=path.resolve(`research.local/public-v${version}`);fs.mkdirSync(destination,{recursive:true});
const names=[`Ember_${version}_x64-setup.exe`,`Ember_${version}_x64-setup.exe.sig`,'latest.json','SHA256SUMS.txt'];
for(const name of names){
 const url=`https://github.com/sanirudh17/Ember/releases/download/v${version}/${name}`;
 const file=path.join(destination,name);
 execFileSync('curl.exe',['--fail','--location','--retry','3','--silent','--show-error','--output',file,url]);
 const received=fs.readFileSync(file);assertEqual(received,fs.readFileSync(path.join(local,name)),name);
 console.log(crypto.createHash('sha256').update(received).digest('hex'),name);
}
const latest=execFileSync('curl.exe',['--fail','--location','--retry','3','--silent','--show-error',`https://github.com/sanirudh17/Ember/releases/latest/download/latest.json?verify=${Date.now()}`],{encoding:'utf8'});
if(JSON.parse(latest).version!==version)throw Error('Latest updater endpoint has the wrong version');
console.log('PASS public bytes, checksums and latest updater endpoint:',version);
function assertEqual(a,b,name){if(!a.equals(b))throw Error(`Public asset differs from the locally verified artifact: ${name}`);}
