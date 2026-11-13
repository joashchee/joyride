// Golden hashes: every registered variant's MIDI bytes, hashed. Output must not change by accident, so a recipe change that alters
// a composition fails here until the new music is accepted on purpose with `UPDATE_GOLDEN=1 npm test` (and the changelog says
// which compositions changed). MIDI is all integers, so the hashes are the same on every machine; audio is not hashed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { RECIPES, compose } from '../recipes/index.js';
import { toMidi } from '../midi.js';

const FILE = fileURLToPath(new URL('./golden.json', import.meta.url));
const current = () => {
  const out = {};
  for (const [id, r] of Object.entries(RECIPES)) r.variants().forEach((v, i) => { out[`${id}#${i} ${JSON.stringify(v)}`] = createHash('sha256').update(toMidi(compose(id, v).score)).digest('hex').slice(0, 16); });
  return out;
};

test('every variant composes the same notes as the golden record', () => {
  const now = current();
  if (process.env.UPDATE_GOLDEN) { writeFileSync(FILE, JSON.stringify(now, null, 2) + '\n'); return; }
  const golden = JSON.parse(readFileSync(FILE, 'utf8'));
  const changed = Object.keys(now).filter((k) => golden[k] !== now[k]), gone = Object.keys(golden).filter((k) => !(k in now));
  assert.deepEqual([changed, gone], [[], []], 'compositions changed or disappeared; accept on purpose with UPDATE_GOLDEN=1 npm test, and say so in the changelog');
});
