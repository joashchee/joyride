// .ntt: Neumetik's UMF ("Neumetik Tracker type") saved with its samples, so it plays with no repository. Written from the layout in
// Neumetik's tracker/docs/umf-format.md and repository.md; no code of theirs is used. `toNtt(module)` writes the chunks INFO, CHAN, ORDR,
// PATT, SMPL, INST, then when used SONG, SYNV (FM patches), INSX (MIDI output, effect slot), EFCT (effect slots), CMPR, WAVE..., END. PATT and every waveform are NLZ1 *stored* (method 0), which every reader accepts.
// `parseNtt` reads back what `toNtt` writes (and any .ntt whose streams are stored); coded streams need Neumetik's decoder and throw.
//
// CMPR is Composer's own chunk (the instrument slots as JSON, the provenance); a UMF reader passes over chunks it does not know.
// A cover (COVR) is left out: it is optional, and Neumetik paints one when it saves the song.
import { ByteWriter, ByteReader } from './bytes.js';
import { sha256 } from './sha256.js';
import { nlzStore, nlzUnstore } from './nlz.js';
import { Module, KINDS, LOOPS, EFFECTS, EFFECT_KINDS } from './module.js';

const MAGIC = [0x55, 0x4d, 0x46, 0x1a], NONE = new Uint8Array(32), TRACKER = 'Composer';
const KIND_NAMES = Object.keys(KINDS), LOOP_NAMES = Object.keys(LOOPS);
const INSTRUMENT = { fadeout: 2048, globalVolume: 128, pitchPanCenter: 60 };   // a note-off fades a pitched note out in about 32 ticks

const cat = (...parts) => { const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0)); let at = 0; for (const p of parts) { out.set(p, at); at += p.length; } return out; };
const sum = (s, ...parts) => sha256(cat(new TextEncoder().encode(s), ...parts));
const u32le = (v) => new ByteWriter().u32(v).done();
const u64le = (v) => new ByteWriter().u64(v).done();
const bitsOf = (pcm) => (pcm instanceof Int8Array ? 8 : 16);
const sameId = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

/** A loop held inside `frames`, or the off loop when nothing is left of it. */
function clampLoop(loop, frames) {
  const end = Math.min(loop?.end ?? 0, frames);
  return !loop || loop.kind === 'off' || loop.start >= end ? { kind: 'off', start: 0, end: 0 } : { kind: loop.kind, start: loop.start, end };
}
/** Where the audible part ends: at a loop's end, else at the last point that is not silent. */
function audibleEnd(s) {
  const lp = clampLoop(s.loop, s.pcm.length);
  if (lp.kind !== 'off') return lp.end;
  let end = s.pcm.length;
  while (end > 0 && s.pcm[end - 1] === 0) end--;
  return end;
}
// Hashed as stored: 8-bit points as bytes, 16-bit as little-endian pairs.
function waveBytes(pcm) {
  if (pcm instanceof Int8Array) return Uint8Array.from(pcm, (v) => v & 255);
  const b = new Uint8Array(pcm.length * 2);
  pcm.forEach((v, i) => { b[i * 2] = v & 255; b[i * 2 + 1] = (v >> 8) & 255; });
  return b;
}
const waveId = (pcm) => sum('umf-wave-1', Uint8Array.of(bitsOf(pcm), 1), u64le(pcm.length), waveBytes(pcm));
const loopBytes = (l) => (l.kind === 'off' ? Uint8Array.of(0) : cat(Uint8Array.of(LOOPS[l.kind]), u32le(l.start), u32le(l.end)));
const sampleId = (wave, lp) => sum('umf-sample-1', wave, loopBytes(lp), loopBytes({ kind: 'off' }));
// An instrument whose notes all play the sample in the slot of the same number (none when there is no sample): the keymap is the identity.
function instrumentId(sid, pts = null) {
  const keys = []; for (let n = 0; n < 120; n++) keys.push(...(sid ? [Uint8Array.of(n, n), sid] : [Uint8Array.of(n, 0xff)]));
  // The volume envelope as the repository hashes it: off is one 0; on is 1, the count, each point's tick (u16) and value, the loop and sustain pairs, carry, filter.
  const env = pts ? cat(Uint8Array.of(1, pts.length), ...pts.map(([t, v]) => Uint8Array.of(t & 255, t >> 8, v & 255)), Uint8Array.of(255, 255, 2, 2, 0, 0)) : Uint8Array.of(0);
  return sum('umf-instrument-1', ...keys, env, Uint8Array.of(0, 0), u32le(pts ? 0 : INSTRUMENT.fadeout), Uint8Array.of(INSTRUMENT.globalVolume, 0xff, 0, 0, 0, 0, INSTRUMENT.pitchPanCenter, 0, 0), Uint8Array.of(0xff, 0xff));
}

