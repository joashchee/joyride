// The figure layer (ANSI only): a face or an item picture, each pixel tagged with its part, laid on its place (the room, a shade darker)
// and lit by it with a rim light. Written for docs/coupler-gaps-plan.md Phase F from Coupler's figure tables (`lay`, `lighter`, `shade`,
// `wild`); the code is this package's own. The figure comes from `creatureface` or `thing` (their `figure`: colours and a part per pixel);
// the place is the pixel canvas `vista` draws on, so the place shows through the rim light. Browser-safe.
import { legalize } from '../cells.js';
import { Px, K, WHT, LIT, checkColors, IDENTITY256 } from './vista-canvas.js';
import { drawOutdoors } from './vista-outdoors.js';
import { drawIndoors } from './vista-indoors.js';
import { OUTDOOR, TERRAINS, TIMES, WEATHER_KINDS, VISTA_STYLES, hasSky, roomSeed } from './vista.js';

/** The part of a pixel that is the place (a room's picture is all of it), and the others. Same order as `FACE_PARTS`. */
export const BACK = 0, BODY = 1, HAIR = 2, EYE = 3, MOUTH = 4, WORN = 5, BROW = 6, DETAIL = 7;
/** A colour that is no pixel at all. */
export const CLEAR = 255;
/** One step brighter, for a rim of light: the light edge of a lit figure and the flash (Coupler's `lighter`). Indexed by VGA colour. */
export const LIGHTER = [8, 9, 10, 11, 12, 13, 14, 15, 7, 15, 15, 15, 15, 15, 15, 15];
/** One step darker (Coupler's `shade`). */
export const SHADE = [8, 8, 8, 1, 8, 8, 8, 8, 0, 1, 2, 3, 4, 5, 6, 7];
/** A step back, for a place behind a figure (Coupler's `recede`): bright colours go to their dark self, light gray to dark gray. */
export const RECEDE = [0, 1, 2, 3, 4, 5, 6, 8, 0, 1, 2, 3, 4, 5, 6, 7];
const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/** A colour at night (Coupler's `darken`): bright colours go to their dark self, the dark ones dither with black at 8 of 16, black stays. */
export const darken = (c, x, y) => (c === K ? K : c >= 9 ? c - 8 : BAYER[y & 3][x & 3] < 8 ? K : c);
/** Where the sun may stand when no time says, in thousandths of its arc (Coupler's list). */
export const SUN_ARCS = [150, 250, 750, 850];
/** A colour a step back at 256 and true colour, where the tables above are 16-colour steps: the place a quarter darker, black staying. */
const RECEDED = 0.75;
/** A pixel of the figure one step lighter (the rim) or darker (night) on a smooth layer: a blend, where 16 colours step along a table. */
const rimmed = (layer, c, t) => layer.between(c, WHT, t);
/** Legal cells for a limit at 16 and 256 colours; true colour is read from its RGB and left as it is. */
export const fitted = (cells, colors, bgLimit) => (colors === 'true' ? cells : legalize(cells, { bgLimit }));
const WILD_PLACES = ['woods', 'plains', 'hills'];

/** A pixel canvas with a part on every pixel: the place, a figure laid on it, or a whole picture of two fighters. */
export class Layer extends Px {
  constructor(w, h, depth = 16) { super(w, h, depth); this.parts = new Uint8Array(w * h); this.outdoors = false; this.night = false; }
  clone() { const l = new Layer(this.w, this.h, this.depth); l.a.set(this.a); l.f.set(this.f); l.parts.set(this.parts); l.outdoors = this.outdoors; l.night = this.night; return l; }
}

/**
 * An animal's place (Coupler's `wild`): an outdoors terrain other than city, spaceport and seaport stays as it is; anything else (a room,
 * a town, a port) is one of woods, plains and hills, chosen from the creature's key so the same creature is always in the same one.
 */
export function wildTerrain(terrain, key = '') {
  if (OUTDOOR.includes(terrain) && !['city', 'spaceport', 'seaport'].includes(terrain)) return terrain;
  return WILD_PLACES[Number(BigInt(roomSeed('wild', key, 0)) % 3n)];
}

// The sun's column on the arc, as vista draws it (vista-outdoors.js `body`).
function sunSpan(arc, W, H) { const r = Math.max(2, Math.round(4 * Math.max(0.25, H / 48))); return { x: Math.round(r + (arc / 1000) * (W - 2 * r - 1)), r }; }

