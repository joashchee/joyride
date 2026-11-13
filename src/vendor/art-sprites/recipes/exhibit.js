// The Museum and the Hall of Fame (T21). `exhibit` frames a picture the game gives it (the pixels of any recipe) in a display case, on a plinth, as a framed picture, a manuscript
// page or a specimen board, with a generic category placeholder when there is no picture, an empty slot, an occupied slot and a "new" mark. `award` draws what a hall of fame
// is made of: a plaque, a podium, a ribbon, a seal, a medal and rank chevrons. No text is ever drawn: the game owns the words, so a plaque is blank and a podium counts in pips.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { drawPlaceholder, PLACEHOLDER_CATEGORIES } from './placeholder.js';

export const EXHIBIT_STYLES = ['case', 'plinth', 'frame', 'page', 'board'];
export const EXHIBIT_STATES = ['empty', 'occupied', 'new'];
const W = 48, H = 48;

/** The pixels of whatever the game gave: a Pixels, an art (its first frame), or a placeholder when it gave nothing. */
const pixelsOf = (p) => (p instanceof Pixels ? p : p?.frames?.[0] ?? null);
/** A picture scaled by a whole number to fit the window: down by the smallest factor that fits, or up by the largest that does. */
function fit(px, w, h) {
  let s = 1; while (Math.ceil(px.w / s) > w || Math.ceil(px.h / s) > h) s++;
  if (s > 1) { const out = new Pixels(Math.ceil(px.w / s), Math.ceil(px.h / s), px.palette); for (let y = 0; y < out.h; y++) for (let x = 0; x < out.w; x++) out.set(x, y, px.get(x * s, y * s)); return out; }
  const up = Math.max(1, Math.min(Math.floor(w / px.w), Math.floor(h / px.h))); if (up === 1) return px;
  const out = new Pixels(px.w * up, px.h * up, px.palette); for (let y = 0; y < out.h; y++) for (let x = 0; x < out.w; x++) out.set(x, y, px.get(Math.floor(x / up), Math.floor(y / up))); return out;
}

const FRAMES = {
  case: { window: [10, 8, 28, 28], mask(m) { m.rect(3, 38, 42, 9, 1); m.rect(3, 38, 42, 2, 3); m.rect(3, 45, 42, 2, 2); m.rect(6, 5, 36, 33, 3); m.rect(8, 7, 32, 31, 5); m.rect(6, 5, 36, 2, 2); } },
  plinth: { window: [8, 0, 32, 26], mask(m) { m.rect(14, 30, 20, 14, 1); m.rect(14, 30, 3, 14, 2); m.rect(8, 26, 32, 5, 3); m.rect(8, 26, 32, 1, 2); m.rect(8, 43, 32, 4, 3); } },
  frame: { window: [8, 8, 32, 32], mask(m) { m.rect(2, 2, 44, 44, 1); m.rect(2, 2, 44, 2, 2); m.rect(6, 6, 36, 36, 2); m.rect(8, 8, 32, 32, 4); for (const [x, y] of [[2, 2], [42, 2], [2, 42], [42, 42]]) m.rect(x, y, 4, 4, 3); } },
  page: { window: [12, 8, 24, 32], mask(m) { m.rect(7, 2, 34, 44, 1); m.poly([[7, 46], [12, 44], [18, 46], [25, 44], [32, 46], [41, 44], [41, 46]], 0); m.rect(7, 2, 34, 1, 2); m.rect(9, 4, 30, 40, 4); m.rect(36, 40, 3, 8, 3); } },
  board: { window: [8, 8, 32, 32], mask(m) { m.rect(2, 3, 44, 42, 1); m.rect(2, 3, 44, 2, 2); m.rect(5, 6, 38, 36, 4); for (const [x, y] of [[4, 5], [42, 5], [4, 41], [42, 41]]) m.rect(x, y, 2, 2, 3); } },
};
const SHADE = { case: { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('gold'), 5: ramp('glass') }, plinth: { 1: ramp('stone'), 2: ramp('iron'), 3: ramp('white') }, frame: { 1: ramp('wood'), 2: ramp('gold'), 3: ramp('gold'), 4: ramp('iron') }, page: { 1: ramp('bone'), 2: ramp('sand'), 3: ramp('red'), 4: ramp('white') }, board: { 1: ramp('wood'), 2: ramp('leather'), 3: ramp('steel'), 4: ramp('sand') } };

