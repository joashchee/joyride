// Items and equipment icons (16x16). The learning kit's item workflow, as code: a silhouette, a three-value shading pass, a
// material cue, one accent colour. The items kit's four tiers (mundane, crafted, enchanted, legendary) change the finish, not the shape.
import { Pixels } from '../pixels.js';
import { shade, rim as rimPass } from '../shade.js';
import { C, RAMPS, ramp } from '../palette.js';
import { rng } from '../rng.js';

const S = 16;
/** The rim of a part: one pixel at 16 and 24, two at 32, so a shield's edge keeps its weight on the bigger canvas. */
const rim = (m, from, to) => { for (let i = 0; i < (m.k >= 2 ? 2 : 1); i++) rimPass(m, from, to); return m; };
/** Tiers from the items kit; `accent` is the ramp the gem part takes at that tier. */
export const TIERS = { mundane: { accent: 'iron' }, crafted: { accent: 'red' }, enchanted: { accent: 'glass' }, legendary: { accent: 'fire' } };

/**
 * Shapes draw part ids onto a mask: 1 main material, 2 secondary, 3 handle or grip, 4 gem. `grip` is where a hand holds it,
 * used to put a weapon into a character's hand. `mat` names the default ramp for each part.
 */
export const SHAPES = {
  sword: { mat: { 1: 'steel', 2: 'gold', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(13, 2, 5, 10, 1, 2).set(14, 1, 1); m.line(3, 9, 7, 13, 2); m.line(4, 12, 3, 13, 3); m.set(2, 14, 4); } },
  dagger: { mat: { 1: 'steel', 2: 'gold', 3: 'leather' }, grip: [4, 12], draw(m) { m.thick(11, 4, 6, 9, 1, 2); m.line(4, 8, 8, 12, 2); m.line(5, 11, 3, 13, 3); m.set(2, 14, 2); } },
  axe: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(3, 14, 10, 4, 3, 2); m.poly([[8, 1], [13, 2], [14, 7], [10, 8], [11, 5]], 1); m.line(8, 1, 10, 8, 2); } },
  mace: { mat: { 1: 'iron', 2: 'steel', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(3, 14, 9, 7, 3, 2); m.ellipse(11, 4.5, 3.5, 3.5, 1); m.set(11, 1, 2).set(14, 4, 2).set(8, 4, 2).set(11, 8, 2); } },
  staff: { mat: { 1: 'wood', 2: 'gold', 3: 'wood' }, grip: [5, 11], draw(m) { m.line(3, 14, 11, 5, 3); m.line(4, 14, 12, 5, 3); m.ellipse(12.5, 3.5, 2, 2, 4); m.line(10, 3, 11, 6, 2); m.line(14, 3, 13, 6, 2); } },
  bow: { mat: { 1: 'wood', 2: 'white', 3: 'wood' }, grip: [10, 8], draw(m) { for (const [a, b] of [[[4, 1], [9, 4]], [[9, 4], [11, 8]], [[11, 8], [9, 12]], [[9, 12], [4, 14]]]) m.thick(a[0], a[1], b[0], b[1], 1, 2); m.line(4, 1, 4, 13, 2); } },
  shield: { mat: { 1: 'blue', 2: 'steel', 3: 'wood' }, grip: [8, 8], draw(m) { m.poly([[2, 2], [14, 2], [14, 9], [8, 15], [2, 9]], 1); rim(m, 1, 2); m.rect(7, 4, 2, 7, 3).rect(5, 6, 6, 2, 3); } },
  helm: { mat: { 1: 'steel', 2: 'iron', 3: 'red' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 5.5, 5.5, 1); m.rect(2, 8, 12, 5, 1); m.rect(5, 8, 6, 2, 2); m.rect(7, 8, 2, 5, 2); m.rect(7, 1, 2, 2, 3); } },
  armor: { mat: { 1: 'steel', 2: 'iron', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[2, 3], [5, 2], [6, 4], [10, 4], [11, 2], [14, 3], [13, 8], [12, 14], [4, 14], [3, 8]], 1); m.rect(7, 6, 2, 8, 2); m.rect(4, 11, 8, 2, 3); } },
  boots: { mat: { 1: 'leather', 2: 'iron', 3: 'wood' }, grip: [8, 8], draw(m) { for (const x of [2, 9]) { m.rect(x, 3, 4, 8, 1); m.rect(x, 10, 5, 3, 1); m.rect(x, 10, 5, 1, 2); } } },
  potion: { mat: { 1: 'red', 2: 'glass', 3: 'wood' }, grip: [8, 8], draw(m) { m.rect(6, 1, 4, 2, 3); m.rect(7, 3, 2, 3, 2); m.ellipse(8, 10.5, 4.5, 4.5, 1); m.set(6, 8, 2); m.set(6, 9, 2); } },
  chest: { mat: { 1: 'wood', 2: 'gold', 3: 'leather' }, grip: [8, 8], draw(m) { m.rect(2, 6, 12, 8, 1); m.rect(2, 4, 12, 3, 3); m.rect(2, 4, 12, 1, 2); m.rect(2, 13, 12, 1, 2); m.rect(4, 4, 1, 10, 2); m.rect(11, 4, 1, 10, 2); m.rect(7, 7, 2, 3, 4); } },
  book: { mat: { 1: 'purple', 2: 'white', 3: 'gold' }, grip: [8, 8], draw(m) { m.rect(3, 2, 10, 12, 1); m.rect(12, 3, 1, 10, 2); m.rect(7, 5, 2, 6, 3); m.rect(6, 7, 4, 2, 3); m.set(8, 4, 4); } },
  ring: { mat: { 1: 'gold', 2: 'gold', 3: 'gold' }, grip: [8, 8], draw(m) { m.ellipse(8, 9.5, 4.5, 4.5, 1); m.ellipse(8, 9.5, 2.5, 2.5, 0); m.rect(7, 3, 2, 3, 4); } },
  key: { mat: { 1: 'gold', 2: 'gold', 3: 'gold' }, grip: [4, 4], draw(m) { m.ellipse(5, 5, 3.5, 3.5, 1); m.ellipse(5, 5, 1.5, 1.5, 0); m.line(7, 7, 13, 13, 1); m.line(8, 7, 14, 13, 1); m.set(11, 14, 1).set(13, 12, 1); } },
  coin: { mat: { 1: 'gold', 2: 'gold', 3: 'gold' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 5.5, 5.5, 2); m.ellipse(8, 8, 3.5, 3.5, 1); m.rect(7, 6, 2, 4, 3); } },
  torch: { mat: { 1: 'wood', 2: 'iron', 3: 'wood', 4: 'fire' }, fixed: [4], grip: [5, 12], draw(m) { m.thick(4, 14, 8, 8, 1, 2); m.rect(6, 7, 4, 2, 2); m.ellipse(8.5, 4.5, 2.5, 3.5, 4); } },
  // Shapes added for Idle Solo's gear list: a rapier, a curved sabre, three axe heads, a warhammer, round and spiked shields, vials, charms, rolled and flat paper, headgear, a cup, a heart.
  rapier: { mat: { 1: 'steel', 2: 'gold', 3: 'leather' }, grip: [3, 13], draw(m) { m.line(14, 1, 6, 9, 1).line(13, 1, 5, 9, 1); m.line(3, 8, 8, 13, 2); m.set(4, 11, 2).set(6, 13, 2); m.line(4, 12, 3, 13, 3); m.set(2, 14, 2); } },
  sabre: { mat: { 1: 'steel', 2: 'gold', 3: 'leather' }, grip: [3, 13], draw(m) { for (const [a, b] of [[[14, 1], [13, 3]], [[13, 3], [11, 6]], [[11, 6], [8, 9]]]) m.thick(a[0], a[1], b[0], b[1], 1, 2); m.line(3, 9, 8, 14, 2); m.line(4, 12, 3, 13, 3); m.set(2, 14, 2); } },
  greatsword: { mat: { 1: 'steel', 2: 'gold', 3: 'leather' }, grip: [3, 13], draw(m) { m.thick(14, 0, 5, 9, 1, 3).thick(14, 1, 6, 9, 1, 2); m.line(2, 8, 8, 14, 2).line(3, 8, 8, 13, 2); m.line(4, 12, 3, 13, 3); m.set(2, 14, 2); } },
  axe2: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(3, 14, 10, 5, 3, 2); m.poly([[6, 1], [11, 1], [11, 6], [8, 5]], 1); m.poly([[8, 5], [12, 5], [15, 9], [11, 10]], 1); m.line(8, 5, 11, 8, 2); } },
  axegreat: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(3, 14, 11, 4, 3, 2); m.poly([[6, 0], [12, 0], [15, 5], [15, 10], [10, 9], [9, 4]], 1); m.line(9, 4, 14, 9, 2); m.set(11, 0, 2); } },
  hammer: { mat: { 1: 'iron', 2: 'steel', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(3, 14, 10, 6, 3, 2); m.poly([[7, 1], [14, 1], [15, 6], [8, 7]], 1); m.rect(7, 1, 8, 1, 2); m.rect(8, 6, 7, 1, 2); } },
  buckler: { mat: { 1: 'wood', 2: 'iron', 3: 'steel' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 6.5, 6.5, 1); rim(m, 1, 2); m.ellipse(8, 8, 2, 2, 3); } },
  targe: { mat: { 1: 'leather', 2: 'iron', 3: 'steel' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 6.5, 6.5, 1); rim(m, 1, 2); m.ellipse(8, 8, 2, 2, 3); for (const [x, y] of [[8, 1], [8, 14], [1, 8], [14, 8], [3, 3], [12, 3], [3, 12], [12, 12]]) m.set(x, y, 3); } },
  vial: { mat: { 1: 'red', 2: 'glass', 3: 'wood' }, grip: [8, 8], draw(m) { m.rect(6, 1, 4, 2, 3); m.rect(7, 3, 2, 2, 2); m.rect(5, 5, 6, 9, 2); m.rect(6, 7, 4, 6, 1); } },
  amulet: { mat: { 1: 'gold', 2: 'iron', 3: 'gold' }, grip: [8, 8], draw(m) { m.line(3, 0, 8, 7, 2); m.line(13, 0, 8, 7, 2); m.ellipse(8, 11, 4, 4, 1); m.rect(7, 9, 2, 3, 4); } },
  starcharm: { mat: { 1: 'gold', 2: 'iron', 3: 'gold' }, grip: [8, 8], draw(m) { m.line(3, 0, 8, 5, 2); m.line(13, 0, 8, 5, 2); m.poly([[8, 5], [10, 9], [14, 9], [11, 12], [12, 15], [8, 13], [4, 15], [5, 12], [2, 9], [6, 9]], 1); } },
  fang: { mat: { 1: 'bone', 2: 'leather', 3: 'gold' }, grip: [8, 8], draw(m) { m.line(5, 0, 8, 3, 2).line(11, 0, 8, 3, 2); m.rect(6, 3, 4, 2, 3); m.poly([[6, 5], [10, 5], [10, 9], [8, 15], [7, 11], [6, 8]], 1); } },
  bonecharm: { mat: { 1: 'bone', 2: 'leather', 3: 'gold' }, grip: [8, 8], draw(m) { m.line(5, 0, 8, 3, 2).line(11, 0, 8, 3, 2); m.rect(7, 3, 2, 2, 3); m.rect(7, 6, 2, 7, 1); m.rect(6, 5, 4, 2, 1); m.rect(6, 12, 4, 2, 1); } },
  scroll: { mat: { 1: 'bone', 2: 'wood', 3: 'wood' }, grip: [8, 8], draw(m) { m.rect(3, 4, 10, 8, 1); m.rect(2, 3, 2, 10, 3); m.rect(12, 3, 2, 10, 3); m.rect(7, 4, 2, 8, 4); } },
  parchment: { mat: { 1: 'bone', 2: 'leather', 3: 'wood' }, grip: [8, 8], draw(m) { m.poly([[3, 1], [13, 2], [12, 8], [13, 14], [3, 15], [4, 8]], 1); for (const y of [4, 6, 8, 10]) m.rect(5, y, 6, 1, 2); } },
  hat: { mat: { 1: 'purple', 2: 'gold', 3: 'wood' }, grip: [8, 8], draw(m) { m.poly([[9, 0], [12, 4], [11, 9], [5, 9], [6, 4]], 1); m.ellipse(8, 11, 7, 2.5, 1); m.rect(5, 8, 7, 2, 2); } },
  hood: { mat: { 1: 'green', 2: 'leather', 3: 'iron' }, grip: [8, 8], draw(m) { m.ellipse(8, 7, 6, 6.5, 1); m.poly([[2, 8], [14, 8], [15, 15], [1, 15]], 1); m.ellipse(8, 8.5, 3.5, 3.5, 3); m.rect(6, 13, 4, 2, 2); } },
  headband: { mat: { 1: 'leather', 2: 'gold', 3: 'wood' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 7, 4.5, 1); m.ellipse(8, 8.5, 5, 2.5, 0); m.rect(7, 11, 2, 2, 2); m.rect(12, 11, 2, 4, 1); m.rect(14, 12, 1, 3, 1); } },
  visor: { mat: { 1: 'steel', 2: 'iron', 3: 'red' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 5.5, 6, 1); m.rect(2, 8, 12, 6, 1); m.rect(4, 7, 8, 2, 2); m.rect(7, 9, 2, 5, 2); m.rect(7, 0, 2, 3, 3); } },
  crownhelm: { mat: { 1: 'gold', 2: 'iron', 3: 'red' }, grip: [8, 8], draw(m) { m.ellipse(8, 9, 5.5, 5, 1); m.rect(2, 9, 12, 4, 1); m.poly([[2, 7], [2, 1], [5, 5], [8, 0], [11, 5], [14, 1], [14, 7]], 1); m.rect(7, 9, 2, 4, 2); m.set(8, 2, 3); } },
  hornhelm: { mat: { 1: 'iron', 2: 'steel', 3: 'bone' }, grip: [8, 8], draw(m) { m.ellipse(8, 9, 5, 5, 1); m.rect(3, 9, 10, 5, 1); m.rect(5, 9, 6, 2, 2); m.rect(7, 9, 2, 5, 2); m.line(3, 7, 1, 2, 3).line(4, 7, 2, 2, 3).line(13, 7, 15, 2, 3).line(12, 7, 14, 2, 3); } },
  spikemail: { mat: { 1: 'iron', 2: 'steel', 3: 'bone' }, grip: [8, 8], draw(m) { m.poly([[2, 4], [5, 3], [6, 5], [10, 5], [11, 3], [14, 4], [13, 9], [12, 14], [4, 14], [3, 9]], 1); m.rect(7, 7, 2, 7, 2); m.line(2, 4, 1, 1, 3).line(14, 4, 15, 1, 3).set(8, 5, 3); } },
  goblet: { mat: { 1: 'gold', 2: 'red', 3: 'gold' }, grip: [8, 8], draw(m) { m.poly([[3, 2], [13, 2], [12, 8], [8, 10], [4, 8]], 1); m.rect(4, 3, 8, 2, 2); m.rect(7, 10, 2, 3, 3); m.rect(4, 13, 8, 2, 3); } },
  heart: { mat: { 1: 'red', 2: 'red', 3: 'red' }, grip: [8, 8], draw(m) { m.poly([[8, 14], [1, 7], [1, 4], [4, 2], [8, 5], [12, 2], [15, 4], [15, 7]], 1); m.ellipse(4.5, 5, 3, 3, 1).ellipse(11.5, 5, 3, 3, 1); } },
  // CoffeeMUD gap-fill shapes: polearms, a whip, a wand, hand and arm armour, a cloak, and the basic carried goods.
  spear: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [5, 11], draw(m) { m.line(2, 14, 11, 5, 3).line(3, 14, 12, 5, 3); m.poly([[12, 0], [15, 1], [15, 4], [12, 5], [10, 4]], 1); m.line(11, 4, 13, 2, 2); m.rect(9, 5, 3, 1, 2); } },
  halberd: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [4, 12], draw(m) { m.line(2, 15, 11, 3, 3).line(3, 15, 12, 3, 3); m.poly([[8, 1], [13, 0], [15, 5], [11, 6], [10, 4]], 1); m.poly([[7, 5], [4, 4], [5, 8], [8, 8]], 1); m.line(10, 4, 14, 2, 2); m.set(12, 0, 2); } },
  trident: { mat: { 1: 'steel', 2: 'iron', 3: 'wood' }, grip: [5, 11], draw(m) { m.line(2, 14, 11, 5, 3).line(3, 14, 12, 5, 3); m.line(9, 4, 13, 8, 1).line(7, 6, 11, 10, 1); m.line(13, 0, 14, 4, 1).line(10, 1, 11, 5, 1).line(15, 3, 15, 7, 1); m.rect(8, 7, 5, 1, 2); } },
  whip: { mat: { 1: 'leather', 2: 'iron', 3: 'wood' }, grip: [3, 13], draw(m) { m.thick(2, 14, 4, 11, 3, 2); for (const [a, b] of [[[4, 11], [8, 9]], [[8, 9], [12, 11]], [[12, 11], [13, 6]], [[13, 6], [10, 3]], [[10, 3], [13, 1]]]) m.line(a[0], a[1], b[0], b[1], 1); m.set(14, 1, 2); } },
  wand: { mat: { 1: 'wood', 2: 'gold', 3: 'wood' }, grip: [4, 12], draw(m) { m.thick(3, 13, 11, 5, 1, 2); m.rect(2, 12, 3, 3, 3); m.rect(10, 4, 2, 2, 2); m.set(13, 2, 4).set(12, 2, 4).set(13, 3, 4); } },
  gauntlet: { mat: { 1: 'steel', 2: 'iron', 3: 'leather' }, grip: [8, 8], draw(m) { m.rect(5, 9, 7, 6, 3); m.rect(4, 5, 8, 5, 1); for (const x of [4, 6, 8, 10]) m.rect(x, 2, 2, 4, 1); m.rect(11, 6, 3, 2, 1); m.rect(4, 9, 8, 1, 2); m.rect(5, 12, 7, 1, 2); } },
  bracers: { mat: { 1: 'leather', 2: 'iron', 3: 'steel' }, grip: [8, 8], draw(m) { for (const x of [1, 9]) { m.rect(x, 3, 6, 10, 1); m.rect(x, 3, 6, 1, 2); m.rect(x, 12, 6, 1, 2); m.rect(x + 2, 6, 2, 4, 3); } } },
  cloak: { mat: { 1: 'red', 2: 'gold', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[4, 2], [12, 2], [14, 8], [15, 15], [1, 15], [2, 8]], 1); m.rect(4, 2, 8, 2, 3); m.rect(7, 2, 2, 2, 2); m.line(8, 5, 8, 14, 3); } },
  robe: { mat: { 1: 'purple', 2: 'gold', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[5, 1], [11, 1], [12, 5], [15, 11], [13, 12], [12, 15], [4, 15], [3, 12], [1, 11], [4, 5]], 1); m.rect(7, 1, 2, 14, 2); m.rect(4, 8, 8, 2, 3); } },
  belt: { mat: { 1: 'leather', 2: 'gold', 3: 'iron' }, grip: [8, 8], draw(m) { m.rect(1, 6, 14, 4, 1); m.rect(1, 6, 14, 1, 3); m.rect(6, 5, 4, 6, 2); m.rect(7, 7, 2, 2, 0); m.rect(12, 10, 2, 4, 1); } },
  lantern: { mat: { 1: 'iron', 2: 'glass', 3: 'iron', 4: 'fire' }, fixed: [4], grip: [8, 3], draw(m) { m.ellipse(8, 2, 3, 2, 3); m.rect(4, 4, 8, 1, 1).rect(4, 13, 8, 2, 1); m.rect(4, 5, 1, 8, 1).rect(11, 5, 1, 8, 1); m.rect(5, 5, 6, 8, 2); m.ellipse(8, 9, 1.8, 2.5, 4); } },
  waterskin: { mat: { 1: 'leather', 2: 'wood', 3: 'leather' }, grip: [8, 8], draw(m) { m.rect(7, 1, 3, 3, 2); m.rect(6, 3, 5, 2, 3); m.ellipse(8.5, 10, 5, 5, 1); m.line(3, 6, 6, 3, 3); m.rect(4, 11, 9, 1, 3); } },
  sack: { mat: { 1: 'sand', 2: 'leather', 3: 'leather' }, grip: [8, 8], draw(m) { m.ellipse(8, 10, 6, 5, 1); m.rect(5, 2, 6, 4, 1); m.rect(4, 5, 8, 2, 3); m.line(5, 1, 4, 3, 1).line(11, 1, 12, 3, 1); } },
  map: { mat: { 1: 'bone', 2: 'red', 3: 'wood' }, grip: [8, 8], draw(m) { m.poly([[2, 3], [6, 2], [10, 3], [14, 2], [14, 13], [10, 14], [6, 13], [2, 14]], 1); m.line(4, 10, 7, 7, 2).line(7, 7, 10, 9, 2).line(10, 9, 12, 5, 2); m.set(12, 4, 4).set(11, 5, 4).set(13, 5, 4).set(12, 6, 4); m.rect(2, 2, 1, 13, 3); } },
  bread: { mat: { 1: 'sand', 2: 'wood', 3: 'wood' }, grip: [8, 8], draw(m) { m.ellipse(8, 9, 7, 4.5, 1); m.rect(3, 9, 10, 2, 1); for (const x of [5, 8, 11]) m.line(x, 6, x - 1, 9, 2); } },
  cheese: { mat: { 1: 'gold', 2: 'sand', 3: 'wood' }, grip: [8, 8], draw(m) { m.poly([[1, 12], [14, 5], [14, 12]], 1); m.rect(1, 12, 14, 2, 2); m.set(10, 9, 0).set(12, 10, 0).set(7, 11, 0).set(11, 7, 0); } },
  // CoffeeMUD remaining gaps: thrown and cord weapons, blades, firearms and siege, clothing, pills and powders, bags, cards, instruments, household goods and food.
  javelin: { mat: { 1: 'steel', 2: 'red', 3: 'wood' }, grip: [5, 11], draw(m) { m.line(2, 14, 11, 5, 3).line(3, 14, 12, 5, 3); m.poly([[11, 5], [13, 1], [14, 2], [13, 6]], 1); m.poly([[1, 12], [2, 10], [5, 13]], 2).poly([[3, 15], [5, 13], [6, 15]], 2); } },
  sling: { mat: { 1: 'leather', 2: 'sand', 3: 'wood' }, grip: [8, 8], draw(m) { m.ellipse(8, 11, 3.5, 2.5, 2); m.line(5, 10, 3, 2, 1).line(11, 10, 13, 2, 1); m.set(3, 1, 3).set(13, 1, 3); m.ellipse(8, 11, 1.2, 1.2, 3); } },
  net: { mat: { 1: 'sand', 2: 'leather', 3: 'wood' }, grip: [8, 8], draw(m) { for (const v of [2, 5, 8, 11, 13]) { m.rect(v, 2, 1, 12, 1); m.rect(2, v, 12, 1, 1); } m.rect(1, 1, 2, 2, 2).rect(13, 1, 2, 2, 2).rect(1, 13, 2, 2, 2).rect(13, 13, 2, 2, 2); } },
  lasso: { mat: { 1: 'sand', 2: 'leather', 3: 'wood' }, grip: [8, 8], draw(m) { m.ellipse(8, 6, 6, 4.5, 1); m.ellipse(8, 6, 4, 2.6, 0); m.rect(7, 10, 3, 2, 2); m.line(8, 12, 5, 14, 1).line(9, 12, 12, 14, 1); } },
  scimitar: { mat: { 1: 'steel', 2: 'gold', 3: 'leather' }, grip: [3, 13], draw(m) { m.thick(12, 1, 13, 5, 1, 2); m.thick(13, 5, 11, 8, 1, 2); m.thick(11, 8, 8, 10, 1, 2); m.line(3, 9, 8, 14, 2); m.line(4, 12, 3, 13, 3); m.set(2, 14, 2); } },
  katana: { mat: { 1: 'steel', 2: 'iron', 3: 'leather' }, grip: [3, 13], draw(m) { m.line(14, 1, 12, 4, 1).line(13, 1, 11, 4, 1).thick(12, 4, 6, 10, 1, 2); m.rect(4, 9, 1, 4, 2).rect(3, 10, 4, 1, 2).rect(5, 10, 3, 3, 2); m.line(4, 12, 3, 13, 3).set(2, 14, 3); } },
  arquebus: { mat: { 1: 'iron', 2: 'wood', 3: 'steel' }, grip: [4, 11], draw(m) { m.thick(5, 6, 13, 3, 1, 2); m.poly([[1, 9], [6, 6], [8, 8], [4, 13], [1, 12]], 2); m.rect(6, 8, 2, 3, 3); m.set(13, 2, 3); } },
  cannon: { mat: { 1: 'iron', 2: 'wood', 3: 'steel' }, grip: [8, 8], draw(m) { m.thick(3, 5, 13, 3, 1, 4); m.ellipse(3, 6, 2.5, 2.5, 1); m.ellipse(5, 13, 2.5, 2.5, 2).ellipse(11, 13, 2.5, 2.5, 2); m.rect(4, 8, 8, 3, 2); m.set(13, 3, 3); } },
  dress: { mat: { 1: 'red', 2: 'gold', 3: 'bone' }, grip: [8, 8], draw(m) { m.rect(5, 1, 2, 3, 1).rect(9, 1, 2, 3, 1); m.rect(5, 3, 6, 4, 1); m.poly([[5, 7], [11, 7], [13, 14], [3, 14]], 1); m.rect(5, 7, 6, 1, 2); m.line(6, 14, 7, 8, 3).line(9, 14, 9, 8, 3); } },
  pants: { mat: { 1: 'leather', 2: 'iron', 3: 'wood' }, grip: [8, 8], draw(m) { m.rect(3, 2, 10, 3, 1); m.rect(3, 5, 4, 10, 1); m.rect(9, 5, 4, 10, 1); m.rect(7, 5, 2, 4, 1); m.rect(3, 2, 10, 1, 2); m.rect(7, 2, 2, 2, 2); } },
  shirt: { mat: { 1: 'white', 2: 'bone', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[5, 2], [11, 2], [15, 5], [13, 8], [12, 7], [12, 14], [4, 14], [4, 7], [3, 8], [1, 5]], 1); m.rect(6, 2, 4, 2, 2); m.rect(7, 3, 2, 5, 3); } },
  earring: { mat: { 1: 'gold', 2: 'gold', 3: 'gold' }, grip: [8, 8], draw(m) { m.ellipse(8, 5, 2.5, 2.5, 1); m.ellipse(8, 5, 1, 1, 0); m.rect(7, 8, 2, 1, 1); m.ellipse(8, 11, 2.5, 2.5, 4); } },
  pill: { mat: { 1: 'white', 2: 'red', 3: 'white' }, grip: [8, 8], draw(m) { m.thick(4, 11, 11, 4, 1, 3); m.thick(4, 11, 7, 8, 2, 3); m.ellipse(4.5, 11.5, 1.8, 1.8, 2); } },
  powder: { mat: { 1: 'bone', 2: 'glass', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[2, 14], [3, 9], [6, 7], [10, 7], [13, 9], [14, 14]], 1); m.rect(4, 4, 8, 3, 2); m.rect(3, 6, 10, 1, 3); m.set(5, 2, 1).set(8, 1, 1).set(10, 3, 1); } },
  bag: { mat: { 1: 'purple', 2: 'gold', 3: 'leather' }, grip: [8, 8], draw(m) { m.ellipse(8, 10, 6, 5, 1); m.poly([[5, 3], [11, 3], [10, 6], [6, 6]], 1); m.rect(5, 5, 6, 2, 3); m.rect(7, 9, 2, 2, 2); m.set(5, 2, 3).set(10, 2, 3); } },
  cards: { mat: { 1: 'white', 2: 'red', 3: 'blue' }, grip: [8, 8], draw(m) { m.rect(2, 3, 8, 11, 3); m.rect(5, 2, 8, 11, 1); m.rect(7, 6, 4, 4, 2); m.set(8, 5, 2).set(9, 11, 2); } },
  hole: { mat: { 1: 'leather', 2: 'purple', 3: 'iron' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 7, 6, 1); m.ellipse(8, 8, 5, 4, 2); m.ellipse(8, 8, 3, 2.4, 3); } },
  lute: { mat: { 1: 'wood', 2: 'bone', 3: 'leather' }, grip: [5, 11], draw(m) { m.ellipse(5.5, 10.5, 4.5, 4, 1); m.ellipse(5.5, 10.5, 1.4, 1.4, 3); m.thick(8, 8, 13, 3, 3, 2); m.rect(12, 1, 3, 3, 2); m.line(4, 11, 11, 4, 2); } },
  flute: { mat: { 1: 'wood', 2: 'gold', 3: 'iron' }, grip: [8, 8], draw(m) { m.thick(2, 13, 13, 2, 1, 2); for (const [x, y] of [[5, 10], [7, 8], [9, 6], [11, 4]]) m.set(x, y, 3); m.rect(1, 13, 2, 2, 2); } },
  drum: { mat: { 1: 'wood', 2: 'bone', 3: 'leather' }, grip: [8, 8], draw(m) { m.rect(3, 5, 10, 8, 1); m.ellipse(8, 5, 5, 2, 2); m.line(3, 6, 8, 12, 3).line(13, 6, 8, 12, 3).line(8, 6, 3, 12, 3).line(8, 6, 13, 12, 3); m.line(10, 2, 7, 4, 3); m.set(10, 1, 3); } },
  horn: { mat: { 1: 'bone', 2: 'gold', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[1, 8], [5, 7], [11, 3], [14, 3], [14, 12], [11, 12], [5, 9], [1, 10]], 1); m.rect(1, 7, 2, 4, 2); m.rect(10, 3, 2, 10, 3); } },
  backpack: { mat: { 1: 'leather', 2: 'iron', 3: 'sand' }, grip: [8, 8], draw(m) { m.rect(3, 4, 10, 10, 1); m.ellipse(8, 4, 5, 2.5, 1); m.rect(4, 8, 8, 4, 3); m.rect(4, 8, 8, 1, 2); m.rect(7, 9, 2, 2, 2); m.rect(2, 7, 1, 6, 2).rect(13, 7, 1, 6, 2); } },
  mirror: { mat: { 1: 'gold', 2: 'glass', 3: 'gold' }, grip: [8, 13], draw(m) { m.ellipse(8, 6, 5.5, 5.5, 1); m.ellipse(8, 6, 3.8, 3.8, 2); m.rect(7, 11, 2, 3, 3); m.rect(6, 13, 4, 1, 3); } },
  pipe: { mat: { 1: 'wood', 2: 'iron', 3: 'red' }, grip: [8, 8], draw(m) { m.rect(8, 5, 5, 5, 1); m.rect(9, 4, 3, 1, 2); m.thick(2, 10, 8, 12, 1, 2); m.set(10, 3, 3).set(11, 2, 3); m.set(1, 10, 2); } },
  apple: { mat: { 1: 'red', 2: 'wood', 3: 'green' }, grip: [8, 8], draw(m) { m.ellipse(5.5, 9, 4, 4.5, 1); m.ellipse(10.5, 9, 4, 4.5, 1); m.rect(7, 3, 1, 3, 2); m.poly([[8, 3], [12, 2], [11, 5]], 3); } },
  meat: { mat: { 1: 'red', 2: 'bone', 3: 'leather' }, grip: [8, 8], draw(m) { m.ellipse(8, 7, 6, 4.5, 1); m.ellipse(8, 7, 3, 2, 3); m.rect(7, 11, 2, 3, 2); m.rect(6, 13, 1, 2, 2).rect(9, 13, 1, 2, 2); } },
  trout: { mat: { 1: 'steel', 2: 'water', 3: 'iron' }, grip: [8, 8], draw(m) { m.ellipse(7, 8, 6, 3.5, 1); m.poly([[11, 8], [15, 4], [15, 12]], 2); m.set(3, 7, 3); m.rect(5, 9, 5, 1, 2); } },
  cigar: { mat: { 1: 'wood', 2: 'bone', 3: 'fire' }, grip: [8, 8], draw(m) { m.thick(2, 12, 12, 4, 1, 3); m.thick(11, 5, 13, 3, 2, 3); m.set(13, 2, 3); } },
  perfume: { mat: { 1: 'glass', 2: 'gold', 3: 'red' }, grip: [8, 8], draw(m) { m.rect(5, 6, 6, 8, 1); m.rect(6, 3, 4, 3, 2); m.rect(7, 1, 2, 2, 3); m.rect(6, 8, 4, 4, 3); } },
  scale: { mat: { 1: 'gold', 2: 'iron', 3: 'gold' }, grip: [8, 8], draw(m) { m.rect(7, 2, 2, 11, 1); m.rect(4, 13, 8, 2, 3); m.rect(2, 4, 12, 1, 1); m.line(2, 5, 2, 9, 2).line(2, 5, 5, 9, 2).line(13, 5, 10, 9, 2).line(13, 5, 13, 9, 2); m.rect(1, 9, 5, 1, 3).rect(9, 9, 5, 1, 3); } },
  apron: { mat: { 1: 'bone', 2: 'leather', 3: 'sand' }, grip: [8, 8], draw(m) { m.rect(5, 1, 6, 2, 3); m.rect(5, 3, 6, 4, 1); m.poly([[3, 7], [13, 7], [14, 15], [2, 15]], 1); m.rect(3, 7, 10, 1, 2); m.rect(6, 10, 4, 3, 3); m.rect(1, 7, 2, 1, 2).rect(13, 7, 2, 1, 2); } },
  ledger: { mat: { 1: 'leather', 2: 'bone', 3: 'gold' }, grip: [8, 8], draw(m) { m.rect(2, 2, 12, 12, 1); m.rect(4, 3, 9, 10, 2); m.rect(2, 2, 2, 12, 3); for (const y of [5, 7, 9, 11]) m.rect(5, y, 7, 1, 1); } },
  gem: { mat: { 1: 'glass', 2: 'glass', 3: 'glass' }, grip: [8, 8], draw(m) { m.poly([[8, 2], [13, 6], [8, 14], [3, 6]], 4); m.line(3, 6, 13, 6, 2); } },
  // Phase 4 (T10): a crossbow, five more shields and a ceremonial sword, a sheath and quiver, and the camping kit.
  crossbow: { mat: { 1: 'wood', 2: 'steel', 3: 'bone' }, grip: [4, 10], draw(m) { m.rect(1, 7, 12, 3, 1); m.rect(3, 10, 2, 4, 1); for (const [a, b] of [[[10, 1], [14, 4]], [[14, 4], [14, 12]], [[14, 12], [10, 15]]]) m.thick(a[0], a[1], b[0], b[1], 2, 2); m.line(10, 1, 10, 15, 3); m.line(2, 8, 11, 8, 2); } },
  kiteshield: { mat: { 1: 'blue', 2: 'steel', 3: 'gold' }, grip: [8, 8], draw(m) { m.poly([[2, 2], [8, 0], [14, 2], [12, 10], [8, 15], [4, 10]], 1); rim(m, 1, 2); m.rect(7, 2, 2, 11, 3); m.rect(4, 5, 8, 2, 3); } },
  towershield: { mat: { 1: 'wood', 2: 'iron', 3: 'steel' }, grip: [8, 8], draw(m) { m.rect(3, 0, 10, 15, 1); m.rect(3, 0, 10, 2, 3).rect(3, 13, 10, 2, 3); m.rect(3, 0, 1, 15, 2).rect(12, 0, 1, 15, 2); m.rect(7, 3, 2, 8, 2); m.set(5, 6, 3).set(10, 6, 3).set(5, 10, 3).set(10, 10, 3); } },
  roundshield: { mat: { 1: 'red', 2: 'iron', 3: 'bone' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 6.5, 6.5, 1); rim(m, 1, 2); m.ellipse(8, 8, 2, 2, 3); m.line(8, 2, 8, 5, 3).line(8, 11, 8, 14, 3).line(2, 8, 5, 8, 3).line(11, 8, 14, 8, 3); } },
  magicshield: { mat: { 1: 'purple', 2: 'gold', 3: 'glass' }, grip: [8, 8], draw(m) { m.poly([[8, 1], [14, 4], [14, 9], [8, 15], [2, 9], [2, 4]], 1); rim(m, 1, 2); m.poly([[8, 4], [11, 8], [8, 12], [5, 8]], 4); m.set(8, 7, 3); } },
  ceremonial: { mat: { 1: 'steel', 2: 'gold', 3: 'purple' }, grip: [3, 13], draw(m) { m.thick(13, 2, 5, 10, 1, 3); m.set(14, 1, 1); m.line(2, 7, 9, 14, 2).line(3, 7, 10, 14, 2); m.rect(2, 12, 2, 2, 4); m.line(4, 12, 3, 13, 3); m.set(1, 15, 2); m.set(2, 15, 2); } },
  sheath: { mat: { 1: 'leather', 2: 'gold', 3: 'iron' }, grip: [3, 13], draw(m) { m.thick(13, 2, 4, 11, 1, 3); m.line(13, 1, 14, 2, 2); m.rect(3, 10, 3, 3, 2); m.rect(7, 7, 1, 1, 3); m.set(10, 5, 3); } },
  quiver: { mat: { 1: 'leather', 2: 'white', 3: 'gold' }, grip: [8, 8], draw(m) { m.poly([[5, 6], [11, 4], [13, 14], [6, 15]], 1); m.line(6, 6, 7, 1, 2).line(8, 5, 9, 0, 2).line(10, 4, 11, 1, 2); m.rect(6, 8, 6, 1, 3); m.rect(7, 12, 6, 1, 3); } },
  tent: { mat: { 1: 'sand', 2: 'wood', 3: 'leather' }, grip: [8, 8], draw(m) { m.poly([[8, 2], [15, 14], [1, 14]], 1); m.poly([[8, 6], [11, 14], [5, 14]], 3); m.line(8, 1, 8, 6, 2); m.line(8, 2, 1, 14, 2); m.line(8, 2, 15, 14, 2); } },
  bedroll: { mat: { 1: 'red', 2: 'bone', 3: 'leather' }, grip: [8, 8], draw(m) { m.rect(2, 5, 12, 7, 1); m.ellipse(13, 8.5, 1.5, 3.5, 2); m.ellipse(2.5, 8.5, 1.5, 3.5, 1); m.rect(4, 5, 1, 7, 3); m.rect(10, 5, 1, 7, 3); } },
  rope: { mat: { 1: 'sand', 2: 'leather', 3: 'sand' }, grip: [8, 8], draw(m) { m.ellipse(8, 8, 6.5, 6, 1); m.ellipse(8, 8, 4.5, 4, 2); m.ellipse(8, 8, 2.5, 2, 0); m.line(12, 11, 15, 15, 3).line(13, 11, 15, 14, 3); } },
};
export const SHAPE_IDS = Object.keys(SHAPES);

