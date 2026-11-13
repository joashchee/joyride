// The ANSI learning kit's rules as checks a cell drawing can fail: what the file format forbids (backgrounds, width, character set,
// colour count) and what the tutorials say to avoid (shade noise, short gradients, a jump from dark gray to white, no outline, a
// flashing loop). Findings are `{ rule, level: 'warn' | 'error', message }`, like lint.js. Not written: `gap` (shapes that touch
// with no one-cell separation) needs a notion of "shape" that cells do not carry yet.
import { NONE, BLINK } from './cells.js';
import { VGA16, indexHex } from './palette-ansi.js';
import { CP437 } from './ansi.js';
import { luma } from './palette.js';

const IN_CP437 = new Set(CP437);
const SHADE = new Set([0x2591, 0x2592, 0x2593]);
// Characters that are drawing, not text: space, the shade and block characters, the box-drawing range.
const isDrawing = (cp) => cp === 0x20 || (cp >= 0x2500 && cp <= 0x259f);
const hex = (i) => (i < 256 ? indexHex(i) : VGA16[0]);
const lum = (i) => luma(hex(i));
const BG_LIMIT = { classic: 8, ice: 16, modern: 16 };

/** Findings for one cell drawing. Options: `target` (classic, ice, modern), `width` (80), `maxHeight` (24, the classic scroll limit),
 *  `depth` (16, 256 or 'true': 16 and 256 limit the indexes used), `name`, `skip` (rule names). */
/** Whether a cell's foreground and background are one colour: the same index, and where the picture has true colour, the same RGB too. */
const sameColor = (cells, i) => {
  if (cells.fg[i] !== cells.bg[i]) return false;
  const tc = cells.tc; return !tc || tc.fg[i] === tc.bg[i];
};

