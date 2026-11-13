// Dungeon tiles seen from above (T15, `tile` with `kind: 'dungeon'` and `view: 'top'`): a floor, a wall that is a flat top with no face, and the pieces of a
// level around them. Tiles are 32 by 32 (the world's grid) or 16 by 16, and a 32 tile is drawn again with more stones and grain, not enlarged.
//
// The autotile is the 4-neighbour edge set plus corner notches, the 47 shapes of a blob: `mask` has a bit for each side whose neighbour is the same kind
// (north 1, east 2, south 4, west 8), and `notch` a bit for each inner corner where both sides join but the diagonal does not (north east 1, south east 2,
// south west 4, north west 8). A wall's open sides get a dark edge with a lit rim on the north and west, where the light comes from; a floor's get the wall's
// shadow, deep on the north and west and a thin dark line on the east and south.
import { Pixels } from '../pixels.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { stepColor } from './enemy-clips.js';

export const TOP_SIZES = [16, 32];
export const TOP_PARTS = ['floor', 'wall', 'pillar', 'broken', 'doorway', 'stairs-down', 'stairs-up', 'pit', 'lava', 'toxic', 'bridge', 'rubble', 'cracks', 'shade', 'ladder-down', 'ladder-up', 'cliff', 'chasm', 'water', 'gate'];
export const DOOR_STATES = ['open', 'closed', 'locked'];
export const TOP_FACINGS = ['north', 'east', 'south', 'west'];
const N = 1, E = 2, S_ = 4, W = 8;
/** The notch bits, and the two side bits each one needs. */
export const NOTCHES = [[1, N | E], [2, E | S_], [4, S_ | W], [8, W | N]];

/** What each dungeon biome looks like from above: the ramp of its floor and its wall top, how the floor is laid, and the colour of its crevices. */
export const TOP_LOOKS = {
  crypt: { floor: 'iron', wall: 'stone', style: 'flag', glow: null },
  cave: { floor: 'dirt', wall: 'iron', style: 'ground', glow: null },
  forge: { floor: 'iron', wall: 'steel', style: 'flag', glow: 'fire' },
  flooded: { floor: 'water', wall: 'moss', style: 'water', glow: null },
  frozen: { floor: 'steel', wall: 'ice', style: 'ground', glow: null },
  overgrown: { floor: 'grass', wall: 'moss', style: 'ground', glow: null },
  ancient: { floor: 'sand', wall: 'bone', style: 'flag', glow: 'gold', inlay: true },
  volcanic: { floor: 'iron', wall: 'dirt', style: 'ground', glow: 'fire' },
  fungal: { floor: 'moss', wall: 'bone', style: 'ground', glow: 'green' },
  arcane: { floor: 'blue', wall: 'purple', style: 'flag', glow: 'glass', inlay: true },
  void: { floor: 'iron', wall: 'purple', style: 'ground', glow: 'purple' },
  mechanical: { floor: 'steel', wall: 'iron', style: 'flag', glow: 'gold', rivets: true },
};
export const TOP_BIOMES = Object.keys(TOP_LOOKS);

const wrap = (v, n) => ((v % n) + n) % n;

