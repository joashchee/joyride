// 16-bit mono PCM WAV, pure bytes. `samples` is a Float32Array in -1..1.
export function toWav(samples, sampleRate = 22050) {
  const n = samples.length, out = new Uint8Array(44 + n * 2), v = new DataView(out.buffer);
  const str = (o, s) => [...s].forEach((c, i) => out[o + i] = c.charCodeAt(0));
  str(0, 'RIFF'); v.setUint32(4, 36 + n * 2, true); str(8, 'WAVE'); str(12, 'fmt ');
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true); v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true); str(36, 'data'); v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++) v.setInt16(44 + i * 2, Math.round(Math.max(-1, Math.min(1, samples[i])) * 32767), true);
  return out;
}
