// The first native ANSI recipes: they draw on cells, in the artists' order (ansi-craft.js `paint`), with the shade ladders and
// glyph overlays, and are made for 8 backgrounds so a classic .ans needs no legalizing. Each returns the Art contract with Cells frames.
// A banner and a menu are the kit's logo and menu-screen briefs, the landscape its shaded sky, the bust its half-block portrait.
import { Cells, NONE, rampCells, legalize, foldPixels } from '../cells.js';
import { VGA16, V } from '../palette-ansi.js';
import { Pixels } from '../pixels.js';
import { paint, runSteps, overlay, lettering, LETTERING_FACES } from '../ansi-craft.js';
import { rng } from '../rng.js';

const art = (id, cells, extra = {}) => ({ id, frames: [legalize(cells, { bgLimit: 8 })], fps: 0, loop: false, ...extra });
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const centre = (cells, x0, w, y, src) => cells.blit(src, x0 + Math.floor((w - src.w) / 2), y);

export const BANNER_FACES = LETTERING_FACES;
/** Block lettering of a text in one of the faces. */
export function drawBanner({ text = 'ANSI', face = 'block', fg = V.yellow } = {}) {
  return art(`banner.${face}`, lettering(text, { face, fg }));
}

// --- landscape ---------------------------------------------------------------------------------------------------------------------
// Sky ramps by time (VGA indexes, top to horizon), all with legal steps; ground runs from the horizon down.
const SKIES = { day: [V.blue, V.brightblue, V.cyan, V.brightcyan], dusk: [V.blue, V.magenta, V.red, V.brown, V.yellow], night: [V.black, V.blue, V.brightblue] };
export const TIMES = Object.keys(SKIES);
export const WEATHERS = ['clear', 'rain', 'snow'];

/** Smooth-ish ridge heights in half rows: a sum of two seeded sines and some steps, `lo` to `hi`. */
const ridge = (w, r, lo, hi, scale) => {
  const a = r.next() * 6, b = r.next() * 6, f1 = scale * (0.8 + r.next() * 0.4), f2 = f1 * 2.3;
  return Array.from({ length: w }, (_, x) => Math.round(lo + (hi - lo) * clamp(0.5 + 0.32 * Math.sin(x * f1 + a) + 0.18 * Math.sin(x * f2 + b), 0, 1)));
};

/** A ramp thinned to what `rows` rows can show: a ladder takes four rows a colour pair, so fewer rows take fewer colours, evenly spread. */
const fit = (list, rows) => {
  const n = clamp(Math.floor((rows - 3) / 4) + 1, 2, list.length);
  return Array.from({ length: n }, (_, i) => list[Math.round((i * (list.length - 1)) / (n - 1))]);
};

/** The sky colour at a cell for the ridge cap: the glyph's dominant colour as a plain background. */
const skyBg = (c) => (c.ch === 0x2588 ? c.fg : c.bg);