/** A tileable floor, mostly the middle values of its ramp so a wall top reads lighter. */
function floorTexture(look, S, seed) {
  const r = rng(seed), rp = ramp(look.floor), out = new Pixels(S, S), u = S / 16;
  if (look.style === 'flag') {
    for (let row = 0; row < S / 8; row++) for (let bx = 0; bx < S / 8; bx++) {
      const x0 = bx * 8 + (row % 2 ? 4 : 0), y0 = row * 8, v = r.pick([1, 1, 2, 1]);
      for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) out.set(wrap(x0 + x, S), y0 + y, y === 7 || x === 7 ? rp[0] : y === 0 || x === 0 ? rp[Math.min(3, v + 1)] : rp[v]);
      if (look.rivets) for (const [rx, ry] of [[1, 1], [6, 1], [1, 6], [6, 6]]) out.set(wrap(x0 + rx, S), y0 + ry, rp[0]).set(wrap(x0 + rx - 1, S), y0 + ry - 1, rp[3]);
      if (look.inlay && r.chance(0.5)) { const g = ramp('gold'); out.set(wrap(x0 + 3, S), y0 + 3, g[2]).set(wrap(x0 + 4, S), y0 + 3, g[1]).set(wrap(x0 + 3, S), y0 + 4, g[1]).set(wrap(x0 + 4, S), y0 + 4, g[2]); }
    }
  } else if (look.style === 'water') {
    out.rect(0, 0, S, S, rp[1]);
    for (let i = 0; i < 4 * u * u; i++) { const x = r.int(S), y = r.int(S), w = 3 * u; for (let k = 0; k < w; k++) out.set(wrap(x + k, S), y, rp[3]); for (let k = 1; k < w - 1; k++) out.set(wrap(x + k, S), wrap(y + 1, S), rp[2]); }
  } else {
    const pts = Array.from({ length: Math.round(7 * u * u) }, () => [r.int(S), r.int(S), r.pick([1, 1, 1, 2])]);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) {
      let best = 1e9, v = 1;
      for (const [px, py, pv] of pts) { const dx = Math.min(Math.abs(x - px), S - Math.abs(x - px)), dy = Math.min(Math.abs(y - py), S - Math.abs(y - py)), d = dx * dx + dy * dy; if (d < best) { best = d; v = pv; } }
      out.set(x, y, rp[v]);
    }
  }
  // Grain: single pixels one step either way, more of them on the larger tile.
  for (let i = 0; i < Math.round(10 * u * u); i++) { const x = r.int(S), y = r.int(S); out.set(x, y, stepColor(out.get(x, y), r.chance(0.5) ? 1 : -1)); }
  if (look.glow && look.style !== 'water') crack(out, look, S, r, S > 16 ? 2 : 1, true);
  return out;
}

/** Cracks: seeded wandering lines in the floor's darkest colour, with the biome's glow inside them when it has one. */
function crack(out, look, S, r, count, glowOnly = false) {
  const rp = ramp(look.floor), g = look.glow ? ramp(look.glow) : null;
  for (let i = 0; i < count; i++) {
    let x = r.int(S), y = r.int(S);
    for (let k = 0; k < S * 0.7; k++) {
      if (!glowOnly || g) out.set(wrap(x, S), wrap(y, S), g ? g[1 + (k & 1)] : rp[0]);
      if (g && !glowOnly) out.set(wrap(x + 1, S), wrap(y, S), rp[0]);
      x += r.pick([-1, 0, 1, 1]); y += r.pick([0, 1, 1, -1]);
    }
  }
}

const dim = (px, rect, n) => { for (let y = rect[1]; y < rect[1] + rect[3]; y++) for (let x = rect[0]; x < rect[0] + rect[2]; x++) if (px.get(x, y)) px.set(x, y, stepColor(px.get(x, y), n)); };

/** The wall's shadow on a floor tile: `mask` says which sides have a wall. */
function shadowed(px, mask, S) {
  const u = S / 16, out = px.clone();
  const band = (rect, deep) => { dim(out, rect, -1); if (deep) { for (let y = rect[1]; y < rect[1] + rect[3]; y++) for (let x = rect[0]; x < rect[0] + rect[2]; x++) if ((x + y) % 2 === 0) out.set(x, y, stepColor(out.get(x, y), -1)); } };
  if (mask & N) { band([0, 0, S, 2 * u], true); dim(out, [0, 2 * u, S, u], -1); }
  if (mask & W) { band([0, 0, 2 * u, S], true); dim(out, [2 * u, 0, u, S], -1); }
  if (mask & E) { for (let y = 0; y < S; y++) if ((y & 1) === 0) out.set(S - 1, y, stepColor(out.get(S - 1, y), -1)); }
  if (mask & S_) { for (let x = 0; x < S; x++) if ((x & 1) === 0) out.set(x, S - 1, stepColor(out.get(x, S - 1), -1)); }
  return out;
}

