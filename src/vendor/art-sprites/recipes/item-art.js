// The item picture (ANSI only): one thing alone on a table, 25 by 24 pixels (two to a cell, so 25 columns by 12 rows, the last row its slot in
// words), drawn from its name, material, wear and auras. Written for docs/coupler-gaps-plan.md Phase D from Coupler's item tables
// (recipes/item-data.js); the painters are this package's own. The kind is the first word of the name that names one; the shape follows the
// kind and the cues in the name (scimitar curved, rapier thin, claymore long and broad); the material's class gives the texture (cloth
// woven, leather stitched, wood grained, metal with a band of light, mithril and gems sparkling, glass see-through, rock speckled); wear
// and auras are drawn over it. A painter lays down parts (main, metal, wood, trim, gem) and the finish colours them, so every kind gets
// every material the same way. Each pixel carries a part in `figure`, for the figure layer. Browser-safe.
import { CLEAR, fitted } from './figure.js';
import { Px, checkColors, IDENTITY256, vnoise } from './vista-canvas.js';
import { volume } from './smooth.js';
import { rng, hashSeed } from '../rng.js';
import { ITEM_KINDS, MATERIAL_WORDS, MATERIALS, SHAPE_CUES, AURAS } from './item-data.js';
import { COLOR_WORDS } from './face-data.js';

export const THING_KINDS = ITEM_KINDS.map((k) => k[0]);
/** The classes a material draws as, each with its texture; every other class in the material table (vegetation, flesh, liquid, ...) draws smooth as `plain`. */
export const THING_CLASSES = ['cloth', 'glass', 'leather', 'metal', 'mithril', 'plain', 'precious', 'rock', 'wooden'];
export const THING_WEARS = ['sound', 'worn', 'damaged', 'battered', 'broken', 'rusty', 'crude', 'fine'];
export const THING_AURAS = ['glowing', 'glowing red', 'glowing blue', 'glowing purple', 'glowing white', 'magical aura', 'metallic aura', 'mithril aura', 'invisible', 'hidden', 'floating', 'flying', 'bound'];

// VGA indexes (the order of VGA16).
const K = 0, RED = 1, GRN = 2, BRN = 3, BLU = 4, MAG = 5, CYN = 6, GRY = 7, DGR = 8, LRED = 9, LGRN = 10, YEL = 11, LBLU = 12, LMAG = 13, LCYN = 14, WHT = 15;
const DARK_ = [0, 0, 0, 0, 0, 0, 0, 8, 0, 1, 2, 3, 4, 5, 6, 7], LIGHT = [8, 9, 10, 11, 12, 13, 14, 15, 7, 15, 15, 15, 15, 15, 15, 15];
const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
const W = 25, H = 24, TABLE = 17, LABEL = 22;
// What a pixel is: the painters write parts and the finish chooses colours.
const BACK = 0, MAIN = 1, METAL = 2, WOOD = 3, TRIM = 4, GEM = 5, FIXED = 6, LINE = 7;
const BODY = 1, WORN = 5, DETAIL = 7;                                    // the figure layer's part names (face.js FACE_PARTS)
export const THING_PARTS = ['back', 'body', 'hair', 'eye', 'mouth', 'worn', 'brow', 'detail'];

const key = (s) => String(s ?? '').toLowerCase().replace(/[^a-z]/g, '');
const words = (s) => String(s ?? '').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

const KIND_INFO = new Map(ITEM_KINDS.map(([kind, cls, main, metal, wood, slot, ws]) => [kind, { kind, cls, main, metal, wood, slot, words: ws }]));
KIND_INFO.set('Bundle', { kind: 'Bundle', cls: 'cloth', main: BRN, metal: GRY, wood: BRN, slot: 'Carried', words: [] });   // what an unknown first word falls back to
const KIND_BY_WORD = new Map(); for (const k of KIND_INFO.values()) for (const w of k.words) if (!KIND_BY_WORD.has(w)) KIND_BY_WORD.set(w, k.kind);
const MATERIAL_TABLE = new Map(MATERIALS.split(';').map((e) => { const [name, rest] = e.split('='), [cls, color] = rest.split(','); return [name, { cls, color: color === '' ? undefined : +color }]; }));
const CUE_WORDS = SHAPE_CUES.map(([cue, ws]) => [cue, new Set(ws)]);
const FABRIC = [BLU, RED, GRN, BRN, MAG, CYN, DGR], FABRIC_KINDS = ['Armor', 'Robe', 'Legs', 'Cloak', 'Hood', 'Hat', 'Book', 'Bundle'];
const BACKDROPS = [BLU, MAG, CYN, GRN, RED, BRN, DGR];
const NOT_COLOURS = new Set(['rust', 'rusty', 'leather', 'wooden', 'wood', 'dark']);
const WEAR_WORDS = { worn: 1, faded: 1, old: 1, dented: 2, damaged: 2, scratched: 2, chipped: 2, cracked: 2, torn: 2, frayed: 2, battered: 3, notched: 3, tattered: 3, ragged: 3, broken: 4, shattered: 4, split: 4 };
const FINE_WORDS = ['fine', 'exquisite', 'masterwork', 'ornate', 'gilded', 'superb', 'elegant'], CRUDE_WORDS = ['crude', 'rough', 'primitive', 'shoddy', 'cheap'], RUST_WORDS = ['rusty', 'rusted', 'rusting'];
const TERRAIN_BACKDROP = { wood: GRN, forest: GRN, plain: GRN, field: GRN, swamp: GRN, jungle: GRN, desert: BRN, dune: BRN, mountain: DGR, hill: DGR, stone: DGR, cave: DGR, dungeon: DGR, crypt: DGR, water: BLU, sea: BLU, ocean: BLU, river: BLU, lake: BLU, ice: CYN, snow: CYN, city: MAG, town: MAG };

// ---------------------------------------------------------------------------------------------------------------------------------
// Reading the facts

/** The kind a name gives, by Coupler's rule: the first word that names a kind, not counting a material word if another word names one; none, a Bag for a container, else a Bundle. `word` is the word that matched. */
export function thingKind(name, { container = false } = {}) {
  const ws = words(name), hit = (w) => KIND_BY_WORD.get(w) ?? KIND_BY_WORD.get(w.replace(/es$/, '')) ?? KIND_BY_WORD.get(w.replace(/s$/, ''));
  for (const w of ws) if (!MATERIAL_WORDS.includes(w) && hit(w)) return { kind: hit(w), word: w };
  for (const w of ws) if (hit(w)) return { kind: hit(w), word: w };
  return { kind: container ? 'Bag' : 'Bundle', word: undefined };
}

/** The class and colour of a material: the material named (EXAMINE's), else a material word in the name before any "of" (two-word pairs first). `cls` is the table's class; `draws` is the texture it gets. */
export function materialClass(material, name) {
  const look = (m) => { const t = MATERIAL_TABLE.get(String(m ?? '').toLowerCase().trim()); return t && { material: String(m).toLowerCase().trim(), ...t, draws: THING_CLASSES.includes(t.cls) ? t.cls : 'plain' }; };
  const own = look(material); if (own) return own;
  const ws = words(name), cut = ws.indexOf('of'), pre = cut < 0 ? ws : ws.slice(0, cut);
  for (let i = 0; i + 1 < pre.length; i++) { const f = look(`${pre[i]} ${pre[i + 1]}`); if (f) return f; }
  for (const w of pre) { const f = look(w); if (f) return f; }
  return undefined;
}

/** The wear a name or param gives: `{ step 0 to 4, rust, crude, fine }`; a word, a list of words or an object. */
export function thingWear(wear, name) {
  const ws = [...words(name), ...(Array.isArray(wear) ? wear : typeof wear === 'string' ? [wear] : []).flatMap(words)];
  const o = wear && typeof wear === 'object' && !Array.isArray(wear) ? wear : {};
  const step = o.step ?? Math.max(0, ...ws.map((w) => WEAR_WORDS[w] ?? 0)), n = ws.map((w) => (w === 'sound' ? 'sound' : w));
  return { step: clamp(Math.round(step), 0, 4), rust: o.rust ?? n.some((w) => RUST_WORDS.includes(w)), crude: o.crude ?? n.some((w) => CRUDE_WORDS.includes(w)), fine: o.fine ?? n.some((w) => FINE_WORDS.includes(w)) };
}

