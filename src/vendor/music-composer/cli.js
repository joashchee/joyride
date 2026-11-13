#!/usr/bin/env node
// The composing tools. Output goes only where --out or --dir says.
//   compose kits [--strict]                      what the reference kits ask for, and what can be composed
//   compose make <recipe> [k=v ...] --out f.mid  one composition, as .mid, .wav or .ntt (by extension; --rows sets rows a beat, --engine opl|midi|sample fills the instruments, --reverb 0..1 adds a reverb)
//   compose show <recipe> [k=v ...] [--track t]  a piano roll in the terminal
//   compose variants <recipe> --dir d            every variant of a recipe as .mid files
//   compose lint <recipe [k=v ...] | file.mid>   the craft rules as checks
//   compose bake <recipe> [k=v ...] --dir d     instruments.json for Neumetik to render, and the .ntt (baked from d/wavs when they are there)
//   compose export <recipe> [k=v ...] --dir d    .mid and .wav plus a provenance record (--ntt adds the Neumetik tracker file)
//   compose cues <sheet.json> --dir d           every cue of a cue sheet as .ntt and .mid plus cues.json and play.txt (an ntt_play --script run of the switching; layers, sections, switch points; --engine sets the instruments; --audio bakes <id>.wav, or one <id>.<layer>.wav stem a layer, for createCuePlayer; --rate sets the sample rate)
//   compose notate <recipe [k=v ...] | file.mid | file.cmp> [--out f.cmp]   the music as text notation (printed, or written to --out)
// Anywhere a <recipe> is taken, a notation file (name.cmp) works too.
import { parseArgs } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { compose, RECIPES, RECIPE_IDS } from './recipes/index.js';
import { coverage, KITS } from './kits/index.js';
import { lint } from './lint.js';
import { toMidi, fromMidi } from './midi.js';
import { toModule } from './tracker.js';
import { toNtt } from './ntt.js';
import { toManifest, bake } from './bake.js';
import { render } from './synth.js';
import { toWav } from './wav.js';
import { pianoRoll } from './text.js';
import { toNotation, parseNotation } from './notation.js';
import { composeCues, playScript, cueAudio } from './cues.js';

