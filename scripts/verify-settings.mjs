// Settings-page verification for the new option rows (mock harness, headless).
// Captures dark + light states, exercises the battery-floor segment and the
// keep-screen-on switch, and prints the live fact cards before/after.
import { Browser, sleep } from 'file:///C:/Users/sanir/.agents/skills/design-taste-frontend/scripts/lib/cdp.mjs';

const BASE = 'http://localhost:1420';
const OUT = 'C:/Users/sanir/Windows Tool/screenshots';

const browser = await Browser.launch({ width: 1440, height: 900 });
const page = await browser.newPage({ width: 1440, height: 900 });

const facts = () =>
  page.eval(() =>
    [...document.querySelectorAll('.fact')].map((c) => ({
      k: c.querySelector('.fact__k')?.textContent ?? '',
      v: c.querySelector('.fact__v')?.textContent ?? '',
      note: c.querySelector('.fact__note')?.textContent ?? '',
    })),
  );

const rowsText = () =>
  page.eval(() =>
    [...document.querySelectorAll('.srow')].map((r) => ({
      label: r.querySelector('.srow__label')?.textContent ?? '',
      hint: r.querySelector('.srow__hint')?.textContent ?? '',
    })),
  );

const zoomRows = async (tag) => {
  await page.eval(() => {
    document.querySelector('.page--settings .rows')?.scrollIntoView({ block: 'center' });
  });
  await sleep(600);
  const geo = await page.eval(() => {
    const el = document.querySelector('.page--settings .rows');
    if (!el) return null;
    // CDP screenshot clips are in document coordinates: add the scroll offset
    // accumulated on every scrolling ancestor so the clip lands where the
    // element actually is on the page, not where it sits in the viewport.
    let ox = 0;
    let oy = 0;
    let node = el.parentElement;
    while (node) {
      ox += node.scrollLeft || 0;
      oy += node.scrollTop || 0;
      node = node.parentElement;
    }
    const r = el.getBoundingClientRect();
    return { x: r.x + ox, y: r.y + oy, w: r.width, h: r.height };
  });
  if (geo) {
    const x = Math.max(0, geo.x - 14);
    const y = Math.max(0, geo.y - 14);
    await page
      .screenshot({
        path: `${OUT}/verify-settings-rows-${tag}.png`,
        format: 'png',
        clip: {
          x,
          y,
          width: Math.min(1440 - x, geo.w + 28),
          height: Math.min(900 - y, geo.h + 28),
          scale: 2,
        },
      })
      .catch((e) => console.error('rows zoom failed:', e.message));
  }
  return geo;
};

await page.goto(`${BASE}/?route=settings`, { settleMs: 1800 });
console.log('settings page rendered:', await page.eval(() => !!document.querySelector('.page--settings')));
console.log('rows:', JSON.stringify(await rowsText(), null, 1));
console.log('facts:', JSON.stringify(await facts(), null, 1));

await page.eval(() => document.documentElement.setAttribute('data-theme', 'dark'));
await sleep(400);
await page.screenshot({ path: `${OUT}/verify-settings-dark.png`, format: 'png' });
await zoomRows('dark');

// --- interactions: 20% battery floor + keep-the-screen-on
const clickedFloor = await page.eval(() => {
  const seg = [...document.querySelectorAll('.seg')].find(
    (s) => s.getAttribute('aria-label') === 'Battery floor',
  );
  const btn = seg && [...seg.querySelectorAll('button')].find((b) => b.textContent.trim() === '20%');
  if (btn) {
    btn.click();
    return true;
  }
  return false;
});
const clickedScreen = await page.eval(() => {
  const sw = document.querySelector('button.switch[aria-label="Keep the screen on"]');
  if (sw) {
    sw.click();
    return true;
  }
  return false;
});
await sleep(500);
console.log('clicked 20% floor:', clickedFloor, '| toggled keep-screen-on:', clickedScreen);
console.log('facts after clicks:', JSON.stringify(await facts(), null, 1));
await zoomRows('dark-picked');

// --- light pass
await page.eval(() => document.documentElement.setAttribute('data-theme', 'light'));
await sleep(400);
await page.screenshot({ path: `${OUT}/verify-settings-light.png`, format: 'png' });
await zoomRows('light-picked');

await browser.close();
console.log('done');
