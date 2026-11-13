// Props seen from above (T16, `prop` with `view: 'top'`): the things a dungeon or a wood is dressed with, on the world's 32 pixel grid. A prop returns its
// `footprint` (whole tiles) and `occlusion` (`none`, `partial` or `blocks-sight`) and nothing about walking: collision stays the game's. Props with a
// condition take a `state`, and the ones that move have clips (flicker, glow, swirl, open, trigger) with events and a reduced still; every prop has a `reveal`.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { clip } from '../anim.js';
import { TOP_FURNITURE } from './topfurniture.js';

const T = 32;
const mk = (w = 1, h = 1) => new Pixels(w * T, h * T);
const done = (m, parts) => shade(m, parts);
const dots = (p, pts, c) => pts.forEach(([x, y]) => p.set(x, y, c));

const glow = (p, cx, cy, r, c, t = 0) => { for (let y = cy - r; y <= cy + r; y++) for (let x = cx - r; x <= cx + r; x++) if (!p.get(x, y) && (x - cx) ** 2 + (y - cy) ** 2 <= r * r && (x + y + t) % 2 === 0) p.set(x, y, c); return p; };

/** Every top-down prop: its footprint, how it hides what is behind it, the states it can be in and `make(state, t)` which draws it (`t` is the frame of its loop). */
export const TOP_PROPS = {
  tree: { foot: [1, 1], occ: 'partial', make() { const m = mk(); m.ellipse(16, 16, 14, 13, 1).ellipse(10, 11, 6, 5, 2).ellipse(21, 20, 6, 5, 3).ellipse(19, 9, 4, 3, 2); return done(m, { 1: ramp('green'), 2: ramp('grass'), 3: ramp('moss') }); } },
  pine: { foot: [1, 1], occ: 'partial', make() { const m = mk(), pts = []; for (let i = 0; i < 16; i++) { const a = (i / 16) * Math.PI * 2, r = i & 1 ? 7 : 14; pts.push([16 + Math.cos(a) * r, 16 + Math.sin(a) * r]); } m.poly(pts, 1).ellipse(16, 16, 5, 5, 2).ellipse(15, 15, 2, 2, 3); return done(m, { 1: ramp('green'), 2: ramp('moss'), 3: ramp('grass') }); } },
  bush: { foot: [1, 1], occ: 'partial', make() { const m = mk(); m.ellipse(16, 17, 11, 9, 1).ellipse(10, 14, 6, 5, 1).ellipse(22, 14, 6, 5, 1).ellipse(15, 19, 4, 3, 2); return done(m, { 1: ramp('grass'), 2: ramp('green') }); } },
  rock: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.poly([[5, 20], [7, 9], [15, 4], [25, 8], [28, 19], [20, 27], [10, 26]], 1).poly([[14, 12], [21, 11], [24, 18], [16, 20]], 2); return done(m, { 1: ramp('stone'), 2: ramp('iron') }); } },
  stump: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.ellipse(16, 16, 9, 9, 1).ellipse(16, 16, 6, 6, 2).ellipse(16, 16, 3, 3, 3); return done(m, { 1: ramp('wood'), 2: ramp('sand'), 3: ramp('wood') }); } },
  door: {
    foot: [1, 1], occ: 'blocks-sight', states: ['closed', 'open', 'locked'],
    make(state) {
      const m = mk(), wood = { 1: ramp('wood'), 2: ramp('iron'), 3: ramp('gold') };
      if (state === 'open') { m.rect(2, 3, 5, 26, 1).rect(1, 3, 2, 3, 2).rect(1, 25, 2, 3, 2); }
      else { m.rect(2, 11, 28, 10, 1).rect(2, 10, 3, 12, 2).rect(27, 10, 3, 12, 2); m.rect(14, 11, 1, 10, 2).rect(18, 11, 1, 10, 2); if (state === 'locked') m.rect(14, 14, 5, 5, 3); }
      return done(m, wood);
    },
  },
  exit: { foot: [1, 1], occ: 'none', make(state, t) { const m = mk(); m.rect(3, 3, 26, 26, 1).rect(6, 6, 20, 20, 2); m.poly([[16, 24], [8, 14], [13, 14], [13, 8], [19, 8], [19, 14], [24, 14]], 3); const out = done(m, { 1: ramp('stone'), 2: [C.night, C.night, C.plum, C.plum], 3: ramp('gold') }); return glow(out, 16, 16, 16, ramp('gold')[2], t); }, loop: { glow: 4 } },
  chest: {
    foot: [1, 1], occ: 'none', states: ['closed', 'open', 'locked'],
    make(state) {
      const m = mk(), wood = { 1: ramp('wood'), 2: ramp('iron'), 3: ramp('gold'), 4: [C.night, C.night, C.plum, C.plum] };
      if (state === 'open') { m.rect(5, 2, 22, 7, 1).rect(5, 2, 22, 2, 2); m.rect(5, 10, 22, 17, 1).rect(7, 12, 18, 13, 4); m.rect(5, 10, 22, 2, 2); dots(m, [[10, 17], [14, 20], [19, 16], [21, 21], [12, 15]], 3); }
      else { m.rect(5, 6, 22, 21, 1).rect(5, 6, 22, 2, 2).rect(5, 25, 22, 2, 2).rect(5, 6, 2, 21, 2).rect(25, 6, 2, 21, 2).rect(14, 6, 4, 21, 2); m.rect(14, state === 'locked' ? 15 : 13, 4, 4, 3); }
      return done(m, wood);
    },
    clips: ['open'],
  },
  torch: { foot: [1, 1], occ: 'none', make(state, t) { const m = mk(), f = [3, 4, 3, 5][t % 4]; m.ellipse(16, 18, 3, 3, 1).ellipse(16, 18, 2, 2, 2); const base = done(m, { 1: ramp('wood'), 2: ramp('iron') }), fl = new Pixels(T, T); const fr = ramp('fire'); fl.ellipse(16, 15, f, f + 1, 1).ellipse(16, 15, f - 1, f, 2).ellipse(16, 16, 1, 2, 3); for (let y = 0; y < T; y++) for (let x = 0; x < T; x++) { const v = fl.get(x, y); if (v) base.set(x, y, fr[v === 1 ? 1 : v === 2 ? 2 : 3]); } return glow(base, 16, 15, 9 + (t & 1), fr[1], t); }, loop: { flicker: 4 } },
  brazier: { foot: [1, 1], occ: 'none', make(state, t) { const m = mk(); m.ellipse(16, 16, 11, 11, 1).ellipse(16, 16, 8, 8, 2); const out = done(m, { 1: ramp('iron'), 2: [C.night, C.night, C.plum, C.plum] }), fr = ramp('fire'), f = [5, 6, 5, 7][t % 4]; out.ellipse(16, 16, f, f, fr[1]).ellipse(16, 16, f - 2, f - 2, fr[2]).ellipse(16, 16, 2, 2, fr[3]); return glow(out, 16, 16, 15, fr[1], t); }, loop: { flicker: 4 } },
  crate: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(4, 4, 24, 24, 1).rect(4, 4, 24, 3, 2).rect(4, 25, 24, 3, 2).rect(4, 4, 3, 24, 2).rect(25, 4, 3, 24, 2); m.thick(7, 7, 25, 25, 2, 2).thick(25, 7, 7, 25, 2, 2); return done(m, { 1: ramp('wood'), 2: ramp('dirt') }); } },
  urn: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.ellipse(16, 16, 10, 10, 1).ellipse(16, 16, 7, 7, 2).ellipse(16, 16, 4, 4, 3); return done(m, { 1: ramp('dirt'), 2: ramp('leather'), 3: [C.night, C.night, C.plum, C.plum] }); } },
  statue: { foot: [1, 1], occ: 'partial', make() { const m = mk(); m.rect(5, 5, 22, 22, 1).rect(7, 7, 18, 18, 2); m.ellipse(16, 14, 5, 5, 3).ellipse(16, 21, 8, 4, 3); return done(m, { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('stone') }); } },
  coffin: { foot: [1, 2], occ: 'none', make() { const m = mk(1, 2); m.poly([[8, 3], [24, 3], [27, 18], [22, 60], [10, 60], [5, 18]], 1).poly([[11, 8], [21, 8], [23, 20], [20, 54], [12, 54], [9, 20]], 2); m.rect(14, 22, 4, 12, 3); m.rect(11, 26, 10, 3, 3); return done(m, { 1: ramp('wood'), 2: ramp('dirt'), 3: ramp('bone') }); } },
  lever: { foot: [1, 1], occ: 'none', states: ['off', 'on'], make(state) { const m = mk(); m.rect(6, 12, 20, 12, 1).rect(8, 14, 16, 8, 2); if (state === 'on') m.thick(16, 19, 24, 6, 3, 3).ellipse(24, 6, 3, 3, 4); else m.thick(16, 19, 8, 6, 3, 3).ellipse(8, 6, 3, 3, 4); return done(m, { 1: ramp('iron'), 2: [C.night, C.night, C.plum, C.plum], 3: ramp('steel'), 4: state === 'on' ? ramp('green') : ramp('red') }); } },
  plate: { foot: [1, 1], occ: 'none', states: ['up', 'down'], make(state) { const m = mk(); m.rect(4, 4, 24, 24, 1); if (state === 'down') m.rect(7, 7, 18, 18, 2); else m.rect(6, 6, 20, 20, 3).rect(9, 9, 14, 14, 2); return done(m, { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('steel') }); } },
  trap: {
    foot: [1, 1], occ: 'none', states: ['armed', 'triggered', 'disarmed'],
    make(state) { const m = mk(); m.rect(4, 4, 24, 24, 1); for (const x of [9, 16, 23]) for (const y of [9, 16, 23]) { if (state === 'disarmed') m.rect(x - 1, y - 1, 2, 2, 2); else if (state === 'armed') m.set(x, y, 3).set(x, y - 1, 3); else m.rect(x, y - 3, 1, 5, 3).set(x, y - 4, 4); } return done(m, { 1: ramp('stone'), 2: [C.night, C.night, C.plum, C.plum], 3: ramp('steel'), 4: ramp('red') }); },
    clips: ['trigger'],
  },
  web: { foot: [1, 1], occ: 'none', make() { const m = mk(); for (let i = 0; i < 6; i++) m.line(1, 1, 30, 4 + i * 5, 1); for (const r of [8, 15, 22]) for (let a = 0; a <= 90; a += 6) { const t = (a * Math.PI) / 180; m.set(1 + Math.cos(t) * r, 1 + Math.sin(t) * r, 1); } return done(m, { 1: ramp('white') }); } },
  crystal: { foot: [1, 1], occ: 'none', make(state, t) { const m = mk(); for (const [x, y, h] of [[16, 16, 11], [9, 19, 7], [23, 20, 8]]) m.poly([[x, y - h], [x + 4, y], [x, y + 4], [x - 4, y]], 1); const out = done(m, { 1: ramp('glass') }); return glow(out, 16, 17, 10 + (t & 1) * 2, ramp('glass')[1], t); }, loop: { glow: 4 } },
  // Harvest: `seedling` and `growing` come before `ripe`, then `picked` (and it grows back); the default stays `ripe`. The leaves reach out further each stage.
  plant: {
    foot: [1, 1], occ: 'partial', states: ['ripe', 'picked', 'seedling', 'growing'], clips: ['harvest'],
    make(state) { const m = mk(), reach = { seedling: 5, growing: 8 }[state] ?? 11, n = state === 'seedling' ? 3 : 7; for (let i = 0; i < n; i++) { const a = (i / n) * Math.PI * 2; m.thick(16, 16, 16 + Math.cos(a) * reach, 16 + Math.sin(a) * reach, 1, state === 'seedling' ? 2 : 3); m.ellipse(16 + Math.cos(a) * reach, 16 + Math.sin(a) * reach, reach === 11 ? 3 : reach / 4 + 1, reach === 11 ? 3 : reach / 4 + 1, 1); } m.ellipse(16, 16, state === 'seedling' ? 2 : 4, state === 'seedling' ? 2 : 4, 2); const out = done(m, { 1: ramp('green'), 2: ramp('grass') }); if (state === 'ripe') dots(out, [[12, 14], [19, 17], [15, 20], [21, 12]], C.red); if (state === 'growing') dots(out, [[13, 15], [19, 18]], C.moss); return out; },
  },
  mushroom: {
    foot: [1, 1], occ: 'none', states: ['ripe', 'picked', 'seedling', 'growing'], clips: ['harvest'],
    make(state) { const m = mk(), caps = { seedling: [[16, 17, 3]], growing: [[12, 18, 4], [20, 15, 4]], ripe: [[10, 19, 5], [21, 14, 6], [18, 23, 4]], picked: [] }[state]; for (const [x, y, r] of caps) { m.ellipse(x, y, r, r, 1); m.ellipse(x - 1, y - 1, r / 2, r / 2, 3); } m.ellipse(16, 26, 6, 2, 2); return done(m, { 1: ramp('skin-red'), 2: ramp('moss'), 3: ramp('bone') }); },
  },
  ore: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.poly([[4, 22], [6, 9], [15, 4], [25, 8], [28, 20], [19, 27], [9, 27]], 1); for (const [x, y] of [[12, 11], [20, 14], [14, 20], [22, 21]]) m.poly([[x, y - 3], [x + 3, y], [x, y + 3], [x - 3, y]], 2); return done(m, { 1: ramp('stone'), 2: ramp('gold') }); } },
  gate: {
    foot: [2, 1], occ: 'blocks-sight', states: ['closed', 'open', 'locked'],
    make(state) {
      const m = mk(2, 1), iron = { 1: ramp('iron'), 2: ramp('steel'), 3: ramp('gold'), 4: [C.night, C.night, C.plum, C.plum] };
      m.rect(0, 8, 6, 16, 1).rect(58, 8, 6, 16, 1);                                    // the two posts
      if (state === 'open') { m.rect(6, 12, 3, 8, 2).rect(55, 12, 3, 8, 2); m.rect(9, 14, 46, 4, 4); }
      else { for (let x = 9; x < 56; x += 5) m.rect(x, 9, 2, 14, 2); m.rect(6, 11, 52, 2, 1).rect(6, 19, 52, 2, 1); if (state === 'locked') m.rect(29, 13, 6, 6, 3); }
      return done(m, iron);
    },
    clips: ['open'],
  },
  dust: {
    foot: [1, 1], occ: 'none', loop: { drift: 6 },
    make(state, t) { const m = mk(), r = rng('dust'); for (let i = 0; i < 9; i++) { const x = r.int(30), y = r.int(30); m.set((x + t * (1 + (i % 2))) % 32, (y + t * (2 - (i % 2)) + (i & 3)) % 32, i % 3 ? 1 : 2); } return done(m, { 1: [C.dust, C.dust, C.mist, C.mist], 2: [C.mist, C.mist, C.ice, C.white] }); },
  },
  gears: {
    foot: [1, 1], occ: 'none', loop: { turn: 4 },
    make(state, t) {
      const m = mk(), cog = (cx, cy, r, teeth, phase, id) => { for (let i = 0; i < teeth; i++) { const a = ((i + phase) / teeth) * Math.PI * 2; m.thick(cx, cy, cx + Math.cos(a) * (r + 2), cy + Math.sin(a) * (r + 2), id, 2); } m.ellipse(cx, cy, r, r, id); m.ellipse(cx, cy, r / 2, r / 2, 3); };
      cog(11, 12, 6, 8, t / 4, 1); cog(22, 20, 5, 6, 0.5 - t / 3, 2);
      return done(m, { 1: ramp('steel'), 2: ramp('gold'), 3: [C.night, C.night, C.plum, C.plum] });
    },
  },
  piston: {
    foot: [1, 1], occ: 'none', loop: { pump: 4 },
    make(state, t) { const m = mk(), ext = [0, 5, 10, 5][t % 4]; m.rect(4, 8, 24, 16, 1).rect(6, 10, 20, 12, 3); m.rect(6 + ext, 12, 8, 8, 2).rect(14 + ext, 15, 12 - ext, 2, 2); for (const y of [9, 22]) for (const x of [5, 25]) m.set(x, y, 4); return done(m, { 1: ramp('iron'), 2: ramp('steel'), 3: [C.night, C.night, C.plum, C.plum], 4: ramp('gold') }); },
  },
  // A belt, one tile, running the way its state says: the slats slide a pixel a frame and chevrons on them point the way. Several side by side make a line.
  conveyor: {
    foot: [1, 1], occ: 'none', states: ['east', 'west', 'north', 'south'], loop: { run: 4 },
    make(state, t) {
      const m = mk(), v = state === 'north' || state === 'south', sign = state === 'east' || state === 'south' ? 1 : -1, lane = new Pixels(T, T);
      m.rect(v ? 4 : 0, v ? 0 : 4, v ? 24 : 32, v ? 32 : 24, 1);                                                 // the frame and its rollers
      lane.rect(v ? 8 : 0, v ? 0 : 8, v ? 16 : 32, v ? 32 : 16, 1);                                              // the belt
      const out = done(m, { 1: ramp('iron') });
      const slat = ramp('leather'), shift = (sign * t * 2 + 64) % 8;
      for (let y = 0; y < T; y++) for (let x = 0; x < T; x++) {
        if (!lane.get(x, y)) continue;
        const along = ((v ? y : x) - shift + 64) % 8, across = v ? x - 8 : y - 8;
        out.set(x, y, along === 0 ? slat[0] : along === 1 ? slat[3] : along < 7 ? slat[2] : slat[1]);
        const chev = Math.abs(across - 7.5), at = along;   // a chevron every slat, its point toward the way of travel
        if (Math.round(chev) === (sign > 0 ? 5 - at : at - 2) && at >= 2 && at <= 5 && chev < 4) out.set(x, y, ramp('gold')[2]);
      }
      for (const a of [8, 23]) for (let i = 0; i < T; i += 8) v ? out.set(a === 8 ? 5 : 26, i + 3, C.steel) : out.set(i + 3, a === 8 ? 5 : 26, C.steel);
      return out;
    },
  },
  ventsteam: {
    foot: [1, 1], occ: 'partial', loop: { steam: 4 },
    make(state, t) { const m = mk(); m.rect(8, 8, 16, 16, 1).rect(10, 10, 12, 12, 2); for (let i = 0; i < 3; i++) m.rect(11, 12 + i * 3, 10, 1, 1); const out = done(m, { 1: ramp('iron'), 2: [C.night, C.night, C.plum, C.plum] }), w = ramp('white'); for (let i = 0; i < 6; i++) { const a = ((i + t) / 6) * Math.PI * 2; out.ellipse(16 + Math.cos(a) * (3 + t), 16 + Math.sin(a) * (3 + t), 2 + (i & 1), 2, w[2 + (i & 1)]); } return glow(out, 16, 16, 11 + t, w[1], t); },
  },
  banner: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(5, 6, 22, 20, 1).rect(5, 6, 22, 2, 3); m.poly([[16, 10], [21, 16], [16, 22], [11, 16]], 2); return done(m, { 1: ramp('red'), 2: ramp('gold'), 3: ramp('gold') }); } },
  bookshelf: { foot: [2, 1], occ: 'partial', make() { const m = mk(2, 1), r = rng('bookshelf'); m.rect(2, 4, 60, 24, 1).rect(4, 6, 56, 20, 2); for (let x = 5; x < 58; ) { const w = r.pick([2, 3, 3]); m.rect(x, 7, w, 18, r.pick([3, 4, 5, 6])); x += w + 1; } return done(m, { 1: ramp('wood'), 2: [C.night, C.night, C.plum, C.plum], 3: ramp('red'), 4: ramp('blue'), 5: ramp('green'), 6: ramp('gold') }); } },
  lectern: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(7, 6, 18, 20, 1).rect(9, 8, 14, 14, 2).rect(15, 8, 2, 14, 3); return done(m, { 1: ramp('wood'), 2: ramp('bone'), 3: ramp('dirt') }); } },
  scrollrack: { foot: [1, 1], occ: 'none', make() { const m = mk(); m.rect(3, 6, 26, 20, 1); for (let i = 0; i < 4; i++) for (let j = 0; j < 3; j++) m.ellipse(8 + i * 6, 11 + j * 6, 2.2, 2.2, 2); return done(m, { 1: ramp('wood'), 2: ramp('bone') }); } },
  portal: { foot: [1, 1], occ: 'none', make(state, t) { const m = mk(), p = ramp('purple'); m.ellipse(16, 16, 14, 14, 1).ellipse(16, 16, 10, 10, 2); const out = done(m, { 1: ramp('stone'), 2: [C.night, C.indigo, C.violet, C.pink] }); for (let i = 0; i < 12; i++) { const a = (i / 12) * Math.PI * 2 + t * 0.5, r = 3 + (i % 4) * 2; out.set(16 + Math.cos(a) * r, 16 + Math.sin(a) * r, p[1 + (i % 3)]); } return out; }, loop: { swirl: 4 } },
};
Object.assign(TOP_PROPS, Object.fromEntries(Object.entries(TOP_FURNITURE).filter(([k]) => !(k in TOP_PROPS))));   // furniture from above (T23)
export const TOP_PROP_IDS = Object.keys(TOP_PROPS);

