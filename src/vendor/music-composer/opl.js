// Two-operator FM patches the OPL2 way (the AdLib's and Sound Blaster's chip), as the eleven bytes Scream Tracker 3 keeps an AdLib
// instrument in. The register layout is the chip's own, from its public datasheet (Yamaha YM3812 application manual):
//   0x20  tremolo 7, vibrato 6, sustained 5, key-scale rate 4, frequency multiplier 3..0
//   0x40  key-scale level 7..6, total level 5..0 (0 loudest, 63 quietest)
//   0x60  attack rate 7..4, decay rate 3..0          0x80  sustain level 7..4 (0 loudest), release rate 3..0
//   0xE0  waveform 1..0                               0xC0  feedback 3..1, connection 0 (0 modulates the carrier, 1 adds the two)
// The bytes go in the order S3M has them: modulator then carrier for 0x20, 0x40, 0x60, 0x80, the two waveforms, then 0xC0.
// A patch is described in these named fields so it reads like a sound, never as a row of hex. Names are generic, never an instrument maker's.
const field = (v, lo, hi, name) => { if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} ${v} is outside ${lo}..${hi}`); return v; };

const OPERATOR = { mult: 1, level: 0, ksl: 0, attack: 15, decay: 4, sustain: 2, release: 5, wave: 0, tremolo: false, vibrato: false, sustained: false, ksr: false };

function registers(spec, who) {
  const o = { ...OPERATOR, ...spec }, n = (k) => `${who} ${k}`;
  const mult = field(o.mult, 0, 15, n('mult')), level = field(o.level, 0, 63, n('level')), ksl = field(o.ksl, 0, 3, n('ksl'));
  const attack = field(o.attack, 0, 15, n('attack')), decay = field(o.decay, 0, 15, n('decay'));
  const sustain = field(o.sustain, 0, 15, n('sustain')), release = field(o.release, 0, 15, n('release')), wave = field(o.wave, 0, 3, n('wave'));
  return {
    r20: (o.tremolo ? 128 : 0) | (o.vibrato ? 64 : 0) | (o.sustained ? 32 : 0) | (o.ksr ? 16 : 0) | mult,
    r40: (ksl << 6) | level, r60: (attack << 4) | decay, r80: (sustain << 4) | release, wave,
  };
}

/** The eleven bytes of a patch: `{ mod, car, feedback (0..7), additive }`, each operator `{ mult, level, attack, decay, sustain, release, wave, ... }`. */
export function fmPatch({ mod = {}, car = {}, feedback = 0, additive = false } = {}) {
  const m = registers(mod, 'modulator'), c = registers(car, 'carrier');
  return [m.r20, c.r20, m.r40, c.r40, m.r60, c.r60, m.r80, c.r80, m.wave, c.wave, (field(feedback, 0, 7, 'feedback') << 1) | (additive ? 1 : 0)];
}

/** Named starting points; `fmPatch` of each is what a track gets. Levels: modulator level is how bright, carrier level how loud. */
export const OPL_PATCHES = {
  lead: { mod: { mult: 1, level: 24, attack: 15, decay: 6, sustain: 4, release: 6 }, car: { mult: 1, level: 2, attack: 15, decay: 3, sustain: 3, release: 6, sustained: true }, feedback: 3 },
  bass: { mod: { mult: 1, level: 30, attack: 15, decay: 8, sustain: 8, release: 8 }, car: { mult: 1, level: 0, attack: 15, decay: 5, sustain: 5, release: 8 }, feedback: 5 },
  pad: { mod: { mult: 2, level: 34, attack: 6, decay: 4, sustain: 2, release: 5, vibrato: true }, car: { mult: 1, level: 8, attack: 5, decay: 3, sustain: 2, release: 5, sustained: true, vibrato: true }, feedback: 1 },
  bell: { mod: { mult: 7, level: 22, attack: 15, decay: 5, sustain: 8, release: 6 }, car: { mult: 1, level: 4, attack: 15, decay: 4, sustain: 8, release: 5 }, feedback: 0 },
  pluck: { mod: { mult: 2, level: 20, attack: 15, decay: 9, sustain: 12, release: 9 }, car: { mult: 1, level: 2, attack: 15, decay: 7, sustain: 10, release: 9 }, feedback: 2 },
  organ: { mod: { mult: 2, level: 38, attack: 15, decay: 0, sustain: 0, release: 8, sustained: true }, car: { mult: 1, level: 6, attack: 15, decay: 0, sustain: 0, release: 8, sustained: true }, feedback: 4, additive: true },
  brass: { mod: { mult: 1, level: 28, attack: 10, decay: 4, sustain: 3, release: 6, sustained: true }, car: { mult: 1, level: 4, attack: 11, decay: 3, sustain: 2, release: 6, sustained: true }, feedback: 6 },
  blip: { mod: { mult: 1, level: 63, attack: 15, decay: 15, sustain: 15, release: 15 }, car: { mult: 1, level: 6, attack: 15, decay: 5, sustain: 4, release: 9, wave: 3 }, feedback: 0 },
};
export const OPL_PATCH_IDS = Object.keys(OPL_PATCHES);

/** The patch named `id` (see OPL_PATCH_IDS) as its eleven bytes. */
export function oplPatch(id) {
  const p = OPL_PATCHES[id];
  if (!p) throw new Error(`unknown FM patch "${id}"; known: ${OPL_PATCH_IDS.join(', ')}`);
  return fmPatch(p);
}
