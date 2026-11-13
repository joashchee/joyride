export const PPQ: 96;
export const STEP: 24;

export interface Note { p: number; t: number; d: number; v: number }
export interface Track { name: string; program: number; drum: boolean; wave: Wave; env: [number, number, number, number] | null; gain: number; notes: Note[] }
export type Wave = 'sine' | 'triangle' | 'square' | 'saw';
export interface KeyChange { t: number; key: string; scale: string }
export interface ScoreInit { bpm?: number; bars?: number; beats?: number; key?: string; scale?: string; title?: string; keyChanges?: KeyChange[] }

export class Score {
  constructor(init?: ScoreInit);
  bpm: number; bars: number; beats: number; key: string; scale: string; title: string;
  tracks: Map<string, Track>;
  /** Where the piece modulates; empty means it stays in `key` and `scale`. */
  keyChanges: KeyChange[];
  readonly barTicks: number;
  readonly length: number;
  track(name: string, opts?: Partial<Omit<Track, 'name' | 'notes'>>): Track;
  note(track: string, pitch: number, start: number, length: number, velocity?: number): this;
  notes(track: string): Note[];
  count(track?: string): number;
  end(): number;
  /** The key and scale in force at tick `t`. */
  keyAt(t: number): { key: string; scale: string };
  /** From tick `t` the music is in `key` (and `scale`, the score's own by default). */
  modulate(t: number, key: string, scale?: string): this;
  clone(): Score;
  equals(other: Score): boolean;
}

export interface Rng { next(): number; int(n: number): number; range(a: number, b: number): number; chance(p: number): boolean; pick<T>(a: readonly T[]): T; fork(label: string): Rng }
export function rng(seed: string | number): Rng;
export function hashSeed(seed: string | number): number;

export const NOTE_NAMES: string[];
export const SCALES: Record<string, number[]>;
export const SCALE_IDS: string[];
export const PARENT: Record<string, string>;
export const PROGRESSIONS: Record<string, number[]>;
export const PROGRESSION_IDS: string[];
export const DRUM: { kick: 36; rim: 37; snare: 38; clap: 39; hat: 42; pedalHat: 44; tomLow: 45; openHat: 46; tomMid: 47; crash: 49; tomHi: 50; ride: 51 };
export function tonic(key: string): number;
export function pitch(name: string): number;
export function pitchName(p: number): string;
export function degree(key: string, scale: string, n: number, octave?: number): number;
export function inScale(key: string, scale: string, p: number): boolean;
export function scaleNotes(key: string, scale: string, lo?: number, hi?: number): number[];
export function snap(key: string, scale: string, p: number): number;
export function chordOf(key: string, scale: string, root: number, opts?: { size?: 3 | 4; octave?: number }): number[];
export function chordName(key: string, scale: string, root: number): string;
export function progression(spec: string | number[]): number[];

export function arp(chord: number[], steps: number, mode?: 'up' | 'down' | 'updown' | 'alberti'): number[];
export function voiceLead(prev: number[], next: number[]): number[];
/** Consonant intervals between two voices, in semitones mod 12. */
export const CONSONANT: Set<number>;
export const PERFECT: Set<number>;
/** Two voices moving the same way from a perfect interval to the same perfect interval. */
export function parallelPerfect(a1: number, b1: number, a2: number, b2: number): boolean;

export function transpose(score: Score, semitones: number, opts?: { tracks?: string[] }): Score;
export function invert(score: Score, axis: number, opts?: { tracks?: string[] }): Score;
export function retrograde(score: Score): Score;
export function stretch(score: Score, num: number, den?: number): Score;
export function repeat(score: Score, n: number): Score;
export function slice(score: Score, fromBar: number, toBar: number): Score;
export function concat(a: Score, b: Score): Score;
export function layer(a: Score, b: Score): Score;
export function swing(score: Score, amount?: number): Score;
export function humanize(score: Score, seed: string | number, opts?: { time?: number; vel?: number }): Score;

export interface Finding { rule: string; level: 'warn' | 'error'; message: string }
/** `voices: [a, b]` names two counterpoint tracks and adds the `parallel` and `dissonance` rules. */
export function lint(score: Score, opts?: { name?: string; skip?: string[]; loop?: boolean; voices?: [string, string] | null }): Finding[];

