// Ramp-aware recolouring. A ramp is four colours, dark to light (palette.js); swapping one ramp for another keeps each pixel's step, so
// shading, outline and dither survive the dye. Colours that two ramps share (`dust` is in both steel and iron) are ambiguous in a
// finished picture: the first step found wins, so dye a drawn picture one ramp at a time, or better, draw it again with the new ramp
// (`dye` in recipes/index.js does that for the recipes that take ramps as params).
import { Pixels } from './pixels.js';
import { ramp } from './palette.js';

const asRamp = (r) => (typeof r === 'string' ? ramp(r) : r);

/** A copy of `px` with every pixel of ramp `from` drawn in the same step of ramp `to`. Ramps by name or as four colour values. */
export function recolor(px, from, to) {
  const a = asRamp(from), b = asRamp(to);
  if (a.length !== b.length) throw new Error('recolor needs two ramps of the same length');
  const map = new Map(); a.forEach((v, i) => { if (!map.has(v)) map.set(v, b[i]); });
  return px.map((x, y) => { const v = px.get(x, y); return map.get(v) ?? v; });
}

/** Recolour a whole drawing: its frames, layers and clips. `pairs` is `[[from, to], ...]`; each pair is applied to the original pixels, so a swap (red to blue and blue to red) does not undo itself. */
export function recolorArt(art, pairs) {
  const table = new Map();
  for (const [from, to] of pairs) { const a = asRamp(from), b = asRamp(to); a.forEach((v, i) => { if (!table.has(v)) table.set(v, b[i]); }); }
  const one = (px) => (px instanceof Pixels ? px.map((x, y) => { const v = px.get(x, y); return table.get(v) ?? v; }) : px);
  const many = (list) => list?.map(one);
  return {
    ...art, frames: many(art.frames),
    ...(art.layers ? { layers: Object.fromEntries(Object.entries(art.layers).map(([k, v]) => [k, many(v)])) } : {}),
    ...(art.clips ? { clips: Object.fromEntries(Object.entries(art.clips).map(([k, c]) => [k, { ...c, frames: many(c.frames), ...(c.reduced ? { reduced: many(c.reduced) } : {}) }])) } : {}),
    ...(art.reduced ? { reduced: many(art.reduced) } : {}),
  };
}
