// A class emblem (T9): the sigil shield in the loadout's cloth colour with the icon glyph of its weapon on it, for the portrait frame and the party panel.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';
import { LOADOUTS } from './characters.js';
import { drawSigil } from './heraldry.js';
import { GLYPHS } from './icons.js';

const GLYPH_OF = { sword: 'sword', greatsword: 'sword', rapier: 'sword', scimitar: 'sword', axe: 'axe', dagger: 'dagger', hammer: 'hammer', mace: 'cross', staff: 'spark', wand: 'star', bow: 'target', book: 'book', ledger: 'book', scale: 'coin' };
export const EMBLEM_LOADOUTS = Object.keys(LOADOUTS);

/** The emblem (16x16) of a `loadout`; `glyph` overrides the symbol (any icon glyph). The shield is the loadout's cloth ramp, the symbol white with a gold accent. */
export function drawEmblem({ loadout = 'guardian', glyph } = {}) {
  const l = LOADOUTS[loadout]; if (!l) throw new Error(`unknown loadout "${loadout}"; known: ${EMBLEM_LOADOUTS.join(', ')}`);
  const g = glyph ?? GLYPH_OF[l.weapon] ?? 'shield'; if (!GLYPHS[g]) throw new Error(`unknown glyph "${g}"`);
  const out = drawSigil({ field: l.cloth, ordinary: 'none', charge: 'none' }).frames[0].clone();
  const mask = Pixels.fromRows(GLYPHS[g], { X: 1, o: 2 });
  out.blit(shade(new Pixels(12, 12).blit(mask, 1, 1), { 1: ramp('white'), 2: ramp('gold') }, { outline: ramp(l.cloth)[0] }), 2, 1);
  return { id: `emblem.${loadout}`, frames: [out], fps: 0, loop: false, spec: { loadout, glyph: g } };
}