// A waveform file (UMFW, version 1): its points as differences, a byte plane at a time, in a stored NLZ1 stream.
function packWave(pcm) {
  const n = pcm.length, bits = bitsOf(pcm);
  let body;
  if (bits === 8) { body = new Uint8Array(n); let prev = 0; for (let i = 0; i < n; i++) { body[i] = (pcm[i] - prev) & 255; prev = pcm[i]; } }
  else { body = new Uint8Array(n * 2); let prev = 0; for (let i = 0; i < n; i++) { const d = (pcm[i] - prev) & 0xffff; body[i] = d & 255; body[n + i] = d >> 8; prev = pcm[i]; } }
  return new ByteWriter().ascii('UMFW').u8(1).u8(bits).u8(1).u8(0).u64(n).bytes(nlzStore(body)).done();
}
function unpackWave(file) {
  const r = new ByteReader(file);
  if (r.ascii(4) !== 'UMFW') throw new Error('a waveform is not a waveform file');
  const version = r.u8(), bits = r.u8(), channels = r.u8(); r.u8();
  const frames = r.u64();
  if (version !== 1) throw new Error('a waveform was packed by a newer Neumetik (version 2 needs its decoder)');
  if (channels !== 1 || (bits !== 8 && bits !== 16)) throw new Error('only mono 8- and 16-bit waveforms can be read here');
  const body = nlzUnstore(r.bytes(r.left));
  if (body.length !== frames * (bits / 8)) throw new Error('a waveform is cut short');
  if (bits === 8) { const pcm = new Int8Array(frames); let prev = 0; for (let i = 0; i < frames; i++) { prev = (prev + body[i]) << 24 >> 24; pcm[i] = prev; } return pcm; }
  const pcm = new Int16Array(frames); let prev = 0;
  for (let i = 0; i < frames; i++) { prev = (prev + (body[i] | (body[frames + i] << 8))) << 16 >> 16; pcm[i] = prev; }
  return pcm;
}

function writeEnvelope(w) { w.u8(0).u8(0).u8(255).u8(255).u8(255).u8(255); }   // off, no points, no loop, no sustain
// A track's [attack, decay, sustain level, release] as a volume envelope: rise to 64, fall to the sustain, hold there until the note is
// let go, then fall to 0. Ticks are 2.5 / tempo seconds, so the envelope is made for the song's own tempo.
function envelopePoints([attack, decay, sustain, release], tempo) {
  const tick = (s) => Math.max(1, Math.round((s * tempo) / 2.5)), pts = [[0, 0]];
  for (const [dt, v] of [[attack, 64], [decay, Math.round(sustain * 64)], [release, 0]]) pts.push([pts[pts.length - 1][0] + tick(dt), v]);
  return pts;
}
function writeVolumeEnvelope(w, pts) {
  w.u8(1).u8(pts.length); for (const [t, v] of pts) w.u16(t).i8(v);
  w.u8(255).u8(255).u8(2).u8(2);   // no loop; hold at the third point until the note ends
}

