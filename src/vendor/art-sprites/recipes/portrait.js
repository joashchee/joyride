// The character-creation pictures (ANSI only): a race standing beside a measuring stick marked every foot up to six, so its size shows
// against the others, or a class, a person in its clothes with its tools. Written for docs/coupler-gaps-plan.md Phase E from Coupler's
// portrait rules (recipes/portrait-data.js: 186 races, 49 classes); the painters are this package's own. Two square pixels a cell (the
// half block), so a picture of R rows is 2R pixels tall, from 8 by 4 cells to 80 by 40. A race or class the tables lack is made from its
// name by Coupler's own seed (FNV-1a) and generator (xorshift64*), draw for draw, so the same name looks the same in both. Browser-safe.
import { Px, K, RED, GRN, BRN, GRY, DGR, LRED, LGRN, YEL, WHT, LIT, BLU, checkColors, IDENTITY256, vnoise } from './vista-canvas.js';
import { volume } from './smooth.js';
import { fitted } from './figure.js';
import { hashSeed } from '../rng.js';
import { RACES, CLASSES, CLASS_BULK, ROBED_KINDS, RACE_FALLBACK, CLASS_FALLBACK } from './portrait-data.js';

export const CREATION_KINDS = ['race', 'class'];
export const CLASS_KINDS = Object.keys(CLASS_BULK);
export const RACE_FEATURES = ['none', 'PointedEars', 'TallHat', 'Tusks', 'BeardAndHelmet', 'BareFeet', 'Wings'];
export const RACE_NAMES = RACES.map((r) => r[0]);
export const CLASS_NAMES = CLASSES.map((c) => c[0]);
const COLOUR_NAMES = ['black', 'red', 'green', 'brown', 'blue', 'magenta', 'cyan', 'lightgray', 'darkgray', 'lightred', 'lightgreen', 'yellow', 'lightblue', 'lightmagenta', 'lightcyan', 'white'];
const FEATURE_WORDS = { PointedEars: 'pointed ears', TallHat: 'tall hat', Tusks: 'tusks', BeardAndHelmet: 'beard and helmet', BareFeet: 'bare feet', Wings: 'wings' };
const CLASS_TOOLS = { warrior: 'helmet, belt, sword and shield', caster: 'robe, pointed hat and a glowing staff', rogue: 'hood and a dagger', priest: 'robe, holy symbol and a mace', nature: 'robe, hood of leaves and a growing staff', bard: 'feathered cap and a lute', monk: 'robe, sash and bare hands', crafter: 'apron and a tool' };
const RACE_BY_KEY = new Map(RACES.map((r) => [r[0], r]));
const CLASS_BY_KEY = new Map(CLASSES.map((c) => [c[0], c]));

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const keyOf = (s) => String(s ?? '').toLowerCase().replace(/[^a-z]/g, '');
const titled = (s) => String(s).trim().replace(/\S+/g, (w) => w[0].toUpperCase() + w.slice(1));
const M64 = (1n << 64n) - 1n;

/** Coupler's seed for a name: FNV-1a (64 bit) of the kind ("race" or "class"), a zero byte, the key and four zero bytes (the look count); a BigInt. */
export function portraitSeed(kind, key) {
  const enc = new TextEncoder(); let h = 0xcbf29ce484222325n;
  for (const b of [...enc.encode(kind), 0, ...enc.encode(key), 0, 0, 0, 0]) { h ^= BigInt(b); h = (h * 0x100000001b3n) & M64; }
  return h;
}
/** Coupler's generator, xorshift64*: `next()` a BigInt below 2^64, `below(n)` 0 to n-1 (0 when n <= 0), `between(a, b)` a to b, `chance(p)` a percent. Not for new art: `rng` is; this is for the fallbacks to match Coupler's. */
export function couplerRng(seed) {
  let s = BigInt(seed) | 1n;
  const next = () => { s ^= s >> 12n; s ^= (s << 25n) & M64; s ^= s >> 27n; return (s * 0x2545f4914f6cdd1dn) & M64; };
  const below = (n) => (n <= 0 ? 0 : Number((next() >> 33n) % BigInt(n)));
  return { next, below, between: (a, b) => a + below(b - a + 1), chance: (p) => below(100) < p };
}