const { values, positionals } = parseArgs({ allowPositionals: true, options: { out: { type: 'string' }, dir: { type: 'string' }, strict: { type: 'boolean' }, ntt: { type: 'boolean' }, rows: { type: 'string' }, engine: { type: 'string' }, reverb: { type: 'string' }, source: { type: 'string' }, track: { type: 'string' }, rate: { type: 'string' }, audio: { type: 'boolean' } } });
const [cmd, target, ...rest] = positionals;
const fromFile = (f) => { const score = parseNotation(readFileSync(f, 'utf8')); return { id: slugOf(f), score, loop: false, sections: { all: { from: 0, to: score.length } }, params: {} }; };
const slugOf = (f) => f.replace(/^.*\//, '').replace(/\.[^.]*$/, '');
const make = (id, params) => { try { return /\.cmp$/i.test(id) ? fromFile(id) : compose(id, params); } catch (e) { console.error(e.message); process.exit(2); } };
const parse = (kv) => Object.fromEntries(kv.map((s) => { const [k, ...v] = s.split('='); const x = v.join('='); return [k, x === 'true' ? true : x === 'false' ? false : x !== '' && !isNaN(Number(x)) ? Number(x) : x]; }));
const need = (v, flag) => { if (!v) { console.error(`${cmd} needs ${flag}`); process.exit(2); } return v; };
// Until samples are filled in (docs/composer-plan.md, phases 4 and 5) an .ntt has the notes, patterns and instrument slots but no sound.
const nttNote = (r) => `ntt: ${r.channels} channels, ${r.unique} pattern(s), speed ${r.speed} tempo ${r.tempo}; ${r.moved} note(s) moved to the row grid, ${r.dropped.length} dropped${r.fallbacks.map((f) => `; ${f.track}: ${f.reason}, so ${f.used}`).join('')}; ${values.engine === 'sample' ? 'every instrument carries its own sample, so it plays in any tracker player' : values.engine ? 'MIDI-out instruments sound where a host plays MIDI: the Neumetik app, or tools/neumetik/ntt_play.rs' : 'instrument slots have no sound yet (use --engine sample), so it plays silent'}`;
const slug = (s) => s.replace(/[^a-z0-9.-]/gi, '_');
const ntt = (a) => { const { module, report } = toModule(a.score, { rowsPerBeat: Number(values.rows ?? 4), sections: a.sections, engine: values.engine ?? 'none', effects: values.reverb ? [{ kind: 'reverb', mix: Number(values.reverb) }] : [] }); return { bytes: toNtt(module), report }; };
const wav = (a) => toWav(render(a.score, { sampleRate: Number(values.rate ?? 22050), loop: a.loop }), Number(values.rate ?? 22050));

if (cmd === 'kits') {
  let problems = 0;
  for (const k of coverage()) {
    console.log(`\n${k.title}\n  ${k.doc}\n  ${Object.entries(k.totals).filter(([, n]) => n).map(([s, n]) => `${n} ${s}`).join(', ')}`);
    for (const r of k.rows) {
      console.log(`  ${r.status.padEnd(14)} ${r.id}${r.engine ? ` [${r.engine}]` : ''}${r.note ? ` (${r.note})` : ''}`);
      for (const p of r.problems) { console.log(`      PROBLEM ${p}`); problems++; }
      for (const w of r.warnings) console.log(`      warn ${w}`);
    }
  }
  const gaps = coverage().flatMap((k) => k.rows.filter((r) => r.status === 'gap').map((r) => `${k.id}/${r.id}`));
  console.log(`\n${KITS.length} kits. Gaps (nothing composes these yet): ${gaps.join(', ') || 'none'}`);
  if (values.strict && problems) process.exit(1);
} else if (cmd === 'make') {
  const a = make(need(target, '<recipe>'), parse(rest)), out = need(values.out, '--out');
  const isNtt = /\.ntt$/i.test(out), made = isNtt ? ntt(a) : null;
  writeFileSync(out, isNtt ? made.bytes : /\.wav$/i.test(out) ? wav(a) : toMidi(a.score));
  if (made) console.log(nttNote(made.report));
  console.log(`${a.id}: ${a.score.bars} bar(s) at ${a.score.bpm} bpm, ${a.score.count()} notes -> ${out}`);
} else if (cmd === 'show') {
  const a = make(need(target, '<recipe>'), parse(rest));
  const tracks = values.track ? [values.track] : [...a.score.tracks.keys()];
  console.log(`${a.id}: ${a.score.key} ${a.score.scale}, ${a.score.bpm} bpm, ${a.score.beats}/4, ${a.score.bars} bars`);
  for (const t of tracks) console.log(`\n${t}\n${pianoRoll(a.score, t)}`);
} else if (cmd === 'variants') {
  const r = RECIPES[need(target, '<recipe>')]; if (!r) { console.error(`unknown recipe; known: ${RECIPE_IDS.join(', ')}`); process.exit(2); }
  const dir = need(values.dir, '--dir');
  mkdirSync(dir, { recursive: true });
  r.variants().forEach((v, i) => { const a = make(target, v); const f = `${slug(target)}.${i}.mid`; writeFileSync(join(dir, f), toMidi(a.score)); console.log(`${f}  ${JSON.stringify(v)}`); });
} else if (cmd === 'lint') {
  const isFile = /\.midi?$/i.test(need(target, '<recipe|file.mid>'));
  const found = isFile ? lint(fromMidi(readFileSync(target)), { name: target }) : (() => { const a = make(target, parse(rest)); return lint(a.score, { name: a.id, loop: a.loop, ...a.lint }); })();
  for (const f of found) console.log(`${f.level} [${f.rule}] ${f.message}`);
  console.log(found.length ? `${found.length} finding(s)` : 'no findings');
  if (found.some((f) => f.level === 'error')) process.exit(1);
} else if (cmd === 'bake') {
  // Two steps with a Neumetik-side tool between them: this writes d/instruments.json, the tool renders it to d/wavs/<id>.wav, and running
  // this again puts those waveforms in the .ntt (an instrument with no WAV yet keeps Composer's own sound).
  const a = make(need(target, '<recipe>'), parse(rest)), dir = need(values.dir, '--dir'), name = slug(a.id), wavDir = join(dir, 'wavs');
  const engine = Object.fromEntries([...a.score.tracks.values()].map((t) => [t.name, { engine: 'bake', ...(t.drum || !values.source ? {} : { source: values.source }) }]));
  const { module, report } = toModule(a.score, { rowsPerBeat: Number(values.rows ?? 4), sections: a.sections, engine, effects: values.reverb ? [{ kind: 'reverb', mix: Number(values.reverb) }] : [] });
  const manifest = toManifest(module, { rate: Number(values.rate ?? 44100) });
  const wavs = {};
  for (const s of manifest.slots) { try { wavs[s.id] = readFileSync(join(wavDir, `${s.id}.wav`)); } catch { /* not rendered yet */ } }
  const { baked, missing } = bake(module, wavs);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'instruments.json'), JSON.stringify(manifest, null, 2));
  writeFileSync(join(dir, `${name}.ntt`), toNtt(module));
  console.log(`${name}.ntt: ${baked.length} of ${manifest.slots.length} instrument(s) baked${missing.length ? `; Composer's own sound for ${missing.join(', ')}` : ''} (${report.channels} channels, ${report.unique} pattern(s))`);
  if (missing.length) console.log(`render the rest: cargo run --release --example render_manifest -- ${join(dir, 'instruments.json')} ${wavDir}   (tools/neumetik/render_manifest.rs, in Neumetik)`);
} else if (cmd === 'cues') {
  const dir = need(values.dir, '--dir');
  let built;
  try { built = composeCues(JSON.parse(readFileSync(need(target, '<sheet.json>'), 'utf8')), { engine: values.engine ?? 'sample', rowsPerBeat: Number(values.rows ?? 4) }); } catch (e) { console.error(e.message); process.exit(2); }
  mkdirSync(dir, { recursive: true });
  for (const c of built.cues) { writeFileSync(join(dir, c.entry.file), c.ntt); writeFileSync(join(dir, c.entry.midi), c.midi); }
  writeFileSync(join(dir, 'cues.json'), JSON.stringify(built.manifest, null, 2) + '\n');
  if (built.manifest.cues.some((c) => c.kind === 'loop')) writeFileSync(join(dir, 'play.txt'), playScript(built.manifest));
  if (values.audio) for (const c of built.cues) for (const [file, bytes] of Object.entries(cueAudio(c, { sampleRate: Number(values.rate ?? 22050) }))) writeFileSync(join(dir, file), bytes);
  for (const e of built.manifest.cues) console.log(`${e.id.padEnd(16)} ${e.kind.padEnd(8)} ${e.bars} bars, ${(e.lengthMs / 1000).toFixed(1)} s${e.tailMs ? ` + ${(e.tailMs / 1000).toFixed(1)} s tail` : ''}; ${Object.keys(e.layers).length} layer(s), ${e.sections.length} section(s)${e.warnings.length ? `; ${e.warnings.length} warning(s)` : ''}`);
  console.log(`${built.cues.length} cue(s) -> ${dir} (cues.json says how to play them)`);
} else if (cmd === 'notate') {
  const text = toNotation(/\.midi?$/i.test(need(target, '<recipe|file.mid|file.cmp>')) ? fromMidi(readFileSync(target)) : make(target, parse(rest)).score);
  if (values.out) { writeFileSync(values.out, text); console.log(`${target} -> ${values.out}`); } else process.stdout.write(text);
} else if (cmd === 'export') {
  const id = need(target, '<recipe>'), params = parse(rest), a = make(id, params), dir = need(values.dir, '--dir'), name = slug(a.id);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${name}.mid`), toMidi(a.score));
  writeFileSync(join(dir, `${name}.wav`), wav(a));
  // Provenance: the recipe and params are the evidence that this music was composed by the engine, not taken from anywhere.
  writeFileSync(join(dir, `${name}.json`), JSON.stringify({ provenance: 'Project-created', recipe: id, params, sections: a.sections, loop: a.loop, bpm: a.score.bpm, key: a.score.key, scale: a.score.scale, bars: a.score.bars }, null, 2));
  if (values.ntt) { const n = ntt(a); writeFileSync(join(dir, `${name}.ntt`), n.bytes); console.log(nttNote(n.report)); }
  console.log(`${name}.mid, ${name}.wav, ${values.ntt ? `${name}.ntt, ` : ''}${name}.json -> ${dir}`);
} else {
  console.error('usage: compose <kits|make|show|variants|lint|bake|export|cues|notate> ...  (see the header of cli.js)');
  process.exit(2);
}
