import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { Score, PPQ, STEP } from '../score.js';
import { rng } from '../rng.js';
import { pitch, pitchName, degree, inScale, snap, chordOf, chordName, scaleNotes, progression, tonic, SCALE_IDS } from '../scales.js';
import { arp, voiceLead } from '../voicing.js';
import { transpose, invert, retrograde, stretch, repeat, slice, concat, layer, swing, humanize } from '../transform.js';
import { lint } from '../lint.js';
import { toMidi, parseMidi, fromMidi } from '../midi.js';
import { render } from '../synth.js';
import { toWav } from '../wav.js';
import { pianoRoll } from '../text.js';
import { RECIPES, compose } from '../recipes/index.js';
import { addDrums } from '../recipes/parts.js';
import { KITS, coverage, registerKit, subjectStatus } from '../kits/index.js';
import { toModule } from '../tracker.js';
import { toNotation, parseNotation } from '../notation.js';
import { toNtt, parseNtt } from '../ntt.js';
import { composeCues, buildCue } from '../cues.js';
import { motifOf, STATES } from '../recipes/motif.js';
import { CHANNEL_MUTED, ORDER_END } from '../module.js';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
// What survives a MIDI round trip: the notes, tempo and meter. A file's key signature only says major or minor.
const sameMusic = (a, b) => a.bpm === b.bpm && a.beats === b.beats && a.tracks.size === b.tracks.size
  && [...a.tracks.keys()].every((k) => JSON.stringify(a.notes(k)) === JSON.stringify(b.notes(k)));

test('seeded randomness repeats, and forks do not disturb each other', () => {
  const a = rng('x'), b = rng('x');
  assert.deepEqual([a.next(), a.next(), a.int(10)], [b.next(), b.next(), b.int(10)]);
  assert.notEqual(rng('x').next(), rng('y').next());
  const p = rng('x'); p.next(); p.next();
  assert.equal(p.fork('f').next(), rng('x').fork('f').next(), 'a fork depends on the seed and label alone');
});

test('pitch and scale arithmetic', () => {
  assert.equal(pitch('C4'), 60); assert.equal(pitch('A4'), 69); assert.equal(pitch('Bb3'), 58);
  assert.equal(pitchName(61), 'C#4');
  assert.equal(degree('C', 'major', 0, 4), 60); assert.equal(degree('C', 'major', 7, 4), 72); assert.equal(degree('C', 'major', -1, 4), 59);
  assert.equal(degree('A', 'minor', 2, 4), 72);
  assert.ok(inScale('C', 'major', 64) && !inScale('C', 'major', 61));
  assert.equal(snap('C', 'major', 61), 60);
  assert.deepEqual(scaleNotes('C', 'major', 60, 72), [60, 62, 64, 65, 67, 69, 71, 72]);
  assert.throws(() => tonic('H'), /unknown key/); assert.throws(() => degree('C', 'nope', 0), /unknown scale/);
  assert.throws(() => progression('nope'), /known:/);
});

test('diatonic chords stack thirds in the scale', () => {
  assert.deepEqual(chordOf('C', 'major', 0), [48, 52, 55]);
  assert.deepEqual(chordOf('C', 'major', 5), [57, 60, 64]);
  assert.deepEqual(chordOf('C', 'major', 0, { size: 4 }), [48, 52, 55, 59]);
  assert.deepEqual([0, 1, 4, 5, 6].map((d) => chordName('C', 'major', d)), ['C', 'Dm', 'G', 'Am', 'Bdim']);
  assert.deepEqual(chordOf('C', 'major-pentatonic', 3), chordOf('C', 'major', 3), 'small scales borrow their parent for chords');
});

test('arpeggios and voice leading', () => {
  assert.deepEqual(arp([60, 64, 67], 6, 'updown'), [60, 64, 67, 64, 60, 64]);
  assert.deepEqual(arp([60, 64, 67], 4, 'alberti'), [60, 67, 64, 67]);
  assert.throws(() => arp([60], 2, 'nope'), /unknown arpeggio/);
  const c = chordOf('C', 'major', 0), g = chordOf('C', 'major', 4), led = voiceLead(c, g);
  const move = (a, b) => a.reduce((s, p, i) => s + Math.abs(p - b[i]), 0);
  assert.ok(move(c, led) <= move(c, g), 'voice leading moves no more than the root position');
  assert.deepEqual(led.map((p) => p % 12).sort(), g.map((p) => p % 12).sort(), 'same chord, different voicing');
});

test('Score validates what goes in and compares by notes', () => {
  const s = new Score({ bars: 1 }).note('a', 60, 0, 96);
  assert.throws(() => s.note('a', 128, 0, 1), /outside 0..127/);
  assert.throws(() => s.note('a', 60, -1, 1), /before the score/);
  assert.throws(() => s.note('a', 60, 0, 0), /at least 1 tick/);
  assert.throws(() => s.note('a', 60, 0, 1, 0), /velocity/);
  assert.throws(() => s.note('a', 60.5, 0, 1), /integer/);
  assert.ok(s.clone().equals(s));
  assert.ok(!s.clone().note('a', 62, 0, 1).equals(s));
  assert.equal(s.length, 4 * PPQ); assert.equal(s.end(), PPQ);
});

