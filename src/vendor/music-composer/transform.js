// Operations on a finished Score (Painter's anim.js): each returns a new Score and leaves its input alone.
import { Score, PPQ } from './score.js';
import { NOTE_NAMES, tonic } from './scales.js';
import { rng } from './rng.js';

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
// Rebuilds a score, passing every note through fn(note, track); a null result drops the note.
function remake(score, fn, over = {}) {
  const out = new Score({ ...score, ...over });
  for (const [name, t] of score.tracks) {
    const nt = out.track(name, { ...t, env: t.env && [...t.env], notes: undefined });
    for (const n of [...t.notes].sort((a, b) => a.t - b.t || a.p - b.p)) { const m = fn(n, t); if (m) nt.notes.push(m); }
  }
  return out;
}
const pitched = (tracks, t) => !t.drum && (!tracks || tracks.includes(t.name));

/** Shift pitched tracks by `semis` semitones; the key moves with them. */
export function transpose(score, semis, { tracks } = {}) {
  const key = NOTE_NAMES[(((tonic(score.key) + semis) % 12) + 12) % 12];
  const shift = (k) => NOTE_NAMES[(((tonic(k) + semis) % 12) + 12) % 12];
  return remake(score, (n, t) => {
    if (!pitched(tracks, t)) return { ...n };
    const p = n.p + semis;
    if (p < 0 || p > 127) throw new Error(`transposing by ${semis} takes pitch ${n.p} out of range`);
    return { ...n, p };
  }, { key, keyChanges: score.keyChanges.map((c) => ({ ...c, key: shift(c.key) })) });
}
/** Mirror pitched tracks around the pitch `axis` (a melody turned upside down). */
export function invert(score, axis, { tracks } = {}) {
  return remake(score, (n, t) => {
    if (!pitched(tracks, t)) return { ...n };
    const p = 2 * axis - n.p;
    if (p < 0 || p > 127) throw new Error(`inverting around ${axis} takes pitch ${n.p} out of range`);
    return { ...n, p };
  });
}
/** Play the score backwards. */
export function retrograde(score) { return remake(score, (n) => ({ ...n, t: score.length - (n.t + n.d) }), { keyChanges: [] }); }
/** Scale all times by `num / den` (2 is half speed in the same bpm, so the piece gets twice the bars). */
export function stretch(score, num, den = 1) {
  return remake(score, (n) => ({ ...n, t: Math.round((n.t * num) / den), d: Math.max(1, Math.round((n.d * num) / den)) }), { bars: Math.ceil((score.bars * num) / den), keyChanges: score.keyChanges.map((c) => ({ ...c, t: Math.round((c.t * num) / den) })) });
}
/** The score `n` times in a row. */
export function repeat(score, n) {
  let out = score.clone();
  for (let i = 1; i < n; i++) out = concat(out, score);
  return out;
}
/** Bars `from` up to (not including) `to`, notes that start inside it only, clipped to the end. */
export function slice(score, from, to) {
  const a = from * score.barTicks, b = to * score.barTicks;
  const at = score.keyAt(a), inside = score.keyChanges.filter((c) => c.t > a && c.t < b).map((c) => ({ ...c, t: c.t - a }));
  return remake(score, (n) => (n.t >= a && n.t < b ? { ...n, t: n.t - a, d: Math.min(n.d, b - n.t) } : null), { bars: to - from, key: at.key, scale: at.scale, keyChanges: inside });
}
/** `b` after `a`. Both must share a meter; tempo, key and scale come from `a`. */
export function concat(a, b) {
  if (a.beats !== b.beats) throw new Error('cannot join scores with different meters');
  const out = remake(a, (n) => ({ ...n }), { bars: a.bars + b.bars });
  const bk = b.keyAt(0), seam = [{ t: a.length, key: bk.key, scale: bk.scale }, ...b.keyChanges.map((c) => ({ ...c, t: c.t + a.length }))];
  if (b.key !== a.keyAt(a.length - 1).key || b.scale !== a.keyAt(a.length - 1).scale || b.keyChanges.length) out.keyChanges.push(...seam);
  for (const [name, t] of b.tracks) { const nt = out.track(name, { ...t, env: t.env && [...t.env], notes: undefined }); for (const n of t.notes) nt.notes.push({ ...n, t: n.t + a.length }); }
  return out;
}
/** `b`'s tracks played together with `a`'s (same-named tracks share notes). */
export function layer(a, b) {
  if (a.beats !== b.beats) throw new Error('cannot layer scores with different meters');
  const out = remake(a, (n) => ({ ...n }), { bars: Math.max(a.bars, b.bars) });
  for (const [name, t] of b.tracks) { const nt = out.track(name, { ...t, env: t.env && [...t.env], notes: undefined }); for (const n of t.notes) nt.notes.push({ ...n }); }
  return out;
}
/** Delay the off-beat eighths: `amount` 0 is straight, 1 is a full triplet feel. */
export function swing(score, amount = 0.5) {
  const delay = Math.round(amount * (PPQ / 6));
  return remake(score, (n) => ({ ...n, t: n.t % PPQ === PPQ / 2 ? n.t + delay : n.t }));
}
/** Loosen the grid: start times move by up to `time` ticks and velocities by up to `vel`, from the seed. */
export function humanize(score, seed, { time = 3, vel = 6 } = {}) {
  const r = rng(`humanize:${seed}`);
  return remake(score, (n) => ({ ...n, t: Math.max(0, n.t + r.range(-time, time)), v: clamp(n.v + r.range(-vel, vel), 1, 127) }));
}