/** A colour from a VGA index or a name ("lightred", "grey"); undefined if it is neither. */
function colour(v) {
  if (Number.isInteger(v) && v >= 0 && v <= 15) return v;
  const n = keyOf(v); return n === 'grey' ? 7 : n === 'darkgrey' ? 8 : COLOUR_NAMES.indexOf(n) < 0 ? undefined : COLOUR_NAMES.indexOf(n);
}
const colourParam = (v, what) => { if (v === undefined) return undefined; const c = colour(v); if (c === undefined) throw new Error(`unknown ${what} colour "${v}"`); return c; };

/** Bulk from weight per inch, in tenths of a human's width: round(10 * sqrt((pounds / inches) / (175 / 74))), 6 to 15. */
export const bulkFor = (inches, pounds) => clamp(Math.round(10 * Math.sqrt(pounds / inches / (175 / 74))), 6, 15);

/**
 * What a race is drawn with: its row in the table, else made from its name (Coupler's fallback: size first if unknown, then the look).
 * Returns `{ key, inches, bulk, darkvision, skin, hair (undefined bald), clothes, feature, featureColor, known }`.
 */
export function raceFacts(name) {
  const key = keyOf(name), row = RACE_BY_KEY.get(key);
  if (row && row[10] && row[11]) return { key, inches: row[1], bulk: row[3], darkvision: !!row[4], skin: row[5], hair: row[6] < 0 ? undefined : row[6], clothes: row[7], feature: row[8], featureColor: row[9] < 0 ? undefined : row[9], known: true };
  const r = couplerRng(portraitSeed('race', key)), inches = r.between(40, 96), bulk = r.between(8, 14), darkvision = r.chance(50), f = RACE_FALLBACK;
  const skin = f.skins[r.below(5)], hair = f.hairs[r.below(5)], clothes = f.clothes[r.below(6)], feature = f.features[r.below(4)];
  return { key, inches, bulk, darkvision, skin, hair, clothes, feature, featureColor: undefined, known: false };
}

/** What a class is drawn with: its row, else made from its name (kind, clothes, trim). Returns `{ key, kind, clothes, trim, known }`. */
export function classFacts(name) {
  const key = keyOf(name), row = CLASS_BY_KEY.get(key);
  if (row && row[4]) return { key, kind: row[1], clothes: row[2], trim: row[3], known: true };
  const r = couplerRng(portraitSeed('class', key)), f = CLASS_FALLBACK;
  const kind = f.kinds[r.below(8)], clothes = f.colors[r.below(8)], trim = f.trims[r.below(4)];
  return { key, kind, clothes, trim, known: false };
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The figure

/** The proportions of a figure `tall` pixels high standing on `ground`, as Coupler's rules give them. */
function proportions({ tall, bulk, ground, cx }) {
  const head = Math.max(Math.floor(tall / 6), 2), headHalf = Math.max(Math.floor((head * bulk + 9) / 20), 1), half = Math.max(Math.floor((tall * bulk) / 50), headHalf + 1);
  const top = ground - tall, shoulders = top + head, hips = top + Math.floor((tall * 11) / 20), arm = Math.max(Math.floor(half / 3), 1), hipHalf = Math.max(Math.floor((half * 3) / 4), 1);
  return { cx, tall, ground, head, headHalf, half, top, shoulders, hips, arm, hipHalf, handY: hips + Math.floor((ground - hips) / 4), rightX: cx + half + arm + 1 };
}

/** Body, legs and arms in the clothes; the head and hands in skin. `legs` is a colour, `hem` a colour or undefined for a row along the feet, `bareFeet` a skin colour. */
function body(c, p, { skin, clothes, legs, hem, bareFeet, handsBare }) {
  const { cx, head, headHalf, half, top, shoulders, hips, arm, hipHalf, ground, handY } = p;
  c.rect(cx - headHalf, top, cx + headHalf, top + head - 1, skin);
  for (let y = shoulders; y < hips; y++) { const hw = half - Math.floor(((half / 4) * (y - shoulders)) / Math.max(hips - shoulders, 1)); c.rect(cx - hw, y, cx + hw, y, clothes); }
  c.rect(cx - hipHalf, hips, cx - 1, ground - 1, legs); c.rect(cx + 1, hips, cx + hipHalf, ground - 1, legs);
  if (hem !== undefined) c.rect(cx - hipHalf, ground - 1, cx + hipHalf, ground - 1, hem);
  if (bareFeet !== undefined) { c.rect(cx - hipHalf, ground - 1, cx - 1, ground - 1, bareFeet); c.rect(cx + 1, ground - 1, cx + hipHalf, ground - 1, bareFeet); }
  for (const side of [-1, 1]) {
    const x0 = side < 0 ? cx - half - arm : cx + half + 1, x1 = x0 + arm - 1;
    c.rect(x0, shoulders, x1, handY, clothes); c.rect(x0, handY - (handsBare ? 1 : 0), x1, handY, skin);
  }
}

function eyes(c, p, color, lit) {
  const y = p.top + Math.floor(p.head / 2), gap = Math.max(Math.floor(p.headHalf / 2), 1);
  for (const x of [p.cx - gap, p.cx + gap]) c.set(x, y, color, lit ? LIT : 0);
}

const line = (c, x0, y0, x1, y1, color) => { const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1); for (let i = 0; i <= n; i++) c.set(Math.round(x0 + ((x1 - x0) * i) / n), Math.round(y0 + ((y1 - y0) * i) / n), color); };
const cap = (c, p, color, rows) => c.rect(p.cx - p.headHalf - 1, p.top - (rows > 1 ? 0 : 1), p.cx + p.headHalf + 1, p.top + rows - 1, color);