export function toMidi(score: Score): Uint8Array;
export function fromMidi(bytes: Uint8Array): Score;
export type MidiEvent =
  | { tick: number; type: 'on'; channel: number; p: number; v: number } | { tick: number; type: 'off'; channel: number; p: number }
  | { tick: number; type: 'program'; channel: number; program: number } | { tick: number; type: 'tempo'; us: number }
  | { tick: number; type: 'timeSig'; num: number; den: number } | { tick: number; type: 'keySig'; sf: number; minor: boolean };
export function parseMidi(bytes: Uint8Array): { format: number; ppq: number; tracks: { name: string; events: MidiEvent[] }[] };

export function sha256(bytes: Uint8Array): Uint8Array;
export function hex(bytes: Uint8Array): string;
export class ByteWriter {
  readonly length: number;
  u8(v: number): this; i8(v: number): this; u16(v: number): this; u32(v: number): this; u64(v: number): this; f32(v: number): this;
  bytes(b: Uint8Array): this; ascii(s: string): this; str(s: string): this;
  chunk(id: string, fill: (w: ByteWriter) => void): this;
  done(): Uint8Array;
}
export class ByteReader {
  constructor(bytes: Uint8Array, at?: number);
  readonly left: number;
  u8(): number; i8(): number; u16(): number; u32(): number; u64(): number; f32(): number;
  bytes(n: number): Uint8Array; ascii(n: number): string; str(): string;
  chunk(): { id: string; body: ByteReader } | null;
}
export function nlzStore(bytes: Uint8Array): Uint8Array;
export function nlzUnstore(bytes: Uint8Array): Uint8Array;

export const NOTE_FADE: 253;
/** In an order list: skip this slot, or end the song here. */
export const ORDER_SKIP: 0xfffe;
export const ORDER_END: 0xffff;
/** The bit of a channel's flags that mutes it. */
export const CHANNEL_MUTED: 2;
export const NOTE_CUT: 254;
export const NOTE_OFF: 255;
export const KINDS: { mod: 0; s3m: 1; xm: 2; it: 3 };
export const LOOPS: { off: 0; forward: 1; pingpong: 2 };
export const EFFECTS: Record<string, Record<string, number>>;
export const EFFECT_KINDS: string[];
export interface EffectSlot { kind: string; name: string; mix: number; bypass: boolean; output: number | null; params: number[] }
export interface FmOperator { mult?: number; level?: number; ksl?: number; attack?: number; decay?: number; sustain?: number; release?: number; wave?: number; tremolo?: boolean; vibrato?: boolean; sustained?: boolean; ksr?: boolean }
export interface FmSpec { mod?: FmOperator; car?: FmOperator; feedback?: number; additive?: boolean }
export const OPL_PATCHES: Record<string, FmSpec>;
export const OPL_PATCH_IDS: string[];
export function fmPatch(spec?: FmSpec): number[];
export function oplPatch(id: string): number[];
export interface Cell { note?: number; ins?: number; vol?: [number, number]; fx?: [number, number] }
export interface Pattern { rows: number; name: string; cells: (Cell | null)[] }
export interface Sample { name: string; pcm: Int8Array | Int16Array; rate: number; volume: number; pan: number | null; loop: { kind: keyof typeof LOOPS; start: number; end: number } | null; source: unknown; fm: number[] | null }
export interface ModuleInit { title?: string; artist?: string; message?: string; kind?: keyof typeof KINDS; speed?: number; tempo?: number; channels?: number }
export class Module {
  constructor(init?: ModuleInit);
  title: string; artist: string; message: string; kind: keyof typeof KINDS; speed: number; tempo: number; restart: number; time: { rowsPerBeat: number; rowsPerMeasure: number };
  channels: { pan: number; vol: number; flags: number; name: string }[];
  patterns: Pattern[]; orders: number[]; samples: Sample[]; instruments: Record<string, any>[]; effects: EffectSlot[];
  /** More named order lists beside `orders`: the sections a host can switch between. */
  lists: { name: string; restart: number; orders: number[] }[];
  /** Composer's notes for a host (layers, sections), kept in the file as JSON. */
  meta: Record<string, any> | null;
  addPattern(rows?: number, name?: string): number;
  order(pattern: number): this;
  end(): this;
  addList(name: string, orders: number[], restart?: number): number;
  mute(channels: number[], muted?: boolean): this;
  cell(pattern: number, row: number, channel: number): Cell | null;
  set(pattern: number, row: number, channel: number, cell: Cell): this;
  addSample(s: { name?: string; pcm: Int8Array | Int16Array; rate?: number; volume?: number; pan?: number | null; loop?: Sample['loop'] }): number;
  addFm(name: string, patch: ArrayLike<number>): number;
  addEffect(kind: string, opts?: { name?: string; mix?: number | null; bypass?: boolean; output?: number | null; params?: Record<string, number> }): number;
  check(): this;
  clone(): Module;
  equals(other: Module): boolean;
}

