// Building exteriors (32x32, front view): the ones `prop` does not draw (it has house, clanhall, shrine, tent). A mask of part ids
// (1 walls, 2 roof, 3 stone and trim, 4 door and wood, 5 glass and light), shaded by shade.js; the seed picks the roof colour.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';
import { rng } from '../rng.js';

const S = 32;
const ROOFS = ['red', 'blue', 'green', 'purple'];
const window = (m, x, y) => { m.rect(x, y, 3, 4, 5); m.rect(x + 1, y, 1, 4, 3); };

export const BUILDINGS = {
  barn: { walls: 'red', draw(m) { m.rect(4, 14, 24, 16, 1); m.poly([[2, 15], [9, 5], [23, 5], [30, 15]], 2); m.rect(11, 18, 10, 12, 4); m.line(11, 18, 20, 29, 3); m.line(20, 18, 11, 29, 3); m.rect(14, 9, 4, 3, 5); } },
  windmill: { walls: 'bone', draw(m) { m.poly([[11, 28], [13, 12], [19, 12], [21, 28]], 1); m.poly([[12, 12], [16, 6], [20, 12]], 2); m.rect(14, 22, 4, 6, 4); m.rect(15, 15, 2, 3, 5); m.thick(16, 10, 6, 2, 4, 2); m.thick(16, 10, 26, 18, 4, 2); m.thick(16, 10, 26, 2, 4, 2); m.thick(16, 10, 6, 18, 4, 2); m.ellipse(16, 10, 1.5, 1.5, 3); } },
  lighthouse: { walls: 'white', draw(m) { m.poly([[11, 29], [13, 11], [19, 11], [21, 29]], 1); m.rect(11, 20, 10, 3, 2); m.rect(13, 14, 6, 2, 2); m.rect(12, 8, 8, 3, 3); m.rect(13, 5, 6, 3, 5); m.poly([[12, 5], [16, 1], [20, 5]], 2); m.rect(14, 24, 4, 5, 4); m.rect(2, 29, 28, 2, 3); } },
  tower: { walls: 'stone', draw(m) { m.rect(9, 8, 14, 22, 1); for (const x of [9, 13, 17, 20]) m.rect(x, 4, 3, 5, 1); m.rect(8, 9, 16, 2, 3); m.rect(14, 22, 4, 8, 4); m.rect(15, 13, 2, 5, 5); } },
  chapel: { walls: 'bone', draw(m) { m.rect(5, 15, 22, 15, 1); m.poly([[3, 16], [16, 8], [29, 16]], 2); m.rect(13, 4, 6, 11, 1); m.poly([[12, 5], [16, 1], [20, 5]], 2); m.rect(15, 6, 2, 3, 5); m.rect(13, 22, 6, 8, 4); window(m, 7, 19); window(m, 22, 19); } },
  inn: { walls: 'wood', draw(m) { m.rect(3, 12, 26, 18, 1); m.poly([[1, 13], [8, 4], [24, 4], [31, 13]], 2); m.rect(3, 20, 26, 1, 3); window(m, 6, 14); window(m, 14, 14); window(m, 22, 14); window(m, 6, 23); window(m, 22, 23); m.rect(13, 22, 6, 8, 4); m.rect(14, 18, 4, 2, 3); m.rect(26, 5, 3, 6, 3); } },
  watchtower: { walls: 'wood', draw(m) { m.rect(8, 16, 2, 14, 4).rect(22, 16, 2, 14, 4); m.line(9, 29, 23, 17, 4); m.line(23, 29, 9, 17, 4); m.rect(6, 10, 20, 7, 1); m.poly([[4, 10], [16, 2], [28, 10]], 2); m.rect(11, 12, 10, 3, 5); m.rect(6, 17, 20, 1, 3); } },
};
export const BUILDING_IDS = Object.keys(BUILDINGS);

