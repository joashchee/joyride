// An image to cells, for the study workflow only (the kit's image-to-character method: bert.org, 247plan): fix the luminance and
// stretch the contrast first, resize with a good filter (Lanczos), let half blocks absorb the 2:1 cell ratio, then choose each cell by
// least colour error over glyph and colour pairs, with `halves` and the legal background limit. A drafting aid, not a recipe: use an
// image you have the right to use, do not commit the output as engine art. It never fetches anything. Browser-safe.
import { Cells, NONE, halves, legalize } from './cells.js';
import { VGA16, XTERM256, rgbOf, distance } from './palette-ansi.js';

const lanczos = (x) => (x === 0 ? 1 : Math.abs(x) >= 3 ? 0 : (3 * Math.sin(Math.PI * x) * Math.sin((Math.PI * x) / 3)) / (Math.PI * Math.PI * x * x));

// Resize premultiplied RGBA (floats) by Lanczos-3, one axis at a time.
function resize(src, w, h, nw, nh) {
  const pass = (data, sw, sh, tw, th, horizontal) => {
    const out = new Float32Array(tw * th * 4), n = horizontal ? sw : sh, m = horizontal ? tw : th, scale = n / m, support = Math.max(1, scale) * 3;
    for (let o = 0; o < m; o++) {
      const centre = (o + 0.5) * scale, lo = Math.max(0, Math.floor(centre - support)), hi = Math.min(n - 1, Math.ceil(centre + support));
      const taps = []; let sum = 0;
      for (let i = lo; i <= hi; i++) { const k = lanczos((i + 0.5 - centre) / Math.max(1, scale)); taps.push([i, k]); sum += k; }
      for (let line = 0; line < (horizontal ? sh : tw); line++) for (let c = 0; c < 4; c++) {
        let v = 0; for (const [i, k] of taps) v += k * data[((horizontal ? line * sw + i : i * sw + line) * 4) + c];
        out[((horizontal ? line * tw + o : o * tw + line) * 4) + c] = v / sum;
      }
    }
    return out;
  };
  return pass(pass(src, w, h, nw, h, true), nw, h, nw, nh, false);
}

/**
 * Convert an image (a `Pixels` canvas, or `{ w, h, rgba }`) to cells. `columns` is the width in cells (default 80); the height follows
 * the image's shape, two square pixels to a cell. `contrast` (default true) stretches the luminance so the ramp's ends are used (the
 * 2nd to 98th percentile). `colors` is 16 (glyph and colour pairs: space, `░ ▒ ▓ █ ▀ ▄`, foreground any of 16, background under
 * `bgLimit`) or 256 (each half block the nearest of the 256). Pixels under half opacity are transparent. Returns `{ cells, warnings }`.
 * Busy sources come out noisy: the kit's advice is a simple, high-contrast picture and, under 60 columns, a short ramp.
 */
export function fromImage(image, { columns = 80, colors = 16, bgLimit = 8, contrast = true } = {}) {
  if (colors !== 16 && colors !== 256) throw new Error(`fromImage colors is 16 or 256, not ${colors}`);
  if (!Number.isInteger(columns) || columns < 1) throw new RangeError(`columns must be a whole number of 1 or more, not ${columns}`);
  const w = image.w, h = image.h, rgba = image.rgba ?? image.toRgba(), warnings = [];
  const px = new Float32Array(w * h * 4);
  for (let i = 0; i < w * h; i++) { const a = rgba[i * 4 + 3] / 255; for (let c = 0; c < 3; c++) px[i * 4 + c] = rgba[i * 4 + c] * a; px[i * 4 + 3] = a; }
  const tw = columns, th = Math.max(2, 2 * Math.round((h * columns) / w / 2)), small = resize(px, w, h, tw, th);
  // Colour and opacity back out of the premultiplied floats.
  const rgb = [], opaque = [];
  for (let i = 0; i < tw * th; i++) { const a = Math.min(1, Math.max(0, small[i * 4 + 3])); opaque.push(a >= 0.5); rgb.push([0, 1, 2].map((c) => (a > 0 ? Math.min(255, Math.max(0, small[i * 4 + c] / a)) : 0))); }
  if (contrast) {
    const l = rgb.filter((_, i) => opaque[i]).map((c) => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).sort((a, b) => a - b);
    const lo = l[Math.floor(l.length * 0.02)] ?? 0, hi = l[Math.min(l.length - 1, Math.floor(l.length * 0.98))] ?? 255;
    if (hi - lo < 16) warnings.push('the image has almost no contrast to stretch');
    else for (const c of rgb) for (let k = 0; k < 3; k++) c[k] = Math.min(255, Math.max(0, ((c[k] - lo) / (hi - lo)) * 255));
  }
  const cells = new Cells(tw, th / 2), memo = new Map(), set = colors === 256 ? XTERM256.map(rgbOf) : VGA16.map(rgbOf);
  const near = (c) => { let b = 0, bd = Infinity; for (let i = 0; i < set.length; i++) { const d = distance(c, set[i]); if (d < bd) { bd = d; b = i; } } return b; };
  const GLYPHS = [0x20, 0x2591, 0x2592, 0x2593, 0x2588, 0x2580, 0x2584];
  // The glyph and colour pairs and their two halves, made once (not per cell pair), in the same order so ties choose the same pair.
  let pairs = null;
  const candidates = () => {
    if (pairs) return pairs;
    pairs = [];
    for (const ch of GLYPHS) for (let fg = 0; fg < 16; fg++) for (let bg = 0; bg < 16; bg++) {
      if ((ch === 0x20 && fg !== bg) || (ch === 0x2588 && bg !== 0)) continue;           // a space ignores fg, a full block ignores bg
      const cell = { ch, fg, bg }, [ht, hb] = halves(cell); pairs.push({ cell, ht, hb });
    }
    return pairs;
  };
  const best = (t, b) => {
    const key = [...t, ...b].map(Math.round).join(',');
    if (memo.has(key)) return memo.get(key);
    let out;
    if (colors === 256) { const f = near(t), g = near(b); out = f === g ? { ch: 0x20, fg: f, bg: f } : { ch: 0x2580, fg: f, bg: g }; }
    else {
      let bd = Infinity;
      for (const c of candidates()) {
        const d = distance(c.ht, t) + distance(c.hb, b);
        if (d < bd) { bd = d; out = c.cell; }
      }
      out = { ...out };
      if (out.ch === 0x2588) out.bg = NONE;
    }
    memo.set(key, out); return out;
  };
  for (let y = 0; y < th / 2; y++) for (let x = 0; x < tw; x++) {
    const i = 2 * y * tw + x, j = i + tw, up = opaque[i], down = opaque[j];
    if (!up && !down) continue;
    if (up && down) cells.set(x, y, best(rgb[i], rgb[j]));
    else if (up) cells.set(x, y, { ch: 0x2580, fg: near(rgb[i]) }); else cells.set(x, y, { ch: 0x2584, fg: near(rgb[j]) });
  }
  return { cells: colors === 16 ? legalize(cells, { bgLimit }) : cells, warnings };
}
