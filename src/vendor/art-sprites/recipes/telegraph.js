// Combat feedback for the top-down world (T19): the area a blow will land on (`telegraph`), who is selected or whose turn it is (`target`), and the
// floating number of a hit (`drawNumber`, which needs the VGA atlas). Colour is never the only cue (Tag Team 9): a telegraph is filled with a pattern
// as well as a colour, and an ally ring, an enemy ring and a neutral ring differ in shape.
import { Pixels } from '../pixels.js';
import { C, ramp } from '../palette.js';
import { clip } from '../anim.js';
import { pixelText } from './pixel-text.js';

export const TELEGRAPH_SHAPES = ['circle', 'line', 'cone', 'rect'];
export const TELEGRAPH_KINDS = { danger: { ramp: 'red', pattern: 'hatch' }, friendly: { ramp: 'green', pattern: 'dots' }, neutral: { ramp: 'steel', pattern: 'checker' } };
const DIRS = { north: [0, -1], east: [1, 0], south: [0, 1], west: [-1, 0] };
export const DIRECTIONS = Object.keys(DIRS);
const SIZES = [32, 48, 64];

/** Whether pixel (x, y) is inside the shape, with the origin at the canvas centre and `dir` the way it points. */
function inside(shape, size, [dx, dy]) {
  const c = size / 2, r = c - 1.5;
  return (x, y) => {
    const px = x + 0.5 - c, py = y + 0.5 - c, along = px * dx + py * dy, across = Math.abs(px * -dy + py * dx);
    if (shape === 'circle') return px * px + py * py <= r * r;
    if (shape === 'line') return along >= 0 && along <= r + 1 && across <= 3.5;
    if (shape === 'cone') return along >= 0 && along <= r + 1 && across <= along * 0.9 + 1;
    return along >= -r * 0.5 && along <= r + 1 && across <= r * 0.55; // rect: in front of the origin, wide
  };
}
const FILL = { hatch: (x, y, t) => (x + y + t) % 4 < 2, dots: (x, y, t) => (x + t) % 3 === 0 && y % 3 === 0, checker: (x, y, t) => (x + y + t) % 2 === 0 };

/** The area a blow will cover, as an outline and a pattern fill (two frames: the pattern slides, so it reads as live). `shape` circle, line, cone or rect; `kind` danger, friendly or neutral; `direction` for line, cone and rect. The origin (the caster) is the canvas centre. */
export function drawTelegraph({ shape = 'circle', kind = 'danger', direction = 'east', size = 48 } = {}) {
  if (!TELEGRAPH_SHAPES.includes(shape)) throw new Error(`unknown telegraph shape "${shape}"; known: ${TELEGRAPH_SHAPES.join(', ')}`);
  if (!TELEGRAPH_KINDS[kind]) throw new Error(`unknown telegraph kind "${kind}"; known: ${Object.keys(TELEGRAPH_KINDS).join(', ')}`);
  if (!DIRS[direction]) throw new Error(`unknown direction "${direction}"; known: ${DIRECTIONS.join(', ')}`);
  if (!SIZES.includes(size)) throw new Error(`unknown telegraph size "${size}"; known: ${SIZES.join(', ')}`);
  const k = TELEGRAPH_KINDS[kind], r = ramp(k.ramp), inn = inside(shape, size, DIRS[direction]);
  const frames = [0, 2].map((t) => {
    const p = new Pixels(size, size);
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
      if (!inn(x, y)) continue;
      const edge = !inn(x - 1, y) || !inn(x + 1, y) || !inn(x, y - 1) || !inn(x, y + 1);
      if (edge) p.set(x, y, r[3]); else if (FILL[k.pattern](x, y, t)) p.set(x, y, r[1]);
    }
    return p;
  });
  return { id: `telegraph.${shape}.${kind}`, frames, fps: 4, loop: true, spec: { shape, kind, direction, size }, anchors: { root: [size / 2, size / 2], effect_origin: [size / 2, size / 2] }, clips: { play: clip('telegraph', frames, { fps: 4, loop: true, reduced: [frames[0]] }) } };
}

export const TARGET_KINDS = ['ally', 'enemy', 'neutral', 'turn', 'seat', 'shadow'];
const ring = (p, rx, ry, c) => { p.ellipse(20, 20, rx, ry, c); return p; };

