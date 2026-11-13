// Game audio: short stingers (jingles) and looping ambient beds. Jingles are written by hand from scale degrees, with the seed
// choosing among a few variants; ambient beds reuse the parts.
import { Score, STEP } from '../score.js';
import { degree } from '../scales.js';
import { addMelody, addChords, addBass } from './parts.js';

export const JINGLES = {
  pickup:  { bars: 1, bpm: 168, scale: 'major-pentatonic', loop: false, describe: 'two quick rising notes' },
  win:     { bars: 2, bpm: 132, scale: 'major', loop: false, describe: 'a rising run and a held tonic chord' },
  lose:    { bars: 2, bpm: 84, scale: 'minor', loop: false, describe: 'a falling line and a low tonic' },
  levelup: { bars: 2, bpm: 150, scale: 'major', loop: false, describe: 'a full scale run into a chord' },
  alert:   { bars: 2, bpm: 150, scale: 'phrygian', loop: true, describe: 'a tritone alternation, a looping warning' },
};
export const JINGLE_IDS = Object.keys(JINGLES);

/** A stinger of the given kind. Returns the Score (key and bpm may be overridden; the scale is the kind's unless given). */
export function writeJingle({ kind = 'win', key = 'C', bpm, scale, seed = 'jingle' } = {}, r) {
  const spec = JINGLES[kind];
  if (!spec) throw new Error(`unknown jingle "${kind}"; known: ${JINGLE_IDS.join(', ')}`);
  const score = new Score({ bpm: bpm ?? spec.bpm, bars: spec.bars, key, scale: scale ?? spec.scale, title: `${kind} jingle` });
  const lead = score.track('lead', { program: 80, wave: 'square' }), deg = (n, o = 5) => degree(key, score.scale, n, o);
  const put = (track, n, step, len, v, o) => score.note(track, deg(n, o), step * STEP, len * STEP - 6, v);
  const chord = (step, len, v, ns = [0, 2, 4, 7]) => ns.forEach((n) => put('lead', n, step, len, v));
  if (kind === 'pickup') {
    put('lead', 0, 0, 2, 104); put('lead', r.pick([2, 3, 4]), 2, 6, 100);
  } else if (kind === 'win') {
    r.pick([[0, 2, 4, 7], [0, 4, 2, 7], [2, 4, 7, 9]]).forEach((n, i) => put('lead', n, i * 2, 2, 100 + i * 3));
    chord(8, 24, 108);
    put('bass', 0, 8, 24, 90, 3);
  } else if (kind === 'lose') {
    [4, 2, 1, 0].forEach((n, i) => put('lead', n, i * 4, 4, 98 - i * 8));
    put('bass', -7, 16, 16, 90, 4);
  } else if (kind === 'levelup') {
    for (let i = 0; i < 8; i++) put('lead', i, i * 2, 2, 90 + i * 3);
    chord(16, 16, 110);
    put('bass', 0, 16, 16, 92, 3);
  } else {
    const tri = degree(key, score.scale, 0, 5) + 6;
    for (let i = 0; i < 8; i++) score.note('lead', i % 2 ? tri : degree(key, score.scale, 0, 5), i * 4 * STEP, 4 * STEP - 8, i % 4 < 2 ? 108 : 92);
  }
  if (score.tracks.has('bass')) Object.assign(score.tracks.get('bass'), { program: 33, wave: 'triangle' });
  return score;
}

export const MOODS = {
  calm:   { scale: 'major-pentatonic', bpm: 72, progression: 'pop', density: 0.3, contour: 'wave' },
  sad:    { scale: 'minor', bpm: 60, progression: 'minor-sad', density: 0.3, contour: 'fall' },
  tense:  { scale: 'phrygian', bpm: 84, progression: 'andalusian', density: 0.35, contour: 'flat' },
  bright: { scale: 'lydian', bpm: 96, progression: 'axis', density: 0.45, contour: 'arch' },
};
export const MOOD_IDS = Object.keys(MOODS);

/** A looping bed: pad chords, a sustained bass and a sparse sine lead. */
export function writeAmbient({ mood = 'calm', key = 'C', bpm, bars = 8, beats = 4, scale } = {}, r) {
  const m = MOODS[mood];
  if (!m) throw new Error(`unknown mood "${mood}"; known: ${MOOD_IDS.join(', ')}`);
  const score = new Score({ bpm: bpm ?? m.bpm, bars, beats, key, scale: scale ?? m.scale, title: `${mood} ambient` });
  addChords(score, r, { progression: m.progression, style: 'pad', vel: 62 });
  addBass(score, r, { progression: m.progression, pattern: 'sustain', vel: 70 });
  addMelody(score, r, { progression: m.progression, density: m.density, contour: m.contour, octave: 5, wave: 'sine', program: 88, vel: 80, ending: false });
  return score;
}
