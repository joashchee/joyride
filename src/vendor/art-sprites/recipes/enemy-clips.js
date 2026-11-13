// What an enemy does beyond idle, hit and defeat (T14), the layers drawn over it (status looks, an intent marker) and the elite and boss treatment (T13).
// Every function takes the creature's still as a `Pixels` and works in either view, so a front creature and a top-down one share them. `d` is the unit
// vector the creature faces: a lunge goes along it and a wind-up goes against it.
import { Pixels } from '../pixels.js';
import { C, RAMPS, ramp } from '../palette.js';
import { rng } from '../rng.js';
import { clip, flash, squash } from '../anim.js';

export const DIRS = { north: [0, -1], east: [1, 0], south: [0, 1], west: [-1, 0] };
export const ENEMY_CLIPS = ['telegraph', 'attack', 'attack_ranged', 'attack_magic', 'stagger', 'block', 'enrage', 'summon', 'retreat', 'dissolve', 'phase'];
export const STATUSES = ['poison', 'burn', 'freeze', 'stun', 'bleed'];
export const INTENTS = ['attack', 'defend', 'magic', 'flee', 'summon'];
export const RANKS = ['normal', 'elite', 'boss'];

const at = (px, [dx, dy], n) => px.shift(dx * n, dy * n);
const outlineOf = (px) => px.map((x, y) => (px.get(x, y) === C.night && [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([i, j]) => !px.get(x + i, y + j)) ? 1 : 0));

/** The art with its dark outline recoloured (a braced guard, an elite's gold edge). */
export function reoutline(px, c) {
  const edge = outlineOf(px), out = px.clone();
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) if (edge.get(x, y)) out.set(x, y, c);
  return out;
}

/** Every colour of the art moved onto one ramp by its value, the outline kept: how a creature is tinted red in a rage. */
export function tint(px, rampName) {
  const r = ramp(rampName);
  return px.map((x, y) => {
    const v = px.get(x, y); if (!v) return 0;
    if (v === C.night) return v;
    const l = px.luma(v);
    return r[l < 70 ? 0 : l < 125 ? 1 : l < 185 ? 2 : 3];
  });
}

/** The colour one step along its own ramp (`+1` lighter, `-1` darker), or itself when it is on none. */
const LOOKUP = new Map();
for (const r of Object.values(RAMPS)) r.forEach((c, i) => { if (!LOOKUP.has(c)) LOOKUP.set(c, [r, i]); });
export const stepColor = (c, n) => { const hit = LOOKUP.get(c); return hit ? hit[0][Math.max(0, Math.min(3, hit[1] + n))] : c; };

const front = (px, d) => { const b = px.bbox() ?? { x: 0, y: 0, w: px.w, h: px.h }; return [d[0] > 0 ? b.x + b.w - 1 : d[0] < 0 ? b.x : b.x + (b.w >> 1), d[1] > 0 ? b.y + b.h - 1 : d[1] < 0 ? b.y : b.y + (b.h >> 1)]; };

function sparks(px, seed, n, ramps, spread) {
  const b = px.bbox() ?? { x: 0, y: 0, w: px.w, h: px.h }, r = rng(seed), p = new Pixels(px.w, px.h, px.palette);
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + spread, x = b.x + b.w / 2 + Math.cos(a) * (b.w / 2 + 2), y = b.y + b.h / 2 + Math.sin(a) * (b.h / 2 + 2);
    p.rect(Math.round(x), Math.round(y), 2, 2, ramps[r.int(ramps.length)]);
  }
  return p;
}

/**
 * The enemy clips of T14, each with its events and a reduced still. `flash: false` swaps the white silhouettes for a plain gold edge (the no-flash setting).
 * telegraph: the wind-up, leaning back, with a `release` event on its last frame. attack, attack_ranged, attack_magic: a lunge (`impact`), a recoil (`fire`),
 * a rising spark ring (`cast`). stagger, block, enrage (red, shaking), summon (a ring at the feet, `spawn`), retreat (backs off, fading), dissolve (seeded, `gone`)
 * and phase (the boss changing form).
 */
