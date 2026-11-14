// Neumetik MIDI for JavaScript: General MIDI and GS music in the browser
// (or Node), no SoundFont. Copyright 2026 ansiapps. MIT licensed.
//
// Two ways to use it:
//
// 1. In real time, through an AudioWorklet (the music never stutters when
//    the page is busy). Phaser, PixiJS, three.js, plain Web Audio:
//
//      import { createMusicNode } from './neumetik-midi.js';
//      const music = await createMusicNode(audioContext);
//      music.node.connect(audioContext.destination);
//      await music.load(await (await fetch('song.mid')).arrayBuffer());
//      music.setLooping(true);
//      music.play();
//
// 2. Directly, on any thread (offline rendering, Node, your own worklet):
//
//      import { loadNeumetikMidi } from './neumetik-midi.js';
//      const midi = await loadNeumetikMidi();          // fetches the .wasm
//      const player = midi.createPlayer(48000);
//      player.load(midiBytes);
//      player.play();
//      player.render(left, right);                      // Float32Arrays
//
// Either can play through a SoundFont (.sf2) or DLS (.dls) bank:
// `await music.loadSoundBank(bytes)`, or `player.loadSoundBank(bytes)`.

const MAX_CHUNK = 4096;
const STATES = ['stopped', 'playing', 'paused'];

// Names and messages are ASCII. (Worklets have no TextDecoder.)
const ascii = (bytes) => String.fromCharCode(...bytes);

/** Instantiates the wasm module: from a URL (default: beside this file),
 *  bytes, or a compiled WebAssembly.Module. */
export async function loadNeumetikMidi(source = new URL('./neumetik_midi.wasm', import.meta.url)) {
  let instance;
  if (source instanceof WebAssembly.Module) {
    instance = await WebAssembly.instantiate(source, {});
  } else if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    instance = (await WebAssembly.instantiate(source, {})).instance;
  } else {
    const response = await fetch(source);
    if (!response.ok) throw new Error(`Couldn't load Neumetik MIDI (${response.status} ${source}).`);
    instance = (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance;
  }
  return new NeumetikMidi(instance);
}

/** The instantiated library: makes players. */
export class NeumetikMidi {
  constructor(instance) {
    this.exports = instance.exports;
  }

  get version() {
    const v = this.exports.nm_version();
    return `${Math.floor(v / 100)}.${String(v % 100).padStart(2, '0')}`;
  }

  /** A player at `sampleRate` (8000 to 192000). */
  createPlayer(sampleRate = 48000) {
    return new Player(this.exports, sampleRate);
  }

  /** A whole MIDI file to an AudioBuffer, at once: for a game that wants
   *  its music as ordinary sounds. `tail` seconds let the last notes ring;
   *  `loops` plays the loop that many more times; `soundBank` (a SoundFont
   *  or DLS file's bytes) plays it through that bank. */
  renderToAudioBuffer(context, midi, { tail = 2, loops = 0, soundBank = null } = {}) {
    const player = this.createPlayer(context.sampleRate);
    try {
      if (soundBank) player.loadSoundBank(soundBank);
      player.load(midi);
      player.looping = loops > 0;
      const [start, end] = player.loopPoints;
      const seconds = (loops > 0 ? end + (end - start) * (loops - 1) : player.duration) + tail;
      const frames = Math.ceil(seconds * context.sampleRate);
      const buffer = context.createBuffer(2, frames, context.sampleRate);
      player.play();
      const left = buffer.getChannelData(0);
      const right = buffer.getChannelData(1);
      let done = 0;
      while (done < frames) {
        if (loops > 0 && done >= Math.floor((seconds - tail) * context.sampleRate)) player.looping = false;
        const n = Math.min(MAX_CHUNK, frames - done);
        player.render(left.subarray(done, done + n), right.subarray(done, done + n));
        done += n;
      }
      return buffer;
    } finally {
      player.destroy();
    }
  }
}

/** One player. Not shared between threads: use it where it was made. */
export class Player {
  constructor(exports, sampleRate) {
    this.x = exports;
    this.ptr = exports.nm_create(sampleRate >>> 0);
    // Room in the module's memory for a chunk of samples, left and right,
    // and a name: allocated once.
    this.left = exports.nm_alloc(MAX_CHUNK * 4);
    this.right = exports.nm_alloc(MAX_CHUNK * 4);
    this.name = exports.nm_alloc(64);
  }

  destroy() {
    if (!this.ptr) return;
    this.x.nm_destroy(this.ptr);
    this.x.nm_free(this.left, MAX_CHUNK * 4);
    this.x.nm_free(this.right, MAX_CHUNK * 4);
    this.x.nm_free(this.name, 64);
    this.ptr = 0;
  }

  get sampleRate() { return this.x.nm_sample_rate(this.ptr); }

