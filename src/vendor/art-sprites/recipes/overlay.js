// The overlay for an item or a slot (T11): a transparent layer the game draws over an icon. A rarity frame, state markers (new, equipped, locked,
// bound, crafted, enchanted, compat, socket, better, worse, same), a durability pip row and a stack count. Colour is never the only cue (Tag Team 9):
// every rarity has its own frame shape, every marker its own shape, and the durability pips are filled or hollow. Drawn on a 16 grid and scaled by
// whole pixels for `size: 32`; the stack count is the VGA font at its own 8 by 16, so it needs `size: 32` and the atlas (`draw(..., { atlas })`).
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { pixelText } from './pixel-text.js';

export const RARITIES = { common: 'stone', uncommon: 'green', rare: 'blue', epic: 'purple', legendary: 'gold' };
export const RARITY_IDS = Object.keys(RARITIES);

const FRAMES = {
  common: (m) => { m.frame(0, 0, 16, 16, 1); },
  uncommon: (m) => { m.frame(0, 0, 16, 16, 1); for (const [x, y] of [[0, 0], [15, 0], [0, 15], [15, 15]]) m.set(x, y, 0); },
  rare: (m) => { m.frame(0, 0, 16, 16, 1); for (const [x, y, sx, sy] of [[2, 2, 1, 1], [13, 2, -1, 1], [2, 13, 1, -1], [13, 13, -1, -1]]) { m.rect(x, y, 1, 1, 2); for (let i = 1; i < 3; i++) { m.set(x + sx * i, y, 2); m.set(x, y + sy * i, 2); } } },
  epic: (m) => { m.frame(0, 0, 16, 16, 1); for (const [x, y] of [[0, 0], [14, 0], [0, 14], [14, 14]]) m.rect(x, y, 2, 2, 2); },
  legendary: (m) => { m.frame(0, 0, 16, 16, 1); for (const [x, y] of [[1, 1], [14, 1], [1, 14], [14, 14]]) { m.set(x, y, 2); m.set(x - 1, y, 2); m.set(x + 1, y, 2); m.set(x, y - 1, 2); m.set(x, y + 1, 2); } m.rect(6, 0, 4, 2, 2); m.set(7, 0, 3).set(8, 0, 3); },
};
const rarityFrame = (name) => { const m = new Pixels(16, 16); FRAMES[name](m); const r = ramp(RARITIES[name]); return m.map((x, y) => { const v = m.get(x, y); return v === 1 ? r[2] : v === 2 ? r[3] : v === 3 ? C.white : 0; }); };

/** Markers, 5 by 5: `X` the ink, `o` the accent. Each has its own shape and a home corner. */
export const MARKS = {
  new: { at: 'tl', ink: 'gold', accent: 'white', rows: ['..X..', '..X..', 'XXoXX', '..X..', '..X..'] },
  equipped: { at: 'bl', ink: 'green', accent: 'white', rows: ['....X', '...XX', 'X.XX.', 'XXX..', '.X...'] },
  locked: { at: 'br', ink: 'steel', accent: 'iron', rows: ['.XXX.', 'X...X', 'XXXXX', 'XXoXX', 'XXXXX'] },
  bound: { at: 'tr', ink: 'gold', accent: 'bone', rows: ['XXX..', 'X.X..', 'XXXXX', '..X.X', '..XXX'] },
  crafted: { at: 'tl', ink: 'wood', accent: 'steel', rows: ['ooooo', 'ooooo', '..X..', '..X..', '..X..'] },
  enchanted: { at: 'tr', ink: 'purple', accent: 'white', rows: ['X.X.X', '.XXX.', 'XXoXX', '.XXX.', 'X.X.X'] },
  compat: { at: 'bl', ink: 'blue', accent: 'white', rows: ['.XXX.', 'X...X', 'X.o.X', 'X...X', '.XXX.'] },
  socket: { at: 'br', ink: 'iron', accent: 'stone', rows: ['.XXX.', 'X...X', 'X...X', 'X...X', '.XXX.'] },
  better: { at: 'tr', ink: 'green', accent: 'white', rows: ['..X..', '.XXX.', 'XX.XX', 'X...X', '.....'] },
  worse: { at: 'tr', ink: 'red', accent: 'white', rows: ['.....', 'X...X', 'XX.XX', '.XXX.', '..X..'] },
  same: { at: 'tr', ink: 'steel', accent: 'iron', rows: ['.....', 'XXXXX', '.....', 'XXXXX', '.....'] },
};
export const MARK_IDS = Object.keys(MARKS);
const CORNERS = { tl: [0, 0], tr: [9, 0], bl: [0, 9], br: [9, 9] }; // the 7 by 7 outlined marker, with the frame's pixel under its edge
const ORDER = ['tl', 'tr', 'bl', 'br'];

