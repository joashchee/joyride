import type { Pixels } from './index.js';
/** Node only (uses node:zlib). RGBA bytes to a PNG file. */
export function encodePng(w: number, h: number, rgba: Uint8Array | Uint8ClampedArray): Buffer;
export function pixelsToPng(px: Pixels, scale?: number, bg?: string | null): Buffer;
export function pngSize(buf: Buffer): { w: number; h: number };
export function decodePng(buf: Buffer): { w: number; h: number; rgba: Uint8Array };
