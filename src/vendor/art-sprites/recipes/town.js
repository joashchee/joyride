// The building grammar and the town (T20). One grammar, not fifteen drawings: `{ shell, roof, theme, serviceIcon }` draws any of Tag Team's services, the shell being the
// footprint (cottage, hall, tower, vault, stall, wing), the theme a roof and wall colour, the service an `icon` glyph. The front view hangs the icon as a sign over the
// building (32 by 48); the top view paints it on the roof plan (32 by 32). A state (`locked`, `available`, `new`, `upgrade`) is a shape as well as a tone. `drawTown`
// lays a list of buildings on streets, with lamps and trees, and returns the hotspots and the walkable cells.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { drawIcon } from './icons.js';
import { groundTile } from './tiles.js';
import { drawTopProp } from './topprops.js';

export const SHELLS = ['cottage', 'hall', 'tower', 'vault', 'stall', 'wing'];
export const THEMES = { village: { roof: 'red', walls: 'wood' }, stone: { roof: 'steel', walls: 'stone' }, arcane: { roof: 'purple', walls: 'white' }, market: { roof: 'gold', walls: 'sand' } };
export const BUILDING_STATES = ['locked', 'available', 'new', 'upgrade'];
/** Tag Team's services: the shell, theme and icon glyph each takes by default (any can be overridden). `marker` is the generic one. */
export const SERVICES = {
  registry: { shell: 'hall', glyph: 'scroll', theme: 'stone' }, atelier: { shell: 'cottage', glyph: 'spark', theme: 'market' }, inn: { shell: 'wing', glyph: 'moon', theme: 'village' },
  training: { shell: 'hall', glyph: 'sword', theme: 'stone' }, archive: { shell: 'vault', glyph: 'book', theme: 'stone' }, blacksmith: { shell: 'cottage', glyph: 'hammer', theme: 'village' },
  alchemist: { shell: 'cottage', glyph: 'flask', theme: 'arcane' }, store: { shell: 'stall', glyph: 'chest', theme: 'market' }, trading: { shell: 'stall', glyph: 'coins', theme: 'market' },
  bank: { shell: 'vault', glyph: 'coin', theme: 'stone' }, portal: { shell: 'tower', glyph: 'burst', theme: 'arcane' }, chronicle: { shell: 'hall', glyph: 'chronicle', theme: 'stone' },
  fame: { shell: 'hall', glyph: 'trophy', theme: 'market' }, research: { shell: 'tower', glyph: 'research', theme: 'arcane' }, museum: { shell: 'hall', glyph: 'medal', theme: 'stone' },
  marker: { shell: 'cottage', glyph: 'target', theme: 'stone' },
};
/** How the streets run: `lattice` (a street between and round every building), `rows` (the streets along each row of buildings, joined at the two ends) or `cross` (the rows, joined by one street down the middle). */
export const TOWN_PATHS = ['lattice', 'rows', 'cross'];
export const SERVICE_IDS = Object.keys(SERVICES);

