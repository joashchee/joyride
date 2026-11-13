// What every drawing carries beside its pixels, filled in by `draw()` for any recipe that did not set it itself:
//   alt         a short factual phrase for a screen reader (Tag Team 9): what it is, never how it looks to the eye
//   anchors     named points; at least root, shadow_center, head and effect_origin, from the drawn pixels (a recipe's own anchors win)
//   bounds      the box the drawing occupies
//   reduced     the still to show instead of an animation (every clip has one, and so does an animated drawing)
//   projection  how the world is seen: 'front' for the paper-doll and creature views; absent where a recipe has not said
// None of this changes a pixel. It is data a compositor, a manifest or a screen reader reads.
import { Pixels } from './pixels.js';

/** The projection a recipe draws in, where it is known. Top-down arrives with the top-down set. */
export const PROJECTION = { character: 'front', portrait: 'front', creature: 'front', telegraph: 'top-down', target: 'top-down' };

/** Anchors from the box the drawing fills: root and shadow_center at the foot, head at the top, effect_origin in the middle. */
export function boxAnchors(px) {
  const b = px.bbox(); if (!b) return { root: [px.w >> 1, px.h - 1], shadow_center: [px.w >> 1, px.h - 1], head: [px.w >> 1, 0], effect_origin: [px.w >> 1, px.h >> 1] };
  const cx = b.x + (b.w >> 1), foot = b.y + b.h - 1;
  return { root: [cx, foot], shadow_center: [cx, foot], head: [cx, b.y], effect_origin: [cx, b.y + (b.h >> 1)] };
}

const words = (s) => String(s).replace(/[-_.]/g, ' ');
const list = (xs) => xs.filter(Boolean).join(', ');
const HEAD = { hair: 'bare head', helm: 'helm', horned: 'horned helm', hood: 'hood', hat: 'hat', circlet: 'circlet' };

/** A short phrase for the art, from its recipe id, params and (for a character) its resolved spec. */
export function altOf(id, params, art) {
  const s = art.spec;
  if (id === 'character' && s) {
    const gear = [s.weapon, s.offhand].filter(Boolean).map(words).join(' and ') || 'unarmed';
    return list([`${params.loadout ?? 'custom'} character`, s.view === 'top' ? `top-down view facing ${s.facing}` : `${s.view} view`, s.pose && words(s.pose), HEAD[s.head], s.torso, gear, s.back && words(s.back)]);
  }
  if (id === 'portrait' && s) return list([`${params.loadout ?? 'custom'} portrait`, HEAD[s.head]]);
  if (id === 'creature' && s) return list([`${words(params.race ?? params.preset ?? params.family ?? s.body)} creature`, `${words(s.body)} body`, s.rank && s.rank !== 'normal' && s.rank, s.view === 'top' && `top-down view facing ${s.facing ?? 'south'}`]);
  const kv = Object.entries(params).filter(([k, v]) => ['string', 'number', 'boolean'].includes(typeof v) && k !== 'seed').map(([k, v]) => (typeof v === 'string' ? words(v) : `${words(k)} ${v}`));
  return list([words(art.id), ...kv.filter((x) => !words(art.id).includes(x))]);
}

/** Fill in what a recipe left out. Idempotent: it never overwrites a field the recipe set. Returns the same object. */
export function decorate(id, params, art) {
  if (!art || !art.frames) return art;
  art.alt ??= altOf(id, params ?? {}, art);
  if (PROJECTION[id]) art.projection ??= PROJECTION[id];
  const first = art.frames[0];
  if (first instanceof Pixels) {
    art.anchors = { ...boxAnchors(first), ...art.anchors };
    const b = first.bbox(); if (b) art.bounds ??= b;
  }
  if (art.fps > 0 && art.frames.length > 1) art.reduced ??= [art.frames[0]];
  for (const c of Object.values(art.clips ?? {})) { c.reduced ??= [c.frames[0]]; c.events ??= []; }
  return art;
}
