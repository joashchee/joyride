// What native ANSI recipes draw with, beyond the Cells primitives: the artists' workflow as an order a recipe cannot get wrong
// (`paint`), glyph overlays (`overlay`: stars, rain, snow, waves, runes) and block lettering (`lettering`). Browser-safe.
import { Cells } from './cells.js';
import { rng } from './rng.js';
import { V } from './palette-ansi.js';

const ORDER = ['focal', 'outline', 'base', 'highlights', 'shadows', 'blend'];

/**
 * Run a drawing in the order the tutorials teach, whatever order the steps are written in: `focal` (the eyes or centre of interest,
 * which sets the scale; what it returns is passed to the rest as `ctx.focal`), `outline`, `base` colour blocks, `highlights`,
 * `shadows`, and `blend` last. Each step is `(ctx) => void`. `ctx.cells` is the canvas; `ctx.outline(x, y, cell)` draws an outline
 * cell and protects it, and `ctx.put(x, y, cell)` draws a cell unless it is protected, so later steps keep the outline (the kit:
 * heavy shading erases the outline, keep most of it).
 */
export function paint(cells, steps) {
  const protectedAt = new Set(), ctx = {
    cells, focal: undefined,
    outline(x, y, cell) { if (cells.inb(x, y)) { cells.set(x, y, cell); protectedAt.add(y * cells.w + x); } },
    put(x, y, cell) { if (cells.inb(x, y) && !protectedAt.has(y * cells.w + x)) cells.set(x, y, cell); },
    isProtected: (x, y) => protectedAt.has(y * cells.w + x),
    /** Every unprotected cell set to `cell`: the ground the base colours are laid on. */
    fill(cell) { for (let y = 0; y < cells.h; y++) for (let x = 0; x < cells.w; x++) ctx.put(x, y, cell); },
    /** Another drawing's non-empty cells as outline: protected, so later steps keep them (a frame drawn on a scratch Cells). */
    stamp(src, dx = 0, dy = 0) { for (let y = 0; y < src.h; y++) for (let x = 0; x < src.w; x++) if (!src.isEmpty(x, y)) ctx.outline(x + dx, y + dy, src.get(x, y)); },
  };
  runSteps(steps, ctx);
  return cells;
}

/** The same order for a drawing that is not on Cells (a pixel canvas folded afterwards): each step gets `ctx`, `focal` is kept on it. */
export function runSteps(steps, ctx = {}) {
  for (const name of ORDER) if (steps[name]) { const r = steps[name](ctx); if (name === 'focal') ctx.focal = r; }
  return ctx;
}

// Glyph sets, all in CP437. `fg` is the default colour; a glyph lands only on a cell with nothing in it but a background, so an
// overlay never erases drawing.
const OVERLAYS = {
  stars: { glyphs: ['.', '·', '*', '+'], fg: [V.white, V.white, V.yellow, V.brightcyan] },
  rain: { glyphs: ['/', '/', '|', "'"], fg: [V.cyan, V.brightblue, V.cyan, V.gray] },
  snow: { glyphs: ['*', '.', '·', '*'], fg: [V.white, V.white, V.gray, V.brightcyan] },
  waves: { glyphs: ['~', '≈', '~', '-'], fg: [V.brightblue, V.cyan, V.brightcyan, V.brightblue] },
  runes: { glyphs: ['Φ', 'Θ', 'Ω', 'δ', '∞', '≡', '±', 'τ'], fg: [V.darkgray, V.darkgray, V.brightblue, V.magenta] },
};
export const OVERLAY_IDS = Object.keys(OVERLAYS);

/**
 * Scatter a glyph overlay over a region (`{ x, y, w, h }`, default the whole canvas) at `density` (0 to 1), chosen by seed. Glyphs
 * go only on cells that show no character yet (and, with `where(x, y)`, only where that says so). Returns the cells.
 */
export function overlay(cells, kind, seed, { region = { x: 0, y: 0, w: cells.w, h: cells.h }, density = 0.05, fg, where } = {}) {
  const set = OVERLAYS[kind]; if (!set) throw new Error(`unknown overlay "${kind}"; known: ${OVERLAY_IDS.join(', ')}`);
  const r = rng(`${seed}:${kind}`);
  for (let y = region.y; y < region.y + region.h; y++) for (let x = region.x; x < region.x + region.w; x++) {
    if (!r.chance(density)) continue;
    const k = r.int(set.glyphs.length), i = y * cells.w + x;
    if (!cells.inb(x, y) || (cells.ch[i] && cells.ch[i] !== 0x20) || (where && !where(x, y))) continue;
    cells.glyph(x, y, set.glyphs[k], fg ?? set.fg[k]);
  }
  return cells;
}