const win = (m, x, y) => { m.rect(x, y, 3, 4, 5); m.rect(x + 1, y, 1, 4, 3); };
// Front view, in the 32 by 32 box (drawn 16 rows down on the canvas). Part 1 walls, 2 roof, 3 stone and trim, 4 door and wood, 5 glass.
const FRONT = {
  cottage: (m) => { m.rect(5, 15, 22, 15, 1); m.poly([[3, 16], [16, 6], [29, 16]], 2); m.rect(13, 22, 6, 8, 4); win(m, 7, 19); win(m, 22, 19); m.rect(21, 7, 3, 6, 3); },
  hall: (m) => { m.rect(2, 13, 28, 17, 1); m.poly([[0, 14], [7, 5], [25, 5], [32, 14]], 2); m.rect(12, 20, 8, 10, 4); m.rect(11, 18, 10, 2, 3); win(m, 4, 18); win(m, 25, 18); win(m, 4, 24); win(m, 25, 24); m.rect(2, 13, 28, 1, 3); },
  tower: (m) => { m.rect(9, 8, 14, 22, 1); for (const x of [9, 13, 17, 20]) m.rect(x, 4, 3, 5, 1); m.rect(8, 9, 16, 2, 3); m.rect(14, 22, 4, 8, 4); win(m, 15, 13); },
  vault: (m) => { m.rect(3, 12, 26, 18, 1); m.rect(2, 10, 28, 3, 3); m.rect(11, 18, 10, 12, 4); for (const x of [13, 16, 19]) m.rect(x, 18, 1, 12, 3); m.rect(11, 22, 10, 1, 3); win(m, 5, 16); win(m, 24, 16); },
  stall: (m) => { m.rect(4, 14, 2, 16, 4).rect(26, 14, 2, 16, 4); m.poly([[2, 14], [30, 14], [28, 7], [4, 7]], 2); for (const x of [5, 11, 17, 23]) m.rect(x, 7, 3, 7, 3); m.rect(4, 24, 24, 6, 4); m.rect(8, 21, 3, 3, 5).rect(14, 21, 3, 3, 3).rect(20, 21, 3, 3, 5); },
  wing: (m) => { m.rect(1, 16, 30, 14, 1); m.poly([[0, 17], [4, 8], [28, 8], [32, 17]], 2); m.rect(6, 22, 5, 8, 4); m.rect(21, 22, 5, 8, 4); win(m, 13, 20); win(m, 3, 19); win(m, 28, 19); },
};
// Top view: a roof plan. Part 1 is the lit slope, 2 the slope in shade, 3 stone and trim, 4 wood, 5 glass.
const TOP = {
  cottage: (m) => { m.rect(5, 6, 22, 10, 1).rect(5, 16, 22, 10, 2).rect(5, 15, 22, 2, 3); m.rect(21, 8, 3, 3, 3); },
  hall: (m) => { m.rect(2, 4, 28, 12, 1).rect(2, 16, 28, 12, 2).rect(2, 15, 28, 2, 3); m.rect(14, 28, 4, 3, 4); },
  tower: (m) => { m.rect(6, 6, 20, 20, 3).rect(9, 9, 14, 14, 1).rect(9, 16, 14, 7, 2); for (const [x, y] of [[4, 4], [24, 4], [4, 24], [24, 24], [14, 4], [14, 25], [4, 14], [25, 14]]) m.rect(x, y, 4, 4, 3); },
  vault: (m) => { m.rect(3, 3, 26, 26, 3).rect(6, 6, 20, 20, 1).rect(6, 6, 20, 2, 2).rect(13, 13, 6, 6, 5); },
  stall: (m) => { for (let i = 0; i < 7; i++) m.rect(2 + i * 4, 3, 4, 19, i % 2 ? 3 : 2); m.rect(3, 23, 26, 5, 4); m.rect(7, 24, 3, 3, 5).rect(15, 24, 3, 3, 3).rect(22, 24, 3, 3, 5); },
  wing: (m) => { m.rect(1, 7, 30, 8, 1).rect(1, 15, 30, 8, 2).rect(1, 14, 30, 2, 3).rect(6, 23, 4, 3, 4).rect(21, 23, 4, 3, 4); },
};

/** The state markers: a pennant (new: a plus, upgrade: an up chevron) or a padlock over a dithered building (locked). Shapes differ, not just tones. */
function stateMark(px, state, f = 0) {
  if (state === 'available' || state === undefined) return px;
  const W = px.w;
  if (state === 'locked') {
    for (let y = 0; y < px.h; y++) for (let x = 0; x < W; x++) if (px.get(x, y) && (x + y) % 2 === 0) px.set(x, y, C.slate);
    const g = ramp('gold'), cx = (W >> 1) - 3, cy = (px.h >> 1) - 1;
    px.rect(cx + 1, cy - 4, 1, 4, C.night).rect(cx + 4, cy - 4, 1, 4, C.night).rect(cx + 2, cy - 5, 2, 1, C.night);
    px.rect(cx, cy, 6, 6, C.night).rect(cx + 1, cy + 1, 4, 4, g[2]).rect(cx + 2, cy + 2, 2, 2, C.night);
    if (f) px.set(cx + (f === 1 ? 1 : f === 2 ? 4 : 1), cy + (f === 3 ? 4 : 1), C.white); // the glint on the padlock, going round
    return px;
  }
  const x0 = W - 11, flag = state === 'new' ? ramp('green') : ramp('gold');
  px.rect(x0, 0, 1, 11, C.night);
  if (state === 'new') {
    // The pennant ripples: its lower half slides a pixel either way along the loop.
    const w = [0, 1, 0, -1][f & 3];
    px.rect(x0 + 1, 1, 7, 3, C.night).rect(x0 + 1 + w, 4, 7, 3, C.night).rect(x0 + 1, 2, 6, 2, flag[2]).rect(x0 + 1 + w, 4, 6, 2, flag[2]);
    px.rect(x0 + 3, 2, 2, 2, C.white).rect(x0 + 2, 3, 4, 1, C.white).rect(x0 + 3 + w, 4, 2, 2, C.white).rect(x0 + 2 + w, 4, 4, 1, C.white);
  } else {
    const b = [0, -1, 0, 1][f & 3]; // the chevron bobs up and down
    px.rect(x0 + 1, 1, 7, 6, C.night).rect(x0 + 1, 2, 6, 4, flag[2]);
    px.set(x0 + 3, 2 + b, C.white).set(x0 + 2, 3 + b, C.white).set(x0 + 4, 3 + b, C.white).set(x0 + 1, 4 + b, C.white).set(x0 + 5, 4 + b, C.white); px.rect(x0 + 2, 8, 3, 2, flag[3]);
  }
  return px;
}

