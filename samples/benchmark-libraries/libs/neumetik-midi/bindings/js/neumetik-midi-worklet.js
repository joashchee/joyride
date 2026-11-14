// Neumetik MIDI's AudioWorklet processor: a Player rendering on the audio
// thread, told what to do through its port. createMusicNode() in
// neumetik-midi.js sets it up. Copyright 2026 ansiapps. MIT licensed.

import { NeumetikMidi } from './neumetik-midi.js';

// Status goes to the page about 20 times a second.
const STATUS_EVERY = 0.05;

class NeumetikMidiProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.player = null;
    this.since = 0;
    this.alive = true;
    WebAssembly.instantiate(options.processorOptions.module, {})
      .then((instance) => {
        this.player = new NeumetikMidi(instance).createPlayer(sampleRate);
        this.port.postMessage({ ready: true });
      })
      .catch((e) => this.port.postMessage({ failed: String(e) }));
    this.port.onmessage = ({ data }) => this.message(data);
  }

  message(data) {
    if (data.destroy) {
      this.player?.destroy();
      this.player = null;
      this.alive = false;
      return;
    }
    const { id, method, args } = data;
    try {
      const result = this.player[method](...args);
      this.port.postMessage({ id, result });
    } catch (e) {
      this.port.postMessage({ id, error: e.message ?? String(e) });
    }
    this.sendStatus();
  }

  sendStatus() {
    this.since = 0;
    if (this.player) this.port.postMessage({ status: this.player.status() });
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    if (!this.player || out.length < 2) return this.alive;
    this.player.render(out[0], out[1]);
    this.since += out[0].length / sampleRate;
    if (this.since >= STATUS_EVERY) this.sendStatus();
    return this.alive;
  }
}

registerProcessor('neumetik-midi', NeumetikMidiProcessor);
