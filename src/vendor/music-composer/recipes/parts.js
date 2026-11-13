// The writers every recipe is built from: each adds one part (lead, chords, bass, drums) to a Score at a bar offset, so a song
// is the same parts written section by section. All randomness comes from the `r` they are given.
import { STEP, PPQ } from '../score.js';
import { degree, scaleNotes, chordOf, progression, snap, DRUM } from '../scales.js';
import { arp, voiceLead } from '../voicing.js';

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
const GAP = 6;   // ticks shaved off a note's length so repeated notes articulate

// One bar of rhythm as sixteenth-note lengths, by sixteenths per bar. A melody picks cells and repeats them (A B A C).
const CELLS = {
  16: [[4, 4, 4, 4], [6, 2, 4, 4], [4, 2, 2, 8], [8, 4, 4], [2, 2, 4, 4, 4], [4, 4, 2, 2, 4], [6, 2, 2, 2, 4], [3, 3, 2, 4, 4]],
  12: [[4, 4, 4], [6, 3, 3], [3, 3, 6], [2, 2, 4, 4], [4, 2, 2, 4]],
  // Five beats are felt 3 + 2, seven 4 + 3 (or 2 + 2 + 3): the cells put their weight there.
  20: [[4, 4, 4, 4, 4], [6, 6, 4, 4], [4, 4, 4, 2, 2, 4], [8, 4, 8], [6, 2, 4, 4, 4], [4, 2, 2, 4, 8]],
  28: [[4, 4, 4, 4, 4, 4, 4], [6, 6, 4, 4, 4, 4], [8, 4, 4, 4, 8], [4, 4, 4, 4, 6, 6], [6, 2, 4, 4, 4, 8]],
};
const ENDING = { 16: [4, 4, 8], 12: [4, 8], 20: [4, 4, 12], 28: [4, 4, 4, 16] };
export const CONTOURS = {
  arch: (t) => Math.sin(Math.PI * t),
  rise: (t) => t,
  fall: (t) => 1 - t,
  wave: (t) => (Math.sin(2 * Math.PI * t) + 1) / 2,
  flat: () => 0.5,
};
const MOVES = [-2, -1, -1, 0, 1, 1, 2];

/**
 * A melody in the score's key and scale: rhythm cells repeated A B A C, a pitch walk that follows `contour`, chord tones on the
 * strong beats, rests where `density` says, and (with `ending`) the last bar settling on the tonic.
 */
export function addMelody(score, r, { at = 0, bars = score.bars - at, progression: prog = 'pop', contour = 'arch', density = 0.7, octave = 5, span = 9, track = 'lead', program = 80, wave = 'square', vel = 96, ending = true } = {}) {
  const chords = progression(prog), spb = score.beats * 4, cells = CELLS[spb];
  if (!cells) throw new Error(`melodies need 3, 4, 5 or 7 beats a bar, not ${score.beats}`);
  if (!CONTOURS[contour]) throw new Error(`unknown contour "${contour}"; known: ${Object.keys(CONTOURS).join(', ')}`);
  score.track(track, { program, wave });
  const list = scaleNotes(score.key, score.scale, 12, 108);
  const home = list.indexOf(degree(score.key, score.scale, 0, octave)), lo = home, hi = home + span;
  const rc = r.fork('rhythm'), rp = r.fork('pitch');
  const a = rc.int(cells.length), plan = [a, rc.int(cells.length), a, rc.int(cells.length)];
  let idx = home + 2;
  for (let b = 0; b < bars; b++) {
    const final = ending && b === bars - 1, cell = final ? ENDING[spb] : cells[plan[b % 4]];
    const target = Math.round(lo + (hi - lo) * CONTOURS[contour](bars > 1 ? b / (bars - 1) : 0));
    const pcs = new Set(chordOf(score.key, score.scale, chords[b % chords.length]).map((p) => p % 12));
    let s = 0;
    for (let n = 0; n < cell.length; n++) {
      const dur = cell[n], strong = s % 4 === 0, last = final && n === cell.length - 1;
      let move = rp.pick(MOVES);
      const pull = target - idx;
      if (pull && rp.chance(Math.min(0.6, Math.abs(pull) * 0.15))) move = Math.sign(pull) * Math.max(1, Math.abs(move));
      const keep = rp.chance(density), jitter = rp.range(-4, 4);
      if (last) idx = home;
      else {
        idx = clamp(idx + move, lo - 2, hi);
        if (strong) for (const d of [0, 1, -1, 2, -2]) if (pcs.has(list[idx + d] % 12)) { idx += d; break; }
      }
      if (strong || last || keep) score.note(track, list[idx], (at + b) * score.barTicks + s * STEP, last ? dur * STEP : dur * STEP - GAP, clamp(strong ? vel + jitter : vel - 14 + jitter, 1, 127));
      s += dur;
    }
  }
  return score;
}

