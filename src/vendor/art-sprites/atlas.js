// The IBM VGA 8x16 glyph atlas, read from docs/fonts (Node only: it uses node:fs). Pass the result to `renderCells`.
import { readFileSync } from 'node:fs';
import { decodePng } from './png.js';

let cached = null;
/** The shipped atlas as `{ w, h, rgba }`. Read once. */
export const loadAtlas = () => (cached ??= decodePng(readFileSync(new URL('./docs/fonts/ibm-vga-8x16.png', import.meta.url))));