/**
 * The picture of a place at a pixel size: `{ terrain, time, weather, style, key, columns, rows, arc, recede, avoid }`. The key seeds it
 * (world "bestiary", room = key). Without a time the sun stands at one of four arcs picked from the key, and never over `avoid` (the
 * columns `[from, to]` of a head): the arc whose sun is furthest from them is taken when none is clear. `recede` (default true) steps
 * every colour back so a figure stands out from it. Returns a `Layer` of the place (all `BACK`, its lit pixels flagged) and whether it
 * is night outdoors.
 */
export function placeLayer({ terrain = 'plains', time, weather = 'clear', style = 'fantasy', key = '', columns, rows, arc, recede = true, avoid, colors = 16 } = {}) {
  checkColors(colors);
  if (!TERRAINS.includes(terrain)) throw new Error(`unknown terrain "${terrain}"; known: ${TERRAINS.join(', ')}`);
  if (time !== undefined && !TIMES.includes(time)) throw new Error(`unknown time "${time}"; known: ${TIMES.join(', ')}`);
  if (!WEATHER_KINDS.includes(weather)) throw new Error(`unknown weather "${weather}"; known: ${WEATHER_KINDS.join(', ')}`);
  if (!VISTA_STYLES.includes(style)) throw new Error(`unknown style "${style}"; known: ${VISTA_STYLES.join(', ')}`);
  const shown = hasSky(terrain) ? { time: time ?? 'day', weather } : { time: 'day', weather: 'clear' }, seed = roomSeed('bestiary', key), outdoors = OUTDOOR.includes(terrain), high = style === 'high';
  if (outdoors && time === undefined && arc === undefined) {
    const first = Number(BigInt(roomSeed('sun', key, 0)) % 4n), mid = avoid ? (avoid[0] + avoid[1]) / 2 : undefined;
    const reach = (a) => { const { x, r } = sunSpan(a, columns, rows * 2); return avoid ? Math.max(0, Math.min(avoid[1] - (x - r), x + r - avoid[0]) + 1) : 0; };
    let best = SUN_ARCS[first], least = reach(best);
    for (let i = 1; i < 4 && least > 0; i++) { const a = SUN_ARCS[(first + i) % 4], o = reach(a); if (o < least || (o === least && mid !== undefined && Math.abs(sunSpan(a, columns, rows * 2).x - mid) > Math.abs(sunSpan(best, columns, rows * 2).x - mid))) { best = a; least = o; } }
    arc = best;
  }
  const map = colors === 16 ? Array.from({ length: 16 }, (_, i) => i) : IDENTITY256;
  const px = outdoors ? drawOutdoors({ terrain, time: shown.time, weather: shown.weather, high, arc, seed, columns, rows, map, depth: colors, raw: true }) : drawIndoors({ terrain, high, seed, columns, rows, map, depth: colors, raw: true });
  const layer = new Layer(columns, rows * 2, colors); layer.a.set(px.a); layer.f.set(px.f);
  if (recede) for (let i = 0; i < layer.a.length; i++) layer.a[i] = colors === 16 ? RECEDE[layer.a[i]] : layer.a[i] === K ? K : layer.scaled(layer.a[i], RECEDED);
  layer.outdoors = outdoors; layer.night = outdoors && hasSky(terrain) && shown.time === 'night';
  return layer;
}

/**
 * Lay a figure on a layer (Coupler's `lay`): for each pixel (x, y) of the layer the figure's pixel is (x + offset.x, y - offset.y). A
 * figure past its right edge repeats its last column where that is body (an animal's back running off the side). Clear and back pixels
 * are skipped, and `hide` (an invisible figure) skips every pixel where x + y is odd. The pixel goes down with its lit flag and part.
 * The figure is `{ w, h, colors, parts, lit? }` as `creatureface` and `thing` return it in `figure`.
 */
export function lay(layer, fig, { offset = { x: 0, y: 0 }, hide = false } = {}) {
  for (let y = 0; y < layer.h; y++) for (let x = 0; x < layer.w; x++) {
    let sx = x + (offset.x ?? 0); const sy = y - (offset.y ?? 0);
    if (sy < 0 || sy >= fig.h || sx < 0) continue;
    if (sx >= fig.w) { if (fig.parts[sy * fig.w + fig.w - 1] !== BODY) continue; sx = fig.w - 1; }
    const j = sy * fig.w + sx, part = fig.parts[j];
    if (part === BACK || fig.colors[j] === CLEAR || (hide && (x + y) & 1)) continue;
    const i = y * layer.w + x; layer.a[i] = fig.colors[j]; layer.parts[i] = part; layer.f[i] = fig.lit?.[j] ? LIT : 0;
  }
  return layer;
}

