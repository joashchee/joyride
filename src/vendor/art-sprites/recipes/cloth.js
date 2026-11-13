// Cloth and hair in motion: the hands/poses/motion kit's secondary-motion lessons as one parameterised loop. Every kind has an
// anchor that does not move (the pole, the shoulders, the rod, the scalp) and a free edge that does; the wave grows with distance
// from the anchor (overlap) and its phase lags behind it (drag), and a cape adds a constant `drag` so it trails a moving body
// (follow-through). Loops are 4 or 6 frames, because the kit finds a short loop reads cleaner than a smooth one. All frames come from
// one phase `f / frames`, so the last frame flows into the first and nothing boils.
import { Pixels } from '../pixels.js';
import { C, ramp } from '../palette.js';
import { clip } from '../anim.js';

const TAU = Math.PI * 2;
export const CLOTH_KINDS = ['flag', 'cape', 'banner', 'curtain', 'hair', 'robe'];
export const WINDS = { calm: 1, steady: 2, gust: 3 };
export const EMBLEMS = ['stripe', 'cross', 'diamond'];

/** Fold shading from the wave's slope: a face turned to the light is the light step, one turned away is the shade step. */
const tone = (r, s) => (s > 0.45 ? r[1] : s < -0.45 ? r[3] : r[2]);
/** Is (u, v) on the emblem? Coordinates are the cloth's own, so the emblem bends with it. */
const onEmblem = (e, u, v, w, h) => {
  if (e === 'stripe') return Math.abs(v - h / 2) < 1.6;
  if (e === 'cross') return Math.abs(u - w / 2) < 1.2 || Math.abs(v - h / 2) < 1;
  if (e === 'diamond') return Math.abs(u - w / 2) + Math.abs(v - h / 2) < Math.min(w, h) / 3;
  return false;
};

function flag({ r, amp, ph, emblem, accent }) {
  const p = new Pixels(24, 16), W = 18, H = 7, x0 = 3, y0 = 4, pole = ramp('wood');
  p.rect(2, 2, 1, 12, pole[2]).set(2, 1, ramp('gold')[3]).set(2, 13, pole[0]);
  for (let u = 0; u < W; u++) {
    const t = u / (W - 1), k = 0.5, off = Math.round(amp * t * Math.sin(ph - u * k)), s = Math.cos(ph - u * k) * (t > 0.12 ? 1 : 0);
    for (let v = 0; v < H; v++) {
      if (u > 14 && Math.abs(v - 3) < u - 14) continue; // the swallowtail notch
      let c = tone(r, s);
      if (onEmblem(emblem, u, v, W, H)) c = c === r[1] ? accent[1] : c === r[3] ? accent[3] : accent[2];
      p.set(x0 + u, y0 + v + off, v === 0 ? (c === r[1] ? r[0] : r[1] === c ? r[1] : r[3]) : c);
    }
  }
  return { px: p.outline(C.night), pin: [2, y0] };
}

function hung(kind, { r, amp, ph, emblem, accent, drag, tie }) {
  const p = new Pixels(24, 24), cx = 12, top = 3, H = kind === 'cape' ? 18 : 17, rod = ramp('wood');
  if (kind !== 'cape') p.rect(3, 1, 18, 1, rod[2]).set(2, 1, ramp('gold')[2]).set(21, 1, ramp('gold')[2]);
  for (let v = 0; v < H; v++) {
    const t = v / (H - 1), sway = amp * (kind === 'curtain' ? 0.5 : 1) * Math.pow(t, 1.4) * Math.sin(ph - v * 0.38) - drag * Math.pow(t, 1.5);
    const s = Math.cos(ph - v * 0.38) * (t > 0.15 ? 1 : 0), off = Math.round(sway);
    let half = kind === 'cape' ? 4.5 + v * 0.2 : kind === 'curtain' ? 5 : 5.5;
    if (kind === 'curtain' && tie) half = 5 - 2.2 * Math.exp(-(((v - 8) / 2.2) ** 2));
    const fold = (u) => (kind === 'cape' || kind === 'curtain' ? ([-3, 0, 3].includes(Math.round(u)) ? (Math.round(u) === 0 ? 1 : -1) : 0) : 0);
    for (let u = -Math.floor(half); u < Math.ceil(half); u++) {
      if (kind === 'banner' && v > H - 4 && Math.abs(u + 0.5) < (v - (H - 4)) * 0.8) continue; // the swallowtail
      let c = tone(r, s + (fold(u) ? 0.0 : 0));
      if (fold(u) === 1) c = r[3]; else if (fold(u) === -1) c = r[1]; // fixed fold lines that ride the sway
      if (onEmblem(emblem, u + half, v, half * 2, H)) c = accent[2];
      if (v === H - 1 && kind !== 'banner') c = r[0]; // the hem catches the least light
      p.set(cx + u + off, top + v, c);
    }
  }
  if (kind === 'cape') p.set(cx - 5, top, ramp('gold')[3]).set(cx + 4, top, ramp('gold')[3]); // the clasps pin the anchor
  return { px: p.outline(C.night), pin: [cx, top] };
}

