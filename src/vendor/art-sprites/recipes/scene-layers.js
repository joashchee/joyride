// The extra layers of a `scene` (the biomes kit's "biome-layers" and "readability" rows). Weather, inhabitants and story evidence
// are the three biome layers the BIOMES table did not carry; depth is the readability layer (background, midground, foreground).
// Everything draws from its own forked rng, so a scene without layers is byte-identical to what it was.
import { Pixels } from '../pixels.js';
import { C, ramp } from '../palette.js';

const T = 16;
/** Per biome: its weather, the people or animals that live there, and the story it tells (props and marks on the ground). */
export const SCENE_LAYERS = {
  forest: { weather: 'fog', folk: { kind: 'beast', n: 1 }, evidence: ['prints', 'gravecross'] },
  cave: { weather: 'drip', folk: { kind: 'bats', n: 3 }, evidence: ['scorch', 'skull'] },
  dungeon: { weather: 'drip', folk: { kind: 'bats', n: 2 }, evidence: ['scorch', 'bones'] },
  desert: { weather: 'dust', folk: { kind: 'folk', n: 2, cloth: 'sand' }, evidence: ['prints', 'ribs'] },
  village: { weather: 'rain', folk: { kind: 'folk', n: 3, cloth: 'blue' }, evidence: ['ruts', 'gravecross'] },
  swamp: { weather: 'fog', folk: { kind: 'birds', n: 3, color: 'night' }, evidence: ['bones', 'thorns'] },
  coast: { weather: 'rain', folk: { kind: 'birds', n: 4, color: 'white' }, evidence: ['prints', 'ribs'] },
  magical: { weather: 'motes', folk: { kind: 'birds', n: 3, color: 'pink' }, evidence: ['scorch', 'crystal'] },
  mountain: { weather: 'snow', folk: { kind: 'birds', n: 2, color: 'night' }, evidence: ['prints', 'bones'] },
  snow: { weather: 'snow', folk: { kind: 'beast', n: 1, coat: 'white' }, evidence: ['prints', 'gravecross'] },
  jungle: { weather: 'rain', folk: { kind: 'birds', n: 4, color: 'red' }, evidence: ['ruts', 'skull'] },
  road: { weather: 'dust', folk: { kind: 'folk', n: 3, cloth: 'leather' }, evidence: ['ruts', 'tombstone'] },
  plains: { weather: 'dust', folk: { kind: 'beast', n: 2 }, evidence: ['ruts', 'bones'] },
  canopy: { weather: 'fog', folk: { kind: 'birds', n: 3, color: 'green' }, evidence: ['prints', 'skull'] },
  castle: { weather: 'rain', folk: { kind: 'folk', n: 3, cloth: 'red' }, evidence: ['ruts', 'scorch'] },
  town: { weather: 'rain', folk: { kind: 'folk', n: 4, cloth: 'green' }, evidence: ['ruts', 'prints'] },
  harbor: { weather: 'fog', folk: { kind: 'birds', n: 4, color: 'white' }, evidence: ['ruts', 'barrel'] },
  river: { weather: 'rain', folk: { kind: 'birds', n: 3, color: 'white' }, evidence: ['prints', 'bones'] },
  ruins: { weather: 'dust', folk: { kind: 'birds', n: 2, color: 'night' }, evidence: ['scorch', 'skull', 'ribs'] },
  mines: { weather: 'dust', folk: { kind: 'bats', n: 3 }, evidence: ['ruts', 'bones'] },
  highlands: { weather: 'snow', folk: { kind: 'beast', n: 2, coat: 'bone' }, evidence: ['prints', 'gravecross'] },
  skyisle: { weather: 'motes', folk: { kind: 'birds', n: 4, color: 'pink' }, evidence: ['scorch', 'crystal'] },
};
export const WEATHER = ['rain', 'snow', 'fog', 'dust', 'motes', 'drip'];

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const bayer = (x, y) => (BAYER[(y & 3) * 4 + (x & 3)] + 0.5) / 16;

/** Whether a layer is wanted: `layers: true` turns every one on, a layer's own param (or `false`) overrides. */
export function wants(opt, layers) { return opt === undefined ? !!layers : !!opt; }

/** A free tile off the path and clear of the wall rows and the landmark, or null. */
function freeTile(ctx, r) {
  for (let i = 0; i < 24; i++) {
    const x = r.int(ctx.cols - 3), y = r.range(ctx.wallRows, ctx.rows - 2);
    if (Math.abs(y - ctx.path[x]) <= 1 || ctx.taken.has(`${x},${y}`)) continue;
    ctx.taken.add(`${x},${y}`); return [x, y];
  }
  return null;
}

