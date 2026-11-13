// Music theory as recipes: the twelve-bar blues, modulation, species counterpoint, and the eight-bar phrase (period or sentence)
// with its cadence. Each follows the vocabulary of the textbooks (chord functions, voice-leading rules), never a particular piece.
import { Score } from '../score.js';
import { rng } from '../rng.js';
import { NOTE_NAMES, tonic, degree, scaleNotes, PARENT } from '../scales.js';
import { CONSONANT, parallelPerfect, PERFECT } from '../voicing.js';
import { addMelody, addChords, addBass, addDrums } from './parts.js';

const GAP = 6;
const rng_ = (seed, id) => rng(`${id}:${seed}`);
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
const isMinor = (scale) => ['minor', 'dorian', 'phrygian', 'harmonic', 'minor-pentatonic', 'blues'].includes(scale);
const shuffled = (r, a) => { const out = [...a]; for (let i = out.length - 1; i > 0; i--) { const j = r.int(i + 1); [out[i], out[j]] = [out[j], out[i]]; } return out; };
// Writers read `key` and `scale` off the score they are given, so a view with another key writes a section in that key.
const inKey = (score, key, scale) => Object.create(score, { key: { value: key }, scale: { value: scale } });

// ---- Twelve-bar blues ---------------------------------------------------------------------------------------------------------------

/** I I I I | IV IV I I | V IV I V, as scale degrees. */
export const BLUES_12 = [0, 0, 0, 0, 3, 3, 0, 0, 4, 3, 0, 4];
export const BLUES_SCALES = ['blues', 'minor', 'major', 'major-pentatonic', 'mixolydian'];

/** Twelve bars in three four-bar lines (statement, repeat, answer) over the blues changes: chords, an octave (boogie) bass, a lead and drums. */
export function writeBlues({ key = 'C', scale = 'blues', bpm = 96, seed = 'blues', drums = 'rock' } = {}) {
  if (!BLUES_SCALES.includes(scale)) throw new Error(`the blues is played in ${BLUES_SCALES.join(', ')}, not "${scale}"`);
  const score = new Score({ bpm, bars: 12, beats: 4, key, scale, title: `twelve-bar blues in ${key} ${scale}` });
  const r = rng_(seed, 'blues');
  addChords(score, r, { progression: BLUES_12, style: 'strum', program: 27 });
  addBass(score, r, { progression: BLUES_12, pattern: 'octave' });
  addMelody(score, r, { progression: BLUES_12, contour: 'wave', density: 0.65 });
  addDrums(score, r, { pattern: drums, phrase: 4 });
  const line = (n) => ({ from: n * 4 * score.barTicks, to: (n + 1) * 4 * score.barTicks });
  return { score, sections: { statement: line(0), repeat: line(1), answer: line(2) } };
}

// ---- Modulation ---------------------------------------------------------------------------------------------------------------------

// Where the new key is, as the semitones above the home tonic; relative and parallel also swap major and minor.
export const MODULATIONS = {
  fifth: (home) => [7, home],
  fourth: (home) => [5, home],
  'whole-step': (home) => [2, home],
  semitone: (home) => [1, home],
  relative: (home) => (isMinor(home) ? [3, 'major'] : [9, 'minor']),
  parallel: (home) => [0, isMinor(home) ? 'major' : 'minor'],
};
export const MODULATION_IDS = Object.keys(MODULATIONS);

// A leg's chords in its own key: a dominant first (the pivot that announces the key), then I IV V round to I when it is the last.
const legChords = (n, final) => Array.from({ length: n }, (_, i) => (i === 0 ? 4 : final && i === n - 1 ? 0 : [0, 3, 4][(i - 1) % 3]));

/**
 * `bars` bars at home, then the same again in the new key (and, with `back`, home again). The new key arrives on a pivot bar: its
 * dominant chord, one bar before the change, so the ear is told where it is going. `score.keyChanges` records each change.
 */