// Block lettering. A 3 by 5 bitmap font made by hand for this package (digits, letters, a little punctuation), drawn in one of four
// faces. Each glyph is five rows of three bits.
const FONT = {
  A: '010101111101101', B: '110101110101110', C: '011100100100011', D: '110101101101110', E: '111100110100111', F: '111100110100100',
  G: '011100101101011', H: '101101111101101', I: '111010010010111', J: '001001001101010', K: '101101110101101', L: '100100100100111',
  M: '101111111101101', N: '111101101101101', O: '010101101101010', P: '110101110100100', Q: '010101101111011', R: '110101110101101',
  S: '011100010001110', T: '111010010010010', U: '101101101101111', V: '101101101101010', W: '101101111111101', X: '101101010101101',
  Y: '101101010010010', Z: '111001010100111',
  0: '111101101101111', 1: '010110010010111', 2: '110001010100111', 3: '110001010001110', 4: '101101111001001', 5: '111100110001110',
  6: '011100111101111', 7: '111001010010010', 8: '111101111101111', 9: '111101111001110',
  '-': '000000111000000', '!': '010010010000010', '.': '000000000000010', ':': '000010000010000', ' ': '000000000000000',
};
export const LETTERING_FACES = ['half', 'block', 'outlined', 'shaded'];
export const LETTERING_CHARS = Object.keys(FONT).join('');

/** The bitmap of a text: rows of 0 and 1, one blank column between letters. Throws on a character the font lacks. */
function bitmap(text) {
  const rows = Array.from({ length: 5 }, () => []);
  [...String(text).toUpperCase()].forEach((ch, n) => {
    const g = FONT[ch]; if (!g) throw new Error(`the lettering font has no "${ch}"; it has ${LETTERING_CHARS}`);
    for (let y = 0; y < 5; y++) { if (n) rows[y].push(0); for (let x = 0; x < 3; x++) rows[y].push(+g[y * 3 + x]); }
  });
  return rows;
}

/**
 * Text as block lettering, as new Cells. Faces: `half` (the font in half blocks: 3 rows tall, 4 columns a letter), `block` (each
 * font pixel a `██`: 5 rows, 8 columns a letter), `outlined` (block with a one-cell dark gray contour) and `shaded` (block, the rows
 * lit from white down through the colour, with a `░` drop shadow). `fg` is the colour (a VGA index); `edge` the outline (default
 * dark gray). Throws if it is wider than `maxWidth` (80).
 */
export function lettering(text, { face = 'block', fg = V.yellow, edge = V.darkgray, maxWidth = 80 } = {}) {
  if (!LETTERING_FACES.includes(face)) throw new Error(`unknown lettering face "${face}"; known: ${LETTERING_FACES.join(', ')}`);
  const bm = bitmap(text), cols = bm[0].length;
  const wide = face === 'half' ? cols : cols * 2, pad = face === 'outlined' ? 1 : 0, shadow = face === 'shaded' ? 1 : 0;
  const w = wide + 2 * pad + 2 * shadow, h = (face === 'half' ? 3 : 5) + 2 * pad + shadow;
  if (w > maxWidth) throw new RangeError(`"${text}" is ${w} columns in the ${face} face; the most is ${maxWidth}`);
  const cells = new Cells(w, h);
  if (face === 'half') {
    for (let cy = 0; cy < 3; cy++) for (let x = 0; x < cols; x++) {
      const top = bm[cy * 2]?.[x], bot = bm[cy * 2 + 1]?.[x];
      if (top || bot) cells.set(x, cy, { ch: top && bot ? '█' : top ? '▀' : '▄', fg });
    }
    return cells;
  }
  const lit = [V.white, V.brightcyan, fg, fg, V.darkgray];
  const on = (x, y) => bm[y]?.[Math.floor(x / 2)] === 1;
  if (shadow) for (let y = 0; y < 5; y++) for (let x = 0; x < wide; x++) if (on(x, y)) cells.set(x + 2, y + 1, { ch: '░', fg: V.darkgray });
  for (let y = 0; y < 5; y++) for (let x = 0; x < wide; x++) if (on(x, y)) cells.set(x + pad, y + pad, { ch: '█', fg: shadow ? lit[y] : fg });
  if (pad) {
    const body = cells.clone();
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      if (!body.isEmpty(x, y)) continue;
      let hit = false;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (!body.isEmpty(x + dx, y + dy) && body.inb(x + dx, y + dy)) hit = true;
      if (hit) cells.set(x, y, { ch: '█', fg: edge });
    }
  }
  return cells;
}
