// Combat and magic effects (32x32, 4 frames each): short, one shot, no flashing. They play over a sprite the way strokes.js overlays do.
import { Pixels } from '../pixels.js';
import { C } from '../palette.js';
import { clip } from '../anim.js';

const S = 32;
const plus = (p, x, y, r, c) => { p.rect(x - r, y, 2 * r + 1, 1, c).rect(x, y - r, 1, 2 * r + 1, c); };

export const EFFECTS = {
  slash() {
    const cols = [C.white, C.ice, C.cyan, C.sky];
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (let a = -60; a <= -60 + 30 * (i + 1) + 30; a += 3) { const t = (a * Math.PI) / 180, x = 16 + 13 * Math.cos(t + 0.3), y = 16 + 13 * Math.sin(t + 0.3); const age = (-60 + 30 * (i + 1) + 30 - a) / 90; p.rect(x, y, 2, 2, cols[Math.min(3, Math.floor(age * 4))]); }
      return p; });
  },
  burst() {
    const cols = [C.white, C.yellow, C.orange, C.red];
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 3 + 4 * i;
      for (let k = 0; k < 8; k++) { const t = (k * Math.PI) / 4; const x = 16 + r * Math.cos(t), y = 16 + r * Math.sin(t); if (i < 3 || k % 2) p.rect(x, y, i < 2 ? 2 : 1, i < 2 ? 2 : 1, cols[i]); }
      return p; });
  },
  sparkle() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = [1, 2, 3, 1][i];
      for (const [x, y, o] of [[9, 10, 0], [22, 8, 1], [16, 22, 2]]) plus(p, x, y, Math.max(1, ((r + o) % 3) + 1) > 2 ? 2 : 1, i % 2 ? C.white : C.yellow);
      return p; });
  },
  heal() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const [x, y] of [[10, 26], [20, 28], [15, 30]]) plus(p, x, y - 6 * i, 1, i === 3 ? C.green : C.lime);
      return p; });
  },
  // Per-school spells for the CoffeeMUD kit. Each grows or falls over four frames; none flashes (colour steps are gradual, no full-frame change).
  fire() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), h = 5 + 4 * i;
      for (const [x, w] of [[11, 3], [16, 4], [21, 3]]) { const hh = h - (x === 16 ? 0 : 2); p.ellipse(x, 28 - hh / 2, w, hh / 2, C.red); p.ellipse(x, 28 - hh / 3, w - 1, hh / 3, C.orange); p.ellipse(x, 28 - hh / 5, Math.max(1, w - 2), hh / 5, C.yellow); }
      return p; });
  },
  ice() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 3 + 3 * i;
      for (let k = 0; k < 6; k++) { const t = (k * Math.PI) / 3; p.line(16, 16, 16 + r * Math.cos(t), 16 + r * Math.sin(t), i < 3 ? C.cyan : C.sky); p.rect(16 + r * Math.cos(t), 16 + r * Math.sin(t), 1, 1, C.white); }
      p.rect(15, 15, 2, 2, C.white); return p; });
  },
  bolt() {
    const path = [[18, 2], [13, 9], [18, 12], [12, 20], [17, 22], [11, 30]];
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), n = Math.min(path.length - 1, 2 + i);
      for (let k = 0; k < n; k++) { p.thick(path[k][0], path[k][1], path[k + 1][0], path[k + 1][1], C.sky, 2); p.line(path[k][0], path[k][1], path[k + 1][0], path[k + 1][1], i === 3 ? C.white : C.yellow); }
      return p; });
  },
  curse() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 4 + 2 * i;
      for (let k = 0; k < 12; k++) { const t = (k * Math.PI) / 6 + i * 0.3; p.rect(16 + r * Math.cos(t), 16 + r * Math.sin(t), 2, 2, k % 3 ? C.violet : C.plum); }
      p.rect(15, 12 + (3 - i), 2, 6 - (3 - i) * 0, i < 2 ? C.violet : C.pink); p.rect(13, 14 + (3 - i), 6, 1, i < 2 ? C.violet : C.pink); return p; });
  },
  poison() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const [x, y, d] of [[10, 24, 0], [17, 26, 1], [22, 25, 2], [14, 28, 3]]) { const rise = 5 * ((i + d) % 4), r = 1 + ((i + d) % 4 > 1 ? 1 : 0); p.ellipse(x, y - rise, r + 1, r + 1, C.green); p.rect(x - 1, y - rise - 1, 1, 1, C.lime); }
      return p; });
  },
  // Per-school spells, second batch: necromancy, illusion, blessing, disease.
  necromancy() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (let k = 0; k < 4; k++) { const x = 7 + 6 * k, y = 28 - 5 * i - 2 * k; p.ellipse(x, y - 2, 2, 2.4, k % 2 ? C.green : C.violet); p.rect(x - 1, y - 3, 1, 1, C.night).rect(x + 1, y - 3, 1, 1, C.night); p.rect(x, y, 1, 3, k % 2 ? C.moss : C.plum); }
      p.ellipse(16, 29, 5 + i, 1.5, C.plum); return p; });
  },
  illusion() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const [r, c] of [[3 + 3 * i, C.cyan], [7 + 3 * i, C.violet]]) for (let k = 0; k < 24; k++) { if ((k + i) % 2) continue; const t = (k * Math.PI) / 12; p.rect(16 + r * Math.cos(t), 16 + r * Math.sin(t), 1, 1, c); }
      p.rect(15, 15, 2, 2, C.white); return p; });
  },
  blessing() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const x of [9, 13, 16, 19, 23]) p.rect(x, 1, 1, 6 + 5 * i - Math.abs(x - 16) / 2, i < 3 ? C.yellow : C.white);
      p.rect(14, 22 - i, 5, 1, C.yellow).rect(16, 20 - i, 1, 5, C.yellow); p.rect(16, 22 - i, 1, 1, C.white); return p; });
  },
  disease() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (let k = 0; k < 6; k++) { const t = (k * Math.PI) / 3 + i * 0.5, r = 6 + (k % 2) * 3; p.ellipse(16 + r * Math.cos(t), 16 + r * Math.sin(t), 2, 2, k % 2 ? C.moss : C.lime); p.rect(16 + r * Math.cos(t), 16 + r * Math.sin(t), 1, 1, C.yellow); }
      p.ellipse(16, 16, 2, 2, C.green); return p; });
  },
  summon() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), rx = 4 + 3 * i;
      p.ellipse(16, 24, rx, rx / 3 + 1, C.violet); p.ellipse(16, 24, Math.max(1, rx - 2), Math.max(1, rx / 3 - 1), C.night);
      for (let k = 0; k < 6; k++) { const t = (k * Math.PI) / 3; p.rect(16 + (rx - 1) * Math.cos(t), 24 + (rx / 3) * Math.sin(t), 1, 1, C.white); }
      for (let y = 0; y < 3 * i; y++) p.rect(16 + ((y * 7) % 5) - 2, 22 - y * 2, 1, 2, y % 2 ? C.cyan : C.violet);
      return p; });
  },
  // Tag Team T19, the rest of the list: barrier and its impact, buff and debuff, arrow, weapon trail, rune, portal, level-up, critical, loot reveal.
  shield() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 8 + 2 * Math.min(i, 2);
      p.ellipse(16, 16, r, r, i === 3 ? C.sky : C.cyan); p.ellipse(16, 16, r - 1, r - 1, 0);
      for (let k = 0; k < 6; k++) { const t = (k * Math.PI) / 3 + i * 0.2; p.rect(16 + (r - 3) * Math.cos(t), 16 + (r - 3) * Math.sin(t), 1, 1, C.ice); }
      p.rect(16 - 2, 16 - r, 5, 1, C.white); return p; });
  },
  shieldhit() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 11;
      p.ellipse(16, 16, r, r, C.sky); p.ellipse(16, 16, r - 1, r - 1, 0);
      for (let k = 0; k < 8; k++) { const t = (k * Math.PI) / 4, d = 3 + 2 * i; p.rect(8 + d * Math.cos(t), 16 + d * Math.sin(t), 1, 1, i < 3 ? C.ice : C.cyan); }
      p.ellipse(8, 16, 4 - i, 4 - i, i < 2 ? C.white : C.ice); return p; });
  },
  buff() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const [x, d] of [[10, 0], [16, 2], [22, 1]]) { const y = 26 - 5 * ((i + d) % 4); p.poly([[x, y - 3], [x - 3, y + 1], [x + 3, y + 1]], C.lime); p.rect(x - 1, y + 1, 3, 2, C.green); }
      return p; });
  },
  debuff() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      for (const [x, d] of [[10, 0], [16, 2], [22, 1]]) { const y = 6 + 5 * ((i + d) % 4); p.poly([[x, y + 3], [x - 3, y - 1], [x + 3, y - 1]], C.red); p.rect(x - 1, y - 3, 3, 2, C.plum); }
      return p; });
  },
  arrow() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), x = 4 + 7 * i;
      p.line(x - 8, 16, x + 4, 16, C.tan); p.poly([[x + 8, 16], [x + 3, 13], [x + 3, 19]], C.steel); p.rect(x - 8, 14, 1, 5, C.ice);
      p.line(x - 14, 16, x - 9, 16, C.mist); return p; });
  },
  trail() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), cols = [C.white, C.ice, C.sky, C.steel];
      for (let k = 0; k <= 3; k++) { const a = -70 + 28 * (i + 1) - 18 * k, t = (a * Math.PI) / 180; if (a >= -70) p.thick(16 + 12 * Math.cos(t), 16 + 12 * Math.sin(t), 16 + 12 * Math.cos(t + 0.3), 16 + 12 * Math.sin(t + 0.3), cols[k], 2); }
      return p; });
  },
  rune() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 11;
      p.ellipse(16, 16, r, r, i % 2 ? C.violet : C.pink); p.ellipse(16, 16, r - 1, r - 1, 0); p.ellipse(16, 16, 6, 6, C.violet); p.ellipse(16, 16, 5, 5, 0);
      for (let k = 0; k < 3; k++) { const t = (k * 2 * Math.PI) / 3 - Math.PI / 2 + i * 0.1; p.rect(16 + 6 * Math.cos(t), 16 + 6 * Math.sin(t), 1, 1, C.white); }
      p.rect(16, 13, 1, 6, C.pink).rect(14, 15, 5, 1, C.pink); return p; });
  },
  portal() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      p.ellipse(16, 16, 7, 12, C.violet); p.ellipse(16, 16, 5, 10, C.night);
      for (let k = 0; k < 8; k++) { const t = (k * Math.PI) / 4 + i * 0.5, rr = 2 + (k % 3); p.rect(16 + rr * Math.cos(t) * 0.8, 16 + rr * 2 * Math.sin(t), 1, 1, k % 2 ? C.cyan : C.pink); }
      return p; });
  },
  levelup() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), y = 26 - 4 * i;
      p.poly([[16, y - 8], [9, y], [23, y]], C.yellow); p.rect(13, y, 7, 5, C.orange); p.rect(15, y - 4, 3, 2, C.white);
      for (const [x, d] of [[6, 0], [26, 2]]) plus(p, x, 22 - 4 * ((i + d) % 4), 1, C.yellow);
      return p; });
  },
  crit() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S), r = 4 + 3 * i;
      for (let k = 0; k < 8; k++) { const t = (k * Math.PI) / 4, a = k % 2 ? r : r + 4; p.line(16 + 2 * Math.cos(t), 16 + 2 * Math.sin(t), 16 + a * Math.cos(t), 16 + a * Math.sin(t), k % 2 ? C.orange : C.yellow); }
      p.ellipse(16, 16, 2, 2, C.white); return p; });
  },
  loot() {
    return Array.from({ length: 4 }, (_, i) => { const p = new Pixels(S, S);
      p.ellipse(16, 24, 5 + i, 2, C.gold ?? C.yellow);
      for (let k = 0; k < 3 + i; k++) { const x = 16 + (k - (2 + i) / 2) * 4; p.rect(x, 22 - 3 * ((k + i) % 3) - i * 2, 1, 4 + (k % 2), i === 3 ? C.white : C.yellow); }
      plus(p, 16, 8 + (3 - i), 1 + (i > 1 ? 1 : 0), C.white); return p; });
  },
};
export const EFFECT_IDS = Object.keys(EFFECTS);

