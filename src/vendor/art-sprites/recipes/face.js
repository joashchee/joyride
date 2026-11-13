// The creature face (ANSI only): a creature's head by race, 25 by 24 pixels (two to a cell, so 25 columns by 12 rows), or the small 16 by 16
// (16 by 8 cells) drawn from its own strokes, never the big face scaled. The input is data, not Coupler's types: a race, a name and
// description the cue parser reads, or explicit colours; a mood, a hurt step, effects. Written for the docs/coupler-gaps-plan.md Phase C
// from Coupler's face tables (recipes/face-data.js); the painters are this package's own. Each pixel carries a part (back, body, hair, eye,
// mouth, worn, brow, detail) in `figure`, for the figure layer. Browser-safe.
import { legalize } from '../cells.js';
import { Px, checkColors, IDENTITY256, vnoise } from './vista-canvas.js';
import { volume } from './smooth.js';
import { rng, hashSeed } from '../rng.js';
import { figureOnPlace, CLEAR, fitted } from './figure.js';
import { FACE_RACES, PERSON_RACES, THINKING_RACES, COLOR_WORDS, CUES, NOUNS, WORN_WORDS, METAL_WORDS, EFFECT_LOOKS, EFFECT_MOODS } from './face-data.js';

export const FACE_MOODS = ['Calm', 'Angry', 'Pain', 'Afraid', 'Asleep', 'Dead'];
export const FACE_SIZES = ['full', 'small'];
export const FACE_LOOKS = ['halo', 'poisoned', 'sick', 'burning', 'frozen', 'blind', 'invisible', 'outlined', 'webbed', 'stone', 'bark', 'asleep', 'afraid', 'raging', 'glowing'];
export const HURT_STEPS = ['unhurt', 'scratched', 'cut', 'bruised', 'bleeding', 'near death'];
export const FACE_SHAPES = ['Person', 'Beast(Pointed)', 'Beast(Round)', 'Beast(Wide)', 'Beast(Long)', 'Beast(Horned)', 'Beast(Curled)', 'Bird', 'Scaled(Lizard)', 'Scaled(Snake)', 'Scaled(Frog)', 'Scaled(Fish)', 'Scaled(Dragon)', 'Bug(Insect)', 'Bug(Spider)', 'Bug(Worm)', 'Skull', 'Stuff(Fire)', 'Stuff(Water)', 'Stuff(Air)', 'Stuff(Earth)', 'Stuff(Golem)', 'Stuff(Slime)', 'Stuff(Plant)'];

// VGA indexes (the order of VGA16).
const K = 0, RED = 1, GRN = 2, BRN = 3, BLU = 4, MAG = 5, CYN = 6, GRY = 7, DGR = 8, LRED = 9, LGRN = 10, YEL = 11, LBLU = 12, LMAG = 13, LCYN = 14, WHT = 15;
const DARK_ = [0, 0, 0, 0, 0, 0, 0, 8, 0, 1, 2, 3, 4, 5, 6, 7], LIGHT = [8, 9, 10, 11, 12, 13, 14, 15, 7, 15, 15, 15, 15, 15, 15, 15];
let BACKDROP = 0;                                                       // the face being painted: a shade is never the backdrop's colour
const dk = (c) => { const d = DARK_[c] || DGR; return d === BACKDROP ? (d === DGR ? K : DGR) : d; };
const dim = dk;
const BACK = 0, BODY = 1, HAIR = 2, EYE = 3, MOUTH = 4, WORN = 5, BROW = 6, DETAIL = 7;
export const FACE_PARTS = ['back', 'body', 'hair', 'eye', 'mouth', 'worn', 'brow', 'detail'];
const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

const key = (s) => String(s ?? '').toLowerCase().replace(/[^a-z]/g, '');
const words = (s) => String(s ?? '').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
const RACE_KEYS = new Map(Object.keys(FACE_RACES).map((r) => [key(r), r]));
const LOOKS = new Map(EFFECT_LOOKS.split(';').map((e) => e.split('=')));

// ---------------------------------------------------------------------------------------------------------------------------------
// Reading the facts

/** The head shape of a race (as the game names it) or a category, by Coupler's rule. */
export function faceShape(race, category) {
  const rk = key(race), known = RACE_KEYS.get(rk);
  if (PERSON_RACES.includes(rk)) return 'Person';
  if (known) return FACE_RACES[known][1];
  const c = key(category ?? race);
  const map = { canine: 'Beast(Pointed)', mustelid: 'Beast(Pointed)', feline: 'Beast(Pointed)', ursine: 'Beast(Round)', rodent: 'Beast(Round)', primate: 'Beast(Round)', seamammal: 'Beast(Round)', animal: 'Beast(Round)', pachyderm: /elephant|mammoth/.test(rk) ? 'Beast(Wide)' : 'Beast(Round)', pteropine: 'Beast(Wide)', equine: 'Beast(Long)', bovine: 'Beast(Horned)', porcine: 'Beast(Horned)', ovine: 'Beast(Curled)', avian: 'Bird', reptile: 'Scaled(Lizard)', ophidian: 'Scaled(Snake)', amphibian: 'Scaled(Frog)', fish: 'Scaled(Fish)', dragon: 'Scaled(Dragon)', insect: 'Bug(Insect)', arachnid: 'Bug(Spider)', worm: 'Bug(Worm)', slime: 'Stuff(Slime)', vegetation: 'Stuff(Plant)', stonegolem: 'Stuff(Golem)', metalgolem: 'Stuff(Golem)', woodgolem: 'Stuff(Golem)', golem: 'Stuff(Golem)' };
  if (c === 'undead') return /zombie|ghoul|ghast|vampire|ghost|spectre|wraith|banshee/.test(rk) ? 'Person' : 'Skull';
  if (c === 'elemental') return /fire/.test(rk) ? 'Stuff(Fire)' : /water/.test(rk) ? 'Stuff(Water)' : /air/.test(rk) ? 'Stuff(Air)' : 'Stuff(Earth)';
  return map[c] ?? 'Person';
}

const colorOf = (ws, nouns, bright) => {
  for (let i = 0; i < ws.length; i++) if (nouns.includes(ws[i])) for (let j = i - 1; j >= Math.max(0, i - 3); j--) for (const [plain, br, list] of COLOR_WORDS) if (list.includes(ws[j])) return bright ? br : plain;
  return undefined;
};
const has = (ws, list) => ws.some((w) => list.includes(w));

/** What a description says: colours of skin, hair, eyes and coat, and the cues (beard, scar, patch, freckles, horns, tusks, fangs, glow, halo, age). */
export function readFaceWords(text) {
  const ws = words(text), joined = ` ${ws.join(' ')} `, any = (list) => list.some((w) => joined.includes(` ${w} `));
  let skin = colorOf(ws, NOUNS.skin, false);
  if (skin === undefined) skin = ws.includes('pale') || ws.includes('pallid') || ws.includes('ghostly') || ws.includes('wan') ? GRY : ws.some((w) => ['tanned', 'swarthy', 'dusky', 'bronzed'].includes(w)) ? BRN : undefined;
  return {
    skin, hair: colorOf(ws, NOUNS.hair, false), eyes: colorOf(ws, NOUNS.eyes, true), coat: colorOf(ws, NOUNS.coat, false),
    bald: any(CUES.bald), beard: any(CUES.beard) && !any(CUES.clean), clean: any(CUES.clean), mustache: any(CUES.mustache), scar: any(CUES.scar), patch: any(CUES.patch),
    freckles: any(CUES.freckles), tattoo: any(CUES.tattoo), horns: any(CUES.horns), tusks: any(CUES.tusks), fangs: any(CUES.fangs), glow: any(CUES.glowEyes),
    halo: any(CUES.halo), old: any(CUES.old), longHair: any(CUES.longHair), kind: any(CUES.kind), arms: has(ws, ['arm', 'arms']),
  };
}

/** The look an effect's name gives (Coupler's table, 160 names to 15 looks), or undefined. The name is trimmed of ( ) and a " spell" suffix. */
export function effectLook(name) { return LOOKS.get(String(name).toLowerCase().replace(/[()]/g, '').replace(/ spell$/, '').trim()); }

