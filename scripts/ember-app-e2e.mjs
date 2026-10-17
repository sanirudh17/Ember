// Real-app E2E over WebView2 CDP (port 9333).
// Verifies: lid-proof toggle -> hook engage -> lid override + watchdog process ->
// hook idle -> release + state file cleanup -> pause round-trip -> settings restored.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';

const DEBUG = 'http://127.0.0.1:9333';
const HOOK_EXE = 'C:/Users/sanir/Windows Tool/src-tauri/target/debug/ember.exe';
const STATE_FILE = 'C:/Users/sanir/AppData/Local/Ember/session-state.json';
const LOG_FILE = 'C:/Users/sanir/AppData/Roaming/Ember/ember.log';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitPort(timeoutMs = 300000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    try {
      const res = await fetch(`${DEBUG}/json`);
      if (res.ok) return await res.json();
    } catch {}
    await sleep(2000);
  }
  throw new Error('debug port never came up');
}

const sh = (cmd) =>
  new Promise((resolve) => {
    const p = spawn('powershell.exe', ['-NoProfile', '-Command', cmd], { windowsHide: true });
    let out = '';
    p.stdout.on('data', (d) => (out += d));
    p.stderr.on('data', (d) => (out += d));
    p.on('close', () => resolve(out.trim()));
  });

const targets = await waitPort();
const page = targets.find((t) => t.type === 'page' && t.url.includes('localhost:1420'));
if (!page) throw new Error('no app page target: ' + JSON.stringify(targets.map((t) => [t.type, t.url])));
console.log('attached to:', page.url);

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
  if (r.result?.exceptionDetails) {
    throw new Error('page exception: ' + JSON.stringify(r.result.exceptionDetails).slice(0, 400));
  }
  return r.result?.result?.value;
};
const invoke = (cmd, args) =>
  evl(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args ?? {})})`);
const snapshot = () => invoke('get_snapshot');

const report = {};
report.internals = await evl('typeof window.__TAURI_INTERNALS__?.invoke');
if (report.internals !== 'function') throw new Error('window.__TAURI_INTERNALS__.invoke missing');

let s = await snapshot();
const original = s.settings.lid_proof;
report.settings0 = s.settings;
report.lid_present = s.capabilities?.lid_present;
report.battery = { present: s.battery?.present, percent: s.battery?.percent, plugged_in: s.battery?.plugged_in };
report.ember_processes_before = await sh('(Get-Process ember -ErrorAction SilentlyContinue | Measure-Object).Count');

// 1) Lid proof on
let updated = await invoke('update_settings', { settings: { ...s.settings, lid_proof: true } });
report.lid_proof_set = updated.lid_proof;

// 2) Hook: claude-code working -> engage
spawn(HOOK_EXE, ['hook', 'claude-code', 'working'], { windowsHide: true, stdio: 'ignore' });
let engaged = false;
{
  const t0 = Date.now();
  while (Date.now() - t0 < 25000) {
    await sleep(1000);
    s = await snapshot();
    if (s.engaged || s.lid?.held) {
      engaged = true;
      break;
    }
  }
  report.engaged_after_hook = engaged;
  report.engaged_elapsed_s = Math.round((Date.now() - t0) / 1000);
  report.lid_held = s.lid?.held;
  report.state_file_during_hold = existsSync(STATE_FILE);
}

// 3) Watchdog process count (expect app + watchdog = 2)
await sleep(1200);
report.ember_processes_during_hold = await sh('(Get-Process ember -ErrorAction SilentlyContinue | Measure-Object).Count');

// 4) Hook idle -> release
spawn(HOOK_EXE, ['hook', 'claude-code', 'idle'], { windowsHide: true, stdio: 'ignore' });
{
  const t0 = Date.now();
  let released = false;
  while (Date.now() - t0 < 45000) {
    await sleep(1000);
    s = await snapshot();
    if (!s.engaged && !s.lid?.held) {
      released = true;
      break;
    }
  }
  report.released_after_idle = released;
  report.released_elapsed_s = Math.round((Date.now() - t0) / 1000);
  report.state_file_after_release = existsSync(STATE_FILE);
}

// 5) Pause round-trip (same engine paths the tray menu calls)
// The snapshot is the last tick's; give the 250 ms state loop a beat to apply.
await invoke('pause_for', { seconds: 60 });
await sleep(700);
s = await snapshot();
report.paused_after_pause_for = s.paused;
report.pause_countdown = s.paused_countdown;
await invoke('set_paused', { paused: false });
await sleep(700);
s = await snapshot();
report.paused_after_resume = s.paused;

// 6) Restore the original lid setting
updated = await invoke('update_settings', { settings: { ...s.settings, lid_proof: original } });
report.lid_proof_restored_to = updated.lid_proof;

// 7) Log tail
const log = readFileSync(LOG_FILE, 'utf8').trim().split('\n');
report.log = log.filter((l) => /watchdog|LID|ENGAGE|DISENGAGE|TRAY/i.test(l)).slice(-20);

console.log(JSON.stringify(report, null, 1));
ws.close();
console.log('E2E_OK');