/** An aura word's glow colour (VGA index, or undefined), as Coupler's table gives it and then by any colour word ("glowing green"). */
export function auraGlow(aura) {
  const a = String(aura).toLowerCase().trim(), t = AURAS.find(([name]) => name === a); if (t && t[1] !== null) return t[1];
  for (const w of words(a)) for (const [, bright, list] of COLOR_WORDS) if (list.includes(w)) return bright;
  return /glow/.test(a) ? YEL : undefined;
}
const auraKind = (a) => { const s = String(a).toLowerCase(); return /glow/.test(s) ? 'glow' : /magic/.test(s) ? 'magic' : /metal|mithril/.test(s) ? 'glint' : /invisib/.test(s) ? 'invisible' : /hidden/.test(s) ? 'hidden' : /float|flying|flies/.test(s) ? 'float' : /bound/.test(s) ? 'bound' : undefined; };

const colorWord = (ws) => { for (const w of ws) if (!NOT_COLOURS.has(w)) for (const [plain, , list] of COLOR_WORDS) if (list.includes(w)) return plain; return undefined; };
const titled = (s) => String(s).replace(/^./, (c) => c.toUpperCase());

/** The facts a picture is drawn from, read from the params. See `drawThing`. */
export function readThing(p = {}) {
  const name = String(p.name ?? ''), ws = words(name);
  const named = p.kind ? { kind: p.kind, word: undefined } : thingKind(name, { container: !!p.container });
  const info = KIND_INFO.get(named.kind); if (!info) throw new Error(`unknown item kind "${p.kind}"; known: ${THING_KINDS.join(', ')}`);
  const desc = /made (?:of|from) (?:a |an |the )?([a-z]+(?: [a-z]+)?)/.exec(String(p.description ?? '').toLowerCase());
  const mat = materialClass(p.material ?? (desc && (MATERIAL_TABLE.has(desc[1]) ? desc[1] : desc[1].split(' ')[0])), name) ?? (named.word && MATERIAL_TABLE.has(named.word) ? materialClass(named.word) : undefined);
  const cues = new Set(); const all = new Set([...ws, ...(named.word ? [named.word] : [])]);
  for (const [cue, list] of CUE_WORDS) if ([...all].some((w) => list.has(w))) cues.add(cue);
  const r = rng(`thing:${key(name) || info.kind}:${p.seed ?? ''}`);
  const forceMetal = !mat && ['mail', 'plate', 'greave', 'gauntlet'].some((c) => cues.has(c)) && ['Armor', 'Legs', 'Gloves', 'Helmet'].includes(info.kind);
  let cls = p.class ? String(p.class).toLowerCase() : mat ? mat.draws : forceMetal ? 'metal' : info.cls;
  if (!THING_CLASSES.includes(cls)) throw new Error(`unknown material class "${p.class}"; known: ${THING_CLASSES.join(', ')}`);
  const fabric = FABRIC_KINDS.includes(info.kind) && cls === 'cloth' ? r.pick(FABRIC) : undefined;
  const own = cls === info.cls || forceMetal;
  const classMain = { cloth: fabric ?? info.main, leather: BRN, metal: GRY, mithril: WHT, wooden: BRN, rock: DGR, precious: LRED, glass: LCYN, plain: info.main }[cls];
  const main = p.color ?? colorWord(ws) ?? (mat?.color ?? (own && !forceMetal ? (fabric ?? info.main) : classMain));
  const metalish = cls === 'metal' || cls === 'mithril';
  const metal = metalish ? main : info.metal, wood = cls === 'wooden' ? main : info.wood;
  const wear = thingWear(p.wear, name), auras = [...new Set([...(p.auras ?? []), ...ws.filter((w) => ['glowing', 'invisible', 'hidden', 'floating', 'flying', 'bound'].includes(w))])];
  const worn = p.worn ? String(p.worn) : undefined;
  const terrain = key(p.terrain), place = Object.entries(TERRAIN_BACKDROP).find(([t]) => terrain.includes(t))?.[1];
  const seedFor = r.fork('look');
  return { name, kind: info.kind, word: named.word, info, cls, material: mat?.material ?? (p.material ? String(p.material).toLowerCase() : undefined), main, metal, wood, trim: main === YEL ? WHT : YEL, gem: seedFor.pick([LRED, LBLU, LGRN, LMAG]), wear, auras, worn, cues, words: all, r, place, backdrop: p.backdrop, slot: titled(worn ?? info.slot).slice(0, 22) };
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The canvas and its strokes

class Canvas extends Px {
  constructor(depth = 16) { super(W, H, depth); this.role = new Uint8Array(W * H); }
  put(x, y, role, col = 0) { x = Math.round(x); y = Math.round(y); if (x < 0 || y < 0 || x >= W || y >= LABEL) return; const i = y * W + x; this.role[i] = role; this.a[i] = col; }
  is(x, y) { return x >= 0 && y >= 0 && x < W && y < H && this.role[y * W + x] !== BACK; }
  rect(x0, y0, x1, y1, role, col) { for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) this.put(x, y, role, col); }
  ell(cx, cy, rx, ry, role, col) { for (let y = Math.floor(cy - ry - 1); y <= Math.ceil(cy + ry + 1); y++) for (let x = Math.floor(cx - rx - 1); x <= Math.ceil(cx + rx + 1); x++) if (((x - cx) / (rx + 0.4)) ** 2 + ((y - cy) / (ry + 0.4)) ** 2 <= 1) this.put(x, y, role, col); }
  disc(cx, cy, r, role, col) { this.ell(cx, cy, r, r, role, col); }
  /** A line `w` pixels thick (a square stamp), ends included. */
  line(x0, y0, x1, y1, role, w = 1, col) {
    const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1), o = Math.floor((w - 1) / 2);
    for (let i = 0; i <= n; i++) { const x = Math.round(x0 + ((x1 - x0) * i) / n), y = Math.round(y0 + ((y1 - y0) * i) / n); for (let j = 0; j < w; j++) for (let k = 0; k < w; k++) this.put(x - o + k, y - o + j, role, col); }
  }
  /** A quadratic curve from `a` through control `c` to `b`, thickness from `w0` to `w1`. */
  curve(a, c, b, role, w0 = 1, w1 = w0, col) {
    for (let i = 0; i <= 24; i++) { const t = i / 24, x = (1 - t) ** 2 * a[0] + 2 * (1 - t) * t * c[0] + t * t * b[0], y = (1 - t) ** 2 * a[1] + 2 * (1 - t) * t * c[1] + t * t * b[1], w = Math.max(1, Math.round(w0 + (w1 - w0) * t)); this.line(x, y, x, y, role, w, col); }
  }
  /** A filled polygon (even-odd) with corners `pts`. */
  poly(pts, role, col) {
    const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
    for (let y = Math.floor(Math.min(...ys)); y <= Math.ceil(Math.max(...ys)); y++) for (let x = Math.floor(Math.min(...xs)); x <= Math.ceil(Math.max(...xs)); x++) {
      let inside = false;
      for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) { const [xi, yi] = pts[i], [xj, yj] = pts[j]; if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside; }
      if (inside) this.put(x, y, role, col);
    }
  }
}

// A point along a direction: `at(c, d, p, a, b)` is c + d*a + p*b, for the weapons that lie on a diagonal.
const D = [0.7071, -0.7071], P = [0.7071, 0.7071];
const at = (c, a, b) => [c[0] + D[0] * a + P[0] * b, c[1] + D[1] * a + P[1] * b];

// ---------------------------------------------------------------------------------------------------------------------------------
// One painter per kind. `t` is the canvas, `q` the facts (`q.cues`, `q.words`, `q.v` 0 to 3 for the name's own variation, `q.r`).

