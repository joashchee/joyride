// Sample slots from Composer's own preview synth, so an .ntt plays in any tracker player with no host: a pitched track becomes one
// looped cycle of its wave (the tracker pitches it, and its volume envelope is the track's attack/decay/sustain/release), a drum key
// becomes a one-shot of the percussion table. These come from Math.sin and Math.exp, so they are as exact as the engine's maths
// (V8 gives the same points everywhere); the MIDI stays the exact artifact and the golden test never hashes these.
import { OSC, DRUMS, noise } from './synth.js';
import { hashSeed } from './rng.js';

export const CYCLE = 64;                            // frames in a looped cycle
export const CYCLE_RATE = Math.round(CYCLE * 261.6256);   // played at this C-5 rate, a 64-frame cycle sounds middle C
export const DRUM_RATE = 22050;

/** One cycle of `wave` (sine, triangle, square, saw) as 64 signed 8-bit points, peak 110 so a loud note does not clip when mixed. */
export function cycle(wave) {
  const f = OSC[wave] ?? OSC.triangle, raw = Array.from({ length: CYCLE }, (_, i) => f(i / CYCLE));
  const peak = Math.max(...raw.map(Math.abs));
  return Int8Array.from(raw, (v) => Math.round((v / peak) * 110));
}

/** A percussion hit for General MIDI key `key` as 16-bit points at DRUM_RATE, peaking at 80% of full scale. */
export function drumShot(key) {
  const [secs, voice] = DRUMS[key] ?? [0.1, (t, i, s) => noise(i, s) * Math.exp(-t / 0.03) * 0.5], salt = hashSeed(`drum:${key}`);
  const raw = Array.from({ length: Math.ceil(secs * DRUM_RATE) }, (_, i) => voice(i / DRUM_RATE, i, salt));
  const peak = Math.max(1e-9, ...raw.map(Math.abs));
  return Int16Array.from(raw, (v) => Math.round((v / peak) * 0.8 * 32767));
}
