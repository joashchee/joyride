// Draws JoyrIDE's app icon (original art, no dependencies): an arcade
// joystick, in the app's pink, on a dark rounded square. Writes a 1024 px PNG
// for `tauri icon` (which makes the .icns, .ico and the sizes):
//
//   node scripts/build-icons.ts            # writes src-tauri/icons/source.png
//   npx tauri icon src-tauri/icons/source.png
//
// Shapes are signed-distance functions, 4x4 supersampled for smooth edges.
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const N = 1024;
type RGB = [number, number, number];
const BG1: RGB = [34, 28, 58];
const BG2: RGB = [14, 12, 30];
const PINK: RGB = [255, 111, 168];
const PINK_DK: RGB = [196, 60, 118];
const WHITE: RGB = [255, 255, 255];
const BASE: RGB = [70, 62, 110];

const mix = (a: RGB, b: RGB, t: number): RGB => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const rrect = (x: number, y: number, cx: number, cy: number, hw: number, hh: number, r: number) => {
  const dx = Math.abs(x - cx) - (hw - r);
  const dy = Math.abs(y - cy) - (hh - r);
  return Math.hypot(Math.max(dx, 0), Math.max(dy, 0)) + Math.min(Math.max(dx, dy), 0) - r;
};
const circle = (x: number, y: number, cx: number, cy: number, r: number) => Math.hypot(x - cx, y - cy) - r;
const capsule = (x: number, y: number, ax: number, ay: number, bx: number, by: number, r: number) => {
  const px = x - ax, py = y - ay, bax = bx - ax, bay = by - ay;
  const h = Math.min(1, Math.max(0, (px * bax + py * bay) / (bax * bax + bay * bay)));
  return Math.hypot(px - bax * h, py - bay * h) - r;
};

/** Colour at a point, or null outside the icon (transparent). */
function shade(x: number, y: number): { c: RGB; a: number } | null {
  // The app-icon square: 824 px tile centred in the canvas, as macOS expects.
  if (rrect(x, y, 512, 512, 412, 412, 185) > 0) return null;
  let c = mix(BG1, BG2, y / N);
  // Joystick base (a flat plate) with a small highlight.
  if (rrect(x, y, 512, 735, 270, 78, 60) < 0) c = mix(BASE, [30, 26, 56], (y - 657) / 156);
  if (rrect(x, y, 512, 700, 255, 14, 14) < 0) c = mix(c, WHITE, 0.18);
  // Stick, tilted a little to the right.
  const top: [number, number] = [610, 330];
  const bot: [number, number] = [512, 690];
  if (capsule(x, y, bot[0], bot[1], top[0], top[1], 34) < 0) c = mix(PINK_DK, [255, 160, 200], Math.min(1, Math.max(0, (x - 480) / 120)));
  // Ball top, with a highlight.
  if (circle(x, y, top[0], top[1], 118) < 0) {
    c = mix(PINK, PINK_DK, Math.min(1, Math.max(0, (y - 230) / 230)));
    const h = circle(x, y, top[0] - 38, top[1] - 42, 34);
    if (h < 0) c = mix(c, WHITE, 0.75);
  }
  // Two arcade buttons on the base.
  for (const [bx, col] of [[300, PINK], [724, WHITE]] as [number, RGB][]) {
    if (circle(x, y, bx, 735, 36) < 0) c = mix(col, BG2, 0.15);
  }
  return { c, a: 1 };
}

const buf = Buffer.alloc(N * N * 4);
const S = 4;
for (let py = 0; py < N; py++) {
  for (let px = 0; px < N; px++) {
    let r = 0, g = 0, b = 0, a = 0;
    for (let sy = 0; sy < S; sy++) {
      for (let sx = 0; sx < S; sx++) {
        const s = shade(px + (sx + 0.5) / S, py + (sy + 0.5) / S);
        if (s) { r += s.c[0]; g += s.c[1]; b += s.c[2]; a++; }
      }
    }
    const o = (py * N + px) * 4;
    if (a > 0) { buf[o] = r / a; buf[o + 1] = g / a; buf[o + 2] = b / a; }
    buf[o + 3] = Math.round((a / (S * S)) * 255);
  }
}

// PNG by hand: signature, IHDR, one IDAT (filter 0 per row), IEND.
const crcTable = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (b: Buffer) => { let c = 0xffffffff; for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (type: string, data: Buffer) => {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const c = Buffer.alloc(4); c.writeUInt32BE(crc(td));
  return Buffer.concat([len, td, c]);
};
const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(N, 0); ihdr.writeUInt32BE(N, 4); ihdr[8] = 8; ihdr[9] = 6;
const raw = Buffer.alloc((N * 4 + 1) * N);
for (let y = 0; y < N; y++) buf.copy(raw, y * (N * 4 + 1) + 1, y * N * 4, (y + 1) * N * 4);
const png = Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0))]);

const out = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons", "source.png");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log("wrote", out, png.length, "bytes");
