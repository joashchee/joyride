// Placeholders: the fallback pictures a game draws when the real art is missing, or must not be shown yet (Tag Team 5.5, invariant 8).
// They are one flat stone-coloured shape with an outline, drawn from the *kind* of thing only. A placeholder for an undiscovered
// creature, item or exhibit is drawn from a size class alone, never from the real thing, so it cannot give the thing away.
// `family`, `category` and `size` name a class the game already shows (a family silhouette is a deliberate fallback, not a secret).
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp, C } from '../palette.js';
import { drawCreature } from './creatures.js';
import { drawItem } from './items.js';

export const PLACEHOLDER_KINDS = ['character', 'family', 'item', 'building', 'exhibit', 'unknown'];
/** Enemy families, each drawn as the silhouette of one representative body plan. */
export const PLACEHOLDER_FAMILIES = { humanoid: 'goblin', beast: 'wolf', insect: 'spider', blob: 'slime', flier: 'bat', serpent: 'snake', undead: 'skeleton', dragon: 'dragon-red', construct: 'golem', spirit: 'ghost', plant: 'treant', aquatic: 'fish' };
/** Item categories, each drawn as one plain shape. */
export const PLACEHOLDER_CATEGORIES = { weapon: 'sword', armour: 'armor', shield: 'shield', consumable: 'potion', material: 'gem', tool: 'key', book: 'book', misc: 'sack' };
export const PLACEHOLDER_SIZES = ['small', 'medium', 'large'];
const RADIUS = { small: 5, medium: 8, large: 11 };

const INK = [C.night, C.night, C.night, C.night]; // marks and openings: one dark colour, so they read against the stone
const stone = (mask) => shade(mask, { 1: ramp('stone') });
const flat = (px) => px.map((x, y) => (px.get(x, y) ? 1 : 0));
/** A question mark from strokes, set on a mask in colour 2 (a mark, not a letter from a font). */
const mark = (m, cx, cy, c = 2) => m.line(cx - 3, cy - 4, cx - 3, cy - 5, c).line(cx - 3, cy - 5, cx - 1, cy - 7, c).line(cx - 1, cy - 7, cx + 2, cy - 7, c).line(cx + 2, cy - 7, cx + 4, cy - 5, c).line(cx + 4, cy - 5, cx + 3, cy - 2, c).line(cx + 3, cy - 2, cx, cy, c).line(cx, cy, cx, cy + 1, c).rect(cx, cy + 4, 2, 2, c);

function make(kind, { family = 'humanoid', category = 'misc', size = 'medium' }) {
  if (kind === 'character') {
    const m = new Pixels(32, 32); m.ellipse(16, 8, 5, 5.5, 1).poly([[10, 15], [22, 15], [24, 24], [8, 24]], 1).rect(11, 24, 4, 6, 1).rect(17, 24, 4, 6, 1);
    return stone(m);
  }
  if (kind === 'family') {
    const preset = PLACEHOLDER_FAMILIES[family]; if (!preset) throw new Error(`unknown family "${family}"; known: ${Object.keys(PLACEHOLDER_FAMILIES).join(', ')}`);
    return stone(flat(drawCreature({ preset }).frames[0]));
  }
  if (kind === 'item') {
    const shape = PLACEHOLDER_CATEGORIES[category]; if (!shape) throw new Error(`unknown category "${category}"; known: ${Object.keys(PLACEHOLDER_CATEGORIES).join(', ')}`);
    return stone(flat(drawItem({ shape }).frames[0]));
  }
  if (kind === 'building') {
    const m = new Pixels(32, 32); m.poly([[3, 14], [16, 3], [29, 14]], 1).rect(6, 14, 20, 15, 1).rect(14, 21, 4, 8, 2);
    return shade(m, { 1: ramp('stone'), 2: INK });
  }
  if (kind === 'exhibit') {
    const m = new Pixels(32, 32); m.rect(3, 3, 26, 26, 1).rect(6, 6, 20, 20, 2); mark(m, 15, 15, 1);
    return shade(m, { 1: ramp('stone'), 2: INK });
  }
  if (!RADIUS[size]) throw new Error(`unknown size "${size}"; known: ${PLACEHOLDER_SIZES.join(', ')}`);
  const r = RADIUS[size], m = new Pixels(32, 32); m.ellipse(16, 16, r, r, 1);
  if (size !== 'small') mark(m, 15, 15 + (size === 'large' ? 1 : 0)); else m.rect(15, 14, 2, 4, 2);
  return shade(m, { 1: ramp('stone'), 2: INK });
}

/** `kind`: character, family (`family`), item (`category`), building, exhibit, or unknown (`size`). */
export function drawPlaceholder({ kind = 'unknown', family, category, size } = {}) {
  if (!PLACEHOLDER_KINDS.includes(kind)) throw new Error(`unknown placeholder kind "${kind}"; known: ${PLACEHOLDER_KINDS.join(', ')}`);
  const px = make(kind, { family, category, size });
  return { id: `placeholder.${kind}${kind === 'family' ? `.${family ?? 'humanoid'}` : kind === 'item' ? `.${category ?? 'misc'}` : kind === 'unknown' ? `.${size ?? 'medium'}` : ''}`, frames: [px], fps: 0, loop: false, anchors: { root: [px.w >> 1, px.h - 2] } };
}