/** The pennant and star of a "new" exhibit: a gold star on a pole at the top right, a shape and not only a colour. */
function newMark(p) { const x = W - 11; p.rect(x, 0, 1, 12, C.night).rect(x + 1, 1, 8, 8, C.night).rect(x + 1, 2, 7, 6, ramp('green')[2]); p.rect(x + 4, 2, 2, 6, C.white).rect(x + 2, 4, 6, 2, C.white); return p; }

/**
 * Frame a picture. `style` is `case`, `plinth`, `frame`, `page` or `board`; `picture` is any art or Pixels (the game draws it with whatever recipe it likes; `{ recipe, params }` is
 * accepted too, resolved by the registry); `state` is `empty` (nothing inside: dust and an outline where it goes), `occupied` or `new` (occupied with a pennant). With no picture, an occupied slot
 * shows a placeholder: `category` (weapon, armour, shield, consumable, material, tool, book, misc) or, with none, the generic exhibit, so an undiscovered one gives nothing away.
 */
export function drawExhibit({ style = 'case', picture, state, category } = {}) {
  const def = FRAMES[style]; if (!def) throw new Error(`unknown exhibit style "${style}"; known: ${EXHIBIT_STYLES.join(', ')}`);
  const pic = pixelsOf(picture); state ??= pic ? 'occupied' : 'empty';
  if (!EXHIBIT_STATES.includes(state)) throw new Error(`unknown exhibit state "${state}"; known: ${EXHIBIT_STATES.join(', ')}`);
  if (category !== undefined && !PLACEHOLDER_CATEGORIES[category]) throw new Error(`unknown category "${category}"; known: ${Object.keys(PLACEHOLDER_CATEGORIES).join(', ')}`);
  const m = new Pixels(W, H); def.mask(m); const out = shade(m, SHADE[style]), [wx, wy, ww, wh] = def.window;
  if (state === 'empty') {
    // An empty slot: a dotted outline where the piece goes and a few specks of dust, so it reads as waiting and not as broken.
    for (let i = 0; i < ww; i += 4) { out.set(wx + i, wy + 2, C.ash).set(wx + i, wy + wh - 3, C.ash); } for (let j = 0; j < wh; j += 4) { out.set(wx + 2, wy + j, C.ash).set(wx + ww - 3, wy + j, C.ash); }
  } else {
    const shown = pic ?? (category ? drawPlaceholder({ kind: 'item', category }) : drawPlaceholder({ kind: 'exhibit' })).frames[0], f = fit(shown, ww, wh);
    out.blit(f, wx + ((ww - f.w) >> 1), style === 'plinth' ? wy + wh - f.h : wy + ((wh - f.h) >> 1));
  }
  if (style === 'case') { for (let i = 0; i < 5; i++) { out.set(11 + i * 2, 9 + i * 2, C.white); } out.set(12, 9, C.white); }
  if (state === 'new') newMark(out);
  return { id: `exhibit.${style}`, frames: [out], fps: 0, loop: false, spec: { style, state, category }, anchors: { root: [24, H - 1] }, window: [wx, wy, ww, wh], projection: 'front', alt: `A ${style}, ${state === 'empty' ? 'empty' : pic ? 'with a picture' : 'with a placeholder'}${state === 'new' ? ', new' : ''}` };
}

export const AWARD_KINDS = ['plaque', 'podium', 'ribbon', 'seal', 'medal', 'rank'];
export const AWARD_TIERS = { bronze: [C.umber, C.bronze, C.orange, C.tan], silver: ramp('white'), gold: ramp('gold'), platinum: ramp('glass') };

/**
 * A hall of fame piece. `plaque` (a blank plate, 40 by 24, with a slot for an `emblem`), `podium` (`place` 1, 2 or 3 stands highest; steps counted in pips), `ribbon` (a rosette and two tails),
 * `seal` (wax, a star), `medal` (a disc on a ribbon, rim notches counting the tier) and `rank` (`level` 1 to 5 chevrons). `tier` is bronze, silver, gold or platinum. `emblem` is any art or Pixels, centred.
 */
