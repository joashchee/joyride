// Creatures seen from above (`view: 'top'`, T23), drawn on the same 40 by 40 canvas as the top-down character with the body centred on it. Each body plan is
// drawn facing north in a mask (part 1 skin, 2 underside or membrane, 3 bone, 4 eyes, 5 dark, 6 accent, 7 metal), turned a quarter at a time for the other
// facings, and only then shaded, so the light always comes from the top left. A canvas of 48 or 64 draws the plan again at that scale (a boss), not enlarged.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

export const TOP_FACINGS = ['north', 'east', 'south', 'west'];
export const TOP_CANVASES = [40, 48, 64, 96, 128, 192, 256];

/** The plan each body is drawn as from above: bodies without a plan of their own share the nearest one. */
export const TOP_PLAN = {
  blob: 'blob', floater: 'floater', biped: 'biped', multiarm: 'multiarm', skeleton: 'skeleton', quad: 'quad', horse: 'horse', centaur: 'centaur', drake: 'drake', spider: 'spider', insect: 'insect',
  snake: 'snake', naga: 'naga', fish: 'fish', merfolk: 'merfolk', golem: 'golem', construct: 'construct', winged: 'winged', griffon: 'griffon', bird: 'bird', fey: 'fey',
  ghost: 'ghost', elemental: 'elemental', treant: 'treant', mimic: 'mimic', fungus: 'fungus', swarm: 'swarm', aberration: 'aberration', flytrap: 'flytrap', vine: 'vine', vermin: 'swarm', flock: 'swarm', wisps: 'swarm',
};

/** The drawing kit for one canvas: coordinates are in 40-canvas pixels from the centre, so one plan draws at any scale. */
function kit(m, k) {
  const c = m.w / 2, s = (v) => c + v * k, n = (v) => Math.max(1, Math.round(v * k));
  return {
    e: (x, y, rx, ry, id) => m.ellipse(s(x), s(y), rx * k, ry * k, id),
    r: (x, y, w, h, id) => m.rect(Math.round(s(x)), Math.round(s(y)), n(w), n(h), id),
    l: (x0, y0, x1, y1, id, t = 2) => m.thick(Math.round(s(x0)), Math.round(s(y0)), Math.round(s(x1)), Math.round(s(y1)), id, n(t)),
    p: (pts, id) => m.poly(pts.map(([x, y]) => [s(x), s(y)]), id),
    pt: (x, y, id) => m.rect(Math.round(s(x)), Math.round(s(y)), n(1), n(1), id),
  };
}
const eyes = (g, list) => list.forEach(([x, y]) => g.pt(x, y, 4));
const horns = (g, count, y) => { if (count >= 1) g.l(-4, y, -7, y - 5, 3, 1); if (count >= 2) g.l(4, y, 7, y - 5, 3, 1); };
const wobble = (t) => (t ? 1 : -1);

