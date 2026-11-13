// A splash (title) screen from two facts: the `title` and the `scene` behind it. Models docs/cc0-splash-screen-reference-kit.md.
// The rules the kit teaches: one focal title, the scene kept quiet behind it, a dark plate under the letters for contrast, margin on
// every side, and nothing else on the screen. Pixel: 256 by 144 (16:9) on DB32, the title set in the VGA font from the atlas passed in
// (no atlas, no letters, the plate stays: the same rule as `map`'s label). ANSI: 80 by 24 cells, block lettering over a vista, legal for
// a classic file. All randomness is from `rng(seed)`. Browser-safe.
import { Pixels } from '../pixels.js';
import { C } from '../palette.js';
import { rng } from '../rng.js';
import { Cells, legalize } from '../cells.js';
import { V } from '../palette-ansi.js';
import { lettering, LETTERING_CHARS } from '../ansi-craft.js';
import { pixelText } from './pixel-text.js';
import { drawVista } from './vista.js';

export const SPLASH_W = 256, SPLASH_H = 144, SPLASH_MAX_TITLE = 28;
export const SPLASH_SCENES = ['meadow', 'forest', 'desert', 'mountains', 'sea', 'snow', 'night'];

// What each scene is in ANSI: a vista's terrain, time and weather.
const VISTAS = {
  meadow: { terrain: 'plains', time: 'day' }, forest: { terrain: 'woods', time: 'dusk' }, desert: { terrain: 'desert', time: 'day' },
  mountains: { terrain: 'mountains', time: 'dawn' }, sea: { terrain: 'watersurface', time: 'dusk' },
  snow: { terrain: 'hills', time: 'day', weather: 'snow' }, night: { terrain: 'plains', time: 'night' },
};

const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/** Sky bands top to horizon, each seam dithered with a 4 by 4 Bayer matrix so the steps do not show as stripes. */
function sky(p, y0, y1, ramp) {
  const n = ramp.length, span = (y1 - y0) / n;
  p.rect(0, y1, p.w, p.h - y1, ramp[n - 1]);   // below the horizon band the last colour runs on, so nothing is left unpainted behind a ridge
  for (let y = y0; y < y1; y++) {
    const t = (y - y0) / span, i = Math.min(n - 1, Math.floor(t)), f = t - i;
    for (let x = 0; x < p.w; x++) p.set(x, y, i + 1 < n && f > BAYER[y & 3][x & 3] / 16 + 0.3 ? ramp[i + 1] : ramp[i]);
  }
}
/** A ridge line from summed sines with seeded phases; `fill` below it, `top` as a lit edge one pixel deep. */
function ridge(p, r, base, amp, scale, fill, top) {
  const a = r.next() * 6.28, b = r.next() * 6.28, c = r.next() * 6.28;
  for (let x = 0; x < p.w; x++) {
    const h = Math.round(base - amp * (0.55 * Math.sin(x / scale + a) + 0.3 * Math.sin(x / (scale * 0.43) + b) + 0.15 * Math.sin(x / (scale * 0.19) + c)));
    for (let y = h; y < p.h; y++) p.set(x, y, y === h && top ? top : fill);
  }
}
const stars = (p, r, n, yMax, cols = [C.white, C.ice, C.sky]) => { for (let i = 0; i < n; i++) p.set(r.int(p.w), r.int(yMax), r.pick(cols)); };
const disc = (p, cx, cy, rad, core, halo) => { if (halo) p.ellipse(cx, cy, rad + 3, rad + 3, halo); p.ellipse(cx, cy, rad, rad, core); };
const pine = (p, x, y, h, dark, light) => {
  p.rect(x, y - 2, 2, 3, C.umber);
  for (let k = 0; k < 3; k++) { const w = 3 + k * 2, yy = y - 3 - k * Math.floor(h / 3); p.poly([[x + 1 - w, yy + 4], [x + 1, yy - 4], [x + 2 + w, yy + 4]], dark); p.line(x + 1, yy - 3, x + 1 - w + 1, yy + 3, light); }
};
const round = (p, x, y, rad, dark, light) => { p.rect(x, y, 2, rad, C.umber); p.ellipse(x + 1, y - rad / 2, rad, rad - 1, dark); p.ellipse(x, y - rad / 2 - 1, Math.max(1, rad - 3), Math.max(1, rad - 4), light); };
const grassTufts = (p, r, y0, n, cols) => { for (let i = 0; i < n; i++) { const x = r.int(p.w), y = y0 + r.int(p.h - y0); p.set(x, y, r.pick(cols)); p.set(x, y - 1, r.pick(cols)); } };

