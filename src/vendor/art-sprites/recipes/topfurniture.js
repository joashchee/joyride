// Furniture seen from above (T23, `furniture` with `view: 'top'`): the same pieces as the 3/4 view, drawn flat on the world's 32 pixel grid as tops, with no front face.
// They are entries of `TOP_PROPS`, so `prop`, `dress` and the footprint and occlusion rules treat them like any other top-down prop. A piece the 3/4 set draws as a wall
// fitting (`window`, `doorway`) or a light (`candle`, `torch`) is not here: the wall is a flat top already, and the light is a `prop`.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

const T = 32;
const mk = (w = 1, h = 1) => new Pixels(w * T, h * T);
const wood = { 1: ramp('wood'), 2: ramp('sand'), 3: ramp('gold'), 4: ramp('iron'), 5: [C.night, C.night, C.plum, C.plum] };
const done = (m, parts) => shade(m, { ...wood, ...parts });

export const TOP_FURNITURE = {
  bed: { foot: [1, 2], occ: 'none', make() { const m = mk(1, 2); m.rect(2, 2, 28, 60, 1).rect(4, 4, 24, 56, 2).rect(6, 6, 20, 10, 3).rect(4, 22, 24, 38, 4); return done(m, { 2: ramp('white'), 3: ramp('steel'), 4: ramp('red') }); } },
  table: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(3, 6, 58, 20, 1).rect(5, 8, 54, 16, 2); m.ellipse(20, 16, 4, 3, 3); m.rect(40, 12, 8, 6, 4); return done(m, { 3: ramp('white'), 4: ramp('steel') }); } },
  chair: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(8, 6, 16, 4, 1).rect(8, 10, 16, 16, 2).rect(10, 12, 12, 12, 3); return done(m, { 3: ramp('leather') }); } },
  shelf: { foot: [2, 1], occ: 'partial', make() { const m = mk(2, 1); m.rect(2, 4, 60, 24, 1).rect(4, 6, 56, 20, 5); for (let x = 6; x < 58; x += 7) m.rect(x, 8, 5, 16, (x / 7) % 2 ? 3 : 4); m.rect(2, 4, 60, 2, 2); return done(m, { 3: ramp('red'), 4: ramp('blue') }); } },
  hearth: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(2, 2, 60, 28, 1).rect(10, 6, 44, 20, 5).rect(14, 10, 36, 12, 2).rect(20, 14, 24, 6, 3); m.rect(2, 2, 60, 2, 4); return done(m, { 1: ramp('stone'), 2: ramp('fire'), 3: ramp('gold'), 4: ramp('stone') }); } },
  altar: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(4, 4, 56, 24, 1).rect(7, 7, 50, 18, 2).rect(14, 10, 36, 12, 3).ellipse(32, 16, 4, 4, 4); return done(m, { 1: ramp('stone'), 2: ramp('white'), 3: ramp('red'), 4: ramp('gold') }); } },
  workbench: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(2, 6, 60, 20, 1).rect(4, 8, 56, 16, 2); m.rect(8, 11, 20, 3, 4).rect(8, 11, 4, 8, 4).rect(40, 10, 14, 10, 3); return done(m, { 3: ramp('leather'), 4: ramp('steel') }); } },
  anvil: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(6, 10, 20, 12, 1).rect(4, 12, 4, 8, 1).rect(10, 12, 12, 8, 2).rect(23, 14, 6, 4, 1); return done(m, { 1: ramp('iron'), 2: ramp('steel') }); } },
  barrel: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.ellipse(16, 16, 12, 12, 1).ellipse(16, 16, 9, 9, 2).ellipse(16, 16, 2, 2, 3); m.rect(4, 15, 24, 2, 4); return done(m, { 1: ramp('wood'), 2: ramp('sand'), 3: ramp('wood'), 4: ramp('iron') }); } },
  rug: { foot: [2, 1], occ: 'none', flat: true, make() { const m = mk(2, 1); m.rect(2, 4, 60, 24, 1).rect(5, 7, 54, 18, 2).rect(10, 11, 44, 10, 3); return done(m, { 1: ramp('red'), 2: ramp('gold'), 3: ramp('red') }); } },
  tub: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(2, 5, 60, 22, 1).rect(5, 8, 54, 16, 2); for (const [x, y] of [[16, 14], [30, 18], [44, 13]]) m.ellipse(x, y, 3, 2, 3); return done(m, { 1: ramp('white'), 2: ramp('water'), 3: ramp('ice') }); } },
  counter: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(2, 8, 60, 16, 1).rect(2, 8, 60, 3, 2).rect(46, 12, 10, 8, 3).rect(8, 13, 3, 3, 4); return done(m, { 3: ramp('steel'), 4: ramp('gold') }); } },
  stove: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(4, 4, 24, 24, 1).ellipse(11, 11, 4, 4, 2).ellipse(21, 11, 4, 4, 2).ellipse(11, 21, 4, 4, 3).ellipse(21, 21, 4, 4, 2); return done(m, { 1: ramp('iron'), 2: ramp('steel'), 3: ramp('fire') }); } },
  fountain: { foot: [2, 2], occ: 'none', make() { const m = mk(2, 2); m.ellipse(32, 32, 28, 28, 1).ellipse(32, 32, 24, 24, 2).ellipse(32, 32, 7, 7, 1).ellipse(32, 32, 4, 4, 3); return done(m, { 1: ramp('stone'), 2: ramp('water'), 3: ramp('ice') }); } },
  piano: { foot: [2, 1], occ: 'none', make() { const m = mk(2, 1); m.rect(2, 2, 60, 28, 1).rect(6, 18, 52, 9, 2); for (let x = 8; x < 56; x += 4) m.rect(x, 18, 1, 9, 5); m.rect(34, 10, 18, 5, 3); return done(m, { 1: ramp('iron'), 2: ramp('white'), 3: ramp('gold') }); } },
  questboard: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(4, 12, 24, 8, 1).rect(6, 13, 20, 5, 2); for (const x of [8, 14, 20]) m.rect(x, 14, 4, 3, 3); return done(m, { 2: ramp('sand'), 3: ramp('white') }); } },
  cage: { foot: [1, 1], occ: 'partial', make() { const m = mk(); m.rect(4, 4, 24, 24, 1).rect(6, 6, 20, 20, 5); for (const x of [10, 15, 20]) m.rect(x, 6, 1, 20, 1); m.rect(6, 15, 20, 1, 1); return done(m, { 1: ramp('iron') }); } },
};
export const TOP_FURNITURE_IDS = Object.keys(TOP_FURNITURE);
