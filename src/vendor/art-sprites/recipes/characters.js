// Modular paper-doll characters (32x32). Every slot is its own layer, drawn separately and composed in an explicit order, so a
// different helmet, armour or weapon is a different layer and never a redrawn character. Weapons are the item shapes from
// items.js (one source for icon and worn art). Clips: idle, walk, attack (with an impact marker), hit, defeat.
//
// Four views (the hands/poses kit's turnaround): front, side (facing right; `facing: 'left'` mirrors it), back and quarter (three-quarter, facing right, between front and side). A view is its own
// set of layer drawings, so a turnaround keeps the same loadout, colours and proportions and only changes what that angle can see.
// `motion` draws clips of poses made elsewhere (Animator's walk, run, jump) with any loadout, view and pose, added to `clips` by id.
// Poses (`pose`) are stances built from a few joints (lean, squat, lift, arm, legs) rather than new art, so any loadout takes any pose.
import { Pixels } from '../pixels.js';
import { shade, dither } from '../shade.js';
import { C, ramp } from '../palette.js';
import { drawItem, SHAPES } from './items.js';
import { EXPRESSIONS } from './faces.js';
import { armorTexture, gearLayers, ARMOR_FAMILIES, BOOT_STYLES, HAIR_STYLES, FACIAL_HAIR, EYEWEAR, NECK_KINDS, SCARS, JEWELLERY, HELM_STYLES, EXTRA_AFTER } from './gear.js';
import { collapse, flash, clip } from '../anim.js';
import { motionClips, motionToClip } from '../motion.js';
import { topFrame, topAnchors, FACINGS, TOP, CX, CY } from './topdown.js';

const S = 32, W = 40; // the canvas is wider than the body so a swung weapon is not cut off; the feet stay at x 16, y 30
const BACK_W = 48, BACK_X = 8; // the back view mirrors the weapon side about the body, so it needs room on both sides
export const LAYER_ORDER = ['shadow', 'back', 'legs', 'torso', 'offhand', 'body', 'headgear', 'weapon'];
/** From behind, the back slot (cape, pack) covers the torso instead of sitting under it. */
export const layerOrder = (view, facing = 'south') => (view === 'top' ? (facing === 'north' ? ['shadow', 'legs', 'torso', 'back', 'offhand', 'body', 'headgear', 'weapon'] : LAYER_ORDER) : view === 'back' ? ['shadow', 'legs', 'torso', 'back', 'offhand', 'body', 'headgear', 'weapon'] : LAYER_ORDER);
export const HEADGEAR = ['hair', 'helm', 'horned', 'hood', 'hat', 'circlet'];
export const TORSO = ['tunic', 'plate', 'robe'];
export const VIEWS = ['front', 'side', 'back', 'quarter'];
/** `top` is the world view (T23): straight down, four `facing`s. It is not in `VIEWS`, which are the turnaround views of the paper doll. */
export const TOP_VIEW = 'top';
export { FACINGS, ARMOR_FAMILIES, BOOT_STYLES, HAIR_STYLES, FACIAL_HAIR, EYEWEAR, NECK_KINDS, SCARS, JEWELLERY, HELM_STYLES };
export const BACK_SLOTS = ['cape', 'quiver', 'pack'];
/** Body proportions, drawn within the same 32 by 32 canvas (decided 2026-10-30: the canvas does not grow). `medium` is the full-height body; the others scale each layer about the feet. */
export const BODY_PROFILES = { medium: [1, 1], small: [0.8, 0.8], slim: [0.85, 1], broad: [1.15, 1] };

/** Class-flavoured loadouts: the five Idle Solo / Tag Team archetypes, as data. */
export const LOADOUTS = {
  guardian: { skin: 'skin-light', torso: 'plate', cloth: 'steel', legs: 'iron', head: 'helm', headRamp: 'steel', weapon: 'sword', offhand: 'shield' },
  berserker: { skin: 'skin-medium', torso: 'tunic', cloth: 'red', legs: 'leather', head: 'horned', headRamp: 'iron', weapon: 'axe', offhand: null },
  rogue: { skin: 'skin-fair', torso: 'tunic', cloth: 'green', legs: 'leather', head: 'hood', headRamp: 'green', weapon: 'dagger', offhand: null },
  scholar: { skin: 'skin-deep', torso: 'robe', cloth: 'purple', legs: 'purple', head: 'hat', headRamp: 'purple', weapon: 'staff', offhand: null },
  healer: { skin: 'skin-light', torso: 'robe', cloth: 'white', legs: 'white', head: 'circlet', headRamp: 'gold', weapon: 'mace', offhand: null, hair: 'blond' },
  // CoffeeMUD classes: bard, druid, paladin, ranger, monk, necromancer.
  bard: { skin: 'skin-medium', torso: 'tunic', cloth: 'red', legs: 'leather', head: 'hat', headRamp: 'blue', weapon: 'rapier', offhand: null, hair: 'red' },
  druid: { skin: 'skin-deep', torso: 'robe', cloth: 'green', legs: 'wood', head: 'hood', headRamp: 'green', weapon: 'staff', offhand: null },
  paladin: { skin: 'skin-fair', torso: 'plate', cloth: 'gold', legs: 'steel', head: 'helm', headRamp: 'gold', weapon: 'mace', offhand: 'shield', hair: 'blond' },
  ranger: { skin: 'skin-medium', torso: 'tunic', cloth: 'green', legs: 'wood', head: 'hood', headRamp: 'wood', weapon: 'bow', offhand: null, hair: 'dark' },
  monk: { skin: 'skin-light', torso: 'tunic', cloth: 'sand', legs: 'sand', head: 'hair', headRamp: 'wood', weapon: null, offhand: null, hair: 'dark' },
  necromancer: { skin: 'skin-fair', torso: 'robe', cloth: 'stone', legs: 'stone', head: 'hood', headRamp: 'stone', weapon: 'staff', offhand: null },
  // CoffeeMUD remaining classes: shaman, archon, pirate, sailor, jester, the mage schools, and the trades that keep a shop (the weapon slot holds the tool of the trade).
  shaman: { skin: 'skin-deep', torso: 'robe', cloth: 'leather', legs: 'wood', head: 'horned', headRamp: 'stone', weapon: 'staff', offhand: null },
  archon: { skin: 'skin-light', torso: 'plate', cloth: 'white', legs: 'gold', head: 'circlet', headRamp: 'gold', weapon: 'greatsword', offhand: null, hair: 'blond' },
  pirate: { skin: 'skin-medium', torso: 'tunic', cloth: 'red', legs: 'iron', head: 'hat', headRamp: 'iron', weapon: 'scimitar', offhand: null, hair: 'dark' },
  sailor: { skin: 'skin-fair', torso: 'tunic', cloth: 'blue', legs: 'white', head: 'hat', headRamp: 'white', weapon: 'dagger', offhand: null, hair: 'brown' },
  jester: { skin: 'skin-light', torso: 'tunic', cloth: 'purple', legs: 'red', head: 'hat', headRamp: 'red', weapon: 'wand', offhand: null, hair: 'red' },
  evoker: { skin: 'skin-medium', torso: 'robe', cloth: 'red', legs: 'red', head: 'hat', headRamp: 'red', weapon: 'staff', offhand: null },
  illusionist: { skin: 'skin-fair', torso: 'robe', cloth: 'blue', legs: 'blue', head: 'hat', headRamp: 'blue', weapon: 'wand', offhand: null },
  conjurer: { skin: 'skin-deep', torso: 'robe', cloth: 'gold', legs: 'gold', head: 'hat', headRamp: 'gold', weapon: 'staff', offhand: null },
  abjurer: { skin: 'skin-light', torso: 'robe', cloth: 'steel', legs: 'steel', head: 'hood', headRamp: 'steel', weapon: 'staff', offhand: 'buckler' },
  diviner: { skin: 'skin-fair', torso: 'robe', cloth: 'glass', legs: 'glass', head: 'circlet', headRamp: 'glass', weapon: 'book', offhand: null },
  enchanter: { skin: 'skin-medium', torso: 'robe', cloth: 'purple', legs: 'blue', head: 'circlet', headRamp: 'purple', weapon: 'wand', offhand: null, hair: 'blond' },
  shopkeeper: { skin: 'skin-medium', torso: 'tunic', cloth: 'bone', legs: 'leather', head: 'hair', headRamp: 'wood', weapon: 'scale', offhand: null, hair: 'brown' },
  banker: { skin: 'skin-fair', torso: 'robe', cloth: 'green', legs: 'green', head: 'hat', headRamp: 'green', weapon: 'ledger', offhand: null },
  librarian: { skin: 'skin-light', torso: 'robe', cloth: 'sand', legs: 'sand', head: 'hair', headRamp: 'iron', weapon: 'book', offhand: null, hair: 'dark' },
  smith: { skin: 'skin-deep', torso: 'tunic', cloth: 'leather', legs: 'iron', head: 'hair', headRamp: 'wood', weapon: 'hammer', offhand: null, hair: 'dark' },
};
const HAIR = { brown: 'wood', blond: 'sand', dark: 'iron', red: 'red' };

