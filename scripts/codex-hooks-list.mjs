// Ask a fresh `codex app-server` what it thinks of the hooks in
// ~/.codex/config.toml: trust status + current hash + key + enabled, exactly
// as the TUI's hooks review screen would see them.
//
//   node scripts/codex-hooks-list.mjs [cwd]
import { spawn } from 'node:child_process';

const CWD = process.argv[2] ?? 'C:\\Users\\sanir\\Windows Tool';

const child = spawn('codex.cmd', ['app-server', '--listen', 'stdio://'], {
  cwd: CWD,
  shell: true,
  windowsHide: true,
});

let buf = '';
const pending = new Map();
const send = (obj) => child.stdin.write(JSON.stringify(obj) + '\n');

child.stdout.on('data', (d) => {
  buf += d.toString();
  let idx;
  while ((idx = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, idx).trim();
    buf = buf.slice(idx + 1);
    if (!line) continue;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      console.log(`[stdout non-JSON] ${line.slice(0, 200)}`);
      continue;
    }
    if (msg.id !== undefined && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    } else if (msg.id !== undefined && msg.method) {
      console.log(`[server request] ${msg.method}`);
      send({ id: msg.id, error: { code: -32601, message: 'probe does not support this' } });
    } else if (msg.method) {
      console.log(`[notification] ${msg.method}`);
    }
  }
});
child.stderr.on('data', (d) => {
  const s = d.toString().trim();
  if (s) console.log(`[stderr] ${s.slice(0, 300)}`);
});

const request = (id, method, params) =>
  new Promise((resolve) => {
    pending.set(id, resolve);
    send({ id, method, params });
  });

const failSafe = setTimeout(() => {
  console.log('timed out waiting for app-server');
  child.kill();
  process.exit(2);
}, 30_000);

const init = await request(1, 'initialize', {
  clientInfo: { name: 'ember-hooks-probe', version: '0.1.0' },
});
console.log(`initialize -> ${JSON.stringify(init).slice(0, 200)}`);

const resp = await request(2, 'hooks/list', { cwds: [CWD] });
clearTimeout(failSafe);

const data = resp?.data ?? resp?.result?.data;
if (!data) {
  console.log('unexpected hooks/list response:');
  console.log(JSON.stringify(resp, null, 2).slice(0, 4000));
} else {
  for (const entry of data) {
    console.log(`\n== cwd: ${entry.cwd}  (${entry.hooks.length} hooks)`);
    for (const w of entry.warnings ?? []) console.log(`  warning: ${w}`);
    for (const e of entry.errors ?? []) console.log(`  error: ${e.path}: ${e.message}`);
    for (const h of entry.hooks) {
      console.log(
        `  ${h.eventName.padEnd(17)} trust=${(h.trustStatus ?? '?').padEnd(9)} enabled=${h.enabled} src=${h.source}`,
      );
      console.log(`    key=${h.key}`);
      console.log(`    hash=${h.currentHash.slice(0, 40)}...`);
      console.log(`    cmd=${(h.command ?? '').slice(0, 90)}`);
    }
  }
}

child.kill();
process.exit(0);
