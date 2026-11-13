// Text notation: a Score as plain text you can write by hand, diff and keep in a repo, and read back exactly. `toNotation` and
// `parseNotation` are inverses (every integer of the Score survives), so a hand-written tune and a generated one are the same kind of
// thing. No Node built-ins: it runs in a page.
//
//   title: Night walk
//   bpm: 96
//   meter: 4                       beats (quarter notes) a bar
//   key: A minor
//   key@1152: C major              a modulation, at tick 1152
//   bars: 2
//   track lead program=80 wave=square
//   | A4:q C5 E5 A5:h | G5:w |      a voice: bars between |; a note is a pitch, then :duration; a duration sticks until changed
//   track chords
//   | [A3 C4 E4]:w | [G3 B3 D4]:w |
//   track drums drum
//   | kick:q hat snare hat | kick:q hat snare:q hat |
//
// Pitches are names (C4 = 60, F#3, Bb2) or `n60`; on a drum track the General MIDI names also work (kick, snare, hat...). A chord is
// `[C4 E4 G4]`. `r` is a rest. Durations: w h q e s t (whole to 32nd), `.` and `..` dot them, or a whole number of ticks (96 to the
// quarter). `!80` sets the velocity from there on (default 96). `~` after a note ties it to the same pitch that follows. Several
// voice lines under one track play together, which is how a track holds notes that overlap. Every bar must add up to the meter.
import { Score, PPQ } from './score.js';
import { pitch, pitchName, DRUM } from './scales.js';

const LETTERS = { w: 4 * PPQ, h: 2 * PPQ, q: PPQ, e: PPQ / 2, s: PPQ / 4, t: PPQ / 8 };
const DRUM_NAMES = Object.fromEntries(Object.entries(DRUM).map(([k, v]) => [v, k]));
const DEFAULT_VEL = 96;
const HEADER = /^(title|bpm|meter|key|key@\d+|bars):\s*(.*)$/;

const pitchOf = (name, drum, fail) => {
  if (drum && name in DRUM) return DRUM[name];
  if (/^n\d+$/.test(name)) return Number(name.slice(1));
  try { return pitch(name); } catch { return fail(`unknown note "${name}"`); }
};
const dur = (ticks) => {
  for (const [c, d] of Object.entries(LETTERS)) {
    if (ticks === d) return c;
    if (ticks === d * 1.5) return `${c}.`;
    if (ticks === d * 1.75) return `${c}..`;
  }
  return String(ticks);
};
const parseDur = (s, fail) => {
  const m = /^([whqest])(\.{0,2})$/.exec(s);
  if (m) return LETTERS[m[1]] * [1, 1.5, 1.75][m[2].length];
  if (/^\d+$/.test(s) && Number(s) > 0) return Number(s);
  return fail(`unknown duration "${s}"; use w h q e s t (dotted with .) or a number of ticks`);
};

// ---- Score to text ------------------------------------------------------------------------------------------------------------------

const pitchText = (p, drum) => (drum ? DRUM_NAMES[p] ?? `n${p}` : pitchName(p));

// The notes of a track as voices: chords (notes sharing a start and a length) laid in the first voice that is free when they begin.
function voices(notes) {
  const chords = new Map();
  for (const n of [...notes].sort((a, b) => a.t - b.t || a.d - b.d || a.p - b.p)) {
    const k = `${n.t}:${n.d}`;
    if (!chords.has(k)) chords.set(k, { t: n.t, d: n.d, notes: [] });
    chords.get(k).notes.push(n);
  }
  const out = [];
  for (const c of chords.values()) {
    let v = out.find((x) => x.end <= c.t);
    if (!v) out.push(v = { end: 0, chords: [] });
    v.chords.push(c); v.end = c.t + c.d;
  }
  return out;
}