/**
 * Stances. Each is a handful of joints, not a drawing: `lean` (head leans, torso half as far), `head` (head drops), `squat` (legs shorten,
 * the body sinks), `lift` (everything but the shadow rises: a jump or a fall), `legs` (x offset of each foot), `arm` and `weapon`.
 * The kit's pose targets: standing, walking, running, sitting, crouching, reaching, carrying, casting, falling, and the postures
 * (confident, injured, sneaky, exhausted, regal).
 */
export const POSES = {
  stand: {}, run: { legs: [-3, 3], lean: 1, lift: 1, arm: 'forward' }, sit: { squat: 4, legs: [-3, 3] }, crouch: { squat: 4, lean: 1, legs: [-1, 1], arm: 'guard' },
  reach: { arm: 'out', lean: 1, weapon: 'none' }, carry: { arm: 'carry', weapon: 'none' }, cast: { arm: 'up', lean: -1, weapon: 'down' }, fall: { lift: 1, legs: [-3, 3], lean: -2, arm: 'up', weapon: 'none' },
  brace: { squat: 1, arm: 'guard' }, confident: { arm: 'down', lift: 0, lean: -1 }, regal: { lean: 0, lift: 1 }, injured: { dy: 1, lean: -1, head: 1, legs: [-1, 2], arm: 'carry', weapon: 'down' },
  point: { arm: 'toward', weapon: 'none', lean: 1 }, kick: { kick: 1, lean: -1, arm: 'guard', lift: 0 },
  sneak: { squat: 3, lean: 2, arm: 'forward' }, exhausted: { dy: 1, head: 2, lean: 1, arm: 'carry', weapon: 'down' },
};
export const POSE_IDS = Object.keys(POSES);

const layer = (w = W) => new Pixels(w, S);
const done = (m, parts, outline = C.night) => shade(m, parts, { outline });
/** A part drawn one step darker than a ramp, so a far limb or a crease reads against a near one. */
const deep = (r) => [r[0], r[0], r[1], r[1]];
const HANDS = { raised: [23, 12], forward: [24, 18], down: [22, 21], out: [27, 16], up: [22, 7], carry: [20, 19], guard: [21, 17], toward: [23, 17] };
const REACHING = ['out', 'up', 'carry', 'guard', 'toward'];
const handPos = (p, view = 'front') => { const [x, y] = HANDS[p.arm] ?? HANDS.down; return view === 'side' ? [x - 3, y] : view === 'quarter' ? [x - 1, y] : [x, y]; };
const tl = (p) => Math.trunc(p.lean / 2); // the torso follows the head, half as far
const hairOf = (s) => ramp(HAIR[s.hair ?? 'brown']);

/** The front view (the original paper doll). */
const FRONT = {
  shadow() { return dither(layer().ellipse(16, 29.5, 9, 2, 1), C.slate).swap(1, C.slate); },
  back(s) {
    // A cape or pack shows only at the edges from the front: the cloak either side of the shoulders, a quiver's feathers, pack straps.
    const m = layer(), kind = s.back; if (!kind) return m;
    if (kind === 'cape') { m.rect(8, 15, 2, 12, 1).rect(22, 15, 2, 12, 1); return done(m, { 1: ramp(s.backRamp ?? 'red') }); }
    if (kind === 'quiver') { m.rect(21, 10, 1, 3, 2).rect(23, 11, 1, 3, 2); return done(m, { 2: ramp('white') }); }
    m.rect(10, 14, 1, 7, 1).rect(21, 14, 1, 7, 1); return done(m, { 1: ramp('leather') });
  },
  legs(s, p) {
    const m = layer(), [lo, ro] = p.legs;
    for (const [x, off, near] of [[12, lo, false], [17, ro, p.kick]]) {
      // Foreshortened: a leg that comes at the viewer loses its length and its boot grows, higher on the canvas than the standing foot.
      if (near) { m.rect(x + off, 24, 3, 2, 1); m.rect(x + off - 1, 25, 5, 4, 2); } else { m.rect(x + off, 24 + p.squat, 3, 6 - p.squat, 1); m.rect(x + off, 28, 3, 2, 2); }
    }
    return done(m, { 1: ramp(s.legs), 2: ramp(s.boots ?? 'leather') });
  },
  torso(s, p) {
    const m = layer();
    if (s.torso === 'plate') { m.poly([[9, 15], [12, 14], [20, 14], [23, 15], [22, 24], [10, 24]], 1); m.rect(15, 15, 2, 9, 2); m.rect(10, 21, 12, 1, 2); }
    else if (s.torso === 'robe') { m.poly([[11, 14], [21, 14], [24, 29], [8, 29]], 1); m.rect(15, 15, 2, 14, 2); m.rect(11, 21, 10, 1, 2); }
    else { m.ellipse(16, 19.5, 5.5, 5.5, 1); m.rect(11, 22, 10, 1, 2); }
    m.thick(10, 16, 9, 21, 1, 1); m.thick(22, 16, 23, 21, 1, 1);
    if (p.arm === 'toward') m.rect(21, 15, 4, 4, 2); // the arm comes at the viewer: a short stump, its length lost
    else if (REACHING.includes(p.arm)) { const [hx, hy] = handPos(p); m.thick(22, 16, hx, hy, 1, 2); } // the new stances show the arm that reaches
    return done(m, { 1: ramp(s.cloth), 2: s.torso === 'plate' ? ramp('iron') : ramp('leather') }).shift(tl(p), p.dy);
  },
  offhand(s, p) {
    if (s.offhand !== 'shield') return layer();
    const m = layer(); m.poly([[2, 15], [10, 15], [10, 22], [6, 27], [2, 22]], 1); m.rect(5, 17, 2, 6, 3);
    return done(m, { 1: ramp('blue'), 3: ramp('gold') }).shift(0, p.dy);
  },
  body(s, p) {
    const m = layer(); m.ellipse(16 + p.lean, 9 + p.head, 6, 5.5, 1);
    const out = shade(m, { 1: ramp(s.skin) }, { outline: null }), skin = ramp(s.skin)[2], x = p.lean, y = p.head;
    out.rect(13 + x, 9 + y, 1, 2, C.night).rect(18 + x, 9 + y, 1, 2, C.night).set(14 + x, 9 + y, C.white).set(17 + x, 9 + y, C.white);
    const [hx, hy] = handPos(p);
    if (p.arm === 'toward') out.ellipse(hx + 1, hy + 2, 3.5, 3.5, skin).rect(hx - 2, hy - 2, 1, 2, skin).rect(hx, hy - 3, 1, 2, skin).rect(hx + 2, hy - 3, 1, 2, skin).rect(hx + 4, hy - 2, 1, 2, skin); // a big hand, fingers fanned, over the shoulder
    else out.rect(hx, hy, 2, 2, skin);
    out.rect(9 + tl(p), 21, 2, 2, skin);
    return out.outline(C.night).shift(0, p.dy);
  },
  headgear(s, p) {
    const m = layer(), r = ramp(s.headRamp ?? 'steel');
    if (s.head === 'helm') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(9, 9, 14, 6, 0); m.rect(15, 9, 2, 5, 2); m.rect(10, 7, 12, 1, 2); m.rect(15, 2, 2, 2, 3); }
    else if (s.head === 'horned') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(9, 9, 14, 6, 0); m.rect(10, 7, 12, 1, 2); m.line(9, 8, 5, 3, 3).line(10, 8, 6, 3, 3).line(23, 8, 27, 3, 3).line(22, 8, 26, 3, 3); }
    else if (s.head === 'hood') { m.ellipse(16, 8.5, 7, 6.5, 1); m.rect(11, 8, 10, 6, 0); m.rect(8, 11, 16, 5, 1); m.rect(12, 9, 8, 6, 0); }
    else if (s.head === 'hat') { m.ellipse(16, 6.5, 8.5, 1.8, 1); m.poly([[11, 6], [21, 6], [18, 2], [16, 1]], 1); m.rect(11, 5, 10, 1, 2); }
    else if (s.head === 'circlet') { m.rect(10, 5, 12, 1, 1); m.set(16, 4, 3); }
    else { m.ellipse(16, 7.5, 6.5, 4.5, 1); m.rect(9, 9, 14, 6, 0); m.rect(12, 8, 8, 2, 0); }
    const parts = s.head === 'hair' ? { 1: ramp(HAIR[s.hair ?? 'brown']) } : { 1: r, 2: ramp(s.head === 'hat' || s.head === 'circlet' ? 'gold' : 'iron'), 3: s.head === 'horned' ? ramp('bone') : ramp('red') };
    return done(m, parts, s.head === 'circlet' ? null : C.night).shift(p.lean, p.dy + p.head);
  },
  weapon(s, p) { return holding(s, p, 'front'); },
};

