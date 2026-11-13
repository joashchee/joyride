// Vehicles (32x32, side view, facing right): the ones `prop` does not draw (it has boat, cart, ship, caravan, spaceship). A mask of
// part ids (1 body, 2 trim, 3 wheels and runners, 4 glass and canvas, 5 metal), shaded by shade.js; the seed picks the body colour.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { ramp } from '../palette.js';
import { rng } from '../rng.js';

const S = 32;
const BODIES = ['red', 'blue', 'green', 'purple', 'gold'];
/** A wheel of radius r with a hub: part 3 for the tyre, part 5 for the hub. */
const wheel = (m, cx, cy, r) => { m.ellipse(cx, cy, r, r, 3); m.ellipse(cx, cy, r / 2.4, r / 2.4, 5); };

export const VEHICLES = {
  wagon: { body: 'wood', draw(m) { m.rect(5, 15, 21, 5, 1); m.rect(4, 13, 2, 7, 2); m.rect(25, 13, 2, 7, 2); m.poly([[7, 14], [9, 5], [23, 5], [25, 14]], 4); m.rect(7, 14, 18, 1, 2); m.rect(26, 17, 5, 1, 2); wheel(m, 10, 23, 5); wheel(m, 22, 23, 5); } },
  sled: { body: 'wood', draw(m) { m.rect(6, 14, 18, 5, 1); m.rect(5, 12, 2, 7, 2); m.rect(23, 12, 2, 7, 2); m.rect(3, 22, 25, 2, 3); m.line(3, 22, 1, 19, 3); m.rect(8, 19, 1, 3, 5); m.rect(20, 19, 1, 3, 5); m.rect(10, 9, 9, 5, 4); } },
  car: { body: 'red', draw(m) { m.rect(2, 15, 28, 6, 1); m.poly([[7, 15], [11, 8], [21, 8], [26, 15]], 1); m.poly([[9, 15], [12, 9], [16, 9], [16, 15]], 4); m.poly([[17, 15], [17, 9], [21, 9], [24, 15]], 4); m.rect(2, 18, 3, 2, 2); m.rect(28, 17, 2, 2, 2); m.rect(2, 20, 28, 1, 5); wheel(m, 9, 22, 4); wheel(m, 23, 22, 4); } },
  locomotive: { body: 'iron', draw(m) { m.rect(3, 12, 17, 9, 1); m.rect(20, 7, 9, 14, 1); m.rect(19, 5, 11, 2, 2); m.rect(21, 9, 5, 5, 4); m.rect(5, 6, 4, 6, 5); m.rect(4, 4, 6, 2, 5); m.ellipse(14, 12, 4, 1.5, 2); m.rect(2, 19, 28, 2, 2); m.line(1, 21, 4, 18, 5); wheel(m, 8, 24, 4); wheel(m, 15, 24, 4); wheel(m, 25, 24, 4); } },
  airship: { body: 'blue', draw(m) { m.ellipse(16, 10, 14, 7, 1); m.line(5, 10, 27, 10, 2); m.rect(11, 16, 11, 5, 4); m.rect(10, 20, 13, 3, 2); m.rect(13, 17, 1, 3, 1); m.rect(17, 17, 1, 3, 1); m.rect(13, 14, 1, 3, 5); m.rect(19, 14, 1, 3, 5); m.poly([[3, 8], [1, 4], [5, 6]], 2); m.poly([[3, 12], [1, 16], [5, 14]], 2); m.rect(27, 14, 3, 2, 5); m.rect(29, 12, 1, 6, 5); } },
  raft: { body: 'wood', draw(m) { for (let i = 0; i < 5; i++) m.rect(4, 17 + i * 2, 24, 2, i % 2 ? 1 : 2); m.rect(15, 6, 2, 11, 3); m.poly([[17, 6], [26, 9], [17, 14]], 4); m.rect(5, 16, 3, 1, 5); } },
  cannon: { body: 'iron', draw(m) { m.poly([[5, 10], [26, 6], [29, 11], [7, 17]], 1); m.rect(26, 5, 4, 8, 2); m.rect(3, 11, 5, 6, 1); m.rect(7, 18, 12, 4, 3); wheel(m, 11, 22, 6); m.rect(4, 15, 2, 2, 5); } },
};
export const VEHICLE_IDS = Object.keys(VEHICLES);

/** Draw one vehicle. `body` overrides the ramp of part 1; otherwise the seed picks one (cars, airships) or the kind's own is used. */
export function drawVehicle({ kind = 'wagon', seed = 'vehicle', body } = {}) {
  const def = VEHICLES[kind]; if (!def) throw new Error(`unknown vehicle "${kind}"`);
  const r = rng(`${seed}:${kind}`), m = new Pixels(S, S);
  def.draw(m);
  const bodyRamp = body ?? (['car', 'airship'].includes(kind) ? r.pick(BODIES) : def.body);
  const out = shade(m, { 1: ramp(bodyRamp), 2: ramp(kind === 'locomotive' ? 'gold' : 'wood'), 3: ramp('iron'), 4: ramp(['wagon', 'raft', 'sled'].includes(kind) ? 'bone' : 'glass'), 5: ramp('steel') });
  return { id: `vehicle.${kind}`, frames: [out], fps: 0, loop: false };
}
