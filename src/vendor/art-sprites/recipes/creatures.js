// Creatures (32x32) from one parametric spec. The face kit's creature method is the design rule: use animal anatomy for
// plausibility, then change ONE anatomical system at a time (eyes, jaw, horns, skin, ears) so every variant stays readable.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { idle, collapse, flash, clip } from '../anim.js';
import { composeFamily } from './families.js';
import { topFrames, TOP_PLAN, TOP_FACINGS, TOP_CANVASES } from './top-creature.js';
import { enemyClips, statusLayer, intentLayer, rankArt, enlarge, bossParts, RANKS, DIRS } from './enemy-clips.js';

const S = 32;
export const BODIES = ['blob', 'biped', 'skeleton', 'quad', 'winged', 'drake', 'bird', 'fish', 'snake', 'spider', 'floater', 'golem', 'horse', 'insect', 'ghost', 'elemental', 'fey', 'treant', 'griffon', 'naga', 'multiarm', 'centaur', 'merfolk', 'mimic', 'fungus', 'construct', 'swarm', 'aberration', 'flytrap', 'vine', 'vermin', 'flock', 'wisps'];
export const SYSTEMS = { eyes: ['white', 'yellow', 'red', 'cyan'], jaw: ['none', 'fangs', 'tusks'], horns: [0, 1, 2], skin: ['green', 'skin-red', 'skin-pale', 'purple', 'steel', 'wood'], ears: [false, true] };

