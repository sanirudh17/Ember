// Dump every live registry session with enough detail to see HOW each one is
// tracked (hook vs process), and whether anything lingers after its process dies.
import { spawn } from 'node:child_process';

const DEBUG = 'http://127.0.0.1:9333';
const HOOK_EXE = 'C:/Users/sanir/Windows Tool/src-tauri/target/debug/ember.exe';

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

const s = await invoke('get_snapshot');

console.log('engaged:', s.engaged, '| want:', s.want_awake, '| working:', s.agents_working, '| watched:', s.watched_agents);
console.log('\nSESSIONS:');
for (const x of s.sessions) {
  console.log(
    `  ${x.agent.padEnd(12)} id=${String(x.session_id).padEnd(28)} pid=${String(x.pid).padEnd(7)} state=${String(x.state).padEnd(8)} approx=${x.approximate} stale=${x.stale} working_for=${x.working_for_secs}s folder=${x.folder}`,
  );
}

console.log('\nAGENTS:');
for (const a of s.agents) {
  console.log(`  ${a.id.padEnd(12)} installed=${a.installed} mode=${a.mode}`);
}

// Who is alive right now?
const alive = await new Promise((resolve) => {
  const p = spawn(
    'powershell.exe',
    ['-NoProfile', '-Command', '(Get-Process -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id) -join ","'],
    { windowsHide: true },
  );
  let out = '';
  p.stdout.on('data', (d) => (out += d));
  p.on('close', () => resolve(out.trim().split(',').map(Number)));
});
const liveSet = new Set(alive);
console.log('\nPID liveness for listed sessions:');
for (const x of s.sessions) {
  if (x.pid != null) console.log(`  ${x.agent} pid=${x.pid} alive=${liveSet.has(x.pid)}`);
}

// Is codex hook-instrumented on this machine? (presence of our config edits)
const fs = await import('node:fs');
const home = process.env.USERPROFILE || '';
for (const [label, file] of [
  ['codex config', `${home}\\.codex\\config.toml`],
  ['antigravity hooks', `${home}\\.gemini\\config\\hooks.json`],
  ['claude settings', `${home}\\.claude\\settings.json`],
  ['opencode plugin dir', `${home}\\.config\\opencode\\plugins`],
]) {
  try {
    const st = fs.statSync(file);
    const text = st.isDirectory() ? '' : fs.readFileSync(file, 'utf8');
    const ours = text.includes('ember') || text.toLowerCase().includes('ember.exe');
    console.log(`\n${label}: exists=${true} dir=${st.isDirectory()} ours=${ours}`);
    if (!st.isDirectory() && label !== 'opencode plugin dir') {
      const lines = text.split('\n').filter((l) => l.toLowerCase().includes('ember') || l.toLowerCase().includes('hook'));
      console.log(lines.slice(0, 12).map((l) => '   ' + l.trim()).join('\n'));
    }
    if (st.isDirectory()) {
      console.log('   entries: ' + fs.readdirSync(file).join(', '));
    }
  } catch (e) {
    console.log(`\n${label}: missing (${e.code})`);
  }
}

ws.close();
console.log('\nSESSIONS_OK');