export type EngineSpec = 'none' | 'midi' | 'opl' | 'sample' | 'bake' | { engine: 'none' | 'midi' | 'opl' | 'sample' | 'bake'; patch?: string; program?: number; bank?: number; source?: string; recipe?: string; note?: number; ms?: number; loop?: 'none' | 'whole'; seed?: number };
export interface ModuleReport { rowsPerBeat: number; ticksPerRow: number; speed: number; tempo: number; tempoError: number; notes: number; moved: number; maxShift: number; dropped: { track: string; p: number; t: number; reason: string }[]; fallbacks: { track: string; wanted: string; used: string; reason: string }[]; channels: number; patterns: number; unique: number; stems: Record<string, number[]>; layers?: Record<string, number[]>; tailRows?: number }
export function toModule(score: Score, opts?: { rowsPerBeat?: number; maxChannels?: number; patternRows?: number; sections?: Record<string, { from: number; to: number }> | null; engine?: EngineSpec | Record<string, EngineSpec>; effects?: ({ kind: string; tracks?: string[] } & Parameters<Module['addEffect']>[1])[]; lists?: boolean; layers?: Record<string, string[]> | null; on?: string[] | null; tail?: number; end?: boolean }): { module: Module; report: ModuleReport };

export const CYCLE: 64;
export const CYCLE_RATE: number;
export const DRUM_RATE: 22050;
export function cycle(wave: string): Int8Array;
export function drumShot(key: number): Int16Array;
export const MANIFEST_VERSION: 1;
export const MANIFEST_ENGINES: string[];
export const NOTE_FLOOR: Record<string, number>;
export const UNPITCHED: string[];
export interface ManifestSlot { id: string; engine: string; recipe: string; note: number; seed: number; ms: number; loop: 'none' | 'whole' }
export interface Manifest { version: 1; rate: number; slots: ManifestSlot[] }
export function toManifest(module: Module, opts?: { rate?: number }): Manifest;
export function bake(module: Module, wavs: Record<string, Uint8Array>): { baked: string[]; missing: string[] };
export function parseWav(bytes: Uint8Array): { rate: number; pcm: Int16Array };
export function toNtt(module: Module): Uint8Array;
export function parseNtt(bytes: Uint8Array): Module;

export const WAVES: Wave[];
export function render(score: Score, opts?: { sampleRate?: number; tail?: number; loop?: boolean }): Float32Array;
export function toWav(samples: Float32Array, sampleRate?: number): Uint8Array;
/** A Score as plain text (see the header of notation.js for the format); `parseNotation` reads it back exactly. */
export function toNotation(score: Score): string;
export function parseNotation(text: string): Score;
export function pianoRoll(score: Score, track: string, opts?: { maxBars?: number }): string;
export function toAudioBuffer(score: Score, ctx: { sampleRate: number; createBuffer(ch: number, len: number, rate: number): any }, opts?: { tail?: number; loop?: boolean }): any;
export function play(score: Score, opts?: { context?: any; loop?: boolean; gain?: number }): { context: any; stop(): void };