/** Story evidence: marks along the path (prints, ruts) and on the ground (scorch), and a prop or two that say what happened here. */
export function drawEvidence(ctx, kinds, r) {
  const { out, path, cols, b, drawProp } = ctx;
  const pathInk = ramp(b.path)[0], groundInk = ramp(b.ground)[0];
  for (const kind of kinds) {
    if (kind === 'prints') {
      if (b.path === 'water') continue;
      const x0 = r.int(3);
      for (let x = x0; x < cols - 2; x += 3) for (let k = 0; k < 2; k++) {
        const px = x * T + 3 + k * 5, py = path[x] * T + (k ? 6 : 9);
        out.set(px, py, pathInk).set(px + 1, py, pathInk).set(px, py + 1, pathInk);
      }
    } else if (kind === 'ruts') {
      if (b.path === 'water') continue;
      const x0 = r.int(2), x1 = cols - 3 - r.int(2);
      for (let x = x0 * T; x < x1 * T; x++) {
        if (x % 6 > 3) continue;
        const py = path[Math.min(cols - 1, Math.floor(x / T))] * T;
        out.set(x, py + 5, pathInk).set(x, py + 10, pathInk);
      }
    } else if (kind === 'scorch') {
      const t = freeTile(ctx, r); if (!t) continue;
      const cx = t[0] * T + 8, cy = t[1] * T + 9;
      for (let y = -5; y <= 5; y++) for (let x = -7; x <= 7; x++) {
        if ((x * x) / 49 + (y * y) / 25 > 1 || (x + y + 20) % 2) continue;
        if (out.get(cx + x, cy + y)) out.set(cx + x, cy + y, groundInk);
      }
      out.set(cx - 1, cy, C.ash).set(cx + 2, cy + 1, C.ash).set(cx, cy - 2, C.dust);
    } else {
      const t = freeTile(ctx, r); if (!t) continue;
      out.blit(drawProp(kind), t[0] * T, t[1] * T - 2);
    }
  }
}

const folkClothes = ['red', 'blue', 'green', 'leather', 'purple'];
/** A walker, 6 by 10: head, tunic, legs. */
function walker(cloth, skin) {
  const p = new Pixels(6, 10), c = ramp(cloth);
  p.rect(1, 0, 4, 1, C.plum).rect(1, 1, 4, 3, skin).set(2, 2, C.night).set(4, 2, C.night);
  p.rect(0, 4, 6, 4, c[2]).rect(0, 4, 6, 1, c[3]).rect(0, 7, 6, 1, c[1]).rect(2, 5, 2, 1, c[3]);
  p.rect(1, 8, 2, 2, C.umber).rect(3, 8, 2, 2, C.umber);
  return p;
}
/** A four-legged animal, 10 by 7. */
function beast(coat) {
  const p = new Pixels(10, 7), c = ramp(coat);
  p.rect(2, 1, 6, 3, c[2]).rect(2, 1, 6, 1, c[3]).rect(2, 3, 6, 1, c[1]).rect(7, 0, 3, 3, c[2]).set(9, 1, C.night).set(8, 0, c[1]).set(7, 0, c[1]);
  p.rect(2, 4, 1, 3, c[1]).rect(4, 4, 1, 3, c[0]).rect(6, 4, 1, 3, c[1]).rect(7, 4, 1, 3, c[0]).set(1, 1, c[1]).set(0, 2, c[1]);
  return p;
}
/** Wings from a distance: a V, 5 by 2. */
function wings(color, flap) {
  const p = new Pixels(5, 2), v = C[color];
  if (flap) p.set(0, 1, v).set(1, 0, v).set(2, 1, v).set(3, 0, v).set(4, 1, v); else p.set(0, 0, v).set(1, 1, v).set(2, 1, v).set(3, 1, v).set(4, 0, v);
  return p;
}

