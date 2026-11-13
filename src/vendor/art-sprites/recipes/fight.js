// The Fight panel (ANSI only): the room with two faces in it, the player mirrored; frames for a big blow (a jolt and a flash) and for a new
// foe (a slide in); on the phone a strip of eight rows with the small 16 by 8 faces at its ends and the words between. Written for
// docs/coupler-gaps-plan.md Phase F from Coupler's `fight.json` (layout, frames, the frontend's limits); the code is this package's own.
// `reducedMotion` is a param, since a clip cannot be filtered afterwards: it takes out the jolt and the slide. Browser-safe.
import { Cells, legalize } from '../cells.js';
import { drawCreatureFace } from './face.js';
import { Layer, placeLayer, stamp, lightFigures, wildTerrain, fitted } from './figure.js';
import { checkColors, IDENTITY256 } from './vista-canvas.js';

export const FIGHT_LAYOUTS = ['wide', 'phone'];
/** A fall of health, out of 100, in one blow that is a big one: it jolts and flashes the one hit. */
export const BIG_HIT = 15;
/** How long a frame of a blow and of a slide lasts, in milliseconds. */
export const BLOW_MS = 100, SLIDE_MS = 24;
/** The flashes of a fight are never closer than this: the frontend's limit, three a second at most. */
export const FLASH_GAP_MS = 334;
const FACE = { wide: { w: 25, rows: 12, least: 40, size: 'full', columns: 64 }, phone: { w: 16, rows: 8, least: 32, size: 'small', columns: 64 } };
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const IDENTITY = Array.from({ length: 16 }, (_, i) => i);

/** The shifts of a blow, frame by frame, as Coupler builds them (`blowFrames`): the one hit steps a cell back and flashes, then the jolt
 *  alone. Reduced motion has no jolt: a flash alone if one is allowed, else no frames. */
export function blowFrames(hit, flash = true, reduced = false) {
  const back = (shift, flashed) => ({ you: { shift: hit === 'you' ? -shift : 0, flash: hit === 'you' && flashed }, foe: { shift: hit === 'foe' ? shift : 0, flash: hit === 'foe' && flashed } });
  if (reduced) return flash ? [back(0, true)] : [];
  return [back(1, flash), back(1, false)];
}
/** A new foe sliding in from the right, one cell a frame, from `width - 1` to 0; none with reduced motion. */
export function slideFrames(width, reduced = false) {
  return reduced ? [] : Array.from({ length: width }, (_, i) => ({ you: { shift: 0, flash: false }, foe: { shift: width - 1 - i, flash: false } }));
}

// Words between the faces of the phone strip, centred in a band and cut to fit.
function writeWords(cells, lines, x0, x1, rows) {
  const width = x1 - x0; if (width < 1) return;
  const shown = lines.slice(0, rows).map((l) => String(l).slice(0, width)), top = Math.floor((cells.h - shown.length) / 2);
  shown.forEach((line, i) => { const left = x0 + Math.floor((width - line.length) / 2); [...line].forEach((ch, k) => cells.set(left + k, top + i, { ch, fg: 15, bg: 0 })); });
}

/**
 * The Fight panel. Params: `you` and `foe` (what `creatureface` takes, or `{ figure }`; the player is mirrored), `layout` ('wide', the faces
 * at 25 by 12 cells, or 'phone', the small ones in a strip of 8 rows), `columns` (clamped to 40 to 160, or 32 to 160 on the phone), the
 * room (`terrain`, `time`, `weather`, `style`, `key`, `wild` for an animal's place; none gives a black-to-gray gradient), `words` (lines
 * between the faces on the phone), `pictures: false` (the phone with pictures off: four rows of words only), `hit` ('you' or 'foe') with
 * `fell` (health lost, out of 100: from 15 on it is a big blow and gives the `blow` clip), `flash` (false when the last flash was less than
 * 334 ms ago), `colors` (16, 256 or 'true': the room and the faces in blended tones, the flash a blend toward white), `slide` (a new foe: the `slide` clip), `reducedMotion` (no jolt, no slide), `seed`, `bgLimit`. A flash comes at most once
 * a blow. Returns the Art with the still, `says`, and the clips.
 */
