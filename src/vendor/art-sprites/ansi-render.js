// Cells back to pixels, for looking at ANSI art without a terminal. The glyphs come from a PNG atlas of IBM VGA 8x16 (VileR, CC BY-SA 4.0;
// see docs/fonts/CREDIT.md): 16 by 16 cells of 8 by 16 pixels in CP437 order, lit pixels opaque. The atlas is passed in (`loadAtlas` in
// atlas.js reads the shipped one under Node), so this module stays free of Node built-ins and runs in a page too.
import { NONE, BOLD } from './cells.js';
import { XTERM256, rgbOf } from './palette-ansi.js';
import { CP437 } from './ansi.js';

const GLYPH_W = 8, GLYPH_H = 16, ATLAS_COLS = 16;
const INDEX_OF = new Map(); CP437.forEach((cp, i) => { if (!INDEX_OF.has(cp)) INDEX_OF.set(cp, i); });
const BLANK = INDEX_OF.get(0x20), QUESTION = INDEX_OF.get(0x3f);
const DEFAULT_FG = 7, DEFAULT_BG = 0;

/**
 * Cells to an image: `{ w, h, rgba, missing }`, 8 by 16 pixels a cell (9 by 16 with `letterSpacing: 9`).
 * `atlas` is `{ w, h, rgba }` (a decoded PNG of the VGA atlas). `theme` is 16 or 256 hex colours (default the xterm set, whose first 16
 * are the VGA colours). A cell with no colour shows palette 7 on 0. `bold` turns a dim foreground bright, as a classic terminal does.
 * `letterSpacing: 9` adds the ninth column of VGA text mode: the line and block characters repeat their eighth column, the rest leave it
 * blank. A character outside CP437 draws as `?` and is listed in `missing` (code points).
 */
const unpack = (n) => [(n >> 16) & 255, (n >> 8) & 255, n & 255];
export function renderCells(cells, { atlas, theme = XTERM256, letterSpacing = 8, bold = true } = {}) {
  if (!atlas || atlas.w !== GLYPH_W * ATLAS_COLS || atlas.h !== GLYPH_H * ATLAS_COLS) throw new Error('renderCells needs the 128 by 256 VGA glyph atlas');
  if (letterSpacing !== 8 && letterSpacing !== 9) throw new Error('letterSpacing is 8 or 9');
  const rgb = theme.map(rgbOf), at = (i) => rgb[i] ?? rgb[i < 16 ? DEFAULT_FG : DEFAULT_BG];
  const cw = letterSpacing, W = cells.w * cw, H = cells.h * GLYPH_H, out = new Uint8Array(W * H * 4), missing = new Set();
  for (let cy = 0; cy < cells.h; cy++) for (let cx = 0; cx < cells.w; cx++) {
    const c = cells.get(cx, cy);
    let fg = c.fg === NONE ? DEFAULT_FG : c.fg;
    if (bold && c.flags & BOLD && fg < 8) fg += 8;
    const back = c.bgRgb >= 0 ? unpack(c.bgRgb) : at(c.bg === NONE ? DEFAULT_BG : c.bg), front = c.fgRgb >= 0 ? unpack(c.fgRgb) : at(fg);
    let g = c.ch === 0 ? BLANK : INDEX_OF.get(c.ch);
    if (g === undefined) { missing.add(c.ch); g = QUESTION; }
    const gx = (g % ATLAS_COLS) * GLYPH_W, gy = Math.floor(g / ATLAS_COLS) * GLYPH_H, joins = g >= 0xc0 && g <= 0xdf;
    for (let y = 0; y < GLYPH_H; y++) for (let x = 0; x < cw; x++) {
      const sx = x < GLYPH_W ? x : joins ? GLYPH_W - 1 : -1;
      const lit = sx >= 0 && atlas.rgba[((gy + y) * atlas.w + gx + sx) * 4 + 3] > 127;
      out.set([...(lit ? front : back), 255], ((cy * GLYPH_H + y) * W + cx * cw + x) * 4);
    }
  }
  return { w: W, h: H, rgba: out, missing: [...missing] };
}
