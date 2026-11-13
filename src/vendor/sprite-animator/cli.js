#!/usr/bin/env node
// The animation tools. Output goes only where --out or --dir says.
//   animate kits [--strict]                       what the reference kits ask for, and what can be made
//   animate types [type]                          the motion types: their Painter recipes, channels and presets
//   animate variants [recipe]                     every variant of a recipe (or all), linted
//   animate show <recipe> [k=v ...]               a clip as a table of its poses
//   animate make <recipe> [k=v ...] --out f.json  a clip as an animator-clip file
//   animate lint <recipe [k=v ...] | file.json>   the motion rules as checks
//   animate tempo <recipe> [k=v ...] --bpm 120    frame lengths and event ticks on a beat (--steps-per-beat 1, --beats n)
//   animate export <recipe> [k=v ...] --dir d     the clip file plus a provenance record
import { parseArgs } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { animate, RECIPES } from './recipes/index.js';
import { coverage } from './kits/index.js';
import { lintClip } from './lint.js';
import { writeClip, readClip, durations } from './clip.js';
import { tempo } from './tempo.js';
import { MOTION_TYPES, MOTION_OF } from './types.js';

const { values, positionals } = parseArgs({ allowPositionals: true, options: { out: { type: 'string' }, dir: { type: 'string' }, strict: { type: 'boolean' }, bpm: { type: 'string' }, 'steps-per-beat': { type: 'string' }, beats: { type: 'string' } } });
const [cmd, target, ...rest] = positionals;
const fail = (m) => { console.error(m); process.exit(2); };
const parse = (kv) => Object.fromEntries(kv.map((s) => { const [k, ...v] = s.split('='); const x = v.join('='); return [k, x === 'true' ? true : x === 'false' ? false : x !== '' && !isNaN(Number(x)) ? Number(x) : x]; }));
const make = (id, params) => { try { return animate(id, params); } catch (e) { return fail(e.message); } };
const need = (v, what) => v ?? fail(`${cmd} needs ${what}`);
const report = (c) => { const found = lintClip(c); for (const f of found) console.log(`${f.level} ${f.rule} ${f.message}`); return found.filter((f) => f.level === 'error').length; };

if (cmd === 'kits') {
  let errors = 0;
  for (const k of coverage()) {
    console.log(`${k.title} (${k.doc}): ${Object.entries(k.totals).map(([s, n]) => `${n} ${s}`).join(', ')}`);
    for (const r of k.rows) { console.log(`  ${r.status.padEnd(14)} ${r.id}${r.note ? ` - ${r.note}` : ''}`); for (const p of r.problems) { console.log(`    problem: ${p}`); errors++; } }
  }
  if (values.strict && errors) process.exit(1);
} else if (cmd === 'types') {
  const known = Object.keys(MOTION_TYPES);
  if (target && !MOTION_TYPES[target]) fail(`unknown motion type "${target}"; known: ${known.join(', ')}`);
  for (const [id, t] of Object.entries(MOTION_TYPES)) {
    if (target && target !== id) continue;
    console.log(`${id}: ${t.describe}\n  Painter recipes: ${Object.entries(MOTION_OF).filter(([, v]) => v === id).map(([k]) => k).join(', ')}\n  channels: ${Object.entries(t.channels).map(([k, v]) => (v === true ? k : `${k} ${v}`)).join(', ')}`);
    for (const [name, p] of Object.entries(t.presets)) console.log(`  ${id}.${name}  ${p.describe} (${Object.entries(p.params).map(([k, v]) => `${k}=${v}`).join(' ')})`);
  }
} else if (cmd === 'variants') {
  let errors = 0;
  for (const [id, r] of Object.entries(RECIPES)) {
    if (target && target !== id) continue;
    for (const v of r.variants()) { const c = make(id, v); console.log(`${id} ${JSON.stringify(v)}: ${c.frames.length} frames`); errors += report(c); }
  }
  if (errors) process.exit(1);
} else if (cmd === 'show') {
  const c = make(need(target, 'a recipe'), parse(rest)), ms = durations(c);
  console.log(`${c.id}: ${c.frames.length} frames at ${c.fps} fps, ${c.loop ? 'loop' : 'one-shot'}${c.wrap ? ', wraps' : ''}`);
  c.frames.forEach((f, i) => console.log(`${String(i).padStart(2)}  ${String(ms[i]).padStart(4)}ms  dx ${String(f.dx).padStart(2)} dy ${String(f.dy).padStart(2)} lift ${f.lift}  legs ${String(f.legs).padEnd(5)} lean ${f.lean} squat ${f.squat} drag ${f.drag} sway ${String(f.sway).padStart(2)} ${f.air ? 'air ' : '    '}${c.events.filter((e) => e.frame === i).map((e) => e.name).join(' ')}`));
} else if (cmd === 'make') {
  const c = make(need(target, 'a recipe'), parse(rest)), text = writeClip(c);
  if (values.out) writeFileSync(need(values.out, '--out'), text); else process.stdout.write(text);
} else if (cmd === 'lint') {
  const c = /\.json$/i.test(need(target, 'a recipe or a file')) ? readClip(readFileSync(target, 'utf8')) : make(target, parse(rest));
  process.exit(report(c) ? 1 : 0);
} else if (cmd === 'tempo') {
  const c = make(need(target, 'a recipe'), parse(rest));
  let t; try { t = tempo(c, { bpm: Number(need(values.bpm, '--bpm')), stepsPerBeat: Number(values['steps-per-beat'] ?? 1), beats: values.beats ? Number(values.beats) : undefined }); } catch (e) { fail(e.message); }
  console.log(`${c.id} at ${t.bpm} bpm: ${t.beats} beats, ${t.totalMs} ms, ${t.totalTicks} ticks`);
  t.ms.forEach((m, i) => console.log(`${String(i).padStart(2)}  ${String(t.starts[i]).padStart(5)}ms  ${String(m).padStart(4)}ms`));
  for (const e of t.events) console.log(`event ${e.name} frame ${e.frame} at ${e.ms}ms tick ${e.tick}`);
} else if (cmd === 'export') {
  const dir = need(values.dir, '--dir'), c = make(need(target, 'a recipe'), parse(rest));
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${c.id}.clip.json`), writeClip(c));
  writeFileSync(join(dir, `${c.id}.provenance.json`), JSON.stringify({ provenance: 'Project-created', engine: 'sprite-animator', recipe: c.spec?.recipe ?? c.id, params: c.spec?.params ?? {}, note: 'Generated by a seeded recipe; no third-party motion data.' }, null, 1) + '\n');
  console.log(`wrote ${join(dir, `${c.id}.clip.json`)}`);
} else {
  console.error('usage: animate kits | types [type] | variants [recipe] | show <recipe> | make <recipe> --out f | lint <recipe|file> | tempo <recipe> --bpm n | export <recipe> --dir d\nrecipes: ' + Object.keys(RECIPES).join(', '));
  process.exit(2);
}