/**
 * A marker drawn under or around a 40 by 40 world sprite: `ally` (a plain ring), `enemy` (a ring with four notches), `neutral` (a dashed ring),
 * `turn` (four corner brackets, two frames that open and close), `seat` (a diamond with `seat` pips, 1 to 4, for the party order) and `shadow` (a soft dithered blob).
 */
export function drawTarget({ kind = 'ally', seat = 1 } = {}) {
  if (!TARGET_KINDS.includes(kind)) throw new Error(`unknown target kind "${kind}"; known: ${TARGET_KINDS.join(', ')}`);
  const S = 40, one = (fn) => { const p = new Pixels(S, S); fn(p); return p; };
  const hollow = (p, rx, ry, c) => { ring(p, rx, ry, c); ring(p, rx - 1.2, ry - 1.2, 0); };
  let frames;
  if (kind === 'ally') frames = [one((p) => { hollow(p, 15, 15, ramp('blue')[3]); })];
  else if (kind === 'enemy') frames = [one((p) => { hollow(p, 15, 15, ramp('red')[2]); for (const [x, y] of [[20, 4], [20, 35], [4, 20], [35, 20]]) p.rect(x - 1, y - 1, 3, 3, ramp('red')[3]); })];
  else if (kind === 'neutral') frames = [one((p) => { const q = new Pixels(S, S); hollow(q, 15, 15, 1); for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (q.get(x, y) && (Math.floor(Math.atan2(y - 19.5, x - 19.5) * 6) & 1)) p.set(x, y, ramp('gold')[3]); })];
  else if (kind === 'turn') frames = [3, 0].map((o) => one((p) => { for (const [x, y, sx, sy] of [[3, 3, 1, 1], [36, 3, -1, 1], [3, 36, 1, -1], [36, 36, -1, -1]]) for (let i = 0; i < 6; i++) { p.set(x + sx * (i + o), y + sy * o, ramp('gold')[3]); p.set(x + sx * o, y + sy * (i + o), ramp('gold')[3]); } }));
  else if (kind === 'seat') { const n = Math.max(1, Math.min(4, seat)); frames = [one((p) => { p.poly([[20, 27], [26, 33], [20, 38], [14, 33]], ramp('gold')[2]); for (let i = 0; i < n; i++) p.set(20 - (n - 1) + 2 * i, 33, C.night); p.rect(19, 27, 2, 1, ramp('gold')[3]); })]; }
  else frames = [one((p) => { const q = new Pixels(S, S); q.ellipse(20, 30, 10, 4, 1); for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (q.get(x, y) && (x + y) % 2 === 0) p.set(x, y, C.slate); })];
  const animated = frames.length > 1;
  return { id: `target.${kind}`, frames, fps: animated ? 3 : 0, loop: animated, spec: { kind, seat }, anchors: { root: [20, 20], shadow_center: [20, kind === 'shadow' ? 30 : 20] } };
}

export const NUMBER_KINDS = { damage: { ink: 'white', outline: 'red' }, crit: { ink: 'yellow', outline: 'red', prefix: '!' }, heal: { ink: 'lime', outline: 'forest', prefix: '+' }, miss: { ink: 'mist', outline: 'night', word: 'miss' }, block: { ink: 'sky', outline: 'indigo', word: 'block' } };

/**
 * A floating combat number in the VGA font: `kind` damage, crit (a `!` before it, bigger outline), heal (a `+`), miss or block (the word). `atlas` is the
 * decoded VGA atlas (`loadAtlas` from `art-sprites/atlas`). The clip rises 6 pixels over 4 frames, the way a number floats off a hit; the still is the last frame.
 */
export function drawNumber(atlas, { value = 0, kind = 'damage' } = {}) {
  const k = NUMBER_KINDS[kind]; if (!k) throw new Error(`unknown number kind "${kind}"; known: ${Object.keys(NUMBER_KINDS).join(', ')}`);
  if (!k.word && !(Number.isInteger(value) && value >= 0 && value < 100000)) throw new Error('a combat number is a whole number from 0 to 99999');
  const text = k.word ?? `${k.prefix ?? ''}${value}`, base = pixelText(atlas, text, { ink: C[k.ink], outline: C[k.outline] });
  const frames = [6, 4, 2, 0].map((rise) => new Pixels(base.w, base.h + 6).blit(base, 0, rise));
  return { id: `number.${kind}`, frames, fps: 8, loop: false, spec: { value, kind }, clips: { float: clip('float', frames, { fps: 8, loop: false, events: [{ frame: 3, name: 'settled' }], reduced: [frames[3]] }) } };
}
