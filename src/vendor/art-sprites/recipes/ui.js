// The CoffeeMUD client surfaces the menu screen does not cover (ANSI only, 80 columns, classic-legal): a status bar, a map pane and a
// character sheet. They take data (numbers, names, a seed), draw boxes and meters in the VGA ramp and say nothing a client could not show.
import { Cells, legalize } from '../cells.js';
import { V } from '../palette-ansi.js';
import { rng } from '../rng.js';

const art = (id, cells) => ({ id, frames: [legalize(cells, { bgLimit: 8 })], fps: 0, loop: false });
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
/** A meter: filled blocks in `fg`, the rest as light shade, `width` cells wide. */
function meter(c, x, y, width, value, max, fg) {
  const full = Math.round(clamp(value / Math.max(1, max), 0, 1) * width);
  for (let i = 0; i < width; i++) c.set(x + i, y, i < full ? { ch: '█', fg, bg: V.black } : { ch: '░', fg: V.darkgray, bg: V.black });
}

/** The prompt bar every MUD client shows: name and level, then hit points, mana and movement as meters with their numbers. */
export function drawBar({ name = 'Aldric', level = 12, hp = 74, hpMax = 100, mana = 31, manaMax = 60, move = 90, moveMax = 120 } = {}) {
  const c = new Cells(80, 5).fill({ ch: ' ', bg: V.black });
  c.box(0, 0, 80, 5, { style: 'single', fg: V.blue });
  c.text(2, 0, ` ${name}  level ${level} `, V.brightcyan, V.black);
  [['HP', hp, hpMax, V.brightred, 2], ['MN', mana, manaMax, V.brightblue, 28], ['MV', move, moveMax, V.brightgreen, 54]].forEach(([label, v, max, fg, x]) => {
    c.text(x, 2, label, V.gray, V.black); meter(c, x + 3, 2, 14, v, max, fg); c.text(x + 3, 3, `${v}/${max}`.padEnd(14), V.white, V.black);
  });
  return art('bar', c);
}

/** A map pane: rooms as small boxes joined by passages, the room you stand in marked `@`, drawn on a seeded walk so the same seed is the same map. */
export function drawMappane({ seed = 'map', columns = 31, rows = 15 } = {}) {
  const w = clamp(Math.round(columns), 11, 80), h = clamp(Math.round(rows), 7, 24), c = new Cells(w, h).fill({ ch: ' ', bg: V.black }), r = rng(`mappane:${seed}`);
  c.box(0, 0, w, h, { style: 'single', fg: V.blue }); c.text(2, 0, ' Map ', V.brightcyan, V.black);
  const gx = Math.floor((w - 2) / 4), gy = Math.floor((h - 2) / 2), at = new Set(), path = [];
  let x = Math.floor(gx / 2), y = Math.floor(gy / 2);
  for (let i = 0; i < Math.min(14, gx * gy); i++) { const k = `${x},${y}`; if (!at.has(k)) { at.add(k); path.push([x, y]); } const [dx, dy] = r.pick([[1, 0], [-1, 0], [0, 1], [0, -1]]); x = clamp(x + dx, 0, gx - 1); y = clamp(y + dy, 0, gy - 1); }
  const cx = (gxn) => 2 + gxn * 4, cy = (gyn) => 1 + gyn * 2;
  path.forEach(([px, py], i) => {
    if (i) { const [qx, qy] = path[i - 1]; if (Math.abs(px - qx) + Math.abs(py - qy) === 1) { if (px !== qx) c.set((cx(px) + cx(qx)) / 2 + 1, cy(py), { ch: '─', fg: V.gray, bg: V.black }); else c.set(cx(px) + 1, (cy(py) + cy(qy)) / 2, { ch: '│', fg: V.gray, bg: V.black }); } }
  });
  path.forEach(([px, py], i) => c.text(cx(px), cy(py), i === path.length - 1 ? '[@]' : '[ ]', i === path.length - 1 ? V.brightyellow : V.cyan, V.black));
  return art('mappane', c);
}

const STATS = ['STR', 'INT', 'WIS', 'DEX', 'CON', 'CHA'], SLOTS = ['head', 'body', 'hands', 'legs', 'feet', 'wielded', 'held', 'worn on finger'];
/** The character sheet: name, race and class, the six stats as meters, and the equipment slots. */
export function drawSheet({ name = 'Aldric', race = 'human', cls = 'fighter', level = 12, stats = [18, 10, 11, 14, 16, 9], worn = ['iron helm', 'chain mail', 'gauntlets', 'greaves', 'boots', 'longsword', 'shield', 'silver ring'] } = {}) {
  const c = new Cells(80, 24).fill({ ch: ' ', bg: V.black });
  c.box(0, 0, 80, 24, { style: 'double', fg: V.cyan }); c.text(3, 1, name, V.brightcyan, V.black); c.text(3, 2, `${race} ${cls}, level ${level}`, V.gray, V.black);
  c.box(2, 4, 34, 9, { style: 'single', fg: V.blue }); c.text(4, 4, ' Abilities ', V.brightcyan, V.black);
  STATS.forEach((s, i) => { c.text(4, 5 + i, s, V.gray, V.black); meter(c, 9, 5 + i, 20, stats[i] ?? 10, 25, V.brightgreen); c.text(31, 5 + i, String(stats[i] ?? 10).padStart(2), V.white, V.black); });
  c.box(40, 4, 38, 10, { style: 'single', fg: V.blue }); c.text(42, 4, ' Equipment ', V.brightcyan, V.black);
  SLOTS.forEach((s, i) => { c.text(42, 5 + i, `<${s}>`.padEnd(18), V.gray, V.black); c.text(61, 5 + i, String(worn[i] ?? '(nothing)').slice(0, 16), worn[i] ? V.white : V.darkgray, V.black); });
  c.text(3, 21, 'Type HELP for commands.', V.darkgray, V.black);
  return art('sheet', c);
}
