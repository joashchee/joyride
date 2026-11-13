// The motion clip: what Animator makes. A clip is a list of poses (one per frame) plus timing and named event markers (a footfall, the
// takeoff). It holds no pixels: Painter (or anything else) draws each pose, see bind.js. Every channel is an integer in the art's own
// units (pixels, or cells), so a clip is exact and hashable. The channels are the pose vocabulary of Painter's `character` recipe
// (dx dy lean head squat lift drag sway legs arm weapon) plus `air`, which Painter ignores and the lint reads.

/** Channels that blend between keys (rounded to an integer). */
export const NUMERIC = ['dx', 'dy', 'lean', 'head', 'squat', 'lift', 'drag', 'sway'];
/** Channels that hold the value of the key before them. */
export const STEPPED = ['arm', 'weapon', 'air'];
export const CHANNELS = [...NUMERIC, 'legs', ...STEPPED];
/** Standing still: y is up-negative (dy -1 lifts the body), `legs` are the two feet's offsets (negative back), `air` is 1 while no foot is down. */
export const REST = Object.freeze({ dx: 0, dy: 0, lean: 0, head: 0, squat: 0, lift: 0, drag: 0, sway: 0, legs: Object.freeze([0, 0]), arm: 'down', weapon: 'up', air: 0 });

/** A full pose from the channels given; the rest stay at REST. An unknown channel is a typo, so it throws. */
export function pose(o = {}) {
  for (const k of Object.keys(o)) if (!CHANNELS.includes(k)) throw new Error(`unknown pose channel "${k}"; known: ${CHANNELS.join(', ')}`);
  const p = {};
  for (const c of CHANNELS) p[c] = c === 'legs' ? (o.legs ?? REST.legs).map((v) => v + 0) : NUMERIC.includes(c) ? (o[c] ?? REST[c]) + 0 : (o[c] ?? REST[c]);   // + 0 turns -0 into 0, which JSON would lose
  return p;
}

/** A clip record: Painter's `clip` (id, frames, fps, loop, events, reduced) plus `kind`, `holds` (frame lengths, in frames), `spec` (recipe and params) and `wrap` (only when true: `dx dy lift` carry what leaves one edge in at the other, so a scrolling tile stays tiled). */
export const clip = (id, frames, { fps = 8, loop = true, events = [], holds, kind = '', reduced, spec, wrap = false } = {}) =>
  ({ id, kind, frames, fps, loop, ...(wrap ? { wrap } : {}), events, holds: holds ?? frames.map(() => 1), reduced: reduced ?? [pose()], spec });

// A pose between two poses: n/d of the way, each channel rounded, so the result stays an integer. Stepped channels hold the first.
const mix = (a, b, n, d) => {
  const p = { ...a, legs: [0, 1].map((i) => Math.round((a.legs[i] * (d - n) + b.legs[i] * n) / d) + 0) };
  for (const c of NUMERIC) p[c] = Math.round((a[c] * (d - n) + b[c] * n) / d) + 0;
  return p;
};

/**
 * Poses from keys. A key is a pose (any channels) plus `at` (its time, an integer; default its index), `hold` (frames it lasts
 * on screen, default 1) and `event` (a name or names). With no `frames`, the keys are the frames, which is the fewest a motion
 * needs. With `frames: n` the keys are sampled at n even times, blending between them: a loop wraps from the last key to the first
 * over `span` (default: the key count), and a one-shot ends exactly on its last key.
 */
export function fromKeys(keys, { loop = true, frames, span } = {}) {
  const ks = keys.map(({ at, hold, event, ...p }, i) => ({ at: at ?? i, hold: hold ?? 1, events: [].concat(event ?? []), pose: pose(p) }));
  if (ks.length < 2) throw new Error('a motion needs at least two keys');
  ks.forEach((k, i) => { if (!Number.isInteger(k.at) || (i && k.at <= ks[i - 1].at)) throw new Error('key times must be strictly increasing integers'); });
  if (frames === undefined) return { frames: ks.map((k) => k.pose), holds: ks.map((k) => k.hold), events: ks.flatMap((k, i) => k.events.map((name) => ({ frame: i, name }))) };
  if (!Number.isInteger(frames) || frames < 2 || frames > 64) throw new Error(`frames must be an integer from 2 to 64, not ${frames}`);
  const end = loop ? (span ?? ks.length) : ks[ks.length - 1].at, den = loop ? frames : frames - 1, last = ks.length - 1;
  if (end <= ks[last].at && loop) throw new Error('a loop span must run past its last key');
  // A key owns the frames from the one nearest its time, so a key's event and its stepped channels (air, arm) change on the same frame.
  const frameOf = (at) => (loop ? Math.round((at * frames) / end) % frames : Math.round((at * (frames - 1)) / end)), owns = ks.map((k) => frameOf(k.at));
  const out = [];
  for (let j = 0; j < frames; j++) {
    const num = j * end;                                        // the sample time is num / den
    let i = last;
    for (let k = 0; k < last; k++) if (num < ks[k + 1].at * den) { i = k; break; }
    let held = 0;
    owns.forEach((f, k) => { if (f <= j) held = k; });
    const a = ks[i], h = ks[held].pose;
    if (!loop && i === last) { out.push({ ...a.pose, arm: h.arm, weapon: h.weapon, air: h.air }); continue; }
    const b = loop ? ks[(i + 1) % ks.length] : ks[i + 1], nextAt = i === last ? end : ks[i + 1].at;
    out.push({ ...mix(a.pose, b.pose, num - a.at * den, (nextAt - a.at) * den), arm: h.arm, weapon: h.weapon, air: h.air });
  }
  return { frames: out, holds: out.map(() => 1), events: ks.flatMap((k, i) => k.events.map((name) => ({ frame: owns[i], name }))).sort((x, y) => x.frame - y.frame) };
}

