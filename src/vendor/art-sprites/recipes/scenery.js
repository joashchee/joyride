// Scenery for the biome scenes: what the environments kit lists beyond the first house, tree and rock. Castle walls, a gatehouse, a
// market stall and a well for villages and castles; a dock and two bridges for coasts, rivers and swamps; a ruined arch, a broken
// wall and a dune for deserts and ruins; a peak and a mine mouth for mountains; a floating island and a portal for magical places.
// Same shape as the props in tiles.js: a mask of part ids, shaded with ramps, 16 by 16 unless it is in BIG.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

const mk = (n = 16) => new Pixels(n, n);

export const SCENERY = {
  // Villages and castles.
  battlement: () => { const m = mk(); m.rect(0, 5, 16, 10, 1); for (const x of [0, 4, 8, 12]) m.rect(x, 2, 3, 3, 1); m.rect(2, 9, 1, 3, 3).rect(10, 9, 1, 3, 3).rect(6, 7, 4, 1, 2); return [m, { 1: ramp('stone'), 2: ramp('iron'), 3: [C.night, C.night, C.night, C.night] }]; },
  well: () => { const m = mk(); m.ellipse(8, 11, 6, 3.5, 1).rect(2, 8, 12, 5, 1).ellipse(8, 8, 4.5, 2, 3).rect(3, 3, 1, 6, 2).rect(12, 3, 1, 6, 2).poly([[2, 3], [8, 0], [14, 3]], 4).line(8, 3, 8, 8, 2); return [m, { 1: ramp('stone'), 2: ramp('wood'), 3: ramp('water'), 4: ramp('red') }]; },
  stall: () => { const m = mk(); m.rect(2, 8, 12, 6, 1).rect(1, 3, 14, 4, 2); for (const x of [2, 6, 10]) m.rect(x, 3, 2, 4, 3); m.rect(2, 7, 1, 7, 1).rect(13, 7, 1, 7, 1).rect(4, 9, 3, 2, 4).rect(8, 9, 3, 2, 5); return [m, { 1: ramp('wood'), 2: ramp('red'), 3: ramp('bone'), 4: ramp('grass'), 5: ramp('gold') }]; },
  signpost: () => { const m = mk(); m.rect(7, 2, 2, 13, 1).poly([[3, 3], [12, 3], [14, 5], [12, 7], [3, 7]], 2).poly([[13, 8], [5, 8], [3, 10], [5, 12], [13, 12]], 2).rect(5, 5, 5, 1, 1); return [m, { 1: ramp('wood'), 2: ramp('sand') }]; },
  // Coasts, rivers, swamps: a dock and two bridges, drawn across their tile so a row of them makes a span.
  dock: () => { const m = mk(); m.rect(0, 6, 16, 5, 1); for (const x of [0, 5, 10]) m.rect(x, 6, 1, 5, 2); m.rect(1, 11, 2, 4, 2).rect(13, 11, 2, 4, 2).rect(7, 11, 2, 4, 2); m.rect(0, 3, 2, 3, 2).rect(14, 3, 2, 3, 2); return [m, { 1: ramp('wood'), 2: ramp('leather') }]; },
  bridge: () => { const m = mk(); m.rect(0, 7, 16, 4, 1); for (const x of [3, 7, 11]) m.rect(x, 7, 1, 4, 3); m.rect(0, 3, 16, 1, 2).rect(0, 4, 1, 6, 2).rect(15, 4, 1, 6, 2).rect(7, 4, 2, 3, 2).rect(0, 11, 16, 1, 3); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('iron') }]; },
  ropebridge: () => { const m = mk(); m.rect(0, 3, 1, 9, 2).rect(15, 3, 1, 9, 2); for (let x = 1; x < 15; x++) m.set(x, 9 + (x % 4 === 0 ? 0 : 1) + Math.round(1.6 * Math.sin((x / 15) * Math.PI)), 1).set(x, 4 + Math.round(1.6 * Math.sin((x / 15) * Math.PI)), 3); for (let x = 2; x < 15; x += 3) m.rect(x, 5 + Math.round(1.6 * Math.sin((x / 15) * Math.PI)), 1, 5, 3); return [m, { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('sand') }]; },
  // Deserts and ruins.
  ruinarch: () => { const m = mk(); m.rect(1, 6, 4, 9, 1).rect(11, 6, 4, 9, 1).rect(1, 4, 5, 2, 1).rect(10, 4, 5, 2, 1).rect(5, 3, 3, 2, 1).rect(8, 2, 2, 1, 1).rect(2, 8, 1, 2, 2).rect(12, 10, 1, 3, 2); return [m, { 1: ramp('sand'), 2: ramp('wood') }]; },
  ruinwall: () => { const m = mk(); m.poly([[0, 14], [0, 8], [3, 6], [6, 9], [9, 5], [12, 8], [16, 7], [16, 14]], 1).rect(3, 10, 3, 1, 2).rect(9, 9, 4, 1, 2).rect(1, 12, 5, 1, 2).rect(10, 12, 4, 1, 2); return [m, { 1: ramp('sand'), 2: ramp('wood') }]; },
  dune: () => { const m = mk(); m.poly([[0, 14], [3, 9], [7, 5], [11, 8], [16, 13], [16, 14]], 1).poly([[7, 5], [11, 8], [16, 13], [10, 13], [8, 9]], 2); return [m, { 1: ramp('sand'), 2: ramp('sand') }]; },
  // Mountains and mines.
  mineentrance: () => { const m = mk(); m.poly([[0, 15], [1, 6], [5, 2], [11, 2], [15, 6], [16, 15]], 1).rect(4, 6, 8, 9, 3).rect(3, 5, 10, 1, 2).rect(3, 6, 1, 9, 2).rect(12, 6, 1, 9, 2).rect(5, 13, 6, 1, 4).line(5, 14, 11, 14, 4); return [m, { 1: ramp('stone'), 2: ramp('wood'), 3: [C.night, C.night, C.night, C.night], 4: ramp('iron') }]; },
  // Magical.
  portal: () => { const m = mk(); m.ellipse(8, 8, 7, 7.5, 1).ellipse(8, 8, 5, 5.5, 2).ellipse(8, 8, 2.5, 3, 3); m.rect(2, 14, 12, 2, 4); return [m, { 1: ramp('stone'), 2: ramp('purple'), 3: ramp('glass'), 4: ramp('stone') }]; },
};

