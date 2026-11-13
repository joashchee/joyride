// The recipe registry: every way the system can compose something, by id. A recipe is `(params) => Art` where Art is
// `{ id, score, loop, sections }`: the Score, whether it is meant to repeat, and named bar ranges `{ name: { from, to } }` in ticks.
// Add a recipe here with a `variants()` list and a kit subject can point at it.
import { Score } from '../score.js';
import { rng } from '../rng.js';
import { SCALE_IDS, PROGRESSION_IDS } from '../scales.js';
import { addMelody, addChords, addBass, addDrums, CONTOURS, CHORD_STYLES, BASS_PATTERNS, DRUM_PATTERN_IDS, DRUM_BEATS } from './parts.js';
import { writeSong, FORM_IDS } from './forms.js';
import { writeBlues, BLUES_SCALES, writeModulation, MODULATION_IDS, writeCounterpoint, writePhrase, CADENCE_IDS } from './theory.js';
import { writeLeitmotif, STATE_IDS } from './motif.js';
import { writeJingle, writeAmbient, JINGLES, JINGLE_IDS, MOOD_IDS } from './game.js';

const art = (id, score, { loop = false, sections } = {}) => ({ id, score, loop, sections: sections ?? { all: { from: 0, to: score.length } } });
const fresh = ({ key = 'C', scale = 'major', bpm = 110, bars = 4, beats = 4 }, title) => new Score({ key, scale, bpm, bars, beats, title });
const rngOf = (id, seed) => rng(`${id}:${seed}`);
const CONTOUR_IDS = Object.keys(CONTOURS);