function voiceText(v, barTicks, drum) {
  const bars = [], bar = (t) => { const i = Math.floor(t / barTicks); while (bars.length <= i) bars.push([]); return bars[i]; };
  let curDur = PPQ, curVel = DEFAULT_VEL, pos = 0;
  // A piece of an event inside one bar. `tie` says it carries on into the next bar.
  const piece = (head, from, to, tie) => {
    const d = to - from;
    bar(from).push(head + (d !== curDur ? `:${dur(d)}` : '') + (tie ? '~' : ''));
    curDur = d;
  };
  const span = (from, to, fn) => { for (let t = from; t < to;) { const end = Math.min(to, (Math.floor(t / barTicks) + 1) * barTicks); fn(t, end, end < to); t = end; } };
  for (const c of v.chords) {
    span(pos, c.t, (a, b) => piece('r', a, b, false));
    const head = c.notes.map((n) => { const vel = n.v !== curVel ? `!${n.v}` : ''; curVel = n.v; return pitchText(n.p, drum) + vel; });
    // Velocity is sticky, so a tied piece repeats nothing: the notes of a later piece already carry the same velocity.
    const text = c.notes.length > 1 ? `[${head.join(' ')}]` : head[0];
    const plain = c.notes.length > 1 ? `[${c.notes.map((n) => pitchText(n.p, drum)).join(' ')}]` : pitchText(c.notes[0].p, drum);
    let first = true;
    span(c.t, c.t + c.d, (a, b, tie) => { piece(first ? text : plain, a, b, tie); first = false; });
    pos = c.t + c.d;
  }
  // Pad the last bar with a rest so every bar in the text is complete.
  const endBar = Math.ceil(pos / barTicks) * barTicks;
  span(pos, endBar, (a, b) => piece('r', a, b, false));
  return `| ${bars.map((b) => b.join(' ')).join(' | ')} |`;
}

/** The notation of a Score, as text that `parseNotation` reads back into the same notes. */
export function toNotation(score) {
  const out = [];
  if (score.title) out.push(`title: ${score.title}`);
  out.push(`bpm: ${score.bpm}`, `meter: ${score.beats}`, `key: ${score.key} ${score.scale}`);
  for (const c of score.keyChanges) out.push(`key@${c.t}: ${c.key} ${c.scale}`);
  out.push(`bars: ${score.bars}`);
  for (const t of score.tracks.values()) {
    if (/\s/.test(t.name)) throw new Error(`track name "${t.name}" has a space; notation names are one word`);
    const opts = [t.program ? `program=${t.program}` : '', t.wave !== 'triangle' ? `wave=${t.wave}` : '', t.drum ? 'drum' : '', t.gain !== 1 ? `gain=${t.gain}` : '', t.env ? `env=${t.env.join(',')}` : ''].filter(Boolean);
    out.push('', `track ${t.name}${opts.length ? ` ${opts.join(' ')}` : ''}`);
    for (const v of voices(t.notes)) out.push(voiceText(v, score.barTicks, t.drum));
  }
  return out.join('\n') + '\n';
}

// ---- Text to Score ------------------------------------------------------------------------------------------------------------------