export interface Art { id: string; score: Score; loop: boolean; sections: Record<string, { from: number; to: number }>; params: Record<string, unknown>; /** Options to pass to `lint` for this composition (counterpoint names its voices). */ lint?: { voices?: [string, string] } }
export interface Recipe { describe: string; variants(): Record<string, unknown>[]; draw(params?: Record<string, unknown>): Omit<Art, 'params'> }
export const RECIPES: Record<string, Recipe>;
export const RECIPE_IDS: string[];
export function compose(id: string, params?: Record<string, unknown>): Art;

export const CONTOURS: Record<string, (t: number) => number>;
export const CHORD_STYLES: string[];
export const BASS_PATTERNS: string[];
export const DRUM_PATTERNS: Record<string, { kick: string; snare: string; hat: string }>;
export const FORMS: Record<string, [string, number, string][]>;
/** Beats a bar for the drum patterns that are not in four. */
export const DRUM_BEATS: Record<string, number>;
export const BLUES_12: number[];
export const BLUES_SCALES: string[];
export const MODULATIONS: Record<string, (homeScale: string) => [number, string]>;
export const MODULATION_IDS: string[];
export const SPECIES: number[];
/** The last two chords of a phrase, as scale degrees. */
export const CADENCES: Record<'authentic' | 'half' | 'plagal' | 'deceptive', [number, number]>;
export const CADENCE_IDS: string[];
export const PHRASE_TYPES: string[];
export function writeBlues(opts?: Record<string, unknown>): { score: Score; sections: Record<string, { from: number; to: number }> };
export function writeModulation(opts?: Record<string, unknown>): { score: Score; sections: Record<string, { from: number; to: number }> };
export function writeCounterpoint(opts?: Record<string, unknown>): { score: Score; sections: Record<string, { from: number; to: number }>; lint: { voices: [string, string] } };
export function writePhrase(opts?: Record<string, unknown>): { score: Score; sections: Record<string, { from: number; to: number }> };
export const JINGLES: Record<string, { bars: number; bpm: number; scale: string; loop: boolean; describe: string }>;
export const MOODS: Record<string, { scale: string; bpm: number; progression: string; density: number; contour: string }>;
export function addMelody(score: Score, r: Rng, opts?: Record<string, unknown>): Score;
export function addChords(score: Score, r: Rng, opts?: Record<string, unknown>): Score;
export function addBass(score: Score, r: Rng, opts?: Record<string, unknown>): Score;
export function addDrums(score: Score, r: Rng, opts?: Record<string, unknown>): Score;

export interface Subject { id: string; label: string; recipes?: { recipe: string; params?: Record<string, unknown> }[]; partial?: string; provides?: string; reference?: string }
export interface Kit { id: string; title: string; doc: string; workflow?: string[]; axes?: Record<string, unknown[]>; subjects: Subject[] }
export const KITS: Kit[];
export function registerKit(kit: Kit): void;
export function subjectStatus(subject: Subject): { status: 'covered' | 'partial' | 'gap' | 'built-in' | 'reference-only'; problems: string[]; warnings: string[] };
export function coverage(kits?: Kit[]): { id: string; title: string; doc: string; rows: ({ id: string; label: string; note: string } & ReturnType<typeof subjectStatus>)[]; totals: Record<string, number> }[];

