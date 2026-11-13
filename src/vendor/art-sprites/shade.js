// The learning kit's shading rules as one function: a single light from the top left, shading in clusters on the edges
// that face away from it, a highlight on the edges that face it, then a closing outline. Input is a mask whose values are
// part ids; `parts` maps each id to a four-step ramp (see palette.js).
import { Pixels } from './pixels.js';
import { C } from './palette.js';

export function shade(mask, parts, { outline = C.night } = {}) {
  const out = new Pixels(mask.w, mask.h, mask.palette);
  for (let y = 0; y < mask.h; y++) for (let x = 0; x < mask.w; x++) {
    const id = mask.get(x, y);
    if (!id) continue;
    const r = parts[id];
    if (!r) throw new Error(`no ramp for part ${id}`);
    const up = mask.get(x, y - 1), left = mask.get(x - 1, y), down = mask.get(x, y + 1), right = mask.get(x + 1, y);
    const lit = !up || !left, dark = !down || !right;
    out.set(x, y, lit && !dark ? r[3] : dark && !lit ? (!down && !right ? r[0] : r[1]) : r[2]);
  }
  return outline ? out.outline(outline) : out;
}

/** Edge pixels of one part, recoloured as another part: a rim on a shield, a trim on a helmet. */
export function rim(mask, from, to) {
  const src = mask.clone();
  for (let y = 0; y < mask.h; y++) for (let x = 0; x < mask.w; x++)
    if (src.get(x, y) === from && (!src.get(x - 1, y) || !src.get(x + 1, y) || !src.get(x, y - 1) || !src.get(x, y + 1))) mask.set(x, y, to);
  return mask;
}

/** Checkerboard dither of a mask in one colour: how a soft shadow is drawn without alpha. */
export function dither(mask, c, phase = 0) {
  return mask.map((x, y) => (mask.get(x, y) && (x + y + phase) % 2 === 0 ? c : 0));
}