  /** Loads a MIDI file (ArrayBuffer or Uint8Array), stopped at its start.
   *  Throws an Error saying why it couldn't. */
  load(midi) {
    const bytes = midi instanceof Uint8Array ? midi : new Uint8Array(midi);
    const at = this.x.nm_alloc(bytes.length);
    new Uint8Array(this.x.memory.buffer, at, bytes.length).set(bytes);
    const code = this.x.nm_load(this.ptr, at, bytes.length);
    this.x.nm_free(at, bytes.length);
    if (code !== 0) throw new Error(`Couldn't play that MIDI file: ${this.errorString(code)}.`);
  }

  errorString(code) {
    const at = this.x.nm_error_string(code);
    const mem = new Uint8Array(this.x.memory.buffer);
    let end = at;
    while (mem[end] !== 0) end++;
    return ascii(mem.subarray(at, end));
  }

  /** Plays through a SoundFont (.sf2) or DLS (.dls) bank, an ArrayBuffer
   *  or Uint8Array: its instruments, and Neumetik's for what it hasn't.
   *  Throws an Error saying why it couldn't. A big bank takes a moment to
   *  read: load it before the music starts. */
  loadSoundBank(bank) {
    const bytes = bank instanceof Uint8Array ? bank : new Uint8Array(bank);
    const at = this.x.nm_alloc(bytes.length);
    new Uint8Array(this.x.memory.buffer, at, bytes.length).set(bytes);
    const code = this.x.nm_load_soundbank(this.ptr, at, bytes.length);
    this.x.nm_free(at, bytes.length);
    if (code !== 0) throw new Error(`Couldn't use that sound bank: ${this.errorString(code)}.`);
  }

  /** Back to Neumetik's own instruments. */
  clearSoundBank() { this.x.nm_set_soundbank(this.ptr, 0); }

  unload() { this.x.nm_unload(this.ptr); }
  play() { this.x.nm_play(this.ptr); }
  pause() { this.x.nm_pause(this.ptr); }
  stop() { this.x.nm_stop(this.ptr); }
  /** 'stopped', 'playing' or 'paused'. */
  get state() { return STATES[this.x.nm_state(this.ptr)]; }
  get playing() { return this.x.nm_state(this.ptr) === 1; }
  get finished() { return this.x.nm_is_finished(this.ptr) !== 0; }

  seek(seconds) { this.x.nm_seek(this.ptr, seconds); }
  get position() { return this.x.nm_position(this.ptr); }
  get duration() { return this.x.nm_duration(this.ptr); }

  get looping() { return this.x.nm_is_looping(this.ptr) !== 0; }
  set looping(on) { this.x.nm_set_looping(this.ptr, on ? 1 : 0); }
  setLooping(on) { this.looping = on; }
  /** In seconds; null for either goes back to the file's own loop points. */
  setLoopPoints(start = null, end = null) { this.x.nm_set_loop_points(this.ptr, start ?? -1, end ?? -1); }
  get loopPoints() { return [this.x.nm_loop_start(this.ptr), this.x.nm_loop_end(this.ptr)]; }

  setSpeed(speed) { this.x.nm_set_speed(this.ptr, speed); }
  get volume() { return this.x.nm_volume(this.ptr); }
  set volume(v) { this.x.nm_set_volume(this.ptr, v); }
  setVolume(v) { this.volume = v; }
  fadeTo(volume, seconds) { this.x.nm_fade_to(this.ptr, volume, seconds); }
  setChannelMuted(channel, muted) { this.x.nm_set_channel_muted(this.ptr, channel, muted ? 1 : 0); }

  noteOn(channel, key, velocity = 100) { this.x.nm_note_on(this.ptr, channel, key, velocity); }
  noteOff(channel, key) { this.x.nm_note_off(this.ptr, channel, key); }
  controlChange(channel, controller, value) { this.x.nm_control_change(this.ptr, channel, controller, value); }
  programChange(channel, program) { this.x.nm_program_change(this.ptr, channel, program); }
  setInstrument(channel, bank, program) { this.x.nm_set_instrument(this.ptr, channel, bank, program); }
  /** -8192 to 8191, 0 in tune. */
  pitchBend(channel, value) { this.x.nm_pitch_bend(this.ptr, channel, value); }
  /** Raw MIDI bytes (an array or Uint8Array): Web MIDI input can go
   *  straight in. */
  sendMidi(data) {
    const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
    const at = this.x.nm_alloc(bytes.length);
    new Uint8Array(this.x.memory.buffer, at, bytes.length).set(bytes);
    this.x.nm_send_midi(this.ptr, at, bytes.length);
    this.x.nm_free(at, bytes.length);
  }
  allNotesOff() { this.x.nm_all_notes_off(this.ptr); }
  allSoundOff() { this.x.nm_all_sound_off(this.ptr); }
  reset() { this.x.nm_reset(this.ptr); }

