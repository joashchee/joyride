import type { Pixels } from './index.js';
/** Node only (returns PNG bytes). */
export function contactSheet(items: Pixels[], opts?: { scale?: number; cols?: number; bg?: string; gap?: number }): Buffer;
export function scaleStrip(p: Pixels, scales?: number[], bg?: string): Buffer;
export interface RgbaImage { w: number; h: number; rgba: Uint8Array }
/** Node only. Rendered cells (`renderCells` output) on one PNG, each at an integer `scale`. */
export function rgbaSheet(images: RgbaImage[], opts?: { scale?: number; cols?: number; bg?: string; gap?: number }): Buffer;
