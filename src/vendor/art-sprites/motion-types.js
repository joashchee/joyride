// Motion types: a drawing does not move the way another does, so every recipe belongs to one motion type, and a type has its own presets, its own
// parameters and its own limits. A hen bobs, a tower settles, a flag gusts, a status bar jolts; none of them walk. The generic channels are those of
// motion.js (`dx dy lift squat lean sway`, plus the joints of a `figure`), so a preset is only a clip of poses and `draw(id, { motion: 'bob' })` is
// the same as passing that clip. Nothing is random and there are no Node built-ins.
//
// `motion` on a recipe may now be a clip (as before), the name of one of its type's presets, `{ preset, ...params }`, or a list of them.
// A preset's params are its own (`wind` for a plant, `distance` for a panel); an unknown param or preset throws. Sizes are the art's own units.
import { motionClips } from './motion.js';

const sine = (n, amp, phase = 0) => Array.from({ length: n }, (_, i) => Math.round(amp * Math.sin((2 * Math.PI * (i + phase)) / n)));
// A sine rounded to whole units repeats a pose at small amplitudes; drop each pose that equals the one before it (the loop's wrap included).
const distinct = (poses) => poses.filter((p, i) => JSON.stringify(p) !== JSON.stringify(poses[(i + poses.length - 1) % poses.length]));
const steps = (n, f) => Array.from({ length: n }, (_, i) => f(i));
const preset = (describe, params, make) => ({ describe, params, make });
const ramp = (from, to, n) => steps(n, (i) => Math.round(from + ((to - from) * i) / (n - 1)));
const quarter = (n) => Math.max(1, Math.round(n / 4));

// Shared makers, so two types that move alike say so in one place.
const bob = (key) => (_, p) => ({ frames: sine(p.steps, p.amount).map((v) => ({ [key]: v })), fps: p.fps });
const grow = (key) => (_, p) => ({ frames: ramp(p.from ?? 0, 0, p.steps).map((v) => ({ [key]: v })), fps: p.fps, loop: false });
const shake = (_, p) => ({ frames: steps(p.steps, (i) => ({ dx: i % 2 ? -p.amount : p.amount })), fps: p.fps });
const slideIn = (axis) => (size, p) => {
  const d = p.distance ?? quarter(axis === 'dx' ? size.w : size.h);
  return { frames: ramp(-d, 0, p.steps).map((v) => ({ [axis]: v })), fps: p.fps, loop: false };
};

/** The motion types. `channels`: which pose channels the type uses, each a share of the art (`dx` and `lean` of its width, `dy`, `lift` and `squat` of
 *  its height, at least 1) it may move by, or `true` for a joint channel; `presets`: named clips with their own params and defaults. */
