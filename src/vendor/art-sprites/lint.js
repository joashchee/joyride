// The learning kit's fundamentals as checks a drawing can fail. Not taste: each rule is one thing the kit says to do or avoid
// (limited palette, no pixel noise, readable value range, a closed outline, intentional frame changes, standard canvas sizes).
import { luma } from './palette.js';

export const SIZES = [8, 12, 16, 24, 32, 40, 48, 64];
const neigh = (p, x, y) => [p.get(x - 1, y), p.get(x + 1, y), p.get(x, y - 1), p.get(x, y + 1)];

/** Findings for one canvas: `{ rule, level: 'warn' | 'error', message }`. Empty means it passes. */
export function lint(p, { maxColors = 16, name = 'sprite', skip = [] } = {}) {
  const out = [], add = (rule, level, message) => { if (!skip.includes(rule)) out.push({ rule, level, message: `${name}: ${message}` }); };
  if (!p.count()) { add('empty', 'error', 'has no pixels'); return out; }
  const used = p.colors();
  if (used.length > maxColors) add('palette', 'warn', `${used.length} colours; the kit says to start with 4 to 16`);
  if (!SIZES.includes(p.w) || !SIZES.includes(p.h)) add('size', 'warn', `${p.w}x${p.h} is not a standard sprite size (${SIZES.join(', ')})`);
  let orphans = 0, open = 0, edge = 0;
  for (let y = 0; y < p.h; y++) for (let x = 0; x < p.w; x++) {
    if (!p.get(x, y)) continue;
    if (!neigh(p, x, y).some(Boolean) && ![p.get(x - 1, y - 1), p.get(x + 1, y - 1), p.get(x - 1, y + 1), p.get(x + 1, y + 1)].some(Boolean)) orphans++;
    if (neigh(p, x, y).some((v) => !v)) { edge++; if (luma(p.hex(p.get(x, y))) > 90) open++; }
  }
  if (orphans > 6) add('noise', 'warn', `${orphans} isolated pixels (pixel noise); effects are the usual exception`);
  if (edge && open / edge > 0.5) add('outline', 'warn', 'more than half the edge is light: no dark outline or the shape may vanish on a bright background');
  const ls = used.map((v) => luma(p.hex(v)));
  if (Math.max(...ls) - Math.min(...ls) < 60) add('value', 'warn', 'narrow value range: forms rely on hue alone');
  // Art on the canvas edge that is not the dark outline colour had its outline cut off.
  let clipped = 0;
  for (let i = 0; i < p.w; i++) for (const [x, y] of [[i, 0], [i, p.h - 1], [0, i], [p.w - 1, i]]) { const v = p.get(x, y); if (v && luma(p.hex(v)) > 80) clipped++; }
  if (clipped) add('margin', 'warn', `${clipped} lit pixels on the canvas edge: the outline is clipped (leave a 1 pixel margin)`);
  return out;
}

/** Pixels that differ between two frames once the previous one is allowed to slide up to `slide` pixels: a lunge or a bob is a move, not boiling. */
function changed(a, b, slide = 3) {
  let best = Infinity, any = 0;
  for (let k = 0; k < b.data.length; k++) if (a.data[k] || b.data[k]) any++;
  for (let dy = -slide; dy <= slide; dy++) for (let dx = -slide; dx <= slide; dx++) {
    let diff = 0;
    for (let y = 0; y < b.h; y++) for (let x = 0; x < b.w; x++) if (a.get(x - dx, y - dy) !== b.get(x, y)) diff++;
    best = Math.min(best, diff);
  }
  return { diff: best, any };
}

