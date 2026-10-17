// Generates the Ember source icons with zero dependencies:
//   app-icon.png              the ember glyph on a rounded dark tile (1024x1024)
//   src-tauri/icons/tray.png  the bare glyph for the 16px tray (64x64, transparent)
// Run: node scripts/gen-icon.mjs   (then: npx tauri icon app-icon.png)
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const SIZE = 1024;
const OUT = resolve(__dirname, "..", "app-icon.png");
const TRAY_SIZE = 64;
const TRAY_OUT = resolve(__dirname, "..", "src-tauri", "icons", "tray.png");

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const lerp = (a, b, t) => a + (b - a) * t;
const smooth = (t) => {
  t = clamp01(t);
  return t * t * (3 - 2 * t);
};

// The mark is drawn a touch larger than the first cut so it still reads at
// 32px; the tile's calm margins stay.
const GLYPH_SCALE = 1.13;
const GLYPH_CY = 0.485; // glyph column centre, tile space

// Rounded-square signed distance (radius r), 1 inside, falls off outside.
function roundedRectAlpha(x, y, w, h, r) {
  const cx = w / 2;
  const cy = h / 2;
  const dx = Math.abs(x - cx) - (w / 2 - r);
  const dy = Math.abs(y - cy) - (h / 2 - r);
  const d =
    Math.min(Math.max(dx, dy), 0) +
    Math.hypot(Math.max(dx, 0), Math.max(dy, 0)) -
    r;
  // anti-alias over ~1.5px
  return 1 - smooth((d + 0.75) / 1.5);
}

// Ember glyph geometry, normalised to tile.
const FLAME = [
  // [u, v] control points of the flame silhouette (top -> bottom), u,v in 0..1
  [0.5, 0.235],
  [0.605, 0.40],
  [0.652, 0.545],
  [0.615, 0.665],
  [0.5, 0.735],
  [0.385, 0.665],
  [0.348, 0.545],
  [0.395, 0.40],
];

function flameHalfWidth(v) {
  // width profile of the flame: narrow at the tip, wider toward the base
  const t = clamp01((v - 0.22) / 0.52);
  const base = 0.042 + 0.108 * Math.sin(Math.PI * Math.pow(t, 0.66));
  // a little inward pinch near the top third
  const pinch = 1 - 0.22 * Math.exp(-Math.pow((t - 0.25) * 4.5, 2));
  return base * pinch;
}

function glyphCoverage(u, v) {
  if (v < 0.225 || v > 0.745) return 0;
  const half = flameHalfWidth(v);
  const d = Math.abs(u - 0.5) - half;
  return 1 - smooth((d + 0.004) / 0.008);
}

// Ember glyph in glyph space: amber tip -> coal-red base with a white-hot
// core and a warm bloom. `bloomGain` is 1 on the tile, gentler in the tray.
function emberGlyph(gu, gv, bloomGain) {
  const cov = glyphCoverage(gu, gv);

  // Flame body gradient along v: bright amber at the tip -> deep coal red base.
  const vt = clamp01((gv - 0.23) / 0.52);
  let r = lerp(255, 196, vt);
  let g = lerp(168, 58, vt);
  let b = lerp(88, 12, vt);

  // White-hot coal hotspot near the lower third.
  const core = Math.hypot((gu - 0.5) * 0.92, (gv - 0.63) * 1.05);
  const hot = Math.pow(clamp01(1 - core / 0.115), 1.7);
  r = lerp(r, 255, hot);
  g = lerp(g, 226, hot);
  b = lerp(b, 168, hot);

  // Additive warm bloom spilling past the glyph edge.
  const bloom = Math.pow(clamp01(1 - core / 0.32), 2.8) * bloomGain;
  return { r, g, b, cov, bloom };
}

const round255 = (v) => Math.round(clamp01(v / 255) * 255);

function tileFrag(x, y) {
  const u = x / SIZE;
  const v = y / SIZE;
  const tileA = roundedRectAlpha(x, y, SIZE, SIZE, SIZE * 0.225);
  if (tileA <= 0) return [0, 0, 0, 0];

  // Vertical background gradient: warm-dark graphite with an ember undertone.
  let r = lerp(30, 16, v);
  let g = lerp(26, 14, v);
  let b = lerp(34, 20, v);

  // Subtle top highlight (glass sheen on the tile).
  const sheen = Math.max(0, 1 - Math.hypot((u - 0.5) / 0.62, (v + 0.16) / 0.5));
  const s = Math.pow(clamp01(sheen), 2.2) * 0.16;
  r = lerp(r, 255, s);
  g = lerp(g, 236, s);
  b = lerp(b, 224, s);

  // The glyph is sampled in glyph space so the mark scales independently.
  const gu = 0.5 + (u - 0.5) / GLYPH_SCALE;
  const gv = GLYPH_CY + (v - GLYPH_CY) / GLYPH_SCALE;
  const { r: gr, g: gg, b: gb, cov, bloom } = emberGlyph(gu, gv, 0.42);

  r = lerp(r, gr, cov);
  g = lerp(g, gg, cov);
  b = lerp(b, gb, cov);

  const hb = bloom * (1 - cov);
  r = lerp(r, 255, hb);
  g = lerp(g, 122, hb);
  b = lerp(b, 40, hb);

  return [round255(r), round255(g), round255(b), Math.round(tileA * 255)];
}

// The tray gets the bare glyph — no tile — scaled to fill the canvas so the
// silhouette survives 16px, with a quieter bloom that reads on any taskbar.
const TRAY_FIT = 1 / 1.54;

function trayFrag(x, y) {
  const u = x / TRAY_SIZE;
  const v = y / TRAY_SIZE;
  const gu = 0.5 + (u - 0.5) * TRAY_FIT;
  const gv = GLYPH_CY + (v - 0.5) * TRAY_FIT;
  const { r, g, b, cov, bloom } = emberGlyph(gu, gv, 0.3);

  const halo = bloom * (1 - cov);
  const a = clamp01(cov + halo);
  if (a <= 0) return [0, 0, 0, 0];

  // Body where solid, blending toward the bloom colour in the halo.
  const mix = halo / a;
  return [round255(lerp(r, 255, mix)), round255(lerp(g, 122, mix)), round255(lerp(b, 40, mix)), Math.round(a * 255)];
}

function crc32(buf) {
  let c;
  const table = [];
  for (let n = 0; n < 256; n++) {
    c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) crc = table[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, "ascii");
  const body = Buffer.concat([t, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

// Build raw RGBA scanlines with filter byte 0 and write a PNG.
function writePng(out, size, frag) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  let o = 0;
  for (let y = 0; y < size; y++) {
    raw[o++] = 0;
    for (let x = 0; x < size; x++) {
      const [r, g, b, a] = frag(x + 0.5, y + 0.5);
      raw[o++] = r;
      raw[o++] = g;
      raw[o++] = b;
      raw[o++] = a;
    }
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);

  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, png);
  console.log(`Wrote ${out} (${size}x${size}, ${png.length} bytes)`);
}

writePng(OUT, SIZE, tileFrag);
writePng(TRAY_OUT, TRAY_SIZE, trayFrag);