/**
 * Stamp a figure with its left edge at `x0` and top at `y0` (the Fight panel's standing faces), over what is there, clipped to the layer.
 * `mirror` reverses its rows, the part and lit flag going with each pixel; `flash` makes every pixel of it lighter and gives its own light.
 */
export function stamp(layer, fig, x0, y0 = 0, { mirror = false, flash = false } = {}) {
  for (let fy = 0; fy < fig.h; fy++) for (let fx = 0; fx < fig.w; fx++) {
    const j = fy * fig.w + fx, part = fig.parts[j];
    if (part === BACK || fig.colors[j] === CLEAR) continue;
    const x = x0 + (mirror ? fig.w - 1 - fx : fx), y = y0 + fy;
    if (x < 0 || y < 0 || x >= layer.w || y >= layer.h) continue;
    const i = y * layer.w + x; layer.a[i] = flash ? (layer.smooth ? rimmed(layer, fig.colors[j], 9) : LIGHTER[fig.colors[j]]) : fig.colors[j]; layer.parts[i] = part; layer.f[i] = flash || fig.lit?.[j] ? LIT : 0;
  }
  return layer;
}

/**
 * Light the figures on a layer (Coupler's `light`). Outdoors by day a figure pixel with no figure pixel above it, that is not an eye and
 * not lit takes a lighter colour (the rim); at night outdoors every figure pixel that is not lit is darkened; indoors nothing happens.
 * `outdoors` and `night` default to the layer's own.
 */
export function lightFigures(layer, { outdoors = layer.outdoors, night = layer.night } = {}) {
  if (!outdoors) return layer;
  const { w, h, parts } = layer;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = y * w + x, part = parts[i];
    if (part === BACK || layer.f[i] & LIT) continue;
    if (layer.smooth) {
      // a blend in place of the dither and the step: night takes the colour down to half, the rim light falls off over two rows
      if (night) layer.a[i] = layer.a[i] === K ? K : layer.scaled(layer.a[i], 0.5);
      else if (part !== EYE) { const one = y === 0 || parts[i - w] === BACK, two = !one && (y === 1 || parts[i - 2 * w] === BACK); if (one || two) layer.a[i] = rimmed(layer, layer.a[i], one ? 7 : 3); }
      continue;
    }
    if (night) layer.a[i] = darken(layer.a[i], x, y);
    else if (part !== EYE && (y === 0 || parts[i - w] === BACK)) layer.a[i] = LIGHTER[layer.a[i]];
  }
  return layer;
}

/** The columns a figure fills in its upper half: where its head is, so the sun is not put behind it. */
export function headSpan(fig) {
  let from = fig.w, to = -1;
  for (let y = 0; y < fig.h >> 1; y++) for (let x = 0; x < fig.w; x++) if (fig.parts[y * fig.w + x] !== BACK) { from = Math.min(from, x); to = Math.max(to, x); }
  return to < 0 ? undefined : [from, to];
}

/**
 * A figure on its place, as cells: `figureOnPlace(figure, place)` with `place` as `placeLayer` takes it (`terrain`, `time`, `weather`,
 * `style`, `key`, `arc`, `recede`) plus `wild` (the creature is an animal: its place is `wildTerrain`), `offset` and `invisible`. The
 * figure's own colours go down over the place, then the rim light (or the night) of the place falls on them: a face on woods at dusk and on
 * stone read differently. Returns `{ cells, layer }`.
 */
export function figureOnPlace(fig, place = {}, { bgLimit = 8, colors = fig.depth ?? 16 } = {}) {
  checkColors(colors);
  const { wild = false, offset, invisible = false, ...rest } = place;
  const terrain = wild ? wildTerrain(rest.terrain ?? 'plains', rest.key ?? '') : rest.terrain ?? 'plains';
  const layer = placeLayer({ ...rest, terrain, columns: fig.w, rows: fig.h >> 1, avoid: headSpan(fig), colors });
  lay(layer, fig, { offset, hide: invisible });
  lightFigures(layer);
  return { cells: fitted(layer.toCells(colors === 16 ? Array.from({ length: 16 }, (_, i) => i) : IDENTITY256), colors, bgLimit), layer, terrain };
}