const reveal = (px, seed) => { const r = rng(`${seed}:reveal`), cut = new Map(); const gate = (n) => px.map((x, y) => { const v = px.get(x, y); if (!v) return 0; const k = `${x},${y}`; if (!cut.has(k)) cut.set(k, r.next()); return cut.get(k) < n ? v : 0; }); return [gate(0.1), gate(0.4), gate(0.75), px]; };

/** One top-down prop. `state` is one of the prop's own (the first is the default). Returns `footprint`, `occlusion`, `states` and its loop and event clips. */
export function drawTopProp({ kind = 'chest', state, seed = 'prop' } = {}) {
  const def = TOP_PROPS[kind]; if (!def) throw new Error(`unknown top-down prop "${kind}"; known: ${TOP_PROP_IDS.join(', ')}`);
  const states = def.states ?? [], cur = state ?? states[0];
  if (state !== undefined && !states.includes(state)) throw new Error(`prop "${kind}" has no state "${state}"${states.length ? `; known: ${states.join(', ')}` : ' (it has no states)'}`);
  const px = def.make(cur, 0), clips = { reveal: clip('reveal', reveal(px, `${seed}:${kind}`), { fps: 6, loop: false, events: [{ frame: 3, name: 'revealed' }], reduced: [px] }) };
  for (const [name, n] of Object.entries(def.loop ?? {})) { const f = Array.from({ length: n }, (_, t) => def.make(cur, t)); clips[name] = clip(name, f, { fps: 6, loop: true }); }
  if (def.clips?.includes('open')) clips.open = clip('open', [def.make('closed', 0), def.make('closed', 0).shift(0, -1), def.make('open', 0)], { fps: 6, loop: false, events: [{ frame: 2, name: 'opened' }], reduced: [def.make('open', 0)] });
  if (def.clips?.includes('harvest')) { const st = (n) => def.make(n, 0); clips.harvest = clip('harvest', [st('seedling'), st('growing'), st('ripe'), st('picked')], { fps: 3, loop: false, events: [{ frame: 2, name: 'ripe' }, { frame: 3, name: 'picked' }], reduced: [st('ripe')] }); }
  if (def.clips?.includes('trigger')) clips.trigger = clip('trigger', [def.make('armed', 0), def.make('armed', 0).shift(0, 1), def.make('triggered', 0)], { fps: 8, loop: false, events: [{ frame: 2, name: 'sprung' }], reduced: [def.make('triggered', 0)] });
  const frames = def.loop ? Object.values(clips).find((c) => c.loop && c.frames.length > 1).frames : [px], animated = frames.length > 1;
  return { id: `prop.top.${kind}`, frames, fps: animated ? 6 : 0, loop: animated, spec: { kind, state: cur }, projection: 'top-down', footprint: def.foot, occlusion: def.occ, states, clips, anchors: { root: [px.w >> 1, px.h >> 1], shadow_center: [px.w >> 1, px.h >> 1] } };
}