export function enemyClips(art, { direction = 'west', seed = 'enemy', flash: strobe = true } = {}) {
  const d = DIRS[direction]; if (!d) throw new Error(`unknown direction "${direction}"; known: ${Object.keys(DIRS).join(', ')}`);
  const muzzle = front(art, d);
  const braced = reoutline(art, C.mist), red = tint(art, 'red'), ring = (n) => sparks(art, `${seed}:ring`, 8, [C.violet, C.pink, C.white], n);
  const feet = art.bbox() ?? { x: 0, y: 0, w: art.w, h: art.h };
  const spawn = (rad) => { const p = new Pixels(art.w, art.h, art.palette), cx = feet.x + feet.w / 2, cy = feet.y + feet.h - 1; for (let k = 0; k < 14; k++) { const a = (k / 14) * Math.PI * 2; p.set(Math.round(cx + Math.cos(a) * rad), Math.round(cy + Math.sin(a) * rad * 0.35), k & 1 ? C.violet : C.pink); } return p; };
  const fade = (px, keep, phase) => px.map((x, y) => (px.get(x, y) && ((x * 7 + y * 13 + phase) % 100) / 100 < keep ? px.get(x, y) : 0));
  const dissolve = (n) => { const q = rng(`${seed}:dissolve`), cut = new Map(); return art.map((x, y) => { const v = art.get(x, y); if (!v) return 0; const k = `${x},${y}`; if (!cut.has(k)) cut.set(k, q.next()); return cut.get(k) >= n ? v : 0; }); };
  const gold = reoutline(art, C.yellow), violet = tint(art, 'purple');
  const clips = {
    telegraph: clip('telegraph', [at(art, d, -1), at(squash(art, 0.96), d, -2), at(art, d, -3)], { fps: 6, loop: false, events: [{ frame: 2, name: 'release' }], reduced: [at(art, d, -3)] }),
    attack: clip('attack', [at(art, d, -2), at(art, d, 3), at(art, d, 1), art], { fps: 10, loop: false, events: [{ frame: 1, name: 'impact' }], reduced: [at(art, d, 3)] }),
    attack_ranged: (() => {
      const f = new Pixels(art.w, art.h, art.palette), [mx, my] = muzzle; f.rect(mx + d[0], my + d[1], 2, 2, C.yellow).set(mx + d[0] * 3, my + d[1] * 3, C.white);
      const shot = at(art, d, -1).blit(f);
      return { ...clip('attack_ranged', [at(art, d, -1), shot, at(art, d, -1), art], { fps: 10, loop: false, events: [{ frame: 1, name: 'fire' }], reduced: [shot] }), anchors: { effect_origin: [mx, my] } };
    })(),
    attack_magic: clip('attack_magic', [art, art.clone().blit(ring(0)), art.clone().blit(ring(0.4)), art.clone().blit(ring(0.8))], { fps: 8, loop: false, events: [{ frame: 2, name: 'cast' }], reduced: [art.clone().blit(ring(0.4))] }),
    stagger: clip('stagger', [at(art, d, -2), at(art, d, -3).shift(d[1] ? 1 : 0, d[0] ? 1 : 0), at(art, d, -1), art], { fps: 8, loop: false, events: [{ frame: 1, name: 'reel' }], reduced: [at(art, d, -3)] }),
    block: clip('block', [at(braced, d, -1), at(braced, d, -2), at(braced, d, -1)], { fps: 8, loop: false, events: [{ frame: 1, name: 'block' }], reduced: [braced] }),
    enrage: clip('enrage', strobe ? [art, red, at(red, [1, 0], 1), at(red, [-1, 0], 1)] : [art, gold, at(gold, [1, 0], 1), at(gold, [-1, 0], 1)], { fps: 8, loop: false, events: [{ frame: 1, name: 'roar' }], reduced: [strobe ? red : gold] }),
    summon: clip('summon', [art, art.clone().blit(spawn(5)), art.clone().blit(spawn(9)), art.clone().blit(spawn(13))], { fps: 6, loop: false, events: [{ frame: 2, name: 'spawn' }], reduced: [art.clone().blit(spawn(9))] }),
    retreat: clip('retreat', [at(fade(art, 0.9, 0), d, 0), at(fade(art, 0.7, 1), d, -2), at(fade(art, 0.5, 2), d, -4), at(fade(art, 0.3, 3), d, -6)], { fps: 8, loop: false, events: [{ frame: 3, name: 'gone' }], reduced: [at(fade(art, 0.5, 2), d, -4)] }),
    dissolve: clip('dissolve', [dissolve(0.15), dissolve(0.4), dissolve(0.7), dissolve(0.92)], { fps: 6, loop: false, events: [{ frame: 3, name: 'gone' }], reduced: [dissolve(0.4)] }),
    phase: clip('phase', [strobe ? flash(art) : gold, gold, violet, art], { fps: 6, loop: false, events: [{ frame: 1, name: 'phase' }], reduced: [violet] }),
  };
  return clips;
}