/**
 * One building from the grammar. `service` (see `SERVICES`) gives a default `shell`, `theme` and icon `glyph`; `shell`, `theme`, `roof` (a ramp name) and `glyph` override.
 * `view` `front` (32 by 48: the icon hangs as a sign above) or `top` (32 by 32: the icon on the roof). `state` is `locked`, `available`, `new` or `upgrade`.
 * `animate: true` gives a four-frame loop to the states that have a marker (the pennant of `new` ripples, the chevron of `upgrade` bobs, the padlock of `locked` glints); `available` stays one still frame.
 */
export function drawService({ service = 'marker', shell, theme, roof, glyph, state = 'available', view = 'front', seed = 'service', animate = false } = {}) {
  const base = SERVICES[service]; if (!base) throw new Error(`unknown service "${service}"; known: ${SERVICE_IDS.join(', ')}`);
  shell ??= base.shell; theme ??= base.theme; glyph ??= base.glyph;
  if (!SHELLS.includes(shell)) throw new Error(`unknown shell "${shell}"; known: ${SHELLS.join(', ')}`);
  if (!THEMES[theme]) throw new Error(`unknown theme "${theme}"; known: ${Object.keys(THEMES).join(', ')}`);
  if (!BUILDING_STATES.includes(state)) throw new Error(`unknown building state "${state}"; known: ${BUILDING_STATES.join(', ')}`);
  if (view !== 'front' && view !== 'top') throw new Error(`unknown building view "${view}"; known: front, top`);
  const front = view === 'front', t = THEMES[theme], rr = ramp(roof ?? t.roof), r = rng(`${seed}:${service}`), icon = drawIcon({ glyph, plate: theme === 'arcane' ? 'purple' : theme === 'market' ? 'green' : 'blue' }).frames[0];
  const m = new Pixels(32, 32); (front ? FRONT : TOP)[shell](m);
  const parts = front ? { 1: ramp(t.walls), 2: rr, 3: ramp('stone'), 4: ramp('wood'), 5: ramp('glass') } : { 1: rr, 2: [rr[0], rr[0], rr[1], rr[1]], 3: ramp('stone'), 4: ramp('wood'), 5: ramp('glass') };
  const body = shade(m, parts);
  // A little seeded grain on the roof so two of a kind do not look cut from one stamp.
  const roofCells = []; for (let y = 0; y < 32; y++) for (let x = 0; x < 32; x++) if (m.get(x, y) === (front ? 2 : 1)) roofCells.push([x, y]);
  if (roofCells.length) for (let i = 0; i < 4; i++) { const [x, y] = r.pick(roofCells); body.set(x, y, rr[Math.min(3, 1 + (i & 1))]); }
  const out = front ? new Pixels(32, 48).blit(body, 0, 16) : body;
  if (front) { out.rect(10, 15, 1, 5, C.slate).rect(21, 15, 1, 5, C.slate); out.blit(icon, 8, 0); } else out.blit(icon, 8, 8);
  const moving = animate && state !== 'available', frames = (moving ? [0, 1, 2, 3] : [0]).map((f) => stateMark(out.clone(), state, f));
  const h = out.h, entry = front ? [16, h - 1] : [16, 31];
  return { id: `building.${service}`, frames, fps: moving ? 4 : 0, loop: moving, projection: front ? 'front' : 'top-down', spec: { service, shell, theme, glyph, state, view, ...(moving ? { animate } : {}) }, anchors: { root: front ? [16, h - 2] : [16, 16], door: entry, ...(front ? { sign: [16, 8] } : {}) }, hotspot: { rect: [0, front ? 16 : 0, 32, 32], entry }, alt: `${service === 'marker' ? 'A building' : `The ${service}`}, ${shell}${state === 'available' ? '' : `, ${state}`}` };
}

/**
 * The town, seen from above. `buildings` is `[{ service, state?, cell?: [column, row] }]` (the game says which are there and in what state); a building without a `cell` takes
 * the next free slot, row by row. The slots form a lattice with a street between and around them; the lattice gets a lamp at each crossing and a tree in each empty slot. Returns the
 * `hotspots` (`id`, `service`, `state`, `rect`, `entry` on the street below the door), the `grid` of 32 pixel cells (`#` building or tree, `.` street, `,` grass) and `alt`.
 */
