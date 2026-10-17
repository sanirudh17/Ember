// Wave-4 pass B: scroll the sessions tables into view and capture crisp zooms (dark + light).
import { Browser, sleep } from 'file:///C:/Users/sanir/.agents/skills/design-taste-frontend/scripts/lib/cdp.mjs';

const BASE = 'http://localhost:1420';
const OUT = 'C:/Users/sanir/Windows Tool/screenshots';

const browser = await Browser.launch({ width: 1440, height: 900 });
const page = await browser.newPage({ width: 1440, height: 900 });

async function captureSessions(route, tag) {
  await page.goto(`${BASE}/${route}`, { settleMs: 1700 });
  const rect = await page.eval(() => {
    const el = document.querySelector('.row--working');
    if (!el) return null;
    el.scrollIntoView({ block: 'center' });
    return null;
  });
  await sleep(900);
  const geo = await page.eval(() => {
    const el = document.querySelector('.row--working');
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, w: r.width, h: r.height };
  });
  await page.screenshot({ path: `${OUT}/verify-${tag}-sessions.png`, format: 'png' });
  if (geo) {
    await page.screenshot({
      path: `${OUT}/verify-${tag}-sessions-zoom.png`,
      format: 'png',
      clip: { x: Math.max(0, geo.x - 20), y: Math.max(0, geo.y - 110), width: Math.min(1440 - Math.max(0, geo.x - 20), geo.w + 40), height: 420, scale: 2 },
    }).catch(() => {});
  }
  return geo;
}

await captureSessions('?route=agents', 'agents');
// Light theme sanity check on the same table.
await page.eval(() => document.documentElement.setAttribute('data-theme', 'light'));
await sleep(400);
await page.screenshot({ path: `${OUT}/verify-agents-sessions-light.png`, format: 'png' });
await page.eval(() => document.documentElement.setAttribute('data-theme', 'dark'));
await captureSessions('/', 'home');

await browser.close();
console.log('done');
