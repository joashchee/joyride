// Contact sheets: many canvases on one PNG, each at the kit's test scales (the learning kit says to check 100%, 200% and 400%).
import { encodePng } from './png.js';

/** Lay canvases out left to right in rows of `cols`, each at `scale`, over `bg`. Returns a PNG buffer. */
export function contactSheet(items, { scale = 4, cols = 8, bg = '#2a2a3a', gap = 2 } = {}) {
  const cw = Math.max(...items.map((p) => p.w)) * scale + gap, ch = Math.max(...items.map((p) => p.h)) * scale + gap;
  const rows = Math.ceil(items.length / cols), W = cw * Math.min(cols, items.length) + gap, H = ch * rows + gap, out = new Uint8Array(W * H * 4);
  const rgb = (hex) => { const n = parseInt(hex.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255, 255]; };
  for (let i = 0; i < W * H; i++) out.set(rgb(bg), i * 4);
  items.forEach((p, i) => {
    const px = p.toRgba(scale), ox = gap + (i % cols) * cw, oy = gap + Math.floor(i / cols) * ch;
    for (let y = 0; y < p.h * scale; y++) for (let x = 0; x < p.w * scale; x++) { const s = (y * p.w * scale + x) * 4; if (px[s + 3]) out.set(px.subarray(s, s + 4), ((oy + y) * W + ox + x) * 4); }
  });
  return encodePng(W, H, out);
}
/** The same canvas at 1x, 2x and 4x on one strip: the readability test. */
export function scaleStrip(p, scales = [1, 2, 4], bg = '#2a2a3a') {
  const W = scales.reduce((n, s) => n + p.w * s + 4, 4), H = p.h * Math.max(...scales) + 4, out = new Uint8Array(W * H * 4);
  const rgb = (hex) => { const n = parseInt(hex.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255, 255]; };
  for (let i = 0; i < W * H; i++) out.set(rgb(bg), i * 4);
  let ox = 4;
  for (const s of scales) {
    const px = p.toRgba(s);
    for (let y = 0; y < p.h * s; y++) for (let x = 0; x < p.w * s; x++) { const k = (y * p.w * s + x) * 4; if (px[k + 3]) out.set(px.subarray(k, k + 4), ((2 + y) * W + ox + x) * 4); }
    ox += p.w * s + 4;
  }
  return encodePng(W, H, out);
}

/** Like `contactSheet`, for rendered cells (`renderCells` output: `{ w, h, rgba }`), each at an integer `scale`, over `bg`. Returns a PNG buffer. */
export function rgbaSheet(images, { scale = 1, cols = 2, bg = '#2a2a3a', gap = 4 } = {}) {
  const cw = Math.max(...images.map((p) => p.w)) * scale + gap, ch = Math.max(...images.map((p) => p.h)) * scale + gap;
  const rows = Math.ceil(images.length / cols), W = cw * Math.min(cols, images.length) + gap, H = ch * rows + gap, out = new Uint8Array(W * H * 4);
  const n = parseInt(bg.slice(1), 16);
  for (let i = 0; i < W * H; i++) out.set([n >> 16, (n >> 8) & 255, n & 255, 255], i * 4);
  images.forEach((p, i) => {
    const ox = gap + (i % cols) * cw, oy = gap + Math.floor(i / cols) * ch;
    for (let y = 0; y < p.h * scale; y++) for (let x = 0; x < p.w * scale; x++) { const s = ((Math.floor(y / scale)) * p.w + Math.floor(x / scale)) * 4; out.set(p.rgba.subarray(s, s + 4), ((oy + y) * W + ox + x) * 4); }
  });
  return encodePng(W, H, out);
}