export const CANVASES = [32, 48, 64, 96, 128, 192, 256];
const BASE = { body: 'biped', skin: 'green', belly: null, eyes: 'yellow', jaw: 'none', horns: 0, ears: false, membrane: 'purple', bone: 'bone', crown: false };
export const PRESETS = {
  slime: { body: 'blob', skin: 'green', eyes: 'white' },
  goblin: { body: 'biped', skin: 'green', ears: true, jaw: 'fangs' },
  ogre: { body: 'biped', skin: 'skin-medium', jaw: 'tusks', belly: 'skin-light' },
  human: { body: 'biped', skin: 'skin-medium', eyes: 'white' },
  ogreking: { body: 'biped', skin: 'skin-medium', jaw: 'tusks', belly: 'skin-light', crown: true },
  skeleton: { body: 'skeleton', skin: 'bone', eyes: 'red' },
  rat: { body: 'quad', skin: 'dirt', ears: true, jaw: 'fangs', eyes: 'red', scale: 0.85 },
  wolf: { body: 'quad', skin: 'iron', ears: true, jaw: 'fangs', eyes: 'yellow' },
  bat: { body: 'winged', skin: 'purple', ears: true, jaw: 'fangs', eyes: 'red', membrane: 'blue' },
  crow: { body: 'bird', skin: 'iron', eyes: 'yellow', scale: 0.85 },
  fish: { body: 'fish', skin: 'blue', belly: 'white', eyes: 'white', membrane: 'water' },
  snake: { body: 'snake', skin: 'green', belly: 'sand', eyes: 'yellow', jaw: 'fangs' },
  spider: { body: 'spider', skin: 'iron', eyes: 'red', jaw: 'fangs', bone: 'iron' },
  beholder: { body: 'floater', skin: 'purple', eyes: 'yellow', jaw: 'fangs' },
  golem: { body: 'golem', skin: 'stone', eyes: 'cyan' },
  bear: { body: 'quad', skin: 'wood', ears: true, jaw: 'fangs', eyes: 'yellow', scale: 1.15 },
  cat: { body: 'quad', skin: 'leather', ears: true, eyes: 'cyan', scale: 0.8 },
  // CoffeeMUD remaining gaps: mounts and hounds, insects, undead, elementals, mephits, fey, winged beasts, serpent folk, the ten dragon colours.
  horse: { body: 'horse', skin: 'wood', eyes: 'white', ears: false },
  mount: { body: 'horse', skin: 'wood', eyes: 'white', saddle: true },
  warhorse: { body: 'horse', skin: 'iron', eyes: 'white', scale: 1.1 },
  dog: { body: 'quad', skin: 'leather', ears: true, eyes: 'white', scale: 0.9 },
  ant: { body: 'insect', skin: 'iron', eyes: 'red', bone: 'iron' },
  beetle: { body: 'insect', skin: 'green', eyes: 'yellow', bone: 'steel', scale: 0.9 },
  zombie: { body: 'biped', skin: 'moss', eyes: 'red', jaw: 'fangs' },
  ghoul: { body: 'biped', skin: 'stone', eyes: 'yellow', jaw: 'fangs', ears: true },
  mummy: { body: 'biped', skin: 'bone', eyes: 'red', wrap: true },
  ghost: { body: 'ghost', skin: 'white', eyes: 'cyan' },
  wraith: { body: 'ghost', skin: 'iron', eyes: 'red' },
  lich: { body: 'skeleton', skin: 'bone', eyes: 'cyan', crown: true },
  vampire: { body: 'biped', skin: 'skin-pale', eyes: 'red', jaw: 'fangs', ears: true },
  shadow: { body: 'ghost', skin: 'iron', eyes: 'red' },
  fireelemental: { body: 'elemental', skin: 'fire', belly: 'white', eyes: 'white' },
  waterelemental: { body: 'elemental', skin: 'water', belly: 'ice', eyes: 'white' },
  airelemental: { body: 'elemental', skin: 'ice', belly: 'white', eyes: 'cyan' },
  earthelemental: { body: 'golem', skin: 'dirt', eyes: 'yellow' },
  mephit: { body: 'winged', skin: 'red', ears: true, horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'fire', scale: 0.8 },
  faerie: { body: 'fey', skin: 'skin-light', bone: 'sand', membrane: 'ice', eyes: 'cyan' },
  pixie: { body: 'fey', skin: 'green', bone: 'wood', membrane: 'green', eyes: 'yellow' },
  nymph: { body: 'fey', skin: 'skin-light', bone: 'blue', membrane: 'water', eyes: 'cyan', scale: 1.1 },
  dryad: { body: 'treant', skin: 'wood', belly: 'grass', eyes: 'yellow', scale: 0.85 },
  treant: { body: 'treant', skin: 'wood', belly: 'grass', eyes: 'yellow' },
  griffon: { body: 'griffon', skin: 'sand', membrane: 'leather', bone: 'gold', eyes: 'yellow' },
  naga: { body: 'naga', skin: 'green', membrane: 'gold', eyes: 'yellow', jaw: 'fangs' },
  slaad: { body: 'multiarm', skin: 'green', eyes: 'red', jaw: 'fangs', ears: true },
  marilith: { body: 'multiarm', skin: 'skin-red', eyes: 'yellow', horns: 2, jaw: 'fangs' },
  centaur: { body: 'centaur', skin: 'skin-medium', eyes: 'white' },
  merfolk: { body: 'merfolk', skin: 'skin-light', membrane: 'water', eyes: 'cyan' },
  'dragon-red': { body: 'drake', skin: 'red', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'red' },
  'dragon-blue': { body: 'drake', skin: 'blue', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'blue' },
  'dragon-green': { body: 'drake', skin: 'green', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'green' },
  'dragon-black': { body: 'drake', skin: 'iron', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'iron' },
  'dragon-white': { body: 'drake', skin: 'white', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'white' },
  'dragon-gold': { body: 'drake', skin: 'gold', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'gold' },
  'dragon-silver': { body: 'drake', skin: 'steel', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'steel' },
  'dragon-bronze': { body: 'drake', skin: 'leather', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'leather' },
  'dragon-copper': { body: 'drake', skin: 'dirt', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'dirt' },
  'dragon-brass': { body: 'drake', skin: 'sand', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'sand' },
  drake: { body: 'drake', skin: 'red', belly: 'sand', horns: 2, jaw: 'fangs', eyes: 'yellow', membrane: 'red' },
  // Families the first set lacked: a chest that bites, fungus, clockwork, a swarm, an aberration, and bandits with their gear.
  mimic: { body: 'mimic', skin: 'wood', belly: 'leather', eyes: 'yellow', jaw: 'fangs' },
  sporeling: { body: 'fungus', skin: 'skin-pale', belly: 'red', eyes: 'white' },
  automaton: { body: 'construct', skin: 'iron', eyes: 'cyan', bone: 'gold' },
  swarm: { body: 'swarm', skin: 'iron', eyes: 'red' },
  'rat-swarm': { body: 'vermin', skin: 'leather', eyes: 'red' },
  'bat-swarm': { body: 'flock', skin: 'purple', eyes: 'red' },
  'wisp-swarm': { body: 'wisps', skin: 'glass', eyes: 'white' },
  flytrap: { body: 'flytrap', skin: 'green', belly: 'skin-red', eyes: 'yellow', jaw: 'fangs' },
  'vine-horror': { body: 'vine', skin: 'moss', belly: 'green', eyes: 'yellow' },
  tentacled: { body: 'aberration', skin: 'purple', belly: 'skin-pale', eyes: 'yellow', jaw: 'fangs' },
  bandit: { body: 'biped', skin: 'skin-medium', eyes: 'white', gear: ['bandana', 'blade'] },
  'bandit-hooded': { body: 'biped', skin: 'skin-light', eyes: 'white', gear: ['hood', 'blade'], accent: 'leather' },
  'bandit-brute': { body: 'biped', skin: 'skin-deep', eyes: 'white', jaw: 'tusks', gear: ['bandana', 'club', 'buckler'], scale: 1.1 },
};
/**
 * CoffeeMUD's races as data: each names a body plan (a preset) and a size, plus any anatomy that sets it apart. `size` is a share of the
 * canvas (a halfling is smaller than a human, a giant larger), the size table Coupler holds, so one plan reads as many races. `draw({ race })`.
 */
