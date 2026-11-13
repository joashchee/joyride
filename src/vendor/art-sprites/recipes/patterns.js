// 1-bit patterns (16x16, tileable): two colours, no shading. Each kind decides ink or paper per pixel from (x, y) alone, so the tile
// repeats seamlessly on a 16 grid (`noise` draws from the seed, mirrored so its edges meet). Ink and paper are ramp names: ink takes
// the darkest step of its ramp, paper the lightest.
import { Pixels } from '../pixels.js';
import { ramp } from '../palette.js';
import { rng } from '../rng.js';

const S = 16;
/** Each kind returns true where the ink goes. */
export const PATTERNS = {
  checker: (x, y) => (x + y) % 2 === 0,
  checker4: (x, y) => ((x >> 2) + (y >> 2)) % 2 === 0,
  stripes: (x) => x % 4 < 2,
  hstripes: (x, y) => y % 4 < 2,
  diagonal: (x, y) => (x + y) % 4 < 2,
  dots: (x, y) => x % 4 === 1 && y % 4 === 1,
  grid: (x, y) => x % 8 === 0 || y % 8 === 0,
  bricks: (x, y) => y % 8 === 0 || x % 8 === ((y >> 3) & 1 ? 4 : 0),
  diamonds: (x, y) => Math.abs((x % 8) - 4) + Math.abs((y % 8) - 4) === 3,
  waves: (x, y) => y % 8 === [3, 2, 1, 1, 2, 3, 4, 4][x % 8],
  rings: (x, y) => [[4, 0], [0, 4], [8, 4], [4, 8]].some(([cx, cy]) => Math.abs(Math.hypot((x % 8) + 0.5 - cx, (y % 8) + 0.5 - cy) - 3.6) < 0.55),
  weave: (x, y) => ((x >> 2) + (y >> 2)) % 2 ? x % 2 === 0 : y % 2 === 0,
  hatch: (x, y) => (x + y) % 4 === 0 || (x - y + 16) % 4 === 0,
};
export const PATTERN_IDS = [...Object.keys(PATTERNS), 'noise'];

/** Draw one pattern tile. `density` (0 to 1) is how much of `noise` is ink. */
export function drawPattern({ kind = 'checker', seed = 'pattern', ink = 'iron', paper = 'white', density = 0.35 } = {}) {
  if (!PATTERN_IDS.includes(kind)) throw new Error(`unknown pattern "${kind}"`);
  const on = ramp(ink)[0], off = ramp(paper)[3], p = new Pixels(S, S);
  if (kind === 'noise') {
    const r = rng(`${seed}:${kind}`), half = S / 2, cells = [];
    for (let y = 0; y < half; y++) for (let x = 0; x < half; x++) cells.push(r.next() < density);
    // Mirror one quarter into four, so every edge meets its opposite edge.
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) p.set(x, y, cells[(y < half ? y : S - 1 - y) * half + (x < half ? x : S - 1 - x)] ? on : off);
  } else for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) p.set(x, y, PATTERNS[kind](x, y) ? on : off);
  return { id: `pattern.${kind}`, frames: [p], fps: 0, loop: false };
}
