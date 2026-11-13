// The tracker side of the engine, as Score is the MIDI side: a Module holds patterns of rows x channels, an order list and samples,
// the model an .ntt (Neumetik's UMF with its samples) is written from. Everything is an integer, as in a Score; sample points are
// Int8Array or Int16Array. Field names and ranges follow Neumetik's UMF layout (tracker/docs/umf-format.md).
export const NOTE_FADE = 253, NOTE_CUT = 254, NOTE_OFF = 255;
export const ORDER_SKIP = 0xfffe, ORDER_END = 0xffff;   // in an order list: skip this slot, or end the song here
export const CHANNEL_MUTED = 2;                        // bit of a channel's flags
export const KINDS = { mod: 0, s3m: 1, xm: 2, it: 3 };   // whose effect rules the song plays by
export const LOOPS = { off: 0, forward: 1, pingpong: 2 };
// Effect slots, in the order a UMF numbers their kinds; each parameter is 0..1 with the default Neumetik's slot starts at.
export const EFFECTS = {
  reverb: { roomSize: 0.6, damping: 0.4, width: 1, preDelay: 0.1 },
  delay: { leftTime: 0.3, rightTime: 0.4, feedback: 0.4, pingPong: 0, damping: 0.3 },
  chorus: { rate: 0.2, depth: 0.5, delay: 0.4, feedback: 0.1 },
  flanger: { rate: 0.15, depth: 0.7, delay: 0.1, feedback: 0.6 },
  distortion: { drive: 0.5, tone: 0.6, level: 0.6 },
  compressor: { threshold: 0.6, ratio: 0.3, attack: 0.1, release: 0.3, makeup: 0.2 },
  equalizer: { low: 0.5, mid: 0.5, midFrequency: 0.5, high: 0.5 },
  filter: { cutoff: 0.7, resonance: 0.2, highPass: 0 },
  gargle: { rate: 0.3, square: 0 },
};
export const EFFECT_KINDS = Object.keys(EFFECTS);

