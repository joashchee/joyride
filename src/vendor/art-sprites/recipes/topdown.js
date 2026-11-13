// The top-down character (`view: 'top'`, T23): the paper doll seen straight down, facing north, east, south or west (`west` is `east` mirrored,
// the way `facing: 'left'` mirrors the side view). The same layer slots, loadouts, poses, dye and body profiles as the other views; only the
// drawing changes. What a class is has to read from above, so identity rests on the headgear's outline, the weapon's length, the shield and
// the back slot, never on a face. The canvas is 40 by 40 with the body centred on (20, 20), so a weapon held forward is not cut off.
//
// Everything is drawn in the character's own frame: `u` runs toward the weapon hand, `v` runs forward. `T(facing)` maps that to the canvas.
import { Pixels } from '../pixels.js';
import { shade, dither } from '../shade.js';
import { C, ramp } from '../palette.js';
import { drawItem } from './items.js';
import { armorTexture } from './gear.js';

export const TOP = 40, CX = 20, CY = 20;
export const FACINGS = ['north', 'east', 'south', 'west'];
const HAIR = { brown: 'wood', blond: 'sand', dark: 'iron', red: 'red' };
const AXES = { north: { f: [0, -1], u: [-1, 0] }, south: { f: [0, 1], u: [1, 0] }, east: { f: [1, 0], u: [0, -1] } };
/** Hands by arm, in (u, v): v is forward. A raised arm is cocked back, a forward arm reaches. */
const HANDS = { raised: [8, -3], forward: [5, 4], down: [7, 2], out: [9, 6], up: [5, 7], carry: [3, 3], guard: [4, 5], toward: [4, 8] };
const REACHING = ['out', 'up', 'carry', 'guard', 'toward', 'forward', 'raised'];
const layer = () => new Pixels(TOP, TOP);
const deep = (r) => [r[0], r[0], r[1], r[1]];
const done = (m, parts, outline = C.night) => shade(m, parts, { outline });
const hairOf = (s) => ramp(HAIR[s.hair ?? 'brown']);

/** The canvas drawing tools for one facing (north, east or south; west is drawn as east and mirrored). */
export function T(facing) {
  const { f: [fx, fy], u: [ux, uy] } = AXES[facing], across = facing === 'east';
  const pt = (u, v) => [CX + u * ux + v * fx, CY + u * uy + v * fy];
  return {
    pt,
    ell(m, u, v, ru, rv, c) { const [x, y] = pt(u, v); return m.ellipse(x, y, across ? rv : ru, across ? ru : rv, c); },
    rect(m, u, v, du, dv, c) { return m.poly([pt(u, v), pt(u + du, v), pt(u + du, v + dv), pt(u, v + dv)], c); },
    line(m, u0, v0, u1, v1, c, t = 2) { const [x0, y0] = pt(u0, v0), [x1, y1] = pt(u1, v1); return m.thick(Math.round(x0) - (t > 1 ? 1 : 0), Math.round(y0) - (t > 1 ? 1 : 0), Math.round(x1) - (t > 1 ? 1 : 0), Math.round(y1) - (t > 1 ? 1 : 0), c, t); },
    hand(m, u, v, c) { const [x, y] = pt(u, v); return m.rect(Math.round(x) - 1, Math.round(y) - 1, 2, 2, c); },
    poly(m, pts, c) { return m.poly(pts.map(([u, v]) => pt(u, v)), c); },
  };
}

const handOf = (p) => HANDS[p.arm] ?? HANDS.down;
/** Which way the weapon points: back when the arm is cocked, to the side in a cast or a carry, forward otherwise. */
export const weaponDir = (p) => (p.arm === 'raised' ? 'back' : p.weapon === 'down' && p.arm !== 'forward' ? 'side' : 'forward');
const TURNS = { north: { forward: 0, side: 3, back: 2 }, east: { forward: 1, side: 0, back: 3 }, south: { forward: 2, side: 1, back: 0 } };

/** The weapon shape turned to point along its direction, with its grip: item art is drawn tip up. */
function turned(s, p, facing) {
  const item = drawItem({ shape: s.weapon, tier: s.tier ?? 'crafted', seed: 'worn' });
  let art = item.frames[0], [gx, gy] = item.anchors.grip;
  for (let i = 0; i < TURNS[facing][weaponDir(p)]; i++) { art = art.rot90(); [gx, gy] = [15 - gy, gx]; }
  return { art, gx, gy };
}

const TORSO_PARTS = (s) => ({ 1: ramp(s.cloth), 2: s.torso === 'plate' ? ramp('iron') : ramp('leather'), 3: deep(ramp(s.cloth)) });

