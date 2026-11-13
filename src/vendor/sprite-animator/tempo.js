// The Composer bridge: a clip's timing on a music grid. Nothing is imported from Composer (it is a separate package); the two meet
// in plain integers. `PPQ` is Composer's tick grid (96 per quarter note) and is kept in step by hand.
export const PPQ = 96;

/**
 * Fit a clip to a tempo so its footfalls land on the beat. `stepsPerBeat` steps fall on each beat (a walk at one step a beat, a run
 * at two); `beats` overrides that for a clip with no steps (a jump that should take two beats). Frame lengths are whole milliseconds that
 * add up to the cycle exactly (the spare milliseconds go to the frames with the biggest remainders), and each event gets its time in
 * ms and its tick on Composer's grid.
 */
export function tempo(c, { bpm, stepsPerBeat = 1, beats, ppq = PPQ } = {}) {
  if (!Number.isInteger(bpm) || bpm < 20 || bpm > 400) throw new Error(`bpm must be an integer from 20 to 400, not ${bpm}`);
  if (!Number.isInteger(stepsPerBeat) || stepsPerBeat < 1) throw new Error('stepsPerBeat must be a whole number');
  const steps = c.events.filter((e) => e.name === 'step').length;
  const span = beats ?? (steps ? steps / stepsPerBeat : undefined);
  if (!(span > 0)) throw new Error(`${c.id} has no steps to set to the beat; pass beats`);
  const total = Math.round((60000 * span) / bpm), weight = c.holds.reduce((a, b) => a + b, 0);
  const ms = c.holds.map((h) => Math.floor((total * h) / weight));
  const spare = total - ms.reduce((a, b) => a + b, 0);
  c.holds.map((h, i) => ({ i, r: (total * h) % weight })).sort((a, b) => b.r - a.r || a.i - b.i).slice(0, spare).forEach(({ i }) => { ms[i] += 1; });
  const starts = ms.map((_, i) => ms.slice(0, i).reduce((a, b) => a + b, 0));
  const tickOf = (t) => Math.round((t * bpm * ppq) / 60000);
  return { bpm, beats: span, totalMs: total, totalTicks: Math.round(span * ppq), ms, starts, events: c.events.map((e) => ({ ...e, ms: starts[e.frame], tick: tickOf(starts[e.frame]) })) };
}