/** The hurt step 0 to 5 for a health line: a step number, a fraction of health left (0 to 1 as `{ health }`), or CoffeeMUD's health text read by its words. */
export function hurtStep(text) {
  if (typeof text === 'number') return Math.max(0, Math.min(5, Math.round(text)));
  if (text && typeof text === 'object' && typeof text.health === 'number') return Math.max(0, Math.min(5, Math.ceil((1 - text.health) * 5 - 1e-9)));
  const t = String(text ?? '').toLowerCase();
  if (/near death|nearly dead|almost dead|dying|death's door|barely|critical/.test(t)) return 5;
  if (/bleed|bloody|badly|gashe|gaping|mangled/.test(t)) return 4;
  if (/bruis|battered|hurt|wounded|beaten/.test(t)) return 3;
  if (/\bcuts?\b|slashed|gash/.test(t)) return 2;
  if (/scratch|nicked|grazed|slightly/.test(t)) return 1;
  return 0;
}

const SKINS = { 'Beast(Pointed)': [BRN, DGR, GRY, YEL], Bird: [BRN, WHT, DGR, BLU, RED], 'Bug(Insect)': [BRN, RED, GRN], 'Bug(Spider)': [BRN, DGR, RED], 'Bug(Worm)': [BRN, MAG, GRY] };
const SKIN_FALLBACK = { Person: [GRY, BRN, YEL, LMAG], 'Beast(Round)': [BRN, DGR, GRY], 'Beast(Wide)': [DGR, BRN], 'Beast(Long)': [BRN, YEL, GRY], 'Beast(Horned)': [WHT, BRN, DGR], 'Beast(Curled)': [WHT, GRY, BRN], 'Scaled(Lizard)': [GRN, BRN, GRY], 'Scaled(Snake)': [GRN, BRN, YEL], 'Scaled(Frog)': [LGRN, GRN, BRN], 'Scaled(Fish)': [LBLU, BLU, YEL, GRY], 'Scaled(Dragon)': [RED, GRN, BLU, DGR], Skull: [WHT, GRY], 'Stuff(Fire)': [LRED], 'Stuff(Water)': [LBLU], 'Stuff(Air)': [GRY], 'Stuff(Earth)': [BRN, DGR], 'Stuff(Golem)': [GRY], 'Stuff(Slime)': [LGRN, LBLU, LMAG], 'Stuff(Plant)': [GRN, LGRN] };
const EYE_FALLBACK = { Person: [BLU, BRN, GRN, DGR], 'Beast(Pointed)': [YEL, LGRN, BRN] };

/** The facts a face is drawn from, read from the params. See `drawCreatureFace`. */
export function readFace(p = {}) {
  const text = `${p.name ?? ''} ${p.description ?? ''}`, w = readFaceWords(text), ws = words(p.name ?? '');
  let race = p.race;
  if (!race) for (let n = Math.min(3, ws.length); n >= 1 && !race; n--) for (let i = 0; i + n <= ws.length && !race; i++) race = RACE_KEYS.get(key(ws.slice(i, i + n).join('')));
  race ??= p.category ?? 'Human';
  const rk = key(race), shape = p.shape ?? faceShape(race, p.category);
  if (!FACE_SHAPES.includes(shape)) throw new Error(`unknown face shape "${shape}"; known: ${FACE_SHAPES.join(', ')}`);
  const wears = (p.wears ?? []).map(String), family = shape.split('(')[0], r = rng(`face:${key(p.name || race)}:${p.seed ?? ''}`);
  const beastly = ['Beast', 'Bird', 'Scaled'].includes(family) && shape !== 'Scaled(Fish)';
  const sapient = p.sapient ?? (shape === 'Person' || THINKING_RACES.includes(rk) || (beastly && [wears.length > 0, !!p.skilled, w.arms].filter(Boolean).length >= 2));
  const animal = p.animal ?? (!sapient && beastly);
  const male = /^m/i.test(p.sex ?? ''), female = /^f/i.test(p.sex ?? '');
  const skin = p.skin ?? w.coat ?? w.skin ?? (shape === 'Stuff(Golem)' ? (/metal|iron|steel/.test(rk) ? DGR : /wood/.test(rk) ? BRN : GRY) : r.pick(SKINS[shape] ?? SKIN_FALLBACK[shape]));
  const hair0 = p.hair ?? w.hair ?? r.pick([BRN, DGR, YEL, RED]), hair = shape === 'Person' || p.hair !== undefined || w.hair !== undefined ? (hair0 === skin ? (skin === DGR ? WHT : DGR) : hair0) : undefined;
  const eyes = p.eyes ?? w.eyes ?? r.pick(EYE_FALLBACK[shape] ?? [BRN, YEL, LRED]);
  const backdrop = p.backdrop === 'clear' ? K : p.backdrop ?? r.pick([BLU, MAG, CYN, GRN, RED, BRN].filter((c) => c !== skin && c !== hair && c !== DARK_[skin]));
  const beard = p.beard ?? (w.beard || (!w.clean && male && r.chance(0.3)));
  const longHair = p.longHair ?? (w.longHair || (female && r.chance(0.8)));
  const ghostly = /ghost|spectre|specter|spirit|shadow|wraith|banshee|phantom/.test(rk) || w.skin === GRY && /ghostly/.test(text);
  const evil = p.evil ?? /evil|chaotic|wicked|cruel|malevolent/.test(`${p.alignment ?? ''} ${text}`);
  const horns = p.horns ?? (w.horns || /tiefling|demon|devil|minotaur|satyr|imp/.test(rk)), fangs = p.fangs ?? (w.fangs || /vampire/.test(rk));
  const halo = p.halo ?? (w.halo || key(FACE_RACES[RACE_KEYS.get(rk)]?.[0]) === 'celestial');
  const looks = [...new Set([...(p.looks ?? []), ...(p.effects ?? []).map((e) => (FACE_LOOKS.includes(e) ? e : effectLook(e))).filter(Boolean)])];
  const lookMood = looks.map((l) => EFFECT_MOODS[l.replace(/^./, (c) => c.toUpperCase())]).find(Boolean);
  const moodName = String(p.mood ?? lookMood ?? 'calm').toLowerCase().replace(/^in /, ''), mood = FACE_MOODS.findIndex((m) => m.toLowerCase() === moodName);
  if (mood < 0) throw new Error(`unknown mood "${p.mood}"; known: ${FACE_MOODS.join(', ')}`);
  if (halo && !looks.includes('halo')) looks.push('halo');
  const worn = {};
  for (const item of wears) for (const [slot, list] of Object.entries(WORN_WORDS)) if (!worn[slot]) { const iw = words(item), hit = iw.find((x) => list.includes(x)); if (hit) worn[slot] = { word: hit, color: itemColor(iw) }; }
  return {
    race, shape, family, sapient, animal, skin, hair: w.bald ? undefined : hair, eyes, backdrop, evil, ghostly, looks, mood, hurt: hurtStep(p.hurt), worn, r,
    cloth: p.cloth ?? r.pick([BLU, GRN, RED, MAG, BRN]), flags: { beard, mustache: p.mustache ?? w.mustache, scar: p.scar ?? w.scar, patch: p.patch ?? w.patch, freckles: p.freckles ?? w.freckles, tattoo: p.tattoo ?? w.tattoo, horns, tusks: p.tusks ?? w.tusks, fangs, glow: p.glow ?? w.glow, halo, old: p.old ?? w.old, longHair, bald: w.bald },
  };
}
const itemColor = (iw) => { for (const x of iw) for (const [plain, , list] of COLOR_WORDS) if (list.includes(x)) return plain; return iw.some((x) => METAL_WORDS.includes(x)) ? GRY : undefined; };

// ---------------------------------------------------------------------------------------------------------------------------------
// Drawing helpers

class Canvas extends Px {
  constructor(w, h, back, depth = 16) { super(w, h, depth); this.a.fill(back); this.parts = new Uint8Array(w * h); this.back = back; }
  put(x, y, c, part = BODY) { x = Math.floor(x); y = Math.floor(y); if (x < 0 || y < 0 || x >= this.w || y >= this.h) return; const i = y * this.w + x; this.a[i] = c; this.parts[i] = part; }
  part(x, y) { return x < 0 || y < 0 || x >= this.w || y >= this.h ? BACK : this.parts[y * this.w + x]; }
  solid(x, y) { return this.part(x, y) !== BACK; }
  ell(cx, cy, rx, ry, c, part = BODY) { for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++) for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++) if (((x - cx) / (rx + 0.3)) ** 2 + ((y - cy) / (ry + 0.3)) ** 2 <= 1) this.put(x, y, c, part); }
  box(x0, y0, x1, y1, c, part = BODY) { for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) this.put(x, y, c, part); }
  line(x0, y0, x1, y1, c, part = DETAIL) { const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1); for (let i = 0; i <= n; i++) this.put(Math.round(x0 + (x1 - x0) * i / n), Math.round(y0 + (y1 - y0) * i / n), c, part); }
  /** The same drawing mirrored about the vertical centre. */
  both(fn) { fn(false); fn(true); }
  mx(x) { return this.w - 1 - x; }
  /** Light from the upper left: a dark edge on the right and below, a bright spot top left, over the body pixels of an ellipse. */
  shade(cx, cy, rx, ry, c) {
    for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++) for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++) {
      if (this.part(x, y) !== BODY || this.at(x, y) !== c || !DARK_[c]) continue;
      const d = ((x - cx) / (rx + 0.3)) ** 2 + ((y - cy) / (ry + 0.3)) ** 2;
      if (this.smooth) {                                                // a blend, where 16 colours dither
        if (d > 0.62 && (x > cx || y > cy + ry * 0.4)) { const t = Math.round(Math.min(1, (d - 0.5) * 2) * 10); if (t > 0) this.put(x, y, dk(c) === K ? this.tint(c, -Math.round(t * 0.7)) : this.between(c, dk(c), t), BODY); }
        else if (d < 0.5 && x < cx && y < cy - ry * 0.1) { const t = Math.round((0.5 - d) * 14); if (t > 0) this.put(x, y, this.between(c, LIGHT[c], t), BODY); }
        continue;
      }
      if (d > 0.62 && (x > cx || y > cy + ry * 0.4) && BAYER[y & 3][x & 3] < (d - 0.5) * 22) this.put(x, y, dk(c), BODY);
      else if (d < 0.5 && x < cx && y < cy - ry * 0.1 && BAYER[y & 3][x & 3] < 5) this.put(x, y, LIGHT[c], BODY);
    }
  }
  at(x, y) { return this.get(x, y); }
}


// ---------------------------------------------------------------------------------------------------------------------------------
// The full head (25 by 24): one painter per shape, each returns its anchors (where the eyes, brows and mouth go)

