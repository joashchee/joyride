// Neumetik's NLZ1 wrapper: "NLZ1", a method byte (0 stored, 1 coded), the length unpacked (u64, little-endian), then the bytes. The .ntt
// chunks Neumetik packs (PATT, COVR) and a waveform file's body may be stored, which every reader must accept, so Composer writes
// method 0 and needs no compressor. Reading a coded stream (method 1) needs Neumetik's range decoder and is refused with a clear error.
import { ByteWriter, ByteReader } from './bytes.js';

export function nlzStore(bytes) { return new ByteWriter().ascii('NLZ1').u8(0).u64(bytes.length).bytes(bytes).done(); }

export function nlzUnstore(bytes) {
  const r = new ByteReader(bytes);
  if (r.ascii(4) !== 'NLZ1') throw new Error('not an NLZ1 stream');
  const method = r.u8(), n = r.u64();
  if (method === 1) throw new Error('this NLZ1 stream is coded; only stored streams (method 0) can be read here');
  if (method !== 0) throw new Error(`unknown NLZ1 method ${method}`);
  if (r.left !== n) throw new Error(`NLZ1 says ${n} bytes, the stream has ${r.left}`);
  return r.bytes(n);
}
