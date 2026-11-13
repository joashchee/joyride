// Equipment and looks beyond the base paper doll (T7): torso families, boots, gloves, belt, shoulders, eyewear, hair styles and facial hair. Each is an
// optional layer in `layers`, drawn only when its option is set, so a character without them is exactly what it always was and a missing one never hides
// the character. They are drawn from the character's named anchors (the head, the hands, the feet, the waist, the shoulders) in the final canvas, so one
// drawing serves every view, facing, pose and body profile. Torso families (`armor`) are a texture on the torso layer, in its own colours.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

export const ARMOR_FAMILIES = ['cloth', 'leather', 'chain', 'scale', 'plate'];
export const BOOT_STYLES = ['short', 'tall', 'greaves'];
export const HAIR_STYLES = ['long', 'ponytail', 'bun', 'topknot', 'mohawk', 'braids', 'pigtails', 'curly'];
export const FACIAL_HAIR = ['beard', 'mustache', 'goatee', 'stubble'];
export const EYEWEAR = ['goggles', 'mask'];
export const NECK_KINDS = ['amulet', 'scarf', 'collar'];
export const SCARS = ['brow', 'cheek', 'mouth'];
export const JEWELLERY = ['earring', 'brooch'];
export const HELM_STYLES = ['visored', 'crested', 'plumed', 'winged'];
/** Where each extra layer goes in the order: after this base layer. */
export const EXTRA_AFTER = { boots: 'legs', tabard: 'torso', belt: 'torso', bracers: 'torso', shoulders: 'torso', neck: 'torso', cloak: 'torso', gloves: 'body', scar: 'body', jewellery: 'body', facial: 'body', helmstyle: 'headgear', hairstyle: 'headgear', eyewear: 'hairstyle' };
const HAIR = { brown: 'wood', blond: 'sand', dark: 'iron', red: 'red' };

/** A torso family as a texture on the torso layer's own colours: a step lighter or darker within its ramp, so dye and shading still work. `ox`, `oy` is how far the layer was shifted, so the texture rides with the body. */
export function armorTexture(torso, spec, ox = 0, oy = 0) {
  const fam = spec.armor; if (!fam || fam === 'cloth') return torso;
  const r = ramp(spec.cloth), step = (v) => r.indexOf(v), out = torso.clone(), b = torso.bbox(); if (!b) return torso;
  const set = (x, y, d) => { const v = torso.get(x, y), i = step(v); if (i >= 0) out.set(x, y, r[Math.max(0, Math.min(3, i + d))]); };
  for (let y = b.y; y < b.y + b.h; y++) for (let x = b.x; x < b.x + b.w; x++) {
    const lx = x - ox, ly = y - oy;
    if (fam === 'chain') { if ((lx + ly) % 2 === 0) set(x, y, -1); }
    else if (fam === 'scale') { const row = ((ly % 3) + 3) % 3, shift = (Math.floor(ly / 3) & 1) * 2; if (row === 0 && ((lx + shift) % 4 + 4) % 4 < 2) set(x, y, 1); else if (row === 2) set(x, y, -1); }
    else if (fam === 'leather') { if (lx === 16 && ly % 2 === 0) set(x, y, -2); else if ((lx + 2 * ly) % 7 === 0) set(x, y, -1); }
    else if (fam === 'plate') { if (!torso.get(x - 1, y) || !torso.get(x, y - 1) || ly % 5 === 0) set(x, y, 1); else if (!torso.get(x + 1, y) || !torso.get(x, y + 1)) set(x, y, -1); }
  }
  return out;
}

const dark = C.night;
const put = (m, rows, ox, oy, legend) => m.blit(Pixels.fromRows(rows, legend), ox, oy);
const at = (a, n) => a[n] ?? [0, 0];

/**
 * The extra layers for a drawn frame. `a` is the frame's final anchors, `layers` its layers (already flipped and scaled), `view` and `facing` how it is seen.
 * Returns `{ name: Pixels }` for every extra the spec asks for (the same size as the torso layer).
 */
