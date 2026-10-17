// Captures the Ember webview through CDP (works even when PrintWindow cannot).
import { writeFileSync, mkdirSync } from 'node:fs';

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
const call = (method, params) =>
  new Promise((res) => {
    const id = ++seq;
    pending.set(id, res);
    ws.send(JSON.stringify({ id, method, params }));
  });

await call('Page.enable', {});
// Tall viewport so the session table below the fold is included.
await call('Emulation.setDeviceMetricsOverride', {
  width: 1920,
  height: 1500,
  deviceScaleFactor: 1,
  mobile: false,
});
await call('Runtime.evaluate', {
  expression:
    "(() => { for (const el of document.querySelectorAll('*')) { if (el.scrollHeight > el.clientHeight + 100 && el.clientHeight > 300) { el.scrollTop = el.scrollHeight; return 'scrolled:' + el.className; } } return 'no-scroll'; })()",
  returnByValue: true,
});
const shot = await call('Page.captureScreenshot', { format: 'png' });
if (!shot.result?.data) throw new Error('screenshot failed: ' + JSON.stringify(shot.error));
mkdirSync('screenshots', { recursive: true });
const out = 'screenshots/folder-fix.png';
writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
ws.close();
console.log('SHOT_OK ' + out);
