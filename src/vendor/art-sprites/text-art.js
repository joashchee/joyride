// The plain text style: art in 7-bit ASCII with no colour, for MUD clients, logs, terminals without colour and text fallbacks.
// The canvas is the same `Cells` grid the ANSI style uses (a glyph per cell, colours left unset), so recipes, seeds, the kits and
// the CLI carry over. A pixel drawing folds as in `foldPixels`, two square pixels to a cell, and is read as light and dark; an ANSI
// drawing is read cell by cell. Density stands for brightness, as on a dark screen: more ink is lighter. Browser-safe.
import { Cells, NONE, tone } from './cells.js';
import { Pixels } from './pixels.js';
import { luma as hexLuma } from './palette.js';

/** Light to dark in ink: a space is black, `@` is white. Opaque pixels never use the space, so a figure stays visible. */
export const ASCII_RAMP = ' .:-=+*#%@';
const INK = ASCII_RAMP.slice(1);
const asciiAt = (l, ramp) => ramp[Math.min(ramp.length - 1, Math.max(0, Math.floor(l * ramp.length)))];
const lumOf = (hex) => (0.299 * parseInt(hex.slice(1, 3), 16) + 0.587 * parseInt(hex.slice(3, 5), 16) + 0.114 * parseInt(hex.slice(5, 7), 16)) / 255;

/** Fold a pixel canvas to ASCII cells: the mean brightness of a cell's two pixels picks the ink; one opaque pixel gives `'` (upper) or `,` (lower). */
export function foldText(px) {
  const cells = new Cells(px.w, Math.ceil(px.h / 2)), lum = (v) => (v ? hexLuma(px.palette[v - 1]) / 255 : -1);
  for (let y = 0; y < cells.h; y++) for (let x = 0; x < px.w; x++) {
    const t = lum(px.get(x, y * 2)), b = lum(px.get(x, y * 2 + 1));
    if (t < 0 && b < 0) continue;
    cells.set(x, y, { ch: t < 0 ? ',' : b < 0 ? "'" : asciiAt((t + b) / 2, INK) });
  }
  return cells;
}

const isBox = (cp) => cp >= 0x2500 && cp <= 0x257f;
const BOX_H = new Set([0x2500, 0x2501, 0x2504, 0x2508, 0x254c]), BOX_V = new Set([0x2502, 0x2503, 0x2506, 0x250a, 0x254e, 0x2551]);
const SPECIAL = { 0x2550: '=', 0x2248: '~', 0x2022: '.', 0x00b7: '.', 0x2219: '.', 0x25cf: 'o', 0x25cb: 'o', 0x25a0: '#', 0x25ac: '-', 0x258c: '[', 0x2590: ']' };

/**
 * ANSI cells to ASCII cells. A printable ASCII glyph stays; box drawing becomes `- = | +`; space, shade and block cells become ink
 * by the brightness of their `tone` (so a shaded sky is a gradient of `. : - = + *`); any other character (runes, symbols) is `*`.
 * Colour is dropped. Empty cells stay empty.
 */
export function asciiFromCells(cells) {
  const out = new Cells(cells.w, cells.h);
  for (let y = 0; y < cells.h; y++) for (let x = 0; x < cells.w; x++) {
    if (cells.isEmpty(x, y)) continue;
    const c = cells.get(x, y), cp = c.ch;
    let ch;
    if (cp > 0x20 && cp < 0x7f) ch = String.fromCharCode(cp);
    else if (SPECIAL[cp]) ch = SPECIAL[cp];
    else if (isBox(cp)) ch = BOX_H.has(cp) ? '-' : BOX_V.has(cp) ? '|' : '+';
    else if (cp === 0 || cp === 0x20 || (cp >= 0x2580 && cp <= 0x259f)) ch = asciiAt(lumOf(tone(c)), ASCII_RAMP);
    else ch = '*';
    if (ch !== ' ') out.set(x, y, { ch });
  }
  return out;
}

