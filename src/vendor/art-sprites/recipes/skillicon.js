// A skill icon (T12): one grammar for every class skill, made from a category, a motif, a target, a tier and a few state markers, on a 24 by 24 canvas.
// The plate (colour and shape) says the category; the motif is a glyph from the approved list (`icon`); the right column says who it targets and how it
// fires; the bottom row counts its tier and its cost. A locked skill is darkened with a padlock; a cooldown is a dithered sweep over the plate. Colour is
// never the only cue: targets and kinds are different shapes, locked and cooling skills are dithered.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { clip } from '../anim.js';
import { drawIcon } from './icons.js';
import { MARKS } from './overlay.js';

const W = 24;
/** Category: the plate's colour and shape, and the motif a skill of that category gets when it names none. */
export const SKILL_CATEGORIES = {
  attack: { plate: 'red', shape: 'square', motif: 'sword' }, defence: { plate: 'blue', shape: 'shield', motif: 'shield' }, mobility: { plate: 'green', shape: 'round', motif: 'chevrons' },
  control: { plate: 'purple', shape: 'octagon', motif: 'stun' }, heal: { plate: 'white', shape: 'round', motif: 'cross' }, cleanse: { plate: 'glass', shape: 'octagon', motif: 'drop' },
  buff: { plate: 'gold', shape: 'square', motif: 'arrowup' }, debuff: { plate: 'stone', shape: 'shield', motif: 'arrowdown' }, summon: { plate: 'purple', shape: 'round', motif: 'spark' },
  utility: { plate: 'steel', shape: 'square', motif: 'key' },
};
export const SKILL_CATEGORY_IDS = Object.keys(SKILL_CATEGORIES);
/** The motifs a skill may use: approved glyphs only, so a generated skill cannot show something the game never meant. */
export const SKILL_MOTIFS = ['sword', 'axe', 'dagger', 'shield', 'book', 'cross', 'spark', 'bolt', 'eye', 'crown', 'skull', 'flame', 'drop', 'clover', 'star', 'key', 'moon', 'potion', 'brace', 'mend', 'rampage', 'freeze', 'stun', 'poison', 'disease', 'bleed', 'arrowup', 'arrowdown', 'mana', 'sleep', 'blind', 'silence', 'slow', 'chevrons', 'target', 'burst', 'swing', 'hammer', 'heart', 'curse'];

const PLATE_MASKS = {
  square: (m) => { m.rect(0, 1, 16, 14, 1).rect(1, 0, 14, 16, 1); },
  round: (m) => { m.ellipse(8, 8, 8, 8, 1); },
  shield: (m) => { m.poly([[0, 0], [16, 0], [16, 9], [8, 16], [0, 9]], 1); },
  octagon: (m) => { m.poly([[4, 0], [12, 0], [16, 4], [16, 12], [12, 16], [4, 16], [0, 12], [0, 4]], 1); },
};
const SHAPES5 = {
  self: ['..X..', '.XXX.', '..X..', '.X.X.', '.....'], ally: ['.X.X.', 'XXXXX', 'XXXXX', '.XXX.', '..X..'], enemy: ['..X..', '..X..', 'XX.XX', '..X..', '..X..'],
  party: ['.X.X.', 'XXXXX', '.....', '.XXX.', '.....'], area: ['.XXX.', 'X...X', 'X...X', 'X...X', '.XXX.'], line: ['.....', '...X.', 'XXXXX', '...X.', '.....'], cone: ['X....', 'XXX..', 'XXXXX', 'XXX..', 'X....'],
};
const KINDS5 = { active: ['.....', '.XXX.', '.XXX.', '.XXX.', '.....'], passive: ['XXXXX', 'X...X', 'X...X', 'X...X', 'XXXXX'], triggered: ['..XX.', '.XX..', 'XXXXX', '..XX.', '.XX..'] };
export const SKILL_TARGETS = Object.keys(SHAPES5), SKILL_KINDS = Object.keys(KINDS5);
const INK = { self: 'white', ally: 'green', enemy: 'red', party: 'blue', area: 'gold', line: 'steel', cone: 'fire', active: 'white', passive: 'steel', triggered: 'gold' };

