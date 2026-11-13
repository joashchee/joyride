// Interiors (the hands/poses/motion/interiors kit): floors, furniture, rooms and lighting. A room is built from the kit's five layers
// (architecture, furniture, props, lighting, storytelling), and its readability checklist is data you can test: the layout is a grid,
// the entrance and the focal point are named, and `unreachable()` says whether every piece of furniture can be walked up to from the door.
// Furniture is drawn 3/4 on: the front face and the top surface, taller than its footprint, bottom-aligned to it, so a bed against
// the back wall rises over the wall tiles the way it does in the CC0 tilesets the kit points at.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { wallTile, groundTile, waterTile, drawProp } from './tiles.js';

const T = 16;
export const FLOOR_KINDS = ['planks', 'flagstone', 'checker'];
const DARK = [C.night, C.night, C.plum, C.plum]; // the inside of a hearth, a doorway, a cupboard

/** A tileable interior floor: boards with staggered joints, flagstones, or a two-tone checker. */
export function floorTile(kind = 'planks', seed = 'floor', rampName = kind === 'planks' ? 'wood' : 'stone') {
  const r = rng(`${seed}:${kind}`), rp = ramp(rampName), out = new Pixels(T, T);
  if (kind === 'planks') {
    for (let row = 0; row < 4; row++) {
      const y0 = row * 4, v = r.pick([1, 2, 2, 3]), joint = r.int(T);
      out.rect(0, y0, T, 3, rp[v]).rect(0, y0 + 3, T, 1, rp[0]);
      out.rect(joint, y0, 1, 3, rp[0]);
      const g = r.int(10); out.rect(g, y0 + 1, 4, 1, rp[Math.min(3, v + 1)]);
    }
  } else if (kind === 'flagstone') {
    for (let row = 0; row < 2; row++) for (let bx = -1; bx < 2; bx++) {
      const x0 = bx * 8 + (row ? 4 : 0), y0 = row * 8, v = r.pick([1, 2, 2, 2]);
      for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) out.set(x0 + x, y0 + y, y === 7 || x === 7 ? rp[0] : y === 0 || x === 0 ? rp[3] : rp[v]);
    }
    const x = r.int(10), y = r.int(10); out.set(x, y, rp[0]).set(x + 1, y + 1, rp[0]).set(x + 1, y + 2, rp[0]);
  } else if (kind === 'checker') {
    const a = ramp(rampName), b = ramp('iron');
    for (let y = 0; y < T; y++) for (let x = 0; x < T; x++) out.set(x, y, ((x >> 3) + (y >> 3)) % 2 ? b[2] : a[2]);
    out.rect(0, 7, T, 1, rp[1]).rect(7, 0, 1, T, rp[1]);
  } else throw new Error(`unknown floor "${kind}"`);
  return out;
}

/**
 * The floor and wall of each of Idle Solo's dungeon biomes, as the tile kinds above (a flagstone floor or a ground floor, a brick wall) in the
 * ramps that biome lives in. `veins` lays glowing cracks over the floor.
 */
export const DUNGEON_SETS = {
  crypt: { floor: ['flagstone', 'iron'], wall: 'stone' }, cave: { floor: ['ground', 'dirt'], wall: 'wood' }, forge: { floor: ['flagstone', 'iron', 'veins'], wall: 'iron' },
  flooded: { floor: ['water', 'water'], wall: 'moss' }, frozen: { floor: ['ground', 'ice'], wall: 'ice' }, overgrown: { floor: ['ground', 'grass'], wall: 'moss' },
  ancient: { floor: ['flagstone', 'sand'], wall: 'sand' }, volcanic: { floor: ['flagstone', 'iron', 'veins'], wall: 'dirt' },
};
export const DUNGEON_BIOMES = Object.keys(DUNGEON_SETS);
/** One tile of a dungeon biome: `part` is 'floor' or 'wall'. */
export function dungeonTile(biome = 'crypt', part = 'floor', seed = 'dungeon') {
  const set = DUNGEON_SETS[biome]; if (!set) throw new Error(`unknown dungeon biome "${biome}"`);
  if (part === 'wall') return wallTile(set.wall, `${seed}:${biome}:w`);
  if (part !== 'floor') throw new Error(`unknown tile part "${part}"`);
  const [kind, rampName, deco] = set.floor, s = `${seed}:${biome}:f`;
  const out = kind === 'flagstone' ? floorTile('flagstone', s, rampName) : kind === 'water' ? waterTile(s, rampName) : groundTile(rampName, s);
  if (deco === 'veins') { const r = rng(`${s}:veins`), f = ramp('fire'); for (let i = 0; i < 3; i++) { let x = r.int(T), y = r.int(T); for (let k = 0; k < 4; k++) { out.set(x, y, f[1 + (k & 1)]); x = (x + r.pick([-1, 1]) + T) % T; y = (y + r.pick([0, 1]) + T) % T; out.set(x, y, f[0]); } } }
  return out;
}