/** A landscape: a banded sky by time of day, a sun or moon, two mountain ranges, a lake and a green ground, weather over it. */
export function drawLandscape({ time = 'day', weather = 'clear', seed = 'landscape', w = 80, h = 24, lake = true } = {}) {
  if (!SKIES[time]) throw new Error(`unknown time "${time}"; known: ${TIMES.join(', ')}`);
  if (!WEATHERS.includes(weather)) throw new Error(`unknown weather "${weather}"; known: ${WEATHERS.join(', ')}`);
  const r = rng(`${seed}:${time}`), cells = new Cells(w, h), horizon = Math.round(h * 0.62);
  const skyRows = horizon, sky = rampCells(fit(SKIES[time], skyRows)), ground = rampCells(fit([V.brightgreen, V.green, V.darkgray, V.black], h - horizon));
  const cell = (x, y) => cells.get(x, y);
  const inLake = new Set(), wob = Array.from({ length: Math.ceil(w / 5) }, () => (r.next() - 0.5) * 1.2);
  return paint(cells, {
    focal: () => ({ x: Math.round(w * (0.25 + 0.5 * r.next())), y: time === 'dusk' ? horizon - 8 : Math.round(h * 0.17) }),
    base: ({ cells: c, focal }) => {
      for (let y = 0; y < skyRows; y++) for (let x = 0; x < w; x++) c.set(x, y, sky[clamp(Math.round(((y + wob[Math.floor(x / 5)]) / (skyRows - 1)) * (sky.length - 1)), 0, sky.length - 1)]);
      // The sun, or the moon: a block disc with a halo of 25 percent shade.
      const lamp = time === 'night' ? V.white : V.yellow, rx = time === 'night' ? 3 : 4, ry = time === 'night' ? 1.6 : 2;
      for (let y = focal.y - 4; y <= focal.y + 4; y++) for (let x = focal.x - 9; x <= focal.x + 9; x++) {
        const d = ((x - focal.x) / (rx + 0.5)) ** 2 + ((y - focal.y) / (ry + 0.5)) ** 2, e = ((x - focal.x) / (rx * 2 + 1)) ** 2 + ((y - focal.y) / (ry * 2 + 0.5)) ** 2;
        if (!c.inb(x, y) || y >= skyRows) continue;
        if (d <= 1) c.set(x, y, { ch: '█', fg: lamp, bg: NONE });
        else if (e <= 1 && time !== 'night') c.set(x, y, { ch: '░', fg: lamp, bg: skyBg(cell(x, y)) });
      }
      // Two ranges: the far one lit gray with dark gray slopes, the near hills green. A cap cell is a half block over the sky.
      const range = (heights, lit, dark, baseRow) => {
        for (let x = 0; x < w; x++) {
          const top = baseRow * 2 - heights[x], shadowed = heights[Math.min(w - 1, x + 3)] < heights[x];
          for (let y = Math.floor(top / 2); y <= baseRow; y++) {
            const color = shadowed ? dark : lit, full = y * 2 >= top;
            if (y >= skyRows + 2) continue;
            if (full) c.set(x, y, { ch: '█', fg: color, bg: NONE });
            else c.set(x, y, { ch: '▄', fg: color, bg: skyBg(cell(x, y)) });
          }
        }
      };
      range(ridge(w, r.fork('far'), 4, Math.round(h * 0.55), 0.13), V.gray, V.darkgray, horizon - 1);
      range(ridge(w, r.fork('near'), 1, Math.round(h * 0.2), 0.21), V.green, V.darkgray, horizon);
      for (let y = horizon; y < h; y++) for (let x = 0; x < w; x++) c.set(x, y, ground[clamp(Math.round(((y - horizon) / (h - horizon - 1)) * (ground.length - 1)), 0, ground.length - 1)]);
    },
    highlights: ({ cells: c }) => {
      if (time === 'night') overlay(c, 'stars', seed, { region: { x: 0, y: 0, w, h: Math.max(1, Math.round(horizon * 0.6)) }, density: 0.05 });
    },
    shadows: ({ cells: c, focal }) => {
      if (!lake) return;
      const cx = clamp(w - focal.x, 14, w - 14), cy = Math.round((horizon + h) / 2), rx = Math.round(w * 0.18), ry = Math.max(1, Math.round((h - horizon) * 0.3));
      for (let y = cy - ry; y <= cy + ry; y++) for (let x = cx - rx; x <= cx + rx; x++)
        if (((x - cx) / rx) ** 2 + ((y - cy) / (ry + 0.5)) ** 2 <= 1) { inLake.add(y * w + x); c.set(x, y, { ch: ' ', fg: V.blue, bg: V.blue }); }
    },
    blend: ({ cells: c }) => {
      if (lake) overlay(c, 'waves', seed, { region: { x: 0, y: horizon, w, h: h - horizon }, density: 0.22, where: (x, y) => inLake.has(y * w + x) });
      if (weather === 'rain') overlay(c, 'rain', seed, { density: 0.06 });
      if (weather === 'snow') overlay(c, 'snow', seed, { density: 0.05 });
    },
  }) && art(`landscape.${time}`, cells);
}

// --- weather loop ------------------------------------------------------------------------------------------------------------------
export const LOOP_WEATHERS = ['rain', 'snow'];
/**
 * A landscape with rain or snow falling, as a seamless loop (ANSI only; the first frame is the still). The weather is one seeded
 * overlay that slides down and wraps, `frames` times over the height, so the last frame leads back to the first; it falls only over
 * cells that show no glyph, so the picture under it is the same in every frame and a delta redraws just the falling glyphs.
 */