const whole = (v, lo, hi, name) => { if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} ${v} is outside ${lo}..${hi}`); return v; };

export class Module {
  /** `channels` tracks; `kind` is a key of KINDS; `speed` ticks a row and `tempo` beats a minute, as a tracker has them. */
  constructor({ title = '', artist = '', message = '', kind = 'it', speed = 6, tempo = 125, channels = 8 } = {}) {
    if (!(kind in KINDS)) throw new Error(`unknown kind "${kind}"; known: ${Object.keys(KINDS).join(', ')}`);
    whole(speed, 1, 255, 'speed'); whole(tempo, 32, 255, 'tempo'); whole(channels, 1, 64, 'channels');
    Object.assign(this, { title, artist, message, kind, speed, tempo, restart: 0, time: { rowsPerBeat: 4, rowsPerMeasure: 16 } });
    this.channels = Array.from({ length: channels }, (_, i) => ({ pan: i % 2 ? 192 : 64, vol: 64, flags: 0, name: '' }));
    this.patterns = []; this.orders = []; this.samples = []; this.instruments = []; this.effects = [];
    // More named order lists beside `orders` (the song as played): the sections a game can switch between. Each `{ name, restart, orders }`.
    this.lists = [];
    // Composer's own notes for a host, kept in the file as JSON: the layers and the sections of a cue. Null when there are none.
    this.meta = null;
  }
  /** A new empty pattern; its index. */
  addPattern(rows = 64, name = '') {
    whole(rows, 1, 256, 'rows');
    this.patterns.push({ rows, name, cells: new Array(rows * this.channels.length).fill(null) });
    return this.patterns.length - 1;
  }
  /** Put pattern `p` next in the song's order. */
  order(p) { if (!this.patterns[p]) throw new Error(`no pattern ${p}`); this.orders.push(p); return this; }
  /** End the song here, so it plays once instead of restarting. */
  end() { this.orders.push(ORDER_END); return this; }
  /** A named order list (a section a host can switch to). `restart` is the order it loops back to. */
  addList(name, orders, restart = 0) {
    this.lists.push({ name, restart, orders: [...orders] });
    return this.lists.length - 1;
  }
  /** Mute or unmute the channels in `which` (indexes); a muted channel is a layer a host has not faded in yet. */
  mute(which, muted = true) {
    for (const c of which) { whole(c, 0, this.channels.length - 1, 'channel'); this.channels[c].flags = muted ? this.channels[c].flags | CHANNEL_MUTED : this.channels[c].flags & ~CHANNEL_MUTED; }
    return this;
  }
  #at(p, row, ch) {
    const pat = this.patterns[p];
    if (!pat) throw new Error(`no pattern ${p}`);
    whole(row, 0, pat.rows - 1, 'row'); whole(ch, 0, this.channels.length - 1, 'channel');
    return row * this.channels.length + ch;
  }
  cell(p, row, ch) { return this.patterns[p]?.cells[this.#at(p, row, ch)] ?? null; }
  /** A cell: `{ note, ins, vol: [kind, value], fx: [effect, parameter] }`, every part optional. Note 1..120 (C-5 is 61) or NOTE_FADE/CUT/OFF. */
  set(p, row, ch, cell) {
    const c = {};
    if (cell.note !== undefined) { if (![NOTE_FADE, NOTE_CUT, NOTE_OFF].includes(cell.note)) whole(cell.note, 1, 120, 'note'); c.note = cell.note; }
    if (cell.ins !== undefined) c.ins = whole(cell.ins, 1, 255, 'instrument');
    if (cell.vol) c.vol = [whole(cell.vol[0], 1, 14, 'volume column kind'), whole(cell.vol[1], 0, 255, 'volume column value')];
    if (cell.fx) c.fx = [whole(cell.fx[0], 1, 43, 'effect'), whole(cell.fx[1], 0, 255, 'effect parameter')];
    const at = this.#at(p, row, ch);
    this.patterns[p].cells[at] = Object.keys(c).length ? c : null;
    return this;
  }
  /** A sample slot (1-based, as cells name it). `pcm` is an Int8Array or Int16Array; `rate` is its C-5 rate in hertz. */
  addSample({ name = '', pcm, rate = 22050, volume = 64, pan = null, loop = null }) {
    if (!(pcm instanceof Int8Array || pcm instanceof Int16Array)) throw new Error('sample points must be an Int8Array or Int16Array');
    whole(rate, 1000, 192000, 'rate'); whole(volume, 0, 64, 'volume');
    if (pan !== null) whole(pan, 0, 64, 'pan');
    if (loop) { whole(loop.start, 0, pcm.length - 1, 'loop start'); whole(loop.end, loop.start + 1, pcm.length, 'loop end'); if (!(loop.kind in LOOPS)) throw new Error(`unknown loop "${loop.kind}"`); }
    this.samples.push({ name, pcm, rate, volume, pan, loop, source: null, fm: null });
    return this.samples.length;
  }
  /** A sample slot that plays a two-operator FM patch (`fmPatch` / `oplPatch`: eleven bytes) instead of points. */
  addFm(name, patch) {
    if (!Array.isArray(patch) && !(patch instanceof Uint8Array) || patch.length !== 11 || [...patch].some((b) => !Number.isInteger(b) || b < 0 || b > 255)) throw new Error('an FM patch is eleven bytes');
    this.samples.push({ name, pcm: new Int8Array(0), rate: 8363, volume: 64, pan: null, loop: null, source: null, fm: [...patch] });
    return this.samples.length;
  }
  /**
   * An effect slot (kind is a key of EFFECTS); `params` by name, the rest at their defaults. `output` chains into a later slot.
   * Returns its index, which an instrument's `effect` names.
   */
  addEffect(kind, { name = '', mix = null, bypass = false, output = null, params = {} } = {}) {
    if (!(kind in EFFECTS)) throw new Error(`unknown effect "${kind}"; known: ${EFFECT_KINDS.join(', ')}`);
    const given = { ...EFFECTS[kind], ...params };
    for (const k of Object.keys(params)) if (!(k in EFFECTS[kind])) throw new Error(`${kind} has no parameter "${k}"; known: ${Object.keys(EFFECTS[kind]).join(', ')}`);
    const values = Object.values(given).map((v, i) => { if (!(v >= 0 && v <= 1)) throw new Error(`${kind} parameter ${Object.keys(given)[i]} ${v} is outside 0..1`); return v; });
    const m = mix ?? (['reverb', 'delay', 'chorus'].includes(kind) ? 0.35 : 1);
    if (!(m >= 0 && m <= 1)) throw new Error(`mix ${m} is outside 0..1`);
    this.effects.push({ kind, name, mix: Math.fround(m), bypass, output, params: values.map(Math.fround) });
    return this.effects.length - 1;
  }
  /** Throws if the song names a pattern, sample or instrument that is not there, or a cell is outside its pattern. */
  check() {
    if (!this.orders.length) throw new Error('the song has no orders');
    const isOrder = (o) => this.patterns[o] || o === ORDER_END || o === ORDER_SKIP;
    for (const o of this.orders) if (!isOrder(o)) throw new Error(`order names missing pattern ${o}`);
    for (const l of this.lists) {
      if (!l.orders.length) throw new Error(`order list "${l.name}" is empty`);
      for (const o of l.orders) if (!isOrder(o)) throw new Error(`order list "${l.name}" names missing pattern ${o}`);
      whole(l.restart, 0, l.orders.length - 1, `restart of "${l.name}"`);
    }
    this.effects.forEach((e, i) => { if (e.output !== null && !(e.output > i && e.output < this.effects.length)) throw new Error(`effect ${i} goes into slot ${e.output}; a chain only goes to a later slot`); });
    this.instruments.forEach((x, i) => {
      if (x.effect != null && !this.effects[x.effect]) throw new Error(`instrument ${i + 1} uses missing effect slot ${x.effect}`);
      if (x.midi) { whole(x.midi.channel, 0, 15, 'MIDI channel'); if (x.midi.program != null) whole(x.midi.program, 0, 127, 'MIDI program'); if (x.midi.bank != null) whole(x.midi.bank, 0, 16383, 'MIDI bank'); }
    });
    const slots = this.instruments.length || this.samples.length;
    this.patterns.forEach((pat, i) => {
      if (pat.cells.length !== pat.rows * this.channels.length) throw new Error(`pattern ${i} has ${pat.cells.length} cells, expected ${pat.rows * this.channels.length}`);
      for (const c of pat.cells) if (c?.ins > slots) throw new Error(`pattern ${i} plays missing ${this.instruments.length ? 'instrument' : 'sample'} ${c.ins}`);
    });
    return this;
  }
  clone() {
    const m = new Module({ title: this.title, artist: this.artist, message: this.message, kind: this.kind, speed: this.speed, tempo: this.tempo, channels: this.channels.length });
    m.restart = this.restart; m.time = { ...this.time };
    m.channels = this.channels.map((c) => ({ ...c }));
    m.patterns = this.patterns.map((p) => ({ ...p, cells: p.cells.map((c) => c && { ...c, vol: c.vol && [...c.vol], fx: c.fx && [...c.fx] }) }));
    m.orders = [...this.orders];
    m.lists = this.lists.map((l) => ({ ...l, orders: [...l.orders] }));
    m.meta = this.meta && structuredClone(this.meta);
    m.samples = this.samples.map((s) => ({ ...s, pcm: s.pcm.slice(), loop: s.loop && { ...s.loop }, fm: s.fm && [...s.fm] }));
    m.instruments = this.instruments.map((i) => structuredClone(i));
    m.effects = this.effects.map((e) => ({ ...e, params: [...e.params] }));
    return m;
  }
  equals(o) {
    const plain = (m) => JSON.stringify({ ...m, samples: m.samples.map((s) => ({ ...s, pcm: [s.pcm.BYTES_PER_ELEMENT, ...s.pcm] })) });
    return plain(this) === plain(o);
  }
}