/** Material cue (step 3 of the item workflow): a metal glint, wood grain, or cloth fold, on the part that is that material. */
function cue(mask, out, parts, r) {
  for (const [id, name] of Object.entries(parts)) {
    const rr = ramp(name);
    const cells = []; for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (mask.get(x, y) === +id) cells.push([x, y]);
    if (cells.length < 6) continue;
    if (/steel|iron|gold|glass|white/.test(name)) { const [x, y] = cells[Math.floor(cells.length * 0.25)]; out.set(x, y, C.white); }
    else if (/wood|leather|dirt/.test(name)) for (let i = 0; i < 2; i++) { const [x, y] = r.pick(cells); out.set(x, y, rr[0]); }
  }
}

/** Tier finish. Mundane gets wear, crafted a clean glint, enchanted a rune cluster and a spark, legendary gold trim and two sparks. */
function finish(out, mask, tier, r) {
  const inner = []; for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (mask.get(x, y)) inner.push([x, y]);
  const spark = (c) => { // a plus in the emptiest corner, at the end of the art that is least in the way
    for (const [x, y] of [[2, 2], [13, 2], [2, 13], [13, 13]]) if (![[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]].some(([a, b]) => out.get(x + a, y + b))) { out.set(x, y, c).set(x - 1, y, c).set(x + 1, y, c).set(x, y - 1, c).set(x, y + 1, c); return; }
  };
  if (tier === 'mundane') for (let i = 0; i < 3; i++) { const [x, y] = r.pick(inner); out.set(x, y, C.ash); }
  if (tier === 'crafted') { const [x, y] = inner[Math.floor(inner.length * 0.2)]; out.set(x, y, C.white); }
  if (tier === 'enchanted' || tier === 'legendary') {
    const accent = tier === 'enchanted' ? C.cyan : C.yellow;
    for (let i = 0; i < 4; i++) { const [x, y] = r.pick(inner); out.set(x, y, accent); }
    spark(tier === 'enchanted' ? C.ice : C.yellow);
    if (tier === 'legendary') { for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (out.get(x, y) === C.night) out.set(x, y, C.umber); spark(C.white); }
  }
}


