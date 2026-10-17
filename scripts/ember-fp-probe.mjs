// Baseline/after probe for process-detection reaction times, using a fake
// `codex.exe` (a copy of node) that burns CPU on demand.
//
// Scenarios:
//   C  short helper: burn 8s, exit            -> old code: row flashes working then vanishes
//   A  steady agent: burn 12s, quiet 35s, exit -> measures appear/working/idle/remove timing
//   B  restart seam: burn 20s, exit, +1.5s, burn 20s -> stop should be visible between runs
//
// Prints one transition table; run before and after the fix to compare.
import { spawn } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const DEBUG = 'http://127.0.0.1:9333';
const FOLDER = 'ember-fake-agent';

const base = join(process.env.TEMP || 'C:\\Windows\\Temp', FOLDER);
mkdirSync(base, { recursive: true });
const fake = join(base, 'codex.exe');
copyFileSync(process.execPath, fake);

const burn = (ms) =>
  `const t0=Date.now(); let x=0; while(Date.now()-t0<${ms}) { x=Math.sqrt(x+1e6)+1e6; }`;

const targets = await (await fetch(`${DEBUG}/json`)).json();
const page = targets.find((t) => t.type === 'page' && t.url.includes('localhost:1420'));
if (!page) throw new Error('no app page target; is the app running with CDP?');

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((res, rej) => {
  ws.onopen = res;
  ws.onerror = rej;
});
let seq = 0;
const pending = new Map();
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
};
const send = (method, params = {}) =>
  new Promise((res) => {
    const id = ++seq;
    pending.set(id, res);
    ws.send(JSON.stringify({ id, method, params }));
  });
const evl = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.result?.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 400));
  return r.result?.result?.value;
};
const snapshot = () => evl("window.__TAURI_INTERNALS__.invoke('get_snapshot')");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const t0 = Date.now();

// per-pid transition record
const seen = new Map(); // pid -> { appear, working, idle, gone, label }
const live = new Set();

const rows = async () => {
  const s = await snapshot();
  return s.sessions.filter((x) => x.folder === FOLDER && x.agent === 'codex');
};

async function sample(label) {
  const cur = await rows();
  const now = Date.now();
  const curPids = new Set(cur.map((r) => r.pid));
  for (const r of cur) {
    let rec = seen.get(r.pid);
    if (!rec) {
      rec = { label, appear: now, working: null, idle: null, gone: null };
      seen.set(r.pid, rec);
    }
    if (r.state === 'working' && rec.working === null) rec.working = now;
    if (r.state === 'idle' && rec.working !== null && rec.idle === null) rec.idle = now;
    live.add(r.pid);
  }
  for (const pid of live) {
    const rec = seen.get(pid);
    if (!rec) continue;
    if (!curPids.has(pid) && rec.gone === null && now - rec.appear > 1000) {
      rec.gone = now;
    }
  }
}

const spin = (label, ms, tailMs = 0) => {
  const args = ['-e', tailMs > 0 ? `${burn(ms)}; setTimeout(()=>{}, ${tailMs});` : burn(ms)];
  const child = spawn(fake, args, { cwd: base, windowsHide: true, stdio: 'ignore' });
  const rec = { label, child };
  return rec;
};

const slept = async (sinceMs, ms) => {
  while (Date.now() - t0 < sinceMs + ms) {
    await sample('?');
    await sleep(500);
  }
};

console.log('scenario C: 8s helper (target: must never be reported)');
const c = spin('C', 8000);
await slept(0, 14000);

console.log('scenario A: 12s burn + 35s quiet (alive), then exit');
const a = spin('A-long', 12000, 35000);
c.child.kill();
await slept(14000, 56000);
a.child.kill();
await slept(70000, 8000);

console.log('scenario B: restart seam (20s burn, exit, +1.5s, 20s burn)');
const b1 = spin('B1', 20000);
await slept(78000, 23500);
b1.child.kill();
await sleep(1500);
const b2 = spin('B2', 20000);
await slept(103000, 26000);
b2.child.kill();
await sleep(2500);

// label each record accurately by spawn windows
const labelFor = (appear) => {
  const rel = appear - t0;
  if (rel < 14000) return 'C(8s helper)';
  if (rel < 70000) return 'A(12s+quiet)';
  if (rel < 103000) return 'B1(20s)';
  return 'B2(20s)';
};

console.log('\npid      scenario        appear     working    idle       gone');
for (const [pid, rec] of seen) {
  const lab = labelFor(rec.appear);
  const f = (t) => (t === null ? '   --    ' : `${((t - rec.appear) / 1000).toFixed(1)}s`.padStart(6));
  const g = (t) => (t === null ? '   --  ' : `${((t - t0) / 1000).toFixed(1)}s`.padStart(6));
  console.log(
    `${String(pid).padEnd(8)} ${lab.padEnd(15)} ${f(rec.appear).padStart(6)}  ${f(rec.working)}  ${f(rec.idle)}  ${rec.gone === null ? '  --  ' : ((rec.gone - t0) / 1000).toFixed(1) + 's'} (t=${g(rec.appear)})`,
  );
}

ws.close();
console.log('\nFP_PROBE_DONE');
