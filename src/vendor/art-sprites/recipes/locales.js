// The CoffeeMUD locales `vista` does not draw (its 24 terrains are Coupler's list and stay as they are): road lengths, an ice plain, frozen
// peaks, a tree surface, a sewer, a hot room, an ice room, a maze corridor, the ship decks and a starfield. Each is data in, picture out, on
// cells for a classic file. Most start from a vista (stone, wooden, plains, mountains) and repaint it; the rest are drawn whole.
import { Cells, NONE, rampCells, legalize } from '../cells.js';
import { V } from '../palette-ansi.js';
import { overlay } from '../ansi-craft.js';
import { rng } from '../rng.js';
import { drawVista } from './vista.js';

export const LOCALE_KINDS = ['road', 'iceplain', 'frozenpeaks', 'treetop', 'sewer', 'hotroom', 'iceroom', 'maze', 'deck', 'hold', 'quarters', 'gundeck', 'magazine', 'starfield'];
export const GUN_DECKS = { light: 2, medium: 4, heavy: 6 };
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const solid = (color) => ({ ch: ' ', fg: color, bg: color });
const glyph = (c, x, y, ch, fg) => { if (c.inb(x, y)) c.glyph(x, y, ch, fg); };
const vista = (terrain, p) => drawVista({ terrain, time: 'day', weather: 'clear', ...p }).frames[0];
/** Swap colours across a picture (fg and bg alike): how a stone room becomes a hot or an icy one. */
function recolor(c, map) {
  for (let i = 0; i < c.fg.length; i++) { if (map[c.fg[i]] !== undefined) c.fg[i] = map[c.fg[i]]; if (map[c.bg[i]] !== undefined) c.bg[i] = map[c.bg[i]]; }
  return c;
}