/** The canvases an icon is drawn on: 16 is the original, 24 and 32 are drawn again from the same shapes (see `bigMask`). */
export const ICON_SIZES = [16, 24, 32];

/**
 * A mask the 16 pixel shape code draws on as if it were 16 by 16. Every call is laid out again at `size` (corners and curves
 * land on the finer grid, strokes keep their weight in the new pixels) rather than the finished picture being blown up.
 */
function bigMask(size) {
  const k = size / 16, px = new Pixels(size, size), q = (v) => Math.round(v * k);
  const stamp = (x, y, w, c) => { for (let j = 0; j < w; j++) for (let i = 0; i < w; i++) px.set(x + i, y + j, c); };
  const stroke = (x0, y0, x1, y1, c, t, nudge) => {
    const w = Math.max(1, Math.round(t * k)), o = nudge ? 0.5 : 0, h = w / 2;
    const ax = Math.round((x0 + 0.5 + o) * k - h), ay = Math.round((y0 + 0.5 + (t >= 3 ? 0.5 : 0)) * k - h), bx = Math.round((x1 + 0.5 + o) * k - h), by = Math.round((y1 + 0.5 + (t >= 3 ? 0.5 : 0)) * k - h);
    const dx = Math.abs(bx - ax), dy = -Math.abs(by - ay), sx = ax < bx ? 1 : -1, sy = ay < by ? 1 : -1;
    let x = ax, y = ay;
    for (let e = dx + dy; ; ) { stamp(x, y, w, c); if (x === bx && y === by) break; const e2 = 2 * e; if (e2 >= dy) { e += dy; x += sx; } if (e2 <= dx) { e += dx; y += sy; } }
  };
  const m = {
    k, w: size, h: size, px,
    get: (x, y) => px.get(x, y), set(x, y, c) { for (let j = q(y); j < q(y + 1); j++) for (let i = q(x); i < q(x + 1); i++) px.set(i, j, c); return m; },
    clone: () => px.clone(),
    rect(x, y, w, h, c) { for (let j = q(y); j < q(y + h); j++) for (let i = q(x); i < q(x + w); i++) px.set(i, j, c); return m; },
    line(x0, y0, x1, y1, c) { stroke(x0, y0, x1, y1, c, 1, false); return m; },
    thick(x0, y0, x1, y1, c, t = 2) { stroke(x0, y0, x1, y1, c, t, t >= 2); return m; },
    ellipse(cx, cy, rx, ry, c) { px.ellipse(cx * k, cy * k, rx * k, ry * k, c); return m; },
    poly(pts, c) { px.poly(pts.map(([x, y]) => [x * k, y * k]), c); return m; },
  };
  return m;
}