/** A wall top: slabs laid flat, lit from the top left, with the edge on every side that does not join another wall and a notch at each inner corner. */
function wallTop(look, S, seed, mask) {
  const r = rng(seed), rp = ramp(look.wall), out = new Pixels(S, S), u = S / 16;
  const bw = 8, bh = 8;
  for (let row = 0; row < S / bh; row++) for (let bx = 0; bx < S / bw; bx++) {
    const x0 = bx * bw + (row % 2 ? 4 : 0), y0 = row * bh, v = r.pick([2, 2, 3, 2]);
    for (let y = 0; y < bh; y++) for (let x = 0; x < bw; x++) out.set(wrap(x0 + x, S), y0 + y, y === bh - 1 || x === bw - 1 ? rp[1] : y === 0 || x === 0 ? rp[3] : rp[v]);
  }
  for (let i = 0; i < Math.round(8 * u * u); i++) { const x = r.int(S), y = r.int(S); out.set(x, y, stepColor(out.get(x, y), r.chance(0.5) ? 1 : -1)); }
  const t = u;                                              // edge thickness
  const open = (side) => !(mask & side);
  if (open(N)) { out.rect(0, 0, S, t, C.night); out.rect(0, t, S, t, rp[3]); }
  if (open(W)) { out.rect(0, 0, t, S, C.night); out.rect(t, 0, t, S, rp[3]); }
  if (open(S_)) { out.rect(0, S - t, S, t, C.night); out.rect(0, S - 2 * t, S, t, rp[1]); }
  if (open(E)) { out.rect(S - t, 0, t, S, C.night); out.rect(S - 2 * t, 0, t, S, rp[1]); }
  return out;
}

/** The corner squares a notch cuts out of a wall (the floor shows through), where both its sides join. */
function notchHole(S, notch, mask) {
  const hole = new Pixels(S, S), t = S / 16;
  for (const [bit, sides] of NOTCHES) {
    if (!(notch & bit) || (mask & sides) !== sides) continue;
    const right = bit === 1 || bit === 2, down = bit === 2 || bit === 4;
    hole.rect(right ? S - 2 * t : 0, down ? S - 2 * t : 0, 2 * t, 2 * t, 1);
  }
  return hole;
}

const axisOf = (facing) => (facing === 'north' || facing === 'south' ? 'v' : 'h');

/** A hole in the floor (a pit, lava, toxic water): open sides (`mask` bit clear) are lipped, joined sides run on into the next tile. */
function hollow(look, S, seed, mask, fill) {
  const out = floorTexture(look, S, seed), t = S / 16 * 2, r = rng(`${seed}:hole`);
  const x0 = mask & W ? 0 : t, y0 = mask & N ? 0 : t, x1 = S - (mask & E ? 0 : t), y1 = S - (mask & S_ ? 0 : t);
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) out.set(x, y, fill(x, y, r));
  // The lip: one pixel of the floor's lightest value on the north and west rim, the darkest on the others.
  const fp = ramp(look.floor);
  for (let x = x0; x < x1; x++) { if (!(mask & N)) out.set(x, y0, C.night); if (!(mask & S_)) out.set(x, y1 - 1, C.night); }
  for (let y = y0; y < y1; y++) { if (!(mask & W)) out.set(x0, y, C.night); if (!(mask & E)) out.set(x1 - 1, y, C.night); }
  if (!(mask & N)) for (let x = x0; x < x1; x++) out.set(x, y0 - 1, fp[3]);
  if (!(mask & W)) for (let y = y0; y < y1; y++) out.set(x0 - 1, y, fp[3]);
  return out;
}

/**
 * One dungeon tile from above. `part` is one of `TOP_PARTS`; `biome` one of `TOP_BIOMES`; `size` 16 or 32; `mask` and `notch` the autotile bits (see the top of this
 * file); `facing` for a doorway, stairs or a bridge; `state` for a doorway (`open`, `closed`, `locked`); `amount` (0 to 1) for the `shade` overlay. Lava and toxic are two frames.
 */
