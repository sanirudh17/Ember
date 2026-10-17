// Wave-4 verification: measures the four fixes in a real browser against the dev server.
//   1. topbar border-radius
//   2. switch thumb centering (off + on)
//   3. no zebra table rows; working row wash
//   4. working vs idle sdot
// Usage: node scripts/verify-wave4.mjs
import { Browser, sleep } from 'file:///C:/Users/sanir/.agents/skills/design-taste-frontend/scripts/lib/cdp.mjs';
import { writeFileSync } from 'node:fs';

const BASE = 'http://localhost:1420';
const OUT = 'C:/Users/sanir/Windows Tool/screenshots';

const browser = await Browser.launch({ width: 1440, height: 900 });
const results = {};

async function tableData(page) {
  return page.eval(() => {
    const cs = (el) => getComputedStyle(el);
    const rect = (el) => { const r = el.getBoundingClientRect(); return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) }; };
    const out = {};
    const tb = document.querySelector('.topbar');
    if (tb) out.topbar = { radius: cs(tb).borderRadius, rect: rect(tb) };
    out.rows = [...document.querySelectorAll('.table tbody tr')].map((r) => ({
      cls: r.className || '(none)',
      bg: cs(r).backgroundColor,
      label: r.querySelector('.state-label')?.textContent || null,
      first: (r.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 46),
      rect: rect(r),
    }));
    out.sdots = [...document.querySelectorAll('.sdot')].map((s) => {
      const c = cs(s);
      return { cls: s.className, bg: c.backgroundColor, border: `${c.borderTopWidth} ${c.borderTopStyle} ${c.borderTopColor}`, shadow: c.boxShadow.slice(0, 90), rect: rect(s) };
    });
    out.labels = [...document.querySelectorAll('.state-label')].map((l) => ({ text: l.textContent, color: cs(l).color }));
    return out;
  });
}

async function switchData(page) {
  return page.eval(() => [...document.querySelectorAll('.switch')].map((el) => {
    const thumb = el.querySelector('.switch__thumb');
    const er = el.getBoundingClientRect();
    const tr = thumb.getBoundingClientRect();
    return {
      on: el.classList.contains('is-on'),
      w: +er.width.toFixed(2), h: +er.height.toFixed(2),
      thumb: { w: +tr.width.toFixed(2), h: +tr.height.toFixed(2) },
      gaps: { l: +(tr.left - er.left).toFixed(2), r: +(er.right - tr.right).toFixed(2), t: +(tr.top - er.top).toFixed(2), b: +(er.bottom - tr.bottom).toFixed(2) },
    };
  }));
}

// --- Agents page -----------------------------------------------------------
{
  const page = await browser.newPage({ width: 1440, height: 900 });
  await page.goto(`${BASE}/?route=agents`, { settleMs: 1600 });
  results.agents = await tableData(page);
  await page.screenshot({ path: `${OUT}/verify-agents.png`, format: 'png' });
  const top = results.agents.topbar?.rect;
  if (top) await page.screenshot({ path: `${OUT}/verify-topbar-zoom.png`, format: 'png', clip: { x: 0, y: 0, width: 1440, height: Math.max(110, top.y + top.h + 24), scale: 2 } }).catch(() => {});
  const firstRow = results.agents.rows[0]?.rect;
  if (firstRow) await page.screenshot({ path: `${OUT}/verify-rows-zoom.png`, format: 'png', clip: { x: 0, y: Math.max(0, firstRow.y - 60), width: 1440, height: 420, scale: 2 } }).catch(() => {});
  await page.close();
}

// --- Home page -------------------------------------------------------------
{
  const page = await browser.newPage({ width: 1440, height: 900 });
  await page.goto(`${BASE}/`, { settleMs: 1600 });
  results.home = await tableData(page);
  await page.screenshot({ path: `${OUT}/verify-home.png`, format: 'png' });
  await page.close();
}

// --- Settings page: switches -----------------------------------------------
{
  const page = await browser.newPage({ width: 1440, height: 900 });
  await page.goto(`${BASE}/?route=settings`, { settleMs: 1600 });
  const before = await switchData(page);
  // Toggle the first switch to sample the opposite state, then restore.
  await page.eval(() => document.querySelector('.switch').click());
  await sleep(600);
  const after = await switchData(page);
  await page.eval(() => document.querySelector('.switch').click());
  await sleep(400);
  results.switches = { before, afterFirstToggle: after[0] };
  const swRect = await page.eval(() => {
    const el = document.querySelector('.switch');
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y };
  });
  await page.screenshot({ path: `${OUT}/verify-settings.png`, format: 'png' });
  await page.screenshot({ path: `${OUT}/verify-switches-zoom.png`, format: 'png', clip: { x: Math.max(0, swRect.x - 260), y: Math.max(0, swRect.y - 120), width: 720, height: 340, scale: 2 } }).catch(() => {});
  await page.close();
}

await browser.close();
writeFileSync(`${OUT}/verify-wave4.json`, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