/** A status look as a transparent layer the size of the art: poison bubbles, burning tongues, an ice crust, orbiting stars, bleeding drops. `phase` (0 or 1) animates it. */
export function statusLayer(art, statuses, { seed = 'status', phase = 0 } = {}) {
  const out = new Pixels(art.w, art.h, art.palette), b = art.bbox(); if (!b) return out;
  const edge = (x, y) => art.get(x, y) && [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([i, j]) => !art.get(x + i, y + j));
  const tops = []; for (let x = b.x; x < b.x + b.w; x++) for (let y = b.y; y < b.y + b.h; y++) if (art.get(x, y)) { tops.push([x, y]); break; }
  const inside = []; for (let y = b.y; y < b.y + b.h; y++) for (let x = b.x; x < b.x + b.w; x++) if (art.get(x, y) && !edge(x, y)) inside.push([x, y]);
  for (const s of statuses) {
    const r = rng(`${seed}:${s}`);
    if (s === 'poison') {
      const g = ramp('green');
      for (let i = 0; i < 4 && inside.length; i++) { const [x, y] = inside[r.int(inside.length)], k = (i + phase) % 2; out.ellipse(x + 0.5, y + 0.5 - k, 1.5, 1.5, g[2]).set(x, y - k - 1, g[3]); }
      for (let x = b.x + 2; x < b.x + b.w - 2; x += 5) { let y = b.y + b.h - 1; while (y > b.y && !art.get(x, y)) y--; out.set(x, y + 1 + phase, g[2]).set(x, y + 2 + phase, g[3]); }
    } else if (s === 'burn') {
      const f = ramp('fire');
      tops.forEach(([x, y], i) => { if (i % 3 !== 0) return; const h = 2 + ((i + phase) % 3); for (let k = 0; k < h; k++) out.set(x, y - 1 - k, k > 1 ? f[2] : k ? f[1] : f[0]); out.set(x + 1, y - 1, f[1]); });
    } else if (s === 'freeze') {
      const ic = ramp('ice');
      for (let y = b.y; y < b.y + b.h; y++) for (let x = b.x; x < b.x + b.w; x++) if (edge(x, y)) out.set(x, y, ic[2 + ((x + y) & 1)]); else if (art.get(x, y) && (x + y) % 5 === 0) out.set(x, y, ic[3]);
      for (let x = b.x + 1; x < b.x + b.w - 1; x += 4) { let y = b.y + b.h - 1; while (y > b.y && !art.get(x, y)) y--; out.set(x, y + 1, ic[2]).set(x, y + 2, ic[3]); }
    } else if (s === 'stun') {
      for (let i = 0; i < 3; i++) { const a = ((i + phase * 0.5) / 3) * Math.PI * 2, x = Math.round(b.x + b.w / 2 + Math.cos(a) * (b.w / 3)), y = Math.round(b.y - 2 + Math.sin(a) * 2); out.rect(x - 1, y, 3, 1, C.yellow).rect(x, y - 1, 1, 3, C.yellow).set(x, y, C.white); }
    } else if (s === 'bleed') {
      const rd = ramp('red');
      for (let i = 0; i < 3 && inside.length; i++) { const [x, y] = inside[r.int(inside.length)], fall = ((i + phase) % 2) * 2; out.set(x, y + fall, rd[1]).set(x, y + fall + 1, rd[2]); }
    } else throw new Error(`unknown status "${s}"; known: ${STATUSES.join(', ')}`);
  }
  return out;
}

const MARKERS = {
  attack: ['x.....x', '.x...x.', '..x.x..', '...x...', '..x.x..', '.x...x.', 'x.....x'],
  defend: ['xxxxxxx', 'x.....x', 'x.....x', 'x.....x', '.x...x.', '..x.x..', '...x...'],
  magic: ['...x...', '...x...', 'x..x..x', '.xxxxx.', 'x..x..x', '...x...', '...x...'],
  flee: ['...x...', '....x..', '.......', 'xxxxxxx', '.......', '....x..', '...x...'],
  summon: ['...x...', '...x...', '...x...', 'xxxxxxx', '...x...', '...x...', '...x...'],
};
const MARKER_INK = { attack: 'red', defend: 'blue', magic: 'purple', flee: 'gold', summon: 'green' };

