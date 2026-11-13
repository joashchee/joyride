// Types for sprite-animator's main entry. Keep in step with index.js (a test checks every export is named here).
export function rng(seed: string | number): { next(): number; int(n: number): number; range(a: number, b: number): number; chance(p: number): boolean; pick<T>(a: T[]): T; fork(label: string): ReturnType<typeof rng> };
export function hashSeed(seed: string | number): number;

/** One frame's pose. Integers throughout; y is up-negative; `legs` are the two feet's offsets (negative back); `air` is 1 while no foot is down. */
export interface Pose { dx: number; dy: number; lean: number; head: number; squat: number; lift: number; drag: number; sway: number; legs: [number, number]; arm: string; weapon: string; air: 0 | 1 }
export interface ClipEvent { frame: number; name: string }
export interface Clip {
  id: string;
  /** 'walk' | 'run' | 'jump' for the lint's motion rules, or '' (a motion-type clip names its type in `spec.recipe`, `figure.hop`) */
  kind: string;
  frames: Pose[];
  fps: number;
  loop: boolean;
  events: ClipEvent[];
  /** How many frames each frame shows (1 or more). */
  /** True on a tile that scrolls: `dx dy lift` carry what leaves one edge in at the other. Left out otherwise. */
  wrap?: boolean;
  holds: number[];
  /** The poses for reduced motion. */
  reduced: Pose[];
  spec?: { recipe: string; params: Record<string, unknown> };
}
export interface Key extends Partial<Omit<Pose, 'legs'>> { legs?: [number, number]; at?: number; hold?: number; event?: string | string[] }

export const NUMERIC: readonly string[];
export const STEPPED: readonly string[];
export const CHANNELS: readonly string[];
export const REST: Readonly<Pose>;
export function pose(o?: Partial<Pose>): Pose;
export function clip(id: string, frames: Pose[], o?: { fps?: number; loop?: boolean; wrap?: boolean; events?: ClipEvent[]; holds?: number[]; kind?: string; reduced?: Pose[]; spec?: Clip['spec'] }): Clip;
export function fromKeys(keys: Key[], o?: { loop?: boolean; frames?: number; span?: number }): { frames: Pose[]; holds: number[]; events: ClipEvent[] };
export function durations(c: Clip): number[];
export function frameDelta(a: Pose, b: Pose): number;
export function checkShape(c: unknown): string[];
export function writeClip(c: Clip): string;
export function readClip(text: string): Clip;

export interface Finding { level: 'error' | 'warn'; rule: string; message: string }
export function lintClip(c: Clip, o?: { min?: number; max?: number /* default 16, 64 for a wrap clip */; limits?: Record<string, number>; skip?: string[] }): Finding[];

export const PPQ: number;
export interface Tempo { bpm: number; beats: number; totalMs: number; totalTicks: number; ms: number[]; starts: number[]; events: (ClipEvent & { ms: number; tick: number })[] }
export function tempo(c: Clip, o: { bpm: number; stepsPerBeat?: number; beats?: number; ppq?: number }): Tempo;

export interface Bound<F> { id: string; frames: F[]; fps: number; loop: boolean; events: ClipEvent[]; reduced: F[]; holds: number[] }
export function bind<F>(c: Clip, drawFrame: (pose: Pose, index: number) => F): Bound<F>;

export interface WalkParams { stride?: number; bounce?: number; frames?: number; fps?: number; wobble?: number; seed?: string }
export interface RunParams { stride?: number; lean?: number; rise?: number; frames?: number; fps?: number; wobble?: number; seed?: string }
export interface JumpParams { height?: number; crouch?: number; frames?: number; fps?: number; wobble?: number; seed?: string }
/** The motion types (Painter 1.20): each has the channels it uses (a share of the art, or true for a joint) and presets with their own params. */
export type MotionTypeId = 'figure' | 'head' | 'creature' | 'object' | 'limb' | 'plant' | 'machine' | 'structure' | 'tiling' | 'scenery' | 'cloth' | 'effect' | 'panel' | 'lettering';
export interface MotionPreset { describe: string; params: Record<string, number | string | null>; make(size: { w: number; h: number }, params: Record<string, any>): { frames: Partial<Pose>[]; fps: number; loop?: boolean; holds?: number[]; events?: ClipEvent[]; wrap?: boolean } }
export interface MotionType { describe: string; channels: Record<string, number | true>; presets: Record<string, MotionPreset> }
export const MOTION_TYPES: Record<MotionTypeId, MotionType>;
/** The motion type of each Painter recipe name. */
export const MOTION_OF: Record<string, MotionTypeId>;
export function motionTypeOf(painterRecipe: string): MotionTypeId;
export function motionPresets(type: MotionTypeId): string[];
/** A clip from a preset of a type; params are the preset's own plus `w` and `h` (the art's size, default 32). The clip id is the preset's name. */
export function motionClip(type: MotionTypeId, preset: string, params?: Record<string, number | string | null>): Clip;
export function lintMotionType(c: Clip, type: MotionTypeId, size?: { w: number; h: number }): Finding[];

export const RECIPES: Record<string, { describe: string; variants(): Record<string, unknown>[]; draw(params?: any): Clip }>;
export const RECIPE_IDS: string[];
export function animate(id: 'walk', params?: WalkParams): Clip;
export function animate(id: 'run', params?: RunParams): Clip;
export function animate(id: 'jump', params?: JumpParams): Clip;
export function animate(id: `${MotionTypeId}.${string}`, params?: Record<string, number | string | null>): Clip;
export function animate(id: string, params?: Record<string, unknown>): Clip;

export interface KitSubject { id: string; label: string; recipes?: { recipe: string; params?: Record<string, unknown>; lint?: Record<string, unknown> }[]; partial?: string; gap?: string; reference?: string }
export interface Kit { id: string; title: string; doc: string; subjects: KitSubject[] }
export const KITS: Kit[];
export function registerKit(kit: Kit): void;
export function subjectStatus(s: KitSubject): { status: 'covered' | 'partial' | 'gap' | 'reference-only'; problems: string[]; warnings: string[] };
export function coverage(kits?: Kit[]): { id: string; title: string; doc: string; rows: ({ id: string; label: string; note: string } & ReturnType<typeof subjectStatus>)[]; totals: Record<string, number> }[];