/** The no-flash setting: the brightest colours drop one step, so a burst is a glow and not a pop. */
const calm = (f) => f.map((x, y) => { const v = f.get(x, y); return v === C.white ? C.ice : v === C.yellow ? C.tan : v; });
/** The low-effects setting: every second small spark (a piece of 5 pixels or fewer, in scan order) is left out, so the particles thin by half and the body of the effect stays. */
function sparse(f) {
  const out = f.clone(), seen = new Set(); let n = 0;
  for (let y = 0; y < f.h; y++) for (let x = 0; x < f.w; x++) {
    if (!f.get(x, y) || seen.has(y * f.w + x)) continue;
    const piece = [], todo = [[x, y]]; seen.add(y * f.w + x);
    while (todo.length) { const [cx, cy] = todo.pop(); piece.push([cx, cy]); for (const [nx, ny] of [[cx - 1, cy], [cx + 1, cy], [cx, cy - 1], [cx, cy + 1]]) if (f.get(nx, ny) && !seen.has(ny * f.w + nx)) { seen.add(ny * f.w + nx); todo.push([nx, ny]); } }
    if (piece.length <= 5 && n++ % 2) for (const [px, py] of piece) out.set(px, py, 0);
  }
  return out;
}

/** `flash: false` is the no-flash setting and `density: 'low'` the low-effects one (half the small sparks) (Tag Team 3.6); the defaults draw what they always did. */
export function drawEffect({ effect = 'slash', flash = true, density = 'normal' } = {}) {
  const make = EFFECTS[effect]; if (!make) throw new Error(`unknown effect "${effect}"`);
  if (density !== 'normal' && density !== 'low') throw new Error(`unknown density "${density}"; known: normal, low`);
  let frames = make();
  if (!flash) frames = frames.map(calm);
  if (density === 'low') frames = frames.map(sparse);
  return { id: `effect.${effect}`, frames, fps: 12, loop: false, clips: { play: clip(effect, frames, { fps: 12, loop: false, reduced: [frames[2]] }) } };
}
