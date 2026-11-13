// Score to Module: the notes of a composition quantised onto a tracker's rows. Lossy by design (a row is a fixed slice of a beat), so
// the report says what moved or was left out. One instrument slot per pitched track and per drum key; these slots are what the
// bake manifest and the sample step fill in later. MIDI pitch 60 (C4) is tracker note 61 (C-5), so tracker notes run 1..120.
import { PPQ } from './score.js';
import { Module, NOTE_OFF } from './module.js';
import { oplPatch } from './opl.js';
import { cycle, drumShot, CYCLE, CYCLE_RATE, DRUM_RATE } from './samples.js';
import { DEFAULT_ENV } from './synth.js';
import { MANIFEST_ENGINES, NOTE_FLOOR, UNPITCHED, slug, seedOf } from './bake.js';

const MIDDLE_C = 61, DRUM_NOTE = MIDDLE_C;

// Speed and tempo so that a row lasts 60 / (bpm * rowsPerBeat) seconds: classic timing has a tick of 2.5 / tempo seconds and a
// row of `speed` ticks, so tempo = bpm * rowsPerBeat * speed / 24. Take the speed whose rounded tempo is closest, near 6.
function timing(bpm, rowsPerBeat) {
  let best = null;
  for (let speed = 1; speed <= 31; speed++) {
    const exact = (bpm * rowsPerBeat * speed) / 24, tempo = Math.round(exact);
    if (tempo < 32 || tempo > 255) continue;
    const err = Math.abs(tempo - exact) / exact;
    if (!best || err < best.err - 1e-12 || (Math.abs(err - best.err) <= 1e-12 && Math.abs(speed - 6) < Math.abs(best.speed - 6))) best = { speed, tempo, err };
  }
  if (!best) throw new Error(`bpm ${bpm} at ${rowsPerBeat} rows a beat has no tracker tempo between 32 and 255`);
  return best;
}

const ENGINES = ['none', 'midi', 'opl', 'sample', 'bake'];
// What plays a track: 'none' leaves its slot for a sample, 'midi' sends its notes out as MIDI (Neumetik's synthesizer plays them),
// 'opl' is a two-operator FM patch the tracker plays itself, 'sample' is a waveform from Composer's own preview synth (a looped cycle
// of the track's wave under its envelope, or a one-shot drum hit), which plays in any tracker player, 'bake' names a Neumetik engine to render it (bake.js: the manifest) and keeps the 'sample' sound until the
// render is in. `engine` is one of these, or `{ track: spec, '*': default }`, a spec being
// a name or `{ engine, patch (opl), program, bank (midi), source, recipe, note, ms, loop, seed (bake) }`.
function specFor(engine, track) {
  const raw = engine !== null && typeof engine === 'object' ? engine[track.name] ?? engine['*'] ?? 'none' : engine;
  const spec = typeof raw === 'string' ? { engine: raw } : { ...raw };
  if (!ENGINES.includes(spec.engine)) throw new Error(`unknown engine "${spec.engine}" for ${track.name}; known: ${ENGINES.join(', ')}`);
  return spec;
}
// A bake spec resolved: which Neumetik engine renders the sound, and its recipe (docs/composer-plan.md has the manifest).
function bakeSpec(spec, t, key) {
  const source = spec.source ?? (t.drum ? 'midi.drum' : 'midi');
  if (!MANIFEST_ENGINES.includes(source)) throw new Error(`unknown bake source "${source}" for ${t.name}; known: ${MANIFEST_ENGINES.join(', ')}`);
  let recipe = spec.recipe;
  if (recipe == null) {
    if (source === 'midi') recipe = spec.bank != null ? `${spec.bank}/${spec.program ?? t.program}` : `${spec.program ?? t.program}`;
    else if (source === 'midi.drum') recipe = String(key ?? 36);
    else if (source.startsWith('chip.')) recipe = t.drum ? 'noise' : /bass/i.test(t.name) ? 'triangle' : 'square';
    else throw new Error(`bake source "${source}" needs a recipe (track ${t.name})`);
  }
  return { source, recipe: String(recipe), note: spec.note ?? null, loop: spec.loop ?? (source === 'bgn' ? 'whole' : 'none'), seed: spec.seed ?? null, ms: spec.ms ?? null };
}
const patchFor = (name, k) => (/bass/i.test(name) ? 'bass' : /chord|pad|harm|string/i.test(name) ? 'pad' : /lead|melod|tune/i.test(name) ? 'lead' : ['organ', 'pluck', 'bell', 'brass'][k % 4]);

