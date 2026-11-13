// Enemy families (T13): a family names the parts a creature of it may have, and `creature({ family, seed })` composes one from them by seed. A part a
// family does not allow (a fish among the undead, a dragon with a wolf's ears) is refused, so a generated enemy always has a valid silhouette.
import { rng } from '../rng.js';

const SKIN = ['skin-light', 'skin-medium', 'skin-deep', 'skin-fair'];
export const FAMILIES = {
  humanoid: { bodies: ['biped'], skin: SKIN, eyes: ['white'], jaw: ['none'], horns: [0], ears: [false, true] },
  bandit: { bodies: ['biped'], skin: SKIN, eyes: ['white', 'yellow'], jaw: ['none'], horns: [0], ears: [false], gear: [['bandana', 'blade'], ['hood', 'blade'], ['hood', 'club'], ['bandana', 'buckler', 'blade']] },
  goblinoid: { bodies: ['biped'], skin: ['green', 'moss', 'skin-red', 'stone'], eyes: ['yellow', 'red'], jaw: ['fangs', 'tusks'], horns: [0, 1], ears: [true] },
  undead: { bodies: ['skeleton', 'biped', 'ghost'], skin: ['bone', 'moss', 'stone', 'white'], eyes: ['red', 'cyan', 'yellow'], jaw: ['none', 'fangs'], horns: [0], ears: [false, true] },
  beast: { bodies: ['quad', 'bird', 'winged', 'horse', 'snake', 'fish'], skin: ['iron', 'wood', 'leather', 'dirt', 'sand'], eyes: ['yellow', 'red', 'white'], jaw: ['none', 'fangs'], horns: [0], ears: [false, true] },
  insect: { bodies: ['insect', 'spider'], skin: ['iron', 'green', 'dirt'], eyes: ['red', 'yellow'], jaw: ['fangs', 'none'], horns: [0], ears: [false] },
  slime: { bodies: ['blob'], skin: ['green', 'blue', 'purple', 'skin-red'], eyes: ['white'], jaw: ['none'], horns: [0], ears: [false] },
  elemental: { bodies: ['elemental', 'golem'], skin: ['fire', 'water', 'ice', 'dirt'], eyes: ['white', 'cyan'], jaw: ['none'], horns: [0], ears: [false] },
  demon: { bodies: ['biped', 'winged', 'multiarm'], skin: ['red', 'skin-red', 'purple', 'iron'], eyes: ['yellow', 'red'], jaw: ['fangs', 'tusks'], horns: [1, 2], ears: [true] },
  dragon: { bodies: ['drake'], skin: ['red', 'blue', 'green', 'iron', 'white', 'gold'], eyes: ['yellow'], jaw: ['fangs'], horns: [2], ears: [false] },
  spirit: { bodies: ['ghost', 'fey', 'floater'], skin: ['white', 'ice', 'iron', 'skin-pale'], eyes: ['cyan', 'white', 'red'], jaw: ['none'], horns: [0], ears: [false] },
  golem: { bodies: ['golem', 'construct'], skin: ['stone', 'iron', 'dirt', 'wood'], eyes: ['cyan', 'yellow', 'red'], jaw: ['none'], horns: [0], ears: [false] },
  mimic: { bodies: ['mimic'], skin: ['wood', 'leather'], eyes: ['yellow', 'red'], jaw: ['fangs'], horns: [0], ears: [false] },
  fungus: { bodies: ['fungus'], skin: ['skin-pale', 'moss', 'bone'], eyes: ['white', 'yellow'], jaw: ['none'], horns: [0], ears: [false] },
  construct: { bodies: ['construct'], skin: ['iron', 'steel', 'stone'], eyes: ['cyan', 'yellow', 'red'], jaw: ['none'], horns: [0], ears: [false] },
  swarm: { bodies: ['swarm', 'vermin', 'flock', 'wisps'], skin: ['iron', 'green', 'dirt', 'purple', 'leather', 'glass'], eyes: ['red', 'yellow', 'white'], jaw: ['none'], horns: [0], ears: [false] },
  plant: { bodies: ['treant', 'flytrap', 'vine'], skin: ['moss', 'green', 'wood', 'grass'], eyes: ['yellow', 'white', 'red'], jaw: ['none', 'fangs'], horns: [0], ears: [false] },
  aberration: { bodies: ['aberration', 'floater'], skin: ['purple', 'skin-pale', 'moss'], eyes: ['yellow', 'red', 'cyan'], jaw: ['fangs', 'none'], horns: [0], ears: [false] },
};
export const FAMILY_IDS = Object.keys(FAMILIES);
const FIELDS = { body: 'bodies', skin: 'skin', eyes: 'eyes', jaw: 'jaw', horns: 'horns', ears: 'ears' };

/**
 * The options of a creature of `family`, composed by `seed`: one allowed value for each part, then whatever `opts` names on top. A named value the family does not
 * allow throws, so nonsense anatomy never draws.
 */
export function composeFamily(family, opts = {}) {
  const fam = FAMILIES[family]; if (!fam) throw new Error(`unknown family "${family}"; known: ${FAMILY_IDS.join(', ')}`);
  const r = rng(`${opts.seed ?? family}:${family}`), out = {};
  for (const [field, list] of Object.entries(FIELDS)) {
    const given = opts[field];
    if (given !== undefined && !fam[list].includes(given)) throw new Error(`${JSON.stringify(given)} is not a ${field} of the ${family} family; allowed: ${fam[list].join(', ')}`);
    out[field] = given ?? r.pick(fam[list]);
  }
  if (fam.gear) out.gear = opts.gear ?? r.pick(fam.gear);
  return { ...opts, ...out, family };
}