function raceFeature(c, p, f, skin, clothes, hair) {
  const { cx, head, headHalf, top, shoulders } = p, eyeY = top + Math.floor(head / 2);
  if (f.feature === 'PointedEars') for (const s of [-1, 1]) { c.set(cx + s * (headHalf + 1), eyeY, skin); c.set(cx + s * (headHalf + 2), eyeY - 1, skin); c.set(cx + s * (headHalf + 2), eyeY - 2, skin); }
  else if (f.feature === 'TallHat') {
    const h = clamp(head, 1, Math.max(top, 1));
    c.rect(cx - headHalf - 1, top - 1, cx + headHalf + 1, top, clothes);
    for (let i = 0; i < h; i++) { const w = Math.max(headHalf - Math.floor((i * headHalf) / h), 0); c.rect(cx - w, top - 1 - i, cx + w, top - 1 - i, clothes); }
  } else if (f.feature === 'Tusks') { const y = head < 3 ? top + head : top + head - 1, gap = Math.max(headHalf - 1, 1); c.set(cx - gap, y, WHT); c.set(cx + gap, y, WHT); }
  else if (f.feature === 'BeardAndHelmet') {
    c.rect(cx - headHalf - 1, top - 1, cx + headHalf + 1, top + Math.max(Math.floor(head / 3), 1) - 1, GRY);
    const bh = Math.max(Math.floor(head / 2), 1), by = top + head - bh, bc = hair ?? DGR;
    c.rect(cx - headHalf, by + 1 > top + head - 1 ? top + head - 1 : by + 1, cx + headHalf, top + head - 1, bc); c.rect(cx - Math.max(Math.floor(headHalf / 2), 1), top + head, cx + Math.max(Math.floor(headHalf / 2), 1), shoulders + bh - 1, bc);
  }
}