/** Findings for an animation: frames that never change, or (in a loop) change so much the sprite "boils". `boil: false` is for a wave, where the whole surface moving is the animation. */
export function lintFrames(frames, { name = 'clip', loop = true, boil = true } = {}) {
  const out = [];
  if (frames.length < 2) return out;
  for (let i = 1; i < frames.length; i++) {
    if (frames[i].equals(frames[i - 1])) { out.push({ rule: 'frames', level: 'warn', message: `${name}: frame ${i} is identical to frame ${i - 1}` }); continue; }
    const { diff, any } = changed(frames[i - 1], frames[i]);
    if (boil && loop && frames.length > 2 && any && diff / any > 0.5) out.push({ rule: 'boil', level: 'warn', message: `${name}: frame ${i} changes ${Math.round((100 * diff) / any)}% of the art beyond a slide from frame ${i - 1}; keep changes intentional` });
  }
  if (loop && frames.length > 8) out.push({ rule: 'length', level: 'warn', message: `${name}: ${frames.length} frames; the kit starts walk cycles at 4 to 6` });
  return out;
}

/**
 * What a compositor and a screen reader need beside the pixels (art-meta.js fills these in): `alt` text, `anchors` inside the canvas,
 * and a `reduced` still for the art and every clip that moves (Tag Team 3.6, 9). All errors: a drawing without them breaks the
 * reduced-motion and screen-reader promises. Pixel art only; cells have no point anchors.
 */
export function lintArt(art, { name = art.id ?? 'art' } = {}) {
  const out = [], add = (rule, message) => out.push({ rule, level: 'error', message: `${name}: ${message}` });
  if (!String(art.alt ?? '').trim()) add('alt', 'has no alt text');
  const f = art.frames?.[0];
  if (f && typeof f.w === 'number' && f.data instanceof Uint8Array) {
    const inside = (pt) => !Array.isArray(pt) || typeof pt[0] !== 'number' || (pt[0] >= 0 && pt[1] >= 0 && pt[0] < f.w && pt[1] < f.h);
    for (const [n, pt] of Object.entries(art.anchors ?? {})) if (!inside(pt)) add('anchors', `anchor "${n}" at ${pt} is outside the ${f.w}x${f.h} canvas`);
  }
  if (art.fps > 0 && art.frames?.length > 1 && !art.reduced?.length) add('reduced', 'animates with no reduced-motion still');
  for (const [n, c] of Object.entries(art.clips ?? {})) if (c.frames?.length > 1 && !c.reduced?.length) add('reduced', `clip "${n}" has no reduced-motion still`);
  return out;
}

/** Side-by-side study numbers for two drawings (the kit's "compare" step): silhouette overlap, colour counts, value range. */
export function compare(a, b) {
  const w = Math.max(a.w, b.w), h = Math.max(a.h, b.h); let both = 0, either = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const p = a.get(x, y) > 0, q = b.get(x, y) > 0; if (p && q) both++; if (p || q) either++; }
  const range = (p) => { const l = p.colors().map((v) => luma(p.hex(v))); return l.length ? Math.round(Math.max(...l) - Math.min(...l)) : 0; };
  return { silhouetteOverlap: either ? +(both / either).toFixed(2) : 1, colors: [a.colors().length, b.colors().length], valueRange: [range(a), range(b)], pixels: [a.count(), b.count()] };
}

/**
 * Do these drawings differ by silhouette alone? Each pair is compared as the filled shape, with no colour: the share of the union that only one
 * covers. A pair below `floor` is an `error`. Tag Team's classes read from above by headgear, weapon, shield and back slot, so this is the
 * check that they still do. `items` is `[{ name, px }]`.
 */
export function lintSilhouettes(items, { floor = 0.15 } = {}) {
  const out = [];
  for (let i = 0; i < items.length; i++) for (let j = i + 1; j < items.length; j++) {
    const a = items[i].px, b = items[j].px; let only = 0, any = 0;
    for (let k = 0; k < a.data.length; k++) { const x = a.data[k] > 0, y = b.data[k] > 0; if (x || y) any++; if (x !== y) only++; }
    const d = any ? only / any : 0;
    if (d < floor) out.push({ rule: 'silhouette', level: 'error', message: `${items[i].name} and ${items[j].name} differ by ${(d * 100).toFixed(0)}% of their shape; at least ${(floor * 100).toFixed(0)}% so a class reads without colour` });
  }
  return out;
}