test('transforms return new scores and keep their promises', () => {
  const s = compose('melody', { bars: 4 }).score, before = s.clone();
  assert.ok(s.equals(before), 'baseline');
  const up = transpose(s, 5);
  assert.equal(up.key, 'F'); assert.ok(s.equals(before), 'the input is untouched');
  assert.deepEqual(up.notes('lead').map((n) => n.p), s.notes('lead').map((n) => n.p + 5));
  assert.ok(sameMusic(transpose(up, -5), s));
  assert.ok(sameMusic(retrograde(retrograde(s)), s));
  assert.ok(sameMusic(invert(invert(s, 72), 72), s));
  assert.equal(repeat(s, 3).bars, 12); assert.equal(repeat(s, 3).count(), 3 * s.count());
  assert.equal(concat(s, s).bars, 8);
  assert.equal(slice(repeat(s, 2), 4, 8).count(), s.count());
  assert.equal(stretch(s, 2).bars, 8); assert.equal(stretch(s, 2).end(), s.end() * 2);
  const both = layer(s, compose('bass').score);
  assert.deepEqual([...both.tracks.keys()].sort(), ['bass', 'lead']);
  const d = compose('drums').score;
  assert.ok(swing(d, 1).notes('drums').some((n, i) => n.t !== d.notes('drums')[i].t), 'swing moves off-beats');
  assert.ok(sameMusic(humanize(s, 'a'), humanize(s, 'a')), 'humanize is seeded');
  assert.ok(!sameMusic(humanize(s, 'a'), humanize(s, 'b')));
  assert.throws(() => transpose(s, 100), /out of range/);
  assert.throws(() => concat(s, new Score({ beats: 3 })), /meter/);
});

test('MIDI: varint edges, header, and a full round trip', () => {
  const s = new Score({ bpm: 97.5, bars: 2, beats: 3, key: 'Eb', scale: 'minor', title: 'T' });
  s.track('d', { drum: true });
  s.note('long', 60, 0, 20000, 100).note('d', 36, 5, 10, 90);
  const bytes = toMidi(s), m = parseMidi(bytes);
  assert.equal(String.fromCharCode(...bytes.slice(0, 4)), 'MThd');
  assert.equal(m.format, 1); assert.equal(m.ppq, PPQ); assert.equal(m.tracks.length, 3);
  const back = fromMidi(bytes);
  assert.equal(back.bpm, 97.5); assert.equal(back.beats, 3);
  assert.equal(back.notes('long')[0].d, 20000);
  assert.equal(back.key, 'D#'); assert.equal(back.scale, 'minor');
  assert.ok(back.tracks.get('d').drum, 'drums come back on channel 10');
  assert.ok(sameMusic(back, s));
  assert.throws(() => parseMidi(new Uint8Array([1, 2, 3, 4])), /not a MIDI file/);
  assert.throws(() => parseMidi(bytes.slice(0, 30)), /ends early/);
});

test('every registered variant composes, lints without errors, and survives MIDI', () => {
  let n = 0;
  for (const [id, r] of Object.entries(RECIPES)) {
    assert.ok(r.describe && r.variants().length, `${id} has a description and variants`);
    for (const v of r.variants()) {
      const a = compose(id, v), name = `${id} ${JSON.stringify(v)}`;
      assert.ok(a.score.count() > 0, name);
      assert.deepEqual(lint(a.score, { name, loop: a.loop, ...a.lint }).filter((f) => f.level === 'error'), [], name);
      assert.ok(sameMusic(fromMidi(toMidi(a.score)), a.score), `${name} round-trips`);
      for (const sec of Object.values(a.sections)) assert.ok(sec.from >= 0 && sec.to <= a.score.length && sec.from < sec.to, `${name} sections`);
      n++;
    }
  }
  assert.ok(n >= 40);
});

test('determinism: the same recipe, params and seed give the same bytes; another seed gives other notes', () => {
  for (const id of Object.keys(RECIPES)) {
    assert.deepEqual(toMidi(compose(id, { seed: 's' }).score), toMidi(compose(id, { seed: 's' }).score), id);
  }
  for (const id of ['melody', 'drums', 'song']) assert.notDeepEqual(toMidi(compose(id, { seed: 'a' }).score), toMidi(compose(id, { seed: 'b' }).score), `${id} varies with the seed`);
});

test('melodies are music: they stay in the scale, end on the tonic, and rest only off the strong beats', () => {
  for (const scale of ['major', 'minor', 'dorian', 'major-pentatonic']) {
    const s = compose('melody', { scale, bars: 8, seed: scale }).score, ns = s.notes('lead');
    assert.ok(ns.every((n) => inScale('C', scale, n.p)), scale);
    assert.equal(ns[ns.length - 1].p % 12, 0, `${scale} ends on the tonic`);
    for (let b = 0; b < 8; b++) assert.ok(ns.some((n) => n.t === b * s.barTicks), `${scale} bar ${b} starts with a note`);
  }
  const lowDensity = compose('melody', { density: 0, bars: 8 }).score.count('lead'), full = compose('melody', { density: 1, bars: 8 }).score.count('lead');
  assert.ok(full > lowDensity);
});

test('songs follow their form and put each part where its role says', () => {
  const a = compose('song', { form: 'verse-chorus', seed: 't' });
  assert.equal(a.score.bars, 20);
  assert.deepEqual(Object.keys(a.sections), ['intro', 'verse', 'chorus', 'verse-2', 'chorus-2', 'outro']);
  const inSection = (track, name) => a.score.notes(track).filter((n) => n.t >= a.sections[name].from && n.t < a.sections[name].to).length;
  assert.equal(inSection('lead', 'intro'), 0); assert.equal(inSection('drums', 'intro'), 0);
  assert.ok(inSection('lead', 'chorus') > 0 && inSection('drums', 'chorus') > inSection('drums', 'verse') * 0.5);
  assert.equal(compose('song', { form: 'loop' }).loop, true);
  assert.throws(() => compose('song', { form: 'nope' }), /unknown form/);
  assert.equal(compose('song', { form: 'AABA', beats: 3 }).score.beats, 3);
});