export const CHORD_STYLES = ['block', 'pad', 'arp', 'strum', 'stab'];
export const PAD_ENV = [0.15, 0.2, 0.8, 0.3];
/** Chords from `progression`, one a bar, voice-led so each moves as little as it can. */
export function addChords(score, r, { at = 0, bars = score.bars - at, progression: prog = 'pop', style = 'block', octave = 3, size = 3, track = 'chords', program, wave = 'triangle', vel = 70 } = {}) {
  if (!CHORD_STYLES.includes(style)) throw new Error(`unknown chord style "${style}"; known: ${CHORD_STYLES.join(', ')}`);
  const roots = progression(prog), rv = r.fork('chords');
  score.track(track, style === 'pad' ? { program: program ?? 89, wave: 'sine', env: PAD_ENV } : { program: program ?? (style === 'arp' ? 46 : 0), wave });
  let prev = null;
  for (let b = 0; b < bars; b++) {
    let ch = chordOf(score.key, score.scale, roots[b % roots.length], { size, octave });
    if (prev) ch = voiceLead(prev, ch);
    prev = ch;
    const base = (at + b) * score.barTicks, v = (x) => clamp(x + rv.range(-3, 3), 1, 127);
    if (style === 'block' || style === 'pad') for (const p of ch) score.note(track, p, base, score.barTicks - GAP, v(vel));
    else if (style === 'arp') arp([...ch, ch[0] + 12], score.beats * 2, 'updown').forEach((p, i) => score.note(track, p, base + i * STEP * 2, STEP * 2 - GAP, v(vel)));
    else if (style === 'strum') for (let k = 0; k < score.beats; k++) for (const p of ch) score.note(track, p, base + k * PPQ, PPQ - GAP - 2, v(k ? vel - 8 : vel));
    else for (let k = 0; k < score.beats; k++) for (const p of ch) score.note(track, p, base + k * PPQ + PPQ / 2, PPQ / 2 - GAP, v(vel));
  }
  return score;
}

export const BASS_PATTERNS = ['root', 'octave', 'root-fifth', 'walking', 'sustain'];
/** A bassline under the progression, in the octave below the chords. */
export function addBass(score, r, { at = 0, bars = score.bars - at, progression: prog = 'pop', pattern = 'root', octave = 2, track = 'bass', program = 33, wave = 'triangle', vel = 92 } = {}) {
  if (!BASS_PATTERNS.includes(pattern)) throw new Error(`unknown bass pattern "${pattern}"; known: ${BASS_PATTERNS.join(', ')}`);
  const roots = progression(prog), rv = r.fork('bass');
  score.track(track, { program, wave });
  for (let b = 0; b < bars; b++) {
    const ch = chordOf(score.key, score.scale, roots[b % roots.length], { octave }), nextRoot = degree(score.key, score.scale, roots[(b + 1) % roots.length], octave);
    const base = (at + b) * score.barTicks, v = (x) => clamp(x + rv.range(-3, 3), 1, 127), put = (p, k, len, x) => score.note(track, p, base + k, len, v(x));
    if (pattern === 'sustain') put(ch[0], 0, score.barTicks - GAP, vel);
    else if (pattern === 'root') for (let k = 0; k < score.beats; k++) put(ch[0], k * PPQ, PPQ - GAP, k ? vel - 8 : vel);
    else if (pattern === 'octave') for (let k = 0; k < score.beats * 2; k++) put(ch[0] + (k % 2 ? 12 : 0), k * (PPQ / 2), PPQ / 2 - GAP, k % 2 ? vel - 10 : vel);
    else if (pattern === 'root-fifth') for (let k = 0; k < score.beats; k++) put(k % 2 ? ch[2] : ch[0], k * PPQ, PPQ - GAP, k ? vel - 8 : vel);
    else {
      const approach = snap(score.key, score.scale, nextRoot - 1) === nextRoot ? nextRoot - 2 : snap(score.key, score.scale, nextRoot - 1);
      const walk = score.beats === 3 ? [ch[0], ch[1], approach] : score.beats === 4 ? [ch[0], ch[1], ch[2], approach] : [...Array.from({ length: score.beats - 1 }, (_, i) => ch[[0, 1, 2, 1][i % 4]]), approach];
      walk.forEach((p, k) => put(p, k * PPQ, PPQ - GAP, k ? vel - 8 : vel));
    }
  }
  return score;
}

