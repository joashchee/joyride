// Environments (16x16 tiles, props, buildings, scenes). The biomes kit's layered model is the data shape: each biome names its
// ground, path, wall, water, props and landmark (geology, vegetation, architecture, story evidence). Tiles are tileable, ground is
// built from clusters (never scattered noise), and the learning kit's transitions (grass to dirt, dirt to stone, water edge) are a recipe.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { drawItem } from './items.js';
import { drawScenery, SCENERY_IDS } from './scenery.js';
import { SCENE_LAYERS, wants, drawEvidence, drawInhabitants, drawWeather, drawDepth } from './scene-layers.js';

const T = 16;
export const BIOMES = {
  forest: { ground: 'grass', path: 'dirt', wall: 'moss', water: 'water', props: ['tree', 'pine', 'bush', 'rock', 'stump'], landmark: 'shrine' },
  cave: { ground: 'stone', path: 'dirt', wall: 'stone', water: 'water', props: ['rock', 'crystal', 'bones', 'mushroom'], landmark: 'door', walls: true },
  dungeon: { ground: 'iron', path: 'stone', wall: 'stone', water: 'water', props: ['pillar', 'torch', 'bones', 'chest'], landmark: 'door', walls: true },
  desert: { ground: 'sand', path: 'dirt', wall: 'sand', water: 'water', props: ['cactus', 'rock', 'bones', 'obelisk'], landmark: 'obelisk' },
  village: { ground: 'grass', path: 'dirt', wall: 'wood', water: 'water', props: ['tree', 'bush', 'fence', 'stump'], landmark: 'house' },
  swamp: { ground: 'moss', path: 'water', wall: 'moss', water: 'water', props: ['reeds', 'mushroom', 'stump', 'tree'], landmark: 'stump' },
  coast: { ground: 'sand', path: 'dirt', wall: 'stone', water: 'water', props: ['rock', 'reeds', 'bones', 'bush'], landmark: 'house' },
  magical: { ground: 'purple', path: 'glass', wall: 'purple', water: 'glass', props: ['crystal', 'mushroom', 'crystal', 'bush'], landmark: 'crystal' },
  // CoffeeMUD remaining biomes: mountain, snow, jungle, road, plains, and the tree surface (a canopy walked on).
  mountain: { ground: 'stone', path: 'dirt', wall: 'stone', water: 'water', props: ['rock', 'pine', 'rubble', 'bones'], landmark: 'obelisk', walls: true },
  snow: { ground: 'white', path: 'ice', wall: 'ice', water: 'ice', props: ['pine', 'rock', 'crystal', 'bones'], landmark: 'shrine' },
  jungle: { ground: 'green', path: 'dirt', wall: 'moss', water: 'water', props: ['tree', 'bush', 'reeds', 'mushroom', 'tree'], landmark: 'obelisk' },
  road: { ground: 'grass', path: 'stone', wall: 'wood', water: 'water', props: ['fence', 'tree', 'bush', 'rock'], landmark: 'house' },
  plains: { ground: 'grass', path: 'dirt', wall: 'wood', water: 'water', props: ['tuft', 'bush', 'rock', 'stump', 'tuft'], landmark: 'tree' },
  canopy: { ground: 'moss', path: 'wood', wall: 'wood', water: 'grass', props: ['bush', 'mushroom', 'stump', 'tuft'], landmark: 'tree', walls: true },
  // Scenery biomes (recipes/scenery.js): a castle, a market town, a harbour, a river crossing, ruins, mines, highlands and a floating isle.
  castle: { ground: 'grass', path: 'stone', wall: 'stone', water: 'water', props: ['battlement', 'tree', 'bush', 'well'], landmark: 'castle', walls: true },
  town: { ground: 'grass', path: 'stone', wall: 'wood', water: 'water', props: ['stall', 'well', 'signpost', 'barrel', 'fence'], landmark: 'gatehouse' },
  harbor: { ground: 'sand', path: 'wood', wall: 'stone', water: 'water', props: ['dock', 'boat', 'barrel', 'rock'], landmark: 'ship' },
  river: { ground: 'grass', path: 'water', wall: 'wood', water: 'water', props: ['ropebridge', 'bridge', 'reeds', 'tree', 'bush'], landmark: 'bridge' },
  ruins: { ground: 'sand', path: 'dirt', wall: 'sand', water: 'water', props: ['ruinwall', 'dune', 'bones', 'ruinarch', 'cactus'], landmark: 'ruinarch' },
  mines: { ground: 'stone', path: 'dirt', wall: 'stone', water: 'water', props: ['rubble', 'cart', 'barrel', 'rock', 'torch'], landmark: 'mineentrance', walls: true },
  highlands: { ground: 'stone', path: 'dirt', wall: 'stone', water: 'water', props: ['pine', 'rock', 'rubble', 'bush'], landmark: 'peak', walls: true },
  skyisle: { ground: 'purple', path: 'glass', wall: 'purple', water: 'glass', props: ['portal', 'crystal', 'mushroom', 'floatisle'], landmark: 'floatisle' },
};