export function drawWeatherLoop({ time = 'night', weather = 'snow', frames, seed = 'weatherloop', w = 40, h = 16 } = {}) {
  if (!LOOP_WEATHERS.includes(weather)) throw new Error(`unknown weather "${weather}"; known: ${LOOP_WEATHERS.join(', ')}`);
  const n = frames ?? (weather === 'rain' ? 4 : 8);
  if (!Number.isInteger(n) || n < 2 || h % n) throw new RangeError(`frames must be a whole number of 2 or more that divides the height ${h}, not ${n}`);
  const base = drawLandscape({ time, weather: 'clear', seed, w, h }).frames[0], layer = overlay(new Cells(w, h), weather, seed, { density: weather === 'rain' ? 0.06 : 0.05 });
  const fall = h / n, clip = Array.from({ length: n }, (_, k) => {
    const f = base.clone();
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const src = layer.get(x, ((y - k * fall) % h + h) % h), under = base.get(x, y);
      if (src.ch && (!under.ch || under.ch === 0x20)) f.set(x, y, { ch: src.ch, fg: src.fg });
    }
    return legalize(f, { bgLimit: 8 });
  });
  return { id: `weatherloop.${weather}`, frames: [clip[0]], fps: 0, loop: false, clips: { loop: { id: 'loop', frames: clip, fps: 6, loop: true, events: [], reduced: [clip[0]] } } };
}

// --- menu screen -------------------------------------------------------------------------------------------------------------------
/** A menu screen, 80 by 24: a double frame, a shaded title, a boxed list with one item selected, and a runic backdrop. */
export function drawMenu({ title = 'ANSI', items = ['New game', 'Load game', 'Options', 'Credits', 'Quit'], selected = 0, seed = 'menu' } = {}) {
  const w = 80, h = 24, cells = new Cells(w, h);
  if (selected < 0 || selected >= items.length) throw new RangeError(`selected ${selected} is not one of the ${items.length} items`);
  const wide = Math.max(...items.map((s) => s.length)) + 8, x0 = Math.floor((w - wide) / 2), y0 = 9;
  paint(cells, {
    outline: ({ stamp }) => stamp(new Cells(w, h).box(0, 0, w, h, { style: 'double', fg: V.cyan })),
    base: ({ cells: c, fill }) => {
      fill({ ch: ' ', bg: V.black });
      overlay(c, 'runes', seed, { density: 0.03 });
    },
    highlights: ({ cells: c }) => {
      centre(c, 0, w, 2, lettering(title, { face: 'shaded', fg: V.brightcyan, maxWidth: w - 4 }));
      c.box(x0, y0, wide, items.length + 2, { style: 'single', fg: V.blue });
      items.forEach((s, i) => c.text(x0 + 2, y0 + 1 + i, `${i + 1}  ${s}`.padEnd(wide - 4), i === selected ? V.white : V.gray, i === selected ? V.blue : V.black));
      const help = 'Up and Down move, Enter chooses';
      c.text(Math.floor((w - help.length) / 2), h - 3, help, V.gray, V.black);
    },
  });
  return art('menu', cells);
}

// --- half-block portrait -----------------------------------------------------------------------------------------------------------
// Skin as three values (shadow, mid, light), hair and cloth as VGA indexes; every colour is a legal VGA16 colour.
const SKINS = { warm: [V.red, V.brown, V.brightred], pale: [V.darkgray, V.gray, V.white], deep: [V.black, V.red, V.brown], green: [V.darkgray, V.green, V.brightgreen] };
const HAIRS = { brown: [V.black, V.red, V.brown], black: [V.black, V.black, V.darkgray], gold: [V.brown, V.yellow, V.yellow], gray: [V.darkgray, V.gray, V.white] };
export const SKIN_IDS = Object.keys(SKINS), HAIR_IDS = Object.keys(HAIRS), LIGHTS = ['left', 'right'];