const mk = (w, h) => new Pixels(w * T, h * T);
const part = (m, id, ...rects) => { for (const [x, y, w, h] of rects) m.rect(x, y, w, h, id); return m; };

/**
 * Furniture. `foot` is [width, depth] in tiles on the floor; `make()` draws the sprite (its bottom edge is the footprint's bottom).
 * `light` is a glow [dx, dy, radius] from the sprite's top left. `flat` things (a rug) lie on the floor and block nothing.
 */
export const FURNITURE = {
  bed: { foot: [1, 2], make() { const m = mk(1, 2); part(m, 1, [1, 1, 14, 30]); part(m, 3, [3, 7, 10, 5]); part(m, 2, [2, 13, 12, 17]); part(m, 3, [2, 13, 12, 2]); return shade(m, { 1: ramp('wood'), 2: ramp('red'), 3: ramp('white') }); } },
  table: { foot: [2, 1], make() { const m = mk(2, 1); part(m, 1, [1, 3, 30, 6]); part(m, 2, [2, 9, 28, 2], [2, 11, 3, 4], [27, 11, 3, 4]); return shade(m, { 1: ramp('wood'), 2: ramp('dirt') }); } },
  chair: { foot: [1, 1], make() { const m = mk(1, 1); part(m, 1, [4, 1, 8, 7], [3, 8, 10, 3]); part(m, 2, [4, 11, 2, 4], [10, 11, 2, 4]); return shade(m, { 1: ramp('wood'), 2: ramp('dirt') }); } },
  shelf: {
    foot: [1, 1], make() {
      const m = mk(1, 2), r = rng('shelf'); part(m, 1, [1, 1, 14, 29]); part(m, 2, [3, 3, 10, 25]); part(m, 1, [3, 10, 10, 1], [3, 18, 10, 1], [3, 26, 10, 1]);
      for (const base of [9, 17, 25]) for (let x = 3; x < 13; ) { const w = r.pick([1, 2, 2]), h = r.range(4, 6); if (x + w > 13) break; part(m, r.pick([3, 4, 5, 6]), [x, base - h + 1, w, h]); x += w; }
      return shade(m, { 1: ramp('wood'), 2: DARK, 3: ramp('red'), 4: ramp('blue'), 5: ramp('green'), 6: ramp('gold') });
    },
  },
  hearth: {
    foot: [2, 1], light: [16, 24, 64], make() {
      const m = mk(2, 2); part(m, 1, [1, 2, 30, 4], [2, 6, 28, 25]); part(m, 2, [8, 12, 16, 19]); part(m, 3, [10, 29, 12, 2]); m.ellipse(16, 26, 4.5, 5.5, 4); m.ellipse(16, 27, 2.5, 3.5, 5);
      return shade(m, { 1: ramp('stone'), 2: DARK, 3: ramp('wood'), 4: ramp('fire'), 5: [C.orange, C.yellow, C.yellow, C.white] });
    },
  },
  altar: { foot: [2, 1], make() { const m = mk(2, 1); part(m, 1, [1, 3, 30, 3]); part(m, 2, [3, 6, 26, 9]); part(m, 3, [11, 3, 10, 12]); return shade(m, { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('purple') }); } },
  workbench: { foot: [2, 1], make() { const m = mk(2, 1); part(m, 1, [1, 4, 30, 4]); part(m, 2, [2, 8, 28, 3], [2, 11, 3, 4], [27, 11, 3, 4]); part(m, 3, [5, 1, 7, 3]); part(m, 4, [13, 2, 6, 2]); part(m, 5, [24, 1, 4, 3]); return shade(m, { 1: ramp('wood'), 2: ramp('dirt'), 3: ramp('steel'), 4: ramp('wood'), 5: ramp('iron') }); } },
  anvil: { foot: [1, 1], make() { const m = mk(1, 1); part(m, 1, [4, 11, 8, 3], [6, 8, 4, 3], [2, 5, 12, 3]); m.poly([[12, 5], [15, 6], [12, 8]], 1); return shade(m, { 1: ramp('iron') }); } },
  barrel: { foot: [1, 1], make() { const m = mk(1, 1); m.ellipse(8, 8, 5, 6.5, 1); part(m, 2, [3, 5, 10, 1], [3, 10, 10, 1]); return shade(m, { 1: ramp('wood'), 2: ramp('iron') }); } },
  candle: { foot: [1, 1], light: [8, 3, 38], make() { const m = mk(1, 1); part(m, 1, [7, 6, 2, 8], [5, 13, 6, 1]); part(m, 2, [6, 3, 1, 3], [9, 3, 1, 3]); part(m, 3, [6, 1, 1, 2], [9, 1, 1, 2]); return shade(m, { 1: ramp('gold'), 2: ramp('bone'), 3: ramp('fire') }); } },
  rug: { foot: [2, 1], flat: true, make() { const m = mk(2, 1); part(m, 1, [1, 2, 30, 12]); part(m, 2, [3, 4, 26, 8]); part(m, 1, [5, 6, 22, 4]); m.poly([[16, 5], [21, 8], [16, 11], [11, 8]], 3); return shade(m, { 1: ramp('red'), 2: ramp('gold'), 3: ramp('blue') }); } },
  window: { foot: [0, 0], wall: true, make() { const m = mk(1, 1); part(m, 1, [2, 1, 12, 13]); part(m, 2, [4, 3, 8, 9]); part(m, 1, [7, 3, 2, 9], [4, 7, 8, 1]); return shade(m, { 1: ramp('wood'), 2: ramp('glass') }); } },
  doorway: { foot: [0, 0], wall: true, make() { const m = mk(1, 2); part(m, 1, [1, 2, 14, 29]); part(m, 2, [3, 6, 10, 25]); m.ellipse(8, 6, 5, 4, 2); part(m, 3, [3, 29, 10, 2]); return shade(m, { 1: ramp('wood'), 2: DARK, 3: ramp('sand') }); } },
  // CoffeeMUD remaining gaps: stove, tub, fountain, piano, quest board, cage, counter.
  stove: { foot: [1, 1], light: [8, 8, 30], make() { const m = mk(1, 2); part(m, 1, [3, 8, 10, 22]); part(m, 2, [5, 14, 6, 6]); part(m, 3, [6, 1, 4, 8]); part(m, 4, [6, 16, 4, 3]); part(m, 1, [2, 8, 12, 2]); return shade(m, { 1: ramp('iron'), 2: DARK, 3: ramp('steel'), 4: ramp('fire') }); } },
  tub: { foot: [2, 1], make() { const m = mk(2, 1); part(m, 1, [1, 4, 30, 10]); part(m, 2, [3, 5, 26, 4]); part(m, 3, [3, 14, 3, 1], [26, 14, 3, 1]); return shade(m, { 1: ramp('wood'), 2: ramp('water'), 3: ramp('iron') }); } },
  fountain: { foot: [2, 2], make() { const m = mk(2, 2); m.ellipse(16, 24, 14, 6, 1); m.ellipse(16, 23, 11, 4, 2); part(m, 1, [14, 8, 4, 14]); m.ellipse(16, 8, 5, 2, 1); part(m, 3, [15, 2, 2, 6]); part(m, 3, [12, 10, 1, 4], [19, 10, 1, 4]); return shade(m, { 1: ramp('stone'), 2: ramp('water'), 3: ramp('ice') }); } },
  piano: { foot: [2, 1], make() { const m = mk(2, 2); part(m, 1, [1, 6, 30, 13]); part(m, 2, [2, 14, 28, 4]); for (let x = 3; x < 29; x += 3) part(m, 3, [x, 14, 1, 4]); part(m, 1, [3, 19, 2, 11], [27, 19, 2, 11]); part(m, 1, [10, 2, 12, 4]); return shade(m, { 1: ramp('wood'), 2: ramp('white'), 3: ramp('iron') }); } },
  questboard: { foot: [1, 1], make() { const m = mk(1, 2); part(m, 1, [1, 2, 14, 18]); part(m, 2, [3, 4, 10, 14]); part(m, 3, [4, 5, 4, 5], [9, 6, 3, 4], [5, 12, 5, 4]); part(m, 1, [3, 20, 2, 10], [11, 20, 2, 10]); return shade(m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('bone') }); } },
  cage: { foot: [1, 1], make() { const m = mk(1, 2); part(m, 1, [2, 22, 12, 3], [2, 4, 12, 3]); for (const x of [3, 6, 9, 12]) part(m, 1, [x, 7, 1, 15]); part(m, 2, [6, 1, 4, 3]); return shade(m, { 1: ramp('iron'), 2: ramp('steel') }); } },
  counter: { foot: [2, 1], make() { const m = mk(2, 1); part(m, 1, [1, 3, 30, 12]); part(m, 2, [1, 3, 30, 3]); part(m, 3, [4, 8, 10, 5], [18, 8, 10, 5]); return shade(m, { 1: ramp('wood'), 2: ramp('sand'), 3: ramp('dirt') }); } },
  pillar: { foot: [1, 1], make: () => drawProp('pillar') },
  bones: { foot: [1, 1], flat: true, make: () => drawProp('bones') },
  torch: { foot: [1, 1], light: [8, 4, 44], make: () => drawProp('torch') },
  chest: { foot: [1, 1], make: () => drawProp('chest') },
};
export const FURNITURE_IDS = Object.keys(FURNITURE);

