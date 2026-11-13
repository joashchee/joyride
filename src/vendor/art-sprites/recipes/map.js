// The level map with fog of war (T18, `map`). The game supplies the cells and what the party knows of each (`unknown`, `visible`, `remembered`, `explored`); the
// picture shows only what is known. An unknown cell is drawn the same whatever it holds, and a marker on one is not drawn, so the picture of a level cannot be
// read for what it has not shown. The state is told by a pattern as well as a tone (colour is never the only cue): visible is solid and bright with a lit top edge,
// explored solid and mid, remembered a dither.
import { Pixels } from '../pixels.js';
import { C, ramp } from '../palette.js';
import { pixelText } from './pixel-text.js';

export const MAP_STATES = ['unknown', 'visible', 'remembered', 'explored'];
export const MAP_KINDS = ['floor', 'wall', 'door', 'exit', 'hazard', 'pit'];
export const MAP_MARKERS = ['party', 'objective', 'boss', 'safe', 'trade', 'exit', 'teleport'];
export const MAP_SIZES = { mini: 4, expanded: 8 };
const GLYPH = {
  party: ['..x..', '.xxx.', 'xxxxx', '.xxx.', '..x..'], objective: ['..x..', '..x..', 'xxxxx', '..x..', '..x..'], boss: ['x...x', '.x.x.', '..x..', '.x.x.', 'x...x'],
  safe: ['.xxx.', 'x...x', 'x...x', 'x...x', '.xxx.'], trade: ['.xxx.', 'xx.xx', 'x.x.x', 'xx.xx', '.xxx.'], exit: ['xxxxx', '.xxx.', '.xxx.', '..x..', '..x..'], teleport: ['x.x.x', '.xxx.', 'xx.xx', '.xxx.', 'x.x.x'],
};
const SMALL = { party: ['.x.', 'xxx', '.x.'], objective: ['.x.', 'xxx', '.x.'], boss: ['x.x', '.x.', 'x.x'], safe: ['xxx', 'x.x', 'xxx'], trade: ['.x.', 'x.x', '.x.'], exit: ['xxx', '.x.', '.x.'], teleport: ['x.x', '.x.', 'x.x'] };
const MARK_INK = { party: 'white', objective: 'gold', boss: 'red', safe: 'green', trade: 'orange', exit: 'sky', teleport: 'violet' };
const INK = { white: C.white, gold: C.yellow, red: C.red, green: C.lime, orange: C.orange, sky: C.sky, violet: C.pink };
const TERRAIN = { floor: 'steel', door: 'wood', exit: 'gold', hazard: 'fire', pit: 'iron', wall: 'stone' };
const LEGEND = { floor: 'floor', wall: 'wall', door: 'door', exit: 'stairs', hazard: 'hazard', pit: 'pit', party: 'the party', objective: 'objective', boss: 'boss', safe: 'safe place', trade: 'trading post', teleport: 'teleport' };

function cellFill(p, x0, y0, c, kind, state) {
  const rp = ramp(TERRAIN[kind]), tone = kind === 'wall' ? 1 : state === 'visible' ? 3 : state === 'explored' ? 2 : 1;
  const col = rp[tone];
  for (let y = 0; y < c; y++) for (let x = 0; x < c; x++) {
    let v = col;
    if (state === 'remembered' && (x + y) % 2) v = rp[0];
    if (state === 'visible' && y === 0 && kind !== 'wall') v = rp[3 === tone ? 3 : tone + 1];
    if (state === 'explored' && c >= 8 && x === c >> 1 && y === c >> 1) v = rp[3];
    p.set(x0 + x, y0 + y, v);
  }
}

/**
 * The level map. `cells` is `[{ x, y, state, kind }]` for the whole level (the game says what the party has seen); `party` is `[x, y]`; `markers` is
 * `[{ kind, x, y }]` with kind objective, boss, safe, trade, exit or teleport; `size` `mini` (4 pixels a cell) or `expanded` (8); `depth` (0 to 9) is shown as pips and
 * `progress` (0 to 1) as a bar, with a frame. `label` (a short text such as `3` or `B2`, up to 6 characters of CP437) is the floor number and the depth badge in words: it is set in the VGA font on a line of its own above the pips, so it needs `atlas` (`loadAtlas` from `art-sprites/atlas`) and the header grows by 16 pixels; without a `label` nothing changes. Returns `mask` (1 where the fog is), the `legend` (only of what is shown) and `alt`.
 */
