// Golden hashes: every registered variant's clip, as written, hashed. Output must not change by accident, so a recipe change that
// alters a motion fails here until the new motion is accepted on purpose with `UPDATE_GOLDEN=1 npm test` (and the changelog says which
// motions changed).
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { RECIPES, animate } from '../recipes/index.js';
import { writeClip } from '../clip.js';

const FILE = fileURLToPath(new URL('./golden.json', import.meta.url));

const current = () => {
  const out = {};
  for (const [id, r] of Object.entries(RECIPES)) for (const v of r.variants()) out[`${id} ${JSON.stringify(v)}`] = createHash('sha256').update(writeClip(animate(id, v))).digest('hex').slice(0, 16);
  return out;
};

test('every variant makes the same clip as the golden record', () => {
  const now = current();
  if (process.env.UPDATE_GOLDEN) { writeFileSync(FILE, JSON.stringify(now, null, 1) + '\n'); return; }
  const golden = JSON.parse(readFileSync(FILE, 'utf8'));
  const changed = Object.keys(golden).filter((k) => now[k] !== golden[k]);
  assert.deepEqual(changed, [], `${changed.length} clip(s) changed; accept on purpose with UPDATE_GOLDEN=1`);
  assert.deepEqual(Object.keys(now).filter((k) => !(k in golden)), [], 'new variants need golden hashes (UPDATE_GOLDEN=1)');
});
