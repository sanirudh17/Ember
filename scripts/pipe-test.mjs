// Raw hook-pipe client (bypasses ember.exe) to isolate server vs client bugs.
//   node scripts/pipe-test.mjs [session_id]
import net from 'node:net';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sessionId = process.argv[2] ?? 'ember-raw-1';
const token = readFileSync(join(process.env.APPDATA, 'Ember', 'hook-token'), 'utf8').trim();
const wire = {
  v: 1,
  token,
  agent: 'opencode',
  event: 'working',
  payload: JSON.stringify({
    session_id: sessionId,
    cwd: 'C:\\Users\\sanir\\Windows Tool',
    pid: process.pid,
  }),
  pid: process.pid,
  ts: Math.floor(Date.now() / 1000),
};

const s = net.connect('\\\\.\\pipe\\ember-hooks');
s.on('connect', () => {
  s.write(JSON.stringify(wire) + '\n', () => {
    console.log('written');
    s.end();
  });
});
s.on('error', (e) => {
  console.log('ERR', e.message);
  process.exit(1);
});
s.on('close', () => {
  console.log('closed');
  process.exit(0);
});
setTimeout(() => {
  console.log('timeout');
  process.exit(2);
}, 5000).unref();