const SCENE_DRAW = {
  meadow(p, r) {
    sky(p, 0, 90, [C.blue, C.sky, C.cyan, C.ice]); disc(p, 190, 40, 10, C.yellow, C.peach);
    for (let i = 0; i < 4; i++) { const x = r.int(220), y = 15 + r.int(40); p.ellipse(x + 12, y, 14, 3, C.white); p.ellipse(x + 8, y - 2, 9, 3, C.white); }
    ridge(p, r, 92, 14, 38, C.green, C.lime); ridge(p, r, 108, 12, 26, C.forest, C.green); ridge(p, r, 126, 6, 20, C.night, C.forest);
    grassTufts(p, r, 110, 90, [C.lime, C.moss, C.green]); for (let i = 0; i < 18; i++) p.set(r.int(p.w), 112 + r.int(30), r.pick([C.yellow, C.pink, C.white]));
  },
  forest(p, r) {
    sky(p, 0, 80, [C.indigo, C.violet, C.rose, C.orange, C.peach]); disc(p, 128, 76, 12, C.yellow, C.orange); stars(p, r, 14, 24);
    ridge(p, r, 84, 10, 30, C.indigo, C.violet); ridge(p, r, 100, 9, 24, C.forest, C.green);
    for (let x = 4; x < p.w; x += 10 + r.int(7)) pine(p, x, 112 + r.int(6), 18, C.forest, C.green);
    ridge(p, r, 128, 5, 18, C.night, C.bark); for (let x = 8; x < p.w; x += 24 + r.int(14)) pine(p, x, 140, 24, C.night, C.forest);
  },
  desert(p, r) {
    sky(p, 0, 92, [C.steel, C.sky, C.cyan, C.ice, C.peach]); disc(p, 64, 46, 12, C.white, C.yellow);
    ridge(p, r, 96, 10, 46, C.tan, C.peach); ridge(p, r, 112, 12, 32, C.orange, C.tan); ridge(p, r, 128, 8, 24, C.brown, C.orange);
    for (const x of [44, 190, 224]) { const y = 124 + r.int(8); p.rect(x, y - 16, 4, 16, C.forest); p.rect(x - 5, y - 11, 3, 6, C.forest); p.rect(x - 5, y - 8, 6, 2, C.forest); p.rect(x + 7, y - 13, 3, 6, C.forest); p.rect(x + 3, y - 10, 6, 2, C.forest); p.rect(x + 1, y - 16, 1, 16, C.green); }
    for (let i = 0; i < 70; i++) p.set(r.int(p.w), 100 + r.int(44), r.pick([C.peach, C.tan, C.yellow]));
  },
  mountains(p, r) {
    sky(p, 0, 90, [C.indigo, C.blue, C.sky, C.cyan, C.peach]); stars(p, r, 10, 20);
    const peaks = (base, amp, fill, snow) => { const cx = [r.range(30, 70), r.range(110, 150), r.range(190, 230)]; for (let x = 0; x < p.w; x++) { const h = Math.round(base - Math.max(...cx.map((c) => amp - Math.abs(x - c) * 0.9), 0) - 2 * Math.sin(x / 5 + c0)); for (let y = h; y < p.h; y++) p.set(x, y, snow && y < h + 4 + (x & 1) * 2 ? snow : fill); } };
    const c0 = r.next() * 6;
    peaks(112, 62, C.indigo, C.ice); ridge(p, r, 112, 8, 28, C.slate, C.ash); ridge(p, r, 128, 6, 20, C.forest, C.green);
    for (let x = 6; x < p.w; x += 14 + r.int(8)) pine(p, x, 138, 20, C.night, C.forest);
  },
  sea(p, r) {
    sky(p, 0, 84, [C.indigo, C.violet, C.rose, C.orange, C.peach]); disc(p, 160, 82, 14, C.yellow, C.orange);
    p.rect(0, 84, p.w, 60, C.steel); for (let y = 84; y < p.h; y++) { const t = (y - 84) / 60; for (let x = 0; x < p.w; x++) p.set(x, y, t < 0.18 ? C.orange : t < 0.4 ? C.rose : t < 0.7 ? C.violet : C.indigo); }
    for (let y = 86; y < p.h; y += 2 + Math.floor((y - 86) / 14)) for (let k = 0; k < 6 + Math.floor((y - 86) / 6); k++) { const x = r.int(p.w), w = 3 + r.int(10); p.rect(x, y, w, 1, y < 112 && Math.abs(x - 160) < 40 ? C.yellow : y < 120 ? C.pink : C.sky); }
    p.poly([[40, 100], [40, 70], [60, 100]], C.night); p.poly([[64, 100], [64, 76], [78, 100]], C.night); p.rect(36, 100, 46, 3, C.night); p.rect(54, 66, 1, 36, C.night);
  },
  snow(p, r) {
    sky(p, 0, 84, [C.indigo, C.steel, C.sky, C.ice]); disc(p, 200, 34, 9, C.white, C.ice);
    ridge(p, r, 92, 14, 40, C.ice, C.white); ridge(p, r, 108, 12, 28, C.mist, C.white); ridge(p, r, 126, 7, 20, C.white, C.white);
    for (let x = 10; x < p.w; x += 18 + r.int(10)) pine(p, x, 120 + r.int(14), 20, C.forest, C.ice);
    for (let i = 0; i < 90; i++) p.set(r.int(p.w), r.int(p.h), C.white);
  },
  night(p, r) {
    sky(p, 0, 100, [C.night, C.indigo, C.blue]); stars(p, r, 120, 90); disc(p, 80, 34, 12, C.ice, C.indigo); p.ellipse(86, 31, 10, 10, C.indigo);
    ridge(p, r, 108, 12, 36, C.indigo, C.blue); ridge(p, r, 124, 9, 26, C.night, C.indigo); ridge(p, r, 134, 5, 18, C.black, C.night);
    for (let i = 0; i < 6; i++) p.set(150 + r.int(80), 118 + r.int(18), C.yellow);
  },
};