const has = (q, ...ws) => ws.some((w) => q.words.has(w));
function blade(t, q, { len, w, bend = 0, guard = 3 }) {
  const hilt = [7, 15], tip = at(hilt, len, 0), mid = at(hilt, len / 2, bend);
  t.line(...at(hilt, -2, 0), ...at(hilt, -5, 0), WOOD, 2); t.disc(...at(hilt, -6, 0), 1.2, TRIM);
  t.line(...at(hilt, 0, -guard), ...at(hilt, 0, guard), TRIM, 2);
  t.curve(at(hilt, 1, 0), at(mid, 0, 0), tip, MAIN, w, 1);
  t.line(...tip, ...tip, MAIN);
}
const PAINT = {
  Sword(t, q) { const c = q.cues; blade(t, q, { len: c.has('long') ? 17 : c.has('short') ? 10 : 13, w: c.has('thin') ? 1 : c.has('broad') ? 3 : 2, bend: c.has('curved') ? 3.2 : 0, guard: c.has('thin') ? 4 : 3 }); },
  Dagger(t, q) { const c = q.cues; blade(t, q, { len: 8, w: c.has('thin') ? 1 : 2, bend: c.has('curved') ? 2.2 : 0, guard: 2 }); },
  Axe(t, q) {
    t.line(4, 20, 16, 7, WOOD, 2);
    t.poly([[13, 4], [19, 1], [23, 6], [20, 11], [16, 10]], MAIN); t.line(14, 5, 16, 9, METAL);
    if (has(q, 'battleaxe', 'great') || q.v === 0) t.poly([[11, 10], [13, 16], [8, 18], [6, 14]], MAIN);
  },
  Mace(t, q) {
    t.line(4, 20, 14, 10, WOOD, 2);
    if (q.cues.has('club')) { t.ell(16, 8, 4.5, 5, MAIN); return; }
    t.disc(16, 8, 3.6, MAIN); for (const [x, y] of [[16, 2], [22, 8], [10, 8], [20, 3], [12, 3], [20, 13]]) t.put(x, y, METAL); t.disc(16, 8, 1, METAL);
  },
  Hammer(t, q) { t.line(4, 21, 15, 9, WOOD, 2); t.poly([[11, 6], [19, 1], [23, 7], [15, 12]], MAIN); t.line(12, 7, 20, 2, TRIM); },
  Spear(t, q) {
    const c = q.cues, C = [18, 6];
    t.line(2, 21, 18, 6, WOOD, 1); t.disc(2, 21, 0.6, METAL);
    if (c.has('forked')) { for (const b of [-3, 0, 3]) t.line(...at(C, 0, b), ...at(C, 5 - Math.abs(b) / 2, b), MAIN); t.line(...at(C, 0, -3), ...at(C, 0, 3), MAIN); }
    else if (c.has('sickle')) t.curve(at(C, 0, 0), at(C, 5, 6), at(C, 9, -1), MAIN, 2, 1);
    else {
      t.poly([at(C, 6, 0), at(C, 1, 2.4), at(C, -1.5, 0), at(C, 1, -2.4)], MAIN);
      if (c.has('bladed')) t.poly([at(C, 0, 2), at(C, 1, 6), at(C, -3, 6.5), at(C, -3, 2)], MAIN);
      if (c.has('flagged')) t.poly([at(C, -2, 0), at(C, -6, 5), at(C, -8, 2)], TRIM);
    }
  },
  Staff(t, q) {
    t.line(4, 21, 18, 5, MAIN, 2); t.line(3, 21, 4, 22, METAL);
    if (has(q, 'crook', 'cane')) { t.curve([18, 5], [19, 0], [23, 4], MAIN, 2); return; }
    t.disc(19, 4, 2.4, TRIM); t.disc(19, 4, 1.1, GEM);
  },
  Wand(t) { t.line(6, 19, 18, 7, MAIN, 1); t.disc(19, 6, 1.4, TRIM); t.line(5, 20, 6, 19, TRIM); t.put(19, 6, GEM); for (const [x, y] of [[21, 3], [22, 7], [17, 3], [22, 5]]) t.put(x, y, FIXED, WHT); },
  Bow(t, q) {
    if (has(q, 'crossbow')) { t.line(12, 21, 12, 6, MAIN, 2); t.curve([2, 8], [12, 2], [22, 8], WOOD, 2, 2); t.line(2, 8, 12, 14, FIXED, 1, GRY); t.line(12, 14, 22, 8, FIXED, 1, GRY); return; }
    if (has(q, 'sling')) { t.line(12, 20, 12, 14, MAIN, 2); t.line(12, 14, 6, 5, MAIN); t.line(12, 14, 18, 5, MAIN); t.disc(12, 4, 1.5, WOOD); return; }
    t.curve([17, 1], [3, 11], [17, 21], MAIN, 2, 2); t.line(17, 1, 17, 21, FIXED, 1, GRY); t.rect(5, 10, 6, 12, TRIM);
  },
  Arrow(t, q) {
    if (has(q, 'quiver')) { t.line(7, 20, 15, 8, MAIN, 5); t.line(6, 21, 16, 7, WOOD, 1); for (let i = 0; i < 3; i++) { t.line(16 + i * 2 - 2, 8 - i, 18 + i * 2 - 2, 3 - i, WOOD); t.line(18 + i * 2 - 2, 3 - i, 19 + i * 2 - 2, 2 - i, FIXED, 1, LRED); } return; }
    for (let i = 0; i < 3; i++) {
      const o = i * 2, a = [3 + o, 20], b = [18 + o, 5 - (i ? 0 : 0)];
      t.line(...a, ...b, WOOD, 1); t.poly([[b[0] + 1, b[1] - 3], [b[0] + 3, b[1] - 1], [b[0] - 1, b[1] - 1]], MAIN); t.line(a[0], a[1] - 1, a[0] + 2, a[1] - 3, FIXED, 1, LRED); t.line(a[0] + 1, a[1], a[0] + 3, a[1] - 2, FIXED, 1, LRED);
    }
  },
  Shield(t, q) {
    if (has(q, 'buckler')) { t.disc(12, 11, 5.5, TRIM); t.disc(12, 11, 4.2, MAIN); t.disc(12, 11, 1.6, METAL); return; }
    if (q.v % 2) { t.poly([[4, 3], [20, 3], [20, 11], [12, 19], [4, 11]], TRIM); t.poly([[5.5, 4.5], [18.5, 4.5], [18.5, 10.5], [12, 17], [5.5, 10.5]], MAIN); t.rect(11, 5, 13, 16, METAL); t.rect(6, 8, 18, 9, METAL); return; }
    t.disc(12, 11, 8.5, TRIM); t.disc(12, 11, 7.2, MAIN); t.disc(12, 11, 2.4, METAL); t.line(12, 4, 12, 18, METAL); t.line(5, 11, 19, 11, METAL);
  },
  Helmet(t, q) {
    if (q.cues.has('mail')) { t.ell(12, 10, 7, 7.5, MAIN); t.rect(5, 12, 19, 18, MAIN); t.ell(12, 13, 3.5, 4.5, LINE); return; }
    t.ell(12, 10, 7.5, 7.5, MAIN); t.rect(4, 10, 20, 16, MAIN); t.rect(4, 11, 20, 12, METAL); t.rect(7, 13, 17, 14, LINE); t.rect(11, 5, 13, 16, METAL);
    if (q.v === 0) t.poly([[12, 1], [15, 2], [12, 4]], FIXED, LRED);
  },
  Crown(t, q) {
    if (q.cues.has('low')) { t.rect(4, 12, 20, 15, MAIN); t.poly([[10, 12], [12, 8], [14, 12]], MAIN); t.rect(12, 12, 12, 13, GEM); t.rect(5, 12, 20, 12, TRIM); return; }
    t.rect(4, 11, 20, 16, MAIN); for (const x of [4, 8, 12, 16]) t.poly([[x, 11], [x + 2, 4], [x + 4, 11]], MAIN);
    for (const x of [6, 12, 18]) t.put(x, 13, GEM); for (const x of [6, 10, 14, 18]) t.put(x, 4 + (x === 10 || x === 14 ? 0 : 0), TRIM); t.rect(4, 16, 20, 16, TRIM);
  },
  Hood(t, q) {
    if (has(q, 'mask', 'veil')) { t.ell(12, 10, 7.5, 6.5, MAIN); t.rect(7, 8, 10, 9, LINE); t.rect(14, 8, 17, 9, LINE); if (has(q, 'veil')) t.rect(5, 12, 19, 18, MAIN); return; }
    t.poly([[12, 2], [18, 7], [21, 18], [3, 18], [6, 7]], MAIN); t.ell(12, 11, 4, 5.5, LINE); t.rect(7, 17, 17, 18, TRIM);
  },
  Hat(t, q) {
    const c = q.cues; t.ell(12, 15, c.has('brim') || c.has('pointed') ? 11 : 8.5, 3.2, MAIN);
    if (c.has('pointed')) t.poly([[6, 15], [16, 2], [18, 5], [18, 15]], MAIN); else t.poly([[6, 14], [7, 6], [10, 4], [14, 4], [17, 6], [18, 14]], MAIN);
    t.rect(6, 12, 18, 13, TRIM);
  },
  Armor(t, q) {
    const c = q.cues; t.poly([[5, 4], [9, 3], [11, 5], [14, 5], [16, 3], [20, 4], [23, 10], [19, 12], [18, 12], [18, 20], [7, 20], [7, 12], [6, 12], [2, 10]], MAIN); t.ell(12, 4, 2.2, 1.6, LINE);
    if (c.has('plate')) { for (const y of [8, 11, 14]) t.rect(7, y, 18, y, LINE); t.disc(5, 6, 2, TRIM); t.disc(19, 6, 2, TRIM); }
    t.rect(7, 16, 18, 17, TRIM);
  },
  Robe(t, q) { t.poly([[9, 3], [15, 3], [18, 6], [23, 17], [22, 20], [2, 20], [1, 17], [6, 6]], MAIN); t.poly([[10, 3], [14, 3], [12, 8]], LINE); t.rect(8, 10, 16, 11, TRIM); t.line(12, 12, 12, 20, LINE); },
  Legs(t, q) {
    const c = q.cues; t.rect(5, 3, 19, 5, TRIM);
    if (c.has('skirt')) { t.poly([[5, 5], [19, 5], [23, 20], [1, 20]], MAIN); t.line(9, 7, 6, 19, LINE); t.line(15, 7, 18, 19, LINE); return; }
    t.poly([[5, 5], [12, 5], [11, 20], [4, 20]], MAIN); t.poly([[13, 5], [20, 5], [21, 20], [14, 20]], MAIN);
    if (c.has('greave')) { t.disc(8, 11, 1.6, TRIM); t.disc(17, 11, 1.6, TRIM); t.rect(4, 19, 11, 20, TRIM); t.rect(14, 19, 21, 20, TRIM); }
  },
  Boots(t, q) { for (const x of [3, 14]) { t.rect(x + 1, 3, x + 6, 13, MAIN); t.rect(x, 13, x + 9, 18, MAIN); t.rect(x + 1, 3, x + 6, 4, TRIM); t.rect(x, 18, x + 9, 18, LINE); t.rect(x + 7, 12, x + 9, 13, LINE); } },
  Gloves(t, q) {
    const g = q.cues.has('gauntlet');
    for (const x of [3, 14]) {
      t.rect(x, 9, x + 6, 15, MAIN); for (let i = 0; i < 4; i++) t.rect(x + i * 2 - (i === 3 ? 1 : 0), i === 1 || i === 2 ? 3 : 5, x + i * 2 + 1 - (i === 3 ? 1 : 0), 9, MAIN);
      t.rect(x - 2, 10, x, 12, MAIN); t.rect(x - 1, 15, x + 7, g ? 20 : 17, g ? METAL : TRIM); if (g) { t.rect(x, 9, x + 6, 9, TRIM); t.rect(x - 1, 17, x + 7, 17, TRIM); }
    }
  },
  Cloak(t, q) { t.poly([[6, 3], [18, 3], [23, 19], [1, 19]], MAIN); for (const x of [7, 12, 17]) t.line(x, 8, x + (x - 12) / 2, 19, LINE); t.disc(12, 4, 1.7, TRIM); t.rect(6, 3, 18, 4, METAL); },
  Belt(t, q) { t.rect(1, 9, 23, 12, MAIN); t.rect(10, 7, 14, 14, TRIM); t.rect(11, 9, 13, 12, LINE); for (const x of [16, 18, 20]) t.put(x, 10, LINE); },
  Ring(t, q) { t.ell(12, 14, 6.5, 4.8, MAIN); t.ell(12, 14.4, 3.8, 2.6, BACK); t.disc(12, 8, 2.4, TRIM); t.disc(12, 8, 1.3, GEM); },
  Amulet(t, q) {
    t.curve([2, 2], [12, 22], [22, 2], MAIN); const v = q.v;
    if (v % 3 === 0) { t.disc(12, 15, 3.6, MAIN); t.disc(12, 15, 1.6, GEM); } else if (v % 3 === 1) { t.poly([[12, 11], [16, 16], [12, 21], [8, 16]], MAIN); t.put(12, 16, GEM); } else { t.rect(11, 11, 13, 20, MAIN); t.rect(8, 14, 16, 15, MAIN); t.put(12, 14, GEM); }
  },
  Bracelet(t, q) { t.ell(12, 12, 9.5, 6.5, MAIN); t.ell(12, 12.4, 6.8, 4, BACK); for (const [x, y] of [[8, 17], [12, 18], [16, 17]]) t.put(x, y, GEM); },
  Potion(t, q) {
    const v = q.v, fill = q.main === GRY ? LRED : q.main, g = LCYN;
    if (v % 3 === 0) { t.ell(12, 14, 6.5, 5.5, FIXED, g); t.rect(10, 5, 14, 10, FIXED, g); t.ell(12, 15, 5, 4.2, FIXED, fill); t.rect(11, 8, 13, 9, FIXED, fill); t.rect(10, 3, 14, 4, WOOD); }
    else if (v % 3 === 1) { t.rect(9, 5, 15, 17, FIXED, g); t.rect(10, 8, 14, 16, FIXED, fill); t.rect(10, 3, 14, 4, WOOD); t.rect(9, 17, 15, 17, FIXED, DGR); }
    else { t.rect(7, 9, 17, 17, FIXED, g); t.rect(8, 11, 16, 16, FIXED, fill); t.rect(10, 3, 14, 8, FIXED, g); t.rect(10, 2, 14, 3, WOOD); }
    t.put(10, 12, FIXED, WHT);
  },
  Scroll(t, q) {
    t.rect(4, 7, 20, 16, MAIN); t.ell(4, 11.5, 2.2, 5, MAIN); t.ell(20, 11.5, 2.2, 5, METAL); t.rect(11, 7, 13, 16, TRIM);
    for (const [y, a, b] of [[9, 6, 10], [11, 6, 9], [13, 6, 10], [9, 14, 18], [11, 14, 17], [13, 14, 18], [15, 14, 16]]) for (let x = a; x <= b; x += 1) if ((x + y) % 3) t.put(x, y, LINE);
  },
  Book(t, q) {
    t.rect(6, 3, 18, 18, MAIN); t.rect(6, 3, 8, 18, METAL); t.rect(6, 5, 8, 6, TRIM); t.rect(6, 15, 8, 16, TRIM); t.rect(19, 4, 20, 17, FIXED, WHT); t.rect(19, 5, 20, 5, FIXED, GRY); t.rect(19, 9, 20, 9, FIXED, GRY); t.rect(19, 13, 20, 13, FIXED, GRY);
    t.rect(10, 7, 15, 10, TRIM); t.rect(11, 8, 14, 9, GEM); t.rect(17, 10, 18, 12, METAL);
  },
  Bag(t, q) { t.ell(12, 14, 8, 6.5, MAIN); t.poly([[8, 3], [10, 8], [14, 8], [16, 3], [12, 6]], MAIN); t.rect(10, 8, 14, 8, MAIN); t.rect(9, 8, 15, 9, TRIM); t.put(6, 17, LINE); t.rect(5, 19, 19, 19, LINE); },
  Chest(t, q) {
    t.rect(3, 10, 21, 19, MAIN); t.ell(12, 10, 9.4, 5.4, MAIN); t.rect(3, 10, 21, 10, METAL); t.rect(6, 5, 7, 19, METAL); t.rect(17, 5, 18, 19, METAL); t.rect(3, 19, 21, 19, METAL); t.rect(11, 9, 13, 13, TRIM); t.put(12, 11, LINE);
  },
  Barrel(t, q) { t.ell(12, 11, 8.5, 9.5, MAIN); t.ell(12, 3.5, 6, 1.4, WOOD); for (const y of [6, 16]) { const half = Math.round(8.5 * Math.sqrt(1 - ((y - 11) / 9.9) ** 2)); t.rect(12 - half, y, 12 + half, y + 1, METAL); } for (const x of [9, 12, 15]) t.line(x, 8, x, 15, LINE); },
  Cup(t, q) {
    t.rect(7, 6, 16, 17, MAIN); t.rect(7, 6, 16, 6, TRIM); t.rect(8, 7, 15, 8, FIXED, BRN); t.rect(17, 8, 20, 9, MAIN); t.rect(19, 9, 20, 14, MAIN); t.rect(17, 14, 20, 15, MAIN);
    if (has(q, 'bowl')) { t.ell(12, 10, 9, 6, MAIN); t.rect(2, 4, 22, 9, BACK); t.rect(2, 10, 22, 10, TRIM); }
  },
  Goblet(t, q) { t.poly([[6, 3], [18, 3], [16, 11], [8, 11]], MAIN); t.ell(12, 11, 4.4, 1.5, MAIN); t.rect(7, 4, 17, 5, FIXED, LRED); t.rect(11, 12, 13, 17, MAIN); t.ell(12, 18, 5.4, 1.8, MAIN); t.rect(6, 3, 18, 3, TRIM); t.put(12, 14, GEM); },
  Bread(t, q) { t.ell(12, 12, 9.5, 5.6, MAIN); for (const x of [7, 11, 15]) t.line(x, 8, x + 2, 12, LINE); t.rect(7, 16, 17, 16, LINE); },
  Fruit(t, q) {
    if (has(q, 'pear')) { t.ell(12, 14, 5.6, 5, MAIN); t.ell(12, 8, 3, 3.4, MAIN); t.line(12, 4, 13, 2, WOOD); return; }
    if (has(q, 'mushroom')) { t.ell(12, 9, 8, 4.5, MAIN); t.rect(10, 11, 14, 17, FIXED, WHT); t.put(8, 8, FIXED, WHT); t.put(13, 6, FIXED, WHT); t.put(16, 9, FIXED, WHT); return; }
    if (has(q, 'berry', 'berries', 'grape', 'grapes')) { for (const [x, y] of [[8, 13], [14, 13], [11, 9], [17, 9], [6, 8], [11, 16]]) t.disc(x, y, 2.4, MAIN); t.line(11, 6, 13, 3, WOOD); t.poly([[13, 3], [18, 2], [15, 5]], FIXED, GRN); return; }
    if (has(q, 'egg')) { t.ell(12, 12, 5, 6.2, FIXED, WHT); return; }
    if (has(q, 'nut', 'nuts', 'potato', 'turnip', 'onion')) { t.ell(12, 13, 6.5, 5.2, MAIN); t.line(12, 7, 12, 5, WOOD); return; }
    t.disc(12, 12, 6.8, MAIN); t.line(12, 6, 13, 3, WOOD); t.poly([[13, 4], [19, 2], [16, 6]], FIXED, GRN);
  },
  Meat(t, q) { t.ell(10, 11, 7.5, 5.8, MAIN); t.ell(8, 9, 3, 2, FIXED, LIGHT[q.main]); t.line(15, 14, 21, 19, FIXED, 2, WHT); t.disc(21, 19, 1.4, FIXED, WHT); t.disc(22, 17, 1.2, FIXED, WHT); },
  Cheese(t, q) { t.poly([[3, 14], [21, 9], [21, 17], [3, 17]], MAIN); t.poly([[3, 14], [11, 7], [21, 9]], FIXED, LIGHT[q.main]); for (const [x, y] of [[8, 15], [14, 14], [17, 12], [11, 10]]) t.put(x, y, LINE); },
  Fish(t, q) { t.ell(11, 11, 8.5, 4.8, MAIN); t.poly([[17, 11], [23, 5], [23, 17]], MAIN); t.poly([[9, 6], [13, 6], [11, 3]], MAIN); t.rect(5, 12, 15, 14, FIXED, WHT); t.put(5, 10, FIXED, WHT); t.put(5, 10, LINE); t.line(14, 8, 14, 14, LINE); },
  Coins(t, q) {
    for (let i = 0; i < 4; i++) { t.rect(3, 15 - i * 3, 12, 16 - i * 3, MAIN); t.ell(7.5, 14 - i * 3, 4.6, 1.8, MAIN); t.rect(3, 16 - i * 3, 12, 16 - i * 3, METAL); }
    t.ell(17, 17, 4.6, 2, MAIN); t.ell(18, 8, 3.6, 3.6, MAIN); t.disc(18, 8, 1.4, TRIM);
  },
  Gem(t, q) { t.poly([[12, 4], [18, 8], [12, 19], [6, 8]], MAIN); t.line(6, 8, 18, 8, LINE); t.line(12, 4, 9, 8, LINE); t.line(12, 4, 15, 8, LINE); t.line(9, 8, 12, 19, LINE); t.line(15, 8, 12, 19, LINE); t.put(10, 6, FIXED, WHT); },
  Key(t, q) { t.ell(6, 10, 4.6, 4.6, MAIN); t.ell(6, 10, 2, 2, BACK); t.rect(10, 9, 21, 11, MAIN); t.rect(17, 11, 18, 14, MAIN); t.rect(20, 11, 21, 13, MAIN); },
  Torch(t, q) {
    t.line(5, 21, 14, 10, WOOD, 2); t.ell(15.5, 8.5, 3, 3.5, MAIN);
    t.poly([[16, 1], [20, 5], [19, 9], [13, 9], [12, 5]], FIXED, RED); t.poly([[16, 3], [18, 6], [17, 9], [14, 8], [14, 6]], FIXED, LRED); t.poly([[16, 5], [17, 8], [14, 8]], FIXED, YEL); t.put(16, 8, FIXED, WHT);
  },
  Lantern(t, q) {
    t.rect(7, 7, 17, 17, MAIN); t.rect(9, 9, 15, 15, FIXED, YEL); t.rect(11, 11, 13, 14, FIXED, WHT); t.poly([[6, 7], [12, 3], [18, 7]], MAIN); t.curve([8, 3], [12, -1], [16, 3], LINE); t.rect(7, 17, 17, 18, METAL);
    t.line(11, 9, 11, 15, MAIN); t.line(13, 9, 13, 15, MAIN);
  },
  Candle(t, q) {
    const many = has(q, 'candles');
    for (const [x, top] of many ? [[6, 10], [12, 7], [18, 11]] : [[12, 8]]) { t.rect(x - 2, top, x + 2, 17, MAIN); t.rect(x - 2, top, x - 2, top + 3, MAIN); t.put(x, top - 1, LINE); t.ell(x, top - 3.5, 1.3, 2.2, FIXED, YEL); t.put(x, top - 3, FIXED, WHT); }
    t.ell(12, 18, many ? 11 : 7, 1.8, TRIM);
  },
  Lute(t, q) { t.ell(12, 15, 6.4, 5.8, MAIN); t.rect(11, 3, 13, 11, WOOD); t.rect(9, 2, 15, 4, TRIM); t.disc(12, 15, 1.9, LINE); t.line(12, 10, 12, 19, FIXED, 1, GRY); t.rect(9, 18, 15, 18, WOOD); },
  Drum(t, q) { t.rect(4, 7, 20, 16, MAIN); t.ell(12, 16, 8.5, 3, MAIN); t.ell(12, 7, 8.5, 3, FIXED, WHT); t.ell(12, 7, 8.5, 3, FIXED, WHT); for (const [a, b, c, d] of [[4, 9, 8, 16], [8, 16, 12, 9], [12, 9, 16, 16], [16, 16, 20, 9]]) t.line(a, b, c, d + 2, LINE); },
  Horn(t, q) { t.curve([2, 17], [3, 5], [18, 5], MAIN, 2, 5); t.ell(20, 6, 3, 4, MAIN); t.ell(20.5, 6, 1.3, 2.5, LINE); t.rect(1, 16, 3, 18, TRIM); t.rect(11, 3, 11, 8, TRIM); },
  Flute(t, q) { t.line(3, 20, 21, 4, MAIN, 2); for (const i of [0.4, 0.5, 0.6, 0.7]) t.put(3 + 18 * i, 20 - 16 * i, LINE); t.put(8, 15, LINE); t.rect(2, 20, 3, 21, TRIM); },
  Pick(t, q) { t.line(6, 21, 14, 7, WOOD, 2); t.curve([3, 9], [14, 0], [24, 10], MAIN, 2, 2); t.disc(14, 5, 1.5, METAL); },
  Shovel(t, q) {
    if (has(q, 'rake')) { t.line(12, 2, 12, 15, WOOD, 2); t.rect(4, 16, 20, 17, MAIN); for (const x of [4, 7, 10, 13, 16, 19]) t.rect(x, 18, x, 21, MAIN); return; }
    t.line(12, 3, 12, 14, WOOD, 2); t.rect(8, 2, 16, 3, WOOD); t.poly([[8, 13], [16, 13], [15, 19], [12, 22], [9, 19]], MAIN);
  },
  Rock(t, q) {
    if (q.cues.has('ingot')) { t.poly([[3, 17], [21, 17], [23, 12], [6, 12]], MAIN); t.poly([[6, 12], [23, 12], [20, 9], [9, 9]], FIXED, LIGHT[q.main]); return; }
    const r = q.r.fork('rock'), pts = []; for (let i = 0; i < 9; i++) { const a = (i / 9) * Math.PI * 2, rad = 5.5 + r.next() * 3; pts.push([12 + Math.cos(a) * rad * 1.15, 12 + Math.sin(a) * rad * 0.8]); }
    t.poly(pts, MAIN); t.line(10, 9, 14, 13, LINE); t.line(14, 13, 13, 17, LINE);
  },
  Bundle(t, q) { t.ell(12, 12, 9, 7.5, MAIN); t.rect(10, 4, 11, 20, TRIM); t.rect(4, 11, 20, 12, TRIM); t.disc(11, 11, 1.8, TRIM); t.line(3, 7, 5, 4, MAIN, 2); t.line(21, 7, 19, 4, MAIN, 2); },
  Jar(t, q) { t.ell(12, 14, 7.5, 6.5, MAIN); t.rect(9, 5, 15, 8, MAIN); t.ell(12, 5, 4.6, 1.4, TRIM); t.rect(3, 11, 5, 15, MAIN); t.rect(19, 11, 21, 15, MAIN); t.rect(6, 13, 18, 13, TRIM); t.rect(8, 18, 16, 18, LINE); },
  Herb(t, q) {
    const heads = [[6, 6], [12, 3], [18, 6], [9, 10], [15, 10]];
    for (const [x, y] of heads) { t.line(12, 19, x, y, FIXED, 1, GRN); t.ell(x, y, 2.2, 2.6, MAIN); }
    t.disc(12, 3, 1.4, FIXED, q.main === GRY ? LMAG : q.main); t.rect(10, 16, 14, 17, TRIM);
  },
  Map(t, q) {
    t.poly([[3, 4], [21, 3], [22, 17], [4, 18]], MAIN); for (const x of [9, 15]) for (let y = 4; y <= 17; y += 2) t.put(x + (y > 10 ? 1 : 0), y, LINE);
    t.line(15, 11, 19, 15, FIXED, 1, RED); t.line(19, 11, 15, 15, FIXED, 1, RED); for (const [x, y] of [[5, 14], [7, 12], [9, 13], [11, 11], [13, 9]]) t.put(x, y, FIXED, BRN);
    t.disc(6, 7, 1.4, TRIM); t.put(6, 7, LINE);
  },
  Relic(t, q) {
    if (has(q, 'skull')) { t.ell(12, 9, 6.4, 6, MAIN); t.rect(8, 13, 16, 17, MAIN); t.rect(7, 8, 10, 11, LINE); t.rect(14, 8, 17, 11, LINE); t.rect(11, 12, 13, 13, LINE); for (const x of [9, 11, 13, 15]) t.put(x, 16, LINE); return; }
    if (has(q, 'orb')) { t.disc(12, 11, 7, MAIN); t.ell(12, 19, 6, 1.6, TRIM); t.disc(9, 8, 1.5, FIXED, WHT); return; }
    t.rect(6, 16, 18, 19, MAIN); t.poly([[9, 7], [15, 7], [17, 16], [7, 16]], MAIN); t.disc(12, 4.5, 3, MAIN); t.put(11, 4, GEM); t.put(13, 4, GEM); t.line(9, 9, 5, 13, MAIN); t.line(15, 9, 19, 13, MAIN);
  },
  Rune(t, q) {
    t.poly([[5, 3], [19, 2], [20, 18], [6, 19]], MAIN);
    const glyphs = [[[8, 5, 8, 16], [8, 8, 15, 5], [8, 12, 15, 9]], [[12, 4, 12, 17], [8, 8, 12, 12], [16, 8, 12, 12]], [[8, 5, 15, 17], [15, 5, 8, 17], [8, 11, 15, 11]], [[8, 5, 15, 5], [15, 5, 8, 17], [8, 17, 15, 17]], [[8, 17, 8, 5], [8, 5, 15, 11], [15, 11, 8, 17]], [[9, 5, 9, 17], [9, 11, 16, 5], [9, 11, 16, 17]]];
    for (const [a, b, c, d] of glyphs[q.v % glyphs.length]) t.line(a, b, c, d, FIXED, 1, LCYN);
    for (const [x, y] of [[6, 4], [18, 4], [7, 18], [19, 17]]) t.put(x, y, FIXED, CYN);
  },
};

