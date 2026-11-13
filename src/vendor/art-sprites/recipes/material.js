// Crafting materials and currency (T10, 16x16): ores, herbs, hides, monster parts and crystals with a `variant` index, then coins of four denominations, a purse,
// keys, fragments, plans and quest tokens. Each is a mask of part ids (1 main, 2 second tone, 3 trim, 4 vein or gem) shaded by shade.js, like `item`.
import { Pixels } from '../pixels.js';
import { shade } from '../shade.js';
import { C, ramp } from '../palette.js';
import { rng } from '../rng.js';

const S = 16;
const COPPER = [C.umber, C.bronze, C.orange, C.tan];
const mk = () => new Pixels(S, S);

/** Each family has its own looks; `variant` is an index into them (it wraps), so a list of looks can grow without renumbering. */
export const MATERIAL_KINDS = {
  ore: {
    looks: [{ name: 'iron', vein: ramp('steel') }, { name: 'copper', vein: COPPER }, { name: 'silver', vein: ramp('white') }, { name: 'gold', vein: ramp('gold') }, { name: 'mithril', vein: ramp('glass') }],
    draw(m, v) { m.poly([[2, 11], [3, 6], [7, 3], [12, 4], [14, 9], [13, 13], [5, 14]], 1); m.poly([[8, 4], [12, 5], [13, 9], [9, 8]], 2); for (const [x, y] of [[5, 8], [9, 10], [7, 6], [11, 11]].slice(0, 3 + (v % 2))) m.rect(x, y, 2, 2, 4); m.set(4, 11, 4); },
    parts: (look) => ({ 1: ramp('stone'), 2: ramp('stone').map((c, i, r) => r[Math.min(3, i + 1)]), 4: look.vein, 3: ramp('stone') }),
  },
  herb: {
    looks: [{ name: 'green', leaf: ramp('green'), bloom: ramp('red') }, { name: 'moss', leaf: ramp('moss'), bloom: ramp('gold') }, { name: 'violet', leaf: ramp('green'), bloom: ramp('purple') }, { name: 'frost', leaf: ramp('ice'), bloom: ramp('white') }],
    draw(m, v) {
      m.line(8, 15, 8, 6, 3); m.line(9, 15, 9, 7, 3);
      m.poly([[8, 12], [3, 9], [2, 11], [7, 13]], 1); m.poly([[9, 10], [14, 7], [14, 9], [9, 12]], 1); m.poly([[8, 7], [4, 3], [6, 3], [9, 6]], 2);
      if (v % 2 === 0) m.ellipse(9.5, 4.5, 2, 2, 4); else { m.rect(9, 2, 2, 2, 4); m.rect(11, 4, 2, 2, 4); m.rect(7, 4, 2, 2, 4); }
    },
    parts: (look) => ({ 1: look.leaf, 2: look.leaf.map((c, i, r) => r[Math.max(0, i - 1)]), 3: look.leaf, 4: look.bloom }),
  },
  hide: {
    looks: [{ name: 'leather', ramp: ramp('leather') }, { name: 'pale', ramp: ramp('bone') }, { name: 'grey', ramp: ramp('iron') }, { name: 'red', ramp: ramp('red') }],
    draw(m, v) { m.poly([[3, 3], [6, 2], [8, 4], [10, 2], [13, 3], [14, 7], [12, 9], [14, 13], [10, 14], [8, 12], [6, 14], [2, 13], [4, 9], [2, 7]], 1); m.poly([[6, 5], [10, 5], [11, 9], [8, 11], [5, 9]], 2); if (v % 2) for (const [x, y] of [[7, 7], [9, 8], [8, 9]]) m.set(x, y, 3); else m.line(8, 4, 8, 12, 3); },
    parts: (look) => ({ 1: look.ramp, 2: look.ramp.map((c, i, r) => r[Math.max(0, i - 1)]), 3: look.ramp.map((c, i, r) => r[Math.max(0, i - 2)]) }),
  },
  part: {
    looks: [{ name: 'fang', main: ramp('bone') }, { name: 'claw', main: ramp('bone') }, { name: 'horn', main: ramp('sand') }, { name: 'eye', main: ramp('glass') }, { name: 'scale', main: ramp('green') }],
    draw(m, v) {
      const i = v % 5;
      if (i === 0) { m.poly([[3, 3], [10, 3], [8, 9], [6, 14], [5, 9]], 1); m.rect(3, 3, 7, 2, 2); m.line(6, 6, 6, 11, 3); }
      else if (i === 1) { m.poly([[3, 14], [3, 8], [6, 3], [12, 2], [8, 6], [7, 14]], 1); m.line(5, 12, 7, 5, 3); m.line(8, 6, 11, 3, 2); }
      else if (i === 2) { m.poly([[3, 14], [4, 9], [8, 4], [13, 2], [11, 7], [8, 10], [8, 14]], 1); m.line(5, 12, 6, 9, 2).line(7, 9, 10, 5, 2); m.rect(3, 13, 6, 2, 3); }
      else if (i === 3) { m.ellipse(8, 8, 6, 5, 1); m.ellipse(8, 8, 3, 3, 3); m.rect(7, 6, 2, 4, 4); m.set(5, 5, 2); }
      else { m.poly([[8, 2], [13, 5], [13, 11], [8, 14], [3, 11], [3, 5]], 1); m.line(8, 2, 8, 14, 3); m.line(3, 5, 13, 11, 2); m.line(13, 5, 3, 11, 2); }
    },
    parts: (look) => ({ 1: look.main, 2: ramp('stone'), 3: ramp('iron'), 4: ramp('red') }),
  },
  crystal: {
    looks: [{ name: 'clear', c: ramp('glass') }, { name: 'ruby', c: ramp('red') }, { name: 'amethyst', c: ramp('purple') }, { name: 'emerald', c: ramp('green') }, { name: 'sun', c: ramp('gold') }],
    draw(m, v) { m.poly([[8, 1], [11, 5], [10, 14], [6, 14], [5, 5]], 1); m.poly([[3, 7], [5, 5], [6, 14], [2, 14]], 2); m.poly([[11, 6], [14, 8], [14, 14], [10, 14]], 2); m.line(8, 2, 8, 13, 3); if (v % 2) m.set(8, 0, 3); },
    parts: (look) => ({ 1: look.c, 2: look.c.map((c, i, r) => r[Math.max(0, i - 1)]), 3: ramp('white') }),
  },
};
export const MATERIAL_IDS = Object.keys(MATERIAL_KINDS);