const plain = (v) => v !== null && typeof v === 'object' && Object.getPrototypeOf(v) === Object.prototype;
const foldAnchors = (a) => (plain(a) ? Object.fromEntries(Object.entries(a).map(([k, p]) => [k, Array.isArray(p) && p.length === 2 && p.every(Number.isFinite) ? [p[0], p[1] / 2] : p])) : a);
const map = (v, f) => (v instanceof Pixels ? foldText(v) : v instanceof Cells ? f(v) : Array.isArray(v) ? v.map((x) => map(x, f)) : plain(v) ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, k === 'anchors' && f === null ? foldAnchors(x) : map(x, f)])) : v);

/** A whole pixel drawing as ASCII cells (frames, clips, layers and masks; anchors move to cell units), marked `style: 'text'`. */
export function textFromPixelArt(art) { return { ...map(art, null), style: 'text', folded: true }; }
/** A whole ANSI drawing (native or folded) as ASCII cells, marked `style: 'text'`. */
export function textFromAnsiArt(art) { return { ...map(art, asciiFromCells), style: 'text', folded: true }; }

/**
 * Cells to plain text. Returns `{ text, bytes, warnings }`: 7-bit ASCII only (anything else is written `?` with a warning, never
 * silently), trailing spaces cut, `eol` `\n` (default) or `\r\n`, and no SAUCE or escapes: it is the same on any terminal or log.
 */
export function writeText(cells, { eol = '\n' } = {}) {
  if (eol !== '\n' && eol !== '\r\n') throw new Error('eol is "\\n" or "\\r\\n"');
  const bad = new Set(), lines = [];
  for (let y = 0; y < cells.h; y++) {
    let s = '';
    for (let x = 0; x < cells.w; x++) { const cp = cells.ch[y * cells.w + x] || 0x20; if (cp < 0x20 || cp > 0x7e) { bad.add(cp); s += '?'; } else s += String.fromCharCode(cp); }
    lines.push(s.replace(/ +$/, ''));
  }
  const text = lines.join(eol) + eol, warnings = bad.size ? [`written as "?": not 7-bit ASCII: ${[...bad].slice(0, 8).map((cp) => `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`).join(', ')}${bad.size > 8 ? ', ...' : ''}`] : [];
  return { text, bytes: new TextEncoder().encode(text), warnings };
}

/**
 * Findings for text art: `ascii` (a character outside 7-bit ASCII, error), `width` (a row used past `width`, default 80, error),
 * `height` (over `maxHeight` rows, default 24, warn), `blank` (nothing drawn, error), `fill` (more than 85% of the box inked: no air
 * around the figure, warn) and `colour` (a colour set on a cell, warn: the style has none).
 */
export function lintText(cells, { name = 'art', width = 80, maxHeight = 24, skip = [] } = {}) {
  const out = [], add = (rule, level, message) => { if (!skip.includes(rule)) out.push({ rule, level, message: `${name}: ${message}` }); };
  let ink = 0, wide = 0, last = -1, coloured = 0; const bad = new Set();
  for (let y = 0; y < cells.h; y++) for (let x = 0; x < cells.w; x++) {
    const i = y * cells.w + x, cp = cells.ch[i];
    if (cells.fg[i] !== NONE || cells.bg[i] !== NONE) coloured++;
    if (!cp || cp === 0x20) continue;
    ink++; wide = Math.max(wide, x + 1); last = y;
    if (cp < 0x20 || cp > 0x7e) bad.add(cp);
  }
  if (bad.size) add('ascii', 'error', `${bad.size} character(s) outside 7-bit ASCII, such as U+${[...bad][0].toString(16).toUpperCase().padStart(4, '0')}`);
  if (!ink) add('blank', 'error', 'nothing is drawn');
  if (wide > width) add('width', 'error', `${wide} columns wide; the limit is ${width}`);
  if (last + 1 > maxHeight) add('height', 'warn', `${last + 1} rows; a plain terminal shows ${maxHeight}`);
  if (ink / (cells.w * cells.h) > 0.85) add('fill', 'warn', 'more than 85% of the cells are inked: no air around the figure');
  if (coloured) add('colour', 'warn', `${coloured} cell(s) carry a colour, which plain text drops`);
  return out;
}