// ---------------------------------------------------------------------------------------------------------------------------------
// Wear in the shape (before the colours), the finish, then rust, trim and the auras

const edges = (t) => { const out = []; for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) if (t.is(x, y) && [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dx, dy]) => !t.is(x + dx, y + dy))) { let n = 0; for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if ((dx || dy) && t.is(x + dx, y + dy)) n++; if (n >= 3) out.push([x, y]); } return out; };
const solids = (t, f) => { const out = []; for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) if (t.is(x, y) && (!f || f(x, y))) out.push([x, y]); return out; };
const bite = (t, r, n) => { const e = edges(t); for (let i = 0; i < n && e.length; i++) { const [x, y] = e.splice(r.int(e.length), 1)[0]; t.role[y * W + x] = BACK; } };
const crack = (t, r, n) => { const s = solids(t, (x, y) => t.role[y * W + x] === MAIN || t.role[y * W + x] === WOOD || t.role[y * W + x] === METAL); for (let k = 0; k < n && s.length; k++) { let [x, y] = s[r.int(s.length)]; for (let i = 0; i < 5; i++) { if (t.is(x, y)) { t.role[y * W + x] = LINE; } x += r.int(3) - 1; y += 1; } } };

function wearShape(t, q) {
  const { step } = q.wear, r = q.r.fork('wear'), cls = q.cls;
  if (step >= 2) {
    if (['cloth', 'leather'].includes(cls)) { const e = edges(t).filter(([x, y]) => !t.is(x, y + 1)); e.forEach(([x, y], i) => { if (i % 2 === 0 && r.chance(0.7)) t.role[y * W + x] = BACK; }); }
    else if (['wooden', 'rock', 'glass', 'precious'].includes(cls)) crack(t, r, step >= 3 ? 3 : 2); else bite(t, r, step >= 3 ? 4 : 2);
  }
  if (step >= 3) {
    bite(t, r, 5); for (const [x, y] of solids(t, (x, y) => t.is(x - 1, y) && t.is(x + 1, y) && t.is(x, y - 1) && t.is(x, y + 1)).filter(() => r.chance(0.04))) t.role[y * W + x] = BACK;
    if (!['cloth', 'leather'].includes(cls)) crack(t, r, 1);
  }
  if (step >= 4) {                                                       // broken: split in two along a jagged cut and pulled apart
    const s = solids(t); if (!s.length) return;
    const cx = Math.round(s.reduce((a, p) => a + p[0], 0) / s.length), cy = Math.round(s.reduce((a, p) => a + p[1], 0) / s.length), flip = r.chance(0.5) ? 1 : -1;
    const keep = s.map(([x, y]) => [x, y, t.role[y * W + x], t.a[y * W + x]]); t.role.fill(BACK);
    const jag = Array.from({ length: H }, () => r.int(3) - 1);
    for (const [x, y, role, col] of keep) {
      const d = (x - cx) * flip + (y - cy) + jag[y];
      if (d === 0) continue;
      t.put(x + (d > 0 ? 1 : -1), y, role, col);
    }
  }
}