test('drums: patterns fit the meter, fills land at the phrase end, and a wrong meter is refused', () => {
  const s = new Score({ bars: 4 });
  addDrums(s, rng('d'), { pattern: 'rock', ghost: 0 });
  const toms = s.notes('drums').filter((n) => [50, 47, 45].includes(n.p));
  assert.ok(toms.length > 0 && toms.every((n) => n.t >= 3 * s.barTicks + 12 * STEP), 'toms only in the last four steps of bar 4');
  assert.throws(() => addDrums(new Score({ beats: 3 }), rng('d'), { pattern: 'rock' }), /needs 4 beats/);
  assert.throws(() => addDrums(new Score({ beats: 4 }), rng('d'), { pattern: 'nope' }), /unknown drum pattern/);
});

test('lint catches what it claims to', () => {
  const rules = (s, o) => lint(s, o).map((f) => `${f.level}:${f.rule}`);
  assert.deepEqual(rules(new Score()), ['error:empty']);
  assert.ok(rules(new Score({ bars: 1 }).note('a', 60, 0, 100).note('a', 60, 50, 100)).includes('error:overlap'));
  assert.ok(rules(new Score({ bars: 1 }).note('a', 60, 400, 100)).includes('error:length'));
  assert.ok(rules(new Score({ bars: 2 }).note('a', 60, 0, 10)).includes('warn:length'), 'an empty last bar');
  assert.ok(rules(new Score({ bars: 1 }).note('a', 61, 0, 10)).includes('warn:scale'));
  assert.ok(rules(new Score({ bars: 1 }).note('a', 36, 0, 10).note('a', 84, 20, 10)).includes('warn:leap'));
  const flat = new Score({ bars: 4 }); for (let i = 0; i < 8; i++) flat.note('a', 60 + (i % 2) * 2, i * 48, 40, 90);
  assert.ok(rules(flat).includes('warn:dynamics'));
  const stuck = new Score({ bars: 4 }); for (let i = 0; i < 9; i++) stuck.note('a', 60, i * 48, 40, 80 + i);
  assert.ok(rules(stuck).includes('warn:repeat'));
  assert.ok(rules(new Score({ bars: 1 }).note('a', 36, 0, 10).note('a', 84, 100, 10), { loop: true }).includes('warn:seam'));
  assert.deepEqual(rules(new Score({ bars: 1 }).note('a', 61, 0, 10), { skip: ['scale'] }), []);
});

test('the preview synth renders finite sound of the right length, and WAV bytes are well formed', () => {
  const a = compose('song', { form: 'loop' }), rate = 8000;
  const out = render(a.score, { sampleRate: rate, tail: 0 });
  assert.equal(out.length, Math.ceil(((a.score.length * 60) / (a.score.bpm * PPQ)) * rate));
  assert.ok(out.every(Number.isFinite) && out.some((x) => x !== 0) && out.every((x) => Math.abs(x) <= 0.9001));
  assert.equal(render(a.score, { sampleRate: rate, loop: true }).length, out.length, 'a loop has no tail');
  const w = toWav(out, rate), dv = new DataView(w.buffer);
  assert.equal(String.fromCharCode(...w.slice(0, 4)), 'RIFF'); assert.equal(dv.getUint32(24, true), rate); assert.equal(w.length, 44 + out.length * 2);
  assert.deepEqual(render(a.score, { sampleRate: rate }), render(a.score, { sampleRate: rate }), 'same machine, same samples');
});

test('piano roll shows notes and sustain', () => {
  const s = new Score({ bars: 1 }).note('a', 60, 0, STEP * 3).note('a', 64, STEP * 4, STEP);
  const roll = pianoRoll(s, 'a');
  assert.match(roll, /^E4\s+\|\.{4}#\.{11}\|$/m); assert.match(roll, /^C4\s+\|#==\.{13}\|$/m);
  assert.throws(() => pianoRoll(s, 'nope'), /no track/);
});

test('kits: every named recipe composes cleanly, and a kit naming a missing recipe is reported', () => {
  assert.ok(KITS.length >= 2);
  for (const k of coverage()) for (const row of k.rows) assert.deepEqual(row.problems, [], `${k.id}/${row.id}`);
  assert.equal(subjectStatus({ recipes: [{ recipe: 'nope' }] }).problems.length, 1);
  assert.equal(subjectStatus({ recipes: [{ recipe: 'melody', params: { scale: 'nope' } }] }).problems.length, 1, 'a recipe that throws is a problem, not a crash');
  assert.equal(subjectStatus({ reference: 'x' }).status, 'reference-only');
  assert.equal(subjectStatus({ label: 'x' }).status, 'gap');
  assert.throws(() => registerKit(KITS[0]), /already registered/);
});

// Rules 2 and 3: nothing outside this folder, and no Node built-ins behind the main entry.
const sources = (dir) => readdirSync(dir).flatMap((f) => { const p = join(dir, f); return statSync(p).isDirectory() ? (f === 'test' || f === 'node_modules' ? [] : sources(p)) : p.endsWith('.js') ? [p] : []; });
const imports = (file) => [...readFileSync(file, 'utf8').replace(/\/\/.*$/gm, '').matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)].map((m) => m[1]);
test('no imports from outside the package, and no Node built-ins outside cli.js', () => {
  for (const file of sources(ROOT)) {
    for (const spec of imports(file)) {
      assert.ok(spec.startsWith('./') || spec.startsWith('../') || spec.startsWith('node:'), `${file}: bare import "${spec}"`);
      if (spec.startsWith('node:')) assert.ok(file.endsWith('cli.js'), `${file} imports ${spec}; only cli.js may`);
    }
  }
  assert.ok(SCALE_IDS.length > 5);
});

