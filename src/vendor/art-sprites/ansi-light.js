// Lighting for cells, the counterpart of `relight` for pixels. A cell is lit by stepping its colours along a shade ramp: brighter near
// a light source, darker under a low ambient (night). Cells marked self-lit (windows, fire, a lantern's own flame) ignore the ambient
// and are never darkened. Colours are VGA indexes, so a step is a move along the hand-made ramps, and the result is made legal again.
import { Cells, NONE, legalize } from './cells.js';
import { ANSI_RAMPS } from './palette-ansi.js';

const RAMPS = Object.values(ANSI_RAMPS);
const stepIn = (ramp, c, n) => ramp[Math.max(0, Math.min(ramp.length - 1, ramp.indexOf(c) + n))];
// A colour moves along the first ramp that holds it, or along the ramp of `hint` (the cell's other colour) when it is black, which
// every ramp starts with: so a shadowed blue cell stays blue-black rather than going gray.
function shift(c, n, hint) {
  if (c === NONE || !n) return c;
  const ramp = c === 0 && hint !== undefined && hint !== NONE ? RAMPS.find((r) => r.indexOf(hint) > 0) : RAMPS.find((r) => r.includes(c));
  return stepIn(ramp ?? ANSI_RAMPS.gray, c, n);
}

/**
 * Light a picture. `ambient` is the steps added everywhere (0 as drawn, -2 for night, +1 for a bright noon; whole steps along the
 * colour ramps). Each of `sources` is `{ x, y, radius, strength }` (strength in steps, default 2): its light falls off to nothing at
 * `radius` cells, with a cell counted as twice as tall as wide. `selfLit` is a `Cells` mask (its non-empty cells, as the folded
 * drawings' `mask` is) or `(x, y, cell) => boolean`: those cells take no ambient, only the light of the sources. Empty cells stay
 * empty. Whole steps only, so lit and unlit areas meet as clean bands; blend them with `gradient` or `bayer` if a smooth edge is
 * wanted. Returns new cells, legal for `bgLimit`.
 */
export function light(cells, { ambient = 0, sources = [], selfLit, bgLimit = 8 } = {}) {
  const out = new Cells(cells.w, cells.h), lit = typeof selfLit === 'function' ? selfLit : selfLit instanceof Cells ? (x, y) => !selfLit.isEmpty(x, y) : () => false;
  for (let y = 0; y < cells.h; y++) for (let x = 0; x < cells.w; x++) {
    const cell = cells.get(x, y);
    if (cells.isEmpty(x, y)) continue;
    let level = lit(x, y, cell) ? 0 : ambient;
    for (const { x: sx, y: sy, radius, strength = 2 } of sources) {
      const d = Math.hypot(x - sx, (y - sy) * 2);
      if (d < radius) level += strength * (1 - d / radius) ** 2;
    }
    const n = Math.round(level);
    out.set(x, y, { ch: cell.ch, fg: shift(cell.fg, n, cell.bg), bg: shift(cell.bg, n, cell.fg), flags: cell.flags });
  }
  return legalize(out, { bgLimit });
}
