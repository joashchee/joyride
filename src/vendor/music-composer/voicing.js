// How chords become notes (Painter's shade.js: the craft between a plan and the marks). Pure functions on pitch lists.
const avg = (a) => a.reduce((s, x) => s + x, 0) / a.length;

/** `steps` pitches from a chord: 'up', 'down', 'updown' (ping-pong) or 'alberti' (low, high, mid, high). */
export function arp(chord, steps, mode = 'updown') {
  const n = chord.length;
  const seq = mode === 'up' ? chord : mode === 'down' ? [...chord].reverse() : mode === 'alberti' ? [chord[0], chord[n - 1], chord[Math.floor(n / 2)], chord[n - 1]]
    : mode === 'updown' ? [...chord, ...chord.slice(1, -1).reverse()] : null;
  if (!seq) throw new Error(`unknown arpeggio "${mode}"; known: up, down, updown, alberti`);
  return Array.from({ length: steps }, (_, i) => seq[i % seq.length]);
}

/** The inversion and octave of `next` that moves least from `prev`, so a progression glides instead of jumping. */
export function voiceLead(prev, next) {
  const target = avg(prev);
  let best = null, bestCost = Infinity;
  for (let i = 0; i < next.length; i++) {
    const rot = [...next.slice(i), ...next.slice(0, i).map((p) => p + 12)];
    const shift = 12 * Math.round((target - avg(rot)) / 12);
    const v = rot.map((p) => p + shift).sort((a, b) => a - b);
    const cost = v.reduce((s, p, j) => s + Math.abs(p - (prev[j] ?? prev[prev.length - 1])), 0);
    if (cost < bestCost) { best = v; bestCost = cost; }
  }
  return best;
}

// Two-voice rules, shared by the counterpoint writer and by lint so what is written and what is checked cannot drift apart.
/** Intervals (semitones, mod 12) that count as consonant between two voices: unison/octave, thirds, fifth, sixths. The fourth is not. */
export const CONSONANT = new Set([0, 3, 4, 7, 8, 9]);
export const PERFECT = new Set([0, 7]);
/** True when two voices move the same way from one perfect interval to the same perfect interval (parallel fifths or octaves). */
export function parallelPerfect(a1, b1, a2, b2) {
  const i1 = Math.abs(a1 - b1) % 12, i2 = Math.abs(a2 - b2) % 12;
  return PERFECT.has(i1) && i1 === i2 && (a2 - a1) * (b2 - b1) > 0;
}