function lift(t, by) { const keep = []; for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) if (t.is(x, y)) keep.push([x, y, t.role[y * W + x], t.a[y * W + x]]); t.role.fill(BACK); for (const [x, y, role, col] of keep) t.put(x, y - by, role, col); }

const GRAIN = { [BRN]: RED, [YEL]: BRN, [WHT]: GRY };
/** Colour the parts: edge light and shade, then the texture of the class. */
function finish(t, q, back, bg) {
  const r = q.r.fork('texture'), out = new Uint32Array(W * H), baseOf = { [MAIN]: q.main, [METAL]: q.metal, [WOOD]: q.wood, [TRIM]: q.trim, [GEM]: q.gem };
  const classOf = { [MAIN]: q.cls, [METAL]: q.cls === 'wooden' || q.cls === 'cloth' || q.cls === 'leather' || q.cls === 'plain' ? 'metal' : q.cls, [WOOD]: 'wooden', [TRIM]: 'metal', [GEM]: 'precious' };
  const dk = (c) => { const d = DARK_[c] || DGR; return d === back ? (d === DGR ? K : DGR) : d; };
  for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, role = t.role[i];
    if (role === BACK) { out[i] = bg[i]; continue; }
    if (role === FIXED) { out[i] = t.a[i]; continue; }
    if (role === LINE) { out[i] = back === K ? DGR : K; continue; }
    let c = baseOf[role], cls = classOf[role];
    if (cls === 'mithril' && c === WHT) c = LCYN;
    const open = !t.is(x - 1, y) || !t.is(x, y - 1) || !t.is(x + 1, y) || !t.is(x, y + 1), lit = (!t.is(x - 1, y) || !t.is(x, y - 1)) && t.is(x + 1, y + 1), shd = (!t.is(x + 1, y) || !t.is(x, y + 1)) && t.is(x - 1, y - 1), deepIn = !lit && !shd, inner = !open;
    let v = c;
    if (lit && !shd) v = LIGHT[c]; else if (shd && !lit) v = dk(c);
    if (q.wear.crude && lit) v = c;
    if (role === GEM) { out[i] = (lit && !shd) || (x + y) % 5 === 0 ? WHT : c; continue; }
    switch (cls) {
      case 'cloth': if (deepIn && (x + y * 2) % 4 === 0) v = dk(c); break;
      case 'leather': if (inner && [[2, 0], [-2, 0], [0, 2], [0, -2]].some(([dx, dy]) => !t.is(x + dx, y + dy)) && (x + y) % 2 === 0) v = LIGHT[c]; break;
      case 'wooden': if (deepIn && (y + ((x * 3) >> 3)) % 3 === 0 && x % 5 !== 0) v = GRAIN[c] ?? dk(c); break;
      case 'metal': if (deepIn && (((x - y) % 7) + 7) % 7 < 2) v = LIGHT[c]; break;
      case 'mithril': if (r.chance(0.2)) v = [LCYN, LBLU, WHT][r.int(3)]; break;
      case 'precious': if (r.chance(0.12)) v = WHT; else if (deepIn && (x + y) % 4 === 0) v = LIGHT[c]; break;
      case 'rock': if (deepIn && r.chance(0.28)) v = r.chance(0.5) ? dk(c) : LIGHT[c]; break;
      case 'glass': if (deepIn && (x + y) % 2 === 0) v = back; else if (deepIn && BAYER[y & 3][x & 3] < 3) v = WHT; break;
      default: break;
    }
    out[i] = v;
  }
  t.a.set(out);
  if (t.smooth) smoothThing(t, q, classOf);
}

