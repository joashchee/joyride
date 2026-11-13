// The Painter bridge: a clip's poses drawn into frames. Animator imports nothing from Painter, so the drawing is a function the
// caller passes: `drawFrame(pose, index)` returns one frame (Painter's Pixels or Cells, or anything). The result has the shape of
// Painter's own clip record (id, frames, fps, loop, events, reduced), so it drops in where `art.clips.walk` goes.
//
//   import { animate, bind } from "sprite-animator";
//   import { draw } from "art-sprites";
//   const walk = bind(animate('walk'), (pose) => draw('character', { loadout: 'rogue', frames: [pose] }).frames[0]);
//
// Painter 1.19+ also takes a clip as is, as the `motion` param of any recipe (`draw('character', { motion: clip })`), and needs no
// bind; `bind` is for drawing the poses with your own function. See docs/animator-plan.md, "The Painter hand-off".
export function bind(c, drawFrame) {
  if (typeof drawFrame !== 'function') throw new Error('bind needs drawFrame(pose, index)');
  const one = (p, i) => { const f = drawFrame(p, i); if (f == null) throw new Error(`drawFrame returned nothing for frame ${i} of ${c.id}`); return f; };
  return { id: c.id, frames: c.frames.map(one), fps: c.fps, loop: c.loop, events: c.events, reduced: c.reduced.map(one), holds: c.holds };
}
