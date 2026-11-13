// The motion lint: the craft rules for a clip, as checks. Each finding is { level: 'error' | 'warn', rule, message }.
//   shape    the clip is well formed (integers, one hold per frame, events on frames)
//   frames   the count is within `min`..`max` (default 2..16)
//   seam     a loop's last pose is no further from its first than its biggest step inside (a quarter over is rounding when `frames` is not a
//            multiple of the key count): no pop at the wrap
//   still    two neighbouring frames are the same pose (a wasted frame; raise `hold` instead)
//   range    a channel is further out than a sprite can take (`limits` overrides)
//   type     a clip made by a motion-type recipe (`figure.hop`, spec.recipe) uses only its type's channels and stays within its limits; its `range`
//            is the type's, a share of the art's size, not the sprite limits below, and a `wrap` clip (a scrolling tile) has no seam to check
//   kind     the rules of the motion itself: a walk always has a foot down and steps twice, a run has flight, a jump leaves and lands
import { NUMERIC, checkShape, frameDelta } from './clip.js';
import { MOTION_TYPES, lintMotionType } from './types.js';

const LIMITS = { dx: 8, dy: 9, lean: 4, head: 3, squat: 4, lift: 4, drag: 4, sway: 3, legs: 5 };

export function lintClip(c, { min = 2, max, limits = {}, skip = [] } = {}) {
  const out = [], add = (level, rule, message) => { if (!skip.includes(rule)) out.push({ level, rule, message: `${c?.id ?? 'clip'}: ${message}` }); };
  const bad = checkShape(c);
  if (bad.length) { for (const m of bad) add('error', 'shape', m); return out; }       // nothing else is safe to read
  const n = c.frames.length, lim = { ...LIMITS, ...limits }, type = typeOf(c);
  max ??= c.wrap ? 64 : 16;                                       // a tile that scrolls one whole tile takes a frame per step
  if (n < min || n > max) add('error', 'frames', `${n} frames, expected ${min} to ${max}`);

  const steps = c.frames.map((f, i) => (i + 1 < n ? frameDelta(f, c.frames[i + 1]) : c.loop ? frameDelta(f, c.frames[0]) : null)).filter((d) => d !== null);
  if (c.loop && n > 2 && !c.wrap) {
    const inner = Math.max(...steps.slice(0, -1));
    if (steps[steps.length - 1] > inner * 1.25 + 1) add('error', 'seam', `the wrap from the last frame to the first moves ${steps[steps.length - 1]}, more than any step inside (${inner}): it will pop`);
  }
  steps.forEach((d, i) => { if (d === 0 && c.holds[i] === 1) add('warn', 'still', `frames ${i} and ${(i + 1) % n} are the same pose; hold one instead of drawing both`); });

  if (type) for (const f of lintMotionType(c, type, sizeOf(c))) add(f.level, f.rule, f.message);
  else c.frames.forEach((f, i) => {
    for (const k of NUMERIC) if (Math.abs(f[k]) > lim[k]) add('warn', 'range', `frame ${i} ${k} ${f[k]} is past ${lim[k]}`);
    if (f.legs.some((v) => Math.abs(v) > lim.legs)) add('warn', 'range', `frame ${i} legs ${f.legs} are past ${lim.legs}`);
  });
  kindRules(c, add);
  return out;
}

// The motion type a clip was made for, from its recipe id (`figure.hop`), and the art size it was made for.
const typeOf = (c) => { const t = c.spec?.recipe?.split('.')[0]; return c.spec?.recipe?.includes('.') && MOTION_TYPES[t] ? t : null; };
const sizeOf = (c) => ({ w: c.spec.params?.w ?? 32, h: c.spec.params?.h ?? 32 });

const framesOf = (c, name) => c.events.filter((e) => e.name === name).map((e) => e.frame);

function kindRules(c, add) {
  const n = c.frames.length, air = c.frames.map((f) => f.air === 1);
  if (c.kind === 'walk' || c.kind === 'run') {
    if (!c.loop) add('error', 'kind', `a ${c.kind} is a loop`);
    const steps = framesOf(c, 'step');
    if (steps.length < 2) add('error', 'kind', `a ${c.kind} needs a "step" event for each foot, found ${steps.length}`);
    else {
      const gaps = steps.map((f, i) => (steps[(i + 1) % steps.length] - f + n) % n);
      if (Math.max(...gaps) - Math.min(...gaps) > 1) add('warn', 'kind', `the steps are unevenly spaced (${gaps.join(', ')} frames apart)`);
      for (const f of steps) if (air[f]) add('error', 'kind', `a step lands on frame ${f}, which is in the air`);
    }
    if (c.kind === 'walk' && air.some(Boolean)) add('error', 'kind', 'a walk always has a foot down, but a frame is in the air (that is a run)');
    if (c.kind === 'run' && (!air.some(Boolean) || air.every(Boolean))) add('error', 'kind', 'a run needs both flight frames and contact frames (no flight is a walk)');
  }
  if (c.kind === 'jump') {
    if (c.loop) add('error', 'kind', 'a jump is a one-shot, not a loop');
    const [off, ...more] = framesOf(c, 'takeoff'), [land, ...extra] = framesOf(c, 'land');
    if (off === undefined || land === undefined || more.length || extra.length) add('error', 'kind', 'a jump needs exactly one "takeoff" and one "land" event');
    else {
      if (off >= land) add('error', 'kind', 'takeoff must come before land');
      for (let i = off; i < land; i++) if (!air[i]) add('error', 'kind', `frame ${i} is between takeoff and land but on the ground`);
      if (air[off - 1] || air[land]) add('error', 'kind', 'the frames just before takeoff and at land must be on the ground');
      if (Math.min(...c.frames.slice(off, land).map((f) => f.dy)) >= 0) add('error', 'kind', 'the body never rises above the ground');
    }
    const end = c.frames[n - 1];
    if (end.air || end.dy !== 0) add('warn', 'kind', 'a jump should end on the ground at rest, so the next clip can follow it');
  }
}