/** The weapon in the hand, upright or turned down, as drawn on every view (the item shapes are side-on already). */
function holding(s, p, view) {
  const m = layer(); if (!s.weapon || p.weapon === 'none') return m;
  const item = drawItem({ shape: s.weapon, tier: s.tier ?? 'crafted', seed: 'worn' });
  let art = item.frames[0], [gx, gy] = item.anchors.grip;
  if (p.weapon === 'down') { art = art.rot90(); [gx, gy] = [15 - gy, gx]; }
  const [hx, hy] = handPos(p, view);
  return m.blit(art, hx + 1 - gx, hy + 1 - gy).shift(0, p.dy);
}

/** The side view, facing right: a profile head, a narrower torso, two legs that overlap, the near arm forward. */
const SIDE = {
  ...FRONT,
  back(s, p) {
    const m = layer(), kind = s.back; if (!kind) return m;
    if (kind === 'cape') { m.poly([[14, 15], [16, 15], [15, 27], [6 - p.drag, 28], [8 - p.drag, 22]], 1); m.line(14, 17, 10 - p.drag, 26, 2); return done(m, { 1: ramp(s.backRamp ?? 'red'), 2: deep(ramp(s.backRamp ?? 'red')) }).shift(tl(p), p.dy); }
    if (kind === 'quiver') { m.thick(12, 14, 10, 24, 1, 3).set(12, 12, 2).set(13, 11, 2).set(11, 12, 2); return done(m, { 1: ramp('leather'), 2: ramp('white') }).shift(tl(p), p.dy); }
    m.rect(9, 15, 5, 9, 1).rect(10, 17, 3, 3, 2); return done(m, { 1: ramp('leather'), 2: ramp('wood') }).shift(tl(p), p.dy);
  },
  legs(s, p) {
    const m = layer(), [lo, ro] = p.legs, top = 24 + p.squat, h = 6 - p.squat;
    m.rect(13 + Math.round(lo * 1.5), top, 3, h, 3).rect(13 + Math.round(lo * 1.5), 28, 4, 2, 4);
    m.rect(17 + Math.round(ro * 1.5), top, 3, h, 1).rect(17 + Math.round(ro * 1.5), 28, 4, 2, 2);
    const far = deep(ramp(s.legs)), boots = ramp(s.boots ?? 'leather');
    return done(m, { 1: ramp(s.legs), 2: boots, 3: far, 4: deep(boots) });
  },
  torso(s, p) {
    const m = layer(), [hx, hy] = handPos(p, 'side');
    if (s.torso === 'plate') { m.poly([[12, 14], [20, 14], [21, 24], [11, 24]], 1); m.rect(11, 21, 10, 1, 2); }
    else if (s.torso === 'robe') { m.poly([[12, 14], [20, 14], [23, 29], [9, 29]], 1); m.rect(11, 21, 10, 1, 2); }
    else { m.ellipse(16, 19.5, 4.5, 5.5, 1); m.rect(12, 22, 8, 1, 2); }
    m.thick(16, 15, Math.max(16, hx - 1), hy, 1, 2);
    return done(m, { 1: ramp(s.cloth), 2: s.torso === 'plate' ? ramp('iron') : ramp('leather') }).shift(tl(p), p.dy);
  },
  offhand(s, p) {
    if (s.offhand !== 'shield') return layer();
    const m = layer(); m.poly([[14, 16], [18, 16], [18, 23], [16, 26], [14, 23]], 1); m.rect(16, 18, 1, 4, 3);
    return done(m, { 1: ramp('blue'), 3: ramp('gold') }).shift(tl(p), p.dy);
  },
  body(s, p) {
    const m = layer(), x = p.lean, y = p.head; m.ellipse(16 + x, 9 + y, 5, 5.5, 1).rect(15 + x, 13 + y, 3, 2, 1).set(22 + x, 10 + y, 1);
    const out = shade(m, { 1: ramp(s.skin) }, { outline: null }), skin = ramp(s.skin);
    out.rect(19 + x, 9 + y, 1, 2, C.night).set(15 + x, 10 + y, skin[1]);
    const [hx, hy] = handPos(p, 'side'); out.rect(hx, hy, 2, 2, skin[2]);
    return out.outline(C.night).shift(0, p.dy);
  },
  headgear(s, p) {
    const m = layer(), r = ramp(s.headRamp ?? 'steel');
    if (s.head === 'helm') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(9, 9, 14, 6, 0); m.rect(10, 7, 12, 1, 2); m.rect(20, 9, 2, 4, 2); m.rect(11, 2, 5, 2, 3); }
    else if (s.head === 'horned') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(9, 9, 14, 6, 0); m.rect(10, 7, 12, 1, 2); m.line(14, 7, 10, 3, 3).line(15, 7, 11, 3, 3).line(11, 2, 9, 2, 3); }
    else if (s.head === 'hood') { m.ellipse(16, 8.5, 7, 6.5, 1); m.rect(19, 8, 6, 7, 0); m.rect(9, 11, 8, 6, 1); }
    else if (s.head === 'hat') { m.ellipse(16, 6.5, 9, 1.8, 1); m.poly([[12, 6], [20, 6], [19, 2], [16, 1], [13, 3]], 1); m.rect(12, 5, 8, 1, 2); }
    else if (s.head === 'circlet') { m.rect(11, 5, 10, 1, 1); m.set(20, 4, 3); }
    else { m.ellipse(16, 7.5, 6, 4.5, 1); m.rect(9, 9, 14, 6, 0); m.rect(10, 9, 4, 5, 1); }
    const parts = s.head === 'hair' ? { 1: hairOf(s) } : { 1: r, 2: ramp(s.head === 'hat' || s.head === 'circlet' ? 'gold' : 'iron'), 3: s.head === 'horned' ? ramp('bone') : ramp('red') };
    return done(m, parts, s.head === 'circlet' ? null : C.night).shift(p.lean, p.dy + p.head);
  },
  weapon(s, p) { return holding(s, p, 'side'); },
};