/** The roof plan of each building (T23): what a building is from straight above. Part 1 is the lit slope of the roof, 2 the slope in shade, 3 stone and trim, 4 wood, 5 glass. */
export const ROOF_PLANS = {
  barn: { walls: 'red', draw(m) { m.rect(3, 6, 26, 10, 1).rect(3, 16, 26, 10, 2).rect(3, 15, 26, 2, 3); m.rect(8, 9, 3, 3, 3).rect(21, 20, 3, 3, 3); } },
  windmill: { walls: 'bone', draw(m) { m.ellipse(16, 16, 8, 8, 2).poly([[16, 8], [24, 16], [16, 24]], 1); m.thick(16, 16, 3, 3, 4, 2).thick(16, 16, 29, 3, 4, 2).thick(16, 16, 3, 29, 4, 2).thick(16, 16, 29, 29, 4, 2); m.ellipse(16, 16, 2, 2, 3); } },
  lighthouse: { walls: 'white', draw(m) { m.ellipse(16, 16, 11, 11, 3).ellipse(16, 16, 8, 8, 1).poly([[16, 8], [24, 16], [16, 24]], 2).ellipse(16, 16, 3, 3, 5); for (let a = 0; a < 8; a++) m.rect(16 + Math.cos(a * Math.PI / 4) * 10, 16 + Math.sin(a * Math.PI / 4) * 10, 2, 2, 4); } },
  tower: { walls: 'stone', draw(m) { m.rect(6, 6, 20, 20, 3).rect(9, 9, 14, 14, 1).rect(9, 16, 14, 7, 2); for (const [x, y] of [[4, 4], [24, 4], [4, 24], [24, 24], [14, 4], [14, 25], [4, 14], [25, 14]]) m.rect(x, y, 4, 4, 3); m.ellipse(16, 16, 2, 2, 5); } },
  chapel: { walls: 'bone', draw(m) { m.rect(10, 8, 6, 20, 1).rect(16, 8, 6, 20, 2).rect(3, 13, 7, 8, 1).rect(22, 13, 7, 8, 2).rect(10, 13, 12, 8, 1).rect(16, 13, 6, 8, 2); m.rect(13, 1, 6, 7, 3).rect(14, 2, 4, 5, 5).rect(15, 8, 2, 20, 3); } },
  inn: { walls: 'wood', draw(m) { m.rect(3, 7, 26, 6, 1).rect(3, 13, 26, 6, 2).rect(3, 19, 12, 5, 1).rect(3, 24, 12, 4, 2).rect(3, 12, 26, 2, 3).rect(21, 5, 4, 4, 3).rect(9, 19, 4, 1, 3); } },
  watchtower: { walls: 'wood', draw(m) { m.rect(5, 5, 22, 22, 4).poly([[16, 16], [7, 7], [25, 7]], 1).poly([[16, 16], [25, 7], [25, 25]], 2).poly([[16, 16], [25, 25], [7, 25]], 2).poly([[16, 16], [7, 25], [7, 7]], 1); m.rect(15, 15, 2, 2, 3); } },
};
export const BUILDING_VIEWS = ['front', 'top'];

/** Draw one building. `roof` overrides the roof ramp; otherwise the seed picks one (the tower and lighthouse take their own). */
export function drawBuilding({ kind = 'barn', seed = 'building', roof, view = 'front' } = {}) {
  if (!BUILDING_VIEWS.includes(view)) throw new Error(`unknown building view "${view}"; known: ${BUILDING_VIEWS.join(', ')}`);
  const def = (view === 'top' ? ROOF_PLANS : BUILDINGS)[kind]; if (!def) throw new Error(`unknown building "${kind}"`);
  const r = rng(`${seed}:${kind}`), m = new Pixels(S, S);
  def.draw(m);
  const roofRamp = roof ?? (['tower', 'lighthouse', 'windmill'].includes(kind) ? 'red' : r.pick(ROOFS));
  if (view === 'top') {
    const rr = ramp(roofRamp), out = shade(m, { 1: rr, 2: [rr[0], rr[0], rr[1], rr[1]], 3: ramp('stone'), 4: ramp('wood'), 5: ramp('glass') });
    return { id: `building.${kind}`, frames: [out], fps: 0, loop: false, projection: 'top-down', spec: { kind, view, roof: roofRamp }, anchors: { root: [16, 16] } };
  }
  const out = shade(m, { 1: ramp(def.walls), 2: ramp(roofRamp), 3: ramp('stone'), 4: ramp('wood'), 5: ramp('glass') });
  return { id: `building.${kind}`, frames: [out], fps: 0, loop: false };
}