export function writeModulation({ key = 'C', scale = 'major', to = 'fifth', bars = 4, bpm = 108, beats = 4, back = false, seed = 'modulation' } = {}) {
  const m = MODULATIONS[to];
  if (!m) throw new Error(`unknown modulation "${to}"; known: ${MODULATION_IDS.join(', ')}`);
  if (bars < 3) throw new Error('each key needs at least 3 bars');
  const [semis, newScale] = m(scale), newKey = NOTE_NAMES[(tonic(key) + semis) % 12];
  const legs = [{ key, scale }, { key: newKey, scale: newScale }, ...(back ? [{ key, scale }] : [])];
  const score = new Score({ bpm, bars: bars * legs.length, beats, key, scale, title: `${key} ${scale} to ${newKey} ${newScale}` });
  const r = rng_(seed, `modulation:${to}`), sections = {};
  legs.forEach((leg, i) => {
    // The first leg stops a bar short and every later one starts a bar early, on its pivot.
    const at = i === 0 ? 0 : i * bars - 1, n = (i === legs.length - 1 ? (i + 1) * bars : (i + 1) * bars - 1) - at, last = i === legs.length - 1;
    const prog = i === 0 ? [0, 5, 3, 4].slice(0, n) : legChords(n, last), lr = r.fork(`leg${i}`), view = inKey(score, leg.key, leg.scale);
    if (i) score.modulate(at * score.barTicks, leg.key, leg.scale);
    addChords(view, lr, { at, bars: n, progression: prog, style: i % 2 ? 'strum' : 'arp' });
    addBass(view, lr, { at, bars: n, progression: prog, pattern: 'octave' });
    addMelody(view, lr, { at, bars: n, progression: prog, contour: i % 2 ? 'arch' : 'rise', density: 0.7, ending: last });
    sections[i === 0 ? 'home' : i === 2 ? 'return' : 'new-key'] = { from: at * score.barTicks, to: (at + n) * score.barTicks };
  });
  addDrums(score, r, { pattern: 'half', phrase: 4 });
  return { score, sections };
}

// ---- Species counterpoint -----------------------------------------------------------------------------------------------------------

export const SPECIES = [1, 2, 3];
const MAX_NODES = 400000;

// A cantus firmus as indexes into `list`: starts and ends on the tonic, ends by step, one climax, no repeated notes, leaps recovered.
function cantus(r, n, home) {
  const seq = [home];
  let nodes = 0;
  const go = (i) => {
    if (++nodes > MAX_NODES) throw new Error('no cantus firmus found');
    if (i === n) {
      const top = Math.max(...seq), at = seq.indexOf(top);
      return seq.lastIndexOf(top) === at && at > 0 && at < n - 1;
    }
    const prev = seq[i - 1], opts = i === n - 1 ? [home] : i === n - 2 ? [home + 1, home - 1] : Array.from({ length: 9 }, (_, k) => home - 3 + k);
    for (const c of shuffled(r, opts)) {
      const step = c - prev, back = prev - (seq[i - 2] ?? prev);
      if (!step || Math.abs(step) > 4 || (Math.abs(back) >= 3 && !(Math.sign(step) !== Math.sign(back) && Math.abs(step) === 1))) continue;
      if (seq.filter((x) => x === c).length >= 3) continue;
      seq.push(c);
      if (go(i + 1)) return true;
      seq.pop();
    }
    return false;
  };
  if (!go(1)) throw new Error('no cantus firmus found');
  return seq;
}

// The counter-line: one slot per note, searched depth first in a seeded order that prefers steps. `slots` says, per slot, which
// cantus bar it sits against and whether it falls on a strong beat.
function counterLine(r, list, cf, slots, above, home) {
  const cfp = cf.map((i) => list[i]), lo = above ? home + 3 : home - 11, hi = above ? home + 11 : home - 3;
  const out = [], last = slots.length - 1;
  let nodes = 0;
  const fits = (j, c) => {
    const s = slots[j], p = list[c], q = cfp[s.bar], gap = Math.abs(p - q);
    if (above ? p <= q : p >= q) return false;
    if (gap > 16) return false;
    const mod = gap % 12;
    if (j === 0) return PERFECT.has(mod) && gap >= 7;
    if (j === last && gap !== 12) return false;
    const prev = out[j - 1], pp = list[prev], pq = cfp[slots[j - 1].bar], step = c - prev, back = j > 1 ? prev - out[j - 2] : 0;
    if (!step || Math.abs(step) > 4 || (j === last && Math.abs(step) !== 1)) return false;   // the close is approached by step
    if (Math.abs(back) >= 3 && !(Math.sign(step) !== Math.sign(back) && Math.abs(step) <= 2)) return false;
    if (Math.abs(p - pp) === 6) return false;
    if (parallelPerfect(pp, pq, p, q)) return false;
    if (s.strong && PERFECT.has(mod) && (p - pp) * (q - pq) > 0) return false;   // no similar motion into a perfect interval
    if (!CONSONANT.has(mod)) {
      // Dissonance is only a passing note on a weak beat (the next slot checks that it is left the same way).
      if (s.strong || Math.abs(step) !== 1) return false;
    }
    if (j > 1) {
      const pgap = Math.abs(pp - pq) % 12;
      if (!CONSONANT.has(pgap) && !(Math.abs(step) === 1 && Math.sign(step) === Math.sign(back))) return false;
    }
    return true;
  };
  const go = (j) => {
    if (++nodes > MAX_NODES) throw new Error('no counterpoint found');
    if (j === slots.length) return true;
    if (j === last) { const c = above ? home + 7 : home - 7; if (!fits(j, c)) return false; out.push(c); return true; }
    const near = j ? out[j - 1] : home;
    const cands = Array.from({ length: hi - lo + 1 }, (_, k) => lo + k).map((c) => [Math.abs(c - near) + r.next() * 1.6, c]).sort((a, b) => a[0] - b[0]);
    for (const [, c] of cands) {
      if (!fits(j, c)) continue;
      out.push(c);
      if (go(j + 1)) return true;
      out.pop();
    }
    return false;
  };
  if (!go(0)) throw new Error('no counterpoint found');
  return out;
}

