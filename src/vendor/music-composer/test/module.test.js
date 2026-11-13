import test from 'node:test';
import assert from 'node:assert/strict';
import { sha256, hex, ByteWriter, ByteReader, nlzStore, nlzUnstore, Module, NOTE_OFF } from '../index.js';

const text = (s) => new TextEncoder().encode(s);

test('sha256 matches the FIPS 180-4 vectors, across block boundaries', () => {
  assert.equal(hex(sha256(new Uint8Array(0))), 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
  assert.equal(hex(sha256(text('abc'))), 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  assert.equal(hex(sha256(text('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq'))), '248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1');
  assert.equal(hex(sha256(text('a'.repeat(1000000)))), 'cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0');
  for (const n of [55, 56, 63, 64, 65]) assert.equal(sha256(new Uint8Array(n)).length, 32);
});

test('byte writer and reader round trip, little-endian, and refuse a short file', () => {
  const b = new ByteWriter().u8(200).i8(-5).u16(51234).u32(4000000000).u64(2 ** 40 + 7).f32(0.5).str('Tëst ♪').done();
  assert.deepEqual([...b.subarray(1, 2)], [251], 'i8 is two\'s complement');
  assert.deepEqual([...new ByteWriter().u16(0x0102).done()], [2, 1], 'little-endian');
  const r = new ByteReader(b);
  assert.deepEqual([r.u8(), r.i8(), r.u16(), r.u32(), r.u64(), r.f32(), r.str()], [200, -5, 51234, 4000000000, 2 ** 40 + 7, 0.5, 'Tëst ♪']);
  assert.equal(r.left, 0);
  assert.throws(() => r.u8(), /ends early/);
  assert.throws(() => new ByteReader(new Uint8Array([5, 0, 65])).str(), /ends early/);
  assert.throws(() => new ByteWriter().u8(256), /outside/); assert.throws(() => new ByteWriter().u16(1.5), /outside/);
});

test('chunks nest, and a long string is cut on a character boundary', () => {
  const b = new ByteWriter().chunk('INFO', (w) => w.str('hi')).chunk('END ', () => {}).done();
  const r = new ByteReader(b), a = r.chunk(), e = r.chunk();
  assert.equal(a.id, 'INFO'); assert.equal(a.body.str(), 'hi'); assert.equal(e.id, 'END '); assert.equal(e.body.left, 0); assert.equal(r.chunk(), null);
  const long = new ByteReader(new ByteWriter().str('é'.repeat(40000)).done()).str();
  assert.ok(long.length > 0 && long.length <= 32767 && /^é+$/.test(long));
});

test('stored NLZ1 streams round trip; coded and damaged ones are refused', () => {
  const data = Uint8Array.from({ length: 1000 }, (_, i) => (i * 7) & 255), s = nlzStore(data);
  assert.deepEqual([...s.subarray(0, 5)], [...text('NLZ1'), 0]);
  assert.deepEqual(nlzUnstore(s), data);
  const coded = s.slice(); coded[4] = 1;
  assert.throws(() => nlzUnstore(coded), /coded/);
  assert.throws(() => nlzUnstore(s.subarray(0, 500)), /says 1000/);
  assert.throws(() => nlzUnstore(text('PK\x03\x04junk-junk-junk')), /not an NLZ1/);
});

test('a hand-built module holds its cells, checks itself and clones equal', () => {
  const m = new Module({ title: 'demo', tempo: 120, channels: 4 });
  const s = m.addSample({ name: 'square', pcm: Int8Array.from([127, 127, -128, -128]), loop: { kind: 'forward', start: 0, end: 4 } });
  const p = m.addPattern(16); m.order(p);
  m.set(p, 0, 0, { note: 61, ins: s, vol: [1, 48] }).set(p, 8, 0, { note: NOTE_OFF }).set(p, 4, 2, { fx: [17, 140] });
  assert.deepEqual(m.cell(p, 0, 0), { note: 61, ins: 1, vol: [1, 48] });
  assert.equal(m.cell(p, 1, 0), null);
  m.check();
  const c = m.clone();
  assert.ok(m.equals(c));
  c.set(p, 0, 0, { note: 62 });
  assert.ok(!m.equals(c), 'a clone is independent');
  assert.throws(() => m.set(p, 16, 0, { note: 60 }), /row 16/);
  assert.throws(() => m.set(p, 0, 0, { note: 121 }), /note/);
  assert.throws(() => m.set(9, 0, 0, { note: 60 }), /no pattern 9/);
  m.set(p, 2, 1, { note: 60, ins: 5 });
  assert.throws(() => m.check(), /missing sample 5/);
  assert.throws(() => new Module().check(), /no orders/);
  assert.throws(() => new Module().addSample({ pcm: new Float32Array(4) }), /Int8Array or Int16Array/);
  assert.throws(() => new Module({ kind: 'ft2' }), /unknown kind/);
});

import { Score, toModule, compose, RECIPE_IDS, RECIPES, NOTE_OFF as OFF } from '../index.js';

const noteCells = (m) => m.orders.flatMap((o) => m.patterns[o].cells).filter((c) => c && c.note !== undefined && c.note !== OFF);

test('toModule: timing, pitch, volume, note-offs and polyphony on a hand-made score', () => {
  const s = new Score({ bpm: 120, bars: 2, beats: 4 });
  s.note('lead', 60, 0, 48, 127).note('lead', 64, 48, 24, 64);          // C4 for 2 beats, then E4 a beat later
  s.note('pad', 48, 0, 192, 90).note('pad', 52, 0, 192, 90);           // two notes at once: two voices
  s.track('kit', { drum: true }); s.note('kit', 36, 0, 12).note('kit', 42, 0, 12);
  const { module: m, report } = toModule(s);
  assert.deepEqual([m.speed, m.tempo, report.tempoError], [6, 120, 0]);
  assert.equal(m.channels.length, 5, 'lead 1 voice, pad 2, kit 2');
  assert.deepEqual(m.instruments.map((i) => i.name), ['lead', 'pad', 'kit 36', 'kit 42']);
  const lead = m.cell(0, 0, 0);
  assert.deepEqual(lead, { note: 61, ins: 1, vol: [1, 64] }, 'MIDI 60 is tracker C-5; velocity 127 is volume 64');
  assert.deepEqual(m.cell(0, 2, 0), { note: 65, ins: 1, vol: [1, 32] }, 'the first note ends where the second starts, so no note-off between');
  assert.deepEqual(m.cell(0, 3, 0), { note: OFF }, 'the second lead note ends at row 3');
  assert.equal(m.cell(0, 0, 3)?.note, 61, 'a drum plays at C-5 whatever its key');
  assert.equal(m.cell(0, 0, 3).ins, 3); assert.equal(m.cell(0, 0, 4).ins, 4);
  assert.equal(report.notes, 6); assert.equal(report.moved, 0); assert.equal(report.dropped.length, 0);
  m.check();
});

test('toModule: odd bpm picks another speed; a grid that does not fit the music is reported, not hidden', () => {
  const s = new Score({ bpm: 100, bars: 1 }); s.note('a', 60, 6, 24);   // 6 ticks off a 24-tick row
  const { module: m, report } = toModule(s, { rowsPerBeat: 4 });
  assert.equal(report.moved, 1); assert.equal(report.maxShift, 6);
  assert.equal(m.cell(0, 0, 0).note, 61, 'rounds to the nearer row');
  const fine = toModule(s, { rowsPerBeat: 8 });
  assert.equal(fine.report.moved, 1, 'tick 6 is still half a 12-tick row');
  assert.equal(toModule(s, { rowsPerBeat: 16 }).report.moved, 0, 'a 6-tick row holds it exactly');
  assert.throws(() => toModule(s, { rowsPerBeat: 5 }), /divide 96/);
  assert.throws(() => toModule(new Score({ bpm: 5, bars: 1 }).note('a', 60, 0, 24), {}), /no tracker tempo/);
  const hi = new Score({ bars: 1 }); hi.note('a', 120, 0, 24).note('a', 119, 24, 24);
  assert.deepEqual(toModule(hi).report.dropped.map((d) => d.p), [120], 'MIDI 120 is above tracker note 120');
});

test('toModule: sections cut patterns, repeats are kept once, channels are capped', () => {
  const s = new Score({ bpm: 120, bars: 8 });
  for (let b = 0; b < 8; b++) s.note('a', 60, b * 384, 96);   // the same bar eight times
  const { module: m, report } = toModule(s, { sections: { intro: { from: 0, to: 4 * 384 }, verse: { from: 4 * 384, to: 8 * 384 } } });
  assert.equal(report.patterns, 2); assert.equal(m.orders.length, 2);
  assert.equal(m.patterns.length, 2, 'same music, but each section keeps its own named pattern');
  assert.deepEqual(m.patterns.map((p) => p.name), ['intro', 'verse']);
  const two = toModule(s, { patternRows: 16 });
  assert.equal(two.module.orders.length, 8); assert.equal(two.module.patterns.length, 1);
  assert.ok(toModule(s, { sections: { b: { from: 2 * 384, to: 8 * 384 } } }).module.patterns.some((p) => p.name === 'b'));
  const busy = new Score({ bars: 1 }); for (let p = 40; p < 50; p++) busy.note('a', p, 0, 96);
  assert.throws(() => toModule(busy, { maxChannels: 8 }), /needs 10 channels/);
});

test('toModule: every registered variant converts, keeps every note and is deterministic', () => {
  for (const id of RECIPE_IDS) for (const params of RECIPES[id].variants()) {
    const art = compose(id, params), a = toModule(art.score, { sections: art.sections }), b = toModule(art.score, { sections: art.sections });
    const label = `${id} ${JSON.stringify(params)}`;
    a.module.check();
    assert.ok(a.module.equals(b.module), `${label} is not deterministic`);
    assert.equal(noteCells(a.module).length, a.report.notes - a.report.dropped.length, `${label} lost notes`);
    assert.equal(a.report.notes, art.score.count(), label);
  }
});

import { toNtt, parseNtt, sha256 as sha, hex as hx } from '../index.js';

const chunks = (b) => { const r = new ByteReader(b); r.bytes(8); const out = []; for (let c = r.chunk(); c; c = r.chunk()) out.push(c); return out; };

test('toNtt: layout, chunk order and IDs follow Neumetik\'s UMF', () => {
  const m = new Module({ title: 'demo', tempo: 120, channels: 2 });
  m.addSample({ name: 'sq', pcm: Int8Array.from([100, 100, -100, -100, 0, 0]), loop: { kind: 'forward', start: 0, end: 4 }, rate: 16000 });
  m.addSample({ name: 'tri', pcm: Int16Array.from([0, 1000, -1000, 300, 0, 0]) });
  m.instruments = [{ name: 'sq', track: 'lead' }, { name: 'tri', track: 'bass' }, { name: 'none', track: 'x' }];
  const p = m.addPattern(8); m.order(p).set(p, 0, 0, { note: 61, ins: 1, vol: [1, 40] }).set(p, 4, 1, { note: 49, ins: 2, fx: [17, 130] });
  const b = toNtt(m);
  assert.deepEqual([...b.subarray(0, 8)], [0x55, 0x4d, 0x46, 0x1a, 1, 0, 1, 0], '"UMF" 1A, version 1, flag: waveforms inside');
  assert.deepEqual(chunks(b).map((c) => c.id), ['INFO', 'CHAN', 'ORDR', 'PATT', 'SMPL', 'INST', 'CMPR', 'WAVE', 'WAVE', 'END ']);
  const wave = chunks(b).filter((c) => c.id === 'WAVE');
  // The sample cut at its loop end (4 frames) is the waveform; its ID is SHA-256("umf-wave-1", bits, channels, frames u64, bytes).
  const want = sha(Uint8Array.from([...new TextEncoder().encode('umf-wave-1'), 8, 1, 4, 0, 0, 0, 0, 0, 0, 0, 100, 100, 156, 156]));
  assert.equal(hx(wave[0].body.bytes(32)), hx(want));
  // The 16-bit sample loses its trailing silence: 4 frames, not 6.
  assert.equal(hx(wave[1].body.bytes(32)), hx(sha(Uint8Array.from([...new TextEncoder().encode('umf-wave-1'), 16, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xe8, 3, 0x18, 0xfc, 0x2c, 1]))));
});

test('toNtt then parseNtt gives the module back', () => {
  const m = new Module({ title: 'demo', artist: 'me', message: 'hi\nthere', kind: 'xm', tempo: 140, speed: 5, channels: 3 });
  m.channels[1].name = 'bass'; m.channels[2].flags = 2;
  m.addSample({ name: 'sq', pcm: Int8Array.from([100, 100, -100, -100]), loop: { kind: 'pingpong', start: 1, end: 4 }, rate: 16000, volume: 50, pan: 20 });
  m.addSample({ name: 'tri', pcm: Int16Array.from([0, 1000, -32768, 32767, 5]) });
  m.instruments = [{ name: 'sq', track: 'lead', program: 80, drum: false, key: null }, { name: 'tri', track: 'kit', program: 0, drum: true, key: 36 }];
  m.time = { rowsPerBeat: 8, rowsPerMeasure: 32 };
  const a = m.addPattern(16, 'intro'), b = m.addPattern(4);
  m.order(a).order(b).order(a);
  m.set(a, 0, 0, { note: 61, ins: 1, vol: [1, 40] }).set(a, 3, 2, { note: NOTE_OFF }).set(b, 1, 1, { fx: [17, 130] });
  const back = parseNtt(toNtt(m));
  assert.ok(m.equals(back), 'the module survives');
  assert.deepEqual(toNtt(back), toNtt(m), 'and writes the same bytes again');
  assert.throws(() => parseNtt(toNtt(m).subarray(0, 60)), /ends early/);
  assert.throws(() => parseNtt(new Uint8Array(20)), /not a UMF/);
  const bad = toNtt(m); bad[bad.length - 12] ^= 0xff;   // a point of the last waveform
  assert.throws(() => parseNtt(bad));
});

test('toNtt: a module with no samples is still a valid file, and a Score converts end to end', () => {
  const art = compose('song', {}), { module } = toModule(art.score, { sections: art.sections });
  const bytes = toNtt(module), back = parseNtt(bytes);
  assert.ok(module.equals(back)); assert.deepEqual(toNtt(back), bytes);
  assert.equal(chunks(bytes).filter((c) => c.id === 'WAVE').length, 0);
  assert.deepEqual(toNtt(toModule(art.score).module), toNtt(toModule(art.score).module), 'deterministic');
});

import { fmPatch, oplPatch, OPL_PATCH_IDS } from '../index.js';

test('FM patches are the chip\'s register bytes in S3M order', () => {
  assert.deepEqual(oplPatch('lead'), [1, 33, 24, 2, 0xf6, 0xf3, 0x46, 0x36, 0, 0, 6]);
  assert.deepEqual(fmPatch({ mod: { mult: 2, tremolo: true, vibrato: true, ksr: true, ksl: 2 }, car: { wave: 3 }, additive: true, feedback: 7 }).slice(0, 3), [128 + 64 + 16 + 2, 1, 128]);
  assert.equal(fmPatch({ feedback: 7, additive: true })[10], 15);
  assert.throws(() => fmPatch({ mod: { level: 64 } }), /modulator level/); assert.throws(() => fmPatch({ car: { wave: 4 } }), /carrier wave/);
  assert.throws(() => oplPatch('nope'), /known:/);
  for (const id of OPL_PATCH_IDS) assert.equal(oplPatch(id).length, 11);
});

test('toModule engines: FM patches, MIDI output, and what is recorded about each', () => {
  const art = compose('song', { form: 'loop' });
  const fm = toModule(art.score, { engine: 'opl' });
  assert.deepEqual(fm.module.instruments.slice(0, 3).map((x) => [x.name, x.engine, x.patch]), [['chords', 'opl', 'pad'], ['bass', 'opl', 'bass'], ['lead', 'opl', 'lead']]);
  assert.ok(fm.module.instruments.filter((x) => x.drum).every((x) => x.engine === 'midi' && x.midi.channel === 9));
  assert.deepEqual(fm.report.fallbacks.map((f) => f.track), ['drums'], 'drums cannot be FM, and the report says so');
  assert.equal(fm.module.samples.length, fm.module.instruments.length, 'sample slot i is instrument i');
  assert.ok(fm.module.samples[0].fm && !fm.module.samples[3].fm);
  const midi = toModule(art.score, { engine: 'midi' }).module;
  assert.deepEqual(midi.instruments.slice(0, 3).map((x) => [x.midi.channel, x.midi.program]), [[0, 0], [1, 33], [2, 80]], 'GM programs of the score, channels in order');
  const kick = midi.instruments.findIndex((x) => x.key === 36) + 1, hit = midi.orders.flatMap((o) => midi.patterns[o].cells).find((c) => c?.ins === kick);
  assert.equal(hit.note, 37, 'a MIDI drum plays its key (36), so the cell note is key + 1');
  const sample = toModule(art.score).module.orders.flatMap((o) => toModule(art.score).module.patterns[o].cells).find((c) => c?.ins === kick);
  assert.equal(sample.note, 61, 'a sampled drum plays at C-5');
  const mixed = toModule(art.score, { engine: { '*': 'opl', lead: { engine: 'midi', program: 73, bank: 80 } } }).module;
  assert.deepEqual(mixed.instruments.find((x) => x.track === 'lead').midi, { channel: 0, program: 73, bank: 80 });
  assert.equal(mixed.samples[2].pcm.length, 1, 'a MIDI instrument has a one-frame sample so the player sends its notes');
  assert.throws(() => toModule(art.score, { engine: 'sid' }), /unknown engine/);
  assert.throws(() => toModule(art.score, { engine: { lead: { engine: 'nope' } } }), /unknown engine/);
});

test('effect slots: validated, routed to instruments, chained', () => {
  const m = new Module({ channels: 1 });
  assert.equal(m.addEffect('reverb', { mix: 0.5, params: { roomSize: 0.9 } }), 0);
  assert.deepEqual(m.effects[0].params, [0.9, 0.4, 1, 0.1].map(Math.fround));
  assert.equal(m.effects[0].mix, 0.5); assert.equal(m.addEffect('delay'), 1); assert.equal(m.effects[1].mix, Math.fround(0.35));
  assert.throws(() => m.addEffect('reverbb'), /unknown effect/); assert.throws(() => m.addEffect('reverb', { params: { size: 1 } }), /no parameter/);
  assert.throws(() => m.addEffect('filter', { params: { cutoff: 2 } }), /outside 0..1/);
  const art = compose('song', { form: 'loop' });
  const r = toModule(art.score, { engine: 'opl', effects: [{ kind: 'delay', tracks: ['lead'], mix: 0.3 }, { kind: 'reverb', mix: 0.2 }] }).module;
  assert.deepEqual(r.instruments.filter((x) => !x.drum).map((x) => [x.track, x.effect]), [['chords', 1], ['bass', 1], ['lead', 0]], 'the first slot naming a track wins');
  r.effects[0].output = 1; r.check();
  r.effects[1].output = 0; assert.throws(() => r.check(), /later slot/);
  r.effects[1].output = null; r.instruments[0].effect = 5; assert.throws(() => r.check(), /missing effect slot/);
});

test('SYNV, INSX and EFCT round trip, and sit in the order Neumetik writes them', () => {
  const art = compose('song', { form: 'loop' });
  const { module } = toModule(art.score, { sections: art.sections, engine: { '*': 'opl', lead: 'midi' }, effects: [{ kind: 'reverb', mix: 0.25 }, { kind: 'chorus', tracks: ['bass'] }] });
  const bytes = toNtt(module), back = parseNtt(bytes);
  assert.deepEqual(chunks(bytes).map((c) => c.id), ['INFO', 'CHAN', 'ORDR', 'PATT', 'SMPL', 'INST', 'SYNV', 'INSX', 'EFCT', 'CMPR', 'WAVE', 'END ']);   // every MIDI carrier sample is the same one frame: one waveform
  assert.ok(module.equals(back)); assert.deepEqual(toNtt(back), bytes);
  const synv = chunks(bytes).find((c) => c.id === 'SYNV').body;
  assert.deepEqual([synv.u16(), synv.u16(), synv.u8()], [2, 0, 1], 'two FM slots; the first is slot 0, kind 1 (OPL)');
  assert.deepEqual([...synv.bytes(11)], oplPatch('pad'));
  const no = toNtt(toModule(art.score).module);
  assert.ok(!chunks(no).some((c) => ['SYNV', 'INSX', 'EFCT'].includes(c.id)), 'no engines, no extra chunks');
});

import { cycle, drumShot, CYCLE, CYCLE_RATE, DRUM_RATE } from '../index.js';

test('own-synth samples: one cycle per wave, one shot per drum key, deterministic', () => {
  for (const w of ['sine', 'triangle', 'square', 'saw']) {
    const c = cycle(w);
    assert.ok(c instanceof Int8Array && c.length === CYCLE);
    assert.equal(Math.max(...c.map(Math.abs)), 110, `${w} peaks at 110`);
    assert.deepEqual(c, cycle(w));
  }
  assert.equal(cycle('sine')[0], 0); assert.equal(cycle('sine')[16], 110); assert.deepEqual(cycle('nonsense'), cycle('triangle'));
  assert.equal(Math.round(CYCLE_RATE / CYCLE * 10) / 10, 261.6, 'a 64-frame cycle at this rate is C-5');
  const kick = drumShot(36), hat = drumShot(42);
  assert.ok(kick instanceof Int16Array && kick.length === Math.ceil(0.4 * DRUM_RATE));
  assert.equal(Math.max(...kick.map(Math.abs)), Math.round(0.8 * 32767));
  assert.deepEqual(drumShot(42), hat); assert.notDeepEqual(drumShot(42), drumShot(46));
  assert.ok(drumShot(99).length > 0, 'a key the table lacks still gets a click');
});

test('toModule engine sample: every instrument carries its sound, so the file plays in any tracker', () => {
  const art = compose('song', { form: 'loop' });
  const { module: m, report } = toModule(art.score, { sections: art.sections, engine: 'sample' });
  assert.equal(report.fallbacks.length, 0, 'drums are samples too');
  assert.equal(m.samples.length, m.instruments.length);
  const lead = m.instruments.findIndex((x) => x.track === 'lead'), kick = m.instruments.findIndex((x) => x.key === 36);
  assert.deepEqual([m.samples[lead].pcm.length, m.samples[lead].loop, m.samples[lead].rate], [64, { kind: 'forward', start: 0, end: 64 }, CYCLE_RATE]);
  assert.deepEqual([m.samples[kick].pcm.constructor.name, m.samples[kick].loop, m.samples[kick].rate], ['Int16Array', null, DRUM_RATE]);
  assert.deepEqual(m.instruments[lead].env, [0.005, 0.08, 0.7, 0.06]);
  assert.ok(m.instruments.every((x) => x.engine === 'sample'));
  const bytes = toNtt(m), back = parseNtt(bytes);
  assert.ok(m.equals(back)); assert.deepEqual(toNtt(back), bytes);
  assert.ok(!chunks(bytes).some((c) => c.id === 'INSX' || c.id === 'SYNV'), 'no MIDI and no FM: nothing a host must supply');
  // The lead's volume envelope is in INST: rise, fall to the sustain, hold, release (ticks are 2.5 / tempo seconds).
  const inst = chunks(bytes).find((c) => c.id === 'INST').body;
  inst.u16(); for (let i = 0; i < lead; i++) { inst.bytes(32); inst.str(); inst.str(); inst.bytes(360); for (let e = 0; e < 3; e++) { inst.u8(); inst.bytes(inst.u8() * 3 + 4); } inst.bytes(18); }
  inst.bytes(32); inst.str(); inst.str(); inst.bytes(360);
  assert.deepEqual([inst.u8(), inst.u8()], [1, 4], 'enabled, four points');
  const pts = [0, 1, 2, 3].map(() => [inst.u16(), inst.i8()]), tick = 2.5 / m.tempo;
  assert.deepEqual(pts.map((p) => p[1]), [0, 64, 45, 0]);
  assert.deepEqual(pts.map((p) => p[0]), [0, 1, 1 + Math.max(1, Math.round(0.08 / tick)), 1 + Math.max(1, Math.round(0.08 / tick)) + Math.max(1, Math.round(0.06 / tick))]);
  assert.deepEqual([inst.u8(), inst.u8(), inst.u8(), inst.u8()], [255, 255, 2, 2], 'no loop; sustain on the third point');
});

import { toManifest, bake, parseWav, toWav, MANIFEST_ENGINES } from '../index.js';

const tone = (hz, secs, rate = 22050) => toWav(Float32Array.from({ length: Math.round(secs * rate) }, (_, i) => 0.5 * Math.sin((2 * Math.PI * hz * i) / rate)), rate);

test('parseWav reads 16-bit and 8-bit PCM, mixes stereo down, and refuses the rest', () => {
  const w = parseWav(tone(440, 0.1));
  assert.equal(w.rate, 22050); assert.equal(w.pcm.length, 2205); assert.ok(Math.max(...w.pcm) > 15000);
  const stereo = new Uint8Array(44 + 8), v = new DataView(stereo.buffer);
  [...'RIFF'].forEach((c, i) => stereo[i] = c.charCodeAt(0)); [...'WAVEfmt '].forEach((c, i) => stereo[8 + i] = c.charCodeAt(0));
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 2, true); v.setUint32(24, 8000, true); v.setUint16(34, 16, true);
  [...'data'].forEach((c, i) => stereo[36 + i] = c.charCodeAt(0)); v.setUint32(40, 8, true);
  v.setInt16(44, 1000, true); v.setInt16(46, 3000, true); v.setInt16(48, -500, true); v.setInt16(50, -1500, true);
  assert.deepEqual([...parseWav(stereo).pcm], [2000, -1000]);
  assert.throws(() => parseWav(new Uint8Array(20)), /not a WAV/);
  const f32 = tone(440, 0.01).slice(); new DataView(f32.buffer).setUint16(20, 3, true);
  assert.throws(() => parseWav(f32), /integer PCM/);
});

test('bake: the manifest names what to render, and the waveform lands in the module at the right pitch', () => {
  const art = compose('song', { form: 'loop' });
  const { module } = toModule(art.score, { sections: art.sections, engine: { '*': { engine: 'bake' }, lead: { engine: 'bake', source: 'chip.nes', recipe: 'pulse' } } });
  const man = toManifest(module, { rate: 32000 });
  assert.equal(man.version, 1); assert.equal(man.rate, 32000); assert.equal(man.slots.length, module.instruments.length);
  const by = Object.fromEntries(man.slots.map((s) => [s.id, s]));
  assert.deepEqual([by.bass.engine, by.bass.recipe, by.lead.engine, by.lead.recipe, by['drums-36'].engine, by['drums-36'].recipe], ['midi', '33', 'chip.nes', 'pulse', 'midi.drum', '36']);
  assert.ok(by.bass.note >= 24 && by.bass.note < 60, 'rendered near the bass\'s own notes, not at middle C'); assert.equal(by['drums-36'].note, 60);
  assert.ok(man.slots.every((s) => MANIFEST_ENGINES.includes(s.engine) && Number.isInteger(s.seed) && s.seed >= 0 && s.ms >= 250));
  assert.deepEqual(toManifest(module, { rate: 32000 }), man, 'deterministic');
  // Before any render: Composer's own sound, so the file plays.
  const before = toNtt(module);
  assert.ok(module.samples.every((s) => s.pcm.length > 1));
  // A render at the instrument's note, played at the same note, must come out at that note: so its C-5 rate is shifted by (60 - note) semitones.
  const note = by.bass.note, rendered = tone(130.8128, 0.5, 32000);
  const r = bake(module, { bass: rendered, 'drums-36': tone(60, 0.2, 32000), nope: rendered });
  assert.deepEqual(r.baked, ['bass', 'drums-36']); assert.ok(r.missing.includes('lead') && !r.missing.includes('nope'));
  const i = module.instruments.findIndex((x) => x.id === 'bass');
  assert.equal(module.samples[i].rate, Math.round(32000 * 2 ** ((60 - note) / 12)));
  assert.equal(module.samples[module.instruments.findIndex((x) => x.id === 'drums-36')].rate, 32000, 'a hit plays at its own rate whatever the note');
  assert.ok(!module.instruments[i].env && module.instruments[i].baked, 'the own-synth envelope is gone: the render has its own');
  assert.notDeepEqual(toNtt(module), before);
  const back = parseNtt(toNtt(module)); assert.ok(module.equals(back));
  assert.throws(() => bake(module, { lead: toWav(new Float32Array(100)) }), /silent/);
  assert.throws(() => toModule(art.score, { engine: { lead: { engine: 'bake', source: 'chip.zx' } } }), /unknown bake source/);
  assert.throws(() => toModule(art.score, { engine: { lead: { engine: 'bake', source: 'bgn' } } }), /needs a recipe/);
  const rain = toModule(art.score, { engine: { lead: { engine: 'bake', source: 'bgn', recipe: 'Light rain' } } }).module;
  const rx = rain.instruments.find((x) => x.source === 'bgn');
  assert.deepEqual([rx.loop, rx.ms], ['whole', 8000]);
  const lead = rain.orders.flatMap((o) => rain.patterns[o].cells).find((c) => c?.ins === rain.instruments.indexOf(rx) + 1);
  assert.equal(lead.note, 61, 'a bed plays at C-5 whatever the score says');
  bake(rain, { 'lead': tone(300, 0.5) });
  assert.deepEqual(rain.samples[rain.instruments.indexOf(rx)].loop?.kind, 'forward');
});