/** A tileable ground tile: nearest-seed clusters, mostly the base value, with a few detail marks of the ground's own kind. */
export function groundTile(rampName, seed = 'g') {
  const r = rng(seed), rp = ramp(rampName), pts = Array.from({ length: 7 }, () => [r.int(T), r.int(T), r.pick([1, 1, 1, 2])]), out = new Pixels(T, T);
  for (let y = 0; y < T; y++) for (let x = 0; x < T; x++) {
    let best = 1e9, v = 2;
    for (const [px, py, pv] of pts) { const dx = Math.min(Math.abs(x - px), T - Math.abs(x - px)), dy = Math.min(Math.abs(y - py), T - Math.abs(y - py)), d = dx * dx + dy * dy; if (d < best) { best = d; v = pv; } }
    out.set(x, y, rp[v]);
  }
  for (let i = 0; i < 3; i++) { const x = r.int(T - 1), y = r.int(T - 2); out.set(x, y, rp[2]).set(x, y + 1, rp[0]); }
  return out;
}

/** Brick or block wall in running bond. */
export function wallTile(rampName, seed = 'w') {
  const r = rng(seed), rp = ramp(rampName), out = new Pixels(T, T);
  for (let row = 0; row < 4; row++) for (let bx = -1; bx < 2; bx++) {
    const x0 = bx * 8 + (row % 2 ? 4 : 0), y0 = row * 4, v = r.pick([1, 2, 2, 2]);
    for (let y = 0; y < 4; y++) for (let x = 0; x < 8; x++) {
      const c = y === 3 || x === 7 ? rp[0] : y === 0 ? rp[3] : rp[v];
      out.set(x0 + x, y0 + y, c);
    }
  }
  return out;
}

/** Water with short highlight strokes. */
export function waterTile(seed = 'wt', rampName = 'water') {
  const r = rng(seed), rp = ramp(rampName), out = new Pixels(T, T).rect(0, 0, T, T, rp[1]);
  for (let i = 0; i < 4; i++) { const x = r.int(T - 3), y = r.int(T); out.rect(x, y, 3, 1, rp[3]).rect(x + 1, y + 1, 2, 1, rp[2]); }
  return out;
}

/** Edge between two grounds: `a` above, `b` below, a wobbling border and a darker lip on b. Turn it with rot90 for other sides. */
export function transitionTile(a, b, seed = 't') {
  const r = rng(seed), ra = ramp(a), rb = ramp(b), A = groundTile(a, `${seed}:a`), B = groundTile(b, `${seed}:b`), out = new Pixels(T, T);
  let edge = 7;
  for (let x = 0; x < T; x++) {
    edge = Math.max(5, Math.min(10, edge + r.pick([-1, 0, 0, 0, 1])));
    for (let y = 0; y < T; y++) out.set(x, y, y < edge ? A.get(x, y) : B.get(x, y));
    out.set(x, edge, rb[0]); out.set(x, edge - 1, ra[1]);
  }
  return out;
}