/**
 * A cantus firmus in whole notes with a counter-line written to the strict rules of species counterpoint: first species is note
 * against note, second two to a note, third four to a note. Perfect intervals at the ends, consonance on strong beats, dissonance only
 * as a passing note, no parallel fifths or octaves, no similar motion into a perfect interval, leaps answered by a step.
 */
export function writeCounterpoint({ key = 'C', scale = 'major', species = 1, bars = 8, position = 'above', bpm = 72, seed = 'counterpoint' } = {}) {
  if (!SPECIES.includes(species)) throw new Error(`species must be ${SPECIES.join(', ')}, not ${species}`);
  if (!['above', 'below'].includes(position)) throw new Error(`position must be above or below, not ${position}`);
  if (!Number.isInteger(bars) || bars < 4) throw new Error('counterpoint needs at least 4 bars');
  const parent = PARENT[scale] ?? scale, list = scaleNotes(key, parent, 24, 108), home = list.indexOf(degree(key, parent, 0, 4));
  const score = new Score({ bpm, bars, beats: 4, key, scale, title: `species ${species} counterpoint in ${key} ${scale}` });
  const r = rng_(seed, `counterpoint:${species}:${position}`);
  const per = species === 1 ? 1 : species === 2 ? 2 : 4, slots = [];
  for (let b = 0; b < bars; b++) {
    const n = b === bars - 1 ? 1 : per, len = score.barTicks / n;
    for (let k = 0; k < n; k++) slots.push({ bar: b, t: b * score.barTicks + k * len, d: len, strong: n === 1 || k % (per === 4 ? 2 : per) === 0 });
  }
  // Not every cantus has a counter-line against it (a tonic-low one has no room below): try the next, in a fixed order.
  let cf, line;
  for (let attempt = 0; !line; attempt++) {
    if (attempt === 12) throw new Error(`no species ${species} counterpoint found for seed "${seed}"`);
    try { cf = cantus(r.fork(`cantus${attempt}`), bars, home); line = counterLine(r.fork(`line${attempt}`), list, cf, slots, position === 'above', home); } catch { line = null; }
  }
  const rv = r.fork('velocity');
  score.track('cantus', { program: 42, wave: 'triangle' });
  score.track('counter', { program: 40, wave: 'triangle' });
  cf.forEach((c, b) => score.note('cantus', list[c], b * score.barTicks, score.barTicks - (b === bars - 1 ? 0 : GAP), clamp(80 + rv.range(-4, 4), 1, 127)));
  slots.forEach((s, j) => score.note('counter', list[line[j]], s.t, s.d - (j === slots.length - 1 ? 0 : GAP), clamp((s.strong ? 90 : 80) + rv.range(-4, 4), 1, 127)));
  return { score, sections: { all: { from: 0, to: score.length } }, lint: { voices: ['cantus', 'counter'] } };
}

// ---- Phrases and cadences -----------------------------------------------------------------------------------------------------------