/** Draw a crafting material. `variant` picks the look within the family (ore: iron, copper, silver, gold, mithril; herb: green, moss, violet, frost; hide: leather, pale, grey, red;
 *  part: fang, claw, horn, eye, scale; crystal: clear, ruby, amethyst, emerald, sun); it wraps. The seed scatters a few specks on the main tone. */
export function drawMaterial({ kind = 'ore', variant = 0, seed = 'material' } = {}) {
  const def = MATERIAL_KINDS[kind]; if (!def) throw new Error(`unknown material "${kind}"; known: ${MATERIAL_IDS.join(', ')}`);
  if (!Number.isInteger(variant) || variant < 0) throw new Error(`a material variant is a whole number from 0, not ${variant}`);
  const v = variant % def.looks.length, look = def.looks[v], m = mk(), r = rng(`${seed}:${kind}:${v}`);
  def.draw(m, v);
  const out = shade(m, def.parts(look)), cells = []; for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) if (m.get(x, y) === 1) cells.push([x, y]);
  for (let i = 0; i < 2; i++) { const [x, y] = r.pick(cells); out.set(x, y, C.white); }
  return { id: `material.${kind}.${look.name}`, frames: [out], fps: 0, loop: false, spec: { kind, variant: v }, mask: m, anchors: { root: [8, 14] } };
}

export const DENOMINATIONS = { copper: COPPER, silver: ramp('white'), gold: ramp('gold'), platinum: ramp('glass') };
export const CURRENCY_KINDS = ['coin', 'purse', 'key', 'fragment', 'plan', 'token'];
export const KEY_LOOKS = ['bronze', 'silver', 'gold', 'skeleton'];
const KEY_RAMPS = { bronze: COPPER, silver: ramp('white'), gold: ramp('gold'), skeleton: ramp('bone') };

/**
 * Money and the things that stand for it. `coin` takes a `denomination` (copper, silver, gold, platinum: a different ridge and centre mark as well as a colour) and a
 * `stack` of 1 to 3; `purse` is a bag whose size grows with the denomination; `key` takes a look (`bronze`, `silver`, `gold`, `skeleton`); `fragment` is a broken piece of
 * a larger thing in the denomination's metal; `plan` is a rolled blueprint (a recipe or a plan); `token` is a quest token, a disc with a star.
 */