/** A robe or skirt through one full turn of the body. The cloth is a flared cylinder: the folds sit at fixed angles round it and the turn
 * carries them across the front (a fold whose angle faces away is hidden), the hem's angle lags the waist's (drag) so it swirls behind, and
 * a stronger wind flares it wider. Five folds, so the loop closes after a fifth of a turn and the last frame flows into the first. */
function robe({ r, amp, ph, accent }) {
  const p = new Pixels(24, 24), cx = 12, top = 3, H = 19, gold = accent, FOLDS = 5;
  for (let v = 0; v < H; v++) {
    const t = v / (H - 1), lag = ph - 0.9 * Math.pow(t, 1.5) * (amp / 2), half = 3.4 + t * (3.6 + amp * 1.5) + (t > 0.85 ? 0.8 : 0);
    for (let u = -Math.ceil(half); u <= Math.ceil(half); u++) {
      const sn = u / half;
      if (Math.abs(sn) > 1) continue;
      const phi = Math.asin(sn), fold = Math.cos(FOLDS * (phi + lag));
      const hem = H - 1 + Math.round(amp * 0.5 * Math.cos(FOLDS * (phi + lag)) * t);
      if (v > hem) continue;
      let c = sn < -0.35 ? r[3] : sn > 0.35 ? r[1] : r[2]; // the light is on the left (a ramp runs dark to light)
      if (t > 0.2 && fold > 0.55) c = c === r[3] ? r[2] : r[1]; // a crease turned from the light
      else if (t > 0.2 && fold < -0.7) c = c === r[1] ? r[2] : r[3];
      if (v >= hem - 1 && t > 0.8) c = gold[2]; // the trim follows the hem
      if (v === 4 && Math.abs(sn) < 0.98) c = gold[3]; // the sash at the waist
      p.set(cx + u, top + v, c);
    }
  }
  return { px: p.outline(C.night), pin: [cx, top + 4] };
}

function hair({ r, amp, ph }) {
  const out = new Pixels(16, 16);
  // Two locks stream from the scalp, each a tapering ribbon whose centre waves with a lag: thick at the anchor, a point at the free end.
  for (const { y, th, len, lag } of [{ y: 6, th: 7, len: 11, lag: 0 }, { y: 10, th: 8, len: 9, lag: 0.9 }]) {
    for (let t = 0; t < len; t++) {
      const k = t / (len - 1), c = y + k * 0.6 + amp * k * 1.3 * Math.sin(ph - t * 0.5 - lag), h = Math.max(1, Math.round(th * Math.pow(1 - k, 0.5)));
      const y0 = Math.round(c - h / 2);
      for (let j = 0; j < h; j++) out.set(4 + t, y0 + j, j === 0 ? r[3] : j === h - 1 && h > 2 ? r[1] : r[2]);
    }
  }
  out.ellipse(4.5, 8, 3.5, 5, 1); // the scalp, over the roots
  for (let y = 0; y < 16; y++) for (let x = 0; x < 9; x++) if (out.get(x, y) === 1) out.set(x, y, !out.get(x, y - 1) || !out.get(x - 1, y) ? r[3] : r[2]);
  return { px: out.outline(C.night), pin: [4, 8] };
}

/** `kind` flag | cape | banner | curtain | hair | robe (a skirt through a full turn); `wind` calm | steady | gust; `frames` 4 or 6; `drag` pixels a cape trails behind a moving body. */
export function drawCloth({ kind = 'flag', wind = 'steady', ramp: rampName = 'red', accent = 'gold', emblem = kind === 'flag' || kind === 'banner' ? 'stripe' : null, frames = 6, drag = 0, tie = false } = {}) {
  if (!CLOTH_KINDS.includes(kind)) throw new Error(`unknown cloth kind "${kind}"`);
  if (!WINDS[wind]) throw new Error(`unknown wind "${wind}"`);
  if (![4, 6].includes(frames)) throw new Error('cloth loops are 4 or 6 frames');
  if (emblem && !EMBLEMS.includes(emblem)) throw new Error(`unknown emblem "${emblem}"`);
  const r = ramp(rampName), acc = ramp(accent), amp = WINDS[wind] * (kind === 'hair' ? 1 : kind === 'flag' ? 1 : 0.7);
  const make = kind === 'flag' ? flag : kind === 'hair' ? hair : kind === 'robe' ? robe : (o) => hung(kind, o);
  const list = Array.from({ length: frames }, (_, f) => make({ r, amp, ph: (TAU * f) / frames, emblem, accent: acc, drag, tie }));
  const fps = wind === 'gust' ? 10 : wind === 'steady' ? 8 : 6;
  return { id: `cloth.${kind}`, frames: list.map((x) => x.px), fps, loop: true, spec: { kind, wind, ramp: rampName, emblem, frames, drag }, anchors: { pin: list[0].pin }, clips: { wave: clip('wave', list.map((x) => x.px), { fps }) } };
}