/** The .ntt bytes of a Module: its samples carried inside, every ID worked out as a repository's scan would. */
export function toNtt(module) {
  module.check();
  const m = module, useInstruments = m.instruments.length > 0;
  const refs = m.samples.map((s) => {
    const end = audibleEnd(s);
    if (!end) return null;
    const pcm = s.pcm.subarray(0, end), wave = waveId(pcm), lp = clampLoop(s.loop, end);
    return { pcm, wave, lp, sample: sampleId(wave, lp), frames: end };
  });
  const out = new ByteWriter().bytes(Uint8Array.from(MAGIC)).u16(1).u16(1);

  out.chunk('INFO', (w) => {
    for (const s of [m.title, m.artist, m.message, 'umf', 'Composer', TRACKER]) w.str(s);
    w.u8(KINDS[m.kind]).u16((m.kind === 'it' || m.kind === 'xm' ? 1 : 0) | (useInstruments ? 4 : 0));
    w.u8(m.speed).u16(m.tempo).u8(128).u8(48).u8(128).u16(m.restart).bytes(NONE).str('');
  });
  out.chunk('CHAN', (w) => { w.u16(m.channels.length); for (const c of m.channels) w.u8(c.pan).u8(c.vol).u8(c.flags).str(c.name); });
  out.chunk('ORDR', (w) => { w.u16(m.orders.length); for (const o of m.orders) w.u16(o); });

  const pat = new ByteWriter().u16(m.patterns.length);
  for (const p of m.patterns) {
    pat.u16(p.rows).u16(m.channels.length).str(p.name);
    for (const c of p.cells) {
      if (!c) { pat.u8(0); continue; }
      pat.u8((c.note !== undefined ? 1 : 0) | (c.ins !== undefined ? 2 : 0) | (c.vol ? 4 : 0) | (c.fx ? 8 : 0));
      if (c.note !== undefined) pat.u8(c.note);
      if (c.ins !== undefined) pat.u8(c.ins);
      if (c.vol) pat.u8(c.vol[0]).u8(c.vol[1]);
      if (c.fx) pat.u8(c.fx[0]).u8(c.fx[1]);
    }
  }
  out.chunk('PATT', (w) => w.bytes(nlzStore(pat.done())));

  out.chunk('SMPL', (w) => {
    w.u16(m.samples.length);
    m.samples.forEach((s, i) => {
      const r = refs[i], lp = r ? r.lp : { kind: 'off', start: 0, end: 0 };
      w.str(s.name).str('').bytes(r ? r.sample : NONE).bytes(r ? r.wave : NONE).u32(r ? r.frames : 0).u8(r ? bitsOf(s.pcm) : 8).u8(1).u32(s.rate).u8(s.volume).u8(64);
      w.u8(s.pan === null ? 0 : 1).u8(s.pan ?? 0).u8(LOOPS[lp.kind]).u32(lp.start).u32(lp.end).u8(0).u32(0).u32(0).u8(0).u8(0).u8(0).u8(0);
    });
  });

  out.chunk('INST', (w) => {
    w.u16(m.instruments.length);
    m.instruments.forEach((x, i) => {
      const r = refs[i] ?? null;
      const pts = x.env ? envelopePoints(x.env, m.tempo) : null;
      w.bytes(instrumentId(r?.sample ?? null, pts)).str(x.name).str('');
      for (let n = 0; n < 120; n++) w.u8(n).u16(i < m.samples.length ? i + 1 : 0);
      if (pts) writeVolumeEnvelope(w, pts); else writeEnvelope(w);
      writeEnvelope(w); writeEnvelope(w);
      w.u32(pts ? 0 : INSTRUMENT.fadeout).u8(INSTRUMENT.globalVolume).u8(0).u8(0);
      w.u8(0).u8(0).u8(0).u8(0).u8(INSTRUMENT.pitchPanCenter).u8(0).u8(0).u8(0).u8(0).u8(0).u8(0);
    });
  });

  // The song's order lists: the one being edited is ORDR (the song as played), then one per section a host can switch to.
  if (m.time.rowsPerBeat !== 4 || m.time.rowsPerMeasure !== 16 || m.lists.length) {
    out.chunk('SONG', (w) => {
      w.u8(0).u16(m.time.rowsPerBeat).u16(m.time.rowsPerMeasure).u8(0).u16(0).u16(1 + m.lists.length);
      for (const l of [{ name: '', restart: m.restart, orders: m.orders }, ...m.lists]) { w.str(l.name).u16(l.restart).u16(l.orders.length); for (const o of l.orders) w.u16(o); }
      w.u16(0).u16(0);
    });
  }
  const fm = m.samples.map((x, i) => [x, i]).filter(([x]) => x.fm);
  if (fm.length) out.chunk('SYNV', (w) => { w.u16(fm.length); for (const [x, i] of fm) w.u16(i).u8(1).bytes(Uint8Array.from(x.fm)); });
  if (m.instruments.some((x) => x.midi || x.effect != null)) {
    out.chunk('INSX', (w) => {
      w.u16(m.instruments.length);
      for (const x of m.instruments) {
        w.u8(255).u8(255).u8(255);   // no envelope release nodes
        if (x.midi) w.u8(1).u8(x.midi.channel).u8(x.midi.program ?? 255).u16(x.midi.bank ?? 65535); else w.u8(0);
        w.u8(x.effect ?? 255).u8(0);   // its effect slot; no phrase
      }
    });
  }
  if (m.effects.length) {
    out.chunk('EFCT', (w) => {
      w.u16(m.effects.length);
      for (const e of m.effects) { w.u8(EFFECT_KINDS.indexOf(e.kind)).str(e.name).f32(e.mix).u8(e.bypass ? 1 : 0).u8(e.output ?? 255).u8(e.params.length); for (const p of e.params) w.f32(p); }
    });
  }
  // Composer's own record: what each instrument slot is for, and that this music was composed here.
  out.chunk('CMPR', (w) => w.u8(1).str(JSON.stringify({ provenance: 'Project-created', instruments: m.instruments, ...(m.meta ? { meta: m.meta } : {}) })));

  const done = [];
  m.samples.forEach((s, i) => {
    const r = refs[i];
    if (!r || done.some((d) => sameId(d, r.wave))) return;
    done.push(r.wave);
    out.chunk('WAVE', (w) => w.bytes(r.wave).bytes(packWave(r.pcm)));
  });
  out.chunk('END ', () => {});
  return out.done();
}

