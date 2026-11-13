// A score as text, for a terminal: the piano roll a producer would look at. One column per sixteenth note, a bar line before each bar.
import { STEP } from './score.js';
import { pitchName, DRUM } from './scales.js';

const DRUM_NAMES = Object.fromEntries(Object.entries(DRUM).map(([k, v]) => [v, k]));

/** `#` where a note starts and `=` while it sounds, highest pitch first. Long scores are cut at `maxBars`. */
export function pianoRoll(score, track, { maxBars = 8 } = {}) {
  const t = score.tracks.get(track);
  if (!t) throw new Error(`no track "${track}"; the score has: ${[...score.tracks.keys()].join(', ')}`);
  const perBar = score.barTicks / STEP, bars = Math.min(score.bars, maxBars), cols = perBar * bars;
  const grid = new Map();
  for (const n of t.notes) {
    const c = Math.floor(n.t / STEP);
    if (c >= cols) continue;
    const row = grid.get(n.p) ?? Array(cols).fill('.');
    row[c] = '#';
    for (let k = c + 1; k < Math.min(cols, Math.ceil((n.t + n.d) / STEP)); k++) if (row[k] === '.') row[k] = '=';
    grid.set(n.p, row);
  }
  const label = (p) => (t.drum ? DRUM_NAMES[p] ?? String(p) : pitchName(p)).padEnd(8);
  const lines = [...grid.keys()].sort((a, b) => b - a).map((p) => {
    const row = grid.get(p), bar = [];
    for (let b = 0; b < bars; b++) bar.push(row.slice(b * perBar, (b + 1) * perBar).join(''));
    return `${label(p)}|${bar.join('|')}|`;
  });
  return lines.join('\n') + (score.bars > bars ? `\n(first ${bars} of ${score.bars} bars)` : '');
}