/** Each plan draws its mask (north at the top) for step `t` (0 or 1: the legs, wings or body in the other half of the stride). */
const PLANS = {
  blob(g, s, t) { g.e(0, 2, 11 + t, 10 - t, 1); g.e(0, 4, 6, 5, 2); eyes(g, [[-4, -3], [3, -3]]); },
  biped(g, s, t) {
    g.r(-5, 7 + wobble(t) * 1, 4, 5, 1); g.r(1, 7 - wobble(t) * 1, 4, 5, 1);
    g.e(0, 2, 8, 5.5, 1); g.e(0, 3, 4, 3, 2); g.e(0, -5, 4.5, 4.5, 1);
    g.l(-8, 0, -10, -5 + wobble(t), 1, 3); g.l(8, 0, 10, -5 - wobble(t), 1, 3);
    if (s.gear?.includes('hood')) { g.e(0, -5, 5.5, 5.5, 6); g.p([[-9, 0], [9, 0], [8, 6], [-8, 6]], 6); }
    if (s.gear?.includes('bandana')) { g.r(-5, -8, 10, 3, 6); g.r(-3, -3, 6, 1, 6); }
    if (s.gear?.includes('blade')) g.l(11, -6, 11, -17, 7, 1);
    if (s.gear?.includes('club')) g.l(11, -5, 12, -14, 3, 3);
    if (s.gear?.includes('buckler')) g.e(-11, -2, 3.5, 3.5, 7);
    if (s.ears) { g.p([[-4, -5], [-8, -7], [-4, -2]], 1); g.p([[4, -5], [8, -7], [4, -2]], 1); }
    horns(g, s.horns, -8);
    if (s.jaw === 'tusks') { g.pt(-2, -10, 3); g.pt(1, -10, 3); }
    if (s.body === 'multiarm') { g.l(-8, 3, -12, 0, 1, 3); g.l(8, 3, 12, 0, 1, 3); }
    eyes(g, [[-2, -9], [1, -9]]);
  },
  skeleton(g, s, t) {
    g.r(-4, 8 + wobble(t), 3, 5, 3); g.r(1, 8 - wobble(t), 3, 5, 3);
    g.e(0, 3, 6, 5, 3); g.e(0, 3, 3, 3, 5); g.l(-6, 0, -9, -5, 3, 1); g.l(6, 0, 9, -5, 3, 1);
    g.e(0, -5, 4, 4, 3); g.pt(-2, -7, 5); g.pt(1, -7, 5); g.r(-1, 2, 2, 8, 3);
    if (s.crown) g.r(-3, -10, 6, 2, 6);
    eyes(g, [[-2, -8], [1, -8]]);
  },
  quad(g, s, t) {
    g.l(0, 12, 0 + wobble(t) * 2, 19, 1, 2);
    g.e(0, 3, 6.5, 9, 1); g.e(0, 4, 3, 5, 2);
    g.r(-9, -3 + wobble(t), 3, 3, 3); g.r(6, -3 - wobble(t), 3, 3, 3); g.r(-9, 8 - wobble(t), 3, 3, 3); g.r(6, 8 + wobble(t), 3, 3, 3);
    g.e(0, -9, 4.5, 4.5, 1); g.r(-2, -15, 4, 4, 1);
    if (s.ears) { g.p([[-4, -9], [-6, -14], [-1, -11]], 1); g.p([[4, -9], [6, -14], [1, -11]], 1); }
    if (s.jaw === 'fangs') { g.pt(-2, -15, 3); g.pt(1, -15, 3); }
    if (s.saddle) g.r(-4, 1, 8, 5, 6);
    eyes(g, [[-3, -10], [2, -10]]);
  },
  drake(g, s, t) {
    const flap = t ? 0.8 : 1;
    g.p([[-2, -4], [-18 * flap, -10], [-17 * flap, 4], [-9, 8], [-2, 6]], 2); g.p([[2, -4], [18 * flap, -10], [17 * flap, 4], [9, 8], [2, 6]], 2);
    PLANS.quad(g, { ...s, ears: false, saddle: false }, t); horns(g, s.horns, -12);
  },
  spider(g, s, t) {
    for (let i = 0; i < 4; i++) {
      const y = -4 + i * 3, out = 11 + (i % 2) * 2 + wobble(t ^ (i & 1)), dy = (i - 1.5) * 3;
      g.l(-3, y, -9, y + dy - 3, 3, 1); g.l(-9, y + dy - 3, -out, y + dy + 3, 3, 1); g.l(3, y, 9, y + dy - 3, 3, 1); g.l(9, y + dy - 3, out, y + dy + 3, 3, 1);
    }
    g.e(0, 8, 6, 7.5, 1); g.e(0, 9, 3, 4, 2); g.e(0, -3, 4.5, 4.5, 1);
    eyes(g, [[-3, -6], [-1, -7], [1, -7], [2, -6]]);
  },
  insect(g, s, t) {
    for (let i = 0; i < 3; i++) { const y = -1 + i * 4; g.l(-3, y, -10, y + 3 + wobble(t ^ (i & 1)), 3, 1); g.l(3, y, 10, y + 3 - wobble(t ^ (i & 1)), 3, 1); }
    g.e(0, 9, 5, 7, 1); g.e(0, 10, 3, 4, 2); g.e(0, 1, 3.5, 3.5, 1); g.e(0, -6, 3.5, 3.5, 1); g.l(-2, -9, -5, -15, 3, 1); g.l(2, -9, 5, -15, 3, 1);
    eyes(g, [[-3, -7], [2, -7]]);
  },
  snake(g, s, t) {
    for (let i = 0; i <= 14; i++) { const y = 17 - i * 2.3, x = Math.sin(i * 0.9 + (t ? 0.8 : 0)) * 5, w = 1.5 + Math.min(i, 14 - i * 0.2) * 0.22 + (i === 14 ? 1.5 : 0); g.e(x, y, w, w, 1); g.e(x, y + 0.6, w * 0.5, w * 0.5, 2); }
    g.e(Math.sin(14 * 0.9 + (t ? 0.8 : 0)) * 5, -15, 4, 3.5, 1);
    const hx = Math.sin(14 * 0.9 + (t ? 0.8 : 0)) * 5;
    if (s.jaw === 'fangs') { g.pt(hx - 2, -18, 3); g.pt(hx + 1, -18, 3); }
    eyes(g, [[hx - 3, -16], [hx + 2, -16]]);
  },
  golem(g, s, t) {
    g.r(-6, 9 + wobble(t), 5, 5, 1); g.r(1, 9 - wobble(t), 5, 5, 1);
    g.r(-9, -6, 18, 15, 1); g.r(-4, -2, 8, 7, 2); g.r(-5, -13, 10, 7, 1);
    g.r(-15, -4 + wobble(t), 6, 12, 1); g.r(9, -4 - wobble(t), 6, 12, 1); g.r(-15, 6 + wobble(t), 6, 5, 3); g.r(9, 6 - wobble(t), 6, 5, 3);
    eyes(g, [[-3, -10], [2, -10]]);
  },
  construct(g, s, t) {
    g.r(-5, 9 + wobble(t), 4, 5, 7); g.r(1, 9 - wobble(t), 4, 5, 7);
    g.r(-8, -5, 16, 14, 1); g.e(0, 2, 4, 4, 3); g.e(0, 2, 1.5, 1.5, 5); g.r(-5, -12, 10, 7, 1); g.r(-4, -10, 8, 2, 6);
    g.r(-14, -3 + wobble(t), 5, 4, 7); g.r(-14, 1 + wobble(t), 6, 7, 3); g.r(9, -3 - wobble(t), 5, 4, 7); g.r(8, 1 - wobble(t), 6, 7, 3);
    eyes(g, [[-3, -9], [2, -9]]);
  },
  winged(g, s, t) {
    const flap = t ? 0.7 : 1;
    g.p([[-2, -4], [-18 * flap, -12], [-17 * flap, 0], [-11, 5], [-2, 4]], 2); g.p([[2, -4], [18 * flap, -12], [17 * flap, 0], [11, 5], [2, 4]], 2);
    g.e(0, 1, 4, 8, 1); g.e(0, -8, 3.5, 3.5, 1); g.l(0, 8, 0, 14, 1, 2);
    if (s.ears) { g.p([[-3, -8], [-5, -13], [-1, -10]], 1); g.p([[3, -8], [5, -13], [1, -10]], 1); }
    horns(g, s.horns, -10);
    eyes(g, [[-2, -10], [1, -10]]);
  },
  ghost(g, s, t) {
    g.e(0, -2, 8 + t, 8, 1); g.r(-8 - t, -2, 16 + 2 * t, 11, 1); for (let x = -8; x < 8; x += 4) g.r(x, 9, 4, 3 + ((x / 4 + t) & 1) * 3, 1);
    g.e(-9, 2, 3, 5, 1); g.e(9, 2, 3, 5, 1); g.e(0, 1, 4, 5, 2);
    eyes(g, [[-3, -5], [2, -5]]);
  },
  treant(g, s, t) {
    g.e(0, 0, 11 + t, 11, 2); g.e(0, 0, 6, 6, 1); g.l(-6, -2, -15, -7, 1, 3); g.l(6, -2, 15, -7, 1, 3); g.l(-5, 5, -13, 12, 1, 3); g.l(5, 5, 13, 12, 1, 3);
    eyes(g, [[-2, -3], [1, -3]]);
  },
  mimic(g, s, t) {
    g.r(-11, -9, 22, 20, 1); g.r(-11, -13 - t, 22, 8, 2); g.r(-11, -9, 22, 2, 7); g.r(-11, 9, 22, 2, 7);
    g.r(-9, -5, 18, 6, 5); for (let x = -9; x < 8; x += 3) { g.p([[x, -5], [x + 3, -5], [x + 1.5, -2 + t]], 3); g.p([[x, 1], [x + 3, 1], [x + 1.5, -2 + t]], 3); }
    g.e(0, 0, 3, 1.5, 6);
    eyes(g, [[-5, -11], [4, -11]]);
  },
  fungus(g, s, t) {
    g.e(0, 0, 11 + t, 11 + t, 2); g.e(0, 0, 5, 5, 1); for (const [x, y] of [[-6, -4], [5, -6], [3, 5], [-4, 6], [8, 1]]) g.e(x, y, 1.6, 1.6, 3);
    eyes(g, [[-2, -1], [1, -1]]);
  },
  swarm(g, s, t) {
    const pts = [[-12, -8], [-6, -13], [4, -14], [11, -9], [-14, 0], [-3, -4], [6, -1], [13, 3], [-9, 7], [0, 9], [8, 11], [-4, 14], [14, -2], [-13, -15], [1, 2]];
    // The kinds of swarm share one scatter: rats with a tail behind, bats with their wings out, wisps with a halo.
    pts.forEach(([x, y], i) => {
      const o = i & 1 ? t : -t, px = x + o, py = y - o;
      if (s.body === 'vermin') { g.e(px, py, 2.4, 1.6, 1); g.pt(px - 3, py, 1); g.l(px + 2, py, px + 5, py + 2 * (i & 1 ? 1 : -1), 3, 1); }
      else if (s.body === 'flock') { g.p([[px - 5, py - 1], [px, py + 1], [px + 5, py - 1], [px + 3, py + 3], [px, py + 2], [px - 3, py + 3]], 1); g.pt(px, py, 5); }
      else if (s.body === 'wisps') { g.e(px, py, 2.4, 2.4, 1); g.e(px, py, 1.2, 1.2, 4); }
      else { g.e(px, py, 2, 2, 1); g.pt(px - 2, py, 3); g.pt(px + 2, py, 3); }
    });
  },
  flytrap(g, s, t) {
    for (let i = 0; i < 3; i++) { const a = (i / 3) * Math.PI * 2 + 0.5; g.l(0, 0, Math.cos(a) * 14, Math.sin(a) * 14, 1, 3); g.e(Math.cos(a) * 14, Math.sin(a) * 14, 4, 3, 1); }
    g.e(0, 0, 9 + t, 7, 2); g.e(0, 0, 7 + t, 4, 1); for (let x = -6; x <= 5; x += 3) { g.p([[x, -3], [x + 2, -3], [x + 1, 0]], 3); g.p([[x + 1, 3], [x + 3, 3], [x + 2, 0]], 3); }
    eyes(g, [[-4, -5], [3, -5]]);
  },
  vine(g, s, t) {
    for (let i = 0; i < 6; i++) { const a = (i / 6) * Math.PI * 2 + (t ? 0.3 : 0); g.l(0, 0, Math.cos(a) * 9, Math.sin(a) * 9, 1, 3); g.l(Math.cos(a) * 9, Math.sin(a) * 9, Math.cos(a + 0.5) * 16, Math.sin(a + 0.5) * 16, 1, 2); g.e(Math.cos(a + 0.5) * 16, Math.sin(a + 0.5) * 16, 2, 2, 2); g.pt(Math.cos(a) * 12, Math.sin(a) * 12, 3); }
    g.e(0, 0, 7, 7, 1); g.e(0, 0, 4, 4, 2);
    eyes(g, [[-2, -1], [1, -1]]);
  },
  fish(g, s, t) {
    const sw = wobble(t) * 2;
    g.p([[0, 9], [-5 + sw, 19], [0, 16], [5 + sw, 19]], 2); g.p([[-4, -3], [-11, 1 + t], [-4, 4]], 2); g.p([[4, -3], [11, 1 + t], [4, 4]], 2);
    g.e(0, 0, 5.5, 12, 1); g.e(0, 1, 2.5, 9, 2); g.l(0, -8, 0, 10, 3, 1); g.pt(-1, -13, 5); g.pt(0, -13, 5);
    eyes(g, [[-4, -8], [3, -8]]);
  },
  horse(g, s, t) {
    g.l(0, 11, wobble(t) * 3, 20, 3, 2); g.l(0, 11, wobble(t) * 2 - 1, 18, 3, 1);
    g.e(0, 4, 5.5, 10, 1); g.e(0, 5, 3, 6, 2);
    g.r(-8, -2 + wobble(t), 2.5, 4, 3); g.r(6, -2 - wobble(t), 2.5, 4, 3); g.r(-8, 9 - wobble(t), 2.5, 4, 3); g.r(6, 9 + wobble(t), 2.5, 4, 3);
    g.l(0, -4, 0, -11, 1, 4); g.l(0, -4, 0, -11, 5, 1); g.e(0, -14, 2.8, 4, 1); g.r(-1, -19, 3, 3, 3);
    if (s.ears) { g.pt(-3, -17, 1); g.pt(2, -17, 1); }
    if (s.saddle) g.r(-4, 2, 8, 5, 6);
    eyes(g, [[-3, -15], [2, -15]]);
  },
  centaur(g, s, t) {
    g.l(0, 10, wobble(t) * 3, 19, 3, 2);
    g.e(0, 5, 6, 9, 1); g.e(0, 6, 3, 5, 2);
    g.r(-9, -1 + wobble(t), 3, 4, 3); g.r(6, -1 - wobble(t), 3, 4, 3); g.r(-9, 9 - wobble(t), 3, 4, 3); g.r(6, 9 + wobble(t), 3, 4, 3);
    g.e(0, -5, 5.5, 4, 1); g.l(-6, -5, -10, -10 + wobble(t), 1, 2); g.l(6, -5, 10, -10 - wobble(t), 1, 2);
    g.e(0, -11, 3.8, 3.8, 1); horns(g, s.horns, -13);
    if (s.gear?.includes('blade')) g.l(11, -10, 11, -20, 7, 1);
    eyes(g, [[-2, -13], [1, -13]]);
  },
  griffon(g, s, t) {
    const flap = t ? 0.7 : 1;
    g.p([[-3, -5], [-20 * flap, -13], [-19 * flap, -4], [-17 * flap, 2], [-12, 6], [-3, 6]], 2); g.p([[3, -5], [20 * flap, -13], [19 * flap, -4], [17 * flap, 2], [12, 6], [3, 6]], 2);
    for (const x of [-15, -11]) g.l(x * flap, -8 + (x === -11 ? 3 : 0), x * flap, 2, 3, 1); for (const x of [15, 11]) g.l(x * flap, -8 + (x === 11 ? 3 : 0), x * flap, 2, 3, 1);
    g.l(0, 9, wobble(t) * 2, 16, 1, 2); g.e(wobble(t) * 2, 17, 2, 2.5, 6);
    g.e(0, 3, 4.5, 8, 1); g.e(0, 4, 2.5, 5, 2); g.e(0, -8, 3.8, 3.8, 1); g.p([[-1.5, -11], [1.5, -11], [0, -16]], 6);
    g.pt(-4, 9, 3); g.pt(3, 9, 3); g.pt(-4, -1, 3); g.pt(3, -1, 3);
    eyes(g, [[-3, -9], [2, -9]]);
  },
  bird(g, s, t) {
    const flap = t ? 0.6 : 1;
    g.p([[-2, -2], [-14 * flap, -7], [-13 * flap, 3], [-2, 5]], 2); g.p([[2, -2], [14 * flap, -7], [13 * flap, 3], [2, 5]], 2);
    g.p([[-3, 8], [0, 18], [3, 8]], 2); g.p([[-2, 8], [-5 + wobble(t), 16], [-1, 9]], 2);
    g.e(0, 1, 4, 7, 1); g.e(0, 2, 2, 4, 2); g.e(0, -6, 3.2, 3.2, 1); g.p([[-1.5, -8], [1.5, -8], [0, -12]], 6);
    eyes(g, [[-2, -7], [1, -7]]);
  },
  naga(g, s, t) {
    for (let i = 0; i <= 11; i++) { const y = 17 - i * 2, x = Math.sin(i * 0.9 + (t ? 0.8 : 0)) * 4, w = 1.4 + i * 0.28; g.e(x, y, w, w, 1); g.e(x, y + 0.6, w * 0.5, w * 0.5, 2); }
    g.e(0, -5, 5.5, 4.5, 1); g.l(-5, -6, -9, -2 + wobble(t), 1, 2); g.l(5, -6, 9, -2 - wobble(t), 1, 2); g.e(0, -11, 3.8, 3.8, 1);
    if (s.crown) g.r(-3, -15, 6, 2, 6); horns(g, s.horns, -13);
    if (s.jaw === 'fangs') { g.pt(-2, -14, 3); g.pt(1, -14, 3); }
    eyes(g, [[-2, -12], [1, -12]]);
  },
  merfolk(g, s, t) {
    const sw = wobble(t) * 2;
    g.p([[0, 10], [-6 + sw, 20], [0, 17], [6 + sw, 20]], 2);
    for (let i = 0; i <= 5; i++) { const y = 12 - i * 2.4, w = 1.8 + i * 0.5; g.e(Math.sin(i * 0.9 + (t ? 0.8 : 0)) * 2, y, w, w, 1); g.e(Math.sin(i * 0.9 + (t ? 0.8 : 0)) * 2, y + 0.5, w * 0.5, w * 0.5, 2); }
    g.e(0, -5, 5, 4.2, 1); g.l(-5, -5, -10, -1 + wobble(t), 1, 2); g.l(5, -5, 10, -1 - wobble(t), 1, 2); g.e(0, -11, 3.6, 3.6, 1);
    eyes(g, [[-2, -12], [1, -12]]);
  },
  floater(g, s, t) {
    for (let i = -3; i <= 3; i++) { const x = i * 3; g.l(x, 6, x + wobble(t ^ (i & 1)) * 2, 15 + (i & 1) * 2, 2, 1); }
    g.e(0, 0, 11 + t, 10, 1); g.e(0, 1, 7, 6, 2); g.e(0, -1, 4, 4, 5); g.e(0, -1, 2, 2, 4);
  },
  fey(g, s, t) {
    const flap = t ? 0.75 : 1;
    g.p([[-2, -3], [-15 * flap, -11], [-14 * flap, -1], [-2, 2]], 2); g.p([[2, -3], [15 * flap, -11], [14 * flap, -1], [2, 2]], 2);
    g.p([[-2, 3], [-11 * flap, 5], [-9 * flap, 11], [-1, 5]], 2); g.p([[2, 3], [11 * flap, 5], [9 * flap, 11], [1, 5]], 2);
    g.e(0, 2, 3.2, 6, 1); g.e(0, -5, 3.4, 3.4, 1); g.l(-3, 0, -6, -3, 1, 1); g.l(3, 0, 6, -3, 1, 1);
    g.p([[-3, -5], [-7, -8], [-3, -3]], 1); g.p([[3, -5], [7, -8], [3, -3]], 1); horns(g, s.horns, -7);
    eyes(g, [[-2, -6], [1, -6]]);
  },
  elemental(g, s, t) {
    const n = 8, off = t ? Math.PI / n : 0;
    for (let i = 0; i < n; i++) { const a = (i / n) * Math.PI * 2 + off; g.p([[Math.cos(a - 0.3) * 8, Math.sin(a - 0.3) * 8], [Math.cos(a) * (15 + (i & 1) * 2), Math.sin(a) * (15 + (i & 1) * 2)], [Math.cos(a + 0.3) * 8, Math.sin(a + 0.3) * 8]], 2); }
    g.e(0, 0, 9, 9, 1); g.e(0, 0, 5, 5, 2); g.e(0, 0, 2, 2, 4);
    eyes(g, [[-3, -3], [2, -3]]);
  },
  multiarm(g, s, t) {
    PLANS.biped(g, { ...s, body: 'biped' }, t);
    for (const [y, out] of [[-2, 12], [2, 13], [6, 11]]) { g.l(-8, y, -out, y - 2 + wobble(t), 1, 2); g.l(8, y, out, y - 2 - wobble(t), 1, 2); }
    if (s.gear?.includes('blade')) g.l(-13, -4, -13, -15, 7, 1);
  },
  aberration(g, s, t) {
    for (let i = 0; i < 7; i++) { const a = (i / 7) * Math.PI * 2 + (t ? 0.25 : 0); g.l(Math.cos(a) * 4, Math.sin(a) * 4, Math.cos(a) * 15, Math.sin(a) * 15 + (i & 1 ? 2 : -2), 1, 3); g.e(Math.cos(a) * 15, Math.sin(a) * 15, 2, 2, 2); }
    g.e(0, 0, 9, 9, 1); g.e(0, 1, 5, 5, 2); g.e(0, 0, 3, 3, 4);
    eyes(g, [[-6, -4], [5, -5], [-3, 5], [5, 4]]);
  },
};

