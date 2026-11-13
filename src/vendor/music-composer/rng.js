// Seeded randomness. A composition never calls Math.random: the same recipe, params and seed give the same notes.
export function hashSeed(seed) {
  let h = 2166136261;
  for (const ch of String(seed)) { h ^= ch.codePointAt(0); h = Math.imul(h, 16777619); }
  return h >>> 0;
}
export function rng(seed) {
  const base = String(seed);
  let s = hashSeed(base);
  const next = () => { s = (s + 0x6d2b79f5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  return {
    next,
    int: (n) => Math.floor(next() * n),
    range: (a, b) => a + Math.floor(next() * (b - a + 1)),
    chance: (p) => next() < p,
    pick: (a) => a[Math.floor(next() * a.length)],
    /** A child stream keyed by the seed and the label alone, so adding a draw in one part of a recipe never shifts another part. */
    fork: (label) => rng(`${base}:${label}`),
  };
}