export function drawMap({ cells = [], party, markers = [], size = 'mini', depth = 0, progress = 0, frame = true, label, atlas } = {}) {
  const c = MAP_SIZES[size]; if (!c) throw new Error(`unknown map size "${size}"; known: ${Object.keys(MAP_SIZES).join(', ')}`);
  if (!cells.length) throw new Error('a map needs cells');
  for (const k of cells) {
    if (!MAP_STATES.includes(k.state ?? 'unknown')) throw new Error(`unknown map state "${k.state}"; known: ${MAP_STATES.join(', ')}`);
    if (k.kind !== undefined && !MAP_KINDS.includes(k.kind)) throw new Error(`unknown map kind "${k.kind}"; known: ${MAP_KINDS.join(', ')}`);
  }
  for (const m of markers) if (!MAP_MARKERS.includes(m.kind) || m.kind === 'party') throw new Error(`unknown marker "${m.kind}"; known: ${MAP_MARKERS.filter((k) => k !== 'party').join(', ')}`);
  if (label !== undefined && ![...String(label)].length) throw new Error('a map label is at least one character');
  const x0 = Math.min(...cells.map((k) => k.x)), y0 = Math.min(...cells.map((k) => k.y)), cols = Math.max(...cells.map((k) => k.x)) - x0 + 1, rows = Math.max(...cells.map((k) => k.y)) - y0 + 1;
  const pad = frame ? 3 : 0, text = frame && label !== undefined ? pixelText(atlas, String(label), { ink: C.white, outline: null }) : null, head = frame ? 7 + (text ? 16 : 0) : 0, W = cols * c + 2 * pad, H = rows * c + 2 * pad + head, p = new Pixels(W, H), mask = new Pixels(W, H);
  if (frame) { p.rect(0, 0, W, H, C.night).frame(0, 0, W, H, ramp('steel')[1]).frame(1, 1, W - 2, H - 2, ramp('steel')[2]); for (const [x, y] of [[0, 0], [W - 1, 0], [0, H - 1], [W - 1, H - 1]]) p.set(x, y, ramp('steel')[3]); }
  else p.rect(0, 0, W, H, C.night);
  const at = (x, y) => [pad + (x - x0) * c, pad + head + (y - y0) * c], known = new Map(), shown = new Set();
  for (const k of cells) {
    const state = k.state ?? 'unknown', [px, py] = at(k.x, k.y);
    if (state === 'unknown') { mask.rect(px, py, c, c, 1); continue; }
    known.set(`${k.x},${k.y}`, { ...k, state, kind: k.kind ?? 'floor' });
  }
  for (const k of known.values()) { const [px, py] = at(k.x, k.y); cellFill(p, px, py, c, k.kind, k.state); shown.add(k.kind); }
  // Edges: the dark line between a known wall and a known open cell. Nothing is drawn toward an unknown cell, so it reads the same whatever is there.
  for (const k of known.values()) {
    if (k.kind !== 'wall') continue;
    const [px, py] = at(k.x, k.y);
    for (const [dx, dy] of [[0, -1], [1, 0], [0, 1], [-1, 0]]) {
      const n = known.get(`${k.x + dx},${k.y + dy}`); if (!n || n.kind === 'wall') continue;
      if (dx) p.rect(dx > 0 ? px + c - 1 : px, py, 1, c, C.night); else p.rect(px, dy > 0 ? py + c - 1 : py, c, 1, C.night);
    }
  }
  const glyph = (kind, k) => { const rows = c >= 8 ? GLYPH[kind] : SMALL[kind], n = rows.length, [px, py] = at(k.x, k.y), ox = px + ((c - n) >> 1), oy = py + ((c - n) >> 1); rows.forEach((row, j) => [...row].forEach((ch, i) => { if (ch === 'x') p.set(ox + i, oy + j, INK[MARK_INK[kind]]); })); if (c >= 8) rows.forEach((row, j) => [...row].forEach((ch, i) => { if (ch === 'x') for (const [a, b] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) if (rows[j + b]?.[i + a] !== 'x' && !p.get(ox + i + a, oy + j + b)) p.set(ox + i + a, oy + j + b, C.night); })); shown.add(kind); };
  for (const m of markers) if (known.has(`${m.x},${m.y}`)) glyph(m.kind, m);
  if (party) glyph('party', { x: party[0], y: party[1] });
  if (frame) {
    const top = text ? 16 : 0, pips = Math.max(0, Math.min(9, depth | 0));
    if (text) { if (text.w > W - 2 * pad - 2) throw new Error(`the label "${label}" is wider than the map; use a shorter label or a bigger map`); p.blit(text, pad + 1, 2); }
    for (let i = 0; i < pips; i++) p.rect(pad + 1 + i * 3, 2 + top, 2, 2, ramp('gold')[3]);
    const bw = W - 2 * pad - 2, fill = Math.round(bw * Math.max(0, Math.min(1, progress)));
    p.rect(pad + 1, 5 + top, bw, 1, ramp('steel')[0]).rect(pad + 1, 5 + top, fill, 1, ramp('green')[3]);
  }
  const legend = [...shown].map((k) => `${LEGEND[k]}`), total = cells.length, seen = known.size;
  return { id: 'map', frames: [p], fps: 0, loop: false, spec: { size, depth, progress, frame }, projection: 'top-down', mask, legend, alt: `Level map, ${seen} of ${total} cells known${party ? `, party at ${party[0]},${party[1]}` : ''}`, cell: c, origin: [x0, y0], anchors: { root: party ? [at(party[0], party[1])[0] + (c >> 1), at(party[0], party[1])[1] + (c >> 1)] : [0, 0] } };
}