/** Small things that say who lives here: 10x10 with an outline, set on a table or an altar. */
const clue = (draw) => { const p = new Pixels(10, 10); draw(p); return p.outline(C.night); };
export const CLUES = {
  book: () => clue((p) => p.rect(2, 4, 6, 4, C.brown).rect(3, 5, 4, 2, C.white)),
  mug: () => clue((p) => p.rect(3, 4, 4, 4, C.tan).set(7, 5, C.tan).set(7, 6, C.tan).rect(3, 4, 4, 1, C.white)),
  scroll: () => clue((p) => p.rect(2, 4, 6, 3, C.peach).rect(1, 3, 1, 5, C.brown).rect(8, 3, 1, 5, C.brown)),
  skull: () => clue((p) => p.ellipse(5, 4.5, 3, 2.5, C.white).rect(4, 6, 3, 2, C.white).set(4, 4, C.night).set(6, 4, C.night)),
  coins: () => clue((p) => p.rect(2, 6, 3, 2, C.yellow).rect(5, 5, 3, 3, C.yellow).set(6, 5, C.white)),
  ingot: () => clue((p) => p.poly([[2, 7], [3, 4], [7, 4], [8, 7]], C.mist).rect(3, 4, 4, 1, C.white)),
  potion: () => clue((p) => p.rect(4, 2, 2, 2, C.dust).ellipse(5, 6, 2.5, 2.5, C.red).set(4, 5, C.pink)),
};

