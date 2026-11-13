// Room dressing from the game's geometry (T17, `dress`). The engine decides the grid and what can be walked on and the art follows it, so this takes a grid in and
// never chooses one: every cell is drawn as what the grid says, and no prop is placed where it would cut the walkable cells off from each other. It returns the
// picture, the props it placed with their footprints, the cells they block, the cells that cannot be reached from the way in (a fault in the grid, reported
// not repaired) and `lintDress` checks that the picture implies no path the grid lacks.
//
// Grid characters: `#` wall (a space is rock outside the level and counts as wall), `.` floor, `+` door, `>` exit, `o` pit, `^` hazard (lava or toxic by theme), `=` gate (a portcullis; drawn in the `gates` state, walkable once the game opens it).
import { Pixels } from '../pixels.js';
import { rng } from '../rng.js';
import { drawTopTile, TOP_BIOMES, DOOR_STATES } from './dungeon.js';
import { drawTopProp, TOP_PROPS } from './topprops.js';

const T = 32;
export const CELLS = { '#': 'wall', ' ': 'wall', '.': 'floor', '+': 'door', '>': 'exit', o: 'pit', '^': 'hazard', '=': 'gate' };
const WALKABLE = ['floor', 'door', 'exit', 'gate'];
export const TEMPLATES = {
  arena: [], corridor: ['torch', 'torch', 'torch'], junction: ['brazier', 'torch'], treasure: ['chest', 'chest', 'urn', 'crate', 'crate'], trap: ['plate', 'trap', 'trap', 'web', 'trap'],
  shrine: ['statue', 'brazier', 'brazier', 'banner', 'banner'], safe: ['brazier', 'crate', 'lectern', 'banner'], merchant: ['crate', 'urn', 'lectern', 'scrollrack', 'crate'],
  boss: ['statue', 'statue', 'brazier', 'brazier', 'banner'], exit: ['torch', 'torch', 'banner'], secret: ['bookshelf', 'web', 'lever', 'scrollrack'],
};
export const TEMPLATE_IDS = Object.keys(TEMPLATES);
/** Props that stand in the way (a floor cell they cover is no longer walkable); the others lie on the floor or hang on a wall (a torch, a lever, a banner) and block nothing. */
const AGAINST_WALL = ['torch', 'lever', 'banner', 'bookshelf', 'scrollrack', 'statue', 'coffin'];
const BLOCKING = ['chest', 'crate', 'urn', 'statue', 'brazier', 'coffin', 'bookshelf', 'lectern', 'scrollrack', 'ore', 'crystal', 'tree', 'pine', 'bush', 'rock', 'stump', 'portal'];
const DIRS = [[0, -1], [1, 0], [0, 1], [-1, 0]], NAMES = ['north', 'east', 'south', 'west'];
const HAZARDS = { volcanic: 'lava', forge: 'lava', flooded: 'water', void: 'chasm' };

/** Parse a grid into `{ w, h, kind(x, y) }`, refusing ragged rows and characters it does not know. */
function parseGrid(grid) {
  if (!Array.isArray(grid) || !grid.length || grid.some((r) => typeof r !== 'string')) throw new Error('a dress grid is an array of strings, one per row');
  const w = grid[0].length; if (!w || grid.some((r) => r.length !== w)) throw new Error('every row of a dress grid is the same length');
  for (const [y, row] of grid.entries()) for (const [x, ch] of [...row].entries()) if (!CELLS[ch]) throw new Error(`unknown grid character "${ch}" at ${x},${y}; known: # . + = > o ^ and space`);
  return { w, h: grid.length, kind: (x, y) => (x < 0 || y < 0 || x >= w || y >= grid.length ? 'wall' : CELLS[grid[y][x]]) };
}

/** Cells reachable on foot from `from`, never through a cell in `blocked` (a Set of "x,y"). */
function reach(g, from, blocked = new Set()) {
  const seen = new Set(), todo = [from];
  while (todo.length) { const [x, y] = todo.pop(), k = `${x},${y}`; if (seen.has(k) || blocked.has(k) || !WALKABLE.includes(g.kind(x, y))) continue; seen.add(k); for (const [dx, dy] of DIRS) todo.push([x + dx, y + dy]); }
  return seen;
}
const entryOf = (g) => { for (const kind of ['door', 'exit', 'floor']) for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) if (g.kind(x, y) === kind) return [x, y]; return null; };