const SHAPES = {
  shadow() { return dither(layer().ellipse(CX, CY + 3, 10, 8, 1), C.slate).swap(1, C.slate); },
  back(s, p, t, facing) {
    const m = layer(), kind = s.back; if (!kind) return m;
    const r = ramp(s.backRamp ?? (kind === 'cape' ? 'red' : 'leather'));
    if (kind === 'cape') { t.ell(m, 0, -3, 7, 7, 1); t.rect(m, -1, -9, 2, 6, 2); return done(m, { 1: r, 2: deep(r) }); }
    if (kind === 'quiver') { t.rect(m, -4, -7, 3, 9, 1); t.rect(m, -4, -9, 3, 2, 2); return done(m, { 1: ramp('leather'), 2: ramp('white') }); }
    t.rect(m, -5, -9, 10, 9, 1); t.rect(m, -3, -8, 6, 5, 2); return done(m, { 1: ramp('leather'), 2: ramp('wood') });
  },
  legs(s, p, t) {
    const m = layer(), [lo, ro] = p.legs;
    for (const [u, off] of [[-3, lo], [3, ro]]) { t.rect(m, u - 1.5, 1, 3, 4 + off, 1); t.rect(m, u - 1.5, 4 + off, 3, 3, 2); }
    return done(m, { 1: ramp(s.legs), 2: ramp(s.boots ?? 'leather') });
  },
  torso(s, p, t) {
    const m = layer(), [hu, hv] = handOf(p);
    if (s.torso === 'plate') { t.ell(m, 0, 0, 9, 5, 1); t.ell(m, -8, 0, 3, 3.5, 2); t.ell(m, 8, 0, 3, 3.5, 2); t.rect(m, -1, -5, 2, 10, 2); }
    else if (s.torso === 'robe') { t.ell(m, 0, 1, 8, 7, 1); t.rect(m, -1, -6, 2, 13, 2); t.rect(m, -7, 4, 14, 1, 2); }
    else { t.ell(m, 0, 0, 7, 4.5, 1); t.rect(m, -7, 2, 14, 1, 2); }
    if (s.offhand === 'shield') t.line(m, -6, 0, -9, 2, 3); else t.line(m, -6, 0, -7, 3, 3);
    t.line(m, 6, 0, hu, hv, REACHING.includes(p.arm) ? 1 : 3);
    return done(m, TORSO_PARTS(s));
  },
  offhand(s, p, t) {
    if (s.offhand !== 'shield') return layer();
    const m = layer(); t.ell(m, -11, 1, 2.5, 6, 1); t.rect(m, -12, 0, 1, 3, 3);
    return done(m, { 1: ramp('blue'), 3: ramp('gold') });
  },
  body(s, p, t, facing) {
    const m = layer(), skin = ramp(s.skin), v = Math.max(-2, Math.min(2, p.lean));
    t.ell(m, 0, v, 5, 5, 1);
    const out = shade(m, { 1: skin }, { outline: null });
    t.rect(out, -6, v - 1, 1, 2, skin[1]); t.rect(out, 5, v - 1, 1, 2, skin[1]); // the ears
    const [hu, hv] = handOf(p);
    t.hand(out, hu, hv, skin[2]);
    if (s.offhand === 'shield') t.hand(out, -9, 2, skin[1]); else t.hand(out, -7, 3, skin[1]);
    return out.outline(C.night);
  },
  headgear(s, p, t, facing) {
    const m = layer(), r = ramp(s.headRamp ?? 'steel'), v = Math.max(-2, Math.min(2, p.lean)), north = facing === 'north';
    let parts;
    if (s.head === 'helm') { t.ell(m, 0, v, 5.8, 5.8, 1); t.rect(m, -1, v - 5, 2, 10, 3); t.rect(m, -5, v - 1, 10, 1, 2); parts = { 1: r, 2: ramp('iron'), 3: ramp('red') }; }
    else if (s.head === 'horned') { t.ell(m, 0, v, 5.8, 5.8, 1); t.rect(m, -5, v - 1, 10, 1, 2); t.line(m, -5, v, -9, v - 1, 3); t.line(m, -9, v - 1, -12, v + 4, 3); t.line(m, 5, v, 9, v - 1, 3); t.line(m, 9, v - 1, 12, v + 4, 3); parts = { 1: r, 2: ramp('iron'), 3: ramp('bone') }; }
    else if (s.head === 'hood') { t.ell(m, 0, v - 1, 6.5, 6.5, 1); t.poly(m, [[-3, v - 4], [3, v - 4], [0, v - 10]], 1); t.rect(m, -1, v - 8, 2, 8, 2); parts = { 1: r, 2: deep(r) }; }
    else if (s.head === 'hat') { t.ell(m, 0, v, 9, 9, 1); t.ell(m, 0, v, 4.5, 4.5, 2); t.ell(m, 0, v, 2, 2, 1); parts = { 1: r, 2: ramp('gold') }; }
    else if (s.head === 'circlet') { t.ell(m, 0, v - (north ? 0 : 1.5), 5, north ? 5 : 4, 1); t.ell(m, 0, v, 5.3, 5.3, 2); t.ell(m, 0, v, 3.8, 3.8, 0); t.rect(m, -1, v + 4, 2, 2, 3); parts = { 1: hairOf(s), 2: ramp('gold'), 3: ramp('gold') }; }
    else { t.ell(m, 0, v - (north ? 0 : 1.5), 5.2, north ? 5.2 : 4.2, 1); parts = { 1: hairOf(s) }; }
    return done(m, parts, s.head === 'circlet' ? null : C.night);
  },
  weapon(s, p, t, facing) {
    const m = layer(); if (!s.weapon || p.weapon === 'none') return m;
    const [hu, hv] = handOf(p), [hx, hy] = t.pt(hu, hv), { art, gx, gy } = turned(s, p, facing);
    return m.blit(art, Math.round(hx) - gx, Math.round(hy) - gy);
  },
};