/**
 * `{ module, report }`. Options: `rowsPerBeat` (a divisor of 96, default 4), `maxChannels` (default 32), `patternRows` (rows a pattern
 * is cut at, a whole number of bars, default 64), `sections` (`{ name: { from, to } }` in ticks, as a recipe returns them: patterns
 * break there and take the name). Identical patterns of the same name are kept once and the order list repeats them.
 * `engine` (see above, default 'none') fills the instrument slots: an FM patch each ('opl'), a MIDI output each ('midi'; a drum track
 * always, as FM has no drum kit), or nothing yet. `effects` is a list of `{ kind, tracks?, mix?, params?, name? }` slots (kinds in
 * EFFECTS); the instruments of `tracks` (all, if absent) are routed through the first slot that names them.
 *
 * For a game: `lists` (with `sections`) also writes each section as its own order list, which loops back to its start, so a host can
 * switch sections at a pattern boundary; `layers` (`{ name: [track names] }`) groups the tracks' channels as stems, `on` names the layers
 * that start audible (the rest start muted, all if absent); `tail` (beats) adds empty rows after the last note so releases ring out, and
 * `end` stops the song there instead of restarting it, which is a stinger. `report.stems` says which channels each track took.
 */
export function toModule(score, { rowsPerBeat = 4, maxChannels = 32, patternRows = 64, sections = null, engine = 'none', effects = [], lists = false, layers = null, on = null, tail = 0, end = false } = {}) {
  if (!Number.isInteger(rowsPerBeat) || rowsPerBeat < 1 || PPQ % rowsPerBeat) throw new Error(`rowsPerBeat must divide ${PPQ}, got ${rowsPerBeat}`);
  const tpr = PPQ / rowsPerBeat, barRows = score.beats * rowsPerBeat, totalRows = score.bars * barRows;
  if (barRows > 256) throw new Error(`a bar of ${barRows} rows does not fit a pattern (256 at most)`);
  const { speed, tempo } = timing(score.bpm, rowsPerBeat);
  const report = { rowsPerBeat, ticksPerRow: tpr, speed, tempo, tempoError: Math.abs(tempo * 24 / (rowsPerBeat * speed) - score.bpm), notes: 0, moved: 0, maxShift: 0, dropped: [], fallbacks: [], channels: 0, patterns: 0, unique: 0 };

  // Instrument slots: a pitched track is one; a drum track is one per key it plays.
  const instruments = [], slotOf = new Map();
  let midiChannel = 0, pitched = 0;   // pitched tracks take MIDI channels in order, skipping 9, as toMidi does
  for (const t of score.tracks.values()) {
    let spec = specFor(engine, t);
    if (t.drum && spec.engine === 'opl') { report.fallbacks.push({ track: t.name, wanted: 'opl', used: 'midi', reason: 'FM has no drum kit' }); spec = { engine: 'midi' }; }
    let channel = 9;
    if (spec.engine === 'midi' && !t.drum) { if (midiChannel === 9) midiChannel++; if (midiChannel > 15) throw new Error('MIDI has 15 channels for pitched tracks; this score has more'); channel = midiChannel++; }
    const keys = t.drum ? [...new Set(t.notes.map((n) => n.p))].sort((a, b) => a - b) : [null];
    for (const key of keys) {
      const x = { name: key === null ? t.name : `${t.name} ${key}`, track: t.name, program: t.program, drum: t.drum, key };
      if (spec.engine === 'midi') Object.assign(x, { engine: 'midi', midi: { channel, program: t.drum ? null : spec.program ?? t.program, bank: spec.bank ?? null } });
      if (spec.engine === 'opl') Object.assign(x, { engine: 'opl', patch: spec.patch ?? patchFor(t.name, pitched) });
      if (spec.engine === 'sample') Object.assign(x, t.drum ? { engine: 'sample' } : { engine: 'sample', wave: t.wave, env: [...(t.env ?? DEFAULT_ENV)] });
      if (spec.engine === 'bake') {
        let id = slug(x.name), n = 1;
        while (instruments.some((o) => o.id === id)) id = `${slug(x.name)}-${++n}`;
        Object.assign(x, { engine: 'bake', id, ...bakeSpec(spec, t, key) }, t.drum ? {} : { wave: t.wave, env: [...(t.env ?? DEFAULT_ENV)] });   // wave and env: the own-synth sound it keeps until baked
        x.seed ??= seedOf(id);
      }
      instruments.push(x);
      slotOf.set(`${t.name}:${key}`, instruments.length);
    }
    if (!t.drum) pitched++;
  }

  // Quantise, then give each simultaneous note of a track its own voice (a channel plays one note at a time).
  const events = [], voices = new Map();
  [...score.tracks.values()].forEach((t, ti) => {
    const free = [];   // per voice: the row it is free from
    for (const n of score.notes(t.name)) {
      report.notes++;
      const row = Math.round(n.t / tpr);
      if (row >= totalRows) { report.dropped.push({ track: t.name, p: n.p, t: n.t, reason: 'after the end of the score' }); continue; }
      const inst = instruments[slotOf.get(`${t.name}:${t.drum ? n.p : null}`) - 1], midiDrum = t.drum && inst.engine === 'midi';   // MIDI output plays the key itself
      if (n.p + 1 > 120 && (!t.drum || midiDrum)) { report.dropped.push({ track: t.name, p: n.p, t: n.t, reason: 'above the tracker\'s top note' }); continue; }
      const end = Math.min(totalRows, Math.max(row + 1, Math.round((n.t + n.d) / tpr)));
      const shift = Math.abs(row * tpr - n.t);
      if (shift) { report.moved++; report.maxShift = Math.max(report.maxShift, shift); }
      let v = free.findIndex((f) => f <= row);
      if (v < 0) v = free.length;
      free[v] = end;
      events.push({ ti, v, row, end, drum: t.drum, note: (t.drum && !midiDrum) || UNPITCHED.includes(inst.source) ? DRUM_NOTE : n.p + 1, ins: slotOf.get(`${t.name}:${t.drum ? n.p : null}`), vol: Math.max(1, Math.round((n.v * 64) / 127)) });
    }
    voices.set(ti, free.length);
  });
  const base = [], names = [];
  let total = 0;
  [...score.tracks.values()].forEach((t, ti) => {
    base[ti] = total;
    const n = voices.get(ti) ?? 0;
    for (let v = 0; v < n; v++) names.push(n > 1 ? `${t.name} ${v + 1}` : t.name);
    total += n;
  });
  if (total > maxChannels) throw new Error(`this score needs ${total} channels; maxChannels is ${maxChannels}`);

  const m = new Module({ title: score.title, kind: 'it', speed, tempo, channels: Math.max(1, total) });
  m.channels.forEach((c, i) => { c.pan = 128; c.name = names[i] ?? ''; });
  report.stems = {};
  [...score.tracks.values()].forEach((t, ti) => { report.stems[t.name] = Array.from({ length: voices.get(ti) ?? 0 }, (_, v) => base[ti] + v); });
  m.instruments = instruments;
  if (instruments.some((x) => x.engine)) {   // slot i of the samples is instrument i's: an FM patch, or empty for MIDI to play
    // A MIDI instrument needs a sample that sounds, or the player never sends its notes: a single frame of 1 is that, and inaudible. The
    // player sends the sample's volume as the note's velocity (the volume column comes after), so it is the mean of the instrument's notes.
    const mean = (i) => { const v = events.filter((e) => e.ins === i + 1).map((e) => e.vol); return v.length ? Math.round(v.reduce((a, b) => a + b, 0) / v.length) : 64; };
    instruments.forEach((x, i) => { if (x.engine === 'opl') m.addFm(x.name, oplPatch(x.patch)); else if (x.engine === 'midi') m.addSample({ name: x.name, pcm: Int8Array.of(1), volume: mean(i) });
      else if (x.engine === 'sample' || x.engine === 'bake') m.addSample(x.drum ? { name: x.name, pcm: drumShot(x.key), rate: DRUM_RATE } : { name: x.name, pcm: cycle(x.wave), rate: CYCLE_RATE, loop: { kind: 'forward', start: 0, end: CYCLE } });
      else m.addSample({ name: x.name, pcm: new Int8Array(0) }); });
  }
  // What each baked sound should be: rendered at the middle of the notes it plays (so the tracker shifts it a little, not an octave or
  // two), as long as the longest of them (and a tail), or what its kind needs.
  const rowSeconds = 60 / (score.bpm * rowsPerBeat);
  instruments.forEach((x, i) => {
    if (x.engine !== 'bake') return;
    const mine = events.filter((e) => e.ins === i + 1), pitches = mine.map((e) => e.note - 1).sort((a, b) => a - b);
    x.note ??= UNPITCHED.includes(x.source) || !pitches.length ? 60 : Math.min(96, Math.max(NOTE_FLOOR[x.source] ?? 24, pitches[pitches.length >> 1]));
    if (x.ms !== null) return;
    const rows = Math.max(0, ...events.filter((e) => e.ins === i + 1).map((e) => e.end - e.row));
    x.ms = x.source === 'bgn' ? 8000 : UNPITCHED.includes(x.source) ? 2000 : Math.min(4000, Math.max(250, Math.round(rows * rowSeconds * 1000) + 250));
  });
  for (const { tracks = null, ...e } of effects) {
    const slot = m.addEffect(e.kind, e);
    for (const x of instruments) if (x.effect == null && (!tracks || tracks.includes(x.track))) x.effect = slot;
  }
  m.time = { rowsPerBeat, rowsPerMeasure: barRows };
  report.channels = total;

  // The whole song as a sparse grid, notes first so a note-off never takes a cell a note wants.
  const C = m.channels.length, grid = new Map();
  for (const e of events) grid.set(e.row * C + base[e.ti] + e.v, { note: e.note, ins: e.ins, vol: [1, e.vol] });
  for (const e of events) {
    const at = e.end * C + base[e.ti] + e.v;
    if (!e.drum && e.end < totalRows && !grid.has(at)) grid.set(at, { note: NOTE_OFF });
  }

  // Cut into patterns at whole bars, and at section starts; identical patterns of the same name are kept once.
  const chunk = Math.max(1, Math.floor(patternRows / barRows)), cuts = new Set([0, score.bars]);
  for (let b = chunk; b < score.bars; b += chunk) cuts.add(b);
  const named = [];
  for (const [name, s] of Object.entries(sections ?? {})) { const b = Math.round(s.from / score.barTicks); if (b >= 0 && b < score.bars) { cuts.add(b); named.push([b, name]); } }
  const bars = [...cuts].sort((a, b) => a - b), seen = new Map();
  for (let k = 0; k + 1 < bars.length; k++) {
    const r0 = bars[k] * barRows, rows = (bars[k + 1] - bars[k]) * barRows;
    const cells = Array.from({ length: rows * C }, (_, i) => grid.get(r0 * C + i) ?? null);
    const name = named.filter(([b]) => b >= bars[k] && b < bars[k + 1]).map(([, n]) => n)[0] ?? '';
    const key = `${name}:${rows}:${JSON.stringify(cells)}`;   // sections keep their own pattern, so their names survive
    let p = seen.get(key);
    if (p === undefined) {
      p = m.addPattern(rows, name);
      m.patterns[p].cells = cells;
      seen.set(key, p);
    }
    m.order(p);
    report.patterns++;
  }

  // Sections as order lists: the patterns that start inside each section, in order.
  const ranges = Object.entries(sections ?? {}).map(([name, x]) => ({ name, from: Math.round(x.from / score.barTicks), to: Math.round(x.to / score.barTicks) }));
  const patternAt = m.orders.slice();
  if (lists && ranges.length > 1) for (const r of ranges) m.addList(r.name, patternAt.filter((_, k) => bars[k] >= r.from && bars[k] < r.to));
  // Layers: stems of channels, some muted until a host brings them in.
  if (layers) {
    const groups = {};
    for (const [name, tracks] of Object.entries(layers)) {
      groups[name] = tracks.flatMap((t) => { if (!report.stems[t]) throw new Error(`layer "${name}" names track "${t}", which the score does not have`); return report.stems[t]; });
    }
    const audible = on ?? Object.keys(groups);
    for (const name of audible) if (!groups[name]) throw new Error(`"${name}" is not a layer; known: ${Object.keys(groups).join(', ')}`);
    for (const [name, channels] of Object.entries(groups)) if (!audible.includes(name)) m.mute(channels);
    report.layers = groups;
    m.meta = { ...m.meta, layers: Object.fromEntries(Object.entries(groups).map(([name, channels]) => [name, { tracks: layers[name], channels }])), on: audible };
  }
  if (lists && ranges.length > 1) m.meta = { ...m.meta, sections: ranges.map((r, i) => ({ name: r.name, fromBar: r.from, toBar: r.to, list: i + 1 })) };
  // A tail: the last pattern gets empty rows after it (as a copy if another order or list shares it).
  if (tail > 0) {
    const extra = Math.round(tail * rowsPerBeat), at = m.orders.length - 1, last = m.orders[at], src = m.patterns[last];
    const shared = m.orders.some((o, i) => i !== at && o === last) || m.lists.some((l) => l.orders.includes(last));
    const p = shared ? m.addPattern(src.rows + extra, src.name) : last;
    if (!shared) { if (src.rows + extra > 256) throw new Error(`a tail of ${extra} rows does not fit a pattern (256 rows at most)`); src.rows += extra; }
    m.patterns[p].cells = [...src.cells.slice(0, (src.rows - (shared ? 0 : extra)) * C), ...new Array(extra * C).fill(null)];
    m.orders[at] = p;
    report.tailRows = extra;
  }
  if (end) m.end();
  report.unique = m.patterns.length;
  return { module: m, report };
}
