// Whole pieces: a song form is a list of sections, each a role (intro, verse, chorus, outro) that decides which parts play.
import { Score } from '../score.js';
import { rng } from '../rng.js';
import { addTheme } from './motif.js';
import { addMelody, addChords, addBass, addDrums } from './parts.js';

// [section name, bars, role]
export const FORMS = {
  'verse-chorus': [['intro', 2, 'intro'], ['verse', 4, 'verse'], ['chorus', 4, 'chorus'], ['verse', 4, 'verse'], ['chorus', 4, 'chorus'], ['outro', 2, 'outro']],
  AABA: [['A', 4, 'verse'], ['A', 4, 'verse'], ['B', 4, 'chorus'], ['A', 4, 'verse']],
  loop: [['loop', 8, 'chorus']],
  // The bridge is the contrast: new chords, a rising line, no drums until the chorus returns.
  'verse-chorus-bridge': [['intro', 2, 'intro'], ['verse', 4, 'verse'], ['chorus', 4, 'chorus'], ['verse', 4, 'verse'], ['chorus', 4, 'chorus'], ['bridge', 4, 'bridge'], ['chorus', 4, 'chorus'], ['outro', 2, 'outro']],
  // A rondo: the refrain A comes back between two contrasting episodes.
  rondo: [['A', 4, 'verse'], ['B', 4, 'chorus'], ['A', 4, 'verse'], ['C', 4, 'bridge'], ['A', 4, 'verse']],
};
export const FORM_IDS = Object.keys(FORMS);

// What plays in each role. `lead` is melody options, `drums` is a pattern name or true for the song's main one.
const ROLES = {
  intro: { prog: 'A', chords: 'pad', lead: null, bass: null, drums: null },
  verse: { prog: 'A', chords: 'arp', lead: { density: 0.55, contour: 'wave' }, bass: 'root', drums: 'half' },
  chorus: { prog: 'B', chords: 'strum', lead: { density: 0.8, contour: 'arch' }, bass: 'octave', drums: true },
  outro: { prog: 'A', chords: 'pad', lead: { density: 0.4, contour: 'fall' }, bass: 'sustain', drums: null },
  bridge: { prog: 'C', chords: 'pad', lead: { density: 0.6, contour: 'rise' }, bass: 'sustain', drums: null },
};
const PROGS = { major: { A: 'pop', B: 'rock', C: 'axis' }, minor: { A: 'minor-sad', B: 'andalusian', C: 'minor-rise' } };
// A meter other than four has its own drum grid; the songs' roles pick patterns for four.
const METER_DRUMS = { 3: 'waltz', 5: 'quintuple', 7: 'septuple' };

const isMinor = (scale) => ['minor', 'dorian', 'phrygian', 'harmonic', 'minor-pentatonic', 'blues'].includes(scale);

/** The song recipe's body: `bars` in the params is ignored, the form decides the length. `theme` names a leitmotif the chorus states as its lead, in the song's key. Returns `{ score, sections }`. */
export function writeSong({ key = 'C', scale = 'major', bpm = 112, beats = 4, form = 'verse-chorus', drums = 'rock', seed = 'song', theme = null } = {}) {
  const plan = FORMS[form];
  if (!plan) throw new Error(`unknown form "${form}"; known: ${FORM_IDS.join(', ')}`);
  const score = new Score({ bpm, bars: plan.reduce((n, s) => n + s[1], 0), beats, key, scale, title: `${form} in ${key} ${scale}` });
  const progs = PROGS[isMinor(scale) ? 'minor' : 'major'], sections = {}, seen = {};
  const odd = METER_DRUMS[beats], main = odd ?? drums;
  let at = 0;
  plan.forEach(([name, bars, role], k) => {
    const spec = ROLES[role], prog = progs[spec.prog], r = rng(`${seed}:${form}:${k}`), last = k === plan.length - 1;
    addChords(score, r, { at, bars, progression: prog, style: spec.chords });
    if (spec.bass) addBass(score, r, { at, bars, progression: prog, pattern: spec.bass });
    if (theme && role === 'chorus') addTheme(score, { theme, at, bars });
    else if (spec.lead) addMelody(score, r, { at, bars, progression: prog, ...spec.lead, ending: last });
    if (spec.drums) addDrums(score, r, { at, bars, pattern: spec.drums === true ? main : odd ?? spec.drums, fills: bars > 1 });
    seen[name] = (seen[name] ?? 0) + 1;
    sections[seen[name] > 1 ? `${name}-${seen[name]}` : name] = { from: at * score.barTicks, to: (at + bars) * score.barTicks };
    at += bars;
  });
  return { score, sections };
}
