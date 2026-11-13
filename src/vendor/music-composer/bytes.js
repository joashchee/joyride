// Little-endian byte writer and reader for the tracker side (.ntt), pure Uint8Array, no Node built-ins. midi.js keeps its own
// big-endian helpers: MIDI bytes are the golden artifact and are not touched. Strings are a u16 length, then UTF-8, as Neumetik's UMF has them.
const enc = new TextEncoder(), dec = new TextDecoder('utf-8', { fatal: true });
const whole = (v, lo, hi, name) => { if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} ${v} is outside ${lo}..${hi}`); return v; };

export class ByteWriter {
  constructor() { this.buf = new Uint8Array(256); this.length = 0; }
  #room(n) {
    if (this.length + n <= this.buf.length) return;
    const grown = new Uint8Array(Math.max(this.buf.length * 2, this.length + n));
    grown.set(this.buf.subarray(0, this.length)); this.buf = grown;
  }
  u8(v) { whole(v, 0, 255, 'u8'); this.#room(1); this.buf[this.length++] = v; return this; }
  i8(v) { whole(v, -128, 127, 'i8'); return this.u8(v & 255); }
  u16(v) { whole(v, 0, 65535, 'u16'); return this.u8(v & 255).u8(v >> 8); }
  u32(v) { whole(v, 0, 4294967295, 'u32'); return this.u16(v & 65535).u16(v >>> 16); }
  u64(v) { whole(v, 0, Number.MAX_SAFE_INTEGER, 'u64'); return this.u32(v % 4294967296).u32(Math.floor(v / 4294967296)); }
  f32(v) { this.#room(4); new DataView(this.buf.buffer).setFloat32(this.length, v, true); this.length += 4; return this; }
  bytes(b) { this.#room(b.length); this.buf.set(b, this.length); this.length += b.length; return this; }
  ascii(s) { for (const c of s) this.u8(c.charCodeAt(0) & 127); return this; }
  /** A u16 length, then UTF-8, cut on a character boundary at 65535 bytes. */
  str(s) {
    let b = enc.encode(s);
    if (b.length > 65535) { let n = 65535; while (n > 0 && (b[n] & 0xc0) === 0x80) n--; b = b.subarray(0, n); }
    return this.u16(b.length).bytes(b);
  }
  /** A chunk: 4-byte id, u32 length, the body made by `fill(writer)`. */
  chunk(id, fill) { const w = new ByteWriter(); fill(w); return this.ascii(id).u32(w.length).bytes(w.done()); }
  done() { return this.buf.slice(0, this.length); }
}

export class ByteReader {
  constructor(bytes, at = 0) { this.b = bytes; this.i = at; this.v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength); }
  get left() { return this.b.length - this.i; }
  #need(n) { if (n < 0 || this.i + n > this.b.length) throw new Error('file ends early'); }
  u8() { this.#need(1); return this.b[this.i++]; }
  i8() { return (this.u8() << 24) >> 24; }
  u16() { this.#need(2); const x = this.v.getUint16(this.i, true); this.i += 2; return x; }
  u32() { this.#need(4); const x = this.v.getUint32(this.i, true); this.i += 4; return x; }
  u64() { const lo = this.u32(), hi = this.u32(); if (hi > 0x1fffff) throw new Error('length is out of range'); return hi * 4294967296 + lo; }
  f32() { this.#need(4); const x = this.v.getFloat32(this.i, true); this.i += 4; return x; }
  bytes(n) { this.#need(n); const x = this.b.subarray(this.i, this.i + n); this.i += n; return x; }
  ascii(n) { return String.fromCharCode(...this.bytes(n)); }
  str() { return dec.decode(this.bytes(this.u16())); }
  /** The next chunk as `{ id, body }` (a reader over its bytes); `null` at the end of the data. */
  chunk() { if (this.left === 0) return null; const id = this.ascii(4), n = this.u32(); return { id, body: new ByteReader(this.bytes(n)) }; }
}
