// Preview / apply Ember's per-agent hook installs over CDP (port 9333).
//
//   node scripts/ember-install.mjs preview codex opencode antigravity
//   node scripts/ember-install.mjs apply   codex opencode antigravity
//
// Every apply writes a <file>.ember-backup-<ts> first and merges; removal is
// `apply <agent>` with op "remove" (edit below if needed).
const DEBUG = 'http://127.0.0.1:9333';

const mode = process.argv[2];
const agents = process.argv.slice(3);
if (!['preview', 'apply'].includes(mode) || agents.length === 0) {
  console.error('usage: node scripts/ember-install.mjs preview|apply <agent...>');
  process.exit(2);
}

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

for (const agent of agents) {
  let res;
  try {
    res = await invoke(mode === 'apply' ? 'apply_install' : 'preview_install', {
      agent,
      op: 'install',
    });
  } catch (e) {
    console.log(`\n=== ${agent} :: ${mode} FAILED ===\n${String(e).slice(0, 600)}`);
    continue;
  }
  console.log(`\n=== ${agent} :: ${mode} ===`);
  if (mode === 'apply') {
    console.log(`ok=${res.ok} installed=${res.installed} needs_repair=${res.needs_repair}`);
    console.log(`message: ${res.message}`);
    console.log(`file: ${res.file}`);
    console.log(`backup: ${res.backup}`);
  } else {
    console.log(
      `supported=${res.supported} installed=${res.installed} creates_file=${res.creates_file} format=${res.format}`,
    );
    console.log(`file: ${res.file}`);
    console.log(`summary: ${res.summary}`);
    if (res.note) console.log(`note: ${res.note}`);
  }
  const diff = res.diff ?? [];
  const nonContext = diff.filter((l) => l.kind !== 'context');
  const adds = nonContext.filter((l) => l.kind === 'add').length;
  const removes = nonContext.filter((l) => l.kind === 'remove').length;
  console.log(`diff: ${diff.length} lines (${adds} added, ${removes} removed)`);
  for (const l of nonContext.slice(0, 120)) {
    console.log(`${l.kind === 'add' ? '+' : '-'} ${l.text}`);
  }
  if (nonContext.length > 120) console.log(`   ... ${nonContext.length - 120} more`);
}

ws.close();
console.log('\nINSTALL_OK');