export const MOTION_TYPES = {
  figure: {
    describe: 'A jointed character: it breathes, hops, lunges, recoils, bows and cheers with its own joints',
    channels: { dx: 0.15, dy: 0.15, lean: 0.1, squat: 0.2, lift: 0.25, sway: 0.1, drag: true, head: true, legs: true, arm: true, weapon: true, air: true },
    presets: {
      breathe: preset('Rise and fall of a body at rest', { amount: 1, fps: 2 }, (_, p) => ({ frames: [{}, { dy: p.amount }], fps: p.fps })),
      hop: preset('Crouch, leap, land', { height: 4, fps: 10 }, (_, p) => ({ frames: [{ squat: 2 }, { dy: -p.height, legs: [1, 1], air: 1 }, { dy: -Math.round(p.height / 2), legs: [1, 1], air: 1 }, { squat: 1 }, {}], fps: p.fps, loop: false, events: [{ frame: 1, name: 'takeoff' }, { frame: 3, name: 'land' }] })),
      lunge: preset('A step in with the weapon', { reach: 3, fps: 10 }, (_, p) => ({ frames: [{ dx: -1, arm: 'raised', weapon: 'up' }, { dx: p.reach, arm: 'forward', weapon: 'down' }, {}], fps: p.fps, loop: false, events: [{ frame: 1, name: 'impact' }] })),
      recoil: preset('Pushed back by a blow', { push: 2, fps: 10 }, (_, p) => ({ frames: [{ dx: -p.push, lean: -1, head: 1 }, { dx: -1 }, {}], fps: p.fps, loop: false })),
      bow: preset('A bow, held a beat', { depth: 2, fps: 6 }, (_, p) => ({ frames: [{ lean: p.depth, head: 1 }, { lean: p.depth, head: 2 }, {}], holds: [1, 3, 1], fps: p.fps, loop: false })),
      cheer: preset('Both arms up and a small jump', { height: 2, fps: 6 }, (_, p) => ({ frames: [{ arm: 'raised', dy: -p.height }, { arm: 'up' }], fps: p.fps })),
    },
  },
  head: {
    describe: 'A face, bust or portrait: it only breathes, nods, tilts and flinches; the head does not travel',
    channels: { dx: 0.1, dy: 0.1, lean: 0.1, squat: 0.1 },
    presets: {
      breathe: preset('Rise and fall at rest', { amount: 1, fps: 2 }, (_, p) => ({ frames: [{}, { dy: p.amount }], fps: p.fps })),
      nod: preset('A nod', { depth: 1, fps: 6 }, (_, p) => ({ frames: [{}, { squat: p.depth }], fps: p.fps })),
      tilt: preset('The head tilts one way and the other', { amount: 1, fps: 3 }, (_, p) => ({ frames: [{ lean: -p.amount }, {}, { lean: p.amount }, {}], fps: p.fps })),
      flinch: preset('A start back from a blow', { push: 1, fps: 10 }, (_, p) => ({ frames: [{ dx: -p.push, squat: 1 }, {}], fps: p.fps, loop: false })),
    },
  },
  creature: {
    describe: 'A creature or a race picture: it breathes, hops, pounces, slithers and recoils; no joints, the whole body moves',
    channels: { dx: 0.2, dy: 0.2, lean: 0.2, squat: 0.25, lift: 0.25, sway: 0.2 },
    presets: {
      breathe: preset('Rise and fall at rest', { amount: 1, fps: 2 }, (_, p) => ({ frames: [{}, { squat: p.amount }], fps: p.fps })),
      hop: preset('Crouch, leap, land', { height: 3, fps: 10 }, (_, p) => ({ frames: [{ squat: 2 }, { dy: -p.height }, { squat: 1 }, {}], fps: p.fps, loop: false })),
      pounce: preset('Crouch, spring forward, land', { reach: 4, fps: 10 }, (_, p) => ({ frames: [{ squat: 2 }, { dx: p.reach, dy: -2, lean: 2 }, { dx: p.reach }, {}], fps: p.fps, loop: false, events: [{ frame: 1, name: 'impact' }] })),
      slither: preset('The body waves along its length', { sway: 2, steps: 4, fps: 6 }, (_, p) => ({ frames: sine(p.steps, p.sway).map((v) => ({ sway: v, dx: Math.sign(v) })), fps: p.fps })),
      recoil: preset('Thrown back by a blow', { push: 2, fps: 10 }, (_, p) => ({ frames: [{ dx: -p.push, lean: -1 }, { dx: -1 }, {}], fps: p.fps, loop: false })),
    },
  },
  object: {
    describe: 'A held or carried thing, an icon or a badge: it floats, pulses, swings and drops; it never walks',
    channels: { dy: 0.15, lift: 0.25, lean: 0.2, squat: 0.2 },
    presets: {
      bob: preset('Floats up and down', { amount: 1, steps: 4, fps: 4 }, bob('dy')),
      pulse: preset('Swells and settles', { amount: 1, fps: 4 }, (_, p) => ({ frames: [{}, { squat: p.amount }], fps: p.fps })),
      swing: preset('Swings from the top', { angle: 2, fps: 6 }, (_, p) => ({ frames: [{ lean: -p.angle }, {}, { lean: p.angle }, {}], fps: p.fps })),
      drop: preset('Falls in and bounces once', { height: 4, fps: 10 }, (_, p) => ({ frames: [{ lift: p.height }, {}, { squat: 1 }, {}], fps: p.fps, loop: false, events: [{ frame: 1, name: 'land' }] })),
    },
  },
  limb: {
    describe: 'A hand: it waves, beckons and trembles from the wrist',
    channels: { dx: 0.1, dy: 0.1, lean: 0.25, sway: 0.15, squat: 0.1 },
    presets: {
      wave: preset('Waves side to side', { angle: 2, fps: 6 }, (_, p) => ({ frames: sine(4, p.angle).map((v) => ({ lean: v })), fps: p.fps })),
      beckon: preset('Curls toward the viewer and back', { amount: 2, fps: 5 }, (_, p) => ({ frames: [{}, { lean: p.amount, squat: 1 }, {}, { lean: p.amount, squat: 1 }], fps: p.fps })),
      tremble: preset('A fine shake', { amount: 1, fps: 14 }, (_, p) => ({ frames: [{ dx: p.amount }, { dx: -p.amount }], fps: p.fps })),
    },
  },
  plant: {
    describe: 'Crops, trees and small farm animals: they sway in a wind, rustle, grow from nothing and bounce',
    channels: { lean: 0.2, sway: 0.2, squat: 0.75, dy: 0.1 },
    presets: {
      sway: preset('A slow sway in the wind', { wind: 1, steps: 4, fps: 3 }, (_, p) => ({ frames: sine(p.steps, p.wind).map((v) => ({ lean: v, sway: v })), fps: p.fps })),
      rustle: preset('A quick rustle', { wind: 1, steps: 6, fps: 10 }, (_, p) => ({ frames: steps(p.steps, (i) => ({ lean: i % 3 === 0 ? p.wind : i % 3 === 1 ? -p.wind : 0 })), fps: p.fps })),
      grow: preset('Rises out of the ground (from: rows hidden at first, default 3/4 of the height)', { from: null, steps: 5, fps: 5 }, (size, p) => grow('squat')(size, { ...p, from: p.from ?? Math.round(size.h * 0.75) })),
      bounce: preset('A small hop, for a hen or a sheep', { height: 1, fps: 8 }, (_, p) => ({ frames: [{ squat: 1 }, { dy: -p.height }, {}], fps: p.fps })),
    },
  },
  machine: {
    describe: 'Wagons, cars, locomotives, airships and cannon: they rumble, roll, bump and lurch',
    channels: { dx: 0.15, dy: 0.1, lean: 0.1, squat: 0.1, sway: 0.1, lift: 0.1 },
    presets: {
      rumble: preset('An engine or a rough road', { amount: 1, fps: 12 }, (_, p) => ({ frames: [{}, { dy: -p.amount }], fps: p.fps })),
      roll: preset('Rolls forward and back slowly', { distance: 2, steps: 4, fps: 6 }, (_, p) => ({ frames: sine(p.steps, p.distance).map((v) => ({ dx: v })), fps: p.fps })),
      bump: preset('Over a bump', { height: 2, fps: 8 }, (_, p) => ({ frames: [{}, { dy: -p.height }, { squat: 1 }, {}], fps: p.fps, loop: false })),
      lurch: preset('Starts or stops hard', { push: 2, fps: 8 }, (_, p) => ({ frames: [{ lean: -p.push }, { lean: p.push }, {}], fps: p.fps, loop: false })),
    },
  },
  structure: {
    describe: 'Buildings, towers, furniture and rooms: they settle, shake, creak, go up and come down; they do not walk',
    channels: { dx: 0.05, dy: 0.05, lean: 0.1, sway: 0.05, squat: 1 },
    presets: {
      settle: preset('Sinks a row and rests', { amount: 1, fps: 6 }, (_, p) => ({ frames: [{ squat: p.amount }, {}], fps: p.fps, loop: false })),
      quake: preset('Shakes', { amount: 1, steps: 6, fps: 16 }, shake),
      creak: preset('A slow lean in the wind', { angle: 1, steps: 6, fps: 3 }, (_, p) => ({ frames: distinct(sine(p.steps, p.angle).map((v) => ({ lean: v }))), fps: p.fps })),
      build: preset('Rises from the ground, top last (from: default 3/4 of the height)', { from: null, steps: 6, fps: 6 }, (size, p) => grow('squat')(size, { ...p, from: p.from ?? Math.round(size.h * 0.75) })),
      crumble: preset('Sinks into rubble (to: default 1/2 of the height)', { to: null, steps: 6, fps: 6 }, (size, p) => ({ frames: ramp(0, p.to ?? Math.round(size.h / 2), p.steps).map((v) => ({ squat: v })), fps: p.fps, loop: false, events: [{ frame: p.steps - 1, name: 'gone' }] })),
    },
  },
  tiling: {
    describe: 'Tiles and patterns: they scroll and ripple and wrap round the edge, so the seam stays tiled',
    channels: { dx: 1, dy: 1, lift: 1, sway: 0.25 },
    presets: {
      scroll: preset('Scrolls one whole tile and loops (direction: left, right, up or down)', { direction: 'left', step: 1, fps: 8 }, (size, p) => {
        const vertical = p.direction === 'up' || p.direction === 'down', sign = p.direction === 'left' || p.direction === 'up' ? -1 : 1;
        if (!['left', 'right', 'up', 'down'].includes(p.direction)) throw new Error(`unknown scroll direction "${p.direction}"; known: left, right, up, down`);
        return { frames: steps(Math.ceil((vertical ? size.h : size.w) / p.step), (i) => ({ [vertical ? 'dy' : 'dx']: sign * i * p.step })), fps: p.fps, wrap: true };
      }),
      drift: preset('Drifts down and to the right one whole tile and loops, for clouds or snow', { step: 1, fps: 8 }, (size, p) => ({ frames: steps(Math.ceil(Math.max(size.w, size.h) / p.step), (i) => ({ dx: i * p.step, dy: i * p.step })), fps: p.fps, wrap: true })),
      ripple: preset('The lower half waves, for water', { amount: 1, steps: 4, fps: 6 }, (_, p) => ({ frames: sine(p.steps, p.amount).map((v) => ({ sway: v })), fps: p.fps, wrap: true })),
    },
  },
  scenery: {
    describe: 'Pictures of places (scenes, vistas, landscapes, locales, figures on a place): they tremble and shimmer; the whole picture never travels',
    channels: { dx: 0.05, dy: 0.05, lean: 0.02, sway: 0.05, squat: 0.05 },
    presets: {
      tremor: preset('The ground shakes', { amount: 1, steps: 6, fps: 16 }, shake),
      shimmer: preset('Heat or water: the lower half wavers', { amount: 1, fps: 4 }, (_, p) => ({ frames: [{}, { sway: p.amount }, {}, { sway: -p.amount }], fps: p.fps })),
      breathe: preset('The picture rises and falls a row', { amount: 1, fps: 2 }, (_, p) => ({ frames: [{}, { dy: p.amount }], fps: p.fps })),
    },
  },
  cloth: {
    describe: 'Flags, capes, banners, curtains, hair and robes: the hem moves most, the top stays tied',
    channels: { sway: 0.4, lean: 0.3, dx: 0.1, dy: 0.1 },
    presets: {
      gust: preset('A gust passes along the hem', { wind: 2, steps: 6, fps: 6 }, (_, p) => ({ frames: sine(p.steps, p.wind).map((v, i) => ({ sway: v, lean: i % 2 })), fps: p.fps })),
      billow: preset('Fills and falls back', { wind: 2, fps: 4 }, (_, p) => ({ frames: [{ sway: p.wind }, { sway: -p.wind }], fps: p.fps })),
      snap: preset('Snaps once in a gust', { wind: 3, fps: 12 }, (_, p) => ({ frames: [{}, { sway: p.wind, lean: 1 }, { sway: -1 }, {}], fps: p.fps, loop: false })),
      flutter: preset('A quick flutter', { wind: 1, fps: 14 }, (_, p) => ({ frames: [{ sway: p.wind }, { sway: -p.wind }], fps: p.fps })),
    },
  },
  effect: {
    describe: 'Combat and magic effects: they pulse, burst, rise and shake; they begin and end',
    channels: { dy: 0.5, lift: 0.5, squat: 1, dx: 0.25, lean: 0.25, sway: 0.25 },
    presets: {
      pulse: preset('Swells and shrinks', { amount: 2, fps: 8 }, (_, p) => ({ frames: [{}, { squat: p.amount }], fps: p.fps })),
      burst: preset('Opens from a point (from: default 3/4 of the height)', { from: null, steps: 4, fps: 12 }, (size, p) => grow('squat')(size, { ...p, from: p.from ?? Math.round(size.h * 0.75) })),
      rise: preset('Floats up and away', { height: 4, steps: 4, fps: 8 }, (_, p) => ({ frames: ramp(0, p.height, p.steps).map((v) => ({ lift: v })), fps: p.fps, loop: false })),
      shake: preset('Shakes', { amount: 1, steps: 4, fps: 16 }, shake),
    },
  },
  panel: {
    describe: 'Panels and screens (status bar, map, sheet, menu, fight, artpack): they jolt, shake and slide or drop in; their words never wave',
    channels: { dx: 1, dy: 1 },
    presets: {
      jolt: preset('A hard knock and back', { push: 2, fps: 16 }, (_, p) => ({ frames: [{ dx: p.push }, { dx: -1 }, {}], fps: p.fps, loop: false })),
      shake: preset('Shakes', { amount: 1, steps: 6, fps: 16 }, shake),
      'slide-in': preset('Slides in from the left (distance: default 1/4 of the width)', { distance: null, steps: 4, fps: 12 }, slideIn('dx')),
      'drop-in': preset('Drops in from above (distance: default 1/4 of the height)', { distance: null, steps: 4, fps: 12 }, slideIn('dy')),
    },
  },
  lettering: {
    describe: 'Block lettering and titles: they wave, wobble and come in; a word can lean but its letters stay whole',
    channels: { dx: 1, dy: 1, sway: 0.15, lean: 0.15, squat: 0.15 },
    presets: {
      wave: preset('A wave runs along the line', { amount: 1, steps: 6, fps: 6 }, (_, p) => ({ frames: distinct(sine(p.steps, p.amount).map((v) => ({ sway: v }))), fps: p.fps })),
      wobble: preset('Rocks on its feet', { amount: 1, fps: 4 }, (_, p) => ({ frames: [{ lean: -p.amount }, {}, { lean: p.amount }, {}], fps: p.fps })),
      'slide-in': preset('Slides in from the left (distance: default 1/4 of the width)', { distance: null, steps: 4, fps: 12 }, slideIn('dx')),
      'drop-in': preset('Drops in from above (distance: default 1/4 of the height)', { distance: null, steps: 4, fps: 12 }, slideIn('dy')),
    },
  },
};

