// Pitch material, as Painter's palette.js is its colours: recipes pick pitches from scales and chords built here, never a raw
// frequency. Pitches are MIDI numbers (C4 = 60); keys are names like 'C', 'F#', 'Bb'.
export const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
const NATURAL = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

/** Pitch class (0..11) of a key name such as 'C', 'F#' or 'Bb'. */
export function tonic(key) {
  const m = /^([A-G])([#b]?)$/.exec(key);
  if (!m) throw new Error(`unknown key "${key}"; use a note name such as C, F# or Bb`);
  return (NATURAL[m[1]] + (m[2] === '#' ? 1 : m[2] === 'b' ? -1 : 0) + 12) % 12;
}
/** 'C4' is 60, 'A4' is 69, 'Bb3' is 58. */
export function pitch(name) {
  const m = /^([A-G][#b]?)(-?\d)$/.exec(name);
  if (!m) throw new Error(`unknown pitch "${name}"; use a name such as C4 or F#3`);
  return 12 * (Number(m[2]) + 1) + tonic(m[1]);
}
export const pitchName = (p) => `${NOTE_NAMES[((p % 12) + 12) % 12]}${Math.floor(p / 12) - 1}`;

export const SCALES = {
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  phrygian: [0, 1, 3, 5, 7, 8, 10],
  lydian: [0, 2, 4, 6, 7, 9, 11],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
  harmonic: [0, 2, 3, 5, 7, 8, 11],
  'major-pentatonic': [0, 2, 4, 7, 9],
  'minor-pentatonic': [0, 3, 5, 7, 10],
  blues: [0, 3, 5, 6, 7, 10],
};
export const SCALE_IDS = Object.keys(SCALES);
// Chords stack thirds, which needs seven degrees: the smaller scales borrow the seven-note scale they come from.
export const PARENT = { major: 'major', minor: 'minor', dorian: 'dorian', phrygian: 'phrygian', lydian: 'lydian', mixolydian: 'mixolydian', harmonic: 'harmonic', 'major-pentatonic': 'major', 'minor-pentatonic': 'minor', blues: 'minor' };
const scaleOf = (scale) => { const s = SCALES[scale]; if (!s) throw new Error(`unknown scale "${scale}"; known: ${SCALE_IDS.join(', ')}`); return s; };

/** Pitch of scale degree `n` (0 is the tonic; negative and past-the-octave degrees wrap), with the tonic in `octave` (C4 = 60). */
export function degree(key, scale, n, octave = 4) {
  const iv = scaleOf(scale);
  return 12 * (octave + 1) + tonic(key) + iv[((n % iv.length) + iv.length) % iv.length] + 12 * Math.floor(n / iv.length);
}
export function inScale(key, scale, p) { return scaleOf(scale).includes((((p - tonic(key)) % 12) + 12) % 12); }
/** Every pitch of the scale from `lo` to `hi`, ascending. */
export function scaleNotes(key, scale, lo = 0, hi = 127) { const out = []; for (let p = lo; p <= hi; p++) if (inScale(key, scale, p)) out.push(p); return out; }
/** The nearest pitch in the scale (the lower one on a tie). */
export function snap(key, scale, p) { for (let d = 0; d < 7; d++) { if (inScale(key, scale, p - d)) return p - d; if (inScale(key, scale, p + d)) return p + d; } return p; }

/** The diatonic chord on scale degree `root`: `size` 3 is a triad, 4 a seventh chord. Lowest note in `octave`. */
export function chordOf(key, scale, root, { size = 3, octave = 3 } = {}) {
  const parent = PARENT[scale] ?? scale;
  return [0, 2, 4, 6].slice(0, size).map((i) => degree(key, parent, root + i, octave));
}
/** 'C', 'Am', 'Bdim' for the triad on degree `root`. */
export function chordName(key, scale, root) {
  const [a, b, c] = chordOf(key, scale, root);
  const q = b - a === 3 ? (c - b === 3 ? 'dim' : 'm') : c - b === 3 ? '' : 'aug';
  return NOTE_NAMES[a % 12] + q;
}

// Chord progressions as scale degrees (0 is the tonic chord). Names follow the usual Roman-numeral habits.
export const PROGRESSIONS = {
  pop: [0, 4, 5, 3],            // I V vi IV
  axis: [5, 3, 0, 4],           // vi IV I V
  doowop: [0, 5, 3, 4],         // I vi IV V
  rock: [0, 3, 4, 3],           // I IV V IV
  canon: [0, 4, 5, 2, 3, 0, 3, 4],
  'jazz-251': [1, 4, 0, 0],     // ii V I I
  'minor-sad': [0, 5, 2, 6],    // i VI III VII
  andalusian: [0, 6, 5, 4],     // i VII VI V
  'minor-rise': [0, 3, 6, 2],   // i iv VII III
  pedal: [0, 0, 0, 0],
};
export const PROGRESSION_IDS = Object.keys(PROGRESSIONS);
export function progression(spec) {
  if (Array.isArray(spec)) return spec;
  const p = PROGRESSIONS[spec];
  if (!p) throw new Error(`unknown progression "${spec}"; known: ${PROGRESSION_IDS.join(', ')}`);
  return p;
}

/** General MIDI percussion note numbers (channel 10). */
export const DRUM = { kick: 36, rim: 37, snare: 38, clap: 39, tomLow: 45, hat: 42, pedalHat: 44, openHat: 46, tomMid: 47, crash: 49, tomHi: 50, ride: 51 };