/**
 * With 256 colours or true colour, the item's volume and the grain of its material as blended tones: light from the upper left and a darker
 * lower right edge over all of it, then each material its own grain (a weave in cloth, a mottle in leather, long grain in wood, a brushed
 * streak in metal, a coarse mottle in rock, a sparkle in precious things). Runs after the 16-colour shading, which it builds on.
 */
function smoothThing(t, q, classOf) {
  const seed = hashSeed(`${q.kind}:${q.name}`) & 0xffff, solid = (x, y) => t.is(x, y) && y < LABEL;
  const GRAINS = { cloth: [1.2, 1.3, 1.3], leather: [2, 1, 1], wooden: [2.6, 0.3, 1.5], metal: [1.6, 0.45, 1.7], mithril: [2, 0.7, 0.7], precious: [1, 1, 1], rock: [3, 0.9, 0.9], glass: [0.8, 0.5, 0.5] };
  const classAt = (x, y) => classOf[t.role[y * W + x]];
  for (const cls of new Set(Object.values(classOf))) {
    const [grain, fx, fy] = GRAINS[cls] ?? [1.2, 1, 1];
    volume(t, { solid, pick: (x, y) => y < LABEL && [MAIN, METAL, WOOD, TRIM].includes(t.role[y * W + x]) && classAt(x, y) === cls, seed, light: cls === 'glass' ? 5 : 3, rim: cls === 'rock' ? 5 : 4, grain, fx, fy });
  }
  for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, role = t.role[i]; if (![MAIN, METAL, WOOD, TRIM].includes(role)) continue;
    const cls = classAt(x, y), c = t.a[i];
    if (cls === 'cloth') t.a[i] = t.tint(c, (x + y) & 1 ? 1 : -1);                                                  // the weave
    else if (cls === 'metal' || cls === 'mithril') { if ((((x - y) % 9) + 9) % 9 < 2) t.a[i] = t.tint(c, 3); }      // a bright streak along the blade
    else if (cls === 'precious' && vnoise(seed + 5, x * 2.3, y * 2.3) > 0.82) t.a[i] = t.tint(c, 9);                  // a sparkle
    else if (cls === 'glass') t.a[i] = t.tint(c, Math.round(((x + y) % 8 - 4) / 2));                                // a slanting sheen
  }
}