export function drawCurrency({ kind = 'coin', denomination = 'gold', stack = 1, look = 'bronze' } = {}) {
  if (!CURRENCY_KINDS.includes(kind)) throw new Error(`unknown currency "${kind}"; known: ${CURRENCY_KINDS.join(', ')}`);
  const d = DENOMINATIONS[denomination]; if (!d) throw new Error(`unknown denomination "${denomination}"; known: ${Object.keys(DENOMINATIONS).join(', ')}`);
  if (!Number.isInteger(stack) || stack < 1 || stack > 3) throw new Error('a coin stack is 1 to 3');
  const m = mk(), tier = Object.keys(DENOMINATIONS).indexOf(denomination);
  let parts = { 1: d, 2: d.map((c, i, r) => r[Math.max(0, i - 1)]), 3: d, 4: ramp('red') };
  if (kind === 'coin') {
    for (let i = stack - 1; i >= 0; i--) {
      const x = 8 - (stack - 1) * 1.5 + i * 3, y = 12 - i * 2.5 - (stack === 1 ? 2 : 0);
      m.ellipse(x, y, 5.5, 4.5, 2).ellipse(x, y - 1, 5.5, 4.5, 1);
      // The mark on the face: copper a dot, silver a bar, gold a diamond, platinum a ring, so the denominations differ by shape as well as colour.
      const fx = Math.round(x), fy = Math.round(y - 1);
      if (i === stack - 1) { if (tier === 0) m.set(fx, fy, 3); else if (tier === 1) m.rect(fx - 1, fy, 3, 1, 3); else if (tier === 2) { m.set(fx, fy - 1, 3).set(fx - 1, fy, 3).set(fx + 1, fy, 3).set(fx, fy + 1, 3); } else { m.ellipse(fx + 0.5, fy + 0.5, 1.6, 1.6, 3); m.set(fx, fy, 1); } }
    }
  } else if (kind === 'purse') {
    const w = 3 + tier; m.ellipse(8, 10, 4 + tier * 0.7, 4, 1); m.poly([[8 - 2, 5], [8 + 2, 5], [8 + w / 2, 8], [8 - w / 2, 8]], 1); m.rect(6, 4, 4, 2, 2); m.rect(7, 3, 2, 1, 3); m.set(8, 10, 4);
    parts = { 1: ramp('leather'), 2: ramp('leather'), 3: d, 4: d };
  } else if (kind === 'key') {
    const kr = KEY_RAMPS[look]; if (!kr) throw new Error(`unknown key "${look}"; known: ${KEY_LOOKS.join(', ')}`);
    if (look === 'skeleton') { m.ellipse(5, 5, 3.5, 3.5, 1); m.ellipse(5, 5, 1.5, 1.5, 0); m.set(3, 3, 1); m.line(7, 7, 14, 14, 1).line(8, 7, 14, 13, 1); m.rect(11, 12, 2, 3, 1); m.rect(13, 10, 2, 2, 1); }
    else { m.ellipse(5, 5, 3.5, 3.5, 1); m.ellipse(5, 5, 1.5, 1.5, 0); m.line(7, 7, 13, 13, 1); m.line(8, 7, 14, 13, 1); m.set(11, 14, 1).set(13, 12, 1); m.set(5, 2, 3); }
    parts = { 1: kr, 2: kr, 3: ramp('white'), 4: ramp('red') };
  } else if (kind === 'fragment') {
    m.poly([[3, 4], [9, 2], [13, 6], [11, 10], [12, 14], [6, 12], [2, 9]], 1); m.poly([[9, 2], [13, 6], [11, 10], [9, 8]], 2); m.line(3, 4, 6, 12, 3); m.line(6, 7, 11, 10, 3);
  } else if (kind === 'plan') {
    m.rect(2, 3, 12, 10, 1); m.rect(2, 3, 12, 1, 2); m.rect(2, 12, 12, 1, 2); m.ellipse(2, 8, 1.5, 5, 2); m.ellipse(14, 8, 1.5, 5, 2); m.frame(5, 5, 6, 5, 3); m.line(5, 7, 10, 7, 3); m.line(8, 5, 8, 9, 3);
    parts = { 1: ramp('white'), 2: ramp('bone'), 3: ramp('blue'), 4: ramp('red') };
  } else {
    m.ellipse(8, 8, 6.5, 6.5, 2); m.ellipse(8, 8, 5, 5, 1); for (const [x, y] of [[8, 4], [9, 7], [12, 7], [10, 9], [11, 12], [8, 10], [5, 12], [6, 9], [4, 7], [7, 7]]) m.set(x, y, 3);
    parts = { 1: ramp('purple'), 2: ramp('iron'), 3: ramp('gold'), 4: ramp('red') };
  }
  return { id: `currency.${kind}${kind === 'key' ? `.${look}` : kind === 'coin' || kind === 'purse' || kind === 'fragment' ? `.${denomination}` : ''}`, frames: [shade(m, parts)], fps: 0, loop: false, spec: { kind, denomination, stack, look }, anchors: { root: [8, 14] }, mask: m };
}
