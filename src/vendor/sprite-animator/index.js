// The browser-safe entry point: everything that makes and checks a clip, with no Node built-ins anywhere behind it, so it runs in a
// page, a worker, Node or Deno unchanged. Only `cli.js` touches `node:`.
//
//   import { animate, bind, tempo } from "sprite-animator";
//   const walk = animate('walk');                       // { id, kind, frames: [pose x4], fps, loop, events, holds, reduced, spec }
//   const beat = tempo(walk, { bpm: 120 });             // frame lengths in ms and event ticks, a step on every beat
export { rng, hashSeed } from './rng.js';
export { NUMERIC, STEPPED, CHANNELS, REST, pose, clip, fromKeys, durations, frameDelta, checkShape, writeClip, readClip } from './clip.js';
export { lintClip } from './lint.js';
export { MOTION_TYPES, MOTION_OF, motionTypeOf, motionPresets, motionClip, lintMotionType } from './types.js';
export { tempo, PPQ } from './tempo.js';
export { bind } from './bind.js';
export { RECIPES, RECIPE_IDS, animate } from './recipes/index.js';
export { KITS, registerKit, coverage, subjectStatus } from './kits/index.js';