export const SIZES = { tiny: 0.6, small: 0.8, medium: 1, large: 1.08, huge: 1.18 };
export const RACES = {
  human: { preset: 'human', size: 'medium' }, elf: { preset: 'human', size: 'medium', skin: 'skin-fair', ears: true, eyes: 'cyan' }, halfelf: { preset: 'human', size: 'medium', skin: 'skin-light', ears: true },
  dwarf: { preset: 'human', size: 'small', skin: 'skin-deep', belly: 'skin-deep' }, halfling: { preset: 'human', size: 'tiny', skin: 'skin-light' }, gnome: { preset: 'human', size: 'tiny', skin: 'skin-medium', ears: true },
  orc: { preset: 'ogre', size: 'medium', skin: 'green', belly: 'green' }, goblin: { preset: 'goblin', size: 'small' }, hobgoblin: { preset: 'goblin', size: 'medium', skin: 'skin-red' }, kobold: { preset: 'goblin', size: 'tiny', skin: 'skin-red', eyes: 'red' },
  drow: { preset: 'human', size: 'medium', skin: 'skin-blue', ears: true, eyes: 'red' }, duergar: { preset: 'human', size: 'small', skin: 'stone', eyes: 'red' }, svirfneblin: { preset: 'human', size: 'tiny', skin: 'dirt', ears: true },
  tiefling: { preset: 'human', size: 'medium', skin: 'skin-red', horns: 2 }, gnoll: { preset: 'goblin', size: 'large', skin: 'dirt', eyes: 'red' }, lizardman: { preset: 'goblin', size: 'medium', skin: 'green', ears: false, eyes: 'yellow' },
  minotaur: { preset: 'ogre', size: 'large', skin: 'wood', belly: 'wood', horns: 2 }, ogre: { preset: 'ogre', size: 'large' }, troll: { preset: 'ogre', size: 'large', skin: 'moss', belly: 'moss', ears: true },
  giant: { preset: 'ogre', size: 'huge', skin: 'skin-medium' }, tabaxi: { preset: 'human', size: 'medium', skin: 'leather', ears: true, eyes: 'cyan' }, centaur: { preset: 'centaur', size: 'large' },
  wemic: { preset: 'centaur', size: 'large', skin: 'sand', ears: true, eyes: 'yellow' }, merfolk: { preset: 'merfolk', size: 'medium' }, selkie: { preset: 'merfolk', size: 'medium', skin: 'steel', membrane: 'steel' },
};
const EYE = { white: C.white, yellow: C.yellow, red: C.red, cyan: C.cyan };

const mask = () => new Pixels(S, S);
const eye = (o, x, y, kind) => { o.rect(x, y, 2, 2, EYE[kind] ?? C.white).set(kind === 'white' ? x + 1 : x, y + 1, C.night); };
const horns = (m, n, cx, y, spread, len = 4) => { if (n >= 1) m.line(cx - spread, y, cx - spread - 2, y - len, 3).line(cx - spread + 1, y, cx - spread - 1, y - len, 3); if (n >= 2) m.line(cx + spread, y, cx + spread + 2, y - len, 3).line(cx + spread - 1, y, cx + spread + 1, y - len, 3); };

