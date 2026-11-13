// Pictogram icons (16x16): a small symbol on a coloured plate, for badges, emblems, skills, statuses and resources. The same
// rules as the item icons: one silhouette, the light from the top left, a closing outline, two tones at most (the symbol, and one
// accent). A glyph is at most 10x10 so the outline and the plate edge both fit; `X` is the symbol, `o` the accent.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

const S = 16;

/** Symbol masks, as text. `X` takes the ink ramp, `o` the accent ramp. */
export const GLYPHS = {
  sword: ['....XX....', '....XX....', '....XX....', '....XX....', '....XX....', '....XX....', '..oooooo..', '....oo....', '....oo....', '....oo....'],
  axe: ['..XXX.oo..', '.XXXXXoo..', 'XXXXXXoo..', 'XXXXXXoo..', '.XXXXXoo..', '..XXX.oo..', '......oo..', '......oo..', '......oo..', '......oo..'],
  dagger: ['....XX....', '....XX....', '....XX....', '....XX....', '...XXXX...', '..oooooo..', '....oo....', '....oo....', '....oo....', '...oooo...'],
  shield: ['XXXXXXXXXX', 'XXXXooXXXX', 'XXXXooXXXX', 'XooooooooX', 'XXXXooXXXX', 'XXXXooXXXX', '.XXXooXXX.', '..XXooXX..', '...XooX...', '....XX....'],
  book: ['.XXXXXXXX.', 'XXXXooXXXX', 'XXoXooXoXX', 'XXXXooXXXX', 'XXoXooXoXX', 'XXXXooXXXX', 'XXoXooXoXX', 'XXXXooXXXX', '.XXXXXXXX.', '..........'],
  cross: ['...XXXX...', '...XXXX...', '...XXXX...', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '...XXXX...', '...XXXX...', '...XXXX...'],
  spark: ['....XX....', '....XX....', '....XX....', '...XXXX...', 'XXXXooXXXX', 'XXXXooXXXX', '...XXXX...', '....XX....', '....XX....', '....XX....'],
  bolt: ['......XXX.', '.....XXX..', '....XXX...', '...XXX....', '..XXXXXXX.', '......XXX.', '.....XXX..', '....XX....', '...XX.....', '...X......'],
  eye: ['..........', '...XXXX...', '.XXXXXXXX.', 'XXXXooXXXX', 'XXXooooXXX', 'XXXooooXXX', 'XXXXooXXXX', '.XXXXXXXX.', '...XXXX...', '..........'],
  crown: ['X...XX...X', 'XX..XX..XX', 'XXX.XX.XXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXoXXoXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '..........', '..........'],
  skull: ['..XXXXXX..', '.XXXXXXXX.', 'XXXXXXXXXX', 'XooXXXXooX', 'XooXXXXooX', 'XXXXooXXXX', '.XXXXXXXX.', '..XoXoXX..', '..XXXXXX..', '..........'],
  die: ['XXXXXXXXXX', 'XooXXXXooX', 'XooXXXXooX', 'XXXXXXXXXX', 'XXXXooXXXX', 'XXXXooXXXX', 'XXXXXXXXXX', 'XooXXXXooX', 'XooXXXXooX', 'XXXXXXXXXX'],
  die1: ['XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXooXXXX', 'XXXooooXXX', 'XXXooooXXX', 'XXXXooXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX'],
  die20: ['....XX....', '...XXXX...', '..XXooXX..', '.XXXooXXX.', 'XXXooooXXX', 'XXXXXXXXXX', '.XXoXXoXX.', '..XXXXXX..', '...XXXX...', '....XX....'],
  diebad: ['XXXXXXXXXX', 'XoXXXXXXoX', 'XXoXXXXoXX', 'XXXoXXoXXX', 'XXXXooXXXX', 'XXXXooXXXX', 'XXXoXXoXXX', 'XXoXXXXoXX', 'XoXXXXXXoX', 'XXXXXXXXXX'],
  dicewarn: ['XXXXXXXX..', 'XXXXXXXX.o', 'XooXXooXoo', 'XXXXXXXXoo', 'XXXXXXXXoo', 'XooXXooX.o', 'XXXXXXXX..', 'XXXXXXXX..', '..........', '..........'],
  heart: ['.XXX..XXX.', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXXXXX..', '...XXXX...', '....XX....', '..........', '..........'],
  hearthalf: ['.XXX..XXX.', 'XXXXXoXXXX', 'XXXXoXXXXX', 'XXXXXoXXXX', '.XXXXoXXX.', '..XXXoXX..', '...XXoX...', '....XX....', '..........', '..........'],
  arrowup: ['....XX....', '...XXXX...', '..XXXXXX..', '.XXXXXXXX.', 'XXXXXXXXXX', '...XXXX...', '...XXXX...', '...XXXX...', '...XXXX...', '..........'],
  chevrons: ['..........', 'XX......XX', '.XX....XX.', '..XX..XX..', '...XXXX...', '..........', 'oo......oo', '.oo....oo.', '..oo..oo..', '...oooo...'],
  coin: ['...XXXX...', '.XXXXXXXX.', '.XXXooXXX.', 'XXXXooXXXX', 'XXXooooXXX', 'XXXXooXXXX', 'XXXXooXXXX', '.XXXooXXX.', '.XXXXXXXX.', '...XXXX...'],
  compass: ['....XX....', '....XX....', '...XXXX...', '.XXXooXXX.', 'XXXooooXXX', 'XXXooooXXX', '.XXXooXXX.', '...XXXX...', '....XX....', '....XX....'],
  coins: ['..........', '...XXXX...', '..XooooX..', '..XXXXXX..', '.XXXXXXXX.', 'XooooooooX', 'XXXXXXXXXX', 'XooooooooX', 'XXXXXXXXXX', '..........'],
  flask: ['...XXXX...', '....XX....', '....XX....', '...XXXX...', '..XXooXX..', '.XXooooXX.', '.XooooooX.', '.XXooooXX.', '..XXXXXX..', '..........'],
  hourglass: ['XXXXXXXXXX', '.XXXXXXXX.', '..XXooXX..', '...XooX...', '....XX....', '....XX....', '...XooX...', '..XooooX..', '.XooooooX.', 'XXXXXXXXXX'],
  flame: ['.....X....', '....XX....', '...XXX..X.', '..XXXXX.XX', '.XXXooXXXX', '.XXoooXXX.', 'XXXooooXXX', 'XXXooooXXX', '.XXooooXX.', '..XXXXXX..'],
  drop: ['....XX....', '....XX....', '...XXXX...', '...XXXX...', '..XXXXXX..', '.XXXooXXX.', '.XXooXXXX.', '.XXXXXXXX.', '..XXXXXX..', '...XXXX...'],
  clover: ['.XXX..XXX.', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXooXX..', '.XXXooXXX.', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXX..XXX.', '....o.....'],
  medal: ['.XX....XX.', '.XXX..XXX.', '..XXXXXX..', '...XXXX...', '..oooooo..', '.ooXooXoo.', '.oooXXooo.', '.ooXooXoo.', '..oooooo..', '...oooo...'],
  rat: ['.XX....XX.', 'XXXX..XXXX', 'XoXXXXXXoX', '.XXXXXXXX.', '.XXoXXoXX.', '.XXXXXXXX.', '..XXXXXX..', '..XXooXX..', '...XXXX...', '....XX....'],
  hammer: ['..XXXXXXX.', '.XXXXXXXXX', '.XXXXXXXXX', '..XXXXXXX.', '....oo....', '....oo....', '....oo....', '....oo....', '....oo....', '....oo....'],
  burst: ['X...XX...X', '.X..XX..X.', '..X.XX.X..', '...XXXX...', 'XXXXooXXXX', 'XXXXooXXXX', '...XXXX...', '..X.XX.X..', '.X..XX..X.', 'X...XX...X'],
  swing: ['......XXXX', '.....XXXXX', '....XXXX..', '...XXXX...', 'X.XXXX....', '.XXXX.....', 'ooXXo.....', '.oo.oo....', '..........', '..........'],
  target: ['...XXXX...', '.XXXooXXX.', '.XXXooXXX.', 'XXXXooXXXX', 'XooooooooX', 'XooooooooX', 'XXXXooXXXX', '.XXXooXXX.', '.XXXooXXX.', '...XXXX...'],
  star: ['....XX....', '....XX....', '...XXXX...', 'XXXXXXXXXX', '.XXXooXXX.', '..XXooXX..', '..XXXXXX..', '.XXX..XXX.', '.XX....XX.', '..........'],
  trophy: ['XXXXXXXXXX', 'XooXXXXooX', 'XooXXXXooX', '.XXXXXXXX.', '..XXXXXX..', '....XX....', '....XX....', '...XXXX...', '..XXXXXX..', '..........'],
  bones: ['XX......XX', 'XXX....XXX', '.XXX..XXX.', '..XXXXXX..', '...XXXX...', '..XXXXXX..', '.XXX..XXX.', 'XXX....XXX', 'XX......XX', '..........'],
  scroll: ['.XXXXXXXX.', 'XXXXXXXXXX', '.XXoooXX..', '.XXXXXXX..', '.XXoooXX..', '.XXXXXXX..', '.XXoooXX..', 'XXXXXXXXXX', '.XXXXXXXX.', '..........'],
  key: ['..XXXX....', '.XX..XX...', '.XX..XX...', '..XXXX....', '...XX.....', '...XX.....', '...XXXX...', '...XX.....', '...XXXX...', '..........'],
  moon: ['...XXXXX..', '..XXXX....', '.XXXX.....', '.XXXX.....', '.XXXX.....', '.XXXX.....', '..XXXX....', '...XXXXXX.', '..........', '..........'],
  chest: ['..........', '.XXXXXXXX.', 'XXXXXXXXXX', 'XXXXXXXXXX', 'oooooooooo', 'XXXXooXXXX', 'XXXXooXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '..........'],
  potion: ['....oo....', '....XX....', '....XX....', '...XXXX...', '..XXXXXX..', '.XXXooXXX.', '.XXooXXXX.', '.XXXXXXXX.', '..XXXXXX..', '..........'],
  brace: ['XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXXXXX..', '...XXXX...', '....XX....', '..........'],
  mend: ['..XXXXXX..', '.XXXXXXXX.', 'XXXXooXXXX', 'XXXXooXXXX', 'XooooooooX', 'XooooooooX', 'XXXXooXXXX', '.XXXooXXX.', '..XXooXX..', '...XXXX...'],
  rampage: ['.XX....XX.', 'XXXX..XXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXooXX..', '..XXooXX..', '.XXXooXXX.', 'XXX.XX.XXX', 'XX..XX..XX', 'X...XX...X'],
  // Status glyphs for the CoffeeMUD kit.
  freeze: ['....XX....', 'X...XX...X', '.X..XX..X.', '..X.XX.X..', 'XXXXooXXXX', 'XXXXooXXXX', '..X.XX.X..', '.X..XX..X.', 'X...XX...X', '....XX....'],
  stun: ['..o....o..', '.ooo..ooo.', '..o....o..', '..........', 'XX..XX..XX', '.XX.XX.XX.', '..XXXXXX..', '.XX.XX.XX.', 'XX..XX..XX', '..........'],
  poison: ['....XX....', '...XXXX...', '..XXXXXX..', '.XXXooXXX.', '.XXXooXXX.', '.XXXXXXXX.', '.XXXooXXX.', '..XXXXXX..', '...XXXX...', '..........'],
  disease: ['X..XXXX..X', '.XXXXXXXX.', '.XXoXXoXX.', 'XXXXXXXXXX', 'XXXXooXXXX', 'XXXXXXXXXX', '.XXoXXoXX.', '.XXXXXXXX.', 'X..XXXX..X', '..........'],
  bleed: ['....XX....', '...XXXX...', '...XXXX...', '..XXXXXX..', '..XXXXXX..', '.XXXXXXXX.', '.XXXXXXXX.', '..XXXXXX..', '...XXXX...', '..oo..oo..'],
  arrowdown: ['..........', '...XXXX...', '...XXXX...', '...XXXX...', '...XXXX...', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXXXXX..', '...XXXX...', '....XX....'],
  mana: ['....XX....', '...XXXX...', '..XXXXXX..', '.XXXooXXX.', 'XXXooooXXX', 'XXXooooXXX', '.XXXooXXX.', '..XXXXXX..', '...XXXX...', '....XX....'],
  // Status glyphs for the CoffeeMUD kit, second batch: sleep, blind, silence, slow.
  sleep: ['XXXXXX....', '...XX.....', '..XX......', '.XXXXXX...', '......oooo', '.......oo.', '......oo..', '......oooo', '..........', '..........'],
  blind: ['o.........', '.o.XXXX...', '..oXXXXXX.', '.XXoXXXXXX', 'XXXXoXXXXX', 'XXXXXoXXXX', '.XXXXXoXX.', '..XXXXXoX.', '...XXXX.o.', '.........o'],
  silence: ['.XXXXXXXo.', 'XXXXXXXoXX', 'XXXXXXoXXX', 'XXXXXoXXXX', 'XXXXoXXXXX', '.XXoXXXXX.', '..oXX.....', '.oXX......', 'o.........', '..........'],
  slow: ['....X....o', '...XX...oo', '..XXX..ooo', '.XXXX.oooo', 'XXXXXooooo', '.XXXX.oooo', '..XXX..ooo', '...XX...oo', '....X....o', '..........'],
  curse: ['XXXXXXXXXX', 'X........X', 'X.oooooo.X', 'X.o....o.X', 'X.oXXXXo.X', 'X.oXXXXo.X', 'X.o....o.X', 'X.oooooo.X', 'X........X', 'XXXXXXXXXX'],
  // The ten panel icons for a carousel of screens (T12): what a panel is, not what a skill does.
  actions: ['X........X', '.X......X.', '..X....X..', '...X..X...', '....XX....', '....XX....', '...o..o...', '..oo..oo..', '.oo....oo.', 'oo......oo'],
  chat: ['XXXXXXXXXX', 'XXXXXXXXXX', 'XooXooXooX', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..XXX.....', '..XX......', '..X.......', '..........'],
  party: ['.XX.oo.XX.', '.XX.oo.XX.', 'XXXXooXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '..........', '..........', '..........', '..........', '..........'],
  character: ['...XXXX...', '..XXXXXX..', '..XXXXXX..', '...XXXX...', '..........', '.XXXXXXXX.', 'XXXXooXXXX', 'XXXXooXXXX', 'XXXXXXXXXX', '..........'],
  inventory: ['...XXXX...', '..X....X..', '...XXXX...', '..XXXXXX..', '.XXXooXXX.', 'XXXXooXXXX', 'XXXXXXXXXX', 'XXXXXXXXXX', '.XXXXXXXX.', '..........'],
  map: ['..........', 'XXX.XXX.XX', 'XXXXXXXXXX', 'XoXXXXXoXX', 'XXoXXXXXXX', 'XXXXXoXXXX', 'XXXXXXXXXX', 'XXX.XXX.XX', '..........', '..........'],
  chronicle: ['XXXXXXXXXX', 'XooooXoooX', 'XXXXXXXXXX', 'XooooXoooX', 'XXXXXXXXXX', 'XooooXoooX', 'XXXXXXXXXX', 'XXXXoXXXXX', '.XXXoXXXX.', '.....o....'],
  charts: ['........XX', '........XX', '....XX..XX', '....XX..XX', 'XX..XX..XX', 'XX..XX..XX', 'XX..XX..XX', 'XX..XX..XX', 'oooooooooo', '..........'],
  research: ['...XXXX...', '..X....X..', '.X......X.', '.X......X.', '.X......X.', '..X....X..', '...XXXXoo.', '.......ooo', '........oo', '.........o'],
  help: ['..XXXXXX..', '.XXXXXXXX.', 'XXX....XXX', '......XXX.', '....XXXX..', '...XXX....', '...XX.....', '..........', '...oo.....', '...oo.....'],
};
export const GLYPH_IDS = Object.keys(GLYPHS);

/** Plate shapes: where the plate is filled. `plate` is a rounded square, `round` a disc, `bare` has none (the symbol and its outline only). */
const PLATES = {
  plate: (m) => { m.rect(0, 1, S, S - 2, 1).rect(1, 0, S - 2, S, 1); },
  round: (m) => { m.ellipse(8, 8, 8, 8, 1); },
};
export const PLATE_IDS = [...Object.keys(PLATES), 'bare'];

/** One pictogram. `plate` and `accent` are ramp names (palette.js); `ink` is the symbol's ramp. */
export function drawIcon({ glyph = 'sword', plate = 'blue', ink = 'white', accent = 'gold', shape = 'plate' } = {}) {
  const rows = GLYPHS[glyph]; if (!rows) throw new Error(`unknown glyph "${glyph}"`);
  if (!PLATE_IDS.includes(shape)) throw new Error(`unknown plate shape "${shape}"`);
  const g = new Pixels(12, 12).blit(Pixels.fromRows(rows, { X: 1, o: 2 }), 1, 1), glyphArt = shade(g, { 1: ramp(ink), 2: ramp(accent) });
  const out = new Pixels(S, S);
  if (shape !== 'bare') { const m = new Pixels(S, S); PLATES[shape](m); out.blit(shade(m, { 1: ramp(plate) }, { outline: C.night })); }
  // The symbol sits in the middle of the plate (an outline cell on every side of its 10x10 body), a touch higher so its weight reads as centred.
  out.blit(glyphArt, 2, 2);
  return { id: `icon.${glyph}`, frames: [out], fps: 0, loop: false, mask: g };
}