/** Read notation into a Score. Throws with the line number for anything it cannot place, or a bar that does not add up. */
export function parseNotation(text) {
  const head = { title: '', bpm: 120, meter: 4, key: 'C major', bars: 0 }, keyLines = [], tracks = [];
  let line = 0;
  const fail = (msg) => { throw new Error(`line ${line}: ${msg}`); };
  for (const raw of String(text).split(/\r?\n/)) {
    line++;
    const s = raw.replace(/\s+#.*$/, '').trim();
    if (!s || s.startsWith('#')) continue;
    const h = HEADER.exec(s);
    if (h && !tracks.length) {
      const [, k, v] = h;
      if (k === 'title') head.title = v;
      else if (k.startsWith('key@')) keyLines.push({ t: Number(k.slice(4)), v, line });
      else if (k === 'key') head.key = v;
      else head[k] = Number(v);
      if (k !== 'title' && !k.startsWith('key') && !(head[k] >= 0)) fail(`${k} must be a number`);
      continue;
    }
    if (s.startsWith('track ')) {
      const [, name, ...opts] = s.split(/\s+/), o = {};
      if (!name) fail('a track needs a name');
      for (const x of opts) {
        if (x === 'drum') o.drum = true;
        else if (/^program=\d+$/.test(x)) o.program = Number(x.slice(8));
        else if (/^gain=[\d.]+$/.test(x)) o.gain = Number(x.slice(5));
        else if (/^wave=\w+$/.test(x)) o.wave = x.slice(5);
        else if (/^env=[\d.,]+$/.test(x) && x.split(',').length === 4) o.env = x.slice(4).split(',').map(Number);
        else fail(`unknown track option "${x}"; use program=N wave=W drum gain=G env=a,d,s,r`);
      }
      tracks.push({ name, o, voices: [] });
      continue;
    }
    if (s.startsWith('|')) {
      if (!tracks.length) fail('notes need a track line first (track NAME)');
      tracks[tracks.length - 1].voices.push({ s, line });
      continue;
    }
    fail(`cannot read "${s}"; expected a header, a track line or a voice starting with |`);
  }

  const km = (v, at) => { const m = /^([A-G][#b]?)\s+(\S+)$/.exec(v.trim()); if (!m) { line = at; fail(`key must look like "C major", not "${v}"`); } return m; };
  const [, key, scale] = km(head.key, 0);
  const beats = head.meter;
  const notes = [];   // [track index, note]
  let longest = 0;
  tracks.forEach((tr, ti) => {
    for (const voice of tr.voices) {
      line = voice.line;
      let pos = 0, curDur = PPQ, curVel = DEFAULT_VEL, tied = new Map();
      const segs = voice.s.split('|').slice(1), closed = voice.s.endsWith('|');
      if (closed) segs.pop();
      segs.forEach((seg, bi) => {
        for (const tok of seg.match(/\[[^\]]*\]\S*|\S+/g) ?? []) {
          const m = /^(\[[^\]]*\]|[^:~]*?)(?::([^~]*))?(~?)$/.exec(tok);
          if (!m || !m[1]) fail(`cannot read "${tok}"`);
          const [, who, d, tie] = m;
          if (d !== undefined) curDur = parseDur(d, fail);
          const names = who === 'r' ? [] : who.startsWith('[') ? who.slice(1, -1).trim().split(/\s+/) : [who];
          const played = new Map();   // pitch -> its note, for the tie bookkeeping below
          for (const nm of names) {
            const pm = /^([^!]+)(?:!(\d+))?$/.exec(nm);
            if (!pm) fail(`cannot read the note "${nm}"`);
            if (pm[2] !== undefined) curVel = Number(pm[2]);
            if (curVel < 1 || curVel > 127) fail(`velocity ${curVel} is outside 1..127`);
            const p = pitchOf(pm[1], tr.o.drum, fail), before = tied.get(p);
            if (before) { if (before.t + before.d !== pos) fail(`the tied ${pm[1]} does not continue where it ends`); before.d += curDur; played.set(p, before); }
            else { const n = { p, t: pos, d: curDur, v: curVel }; notes.push([ti, n]); played.set(p, n); }
          }
          for (const p of tied.keys()) if (!played.has(p)) fail(`a tie into nothing: ${pitchName(p)} is not played again`);
          tied = tie ? played : new Map();
          pos += curDur;
        }
        if (closed || bi < segs.length - 1) {
          const want = (bi + 1) * beats * PPQ;
          if (pos !== want) fail(`bar ${bi + 1} is ${pos - bi * beats * PPQ} ticks long, the meter ${beats}/4 needs ${beats * PPQ}`);
        }
      });
      if (tied.size) fail('the last note is tied to nothing');
      longest = Math.max(longest, pos);
    }
  });

  const bars = head.bars || Math.max(1, Math.ceil(longest / (beats * PPQ)));
  if (!Number.isInteger(beats) || beats < 1) throw new Error('meter must be a whole number of beats');
  const score = new Score({ bpm: head.bpm, bars, beats, key, scale, title: head.title });
  for (const k of keyLines) { line = k.line; const [, kk, ks] = km(k.v, k.line); score.modulate(k.t, kk, ks); }
  tracks.forEach((tr) => score.track(tr.name, tr.o));
  for (const [ti, n] of notes) score.note(tracks[ti].name, n.p, n.t, n.d, n.v);
  return score;
}