/** The last two chords of a phrase, as scale degrees: V I, ii V (stops on the dominant), IV I, V vi (the expected I is withheld). */
export const CADENCES = { authentic: [4, 0], half: [1, 4], plagal: [3, 0], deceptive: [4, 5] };
export const CADENCE_IDS = Object.keys(CADENCES);
export const PHRASE_TYPES = ['period', 'sentence'];

// Copy the notes of `src` starting inside [from, to) to `dst`, `at` ticks later, moved `steps` scale steps along the scale.
function copyNotes(src, dst, track, from, to, at, steps = 0) {
  const list = scaleNotes(dst.key, dst.scale, 0, 127);
  for (const n of src.notes(track)) {
    if (n.t < from || n.t >= to) continue;
    const i = list.indexOf(n.p);
    dst.note(track, steps && i >= 0 ? list[i + steps] : n.p, n.t - from + at, n.d, n.v);
  }
}
// Move the phrase's last note to the nearest dominant or supertone, the notes a half cadence rests on.
function restOnDominant(score, track, before) {
  const last = score.tracks.get(track).notes.filter((n) => n.t < before).reduce((a, b) => (b.t > a.t ? b : a));
  const want = new Set([degree(score.key, score.scale, 4, 4) % 12, degree(score.key, score.scale, 1, 4) % 12]);
  last.p = scaleNotes(score.key, score.scale, 0, 127).filter((p) => want.has(p % 12)).reduce((a, b) => (Math.abs(b - last.p) < Math.abs(a - last.p) ? b : a));
}

/**
 * Eight bars as a period (an antecedent that asks, ending on a half cadence, and a consequent that answers with the same opening and
 * the chosen `cadence`) or a sentence (a two-bar idea, its repeat a fifth higher, then four bars of continuation that speed up).
 */
export function writePhrase({ key = 'C', scale = 'major', type = 'period', cadence = 'authentic', bpm = 100, beats = 4, seed = 'phrase' } = {}) {
  if (!PHRASE_TYPES.includes(type)) throw new Error(`unknown phrase type "${type}"; known: ${PHRASE_TYPES.join(', ')}`);
  if (!CADENCES[cadence]) throw new Error(`unknown cadence "${cadence}"; known: ${CADENCE_IDS.join(', ')}`);
  const score = new Score({ bpm, bars: 8, beats, key, scale, title: `${type} ending in a ${cadence} cadence` });
  const r = rng_(seed, `phrase:${type}`), bt = score.barTicks, end = cadence !== 'half';
  const prog = type === 'period' ? [0, 3, 1, 4, 0, 3, ...CADENCES[cadence]] : [0, 0, 4, 4, 3, 1, ...CADENCES[cadence]];
  addChords(score, r, { progression: prog, style: 'arp' });
  addBass(score, r, { progression: prog, pattern: 'octave' });
  score.track('lead', { program: 80, wave: 'square' });
  const scratch = (bars, over) => { const s = new Score({ bpm, bars, beats, key, scale }); addMelody(s, r.fork(over.name), { bars, ...over }); return s; };
  const sections = {};
  if (type === 'period') {
    const ante = scratch(4, { name: 'antecedent', progression: prog.slice(0, 4), ending: false, contour: 'arch', density: 0.8 });
    copyNotes(ante, score, 'lead', 0, 4 * bt, 0);
    restOnDominant(score, 'lead', 4 * bt);
    copyNotes(ante, score, 'lead', 0, 2 * bt, 4 * bt);
    addMelody(score, r.fork('consequent'), { at: 6, bars: 2, progression: prog.slice(6), contour: 'fall', density: 0.8, ending: end });
    sections.antecedent = { from: 0, to: 4 * bt }; sections.consequent = { from: 4 * bt, to: 8 * bt };
  } else {
    const idea = scratch(2, { name: 'idea', progression: [0, 0], ending: false, contour: 'arch', density: 0.9 });
    copyNotes(idea, score, 'lead', 0, 2 * bt, 0);
    copyNotes(idea, score, 'lead', 0, 2 * bt, 2 * bt, 4);
    addMelody(score, r.fork('continuation'), { at: 4, bars: 4, progression: prog.slice(4), contour: 'fall', density: 1, ending: end });
    sections.presentation = { from: 0, to: 4 * bt }; sections.continuation = { from: 4 * bt, to: 8 * bt };
  }
  if (!end) restOnDominant(score, 'lead', 8 * bt);
  return { score, sections };
}