const FULL = {
  Person(cv, s) {
    const { skin, hair } = s, f = s.flags, bald = hair === undefined;
    cv.ell(12, 11, 6, 8, skin);
    if (!bald) {
      for (let y = 2; y <= 20; y++) for (let x = 3; x <= 21; x++) {
        const inHead = ((x - 12) / 6.3) ** 2 + ((y - 11) / 8.3) ** 2 <= 1, vol = ((x - 12) / 7.8) ** 2 + ((y - 10.5) / 9.3) ** 2 <= 1;
        if (inHead && (y <= 7 || (Math.abs(x - 12) >= 6 && y <= 11 && !f.longHair))) cv.put(x, y, hair, HAIR);
        else if (!inHead && vol && (y <= 9 || (f.longHair && y <= 20 && Math.abs(x - 12) >= 6))) cv.put(x, y, hair, HAIR);
      }
      for (let x = 8; x <= 16; x++) if (cv.part(x, 8) === BODY && BAYER[8 & 3][x & 3] < 7) cv.put(x, 8, hair, HAIR);
    } else for (let x = 9; x <= 12; x++) cv.put(x, 4, LIGHT[skin], BODY);
    cv.box(10, 19, 14, 21, dim(skin), BODY); cv.shade(12, 11, 6, 8, skin);
    for (const x of [5, 19]) { cv.put(x, 11, skin, BODY); cv.put(x, 12, dim(skin), BODY); }
    cv.put(12, 12, dim(skin), BODY); cv.put(12, 13, dim(skin), BODY); cv.put(11, 13, dim(skin), BODY);
    shoulders(cv, s);
    return { ey: 10, ex: 9, by: 9, my: 15, mw: 5, ew: 2, top: 3, chin: 19, side: 5 };
  },
  Beast(cv, s) {
    const v = s.shape.slice(6, -1), cx = s.animal ? 9 : 12, sk = s.skin, d = dim(sk), l = LIGHT[sk];
    if (s.animal) { cv.ell(19, 22, 8, 5, sk); cv.box(cx, 19, 24, 23, sk); } else shoulders(cv, s);
    const head = { Pointed: [7, 5.5], Round: [7, 6], Wide: [5.5, 5.5], Long: [5.5, 7], Horned: [6.5, 6], Curled: [5.5, 6.5] }[v];
    cv.ell(cx, 12, head[0], head[1], sk);
    const ear = (side, fn) => cv.both((m) => fn(m ? cx + side : cx - side, m ? -1 : 1));
    if (v === 'Pointed') { ear(5, (x, k) => { for (let i = 0; i < 5; i++) { cv.put(x, 3 + i, sk); if (i > 0) cv.put(x + k, 3 + i, i < 3 ? d : sk); } }); }
    if (v === 'Round') { ear(6, (x) => { cv.ell(x, 7, 2, 2, sk); cv.put(x, 7, d); cv.put(x, 8, d); }); }
    if (v === 'Wide') { ear(6, (x, k) => { for (let i = 0; i < 6; i++) for (let j = 0; j <= i; j++) cv.put(x + k * j, 6 + i, i % 3 === 1 && j > 1 ? d : sk); cv.put(x, 9, d); }); }
    if (v === 'Long') { ear(4, (x, k) => { for (let i = 0; i < 5; i++) { cv.put(x, 4 + i, sk); cv.put(x + k, 4 + i, i < 2 ? sk : d); } }); if (s.flags.horns || /deer|elk|moose|stag/.test(key(s.race))) ear(3, (x, k) => { cv.line(x, 6, x + k * 2, 1, WHT, DETAIL); cv.line(x + k, 4, x + k * 3, 3, WHT, DETAIL); }); }
    if (v === 'Horned') { ear(6, (x, k) => { cv.line(x + k, 8, x + k * 2, 5, WHT, DETAIL); cv.line(x + k * 2, 5, x + k * 1, 3, WHT, DETAIL); cv.put(x + k * 3, 9, sk); cv.put(x + k * 4, 10, sk); }); }
    if (v === 'Curled') { ear(5, (x, k) => { cv.line(x, 8, x + k * 2, 5, GRY, DETAIL); cv.line(x + k * 2, 5, x + k * 4, 6, GRY, DETAIL); cv.line(x + k * 4, 6, x + k * 4, 8, DGR, DETAIL); cv.put(x + k * 3, 9, d); cv.put(x + k * 3, 10, sk); }); cv.put(cx, 19, l, BODY); cv.line(cx, 19, cx, 21, WHT, DETAIL); }
    // muzzle
    const mz = { Pointed: [4, 15, 3, 3.5], Round: [3, 15, 3, 2.5], Wide: [3, 16, 3, 2], Long: [4, 16.5, 3, 3.5], Horned: [4.5, 16, 4, 3], Curled: [3, 16, 3, 3] }[v];
    cv.ell(cx, mz[1], mz[0], mz[3], v === 'Round' ? LIGHT[sk] : l); cv.shade(cx, 12, head[0], head[1], sk);
    cv.box(cx - 1, mz[1] - 1, cx + 1, mz[1] - 1, K, DETAIL); cv.put(cx, mz[1], K, DETAIL);
    if (v === 'Horned') { cv.put(cx - 2, mz[1] - 1, K, DETAIL); cv.put(cx + 2, mz[1] - 1, K, DETAIL); }
    return { ey: 12, ex: cx - 4, by: 11, my: Math.min(19, Math.round(mz[1] + mz[3] - 0.5)), mw: 3, mcx: cx, ew: 2, top: 3, chin: 19, side: 5, cx, nose: true };
  },
  Bird(cv, s) {
    const sk = s.skin, d = dim(sk), l = LIGHT[sk];
    cv.ell(12, 11, 6, 7, sk); cv.box(9, 18, 15, 21, d); cv.shade(12, 11, 6, 7, sk);
    for (const [x, y] of [[10, 3], [12, 2], [14, 3], [11, 4], [13, 4]]) { cv.put(x, y, sk, BODY); cv.put(x, y + 1, sk, BODY); }
    for (let y = 14; y <= 20; y++) for (let x = 5; x <= 19; x++) if (cv.part(x, y) === BODY && (x + y) % 3 === 0) cv.put(x, y, l, BODY);
    const beak = s.skin === YEL ? BRN : YEL;
    for (let i = 0; i < 6; i++) for (let x = 12 - Math.floor((6 - i) / 2); x <= 12 + Math.floor((6 - i) / 2); x++) cv.put(x, 11 + i, i === 0 ? beak : beak, DETAIL);
    cv.put(12, 11, dk(beak), DETAIL); cv.line(12, 16, 12, 17, K, MOUTH);
    shoulders(cv, s, 22);
    return { ey: 9, ex: 7, by: 8, my: 17, mw: 1, ew: 2, top: 2, chin: 19, side: 5, beak: true };
  },
  Scaled(cv, s) {
    const v = s.shape.slice(7, -1), sk = s.skin, d = dim(sk), l = LIGHT[sk];
    const dither = (cx, cy, rx, ry) => { for (let y = cy - ry - 1; y <= cy + ry + 1; y++) for (let x = cx - rx - 1; x <= cx + rx + 1; x++) if (cv.part(x, y) === BODY && cv.at(x, y) === sk && (x * 2 + y * 3) % 7 === 0) cv.put(x, y, d, BODY); };
    if (v === 'Fish') {
      cv.ell(11, 12, 9, 6, sk); cv.line(20, 8, 24, 5, d, BODY); cv.line(20, 16, 24, 19, d, BODY); cv.box(21, 8, 23, 16, d); cv.line(8, 6, 13, 3, d, BODY); cv.line(8, 18, 12, 21, d, BODY);
      cv.ell(12, 12, 7, 5, l, BODY); dither(11, 12, 9, 6); cv.shade(11, 12, 9, 6, sk); cv.line(14, 8, 14, 16, d, DETAIL); cv.line(15, 9, 15, 15, d, DETAIL);
      cv.put(3, 13, K, MOUTH); cv.put(4, 13, K, MOUTH);
      return { ey: 10, ex: 5, by: 9, my: 14, mw: 3, mcx: 5, ew: 2, top: 4, chin: 19, side: 3, oneEye: true, fish: true };
    }
    if (v === 'Snake') {
      cv.ell(12, 10, 5, 7, sk); cv.ell(12, 16, 3, 3, sk); cv.box(11, 18, 13, 21, d); dither(12, 11, 5, 7); cv.shade(12, 11, 5, 8, sk);
      for (let i = 0; i < 4; i++) cv.put(12 + (i % 2 ? 1 : -1), 3 + i, l, BODY);
      cv.line(12, 19, 12, 21, LRED, MOUTH); cv.put(11, 22, LRED, MOUTH); cv.put(13, 22, LRED, MOUTH);
      shoulders(cv, s, 23);
      return { ey: 9, ex: 8, by: 8, my: 18, mw: 3, ew: 2, top: 3, chin: 19, side: 6, slit: true };
    }
    if (v === 'Frog') {
      cv.ell(12, 14, 9, 6, sk); cv.ell(12, 18, 7, 3, l, BODY); cv.both((m) => cv.ell(m ? 18 : 6, 7, 3, 3, sk)); dither(12, 14, 9, 6); cv.shade(12, 13, 9, 7, sk);
      cv.both((m) => cv.ell(m ? 18 : 6, 7, 2, 2, YEL, DETAIL));
      shoulders(cv, s, 22);
      return { ey: 7, ex: 5, by: 5, my: 17, mw: 9, ew: 2, top: 4, chin: 20, side: 3, frog: true };
    }
    // Lizard and Dragon: a long head seen from the front, snout wide at the bottom
    cv.ell(12, 10, 6, 6, sk); cv.box(8, 14, 16, 19, sk); cv.line(8, 19, 16, 19, d, BODY); dither(12, 12, 6, 8); cv.shade(12, 12, 6, 8, sk);
    cv.both((m) => { const x = m ? 16 : 8; cv.put(x, 15, d); cv.put(x, 16, d); });
    cv.put(10, 16, K, DETAIL); cv.put(14, 16, K, DETAIL);
    if (v === 'Dragon') cv.both((m) => { const x = m ? 19 : 5; cv.line(x, 8, x + (m ? 2 : -2), 3, WHT, DETAIL); cv.line(x + (m ? 2 : -2), 3, x + (m ? 3 : -3), 2, GRY, DETAIL); cv.line(x, 11, x + (m ? 3 : -3), 11, d, DETAIL); cv.line(x, 13, x + (m ? 3 : -3), 14, d, DETAIL); });
    else for (const x of [11, 12, 13]) cv.put(x, 3, d, BODY);
    shoulders(cv, s, 22);
    return { ey: 9, ex: 7, by: 8, my: 18, mw: 7, ew: 2, top: 3, chin: 19, side: 5, slit: v === 'Dragon' };
  },
  Bug(cv, s) {
    const v = s.shape.slice(4, -1), sk = s.skin, d = dim(sk), l = LIGHT[sk];
    if (v === 'Worm') {
      for (let i = 0; i < 4; i++) cv.ell(12, 9 + i * 4, 7 - (i === 3 ? 2 : 0), 3, sk, BODY);
      for (let y = 6; y <= 22; y += 4) for (let x = 5; x <= 19; x++) if (cv.solid(x, y)) cv.put(x, y, d, BODY);
      cv.ell(12, 9, 6, 5, l, BODY); cv.shade(12, 9, 7, 5, l); cv.ell(12, 15, 3, 2, K, MOUTH);
      for (const x of [10, 12, 14]) cv.put(x, 14, WHT, DETAIL);
      return { ey: 8, ex: 8, by: 7, my: 16, mw: 1, ew: 2, top: 4, chin: 19, side: 5, noMouth: true };
    }
    if (v === 'Spider') {
      cv.ell(12, 11, 6, 7, sk); cv.ell(12, 7, 4, 3, d, BODY);
      cv.both((m) => { for (let i = 0; i < 3; i++) cv.line(m ? 18 : 6, 9 + i * 3, m ? 24 : 0, 6 + i * 3, sk === DGR ? K : DGR, BODY); });
      for (const [x, y] of [[10, 5], [14, 5], [9, 7], [15, 7]]) cv.put(x, y, s.eyes, EYE);
      cv.both((m) => { cv.line(m ? 14 : 10, 16, m ? 15 : 9, 20, WHT, DETAIL); cv.put(m ? 15 : 9, 21, LRED, DETAIL); });
      cv.shade(12, 11, 6, 7, sk);
      return { ey: 9, ex: 8, by: 8, my: 15, mw: 3, ew: 2, top: 4, chin: 20, side: 5, spider: true };
    }
    cv.ell(12, 13, 5, 6, sk); cv.ell(12, 7, 4, 3, l); cv.both((m) => { cv.ell(m ? 18 : 6, 9, 3, 3, s.eyes, EYE); cv.line(m ? 12 : 12, 4, m ? 18 : 6, 1, sk, DETAIL); cv.put(m ? 18 : 6, 1, l, DETAIL); cv.line(m ? 14 : 10, 18, m ? 17 : 7, 21, d, DETAIL); cv.line(m ? 15 : 9, 19, m ? 12 : 12, 21, d, DETAIL); });
    for (let y = 6; y <= 12; y++) for (let x = 3; x <= 21; x++) if (cv.part(x, y) === EYE && (x + y) % 2 === 0) cv.put(x, y, dk(s.eyes), EYE);
    cv.shade(12, 13, 5, 6, sk);
    return { ey: 9, ex: 5, by: 6, my: 16, mw: 3, ew: 2, top: 1, chin: 21, side: 3, insect: true };
  },
  Skull(cv, s) {
    const sk = s.skin, d = dim(sk);
    cv.ell(12, 9, 7, 7, sk); cv.box(8, 14, 16, 19, sk); cv.put(8, 14, K); cv.put(16, 14, K);
    cv.both((m) => { cv.ell(m ? 16 : 8, 10, 2.2, 2.2, K, EYE); }); cv.put(12, 13, K, DETAIL); cv.put(11, 14, K, DETAIL); cv.put(13, 14, K, DETAIL);
    for (let x = 8; x <= 16; x++) { cv.put(x, 17, WHT, MOUTH); cv.put(x, 18, x % 2 ? d : WHT, MOUTH); cv.put(x, 19, d, MOUTH); }
    for (let y = 15; y <= 16; y++) for (let x = 8; x <= 16; x++) cv.put(x, y, x % 2 ? d : sk, BODY);
    cv.shade(12, 9, 7, 7, sk); cv.line(11, 3, 12, 5, d, DETAIL);
    cv.box(10, 20, 14, 21, d); shoulders(cv, s, 22);
    return { ey: 10, ex: 7, by: 8, my: 18, mw: 7, ew: 3, top: 2, chin: 19, side: 5, socket: true };
  },
  Stuff(cv, s) {
    const v = s.shape.slice(6, -1), sk = s.skin, d = dim(sk), l = LIGHT[sk];
    const noise = (cx, cy, rx, ry, a, b) => { for (let y = cy - ry - 1; y <= cy + ry + 1; y++) for (let x = cx - rx - 1; x <= cx + rx + 1; x++) if (cv.part(x, y) === BODY && BAYER[y & 3][x & 3] < 6 && (x * 3 + y) % 5 < 2) cv.put(x, y, b, BODY); };
    if (v === 'Fire') {
      cv.ell(12, 13, 6, 7, sk); for (let i = 0; i < 5; i++) { const x = 6 + i * 3 + (i % 2), h = 5 + (i % 3) * 2; for (let y = 0; y < h; y++) cv.put(x, 8 - y + 2, y > h / 2 ? YEL : sk, BODY), cv.put(x + 1, 8 - y + 3, y > h / 2 ? YEL : sk, BODY); }
      cv.ell(12, 14, 4, 5, YEL, BODY); cv.ell(12, 16, 2, 3, WHT, BODY); noise(12, 13, 6, 7, sk, LIGHT[sk]);
      return { ey: 12, ex: 8, by: 11, my: 17, mw: 5, ew: 2, top: 1, chin: 20, side: 6, hot: true };
    }
    if (v === 'Water') {
      cv.ell(12, 12, 7, 8, sk); cv.ell(11, 11, 4, 5, l, BODY); for (let y = 4; y <= 20; y += 3) for (let x = 6; x <= 18; x++) if (cv.solid(x, y) && (x + y) % 2 === 0) cv.put(x, y, WHT, DETAIL);
      cv.shade(12, 12, 7, 8, sk); cv.put(11, 5, WHT, DETAIL); cv.put(12, 5, WHT, DETAIL);
      return { ey: 11, ex: 8, by: 10, my: 17, mw: 5, ew: 2, top: 4, chin: 20, side: 5 };
    }
    if (v === 'Air') {
      for (let y = 3; y <= 21; y++) for (let x = 4; x <= 20; x++) { const q = ((x - 12) / 8) ** 2 + ((y - 12) / 9) ** 2; if (q <= 1 && (x + y) % 2 === (q > 0.55 ? 0 : (x + y) % 2)) cv.put(x, y, q < 0.4 ? WHT : q < 0.7 ? GRY : DGR, BODY); }
      cv.both((m) => { cv.line(m ? 19 : 5, 10, m ? 22 : 2, 12, GRY, BODY); cv.line(m ? 22 : 2, 12, m ? 20 : 4, 15, GRY, BODY); });
      return { ey: 10, ex: 8, by: 9, my: 16, mw: 5, ew: 2, top: 3, chin: 21, side: 4, air: true };
    }
    if (v === 'Earth') {
      for (let y = 3; y <= 21; y++) for (let x = 4; x <= 20; x++) { const q = Math.abs(x - 12) / 8 + Math.abs(y - 12) / 9.5; if (q <= 1) cv.put(x, y, (x * 7 + y * 3) % 7 < 2 ? d : (x + y) % 5 === 0 ? l : sk, BODY); }
      cv.line(8, 4, 10, 8, K, DETAIL); cv.line(15, 18, 17, 13, K, DETAIL); cv.shade(12, 12, 8, 9, sk);
      return { ey: 10, ex: 8, by: 9, my: 16, mw: 5, ew: 2, top: 3, chin: 21, side: 4 };
    }
    if (v === 'Golem') {
      cv.box(6, 3, 18, 18, sk); cv.box(5, 5, 19, 7, sk); cv.box(9, 19, 15, 21, d); for (let x = 6; x <= 18; x++) { cv.put(x, 10, d); cv.put(x, 14, d); }
      for (const [x, y] of [[7, 4], [17, 4], [7, 17], [17, 17]]) cv.put(x, y, l, DETAIL); for (let y = 4; y <= 17; y++) cv.put(12, y, y === 10 || y === 14 ? K : d, BODY);
      cv.shade(12, 11, 7, 8, sk); shoulders(cv, s, 22);
      return { ey: 8, ex: 8, by: 7, my: 16, mw: 7, ew: 2, top: 3, chin: 19, side: 5, golem: true };
    }
    if (v === 'Slime') {
      for (let y = 6; y <= 21; y++) for (let x = 2; x <= 22; x++) { const t = (y - 6) / 15, hw = 4 + 8.5 * Math.sqrt(Math.min(1, t * 1.3)); if (Math.abs(x - 12) <= hw && (y > 8 || Math.abs(x - 12) <= 3 + (y - 6))) cv.put(x, y, sk, BODY); }
      cv.ell(8, 10, 2, 2, l, BODY); cv.put(7, 9, WHT, DETAIL); cv.put(8, 9, WHT, DETAIL); for (let y = 6; y <= 21; y++) for (let x = 2; x <= 22; x++) if (cv.part(x, y) === BODY && (y > 17 || x > 17) && BAYER[y & 3][x & 3] < 6) cv.put(x, y, d, BODY);
      cv.put(5, 22, sk, BODY); cv.put(19, 22, sk, BODY); cv.put(20, 23, d, BODY);
      return { ey: 12, ex: 7, by: 10, my: 16, mw: 5, ew: 2, top: 6, chin: 21, side: 3, slime: true };
    }
    // Plant: a flower face, petals around a centre
    const petal = s.skin === GRN || s.skin === LGRN ? LMAG : sk;
    for (let i = 0; i < 8; i++) { const a = i * Math.PI / 4; cv.ell(12 + Math.cos(a) * 7, 11 + Math.sin(a) * 7, 3, 3, petal, BODY); }
    cv.ell(12, 11, 6, 6, YEL, BODY); cv.shade(12, 11, 6, 6, YEL); cv.line(12, 18, 12, 23, GRN, BODY); cv.line(12, 21, 8, 19, LGRN, BODY); cv.line(12, 22, 16, 20, LGRN, BODY);
    return { ey: 10, ex: 8, by: 9, my: 14, mw: 5, ew: 2, top: 2, chin: 17, side: 4, plant: true };
  },
};