export function drawTopTile({ biome = 'crypt', part = 'floor', size = 32, mask = 0, notch = 0, facing = 'south', state = 'open', amount = 0.5, seed = 'dungeon' } = {}) {
  const look = TOP_LOOKS[biome]; if (!look) throw new Error(`unknown dungeon biome "${biome}"; known: ${TOP_BIOMES.join(', ')}`);
  if (!TOP_PARTS.includes(part)) throw new Error(`unknown top-down tile part "${part}"; known: ${TOP_PARTS.join(', ')}`);
  if (!TOP_SIZES.includes(size)) throw new Error(`unknown tile size "${size}"; known: ${TOP_SIZES.join(', ')}`);
  if (!TOP_FACINGS.includes(facing)) throw new Error(`unknown facing "${facing}"; known: ${TOP_FACINGS.join(', ')}`);
  if (!DOOR_STATES.includes(state)) throw new Error(`unknown doorway state "${state}"; known: ${DOOR_STATES.join(', ')}`);
  if (!Number.isInteger(mask) || mask < 0 || mask > 15 || !Number.isInteger(notch) || notch < 0 || notch > 15) throw new Error('mask and notch are whole numbers from 0 to 15');
  const S = size, u = S / 16, s = `${seed}:${biome}:${S}`, wp = ramp(look.wall), fp = ramp(look.floor);
  const floor = () => floorTexture(look, S, `${s}:f`);
  let frames;
  if (part === 'floor') frames = [shadowed(floor(), mask, S)];
  else if (part === 'wall') {
    const w = wallTop(look, S, `${s}:w`, mask), hole = notchHole(S, notch, mask);
    if (hole.count()) {
      const f = shadowed(floor(), 0, S), cut = w.map((x, y) => (hole.get(x, y) ? f.get(x, y) : w.get(x, y)));
      frames = [cut.map((x, y) => (!hole.get(x, y) && [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([i, j]) => hole.get(x + i, y + j)) ? C.night : cut.get(x, y)))];
    } else frames = [w];
  } else if (part === 'pillar') {
    const f = shadowed(floor(), mask, S), p = new Pixels(S, S), c = S / 2;
    p.ellipse(c, c, 6 * u, 6 * u, 1).ellipse(c, c, 4 * u, 4 * u, 2).ellipse(c - u, c - u, 2 * u, 2 * u, 3);
    const body = new Pixels(S, S); for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) { const v = p.get(x, y); if (v) body.set(x, y, wp[v === 1 ? 1 : v === 2 ? 2 : 3]); }
    f.ellipse(c + u, c + u, 6.5 * u, 6.5 * u, fp[0]); frames = [f.blit(body.outline(C.night))];
  } else if (part === 'broken') {
    const w = wallTop(look, S, `${s}:w`, mask), f = floor(), r = rng(`${s}:broken`), hole = new Pixels(S, S);
    for (let i = 0; i < 2; i++) hole.poly([[r.range(S / 4, S / 2), r.range(S / 4, S / 2)], [r.range(S / 2, S - 4), r.range(S / 8, S / 2)], [r.range(S / 2, S - 4), r.range(S / 2, S - 4)], [r.range(4, S / 2), r.range(S / 2, S - 4)]], 1);
    const out = w.map((x, y) => (hole.get(x, y) ? f.get(x, y) : w.get(x, y)));
    out.outline(C.night, false); for (let i = 0; i < 6 * u; i++) { const x = r.int(S), y = r.int(S); if (hole.get(x, y)) out.set(x, y, r.chance(0.5) ? wp[2] : wp[1]); }
    frames = [out];
  } else if (part === 'doorway') {
    const f = shadowed(floor(), 0, S), v = axisOf(facing) === 'v', jamb = new Pixels(S, S), w = wallTop(look, S, `${s}:w`, 15), t = 4 * u;
    // The passage runs along the axis; the jambs close the two sides of it.
    if (v) { jamb.rect(0, 0, t, S, 1).rect(S - t, 0, t, S, 1); } else { jamb.rect(0, 0, S, t, 1).rect(0, S - t, S, t, 1); }
    const out = f.map((x, y) => (jamb.get(x, y) ? w.get(x, y) : f.get(x, y)));
    const wood = ramp('wood');
    if (state === 'open') { if (v) { out.rect(t, 1, u, 2 * u, wood[0]); } else out.rect(1, t, 2 * u, u, wood[0]); for (let i = 0; i < S; i += 4 * u) v ? out.rect(t, i, S - 2 * t, u, wood[1]) : out.rect(i, t, u, S - 2 * t, wood[1]); }
    else {
      const slab = new Pixels(S, S), a = v ? [t, S / 2 - 2 * u, S - 2 * t, 4 * u] : [S / 2 - 2 * u, t, 4 * u, S - 2 * t];
      slab.rect(...a, 1);
      for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (slab.get(x, y)) out.set(x, y, (v ? x - t : y - t) % (2 * u + 1) === 0 ? wood[0] : (v ? y : x) < (v ? a[1] + u : a[0] + u) ? wood[3] : wood[2]);
      out.blit(slab.map((x, y) => (slab.get(x, y) && (x === a[0] || y === a[1] || x === a[0] + a[2] - 1 || y === a[1] + a[3] - 1) ? 1 : 0)).silhouette(C.night));
      if (state === 'locked') out.rect(S / 2 - u, S / 2 - u, 2 * u, 2 * u, ramp('gold')[3]).rect(S / 2 - u, S / 2 - 2 * u, 2 * u, u, ramp('gold')[1]);
    }
    frames = [out];
  } else if (part === 'gate') {
    // A portcullis in a doorway: iron bars across the passage. Closed they stand in the opening, open they are drawn up into the lintel (a short stub of each bar is left), locked has a padlock on the middle bar.
    const f = shadowed(floor(), 0, S), v = axisOf(facing) === 'v', jamb = new Pixels(S, S), w = wallTop(look, S, `${s}:w`, 15), t = 4 * u, iron = ramp('iron'), out = f.clone();
    if (v) jamb.rect(0, 0, t, S, 1).rect(S - t, 0, t, S, 1); else jamb.rect(0, 0, S, t, 1).rect(0, S - t, S, t, 1);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (jamb.get(x, y)) out.set(x, y, w.get(x, y));
    const bar = (x, y, w2, h2, col) => out.rect(v ? x : y, v ? y : x, v ? w2 : h2, v ? h2 : w2, col);   // laid out as if the passage ran down the tile, turned when it runs across
    const mid = S / 2, steel = ramp('steel');
    if (state === 'open') for (const side of [t, S - t - 2 * u]) { bar(side, mid - 3 * u, 2 * u, 6 * u, C.night); bar(side, mid - 3 * u, u, 6 * u, steel[2]); bar(side + u, mid - 3 * u, u, 6 * u, iron[1]); }   // drawn up: the bars' stubs at each jamb
    else {
      bar(t, mid - 3 * u, S - 2 * t, 7 * u, C.night);                                                    // the dark the bars stand in
      bar(t, mid - 2 * u, S - 2 * t, u, steel[3]); bar(t, mid + u, S - 2 * t, u, steel[1]);              // the two crossbars
      for (let k = t + u; k < S - t; k += 3 * u) { bar(k, mid - 3 * u, u, 6 * u, steel[2]); bar(k + u, mid - 3 * u, u, 6 * u, iron[0]); bar(k, mid + 3 * u, u, u, steel[3]); }   // the bars, with a point at the bottom
    }
    if (state === 'locked') out.rect(S / 2 - u, S / 2 - u, 2 * u, 2 * u, ramp('gold')[3]).rect(S / 2 - u, S / 2 - 2 * u, 2 * u, u, ramp('gold')[1]);
    frames = [out];
  } else if (part === 'stairs-down' || part === 'stairs-up') {
    const down = part === 'stairs-down', f = shadowed(floor(), 0, S), d = { north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0] }[facing], steps = 4, out = f.clone(), lo = 2 * u, run = (S - 2 * lo) / steps;
    for (let y = lo; y < S - lo; y++) for (let x = lo; x < S - lo; x++) {
      const along = d[0] ? (d[0] > 0 ? x : S - 1 - x) : (d[1] > 0 ? y : S - 1 - y), k = Math.min(steps - 1, Math.floor((along - lo) / run)), lvl = down ? [2, 1, 1, 0][k] : [1, 1, 2, 3][k], nosing = Math.floor((along - lo) % run) === 0;
      out.set(x, y, fp[nosing && lvl < 3 ? lvl + 1 : lvl]);
    }
    out.frame(lo, lo, S - 2 * lo, S - 2 * lo, C.night);
    frames = [out];
  } else if (part === 'pit') {
    const dark = [C.black, C.night, C.night, C.plum];
    frames = [hollow(look, S, `${s}:f`, mask, (x, y) => { const k = ((x * 5 + y * 3) % 11) === 0 ? 2 : 1; return dark[k]; })];
  } else if (part === 'lava' || part === 'toxic' || part === 'water') {
    const rp = part === 'lava' ? ramp('fire') : part === 'water' ? ramp('water') : ramp('green'), bubbles = [[0.3, 0.3], [0.7, 0.6], [0.45, 0.8]];
    frames = [0, 1].map((t) => hollow(look, S, `${s}:f`, mask, (x, y) => {
      const stripe = Math.floor((x + y + t * 2 * u) / (2 * u)) & 1, b = bubbles.some(([bx, by]) => Math.abs(x - bx * S) + Math.abs(y - by * S - t * u) < 1.5 * u);
      return b ? rp[3] : stripe ? rp[1] : rp[2];
    }));
  } else if (part === 'ladder-down' || part === 'ladder-up') {
    // A hatch in the floor with a ladder across it: down, the rails run into the dark and the rungs fade out; up, the hatch is lit from above and the whole ladder shows.
    const down = part === 'ladder-down', v = axisOf(facing) === 'v', out = shadowed(floor(), 0, S), wood = ramp('wood'), c = S / 2, hw = 5 * u, hl = 7 * u;
    const [x0, y0, w, h] = v ? [c - hw, c - hl, 2 * hw, 2 * hl] : [c - hl, c - hw, 2 * hl, 2 * hw];
    out.rect(x0 - u, y0 - u, w + 2 * u, h + 2 * u, C.night).rect(x0, y0, w, h, down ? C.black : ramp('sand')[1]);
    if (!down) out.rect(x0 + u, y0 + u, w - 2 * u, h - 2 * u, ramp('sand')[2]);
    const rail = (a) => (v ? out.rect(a, y0 - 2 * u, u, h + 4 * u, wood[2]).rect(a - 1, y0 - 2 * u, 1, h + 4 * u, C.night) : out.rect(x0 - 2 * u, a, w + 4 * u, u, wood[2]).rect(x0 - 2 * u, a - 1, w + 4 * u, 1, C.night));
    rail(c - 3 * u); rail(c + 2 * u);
    const rungs = Math.floor((v ? h : w) / (3 * u)) + 1;
    for (let k = 0; k < rungs; k++) { const o = (v ? y0 : x0) + k * 3 * u, col = down && k > rungs - 3 ? wood[0] : wood[3]; if (down && k > rungs - 2) continue; v ? out.rect(c - 3 * u, o, 6 * u, u, col) : out.rect(o, c - 3 * u, u, 6 * u, col); }
    frames = [out];
  } else if (part === 'cliff') {
    // The floor ends along the side `facing` names: a lit lip, a rock face with its grain running across the drop, then the dark below. Cliffs placed side by side join.
    const d = { north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0] }[facing], out = shadowed(floor(), 0, S), r = rng(`${s}:cliff`), face = wp;
    const edge = Math.round(S * 0.5), drop = Math.round(S * 0.28);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) {
      const along = d[0] ? (d[0] > 0 ? x : S - 1 - x) : (d[1] > 0 ? y : S - 1 - y), across = d[0] ? y : x;
      if (along < edge) continue;
      if (along === edge) out.set(x, y, fp[3]);
      else if (along < edge + drop) out.set(x, y, across % (3 * u) === 0 ? face[0] : (along - edge) <= u ? face[2] : face[1 + (r.chance(0.15) ? 1 : 0)]);
      else out.set(x, y, (x + y) % 2 === 0 && along < edge + drop + 2 * u ? C.plum : along < edge + drop + 2 * u ? C.night : C.black);
    }
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) { const along = d[0] ? (d[0] > 0 ? x : S - 1 - x) : (d[1] > 0 ? y : S - 1 - y); if (along === edge - 1) out.set(x, y, stepColor(out.get(x, y), -1)); }
    frames = [out];
  } else if (part === 'chasm') {
    // Deeper than a pit: a rock face just inside the rim, then dark that gets darker toward the middle, with a few far-off glints.
    const tt = 2 * u, depth = (x, y) => Math.min(mask & W ? 1e9 : x - tt, mask & N ? 1e9 : y - tt, mask & E ? 1e9 : S - tt - 1 - x, mask & S_ ? 1e9 : S - tt - 1 - y);
    frames = [hollow(look, S, `${s}:f`, mask, (x, y, r) => { const dd = depth(x, y); return dd < 3 * u ? (dd < u ? wp[1] : wp[0]) : dd < 5 * u ? (((x + y) & 1) ? C.night : C.plum) : r.chance(0.03) ? C.indigo : dd < 7 * u ? C.night : C.black; })];
  } else if (part === 'bridge') {
    const v = axisOf(facing) === 'v', wood = ramp('wood'), out = new Pixels(S, S);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) out.set(x, y, C.night);
    const pad = 3 * u;
    for (let i = 0; i < S; i += 3 * u) {
      const plank = (a, b, c0, c1) => { for (let y = a; y < b; y++) for (let x = c0; x < c1; x++) out.set(x, y, (v ? y - a : x - c0) === 0 ? wood[3] : (v ? y - a : x - c0) >= 3 * u - 1 ? wood[0] : wood[2]); };
      if (v) plank(i, Math.min(S, i + 3 * u), pad, S - pad); else plank(pad, S - pad, i, Math.min(S, i + 3 * u));
    }
    if (v) { out.rect(pad - u, 0, u, S, C.night).rect(S - pad, 0, u, S, C.night); out.rect(pad - 2 * u, 0, u, S, wood[1]).rect(S - pad + u, 0, u, S, wood[1]); }
    else { out.rect(0, pad - u, S, u, C.night).rect(0, S - pad, S, u, C.night); out.rect(0, pad - 2 * u, S, u, wood[1]).rect(0, S - pad + u, S, u, wood[1]); }
    frames = [out];
  } else if (part === 'rubble') {
    const f = shadowed(floor(), mask, S), r = rng(`${s}:rubble`), wr = ramp(look.wall);
    for (let i = 0; i < Math.round(5 * u); i++) {
      const x = r.range(2, S - 5 * u), y = r.range(2, S - 5 * u), w = r.range(2, 3 * u), h = r.range(2, 2 * u + 1), tmp = new Pixels(S, S);
      tmp.poly([[x, y + h], [x + 1, y], [x + w, y + 1], [x + w + 1, y + h]], 1);
      for (let yy = 0; yy < S; yy++) for (let xx = 0; xx < S; xx++) if (tmp.get(xx, yy)) f.set(xx, yy, tmp.get(xx, yy - 1) ? wr[3] : tmp.get(xx + 1, yy) || tmp.get(xx, yy + 1) ? wr[1] : wr[2]);
      f.rect(x, y + h + 1, w + 1, 1, fp[0]);
    }
    frames = [f];
  } else if (part === 'cracks') {
    const f = shadowed(floor(), mask, S); crack(f, look, S, rng(`${s}:cracks`), 2 + (S > 16 ? 2 : 0)); frames = [f];
  } else {
    // shade: an overlay of dark dither, ordered so it tiles; `amount` is how much of the tile it covers.
    const bayer = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5], out = new Pixels(S, S), a = Math.max(0, Math.min(1, amount));
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (bayer[(y % 4) * 4 + (x % 4)] < a * 16) out.set(x, y, C.night);
    frames = [out];
  }
  const animated = frames.length > 1;
  return { id: `tile.dungeon.${part}`, frames, fps: animated ? 3 : 0, loop: animated, spec: { biome, part, size, mask, notch, facing, state, amount }, projection: 'top-down', anchors: { root: [S >> 1, S >> 1] } };
}
