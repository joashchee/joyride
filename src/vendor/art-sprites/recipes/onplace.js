// A face or an item on its place (ANSI only): the figure layer as a recipe. The figure is drawn by `creatureface` or `thing`, laid on a darker
// picture of the place and lit by it (figure.js). Written for docs/coupler-gaps-plan.md Phase F. Browser-safe.
import { drawCreatureFace, readFace } from './face.js';
import { drawThing } from './item-art.js';
import { figureOnPlace } from './figure.js';
import { checkColors } from './vista-canvas.js';

/**
 * Params: `face` (what `creatureface` takes; a human by default) or `thing` (what `thing` takes), and the place: `terrain`, `time`,
 * `weather`, `style`, `key` (seeds the place; the race or name by default), `arc`, `recede`, `wild` (an animal stands on woods, plains or
 * hills when its terrain is a room or a town; true for a face that is not a person, by default), `invisible`, `offset`, `bgLimit`, `colors` (16, 256 or 'true', for the figure and the place). Returns
 * the picture, the figure's own words with the place, and the figure.
 */
export function drawOnPlace(params = {}) {
  const { face, thing, wild, bgLimit = 8, colors = 16, ...place } = params;
  checkColors(colors);
  const art = thing ? drawThing({ colors, ...thing }) : drawCreatureFace({ colors, ...(face ?? { race: 'Human' }) });
  const f = thing ? undefined : readFace(face ?? { race: 'Human' });
  const { cells, terrain } = figureOnPlace(art.figure, { key: thing?.name ?? face?.race ?? 'human', wild: wild ?? (f ? f.family !== 'Person' && !f.sapient : false), ...place }, { bgLimit, colors });
  return { id: `onplace.${terrain}`, frames: [cells], fps: 0, loop: false, says: `${String(art.says).replace(/\.$/, '')}, on ${terrain}${place.time ? ` at ${place.time}` : ''}.`, figure: art.figure, ...(colors !== 16 ? { colors } : {}) };
}
