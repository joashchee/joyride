// A leitmotif: one short theme that returns in different states of a game. The theme is made from its name alone (`theme`), as scale
// steps and a rhythm; a state decides the palette it is played in (scale, tempo, instrument, octave, speed, accompaniment), so the
// ear hears the same tune in a new light. The steps and rhythm are the identity; everything else is the palette.
import { Score } from '../score.js';
import { rng } from '../rng.js';
import { scaleNotes, degree } from '../scales.js';
import { addMelody, addChords, addBass, addDrums } from './parts.js';

// `shift` moves the theme by whole octaves of scale steps; `speed` stretches its time (2 is twice as slow, 0.5 twice as fast).
export const STATES = {
  calm: { scale: 'major', bpm: 72, program: 73, wave: 'sine', shift: 0, speed: 1, prog: 'pop', chords: 'pad', bass: 'sustain', drums: null },
  tense: { scale: 'phrygian', bpm: 96, program: 48, wave: 'saw', shift: -7, speed: 1, prog: 'minor-sad', chords: 'stab', bass: 'root', drums: 'half' },
  triumph: { scale: 'major', bpm: 126, program: 56, wave: 'square', shift: 0, speed: 1, prog: 'rock', chords: 'strum', bass: 'octave', drums: 'four' },
  lament: { scale: 'minor', bpm: 60, program: 42, wave: 'triangle', shift: -7, speed: 2, prog: 'minor-sad', chords: 'pad', bass: 'sustain', drums: null },
  chase: { scale: 'harmonic', bpm: 150, program: 80, wave: 'square', shift: 0, speed: 0.5, prog: 'andalusian', chords: 'stab', bass: 'octave', drums: 'breakbeat' },
};
export const STATE_IDS = Object.keys(STATES);
const MOTIF_BARS = 2;

/** The theme called `theme`: its notes as `{ step, t, d, v }`, steps from the tonic of a seven-note scale, ticks in a two-bar motif. */
export function motifOf(theme) {
  const scratch = new Score({ bars: MOTIF_BARS, key: 'C', scale: 'major' });
  addMelody(scratch, rng(`motif:${theme}`), { bars: MOTIF_BARS, progression: 'pop', contour: 'arch', density: 0.85, ending: false });
  const list = scaleNotes('C', 'major', 0, 127), home = list.indexOf(degree('C', 'major', 0, 5));
  return scratch.notes('lead').map((n) => ({ step: list.indexOf(n.p) - home, t: n.t, d: n.d, v: n.v }));
}

/** The motif in a state, with that state's accompaniment under it, repeated to fill `bars`. */
export function writeLeitmotif({ theme = 'hero', state = 'calm', key = 'C', bars = 4, seed = 'leitmotif' } = {}) {
  const st = STATES[state];
  if (!st) throw new Error(`unknown state "${state}"; known: ${STATE_IDS.join(', ')}`);
  const length = MOTIF_BARS * st.speed;
  if (!Number.isInteger(bars) || bars < length || bars % length) throw new Error(`the ${state} motif is ${length} bar(s) long, so bars must be a multiple of that, not ${bars}`);
  const score = new Score({ bpm: st.bpm, bars, beats: 4, key, scale: st.scale, title: `${theme} theme, ${state}` });
  const r = rng(`leitmotif:${state}:${seed}`), list = scaleNotes(key, st.scale, 0, 127), home = list.indexOf(degree(key, st.scale, 0, 5)), bt = score.barTicks;
  score.track('lead', { program: st.program, wave: st.wave });
  for (let rep = 0; rep < bars / length; rep++) {
    for (const n of motifOf(theme)) {
      const p = list[home + n.step + st.shift];
      if (p === undefined) throw new Error(`the ${theme} theme does not fit ${key} ${st.scale} in the ${state} register`);
      score.note('lead', p, rep * length * bt + Math.round(n.t * st.speed), Math.max(1, Math.round(n.d * st.speed)), n.v);
    }
  }
  addChords(score, r, { progression: st.prog, style: st.chords });
  addBass(score, r, { progression: st.prog, pattern: st.bass });
  if (st.drums) addDrums(score, r, { pattern: st.drums });
  return { score, sections: { theme: { from: 0, to: score.length } } };
}

/** State the theme as the lead in the score's own key and scale over `bars` bars from bar `at`, repeating every two bars. Needs four beats and a seven-note scale. */
export function addTheme(score, { theme = 'hero', at = 0, bars = MOTIF_BARS } = {}) {
  if (score.beats !== 4) throw new Error('a theme is written in four beats a bar');
  if (bars % MOTIF_BARS) throw new Error(`a theme is ${MOTIF_BARS} bars long, so it fills a multiple of that, not ${bars}`);
  if (scaleNotes(score.key, score.scale, 60, 71).length !== 7) throw new Error(`the theme needs a seven-note scale, not ${score.scale}`);
  const list = scaleNotes(score.key, score.scale, 0, 127), home = list.indexOf(degree(score.key, score.scale, 0, 5));
  if (!score.tracks.has('lead')) score.track('lead');
  for (let rep = 0; rep < bars / MOTIF_BARS; rep++) {
    for (const n of motifOf(theme)) score.note('lead', list[home + n.step], (at + rep * MOTIF_BARS) * score.barTicks + n.t, n.d, n.v);
  }
}