/**
 * Rooms are data. `items` are [furniture, column, row] with row 0 against the back wall; `windows` and the door are wall columns;
 * `clues` are [clue, furniture, dx, dy] set on the first of that furniture. `focal` is the item the eye should land on.
 */
export const ROOMS = {
  cottage: { floor: 'planks', wall: 'wood', time: 'day', focal: 'hearth', windows: [7], items: [['bed', 0, 0], ['shelf', 2, 0], ['hearth', 4, 0], ['barrel', 11, 0], ['barrel', 11, 1], ['rug', 5, 3], ['table', 5, 3], ['chair', 4, 3], ['chair', 7, 3]], clues: [['book', 'table', 18, -1], ['mug', 'table', 6, -1]], story: 'a lived-in home: a bed, a book left open, a mug still on the table' },
  smithy: { floor: 'flagstone', wall: 'stone', time: 'dusk', focal: 'anvil', windows: [4], items: [['hearth', 1, 0], ['workbench', 6, 0], ['anvil', 4, 2], ['barrel', 11, 0], ['barrel', 11, 2], ['chest', 8, 3]], clues: [['ingot', 'workbench', 18, 0]], story: 'a working trade: a forge, an anvil and bar stock on the bench' },
  library: { floor: 'planks', wall: 'wood', time: 'day', focal: 'table', windows: [5, 6], items: [['shelf', 0, 0], ['shelf', 1, 0], ['shelf', 2, 0], ['shelf', 3, 0], ['shelf', 8, 0], ['shelf', 9, 0], ['candle', 4, 1], ['candle', 8, 2], ['rug', 5, 3], ['table', 5, 3], ['chair', 4, 3], ['chair', 7, 3]], clues: [['scroll', 'table', 18, -1], ['book', 'table', 6, -1]], story: 'a scholar: more books than furniture and a scroll half read' },
  shrine: { floor: 'checker', wall: 'stone', time: 'dusk', focal: 'altar', windows: [3, 8], items: [['altar', 5, 0], ['candle', 4, 0], ['candle', 7, 0], ['pillar', 2, 0], ['pillar', 9, 0], ['pillar', 2, 3], ['pillar', 9, 3], ['rug', 5, 2]], clues: [['coins', 'altar', 18, -1]], story: 'a place of offering: candles lit either side of the altar, coins left on it' },
  cell: { floor: 'flagstone', wall: 'stone', time: 'night', focal: 'chest', windows: [], items: [['barrel', 0, 0], ['torch', 6, 0], ['chest', 11, 0], ['bones', 3, 3]], clues: [['skull', 'bones', 3, -2]], story: 'a place someone did not leave: bones on the floor, one chest, one torch' },
  tavern: { floor: 'planks', wall: 'wood', time: 'dusk', focal: 'counter', windows: [3, 8], items: [['counter', 4, 0], ['barrel', 7, 0], ['barrel', 8, 0], ['shelf', 0, 0], ['hearth', 1, 0], ['table', 1, 3], ['chair', 0, 3], ['table', 6, 3], ['chair', 5, 3], ['rug', 3, 3]], clues: [['mug', 'counter', 8, -2], ['coins', 'counter', 20, -2]], story: 'a house of drink: casks behind the bar, a hearth, a mug and coins left on the counter' },
  shop: { floor: 'planks', wall: 'wood', time: 'day', focal: 'counter', windows: [2, 6], items: [['shelf', 0, 0], ['shelf', 1, 0], ['shelf', 8, 0], ['shelf', 9, 0], ['counter', 4, 1], ['barrel', 7, 0], ['chest', 2, 3], ['rug', 5, 3]], clues: [['coins', 'counter', 8, -2], ['ingot', 'counter', 20, -2]], story: 'a trade: stocked shelves, a counter with coin on it, goods waiting by the door' },
  castlehall: { floor: 'checker', wall: 'stone', time: 'dusk', focal: 'altar', windows: [2, 5, 8], items: [['pillar', 1, 0], ['pillar', 3, 0], ['hearth', 4, 0], ['altar', 6, 0], ['pillar', 8, 0], ['pillar', 9, 0], ['table', 3, 3], ['table', 7, 3], ['chair', 2, 3], ['chair', 9, 3], ['rug', 5, 2], ['torch', 0, 1], ['torch', 11, 1]], clues: [['mug', 'table', 6, -1], ['book', 'table', 18, -1]], story: 'a seat of power: a long hall of pillars, a hearth and a high table, torches along the walls' },
  ruin: { floor: 'flagstone', wall: 'stone', time: 'night', focal: 'altar', windows: [], items: [['pillar', 2, 0], ['altar', 5, 0], ['bones', 8, 2], ['barrel', 0, 0], ['chest', 11, 3], ['torch', 7, 0]], clues: [['skull', 'bones', 3, -2]], story: 'a place that fell: a broken altar, a lone torch, bones where someone stood last' },
  sewer: { floor: 'flagstone', wall: 'stone', time: 'night', focal: 'barrel', windows: [], items: [['barrel', 0, 0], ['barrel', 11, 0], ['bones', 4, 2], ['torch', 6, 0], ['chest', 9, 3]], clues: [['skull', 'bones', 3, -2]], story: 'a drain under the town: rotted casks, a lone torch, something left to the dark' },
  stable: { floor: 'planks', wall: 'wood', time: 'day', focal: 'barrel', windows: [3, 8], items: [['barrel', 0, 0], ['barrel', 1, 0], ['shelf', 5, 0], ['chest', 11, 0], ['rug', 3, 3], ['workbench', 6, 2]], clues: [['ingot', 'workbench', 18, 0]], story: 'a working yard: feed barrels, a bench of tack, straw on the floor' },
  inn: { floor: 'planks', wall: 'wood', time: 'night', focal: 'hearth', windows: [7], items: [['bed', 0, 0], ['bed', 2, 0], ['hearth', 4, 0], ['chest', 7, 0], ['table', 8, 2], ['chair', 7, 2], ['chair', 10, 2], ['rug', 3, 3]], clues: [['mug', 'table', 6, -1], ['book', 'table', 18, -1]], story: 'a place to sleep: beds in a row, a hearth burning low, a mug left on the table' },
  // Phase 4 (T20): the interiors the town's services need.
  alchemist: { floor: 'flagstone', wall: 'stone', time: 'dusk', focal: 'workbench', windows: [4], items: [['shelf', 0, 0], ['shelf', 1, 0], ['shelf', 8, 0], ['workbench', 5, 0], ['stove', 3, 0], ['barrel', 11, 0], ['candle', 7, 1], ['table', 2, 3], ['chair', 1, 3], ['rug', 5, 3]], clues: [['potion', 'workbench', 18, -1], ['book', 'table', 18, -1]], story: 'a brewer trade: shelves of bottles, a still on the bench, a book of recipes on the table' },
  bank: { floor: 'checker', wall: 'stone', time: 'day', focal: 'counter', windows: [2, 7], items: [['counter', 4, 1], ['pillar', 2, 0], ['pillar', 9, 0], ['chest', 0, 0], ['chest', 1, 0], ['chest', 7, 0], ['rug', 5, 3], ['candle', 8, 1]], clues: [['coins', 'counter', 8, -2], ['scroll', 'counter', 20, -2]], story: 'a house of coin: a long counter with a ledger and coins, strongboxes against the wall' },
  training: { floor: 'flagstone', wall: 'wood', time: 'day', focal: 'questboard', windows: [3, 8], items: [['questboard', 5, 0], ['barrel', 0, 0], ['barrel', 1, 0], ['workbench', 8, 0], ['bones', 4, 3], ['rug', 5, 3], ['torch', 2, 0], ['torch', 10, 1]], clues: [['ingot', 'workbench', 18, 0]], story: 'a place to drill: a board of orders, racks of barrels and a bench of weapons' },
  atelier: { floor: 'planks', wall: 'wood', time: 'day', focal: 'workbench', windows: [2, 5], items: [['workbench', 4, 0], ['shelf', 0, 0], ['shelf', 1, 0], ['shelf', 9, 0], ['chest', 11, 0], ['table', 7, 3], ['chair', 6, 3], ['rug', 3, 3], ['candle', 8, 1]], clues: [['scroll', 'table', 18, -1], ['ingot', 'workbench', 18, 0]], story: 'a maker\'s room: a long bench, shelves of parts and a table of sketches' },
  portal: { floor: 'checker', wall: 'stone', time: 'night', focal: 'altar', windows: [], items: [['pillar', 2, 0], ['pillar', 9, 0], ['pillar', 2, 3], ['pillar', 9, 3], ['altar', 5, 0], ['candle', 4, 0], ['candle', 7, 0], ['rug', 5, 2], ['fountain', 5, 3]], clues: [['coins', 'altar', 18, -1]], story: 'a gate chamber: four pillars, a lit altar and a basin where the way opens' },
  museum: { floor: 'checker', wall: 'stone', time: 'day', focal: 'altar', windows: [2, 5, 8], items: [['pillar', 1, 0], ['altar', 3, 0], ['altar', 6, 0], ['altar', 8, 0], ['pillar', 0, 0], ['rug', 5, 3], ['table', 2, 3], ['table', 8, 3], ['candle', 5, 1]], clues: [['skull', 'altar', 18, -1], ['scroll', 'table', 18, -1]], story: 'a hall of keeping: plinths along the wall, cases on tables, a long rug between them' },
};
export const ROOM_IDS = Object.keys(ROOMS);
export const TIMES = { day: 2, dusk: 1, night: 0 };
const DOOR = 10;