/** Shoulders and the cloth on them: two rows of the tunic, a collar. */
function shoulders(cv, s, from = 21) {
  if (!s.sapient && s.family !== 'Person' && s.shape !== 'Skull' && s.shape !== 'Stuff(Golem)') { const k = s.skin; for (let y = from; y <= 23; y++) { const hw = 3 + (y - from) * 3 + (from > 21 ? 2 : 0); for (let x = 12 - hw; x <= 12 + hw; x++) cv.put(x, y, y === 23 || x % 4 ? k : dim(k), BODY); } return; }
  const c = s.cloth, d = dim(c);
  for (let y = from; y <= 23; y++) { const hw = 5 + (y - from) * 3 + (from > 21 ? 2 : 0); for (let x = 12 - hw; x <= 12 + hw; x++) cv.put(x, y, y === 23 ? c : x % 5 === 0 ? d : c, WORN); }
  if (from <= 21) { cv.put(11, 21, d, WORN); cv.put(13, 21, d, WORN); }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The small face (16 by 16): its own strokes for each shape, so less is drawn

const SMALL = {
  Person(cv, s) {
    const { skin, hair } = s, f = s.flags;
    cv.ell(7.5, 7, 4, 5, skin);
    if (hair !== undefined) { cv.box(4, 2, 11, 3, hair, HAIR); cv.put(3, 4, hair, HAIR); cv.put(12, 4, hair, HAIR); cv.put(4, 4, hair, HAIR); cv.put(11, 4, hair, HAIR); if (f.longHair) { cv.box(3, 5, 3, 12, hair, HAIR); cv.box(12, 5, 12, 12, hair, HAIR); } cv.box(5, 2, 10, 2, hair, HAIR); }
    cv.box(6, 12, 9, 12, dim(skin)); cv.box(2, 13, 13, 15, s.cloth, WORN);
    return { ey: 6, ex: 5, by: 5, my: 10, mw: 3, ew: 2 };
  },
  Beast(cv, s) {
    const v = s.shape.slice(6, -1), sk = s.skin, cx = 7.5, d = dim(sk);
    cv.ell(cx, 8, v === 'Long' ? 3.5 : 4.5, v === 'Long' ? 5 : 4, sk); cv.ell(cx, 11, 2.5, 2, LIGHT[sk]);
    if (v === 'Pointed' || v === 'Long') cv.both((m) => { cv.box(m ? 11 : 3, 1, m ? 11 : 3, 4, sk); cv.put(m ? 11 : 3, 2, d); });
    else if (v === 'Round') cv.both((m) => cv.ell(m ? 11 : 4, 3, 1.5, 1.5, sk));
    else if (v === 'Wide') cv.both((m) => { cv.box(m ? 11 : 1, 3, m ? 14 : 4, 6, sk); cv.put(m ? 13 : 2, 3, K); });
    else cv.both((m) => { cv.line(m ? 12 : 3, 4, m ? 14 : 1, 1, WHT, DETAIL); });
    cv.put(7, 10, K, DETAIL); cv.put(8, 10, K, DETAIL);
    if (!s.sapient) cv.box(0, 13, 15, 15, sk); else cv.box(2, 13, 13, 15, s.cloth, WORN);
    return { ey: 7, ex: 4, by: 6, my: 12, mw: 2, ew: 2 };
  },
  Bird(cv, s) { cv.ell(7.5, 7, 4, 4.5, s.skin); cv.box(7, 8, 8, 11, s.skin === YEL ? BRN : YEL, DETAIL); cv.put(6, 2, s.skin); cv.put(9, 2, s.skin); cv.put(7, 1, s.skin); cv.box(2, 13, 13, 15, s.skin, BODY); return { ey: 6, ex: 4, by: 5, my: 12, mw: 1, ew: 2 }; },
  Scaled(cv, s) {
    const v = s.shape.slice(7, -1), sk = s.skin;
    if (v === 'Fish') { cv.ell(7, 8, 6, 4, sk); cv.line(13, 5, 15, 3, dim(sk), BODY); cv.line(13, 11, 15, 13, dim(sk), BODY); return { ey: 7, ex: 3, by: 5, my: 10, mw: 2, ew: 2, oneEye: true }; }
    if (v === 'Frog') { cv.ell(7.5, 9, 6, 4, sk); cv.both((m) => cv.ell(m ? 12 : 3, 4, 1.8, 1.8, sk)); return { ey: 5, ex: 2, by: 3, my: 11, mw: 7, ew: 2 }; }
    cv.ell(7.5, 7, 4, 5, sk); cv.box(5, 9, 10, 12, sk); if (v === 'Dragon') cv.both((m) => cv.line(m ? 12 : 3, 4, m ? 14 : 1, 1, WHT, DETAIL)); cv.box(2, 13, 13, 15, sk);
    return { ey: 6, ex: 4, by: 5, my: 11, mw: 4, ew: 2 };
  },
  Bug(cv, s) {
    const v = s.shape.slice(4, -1), sk = s.skin;
    cv.ell(7.5, 8, 4.5, 5, sk);
    if (v === 'Insect') { cv.both((m) => { cv.ell(m ? 11 : 4, 6, 2, 2, s.eyes, EYE); cv.line(m ? 9 : 6, 2, m ? 13 : 2, 0, sk, DETAIL); }); return { ey: 6, ex: 3, by: 4, my: 12, mw: 2, ew: 2 }; }
    if (v === 'Spider') { cv.both((m) => cv.line(m ? 12 : 3, 8, m ? 15 : 0, 11, dim(sk), BODY)); for (const x of [5, 6, 9, 10]) cv.put(x, 5, s.eyes, EYE); return { ey: 7, ex: 4, by: 5, my: 11, mw: 2, ew: 2 }; }
    cv.ell(7.5, 8, 5, 5, sk); for (const y of [4, 8, 12]) cv.box(3, y, 12, y, dim(sk)); return { ey: 6, ex: 4, by: 5, my: 11, mw: 2, ew: 2 };
  },
  Skull(cv, s) { cv.ell(7.5, 6, 5, 5, s.skin); cv.box(4, 9, 11, 12, s.skin); cv.both((m) => cv.box(m ? 9 : 4, 5, m ? 10 : 5, 7, K, EYE)); for (let x = 4; x <= 11; x++) cv.put(x, 11, x % 2 ? WHT : dim(s.skin), MOUTH); return { ey: 6, ex: 4, by: 4, my: 11, mw: 0, ew: 2, socket: true }; },
  Stuff(cv, s) {
    const v = s.shape.slice(6, -1), sk = s.skin;
    if (v === 'Golem') { cv.box(3, 2, 12, 11, sk); cv.box(2, 13, 13, 15, dim(sk), BODY); cv.put(3, 3, LIGHT[sk], DETAIL); cv.put(12, 3, LIGHT[sk], DETAIL); return { ey: 5, ex: 4, by: 4, my: 9, mw: 4, ew: 2, golem: true }; }
    if (v === 'Slime') { for (let y = 4; y <= 14; y++) { const hw = 2 + (y - 4) * 0.55; for (let x = 8 - Math.ceil(hw); x <= 7 + Math.ceil(hw); x++) cv.put(x, y, y > 11 && x % 2 ? dim(sk) : sk); } cv.put(4, 7, WHT, DETAIL); return { ey: 9, ex: 4, by: 8, my: 12, mw: 3, ew: 2 }; }
    if (v === 'Fire') { cv.ell(7.5, 9, 4, 5, sk); for (const x of [4, 7, 10]) cv.line(x, 5, x + 1, 1, YEL, BODY); cv.ell(7.5, 10, 2, 3, YEL); return { ey: 7, ex: 4, by: 6, my: 12, mw: 3, ew: 2 }; }
    if (v === 'Plant') { for (let i = 0; i < 6; i++) cv.ell(7.5 + Math.cos(i * 1.05) * 5, 7 + Math.sin(i * 1.05) * 4.5, 2, 2, sk); cv.ell(7.5, 7, 3.5, 3.5, YEL); cv.line(7, 11, 7, 15, GRN, BODY); return { ey: 6, ex: 5, by: 5, my: 9, mw: 2, ew: 2 }; }
    cv.ell(7.5, 8, v === 'Air' ? 5.5 : 5, 6, sk); if (v === 'Air') for (let y = 2; y < 14; y++) for (let x = 2; x < 14; x++) if (cv.solid(x, y) && (x + y) % 2) cv.put(x, y, GRY, BODY);
    return { ey: 6, ex: 4, by: 5, my: 11, mw: 3, ew: 2 };
  },
};

// ---------------------------------------------------------------------------------------------------------------------------------
// Moods: eyes, brows and a mouth at the shape's anchors

function moodLayer(cv, s, a, small) {
  const mood = FACE_MOODS[s.mood], white = a.socket ? K : WHT, W = cv.w;
  let iris = a.socket ? (s.eyes === BRN || s.eyes === BLU ? LRED : s.eyes) : a.golem || a.hot ? YEL : s.eyes;
  if (iris === s.skin || (iris === dk(s.skin) && !a.socket)) iris = s.skin === K || s.skin === DGR ? YEL : K;
  const browC = s.hair ?? dim(s.skin), mouthC = a.socket ? K : s.skin === RED || s.skin === LRED ? K : RED === s.skin ? K : dim(s.skin) === s.skin ? K : dk(s.skin) === K ? RED : K;
  const glow = s.flags.glow;
  const mid0 = a.cx ?? (W - 1) / 2, eyes = a.oneEye ? [a.ex] : [a.ex, Math.round(2 * mid0 - a.ex - a.ew + 1)];
  const ew = a.ew, ey = a.ey, by = a.by;
  const px = (x, y, c, p = EYE) => cv.put(x, y, c, p);
  eyes.forEach((x, k) => {
    const left = a.oneEye ? true : k === 0, inner = left ? 1 : -1;             // inner is the step toward the face's middle
    const iris0 = left ? x : x + ew - 1, white0 = left ? x + ew - 1 : x;
    const edge = (i) => (left ? x + i : x + ew - 1 - i);                        // i = 0 the outer end
    const at = (i) => edge(i);
    if (mood === 'Asleep') { for (let i = 0; i < ew; i++) px(at(i), ey, dim(s.skin) === s.skin ? K : dim(s.skin)); return; }
    if (mood === 'Dead') { const c = K; px(x, ey - 1, c); px(x + ew - 1, ey - 1, c); px(x + 1, ey, c); if (ew > 2) px(x + 2, ey, c); px(x, ey + 1, c); px(x + ew - 1, ey + 1, c); return; }
    if (mood === 'Pain') { px(at(0), ey, K); px(at(1), ey + 1 > ey ? ey : ey, K); px(at(0), ey - 1, K); if (ew > 2) px(at(2), ey, K); px(at(1), ey, K); return; }
    if (mood === 'Afraid' && !small) { for (let i = 0; i < ew; i++) { px(at(i), ey - 1, white); px(at(i), ey, white); } px(iris0 + (left ? 0 : 0), ey, iris); px(iris0, ey - 1, iris === white ? K : white); if (ew > 2) px(at(1), ey, iris); return; }
    if (mood === 'Afraid') { px(at(0), ey - 1, white); px(at(1), ey - 1, white); px(at(0), ey, iris); px(at(1), ey, white); return; }
    // Calm and Angry: an iris at the outer end, the white by it; angry lowers the lid over the top
    const lit = glow ? LIGHT[iris] : iris;
    px(iris0, ey, lit); for (let i = 1; i < ew; i++) px(at(i), ey, i === 1 ? white : lit);
    if (a.socket) for (let i = 0; i < ew; i++) px(at(i), ey, i === 0 ? lit : K);
    if (mood === 'Angry') px(at(ew - 1), ey - 1, browC, BROW);
  });
  // brows
  const brow = (x, left) => {
    const y = by;
    for (let i = -1; i <= ew; i++) {
      const outer = left ? i : ew - 1 - i;                                       // 0 at the outer end, rising toward the middle
      if (outer < -1 || a.noBrow) continue;
      let dy = 0;
      if (mood === 'Angry') dy = outer < 1 ? -1 : outer < 2 ? 0 : 1;           // down toward the middle
      else if (mood === 'Pain') dy = outer < 1 ? 1 : outer < 2 ? 0 : -1;       // inner end raised
      else if (mood === 'Afraid') dy = -1;
      else if (mood === 'Asleep') dy = 1;
      if (mood === 'Dead') continue;
      if (small && (i < 0 || i >= ew)) continue;
      const bx = x + i; if (cv.solid(bx, y + dy) || a.socket || true) cv.put(bx, y + dy - (mood === 'Afraid' ? 0 : 0), a.socket ? DGR : browC, BROW);
    }
  };
  if (!a.oneEye && !a.spider && !a.insect && !a.fish && !a.plant && !a.slime && !a.air) { brow(eyes[0], true); if (eyes[1] !== undefined) brow(eyes[1], false); }
  // mouth
  const mx = a.mcx ?? mid0, mw = a.mw, my = a.my;
  const mp = (dx, dy, c = mouthC, p = MOUTH) => cv.put(Math.round(mx + dx), my + dy, c, p);
  if (a.noMouth || a.beak || mw === 0) { if (a.beak && mood === 'Afraid') { mp(0, 0, K); mp(0, 1, K); } return; }
  const half = (mw - 1) / 2;
  const sneer = mood === 'Calm' && s.evil && s.sapient;
  if (mood === 'Afraid' || (mood === 'Pain' && !small)) { const o = mood === 'Afraid' ? 1 : 0; for (let i = -Math.floor(half + 1); i <= Math.floor(half + 1); i++) if (Math.abs(i) <= Math.max(1, half - 0.5)) { mp(i, 0, K); mp(i, 1, K); } if (mw >= 5 && mood === 'Afraid') mp(0, 2, K); if (o && mw >= 3) { mp(-1, -1, white); mp(1, -1, white); } return; }
  for (let i = -Math.floor(half); i <= Math.ceil(half); i++) {
    let dy = 0;
    if (mood === 'Angry') dy = Math.abs(i) > half - 1 ? 1 : 0;
    else if (mood === 'Pain') dy = i % 2 ? 1 : 0;
    else if (mood === 'Asleep') dy = 0;
    else if (sneer) dy = i >= half - 0.5 ? -1 : 0;
    mp(i, dy, mouthC);
  }
  if (mood === 'Angry' && mw >= 3) { mp(-1, 0, WHT); mp(1, 0, WHT); mp(0, 1, mouthC); }
  if (mood === 'Dead' && !a.slime && mw >= 3) mp(1, 1, LRED);
  if (mood === 'Asleep' && !small) { const zx = Math.min(W - 6, Math.round(mx + 9)); for (const [dx, dy, c] of [[0, 0, WHT], [1, 0, WHT], [1, 1, WHT], [0, 2, WHT], [1, 2, WHT]]) cv.put(zx + dx, 3 + dy, c, DETAIL); cv.put(zx + 3, 1, GRY, DETAIL); cv.put(zx + 4, 1, GRY, DETAIL); cv.put(zx + 4, 2, GRY, DETAIL); cv.put(zx + 3, 3, GRY, DETAIL); cv.put(zx + 4, 3, GRY, DETAIL); }
  if (mood === 'Asleep' && small) { cv.put(W - 3, 1, WHT, DETAIL); cv.put(W - 2, 1, WHT, DETAIL); cv.put(W - 2, 2, WHT, DETAIL); cv.put(W - 3, 3, WHT, DETAIL); cv.put(W - 2, 3, WHT, DETAIL); }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Cues from the description, and what is worn

function cues(cv, s, a, small) {
  const f = s.flags, W = cv.w, cx = a.cx ?? 12, mid = small ? 7.5 : cx, hairC = s.hair ?? LIGHT[s.skin];
  const L = (dx) => Math.round(mid - dx), R = (dx) => Math.round(mid + dx);
  const person = s.family === 'Person' || s.sapient;
  if (f.beard && !a.beak && !a.socket) {
    const top = a.my - 1, depth = small ? 3 : 5;
    for (let i = 0; i < depth; i++) { const hw = Math.max(1, (small ? 3 : 5) - Math.floor(i * 0.9)); for (let x = L(hw); x <= R(hw); x++) if (!(i === 0 && Math.abs(x - mid) < 2.5 && x === Math.round(mid))) cv.put(x, top + 1 + i, hairC, HAIR); }
    if (!small) { cv.box(L(a.mw / 2 + 1), a.my - 1, R(a.mw / 2 + 1), a.my - 1, hairC, HAIR); }
  }
  if (f.mustache && !a.beak && !small) { cv.box(L(3), a.my - 2, L(1), a.my - 2, hairC, HAIR); cv.box(R(1), a.my - 2, R(3), a.my - 2, hairC, HAIR); }
  if (f.horns && !a.fish) cv.both((m) => { const x = m ? W - 1 - (small ? 4 : 6) : small ? 4 : 6, k = m ? 1 : -1; const top = small ? 1 : (a.top ?? 3); cv.line(x, top + 2, x + k * 1, top, WHT, DETAIL); cv.line(x + k, top, x + k * 2, top - 1 < 0 ? 0 : top - 1, GRY, DETAIL); });
  if (f.tusks && !a.beak) cv.both((m) => { const x = m ? R((a.mw ?? 3) / 2 + 1) : L((a.mw ?? 3) / 2 + 1); cv.line(x, a.my, x, a.my - (small ? 1 : 2), WHT, DETAIL); });
  if (f.fangs && !a.beak && a.mw >= 2) cv.both((m) => cv.put(m ? R(1) : L(1), a.my + 1, WHT, DETAIL));
  if (small) return;
  if (f.scar && !a.socket) cv.line(L(4), a.ey - 2, L(2), a.ey + 3, s.skin === LRED || s.skin === RED ? LGRN : LRED, DETAIL);
  if (f.patch && !a.socket) { const x = L(a.mid ?? 3) - 1; cv.box(x, a.ey - 1, x + 3, a.ey + 1, K, DETAIL); cv.line(x - 1, a.ey - 3, R(5), a.ey - 4, K, DETAIL); }
  if (f.freckles && person) for (const [dx, dy] of [[4, 2], [5, 3], [3, 3], [4, 4]]) { { const fc = s.skin === BRN ? DGR : BRN; cv.put(L(dx), a.ey + dy - 1, fc, DETAIL); cv.put(R(dx), a.ey + dy - 1, fc, DETAIL); } }
  if (f.tattoo && person) { cv.line(L(1), a.ey - 4, R(1), a.ey - 4, BLU, DETAIL); cv.put(Math.round(mid), a.ey - 3, BLU, DETAIL); cv.put(Math.round(mid), a.ey - 5, BLU, DETAIL); }
  if (f.old && person) { for (let x = L(3); x <= R(3); x++) if ((x + 1) % 2) cv.put(x, a.by - 2, dim(s.skin), DETAIL); cv.line(L(2), a.my - 2, L(3), a.my + 1, dim(s.skin), DETAIL); cv.line(R(2), a.my - 2, R(3), a.my + 1, dim(s.skin), DETAIL); }
}

function wornItems(cv, s, a, small) {
  const mid = (cv.w - 1) / 2, w = s.worn, W = cv.w, top = small ? 1 : (a.top ?? 3), hw = small ? 4 : 6, ey = a.ey;
  if (w.head) {
    const { word, color } = w.head, c = color ?? (/crown|tiara|circlet/.test(word) ? YEL : /hood|cowl/.test(word) ? BRN : GRY), d = dim(c);
    if (/crown|tiara|circlet/.test(word)) { for (let x = Math.round(mid - hw + 1); x <= mid + hw - 1; x++) cv.put(x, top + 2, c, WORN); for (let x = Math.round(mid - hw + 1); x <= mid + hw - 1; x += 2) cv.put(x, top + 1, c, WORN); cv.put(Math.round(mid), top + 1, LRED, WORN); }
    else if (/hood|cowl/.test(word)) { for (let y = top; y <= ey + 3; y++) for (let x = Math.round(mid - hw - 1); x <= mid + hw + 1; x++) { const edge = Math.abs(x - mid) > hw - 1.5 || y < top + 3; if (cv.solid(x, y) && (edge || y <= top + 1)) cv.put(x, y, edge && y > top + 2 ? d : c, WORN); } cv.put(Math.round(mid), top, c, WORN); }
    else if (/hat|cap|turban|bandana|headband/.test(word) && !/helm/.test(word)) { for (let x = Math.round(mid - hw - 1); x <= mid + hw + 1; x++) cv.put(x, top + 3, d, WORN); for (let y = top - 1; y <= top + 2; y++) for (let x = Math.round(mid - (y - top + 3)); x <= mid + (y - top + 3); x++) cv.put(x, y, c, WORN); }
    else { for (let y = top; y <= top + 3; y++) for (let x = Math.round(mid - hw); x <= mid + hw; x++) if (cv.solid(x, y)) cv.put(x, y, y === top + 3 ? d : c, WORN); cv.line(Math.round(mid), top + 3, Math.round(mid), ey + 2, c, WORN); }
  }
  if (w.neck && !small) { const c = w.neck.color ?? (/scarf/.test(w.neck.word) ? RED : YEL); if (/scarf|collar/.test(w.neck.word)) cv.box(Math.round(mid - 3), a.chin, Math.round(mid + 3), a.chin + 1, c, WORN); else { cv.put(Math.round(mid), a.chin + 1, c, WORN); cv.put(Math.round(mid), a.chin + 2, LIGHT[c], WORN); } }
  if (w.neck && small) cv.put(Math.round(mid), 12, w.neck.color ?? YEL, WORN);
  if (w.eyes && !small) cv.both((m) => { const x = m ? W - 1 - a.ex : a.ex; cv.box(x - 1, ey - 1, x + a.ew, ey + 1, K, WORN); cv.put(x, ey, LCYN, WORN); cv.put(x + 1, ey, LCYN, WORN); });
  if (w.ears && !small) cv.both((m) => cv.put(m ? W - 1 - (a.side ?? 5) : a.side ?? 5, 13, YEL, WORN));
  if (w.back && !small) { for (let y = 20; y < cv.h; y++) for (let x = 0; x < W; x++) if (!cv.solid(x, y) && (x < 4 || x > W - 5)) cv.put(x, y, w.back.color ?? MAG, WORN); }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Hurt: five steps, shown by the material (flesh and scales bleed, fur mats, stone and bone crack, ghosts and elementals fade, slime shrinks)

export function hurtClass(s) {
  const sh = s.shape;
  if (sh === 'Stuff(Slime)') return 'Shrink';
  if (sh === 'Skull' || sh === 'Stuff(Golem)' || sh === 'Stuff(Earth)' || sh === 'Stuff(Plant)') return 'Crack';
  if (sh === 'Stuff(Fire)' || sh === 'Stuff(Water)' || sh === 'Stuff(Air)' || s.ghostly) return 'Fade';
  if (s.family === 'Beast' || s.family === 'Bird') return 'Mat';
  if (sh === 'Bug(Insect)' || sh === 'Bug(Spider)' || sh === 'Bug(Worm)') return 'Bleed(GREEN)';
  return 'Bleed(RED)';
}

function hurt(cv, s, a) {
  const step = s.hurt; if (!step) return;
  const cls = hurtClass(s), r = rng(`wounds:${key(s.race)}:${s.shape}`), body = [];
  for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) { const p = cv.part(x, y); if (p === BODY && y < (cv.h > 16 ? 20 : 12)) body.push([x, y]); }
  const spots = Array.from({ length: 8 }, () => body.length ? body[r.int(body.length)] : [0, 0]);
  const n = [0, 2, 4, 5, 8, 8][step], blood = cls === 'Bleed(GREEN)' ? LGRN : LRED, drop = cls === 'Bleed(GREEN)' ? GRN : RED, big = cv.h > 16;
  if (cls === 'Bleed(RED)' || cls === 'Bleed(GREEN)') {
    for (let i = 0; i < n; i++) { const [x, y] = spots[i]; cv.put(x, y, blood, DETAIL); if (step >= 2 && i % 2 === 0) cv.put(x + 1, y, drop, DETAIL); if (step >= 4 && i % 3 === 0) { cv.put(x, y + 1, blood, DETAIL); if (big) cv.put(x, y + 2, drop, DETAIL); } }
    if (step >= 3) for (const [x, y] of spots.slice(0, 3)) { cv.put(x - 1, y, MAG, DETAIL); cv.put(x, y - 1, MAG, DETAIL); }
    if (step === 5) paleBody(cv, s);
  } else if (cls === 'Mat') {
    const matC = s.skin === K ? DGR : DARK_[s.skin] && DARK_[s.skin] !== s.skin ? DARK_[s.skin] : K;
    for (let i = 0; i < n; i++) { const [x, y] = spots[i]; cv.put(x, y, matC, BODY); if (step >= 2) cv.put(x + 1, y + 1, matC, BODY); if (step >= 3 && i % 2 === 0) { cv.put(x, y + 1, matC, BODY); cv.put(x - 1, y, matC, BODY); } if (step >= 4 && i % 3 === 0) cv.put(x, y, LRED, DETAIL); }
    if (step === 5) { paleBody(cv, s); for (const [x, y] of spots.slice(0, 4)) cv.put(x, y, LRED, DETAIL); }
  } else if (cls === 'Crack') {
    const cracks = [0, 1, 1, 2, 3, 4][step], len = [0, 3, 5, 6, 8, 11][step];
    for (let k = 0; k < cracks; k++) { let [x, y] = spots[k]; for (let i = 0; i < len; i++) { cv.put(x, y, K, DETAIL); const d = r.int(3); x += d === 0 ? -1 : d === 1 ? 1 : 0; y += 1; if (!cv.solid(x, y)) break; } }
    if (step >= 4) for (const [x, y] of spots.slice(4, 4 + (step - 3) * 2)) { cv.put(x, y, s.backdrop, BACK); }
  } else if (cls === 'Fade') {
    const keep = [0, 0.06, 0.12, 0.2, 0.32, 0.5][step] * 16;
    for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.solid(x, y) && cv.part(x, y) !== EYE && BAYER[y & 3][x & 3] < keep) { cv.put(x, y, s.backdrop, BACK); }
    if (step >= 3) for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === BODY && (x + y) % 3 === 0) cv.put(x, y, dk(cv.at(x, y)), BODY);
  } else if (cls === 'Shrink') {
    const k = [0, 1, 2, 4, 6, 9][step] * (big ? 1 : 0.6) | 0, h = cv.h, a0 = new Uint8Array(cv.a), p0 = new Uint8Array(cv.parts);
    for (let y = 0; y < h; y++) for (let x = 0; x < cv.w; x++) { const sy = y < k ? -1 : Math.round((y - k) * h / (h - k)); const i = y * cv.w + x; if (sy < 0 || sy >= h) { cv.a[i] = s.backdrop; cv.parts[i] = BACK; } else { cv.a[i] = a0[sy * cv.w + x]; cv.parts[i] = p0[sy * cv.w + x]; } }
    if (step >= 3) for (const [x, y] of spots.slice(0, step - 1)) if (cv.part(x, y) === BODY) cv.put(x, y, dk(s.skin), BODY);
  }
  // Near death: the eyes half shut, on any material that has them
  if (step === 5 && a.ey !== undefined && cls !== 'Fade') for (let x = 0; x < cv.w; x++) if (cv.part(x, a.ey - 1) === EYE) cv.put(x, a.ey - 1, dk(s.skin), BODY);
}
function paleBody(cv, s) { for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === BODY && cv.at(x, y) === s.skin) cv.put(x, y, s.skin === GRY ? WHT : GRY, BODY); }