/** Each body draws a mask (part 1 skin, 2 underside or membrane, 3 bone: horn, claw, skull) and returns where its eyes and mouth go. */
const BODY = {
  blob(m, s) { m.ellipse(16, 22, 11, 8, 1); m.rect(5, 29, 22, 3, 0); m.ellipse(16, 25, 7, 3, 2); return { eyes: [[12, 18], [18, 18]], mouth: [16, 23] }; },
  biped(m, s) {
    m.ellipse(16, 9, 5.5, 5, 1); m.ellipse(16, 19.5, 5.5, 5.5, 1); m.ellipse(16, 20.5, 3, 3.5, 2);
    m.thick(10, 15, 7, 23, 1, 2); m.thick(21, 15, 24, 23, 1, 2); m.rect(12, 24, 3, 6, 1).rect(17, 24, 3, 6, 1); m.rect(11, 29, 4, 2, 1).rect(17, 29, 4, 2, 1);
    if (s.ears) { m.poly([[10, 9], [3, 6], [10, 12]], 1); m.poly([[22, 9], [29, 6], [22, 12]], 1); }
    horns(m, s.horns, 16, 5, 4);
    if (s.jaw === 'tusks') m.rect(13, 12, 1, 3, 3).rect(18, 12, 1, 3, 3);
    if (s.gear?.includes('hood')) { m.ellipse(16, 8.5, 6.5, 6, 4); m.poly([[9, 14], [23, 14], [25, 20], [7, 20]], 4); }
    if (s.gear?.includes('bandana')) { m.rect(11, 5, 11, 3, 4); m.rect(21, 6, 4, 2, 4); }
    if (s.gear?.includes('blade')) { m.thick(25, 22, 28, 11, 5, 1); m.rect(23, 21, 4, 1, 3); }
    if (s.gear?.includes('club')) m.thick(25, 22, 28, 11, 3, 3);
    if (s.gear?.includes('buckler')) m.ellipse(6, 21, 3.5, 3.5, 5);
    return { eyes: [[13, 8], [17, 8]], mouth: [16, 12] };
  },
  skeleton(m, s) {
    m.ellipse(16, 8, 5, 5, 3); m.rect(13, 12, 7, 3, 3); m.rect(15, 14, 2, 10, 3);
    for (const y of [16, 18, 20]) m.rect(11, y, 10, 1, 3);
    m.thick(10, 15, 7, 24, 3, 1); m.thick(22, 15, 25, 24, 3, 1); m.rect(12, 23, 8, 2, 3); m.thick(13, 25, 12, 30, 3, 2); m.thick(18, 25, 19, 30, 3, 2);
    return { eyes: null, mouth: null };
  },
  quad(m, s) {
    m.ellipse(18, 19, 9, 5, 1); m.ellipse(7, 18, 4.5, 4, 1); m.poly([[2, 17], [5, 15], [5, 21], [2, 21]], 1);
    m.thick(26, 17, 29, 13, 1, 1); m.rect(10, 22, 2, 6, 1).rect(14, 22, 2, 6, 1).rect(22, 22, 2, 6, 1).rect(26, 22, 2, 6, 1);
    if (s.ears) { m.poly([[7, 15], [8, 10], [11, 15]], 1); m.poly([[11, 15], [13, 10], [14, 16]], 1); }
    m.ellipse(18, 22, 6, 2, 2);
    return { eyes: [[5, 16]], mouth: [3, 20], small: true };
  },
  winged(m, s) {
    m.ellipse(16, 17, 3.5, 5, 1); m.ellipse(16, 10, 3.5, 3.5, 1);
    const wing = [[13, 13], [2, 5], [4, 12], [6, 11], [8, 17], [10, 16], [13, 19]];
    m.poly(wing, 2); m.poly(wing.map(([x, y]) => [32 - x, y]), 2);
    if (s.ears) { m.poly([[13, 9], [12, 3], [15, 7]], 1); m.poly([[19, 9], [20, 3], [17, 7]], 1); }
    horns(m, s.horns, 16, 8, 2, 3);
    return { eyes: [[14, 9], [17, 9]], mouth: [16, 12], small: true };
  },
  drake(m, s) {
    m.ellipse(18, 20, 8, 5, 1); m.poly([[10, 16], [7, 9], [11, 8], [14, 15]], 1); m.ellipse(7, 8, 4, 3, 1); m.poly([[1, 8], [4, 6], [4, 10]], 1);
    m.poly([[17, 16], [24, 3], [30, 9], [27, 12], [29, 16], [23, 16]], 2); m.poly([[25, 19], [31, 25], [25, 22]], 1);
    m.rect(14, 24, 3, 5, 1).rect(21, 24, 3, 5, 1); m.ellipse(18, 22, 6, 2.5, 2);
    horns(m, s.horns, 8, 5, 1, 2);
    return { eyes: [[5, 7]], mouth: [3, 10] };
  },
  bird(m, s) {
    m.ellipse(16, 20, 7, 5, 1); m.ellipse(23, 14, 3.5, 3.5, 1); m.poly([[26, 13], [31, 15], [26, 16]], 3);
    m.poly([[10, 18], [3, 12], [5, 22], [11, 23]], 2); m.poly([[9, 21], [1, 22], [3, 25], [10, 24]], 1);
    m.rect(15, 25, 1, 5, 3).rect(19, 25, 1, 5, 3).rect(13, 29, 4, 1, 3).rect(18, 29, 4, 1, 3); m.ellipse(18, 22, 4, 2, 2);
    return { eyes: [[22, 13]], mouth: null, small: true };
  },
  fish(m, s) {
    m.ellipse(15, 16, 11, 6, 1); m.poly([[24, 16], [31, 9], [31, 23]], 2); m.poly([[11, 10], [16, 5], [19, 10]], 2); m.poly([[13, 22], [16, 27], [19, 22]], 2);
    m.ellipse(14, 19, 8, 2.5, 2);
    return { eyes: [[6, 14]], mouth: [3, 18], small: true };
  },
  snake(m, s) {
    m.ellipse(8, 22, 7, 4, 1); m.ellipse(20, 26, 9, 3.5, 1); m.thick(12, 18, 14, 11, 1, 3); m.ellipse(14, 9, 4, 3, 1);
    m.ellipse(20, 27, 7, 1.5, 2); m.line(8, 7, 5, 6, 3);
    return { eyes: [[12, 8]], mouth: [12, 11], small: true };
  },
  spider(m, s) {
    m.ellipse(20, 17, 6, 5.5, 1); m.ellipse(11, 16, 4, 3.5, 1); m.ellipse(20, 18, 3, 3, 2);
    for (const [y, dy] of [[14, -8], [16, -4], [18, 3], [19, 8]]) { m.thick(10, y, 4, y + dy, 3, 1); m.thick(4, y + dy, 2, y + dy + 8, 3, 1); m.thick(22, y, 28, y + dy, 3, 1); m.thick(28, y + dy, 30, y + dy + 8, 3, 1); }
    return { eyes: [[8, 14], [11, 14]], mouth: [9, 18], small: true };
  },
  floater(m, s) {
    m.ellipse(16, 17, 10, 9, 1); m.ellipse(16, 21, 8, 4, 2);
    for (const [x, y] of [[8, 6], [13, 3], [19, 3], [24, 6]]) { m.line(x < 16 ? x + 3 : x - 3, 10, x, y, 1); m.ellipse(x, y - 1, 2, 2, 1); }
    return { eyes: [[15, 13], [8, 3], [13, 0], [19, 0], [24, 3]], mouth: [16, 22], small: true };
  },
  golem(m, s) {
    m.rect(11, 3, 10, 8, 1); m.rect(8, 11, 16, 11, 1); m.rect(11, 14, 10, 6, 2);
    m.rect(2, 11, 6, 14, 1).rect(24, 11, 6, 14, 1).rect(1, 24, 8, 5, 3).rect(23, 24, 8, 5, 3);
    m.rect(10, 22, 5, 8, 1).rect(17, 22, 5, 8, 1);
    return { eyes: [[12, 6], [17, 6]], mouth: null };
  },
  // CoffeeMUD remaining gaps: horse, insect, ghost, elemental, fey, treant, griffon, naga, a four-armed biped, centaur and merfolk.
  horse(m, s) {
    m.ellipse(19, 16, 8, 4, 1); m.poly([[11, 14], [8, 6], [12, 5], [15, 14]], 1); m.ellipse(6.5, 8, 3.5, 2.5, 1); m.poly([[8, 6], [9, 2], [11, 6]], 1);
    for (const x of [11, 15, 23, 27]) m.rect(x, 19, 2, 10, 1);
    m.thick(27, 13, 30, 22, 1, 1); m.line(11, 6, 14, 13, 3); m.ellipse(19, 18, 6, 1.5, 2);
    return { eyes: [[6, 7]], mouth: null, small: true };
  },
  insect(m, s) {
    m.ellipse(8, 14, 3.5, 3, 1); m.ellipse(15, 15, 3, 2.5, 1); m.ellipse(24, 17, 5, 4, 1); m.ellipse(24, 19, 4, 1.5, 2);
    m.line(14, 17, 10, 26, 3).line(15, 17, 15, 27, 3).line(17, 17, 21, 26, 3).line(20, 20, 25, 27, 3); m.line(6, 12, 3, 6, 3).line(9, 12, 11, 6, 3); m.set(5, 17, 3).set(6, 18, 3);
    return { eyes: [[7, 13]], mouth: null, small: true };
  },
  ghost(m, s) {
    m.ellipse(16, 10, 7, 7, 1); m.rect(9, 10, 14, 14, 1); for (let x = 9; x < 23; x += 3) m.rect(x, 24, 3, 3 + (((x - 9) / 3) % 2) * 3, 1);
    m.thick(9, 14, 4, 21, 1, 2); m.thick(22, 14, 27, 21, 1, 2); m.ellipse(16, 17, 4, 5, 2);
    return { eyes: [[12, 9], [17, 9]], mouth: [16, 14], small: true };
  },
  elemental(m, s) {
    m.ellipse(16, 26, 9, 3.5, 1); m.ellipse(16, 21, 7, 4, 1); m.ellipse(16, 15, 6, 5, 1); m.ellipse(16, 9, 4.5, 5, 1);
    m.thick(10, 15, 5, 10, 1, 2); m.thick(22, 15, 27, 10, 1, 2); m.ellipse(16, 18, 3, 5, 2);
    return { eyes: [[13, 8], [17, 8]], mouth: [16, 12], small: true };
  },
  fey(m, s) {
    m.ellipse(16, 9, 3.5, 3.5, 1); m.poly([[13, 12], [19, 12], [20, 21], [12, 21]], 1); m.rect(13, 21, 2, 6, 1).rect(17, 21, 2, 6, 1);
    const wing = [[13, 13], [3, 4], [2, 11], [6, 17], [13, 17]]; m.poly(wing, 2); m.poly(wing.map(([x, y]) => [32 - x, y]), 2);
    m.poly([[12, 8], [8, 6], [12, 10]], 1); m.poly([[20, 8], [24, 6], [20, 10]], 1); m.rect(13, 5, 7, 2, 3);
    return { eyes: [[14, 8], [17, 8]], mouth: [16, 11], small: true };
  },
  treant(m, s) {
    m.rect(11, 9, 10, 19, 1); m.poly([[7, 31], [11, 24], [21, 24], [25, 31]], 1); m.ellipse(16, 6, 10, 5.5, 2);
    m.thick(11, 14, 4, 10, 1, 2); m.thick(21, 14, 28, 10, 1, 2); m.line(4, 10, 2, 5, 3).line(28, 10, 30, 5, 3); m.line(4, 10, 5, 5, 3).line(28, 10, 27, 5, 3);
    return { eyes: [[13, 13], [18, 13]], mouth: [16, 19] };
  },
  griffon(m, s) {
    m.ellipse(19, 20, 8, 4.5, 1); m.poly([[11, 19], [9, 12], [12, 12], [15, 19]], 1); m.ellipse(8, 11, 3.5, 3, 1); m.poly([[5, 11], [1, 13], [5, 15]], 3);
    m.poly([[16, 18], [8, 3], [18, 6], [26, 16]], 2); m.rect(11, 24, 2, 5, 1).rect(23, 24, 2, 5, 1).rect(10, 28, 4, 2, 3).rect(22, 28, 4, 2, 3); m.thick(27, 19, 30, 25, 1, 1); m.poly([[8, 8], [9, 5], [11, 8]], 1);
    return { eyes: [[7, 10]], mouth: null, small: true };
  },
  naga(m, s) {
    m.ellipse(11, 27, 8, 3, 1); m.ellipse(22, 27, 8, 3, 1); m.ellipse(16, 22, 6, 2.5, 1); m.poly([[12, 11], [20, 11], [19, 21], [13, 21]], 1); m.ellipse(16, 6.5, 3.5, 3.5, 1);
    m.thick(12, 13, 8, 19, 1, 2); m.thick(20, 13, 24, 19, 1, 2); m.poly([[10, 7], [22, 7], [24, 13], [8, 13]], 2); m.ellipse(16, 28, 9, 1.2, 2);
    return { eyes: [[14, 6], [17, 6]], mouth: [16, 9], small: true };
  },
  multiarm(m, s) {
    BODY.biped(m, s); m.thick(10, 19, 5, 27, 1, 2); m.thick(21, 19, 26, 27, 1, 2);
    return { eyes: [[13, 8], [17, 8]], mouth: [16, 12] };
  },
  centaur(m, s) {
    m.ellipse(20, 20, 8, 4, 1); for (const x of [14, 18, 24, 28]) m.rect(x, 22, 2, 8, 1); m.thick(28, 17, 31, 25, 1, 1);
    m.poly([[6, 20], [6, 11], [12, 11], [15, 20]], 1); m.ellipse(9, 7, 3.5, 3.5, 1); m.thick(6, 13, 3, 20, 1, 2); m.thick(12, 13, 15, 19, 1, 2); m.ellipse(20, 22, 6, 1.5, 2);
    if (s.ears) { m.poly([[6, 6], [3, 3], [7, 8]], 1); }
    return { eyes: [[8, 6], [11, 6]], mouth: [9, 10], small: true };
  },
  merfolk(m, s) {
    m.poly([[12, 10], [20, 10], [19, 20], [13, 20]], 1); m.ellipse(16, 6, 3.5, 3.5, 1); m.thick(12, 12, 8, 18, 1, 2); m.thick(20, 12, 24, 18, 1, 2);
    m.ellipse(16, 22, 4.5, 4, 1); m.poly([[12, 25], [16, 22], [20, 25], [24, 31], [16, 28], [8, 31]], 2);
    return { eyes: [[14, 5], [17, 5]], mouth: [16, 9], small: true };
  },
  mimic(m, s) {
    m.rect(4, 16, 24, 13, 1); m.rect(4, 5, 24, 10, 2); m.rect(4, 15, 24, 2, 5); m.rect(4, 27, 24, 2, 5); m.rect(14, 12, 4, 6, 5);
    m.rect(6, 15, 20, 3, 6); for (let x = 6; x < 25; x += 4) { m.poly([[x, 15], [x + 3, 15], [x + 1.5, 19]], 3); m.poly([[x + 2, 18], [x + 5, 18], [x + 3.5, 14]], 3); }
    m.ellipse(16, 20, 5, 1.5, 4);
    return { eyes: [[10, 9], [20, 9]], mouth: null };
  },
  fungus(m, s) {
    m.rect(13, 16, 6, 13, 1); m.poly([[9, 31], [13, 25], [19, 25], [23, 31]], 1); m.ellipse(16, 12, 11, 7, 2); m.rect(6, 15, 20, 3, 6);
    for (const [x, y] of [[10, 9], [16, 6], [21, 10], [14, 12]]) m.ellipse(x, y, 1.6, 1.6, 3);
    return { eyes: [[13, 20], [17, 20]], mouth: null };
  },
  construct(m, s) {
    m.rect(9, 11, 14, 12, 1); m.rect(11, 3, 10, 8, 1); m.rect(12, 5, 8, 2, 4); m.ellipse(16, 17, 3.5, 3.5, 3); m.ellipse(16, 17, 1.5, 1.5, 6);
    m.rect(3, 11, 6, 3, 5); m.rect(23, 11, 6, 3, 5); m.rect(2, 14, 6, 7, 3); m.rect(24, 14, 6, 7, 3); m.rect(10, 23, 5, 7, 5); m.rect(17, 23, 5, 7, 5);
    return { eyes: [[13, 5], [17, 5]], mouth: null };
  },
  flytrap(m, s) {
    m.thick(16, 30, 16, 20, 1, 3); m.thick(16, 27, 6, 23, 1, 2); m.ellipse(5, 22, 3, 1.6, 1); m.thick(16, 25, 26, 21, 1, 2); m.ellipse(27, 20, 3, 1.6, 1);
    m.poly([[5, 3], [27, 3], [29, 11], [16, 14], [3, 11]], 2); m.poly([[3, 11], [16, 14], [29, 11], [26, 19], [16, 21], [6, 19]], 1);
    for (let x = 5; x < 27; x += 4) { m.poly([[x, 11], [x + 3, 11], [x + 1.5, 15]], 3); m.poly([[x + 2, 17], [x + 5, 17], [x + 3.5, 13]], 3); }
    return { eyes: [[11, 7], [20, 7]], mouth: null };
  },
  vine(m, s) {
    for (const [x, y] of [[5, 6], [11, 3], [20, 3], [27, 7], [3, 17], [29, 17]]) { m.thick(16, 28, x, y, 1, 2); m.ellipse(x, y, 2, 2, 2); m.set(x + 1, y + 2, 3); }
    m.ellipse(16, 25, 9, 5, 1); m.ellipse(16, 20, 5, 4, 2); m.thick(10, 29, 4, 31, 1, 2); m.thick(22, 29, 28, 31, 1, 2);
    return { eyes: [[14, 19], [18, 19]], mouth: null };
  },
  vermin(m, s) {
    for (const [x, y] of [[7, 21], [14, 15], [22, 19], [26, 12], [10, 28], [18, 26], [27, 27], [5, 10], [16, 8]]) { m.ellipse(x, y, 3, 1.8, 1); m.ellipse(x - 3, y, 1.5, 1.2, 1); m.set(x - 4, y - 2, 3); m.line(x + 3, y, x + 6, y + 1, 3); }
    return { eyes: null, mouth: null };
  },
  flock(m, s) {
    for (const [x, y] of [[7, 20], [14, 12], [22, 17], [26, 8], [10, 27], [19, 25], [27, 26], [5, 8], [16, 5]]) { m.poly([[x - 5, y - 3], [x - 1, y], [x, y - 1], [x + 1, y], [x + 5, y - 3], [x + 3, y + 2], [x, y + 1], [x - 3, y + 2]], 1); m.set(x, y - 2, 5); }
    return { eyes: null, mouth: null };
  },
  wisps(m, s) {
    for (const [x, y] of [[7, 21], [14, 14], [22, 19], [26, 11], [10, 27], [18, 25], [27, 27], [5, 9], [16, 7]]) { m.ellipse(x, y, 2.5, 3, 1); m.ellipse(x, y, 1.2, 1.5, 3); m.set(x, y + 4, 2); m.set(x - 1, y + 5, 2); }
    return { eyes: null, mouth: null };
  },
  swarm(m, s) {
    for (const [x, y] of [[6, 20], [11, 14], [17, 17], [23, 13], [27, 21], [9, 26], [15, 24], [21, 26], [25, 29], [13, 8], [19, 9], [5, 12], [28, 8], [16, 29], [3, 28]]) { m.ellipse(x, y, 2, 1.6, 1); m.set(x - 2, y + 1, 3); m.set(x + 2, y + 1, 3); }
    return { eyes: null, mouth: null };
  },
  aberration(m, s) {
    m.ellipse(16, 11, 10, 8, 1); m.ellipse(16, 13, 6, 4, 2);
    for (const [x0, x1] of [[8, 4], [12, 10], [16, 17], [20, 22], [24, 28], [14, 7]]) { m.thick(x0, 15, x1, 29, 1, 3); m.set(x1, 28, 2); m.set(x1 + 1, 25, 2); }
    m.rect(11, 17, 10, 2, 3);
    return { eyes: [[9, 8], [14, 6], [20, 8], [15, 11]], mouth: [16, 17], small: true };
  },
};

