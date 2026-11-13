// Clan sigils composed from parts (CoffeeMUD clan items): a shield, a field colour, an ordinary across it and a charge on it. The parts
// are data, so a clan's look is three words (`{ field: 'blue', ordinary: 'bend', charge: 'star' }`) and the same words draw the same sigil.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';

const S = 16;
export const SIGIL_ORDINARIES = ['none', 'pale', 'fess', 'bend', 'chevron', 'cross', 'saltire'];
export const SIGIL_CHARGES = ['none', 'star', 'moon', 'crown', 'tower', 'sword', 'flame'];
const SHAPE = (m) => m.poly([[2, 1], [13, 1], [13, 8], [8, 15], [2, 8]], 1);
const ORDINARY = {
  none: () => {},
  pale: (m) => m.rect(6, 1, 4, 14, 2),
  fess: (m) => m.rect(1, 5, 14, 3, 2),
  bend: (m) => m.thick(1, 1, 13, 13, 2, 3),
  chevron: (m) => m.thick(1, 9, 8, 3, 2, 2).thick(8, 3, 14, 9, 2, 2),
  cross: (m) => m.rect(6, 1, 3, 14, 2).rect(1, 5, 14, 3, 2),
  saltire: (m) => m.thick(1, 1, 13, 13, 2, 2).thick(13, 1, 1, 13, 2, 2),
};
const CHARGE = {
  none: [],
  star: ['..X..', '.XXX.', 'XXXXX', '.XXX.', 'X...X'],
  moon: ['.XXX.', 'XX...', 'X....', 'XX...', '.XXX.'],
  crown: ['X.X.X', 'XXXXX', 'XXXXX', 'XXXXX', '.....'],
  tower: ['X.X.X', 'XXXXX', '.XXX.', '.XXX.', 'XXXXX'],
  sword: ['..X..', '..X..', '..X..', 'XXXXX', '..X..'],
  flame: ['..X..', '.XXX.', 'XXXXX', 'XXXXX', '.XXX.'],
};

/** One sigil (16x16). `field` and `color` are ramp names (palette.js): the shield and the ordinary; the charge takes `charge_color`. */
export function drawSigil({ field = 'blue', ordinary = 'none', charge = 'star', color = 'white', chargeColor = 'gold' } = {}) {
  if (!ORDINARY[ordinary]) throw new Error(`unknown ordinary "${ordinary}"; known: ${SIGIL_ORDINARIES.join(', ')}`);
  if (!CHARGE[charge]) throw new Error(`unknown charge "${charge}"; known: ${SIGIL_CHARGES.join(', ')}`);
  const shield = SHAPE(new Pixels(S, S)), band = new Pixels(S, S); ORDINARY[ordinary](band);
  const m = new Pixels(S, S);
  for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (shield.get(x, y)) m.set(x, y, band.get(x, y) ? 2 : 1);
  CHARGE[charge].forEach((row, j) => [...row].forEach((ch, i) => { if (ch === 'X' && m.get(5 + i, 4 + j)) m.set(5 + i, 4 + j, 3); }));
  const out = shade(m, { 1: ramp(field), 2: ramp(color), 3: ramp(chargeColor) });
  return { id: `sigil.${field}.${ordinary}.${charge}`, frames: [out], fps: 0, loop: false, mask: m };
}
