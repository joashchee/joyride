// Farm and food (16x16): crops, produce and two farm animals, the ones `items` does not draw (it has bread, cheese, apple, meat).
// A mask of part ids (1 body, 2 leaf or second colour, 3 accent), shaded by shade.js from each kind's own ramps.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';

const S = 16;

// Shapes are drawn in a 14 by 14 area; `draw` below offsets them by one pixel so the outline keeps a margin.
export const FARM = {
  wheat: { ramps: { 1: 'sand', 2: 'grass', 3: 'gold' }, draw(m) { m.line(7, 13, 7, 6, 2); m.line(7, 11, 4, 8, 2); m.line(7, 11, 10, 8, 2); for (const [x, y] of [[7, 1], [7, 3], [7, 5], [5, 3], [5, 5], [9, 3], [9, 5]]) m.rect(x - 1, y, 2, 2, 1); m.rect(6, 0, 2, 2, 3); } },
  carrot: { ramps: { 1: 'fire', 2: 'grass', 3: 'dirt' }, draw(m) { m.poly([[4, 4], [10, 4], [8, 13], [6, 13]], 1); m.line(7, 4, 4, 0, 2); m.line(7, 4, 7, 0, 2); m.line(7, 4, 10, 0, 2); m.line(5, 7, 8, 7, 3); m.line(6, 10, 8, 10, 3); } },
  pumpkin: { ramps: { 1: 'fire', 2: 'grass', 3: 'dirt' }, draw(m) { m.ellipse(7, 9, 6, 4, 1); m.rect(6, 2, 2, 3, 2); m.line(8, 3, 11, 1, 2); m.line(7, 6, 7, 12, 3); m.line(3, 7, 3, 11, 3); m.line(11, 7, 11, 11, 3); } },
  corn: { ramps: { 1: 'gold', 2: 'grass', 3: 'sand' }, draw(m) { m.ellipse(7, 6, 3, 5, 1); m.poly([[7, 13], [2, 4], [4, 8], [7, 12]], 2); m.poly([[7, 13], [12, 4], [10, 8], [7, 12]], 2); for (let y = 2; y < 10; y += 2) m.line(5, y, 9, y, 3); } },
  turnip: { ramps: { 1: 'white', 2: 'grass', 3: 'purple' }, draw(m) { m.ellipse(7, 8, 5, 4, 1); m.poly([[6, 12], [8, 12], [7, 13]], 1); m.rect(3, 5, 9, 2, 3); m.line(7, 4, 4, 0, 2); m.line(7, 4, 7, 0, 2); m.line(7, 4, 10, 0, 2); } },
  grapes: { ramps: { 1: 'purple', 2: 'grass', 3: 'wood' }, draw(m) { for (const [x, y] of [[2, 4], [6, 4], [10, 4], [4, 7], [8, 7], [6, 10], [3, 10], [9, 10]]) m.ellipse(x + 1, y + 1, 2, 2, 1); m.rect(6, 0, 2, 2, 3); m.poly([[7, 0], [12, 1], [11, 3], [8, 2]], 2); } },
  egg: { ramps: { 1: 'bone', 2: 'wood', 3: 'sand' }, draw(m) { m.ellipse(7, 8, 4, 5, 1); m.ellipse(7, 4, 3, 4, 1); m.line(5, 10, 9, 10, 3); } },
  mushroom: { ramps: { 1: 'red', 2: 'bone', 3: 'white' }, draw(m) { m.ellipse(7, 5, 6, 4, 1); m.rect(1, 5, 12, 2, 1); m.rect(5, 7, 4, 6, 2); for (const [x, y] of [[3, 3], [7, 1], [10, 4]]) m.rect(x, y, 2, 2, 3); } },
  hen: { ramps: { 1: 'white', 2: 'red', 3: 'gold' }, draw(m) { m.ellipse(6, 8, 5, 4, 1); m.ellipse(10, 3, 3, 3, 1); m.rect(9, 0, 3, 2, 2); m.rect(12, 3, 2, 2, 3); m.rect(10, 6, 2, 2, 2); m.poly([[1, 5], [0, 3], [3, 7]], 1); m.line(5, 12, 5, 13, 3); m.line(8, 12, 8, 13, 3); m.line(4, 9, 8, 9, 3); } },
  sheep: { ramps: { 1: 'white', 2: 'iron', 3: 'bone' }, draw(m) { m.ellipse(6, 7, 5, 4, 1); m.ellipse(3, 5, 3, 3, 1); m.ellipse(8, 5, 3, 3, 1); m.ellipse(11, 7, 2, 3, 2); m.rect(10, 4, 3, 2, 2); m.rect(3, 10, 2, 4, 2); m.rect(8, 10, 2, 4, 2); } },
};
export const FARM_IDS = Object.keys(FARM);

/** Draw one farm picture. `ink` overrides the ramp of the body (part 1), e.g. a purple carrot. */
export function drawFarm({ kind = 'wheat', ink } = {}) {
  const def = FARM[kind]; if (!def) throw new Error(`unknown farm kind "${kind}"`);
  const m = new Pixels(S, S);
  const area = new Pixels(S - 2, S - 2);
  def.draw(area);
  m.blit(area, 1, 1);
  const out = shade(m, Object.fromEntries(Object.entries(def.ramps).map(([part, name]) => [part, ramp(part === '1' && ink ? ink : name)])));
  return { id: `farm.${kind}`, frames: [out], fps: 0, loop: false };
}