/**
 * The back view. Drawn on the front canvas, then placed on the wider one with the weapon side, the shield and its hand mirrored about
 * the body (the weapon hand is on the other side when you walk round to the back). The head is hair or headgear only: no face.
 */
const BACK = {
  ...FRONT,
  back(s, p) {
    const m = layer(), kind = s.back, r = ramp(s.backRamp ?? 'red'); if (!kind) return m;
    if (kind === 'cape') { const sw = p.sway; m.poly([[10, 14], [22, 14], [24 + sw, 28], [8 + sw, 28]], 1).rect(15, 16, 1, 12, 2).rect(11, 16, 1, 10, 2).rect(20, 16, 1, 10, 2); return done(m, { 1: r, 2: deep(r) }).shift(tl(p), p.dy); }
    if (kind === 'quiver') { m.thick(10, 13, 21, 22, 2, 1).rect(17, 11, 4, 13, 1).set(18, 9, 3).set(20, 9, 3).set(19, 10, 3); return done(m, { 1: ramp('leather'), 2: ramp('wood'), 3: ramp('white') }).shift(tl(p), p.dy); }
    m.rect(10, 15, 12, 11, 1).rect(12, 20, 8, 5, 2).rect(10, 15, 12, 1, 3); return done(m, { 1: ramp('leather'), 2: ramp('wood'), 3: ramp('wood') }).shift(tl(p), p.dy);
  },
  torso(s, p) {
    const m = layer();
    if (s.torso === 'plate') { m.poly([[9, 15], [12, 14], [20, 14], [23, 15], [22, 24], [10, 24]], 1); m.rect(15, 15, 2, 9, 2); m.rect(10, 21, 12, 1, 2); m.rect(10, 17, 4, 1, 2).rect(18, 17, 4, 1, 2); }
    else if (s.torso === 'robe') { m.poly([[11, 14], [21, 14], [24, 29], [8, 29]], 1); m.rect(15, 15, 2, 14, 2); m.rect(11, 21, 10, 1, 2); }
    else { m.ellipse(16, 19.5, 5.5, 5.5, 1); m.rect(11, 22, 10, 1, 2); m.rect(15, 15, 2, 7, 2); }
    m.thick(10, 16, 9, 21, 1, 1); m.thick(22, 16, 23, 21, 1, 1);
    return done(m, { 1: ramp(s.cloth), 2: s.torso === 'plate' ? ramp('iron') : ramp('leather') }).shift(tl(p), p.dy);
  },
  offhand(s, p) {
    const m = layer(), skin = ramp(s.skin)[2];
    if (s.offhand === 'shield') { m.poly([[2, 15], [10, 15], [10, 22], [6, 27], [2, 22]], 1); m.rect(4, 18, 3, 1, 3).rect(4, 22, 3, 1, 3); }
    else m.rect(9 + tl(p), 21, 2, 2, 1);
    return done(m, { 1: s.offhand === 'shield' ? ramp('wood') : [skin, skin, skin, skin], 3: ramp('leather') }).shift(0, p.dy);
  },
  body(s, p) {
    const m = layer(), x = p.lean, y = p.head; m.ellipse(16 + x, 9 + y, 6, 5.5, 1).rect(14 + x, 13 + y, 4, 2, 2);
    // From behind the head is hair, or whatever covers it; the neck is the only skin.
    const hair = ['hair', 'hat', 'circlet'].includes(s.head) ? hairOf(s) : ramp(s.headRamp ?? 'steel');
    return done(m, { 1: hair, 2: ramp(s.skin) }).shift(0, p.dy);
  },
  headgear(s, p) {
    const m = layer(), r = ramp(s.headRamp ?? 'steel');
    if (s.head === 'helm') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(10, 12, 12, 1, 2); m.rect(15, 2, 2, 8, 3); }
    else if (s.head === 'horned') { m.ellipse(16, 8.5, 6.5, 6, 1); m.rect(10, 12, 12, 1, 2); m.line(9, 8, 5, 3, 3).line(10, 8, 6, 3, 3).line(23, 8, 27, 3, 3).line(22, 8, 26, 3, 3); }
    else if (s.head === 'hood') { m.ellipse(16, 8.5, 7, 6.5, 1); m.rect(8, 11, 16, 5, 1); m.rect(15, 5, 2, 10, 2); }
    else if (s.head === 'hat') { m.ellipse(16, 6.5, 8.5, 1.8, 1); m.poly([[11, 6], [21, 6], [18, 2], [16, 1]], 1); m.rect(11, 5, 10, 1, 2); }
    else if (s.head === 'circlet') { m.rect(10, 5, 12, 1, 1); }
    else return layer();
    const parts = { 1: r, 2: ramp(s.head === 'hat' || s.head === 'circlet' ? 'gold' : 'iron'), 3: s.head === 'horned' ? ramp('bone') : ramp('red') };
    return done(m, parts, s.head === 'circlet' ? null : C.night).shift(p.lean, p.dy + p.head);
  },
  // The weapon hand goes with the weapon so both mirror together.
  weapon(s, p) {
    const m = holding(s, p, 'front'), [hx, hy] = handPos(p); const skin = ramp(s.skin)[2];
    return new Pixels(W, S).rect(hx, hy + p.dy, 2, 2, skin).outline(C.night).blit(m);
  },
};

/**
 * The three-quarter view, facing right: the front turned half way to the side. The head and torso narrow and sit a pixel right, the eyes
 * crowd to the facing edge with a nose past it, the legs close up, the near arm leads and the far arm and shield drop behind the body.
 */
const QUARTER = {
  ...FRONT,
  back: (s, p) => SIDE.back(s, p).shift(1, 0),
  legs(s, p) {
    const m = layer(), [lo, ro] = p.legs;
    for (const [x, off, a, b] of [[13, lo, 3, 4], [17, ro, 1, 2]]) { m.rect(x + off, 24 + p.squat, 3, 6 - p.squat, a); m.rect(x + off, 28, 3, 2, b); }
    const boots = ramp(s.boots ?? 'leather');
    return done(m, { 1: ramp(s.legs), 2: boots, 3: deep(ramp(s.legs)), 4: deep(boots) });
  },
  torso(s, p) {
    const m = layer(), [hx, hy] = handPos(p, 'quarter');
    if (s.torso === 'plate') { m.poly([[11, 15], [13, 14], [21, 14], [23, 15], [22, 24], [11, 24]], 1); m.rect(17, 15, 2, 9, 2); m.rect(11, 21, 11, 1, 2); }
    else if (s.torso === 'robe') { m.poly([[12, 14], [21, 14], [24, 29], [9, 29]], 1); m.rect(17, 15, 2, 14, 2); m.rect(12, 21, 10, 1, 2); }
    else { m.ellipse(17, 19.5, 4.5, 5.5, 1); m.rect(12, 22, 9, 1, 2); }
    m.thick(12, 16, 11, 21, 1, 3); // the far arm hangs behind the chest
    m.thick(22, 16, Math.max(22, hx), hy, 1, 2);
    return done(m, { 1: ramp(s.cloth), 2: s.torso === 'plate' ? ramp('iron') : ramp('leather'), 3: deep(ramp(s.cloth)) }).shift(tl(p), p.dy);
  },
  offhand(s, p) {
    if (s.offhand !== 'shield') return layer();
    const m = layer(); m.poly([[6, 15], [12, 15], [12, 22], [9, 26], [6, 22]], 1); m.rect(8, 17, 1, 5, 3);
    return done(m, { 1: ramp('blue'), 3: ramp('gold') }).shift(tl(p), p.dy);
  },
  body(s, p) {
    const m = layer(), x = p.lean, y = p.head; m.ellipse(17 + x, 9 + y, 5.5, 5.5, 1).rect(16 + x, 13 + y, 3, 2, 1).set(23 + x, 10 + y, 1);
    const out = shade(m, { 1: ramp(s.skin) }, { outline: null }), skin = ramp(s.skin);
    out.rect(16 + x, 9 + y, 1, 2, C.night).rect(20 + x, 9 + y, 1, 2, C.night).set(15 + x, 9 + y, C.white).set(19 + x, 9 + y, C.white).set(16 + x, 12 + y, skin[0]);
    const [hx, hy] = handPos(p, 'quarter'); out.rect(hx, hy, 2, 2, skin[2]).rect(10 + tl(p), 21, 2, 2, skin[1]);
    return out.outline(C.night).shift(0, p.dy);
  },
  headgear: (s, p) => FRONT.headgear(s, p).shift(1, 0),
  weapon(s, p) { return holding(s, p, 'quarter'); },
};