/** A small gold crown for a biped's head (rows 2 to 5), with its own outline so it reads against the sky as well as the skin. */
function crownArt() {
  const g = ramp('gold'), c = new Pixels(S, S);
  c.rect(12, 4, 9, 2, g[2]).set(12, 3, g[3]).set(16, 3, g[3]).set(20, 3, g[3]).set(16, 4, C.red).rect(12, 5, 9, 1, g[1]);
  return c.outline(C.night);
}

const VIEWS = ['front', 'top'];
const HORIZONTAL = ['quad', 'drake', 'bird', 'fish', 'snake', 'horse', 'insect', 'griffon', 'centaur', 'spider'];

/** The options of one creature, merged: base, preset, race, family (composed by seed, refused if a part is not the family's) and then the caller's own. */
function specOf(opts) {
  const race = opts.race ? RACES[opts.race] : null;
  if (opts.race && !race) throw new Error(`unknown race "${opts.race}"`);
  const { preset: racePreset, size, ...raceBody } = race ?? {};
  if (opts.preset !== undefined && !PRESETS[opts.preset]) throw new Error(`unknown preset "${opts.preset}"`);
  let given = { ...(PRESETS[opts.preset ?? racePreset] ?? {}), ...raceBody, ...opts };
  if (opts.family) given = composeFamily(opts.family, given);
  return { ...BASE, ...(race ? { scale: SIZES[size] === 1 ? undefined : SIZES[size] } : {}), ...given };
}

