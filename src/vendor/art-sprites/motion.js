// Motion for anything Painter draws. A motion is a clip of poses made elsewhere (Animator's clips are this shape):
// `{ id, frames: [pose], fps, loop, events, holds?, reduced? }`. The `character` recipe draws a pose with its own joints; every other recipe
// (and every style) gets the generic move below, applied to the recipe's still (its first frame), so a drawing can walk, hop or squash
// without a recipe knowing about motion. The units are the art's own: pixels for a pixel drawing, cells for ANSI and text.
//
// What a pose does to a still (y is up-negative, as in the poses):
//   dx dy lift   move it (`lift` raises it)
//   squat        squashes it from the bottom by that many rows, nearest row (a landing, a crouch, a slime)
//   lean         shears it: the top row moves by `lean`, easing to nothing at the bottom
//   sway         shears the lower half: the bottom row moves by `sway`, easing to nothing at the middle (a hem, a tail)
// A clip with `wrap: true` carries what leaves one edge in at the other (a tile that scrolls and stays tiled), for `dx dy lift`.
// `head drag legs arm weapon air` belong to a figure's joints and are ignored here (the lint reads `air`).
// Nothing is random, so the same drawing and the same clip give the same frames. No Node built-ins.
import { Cells } from './cells.js';
import { clip } from './anim.js';

export const POSE_KEYS = ['dx', 'dy', 'lean', 'head', 'squat', 'lift', 'drag', 'sway', 'legs', 'arm', 'weapon', 'air'];
const MOVE_KEYS = ['dx', 'dy', 'lift', 'squat', 'lean', 'sway'];

/** A motion (a clip, or a list of clips) checked and given ids: [{ id, frames, holds, events, fps, loop, reduced }]. Throws on a bad clip or pose channel. */
export function motionClips(motion) {
  return [].concat(motion ?? []).map((m, n) => {
    if (!m || !Array.isArray(m.frames) || !m.frames.length || m.frames.some((f) => !f || typeof f !== 'object')) throw new Error('motion needs a clip with a frames list of poses');
    for (const f of [...m.frames, ...(m.reduced ?? [])]) for (const k of Object.keys(f)) if (!POSE_KEYS.includes(k)) throw new Error(`unknown pose channel "${k}" in motion`);
    const holds = m.holds ?? m.frames.map(() => 1);
    if (holds.length !== m.frames.length || holds.some((h) => !Number.isInteger(h) || h < 1)) throw new Error('motion holds must be one integer of 1 or more per frame');
    return { id: m.id ?? `motion${n || ''}`, frames: m.frames, holds, events: m.events ?? [], fps: m.fps ?? 8, loop: m.loop ?? true, reduced: m.reduced, wrap: !!m.wrap };
  });
}

/** A Painter clip from drawn poses: each frame repeated by its hold, event frames moved to match. */
export function motionToClip(m, drawn, reduced) {
  const start = m.holds.map((_, i) => m.holds.slice(0, i).reduce((a, b) => a + b, 0));
  return clip(m.id, drawn.flatMap((f, i) => Array(m.holds[i]).fill(f)), { fps: m.fps, loop: m.loop, events: m.events.map((e) => ({ ...e, frame: start[e.frame] })), reduced });
}

const isCells = (art) => art instanceof Cells;
const empty = (art, x, y) => (isCells(art) ? art.isEmpty(x, y) : art.get(x, y) === 0);
function bounds(art) {
  let x0 = art.w, y0 = art.h, x1 = -1, y1 = -1;
  for (let y = 0; y < art.h; y++) for (let x = 0; x < art.w; x++) if (!empty(art, x, y)) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
  return x1 < 0 ? null : { y: y0, h: y1 - y0 + 1 };
}

/** One still (Pixels or Cells) moved into a pose. The still is not changed; the canvas keeps its size and what leaves it is cut. */
export function moveFrame(still, pose = {}, { wrap = false } = {}) {
  const p = Object.fromEntries(MOVE_KEYS.map((k) => [k, pose[k] ?? 0])), b = wrap ? { y: 0, h: still.h } : bounds(still), mod = (n, m) => (wrap ? ((n % m) + m) % m : n);
  const out = isCells(still) ? new Cells(still.w, still.h) : still.map(() => 0);
  if (!b) return out;
  const bottom = b.y + b.h, nh = Math.max(1, b.h - p.squat), mid = b.y + Math.floor(b.h / 2), lowest = Math.max(1, bottom - 1 - mid);
  for (let y = 0; y < still.h; y++) {
    const y1 = mod(y - p.dy + p.lift, still.h);                                            // undo the move
    if (y1 < bottom - nh || y1 >= bottom) continue;                            // above the squashed art, or below it
    const sy = b.y + Math.floor(((y1 - (bottom - nh)) * b.h) / nh);            // undo the squash: the source row
    const top = b.h > 1 ? Math.round((p.lean * (bottom - 1 - sy)) / (b.h - 1)) : 0, hem = sy > mid ? Math.round((p.sway * (sy - mid)) / lowest) : 0;
    for (let x = 0; x < still.w; x++) {
      const sx = mod(x - p.dx - top - hem, still.w);
      if (sx < 0 || sx >= still.w || empty(still, sx, sy)) continue;
      if (isCells(still)) { const c = still.get(sx, sy); if (!still.tc) { delete c.fgRgb; delete c.bgRgb; } out.set(x, y, c); } else out.set(x, y, still.get(sx, sy));
    }
  }
  return out;
}

/** An art with `motion` added to its `clips`, drawn from its first frame. Art that already has a clip of that id keeps the motion's. */
export function withMotion(art, motion) {
  const list = motionClips(motion), still = art.frames[0], clips = { ...(art.clips ?? {}) };
  for (const m of list) clips[m.id] = motionToClip(m, m.frames.map((f) => moveFrame(still, f, m)), m.reduced?.length ? m.reduced.map((f) => moveFrame(still, f, m)) : undefined);
  return { ...art, clips };
}