// --- lighting ---------------------------------------------------------------------------------------------------------------

const maps = new WeakMap();
/** For each palette colour, the nearest palette colour after `fn` is applied to its RGB: how a whole scene is darkened or lightened without leaving the palette. */
function tintMap(palette, kind) {
  const key = maps.get(palette) ?? maps.set(palette, {}).get(palette);
  if (key[kind]) return key[kind];
  const rgb = palette.map((h) => { const n = parseInt(h.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; });
  const fn = kind === 'dark' ? ([r, g, b]) => [r * 0.5, g * 0.55 + 4, b * 0.6 + 26] : ([r, g, b]) => [r * 1.18 + 16, g * 1.15 + 12, b * 1.05 + 2];
  return (key[kind] = rgb.map((c) => { const t = fn(c); let best = 0, bd = Infinity; rgb.forEach((o, i) => { const d = (o[0] - t[0]) ** 2 + (o[1] - t[1]) ** 2 + (o[2] - t[2]) ** 2; if (d < bd) { bd = d; best = i; } }); return best + 1; }));
}
const tint = (px, kind) => { const m = tintMap(px.palette, kind); return px.map((x, y) => (px.get(x, y) ? m[px.get(x, y) - 1] : 0)); };

/**
 * Light a scene. `ambient` is day (2), dusk (1) or night (0); each source `{ x, y, r }` lights a circle in full and dithers out to r;
 * each shaft (a window's light falling on the floor) is a polygon of lightened pixels with a dithered rim. Dithering is how a
 * soft edge is drawn without alpha, so the result stays on the palette.
 */
export function relight(px, { ambient = 'day', sources = [], shafts = [] } = {}) {
  const base = TIMES[ambient]; if (base === undefined) throw new Error(`unknown time "${ambient}"`);
  const dark = tint(px, 'dark'), light = tint(px, 'light'), out = px.clone();
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) {
    if (!px.get(x, y)) continue;
    let level = base;
    for (const s of sources) { const d = Math.hypot(x - s.x, (y - s.y) * 1.25); level = Math.max(level, d <= s.r * 0.55 ? 2 : d <= s.r ? 1 : 0); }
    const v = level === 2 ? px.get(x, y) : level === 1 ? ((x + y) % 2 ? px.get(x, y) : dark.get(x, y)) : dark.get(x, y);
    out.set(x, y, v);
  }
  for (const poly of shafts) {
    const m = new Pixels(px.w, px.h).poly(poly, 1);
    for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) {
      if (!m.get(x, y) || !px.get(x, y)) continue;
      const core = [-1, 0, 1].every((j) => [-1, 0, 1].every((i) => m.get(x + i, y + j)));
      if (core || (x + y) % 2 === 0) out.set(x, y, light.get(x, y) ? light.get(x, y) : out.get(x, y));
    }
  }
  return out;
}