/** The motion type of every recipe. A test fails if a registered recipe is missing here. */
export const MOTION_OF = {
  character: 'figure',
  face: 'head', portrait: 'head', bust: 'head', creatureface: 'head',
  creature: 'creature', creation: 'creature',
  item: 'object', thing: 'object', icon: 'object', sigil: 'object', emblem: 'object', placeholder: 'object', overlay: 'object', skillicon: 'object', telegraph: 'effect', target: 'object',
  hand: 'limb',
  farm: 'plant', prop: 'plant',
  vehicle: 'machine',
  building: 'structure', strategy: 'structure', furniture: 'structure', room: 'structure',
  tile: 'tiling', pattern: 'tiling', dress: 'scenery', splash: 'scenery', map: 'panel', town: 'scenery', material: 'object', currency: 'object', exhibit: 'structure', award: 'object',
  scene: 'scenery', vista: 'scenery', landscape: 'scenery', locale: 'scenery', onplace: 'scenery', weatherloop: 'scenery',
  cloth: 'cloth',
  effect: 'effect',
  bar: 'panel', mappane: 'panel', sheet: 'panel', menu: 'panel', fight: 'panel', artpack: 'panel',
  banner: 'lettering',
};

/** The motion type (id, describe, channels, presets) of a recipe. Throws on a recipe with none. */
export function motionType(recipe) {
  const id = MOTION_OF[recipe]; if (!id) throw new Error(`recipe "${recipe}" has no motion type`);
  return { id, ...MOTION_TYPES[id] };
}