export const BIG_SCENERY = {
  castle: () => { const m = mk(32); m.rect(3, 13, 26, 17, 1).rect(1, 6, 7, 24, 1).rect(24, 6, 7, 24, 1).rect(11, 2, 10, 28, 1); for (const x of [1, 4, 24, 27]) m.rect(x, 3, 3, 3, 1); for (const x of [11, 14, 17, 19]) m.rect(x, 0, 2, 2, 1); m.poly([[10, 3], [16, -1], [22, 3]], 2); m.rect(13, 22, 6, 8, 3).rect(3, 9, 2, 4, 3).rect(27, 9, 2, 4, 3).rect(15, 8, 2, 5, 3).rect(6, 20, 2, 3, 3).rect(24, 20, 2, 3, 3).rect(16, -3, 1, 1, 4); return [m, { 1: ramp('stone'), 2: ramp('red'), 3: [C.night, C.night, C.night, C.night], 4: ramp('red') }]; },
  gatehouse: () => { const m = mk(32); m.rect(0, 12, 6, 18, 1).rect(26, 12, 6, 18, 1).rect(5, 8, 22, 22, 1); for (const x of [0, 3, 26, 29]) m.rect(x, 9, 2, 3, 1); for (const x of [6, 10, 14, 18, 22]) m.rect(x, 5, 3, 3, 1); m.rect(11, 14, 10, 16, 3).ellipse(16, 14, 5, 3, 3); for (const x of [12, 14, 16, 18, 20]) m.rect(x, 14, 1, 16, 2); m.rect(11, 20, 10, 1, 2).rect(2, 17, 2, 3, 3).rect(28, 17, 2, 3, 3); return [m, { 1: ramp('stone'), 2: ramp('iron'), 3: [C.night, C.night, C.night, C.night] }]; },
  peak: () => { const m = mk(32); m.poly([[0, 30], [8, 14], [12, 17], [19, 3], [26, 15], [32, 30]], 1).poly([[19, 3], [15, 10], [17, 9], [19, 12], [21, 9], [23, 10]], 2).poly([[8, 14], [6, 18], [8, 17], [10, 18], [12, 17]], 2); return [m, { 1: ramp('stone'), 2: ramp('white') }]; },
  floatisle: () => { const m = mk(32); m.poly([[3, 14], [29, 14], [26, 20], [21, 25], [17, 30], [14, 25], [8, 21]], 1).rect(3, 12, 26, 3, 2).rect(14, 7, 5, 5, 3).poly([[13, 7], [16, 3], [20, 7]], 4).rect(7, 9, 2, 3, 5).rect(23, 8, 3, 4, 5); m.set(10, 22, 1).set(22, 26, 1).set(5, 25, 1).set(26, 9, 5); return [m, { 1: ramp('stone'), 2: ramp('grass'), 3: ramp('bone'), 4: ramp('red'), 5: ramp('green') }]; },
};

export const SCENERY_IDS = [...Object.keys(SCENERY), ...Object.keys(BIG_SCENERY)];

/** One piece of scenery, or null if `kind` is not one. */
export function drawScenery(kind) {
  const make = SCENERY[kind] ?? BIG_SCENERY[kind]; if (!make) return null;
  const [m, parts] = make(); return shade(m, parts);
}
