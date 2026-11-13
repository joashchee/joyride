// Web Audio helpers (Painter's render.js). No DOM at import time: the AudioContext is passed in or made when play() is called.
import { render } from './synth.js';

/** An AudioBuffer of the score, for a context you already have. */
export function toAudioBuffer(score, ctx, opts = {}) {
  const samples = render(score, { sampleRate: ctx.sampleRate, ...opts });
  const buf = ctx.createBuffer(1, samples.length, ctx.sampleRate);
  buf.copyToChannel(samples, 0);
  return buf;
}

/** Play a score until it ends (or until `stop()` when `loop`). Returns `{ stop, context }`. */
export function play(score, { context = new globalThis.AudioContext(), loop = false, gain = 1 } = {}) {
  const src = context.createBufferSource(), g = context.createGain();
  src.buffer = toAudioBuffer(score, context, { loop });
  src.loop = loop;
  g.gain.value = gain;
  src.connect(g).connect(context.destination);
  src.start();
  return { context, stop: () => { try { src.stop(); } catch { /* already stopped */ } } };
}