/** Shading with bands as wide as the canvas allows: the lit edge and the shadow edge are `band` pixels deep, with a dithered seam, so a 32 pixel shape has volume where a 16 pixel one has an edge. */
function shadeBig(mask, parts, band) {
  const out = new Pixels(mask.w, mask.h, mask.palette), run = (x, y, dx, dy) => { let n = 0; while (n <= band && mask.get(x + dx * (n + 1), y + dy * (n + 1)) === mask.get(x, y)) n++; return n; };
  for (let y = 0; y < mask.h; y++) for (let x = 0; x < mask.w; x++) {
    const id = mask.get(x, y);
    if (!id) continue;
    const r = parts[id];
    if (!r) throw new Error(`no ramp for part ${id}`);
    const lit = Math.min(run(x, y, -1, 0), run(x, y, 0, -1)), dark = Math.min(run(x, y, 1, 0), run(x, y, 0, 1)), seam = (x + y) % 2 === 0;
    let v = r[2];
    if (lit === 0 && dark > 0) v = r[3];
    else if (dark === 0 && lit > 0) v = r[0];
    else if (lit < band && dark >= band) v = lit === 0 ? r[3] : seam ? r[3] : r[2];
    else if (dark < band && lit >= band) v = dark === 1 ? r[0] : seam ? r[1] : r[2];
    else if (lit === 0 && dark === 0) v = r[2];
    else if (dark < lit && dark < band) v = r[1];
    else if (dark === 0) v = r[0];
    out.set(x, y, v);
  }
  return out.outline(C.night);
}