function ground(t, back, r) {
  // the backdrop dithers into black at its top and the table into black at its front: one `mix` is that dither at 16 colours and a blend at 256
  for (let y = 0; y < TABLE; y++) for (let x = 0; x < W; x++) t.a[y * W + x] = y < 5 ? t.mix(x, y, back, K, (5 - y) * 2) : back;
  for (let y = TABLE; y < LABEL; y++) for (let x = 0; x < W; x++) t.a[y * W + x] = y === TABLE ? K : y < TABLE + 4 ? BRN : t.mix(x, y, BRN, K, (y - TABLE - 3) * 6);
  for (let y = LABEL; y < H; y++) for (let x = 0; x < W; x++) t.a[y * W + x] = K;
  for (let i = 0; i < 4; i++) { const y = TABLE + 2 + r.int(3), x = r.int(W); t.a[y * W + x] = RED; t.a[y * W + Math.min(W - 1, x + 1 + r.int(3))] = RED; }
  if (t.smooth) {
    // the broader palette: the backdrop darkens toward the table, with a soft mottle; the table has a grain along it
    for (let y = 0; y < TABLE; y++) for (let x = 0; x < W; x++) { const i = y * W + x; if (t.a[i] === K) continue; t.a[i] = t.tint(t.a[i], -Math.round(y * 4 / TABLE) + Math.round((vnoise(11, x / 4, y / 3) - 0.5) * 4)); }
    for (let y = TABLE + 1; y < LABEL; y++) for (let x = 0; x < W; x++) { const i = y * W + x; if (t.a[i] === K || t.a[i] === RED) continue; t.a[i] = t.tint(t.a[i], Math.round((vnoise(13, x / 5, y * 1.4) - 0.5) * 5)); }
  }
}

function shadow(t, back) {
  const s = solids(t); if (!s.length) return;
  const xs = s.map((p) => p[0]), bottom = Math.max(...s.map((p) => p[1])), x0 = Math.min(...xs), x1 = Math.max(...xs), y = Math.min(bottom + 1, LABEL - 1);
  if (y > TABLE) for (let x = x0 + 1; x <= x1; x++) if (t.role[y * W + x] === BACK && t.a[y * W + x] === BRN) t.a[y * W + x] = K;
}

function rustAndTrim(t, q) {
  const r = q.r.fork('rust'), { rust, fine, crude, step } = q.wear;
  for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, role = t.role[i];
    if (role === BACK || role === FIXED || role === LINE) continue;
    if (rust && (role === MAIN || role === METAL) && (role === MAIN ? q.cls : 'metal') === 'metal' && r.chance(0.5)) t.a[i] = r.chance(0.75) ? BRN : RED;
    else if (step === 1 && r.chance(0.16)) t.a[i] = DARK_[t.a[i]] || DGR;
    else if (crude && r.chance(0.1)) t.a[i] = DARK_[t.a[i]] || DGR;
  }
  if (crude) for (const [x, y] of edgesOut(t)) if (r.chance(0.75)) t.put(x, y, LINE);
  if (fine) {
    const s = solids(t, (x, y) => [MAIN, METAL, WOOD].includes(t.role[y * W + x]));
    for (const [x, y] of s) if ((x + y) % 2 === 0 && (!t.is(x - 1, y) || !t.is(x, y - 1))) t.put(x, y, FIXED, q.trim);
    if (s.length) { const cx = s.reduce((a, p) => a + p[0], 0) / s.length, cy = s.reduce((a, p) => a + p[1], 0) / s.length, [gx, gy] = s.reduce((b, p) => ((p[0] - cx) ** 2 + (p[1] - cy) ** 2 < (b[0] - cx) ** 2 + (b[1] - cy) ** 2 ? p : b)); t.put(gx, gy, FIXED, q.gem); t.put(gx, gy - 1, FIXED, WHT); t.put(gx + 1, gy, FIXED, LIGHT[q.gem]); }
  }
}
/** The empty pixels touching a drawn one (eight ways) above the table's front edge: where a rough outline goes. */
function edgesOut(t) { const out = []; for (let y = 0; y < LABEL; y++) for (let x = 0; x < W; x++) if (!t.is(x, y) && [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1]].some(([dx, dy]) => t.is(x + dx, y + dy))) out.push([x, y]); return out; }