function wings(c, p, color) {
  const { cx, half, shoulders, tall } = p, wl = Math.max(Math.floor(tall / 3), 2);
  for (let dx = 1; dx <= wl; dx++) for (const s of [-1, 1]) for (let y = shoulders - Math.floor(dx * 0.7); y <= shoulders + Math.floor((wl - dx) * 0.8) + 1; y++) c.set(cx + s * (half + dx), y, color);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The tools of a class

const TOOLS = {
  warrior(c, p, q) {
    cap(c, p, GRY, 2); c.rect(p.cx - p.half, p.hips - 1, p.cx + p.half, p.hips - 1, q.trim);
    line(c, p.rightX, p.handY - 1, p.rightX, Math.max(p.handY - Math.floor(p.tall / 2), 0), 7); c.rect(p.rightX - 1, p.handY - 1, p.rightX + 1, p.handY - 1, YEL);
    const w = Math.max(Math.floor(p.half / 2) + 1, 2), h = Math.max(Math.floor(p.tall / 4), 2);
    c.rect(p.cx - p.half - p.arm - w + 1, p.shoulders + 1, p.cx - p.half, p.shoulders + h, q.trim); c.set(p.cx - p.half - Math.floor(w / 2), p.shoulders + 1 + Math.floor(h / 2), q.clothes);
  },
  caster(c, p, q) {
    const h = clamp(p.head + 1, 2, Math.max(p.top, 1)); c.rect(p.cx - p.headHalf - 1, p.top - 1, p.cx + p.headHalf + 1, p.top, q.clothes);
    for (let i = 0; i < h; i++) { const w = Math.max(p.headHalf - Math.floor((i * p.headHalf) / h), 0); c.rect(p.cx - w, p.top - 1 - i, p.cx + w, p.top - 1 - i, q.clothes); }
    c.set(p.cx, p.top - 1, q.trim);
    line(c, p.rightX, p.ground - 1, p.rightX, Math.max(p.top, 2), BRN); const r = p.tall > 14 ? 1 : 0; c.disc(p.rightX, Math.max(p.top - 1, 1), r, q.trim, LIT); c.set(p.rightX, Math.max(p.top - 1, 1), WHT, LIT);
  },
  rogue(c, p, q) {
    c.rect(p.cx - p.headHalf - 1, p.top - 1, p.cx + p.headHalf + 1, p.top + Math.max(Math.floor(p.head / 3), 1) - 1, q.clothes);
    for (const s of [-1, 1]) c.rect(p.cx + s * (p.headHalf + 1), p.top, p.cx + s * (p.headHalf + 1), p.shoulders, q.clothes);
    line(c, p.rightX, p.handY - 1, p.rightX, p.handY - Math.max(Math.floor(p.tall / 6), 2), 7); c.set(p.rightX, p.handY, q.trim);
  },
  priest(c, p, q) {
    const y = p.shoulders + Math.floor((p.hips - p.shoulders) / 3); c.set(p.cx, y, q.trim); if (p.tall > 12) { c.set(p.cx - 1, y, q.trim); c.set(p.cx + 1, y, q.trim); c.set(p.cx, y - 1, q.trim); c.set(p.cx, y + 1, q.trim); }
    line(c, p.rightX, p.handY, p.rightX, p.handY - Math.max(Math.floor(p.tall / 4), 3), BRN); c.rect(p.rightX - (p.tall > 12 ? 1 : 0), p.handY - Math.max(Math.floor(p.tall / 4), 3) - 1, p.rightX, p.handY - Math.max(Math.floor(p.tall / 4), 3), GRY);
  },
  nature(c, p, q) {
    c.rect(p.cx - p.headHalf - 1, p.top - 1, p.cx + p.headHalf + 1, p.top + Math.max(Math.floor(p.head / 3), 1) - 1, GRN);
    for (let x = p.cx - p.headHalf - 1; x <= p.cx + p.headHalf + 1; x += 2) c.set(x, p.top - 1, LGRN);
    line(c, p.rightX, p.ground - 1, p.rightX, Math.max(p.top, 1), BRN); c.set(p.rightX + 1, Math.max(p.top, 1), LGRN); c.set(p.rightX - 1, Math.max(p.top + 1, 1), GRN); c.set(p.rightX, Math.max(p.top - 1, 0), LGRN);
  },
  bard(c, p, q) {
    c.rect(p.cx - p.headHalf - 1, p.top - 1, p.cx + p.headHalf + 1, p.top, q.clothes); line(c, p.cx + p.headHalf, p.top - 1, p.cx + p.headHalf + 2, p.top - Math.max(Math.floor(p.head / 2), 2), q.trim);
    line(c, p.cx - p.half, p.shoulders + 1, p.cx + p.half - 1, p.hips - 1, BRN); c.disc(p.cx + p.half - 1, p.hips - 2, Math.max(Math.floor(p.half / 3), 1), BRN); c.set(p.cx + p.half - 1, p.hips - 2, K);
  },
  monk(c, p, q) { c.rect(p.cx - p.half, p.hips - 1, p.cx + p.half, p.hips - 1, q.trim); },
  crafter(c, p, q) {
    c.rect(p.cx - Math.max(p.half - 1, 1), p.shoulders + Math.floor((p.hips - p.shoulders) / 3), p.cx + Math.max(p.half - 1, 1), p.ground - Math.floor((p.ground - p.hips) / 3), q.trim);
    if (q.trim === 7) { line(c, p.rightX, p.handY, p.rightX, p.handY - Math.max(Math.floor(p.tall / 6), 2), BRN); c.rect(p.rightX - 1, p.handY - Math.max(Math.floor(p.tall / 6), 2) - 1, p.rightX + 1, p.handY - Math.max(Math.floor(p.tall / 6), 2), DGR); }
    else if (q.trim === 15 || q.trim === 14) c.rect(p.rightX - 1, p.handY - 2, p.rightX + 1, p.handY, q.trim);
    else { c.disc(p.rightX, p.handY - 1, 1, q.trim); c.set(p.rightX, p.handY - 1, K); }
  },
};

// ---------------------------------------------------------------------------------------------------------------------------------

/** The race's picture, `w` by `h` pixels. Returns the facts it was drawn with. */
function paintRace(c, f, ground) {
  const foot = Math.max(Math.floor(((ground - 1) * 12) / 101), 1), tall = Math.min(Math.floor((f.inches * foot) / 12), ground - 1), cx = Math.floor((c.w * 9) / 16);
  const stickX = Math.floor(c.w / 6), p = proportions({ tall, bulk: f.bulk, ground, cx });
  c.rect(stickX, ground - 6 * foot, stickX, ground - 1, DGR);
  for (let n = 1; n <= 6; n++) c.rect(stickX - 1, ground - n * foot, stickX + 2, ground - n * foot, GRY);
  if (f.feature === 'Wings') wings(c, p, f.featureColor ?? WHT);
  body(c, p, { skin: f.skin, clothes: f.clothes, legs: f.feature === 'Tusks' ? f.skin : DGR, bareFeet: f.feature === 'BareFeet' ? f.skin : undefined });
  if (f.hair !== undefined) { const rows = Math.max(Math.floor(p.head / 3), 1); c.rect(p.cx - p.headHalf, p.top, p.cx + p.headHalf, p.top + rows - 1, f.hair); if (p.head > 3) for (const s of [-1, 1]) c.rect(p.cx + s * p.headHalf, p.top + rows, p.cx + s * p.headHalf, p.top + rows, f.hair); }
  eyes(c, p, f.darkvision ? (f.skin === LRED ? RED : LRED) : K, f.darkvision);
  raceFeature(c, p, f, f.skin, f.clothes, f.hair);
  return { foot, tall, p };
}

function paintClass(c, f, ground) {
  const robed = ROBED_KINDS.includes(f.kind), tall = ground - 3, cx = Math.floor(c.w / 2), p = proportions({ tall, bulk: CLASS_BULK[f.kind], ground, cx });
  body(c, p, { skin: LRED, clothes: f.clothes, legs: robed ? f.clothes : DGR, hem: robed ? f.trim : undefined, handsBare: f.kind === 'monk' });
  eyes(c, p, K, false);
  TOOLS[f.kind](c, p, f);
  return { tall, p };
}

/**
 * With 256 colours or true colour: the sky falls from black toward a deep blue at the horizon and the ground darkens with a grain along it,
 * and the figure (every pixel that is not the plain backdrop) takes light from the upper left, a darker lower right edge and a fine grain.
 */
function smoothCreation(c, base, ground, name) {
  const seed = hashSeed(name) & 0xffff, plain = (i) => c.a[i] === base[i], solid = (x, y) => x >= 0 && y >= 0 && x < c.w && y < c.h && !plain(y * c.w + x);
  volume(c, { solid, pick: (x, y) => y < ground && solid(x, y), seed, light: 3, rim: 4, grain: 1.4 });
  for (let y = 0; y < c.h; y++) for (let x = 0; x < c.w; x++) {
    const i = y * c.w + x; if (!plain(i)) continue;
    if (y < ground) { if (base[i] === K) c.a[i] = c.between(K, BLU, Math.round(y * 5 / ground) + (vnoise(seed + 1, x / 4, y / 2) > 0.7 ? 1 : 0)); }
    else if (base[i] === BRN) c.a[i] = c.tint(BRN, -Math.round((y - ground) * 5 / Math.max(1, c.h - ground)) + Math.round((vnoise(seed + 2, x / 3, y * 1.2) - 0.5) * 4));
  }
}

/**
 * A character-creation picture (ANSI only). Params: `kind` ('race', the default, or 'class'), `name` ("Elf", "half elf", "Cleric"; one the tables
 * lack is made from its name, the same every time), `columns` 8 to 80 and `rows` 4 to 40 (the cells; two square pixels a cell), `bgLimit`.
 * A race may be overridden by `inches`, `pounds` (or `bulk`, 6 to 15), `darkvision`, `skin`, `hair` ('none' for bald), `clothes`, `feature`
 * (one of RACE_FEATURES) and `featureColor` (colours are VGA indexes or names); a class by `classKind`, `clothes` and `trim`. Returns the Art
 * with the picture, its words (`says`: "Elf, 5 feet 5, pointed ears, sees in the dark.") and `facts`, what it was drawn with.
 */
export function drawCreation(params = {}) {
  const kind = params.kind ?? 'race';
  if (!CREATION_KINDS.includes(kind)) throw new Error(`unknown creation kind "${kind}"; known: ${CREATION_KINDS.join(', ')}`);
  const name = String(params.name ?? (kind === 'race' ? 'human' : 'fighter')), w = clamp(Math.round(params.columns ?? 32), 8, 80), rows = clamp(Math.round(params.rows ?? 16), 4, 40), h = rows * 2;
  const colors = checkColors(params.colors ?? 16), ground = h - Math.max(Math.floor(h / 10), 2), c = new Px(w, h, colors);
  c.rect(0, 0, w - 1, ground - 1, K); c.rect(0, ground, w - 1, h - 1, BRN); for (let x = 0; x < w; x += 2) c.set(x, ground, GRN);
  const base = c.smooth ? c.a.slice() : undefined;
  let facts, says;
  if (kind === 'race') {
    const f = raceFacts(name);
    if (params.inches !== undefined) f.inches = clamp(Math.round(params.inches), 6, 240);
    if (params.bulk !== undefined) f.bulk = clamp(Math.round(params.bulk), 6, 15); else if (params.pounds !== undefined) f.bulk = bulkFor(f.inches, params.pounds);
    if (params.darkvision !== undefined) f.darkvision = !!params.darkvision;
    f.skin = colourParam(params.skin, 'skin') ?? f.skin; f.clothes = colourParam(params.clothes, 'clothes') ?? f.clothes; f.featureColor = colourParam(params.featureColor, 'feature') ?? f.featureColor;
    if (params.hair !== undefined) f.hair = params.hair === 'none' || params.hair === null ? undefined : colourParam(params.hair, 'hair');
    if (params.feature !== undefined) { if (!RACE_FEATURES.includes(params.feature)) throw new Error(`unknown feature "${params.feature}"; known: ${RACE_FEATURES.join(', ')}`); f.feature = params.feature; }
    const { foot, tall } = paintRace(c, f, ground), feet = Math.floor(f.inches / 12), inch = f.inches % 12;
    says = `${[titled(name), `${feet} ${feet === 1 ? 'foot' : 'feet'}${inch ? ` ${inch}` : ''}`, FEATURE_WORDS[f.feature], f.darkvision && 'sees in the dark'].filter(Boolean).join(', ')}.`;
    facts = { kind, key: f.key, known: f.known, inches: f.inches, bulk: f.bulk, darkvision: f.darkvision, skin: f.skin, hair: f.hair, clothes: f.clothes, feature: f.feature, foot, tall };
  } else {
    const f = classFacts(name);
    if (params.classKind !== undefined) { if (!CLASS_KINDS.includes(params.classKind)) throw new Error(`unknown class kind "${params.classKind}"; known: ${CLASS_KINDS.join(', ')}`); f.kind = params.classKind; }
    f.clothes = colourParam(params.clothes, 'clothes') ?? f.clothes; f.trim = colourParam(params.trim, 'trim') ?? f.trim;
    const { tall } = paintClass(c, f, ground);
    says = `${titled(name)}, ${f.kind}: ${CLASS_TOOLS[f.kind]}.`;
    facts = { kind, key: f.key, known: f.known, classKind: f.kind, clothes: f.clothes, trim: f.trim, tall };
  }
  if (base) smoothCreation(c, base, ground, `${kind}:${name}`);
  const cells = c.toCells(colors === 16 ? Array.from({ length: 16 }, (_, i) => i) : IDENTITY256);
  return { id: `creation.${kind}`, frames: [fitted(cells, colors, params.bgLimit ?? 8)], fps: 0, loop: false, says, facts, ...(colors !== 16 ? { colors } : {}) };
}