export function drawTown({ buildings = [], seed = 'town', cols, rows, paths = 'lattice' } = {}) {
  if (!buildings.length) throw new Error('a town needs buildings');
  if (!TOWN_PATHS.includes(paths)) throw new Error(`unknown town paths "${paths}"; known: ${TOWN_PATHS.join(', ')}`);
  const n = buildings.length; cols ??= Math.max(...buildings.map((b) => (b.cell?.[0] ?? -1) + 1), Math.ceil(Math.sqrt(n))); rows ??= Math.max(...buildings.map((b) => (b.cell?.[1] ?? -1) + 1), Math.ceil(n / cols));
  const taken = new Map(), slots = []; for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) slots.push([x, y]);
  buildings.forEach((b, i) => { if (b.cell) { const k = b.cell.join(','); if (taken.has(k)) throw new Error(`two buildings in cell ${k}`); taken.set(k, i); } });
  let next = 0; buildings.forEach((b, i) => { if (b.cell) return; while (taken.has(slots[next]?.join(','))) next++; if (!slots[next]) throw new Error('the town has no free slot'); taken.set(slots[next].join(','), i); });
  const GW = 2 * cols + 1, GH = 2 * rows + 1, W = GW * 32, H = GH * 32, out = new Pixels(W, H), rand = rng(`${seed}:town`);
  const grass = [groundTile('grass', `${seed}:a`), groundTile('grass', `${seed}:b`)], dirt = [groundTile('dirt', `${seed}:c`), groundTile('dirt', `${seed}:d`)];
  // Every building's door opens on the even row below it, so the rows always stay streets; `paths` decides which of the even columns join them.
  const spine = 2 * (cols >> 1), column = (gx) => paths === 'lattice' || gx === 0 || gx === GW - 1 ? true : paths === 'cross' && gx === spine;
  const street = (gx, gy) => gy % 2 === 0 || (gx % 2 === 0 && column(gx));
  const grid = Array.from({ length: GH }, () => Array(GW).fill(','));
  for (let gy = 0; gy < GH; gy++) for (let gx = 0; gx < GW; gx++) {
    const s = street(gx, gy), tiles = s ? dirt : grass; if (s) grid[gy][gx] = '.';
    for (let j = 0; j < 2; j++) for (let i = 0; i < 2; i++) out.blit(tiles[(gx + gy + i + j) % 2], gx * 32 + i * 16, gy * 32 + j * 16);
  }
  const hotspots = [], lamp = drawTopProp({ kind: 'brazier' }).frames[0], tree = drawTopProp({ kind: 'tree', seed }).frames[0];
  for (let gy = 2; gy < GH - 1; gy += 2) for (let gx = 2; gx < GW - 1; gx += 2) if (column(gx)) out.blit(lamp, gx * 32, gy * 32);
  const at = new Map([...taken].map(([k, i]) => [k, i]));
  for (let sy = 0; sy < rows; sy++) for (let sx = 0; sx < cols; sx++) {
    const gx = 2 * sx + 1, gy = 2 * sy + 1, i = at.get(`${sx},${sy}`); grid[gy][gx] = '#';
    if (i === undefined) { out.blit(tree, gx * 32 + rand.int(5) - 2, gy * 32 + rand.int(5) - 2); continue; }
    const b = buildings[i], art = drawService({ ...b, view: 'top', seed });
    out.blit(art.frames[0], gx * 32, gy * 32);
    hotspots.push({ i, id: `${b.service}${buildings.filter((q, j) => j < i && q.service === b.service).length ? `-${buildings.filter((q, j) => j < i && q.service === b.service).length + 1}` : ''}`, service: b.service, state: b.state ?? 'available', rect: [gx * 32, gy * 32, 32, 32], entry: [gx * 32 + 16, (gy + 1) * 32 + 16] });
  }
  hotspots.sort((a, b) => a.i - b.i).forEach((h) => delete h.i);
  return { id: 'town', frames: [out], fps: 0, loop: false, projection: 'top-down', spec: { seed, cols, rows, ...(paths === 'lattice' ? {} : { paths }) }, hotspots, grid: grid.map((r) => r.join('')), cell: 32, alt: `Town of ${n} building${n === 1 ? '' : 's'}: ${hotspots.map((h) => h.service).join(', ')}`, anchors: { root: [W >> 1, H >> 1] } };
}