export function lintAnsi(cells, { target = 'classic', width = 80, maxHeight = 24, depth = 16, name = 'ansi', skip = [] } = {}) {
  const out = [], add = (rule, level, message) => { if (!skip.includes(rule)) out.push({ rule, level, message: `${name}: ${message}` }); };
  if (!BG_LIMIT[target]) throw new Error(`unknown target "${target}"; known: ${Object.keys(BG_LIMIT).join(', ')}`);
  const { w, h } = cells, n = w * h, limit = BG_LIMIT[target];
  let any = false;
  for (let i = 0; i < n; i++) if (cells.ch[i] || cells.fg[i] !== NONE || cells.bg[i] !== NONE) { any = true; break; }
  if (!any) { add('empty', 'error', 'has no cells'); return out; }
  if (w > width) add('width', 'error', `${w} columns; the width is ${width}`);
  if (target === 'classic' && h > maxHeight) add('height', 'warn', `${h} rows; a classic screen scrolls past ${maxHeight}`);

  const used = new Set();
  let bright = 0, blink = 0, outside = 0, invisible = 0, lowContrast = 0;
  for (let i = 0; i < n; i++) {
    const ch = cells.ch[i], fg = cells.fg[i], bg = cells.bg[i];
    const showsFg = ch !== 0 && ch !== 0x20;
    if (showsFg && fg !== NONE) used.add(fg);
    if (bg !== NONE) used.add(bg);
    if (bg !== NONE && bg >= 8 && bg < 16 && limit === 8 && !(cells.tc && cells.tc.bg[i] !== -1)) bright++;   // exact RGB is not a classic index
    if ((cells.flags[i] & BLINK) && target === 'classic') blink++;
    if (target !== 'modern' && ch && !IN_CP437.has(ch)) outside++;
    if (showsFg && fg !== NONE && sameColor(cells, i) && ch !== 0x2588) invisible++;
    else if (showsFg && !isDrawing(ch) && fg !== NONE && bg !== NONE && Math.abs(lum(fg) - lum(bg)) < 50) lowContrast++;
  }
  const top = depth === 16 ? 16 : depth === 256 ? 256 : Infinity;
  if (used.size > 0 && [...used].some((c) => c >= top)) add('colors', 'error', `uses colour indexes beyond the ${depth}-colour set`);
  if (bright) add('bg-limit', 'error', `${bright} cell(s) with a bright background; a ${target} file has ${limit} (legalize it, or use the ice target)`);
  if (blink) add('blink', 'warn', `${blink} cell(s) blink: in a classic file the blink bit really blinks (use the ice target for bright backgrounds)`);
  if (outside) add('charset', 'warn', `${outside} cell(s) with a character outside CP437; a ${target} file writes them as ?`);
  if (invisible) add('invisible', 'warn', `${invisible} cell(s) with a character in the colour of its own background`);
  if (lowContrast) add('contrast', 'warn', `${lowContrast} text cell(s) with foreground and background within 50 of each other in brightness`);

  const at = (x, y) => (x >= 0 && y >= 0 && x < w && y < h ? y * w + x : -1);
  const solid = (i) => i < 0 || cells.ch[i] === 0 || cells.ch[i] === 0x20 || cells.ch[i] === 0x2588;
  // shade-noise: one shade glyph with all four neighbours solid or empty (a deliberate gradient has shade glyphs beside each other).
  let noise = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = at(x, y);
    if (SHADE.has(cells.ch[i]) && [at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1)].every(solid)) noise++;
  }
  if (noise > 4) add('shade-noise', 'warn', `${noise} lone shade glyphs among solid cells; place shading on purpose (stars and rain are overlays, not shading)`);

  // ramp: a column run of shade glyphs between two solid cells is a gradient of the run plus its two ends; under four steps is too
  // short for a smooth blend (a ladder, ends and three shades, is five). And dark gray beside white with nothing between has no
  // step at all (the kit: dark gray cannot reach white directly).
  let shortRamps = 0, jumps = 0;
  const isSolid = (i) => i >= 0 && (cells.ch[i] === 0x20 || cells.ch[i] === 0x2588);
  for (let x = 0; x < w; x++) for (let y = 0; y < h; y++) {
    if (!SHADE.has(cells.ch[at(x, y)]) || SHADE.has(cells.ch[at(x, y - 1)])) continue;
    let len = 0; while (SHADE.has(cells.ch[at(x, y + len)])) len++;
    // Only a run whose ends are the two colours of its shade glyphs is a gradient; one cut off by a shape in front of it is not.
    const before = at(x, y - 1), after = at(x, y + len), i = at(x, y);
    if (!isSolid(before) || !isSolid(after) || len + 2 >= 4) continue;
    const solidColour = (j) => (cells.ch[j] === 0x20 ? cells.bg[j] : cells.fg[j]), f = cells.fg[i], b = cells.bg[i];
    if ((solidColour(before) === b && solidColour(after) === f) || (solidColour(before) === f && solidColour(after) === b)) shortRamps++;
  }
  const flat = (i) => (cells.ch[i] === 0x20 ? cells.bg[i] : cells.ch[i] === 0x2588 ? cells.fg[i] : -1);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) for (const j of [at(x + 1, y), at(x, y + 1)]) {
    const a = flat(at(x, y)), b = j < 0 ? -1 : flat(j);
    if ((a === 8 && b === 15) || (a === 15 && b === 8)) jumps++;
  }
  if (shortRamps) add('ramp', 'warn', `${shortRamps} gradient(s) of only two or three steps; the kit wants about four`);
  if (jumps) add('ramp', 'warn', `${jumps} place(s) where dark gray meets white with no step between`);

  // outline: where the art stands on empty cells, more than half its edge being light means no dark contour.
  let edge = 0, light = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = at(x, y); if (cells.ch[i] === 0 && cells.fg[i] === NONE && cells.bg[i] === NONE) continue;
    const empty = (j) => j >= 0 && cells.ch[j] === 0 && cells.fg[j] === NONE && cells.bg[j] === NONE;
    if (![at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1)].some(empty)) continue;
    edge++;
    const c = cells.ch[i] === 0x20 ? cells.bg[i] : cells.fg[i];
    if (c !== NONE && lum(c) > 90) light++;
  }
  if (edge && light / edge > 0.5) add('outline', 'warn', 'more than half the edge is light: no dark contour around the figure');
  return out;
}

/** Findings for an animation (frames of cells at `fps`): identical frames, a long loop, and flashing. A flash is a jump in the mean
 *  brightness of more than 25; more than three in any second of the loop is a warning (the photosensitivity limit Coupler holds). */
export function lintAnsiFrames(frames, { name = 'clip', fps = 8, loop = true } = {}) {
  const out = [];
  if (frames.length < 2) return out;
  const mean = (c) => { let s = 0; for (let i = 0; i < c.ch.length; i++) { const k = c.ch[i] === 0x20 ? c.bg[i] : c.fg[i]; s += k === NONE ? 0 : lum(k); } return s / c.ch.length; };
  const means = frames.map(mean), jumps = [];
  for (let i = 1; i < frames.length; i++) {
    if (frames[i].equals(frames[i - 1])) out.push({ rule: 'frames', level: 'warn', message: `${name}: frame ${i} is identical to frame ${i - 1}` });
    if (Math.abs(means[i] - means[i - 1]) > 25) jumps.push(i);
  }
  if (loop && Math.abs(means[0] - means[frames.length - 1]) > 25) jumps.push(0);
  const span = Math.max(1, Math.ceil(fps));
  for (const j of jumps) if (jumps.filter((k) => ((k - j + frames.length) % frames.length) < span || (!loop && k >= j && k - j < span)).length > 3) {
    out.push({ rule: 'flash', level: 'error', message: `${name}: more than three flashes in a second (at ${fps} fps) starting at frame ${j}` }); break;
  }
  if (loop && frames.length > 8) out.push({ rule: 'length', level: 'warn', message: `${name}: ${frames.length} frames; keep a loop short` });
  return out;
}
