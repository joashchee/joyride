// Hands (24x24). The hands kit's workflow as a parameter: `stage` is how far the study has got. A mitten is the gesture as one mass,
// blocks separate the fingers, detailed adds nails, knuckles and a palm crease, so the same gesture can be drawn at any of the
// three and a small sprite can stop at the stage that reads at its size. Hands are right hands seen from the back; `side: 'left'` mirrors.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';

const S = 24, FX = [6, 9, 12, 15], TOP = 12; // finger columns (index to pinky), two wide with a one pixel gap; the palm starts at TOP
export const STAGES = ['mitten', 'blocks', 'detailed'];
/** `fingers` is each finger's length (0 = curled to the palm); `spread` fans them; `thumb` is out, tuck (across the palm) or up. */
export const GESTURES = {
  open: { fingers: [8, 9, 8, 6], thumb: 'out' },
  ward: { fingers: [8, 9, 8, 6], thumb: 'tuck' },
  fist: { fingers: [0, 0, 0, 0], thumb: 'tuck' },
  point: { fingers: [10, 0, 0, 0], thumb: 'tuck' },
  thumbsup: { fingers: [0, 0, 0, 0], thumb: 'up' },
  cast: { fingers: [8, 9, 8, 6], thumb: 'out', spread: 1 },
  grip: { fingers: [0, 0, 0, 0], thumb: 'tuck', turn: true },
};
export const GESTURE_IDS = Object.keys(GESTURES);

function build(g, stage) {
  const m = new Pixels(S, S);
  m.rect(6, TOP, 11, 8, 1).rect(8, 20, 7, 3, 1); // palm and wrist
  g.fingers.forEach((len, i) => {
    const x = FX[i];
    if (!len) { m.rect(x, TOP - 2, 2, 2, 1); return; } // a curled finger is its knuckle
    const dx = stage === 'mitten' ? 0 : Math.round((g.spread ?? 0) * [-3, -1, 1, 3][i]), top = TOP - len;
    if (dx) m.poly([[x, TOP], [x + 2, TOP], [x + 2 + dx, top], [x + dx, top]], 1); else m.rect(x, top, 2, len, 1);
  });
  if (g.thumb === 'out') m.poly([[7, 19], [6, 14], [3, 10], [1, 11], [2, 15], [4, 19]], 1);
  else if (g.thumb === 'up') m.poly([[6, 16], [6, 10], [4, 5], [2, 6], [3, 11], [4, 16]], 1);
  else m.rect(7, 15, 6, 3, 1);
  if (stage === 'mitten') { // close the gaps: the gesture as one mass
    for (let pass = 0; pass < 2; pass++) { const src = m.clone(); for (let y = 0; y < S; y++) for (let x = 1; x < S - 1; x++) if (!src.get(x, y) && src.get(x - 1, y) && src.get(x + 1, y)) m.set(x, y, 1); }
    return m;
  }
  if (stage === 'detailed') {
    g.fingers.forEach((len, i) => { if (len) { m.rect(FX[i], TOP - len, 2, 1, 2); m.rect(FX[i], TOP + 1, 2, 1, 3); } });
    m.line(7, 17, 15, 16, 3);
    if (g.thumb === 'tuck') m.line(8, 17, 12, 17, 3);
  }
  return m;
}

export function drawHand({ gesture = 'open', stage = 'detailed', skin = 'skin-light', side = 'right' } = {}) {
  const g = GESTURES[gesture]; if (!g) throw new Error(`unknown gesture "${gesture}"`);
  if (!STAGES.includes(stage)) throw new Error(`unknown stage "${stage}"`);
  let m = build(g, stage); if (g.turn) m = m.rot90();
  const sk = ramp(skin);
  let px = shade(m, { 1: sk, 2: ramp('skin-pale'), 3: [sk[0], sk[0], sk[1], sk[1]] }, { outline: C.night });
  if (side === 'left') px = px.flipX();
  return { id: `hand.${gesture}.${stage}`, frames: [px], fps: 0, loop: false, spec: { gesture, stage, skin, side }, anchors: { wrist: [side === 'left' ? 12 : 11, 22], grip: [11, 15] } };
}