const marker = (rows, ink) => shade(new Pixels(7, 7).blit(Pixels.fromRows(rows, { X: 1 }), 1, 1), { 1: ramp(ink) });

/** Dither the plate over the clockwise sweep from 12 o'clock: `c` of the way round (0 none, 1 all). */
function sweep(out, plate, c) {
  for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
    if (!plate.get(x, y)) continue;
    const a = (Math.atan2(x + 0.5 - 8, -(y + 0.5 - 8)) + 2 * Math.PI) % (2 * Math.PI);
    if (a / (2 * Math.PI) < c && (x + y) % 2 === 0) out.set(x, y, C.night);
  }
}

/** One skill icon. `category` and `motif` make the plate and the picture; `target` and `kind` the right column; `tier` (0 to 3) and `cost` (0 to 3) the bottom row; `locked` and `cooldown` (0 to 1) the state. */
export function drawSkillIcon({ category = 'attack', motif, target, kind = 'active', tier = 0, cost = 0, locked = false, cooldown = 0 } = {}) {
  const cat = SKILL_CATEGORIES[category]; if (!cat) throw new Error(`unknown skill category "${category}"; known: ${SKILL_CATEGORY_IDS.join(', ')}`);
  motif ??= cat.motif; if (!SKILL_MOTIFS.includes(motif)) throw new Error(`unknown skill motif "${motif}": a skill uses an approved glyph (${SKILL_MOTIFS.length} of them)`);
  if (target !== undefined && !SHAPES5[target]) throw new Error(`unknown skill target "${target}"; known: ${SKILL_TARGETS.join(', ')}`);
  if (!KINDS5[kind]) throw new Error(`unknown skill kind "${kind}"; known: ${SKILL_KINDS.join(', ')}`);
  if (!Number.isInteger(tier) || tier < 0 || tier > 3 || !Number.isInteger(cost) || cost < 0 || cost > 3) throw new Error('tier and cost are whole numbers from 0 to 3');
  if (!(cooldown >= 0 && cooldown <= 1)) throw new Error('cooldown is from 0 to 1');
  const make = (cd) => {
    const out = new Pixels(W, W), m = new Pixels(16, 16); PLATE_MASKS[cat.shape](m);
    const plate = shade(m, { 1: ramp(cat.plate) }), glyph = drawIcon({ glyph: motif, shape: 'bare', ink: cat.plate === 'white' || cat.plate === 'glass' ? 'blue' : 'white', accent: 'gold' }).frames[0];
    out.blit(plate).blit(glyph);
    if (locked) { sweep(out, m, 1); out.blit(shade(new Pixels(7, 7).blit(Pixels.fromRows(MARKS.locked.rows, { X: 1, o: 2 }), 1, 1), { 1: ramp('steel'), 2: ramp('iron') }), 5, 5); }
    else if (cd > 0) sweep(out, m, cd);
    if (target) out.blit(marker(SHAPES5[target], INK[target]), 17, 0);
    out.blit(marker(KINDS5[kind], INK[kind]), 17, 8);
    for (let i = 0; i < tier; i++) out.rect(1 + 4 * i, 20, 3, 1, ramp('gold')[3]).rect(2 + 4 * i, 19, 1, 3, ramp('gold')[3]);
    for (let i = 0; i < cost; i++) out.rect(14 + 3 * i, 19, 2, 2, ramp('blue')[3]);
    return out;
  };
  const still = make(cooldown), frames = [still];
  const sweepFrames = [1, 0.75, 0.5, 0.25, 0].map(make);
  return { id: `skillicon.${category}.${motif}`, frames, fps: 0, loop: false, spec: { category, motif, target, kind, tier, cost, locked, cooldown }, clips: { cooldown: clip('cooldown', sweepFrames, { fps: 4, loop: false, events: [{ frame: 4, name: 'ready' }], reduced: [sweepFrames[4]] }) } };
}