export function drawFight(params = {}) {
  const layout = params.layout ?? 'wide';
  if (!FIGHT_LAYOUTS.includes(layout)) throw new Error(`unknown fight layout "${layout}"; known: ${FIGHT_LAYOUTS.join(', ')}`);
  const hit = params.hit; if (hit !== undefined && hit !== 'you' && hit !== 'foe') throw new Error(`hit is "you" or "foe", not "${hit}"`);
  const colors = checkColors(params.colors ?? 16), spec = FACE[layout], bgLimit = params.bgLimit ?? 8, reduced = params.reducedMotion === true, words = (params.words ?? []).map(String);
  const columns = clamp(Math.round(params.columns ?? spec.columns), spec.least, 160), rows = spec.rows, flashOk = params.flash !== false;
  const says = (a) => String(a.says ?? '').replace(/\.$/, '');
  if (layout === 'phone' && params.pictures === false) {
    const cells = new Cells(columns, 4); for (let y = 0; y < 4; y++) for (let x = 0; x < columns; x++) cells.set(x, y, { ch: ' ', fg: 15, bg: 0 });
    writeWords(cells, words, 0, columns, 4);
    return { id: 'fight.words', frames: [legalize(cells, { bgLimit })], fps: 0, loop: false, says: words.join(' ') };
  }
  const drawn = (spec_, fallback) => (spec_?.figure ? undefined : drawCreatureFace({ colors, ...(spec_ ?? { race: fallback }), size: spec.size }));
  const youArt = drawn(params.you, 'Human'), foeArt = drawn(params.foe, 'Orc'), you = params.you?.figure ?? youArt.figure, foe = params.foe?.figure ?? foeArt.figure;
  if (you.w !== spec.w || you.h !== rows * 2 || foe.w !== spec.w || foe.h !== rows * 2) throw new Error(`the ${layout} fight takes faces of ${spec.w} by ${rows * 2} pixels`);
  // The room: the place a shade darker, or the gradient with no room.
  let room;
  if (params.terrain !== undefined) {
    const terrain = params.wild ? wildTerrain(params.terrain, params.key ?? '') : params.terrain;
    room = placeLayer({ terrain, time: params.time, weather: params.weather, style: params.style, key: params.key ?? '', columns, rows, colors });
  } else { room = new Layer(columns, rows * 2, colors); room.gradient(0, rows * 2, [0, 0, 8]); }
  const toCells = (frame) => {
    const layer = room.clone();
    stamp(layer, you, frame.you.shift, 0, { mirror: true, flash: frame.you.flash });
    stamp(layer, foe, columns - spec.w + frame.foe.shift, 0, { flash: frame.foe.flash });
    lightFigures(layer);
    const cells = layer.toCells(colors === 16 ? IDENTITY : IDENTITY256);
    if (layout === 'phone') writeWords(cells, words, spec.w, columns - spec.w, rows);
    return fitted(cells, colors, bgLimit);
  };
  const at = (you, foe) => toCells({ you, foe }), rest = { shift: 0, flash: false };
  const still = at(rest, rest), clips = {};
  const big = hit !== undefined && (params.fell ?? 0) >= BIG_HIT;
  if (big) {
    const frames = blowFrames(hit, flashOk, reduced).map((f) => toCells(f));
    if (frames.length) clips.blow = { id: 'blow', frames: reduced ? frames : [...frames, still], fps: 1000 / BLOW_MS, loop: false, events: [], reduced: [frames[0]] };
  }
  if (params.slide && !reduced) clips.slide = { id: 'slide', frames: slideFrames(spec.w).map((f) => toCells(f)).filter((c, i, all) => !i || !c.equals(all[i - 1])), fps: 1000 / SLIDE_MS, loop: false, events: [], reduced: [still] };
  const youSays = youArt ? says(youArt) : '', foeSays = foeArt ? says(foeArt) : '';
  const art = { id: `fight.${layout}`, frames: [still], fps: 0, loop: false, says: [foeSays && `Foe: ${foeSays}.`, youSays && `You: ${youSays}.`].filter(Boolean).join(' ') };
  if (Object.keys(clips).length) art.clips = clips;
  if (colors !== 16) art.colors = colors;
  return art;
}