function auras(t, q, bg) {
  const r = q.r.fork('aura'), kinds = q.auras.map((a) => [auraKind(a), auraGlow(a)]).filter(([k]) => k);
  const near = (x, y, d) => { let best = 0; for (let dy = -d; dy <= d; dy++) for (let dx = -d; dx <= d; dx++) if (t.is(x + dx, y + dy)) { const k = Math.max(Math.abs(dx), Math.abs(dy)); if (!best || k < best) best = k; } return best; };
  for (const [kind, glow] of kinds) {
    if (kind === 'glow') {
      const g = glow ?? YEL, snap = t.role.slice(), solid = (x, y) => x >= 0 && y >= 0 && x < W && y < H && snap[y * W + x] !== BACK;
      const dist = (x, y) => { for (let d = 1; d <= 2; d++) for (let dy = -d; dy <= d; dy++) for (let dx = -d; dx <= d; dx++) if (Math.max(Math.abs(dx), Math.abs(dy)) === d && solid(x + dx, y + dy)) return d; return 0; };
      for (let y = 0; y < TABLE + 4; y++) for (let x = 0; x < W; x++) { if (snap[y * W + x] !== BACK) continue; const d = dist(x, y); if (t.smooth) { if (d) t.put(x, y, FIXED, t.between(t.a[y * W + x], g, d === 1 ? 12 : 5)); continue; } if (d === 1 || (d === 2 && BAYER[y & 3][x & 3] < 7)) t.put(x, y, FIXED, g); else if (d === 2) t.put(x, y, FIXED, t.a[y * W + x] === K ? DARK_[g] || DGR : t.a[y * W + x]); }
    } else if (kind === 'magic') {
      const s = solids(t), spots = [];
      for (let k = 0; k < 14; k++) { const [px, py] = s[r.int(s.length)] ?? [12, 10], x = px + r.int(7) - 3, y = py + r.int(7) - 3; if (!t.is(x, y) && y < TABLE) spots.push([x, y]); }
      spots.slice(0, 8).forEach(([x, y], i) => { const c = [LMAG, WHT, LCYN, YEL][i % 4]; t.put(x, y, FIXED, c); if (i % 3 === 0) { t.put(x - 1, y, FIXED, c); t.put(x + 1, y, FIXED, c); t.put(x, y - 1, FIXED, c); t.put(x, y + 1, FIXED, c); } });
    } else if (kind === 'glint') {
      const s = solids(t, (x, y) => !t.is(x - 1, y) || !t.is(x, y - 1)); for (let k = 0; k < 3 && s.length; k++) { const [x, y] = s.splice(r.int(s.length), 1)[0]; t.put(x, y, FIXED, WHT); t.put(x - 1, y, FIXED, LCYN); t.put(x + 1, y, FIXED, LCYN); t.put(x, y - 1, FIXED, LCYN); t.put(x, y + 1, FIXED, LCYN); }
    } else if (kind === 'invisible') {
      for (const [x, y] of solids(t)) if ((x + y) % 2 === 0) { const i = y * W + x; t.role[i] = BACK; t.a[i] = bg[i]; }
    } else if (kind === 'hidden') {
      for (const [x, y] of solids(t)) { const i = y * W + x; if (t.smooth) { t.a[i] = t.between(t.a[i], K, 9); continue; } t.a[i] = BAYER[y & 3][x & 3] < 9 ? DARK_[t.a[i]] || K : t.a[i]; if (BAYER[y & 3][x & 3] < 3) t.a[i] = K; }
    } else if (kind === 'bound') {
      const s = solids(t); if (!s.length) continue;
      const x0 = Math.min(...s.map((p) => p[0])), x1 = Math.max(...s.map((p) => p[0])), y0 = Math.min(...s.map((p) => p[1])), y1 = Math.max(...s.map((p) => p[1]));
      const snap = t.role.slice(), rope = (x, y, c) => { if (snap[y * W + x] !== BACK || near(x, y, 0) === 0) t.put(x, y, FIXED, c); };
      for (const [ax, ay, bx, by] of [[x0, y0 + 1, x1, y1 - 1], [x0, y1 - 1, x1, y0 + 1]]) { const n = Math.max(x1 - x0, 1); for (let i = 0; i <= n; i++) { const x = Math.round(ax + ((bx - ax) * i) / n), y = Math.round(ay + ((by - ay) * i) / n); if (snap[y * W + x] !== BACK) rope(x, y, i % 2 ? BRN : YEL); } }
      const mx = Math.round((x0 + x1) / 2), my = Math.round((y0 + y1) / 2); t.disc(mx, my, 1.2, FIXED, YEL);
    }
  }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/**
 * An item picture (ANSI only), 25 by 12 cells: the item on a table with its slot in words on the bottom row. Params: `name` ("a rusty iron
 * longsword": the kind is its first word that names one, a material and wear words are read from it), `material` (EXAMINE's, else read
 * from the name or `description`), `wear` (a word like "battered", a list, or `{ step, rust, crude, fine }`), `auras` (words like "glowing
 * red", "magical aura", "invisible", "floating", "bound"), `worn` (where it was seen worn: the label), `container`, `terrain` (the place it
 * was found: the backdrop), explicit `kind`, `class`, `color` and `backdrop` (VGA indexes), `seed`, `bgLimit`, `colors` (16, 256 or 'true': at 256 and true colour the shading, the grain of each material and the glow are blended tones, 16 colours keep the dither). Returns the Art with the
 * picture, its words (`says`: "Longsword, rusty iron, wielded.") and `figure`, the part-tagged pixels.
 */
export function drawThing(params = {}) {
  const q = readThing(params), bgLimit = params.bgLimit ?? 8, colors = checkColors(params.colors ?? 16);
  q.v = q.r.fork('variation').int(4);
  const pick = q.r.fork('backdrop');
  const clash = new Set([q.main, DARK_[q.main], q.wear.fine ? YEL : -1]);
  let back = q.backdrop ?? q.place ?? pick.pick(BACKDROPS.filter((c) => !clash.has(c)));
  if (clash.has(back) && q.backdrop === undefined) back = BACKDROPS.find((c) => !clash.has(c));
  const t = new Canvas(colors); ground(t, back, q.r.fork('table')); const bg = t.a.slice();
  (PAINT[q.kind] ?? PAINT.Bundle)(t, q);
  wearShape(t, q);
  const floating = q.auras.some((a) => auraKind(a) === 'float'); if (floating) lift(t, 3);
  finish(t, q, back, bg);
  shadow(t, back);
  rustAndTrim(t, q);
  auras(t, q, bg);
  const cells = t.toCells(colors === 16 ? Array.from({ length: 16 }, (_, i) => i) : IDENTITY256), label = q.slot.slice(0, 22), x0 = Math.floor((W - label.length) / 2);
  for (let x = 0; x < W; x++) cells.set(x, LABEL >> 1, { ch: ' ', fg: GRY, bg: K });
  [...label].forEach((ch, i) => cells.set(x0 + i, LABEL >> 1, { ch, fg: GRY, bg: K }));
  const toPart = [0, BODY, BODY, BODY, WORN, DETAIL, DETAIL, DETAIL];
  const parts = Uint8Array.from(t.role, (r, i) => (i >= LABEL * W ? 0 : toPart[r]));
  return {
    id: `thing.${q.kind.toLowerCase()}`, frames: [fitted(cells, colors, bgLimit)], fps: 0, loop: false, says: thingWords(q),
    figure: { w: W, h: H, colors: t.smooth ? Uint32Array.from(t.a, (c) => (c === CLEAR ? CLEAR - 1 : c)) : Uint8Array.from(t.a), ...(colors !== 16 ? { depth: colors } : {}), parts, partNames: THING_PARTS },
    ...(colors !== 16 ? { colors } : {}),
  };
}

/** The facts of a picture in words, for the line beside it: "Longsword, rusty iron, wielded." */
function thingWords(q) {
  const w = q.wear, adjectives = [w.fine && 'fine', w.crude && 'crude', w.rust && 'rusty', ['', 'worn', 'damaged', 'battered', 'broken'][w.step]].filter(Boolean);
  const auraWords = q.auras.map((a) => String(a).replace(/ aura$/, '')).filter((a, i, l) => l.indexOf(a) === i);
  const material = q.material ?? (q.cls === q.info.cls ? undefined : q.cls);
  const description = [...adjectives, ...auraWords, material].filter(Boolean).join(' ');
  return `${[titled(q.word ?? q.kind.toLowerCase()), description].filter(Boolean).join(', ')}, ${q.slot.toLowerCase()}.`;
}