export const RECIPES = {
  melody: {
    describe: 'A lead line: rhythm cells repeated A B A C, a contour, chord tones on strong beats, a tonic ending',
    variants: () => [...CONTOUR_IDS.map((contour) => ({ contour })), ...['minor', 'dorian', 'major-pentatonic', 'blues'].map((scale) => ({ scale })), { beats: 3 }, { beats: 5 }, { beats: 7 }],
    draw: ({ seed = 'melody', progression, contour, density, octave, span, ending, ...p } = {}) => {
      const s = fresh(p, 'melody');
      addMelody(s, rngOf('melody', seed), { progression: progression ?? (['minor', 'dorian', 'phrygian', 'harmonic', 'minor-pentatonic', 'blues'].includes(s.scale) ? 'minor-sad' : 'pop'), contour, density, octave, span, ending });
      return art('melody', s);
    },
  },
  chords: {
    describe: 'A chord progression, voice-led, one chord a bar, as block chords, pads, arpeggios, strums or stabs',
    variants: () => [...CHORD_STYLES.map((style) => ({ style })), ...PROGRESSION_IDS.filter((p) => p !== 'pedal').map((progression) => ({ progression, bars: 8 }))],
    draw: ({ seed = 'chords', progression = 'pop', style = 'block', size, octave, ...p } = {}) => {
      const s = fresh(p, 'chords');
      addChords(s, rngOf('chords', seed), { progression, style, size, octave });
      return art('chords', s);
    },
  },
  bass: {
    describe: 'A bassline under a progression: root, octave, root-fifth, walking or sustained',
    variants: () => BASS_PATTERNS.map((pattern) => ({ pattern })),
    draw: ({ seed = 'bass', progression = 'pop', pattern = 'root', ...p } = {}) => {
      const s = fresh(p, 'bass');
      addBass(s, rngOf('bass', seed), { progression, pattern });
      return art('bass', s);
    },
  },
  drums: {
    describe: 'A drum pattern with ghost notes, fills at phrase ends and crashes at phrase starts',
    variants: () => DRUM_PATTERN_IDS.map((pattern) => ({ pattern, beats: DRUM_BEATS[pattern] ?? 4 })),
    draw: ({ seed = 'drums', pattern = 'rock', fills, ghost, phrase, ...p } = {}) => {
      const s = fresh({ beats: DRUM_BEATS[pattern] ?? 4, ...p }, 'drums');
      addDrums(s, rngOf('drums', seed), { pattern, fills, ghost, phrase });
      return art('drums', s);
    },
  },
  song: {
    describe: 'A whole song from a form (verse-chorus, AABA, loop): intro, verse, chorus and outro roles with lead, chords, bass and drums',
    variants: () => [...FORM_IDS.map((form) => ({ form })), { form: 'verse-chorus', scale: 'minor' }, { form: 'AABA', beats: 3 }, { form: 'verse-chorus', beats: 5 }, { form: 'rondo', beats: 7, scale: 'minor' }, { form: 'loop', drums: 'breakbeat', bpm: 140 }, { form: 'verse-chorus', theme: 'hero' }, { form: 'AABA', theme: 'villain', scale: 'minor', key: 'D' }],
    draw: ({ seed = 'song', ...p } = {}) => { const { score, sections } = writeSong({ seed, ...p }); return art('song', score, { loop: p.form === 'loop', sections }); },
  },
  jingle: {
    describe: 'A game stinger: pickup, win, lose, levelup or an alert that loops',
    variants: () => JINGLE_IDS.map((kind) => ({ kind })),
    draw: ({ seed = 'jingle', ...p } = {}) => { const s = writeJingle({ seed, ...p }, rngOf('jingle', seed)); return art(`jingle.${p.kind ?? 'win'}`, s, { loop: JINGLES[p.kind ?? 'win'].loop }); },
  },
  ambient: {
    describe: 'A looping bed for a scene: pad chords, a sustained bass and a sparse lead, by mood',
    variants: () => MOOD_IDS.map((mood) => ({ mood })),
    draw: ({ seed = 'ambient', ...p } = {}) => art(`ambient.${p.mood ?? 'calm'}`, writeAmbient(p, rngOf('ambient', seed)), { loop: true }),
  },
  blues: {
    describe: 'A twelve-bar blues: three four-bar lines over I IV V, chords, an octave (boogie) bass, a lead and drums',
    variants: () => [...BLUES_SCALES.map((scale) => ({ scale })), { key: 'A', bpm: 120 }],
    draw: ({ seed = 'blues', ...p } = {}) => { const { score, sections } = writeBlues({ seed, ...p }); return art('blues', score, { sections }); },
  },
  modulation: {
    describe: 'A piece that changes key partway, through a pivot dominant: up a fifth, fourth, step or semitone, or to the relative or parallel key',
    variants: () => [...MODULATION_IDS.map((to) => ({ to })), { to: 'fifth', scale: 'minor' }, { to: 'relative', scale: 'minor' }, { to: 'fifth', back: true }],
    draw: ({ seed = 'modulation', ...p } = {}) => { const { score, sections } = writeModulation({ seed, ...p }); return art(`modulation.${p.to ?? 'fifth'}`, score, { sections }); },
  },
  counterpoint: {
    describe: 'Species counterpoint: a cantus firmus and a counter-line above or below it, first, second or third species, to the strict rules',
    variants: () => [1, 2, 3].flatMap((species) => ['above', 'below'].map((position) => ({ species, position }))).concat([{ species: 1, scale: 'minor' }, { species: 3, scale: 'minor', bars: 10 }]),
    draw: ({ seed = 'counterpoint', ...p } = {}) => { const { score, sections, lint } = writeCounterpoint({ seed, ...p }); return { ...art(`counterpoint.${p.species ?? 1}`, score, { sections }), lint }; },
  },
  phrase: {
    describe: 'An eight-bar phrase as a period (half cadence, then answer) or a sentence (idea, repeat, continuation) ending on a chosen cadence',
    variants: () => [{ type: 'period' }, { type: 'sentence' }, ...CADENCE_IDS.filter((c) => c !== 'authentic').flatMap((cadence) => [{ type: 'period', cadence }, { type: 'sentence', cadence }]), { type: 'period', scale: 'minor' }],
    draw: ({ seed = 'phrase', ...p } = {}) => { const { score, sections } = writePhrase({ seed, ...p }); return art(`phrase.${p.type ?? 'period'}`, score, { sections }); },
  },
  leitmotif: {
    describe: 'A short theme (named by `theme`) in a state of the game: calm, tense, triumph, lament or chase. The tune is the same, the palette changes',
    variants: () => [...STATE_IDS.map((state) => ({ state })), { theme: 'villain', state: 'tense', key: 'D' }, { theme: 'villain', state: 'chase', key: 'A', bars: 8 }],
    draw: ({ seed = 'leitmotif', ...p } = {}) => { const { score, sections } = writeLeitmotif({ seed, ...p }); return art(`leitmotif.${p.state ?? 'calm'}`, score, { sections }); },
  },
};
export const RECIPE_IDS = Object.keys(RECIPES);
export { SCALE_IDS };

/** Compose with a recipe by id. Throws for an unknown recipe, naming the known ones. */
export function compose(id, params = {}) {
  const r = RECIPES[id];
  if (!r) throw new Error(`unknown recipe "${id}"; known: ${RECIPE_IDS.join(', ')}`);
  return { ...r.draw(params), params: { ...params } };
}