/** A Module from .ntt (or .umf) bytes whose waveforms are in the file; every waveform is checked against its ID. */
export function parseNtt(bytes) {
  const r = new ByteReader(bytes);
  if (bytes.length < 8 || MAGIC.some((b, i) => bytes[i] !== b)) throw new Error('not a UMF or NTT file');
  r.bytes(4);
  if (r.u16() > 1) throw new Error('it was saved by a newer Neumetik');
  r.u16();
  let m = null, info = null, cmpr = null, time = null, lists = [], synv = null, insx = null, efct = null;
  const waves = new Map(), pending = { chan: null, ordr: null, patt: null, smpl: null, inst: null };
  for (let c = r.chunk(); c; c = r.chunk()) {
    const { id, body: b } = c;
    if (id === 'END ') break;
    if (id === 'INFO') {
      const [title, artist, message] = [b.str(), b.str(), b.str()]; b.str(); b.str(); b.str();
      const kind = KIND_NAMES[b.u8()] ?? 'mod', flags = b.u16(), speed = b.u8(), tempo = b.u16(); b.u8(); b.u8(); b.u8();
      info = { title, artist, message, kind, flags, speed, tempo, restart: b.u16() };
    } else if (id === 'CHAN') pending.chan = b;
    else if (id === 'ORDR') pending.ordr = b;
    else if (id === 'PATT') pending.patt = new ByteReader(nlzUnstore(b.bytes(b.left)));
    else if (id === 'SMPL') pending.smpl = b;
    else if (id === 'INST') pending.inst = b;
    else if (id === 'SONG') {
      b.u8(); time = { rowsPerBeat: b.u16(), rowsPerMeasure: b.u16() }; b.u8();
      const edited = b.u16();
      lists = Array.from({ length: b.u16() }, () => ({ name: b.str(), restart: b.u16(), orders: Array.from({ length: b.u16() }, () => b.u16()) })).filter((_, i) => i !== edited);
    }
    else if (id === 'SYNV') synv = b;
    else if (id === 'INSX') insx = b;
    else if (id === 'EFCT') efct = b;
    else if (id === 'CMPR') { b.u8(); cmpr = JSON.parse(b.str()); }
    else if (id === 'WAVE') { const wid = b.bytes(32); waves.set(Array.from(wid).join(','), unpackWave(b.bytes(b.left))); }
  }
  if (!info || !pending.chan || !pending.ordr || !pending.patt) throw new Error('the file lacks INFO, CHAN, ORDR or PATT');

  const n = pending.chan.u16();
  m = new Module({ ...info, channels: Math.max(1, n) });
  m.channels = Array.from({ length: n }, () => { const pan = pending.chan.u8(), vol = pending.chan.u8(), flags = pending.chan.u8(); return { pan, vol, flags, name: pending.chan.str() }; });
  m.restart = info.restart;
  if (time) m.time = time;
  m.lists = lists; m.meta = cmpr?.meta ?? null;
  for (let k = pending.ordr.u16(); k > 0; k--) m.orders.push(pending.ordr.u16());
  const p = pending.patt;
  for (let k = p.u16(); k > 0; k--) {
    const rows = p.u16(), chans = p.u16(), name = p.str();
    if (chans !== m.channels.length) throw new Error('a pattern has a different channel count from the song');
    const cells = [];
    for (let i = 0; i < rows * chans; i++) {
      const f = p.u8(), cell = {};
      if (f & 1) cell.note = p.u8();
      if (f & 2) cell.ins = p.u8();
      if (f & 4) cell.vol = [p.u8(), p.u8()];
      if (f & 8) cell.fx = [p.u8(), p.u8()];
      cells.push(f ? cell : null);
    }
    m.patterns.push({ rows, name, cells });
  }
  const s = pending.smpl;
  for (let k = s ? s.u16() : 0; k > 0; k--) {
    const name = s.str(); s.str();
    s.bytes(32);   // the sample's ID: worked out again when written
    const wid = s.bytes(32), frames = s.u32(), bits = s.u8(); s.u8();
    const rate = s.u32(), volume = s.u8(); s.u8();
    const panOn = s.u8(), panValue = s.u8(), kind = LOOP_NAMES[s.u8()] ?? 'off', start = s.u32(), end = s.u32(); s.bytes(13);   // the sustain loop and the auto-vibrato
    let pcm = bits === 8 ? new Int8Array(0) : new Int16Array(0);
    if (!sameId(wid, NONE)) {
      pcm = waves.get(Array.from(wid).join(','));
      if (!pcm) throw new Error(`sample "${name}" has no waveform in the file`);
      if (pcm.length !== frames || !sameId(waveId(pcm), wid)) throw new Error(`sample "${name}" does not match its waveform ID`);
    }
    m.samples.push({ name, pcm, rate, volume, pan: panOn ? panValue : null, loop: kind === 'off' ? null : { kind, start, end }, source: null, fm: null });
  }
  const i = pending.inst;
  const named = [];
  for (let k = i ? i.u16() : 0; k > 0; k--) {
    i.bytes(32); named.push({ name: i.str() }); i.str(); i.bytes(120 * 3);
    for (let e = 0; e < 3; e++) { i.u8(); i.bytes(i.u8() * 3 + 4); }   // an envelope: flags, points, then the loop and sustain
    i.bytes(4 + 1 + 2 + 7 + 4);   // fadeout, global volume, panning, the actions and variation, the filter
  }
  m.instruments = cmpr?.instruments ?? named;
  for (let k = synv ? synv.u16() : 0; k > 0; k--) {
    const at = synv.u16(), kind = synv.u8();
    if (kind !== 1) throw new Error(`a synth sound of kind ${kind} (OctaMED, JamCracker) cannot be read here`);
    if (!m.samples[at]) throw new Error(`FM patch for missing sample slot ${at + 1}`);
    m.samples[at].fm = [...synv.bytes(11)];
  }
  for (let k = efct ? efct.u16() : 0; k > 0; k--) {
    const kind = EFFECT_KINDS[efct.u8()] ?? 'reverb', name = efct.str(), mix = efct.f32(), bypass = efct.u8() !== 0, out = efct.u8(), n = efct.u8();
    const params = Array.from({ length: n }, () => efct.f32());
    m.effects.push({ kind, name, mix, bypass, output: out === 255 ? null : out, params: Object.keys(EFFECTS[kind]).map((_, i) => params[i] ?? Object.values(EFFECTS[kind])[i]) });
  }
  if (insx && !cmpr) {   // a file that is not Composer's: take the MIDI output and effect slot of each named instrument
    for (let k = insx.u16(), i = 0; k > 0; k--, i++) {
      insx.bytes(3);
      const midi = insx.u8() ? { channel: insx.u8() & 15, program: insx.u8(), bank: insx.u16() } : null, effect = insx.u8(); if (insx.u8()) break;   // a phrase: not read here
      if (named[i]) { if (midi) named[i].midi = { channel: midi.channel, program: midi.program === 255 ? null : midi.program & 127, bank: midi.bank === 65535 ? null : midi.bank }; if (effect !== 255) named[i].effect = effect; }
    }
  }
  return m;
}