// ---------------------------------------------------------------------------------------------------------------------------------
// Effects as looks: a tint, a skin, the eyes, or something drawn around the figure; burning and glowing are animated

function tint(cv, s, from, to) { for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === BODY && cv.at(x, y) === from) cv.put(x, y, to, BODY); else if (cv.part(x, y) === BODY && cv.at(x, y) === dk(from)) cv.put(x, y, dk(to), BODY); else if (cv.part(x, y) === BODY && cv.at(x, y) === LIGHT[from]) cv.put(x, y, LIGHT[to], BODY); }
const ring = (cv, fn) => { const m = new Uint8Array(cv.w * cv.h); for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (!cv.solid(x, y)) { let hit = false; for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (cv.solid(x + dx, y + dy)) hit = true; if (hit) m[y * cv.w + x] = 1; } for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (m[y * cv.w + x]) fn(x, y); };

function applyLooks(cv, s, a, frame, small) {
  const big = !small, mid = (cv.w - 1) / 2, top = small ? 0 : (a.top ?? 3);
  for (const look of s.looks) {
    if (look === 'halo') { const y = Math.max(0, top - 2), hw = small ? 3 : 5; for (let x = Math.round(mid - hw); x <= mid + hw; x++) { cv.put(x, y, YEL, DETAIL); if (big && Math.abs(x - mid) >= hw - 1) cv.put(x, y + 1, YEL, DETAIL); } if (big) for (let x = Math.round(mid - hw + 1); x <= mid + hw - 1; x++) cv.put(x, y - 1 < 0 ? y : y - 1, LIGHT[YEL], DETAIL); }
    if (look === 'poisoned') { tint(cv, s, s.skin, s.skin === GRN || s.skin === LGRN ? DGR : LGRN); cv.put(a.ex - 2 < 0 ? 1 : a.ex - 2, a.my, LGRN, DETAIL); cv.put(Math.round(mid) + (small ? 3 : 5), a.my - 1, LGRN, DETAIL); }
    if (look === 'sick') { tint(cv, s, s.skin, s.skin === GRN ? BRN : GRN); cv.line(Math.round(mid) - 2, a.by - 1, Math.round(mid) - 1, a.by - 1, DGR, DETAIL); }
    if (look === 'burning') { const flick = frame ?? 0; const base = [[LRED, YEL], [YEL, LRED], [LRED, LIGHT[YEL]]][flick % 3]; ring(cv, (x, y) => { if ((x * 5 + y * 3 + flick) % 4 < 3) cv.put(x, y, base[(x + y) % 2], DETAIL); if (y > 0 && (x + flick) % 3 === 0 && !cv.solid(x, y - 1)) cv.put(x, y - 1, base[(x + 1) % 2], DETAIL); }); }
    if (look === 'frozen') { tint(cv, s, s.skin, s.skin === LCYN ? WHT : LCYN); ring(cv, (x, y) => { if ((x + y) % 2 === 0) cv.put(x, y, WHT, DETAIL); }); }
    if (look === 'blind') for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === EYE) cv.put(x, y, cv.at(x, y) === WHT ? GRY : WHT, EYE);
    if (look === 'invisible') for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.solid(x, y) && (x + y) % 2 === 0) cv.put(x, y, s.backdrop, BACK);
    if (look === 'outlined') ring(cv, (x, y) => cv.put(x, y, (x + y + (frame ?? 0)) % 2 ? LMAG : MAG, DETAIL));
    if (look === 'webbed') { const w = cv.w, h = cv.h; for (let i = -h; i < w; i += 5) cv.line(i, 0, i + h, h - 1, GRY, DETAIL); for (let i = 0; i < w + h; i += 5) cv.line(i, 0, i - h, h - 1, GRY, DETAIL); }
    if (look === 'stone') { for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.solid(x, y) && cv.part(x, y) !== DETAIL) { const p = cv.part(x, y); cv.put(x, y, p === EYE ? DGR : (x * 3 + y) % 5 === 0 ? DGR : (x + y) % 7 === 0 ? WHT : GRY, p); } }
    if (look === 'bark') { for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) { const p = cv.part(x, y); if ((p === BODY || p === HAIR) && cv.at(x, y) !== K) cv.put(x, y, x % 3 === 0 ? K : x % 3 === 1 ? BRN : DGR === 8 && (x + y) % 5 === 0 ? 8 : BRN, p); } }
    if (look === 'glowing') { const on = (frame ?? 0) % 2 === 0; for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === EYE) cv.put(x, y, on ? WHT : YEL, EYE); ring(cv, (x, y) => { if ((x + y) % 2 === (on ? 0 : 1)) cv.put(x, y, YEL, DETAIL); }); }
  }
}
const ANIMATED = new Set(['burning', 'glowing']);