/** The core semantic clips: what the game asks a character to do besides walk and strike. Each is joints only, so any loadout, view and body profile takes it. */
export const SEMANTIC = {
  turn: { fps: 8, at: 1, event: 'faced', poses: [{ lean: -1, head: 0 }, { lean: 1, legs: [-1, 1] }, { lean: 0 }] },
  block: { fps: 8, at: 1, event: 'blocked', poses: [{ arm: 'guard', squat: 1 }, { arm: 'guard', squat: 1, dx: -1 }, { arm: 'guard', squat: 1 }] },
  cast: { fps: 6, at: 2, event: 'release', poses: [{ arm: 'carry', weapon: 'down' }, { arm: 'up', lean: -1, weapon: 'down' }, { arm: 'up', lean: -1, lift: 1, weapon: 'down' }, { arm: 'up', lean: -1, weapon: 'down' }] },
  heal: { fps: 6, at: 2, event: 'pulse', poses: [{ arm: 'carry', weapon: 'none' }, { arm: 'out', lean: 1, weapon: 'none' }, { arm: 'out', lean: 1, lift: 1, weapon: 'none' }, { arm: 'out', lean: 1, weapon: 'none' }] },
  celebrate: { fps: 6, at: 1, event: 'cheer', poses: [{ arm: 'up', lift: 1, weapon: 'none' }, { arm: 'up', weapon: 'none' }, { arm: 'up', lift: 1, weapon: 'none' }, { arm: 'up', weapon: 'none' }] },
  // The expanded list (T8, phase 4). `flashAt` is a frame that is also drawn as a coloured flash (a level up, a buff); `cycle` repeats its poses in a loop.
  dodge: { fps: 10, at: 1, event: 'evaded', poses: [{ dx: 0, lean: 1 }, { dx: -4, lean: -2, squat: 1 }, { dx: -5, lean: -2, squat: 1 }, { dx: -2, lean: -1 }, { dx: 0 }] },
  parry: { fps: 10, at: 1, event: 'parried', poses: [{ arm: 'raised', weapon: 'up' }, { arm: 'guard', weapon: 'up', dx: 1 }, { arm: 'guard', weapon: 'up', dx: 2 }, { arm: 'raised', weapon: 'up' }] },
  charge: { fps: 12, at: 2, event: 'impact', poses: [{ arm: 'raised', lean: -1, squat: 1 }, { dx: 2, arm: 'forward', lean: 2, legs: [-3, 3], lift: 1 }, { dx: 5, arm: 'forward', lean: 3, legs: [3, -3], lift: 1 }, { dx: 5, arm: 'forward', lean: 2, legs: [-3, 3], lift: 0 }] },
  stun: { fps: 6, at: 0, event: 'stunned', loop: true, poses: [{ lean: -1, head: 1, arm: 'carry', weapon: 'down' }, { lean: 1, head: 2, arm: 'carry', weapon: 'down' }] },
  potion: { fps: 6, at: 2, event: 'drink', poses: [{ arm: 'carry', weapon: 'none' }, { arm: 'up', weapon: 'none', lean: -1 }, { arm: 'up', weapon: 'none', lean: -2, head: 0 }, { arm: 'carry', weapon: 'none' }] },
  buffed: { fps: 6, at: 1, event: 'pulse', loop: true, flashAt: 1, flash: 'sky', poses: [{ lift: 0 }, { lift: 1, arm: 'guard' }] },
  lowhealth: { fps: 3, at: 0, event: 'wheeze', loop: true, poses: [{ dy: 1, lean: -1, head: 1, arm: 'carry', weapon: 'down' }, { dy: 1, lean: -1, head: 2, arm: 'carry', weapon: 'down', legs: [-1, 1] }] },
  levelup: { fps: 6, at: 2, event: 'ding', flashAt: 2, flash: 'yellow', poses: [{ arm: 'down', squat: 1 }, { arm: 'up', lift: 1, weapon: 'up' }, { arm: 'up', lift: 2, weapon: 'up' }, { arm: 'up', lift: 1, weapon: 'up' }, { arm: 'down' }] },
  tagin: { fps: 10, at: 3, event: 'ready', poses: [{ dx: -6, legs: [-2, 2] }, { dx: -4, legs: [2, -2] }, { dx: -2, legs: [-2, 2] }, { dx: 0, legs: [0, 0] }] },
  tagout: { fps: 10, at: 3, event: 'gone', poses: [{ dx: 0, legs: [0, 0] }, { dx: 2, legs: [-2, 2] }, { dx: 4, legs: [2, -2] }, { dx: 6, legs: [-2, 2] }] },
  equip: { fps: 6, at: 2, event: 'equipped', poses: [{ arm: 'carry', weapon: 'none' }, { arm: 'out', weapon: 'none', lean: 1 }, { arm: 'forward', weapon: 'down' }, { arm: 'down', weapon: 'up' }] },
  // Weapon attacks (T8): `shoot` for a bow, sling or gun; `thrust` for a spear, rapier and the like; `slam` for a hammer or a great axe. `weaponAttack(shape)` says which a weapon takes.
  shoot: { fps: 8, at: 2, event: 'fire', poses: [{ arm: 'out', weapon: 'up' }, { arm: 'out', weapon: 'up', lean: -2, dx: -1 }, { arm: 'out', weapon: 'up', lean: -2, dx: -2 }, { arm: 'out', weapon: 'up', dx: -1 }] },
  thrust: { fps: 10, at: 2, event: 'impact', poses: [{ arm: 'guard', weapon: 'down', dx: -1 }, { arm: 'guard', weapon: 'down', dx: -2 }, { arm: 'forward', weapon: 'down', dx: 4 }, { arm: 'forward', weapon: 'down', dx: 1 }] },
  slam: { fps: 8, at: 2, event: 'impact', poses: [{ arm: 'up', weapon: 'up', lean: -1 }, { arm: 'up', weapon: 'up', lean: -2, lift: 1 }, { arm: 'forward', weapon: 'down', lean: 2, squat: 1, dx: 2 }, { arm: 'down', weapon: 'up' }] },
  recall: { fps: 6, at: 3, event: 'vanish', poses: [{ arm: 'up', weapon: 'down' }, { arm: 'up', lift: 1, weapon: 'down' }, { arm: 'up', lift: 2, weapon: 'down' }, { arm: 'up', lift: 3, weapon: 'down' }] },
};
export const SEMANTIC_IDS = Object.keys(SEMANTIC);
/** The attack clip a weapon takes: `shoot`, `thrust`, `slam`, `cast` (a staff or wand), else the plain `attack`. */
const WEAPON_ATTACKS = { bow: 'shoot', sling: 'shoot', arquebus: 'shoot', cannon: 'shoot', crossbow: 'shoot', spear: 'thrust', javelin: 'thrust', trident: 'thrust', halberd: 'thrust', rapier: 'thrust', hammer: 'slam', axe2: 'slam', axegreat: 'slam', mace: 'slam', greatsword: 'slam', staff: 'cast', wand: 'cast' };
export const weaponAttack = (shape) => WEAPON_ATTACKS[shape] ?? 'attack';