function markerAt(name) {
  const k = MARKS[name], m = Pixels.fromRows(k.rows, { X: 1, o: 2 }), big = new Pixels(7, 7).blit(m, 1, 1);
  return shade(big, { 1: ramp(k.ink), 2: ramp(k.accent) });
}

const scaleUp = (px, n) => { if (n === 1) return px; const big = new Pixels(px.w * n, px.h * n); return big.map((x, y) => px.get(Math.floor(x / n), Math.floor(y / n))); };

/**
 * One overlay: `rarity` (a frame), `marks` (a list of marker names), `durability` (0 to 1, five pips, filled or hollow) and `count` (a stack count, VGA
 * digits, `size: 32` and the atlas only). `size` 16 or 32. Without a `rarity` there is no frame.
 */
export function drawOverlay({ rarity, marks = [], durability, count, size = 16 } = {}, { atlas } = {}) {
  if (size !== 16 && size !== 32) throw new Error(`unknown overlay size "${size}"; known: 16, 32`);
  if (rarity !== undefined && !FRAMES[rarity]) throw new Error(`unknown rarity "${rarity}"; known: ${RARITY_IDS.join(', ')}`);
  for (const k of marks) if (!MARKS[k]) throw new Error(`unknown marker "${k}"; known: ${MARK_IDS.join(', ')}`);
  if (durability !== undefined && !(durability >= 0 && durability <= 1)) throw new Error('durability is from 0 to 1');
  if (count !== undefined && size !== 32) throw new Error('a stack count needs size 32: the digits are the 8 by 16 font');
  const n = size / 16, out = new Pixels(16, 16), free = [...ORDER], at = {};
  if (rarity) out.blit(rarityFrame(rarity));
  if (count !== undefined) free.splice(free.indexOf('br'), 1); // the count owns the bottom right
  for (const k of marks) { // each marker takes its home corner, else the next free one
    const home = MARKS[k].at, corner = free.includes(home) ? home : free[0];
    if (!corner) throw new Error(`no free corner for marker "${k}": a corner holds one marker (four at most)`);
    free.splice(free.indexOf(corner), 1); at[k] = corner;
    out.blit(markerAt(k), ...CORNERS[corner]);
  }
  if (durability !== undefined) {
    const filled = Math.ceil(durability * 5), r = ramp(durability > 0.5 ? 'green' : durability > 0.2 ? 'gold' : 'red');
    for (let i = 0; i < 5; i++) { out.set(3 + i * 2, 14, i < filled ? r[3] : C.night); }
  }
  let big = scaleUp(out, n);
  if (count !== undefined) {
    const t = pixelText(atlas, String(count), { ink: C.white, outline: C.night });
    big = new Pixels(32, 32).blit(big).blit(t, 32 - t.w, 32 - t.h);
  }
  const id = ['overlay', rarity, ...marks].filter(Boolean).join('.');
  return { id, frames: [big], fps: 0, loop: false, spec: { rarity, marks, durability, count, size }, corners: at };
}