/** The names of a recipe's presets. */
export const motionPresets = (recipe) => Object.keys(motionType(recipe).presets);

function presetClip(recipe, spec, size) {
  const { id: type, presets } = motionType(recipe), { preset: name, id, ...given } = typeof spec === 'string' ? { preset: spec } : spec, p = presets[name];
  if (!p) throw new Error(`unknown motion "${name}" for ${recipe} (a ${type}); known: ${Object.keys(presets).join(', ')}`);
  for (const k of Object.keys(given)) if (!(k in p.params)) throw new Error(`unknown param "${k}" for motion "${name}" of ${recipe}; known: ${Object.keys(p.params).join(', ')}`);
  const params = { ...p.params, ...given };
  for (const [k, v] of Object.entries(params)) if (v !== null && typeof v !== typeof p.params[k] && p.params[k] !== null) throw new Error(`param "${k}" of motion "${name}" must be a ${typeof p.params[k]}`);
  return { id: id ?? name, ...p.make(size, params) };
}

/** A recipe's `motion` with presets turned into clips: a clip passes through unchanged, a name or `{ preset, ...params }` is made for the art's `size`
 *  (`{ w, h }`, the first frame's). Returns a list of clips (empty for no motion). */
export function resolveMotion(recipe, motion, size) {
  return [].concat(motion ?? []).map((m) => (typeof m === 'string' || (m && typeof m === 'object' && 'preset' in m) ? presetClip(recipe, m, size) : m));
}

/** What a motion asks of a recipe that its type does not use: a channel the type lacks (`channel`) or a move past the type's limit (`range`).
 *  Findings only, never a throw: `[{ rule, clip, frame, channel, message }]`. Pass the art's `size` (`{ w, h }`). */
export function lintMotion(recipe, motion, size) {
  const { id: type, channels } = motionType(recipe), out = [];
  const unit = { dx: size.w, lean: size.w, sway: size.w, dy: size.h, lift: size.h, squat: size.h };
  for (const c of motionClips(resolveMotion(recipe, motion, size))) c.frames.forEach((f, frame) => {
    for (const [k, v] of Object.entries(f)) {
      if (!(k in channels)) { out.push({ rule: 'channel', clip: c.id, frame, channel: k, message: `a ${type} does not use "${k}"` }); continue; }
      const share = channels[k], max = share === true ? Infinity : Math.max(1, Math.floor(share * unit[k]));
      if (typeof v === 'number' && Math.abs(v) > max) out.push({ rule: 'range', clip: c.id, frame, channel: k, message: `${k} ${v} is past ${max}, the most a ${type} moves` });
    }
  });
  return out;
}
