// Frozen looks and dye. A look is a recipe and its params. Tag Team stores what a character looks like and must get the same pixels
// back later, but a new default in a later version would change a look stored as "rogue". `resolveLook` writes every default into the
// params, so the stored look says everything and only a change to a recipe's drawing (named in the CHANGELOG) can alter it.
import { ENGINE_VERSION } from './version.js';
import { resolveCharacter, DYE_PARAMS } from './recipes/characters.js';
import { resolveCreature } from './recipes/creatures.js';
import { resolveItem } from './recipes/items.js';
import { draw } from './recipes/index.js';

const RESOLVERS = { character: resolveCharacter, creature: resolveCreature, item: resolveItem };
/** The ramp groups a recipe can be dyed in, and the param each one sets. */
export const DYE_GROUPS = { character: DYE_PARAMS, creature: { skin: 'skin', belly: 'belly', bone: 'bone' } };

/**
 * `{ recipe, params, engine, explicit }`: the params with every default written in, JSON-safe (`motion` is left out: it is a movement,
 * not a look). `explicit` is true for the recipes that have a resolver (character, creature, item); for the others the params are
 * returned as given and a stored look is exact only for the `engine` version.
 */
export function resolveLook(recipe, params = {}) {
  const resolve = RESOLVERS[recipe];
  const { motion, ...rest } = params;
  return { recipe, params: JSON.parse(JSON.stringify(resolve ? resolve(rest) : rest)), engine: ENGINE_VERSION, explicit: !!resolve };
}

/** Draw a recipe with some colour groups set: `dye('character', { loadout: 'rogue' }, { cloth: 'blue', skin: 'skin-deep' })`. Draws again, so shading and outline are exact. */
export function dye(recipe, params = {}, groups = {}, options) {
  const names = DYE_GROUPS[recipe]; if (!names) throw new Error(`recipe "${recipe}" has no dye groups; known: ${Object.keys(DYE_GROUPS).join(', ')}`);
  const set = {};
  for (const [g, ramp] of Object.entries(groups)) { if (!names[g]) throw new Error(`unknown dye group "${g}" for ${recipe}; known: ${Object.keys(names).join(', ')}`); set[names[g]] = ramp; }
  return draw(recipe, { ...params, ...set }, options);
}