/** Every option explicit: the body plan, the preset's or race's fields and the size filled in. Drawn again it gives the same pixels. */
export function resolveCreature(opts = {}) {
  const { motion, ...rest } = opts;
  return specOf(rest);
}

/** The still shrunk toward the middle of its canvas, nearest neighbour: how a top-down creature goes down. */
function shrink(px, f) {
  const out = new Pixels(px.w, px.h, px.palette), c = px.w / 2;
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) out.set(x, y, px.get(Math.round(c + (x + 0.5 - c) / f - 0.5), Math.round(c + (y + 0.5 - c) / f - 0.5)));
  return out;
}

/**
 * Draw a creature. Beside the body (a preset, a race, or a `family` composed by `seed`): `view` front (32 by 32) or top (40 by 40 from above, with a `facing`),
 * `rank` normal, elite (gold edge and crest) or boss (a larger `canvas`, a taller crest, `parts` to move apart, from 2 to 5 with `segments`; a `canvas` up to 256), `statuses` and `intent` (transparent layers over it),
 * `direction` (the way a front creature lunges; a top-down one lunges along its facing) and `flash: false` for the no-flash setting.
 */
export function drawCreature(opts = {}) {
  const spec = specOf(opts), view = spec.view ?? 'front', rank = spec.rank ?? 'normal', seed = spec.seed ?? 'creature';
  if (!BODY[spec.body]) throw new Error(`unknown body "${spec.body}"`);
  if (!VIEWS.includes(view)) throw new Error(`unknown creature view "${view}"; known: ${VIEWS.join(', ')}`);
  if (!RANKS.includes(rank)) throw new Error(`unknown rank "${rank}"; known: ${RANKS.join(', ')}`);
  const top = view === 'top', sizes = top ? TOP_CANVASES : CANVASES, canvas = spec.canvas ?? (rank === 'boss' ? 64 : sizes[0]);
  if (!sizes.includes(canvas)) throw new Error(`unknown ${view} canvas "${canvas}"; known: ${sizes.join(', ')}`);
  const facing = spec.facing ?? 'south', direction = top ? facing : spec.direction ?? 'west';
  if (top && !TOP_FACINGS.includes(facing)) throw new Error(`unknown facing "${facing}"; known: ${TOP_FACINGS.join(', ')}`);
  if (!DIRS[direction]) throw new Error(`unknown direction "${direction}"; known: ${Object.keys(DIRS).join(', ')}`);
  let frames, art;
  if (top) {
    if (!TOP_PLAN[spec.body]) throw new Error(`body "${spec.body}" has no top-down plan`);
    frames = topFrames(spec, { facing, canvas }).map((f) => rankArt(f, rank)); art = frames[0];
  } else {
    art = drawFront(spec);
    if (canvas > S) art = enlarge(art, canvas, seed);
    art = rankArt(art, rank); frames = idle(art, spec.body === 'blob');
  }
  const dir = DIRS[direction], c = canvas / 2;
  const clips = {
    idle: clip('idle', frames, { fps: 3 }), hit: clip('hit', [spec.flash === false ? art.shift(-dir[0], -dir[1]) : flash(art), art.shift(2 * -dir[0], 2 * -dir[1])], { fps: 10, loop: false }),
    defeat: clip('defeat', top ? [0.8, 0.55, 0.3, 0.12].map((f) => shrink(art, f)) : collapse(art), { fps: 8, loop: false, events: [{ frame: 3, name: 'gone' }] }),
    ...enemyClips(art, { direction, seed, flash: spec.flash !== false }),
  };
  const out = {
    id: `creature.${opts.race ?? opts.preset ?? opts.family ?? spec.body}`, frames, fps: 3, loop: true, spec, projection: top ? 'top-down' : 'front',
    paletteGroups: { skin: spec.skin, ...(spec.belly ? { belly: spec.belly } : {}), bone: spec.bone }, clips,
  };
  if (top) out.anchors = { root: [c, c], shadow_center: [c, c], effect_origin: [c, c], head: [c + dir[0] * 11 * (canvas / 40), c + dir[1] * 11 * (canvas / 40)].map(Math.round) };
  const layers = {};
  if (spec.statuses?.length) layers.status = frames.map((f, i) => statusLayer(f, spec.statuses, { seed, phase: i }));
  if (spec.intent) layers.intent = [intentLayer(art, spec.intent)];
  if (Object.keys(layers).length) out.layers = layers;
  if (rank === 'boss') out.parts = bossParts(art, top ? facing === 'east' || facing === 'west' : HORIZONTAL.includes(spec.body), spec.segments);
  return out;
}