/** The autotile bits of one cell: for a wall the sides that are wall, for a floor the sides that have one (the shadow), for a pit or hazard the sides that join. */
function bitsOf(g, x, y, kind) {
  const hit = (k) => (kind === 'wall' ? k === 'wall' : kind === 'floor' || kind === 'door' || kind === 'exit' || kind === 'gate' ? k === 'wall' : k === kind || k === 'wall');
  return DIRS.reduce((m, [dx, dy], i) => m | (hit(g.kind(x + dx, y + dy)) ? 1 << i : 0), 0);
}
const notchOf = (g, x, y) => [[1, 1, -1], [2, 1, 1], [4, -1, 1], [8, -1, -1]].reduce((m, [bit, dx, dy]) => m | (g.kind(x + dx, y + dy) !== 'wall' && g.kind(x + dx, y) === 'wall' && g.kind(x, y + dy) === 'wall' ? bit : 0), 0);

function tileFor(g, x, y, opts, cache) {
  const kind = g.kind(x, y), part = { wall: 'wall', floor: 'floor', door: 'doorway', gate: 'gate', exit: 'stairs-down', pit: 'pit', hazard: opts.hazard }[kind], mask = bitsOf(g, x, y, kind);
  let notch = 0, facing = 'south';
  if (kind === 'wall') notch = notchOf(g, x, y);
  if (kind === 'door' || kind === 'gate') facing = g.kind(x - 1, y) === 'wall' && g.kind(x + 1, y) === 'wall' ? 'north' : g.kind(x, y - 1) === 'wall' && g.kind(x, y + 1) === 'wall' ? 'east' : 'north';
  if (kind === 'exit') facing = NAMES[DIRS.findIndex(([dx, dy]) => g.kind(x + dx, y + dy) === 'wall')] ?? 'south';
  const variant = kind === 'floor' || kind === 'wall' ? (x * 7 + y * 13) % 4 : 0, key = `${part}:${mask}:${notch}:${facing}:${variant}:${kind === 'door' ? opts.doors : kind === 'gate' ? opts.gates : ''}`;
  if (!cache.has(key)) cache.set(key, drawTopTile({ biome: opts.theme, part, mask, notch, facing, state: kind === 'gate' ? opts.gates : opts.doors, seed: `${opts.seed}:${variant}` }));
  return cache.get(key);
}

/**
 * Dress a level from above. `grid` is the game's rows; `theme` a dungeon biome; `template` names the props to scatter (`TEMPLATES`) and `props` lists the game's own
 * placements `[{ kind, x, y, state }]`, laid first; `doors` the state every doorway is drawn in and `gates` the state of every `=` gate (`closed` by default); `hazard` `lava`, `toxic`, `water` or `chasm` (the theme's own by default).
 */
