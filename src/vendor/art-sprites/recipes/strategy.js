// Strategy pieces (16x16): pointy-top hex terrain tiles and tower-defense towers. A mask of part ids (1 body, 2 second colour, 3 accent),
// shaded by shade.js from each kind's own ramps. Shapes are drawn in a 14 by 14 area and offset by one pixel so the outline keeps a margin.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';

const S = 16;
/** The hex itself (part 1): a pointy-top hexagon filling the area. */
const hex = (m) => m.poly([[7, 0], [13, 3], [13, 10], [7, 13], [1, 10], [1, 3]], 1);

export const STRATEGY = {
  'hex-grass': { ramps: { 1: 'grass', 2: 'moss', 3: 'green' }, draw(m) { hex(m); for (const [x, y] of [[4, 4], [9, 3], [6, 8], [10, 9], [3, 8]]) m.rect(x, y, 2, 1, 2); } },
  'hex-forest': { ramps: { 1: 'grass', 2: 'green', 3: 'wood' }, draw(m) { hex(m); for (const x of [4, 8]) { m.poly([[x, 2], [x + 3, 7], [x - 1, 7]], 2); m.rect(x, 7, 1, 2, 3); } m.poly([[6, 6], [9, 11], [3, 11]], 2); m.rect(6, 11, 1, 1, 3); } },
  'hex-mountain': { ramps: { 1: 'grass', 2: 'white', 3: 'stone' }, draw(m) { hex(m); m.poly([[2, 11], [6, 3], [10, 11]], 3); m.poly([[7, 11], [10, 5], [13, 11]], 3); m.poly([[6, 3], [4, 7], [8, 7]], 2); } },
  'hex-water': { ramps: { 1: 'water', 2: 'ice', 3: 'glass' }, draw(m) { hex(m); for (const [x, y] of [[3, 4], [8, 6], [4, 9], [9, 10]]) { m.rect(x, y, 3, 1, 2); } } },
  'hex-desert': { ramps: { 1: 'sand', 2: 'gold', 3: 'dirt' }, draw(m) { hex(m); m.line(2, 5, 5, 3, 3); m.line(5, 9, 11, 6, 3); m.line(8, 11, 11, 9, 3); m.rect(10, 3, 2, 1, 2); } },
  'hex-snow': { ramps: { 1: 'white', 2: 'ice', 3: 'glass' }, draw(m) { hex(m); for (const [x, y] of [[4, 4], [9, 5], [6, 9], [10, 9]]) m.rect(x, y, 2, 1, 2); m.rect(7, 6, 1, 1, 3); } },
  'tower-archer': { ramps: { 1: 'stone', 2: 'wood', 3: 'steel' }, draw(m) { m.rect(3, 6, 8, 8, 1); for (const x of [3, 6, 9]) m.rect(x, 4, 2, 2, 1); m.rect(4, 2, 6, 2, 2); m.rect(6, 8, 2, 3, 3); m.line(7, 0, 7, 2, 3); } },
  'tower-cannon': { ramps: { 1: 'stone', 2: 'iron', 3: 'fire' }, draw(m) { m.rect(2, 7, 10, 7, 1); m.rect(1, 5, 12, 2, 1); m.rect(5, 1, 7, 3, 2); m.rect(10, 0, 3, 5, 2); m.rect(5, 9, 2, 3, 2); m.set(13, 2, 3); } },
  'tower-mage': { ramps: { 1: 'purple', 2: 'stone', 3: 'glass' }, draw(m) { m.rect(4, 6, 6, 8, 2); m.poly([[7, 0], [11, 6], [3, 6]], 1); m.rect(5, 9, 4, 1, 1); m.ellipse(7, 3, 1, 1, 3); m.rect(6, 11, 2, 3, 1); } },
  'tower-frost': { ramps: { 1: 'ice', 2: 'steel', 3: 'white' }, draw(m) { m.rect(4, 7, 6, 7, 2); m.poly([[7, 0], [10, 4], [7, 8], [4, 4]], 1); m.rect(3, 7, 8, 1, 1); m.line(7, 1, 7, 6, 3); m.rect(6, 10, 2, 4, 1); } },
};
export const STRATEGY_IDS = Object.keys(STRATEGY);

/** Draw one strategy piece. `ink` overrides the ramp of the body (part 1), e.g. a red mage tower. */
export function drawStrategy({ kind = 'hex-grass', ink } = {}) {
  const def = STRATEGY[kind]; if (!def) throw new Error(`unknown strategy kind "${kind}"`);
  const m = new Pixels(S, S), area = new Pixels(S - 2, S - 2);
  def.draw(area);
  m.blit(area, 1, 1);
  const out = shade(m, Object.fromEntries(Object.entries(def.ramps).map(([part, name]) => [part, ramp(part === '1' && ink ? ink : name)])));
  return { id: `strategy.${kind}`, frames: [out], fps: 0, loop: false };
}
