// Live Codex hook probe: runs a real `codex exec` and watches Ember's
// registry for a hook-managed (approximate=false) codex session.
//
//   node scripts/ember-codex-probe.mjs [prompt]
//
// Requires: codex hooks applied in ~/.codex/config.toml + the app running.
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const DEBUG = 'http://127.0.0.1:9333';
const PROMPT = process.argv[2] ?? 'Reply with exactly: ember-probe-ok';
const WORKDIR = join(tmpdir(), 'ember-codex-probe');

mkdirSync(WORKDIR, { recursive: true });

// 1. CDP plumbing.
const targets = await (await fetch(`${DEBUG}/json`)).json();
const page = targets.find((t) => t.type === 'page' && t.url.includes('localhost:1420'));
if (!page) throw new Error('no app page target (is the app running?)');
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
const invoke = (cmd, args) => evl(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args ?? {})})`);

const codexSessions = async () => {
  const s = await invoke('get_snapshot');
  return s.sessions.filter((x) => x.agent === 'codex');
};
const key = (x) => `${x.session_id}|${x.state}|${x.approximate}|${x.stale}`;

// 2. Baseline.
const before = new Set((await codexSessions()).map(key));
console.log(`baseline codex sessions: ${before.size}`);

// 3. Run codex exec in a throwaway cwd.
console.log(`starting: codex exec (cwd=${WORKDIR})`);
// shell: true on Windows joins args unescaped; quote the prompt as one word.
const EXTRA = process.env.EMBER_CODEX_PROBE_FLAGS ?? '';
const child = spawn(`codex.cmd exec ${EXTRA} --skip-git-repo-check "${PROMPT}"`, {
  cwd: WORKDIR,
  shell: true,
  windowsHide: true,
});
// `codex exec` reads piped stdin until EOF before starting its turn; with an
// open pipe it stalls. Close our end so it sees EOF immediately.
child.stdin.end();
let out = '';
child.stdout.on('data', (d) => (out += d));
child.stderr.on('data', (d) => (out += d));

let sawHookWorking = false;
let sawHookSession = false;
const deadline = Date.now() + 240_000;
while (Date.now() < deadline) {
  const now = await codexSessions();
  const fresh = now.filter((x) => !before.has(key(x)));
  const hooks = fresh.filter((x) => !x.approximate);
  for (const x of hooks) {
    sawHookSession = true;
    if (x.state === 'working') sawHookWorking = true;
    console.log(
      `  hook session: id=${x.session_id} state=${x.state} pid=${x.pid} folder=${x.folder} stale=${x.stale}`,
    );
  }
  // SessionStart + UserPromptSubmit both observed — the pipeline is proven;
  // don't wait for the (possibly slow) model turn to finish.
  if (sawHookSession && sawHookWorking) break;
  if (child.exitCode !== null) break;
  await new Promise((r) => setTimeout(r, 700));
}

if (child.exitCode === null) child.kill();
await new Promise((r) => setTimeout(r, 1500));

console.log(`\ncodex exit: ${child.exitCode}`);
console.log(`hook session appeared: ${sawHookSession}`);
console.log(`hook session went working: ${sawHookWorking}`);
console.log('--- codex output (tail) ---');
console.log(out.split('\n').slice(-12).join('\n'));
ws.close();
console.log(sawHookSession ? '\nCODEX_PROBE_OK' : '\nCODEX_PROBE_NO_HOOKS');
process.exit(sawHookSession ? 0 : 3);