const VIEW_DRAW = { front: FRONT, side: SIDE, back: BACK, quarter: QUARTER };
const MIRRORED = ['offhand', 'weapon']; // the back view mirrors these about the body
const pose = (o = {}) => ({ dx: 0, dy: 0, lean: 0, head: 0, squat: 0, lift: 0, drag: 0, sway: 0, legs: [0, 0], arm: 'down', weapon: 'up', ...o });

/** Nearest-neighbour scale of a layer about (cx, cy), kept on the pixel grid. */
function scaleAbout(px, sx, sy, cx, cy) {
  return px.map((x, y) => px.get(Math.floor((x - cx) / sx + cx), Math.floor((y - cy) / sy + cy)));
}

/**
 * Named anchors for one pose (Tag Team's compositor names: root, head, neck, shoulders, hands, waist, back, feet, weapon_grip,
 * shield_grip, effect_origin, shadow_center), in the pixel coordinates of the canvas the view draws on. They follow the same joints the
 * layers are drawn from, so a lean, a squat or a swung arm moves them. `effect_origin` is where a swing or a cast starts (the weapon hand,
 * lifted when the weapon is held up). Always inside the canvas.
 */
function anchorsOf(spec, p) {
  const view = spec.view, [lo, ro] = p.legs, t = tl(p), d = p.dy, w = view === 'back' ? BACK_W : W;
  const [hx, hy] = handPos(p, view);
  const head = [(view === 'quarter' ? 17 : 16) + p.lean, 9 + p.head + d];
  const shoulders = view === 'side' ? [[16 + t, 16 + d], [16 + t, 16 + d]] : view === 'quarter' ? [[12 + t, 16 + d], [22 + t, 16 + d]] : [[10 + t, 16 + d], [22 + t, 16 + d]];
  const handOff = view === 'side' ? [14 + t, 22 + d] : view === 'quarter' ? [11 + t, 22 + d] : [10 + t, 22 + d];
  const shield = view === 'side' ? [16 + t, 20 + d] : view === 'quarter' ? [9 + t, 20 + d] : [6, 19 + d];
  const foot = view === 'quarter' ? [14 + lo, 18 + ro] : view === 'side' ? [14 + Math.round(lo * 1.5), 18 + Math.round(ro * 1.5)] : [13 + lo, 18 + ro];
  const mirrored = new Set(['hand_main', 'hand_off', 'weapon_grip', 'shield_grip', 'effect_origin']);
  const raw = {
    head, neck: [head[0], head[1] + 5], shoulder_left: shoulders[0], shoulder_right: shoulders[1], waist: [16 + t, 22 + d], back: [16 + t, 18 + d],
    hand_main: [hx, hy + d], hand_off: handOff, weapon_grip: [hx, hy + d], shield_grip: shield,
    effect_origin: [hx + (p.weapon === 'up' && spec.weapon ? 3 : 0), hy + d - (p.weapon === 'up' && spec.weapon ? 8 : 0)],
    foot_left: [foot[0], 29], foot_right: [foot[1], 29],
  };
  const out = {};
  for (const [name, [x0, y0]] of Object.entries(raw)) {
    let x = x0 + p.dx, y = y0 - p.lift;
    if (view === 'back') { x += BACK_X; if (mirrored.has(name)) x = BACK_W - 1 - x; } else if (view === 'side' && spec.facing === 'left') x = W - 1 - x;
    out[name] = [Math.min(w - 1, Math.max(0, Math.round(x))), Math.min(S - 1, Math.max(0, Math.round(y)))];
  }
  return out;
}
const shiftAnchors = (a, dx) => Object.fromEntries(Object.entries(a).map(([n, [x, y]]) => [n, [x + dx, y]]));
/** The anchors with the body profile applied, matching the scaled layers. */
function anchorsFor(spec, p) {
  const a = anchorsOf(spec, p); if (!spec.bodyProfile || spec.bodyProfile === 'medium') return a;
  const [sx, sy] = BODY_PROFILES[spec.bodyProfile], cx = spec.view === 'back' ? BACK_X + 16 : 16, flip = spec.view === 'side' && spec.facing === 'left';
  return Object.fromEntries(Object.entries(a).map(([n, [x, y]]) => { const px = flip ? W - 1 - x : x, nx = Math.round(cx + (px - cx) * sx); return [n, [flip ? W - 1 - nx : nx, Math.round(30 + (y - 30) * sy)]]; }));
}

/** Which ramp each group of colour uses: the handles `dye` and `recolor` work with. */
export const paletteGroupsOf = (s) => ({
  skin: s.skin, cloth: s.cloth, legs: s.legs, boots: s.boots ?? 'leather',
  head: s.head === 'hair' ? HAIR[s.hair ?? 'brown'] : s.headRamp ?? 'steel',
  ...(s.hair ? { hair: HAIR[s.hair] } : {}), ...(s.gloves ? { gloves: s.gloves } : {}), ...(s.belt ? { belt: s.belt } : {}), ...(s.shoulders ? { shoulders: s.shoulders } : {}), ...(s.bracers ? { bracers: s.bracers } : {}), ...(s.tabard ? { tabard: s.tabard } : {}), ...(s.cloak ? { cloak: s.cloak } : {}), ...(s.neck ? { neck: s.neckRamp ?? (s.neck === 'amulet' ? 'gold' : s.neck === 'scarf' ? 'red' : 'leather') } : {}), ...(s.back ? { back: s.backRamp ?? (s.back === 'cape' ? 'red' : 'leather') } : {}),
});
/** The `drawCharacter` option each dye group sets (the hair group is the `hair` option's own names: brown, blond, dark, red). */
export const DYE_PARAMS = { skin: 'skin', cloth: 'cloth', legs: 'legs', boots: 'boots', head: 'headRamp', back: 'backRamp', gloves: 'gloves', belt: 'belt', shoulders: 'shoulders', bracers: 'bracers', tabard: 'tabard', cloak: 'cloak', neck: 'neckRamp' };

/** Every option explicit: the loadout's fields and the view filled in, `motion` left out. Drawn again it gives the same pixels, whatever a later version defaults to. */
export function resolveCharacter(opts = {}) {
  const { motion, ...rest } = opts;
  return { ...(opts.loadout ? { loadout: opts.loadout } : {}), view: 'front', ...(rest.view === TOP_VIEW ? { facing: 'south' } : {}), ...(LOADOUTS[opts.loadout] ?? LOADOUTS.guardian), ...rest };
}