test('no machine-specific paths or non-deterministic sources in the engine', () => {
  for (const file of sources(ROOT)) {
    const text = readFileSync(file, 'utf8');
    assert.ok(!/\/Users\//.test(text), `${file} names a home directory`);
    assert.ok(!/Math\.random|Date\.now|new Date\(|performance\.now/.test(text.replace(/\/\/.*$/gm, '')), `${file} uses a non-deterministic source`);
  }
});

test('twelve-bar blues: three four-bar lines over I IV V', () => {
  const s = compose('blues', { seed: 'b' }).score;
  assert.equal(s.bars, 12);
  assert.deepEqual(Object.keys(compose('blues').sections), ['statement', 'repeat', 'answer']);
  const root = (bar) => Math.min(...s.notes('bass').filter((n) => n.t >= bar * s.barTicks && n.t < (bar + 1) * s.barTicks).map((n) => n.p)) % 12;
  assert.deepEqual([0, 3, 4, 7, 8, 9, 11].map(root), [0, 0, 5, 0, 7, 5, 7], 'I, I, IV, I, V, IV, V in C');
  assert.throws(() => compose('blues', { scale: 'lydian' }), /blues is played in/);
});

test('modulation: the key changes at a pivot bar, is recorded, and lint follows it', () => {
  const a = compose('modulation', { to: 'fifth', seed: 'm' }), s = a.score;
  assert.deepEqual(s.keyChanges, [{ t: 3 * s.barTicks, key: 'G', scale: 'major' }]);
  assert.deepEqual([s.keyAt(0).key, s.keyAt(3 * s.barTicks - 1).key, s.keyAt(3 * s.barTicks).key], ['C', 'C', 'G']);
  const lead = s.notes('lead');
  assert.ok(lead.some((n) => n.p % 12 === 6), 'the new key brings F#');
  assert.ok(!lead.some((n) => n.t < 3 * s.barTicks && n.p % 12 === 6), 'and not before the change');
  assert.ok(!lint(s, { loop: false }).some((f) => f.rule === 'scale'), 'no note is outside the key in force');
  const flat = s.clone(); flat.keyChanges = [];
  assert.ok(lint(flat).some((f) => f.rule === 'scale'), 'without the key change those notes are out of key');
  assert.deepEqual(compose('modulation', { to: 'relative', scale: 'minor' }).score.keyChanges.map((c) => [c.key, c.scale]), [['D#', 'major']]);
  assert.deepEqual(compose('modulation', { to: 'parallel' }).score.keyChanges.map((c) => [c.key, c.scale]), [['C', 'minor']]);
  assert.equal(compose('modulation', { to: 'fifth', back: true }).score.keyChanges.length, 2);
  assert.throws(() => compose('modulation', { to: 'nope' }), /unknown modulation/);
  // Transforms keep the key changes honest.
  assert.equal(transpose(s, 2).keyChanges[0].key, 'A');
  assert.equal(stretch(s, 2).keyChanges[0].t, 6 * s.barTicks);
  assert.equal(slice(s, 3, 8).key, 'G'); assert.equal(slice(s, 3, 8).keyChanges.length, 0);
  assert.equal(concat(s, s).keyChanges.length, 3);
});

// An independent check of the counterpoint rules, written from the textbook and not from the writer's code.
const sounding = (ns, t) => ns.find((n) => n.t <= t && t < n.t + n.d);
test('species counterpoint obeys its rules, for every seed tried', () => {
  for (const species of [1, 2, 3]) for (const position of ['above', 'below']) for (const seed of ['a', 'b', 'c', 'd']) {
    const s = compose('counterpoint', { species, position, seed, bars: 8 }).score, cf = s.notes('cantus'), cp = s.notes('counter'), name = `species ${species} ${position} ${seed}`;
    assert.equal(cf.length, 8, name);
    assert.equal(cf[0].p % 12, 0, `${name}: cantus starts on the tonic`); assert.equal(cf[7].p % 12, 0, `${name}: and ends on it`);
    assert.equal(cp.length, species === 1 ? 8 : (species === 2 ? 2 : 4) * 7 + 1, `${name}: notes against a note`);
    const gap = (n) => Math.abs(n.p - sounding(cf, n.t).p);
    assert.ok([7, 12].includes(gap(cp[0])), `${name}: opens on a fifth or octave`);
    assert.equal(gap(cp[cp.length - 1]), 12, `${name}: closes on an octave`);
    assert.ok(Math.abs(cp[cp.length - 1].p - cp[cp.length - 2].p) <= 2, `${name}: and approaches it by step`);
    for (const n of cp) {
      const c = sounding(cf, n.t);
      assert.ok(position === 'above' ? n.p > c.p : n.p < c.p, `${name}: voices do not cross`);
      if (species === 1 || n.t % (species === 3 ? s.barTicks / 2 : s.barTicks) === 0) assert.ok([0, 3, 4, 7, 8, 9].includes(gap(n) % 12), `${name}: consonant on a strong beat at ${n.t}`);
    }
    assert.deepEqual(lint(s, { voices: ['cantus', 'counter'] }), [], name);
    for (let i = 1; i < cp.length; i++) {
      const a = cf.find((x) => x.t <= cp[i - 1].t && cp[i - 1].t < x.t + x.d), b = sounding(cf, cp[i].t), p1 = Math.abs(cp[i - 1].p - a.p) % 12, p2 = Math.abs(cp[i].p - b.p) % 12;
      assert.ok(!([0, 7].includes(p1) && p1 === p2 && (cp[i].p - cp[i - 1].p) * (b.p - a.p) > 0), `${name}: no parallel fifths or octaves at ${cp[i].t}`);
    }
  }
  assert.throws(() => compose('counterpoint', { species: 4 }), /species must be/);
  assert.throws(() => compose('counterpoint', { bars: 2 }), /at least 4 bars/);
});

test('lint voices: parallel perfect intervals and untreated dissonance are caught', () => {
  const two = (a, b) => { const s = new Score({ bars: 1 }); a.forEach((p, i) => s.note('a', p, i * 96, 90)); b.forEach((p, i) => s.note('b', p, i * 96, 90)); return lint(s, { voices: ['a', 'b'] }).map((f) => f.rule); };
  assert.deepEqual(two([67, 69, 71], [60, 62, 64]), ['parallel'], 'fifths in parallel');
  assert.deepEqual(two([72, 74, 76], [60, 62, 64]), ['parallel'], 'octaves in parallel');
  assert.deepEqual(two([67, 71, 72], [60, 62, 60]), [], 'a fifth to a sixth to an octave in contrary motion is fine');
  assert.deepEqual(two([65, 67, 72], [60, 60, 60]), ['dissonance'], 'a fourth that is neither approached nor left by step');
  assert.deepEqual(two([64, 65, 67], [60, 60, 60]), [], 'a passing fourth between a third and a fifth');
});

test('phrases: a period asks and answers, a sentence repeats its idea, and the cadence decides the last chord and note', () => {
  const p = compose('phrase', { type: 'period', seed: 'p' }), s = p.score, bt = s.barTicks;
  assert.deepEqual(Object.keys(p.sections), ['antecedent', 'consequent']);
  const lead = s.notes('lead'), ante = lead.filter((n) => n.t < 2 * bt), cons = lead.filter((n) => n.t >= 4 * bt && n.t < 6 * bt);
  assert.deepEqual(cons.map((n) => [n.p, n.t - 4 * bt, n.d]), ante.map((n) => [n.p, n.t, n.d]), 'the consequent opens as the antecedent did');
  const ends = (from, to) => lead.filter((n) => n.t >= from && n.t < to).pop().p % 12;
  assert.ok([7, 2].includes(ends(0, 4 * bt)), 'the antecedent rests on the dominant or supertone (half cadence)');
  assert.equal(ends(4 * bt, 8 * bt), 0, 'the authentic cadence lands on the tonic');
  const lastRoot = (sc) => Math.min(...sc.notes('bass').filter((n) => n.t >= 7 * bt).map((n) => n.p)) % 12;
  assert.equal(lastRoot(s), 0);
  assert.equal(lastRoot(compose('phrase', { cadence: 'plagal' }).score), 0);
  assert.equal(lastRoot(compose('phrase', { cadence: 'half' }).score), 7, 'a half cadence ends on V');
  assert.equal(lastRoot(compose('phrase', { cadence: 'deceptive' }).score), 9, 'a deceptive cadence ends on vi');
  const half = compose('phrase', { cadence: 'half', seed: 'h' }).score.notes('lead').pop();
  assert.ok([7, 2].includes(half.p % 12), 'and the melody rests on the dominant or supertone');
  const sen = compose('phrase', { type: 'sentence', seed: 'q' }).score, a = sen.notes('lead').filter((n) => n.t < 2 * bt), b = sen.notes('lead').filter((n) => n.t >= 2 * bt && n.t < 4 * bt);
  assert.deepEqual(b.map((n) => n.t - 2 * bt), a.map((n) => n.t), 'the idea repeats in the same rhythm');
  const list = scaleNotes('C', 'major', 0, 127);
  assert.deepEqual(b.map((n) => list.indexOf(n.p)), a.map((n) => list.indexOf(n.p) + 4), 'a fifth higher');
  assert.throws(() => compose('phrase', { type: 'nope' }), /unknown phrase type/);
  assert.throws(() => compose('phrase', { cadence: 'nope' }), /unknown cadence/);
});

test('odd meters: five and seven beats a bar fill every bar, in melodies, drums and songs', () => {
  for (const beats of [5, 7]) {
    const m = compose('melody', { beats, bars: 4, seed: 'o' }).score;
    assert.equal(m.barTicks, beats * PPQ);
    for (let b = 0; b < 4; b++) assert.ok(m.notes('lead').some((n) => n.t === b * m.barTicks), `${beats}/4 bar ${b} starts with a note`);
    assert.ok(m.end() <= m.length);
    const d = compose('drums', { pattern: beats === 5 ? 'quintuple' : 'septuple' }).score;
    assert.equal(d.beats, beats);
    assert.equal(fromMidi(toMidi(d)).beats, beats, 'the time signature survives MIDI');
    const song = compose('song', { form: 'verse-chorus-bridge', beats }).score;
    assert.equal(song.beats, beats); assert.ok(song.tracks.has('drums') && song.end() <= song.length);
  }
  assert.throws(() => addDrums(new Score({ beats: 5 }), rng('d'), { pattern: 'rock' }), /needs 4 beats/);
});

test('songs with a bridge and the rondo follow their forms', () => {
  const a = compose('song', { form: 'verse-chorus-bridge', seed: 'b' });
  assert.deepEqual(Object.keys(a.sections), ['intro', 'verse', 'chorus', 'verse-2', 'chorus-2', 'bridge', 'chorus-3', 'outro']);
  const inB = (track) => a.score.notes(track).filter((n) => n.t >= a.sections.bridge.from && n.t < a.sections.bridge.to).length;
  assert.equal(inB('drums'), 0, 'the bridge drops the drums'); assert.ok(inB('lead') > 0 && inB('chords') > 0);
  const r = compose('song', { form: 'rondo' });
  assert.deepEqual(Object.keys(r.sections), ['A', 'B', 'A-2', 'C', 'A-3']);
  const notes = (sec) => r.score.notes('chords').filter((n) => n.t >= r.sections[sec].from && n.t < r.sections[sec].to).map((n) => [n.p, n.t - r.sections[sec].from]).join();
  assert.equal(notes('A'), notes('A-2'), 'the refrain returns the same'); assert.notEqual(notes('A'), notes('B'));
});

test('every new recipe also converts to a tracker module, and key changes survive a clone', () => {
  for (const id of ['blues', 'modulation', 'counterpoint', 'phrase']) for (const v of RECIPES[id].variants()) {
    const a = compose(id, v);
    assert.ok(toModule(a.score, { sections: a.sections }).report, `${id} ${JSON.stringify(v)}`);
  }
  const m = compose('modulation').score;
  assert.deepEqual(m.clone().keyChanges, m.keyChanges); assert.notEqual(m.clone().keyChanges, m.keyChanges);
});

test('notation: every variant writes to text and reads back to the same notes, the same MIDI bytes and the same text', () => {
  for (const [id, r] of Object.entries(RECIPES)) for (const v of r.variants()) {
    const a = compose(id, v), text = toNotation(a.score), back = parseNotation(text), name = `${id} ${JSON.stringify(v)}`;
    assert.deepEqual(toMidi(back), toMidi(a.score), `${name}: same MIDI`);
    assert.ok(back.equals(a.score) && back.title === a.score.title, name);
    assert.deepEqual(back.keyChanges, a.score.keyChanges, name);
    assert.equal(toNotation(back), text, `${name}: writing it again gives the same text`);
  }
});

test('notation: a hand-written tune reads as written, with sticky durations and velocities, ties, chords, rests and drums', () => {
  const s = parseNotation(`# a comment
title: Hand
bpm: 100
key: A minor
track lead program=80 wave=square
| A4:q C5 E5 A5!70 | G5:h~ G5:h | r:w |
track chords gain=0.5
| [A3 C4 E4]:w | [G3 B3 D4]:h.  r:q |
track drums drum
| kick:q hat snare hat |`);
  assert.equal(s.title, 'Hand'); assert.equal(s.bpm, 100); assert.equal(s.key, 'A'); assert.equal(s.scale, 'minor'); assert.equal(s.bars, 3);
  assert.deepEqual(s.notes('lead').map((n) => [n.p, n.t, n.d, n.v]), [[69, 0, 96, 96], [72, 96, 96, 96], [76, 192, 96, 96], [81, 288, 96, 70], [79, 384, 384, 70]], 'the tie joined two halves; velocity sticks');
  assert.equal(s.tracks.get('lead').program, 80); assert.equal(s.tracks.get('lead').wave, 'square'); assert.equal(s.tracks.get('chords').gain, 0.5);
  assert.deepEqual(s.notes('chords').slice(3).map((n) => [n.p, n.t, n.d]), [[55, 384, 288], [59, 384, 288], [62, 384, 288]], 'a dotted half chord');
  assert.deepEqual(s.notes('drums').map((n) => n.p), [36, 42, 38, 42]); assert.ok(s.tracks.get('drums').drum);
  assert.equal(parseNotation('track a\n| C4:w |\ntrack b\n| n60:w |').notes('b')[0].p, 60);
  const two = parseNotation('track a\n| C4:w |\n| E4:h G4:h |');
  assert.equal(two.count('a'), 3, 'a second voice line under the same track plays with the first');
  assert.deepEqual(lint(s, { skip: ['dynamics'] }).filter((f) => f.level === 'error'), []);
});

test('notation: mistakes are named with their line, and a bar that does not add up is refused', () => {
  const err = (text) => { try { parseNotation(text); } catch (e) { return e.message; } return 'no error'; };
  assert.match(err('track a\n| C4:q |'), /line 2: bar 1 is 96 ticks long, the meter 4\/4 needs 384/);
  assert.match(err('track a\n| C4:q C4 C4 C4 C4 |'), /bar 1 is 480 ticks/);
  assert.match(err('meter: 3\ntrack a\n| C4:h. | C4:w |'), /line 3: bar 2 is 384 ticks long, the meter 3\/4 needs 288/);
  assert.match(err('track a\n| C4:h~ D4:h |'), /tie into nothing/);
  assert.match(err('track a\n| C4:w~ |'), /tied to nothing/);
  assert.match(err('track a\n| H4:w |'), /unknown note "H4"/);
  assert.match(err('track a\n| C4:z |'), /unknown duration "z"/);
  assert.match(err('track a\n| C4!200:w |'), /velocity 200/);
  assert.match(err('| C4:w |'), /need a track line/);
  assert.match(err('key: H major\ntrack a\n| C4:w |'), /unknown key|key must look like/);
  assert.match(err('track a size=3'), /unknown track option/);
  assert.match(err('hello'), /cannot read "hello"/);
  assert.match(err('bpm: fast'), /bpm must be a number/);
  assert.throws(() => toNotation(new Score({ bars: 1 }).note('two words', 60, 0, 10)), /has a space/);
  // A note across a barline is written as a tie, and a long rest as one rest a bar.
  const long = new Score({ bars: 3 }).note('a', 60, 300, 200).note('a', 62, 1000, 10);
  const text = toNotation(long);
  assert.match(text, /~/); assert.ok(parseNotation(text).equals(long));
});

test('game structure: sections become order lists, layers become muted channel groups, and both survive the .ntt', () => {
  const a = compose('song', { form: 'verse-chorus', seed: 'g' });
  const layers = { base: ['chords', 'bass'], melody: ['lead'], percussion: ['drums'] };
  const { module: m, report } = toModule(a.score, { sections: a.sections, lists: true, layers, on: ['base'], engine: 'sample' });
  assert.deepEqual(m.lists.map((l) => l.name), ['intro', 'verse', 'chorus', 'verse-2', 'chorus-2', 'outro']);
  assert.deepEqual(m.lists.flatMap((l) => l.orders), m.orders, 'the sections together are the song as played');
  assert.ok(m.lists.every((l) => l.restart === 0), 'each section loops back to its own start');
  assert.deepEqual(Object.keys(report.stems), ['chords', 'bass', 'lead', 'drums']);
  assert.deepEqual(report.layers.base, [...report.stems.chords, ...report.stems.bass]);
  const muted = m.channels.map((c, i) => (c.flags & CHANNEL_MUTED ? i : -1)).filter((i) => i >= 0);
  assert.deepEqual(muted, [...report.layers.melody, ...report.layers.percussion], 'layers not in "on" start muted');
  const back = parseNtt(toNtt(m));
  assert.deepEqual(back.lists, m.lists); assert.deepEqual(back.meta, m.meta); assert.ok(back.equals(m));
  assert.deepEqual(back.meta.layers.melody, { tracks: ['lead'], channels: report.layers.melody });
  assert.ok(Buffer.from(toNtt(m)).includes(Buffer.from('SONG')), 'the order lists are in a SONG chunk');
  assert.throws(() => toModule(a.score, { layers: { x: ['nope'] } }), /names track "nope"/);
  assert.throws(() => toModule(a.score, { layers, on: ['nope'] }), /not a layer/);
  // Without game options nothing is added to the file.
  const plain = toModule(a.score, { sections: a.sections, engine: 'sample' }).module;
  assert.deepEqual([plain.lists, plain.meta], [[], null]);
});

test('stingers: a tail of empty rows, then the song ends instead of restarting', () => {
  const j = compose('jingle', { kind: 'win' }), plain = toModule(j.score, { engine: 'sample' }), tailed = toModule(j.score, { engine: 'sample', tail: 2, end: true });
  assert.equal(tailed.module.patterns[0].rows, plain.module.patterns[0].rows + 8);
  assert.equal(tailed.report.tailRows, 8); assert.equal(tailed.module.orders.at(-1), ORDER_END);
  assert.equal(tailed.module.patterns.length, plain.module.patterns.length, 'no orphan pattern is left behind');
  const back = parseNtt(toNtt(tailed.module));
  assert.deepEqual(back.orders, tailed.module.orders); assert.equal(back.patterns[0].rows, tailed.module.patterns[0].rows);
  assert.ok(back.patterns[0].cells.slice(-8 * back.channels.length).every((c) => c === null), 'the tail is empty');
  assert.throws(() => toModule(j.score, { tail: 500 }), /outside 1..256|does not fit/);
});

test('leitmotifs: the tune stays the same across states, the palette changes', () => {
  const theme = motifOf('hero'), again = motifOf('hero'), other = motifOf('villain');
  assert.deepEqual(theme, again); assert.notDeepEqual(theme.map((n) => n.step), other.map((n) => n.step));
  const stepsOf = (state, name = 'hero') => {
    const a = compose('leitmotif', { theme: name, state }), st = STATES[state], list = scaleNotes('C', st.scale, 0, 127), home = list.indexOf(pitch('C5'));
    const len = 2 * st.speed * a.score.barTicks, first = a.score.notes('lead').filter((n) => n.t < len);
    return { a, steps: first.map((n) => list.indexOf(n.p) - home - st.shift), times: first.map((n) => n.t / st.speed) };
  };
  for (const state of Object.keys(STATES)) {
    const x = stepsOf(state);
    assert.deepEqual(x.steps, theme.map((n) => n.step), `${state}: the same steps`);
    assert.deepEqual(x.times, theme.map((n) => n.t), `${state}: the same rhythm (scaled by its speed)`);
    assert.equal(x.a.score.notes('lead').length % theme.length, 0, `${state}: the whole theme, repeated`);
  }
  const tense = compose('leitmotif', { state: 'tense' }).score, triumph = compose('leitmotif', { state: 'triumph' }).score;
  assert.notEqual(tense.scale, triumph.scale); assert.notEqual(tense.bpm, triumph.bpm); assert.notEqual(tense.tracks.get('lead').program, triumph.tracks.get('lead').program);
  assert.throws(() => compose('leitmotif', { state: 'nope' }), /unknown state/);
  assert.throws(() => compose('leitmotif', { state: 'lament', bars: 6 }), /multiple of that/);
});

test('cue sheets: cues are composed and checked, and the manifest says how to play them', () => {
  const sheet = {
    title: 'Test keep',
    cues: [
      { id: 'explore', recipe: 'song', params: { form: 'verse-chorus', seed: 'k' }, layers: { base: ['chords', 'bass'], melody: ['lead'], percussion: ['drums'] }, on: ['base'] },
      { id: 'battle', recipe: 'song', params: { form: 'loop', drums: 'breakbeat', scale: 'minor' } },
      { id: 'alarm', recipe: 'jingle', params: { kind: 'alert' } },
      { id: 'pickup', recipe: 'jingle', params: { kind: 'pickup' }, tail: 3 },
    ],
    transitions: [{ from: 'explore:verse', to: 'battle', via: 'pickup' }],
  };
  const { manifest, cues } = composeCues(sheet), by = Object.fromEntries(manifest.cues.map((c) => [c.id, c]));
  assert.deepEqual(manifest.cues.map((c) => c.kind), ['loop', 'loop', 'loop', 'stinger']);
  assert.equal(by.explore.sections.length, 6); assert.deepEqual(by.explore.start, ['base']);
  assert.deepEqual(Object.keys(by.explore.layers), ['base', 'melody', 'percussion']);
  assert.ok(by.explore.switchBars.includes(0) && by.explore.switchBars.includes(6) && by.explore.switchBars.every((b) => b < 20));
  assert.equal(by.pickup.tailMs, Math.round((3 * 60000) / by.pickup.bpm)); assert.equal(by.explore.provenance, 'Project-created');
  assert.deepEqual(manifest.transitions, [{ at: 'phrase', from: 'explore:verse', to: 'battle', via: 'pickup' }]);
  const m = parseNtt(cues[0].ntt); assert.equal(m.lists.length, 6); assert.ok(m.meta.layers.base);
  assert.equal(parseNtt(cues[3].ntt).orders.at(-1), ORDER_END, 'a stinger ends');
  assert.equal(parseNtt(cues[1].ntt).orders.at(-1) === ORDER_END, false, 'a loop does not');
  assert.deepEqual(composeCues(sheet).manifest, manifest, 'the same sheet gives the same manifest');
  const bad = (s) => { try { composeCues(s); } catch (e) { return e.message; } return 'no error'; };
  const one = (c, extra = {}) => ({ cues: [c], ...extra });
  assert.match(bad({}), /needs a "cues" list/);
  assert.match(bad(one({ id: 'a', recipe: 'nope' })), /unknown recipe "nope"/);
  assert.match(bad(one({ id: 'a b', recipe: 'melody' })), /must be letters/);
  assert.match(bad({ cues: [{ id: 'a', recipe: 'melody' }, { id: 'a', recipe: 'melody' }] }), /used twice/);
  assert.match(bad(one({ id: 'a', recipe: 'melody', layers: { x: ['lead', 'ghost'] } })), /names track "ghost"/);
  assert.match(bad(one({ id: 'a', recipe: 'song', layers: { x: ['lead'] } })), /track "chords" is in no layer/);
  assert.match(bad(one({ id: 'a', recipe: 'song', layers: { x: ['lead', 'chords', 'bass', 'drums'], y: ['lead'] } })), /is in layers "x" and "y"/);
  assert.match(bad(one({ id: 'a', recipe: 'melody', on: ['x'] })), /"on" needs layers/);
  assert.match(bad(one({ id: 'a', recipe: 'song', kind: 'stinger' })), /a stinger is a bar or four/);
  assert.match(bad(one({ id: 'a', recipe: 'melody', kind: 'nope' })), /kind must be/);
  assert.match(bad({ cues: [{ id: 'a', recipe: 'melody' }], transitions: [{ from: 'a', to: 'b' }] }), /unknown cue "b"/);
  assert.match(bad({ cues: [{ id: 'a', recipe: 'song' }], transitions: [{ from: 'a:nope', to: 'a' }] }), /no section "nope"/);
  assert.match(bad({ cues: [{ id: 'a', recipe: 'song' }], transitions: [{ from: 'a', to: 'a', at: 'whenever' }] }), /"at" must be/);
  assert.match(bad({ cues: [{ id: 'a', recipe: 'song' }], transitions: [{ from: 'a', to: 'a', via: 'a' }] }), /must be a stinger/);
  assert.deepEqual(buildCue({ id: 'solo', recipe: 'ambient' }).entry.layers, {}, 'a cue without layers has none');
});

test('a song states its theme in the chorus and nextSwitch finds legal bars', async () => {
  const { compose, composeCues, nextSwitch } = await import('../index.js');
  const plain = compose('song', { seed: 'x' }), themed = compose('song', { seed: 'x', theme: 'hero' });
  assert.notDeepEqual(plain.score.notes('lead'), themed.score.notes('lead'));
  const ch = themed.sections.chorus, inChorus = themed.score.notes('lead').filter((n) => n.t >= ch.from && n.t < ch.to);
  assert.ok(inChorus.length > 0);
  assert.throws(() => compose('song', { theme: 'hero', beats: 3 }), /four beats/);
  const e = composeCues({ cues: [{ id: 'a', recipe: 'song', params: { theme: 'hero' } }] }).manifest.cues[0];
  assert.equal(nextSwitch(e, 0.5, 'bar'), 1);
  assert.equal(nextSwitch(e, 1, 'phrase'), 2);
  assert.equal(nextSwitch(e, 3, 'section'), 6);
  assert.equal(nextSwitch(e, e.bars - 0.5, 'phrase'), e.bars);
});

test('playScript follows a cue sheet through its switching', async () => {
  const { composeCues, playScript } = await import('../index.js');
  const sheet = { cues: [
    { id: 'explore', recipe: 'song', params: { form: 'verse-chorus', seed: 'k' }, layers: { base: ['chords', 'bass'], rest: ['lead', 'drums'] }, on: ['base'] },
    { id: 'battle', recipe: 'song', params: { form: 'loop' } },
    { id: 'pickup', recipe: 'jingle', params: { kind: 'pickup' }, tail: 3 }],
  transitions: [{ from: 'explore:verse', to: 'explore:chorus' }, { from: 'explore', to: 'battle', via: 'pickup', at: 'bar' }] };
  const { manifest } = composeCues(sheet), text = playScript(manifest), lines = text.split('\n').filter((l) => l && !l.startsWith('#'));
  assert.equal(lines[0], 'play explore.ntt');
  assert.match(text, /channels \d+(,\d+)* on pattern 2 {3}# layer rest/);
  assert.match(text, /sequence explore\.ntt chorus phrase/);
  assert.match(text, /transition pickup\.ntt battle\.ntt bar/);
  assert.match(lines.at(-1), /^end \d/);
  const times = lines.filter((l) => l.startsWith('at ')).map((l) => Number(l.split(' ')[1]));
  assert.deepEqual(times, [...times].sort((a, b) => a - b), 'events are in time order');
  assert.equal(playScript(manifest), text, 'deterministic');
  assert.throws(() => playScript({ cues: [{ id: 's', kind: 'stinger', layers: {} }] }), /loop cue/);
});