const hits = (n, ...at) => Array.from({ length: n }, (_, i) => (at.includes(i) ? 'x' : '.')).join('');
// Drum grids, one character a sixteenth: x hit, X accent, o open hat, . rest. 4/4 grids are 16 steps, waltz time 12.
export const DRUM_PATTERNS = {
  rock: { kick: 'x.......x.x.....', snare: '....x.......x...', hat: 'x.x.x.x.x.x.x.x.' },
  four: { kick: 'x...x...x...x...', snare: '....x.......x...', hat: '..o...o...o...o.' },
  breakbeat: { kick: 'x.....x..xx.....', snare: '....x..x.x..x...', hat: 'x.xxx.xxx.xxx.xx' },
  half: { kick: 'x.......x.......', snare: '........x.......', hat: 'x...x...x...x...' },
  march: { kick: 'x...x...x...x...', snare: 'x.x.x.x.x.x.xxxx', hat: '................' },
  waltz: { kick: 'x...........', snare: '....x...x...', hat: 'x...x...x...' },
  // Five beats as 3 + 2 and seven as 4 + 3: the kick opens each group, the snare takes the backbeats.
  quintuple: { kick: hits(20, 0, 12), snare: hits(20, 8, 16), hat: 'x.x.x.x.x.x.x.x.x.x.' },
  septuple: { kick: hits(28, 0, 16), snare: hits(28, 8, 20), hat: 'x.x.x.x.x.x.x.x.x.x.x.x.x.x.' },
};
export const DRUM_PATTERN_IDS = Object.keys(DRUM_PATTERNS);
/** Beats a bar for the patterns that are not in four. */
export const DRUM_BEATS = { waltz: 3, quintuple: 5, septuple: 7 };
/** Drums from a pattern grid, with optional ghost snares, a fill every `phrase` bars and a crash at each phrase start. */
export function addDrums(score, r, { at = 0, bars = score.bars - at, pattern = 'rock', fills = true, phrase = 4, ghost = 0.15, crash = true, track = 'drums', vel = 100 } = {}) {
  const g = DRUM_PATTERNS[pattern];
  if (!g) throw new Error(`unknown drum pattern "${pattern}"; known: ${DRUM_PATTERN_IDS.join(', ')}`);
  const spb = g.kick.length;
  if (spb !== score.beats * 4) throw new Error(`the "${pattern}" pattern needs ${spb / 4} beats a bar, the score has ${score.beats}`);
  score.track(track, { drum: true, gain: 0.8 });
  const rv = r.fork('drums'), TOMS = [DRUM.tomHi, DRUM.tomHi, DRUM.tomMid, DRUM.tomLow];
  for (let b = 0; b < bars; b++) {
    const base = (at + b) * score.barTicks, fill = fills && (b + 1) % phrase === 0;
    for (let s = 0; s < spb; s++) {
      const t = base + s * STEP, inFill = fill && s >= spb - 4, j = rv.range(-4, 4), ghosted = rv.chance(ghost);
      if (inFill) { score.note(track, TOMS[s - (spb - 4)], t, STEP - 2, clamp(vel - 10 + s * 2 + j, 1, 127)); continue; }
      const hit = (name, ch, v) => { if (ch !== '.') score.note(track, ch === 'o' ? DRUM.openHat : DRUM[name], t, STEP - 2, clamp((ch === 'X' ? v + 12 : v) + j, 1, 127)); };
      hit('kick', g.kick[s], vel); hit('snare', g.snare[s], vel - 4); hit('hat', g.hat[s], s % 4 === 0 ? vel - 22 : vel - 36);
      if (g.snare[s] === '.' && spb === 16 && s % 2 === 1 && ghosted) score.note(track, DRUM.snare, t, STEP - 2, clamp(38 + j, 1, 127));
    }
    if (crash && b % phrase === 0) score.note(track, DRUM.crash, base, PPQ, clamp(vel + 4, 1, 127));
  }
  return score;
}
