// Animation helpers. Frames are plain Pixels; a clip is frames plus timing and named event markers (the impact frame, say).
// Principles from the learning kit: largest masses first, intentional frame-to-frame change, 4 to 6 frames for a cycle.
import { Pixels } from './pixels.js';
import { C } from './palette.js';

/** Vertical squash/stretch anchored to the bottom row of the art (f < 1 squashes), nearest neighbour, so it stays on the pixel grid. */
export function squash(px, f) {
  const b = px.bbox(); if (!b) return px.clone();
  const nh = Math.max(1, Math.round(b.h * f)), out = new Pixels(px.w, px.h, px.palette), bottom = b.y + b.h;
  for (let y = 0; y < nh; y++) { const sy = b.y + Math.min(b.h - 1, Math.floor((y * b.h) / nh)); for (let x = 0; x < px.w; x++) out.set(x, bottom - nh + y, px.get(x, sy)); }
  return out;
}
/** The hit flash: the same silhouette in one colour. */
export const flash = (px, c = C.white) => px.silhouette(c);
/** Breathing: the art one pixel up on the second frame, or squashed for soft bodies. */
export const idle = (px, soft = false) => [px, soft ? squash(px, 0.9) : px.shift(0, -1)];
/** Collapse in `n` frames: squash toward the floor, the last frame nearly flat. */
export const collapse = (px, n = 4) => Array.from({ length: n }, (_, i) => squash(px, 1 - (0.75 * (i + 1)) / n));
/** A clip record: what the manifest's animation entry is made of. */
export const clip = (id, frames, { fps = 8, loop = true, events = [], reduced } = {}) => ({ id, frames, fps, loop, events, reduced: reduced ?? [frames[0]] });
