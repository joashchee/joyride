// Digits and short words in pixels, from the IBM VGA 8x16 glyph atlas (VileR, CC BY-SA 4.0; docs/fonts/CREDIT.md). The atlas is passed in
// (`loadAtlas` from `art-sprites/atlas` reads the shipped one under Node), so this module has no Node built-ins and runs in a page too.
// Each glyph is the unmodified 8 by 16 cell: not scaled, subset or redrawn. Only CP437 characters exist; any other draws as `?`.
import { Pixels } from '../pixels.js';
import { C } from '../palette.js';
import { CP437 } from '../ansi.js';

const GW = 8, GH = 16, COLS = 16;
const INDEX_OF = new Map(); CP437.forEach((cp, i) => { if (!INDEX_OF.has(cp)) INDEX_OF.set(cp, i); });

/** `text` as a canvas 8 pixels a character by 16 high, in palette value `ink`, with a one-pixel `outline` (a colour value, or null for none; the canvas then has no border). */
export function pixelText(atlas, text, { ink = C.white, outline = C.night } = {}) {
  if (!atlas || atlas.w !== GW * COLS || atlas.h !== GH * COLS) throw new Error('pixel text needs the 128 by 256 VGA glyph atlas (loadAtlas from art-sprites/atlas)');
  const chars = [...String(text)], pad = outline ? 1 : 0, out = new Pixels(chars.length * GW + 2 * pad, GH + 2 * pad);
  chars.forEach((ch, n) => {
    const g = INDEX_OF.get(ch.codePointAt(0)) ?? INDEX_OF.get(0x3f), gx = (g % COLS) * GW, gy = Math.floor(g / COLS) * GH;
    for (let y = 0; y < GH; y++) for (let x = 0; x < GW; x++) if (atlas.rgba[((gy + y) * atlas.w + gx + x) * 4 + 3] > 127) out.set(pad + n * GW + x, pad + y, ink);
  });
  return outline ? out.outline(outline, true) : out;
}
