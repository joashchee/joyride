// A small preview synth: renders a Score to mono samples so a composition can be heard without a MIDI player or sound bank.
// It is a sketching voice (oscillators and noise drums), not a mixing tool. The MIDI is the exact artifact; these floats come
// from Math.sin and can differ in the last bit between JS engines, so nothing tests them byte for byte.
import { PPQ } from './score.js';
import { hashSeed } from './rng.js';

const TAU = Math.PI * 2;
export const WAVES = ['sine', 'triangle', 'square', 'saw'];
export const OSC = {
  sine: (ph) => Math.sin(TAU * ph),
  triangle: (ph) => 4 * Math.abs(ph - 0.5) - 1,
  square: (ph) => (ph < 0.5 ? 0.6 : -0.6),
  saw: (ph) => 0.8 * (2 * ph - 1),
};
const hz = (p) => 440 * 2 ** ((p - 69) / 12);
export const DEFAULT_ENV = [0.005, 0.08, 0.7, 0.06];   // attack, decay (seconds), sustain (level), release (seconds)

// Noise that depends only on the sample index and a salt, so a drum hit sounds the same every time.
export function noise(i, salt) { let h = (Math.imul(i + 1, 2654435761) ^ salt) >>> 0; h ^= h >>> 15; h = Math.imul(h, 2246822519); h ^= h >>> 13; return (h >>> 0) / 2147483648 - 1; }

// [length seconds, builder(time, i, salt)] per General MIDI percussion note.
const kick = (t) => Math.sin(TAU * (45 * t + 75 * 0.03 * (1 - Math.exp(-t / 0.03)))) * Math.exp(-t / 0.12);
const tom = (f) => (t) => Math.sin(TAU * (f * t + f * 0.4 * 0.04 * (1 - Math.exp(-t / 0.04)))) * Math.exp(-t / 0.15);
export const DRUMS = {
  36: [0.4, kick], 35: [0.4, kick],
  38: [0.25, (t, i, s) => noise(i, s) * Math.exp(-t / 0.05) * 0.7 + Math.sin(TAU * 190 * t) * Math.exp(-t / 0.04) * 0.5],
  37: [0.05, (t, i, s) => noise(i, s) * Math.exp(-t / 0.01) * 0.6],
  39: [0.2, (t, i, s) => noise(i, s) * Math.exp(-t / 0.04) * (0.6 + 0.4 * Math.sin(TAU * 90 * t) ** 2)],
  42: [0.08, (t, i, s) => (noise(i, s) - noise(i - 1, s)) * 0.5 * Math.exp(-t / 0.015)],
  44: [0.08, (t, i, s) => (noise(i, s) - noise(i - 1, s)) * 0.4 * Math.exp(-t / 0.012)],
  46: [0.4, (t, i, s) => (noise(i, s) - noise(i - 1, s)) * 0.45 * Math.exp(-t / 0.12)],
  45: [0.4, tom(90)], 47: [0.4, tom(120)], 48: [0.4, tom(150)], 50: [0.4, tom(180)],
  49: [1.2, (t, i, s) => (noise(i, s) - noise(i - 1, s)) * 0.5 * Math.exp(-t / 0.35)],
  51: [0.6, (t, i, s) => (noise(i, s) - noise(i - 1, s)) * 0.25 * Math.exp(-t / 0.15) + Math.sin(TAU * 5200 * t) * 0.1 * Math.exp(-t / 0.2)],
};

/**
 * Mono samples for the score at `sampleRate`. A pitched note uses its track's `wave` (sine, triangle, square, saw) and `env`
 * [attack, decay, sustain, release]; a drum track uses the percussion table. `tail` seconds are kept after the last bar for the
 * release (with `loop`, the tail wraps onto the start so the loop is seamless). `tracks` renders only those track names, and
 * `normalize: false` skips the peak limit, so stems rendered apart can share one scale (see `renderCue`).
 */
export function render(score, { sampleRate = 22050, tail = 0.5, loop = false, tracks = null, normalize = true } = {}) {
  const secs = (ticks) => (ticks * 60) / (score.bpm * PPQ);
  const bodyLen = Math.ceil(secs(score.length) * sampleRate), total = loop ? bodyLen : bodyLen + Math.ceil(tail * sampleRate);
  const mix = new Float32Array(total);
  const put = (i, v) => { mix[loop ? i % bodyLen : i] += v; };
  for (const t of score.tracks.values()) {
    if (tracks && !tracks.includes(t.name)) continue;
    for (const n of t.notes) {
      const start = Math.round(secs(n.t) * sampleRate), amp = (n.v / 127) * 0.25 * t.gain;
      if (t.drum) {
        const d = DRUMS[n.p] ?? [0.1, (tt, i, s) => noise(i, s) * Math.exp(-tt / 0.03) * 0.5], len = Math.ceil(d[0] * sampleRate), salt = hashSeed(`${n.p}:${n.t}`);
        for (let i = 0; i < len && start + i < (loop ? Infinity : total); i++) put(start + i, d[1](i / sampleRate, i, salt) * amp * 1.6);
        continue;
      }
      const [a, dec, sus, rel] = t.env ?? DEFAULT_ENV, hold = secs(n.d), f = hz(n.p), wave = OSC[t.wave] ?? OSC.triangle;
      const len = Math.ceil((hold + rel) * sampleRate);
      for (let i = 0; i < len && start + i < (loop ? Infinity : total); i++) {
        const tt = i / sampleRate;
        let e = tt < a ? tt / a : tt < a + dec ? 1 - (1 - sus) * ((tt - a) / dec) : sus;
        if (tt > hold) e *= Math.max(0, 1 - (tt - hold) / rel);
        const ph = (f * tt) % 1;
        put(start + i, wave(ph) * e * amp);
      }
    }
  }
  let peak = 0;
  if (normalize) for (let i = 0; i < total; i++) peak = Math.max(peak, Math.abs(mix[i]));
  if (peak > 0.9) for (let i = 0; i < total; i++) mix[i] *= 0.9 / peak;
  return mix;
}