/** The title, upper-cased and trimmed; the VGA font has CP437, so only a character outside it draws as `?`. */
const clean = (title) => String(title).trim().replace(/\s+/g, ' ');

/** The pixel splash: scene, a dark plate across the upper third, the title centred on it. */
export function drawSplash({ title = 'Title', scene = 'meadow', seed = 'splash' } = {}, { atlas } = {}) {
  if (!SPLASH_SCENES.includes(scene)) throw new Error(`unknown scene "${scene}"; known: ${SPLASH_SCENES.join(', ')}`);
  const text = clean(title); if (!text) throw new Error('a splash needs a title');
  if ([...text].length > SPLASH_MAX_TITLE) throw new RangeError(`"${text}" is ${[...text].length} characters; the most is ${SPLASH_MAX_TITLE} at 8 pixels each`);
  const p = new Pixels(SPLASH_W, SPLASH_H), r = rng(`${seed}:${scene}`);
  SCENE_DRAW[scene](p, r);
  // The plate: solid night across the band, its two edge rows dithered so the scene shows through, a plum rule on the outer rows.
  const py = 14, ph = 28;
  for (let y = py; y < py + ph; y++) for (let x = 0; x < p.w; x++) { const edge = y === py + 1 || y === py + ph - 2; if (!edge || ((x + y) & 1) === 0) p.set(x, y, C.night); }
  p.rect(0, py, p.w, 1, C.plum); p.rect(0, py + ph - 1, p.w, 1, C.plum);
  if (atlas) { const t = pixelText(atlas, text, { ink: C.white, outline: C.black }); p.blit(t, Math.floor((p.w - t.w) / 2), py + Math.floor((ph - t.h) / 2)); }
  return { id: `splash.${scene}`, frames: [p], fps: 0, loop: false, says: `A splash screen: ${scene}, titled "${text}".` };
}

/** The ANSI splash: a vista for the scene, a dark band, the title in the widest block lettering that fits. */
export function drawSplashAnsi({ title = 'Title', scene = 'meadow', seed = 'splash', bgLimit = 8 } = {}) {
  const v = VISTAS[scene]; if (!v) throw new Error(`unknown scene "${scene}"; known: ${SPLASH_SCENES.join(', ')}`);
  const text = [...clean(title).toUpperCase()].filter((ch) => LETTERING_CHARS.includes(ch)).join('').trim();
  if (!text) throw new Error(`the title has none of the characters the lettering font has: ${LETTERING_CHARS}`);
  const w = 80, h = 24, cells = drawVista({ ...v, seed: `${seed}:${scene}`, columns: w, rows: h, bgLimit: 16 }).frames[0].clone();
  let word = null;
  for (const face of ['shaded', 'block', 'half']) { try { word = lettering(text, { face, fg: V.yellow, maxWidth: w - 6 }); break; } catch (e) { if (!(e instanceof RangeError)) throw e; } }
  if (!word) throw new RangeError(`"${text}" does not fit 80 columns in block lettering; shorten the title`);
  const y0 = 2, x0 = Math.floor((w - word.w) / 2);
  for (let y = y0 - 1; y < y0 + word.h + 1; y++) for (let x = 0; x < w; x++) cells.set(x, y, { ch: ' ', bg: V.black });   // the plate: solid black under the letters
  cells.box(0, y0 - 1, w, word.h + 2, { style: 'single', fg: V.blue, bg: V.black }); cells.blit(word, x0, y0);
  // box() drew a frame over the rows' sides; put black behind the letters so none of them shows a scene colour.
  for (let y = 0; y < word.h; y++) for (let x = 0; x < word.w; x++) if (!word.isEmpty(x, y)) cells.set(x0 + x, y0 + y, { bg: V.black });
  return { id: `splash.${scene}`, frames: [legalize(cells, { bgLimit })], fps: 0, loop: false, says: `A splash screen: ${scene}, titled "${text}".` };
}