export const TOP_ORDER = (facing) => (facing === 'north' ? ['shadow', 'legs', 'torso', 'back', 'offhand', 'body', 'headgear', 'weapon'] : ['shadow', 'back', 'legs', 'torso', 'offhand', 'body', 'headgear', 'weapon']);

/** Move a layer along the facing by `n` pixels (a lunge) and down by `dy` (a breath, a step). */
const slide = (px, facing, n, dy) => {
  if (!n && !dy) return px;
  const { f: [fx, fy] } = AXES[facing === 'west' ? 'east' : facing];
  return px.shift(n * fx, n * fy + dy);
};

/** The layers of one top-down frame (the drawing for one facing and pose), in the view's order. `scale` is the body profile's [across, along]. */
export function topFrame(spec, p, scale = [1, 1]) {
  const facing = spec.facing === 'west' ? 'east' : spec.facing, t = T(facing), layers = {};
  for (const name of TOP_ORDER(facing)) {
    let l = SHAPES[name](spec, p, t, facing);
    if (name === 'torso') l = armorTexture(l, spec);
    if (name !== 'shadow') l = slide(l, facing, p.dx, p.dy - p.lift);
    layers[name] = l;
  }
  const [a, b] = scale; // across the body, then along the facing
  if (a !== 1 || b !== 1) { const [sx, sy] = facing === 'east' ? [b, a] : [a, b]; for (const n of Object.keys(layers)) layers[n] = layers[n].map((x, y) => layers[n].get(Math.floor((x - CX) / sx + CX), Math.floor((y - CY) / sy + CY))); }
  let comp = new Pixels(TOP, TOP); for (const name of TOP_ORDER(facing)) comp.blit(layers[name]);
  if (spec.facing === 'west') { comp = comp.flipX(); for (const n of Object.keys(layers)) layers[n] = layers[n].flipX(); }
  return { layers, comp, order: TOP_ORDER(facing) };
}

/** Named anchors for one top-down pose, in canvas pixels, inside the canvas. `root` is the body's centre. */
export function topAnchors(spec, p, scale = [1, 1]) {
  const facing = spec.facing === 'west' ? 'east' : spec.facing, t = T(facing), [lo, ro] = p.legs;
  const [hu, hv] = handOf(p), v = Math.max(-2, Math.min(2, p.lean)), dir = weaponDir(p), up = spec.weapon && p.weapon !== 'none';
  const { f: [fx, fy] } = AXES[facing];
  const tipLen = up ? 11 : 0, tip = dir === 'forward' ? [hu, hv + tipLen] : dir === 'back' ? [hu, hv - tipLen] : [hu + tipLen, hv];
  const sh = [t.pt(-7, 0), t.pt(7, 0)].sort((a, b) => a[0] - b[0] || a[1] - b[1]), ft = [t.pt(-3, 5 + lo), t.pt(3, 5 + ro)].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const off = spec.offhand === 'shield' ? [-9, 2] : [-7, 3];
  const raw = {
    root: [CX, CY], head: t.pt(0, v), neck: t.pt(0, v), shoulder_left: sh[0], shoulder_right: sh[1], hand_main: t.pt(hu, hv), hand_off: t.pt(...off),
    waist: [CX, CY], back: t.pt(0, -5), weapon_grip: t.pt(hu, hv), shield_grip: t.pt(-11, 1), effect_origin: t.pt(...tip), foot_left: ft[0], foot_right: ft[1], shadow_center: [CX, CY + 3],
  };
  const [sa, sb] = scale, [sx, sy] = facing === 'east' ? [sb, sa] : [sa, sb], out = {};
  for (const [name, [x0, y0]] of Object.entries(raw)) {
    let x = CX + (x0 - CX) * sx + p.dx * fx, y = CY + (y0 - CY) * sy + p.dx * fy + p.dy - p.lift;
    if (spec.facing === 'west') x = TOP - 1 - x;
    out[name] = [Math.min(TOP - 1, Math.max(0, Math.round(x))), Math.min(TOP - 1, Math.max(0, Math.round(y)))];
  }
  return out;
}