// ---------------------------------------------------------------------------------------------------------------------------------
// The picture

/**
 * With 256 colours or true colour, the broader palette on a finished face: light from the upper left and a darker lower right edge over the
 * skin, strands down the hair, a grain over fur and feathers, and the backdrop falling off toward the bottom. It runs last, since the
 * hurt steps and the looks find colours by their index.
 */
function smoothFace(cv, s) {
  const seed = hashSeed(`${s.race}:${s.shape}`) & 0xffff, furry = s.family === 'Beast' || s.family === 'Bird', solid = (x, y) => cv.solid(x, y);
  volume(cv, { solid, pick: (x, y) => cv.part(x, y) === BODY, seed, light: 3, rim: 4, grain: furry ? 2 : 1.2, fx: furry ? 1.4 : 1, fy: furry ? 0.6 : 1 });
  volume(cv, { solid, pick: (x, y) => cv.part(x, y) === HAIR, seed: seed + 1, light: 2, rim: 3, grain: 2.2, fx: 1.7, fy: 0.45 });
  for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (!cv.solid(x, y)) {
    const i = y * cv.w + x, n = vnoise(seed + 2, x / 3, y / 2) - 0.5;
    cv.a[i] = cv.tint(cv.a[i], -Math.round(y * 5 / cv.h) + Math.round(n * 3));
  }
}

