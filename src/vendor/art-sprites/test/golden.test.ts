// Golden hashes: every registered variant's drawing, in every style, hashed. Output must not change by accident, so a recipe
// change that alters a drawing fails here until the new look is accepted on purpose with `UPDATE_GOLDEN=1 npm test`
// (and the changelog says which looks changed).
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { RECIPES, draw } from '../recipes/index.js';

const FILE = fileURLToPath(new URL('./golden.json', import.meta.url));

// A stable text form of any Art: sorted keys, typed arrays as lists, Pixels as their size, palette and data.
const walk = (v: any): any => {
  if (v === null || typeof v !== 'object') return typeof v === 'function' ? undefined : v;
  if (ArrayBuffer.isView(v)) return Array.from(v as any);
  if (Array.isArray(v)) return v.map(walk);
  if (v instanceof Map) return [...v].map(walk);
  if (v instanceof Set) return [...v].map(walk);
  return Object.fromEntries(Object.keys(v).sort().map((k) => [k, walk(v[k])]));
};
const hash = (art: any) => createHash('sha256').update(JSON.stringify(walk(art))).digest('hex').slice(0, 16);

const current = () => {
  const out: Record<string, string> = {};
  for (const [id, r] of Object.entries(RECIPES)) for (const v of r.variants()) {
    if (!r.ansiOnly) out[`${id} ${JSON.stringify(v)}`] = hash(draw(id, v));
    out[`text ${id} ${JSON.stringify(v)}`] = hash(draw(id, v, { style: 'text' }));
    out[`ansi ${id} ${JSON.stringify(v)}`] = hash(draw(id, v, { style: 'ansi' }));   // folded unless the recipe has a native drawing (ansiOnly recipes have only that)
  }
  return out;
};

test('pixel, ansi and text output is byte-identical to the golden hashes', () => {
  const now = current();
  if (process.env.UPDATE_GOLDEN) { writeFileSync(FILE, JSON.stringify(now, null, 1) + '\n'); return; }
  const golden = JSON.parse(readFileSync(FILE, 'utf8'));
  const changed = Object.keys(golden).filter((k) => now[k] !== golden[k]);
  assert.deepEqual(changed, [], `${changed.length} drawing(s) changed; accept on purpose with UPDATE_GOLDEN=1`);
  assert.deepEqual(Object.keys(now).filter((k) => !(k in golden)), [], 'new variants need golden hashes (UPDATE_GOLDEN=1)');
});