  get activeVoices() { return this.x.nm_active_voices(this.ptr); }
  instrumentName(channel) {
    const n = Math.min(this.x.nm_instrument_name(this.ptr, channel, this.name, 64), 63);
    return ascii(new Uint8Array(this.x.memory.buffer, this.name, n));
  }

  /** Views of the sample chunks, made again only if the memory grew: the
   *  audio thread makes no garbage. */
  chunks() {
    const buf = this.x.memory.buffer;
    if (this.viewsOf !== buf) {
      this.viewsOf = buf;
      this.l = new Float32Array(buf, this.left, MAX_CHUNK);
      this.r = new Float32Array(buf, this.right, MAX_CHUNK);
    }
    return [this.l, this.r];
  }

  /** Renders into two Float32Arrays of the same length. */
  render(left, right) {
    const frames = Math.min(left.length, right.length);
    for (let done = 0; done < frames; done += MAX_CHUNK) {
      const n = Math.min(MAX_CHUNK, frames - done);
      this.x.nm_render_f32_planar(this.ptr, this.left, this.right, n);
      const [l, r] = this.chunks();
      if (n === MAX_CHUNK) {
        left.set(l, done);
        right.set(r, done);
      } else {
        for (let i = 0; i < n; i++) {
          left[done + i] = l[i];
          right[done + i] = r[i];
        }
      }
    }
  }

  /** Renders interleaved stereo into one Float32Array (length / 2 frames). */
  renderInterleaved(out) {
    const frames = out.length >> 1;
    for (let done = 0; done < frames; done += MAX_CHUNK) {
      const n = Math.min(MAX_CHUNK, frames - done);
      this.x.nm_render_f32_planar(this.ptr, this.left, this.right, n);
      const [l, r] = this.chunks();
      for (let i = 0; i < n; i++) {
        out[2 * (done + i)] = l[i];
        out[2 * (done + i) + 1] = r[i];
      }
    }
  }

  /** Where it stands, all at once. */
  status() {
    return {
      state: this.state,
      position: this.position,
      duration: this.duration,
      looping: this.looping,
      loopPoints: this.loopPoints,
      finished: this.finished,
      volume: this.volume,
      voices: this.activeVoices,
    };
  }
}

// What a MusicNode passes on to the player in the worklet.
const METHODS = [
  'load', 'loadSoundBank', 'clearSoundBank', 'unload', 'play', 'pause', 'stop', 'seek', 'setLooping', 'setLoopPoints', 'setSpeed', 'setVolume', 'fadeTo',
  'setChannelMuted', 'noteOn', 'noteOff', 'controlChange', 'programChange', 'setInstrument', 'pitchBend', 'sendMidi',
  'allNotesOff', 'allSoundOff', 'reset', 'instrumentName', 'status',
];

/**
 * Music in real time: a player in an AudioWorklet, behind an
 * AudioWorkletNode (`.node`, stereo out) to connect where you like.
 * Every Player method is here too, returning a Promise; `.status` is kept
 * up to date (about 20 times a second) and `onstatus` called with it.
 */
export async function createMusicNode(context, {
  wasmUrl = new URL('./neumetik_midi.wasm', import.meta.url),
  workletUrl = new URL('./neumetik-midi-worklet.js', import.meta.url),
} = {}) {
  const [module] = await Promise.all([
    fetch(wasmUrl).then((r) => {
      if (!r.ok) throw new Error(`Couldn't load Neumetik MIDI (${r.status} ${wasmUrl}).`);
      return r.arrayBuffer();
    }).then((b) => WebAssembly.compile(b)),
    context.audioWorklet.addModule(workletUrl),
  ]);
  const node = new AudioWorkletNode(context, 'neumetik-midi', {
    numberOfInputs: 0,
    numberOfOutputs: 1,
    outputChannelCount: [2],
    processorOptions: { module },
  });
  const music = new MusicNode(node);
  await music.ready;
  return music;
}

export class MusicNode {
  constructor(node) {
    this.node = node;
    this.status = null;
    this.onstatus = null;
    this.calls = new Map();
    this.next = 1;
    this.ready = new Promise((resolve, reject) => {
      node.port.onmessage = ({ data }) => {
        if (data.ready) resolve();
        else if (data.failed) reject(new Error(data.failed));
        else if (data.status) {
          this.status = data.status;
          this.onstatus?.(data.status);
        } else if (this.calls.has(data.id)) {
          const { resolve: ok, reject: fail } = this.calls.get(data.id);
          this.calls.delete(data.id);
          if ('error' in data) fail(new Error(data.error));
          else ok(data.result);
        }
      };
    });
    for (const method of METHODS) {
      this[method] = (...args) => this.call(method, args);
    }
  }

  call(method, args) {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.calls.set(id, { resolve, reject });
      const transfer = args.filter((a) => a instanceof ArrayBuffer);
      this.node.port.postMessage({ id, method, args }, transfer);
    });
  }

  disconnect() {
    this.node.port.postMessage({ destroy: true });
    this.node.disconnect();
  }
}