/** Material cue for a big icon: a glint streak and a point of light on metal, grain strokes on wood and leather (more of them, longer, on a bigger canvas). */
function cueBig(mask, out, parts, r, size) {
  const k = size / 16;
  for (const [id, name] of Object.entries(parts)) {
    const rr = ramp(name), cells = [];
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (mask.get(x, y) === +id) cells.push([x, y]);
    if (cells.length < 6 * k * k) continue;
    if (/steel|iron|gold|glass|white/.test(name)) {
      const [x, y] = cells[Math.floor(cells.length * 0.25)], len = Math.round(2 * k);
      for (let i = 0; i < len; i++) if (mask.get(x + i, y + i) === +id) out.set(x + i, y + i, i === 0 ? C.white : rr[3]);
      const [a, b] = cells[Math.floor(cells.length * 0.6)];
      out.set(a, b, rr[3]);
    } else if (/wood|leather|dirt/.test(name)) {
      for (let i = 0; i < Math.round(3 * k); i++) {
        const [x, y] = r.pick(cells), len = 1 + Math.round(k);
        for (let j = 0; j < len; j++) if (mask.get(x, y + j) === +id) out.set(x, y + j, j === len - 1 ? rr[1] : rr[0]);
      }
    }
  }
}

/** Tier finish for a big icon: the same marks as the 16 pixel one, drawn bigger. */
function finishBig(out, mask, tier, r, size) {
  const k = size / 16, inner = [];
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (mask.get(x, y)) inner.push([x, y]);
  const arm = Math.round(k), spark = (c) => {
    for (const [x, y] of [[2, 2], [13, 2], [2, 13], [13, 13]].map(([a, b]) => [Math.round((a + 0.5) * k), Math.round((b + 0.5) * k)])) {
      let free = true;
      for (let j = -arm - 1; j <= arm + 1; j++) for (let i = -arm - 1; i <= arm + 1; i++) if (out.get(x + i, y + j)) free = false;
      if (!free) continue;
      for (let d = -arm; d <= arm; d++) { out.set(x + d, y, c); out.set(x, y + d, c); }
      return;
    }
  };
  if (tier === 'mundane') for (let i = 0; i < Math.round(3 * k); i++) { const [x, y] = r.pick(inner); out.set(x, y, C.ash); if (k >= 2) out.set(x + 1, y, C.ash); }
  if (tier === 'crafted') { const [x, y] = inner[Math.floor(inner.length * 0.2)]; out.set(x, y, C.white); if (k >= 2) out.set(x + 1, y, C.white); }
  if (tier === 'enchanted' || tier === 'legendary') {
    const accent = tier === 'enchanted' ? C.cyan : C.yellow;
    for (let i = 0; i < Math.round(4 * k); i++) { const [x, y] = r.pick(inner); out.set(x, y, accent); }
    spark(tier === 'enchanted' ? C.ice : C.yellow);
    if (tier === 'legendary') { for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (out.get(x, y) === C.night) out.set(x, y, C.umber); spark(C.white); }
  }
}

