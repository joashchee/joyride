// Standard MIDI File codec, pure bytes (no Node built-ins), so a page can offer a download. The MIDI is the exact artifact of a
// composition: integers only, so the same recipe gives the same bytes anywhere. `toMidi` writes format 1 (a tempo track, then one
// track per Score track); `parseMidi` reads format 0 and 1; `fromMidi` turns a file back into a Score.
import { Score, PPQ } from './score.js';
import { NOTE_NAMES, tonic } from './scales.js';

const u16 = (v) => [(v >> 8) & 255, v & 255];
const u32 = (v) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];
const ascii = (s) => [...s].map((c) => c.charCodeAt(0) & 127);
function vlq(v) { const b = [v & 127]; while ((v >>= 7)) b.unshift((v & 127) | 128); return b; }
const meta = (type, data) => [0xff, type, ...vlq(data.length), ...data];
const chunk = (id, body) => [...ascii(id), ...u32(body.length), ...body];

// Key signature: sharps (+) or flats (-) of the major key with this tonic pitch class.
const SF = [0, -5, 2, -3, 4, -1, 6, 1, -4, 3, -2, 5];
function keySignature(key, scale) {
  const minor = scale.includes('minor') || ['minor', 'dorian', 'phrygian', 'harmonic', 'blues'].includes(scale);
  return [SF[(tonic(key) + (minor ? 3 : 0)) % 12] & 255, minor ? 1 : 0];
}

export function toMidi(score) {
  const ppq = PPQ, tempo = Math.round(60e6 / score.bpm);
  const head = [...vlq(0), ...meta(0x03, ascii(score.title || 'composer')),
    ...vlq(0), ...meta(0x51, [(tempo >> 16) & 255, (tempo >> 8) & 255, tempo & 255]),
    ...vlq(0), ...meta(0x58, [score.beats, 2, 24, 8]),
    ...vlq(0), ...meta(0x59, keySignature(score.key, score.scale)),
    ...vlq(0), ...meta(0x2f, [])];
  const tracks = [chunk('MTrk', head)];
  let ch = 0;
  for (const t of score.tracks.values()) {
    let channel = 9;
    if (!t.drum) { if (ch === 9) ch++; if (ch > 15) throw new Error('MIDI has 15 channels for pitched tracks; this score has more'); channel = ch++; }
    const ev = [];   // [tick, order, bytes]; note-offs sort before note-ons at the same tick
    if (!t.drum) ev.push([0, -1, [0xc0 | channel, t.program & 127]]);
    for (const n of t.notes) { ev.push([n.t, 1, [0x90 | channel, n.p, n.v]], [n.t + n.d, 0, [0x80 | channel, n.p, 0]]); }
    ev.sort((a, b) => a[0] - b[0] || a[1] - b[1] || a[2][1] - b[2][1]);
    const body = [...vlq(0), ...meta(0x03, ascii(t.name))];
    let last = 0;
    for (const [tick, , bytes] of ev) { body.push(...vlq(tick - last), ...bytes); last = tick; }
    body.push(...vlq(Math.max(0, score.length - last)), ...meta(0x2f, []));
    tracks.push(chunk('MTrk', body));
  }
  return Uint8Array.from([...chunk('MThd', [...u16(1), ...u16(tracks.length), ...u16(ppq)]), ...tracks.flat()]);
}