const EYE = { white: C.white, yellow: C.yellow, red: C.red, cyan: C.cyan };

/** The two stride frames of a top-down creature, turned to `facing`, shaded and outlined. `spec` is the resolved creature spec. */
export function topFrames(spec, { facing = 'south', canvas = 40 } = {}) {
  if (!TOP_FACINGS.includes(facing)) throw new Error(`unknown facing "${facing}"; known: ${TOP_FACINGS.join(', ')}`);
  if (!TOP_CANVASES.includes(canvas)) throw new Error(`unknown top-down canvas "${canvas}"; known: ${TOP_CANVASES.join(', ')}`);
  const plan = TOP_PLAN[spec.body]; if (!plan) throw new Error(`unknown body "${spec.body}"`);
  const k = canvas / 40, turns = TOP_FACINGS.indexOf(facing), eye = EYE[spec.eyes] ?? C.white, skin = ramp(spec.skin);
  const under = spec.belly ? ramp(spec.belly) : [skin[1], skin[2], skin[3], skin[3]];
  const membrane = spec.body === 'winged' || spec.body === 'drake' ? ramp(spec.membrane) : under;
  const parts = { 1: skin, 2: membrane, 3: ramp(spec.bone), 4: [eye, eye, eye, eye], 5: [C.night, C.night, C.night, C.night], 6: ramp(spec.accent ?? 'red'), 7: ramp('steel') };
  return [0, 1].map((t) => {
    let m = new Pixels(canvas, canvas); PLANS[plan](kit(m, k), spec, t);
    for (let i = 0; i < turns; i++) m = m.rot90();
    return shade(m, parts);
  });
}