export function drawDress({ grid, theme = 'crypt', template = 'arena', props = [], doors = 'open', gates = 'closed', hazard, seed = 'dress' } = {}) {
  if (!TOP_BIOMES.includes(theme)) throw new Error(`unknown dungeon theme "${theme}"; known: ${TOP_BIOMES.join(', ')}`);
  if (!TEMPLATES[template]) throw new Error(`unknown template "${template}"; known: ${TEMPLATE_IDS.join(', ')}`);
  if (!DOOR_STATES.includes(doors)) throw new Error(`unknown doorway state "${doors}"; known: ${DOOR_STATES.join(', ')}`);
  if (!DOOR_STATES.includes(gates)) throw new Error(`unknown gate state "${gates}"; known: ${DOOR_STATES.join(', ')}`);
  hazard ??= HAZARDS[theme] ?? 'toxic';
  if (!['lava', 'toxic', 'water', 'chasm'].includes(hazard)) throw new Error(`unknown hazard "${hazard}"; known: lava, toxic, water, chasm`);
  const g = parseGrid(grid), opts = { theme, doors, gates, hazard, seed }, cache = new Map(), base = new Pixels(g.w * T, g.h * T);
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) base.blit(tileFor(g, x, y, opts, cache).frames[0], x * T, y * T);

  // Props: the game's first, then the template's, each only where the walkable cells stay joined.
  const entry = entryOf(g), whole = entry ? reach(g, entry) : new Set(), blocked = new Set(), taken = new Set(), placed = [], r = rng(`${seed}:${theme}:${template}`);
  const cellsOf = (kind, x, y) => { const [fw, fh] = TOP_PROPS[kind].foot; return Array.from({ length: fw * fh }, (_, i) => [x + (i % fw), y + Math.floor(i / fw)]); };
  const fits = (kind, x, y) => {
    const cs = cellsOf(kind, x, y);
    if (cs.some(([cx, cy]) => g.kind(cx, cy) !== 'floor' || taken.has(`${cx},${cy}`))) return false;
    if (!BLOCKING.includes(kind)) return true;
    const next = new Set([...blocked, ...cs.map(([cx, cy]) => `${cx},${cy}`)]), after = reach(g, entry, next);
    return [...whole].every((k) => next.has(k) || after.has(k));
  };
  const put = (kind, x, y, state) => {
    for (const [cx, cy] of cellsOf(kind, x, y)) { taken.add(`${cx},${cy}`); if (BLOCKING.includes(kind)) blocked.add(`${cx},${cy}`); }
    const art = drawTopProp({ kind, state, seed: `${seed}:${placed.length}` }); placed.push({ kind, x, y, state: art.spec.state, footprint: art.footprint, occlusion: art.occlusion, art });
  };
  for (const p of props) {
    if (!TOP_PROPS[p.kind]) throw new Error(`unknown top-down prop "${p.kind}"`);
    if (!entry || !fits(p.kind, p.x, p.y)) throw new Error(`prop "${p.kind}" at ${p.x},${p.y} does not fit: it needs free floor cells and must leave the walkable cells joined`);
    put(p.kind, p.x, p.y, p.state);
  }
  const floorCells = []; for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) if (g.kind(x, y) === 'floor') floorCells.push([x, y]);
  const awayFromDoors = ([x, y]) => !DIRS.some(([dx, dy]) => ['door', 'exit', 'gate'].includes(g.kind(x + dx, y + dy)));
  for (const kind of entry ? TEMPLATES[template] : []) {
    const open = floorCells.filter(awayFromDoors), hug = open.filter(([x, y]) => DIRS.some(([dx, dy]) => g.kind(x + dx, y + dy) === 'wall')), pool = AGAINST_WALL.includes(kind) && hug.length ? hug : open, start = pool.length ? r.int(pool.length) : 0;
    for (let i = 0; i < pool.length; i++) { const [x, y] = pool[(start + i) % pool.length]; if (fits(kind, x, y)) { put(kind, x, y); break; } }
  }

  // Frames: one, or the length of the longest loop among the props.
  const loops = placed.filter((p) => p.art.frames.length > 1), n = loops.length ? Math.max(...loops.map((p) => p.art.frames.length)) : 1;
  const frames = Array.from({ length: n }, (_, t) => { const f = base.clone(); for (const p of placed) f.blit(p.art.frames[t % p.art.frames.length], p.x * T, p.y * T); return f; });
  const walkable = [...reach(g, entry ?? [0, 0], blocked)].map((k) => k.split(',').map(Number)), unreachable = [];
  if (entry) for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) if (WALKABLE.includes(g.kind(x, y)) && !whole.has(`${x},${y}`)) unreachable.push([x, y]);
  const cells = Array.from({ length: g.h }, (_, y) => Array.from({ length: g.w }, (_, x) => g.kind(x, y)));
  return {
    id: `dress.${template}`, alt: `Top-down ${theme} ${template} room, ${g.w} by ${g.h} cells, ${placed.length} ${placed.length === 1 ? 'prop' : 'props'}`, frames, fps: n > 1 ? 6 : 0, loop: n > 1, projection: 'top-down', spec: { theme, template, doors, ...(grid.some((r) => r.includes('=')) ? { gates } : {}), hazard, seed, grid }, cells, entry,
    props: placed.map(({ art, ...p }) => p), blocked: [...blocked].map((k) => k.split(',').map(Number)), walkable, unreachable, tile: T,
    anchors: { root: entry ? [entry[0] * T + T / 2, entry[1] * T + T / 2] : [0, 0] },
  };
}

/** Cells whose picture does not match their kind: a wall cell drawn with anything but its wall tile, or a floor cell without its floor. A dressed level that implies a path the grid lacks fails here. */
export function lintDress(art) {
  const { grid, theme, doors, gates = 'closed', hazard, seed } = art.spec, g = parseGrid(grid), opts = { theme, doors, gates, hazard, seed }, cache = new Map(), bad = [], propCells = new Set();
  for (const p of art.props) { const [fw, fh] = p.footprint; for (let j = 0; j < fh; j++) for (let i = 0; i < fw; i++) propCells.add(`${p.x + i},${p.y + j}`); }
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    if (propCells.has(`${x},${y}`)) { if (g.kind(x, y) !== 'floor') bad.push([x, y]); continue; }
    if (!art.frames[0].crop(x * T, y * T, T, T).equals(tileFor(g, x, y, opts, cache).frames[0])) bad.push([x, y]);
  }
  return bad;
}
