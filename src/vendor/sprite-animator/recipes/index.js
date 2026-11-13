// The recipe registry: every motion Animator can make, by id. A recipe is `(params) => Clip` (see clip.js); `walk`, `run` and `jump` are locomotion, and the rest are `<type>.<preset>` from the motion types (types.js). Add one here and a kit
// subject can point at it. `variants()` lists the params the tests, the golden hashes and `animate variants` exercise.
import { drawWalk, drawRun, drawJump } from './locomotion.js';
import { TYPE_RECIPES } from './types.js';

export const RECIPES = {
  walk: { describe: 'Walk loop: contact, passing, contact, passing (4 frames); stride, bounce, frames, fps, wobble', variants: () => [{}, { stride: 3 }, { bounce: 0 }, { frames: 8 }, { stride: 1, bounce: 2, frames: 6 }, { wobble: 1, seed: 'a' }], draw: drawWalk },
  run: { describe: 'Run loop: contact, flight, contact, flight (4 frames); stride, lean, rise, frames, fps, wobble', variants: () => [{}, { stride: 4, lean: 3 }, { rise: 3 }, { frames: 8 }, { frames: 6, rise: 1 }, { wobble: 1, seed: 'a' }], draw: drawRun },
  jump: { describe: 'Jump in place (one-shot, 6 frames): crouch, takeoff, apex, fall, land, recover; height, crouch, frames, fps, wobble', variants: () => [{}, { height: 3 }, { height: 8, crouch: 3 }, { frames: 8 }, { frames: 12, height: 6 }, { wobble: 1, seed: 'a' }], draw: drawJump },
  ...TYPE_RECIPES,
};
export const RECIPE_IDS = Object.keys(RECIPES);

/** Make a clip from a recipe, by id. */
export function animate(id, params = {}) {
  const r = RECIPES[id];
  if (!r) throw new Error(`unknown recipe "${id}"; known: ${RECIPE_IDS.join(', ')}`);
  return r.draw(params);
}