const mk = () => new Pixels(T, T);
const PROPS = {
  tree: () => { const m = mk(); m.rect(7, 10, 2, 5, 2).ellipse(8, 6, 5.5, 5, 1).ellipse(5, 8, 3.5, 3, 1).ellipse(11, 8, 3.5, 3, 1); return [m, { 1: ramp('grass'), 2: ramp('wood') }]; },
  pine: () => { const m = mk(); m.rect(7, 12, 2, 3, 2).poly([[8, 1], [12, 6], [4, 6]], 1).poly([[8, 4], [13, 10], [3, 10]], 1).poly([[8, 7], [14, 13], [2, 13]], 1); return [m, { 1: ramp('green'), 2: ramp('wood') }]; },
  bush: () => { const m = mk(); m.ellipse(8, 10.5, 6, 4, 1).ellipse(5, 9.5, 3, 3, 1).ellipse(11, 9.5, 3, 3, 1); return [m, { 1: ramp('grass') }]; },
  rock: () => { const m = mk(); m.poly([[2, 14], [3, 8], [7, 5], [12, 7], [14, 14]], 1); return [m, { 1: ramp('stone') }]; },
  stump: () => { const m = mk(); m.rect(5, 8, 6, 6, 1).ellipse(8, 8, 3, 1.5, 2); return [m, { 1: ramp('wood'), 2: ramp('tan' in {} ? 'wood' : 'sand') }]; },
  cactus: () => { const m = mk(); m.rect(7, 3, 3, 12, 1).rect(4, 7, 3, 2, 1).rect(4, 5, 1, 4, 1).rect(10, 9, 3, 2, 1).rect(12, 7, 1, 4, 1); return [m, { 1: ramp('green') }]; },
  crystal: () => { const m = mk(); m.poly([[8, 1], [11, 5], [10, 14], [6, 14], [5, 5]], 1).poly([[3, 14], [4, 9], [6, 14]], 1).poly([[11, 14], [12, 10], [14, 14]], 1); return [m, { 1: ramp('glass') }]; },
  mushroom: () => { const m = mk(); m.rect(7, 9, 2, 5, 2).ellipse(8, 8, 5, 3.5, 1); m.rect(0, 11, 0, 0, 0); return [m, { 1: ramp('red'), 2: ramp('bone') }]; },
  reeds: () => { const m = mk(); for (const [x, h] of [[4, 9], [7, 12], [10, 8], [12, 10]]) m.rect(x, 14 - h, 1, h, 1).set(x, 14 - h - 1, 2); return [m, { 1: ramp('grass'), 2: ramp('wood') }]; },
  pillar: () => { const m = mk(); m.rect(5, 4, 6, 9, 1).rect(4, 2, 8, 2, 1).rect(4, 13, 8, 2, 1); return [m, { 1: ramp('stone') }]; },
  obelisk: () => { const m = mk(); m.poly([[8, 1], [11, 4], [11, 14], [5, 14], [5, 4]], 1).rect(7, 6, 2, 1, 2).rect(7, 9, 2, 1, 2); return [m, { 1: ramp('sand'), 2: ramp('wood') }]; },
  bones: () => { const m = mk(); m.ellipse(7, 10, 3, 3, 1).thick(3, 13, 13, 12, 1, 1).set(11, 11, 1); m.rect(6, 10, 1, 1, 0).rect(8, 10, 1, 1, 0); return [m, { 1: ramp('bone') }]; },
  fence: () => { const m = mk(); for (const x of [2, 7, 12]) m.rect(x, 5, 2, 9, 1); m.rect(2, 7, 12, 1, 2).rect(2, 10, 12, 1, 2); return [m, { 1: ramp('wood'), 2: ramp('wood') }]; },
  door: () => { const m = mk(); m.rect(3, 4, 10, 11, 1).ellipse(8, 4, 5, 3, 1).rect(7, 9, 2, 2, 2); return [m, { 1: ramp('wood'), 2: ramp('gold') }]; },
  // Exits for the CoffeeMUD kit: a gate, grate, curtain, ladder, stairs, pit and the locked, trapped and hidden doors.
  gate: () => { const m = mk(); m.rect(1, 3, 3, 12, 2).rect(12, 3, 3, 12, 2).rect(1, 2, 3, 1, 3).rect(12, 2, 3, 1, 3); for (const x of [5, 7, 9, 11]) m.rect(x, 4, 1, 11, 1); m.rect(4, 6, 8, 1, 1).rect(4, 11, 8, 1, 1); return [m, { 1: ramp('iron'), 2: ramp('stone'), 3: ramp('steel') }]; },
  grate: () => { const m = mk(); m.rect(2, 4, 12, 9, 2).rect(3, 5, 10, 7, 0); for (const x of [4, 6, 8, 10, 12]) m.rect(x, 5, 1, 7, 1); m.rect(3, 7, 10, 1, 1).rect(3, 10, 10, 1, 1); return [m, { 1: ramp('iron'), 2: ramp('stone') }]; },
  curtain: () => { const m = mk(); m.rect(1, 2, 14, 2, 2); m.poly([[2, 4], [8, 4], [7, 9], [8, 14], [2, 14], [3, 9]], 1).poly([[8, 4], [14, 4], [13, 9], [14, 14], [8, 14], [9, 9]], 1); m.line(5, 5, 5, 13, 3).line(11, 5, 11, 13, 3); return [m, { 1: ramp('red'), 2: ramp('wood'), 3: ramp('iron') }]; },
  ladder: () => { const m = mk(); m.rect(4, 1, 2, 14, 1).rect(10, 1, 2, 14, 1); for (const y of [3, 6, 9, 12]) m.rect(6, y, 4, 1, 2); return [m, { 1: ramp('wood'), 2: ramp('leather') }]; },
  stairs: () => { const m = mk(); m.poly([[2, 14], [2, 11], [5, 11], [5, 8], [8, 8], [8, 5], [11, 5], [11, 2], [14, 2], [14, 14]], 1); for (const y of [5, 8, 11]) m.rect(2, y + 3 - 1, 12, 1, 2); return [m, { 1: ramp('stone'), 2: ramp('iron') }]; },
  pit: () => { const m = mk(); m.ellipse(8, 9, 7, 4.5, 1).ellipse(8, 9.5, 5, 3, 2).ellipse(8, 10, 3, 1.8, 3); return [m, { 1: ramp('dirt'), 2: ramp('stone'), 3: [C.night, C.night, C.night, C.night] }]; },
  doorlocked: () => { const m = mk(); m.rect(3, 4, 10, 11, 1).ellipse(8, 4, 5, 3, 1).rect(3, 7, 10, 1, 3).rect(3, 11, 10, 1, 3).rect(6, 8, 4, 4, 2).rect(7, 6, 2, 3, 2).rect(7, 9, 2, 2, 0); return [m, { 1: ramp('wood'), 2: ramp('gold'), 3: ramp('iron') }]; },
  doortrapped: () => { const m = mk(); m.rect(3, 4, 10, 11, 1).ellipse(8, 4, 5, 3, 1).rect(7, 9, 2, 2, 2); m.set(5, 7, 3).set(10, 7, 3).set(5, 10, 3).set(10, 10, 3).set(7, 6, 3).set(8, 6, 3); return [m, { 1: ramp('wood'), 2: ramp('gold'), 3: ramp('red') }]; },
  hiddendoor: () => { const m = mk(); m.rect(2, 1, 12, 14, 1).rect(4, 4, 8, 11, 2).rect(4, 4, 8, 1, 3).rect(10, 9, 1, 1, 3); return [m, { 1: ramp('stone'), 2: ramp('stone'), 3: ramp('iron') }]; },
  // Dungeon dressing for Idle Solo's rooms: a cabinet, graves, a sarcophagus, bones, thorns, a barrel, rubble.
  cabinet: () => { const m = mk(); m.rect(3, 2, 10, 13, 1).rect(7, 3, 2, 11, 2).set(6, 8, 3).set(9, 8, 3); return [m, { 1: ramp('wood'), 2: ramp('dirt'), 3: ramp('gold') }]; },
  gravecross: () => { const m = mk(); m.rect(7, 2, 2, 12, 1).rect(4, 5, 8, 2, 1).rect(3, 13, 10, 2, 2); return [m, { 1: ramp('stone'), 2: ramp('dirt') }]; },
  slab: () => { const m = mk(); m.rect(2, 8, 12, 6, 1).rect(1, 6, 14, 3, 2).rect(6, 10, 4, 1, 2); return [m, { 1: ramp('stone'), 2: ramp('iron') }]; },
  tombstone: () => { const m = mk(); m.rect(4, 6, 8, 9, 1).ellipse(8, 6, 4, 3.5, 1).rect(7, 4, 2, 6, 2).rect(5, 6, 6, 2, 2); return [m, { 1: ramp('stone'), 2: ramp('iron') }]; },
  tombblue: () => { const m = mk(); m.rect(4, 6, 8, 9, 1).ellipse(8, 6, 4, 3.5, 1).rect(7, 4, 2, 6, 2).rect(5, 6, 6, 2, 2); return [m, { 1: ramp('blue'), 2: ramp('stone') }]; },
  ribs: () => { const m = mk(); m.rect(7, 2, 2, 12, 1); for (const y of [4, 7, 10]) m.line(2, y + 2, 7, y, 1).line(14, y + 2, 8, y, 1); return [m, { 1: ramp('bone') }]; },
  skull: () => { const m = mk(); m.ellipse(8, 7.5, 5, 4.5, 1).rect(5, 10, 6, 4, 1).rect(5, 7, 2, 2, 2).rect(9, 7, 2, 2, 2).rect(7, 10, 2, 1, 2).rect(6, 13, 1, 1, 2).rect(8, 13, 1, 1, 2).rect(10, 13, 1, 1, 2); return [m, { 1: ramp('bone'), 2: ramp('iron') }]; },
  thorns: () => { const m = mk(); m.poly([[8, 1], [10, 7], [15, 4], [12, 11], [15, 15], [1, 15], [4, 10], [1, 5], [6, 7]], 1); return [m, { 1: ramp('green') }]; },
  thornsmall: () => { const m = mk(); m.poly([[8, 6], [10, 10], [13, 8], [12, 13], [14, 15], [2, 15], [4, 12], [3, 9], [6, 10]], 1); return [m, { 1: ramp('green') }]; },
  tuft: () => { const m = mk(); for (const [x, y, h] of [[4, 14, 6], [7, 14, 9], [10, 14, 7], [12, 14, 5]]) m.line(x, y, x - 1, y - h, 1).line(x + 1, y, x, y - h, 1); return [m, { 1: ramp('grass') }]; },
  barrel: () => { const m = mk(); m.rect(4, 3, 8, 11, 1).rect(3, 5, 10, 7, 1).rect(3, 5, 10, 1, 2).rect(3, 11, 10, 1, 2).ellipse(8, 3, 4, 1.5, 3); return [m, { 1: ramp('wood'), 2: ramp('iron'), 3: ramp('dirt') }]; },
  rubble: () => { const m = mk(); m.poly([[1, 14], [2, 10], [6, 9], [7, 14]], 1).poly([[6, 14], [8, 7], [12, 6], [14, 14]], 1).poly([[11, 14], [12, 11], [15, 12], [15, 14]], 1); return [m, { 1: ramp('stone') }]; },
  pillarmoss: () => { const m = mk(); m.rect(5, 4, 6, 9, 1).rect(4, 2, 8, 2, 1).rect(4, 13, 8, 2, 1).rect(5, 5, 2, 4, 2).rect(9, 8, 2, 4, 2); return [m, { 1: ramp('stone'), 2: ramp('moss') }]; },
  // CoffeeMUD remaining gaps. Exits: a gap, a skyway, an airlock. Traps: needle, dart, gas, spikes, alarm, ward. Vehicles: boat, tent, cart. Tech and space: terminal, robot, scanner, star, planet, gas giant, moonlet, asteroid.
  gap: () => { const m = mk(); m.rect(1, 3, 4, 11, 1).rect(11, 3, 4, 11, 1).rect(5, 3, 6, 11, 2).rect(5, 3, 6, 1, 3); m.set(4, 13, 3).set(11, 13, 3); return [m, { 1: ramp('dirt'), 2: [C.night, C.night, C.night, C.plum], 3: ramp('stone') }]; },
  skyway: () => { const m = mk(); m.rect(1, 9, 14, 2, 1); m.rect(1, 4, 1, 7, 2).rect(14, 4, 1, 7, 2); m.line(1, 5, 14, 5, 3).line(1, 7, 14, 7, 3); for (const x of [4, 7, 10]) m.rect(x, 11, 1, 3, 2); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('sand') }]; },
  airlock: () => { const m = mk(); m.ellipse(8, 8, 7, 7, 1).ellipse(8, 8, 5, 5, 2).rect(7, 4, 2, 8, 3).rect(4, 7, 8, 2, 3); m.set(2, 2, 4).set(13, 2, 4).set(2, 13, 4).set(13, 13, 4); return [m, { 1: ramp('steel'), 2: ramp('iron'), 3: ramp('steel'), 4: ramp('red') }]; },
  trapneedle: () => { const m = mk(); m.rect(2, 8, 12, 6, 1).rect(2, 6, 12, 3, 2).rect(7, 9, 2, 2, 3); m.line(8, 1, 8, 5, 4).set(8, 6, 4); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('gold'), 4: ramp('green') }]; },
  trapdart: () => { const m = mk(); m.rect(1, 2, 14, 12, 1).rect(1, 6, 4, 4, 2).rect(2, 7, 2, 2, 0); m.rect(6, 7, 6, 1, 3).poly([[12, 6], [14, 7], [12, 9]], 3); m.set(6, 6, 4).set(6, 8, 4); return [m, { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('steel'), 4: ramp('red') }]; },
  trapgas: () => { const m = mk(); m.rect(3, 10, 10, 4, 1); for (const x of [4, 7, 10]) m.rect(x, 10, 1, 4, 0); m.ellipse(6, 7, 3, 2.5, 2).ellipse(10, 5, 3, 2.5, 2).ellipse(7, 3, 2, 1.8, 2); return [m, { 1: ramp('iron'), 2: ramp('green') }]; },
  trapspikes: () => { const m = mk(); m.rect(1, 9, 14, 5, 1); for (const x of [2, 5, 8, 11]) m.poly([[x, 9], [x + 1, 2], [x + 2, 9]], 2); return [m, { 1: ramp('dirt'), 2: ramp('steel') }]; },
  trapalarm: () => { const m = mk(); m.rect(1, 6, 2, 8, 1).rect(13, 6, 2, 8, 1); m.line(3, 7, 12, 7, 2); m.ellipse(8, 11, 3, 2.5, 3).rect(7, 8, 2, 1, 2).set(8, 13, 2); return [m, { 1: ramp('wood'), 2: ramp('iron'), 3: ramp('gold') }]; },
  trapward: () => { const m = mk(); m.ellipse(8, 9, 7, 5, 1).ellipse(8, 9, 5, 3, 0); m.rect(7, 6, 2, 7, 2).rect(5, 8, 6, 2, 2); m.set(2, 4, 2).set(13, 4, 2); return [m, { 1: ramp('purple'), 2: ramp('glass') }]; },
  boat: () => { const m = mk(); m.poly([[1, 10], [15, 10], [12, 14], [4, 14]], 1); m.rect(8, 2, 1, 8, 2); m.poly([[9, 2], [14, 9], [9, 9]], 3); m.rect(1, 10, 14, 1, 2); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('bone') }]; },
  tent: () => { const m = mk(); m.poly([[8, 2], [15, 14], [1, 14]], 1).poly([[8, 7], [11, 14], [5, 14]], 2); m.rect(8, 1, 1, 2, 3); return [m, { 1: ramp('red'), 2: [C.night, C.night, C.plum, C.plum], 3: ramp('wood') }]; },
  cart: () => { const m = mk(); m.rect(2, 6, 12, 4, 1).rect(4, 3, 5, 3, 3); m.ellipse(5, 12, 2.5, 2.5, 2).ellipse(11, 12, 2.5, 2.5, 2); m.rect(5, 12, 1, 1, 0).rect(11, 12, 1, 1, 0); m.line(13, 8, 14, 4, 3); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('sand') }]; },
  terminal: () => { const m = mk(); m.rect(2, 2, 12, 9, 1).rect(3, 3, 10, 7, 2).rect(4, 4, 5, 1, 3).rect(4, 6, 7, 1, 3).rect(4, 8, 3, 1, 3).rect(6, 11, 4, 2, 1).rect(3, 13, 10, 2, 1); return [m, { 1: ramp('iron'), 2: [C.night, C.night, C.indigo, C.indigo], 3: ramp('green') }]; },
  robot: () => { const m = mk(); m.rect(4, 2, 8, 5, 1).rect(5, 4, 2, 1, 3).rect(9, 4, 2, 1, 3).rect(3, 8, 10, 5, 1).rect(6, 9, 4, 2, 2).rect(1, 8, 2, 5, 1).rect(13, 8, 2, 5, 1).rect(5, 13, 2, 2, 1).rect(9, 13, 2, 2, 1).rect(7, 7, 2, 1, 2); return [m, { 1: ramp('steel'), 2: ramp('iron'), 3: ramp('red') }]; },
  scanner: () => { const m = mk(); m.rect(4, 3, 8, 11, 1).rect(5, 4, 6, 5, 2).rect(6, 11, 1, 1, 3).rect(9, 11, 1, 1, 3).rect(11, 1, 1, 3, 3); m.set(7, 6, 4).set(8, 7, 4); return [m, { 1: ramp('iron'), 2: [C.night, C.night, C.forest, C.forest], 3: ramp('steel'), 4: ramp('green') }]; },
  asteroid: () => { const m = mk(); m.poly([[3, 8], [5, 3], [10, 2], [14, 6], [13, 12], [7, 14], [3, 12]], 1); m.set(6, 6, 2).set(7, 6, 2).set(10, 9, 2).set(9, 9, 2).set(6, 11, 2); return [m, { 1: ramp('stone'), 2: ramp('iron') }]; },
  moonlet: () => { const m = mk(); m.ellipse(8, 8, 5, 5, 1); m.set(6, 6, 2).set(10, 7, 2).set(8, 10, 2).set(7, 10, 2); return [m, { 1: ramp('stone'), 2: ramp('iron') }]; },
  star: () => { const m = mk(); m.ellipse(8, 8, 4, 4, 1); for (const [a, b] of [[8, 1], [8, 14], [1, 8], [14, 8], [3, 3], [12, 3], [3, 12], [12, 12]]) m.line(8, 8, a, b, 2); m.ellipse(8, 8, 4, 4, 1); return [m, { 1: ramp('fire'), 2: ramp('gold') }]; },
  planet: () => { const m = mk(); m.ellipse(8, 8, 6.5, 6.5, 1); m.ellipse(6, 6, 2.5, 2, 2).ellipse(10, 10, 2.5, 3, 2).set(11, 5, 2); return [m, { 1: ramp('water'), 2: ramp('grass') }]; },
  gasgiant: () => { const m = mk(); m.ellipse(8, 8, 6.5, 6.5, 1); for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) if (m.get(x, y) === 1 && (y === 5 || y === 6 || y === 10)) m.set(x, y, 2); m.ellipse(10, 8, 1.5, 1, 3); return [m, { 1: ramp('sand'), 2: ramp('leather'), 3: ramp('red') }]; },
  shrine: () => { const m = mk(); m.rect(4, 12, 8, 2, 1).rect(6, 6, 4, 6, 1).poly([[5, 6], [8, 2], [11, 6]], 2); return [m, { 1: ramp('stone'), 2: ramp('moss') }]; },
};
/** Props that need 32x32: a house, a sailing ship, a covered wagon, a clan hall, a spaceship. */
const BIG = {
  ship: () => { const m = new Pixels(32, 32); m.poly([[2, 21], [30, 21], [26, 29], [6, 29]], 1).rect(2, 21, 28, 2, 2); m.rect(10, 3, 2, 18, 2).rect(20, 5, 2, 16, 2); m.poly([[12, 4], [19, 5], [19, 11], [12, 11]], 3).poly([[12, 12], [19, 12], [19, 19], [12, 19]], 3).poly([[22, 6], [28, 11], [28, 19], [22, 19]], 3); m.poly([[10, 1], [15, 2], [10, 3]], 4); m.rect(4, 24, 2, 1, 2).rect(13, 24, 2, 1, 2).rect(22, 24, 2, 1, 2); return shade(m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('bone'), 4: ramp('red') }); },
  caravan: () => { const m = new Pixels(32, 32); m.rect(4, 12, 22, 10, 1).poly([[3, 12], [8, 3], [22, 3], [27, 12]], 3).rect(12, 14, 6, 6, 5); m.ellipse(9, 25, 4.5, 4.5, 2).ellipse(21, 25, 4.5, 4.5, 2).ellipse(9, 25, 1.5, 1.5, 4).ellipse(21, 25, 1.5, 1.5, 4); m.line(26, 20, 30, 23, 2); return shade(m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('bone'), 4: ramp('iron'), 5: [C.night, C.night, C.plum, C.plum] }); },
  clanhall: () => { const m = new Pixels(32, 32); m.rect(3, 13, 26, 17, 1).poly([[1, 14], [16, 3], [31, 14]], 2).rect(13, 21, 6, 9, 3).rect(6, 16, 3, 8, 4).rect(23, 16, 3, 8, 4).rect(15, 1, 1, 3, 3).rect(16, 1, 3, 2, 4); m.rect(6, 24, 3, 1, 3).rect(23, 24, 3, 1, 3); return shade(m, { 1: ramp('stone'), 2: ramp('red'), 3: ramp('wood'), 4: ramp('gold') }); },
  spaceship: () => { const m = new Pixels(32, 32); m.poly([[3, 16], [11, 11], [26, 12], [30, 16], [26, 20], [11, 21]], 1).poly([[12, 11], [8, 3], [17, 11]], 1).poly([[12, 21], [8, 29], [17, 21]], 1).ellipse(22, 16, 3, 2.2, 2).rect(3, 14, 2, 5, 3).poly([[2, 15], [1, 17], [2, 18]], 4); return shade(m, { 1: ramp('steel'), 2: ramp('glass'), 3: ramp('iron'), 4: ramp('fire') }); },
};
export const PROP_IDS = [...Object.keys(PROPS), 'torch', 'chest', 'house', ...Object.keys(BIG), ...SCENERY_IDS];