/** A frame with the optional gear layers (boots, belt, shoulders, gloves, facial hair, hair style, eyewear) added, in their place in the order, and the composite redrawn. Without gear options it is the frame itself. */
function withGear(spec, p, fr, anchors) {
  const extras = gearLayers(spec, spec.view, spec.facing, anchors(spec, p), fr.layers); if (!Object.keys(extras).length) return fr;
  const layers = {};
  for (const [n, l] of Object.entries(fr.layers)) { layers[n] = l; for (const [x, after] of Object.entries(EXTRA_AFTER)) if (after === n && extras[x]) layers[x] = extras[x]; }
  for (const x of Object.keys(extras)) layers[x] ??= extras[x]; // eyewear with no hair-style layer goes last
  const order = Object.keys(layers), comp = new Pixels(fr.comp.w, fr.comp.h); for (const n of order) comp.blit(layers[n]);
  return { ...fr, layers, comp, order };
}

/** The layers of one frame, and the composite in the view's order. `dx` moves everything but the shadow, `lift` raises it. */
function frame(spec, p) {
  const view = spec.view, draw = VIEW_DRAW[view], layers = {}, w = view === 'back' ? BACK_W : W;
  for (const name of layerOrder(view)) {
    let l = draw[name](spec, p);
    if (name === 'torso') l = armorTexture(l, spec, tl(p), p.dy);
    if (name !== 'shadow') l = l.shift(p.dx, -p.lift);
    if (view === 'back') { const big = new Pixels(BACK_W, S).blit(l, BACK_X, 0); l = MIRRORED.includes(name) ? big.flipX() : big; }
    layers[name] = l;
  }
  if (spec.bodyProfile && spec.bodyProfile !== 'medium') { const cx = view === 'back' ? BACK_X + 16 : 16, [sx, sy] = BODY_PROFILES[spec.bodyProfile]; for (const n of Object.keys(layers)) layers[n] = scaleAbout(layers[n], sx, sy, cx, 30); }
  let comp = new Pixels(w, S); for (const name of layerOrder(view)) comp.blit(layers[name]);
  if (view === 'side' && spec.facing === 'left') { comp = comp.flipX(); for (const n of Object.keys(layers)) layers[n] = layers[n].flipX(); }
  return { layers, comp };
}

export function drawCharacter(opts = {}) {
  // `appearance` is what is shown, over what is equipped (the other options): a cosmetic override is a different param, never a different recipe.
  const spec = { view: 'front', ...(LOADOUTS[opts.loadout] ?? LOADOUTS.guardian), ...opts, ...opts.appearance };
  if (!HEADGEAR.includes(spec.head)) throw new Error(`unknown headgear "${spec.head}"`);
  if (!TORSO.includes(spec.torso)) throw new Error(`unknown torso "${spec.torso}"`);
  if (!VIEWS.includes(spec.view) && spec.view !== TOP_VIEW) throw new Error(`unknown view "${spec.view}"`);
  const top = spec.view === TOP_VIEW;
  if (top) { spec.facing ??= 'south'; if (!FACINGS.includes(spec.facing)) throw new Error(`unknown facing "${spec.facing}"; the top view faces ${FACINGS.join(', ')}`); }
  if (spec.pose && !POSES[spec.pose]) throw new Error(`unknown pose "${spec.pose}"`);
  if (spec.back && !BACK_SLOTS.includes(spec.back)) throw new Error(`unknown back slot "${spec.back}"`);
  if (spec.bodyProfile && !BODY_PROFILES[spec.bodyProfile]) throw new Error(`unknown body profile "${spec.bodyProfile}"; known: ${Object.keys(BODY_PROFILES).join(', ')}`);
  for (const [key, list] of [['armor', ARMOR_FAMILIES], ['bootStyle', BOOT_STYLES], ['hairStyle', HAIR_STYLES], ['facialHair', FACIAL_HAIR], ['eyewear', EYEWEAR], ['neck', NECK_KINDS], ['scar', SCARS], ['jewellery', JEWELLERY], ['helmStyle', HELM_STYLES]]) if (spec[key] && !list.includes(spec[key])) throw new Error(`unknown ${key} "${spec[key]}"; known: ${list.join(', ')}`);
  if (spec.weapon && !SHAPES[spec.weapon]) throw new Error(`unknown weapon "${spec.weapon}"`);
  const stance = POSES[spec.pose ?? 'stand'], view = spec.view;
  // Squatting sinks the body by the same amount; a cape lags behind a stride (side) or swings its hem (back).
  const base = (o = {}) => { const merged = { ...stance, ...o }; return pose({ ...merged, dy: (merged.dy ?? 0) + (merged.squat ?? 0) }); };
  const walkDrag = [2, 0, 1, 0], sway = [1, 0, -1, 0];
  const idle = [base(), base({ dy: (stance.dy ?? 0) + 1 })];
  const walk = [{ dy: 0, legs: [-2, 2] }, { dy: -1, legs: [0, 0] }, { dy: 0, legs: [2, -2] }, { dy: -1, legs: [0, 0] }].map((o, i) => base({ ...o, drag: walkDrag[i], sway: sway[i] }));
  // Seen from above a lunge is along the facing and the canvas ends 20 pixels out, so the top view lunges less and cocks the weapon back.
  const attack = (top ? [{ dx: -1, arm: 'raised', weapon: 'up' }, { dx: 0, arm: 'raised', weapon: 'up' }, { dx: 1, arm: 'forward', weapon: 'down' }, { dx: 0, arm: 'down', weapon: 'up' }] : [{ dx: -1, arm: 'raised', weapon: 'up' }, { dx: 1, arm: 'raised', weapon: 'up' }, { dx: 3, arm: 'forward', weapon: 'down' }, { dx: 0, arm: 'down', weapon: 'up' }]).map((o) => base(o));
  const profile = BODY_PROFILES[spec.bodyProfile ?? 'medium'];
  const anchors_ = top ? (sp, p) => topAnchors(sp, p, profile) : anchorsFor, frame0 = top ? (sp, p) => topFrame(sp, p, profile) : frame;
  const frame_ = (sp, p) => withGear(sp, p, frame0(sp, p), anchors_);
  const run = (list) => list.map((p) => frame_(spec, p));
  // `motion`: a clip (or list of clips) of poses from Animator, `{ id, frames: Pose[], fps, loop, events, holds?, reduced? }`, drawn with this
  // loadout and added to `clips` under its id. A pose is the channels of `pose` above; it merges onto the stance like the built-in clips.
  const feet = top ? [CX, CY] : view === 'back' ? [BACK_X + 16, 30] : spec.view === 'side' && spec.facing === 'left' ? [W - 1 - 16, 30] : [16, 30];
  const withRoot = (a) => (top ? { ...a, root: [CX, CY], feet: [CX, CY], shadow_center: [CX, CY + 3] } : { ...a, root: [feet[0], 30], feet: [feet[0], 30], shadow_center: [feet[0], 29] });
  const motionClip = (m) => ({ ...motionToClip(m, m.frames.map((f) => frame_(spec, base(f)).comp), m.reduced?.length ? m.reduced.map((f) => frame_(spec, base(f)).comp) : undefined), anchors: m.frames.map((f) => withRoot(anchors_(spec, base(f)))) });
  const motions = motionClips(opts.motion).map(motionClip);
  const idleF = run(idle), walkF = run(walk), attackF = view === 'back' ? null : run(attack), still = idleF[0].comp;
  // The core semantic clips (T8), from the same joints as the rest: each is a list of pose channels, an event name where it lands, and a still for reduced motion.
  const semantic = Object.fromEntries(Object.entries(SEMANTIC).map(([id, def]) => {
    const poses = def.poses.map((o) => base(o.lift ? { ...o, lift: (stance.lift ?? 0) + o.lift } : o)), frames = run(poses).map((f) => f.comp), last = id === 'recall' ? flash(frames.at(-1), C.ice) : null;
    if (last) frames.push(last);
    if (def.flashAt !== undefined) frames[def.flashAt] = flash(frames[def.flashAt], C[def.flash]);
    return [id, { ...clip(id, frames, { fps: def.fps, loop: def.loop ?? false, events: [{ frame: Math.min(def.at, frames.length - 1), name: def.event }], reduced: [frames[Math.min(def.at, frames.length - 1)]] }), anchors: frames.map((_, i) => withRoot(anchors_(spec, poses[Math.min(i, poses.length - 1)]))) }];
  }));
  const clips = {
    idle: { ...clip('idle', idleF.map((f) => f.comp), { fps: 2 }), anchors: idle.map((q) => withRoot(anchors_(spec, q))) },
    walk: { ...clip('walk', walkF.map((f) => f.comp), { fps: 8, events: [{ frame: 0, name: 'step' }, { frame: 2, name: 'step' }] }), anchors: walk.map((q) => withRoot(anchors_(spec, q))) },
    ...(attackF ? { attack: { ...clip('attack', attackF.map((f) => f.comp), { fps: 10, loop: false, events: [{ frame: 2, name: 'impact' }], reduced: [attackF[2].comp] }), anchors: attack.map((q) => withRoot(anchors_(spec, q))) } } : {}),
    hit: { ...clip('hit', [flash(still), still.shift(-2, 0)], { fps: 10, loop: false }), anchors: [0, -2].map((dx) => shiftAnchors(withRoot(anchors_(spec, base())), dx)) },
    defeat: { ...clip('defeat', collapse(still), { fps: 8, loop: false, events: [{ frame: 3, name: 'gone' }] }), anchors: collapse(still).map(() => withRoot({})) },
    ...semantic,
    ...Object.fromEntries(motions.map((c) => [c.id, c])),
  };
  return {
    id: `character.${opts.loadout ?? 'custom'}${view === 'front' ? '' : `.${view}${top ? `.${spec.facing}` : ''}`}${spec.pose ? `.${spec.pose}` : ''}`, frames: idleF.map((f) => f.comp), fps: 2, loop: true, spec,
    anchors: { ...withRoot(anchors_(spec, idle[0])), feet, frames: idle.map((q) => withRoot(anchors_(spec, q))) }, paletteGroups: paletteGroupsOf(spec), projection: top ? 'top-down' : 'front', weaponAttack: weaponAttack(spec.weapon), layers: Object.fromEntries(Object.keys(idleF[0].layers).map((n) => [n, idleF.map((f) => f.layers[n])])), clips,
  };
}