/** The inhabitants: walkers on the path, animals on the ground, birds above and bats under the wall rows. */
export function drawInhabitants(ctx, spec, r) {
  const { out, path, cols, wallRows } = ctx;
  for (let i = 0; i < spec.n; i++) {
    if (spec.kind === 'folk') {
      const x = 1 + ((i * 3 + r.int(2)) % (cols - 3)), skin = ramp(r.pick(['skin-fair', 'skin-light', 'skin-medium', 'skin-deep']))[2];
      out.blit(walker(i === 0 && spec.cloth ? spec.cloth : r.pick(folkClothes), skin), x * T + r.range(2, 8), path[x] * T + 3);
    } else if (spec.kind === 'beast') {
      const t = freeTile(ctx, r); if (!t) continue;
      out.blit(beast(spec.coat ?? r.pick(['leather', 'bone', 'dirt'])), t[0] * T + 3, t[1] * T + 6);
    } else if (spec.kind === 'bats') {
      out.blit(wings('night', i & 1), r.range(8, cols * T - 16), wallRows * T + r.range(2, 22));
    } else {
      out.blit(wings(spec.color ?? 'night', i & 1), r.range(10, cols * T - 18), r.range(3, 22));
    }
  }
}

/** Weather laid over the finished scene. Sparse on purpose: the path and the landmark must still read through it. */
export function drawWeather(out, kind, r) {
  const { w, h } = out, area = (w * h) / 256;
  if (kind === 'rain') {
    const ink = ramp('water')[2];
    for (let i = 0; i < area * 1.6; i++) { const x = r.int(w), y = r.int(h - 3); out.set(x, y, ink).set(x - 1, y + 1, ink).set(x - 1, y + 2, ink); }
  } else if (kind === 'snow') {
    const ink = ramp('white')[3];
    for (let i = 0; i < area * 2; i++) { const x = r.int(w), y = r.int(h); out.set(x, y, ink); if (r.chance(0.25)) out.set(x + 1, y, ink).set(x, y + 1, ink); }
  } else if (kind === 'fog') {
    const ink = ramp('white')[1];
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) if (bayer(x, y) < 0.34 * (1 - y / h) + 0.06 + 0.06 * Math.sin((x + y * 2) / 9)) out.set(x, y, ink);
  } else if (kind === 'dust') {
    const ink = ramp('sand')[2];
    for (let i = 0; i < area * 0.9; i++) { const x = r.int(w - 4), y = r.int(h), n = r.range(2, 4); for (let k = 0; k < n; k++) out.set(x + k, y, ink); }
  } else if (kind === 'motes') {
    for (let i = 0; i < area * 0.8; i++) {
      const x = r.int(w), y = r.int(h), ink = r.pick([C.cyan, C.white, C.pink]);
      out.set(x, y, ink); if (r.chance(0.2)) out.set(x - 1, y, ink).set(x + 1, y, ink).set(x, y - 1, ink).set(x, y + 1, ink);
    }
  } else if (kind === 'drip') {
    const ink = ramp('water')[2];
    for (let i = 0; i < area * 0.3; i++) { const x = r.int(w), y = r.int(h - 2); out.set(x, y, ink).set(x, y + 1, ink); }
  } else throw new Error(`unknown weather "${kind}"`);
}

const half = (src) => { const p = new Pixels(src.w >> 1, src.h >> 1); for (let y = 0; y < p.h; y++) for (let x = 0; x < p.w; x++) p.set(x, y, src.get(x * 2, y * 2)); return p; };
const twice = (src, ink) => { const p = new Pixels(src.w * 2, src.h * 2); for (let y = 0; y < p.h; y++) for (let x = 0; x < p.w; x++) if (src.get(x >> 1, y >> 1)) p.set(x, y, ink); return p; };

/**
 * Depth: a background of small far props lost in haze along the top, the midground as drawn, and a foreground of dark, oversized
 * shapes cropped at the bottom corners under a shadow dither, so the eye reads far to near and the path stays clear.
 */
export function drawDepth(ctx, r) {
  const { out, cols, rows, b, wallRows, drawProp } = ctx, W = cols * T, H = rows * T;
  if (!wallRows) for (let i = 0; i < 4; i++) {
    const far = half(drawProp(r.pick(b.props))); out.blit(far, r.range(2, W - 12), r.range(0, 3));
  }
  const haze = ramp('white')[1];
  for (let y = 0; y < 14; y++) for (let x = 0; x < W; x++) if (bayer(x, y) < 0.5 * (1 - y / 14)) out.set(x, y, haze);
  const shade = ramp(b.ground)[0];
  for (let y = H - 8; y < H; y++) for (let x = 0; x < W; x++) if (bayer(x, y) < 0.5 * ((y - (H - 8)) / 8)) out.set(x, y, shade);
  for (const left of [true, false]) {
    const big = twice(drawProp(r.pick(b.props)), C.night), bw = big.w, ox = left ? -r.int(6) : W - bw + r.int(6);
    out.blit(big, ox, H - 14);
  }
}
