// The pixel canvas `vista` draws on, outdoors and in: two pixels to a cell (the half block: top pixel the foreground, bottom the
// background), 16 ANSI indexes in the order of VGA16 (the same as `V`), Bayer dithering as Coupler's painter does it. Browser-safe.
import { Cells, NONE } from '../cells.js';
import { XTERM256 } from '../palette-ansi.js';

const RGB = XTERM256.map((h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)));
/** A colour handle at or above this is an exact colour (true colour): `EXACT | 0xRRGGBB`. Below it, an index of the 256. */
export const EXACT = 0x1000000;
const rgbOfHandle = (c) => (c >= EXACT ? [(c >> 16) & 255, (c >> 8) & 255, c & 255] : RGB[c]);
const pack = (c) => EXACT | (Math.round(c[0]) << 16) | (Math.round(c[1]) << 8) | Math.round(c[2]);
const nearest256 = (c, noBlack = false) => { let best = 1e9, v = 0; for (let k = noBlack ? 1 : 0; k < 256; k++) { const d = (RGB[k][0] - c[0]) ** 2 + (RGB[k][1] - c[1]) ** 2 + (RGB[k][2] - c[2]) ** 2; if (d < best) { best = d; v = k; } } return v; };
const near = new Map();
/**
 * The colour `t` sixteenths of the way from colour `a` to `b`, where 16 colours had to dither: the nearest of the xterm-256, or with `exact`
 * (true colour) the exact blend, as an `EXACT` handle.
 */
export function between(a, b, t, exact = false) {
  if (exact || a >= EXACT || b >= EXACT) { const x = rgbOfHandle(a), y = rgbOfHandle(b); return pack([0, 1, 2].map((i) => x[i] + (y[i] - x[i]) * t / 16)); }
  const key = (a << 12) | (b << 4) | t; let v = near.get(key);
  if (v === undefined) { v = nearest256([0, 1, 2].map((i) => RGB[a][i] + (RGB[b][i] - RGB[a][i]) * t / 16)); near.set(key, v); }
  return v;
}
/** Colour `c` scaled by `k` (0 to 1, or more to lighten): the nearest of the 256, or with `exact` the exact one. A darker self. */
export function shade256(c, k, exact = false, noBlack = false) {
  const m = rgbOfHandle(c).map((v) => Math.min(255, v * k));
  return exact || c >= EXACT ? pack(m) : nearest256(m, noBlack);
}
/** The xterm-256 index nearest to a colour handle (an index is its own). */
export const indexOf = (c) => (c >= EXACT ? nearest256(rgbOfHandle(c)) : c);

/** Check a `colors` param: 16, 256 or 'true'. */
export function checkColors(colors) {
  if (colors !== 16 && colors !== 256 && colors !== 'true') throw new Error(`colors is 16, 256 or 'true', not ${colors}`);
  return colors;
}
/** A palette index list that maps every handle to itself, for `toCells`: all 256 indexes (a 16-colour picture needs only the first 16). */
export const IDENTITY256 = Array.from({ length: 256 }, (_, i) => i);
export const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
export const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
export const K = 0, RED = 1, GRN = 2, BRN = 3, BLU = 4, MAG = 5, CYN = 6, GRY = 7, DGR = 8, LRED = 9, LGRN = 10, YEL = 11, LBLU = 12, LCYN = 14, WHT = 15;
export const LIT = 1, WET = 2;
export const ridgeLine = (r, W, amp, f) => { const a = r.next() * 6, b = r.next() * 6; return Array.from({ length: W }, (_, x) => Math.round(amp * clamp(0.5 + 0.32 * Math.sin(x * f + a) + 0.18 * Math.sin(x * f * 2.3 + b), 0, 1))); };
/**
 * The pixel canvas. `depth` is 16 (Bayer dithers between the colours), 256 (the colour between, from the xterm-256) or 'true' (the exact
 * colour between, as an `EXACT` handle); `true` and `false` are 256 and 16, as it was. A pixel is a handle: an index, or an exact colour.
 */