/** Portrait sizes (T9): `party` the head alone (16 by 16), `status` the bust (20 by 20, the original), `card` the whole unarmed figure (32 by 32). */
export const PORTRAIT_SIZES = { party: [8, 0, 16, 16], status: [6, 1, 20, 20], card: [0, 0, 32, 32] };
export const PORTRAIT_STATES = ['normal', 'defeated', 'recalling'];

/** The face by expression: the nine `face` expressions drawn on the front bust (eyes, brows, mouth), only over skin so a helm or a hood is never painted over. */
function expressed(px, spec, name, [ox, oy]) {
  const ex = EXPRESSIONS[name]; if (!ex) throw new Error(`unknown expression "${name}"`);
  const skin = ramp(spec.skin), isSkin = (x, y) => skin.includes(px.get(x, y)), N = C.night;
  const at = (x, y, c) => { if (isSkin(x - ox, y - oy)) px.set(x - ox, y - oy, c); };
  for (const x of [13, 14, 17, 18]) for (const y of [9, 10]) at(x, y, skin[2]); // wipe the neutral eyes
  for (const [x0, flip] of [[13, 1], [18, -1]]) {
    if (ex.eye === 0) { at(x0, 10, N); at(x0 + flip, 10, N); at(x0 + flip, 9, N); } // squeezed shut
    else if (ex.eye === 1) { at(x0, 10, N); at(x0 + flip, 10, N); } // narrowed
    else if (ex.eye === 2) { at(x0, 9, N); at(x0, 10, N); at(x0 + flip, 9, C.white); }
    else { at(x0, 9, N); at(x0, 10, N); at(x0, 11, N); at(x0 + flip, 9, C.white); at(x0 + flip, 10, C.white); }
    const up = ex.lift, inner = x0 + flip;
    if (ex.brow === 0 && up) { at(x0, 8 - up, N); at(inner, 8 - up, N); }
    else if (ex.brow === 1) { at(x0, 8 - up, N); at(inner, 7 - up, N); } // the inner end up: worry, pain
    else if (ex.brow === -1) { at(x0, 7, N); at(inner, 8, N); } // the inner end down: anger
  }
  const m = ex.mouth;
  if (m === 'flat') { at(15, 13, N); at(16, 13, N); }
  else if (m === 'smile') { at(14, 12, N); at(15, 13, N); at(16, 13, N); at(17, 12, N); }
  else if (m === 'open') { for (const x of [15, 16]) for (const y of [12, 13]) at(x, y, N); }
  else if (m === 'frown') { at(14, 13, N); at(15, 12, N); at(16, 12, N); at(17, 13, N); }
  else { for (let x = 14; x <= 17; x++) { at(x, 12, C.white); at(x, 13, N); } }
  return px;
}
/** Grey the picture: every colour becomes the stone ramp step of its lightness, with the dark outline kept. */
const greyed = (px) => px.map((x, y) => { const v = px.get(x, y); if (!v) return 0; const l = px.luma(v); return l < 40 ? C.night : l < 100 ? ramp('stone')[1] : l < 170 ? ramp('stone')[2] : ramp('stone')[3]; });
/** Fade the picture from the bottom up in dither bands, with two sparks above: the character is being called away. */
const fading = (px) => { const b = px.bbox(); return px.map((x, y) => { const v = px.get(x, y); if (!v) return 0; const t = (y - b.y) / b.h; return t > 0.75 && (x + y) % 2 ? 0 : t > 0.9 && x % 2 ? 0 : v; }).clone().set(b.x + (b.w >> 1) - 2, Math.max(0, b.y - 1), C.ice).set(b.x + (b.w >> 1) + 2, Math.max(0, b.y), C.white); };

/**
 * A portrait of a character. `size` (T9): `status` (20 by 20 bust, the default and what this always drew), `party` (16 by 16, the head) or `card` (32 by 32, the whole
 * unarmed figure). `expression` is one of `face`'s nine (the bust sizes). `state` is `normal`, `defeated` (grey) or `recalling` (fading from the feet up). The art
 * carries `mask` (its silhouette) and `safe` (the box inside it a frame or a badge must not cover), so a compositor can frame it.
 */
export function drawPortrait(opts = {}) {
  const { size = 'status', state = 'normal', expression, ...rest } = opts, box = PORTRAIT_SIZES[size];
  if (!box) throw new Error(`unknown portrait size "${size}"; known: ${Object.keys(PORTRAIT_SIZES).join(', ')}`);
  if (!PORTRAIT_STATES.includes(state)) throw new Error(`unknown portrait state "${state}"; known: ${PORTRAIT_STATES.join(', ')}`);
  const art = drawCharacter({ weapon: null, offhand: null, ...rest }), [bx, by, bw, bh] = box;
  let px = art.frames[0].crop(bx, by, bw, bh);
  if (expression !== undefined) px = expressed(px, art.spec, expression, [bx, by]);
  if (state === 'defeated') px = greyed(px); else if (state === 'recalling') px = fading(px);
  const b = px.bbox(), tail = [size === 'status' ? '' : `.${size}`, expression ? `.${expression}` : '', state === 'normal' ? '' : `.${state}`].join('');
  return { id: `portrait.${art.id.replace(/^character\./, '')}${tail}`, frames: [px], fps: 0, loop: false, mask: px.silhouette(1), safe: b ? { x: b.x + 1, y: b.y + 1, w: Math.max(1, b.w - 2), h: Math.max(1, b.h - 2) } : null };
}
