// The bake step. Neumetik's engines (its MIDI synthesizer, BGN, chipbank, the sound chips) are Rust, and Composer imports nothing from
// Neumetik, so the two meet in files: `toManifest` lists what each instrument should sound like (engine, recipe, note, seed), a tool
// on the Neumetik side (tools/neumetik/render_manifest.rs) renders each entry to `<id>.wav`, and `bake` puts those waveforms into the
// Module. Until the WAVs exist an instrument keeps Composer's own preview-synth sample, so the .ntt always plays.
import { hashSeed } from './rng.js';

export const MANIFEST_VERSION = 1;
// midi: a General MIDI / GS / classics program (recipe "80" or "81/12": bank/program) played at `note`.
// midi.drum: a drum key (recipe "36"). bgn: a bed from Neumetik's background-noise bank (recipe = the preset's name).
// chipbank: a game sound recipe (recipe = its id, "loot.reveal.rare"). chip.<name>: a sound chip's voice at `note` (recipe: square, triangle, noise...).
// What the Neumetik-side tool renders today (tools/neumetik/render_manifest.rs); another chip is a name here and an arm there.
export const MANIFEST_ENGINES = ['midi', 'midi.drum', 'bgn', 'chipbank', 'chip.sn76489', 'chip.nes'];
// The lowest note a chip's period registers reach (the SN76489's 10 bits stop near A2, the NES's pulse near C#2).
export const NOTE_FLOOR = { 'chip.sn76489': 45, 'chip.nes': 34 };
export const UNPITCHED = ['midi.drum', 'bgn', 'chipbank'];   // played at C-5 whatever the note: speeding a bed or a hit up is not a pitch

/** The manifest for a Module's `bake` instruments: `{ version, rate, slots: [{ id, engine, recipe, note, seed, ms, loop }] }`. */
export function toManifest(module, { rate = 44100 } = {}) {
  const slots = module.instruments.filter((x) => x.engine === 'bake').map((x) => ({ id: x.id, engine: x.source, recipe: x.recipe, note: x.note, seed: x.seed, ms: x.ms, loop: x.loop }));
  return { version: MANIFEST_VERSION, rate, slots };
}

export const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'x';
export const seedOf = (id) => hashSeed(`manifest:${id}`) & 0x7fffffff;

/** Mono PCM from WAV bytes: 8- or 16-bit integer PCM, stereo averaged. `{ rate, pcm: Int16Array }`. */
export function parseWav(bytes) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), tag = (o) => String.fromCharCode(...bytes.subarray(o, o + 4));
  if (bytes.length < 12 || tag(0) !== 'RIFF' || tag(8) !== 'WAVE') throw new Error('not a WAV file');
  let fmt = null, data = null;
  for (let o = 12; o + 8 <= bytes.length;) {
    const id = tag(o), n = v.getUint32(o + 4, true), body = o + 8;
    if (body + n > bytes.length && id !== 'data') throw new Error('WAV chunk runs past the file');
    if (id === 'fmt ') fmt = { format: v.getUint16(body, true), channels: v.getUint16(body + 2, true), rate: v.getUint32(body + 4, true), bits: v.getUint16(body + 14, true) };
    if (id === 'data') { data = [body, Math.min(n, bytes.length - body)]; break; }
    o = body + n + (n & 1);
  }
  if (!fmt || !data) throw new Error('WAV lacks fmt or data');
  if (fmt.format !== 1 || (fmt.bits !== 8 && fmt.bits !== 16) || fmt.channels < 1 || fmt.channels > 2) throw new Error(`only 8- or 16-bit integer PCM, mono or stereo (got format ${fmt.format}, ${fmt.bits} bits, ${fmt.channels} channels)`);
  const step = (fmt.bits / 8) * fmt.channels, frames = Math.floor(data[1] / step), pcm = new Int16Array(frames);
  for (let i = 0; i < frames; i++) {
    let sum = 0;
    for (let c = 0; c < fmt.channels; c++) { const at = data[0] + i * step + c * (fmt.bits / 8); sum += fmt.bits === 16 ? v.getInt16(at, true) : (bytes[at] - 128) << 8; }
    pcm[i] = Math.round(sum / fmt.channels);
  }
  return { rate: fmt.rate, pcm };
}

/**
 * Puts rendered waveforms into the Module: `wavs` is `{ id: wavBytes }`. A baked instrument loses its own-synth envelope (the render has
 * its own) and plays its waveform at the pitch it was rendered at. Returns `{ baked, missing }` (instrument ids).
 */
export function bake(module, wavs) {
  const baked = [], missing = [];
  module.instruments.forEach((x, i) => {
    if (x.engine !== 'bake') return;
    const bytes = wavs[x.id];
    if (!bytes) { missing.push(x.id); return; }
    const { rate, pcm } = parseWav(bytes);
    if (!pcm.length) throw new Error(`the render for "${x.id}" is empty`);
    const peak = pcm.reduce((a, b) => Math.max(a, Math.abs(b)), 0);
    if (!peak) throw new Error(`the render for "${x.id}" is silent`);
    // Played at C-5 (MIDI 60) the waveform must sound at 60, but it was rendered at x.note: a rate that is as many semitones off.
    const c5 = Math.round(rate * 2 ** ((60 - (UNPITCHED.includes(x.source) ? 60 : x.note)) / 12));
    module.samples[i] = { name: x.name, pcm, rate: Math.min(192000, Math.max(1000, c5)), volume: 64, pan: null, loop: x.loop === 'whole' ? { kind: 'forward', start: 0, end: pcm.length } : null, source: null, fm: null };
    delete x.env; delete x.wave; x.baked = true;
    baked.push(x.id);
  });
  return { baked, missing };
}
