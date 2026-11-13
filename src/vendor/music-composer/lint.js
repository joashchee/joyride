// The craft rules as checks a composition can fail (Painter's lint.js). Not taste: each rule is one thing that makes MIDI
// ambiguous, a loop clunk, or a part sound machine-made. Findings are `{ rule, level: 'warn' | 'error', message }`.
// `voices: [a, b]` names two tracks that are meant to be counterpoint and adds `parallel` and `dissonance`; they are off otherwise,
// since chords and pads are not written to the strict two-voice rules.
import { inScale, PARENT } from './scales.js';
import { CONSONANT, parallelPerfect } from './voicing.js';

/** Findings for one Score. `loop` also checks the seam where the end meets the start. Empty means it passes. */
export function lint(score, { name = 'score', skip = [], loop = false, voices = null } = {}) {
  const out = [], add = (rule, level, message) => { if (!skip.includes(rule)) out.push({ rule, level, message: `${name}: ${message}` }); };
  if (!score.count()) { add('empty', 'error', 'has no notes'); return out; }
  if (score.end() > score.length) add('length', 'error', `notes run ${score.end() - score.length} ticks past the last bar`);
  const lastStart = Math.max(...[...score.tracks.values()].flatMap((t) => t.notes.map((n) => n.t)));
  if (score.bars > 1 && lastStart < (score.bars - 1) * score.barTicks) add('length', 'warn', 'the last bar is empty');

  for (const t of score.tracks.values()) {
    if (!t.notes.length) { add('empty', 'warn', `track "${t.name}" has no notes`); continue; }
    const ns = score.notes(t.name), label = `track "${t.name}"`;
    // The same pitch sounding twice at once: a note-off for one ends the other in MIDI.
    const lastEnd = new Map();
    let overlaps = 0;
    for (const n of ns) { if ((lastEnd.get(n.p) ?? 0) > n.t) overlaps++; lastEnd.set(n.p, Math.max(lastEnd.get(n.p) ?? 0, n.t + n.d)); }
    if (overlaps) add('overlap', 'error', `${label}: ${overlaps} note(s) start while the same pitch is still sounding`);
    if (t.drum) continue;
    const ps = ns.map((n) => n.p);
    if (Math.max(...ps) - Math.min(...ps) > 36) add('range', 'warn', `${label} spans ${Math.max(...ps) - Math.min(...ps)} semitones; more than three octaves is hard to play or sing`);
    // A piece that modulates is checked against the key in force where each note starts.
    const out_ = ns.filter((n) => { const k = score.keyAt(n.t); return !inScale(k.key, k.scale, n.p) && !inScale(k.key, PARENT[k.scale] ?? k.scale, n.p); }).length;
    if (out_) add('scale', 'warn', `${label}: ${out_} note(s) outside ${score.key} ${score.scale}${score.keyChanges.length ? ' (or the keys it modulates to)' : ''}`);
    if (ns.length >= 8 && ns.every((n) => n.v === ns[0].v)) add('dynamics', 'warn', `${label}: every velocity is ${ns[0].v}; flat dynamics sound machine-made`);
    // A monophonic line (nothing overlaps in time) is a melody: check its shape.
    const mono = ns.every((n, i) => i === 0 || n.t >= ns[i - 1].t + ns[i - 1].d);
    if (mono && ns.length > 1) {
      const leaps = ns.filter((n, i) => i && Math.abs(n.p - ns[i - 1].p) > 12).length;
      if (leaps) add('leap', 'warn', `${label}: ${leaps} leap(s) wider than an octave`);
      let run = 1, worst = 1;
      for (let i = 1; i < ns.length; i++) { run = ns[i].p === ns[i - 1].p ? run + 1 : 1; worst = Math.max(worst, run); }
      if (worst >= 8) add('repeat', 'warn', `${label}: the same pitch ${worst} times in a row`);
      if (loop && Math.abs(ns[0].p - ns[ns.length - 1].p) > 12) add('seam', 'warn', `${label}: the loop jumps ${Math.abs(ns[0].p - ns[ns.length - 1].p)} semitones from its last note to its first`);
    }
  }
  if (voices) {
    const [a, b] = voices.map((v) => score.notes(v));
    if (!a.length || !b.length) add('empty', 'error', `voices ${voices.join(' and ')}: a track has no notes`);
    else {
      const sounding = (ns, t) => ns.find((n) => n.t <= t && t < n.t + n.d);
      const ticks = [...new Set([...a, ...b].map((n) => n.t))].sort((x, y) => x - y);
      let parallels = 0, bad = 0, before = null;
      // A dissonant note is fine as a passing or neighbour note: reached by step and left by step.
      const treated = (ns, n) => {
        const i = ns.indexOf(n), p = ns[i - 1], q = ns[i + 1];
        return p && q && Math.abs(n.p - p.p) <= 3 && Math.abs(q.p - n.p) <= 3 && Math.sign(n.p - p.p) === Math.sign(q.p - n.p);
      };
      for (const t of ticks) {
        const x = sounding(a, t), y = sounding(b, t);
        if (!x || !y) { before = null; continue; }
        if (before && parallelPerfect(before[0].p, before[1].p, x.p, y.p)) parallels++;
        if (!CONSONANT.has(Math.abs(x.p - y.p) % 12)) {
          const moved = [x.t === t && !treated(a, x), y.t === t && !treated(b, y)];
          if (moved[0] || moved[1] || (x.t !== t && y.t !== t)) bad++;
        }
        before = [x, y];
      }
      if (parallels) add('parallel', 'warn', `voices ${voices.join(' and ')}: ${parallels} parallel fifth(s) or octave(s)`);
      if (bad) add('dissonance', 'warn', `voices ${voices.join(' and ')}: ${bad} dissonance(s) not approached and left by step in the same direction`);
    }
  }
  // How many pitched notes sound together at the busiest moment.
  const edges = [];
  for (const t of score.tracks.values()) if (!t.drum) for (const n of t.notes) edges.push([n.t, 1], [n.t + n.d, -1]);
  edges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let on = 0, peak = 0;
  for (const [, d] of edges) { on += d; peak = Math.max(peak, on); }
  if (peak > 12) add('density', 'warn', `up to ${peak} pitched notes sound at once; a mix this thick turns to mud`);
  return out;
}