/** `{ format, ppq, tracks: [{ name, events: [{ tick, type, ... }] }] }`; events are note-on/off, program, tempo, timeSig, keySig. */
export function parseMidi(bytes) {
  let i = 0;
  const need = (n) => { if (i + n > bytes.length) throw new Error('MIDI file ends early'); };
  const r8 = () => { need(1); return bytes[i++]; };
  const r16 = () => (r8() << 8) | r8();
  const r32 = () => ((r8() << 24) | (r8() << 16) | (r8() << 8) | r8()) >>> 0;
  const rv = () => { let v = 0, b; do { b = r8(); v = (v << 7) | (b & 127); } while (b & 128); return v; };
  const tag = () => String.fromCharCode(r8(), r8(), r8(), r8());
  if (tag() !== 'MThd') throw new Error('not a MIDI file');
  const hl = r32(), format = r16(), count = r16(), division = r16();
  if (division & 0x8000) throw new Error('SMPTE time division is not supported');
  i += hl - 6;
  const tracks = [];
  for (let k = 0; k < count; k++) {
    if (tag() !== 'MTrk') throw new Error('expected a track chunk');
    const len = r32(), end = i + len;
    need(len);
    const events = [];
    let tick = 0, status = 0, name = '';
    while (i < end) {
      tick += rv();
      let b = r8();
      if (b === 0xff) {
        const type = r8(), l = rv(), d = Array.from(bytes.slice(i, i + l)); i += l;
        if (type === 0x03) name = String.fromCharCode(...d);
        else if (type === 0x51) events.push({ tick, type: 'tempo', us: (d[0] << 16) | (d[1] << 8) | d[2] });
        else if (type === 0x58) events.push({ tick, type: 'timeSig', num: d[0], den: 2 ** d[1] });
        else if (type === 0x59) events.push({ tick, type: 'keySig', sf: (d[0] << 24) >> 24, minor: d[1] === 1 });
        continue;
      }
      if (b === 0xf0 || b === 0xf7) { i += rv(); continue; }
      if (b & 128) { status = b; b = r8(); }
      const kind = status >> 4, channel = status & 15;
      if (kind === 0xc) events.push({ tick, type: 'program', channel, program: b });
      else if (kind === 0xd) { /* channel pressure: one data byte, ignored */ }
      else {
        const v = r8();
        if (kind === 0x9 && v > 0) events.push({ tick, type: 'on', channel, p: b, v });
        else if (kind === 0x8 || kind === 0x9) events.push({ tick, type: 'off', channel, p: b });
      }
    }
    i = end;
    tracks.push({ name, events });
  }
  return { format, ppq: division, tracks };
}

// Major tonic pitch class for a count of sharps, and the relative minor's.
const MAJOR_PC = (sf) => (((sf * 7) % 12) + 12) % 12;
/** A Score from MIDI bytes: ticks are rescaled to this engine's resolution, bars are rounded up. */
export function fromMidi(bytes) {
  const m = parseMidi(bytes), k = PPQ / m.ppq;
  const all = m.tracks.flatMap((t) => t.events);
  const tempo = all.find((e) => e.type === 'tempo'), sig = all.find((e) => e.type === 'timeSig'), ks = all.find((e) => e.type === 'keySig');
  const beats = sig ? sig.num : 4;
  const pc = ks ? (MAJOR_PC(ks.sf) + (ks.minor ? 9 : 0)) % 12 : 0;
  const score = new Score({ bpm: tempo ? Math.round((60e6 / tempo.us) * 1000) / 1000 : 120, bars: 1, beats, key: NOTE_NAMES[pc], scale: ks?.minor ? 'minor' : 'major' });
  let end = 0;
  for (const t of m.tracks) {
    const open = new Map(), program = new Map();
    for (const e of t.events) {
      if (e.type === 'program') program.set(e.channel, e.program);
      if (e.type === 'on') { const q = open.get(`${e.channel}:${e.p}`) ?? []; q.push(e); open.set(`${e.channel}:${e.p}`, q); }
      if (e.type === 'off') {
        const on = open.get(`${e.channel}:${e.p}`)?.shift();
        if (!on) continue;
        const drum = on.channel === 9, name = t.name || (drum ? 'drums' : `track ${m.tracks.indexOf(t)}`);
        const tr = score.track(name, { drum, program: program.get(on.channel) ?? 0 });
        const st = Math.round(on.tick * k), d = Math.max(1, Math.round((e.tick - on.tick) * k));
        tr.notes.push({ p: on.p, t: st, d, v: on.v });
        end = Math.max(end, st + d);
      }
    }
  }
  score.bars = Math.max(1, Math.ceil(end / score.barTicks));
  return score;
}