/** An intent marker (what the enemy will do next) over its head: a 9 by 9 plate with a picture that differs in shape, not only colour. */
export function intentLayer(art, intent) {
  if (!MARKERS[intent]) throw new Error(`unknown intent "${intent}"; known: ${INTENTS.join(', ')}`);
  const out = new Pixels(art.w, art.h, art.palette), b = art.bbox() ?? { x: 0, y: 0, w: art.w, h: art.h }, r = ramp(MARKER_INK[intent]);
  const x0 = Math.max(0, Math.min(art.w - 9, b.x + (b.w >> 1) - 4)), y0 = Math.max(0, b.y - 10);
  out.rect(x0, y0, 9, 9, r[0]).frame(x0, y0, 9, 9, C.night);
  MARKERS[intent].forEach((row, j) => [...row].forEach((ch, i) => { if (ch === 'x') out.set(x0 + 1 + i, y0 + 1 + j, r[3]); }));
  return out;
}

/** Elite: a gold edge and a three-point crest over the middle of the head. Boss: the same, a taller crest. */
export function rankArt(px, rank) {
  if (rank === 'normal' || !rank) return px;
  if (!RANKS.includes(rank)) throw new Error(`unknown rank "${rank}"; known: ${RANKS.join(', ')}`);
  const out = reoutline(px, ramp('gold')[0]), b = px.bbox(); if (!b) return out;
  const g = ramp('gold'), cx = b.x + (b.w >> 1), top = Math.max(b.y, rank === 'boss' ? 5 : 3), tall = rank === 'boss' ? 4 : 2, crest = new Pixels(px.w, px.h, px.palette);
  for (const dx of [-2, 0, 2]) for (let k = 0; k < tall + (dx ? 0 : 1); k++) crest.set(cx + dx, top - 1 - k + (rank === 'boss' ? 2 : 1), k ? g[3] : g[2]);
  crest.rect(cx - 2, top + (rank === 'boss' ? 1 : 0), 5, 1, g[1]);
  return out.blit(crest.outline(C.night));
}

/** Resample by an exact factor, then break the 2 by 2 blocks with seeded one-step ramp changes inside the body, so a boss reads as drawn at its size and not enlarged. */
export function enlarge(px, canvas, seed = 'boss') {
  const f = canvas / px.w, out = new Pixels(canvas, canvas, px.palette), r = rng(`${seed}:enlarge`);
  for (let y = 0; y < canvas; y++) for (let x = 0; x < canvas; x++) out.set(x, y, px.get(Math.floor(x / f), Math.floor(y / f)));
  const base = out.clone();
  for (let y = 1; y < canvas - 1; y++) for (let x = 1; x < canvas - 1; x++) {
    const v = base.get(x, y); if (!v || v === C.night) continue;
    if ([[1, 0], [-1, 0], [0, 1], [0, -1]].every(([i, j]) => base.get(x + i, y + j) && base.get(x + i, y + j) !== C.night) && r.chance(0.1)) out.set(x, y, stepColor(v, r.chance(0.5) ? 1 : -1));
  }
  return out;
}

export const SEGMENT_IDS = { 2: ['head', 'body'], 3: ['head', 'body', 'tail'], 4: ['head', 'chest', 'body', 'tail'], 5: ['head', 'chest', 'body', 'hips', 'tail'] };

/**
 * A boss in parts so a game can move the segments: from 2 (the head and the body) to 5 (`SEGMENT_IDS`), each a canvas-sized layer with an offset of 0, so laying them
 * together gives the still exactly. The head is the first 38 percent along the long axis, the rest is cut evenly; a part's `joint` is the cut it hangs from.
 */
export function bossParts(px, horizontal, segments = 2) {
  const ids = SEGMENT_IDS[segments]; if (!ids) throw new Error(`a boss has 2 to 5 segments, not ${segments}`);
  const b = px.bbox(); if (!b) return [];
  const start = horizontal ? b.x : b.y, len = horizontal ? b.w : b.h, cuts = [Math.round(len * 0.38)];
  for (let i = 1; i < segments - 1; i++) cuts.push(Math.round(len * 0.38 + (len * 0.62 * i) / (segments - 1)));
  const at = (x, y) => { const d = (horizontal ? x : y) - start; let k = 0; while (k < cuts.length && d >= cuts[k]) k++; return k; };
  const mid = horizontal ? (n) => [start + n, b.y + (b.h >> 1)] : (n) => [b.x + (b.w >> 1), start + n];
  return ids.map((id, k) => ({ id, frame: px.map((x, y) => (at(x, y) === k ? px.get(x, y) : 0)), x: 0, y: 0, joint: mid(cuts[Math.max(0, k - 1)]) }));
}
