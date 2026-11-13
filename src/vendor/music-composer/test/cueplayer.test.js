import test from 'node:test';
import assert from 'node:assert/strict';
import { composeCues, nextSwitch, audioFiles, renderCue, cueAudio, createCuePlayer, loadCueAudio, parseWav } from '../index.js';

// A Web Audio context that only records: enough to see when the player starts, stops and fades things.
function fake() {
  const ctx = { currentTime: 0, made: [], destination: { kind: 'destination' } };
  const param = () => ({ value: 1, calls: [], setValueAtTime(v, t) { this.calls.push(['set', v, t]); }, linearRampToValueAtTime(v, t) { this.calls.push(['ramp', v, t]); }, cancelScheduledValues(t) { this.calls.push(['cancel', t]); } });
  ctx.createGain = () => { const n = { kind: 'gain', gain: param(), connect() {}, disconnect() { this.gone = true; } }; ctx.made.push(n); return n; };
  ctx.createBufferSource = () => { const n = { kind: 'source', starts: [], stops: [], connect() {}, start(w, o = 0) { this.starts.push([w, o]); }, stop(w) { this.stops.push(w); } }; ctx.made.push(n); return n; };
  ctx.sources = () => ctx.made.filter((n) => n.kind === 'source');
  return ctx;
}
const buf = (id) => ({ duration: 1, id });
const sheet = {
  cues: [
    { id: 'explore', recipe: 'song', params: { form: 'verse-chorus', seed: 'k' }, layers: { base: ['chords', 'bass'], rest: ['lead', 'drums'] }, on: ['base'] },
    { id: 'battle', recipe: 'song', params: { form: 'loop', seed: 'b' } },
    { id: 'pickup', recipe: 'jingle', params: { kind: 'pickup' }, tail: 2 },
  ],
  transitions: [{ from: 'explore', to: 'battle', via: 'pickup', at: 'bar' }, { from: 'battle', to: 'explore', at: 'phrase' }],
};
const { manifest, cues } = composeCues(sheet), by = Object.fromEntries(manifest.cues.map((c) => [c.id, c]));
const audio = { explore: { base: buf('eb'), rest: buf('er') }, battle: buf('battle'), pickup: buf('pickup') };
const bar = (e) => (e.beats * 60) / e.bpm;
const setup = (opts = {}) => { const ctx = fake(), timers = []; const p = createCuePlayer(ctx, manifest, audio, { schedule: (f, ms) => timers.push([f, ms]), ...opts }); return { ctx, p, timers }; };

test('cue audio: loops seam, stingers ring out, stems sum to the mix', () => {
  const [explore, battle, pickup] = cues;
  assert.deepEqual(audioFiles(by.explore), { layers: { base: 'explore.base.wav', rest: 'explore.rest.wav' } });
  assert.deepEqual(audioFiles(by.battle), { mix: 'battle.wav' });
  const r = renderCue(explore), rate = r.sampleRate, loopLen = Math.ceil((by.explore.bars * by.explore.beats * 60 * rate) / by.explore.bpm);
  assert.equal(r.layers.base.length, loopLen); assert.equal(r.layers.rest.length, loopLen);
  const peak = r.layers.base.reduce((m, v, i) => Math.max(m, Math.abs(v + r.layers.rest[i])), 0);
  assert.ok(peak <= 0.9 + 1e-6 && peak > 0.3, `stems sum within the mix's limit (${peak})`);
  assert.equal(renderCue(battle).mix.length, Math.ceil((by.battle.bars * by.battle.beats * 60 * rate) / by.battle.bpm));
  assert.ok(renderCue(pickup).mix.length > Math.ceil((by.pickup.bars * by.pickup.beats * 60 * rate) / by.pickup.bpm), 'a stinger keeps its tail');
  const files = cueAudio(explore);
  assert.deepEqual(Object.keys(files), ['explore.base.wav', 'explore.rest.wav']);
  assert.equal(parseWav(files['explore.base.wav']).pcm.length, loopLen);
  assert.throws(() => audioFiles({ id: 'x', layers: { '../up': {} } }), /must be letters/);
});

test('cue player: starts now, brings stems up on the bar, and stays idempotent', () => {
  const { ctx, p } = setup();
  ctx.currentTime = 10;
  const t = p.play('explore');
  assert.ok(Math.abs(t - 10.03) < 1e-9);
  const [a, b] = ctx.sources(); assert.deepEqual([a.starts[0][0], b.starts[0][0]], [t, t], 'stems start together');
  assert.equal(a.loop, true); assert.ok(Math.abs(a.loopEnd - by.explore.bars * bar(by.explore)) < 1e-9 && a.loopStart === 0, 'loops on the cue\'s exact length');
  const gains = ctx.made.filter((n) => n.kind === 'gain'), stem = (i) => gains[i + 2];   // master, out, then one gain a stem
  assert.deepEqual([stem(0).gain.value, stem(1).gain.value], [1, 0], 'only the starting layer is up');
  assert.equal(p.play('explore'), t, 'asking again does nothing'); assert.equal(ctx.sources().length, 2);
  ctx.currentTime = t + bar(by.explore) * 2.2;
  const when = p.layers(['base', 'rest'], { seconds: 1 });
  assert.ok(Math.abs(when - (t + bar(by.explore) * 3)) < 1e-9, 'lands on the next bar line');
  assert.deepEqual(stem(1).gain.calls.slice(-2), [['set', 0, when], ['ramp', 1, when + 1]]);
  assert.deepEqual(p.state().layers, ['base', 'rest']);
  assert.throws(() => p.layers(['nope']), /no layer/);
});