export class Px {
  constructor(w, h, depth = 16) {
    depth = depth === true ? 256 : depth === false ? 16 : depth;
    if (depth !== 16 && depth !== 256 && depth !== 'true') throw new Error(`depth is 16, 256 or 'true', not ${depth}`);
    this.w = w; this.h = h; this.depth = depth; this.smooth = depth !== 16; this.exact = depth === 'true';
    this.a = new Uint32Array(w * h).fill(K); this.f = new Uint8Array(w * h);
  }
  /** The colour between `a` and `b`, `t` sixteenths along, at this canvas's depth (smooth only). */
  between(a, b, t) { return between(a, b, t, this.exact); }
  /** Colour `c` scaled by `k`, at this canvas's depth (smooth only). */
  scaled(c, k) { return shade256(c, k, this.exact); }
  /**
   * Colour `c` stepped `t` sixteenths toward white (t > 0) or black (t < 0), at this canvas's depth (smooth only). A dark colour with no
   * neighbour a small step away in the 256 (the cube jumps from 0 to 95) takes a scaled one instead when the step was 2 or more.
   */
  tint(c, t) {
    if (t === 0) return c;
    const v = t > 0 ? this.between(c, WHT, Math.min(t, 16)) : this.between(c, K, Math.min(-t, 16));
    if (v === K && c !== K && t > -16 && !this.exact) return shade256(c, 1 + t / 16, false, true);       // a dark colour is not made black by a small step
    return v === c && Math.abs(t) >= 2 && c !== K ? this.scaled(c, t > 0 ? 1 + t / 8 : 1 + t / 20) : v;
  }
  set(x, y, c, f = 0) { x = Math.floor(x); y = Math.floor(y); if (x < 0 || y < 0 || x >= this.w || y >= this.h) return; const i = y * this.w + x; this.a[i] = c; this.f[i] = f; }
  get(x, y) { return x < 0 || y < 0 || x >= this.w || y >= this.h ? K : this.a[Math.floor(y) * this.w + Math.floor(x)]; }
  is(x, y, f) { return x >= 0 && y >= 0 && x < this.w && y < this.h && (this.f[Math.floor(y) * this.w + Math.floor(x)] & f) !== 0; }
  rect(x0, y0, x1, y1, c, f = 0) { for (let y = Math.floor(y0); y <= y1; y++) for (let x = Math.floor(x0); x <= x1; x++) this.set(x, y, c, f); }
  disc(cx, cy, r, c, f = 0) { for (let y = Math.floor(cy - r); y <= cy + r; y++) for (let x = Math.floor(cx - r); x <= cx + r; x++) if ((x - cx) ** 2 + (y - cy) ** 2 <= r * r + 0.5) this.set(x, y, c, f); }
  /** `t` sixteenths of the way from `a` to `b`: a Bayer dither in 16 colours, the colour between itself with 256. */
  mix(x, y, a, b, t) { return this.smooth ? (t <= 0 ? a : t >= 16 ? b : this.between(a, b, t)) : BAYER[y & 3][x & 3] < t ? b : a; }
  /** Bands from `y0` to `y1` through `stops`, solid, dithered only where one meets the next (Coupler's `gradient`). */
  gradient(y0, y1, stops, f = 0) {
    const n = stops.length, span = Math.max(1, y1 - y0);
    for (let y = Math.floor(y0); y < y1; y++) {
      const at = Math.floor(((y - y0) * (n - 1) * 16) / span), band = at >> 4, t = clamp(Math.floor(((at % 16) - 5) * 16 / 6), 0, 16);
      for (let x = 0; x < this.w; x++) this.set(x, y, band >= n - 1 ? stops[n - 1] : this.smooth ? (t === 0 ? stops[band] : t === 16 ? stops[band + 1] : this.between(stops[band], stops[band + 1], t)) : this.mix(x, y, stops[band], stops[band + 1], t), f);
    }
  }
  /** Scatter `b` over a region at `density`, only on pixels already `over` (any if omitted). */
  speckle(r, x0, y0, x1, y1, b, density, over) { for (let y = Math.floor(y0); y <= y1; y++) for (let x = Math.floor(x0); x <= x1; x++) if (r.chance(density) && (over === undefined || this.get(x, y) === over)) this.set(x, y, b); }
  /** Queue a character over a cell that is one colour (both pixels alike): small things like bubbles that a pixel would draw too big. */
  glyph(cx, cy, ch, fg) { (this.glyphs ??= []).push([cx, cy, ch, fg]); }
  toCells(map) {
    const cells = new Cells(this.w, this.h >> 1), tc = this.exact;
    // an exact colour is the index nearest to it for a 256-colour viewer, and its own RGB for a true-colour one
    const idx = (c) => (c >= EXACT ? indexOf(c) : map[c]), put = (x, y, top, bot) => {
      const cell = top === bot ? { ch: 0x2588, fg: idx(top), bg: NONE } : { ch: 0x2580, fg: idx(top), bg: idx(bot) };
      if (tc) { if (top >= EXACT) cell.fgRgb = top & 0xffffff; if (bot !== top && bot >= EXACT) cell.bgRgb = bot & 0xffffff; }
      cells.set(x, y, cell);
    };
    for (let y = 0; y < cells.h; y++) for (let x = 0; x < this.w; x++) put(x, y, this.a[2 * y * this.w + x], this.a[(2 * y + 1) * this.w + x]);
    for (const [cx, cy, ch, fg] of this.glyphs ?? []) {
      const top = this.a[2 * cy * this.w + cx], bot = this.a[(2 * cy + 1) * this.w + cx];
      if (cx >= 0 && cy >= 0 && cx < this.w && cy < cells.h && top === bot && top !== fg) cells.set(cx, cy, { ch, fg: idx(fg), bg: idx(top), ...(tc ? { fgRgb: fg >= EXACT ? fg & 0xffffff : undefined, bgRgb: top >= EXACT ? top & 0xffffff : undefined } : {}) });
    }
    return cells;
  }
}


