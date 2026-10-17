// Functional probe for the INSTALLED OpenCode plugin: load the real
// ~/.config/opencode/plugins/ember-hooks.js, drive it with fake lifecycle
// events, and assert Ember's registry reflects every transition.
// Requires: the app running (CDP 9333) + the plugin installed.
//
//   node scripts/ember-plugin-probe.mjs
import { copyFileSync, mkdirSync, existsSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const DEBUG = 'http://127.0.0.1:9333';
const INSTALLED = join(homedir(), '.config', 'opencode', 'plugins', 'ember-hooks.js');
const PROBE_ID = 'ember-probe-1';
const CWD = 'C:\\Users\\sanir\\Windows Tool';

if (!existsSync(INSTALLED)) {
  console.error(`not installed: ${INSTALLED}`);
  process.exit(2);
}

// 1. Load the installed plugin as ESM (copy to .mjs so Node parses it as a module).
const dir = join(tmpdir(), 'ember-plugin-probe');
mkdirSync(dir, { recursive: true });
const mjs = join(dir, 'ember-hooks.mjs');
copyFileSync(INSTALLED, mjs);
const mod = await import(pathToFileURL(mjs).href + `?t=${Date.now()}`);
const factory = mod.EmberHooks ?? mod.default;
if (typeof factory !== 'function') throw new Error('plugin does not export a factory function');
const hooks = await factory({ directory: CWD, worktree: CWD });
console.log('plugin loaded; hooks:', Object.keys(hooks).join(', '));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 2. CDP plumbing.
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

const session = async () => {
  const s = await invoke('get_snapshot');
  return s.sessions.find((x) => x.agent === 'opencode' && x.session_id === PROBE_ID) ?? null;
};
const expect = async (label, wantState, { wantPresent = true } = {}) => {
  for (let i = 0; i < 24; i++) {
    const x = await session();
    if (!wantPresent) {
      if (!x) {
        console.log(`ok   ${label}: session gone`);
        return;
      }
    } else if (x && x.state === wantState) {
      console.log(`ok   ${label}: state=${x.state} approx=${x.approximate} stale=${x.stale} folder=${x.folder}`);
      return;
    }
    await sleep(250);
  }
  const x = await session();
  throw new Error(
    `FAIL ${label}: wanted ${wantPresent ? wantState : 'absent'}, saw ${x ? JSON.stringify({ state: x.state, approx: x.approximate, folder: x.folder }) : 'absent'}`,
  );
};

const fire = async (event) => {
  await hooks.event({ event });
  await sleep(150);
};
const fireTool = async (input) => {
  await hooks['tool.execute.before'](input);
  await sleep(150);
};

// 3. The lifecycle, exactly as OpenCode v2.0.29 emits it.
await fire({ type: 'session.created', properties: { sessionID: PROBE_ID } });
await expect('session.created -> idle row appears', 'idle');

await fire({ type: 'session.status', properties: { sessionID: PROBE_ID, status: { type: 'busy' } } });
await expect('session.status busy -> working', 'working');

await fire({ type: 'session.next.tool.called', properties: { sessionID: PROBE_ID } });
await expect('session.next.tool.called -> working', 'working');

await fireTool({ sessionID: PROBE_ID });
await expect('tool.execute.before -> working', 'working');

await fire({ type: 'session.status', properties: { sessionID: PROBE_ID, status: { type: 'idle' } } });
await expect('session.status idle -> idle', 'idle');

await fire({ type: 'session.status', properties: { sessionID: PROBE_ID, status: { type: 'retry' } } });
await expect('session.status retry -> working', 'working');

await fire({ type: 'session.execution.succeeded', properties: { sessionID: PROBE_ID } });
await expect('session.execution.succeeded -> idle', 'idle');

// A recoverable compaction error must NOT flip the row idle.
await fire({ type: 'session.status', properties: { sessionID: PROBE_ID, status: { type: 'busy' } } });
await fire({ type: 'session.error', properties: { sessionID: PROBE_ID } });
await expect('session.error keeps working (compaction-safe)', 'working');

await fire({ type: 'session.idle', properties: { sessionID: PROBE_ID } });
await expect('session.idle (deprecated) -> idle', 'idle');

await fire({ type: 'session.deleted', properties: { sessionID: PROBE_ID } });
await expect('session.deleted -> gone', null, { wantPresent: false });

console.log('\nPROBE_OK');
ws.close();
process.exit(0);
