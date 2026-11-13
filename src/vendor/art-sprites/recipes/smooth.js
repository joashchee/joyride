// What a figure gets with a broader palette (256 colours, true colour): light from the upper left and grain, as blends in place of the Bayer
// dithers and flat steps of 16 colours. Used by the face, the item picture and the creation picture; the 16-colour paths never call it, so
// their output stays as it was. Browser-safe.
import { vnoise, K } from './vista-canvas.js';

/**
 * Volume and grain over the pixels `pick(x, y)` names, in place on `px` (a smooth canvas). `solid(x, y)` says what the figure is, so the
 * edges can be found: a pixel with empty neighbours below or to the right takes a darker tone, one with empty neighbours above or to the left
 * a lighter one (rim of `rim` sixteenths at most), and across the figure the tone slopes from `light` sixteenths lighter at the upper left
 * to as much darker at the lower right. Grain is noise of `grain` sixteenths, stretched by `fx` and `fy` (a small `fy` runs strands down,
 * a small `fx` runs a grain across). Black pixels are left alone. The same call gives the same pixels.
 */
export function volume(px, { solid, pick, seed = 0, light = 3, rim = 4, grain = 1.5, fx = 1, fy = 1 }) {
  let x0 = px.w, x1 = -1, y0 = px.h, y1 = -1;
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) if (solid(x, y)) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
  if (x1 < 0) return px;
  const cx = (x0 + x1) / 2, cy = (y0 + y1) / 2, ew = Math.max(1, (x1 - x0) / 2), eh = Math.max(1, (y1 - y0) / 2), out = px.a.slice();
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) {
    if (!pick(x, y)) continue;
    const i = y * px.w + x, c = px.a[i]; if (c === K) continue;
    const g = ((x - cx) / ew + (y - cy) / eh) / 2;
    const down = (solid(x + 1, y) ? 0 : 1) + (solid(x, y + 1) ? 0 : 1) + (solid(x + 1, y + 1) ? 0 : 1);
    const up = (solid(x - 1, y) ? 0 : 1) + (solid(x, y - 1) ? 0 : 1) + (solid(x - 1, y - 1) ? 0 : 1);
    const n = vnoise(seed, x * fx, y * fy) - 0.5;
    out[i] = px.tint(c, Math.round(-g * light + (up - down) * rim / 3 + n * 2 * grain));
  }
  px.a.set(out);
  return px;
}
