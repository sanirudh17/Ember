// Print the per-agent detection state (what the Agents page tiles show).
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const DEBUG = 'http://127.0.0.1:9333';
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
const evl = async (expression) => {
  const r = await new Promise((res) => {
    const id = ++seq;
    pending.set(id, res);
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
  });
  if (r.result?.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 300));
  return r.result?.result?.value;
};

const s = await evl("window.__TAURI_INTERNALS__.invoke('get_snapshot')");
console.log('engaged:', s.engaged, '| working:', s.agents_working, '| watched:', s.watched_agents);
console.log('AGENTS (tiles):');
for (const a of s.agents) {
  console.log(`  ${a.id.padEnd(12)} detected=${String(a.detected).padEnd(5)} detail="${a.detect_detail}" installed=${a.installed} mode=${a.mode}`);
}
console.log('SESSIONS:');
for (const x of s.sessions) {
  console.log(`  ${x.agent.padEnd(12)} pid=${String(x.pid).padEnd(7)} ${String(x.state).padEnd(8)} working_for=${x.working_for_secs}s folder=${x.folder}`);
}
ws.close();
console.log('AGENTS_OK');