// --- rooms ------------------------------------------------------------------------------------------------------------------

/** The room's layout as data: the walkable grid, the door's entry cell, the focal item, and the placed items. */
export function layoutOf(room, { cols = 12, rows = 6 } = {}) {
  const spec = ROOMS[room]; if (!spec) throw new Error(`unknown room "${room}"`);
  const blocked = Array.from({ length: rows }, () => Array(cols).fill(false)), placed = [];
  for (const [name, x, y] of spec.items) {
    const f = FURNITURE[name]; if (!f) throw new Error(`unknown furniture "${name}"`);
    const [w, d] = f.foot; placed.push({ name, x, y, w, d, flat: !!f.flat });
    if (!f.flat) for (let j = 0; j < d; j++) for (let i = 0; i < w; i++) { if (blocked[y + j]?.[x + i] !== false) throw new Error(`${room}: ${name} at ${x},${y} is off the floor or overlaps another`); blocked[y + j][x + i] = true; }
  }
  return { cols, rows, blocked, entry: [DOOR, 0], door: DOOR, focal: placed.find((p) => p.name === spec.focal), items: placed };
}
/** Every free cell reachable from the door on foot. */
export function walkable(layout) {
  const seen = new Set(), todo = [layout.entry];
  while (todo.length) { const [x, y] = todo.pop(), k = `${x},${y}`; if (seen.has(k) || layout.blocked[y]?.[x] !== false) continue; seen.add(k); todo.push([x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]); }
  return seen;
}
/** Furniture you cannot walk up to from the door: the readability checklist's "can the viewer tell where the walkable space is". */
export function unreachable(layout) {
  const ok = walkable(layout);
  return layout.items.filter((p) => !p.flat && !(() => { for (let j = -1; j <= p.d; j++) for (let i = -1; i <= p.w; i++) { if ((i >= 0 && i < p.w && j >= 0 && j < p.d) || (i !== -1 && i !== p.w && j !== -1 && j !== p.d)) continue; if (ok.has(`${p.x + i},${p.y + j}`)) return true; } return false; })()).map((p) => p.name);
}