function paintFace(s, size, frame, depth = 16) {
  BACKDROP = s.backdrop; const small = size === 'small', cv = new Canvas(small ? 16 : 25, small ? 16 : 24, s.backdrop, depth);
  const fam = s.family, table = small ? SMALL : FULL, anchors = table[fam](cv, s);
  if (!small || true) moodLayer(cv, s, anchors, small);
  cues(cv, s, anchors, small);
  wornItems(cv, s, anchors, small);
  if (s.flags.glow && !s.looks.includes('glowing')) for (let y = 0; y < cv.h; y++) for (let x = 0; x < cv.w; x++) if (cv.part(x, y) === EYE && cv.at(x, y) !== WHT && cv.at(x, y) !== K) cv.put(x, y, LIGHT[cv.at(x, y)] === cv.at(x, y) ? YEL : LIGHT[cv.at(x, y)], EYE);
  hurt(cv, s, anchors);
  applyLooks(cv, s, anchors, frame, small);
  if (cv.smooth) smoothFace(cv, s);
  return cv;
}

/**
 * A creature's face (ANSI only). Params: `race` (as the game names it; or `name`, a description's noun phrase the race is found in, or
 * `category`), `description` (read for colours and cues), `size` ('full' 25 by 12 cells, 'small' 16 by 8), `mood` (calm, angry, pain, afraid,
 * asleep, dead), `hurt` (0 to 5, a health fraction `{ health }` or health text), `effects` (names like "bless" or looks like "burning"),
 * `place` (`{ terrain, time, weather, style, key, wild }`: the face laid on that place, see `figureOnPlace`), `wears`, `skilled`, `sex`, `alignment`, explicit `skin`, `hair`, `eyes`, `backdrop` (VGA indexes), `seed`, `bgLimit`, `colors` (16, 256 or 'true': at 256 and true colour the volume, hair and fur are blended tones and grain, 16 colours keep the dither). Returns the Art
 * with the picture, its words (`says`) and `figure`, the part-tagged pixels.
 */