// --- texture with a broader palette --------------------------------------------------------------------------------------------------
export const hash2 = (seed, x, y) => { let h = (Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(seed, 2147483647)) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; };
export const vnoise = (seed, x, y) => {
  const ix = Math.floor(x), iy = Math.floor(y), fx = x - ix, fy = y - iy, u = fx * fx * (3 - 2 * fx), v = fy * fy * (3 - 2 * fy);
  const a = hash2(seed, ix, iy), b = hash2(seed, ix + 1, iy), c = hash2(seed, ix, iy + 1), d = hash2(seed, ix + 1, iy + 1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
};
/**
 * With 256 colours, grain over the whole picture, in the in-between tones 16 colours cannot reach: the ground in noise that is coarse near the
 * viewer and fine at the horizon, the sky in long soft streaks, water in horizontal shimmer, rooms in an even weathering. `hz` is the horizon row
 * (0 when the picture has none: underwater, indoors). Black and lit pixels (sun, windows, lightning) are left alone.
 */
export function texture(px, seed, hz = 0, { land = true } = {}) {
  if (!px.smooth) return;
  const H = px.h, below = Math.max(1, H - hz);
  for (let y = 0; y < H; y++) for (let x = 0; x < px.w; x++) {
    const i = y * px.w + x, c = px.a[i], f = px.f[i];
    if (c === K || (f & LIT)) continue;
    let n, amp;
    if (f & WET) { n = 0.6 * vnoise(seed, x / 5, y * 1.3) + 0.4 * vnoise(seed + 1, x / 2, y * 0.8); amp = 2.5; }
    else if (land && y < hz) { n = 0.65 * vnoise(seed + 2, x / 11, y / 1.6) + 0.35 * vnoise(seed + 3, x / 4, y / 1.2); amp = 1.6; }
    else { const d = hz ? (y - hz) / below : 0.5; n = 0.55 * vnoise(seed + 4, x / (1.5 + 4 * d), y / (1 + 1.5 * d)) + 0.45 * vnoise(seed + 5, x / (0.9 + 1.6 * d), y / (0.7 + d)); amp = 2.2 + 1.8 * d; }
    const t = Math.round((n - 0.5) * 2 * amp * 1.35);
    // dark colours have no neighbour a small step away (the cube jumps from 0 to 95): the peaks of the noise take the next one up
    if (t > 0) { let v = px.between(c, WHT, Math.min(t, 4)); if (v === c && t >= 2) v = px.scaled(c, 1.45); px.a[i] = v; }
    else if (t < 0) px.a[i] = px.between(c, K, Math.min(-t, 4));
  }
}