export interface CueSpec { id: string; recipe: string; params?: Record<string, unknown>; kind?: 'loop' | 'stinger'; layers?: Record<string, string[]>; on?: string[]; tail?: number; engine?: EngineSpec | Record<string, EngineSpec> }
export interface CueSheet { title?: string; cues: CueSpec[]; transitions?: { from: string; to: string; at?: 'bar' | 'phrase' | 'section'; via?: string }[] }
export interface CueEntry { id: string; kind: 'loop' | 'stinger'; file: string; midi: string; recipe: string; params: Record<string, unknown>; key: string; scale: string; bpm: number; beats: number; bars: number; lengthMs: number; tailMs?: number; sections: { name: string; fromBar: number; toBar: number; list?: number }[]; switchBars: number[]; layers: Record<string, { tracks: string[]; channels: number[] }>; start: string[]; provenance: 'Project-created'; warnings: string[] }
export interface BuiltCue { id: string; art: Art; module: Module; report: ModuleReport; ntt: Uint8Array; midi: Uint8Array; entry: CueEntry }
export const CUE_KINDS: string[];
export const SWITCH_AT: string[];
export function nextSwitch(entry: CueEntry, bar: number, at?: 'bar' | 'phrase' | 'section'): number;
/** One cue's files and manifest entry; throws with every problem found. */
export function buildCue(cue: CueSpec, opts?: { engine?: EngineSpec | Record<string, EngineSpec>; rowsPerBeat?: number }): BuiltCue;
/** Every cue of a sheet, built; `manifest` is plain JSON for the game to read. */
export function composeCues(sheet: CueSheet, opts?: { engine?: EngineSpec | Record<string, EngineSpec>; rowsPerBeat?: number }): { manifest: { version: 1; title: string; cues: CueEntry[]; transitions: { from: string; to: string; at: string; via?: string }[] }; cues: BuiltCue[] };
/** The text `ntt_play --script` reads, following the manifest's switching; `hold` is bars of music before each change. */
export function playScript(manifest: { title?: string; cues: CueEntry[]; transitions?: { from: string; to: string; at?: string; via?: string }[] }, opts?: { hold?: number }): string;
/** The audio files a cue is baked to: one mix, or one stem per layer. Layer names must be usable in a file name. */
export function audioFiles(entry: CueEntry): { mix?: string; layers?: Record<string, string> };
/** A built cue rendered with the preview synth: a loop is exactly its length (it seams), a stinger its length plus its tail; stems sum to the mix. */
export function renderCue(built: BuiltCue, opts?: { sampleRate?: number }): { sampleRate: number; mix?: Float32Array; layers?: Record<string, Float32Array> };
/** A built cue baked to WAV bytes, keyed by the file names `audioFiles` gives. */
export function cueAudio(built: BuiltCue, opts?: { sampleRate?: number }): Record<string, Uint8Array>;
/** Baked audio per cue: an AudioBuffer (a mix) or `{ layer: AudioBuffer }` (stems of equal length). */
export type CueAudio = Record<string, any | Record<string, any>>;
export interface CuePlayerOptions { destination?: any; volume?: number; fade?: number; schedule?: (f: () => void, ms: number) => unknown }
export interface CuePlayer {
  /** Play a cue or `cue:section`: now if idle, else switched at `at` (default: the sheet's transition, else `phrase`), through `via`. Asking for what plays does nothing. Returns the context time it sounds. */
  play(ref: string, opts?: { at?: 'now' | 'bar' | 'phrase' | 'section'; via?: string }): number;
  /** A stinger cue once over the music; returns when it ends. */
  stinger(id: string, opts?: { at?: 'now' | 'bar' | 'phrase' | 'section' }): number;
  /** Exactly these layers sound, faded at the next `at` (default `bar`) over `seconds`; returns the context time it starts. */
  layers(on: string[], opts?: { at?: 'now' | 'bar' | 'phrase' | 'section'; seconds?: number }): number;
  stop(opts?: { seconds?: number }): void;
  setVolume(v: number): void;
  position(): number | null;
  state(): { cue: string | null; section: string | null; layers: string[]; switching: { to: string; at: number } | null };
}
/** A cue player on a Web Audio context: loops, stems and stingers, switching on the music's own bars by the manifest's rules. */
export function createCuePlayer(context: any, manifest: { cues: CueEntry[]; transitions?: { from: string; to: string; at?: string; via?: string }[] }, audio: CueAudio, opts?: CuePlayerOptions): CuePlayer;
/** Fetch and decode the baked audio of every cue under `base` (a URL ending in "/"); `formats` are extensions to try in order, e.g. `['opus', 'wav']`. */
export function loadCueAudio(context: any, manifest: { cues: CueEntry[] }, base?: string, opts?: { fetch?: typeof fetch; formats?: string[] }): Promise<CueAudio>;
export function writeLeitmotif(opts?: Record<string, unknown>): { score: Score; sections: Record<string, { from: number; to: number }> };
export function motifOf(theme: string): { step: number; t: number; d: number; v: number }[];
export const STATES: Record<string, { scale: string; bpm: number; program: number; wave: string; shift: number; speed: number; prog: string; chords: string; bass: string; drums: string | null }>;
export const STATE_IDS: string[];