/** The 32 by 32 side drawing of a resolved spec: the body mask shaded in its ramps, then the face, crown and saddle. */
function drawFront(spec) {
  const m = mask(), at = BODY[spec.body](m, spec);
  const skin = ramp(spec.skin === 'bone' ? 'bone' : spec.skin);
  const under = spec.belly ? ramp(spec.belly) : [skin[1], skin[2], skin[3], skin[3]];
  const out = shade(m, { 1: skin, 2: spec.body === 'winged' || spec.body === 'drake' ? ramp(spec.membrane) : under, 3: ramp(spec.bone), 4: ramp(spec.accent ?? 'red'), 5: ramp('steel'), 6: [C.night, C.night, C.plum, C.plum] });
  if (at.eyes) at.eyes.forEach(([x, y]) => eye(out, x, y, spec.eyes));
  if (spec.body === 'skeleton') { out.rect(13, 7, 2, 3, C.night).rect(18, 7, 2, 3, C.night).set(16, 10, C.night); out.rect(13, 12, 7, 1, C.white); for (const x of [14, 16, 18]) out.set(x, 12, C.night); if (spec.eyes === 'cyan') out.set(14, 8, C.cyan).set(19, 8, C.cyan); }
  if (spec.wrap) for (let y = 6; y < 30; y += 3) for (let x = 0; x < S; x++) if (m.get(x, y) === 1) out.set(x, y, ramp('bone')[0]);
  if (at.mouth) {
    const [mx, my] = at.mouth;
    if (spec.jaw === 'fangs') out.set(mx - 1, my, C.white).set(mx + 1, my, C.white).set(mx - 1, my + 1, C.white).set(mx + 1, my + 1, C.white);
    if (spec.body === 'blob' || spec.jaw === 'none') out.rect(mx - 1, my, 3, 1, C.night);
  }
  if (spec.crown && (spec.body === 'biped' || spec.body === 'skeleton')) out.blit(crownArt());
  if (spec.saddle && spec.body === 'horse') { const sd = new Pixels(S, S).rect(17, 12, 7, 3, ramp('red')[2]).rect(17, 14, 7, 1, ramp('red')[0]).rect(19, 11, 3, 1, ramp('leather')[1]); out.blit(sd.outline(C.night)); }
  return spec.scale && spec.scale !== 1 ? rescale(out, spec.scale) : out;
}

/** Resample around the bottom centre by a factor, nearest neighbour (monsters of one preset in different sizes). */
function rescale(px, f) {
  const out = new Pixels(S, S, px.palette);
  for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) out.set(x, y, px.get(Math.round(16 + (x - 16) / f), Math.round(31 + (y - 31) / f)));
  return out;
}

/** The same creature with one anatomical system changed to a different value (the face kit's "one system at a time"). */
export function mutate(spec, system, seed = 'mutate') {
  const opts = SYSTEMS[system]; if (!opts) throw new Error(`unknown system "${system}"`);
  const cur = spec[system] ?? BASE[system], choices = opts.filter((v) => v !== cur);
  return { ...spec, [system]: rng(`${seed}:${system}`).pick(choices) };
}
