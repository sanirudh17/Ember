// Live app probe: current snapshot summary + pause round-trip via CDP.
import { existsSync } from 'node:fs';

const DEBUG = 'http://127.0.0.1:9333';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const targets = await (await fetch(`${DEBUG}/json`)).json();
const page = targets.find((t) => t.type === 'page' && t.url.includes('localhost:1420'));
if (!page) throw new Error('no app page target');

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
  if (r.result?.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 300));
  return r.result?.result?.value;
};
const invoke = (cmd, args) => evl(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args ?? {})})`);
const snapshot = () => invoke('get_snapshot');

const brief = (s) => ({
  engaged: s.engaged,
  want_awake: s.want_awake,
  paused: s.paused,
  paused_countdown: s.paused_countdown,
  agents_working: s.agents_working,
  reason: s.reason,
  blocker: s.blocker_label,
  lid: { present: s.lid.present, proof: s.lid.proof_enabled, held: s.lid.held, closed: s.lid.closed },
  sessions: s.sessions.map((x) => ({ agent: x.agent, state: x.state, pid: x.pid, secs: x.working_for_secs })),
});

const report = {};
report.now = brief(await snapshot());

await invoke('pause_for', { seconds: 120 });
await sleep(800);
report.after_pause_for = brief(await snapshot());

await invoke('set_paused', { paused: false });
await sleep(800);
report.after_resume = brief(await snapshot());

report.state_file = existsSync('C:/Users/sanir/AppData/Local/Ember/session-state.json');

console.log(JSON.stringify(report, null, 1));
ws.close();
console.log('PROBE_OK');