/** How long each frame shows, in whole milliseconds. */
export const durations = (c) => c.frames.map((_, i) => Math.round((1000 * c.holds[i]) / c.fps));

/** How far apart two poses are: the sum of every channel's step, plus one for each changed arm or weapon. */
export function frameDelta(a, b) {
  let d = Math.abs(a.legs[0] - b.legs[0]) + Math.abs(a.legs[1] - b.legs[1]) + (a.arm !== b.arm ? 1 : 0) + (a.weapon !== b.weapon ? 1 : 0) + Math.abs(a.air - b.air);
  for (const c of NUMERIC) d += Math.abs(a[c] - b[c]);
  return d;
}

/** What is wrong with a clip's structure, as messages (empty if it is sound). Used by the lint and by `readClip`. */
export function checkShape(c) {
  const bad = [];
  if (!c || typeof c !== 'object') return ['not a clip'];
  if (typeof c.id !== 'string' || !c.id) bad.push('id must be a non-empty string');
  if (!Array.isArray(c.frames) || !c.frames.length) return [...bad, 'a clip needs at least one frame'];
  if (!Number.isInteger(c.fps) || c.fps < 1) bad.push(`fps must be a positive integer, not ${c.fps}`);
  if (typeof c.loop !== 'boolean') bad.push('loop must be true or false');
  if (c.wrap !== undefined && c.wrap !== true) bad.push('wrap must be true, or left out');
  if (!Array.isArray(c.holds) || c.holds.length !== c.frames.length || c.holds.some((h) => !Number.isInteger(h) || h < 1)) bad.push('holds must be one integer of 1 or more per frame');
  c.frames.forEach((f, i) => {
    for (const k of CHANNELS) if (!(k in f)) bad.push(`frame ${i} has no ${k}`);
    for (const k of NUMERIC) if (k in f && !Number.isInteger(f[k])) bad.push(`frame ${i} ${k} is not an integer (${f[k]})`);
    if (!Array.isArray(f.legs) || f.legs.length !== 2 || !f.legs.every(Number.isInteger)) bad.push(`frame ${i} legs must be two integers`);
    if (f.air !== 0 && f.air !== 1) bad.push(`frame ${i} air must be 0 or 1`);
    if (typeof f.arm !== 'string' || typeof f.weapon !== 'string') bad.push(`frame ${i} arm and weapon must be strings`);
  });
  for (const e of c.events ?? []) if (typeof e.name !== 'string' || !Number.isInteger(e.frame) || e.frame < 0 || e.frame >= c.frames.length) bad.push(`event ${JSON.stringify(e)} is not on a frame`);
  return bad;
}

/** The exact artifact: a clip as JSON text, keys in a fixed order, so the same clip is the same bytes. */
export function writeClip(c) {
  const bad = checkShape(c);
  if (bad.length) throw new Error(`cannot write the clip: ${bad[0]}`);
  const frame = (f) => Object.fromEntries(CHANNELS.map((k) => [k, f[k]]));
  return JSON.stringify({ format: 'animator-clip', version: 1, id: c.id, kind: c.kind, fps: c.fps, loop: c.loop, ...(c.wrap ? { wrap: true } : {}), holds: c.holds, frames: c.frames.map(frame), events: c.events, reduced: c.reduced.map(frame), spec: c.spec ?? null }, null, 1) + '\n';
}

/** Read a clip back from `writeClip`'s text, checking it as it goes. */
export function readClip(text) {
  let j;
  try { j = JSON.parse(text); } catch { throw new Error('not JSON'); }
  if (j?.format !== 'animator-clip') throw new Error('not an animator-clip file');
  if (j.version !== 1) throw new Error(`unknown animator-clip version ${j.version}`);
  const c = clip(j.id, j.frames.map((f) => pose(f)), { fps: j.fps, loop: j.loop, wrap: j.wrap, events: j.events, holds: j.holds, kind: j.kind, reduced: (j.reduced ?? []).map((f) => pose(f)), spec: j.spec ?? undefined });
  const bad = checkShape(c);
  if (bad.length) throw new Error(`bad clip: ${bad[0]}`);
  return c;
}