/** Draw one item. `materials` overrides the shape's ramps by part id; `tint` recolours the part that carries colour (a potion's liquid, a book's cover). */
/** Every option explicit: the shape, tier, seed and materials filled in. Drawn again it gives the same pixels. */
export const resolveItem = ({ shape = 'sword', tier = 'mundane', seed = 'item', materials = {}, ...rest } = {}) => ({ shape, tier, seed, materials, ...rest });

export function drawItem({ shape = 'sword', tier = 'mundane', seed = 'item', materials = {}, tint, iconSize = 16 } = {}) {
  const def = SHAPES[shape]; if (!def) throw new Error(`unknown item shape "${shape}"`);
  if (!TIERS[tier]) throw new Error(`unknown tier "${tier}"`);
  if (!ICON_SIZES.includes(iconSize)) throw new Error(`iconSize must be 16, 24 or 32 (got ${iconSize})`);
  if (iconSize > S) return drawBigItem(def, { shape, tier, seed, materials, tint, iconSize });
  const r = rng(`${seed}:${shape}:${tier}`), mask = new Pixels(S, S);
  def.draw(mask);
  const names = { ...def.mat, ...(tint ? { 1: tint } : {}), ...materials, 4: materials[4] ?? (def.fixed?.includes(4) ? def.mat[4] : TIERS[tier].accent) };
  const out = shade(mask, Object.fromEntries(Object.entries(names).map(([k, v]) => [k, ramp(v)])));
  cue(mask, out, names, r);
  finish(out, mask, tier, r);
  return { id: `item.${shape}.${tier}`, frames: [out], fps: 0, loop: false, anchors: { grip: def.grip }, mask };
}

/** The 24 and 32 pixel icon: the shape's own strokes laid out again on the finer grid, shaded in wider bands, with a longer glint, more grain and a bigger tier mark. */
function drawBigItem(def, { shape, tier, seed, materials, tint, iconSize }) {
  const r = rng(`${seed}:${shape}:${tier}:${iconSize}`), m = bigMask(iconSize);
  def.draw(m);
  const mask = m.px, k = iconSize / S;
  const names = { ...def.mat, ...(tint ? { 1: tint } : {}), ...materials, 4: materials[4] ?? (def.fixed?.includes(4) ? def.mat[4] : TIERS[tier].accent) };
  const out = shadeBig(mask, Object.fromEntries(Object.entries(names).map(([id, v]) => [id, ramp(v)])), k >= 2 ? 2 : 1);
  cueBig(mask, out, names, r, iconSize);
  finishBig(out, mask, tier, r, iconSize);
  const grip = def.grip.map((v) => Math.round((v + 0.5) * k - 0.5));
  return { id: `item.${shape}.${tier}`, frames: [out], fps: 0, loop: false, anchors: { grip }, mask };
}