export function drawAward({ kind = 'medal', tier = 'gold', place = 1, level = 1, emblem } = {}) {
  if (!AWARD_KINDS.includes(kind)) throw new Error(`unknown award "${kind}"; known: ${AWARD_KINDS.join(', ')}`);
  const t = AWARD_TIERS[tier]; if (!t) throw new Error(`unknown tier "${tier}"; known: ${Object.keys(AWARD_TIERS).join(', ')}`);
  const emb = pixelsOf(emblem), tn = Object.keys(AWARD_TIERS).indexOf(tier);
  let out, anchors = { root: [16, 30] };
  if (kind === 'plaque') {
    const m = new Pixels(40, 24); m.rect(0, 0, 40, 24, 1); m.rect(2, 2, 36, 20, 2); m.rect(2, 2, 36, 1, 3); for (const [x, y] of [[1, 1], [37, 1], [1, 21], [37, 21]]) m.rect(x, y, 2, 2, 3);
    out = shade(m, { 1: t, 2: ramp('wood'), 3: ramp('white') }); if (emb) out.blit(fit(emb, 16, 16), 4, 4); else out.rect(4, 4, 16, 16, ramp('wood')[0]); out.rect(23, 7, 12, 1, ramp('wood')[0]).rect(23, 11, 12, 1, ramp('wood')[0]).rect(23, 15, 12, 1, ramp('wood')[0]);
    anchors = { root: [20, 23] };
  } else if (kind === 'podium') {
    if (![1, 2, 3].includes(place)) throw new Error('a podium place is 1, 2 or 3');
    const m = new Pixels(48, 32), hs = { 2: 14, 1: 20, 3: 10 }; let x = 0;
    for (const p of [2, 1, 3]) { const h = hs[p]; m.rect(x + 1, 31 - h, 14, h, p === place ? 2 : 1); m.rect(x + 1, 31 - h, 14, 2, 3); for (let i = 0; i < p; i++) m.rect(x + 8 - p * 2 + i * 4, 31 - h + 5, 2, 2, 3); x += 16; }
    out = shade(m, { 1: ramp('stone'), 2: t, 3: ramp('white') }); anchors = { root: [24, 30] };
  } else if (kind === 'ribbon') {
    const m = new Pixels(32, 32); m.poly([[10, 16], [15, 16], [12, 30], [10, 26], [6, 28]], 1); m.poly([[17, 16], [22, 16], [26, 28], [22, 26], [20, 30]], 1); m.ellipse(16, 12, 9, 9, 1); m.ellipse(16, 12, 6, 6, 2); m.ellipse(16, 12, 2.5, 2.5, 3);
    out = shade(m, { 1: t, 2: t.map((c, i, r) => r[Math.max(0, i - 1)]), 3: ramp('white') });
  } else if (kind === 'seal') {
    const m = new Pixels(32, 32); m.ellipse(16, 16, 12, 12, 1); for (let a = 0; a < 8; a++) m.ellipse(16 + Math.cos(a * Math.PI / 4) * 11, 16 + Math.sin(a * Math.PI / 4) * 11, 2.5, 2.5, 1); m.ellipse(16, 16, 8, 8, 2);
    out = shade(m, { 1: ramp('red'), 2: ramp('red').map((c, i, r) => r[Math.max(0, i - 1)]) }); if (emb) out.blit(fit(emb, 12, 12), 10, 10); else { for (const [x, y] of [[16, 10], [17, 14], [22, 14], [18, 17], [20, 22], [16, 19], [12, 22], [14, 17], [10, 14], [15, 14]]) out.set(x, y, ramp('red')[3]); }
  } else if (kind === 'medal') {
    const m = new Pixels(32, 32); m.poly([[9, 1], [16, 1], [18, 12], [12, 12]], 3); m.poly([[16, 1], [23, 1], [20, 12], [14, 12]], 3); m.ellipse(16, 21, 9, 9, 1); m.ellipse(16, 21, 6.5, 6.5, 2);
    out = shade(m, { 1: t, 2: t.map((c, i, r) => r[Math.max(0, i - 1)]), 3: ramp('red') }); for (let i = 0; i <= tn; i++) out.set(16 - tn + i * 2, 29, C.night); // notches on the rim: one more for each tier
    if (emb) out.blit(fit(emb, 10, 10), 11, 16); else out.rect(15, 19, 2, 5, t[3]).rect(13, 21, 6, 1, t[3]);
  } else {
    if (!Number.isInteger(level) || level < 1 || level > 5) throw new Error('a rank level is 1 to 5');
    const m = new Pixels(32, 32); for (let i = 0; i < level; i++) { const y = 24 - i * 5; m.poly([[4, y], [16, y - 7], [28, y], [28, y + 3], [16, y - 4], [4, y + 3]], 1); }
    out = shade(m, { 1: t });
  }
  return { id: `award.${kind}`, frames: [out], fps: 0, loop: false, spec: { kind, tier, place, level }, anchors, projection: 'front', alt: `${kind === 'rank' ? `Rank ${level}` : kind === 'podium' ? `Podium, place ${place} highlighted` : `A ${tier} ${kind}`}` };
}
