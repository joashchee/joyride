// Locomotion loops with the fewest frames that still read: walk (4), run (4), jump (6). Each is a few keys; `frames: n` blends
// them to a smoother clip. A key's pose uses Painter's `character` channels (see clip.js), so Painter can draw it as it is.
// y is up-negative; `legs` are the two feet's offsets (negative back); `air` is 1 while no foot is down.
import { rng } from '../rng.js';
import { clip, fromKeys } from '../clip.js';

const int = (name, v, lo, hi) => { if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} must be an integer from ${lo} to ${hi}, not ${v}`); return v; };

// Head bob from the seed, off by default: a clip with `wobble: 0` draws no randomness at all.
// A loop's last bob repeats its first, so the wrap never gets bigger than a step inside.
const wobbled = (frames, wobble, seed, loop) => {
  if (!wobble) return frames;
  const r = rng(seed).fork('wobble'), bob = frames.map(() => r.range(-wobble, wobble));
  if (loop) bob[bob.length - 1] = bob[0];
  return frames.map((f, i) => ({ ...f, head: f.head + bob[i] }));
};

const build = (id, kind, keys, { loop, frames, span, fps, wobble, seed, spec }) => {
  const m = fromKeys(keys, { loop, frames, span });
  const params = Object.fromEntries(Object.entries(spec.params).filter(([, v]) => v !== undefined));   // JSON drops undefined, so leave it out up front
  return clip(id, wobbled(m.frames, wobble, seed, loop), { fps, loop, kind, events: m.events, holds: m.holds, spec: { ...spec, params } });
};

/** Walk: contact, passing, contact, passing. One foot is always down, the body is lowest at contact and highest as the legs pass. */
export function drawWalk({ stride = 2, bounce = 1, frames, fps = 8, wobble = 0, seed = 'walk' } = {}) {
  int('stride', stride, 1, 4); int('bounce', bounce, 0, 3); int('wobble', wobble, 0, 2);
  const keys = [
    { legs: [-stride, stride], dy: 0, drag: 2, sway: 1, event: 'step' },
    { legs: [0, 0], dy: -bounce, drag: 0, sway: 0 },
    { legs: [stride, -stride], dy: 0, drag: 1, sway: -1, event: 'step' },
    { legs: [0, 0], dy: -bounce, drag: 0, sway: 0 },
  ];
  return build('walk', 'walk', keys, { loop: true, frames, fps, wobble, seed, spec: { recipe: 'walk', params: { stride, bounce, frames, fps, wobble, seed } } });
}

/** Run: contact, flight, contact, flight. The difference from a walk is the flight: for a moment no foot is down. */
export function drawRun({ stride = 3, lean = 2, rise = 2, frames, fps = 12, wobble = 0, seed = 'run' } = {}) {
  int('stride', stride, 2, 5); int('lean', lean, 0, 4); int('rise', rise, 1, 4); int('wobble', wobble, 0, 2);
  const keys = [
    { legs: [-stride, stride], dy: 0, squat: 1, lean, drag: 3, sway: 1, arm: 'swing', event: 'step' },
    { legs: [1, 1], dy: -rise, lean, drag: 1, sway: 1, arm: 'swing', air: 1 },
    { legs: [stride, -stride], dy: 0, squat: 1, lean, drag: 3, sway: -1, arm: 'swing', event: 'step' },
    { legs: [1, 1], dy: -rise, lean, drag: 1, sway: -1, arm: 'swing', air: 1 },
  ];
  return build('run', 'run', keys, { loop: true, frames, fps, wobble, seed, spec: { recipe: 'run', params: { stride, lean, rise, frames, fps, wobble, seed } } });
}

/** Jump, in place: crouch, takeoff, apex, fall, land, recover. It does not loop; it starts crouched and ends standing. */
export function drawJump({ height = 5, crouch = 2, frames, fps = 10, wobble = 0, seed = 'jump' } = {}) {
  int('height', height, 2, 9); int('crouch', crouch, 1, 3); int('wobble', wobble, 0, 2);
  const mid = Math.round(height * 0.6);
  const keys = [
    { at: 0, squat: crouch, lean: 1, hold: 2 },
    { at: 2, dy: -mid, legs: [0, 0], lift: 1, air: 1, event: 'takeoff' },
    { at: 4, dy: -height, legs: [1, 1], lift: 1, air: 1, hold: 2 },
    { at: 6, dy: -mid, legs: [0, 0], air: 1 },
    { at: 8, squat: crouch, lean: 1, event: 'land' },
    { at: 10 },
  ];
  return build('jump', 'jump', keys, { loop: false, frames, fps, wobble, seed, spec: { recipe: 'jump', params: { height, crouch, frames, fps, wobble, seed } } });
}