const KINDS = {
  road({ columns: w, rows: h, length }) {
    const c = vista('plains', { columns: w, rows: h, seed: 'road' }), horizon = Math.round(h * 0.45), end = length === 'short' ? horizon + Math.round((h - horizon) * 0.5) : horizon, vx = Math.round(w / 2);
    for (let y = h - 1; y > end; y--) {
      const t = (y - horizon) / Math.max(1, h - horizon), half = Math.max(1, Math.round(1 + t * w * 0.16));
      for (let x = vx - half; x <= vx + half; x++) c.set(x, y, solid(V.brown));
      glyph(c, vx - half, y, '▌', V.darkgray); glyph(c, vx + half, y, '▐', V.darkgray); if (y % 2) glyph(c, vx, y, '│', V.red);
    }
    if (length === 'short') { for (let y = end - 3; y <= end; y++) for (let x = vx - 3; x <= vx + 3; x++) c.set(x, y, solid(y === end - 3 ? V.red : V.brown)); for (let y = end - 2; y <= end; y++) c.set(vx, y, solid(V.black)); }
    return c;
  },
  iceplain({ columns: w, rows: h }) {
    const c = vista('plains', { columns: w, rows: h, weather: 'cold', seed: 'ice' }), horizon = Math.round(h * 0.45), r = rng('iceplain');
    const ramp = rampCells([V.white, V.brightcyan, V.cyan].slice(0, h - horizon > 6 ? 3 : 2));
    for (let y = horizon; y < h; y++) for (let x = 0; x < w; x++) c.set(x, y, ramp[clamp(Math.floor(((y - horizon) / Math.max(1, h - horizon)) * ramp.length), 0, ramp.length - 1)]);
    for (let i = 0; i < Math.round(w / 5); i++) glyph(c, r.int(w), horizon + 1 + r.int(Math.max(1, h - horizon - 1)), r.chance(0.5) ? '*' : '·', V.white);
    return c;
  },
  frozenpeaks({ columns: w, rows: h }) { return vista('mountains', { columns: w, rows: h, weather: 'snow', seed: 'frozen' }); },
  treetop({ columns: w, rows: h }) {
    const c = new Cells(w, h), r = rng('treetop'), sky = rampCells([V.blue, V.cyan], 2), top = Math.round(h * 0.5);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) c.set(x, y, y < top * 0.5 ? sky[0] : y < top ? sky[1] : solid(V.green));
    for (let x = 4; x < w; x += 9 + r.int(5)) for (let y = Math.max(1, top - 4 - r.int(3)); y < h; y++) { c.set(x, y, solid(V.brown)); c.set(x + 1, y, solid(V.red)); }
    for (let y = top; y < h; y++) for (let x = 0; x < w; x++) if (r.chance(0.35)) c.set(x, y, { ch: r.chance(0.5) ? '▒' : '░', fg: r.chance(0.5) ? V.brightgreen : V.black, bg: V.green });
    for (let x = 0; x < w; x++) { glyph(c, x, top, '♣', V.brightgreen); }
    for (let x = 2; x < w - 2; x++) { c.set(x, h - 4, { ch: '═', fg: V.brown, bg: V.green }); }
    return c;
  },
  sewer({ columns: w, rows: h }) {
    const c = vista('stone', { columns: w, rows: h, seed: 'sewer' }), y1 = Math.floor((h * 7) / 10), r = rng('sewer');
    recolor(c, { [V.gray]: V.green, [V.darkgray]: V.black });
    for (let y = y1 + 1; y < h; y++) { const t = (y - y1) / Math.max(1, h - y1), half = Math.round(w * (0.06 + 0.2 * t)); for (let x = Math.floor(w / 2) - half; x <= Math.floor(w / 2) + half; x++) c.set(x, y, { ch: r.chance(0.3) ? '≈' : ' ', fg: V.brightgreen, bg: V.green }); }
    for (let x = 2; x < w - 2; x += 7) glyph(c, x, y1 - 1, 'o', V.darkgray);
    return c;
  },
  hotroom({ columns: w, rows: h }) {
    const c = vista('stone', { columns: w, rows: h, seed: 'hot' }), r = rng('hot');
    recolor(c, { [V.gray]: V.red, [V.darkgray]: V.brown });
    for (let i = 0; i < Math.round(w / 3); i++) glyph(c, r.int(w), Math.floor(h * 0.35) + r.int(Math.ceil(h * 0.6)), r.chance(0.5) ? '~' : '^', V.yellow);
    for (let x = 0; x < w; x++) c.set(x, h - 1, { ch: '▄', fg: V.brightred, bg: V.red });
    return c;
  },
  iceroom({ columns: w, rows: h }) {
    const c = vista('stone', { columns: w, rows: h, seed: 'icy' }), r = rng('icy');
    recolor(c, { [V.gray]: V.cyan, [V.darkgray]: V.blue });
    for (let x = 1; x < w - 1; x += 3) for (let k = 0; k < 1 + r.int(3); k++) glyph(c, x, k, '▼', V.white);
    for (let i = 0; i < Math.round(w / 4); i++) glyph(c, r.int(w), 3 + r.int(Math.max(1, h - 4)), '*', V.white);
    return c;
  },
  maze({ columns: w, rows: h }) {
    const c = vista('stone', { columns: w, rows: h, seed: 'maze' }), x0 = Math.floor(w / 5), y0 = Math.floor(h / 6), y1 = Math.floor((h * 7) / 10), r = rng('maze');
    for (let d = 0; d < 3; d++) for (const side of [0, 1]) {
      if (!r.chance(0.7)) continue;
      const wide = Math.max(1, Math.round((x0 - 1) / (d + 1.5))), x = side ? w - 1 - wide - d * 0 - Math.round(d * x0 / 3) : Math.round(d * x0 / 3), top = Math.round(y0 + (y1 - y0) * (0.25 + d * 0.1));
      for (let yy = top; yy <= y1 - d; yy++) for (let xx = x; xx < x + wide; xx++) c.set(xx, yy, solid(V.black));
    }
    for (let y = y0 + 1; y <= y1; y++) for (let x = x0 + 1; x < w - x0 - 1; x++) if ((x + y) % 7 === 0) glyph(c, x, y, '?', V.darkgray);
    return c;
  },
  deck({ columns: w, rows: h }) {
    const c = new Cells(w, h), sea = Math.round(h * 0.4), deckY = Math.round(h * 0.55), sky = rampCells([V.blue, V.brightblue, V.cyan], 3), r = rng('deck');
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) c.set(x, y, y < sea ? sky[clamp(Math.floor((y / sea) * 3), 0, 2)] : y < deckY ? solid(V.blue) : solid(V.brown));
    for (let i = 0; i < w / 3; i++) glyph(c, r.int(w), sea + r.int(Math.max(1, deckY - sea)), '~', V.cyan);
    for (let y = deckY; y < h; y += 2) for (let x = 0; x < w; x++) glyph(c, x, y, '─', V.red);
    const mx = Math.round(w * 0.45); for (let y = 0; y < deckY + 2; y++) c.set(mx, y, solid(V.brown));
    for (let y = 1; y < deckY - 2; y++) for (let x = mx + 1; x < Math.min(w, mx + 2 + Math.round(w * 0.2)); x++) c.set(x, y, solid(V.white));
    for (let x = 0; x < w; x++) { c.set(x, deckY - 1, solid(V.red)); if (x % 4 === 0) for (let y = deckY - 3; y < deckY - 1; y++) c.set(x, y, solid(V.red)); }
    return c;
  },
  hold({ columns: w, rows: h }) {
    const c = vista('wooden', { columns: w, rows: h, seed: 'hold' }), r = rng('hold'), y1 = Math.floor((h * 7) / 10);
    recolor(c, { [V.red]: V.black });
    for (let x = Math.floor(w / 5) + 2; x < w - Math.floor(w / 5) - 4; x += 6) { glyph(c, x, y1 - 1, '■', V.brightred); glyph(c, x + 1, y1 - 1, '■', V.yellow); if (r.chance(0.5)) glyph(c, x, y1 - 2, '■', V.brown); }
    return c;
  },
  quarters({ columns: w, rows: h }) {
    const c = vista('wooden', { columns: w, rows: h, seed: 'quarters' }), x0 = Math.floor(w / 5), y1 = Math.floor((h * 7) / 10);
    for (let k = 0; k < 2; k++) for (let x = x0 + 2; x < x0 + 10; x++) c.set(x, y1 - 1 - k * 2, solid(k ? V.blue : V.white));
    glyph(c, Math.floor(w / 2), Math.floor(h / 4), '¥', V.yellow); glyph(c, Math.floor(w / 2), Math.floor(h / 4) - 1, '*', V.brightred);
    return c;
  },
  gundeck({ columns: w, rows: h, guns }) {
    const c = vista('wooden', { columns: w, rows: h, seed: 'guns' }), x0 = Math.floor(w / 5), y0 = Math.floor(h / 6), y1 = Math.floor((h * 7) / 10), n = Math.max(1, guns);
    recolor(c, { [V.red]: V.black });
    for (let i = 0; i < n; i++) { const side = i % 2, lane = Math.floor(i / 2), x = side ? w - x0 + 1 : 1, depth = lane / Math.max(1, Math.ceil(n / 2)), y = Math.round(y1 + (h - y1) * (0.55 - depth * 0.5)); glyph(c, x + (side ? 0 : 1), y, '▄', V.darkgray); glyph(c, x + (side ? 1 : 0), y, '▄', V.darkgray); glyph(c, x, y - 1, '■', V.black); glyph(c, side ? w - 2 : 2, y, '■', V.gray); }
    for (let x = x0 + 1; x < w - x0 - 1; x += 5) for (let y = y0 + 1; y < y0 + 3; y++) glyph(c, x, y, '│', V.brown);
    return c;
  },
  magazine({ columns: w, rows: h }) {
    const c = vista('wooden', { columns: w, rows: h, seed: 'magazine' }), y1 = Math.floor((h * 7) / 10), r = rng('magazine');
    recolor(c, { [V.brown]: V.black, [V.red]: V.black });
    for (let row = 0; row < 3; row++) for (let x = Math.floor(w / 5) + 1; x < w - Math.floor(w / 5) - 1; x += 3) glyph(c, x, y1 - row, r.chance(0.8) ? '■' : '□', V.brightred);
    glyph(c, Math.floor(w / 2), 2, '¥', V.yellow);
    return c;
  },
  starfield({ columns: w, rows: h }) {
    const c = new Cells(w, h), r = rng('starfield'), px = Math.round(w * 0.68), py = Math.round(h * 0.55), rx = Math.max(3, Math.round(w * 0.14)), ry = Math.max(2, Math.round(rx / 2));
    c.fill(solid(V.black)); overlay(c, 'stars', 'starfield', { density: 0.06 });
    for (let y = py - ry; y <= py + ry; y++) for (let x = px - rx; x <= px + rx; x++) if (((x - px) / rx) ** 2 + ((y - py) / ry) ** 2 <= 1) c.set(x, y, solid(((y - py + ry) % 3 === 1) ? V.brown : V.blue));
    for (let x = px - rx - 3; x <= px + rx + 3; x++) glyph(c, x, py + 1, '─', V.cyan);
    const mx = Math.round(w * 0.2), my = Math.round(h * 0.3); for (let y = my - 1; y <= my + 1; y++) for (let x = mx - 2; x <= mx + 2; x++) c.set(x, y, solid(V.gray));
    return c;
  },
};

/** A CoffeeMUD locale as a picture: `kind` from LOCALE_KINDS; `length` ('short' or 'long') for a road; `guns` (2, 4 or 6, or 'light', 'medium', 'heavy') for a gun deck. */
export function drawLocale({ kind = 'road', length = 'long', guns = 4, columns = 80, rows = 24 } = {}) {
  if (!KINDS[kind]) throw new Error(`unknown locale "${kind}"; known: ${LOCALE_KINDS.join(', ')}`);
  const w = clamp(Math.round(columns), 8, 160), h = clamp(Math.round(rows), 4, 45), count = GUN_DECKS[guns] ?? guns;
  const cells = KINDS[kind]({ columns: w, rows: h, length, guns: count });
  return { id: `locale.${kind}`, frames: [legalize(cells, { bgLimit: 8 })], fps: 0, loop: false };
}