test('cue player: a switch lands on the sheet\'s bar, through its stinger', () => {
  const { ctx, p, timers } = setup();
  const t0 = p.play('explore'); ctx.currentTime = t0 + bar(by.explore) * 5.4;
  const starts = p.play('battle');
  const lands = t0 + bar(by.explore) * 6, bridge = (by.pickup.lengthMs + by.pickup.tailMs) / 1000;
  assert.ok(Math.abs(starts - (lands + bridge)) < 1e-9, 'the new loop starts after the bridge');
  const pickup = ctx.sources().find((s) => s.buffer?.id === 'pickup' && s.starts[0][0] === lands);
  assert.ok(pickup, 'the stinger starts on the bar line');
  assert.deepEqual(p.state().switching.to, 'battle'); assert.equal(p.state().cue, 'explore', 'still the old music until it lands');
  const old = ctx.made.filter((n) => n.kind === 'gain')[1];
  assert.deepEqual(old.gain.calls[0], ['set', 1, lands]); assert.equal(old.gain.calls[1][0], 'ramp');
  ctx.currentTime = lands + 0.01;
  assert.equal(p.state().switching, null); assert.equal(p.state().cue, 'battle');
  timers.forEach(([f]) => f());
  assert.ok(ctx.sources().slice(0, 2).every((s) => s.stops.length === 1), 'the old music is stopped once it has faded');
});

test('cue player: a request before a switch lands replaces it, and the old music carries on', () => {
  const { ctx, p, timers } = setup();
  const t0 = p.play('explore'); ctx.currentTime = t0 + bar(by.explore) * 1.1;
  p.play('battle'); p.play('explore');
  assert.equal(p.state().switching, null); assert.equal(p.state().cue, 'explore');
  const old = ctx.made.filter((n) => n.kind === 'gain')[1];
  assert.equal(old.gain.calls.at(-1)[0], 'set', 'its fade-out is cancelled'); assert.equal(old.gain.calls.at(-1)[1], 1);
  timers.forEach(([f]) => f());
  assert.equal(ctx.sources().slice(0, 2).some((s) => s.stops.length), false, 'and it is not stopped by the timer');
});

test('cue player: a section seeks inside the cue, a stinger plays over, stop fades out', () => {
  const { ctx, p } = setup();
  const t0 = p.play('explore'), sec = by.explore.sections.find((s) => s.fromBar > 0);
  ctx.currentTime = t0 + 0.1;
  const lands = p.play(`explore:${sec.name}`, { at: 'section' });
  const seek = ctx.sources().filter((s) => s.starts[0][0] === lands);
  assert.equal(seek.length, 2); assert.ok(Math.abs(seek[0].starts[0][1] - sec.fromBar * bar(by.explore)) < 1e-9, 'starts at the section\'s bar in the audio');
  assert.ok(Math.abs(lands - (t0 + sec.fromBar * bar(by.explore))) < 1e-9 || lands > t0, 'lands at a section start');
  const ends = p.stinger('pickup');
  assert.ok(ends > ctx.currentTime); assert.equal(p.state().switching.to, 'explore');
  p.stop({ seconds: 1 });
  assert.equal(p.state().cue, null); assert.equal(p.position(), null);
  assert.throws(() => p.play('nope'), /unknown cue/);
  assert.throws(() => createCuePlayer(fake(), manifest, {}).play('battle'), /no audio/);
});

test('loadCueAudio fetches each file the cue is baked to', async () => {
  const ctx = { decodeAudioData: async (b) => ({ duration: 1, bytes: b.byteLength }) }, asked = [];
  const out = await loadCueAudio(ctx, manifest, '/m/', { fetch: async (u) => { asked.push(u); return { ok: true, arrayBuffer: async () => new ArrayBuffer(4) }; } });
  assert.deepEqual(asked.sort(), ['/m/battle.wav', '/m/explore.base.wav', '/m/explore.rest.wav', '/m/pickup.wav']);
  assert.deepEqual(Object.keys(out.explore), ['base', 'rest']); assert.equal(out.battle.duration, 1);
  await assert.rejects(loadCueAudio(ctx, manifest, '/m/', { fetch: async () => ({ ok: false, status: 404 }) }), /404/);
  assert.equal(nextSwitch(by.battle, 0, 'phrase'), 0);
  // Best format first, falling through to WAV when it is missing or will not decode.
  const tried = [], bad = { decodeAudioData: async (b) => { if (b.byteLength === 9) throw new Error('cannot decode'); return { duration: 1 }; } };
  const f2 = async (u) => { tried.push(u); return u.endsWith('.opus') ? (u.includes('battle') ? { ok: false, status: 404 } : { ok: true, arrayBuffer: async () => new ArrayBuffer(9) }) : { ok: true, arrayBuffer: async () => new ArrayBuffer(4) }; };
  const got = await loadCueAudio(bad, manifest, '/m/', { fetch: f2, formats: ['opus', 'wav'] });
  assert.ok(got.battle.duration && got.explore.base.duration && got.pickup.duration);
  assert.deepEqual(tried.filter((u) => u.includes('battle')), ['/m/battle.opus', '/m/battle.wav']);
  assert.deepEqual(tried.filter((u) => u.includes('pickup')), ['/m/pickup.opus', '/m/pickup.wav'], 'a file that will not decode falls through too');
});