/** A room at a time of day: floor, back wall, windows and door, furniture sorted by depth, clues, then light. Night and dusk flicker (2 frames). */
export function drawRoom({ room = 'cottage', time, seed = 'room' } = {}) {
  const spec = ROOMS[room]; if (!spec) throw new Error(`unknown room "${room}"`);
  time ??= spec.time; if (TIMES[time] === undefined) throw new Error(`unknown time "${time}"`);
  const layout = layoutOf(room), { cols, rows } = layout, wallRows = 2, W = cols * T, H = (rows + wallRows) * T, floorY = wallRows * T;
  const base = new Pixels(W, H), floors = [floorTile(spec.floor, `${seed}:a`), floorTile(spec.floor, `${seed}:b`)];
  for (let y = 0; y < rows + wallRows; y++) for (let x = 0; x < cols; x++) base.blit(y < wallRows ? wallTile(spec.wall, `${seed}:w${x}${y}`) : floors[(x + y) % 2], x * T, y * T);
  // Where the wall meets the floor: a contact shadow, so the wall tile transition reads.
  const shadow = tint(base, 'dark');
  for (let x = 0; x < W; x++) { base.set(x, floorY, shadow.get(x, floorY)); if (x % 2 === 0) base.set(x, floorY + 1, shadow.get(x, floorY + 1)); }
  for (const c of spec.windows) base.blit(FURNITURE.window.make(), c * T, floorY - 2 * T);
  base.blit(FURNITURE.doorway.make(), DOOR * T, floorY - 2 * T);
  const sources = [], pos = {}, ordered = [...layout.items].sort((a, b) => Number(!a.flat) - Number(!b.flat) || a.y + a.d - (b.y + b.d));
  for (const it of ordered) {
    const f = FURNITURE[it.name], art = f.make(), x0 = it.x * T, y0 = floorY + (it.y + it.d) * T - art.h;
    base.blit(art, x0, y0); pos[it.name] ??= [x0, y0];
    if (f.light) sources.push({ x: x0 + f.light[0], y: y0 + f.light[1], r: f.light[2] });
  }
  for (const [name, on, dx, dy] of spec.clues) { const [x0, y0] = pos[on]; base.blit(CLUES[name](), x0 + dx, y0 + dy); }
  const shafts = spec.windows.map((c) => { const wx = c * T; return [[wx + 3, floorY], [wx + 13, floorY], [wx + 37, floorY + 44], [wx + 27, floorY + 44]]; });
  const frames = time === 'day' ? [relight(base, { ambient: time, shafts })] : [0, 1].map((f) => relight(base, { ambient: time, sources: sources.map((s) => ({ ...s, r: s.r + (f ? -3 : 2) })) }));
  const door = [DOOR * T + 8, floorY + 4];
  return { id: `room.${room}`, frames, fps: 4, loop: frames.length > 1, spec: { room, time, seed }, layout, anchors: { entry: door, focal: layout.focal ? [layout.focal.x * T + layout.focal.w * 8, floorY + (layout.focal.y + layout.focal.d) * T] : null }, story: spec.story };
}