export function drawCreatureFace(params = {}) {
  const size = params.size ?? 'full', colors = checkColors(params.colors ?? 16);
  if (!FACE_SIZES.includes(size)) throw new Error(`unknown face size "${size}"; known: ${FACE_SIZES.join(', ')}`);
  const s = readFace(params), bgLimit = params.bgLimit ?? 8, animated = s.looks.some((l) => ANIMATED.has(l));
  const figureOf = (cv) => ({ w: cv.w, h: cv.h, colors: cv.smooth ? Uint32Array.from(cv.a, (c) => (c === CLEAR ? CLEAR - 1 : c)) : Uint8Array.from(cv.a), ...(colors !== 16 ? { depth: colors } : {}), parts: Uint8Array.from(cv.parts), partNames: FACE_PARTS });
  const make = (frame) => {
    const cv = paintFace(s, size, frame, colors);
    // On a place (`place`): the face laid on a darker picture of it, lit by it, an animal on its wild terrain (figure.js).
    const cells = params.place ? figureOnPlace(figureOf(cv), { key: s.race, wild: s.family !== 'Person' && !s.sapient, ...params.place }, { bgLimit, colors }).cells : fitted(cv.toCells(colors === 16 ? Array.from({ length: 16 }, (_, i) => i) : IDENTITY256), colors, bgLimit);
    return { cv, cells };
  };
  const first = make(0), n = animated ? 3 : 1, frames = Array.from({ length: n }, (_, i) => (i ? make(i).cells : first.cells));
  const cv = first.cv;
  const art = {
    id: `creatureface.${s.shape.toLowerCase().replace(/[()]/g, '-').replace(/-$/, '')}`, frames: [frames[0]], fps: 0, loop: false, says: faceWords(s),
    figure: figureOf(cv), ...(colors !== 16 ? { colors } : {}),
  };
  if (animated) art.clips = { loop: { id: 'loop', frames, fps: 3, loop: true, events: [], reduced: [frames[0]] } };
  return art;
}

/** The facts of a face in words, for the line beside it: "Orc, angry, bleeding." */
function faceWords(s) {
  const bits = [String(s.race).replace(/^./, (c) => c.toUpperCase()), FACE_MOODS[s.mood] === 'Pain' ? 'in pain' : FACE_MOODS[s.mood].toLowerCase()];
  if (s.hurt) bits.push(HURT_STEPS[s.hurt]);
  const f = s.flags; for (const [k, t] of [['beard', 'bearded'], ['scar', 'scarred'], ['patch', 'eye patch'], ['horns', 'horned'], ['tusks', 'tusked'], ['fangs', 'fanged'], ['halo', 'haloed'], ['old', 'old']]) if (f[k]) bits.push(t);
  bits.push(...s.looks.map((l) => (l === 'invisible' ? 'half seen' : l)));
  return `${bits.join(', ')}.`;
}
