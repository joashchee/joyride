// Faces and expressions (16x16). The face kit's two ideas as parameters: Duchenne's muscle mechanics (each expression is a brow,
// an eye and a mouth setting, changed separately) and its four axes of range: age, phenotype, head angle, emotional intensity.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

/** brow: +1 raises the inner end (fear, pain, sadness), -1 lowers it (anger, disgust); lift raises the whole brow; eye: 0 shut, 1 narrow, 2 open, 3 wide. */
export const EXPRESSIONS = {
  neutral: { brow: 0, lift: 0, eye: 2, mouth: 'flat' },
  joy: { brow: 0, lift: 0, eye: 1, mouth: 'smile' },
  surprise: { brow: 0, lift: 2, eye: 3, mouth: 'open' },
  fear: { brow: 1, lift: 1, eye: 3, mouth: 'open' },
  anger: { brow: -1, lift: 0, eye: 1, mouth: 'teeth' },
  pain: { brow: 1, lift: 0, eye: 0, mouth: 'frown' },
  sadness: { brow: 1, lift: 0, eye: 2, mouth: 'frown' },
  attention: { brow: 0, lift: 1, eye: 2, mouth: 'flat' },
  disgust: { brow: -1, lift: 0, eye: 1, mouth: 'frown' },
};
export const PHENOTYPES = ['skin-fair', 'skin-light', 'skin-medium', 'skin-deep'];
export const AGES = ['child', 'adult', 'elder'];
export const HAIR_STYLES = ['short', 'long', 'bald', 'hood'];
const HAIR = {
  dark: [C.night, C.night, C.plum, C.ash], brown: [C.night, C.plum, C.umber, C.brown], red: [C.plum, C.red, C.orange, C.yellow],
  blond: [C.umber, C.bronze, C.yellow, C.peach], grey: [C.dust, C.grey, C.mist, C.ice], white: [C.grey, C.mist, C.ice, C.white],
};

export function drawFace({ expression = 'neutral', age = 'adult', skin = 'skin-light', angle = 'front', intensity = 1, hair = 'brown', style = 'short' } = {}) {
  const ex = EXPRESSIONS[expression]; if (!ex) throw new Error(`unknown expression "${expression}"`);
  if (!AGES.includes(age)) throw new Error(`unknown age "${age}"`);
  const S = 16, q = angle === 'three-quarter' ? 1 : 0; // three-quarter: the features sit toward the near side and the far cheek narrows
  const head = new Pixels(S, S), cy = age === 'child' ? 8.5 : 8, ry = age === 'child' ? 5.5 : 6;
  head.ellipse(8 + q * 0.5, cy, age === 'child' ? 6 : 5.5 - q * 0.5, ry, 1);
  const out = shade(head, { 1: ramp(skin) }, { outline: null });
  const strand = age === 'elder' ? HAIR.grey : HAIR[hair] ?? HAIR.brown;
  if (style !== 'bald') {
    const hm = head.map((x, y) => (head.get(x, y) && (y <= 4 || ((x <= 3 || x >= 12) && y <= (style === 'long' ? 12 : 7)) || (y === 5 && (x <= 5 || x >= 10))) ? 1 : 0));
    if (style === 'hood') for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (head.get(x, y) && (y <= 5 || x <= 3 || x >= 12 || y >= 13)) hm.set(x, y, 1);
    out.blit(shade(hm, { 1: style === 'hood' ? ramp('blue') : strand }, { outline: null }));
  }
  const dark = C.night, ox = q, eyeY = age === 'child' ? 8 : 7, lx = 4 + ox, rx = 10 + ox, ts = Math.max(-2, Math.min(2, ex.brow * intensity));
  const eye = (x, inner) => { // two pixels wide; `inner` is the column nearer the nose, where the pupil sits
    if (ex.eye === 0) { out.rect(x, eyeY + 1, 2, 1, dark); return; }
    const h = ex.eye === 1 ? 1 : ex.eye === 2 ? 2 : 3, y = ex.eye === 1 ? eyeY + 1 : eyeY - (h - 2 > 0 ? 1 : 0);
    out.rect(x, y, 2, h, C.white); out.rect(inner, y + (h > 1 ? 1 : 0), 1, 1, dark);
    if (h === 3) out.set(inner, y + 2, dark);
  };
  eye(lx, lx + 1); eye(rx, rx);
  const brow = (x, innerAtRight) => { // outer, middle and inner pixel, the inner one moved by the tilt
    const y = eyeY - 2 - ex.lift - (ex.eye === 3 ? 1 : 0), dyIn = -ts, dyMid = Math.trunc(dyIn / 2), cols = innerAtRight ? [x - 1, x, x + 1] : [x, x + 1, x + 2];
    const dys = innerAtRight ? [0, dyMid, dyIn] : [dyIn, dyMid, 0];
    cols.forEach((cx, i) => out.set(cx, y + dys[i], dark));
  };
  brow(lx, true); brow(rx, false);
  const my = age === 'child' ? 11 : 11, mx = 8 + ox, n = Math.min(2, Math.max(1, intensity));
  if (ex.mouth === 'flat') out.rect(mx - 2, my, 4, 1, dark);
  else if (ex.mouth === 'smile') { out.rect(mx - 1, my + 1, 3, 1, dark).set(mx - 2, my, dark).set(mx + 2, my, dark); if (intensity > 1) out.rect(mx - 1, my, 3, 1, C.white); }
  else if (ex.mouth === 'frown') out.rect(mx - 1, my, 3, 1, dark).set(mx - 2, my + 1, dark).set(mx + 2, my + 1, dark);
  else if (ex.mouth === 'open') { out.rect(mx - 1, my - 1, 3, 1 + n, dark).set(mx, my - 1 + n, C.red); }
  else if (ex.mouth === 'teeth') out.rect(mx - 2, my - 1, 5, 3, dark).rect(mx - 1, my - 1, 3, 1, C.white).rect(mx - 1, my + 1, 3, 1, C.ice);
  if (age === 'elder') out.set(lx - 1, eyeY + 2, ramp(skin)[0]).set(rx + 2, eyeY + 2, ramp(skin)[0]).set(mx - 3, my - 1, ramp(skin)[0]).set(mx + 3, my - 1, ramp(skin)[0]);
  if (age === 'child') out.set(lx, eyeY + 3, C.rose).set(rx + 1, eyeY + 3, C.rose);
  out.outline(dark);
  return { id: `face.${expression}`, frames: [out], fps: 0, loop: false };
}