/** A head and shoulders, 80 by 48 pixels folded to 80 by 24 cells, in three values lit from one side with a dark gray outline. */
export function drawBust({ skin = 'warm', hair = 'brown', light = 'left', cloth = V.blue, seed = 'bust' } = {}) {
  if (!SKINS[skin]) throw new Error(`unknown skin "${skin}"; known: ${SKIN_IDS.join(', ')}`);
  if (!HAIRS[hair]) throw new Error(`unknown hair "${hair}"; known: ${HAIR_IDS.join(', ')}`);
  if (!LIGHTS.includes(light)) throw new Error(`unknown light "${light}"; known: ${LIGHTS.join(', ')}`);
  const r = rng(`${seed}:${skin}`), W = 80, H = 48, px = new Pixels(W, H, VGA16), side = light === 'left' ? -1 : 1;
  const col = (i) => i + 1, [sd, sm, sl] = SKINS[skin], [hd, hm, hl] = HAIRS[hair];
  // Value of a point on an ellipse: the light side is lit, the far side shadowed, the middle plain.
  const shadeOn = (cx, cy, rx, ry, [d, m, l], edge = 0.5) => (x, y) => {
    const nx = (x + 0.5 - cx) / rx, ny = (y + 0.5 - cy) / ry, t = nx * side - ny * 0.15;
    return t > edge ? l : t < -edge ? d : m;
  };
  const fillEllipse = (cx, cy, rx, ry, pick) => {
    for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++) for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++)
      if (((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1) px.set(x, y, col(pick(x, y)));
  };
  const cx = 40;
  const cy = 21 + r.int(2), tilt = r.range(-1, 1);
  runSteps({
    base: () => {
      fillEllipse(cx, 56, 34, 20, shadeOn(cx, 56, 34, 20, [cloth === V.blue ? V.blue : cloth, cloth === V.blue ? V.brightblue : cloth, V.cyan]));
      px.rect(cx - 6, cy + 14, 12, 12, col(sd));                                                      // neck, in shadow under the jaw
      fillEllipse(cx, cy, 14, 17, shadeOn(cx, cy, 14, 17, [sd, sm, sl]));                              // the head, three values
    },
    highlights: () => {
      // Hair: a cap over the top of the head with a lit side.
      for (let y = cy - 20; y <= cy - 5; y++) for (let x = cx - 16; x <= cx + 16; x++) {
        const nx = (x + 0.5 - cx) / 16, ny = (y + 0.5 - (cy - 4)) / 17;
        if (nx * nx + ny * ny > 1 || y > cy - 7 + Math.round(Math.abs(nx) * 6) || ((x + 0.5 - cx) / 14) ** 2 + ((y + 0.5 - cy) / 17) ** 2 > 1.6) continue;
        px.set(x, y, col(nx * side > 0.2 ? hl : nx * side < -0.3 ? hd : hm));
      }
      // Eyes: white with a dark pupil, brows above them.
      for (const e of [-1, 1]) {
        const ex = cx + e * 6 + tilt;
        px.rect(ex - 2, cy - 1, 5, 2, col(V.gray)); px.rect(ex - 1 + (e === side ? 0 : 1), cy - 1, 2, 2, col(V.black));
        px.rect(ex - 2, cy - 4, 5, 1, col(hd));
      }
      px.rect(cx - 4, cy + 9, 9, 1, col(V.red)); px.rect(cx - 3, cy + 10, 7, 1, col(V.brightred));  // mouth
    },
    shadows: () => {
      px.rect(cx + side * 2, cy + 2, 1, 5, col(sd)); px.rect(cx - 1, cy + 7, 3, 1, col(sd));          // nose, on the shadow side
      px.rect(cx - 10, cy + 15, 20, 2, col(sd));                                                       // jaw shadow onto the neck
    },
  });
  // Dark gray is the outline, unless a white value would sit right on it (dark gray cannot step up to white): then black.
  px.outline(col([sl, hl].includes(V.white) ? V.black : V.darkgray));
  return art('bust', foldPixels(px, { bgLimit: 8 }));
}

// --- artpack piece -----------------------------------------------------------------------------------------------------------------
/** A whole piece with SAUCE: a title in half-block lettering, a landscape in a double frame, a credit line. */
export function drawArtpack({ title = 'ARTPACK', time = 'dusk', weather = 'clear', author = 'art-sprites', seed = 'artpack' } = {}) {
  const w = 80, h = 24, cells = new Cells(w, h);
  paint(cells, {
    outline: ({ stamp }) => stamp(new Cells(w, h).box(0, 0, w, h, { style: 'double', fg: V.cyan }).box(1, 4, w - 2, 18, { style: 'single', fg: V.blue })),
    base: ({ cells: c, fill }) => {
      fill({ ch: ' ', bg: V.black });
      c.blit(drawLandscape({ time, weather, seed, w: 76, h: 16 }).frames[0], 2, 5);
    },
    highlights: ({ cells: c }) => {
      centre(c, 0, w, 1, lettering(title, { face: 'half', fg: V.brightcyan, maxWidth: w - 4 }));
      const credit = `${title.toLowerCase()} :: ${time} ${weather} :: ${author}`;
      c.text(Math.floor((w - credit.length) / 2), h - 2, credit, V.gray, V.black);
    },
  });
  return art('artpack', cells, { sauce: { title: title.slice(0, 35), author: author.slice(0, 20), group: '', font: 'IBM VGA' } });
}
