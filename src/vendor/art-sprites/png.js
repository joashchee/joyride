// Minimal PNG reader and writer (8-bit, non-interlaced) on node:zlib, so the sprite tools need no dependencies.
import { deflateSync, inflateSync } from 'node:zlib';

const CRC = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc32 = (b) => { let c = 0xffffffff; for (const x of b) c = CRC[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (type, data) => {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0); out.write(type, 4, 'latin1'); Buffer.from(data).copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
};
const SIG = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

/** RGBA bytes (w*h*4) to a PNG file. */
export function encodePng(w, h, rgba) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) Buffer.from(rgba.buffer, rgba.byteOffset + y * w * 4, w * 4).copy(raw, y * (w * 4 + 1) + 1);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([SIG, chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

/** A canvas as a PNG file (Node only: the browser has no node:zlib, and draws through a canvas instead). */
export const pixelsToPng = (px, scale = 1, bg = null) => encodePng(px.w * scale, px.h * scale, px.toRgba(scale, bg));

/** Header only: the size check an ingest needs before it trusts a file. */
export function pngSize(buf) {
  if (buf.length < 24 || !buf.subarray(0, 8).equals(SIG)) throw new Error('not a PNG');
  return { w: buf.readUInt32BE(16), h: buf.readUInt32BE(20) };
}

/** A PNG file (8-bit grey, grey+alpha, RGB, RGBA; 1, 2, 4 or 8-bit palette or grey) to RGBA bytes. */
export function decodePng(buf) {
  const { w, h } = pngSize(buf);
  if (w * h > 4096 * 4096) throw new Error('image too large');
  let pos = 8, depth = 0, type = 0, palette = null, trns = null; const idat = [];
  if (buf.readUInt8(28)) throw new Error('interlaced PNG not supported');
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos), name = buf.toString('latin1', pos + 4, pos + 8), data = buf.subarray(pos + 8, pos + 8 + len);
    if (name === 'IHDR') { depth = data[8]; type = data[9]; }
    else if (name === 'PLTE') palette = data;
    else if (name === 'tRNS') trns = data;
    else if (name === 'IDAT') idat.push(data);
    pos += 12 + len;
  }
  if (depth !== 8 && !((type === 0 || type === 3) && [1, 2, 4].includes(depth))) throw new Error(`bit depth ${depth} not supported for colour type ${type}`);
  const ch = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[type];
  if (!ch) throw new Error(`colour type ${type} not supported`);
  const bits = depth * ch, stride = Math.ceil((w * bits) / 8), bpp = Math.max(1, bits >> 3);
  const raw = inflateSync(Buffer.concat(idat)), px = Buffer.alloc(stride * h);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)];
    for (let i = 0; i < stride; i++) {
      const x = raw[y * (stride + 1) + 1 + i], a = i >= bpp ? px[y * stride + i - bpp] : 0, b = y ? px[(y - 1) * stride + i] : 0, c = y && i >= bpp ? px[(y - 1) * stride + i - bpp] : 0;
      const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      px[y * stride + i] = (x + (f === 0 ? 0 : f === 1 ? a : f === 2 ? b : f === 3 ? (a + b) >> 1 : pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 255;
    }
  }
  const out = new Uint8Array(w * h * 4), max = (1 << depth) - 1;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = y * w + x, s = y * stride + (depth < 8 ? 0 : x * ch);
    const sample = depth < 8 ? (px[y * stride + ((x * depth) >> 3)] >> (8 - depth - ((x * depth) & 7))) & max : px[s];
    const [r, g, b, a] = type === 6 ? [px[s], px[s + 1], px[s + 2], px[s + 3]] : type === 2 ? [px[s], px[s + 1], px[s + 2], 255]
      : type === 0 ? [(sample * 255) / max, (sample * 255) / max, (sample * 255) / max, 255] : type === 4 ? [px[s], px[s], px[s], px[s + 1]]
      : [palette[sample * 3], palette[sample * 3 + 1], palette[sample * 3 + 2], trns && sample < trns.length ? trns[sample] : 255];
    out.set([r, g, b, a], i * 4);
  }
  return { w, h, rgba: out };
}