export function gearLayers(spec, view, facing, a, layers) {
  const w = layers.torso.w, h = layers.torso.h, out = {}, top = view === 'top', back = view === 'back', side = view === 'side', quarter = view === 'quarter';
  const dir = side ? (facing === 'left' ? -1 : 1) : top ? (facing === 'west' ? -1 : 1) : 1, mk = () => new Pixels(w, h);
  const [hx, hy] = at(a, 'head'), skin = ramp(spec.skin), hair = ramp(HAIR[spec.hair ?? 'brown']);
  if (spec.bootStyle) {
    const m = mk(), r = ramp(spec.bootStyle === 'greaves' ? 'iron' : spec.boots ?? 'leather'), tall = spec.bootStyle !== 'short';
    for (const n of ['foot_left', 'foot_right']) {
      const [x, y] = at(a, n);
      if (top) { m.rect(x - 1, y - 1, 3, 3, 1); if (tall) m.rect(x - 1, y - 1, 3, 1, 2); }
      else { const bw = side ? 4 : 3; m.rect(x, tall ? 25 : 27, bw, tall ? 5 : 3, 1); if (tall) m.rect(x, 25, bw, 1, 2); }
    }
    out.boots = shade(m, { 1: r, 2: ramp(spec.bootStyle === 'greaves' ? 'steel' : 'bone') });
  }
  if (spec.belt && !top) {
    const m = mk(), [wx, wy] = at(a, 'waist'), r = ramp(spec.belt), t = layers.torso;
    for (let y = wy - 1; y <= wy; y++) for (let x = 0; x < w; x++) { const v = t.get(x, y); if (v && v !== dark) m.set(x, y, y === wy ? r[1] : r[2]); }
    if (!back && !side) put(m, ['XX', 'XX'], wx - 1, wy - 1, { X: ramp('gold')[3] });
    out.belt = m.outline(dark);
  }
  if (spec.shoulders) {
    const m = mk(), r = ramp(spec.shoulders), pts = side ? [at(a, 'shoulder_left')] : [at(a, 'shoulder_left'), at(a, 'shoulder_right')];
    for (const [x, y] of pts) m.ellipse(x + 0.5, y + 0.5, top ? 3.5 : 3, top ? 3 : 2.4, 1);
    out.shoulders = shade(m, { 1: r });
  }
  if (spec.gloves) {
    const m = mk(), r = ramp(spec.gloves);
    for (const n of ['hand_main', 'hand_off']) { const [x, y] = at(a, n); m.rect(x - 1, y - 1, 3, 3, 1); }
    out.gloves = shade(m, { 1: r });
  }
  if (spec.facialHair && !back && !top) {
    const m = mk(), f = spec.facialHair, fx = side ? hx + 2 * dir : quarter ? hx + 1 : hx, lo = hy + 3;
    if (f === 'beard') { m.ellipse(fx + 0.5, lo + 2, side ? 3 : 4.5, 3.2, 1); m.rect(fx - 2, lo - 1, 5, 1, 0); }
    else if (f === 'mustache') { m.rect(fx - 3, lo - 1, 7, 1, 1); m.rect(fx - 4, lo, 2, 1, 1); m.rect(fx + 3, lo, 2, 1, 1); }
    else if (f === 'goatee') { m.rect(fx - 1, lo, 3, 4, 1); m.rect(fx - 2, lo - 1, 5, 1, 1); }
    else { for (let y = lo - 1; y < lo + 3; y++) for (let x = fx - 4; x <= fx + 4; x++) if ((x + y) % 2 === 0 && layers.body.get(x, y) && skin.includes(layers.body.get(x, y))) m.set(x, y, 1); }
    out.facial = shade(m, { 1: hair }, { outline: null });
  }
  if (spec.hairStyle && (spec.head === 'hair' || spec.head === 'circlet')) {
    const m = mk(), s = spec.hairStyle, back2 = back || (top && facing === 'north');
    if (top) {
      const f = { north: [0, 1], south: [0, -1], east: [-1, 0], west: [1, 0] }[facing] ?? [0, 1]; // back of the head, on the canvas
      const bx = hx + f[0] * 6, by = hy + f[1] * 6;
      if (s === 'ponytail') m.thick(hx + f[0] * 4, hy + f[1] * 4, hx + f[0] * 10, hy + f[1] * 10, 1, 2);
      else if (s === 'long') m.ellipse(bx, by, f[0] ? 4 : 6, f[1] ? 4 : 6, 1);
      else if (s === 'bun' || s === 'topknot') m.ellipse(hx + 0.5, hy + 0.5, 2.5, 2.5, 1);
      else if (s === 'mohawk') m.rect(hx - (f[0] ? 5 : 1), hy - (f[1] ? 5 : 1), f[0] ? 10 : 2, f[1] ? 10 : 2, 1);
      else if (s === 'braids') { m.thick(hx - 4, hy, hx - 4 + f[0] * 8, hy + f[1] * 8, 1, 1); m.thick(hx + 4, hy, hx + 4 + f[0] * 8, hy + f[1] * 8, 1, 1); }
      else if (s === 'pigtails') { m.ellipse(hx - 6, hy + 0.5, 2, 2, 1); m.ellipse(hx + 6, hy + 0.5, 2, 2, 1); }
      else { for (let y = hy - 7; y <= hy + 7; y++) for (let x = hx - 7; x <= hx + 7; x++) if ((x - hx) ** 2 + (y - hy) ** 2 <= 49 && (x - hx) ** 2 + (y - hy) ** 2 >= 25 && (x + y) % 2 === 0) m.set(x, y, 1); }
    } else if (s === 'long') { if (back2) m.rect(hx - 5, hy, 10, 9, 1); else if (side) m.rect(hx - 6 * dir - (dir > 0 ? 0 : -1), hy - 1, 3, 9, 1); else { m.rect(hx - 6, hy - 1, 2, 9, 1); m.rect(hx + 4, hy - 1, 2, 9, 1); } }
    else if (s === 'ponytail') { if (back2) m.thick(hx, hy - 2, hx, hy + 9, 1, 2); else if (side) m.thick(hx - 5 * dir, hy - 3, hx - 8 * dir, hy + 6, 1, 2); else m.rect(hx - 1, hy - 8, 3, 2, 1); }
    else if (s === 'bun') m.ellipse(hx + 0.5, hy - 6.5, 2.6, 2.4, 1);
    else if (s === 'topknot') { m.ellipse(hx + 0.5, hy - 8, 1.8, 2, 1); m.rect(hx - 1, hy - 6, 3, 1, 1); }
    else if (s === 'mohawk') { if (side) m.rect(hx - 4, hy - 7, 8, 2, 1); else m.rect(hx - 1, hy - 8, 3, 7, 1); }
    else if (s === 'braids') { if (side) m.thick(hx - 4 * dir, hy, hx - 5 * dir, hy + 9, 1, 1); else { m.thick(hx - 5, hy, hx - 6, hy + 9, 1, 1); m.thick(hx + 5, hy, hx + 6, hy + 9, 1, 1); } }
    else if (s === 'pigtails') { if (side) m.ellipse(hx - 5 * dir + 0.5, hy + 2, 2, 3, 1); else { m.ellipse(hx - 7 + 0.5, hy + 1, 2, 3, 1); m.ellipse(hx + 7 + 0.5, hy + 1, 2, 3, 1); } }
    else { for (let y = hy - 7; y <= hy + 5; y++) for (let x = hx - 7; x <= hx + 7; x++) if ((x - hx) ** 2 / 49 + (y - hy + 1) ** 2 / 42 <= 1 && (x - hx) ** 2 / 20 + (y - hy + 1) ** 2 / 20 >= 1 && y <= hy + 2 && (x + y) % 2 === 0) m.set(x, y, 1); }
    out.hairstyle = shade(m, { 1: hair });
  }
  if (spec.eyewear) {
    const m = mk(), r = spec.eyewear === 'goggles' ? [ramp('glass'), ramp('iron')] : [ramp('iron'), ramp('iron')];
    if (top) m.rect(hx - 5, hy - 1, 10, 2, 2);
    else if (back) m.rect(hx - 6, hy, 12, 1, 2);
    else if (spec.eyewear === 'goggles') {
      if (side) { m.ellipse(hx + 4 * dir + 0.5, hy + 1, 2, 2, 1); m.rect(Math.min(hx - 5, hx + 2 * dir), hy, 8, 1, 2); }
      else { m.ellipse(hx - 2.5 + (quarter ? 1 : 0), hy + 1, 2, 2, 1); m.ellipse(hx + 3.5 + (quarter ? 1 : 0), hy + 1, 2, 2, 1); m.rect(hx - 6, hy, 12, 1, 2); }
    } else if (side) m.rect(hx + (dir > 0 ? 1 : -5), hy + 1, 5, 4, 1); else m.rect(hx - 4, hy + 1, 9, 4, 1);
    out.eyewear = shade(m, { 1: r[0], 2: r[1] });
  }
  // Phase 4 (T7 rest): tabard, bracers, neck, scar, jewellery and helm styles, from the same anchors.
  if (spec.tabard) {
    const m = mk(), [wx, wy] = at(a, 'waist'), t = layers.torso, r = ramp(spec.tabard), half = side ? 1 : 2;
    const y0 = top ? wy - 5 : wy - 7, y1 = top ? wy + 5 : wy + 4;
    for (let y = y0; y <= y1; y++) for (let x = wx - half; x <= wx + half; x++) { const v = t.get(x, y); if (v && v !== dark) m.set(x, y, y === y1 && !side ? 2 : 1); }
    if (!back && !top) m.set(wx, wy - 3, 3);
    out.tabard = shade(m, { 1: r, 2: [r[0], r[0], r[1], r[1]], 3: ramp('gold') }, { outline: dark });
  }
  if (spec.bracers) {
    const m = mk(), r = ramp(spec.bracers);
    for (const [s0, h0] of [['shoulder_left', 'hand_off'], ['shoulder_right', 'hand_main']]) { const [sx, sy] = at(a, s0), [x, y] = at(a, h0), bx = Math.round(sx + (x - sx) * 0.7), by = Math.round(sy + (y - sy) * 0.7); m.rect(bx - 1, by - 1, 3, 2, 1); }
    out.bracers = shade(m, { 1: r });
  }
  if (spec.neck) {
    const m = mk(), [nx, ny] = at(a, 'neck'), kind = spec.neck, r = ramp(spec.neckRamp ?? (kind === 'amulet' ? 'gold' : kind === 'scarf' ? 'red' : 'leather'));
    if (top) { if (kind !== 'amulet') m.ellipse(nx + 0.5, ny + 0.5, 4, 4, 1); }
    else if (kind === 'amulet') { if (!back) { m.line(nx - 3, ny + 1, nx, ny + 4, 1).line(nx + 3, ny + 1, nx, ny + 4, 1); m.rect(nx - 1, ny + 4, 2, 2, 2); } }
    else if (kind === 'scarf') { m.rect(nx - 3, ny + 1, 7, 2, 1); if (!back) m.rect(nx + (side ? -2 * dir : 2), ny + 3, 2, 5, 1); }
    else m.rect(nx - 3, ny + 1, 7, 1, 1);
    out.neck = shade(m, kind === 'amulet' ? { 1: ramp('gold'), 2: ramp(spec.neckRamp ?? 'red') } : { 1: r }, { outline: kind === 'collar' ? null : dark });
  }
  if (spec.cloak) {
    // A cloak of its own, over the torso and under the head, so a pack or quiver in the back slot can be worn with it. From the front and the quarter it hangs in two panels
    // from the shoulders and leaves the chest clear; from the side it hangs behind; from behind it is one sheet; from above it is a round mantle round the shoulders.
    const m = mk(), [lx, ly] = at(a, 'shoulder_left'), [rx, ry] = at(a, 'shoulder_right'), [wx, wy] = at(a, 'waist'), r = ramp(spec.cloak), hem = wy + 9;
    const x0 = Math.min(lx, rx) - 1, x1 = Math.max(lx, rx) + 1, sy = Math.min(ly, ry);
    if (top) { m.ellipse((x0 + x1) / 2 + 0.5, wy - 2, (x1 - x0) / 2 + 3, 7, 1); m.line(wx, wy - 8, wx, wy + 4, 2); }
    else if (back) {
      m.poly([[x0, sy], [x1, sy], [x1 + 3, hem], [x0 - 3, hem]], 1);
      for (const fx of [wx - 3, wx, wx + 3]) m.line(fx, sy + 3, fx + (fx - wx), hem - 1, 2);
    } else if (side) {
      const bx = wx - 3 * dir;
      m.poly([[wx - 1 * dir, sy], [bx - 1 * dir, sy + 2], [bx - 5 * dir, hem], [wx + 1 * dir, hem]], 1);
      m.line(bx - 1 * dir, sy + 4, bx - 3 * dir, hem - 1, 2);
    } else {
      for (const [sx, d] of [[x0, -1], [x1, 1]]) {
        m.poly([[sx, sy], [sx - d * 3, sy + 1], [sx + d * 2, hem], [sx + d * 7, hem]], 1);
        m.line(sx + d, sy + 4, sx + d * 3, hem - 1, 2);
      }
      m.rect(x0, sy, x1 - x0 + 1, 2, 1);
    }
    out.cloak = shade(m, { 1: r, 2: [r[0], r[0], r[1], r[1]] }, { outline: dark });
  }
  if (spec.scar && !back && !top) {
    const m = mk(), fx = side ? hx + 2 * dir : quarter ? hx + 1 : hx, sx = fx + (spec.scar === 'cheek' ? 3 : spec.scar === 'brow' ? -2 : 0), sy = hy + (spec.scar === 'brow' ? -1 : spec.scar === 'cheek' ? 2 : 4);
    m.line(sx, sy, sx + 2 * (spec.scar === 'mouth' ? 0 : 1), sy + 3, 1); if (spec.scar === 'mouth') m.line(sx - 1, sy, sx + 1, sy, 1);
    const body = layers.body, out2 = mk();
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) if (m.get(x, y) && body.get(x, y) && skin.includes(body.get(x, y))) out2.set(x, y, ramp('red')[0]);
    out.scar = out2;
  }
  if (spec.jewellery && !top) {
    const m = mk();
    if (spec.jewellery === 'earring' && !back) { m.rect(side ? hx - 1 * dir : hx - 6, hy + 3, 1, 2, 1); if (!side) m.rect(hx + 6, hy + 3, 1, 2, 1); }
    else if (spec.jewellery === 'brooch') { const [nx, ny] = at(a, 'neck'); m.rect(nx + (side ? 0 : 3), ny + 3, 2, 2, 1); }
    out.jewellery = shade(m, { 1: ramp('gold') }, { outline: null });
  }
  if (spec.helmStyle && (spec.head === 'helm' || spec.head === 'horned')) {
    const m = mk(), st = spec.helmStyle, f = top ? ({ north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0] }[facing] ?? [0, 1]) : [0, 0];
    if (top) {
      if (st === 'visored') m.rect(hx + f[0] * 5 - (f[1] ? 5 : 0), hy + f[1] * 5 - (f[0] ? 5 : 0), f[1] ? 10 : 2, f[0] ? 10 : 2, 2);
      else if (st === 'crested') m.rect(hx - (f[0] ? 6 : 1), hy - (f[1] ? 6 : 1), f[0] ? 12 : 2, f[1] ? 12 : 2, 1);
      else if (st === 'plumed') m.thick(hx, hy, hx - f[0] * 8, hy - f[1] * 8, 1, 3);
      else { m.ellipse(hx - (f[1] ? 7 : 0), hy - (f[0] ? 7 : 0), 2, 2, 1); m.ellipse(hx + (f[1] ? 7 : 0), hy + (f[0] ? 7 : 0), 2, 2, 1); }
    } else if (st === 'visored') { if (!back) { const fx = side ? hx + 2 * dir : hx; m.rect(fx - 5, hy, 11, 5, 1); m.rect(fx - 4, hy + 1, 9, 1, 2); } }
    else if (st === 'crested') { if (side || back) m.rect(hx - 1, hy - 9, 3, 8, 1); else m.rect(hx - 1, hy - 10, 3, 9, 1); }
    else if (st === 'plumed') { m.ellipse(hx + 0.5 - (side ? 3 * dir : 0), hy - 9, 2.4, 3, 1); m.thick(hx - (side ? 3 * dir : 0), hy - 7, hx - (side ? 5 * dir : 3), hy - 4, 1, 3); }
    else { m.poly([[hx - 6, hy - 1], [hx - 10, hy - 6], [hx - 6, hy - 6]], 1); m.poly([[hx + 6, hy - 1], [hx + 10, hy - 6], [hx + 6, hy - 6]], 1); }
    out.helmstyle = shade(m, { 1: st === 'plumed' || st === 'crested' ? ramp('red') : ramp(spec.headRamp ?? 'steel'), 2: ramp('iron'), 3: ramp('red') });
  }
  return out;
}