/** One prop. `torch` and `chest` reuse the item shapes; `house` is 32x32. */
export function drawProp(kind) {
  if (kind === 'torch' || kind === 'chest') return drawItem({ shape: kind, tier: 'crafted', seed: 'prop' }).frames[0];
  if (kind === 'house') {
    const m = new Pixels(32, 32); m.rect(4, 15, 24, 15, 1).poly([[1, 16], [16, 3], [31, 16]], 2).rect(23, 5, 3, 7, 3).rect(13, 20, 6, 10, 4).rect(7, 19, 4, 4, 5).rect(21, 19, 4, 4, 5);
    const out = shade(m, { 1: ramp('bone'), 2: ramp('red'), 3: ramp('stone'), 4: ramp('wood'), 5: ramp('glass') });
    for (let y = 5; y < 16; y += 3) for (let x = 0; x < 32; x++) if (out.get(x, y) && m.get(x, y) === 2) out.set(x, y, ramp('red')[1]);
    return out;
  }
  if (BIG[kind]) return BIG[kind]();
  const scenery = drawScenery(kind); if (scenery) return scenery;
  const make = PROPS[kind]; if (!make) throw new Error(`unknown prop "${kind}"`);
  const [m, parts] = make(); return shade(m, parts);
}

/** A small readable scene: ground, a path with soft edges, props clustered off the path, one landmark at the far end (the biomes kit's readability checklist). */
export function drawScene({ biome = 'forest', seed = 'scene', cols = 12, rows = 8, layers = false, weather, inhabitants, evidence, depth } = {}) {
  const b = BIOMES[biome]; if (!b) throw new Error(`unknown biome "${biome}"`);
  const r = rng(`${seed}:${biome}`), out = new Pixels(cols * T, rows * T), grounds = [groundTile(b.ground, `${seed}:a`), groundTile(b.ground, `${seed}:b`)];
  const wallRows = b.walls ? 2 : 0, pathRow = Math.floor(rows / 2) + 1, path = [];
  let py = pathRow;
  for (let x = 0; x < cols; x++) { py = Math.max(wallRows + 2, Math.min(rows - 2, py + r.pick([-1, 0, 0, 0, 0, 1]))); path.push(py); }
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) {
    let tile;
    if (y < wallRows) tile = wallTile(b.wall, `${seed}:w${x}${y}`);
    else if (y === path[x]) tile = b.path === 'water' ? waterTile(`${seed}:${x}`) : groundTile(b.path, `${seed}:p${x}`);
    else if (y === path[x] - 1) tile = transitionTile(b.ground, b.path === 'water' ? 'water' : b.path, `${seed}:t${x}`);
    else if (y === path[x] + 1) tile = transitionTile(b.ground, b.path === 'water' ? 'water' : b.path, `${seed}:b${x}`).flipY();
    else tile = grounds[(x + y) % 2];
    out.blit(tile, x * T, y * T);
  }
  const taken = new Set();
  for (let i = 0; i < Math.floor((cols * rows) / 10); i++) {
    const x = r.int(cols - 2), y = r.range(wallRows, rows - 1);
    if (Math.abs(y - path[x]) <= 1 || taken.has(`${x},${y}`)) continue;
    taken.add(`${x},${y}`); out.blit(drawProp(r.pick(b.props)), x * T, y * T - 2);
  }
  const lx = cols - 2, mark = drawProp(b.landmark), big = mark.w;
  out.blit(mark, lx * T - (big - T) / 2 + 4, Math.max(0, path[cols - 1] * T - (big - T) - 14));
  // The optional layers (recipes/scene-layers.js), each from its own forked stream so the scene above never changes.
  const L = SCENE_LAYERS[biome], lr = rng(`${seed}:${biome}:layers`), ctx = { out, path, cols, rows, wallRows, taken, b, drawProp };
  if (wants(evidence, layers)) drawEvidence(ctx, L.evidence, lr.fork('evidence'));
  if (wants(inhabitants, layers)) drawInhabitants(ctx, L.folk, lr.fork('inhabitants'));
  if (wants(depth, layers)) drawDepth(ctx, lr.fork('depth'));
  const sky = weather === true || (weather === undefined && layers) ? L.weather : weather || null;
  if (sky) drawWeather(out, sky, lr.fork('weather'));
  return { id: `scene.${biome}`, frames: [out], fps: 0, loop: false };
}
