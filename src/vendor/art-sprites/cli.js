#!/usr/bin/env node
// The sprite drawing tools. Output goes only where --out or --dir says.
//   sprites kits [--strict]                      what the reference kits ask for, and what can be drawn
//   sprites draw <recipe> [k=v ...] --out f.png  one drawing (all frames side by side), at --scale
//   sprites draw <recipe> --style ansi --out f.ans|f.png|f.txt   an ANSI file, its render (VGA font), or its characters
//   sprites sheet <recipe> --out f.png           every variant of a recipe on one sheet
//   sprites draw <recipe> --style ansi --clip <name> --out f.ans   the clip as an ANSImation (--baud 2400): f.still.ans beside it is the last frame
//   sprites draw <recipe> --style text --out f.txt|f.png   plain 7-bit ASCII (no colour), or its render
//   sprites convert <image.png> --out f.ans|png|txt   an image to cells for study (--columns 80, --colors 16|256, --target); never engine art
//   sprites lint <recipe [k=v ...] | file.png | file.ans>   the learning kit's fundamentals as checks (--style ansi, or a .ans: the ANSI rules)
//   sprites study <ref.png> <mine.png> --out f   the kit's compare step: side by side, with numbers
//   sprites export <recipe> [k=v ...] --dir d    PNGs per frame and clip, plus a provenance record
//   sprites pack <list.json> --dir d             a whole pack: PNGs, an atlas, manifest.json (anchors, groups, hashes, version) and goldens.json
// Every drawing command takes --style (pixel, the default, ansi, or text). For ansi, --target classic|ice|modern, --colors 16|256|true,
// --charset cp437|utf8 and --spacing 8|9 (a .png render) apply; a .ans says which charset it wrote (UTF-8 shows CP437 as mojibake).
import { parseArgs } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Pixels } from './pixels.js';
import { writeAnsi, readAnsi, toPlainText } from './ansi.js';
import { writeAnsimation } from './ansimation.js';
import { fromImage } from './ansi-image.js';
import { writeText, lintText } from './text-art.js';
import { lintAnsi, lintAnsiFrames } from './ansi-lint.js';
import { renderCells } from './ansi-render.js';
import { loadAtlas } from './atlas.js';
import { decodePng, pixelsToPng } from './png.js';
import { draw, RECIPES, STYLES, DEFAULT_STYLE } from './recipes/index.js';
import { coverage, KITS } from './kits/index.js';
import { lint, lintFrames, compare } from './lint.js';
import { contactSheet, rgbaSheet } from './sheet.js';
import { writePack } from './pack.js';

const { values, positionals } = parseArgs({ allowPositionals: true, options: { out: { type: 'string' }, dir: { type: 'string' }, scale: { type: 'string' }, strict: { type: 'boolean' }, creator: { type: 'string' }, style: { type: 'string' }, target: { type: 'string' }, colors: { type: 'string' }, width: { type: 'string' }, charset: { type: 'string' }, spacing: { type: 'string' }, clip: { type: 'string' }, columns: { type: 'string' }, baud: { type: 'string' } } });
const [cmd, target, ...rest] = positionals;
const style = values.style ?? DEFAULT_STYLE;
const cellStyle = style !== 'pixel';                    // ansi and text both draw on cells
const scale = Number(values.scale ?? (cellStyle || cmd === 'convert' ? 1 : 4));
if (!STYLES.includes(style)) { console.error(`unknown --style "${style}"; known: ${STYLES.join(', ')}`); process.exit(2); }
// `--colors` is also the depth an ANSI recipe draws at (faces, items, creations, fights, places, rooms); a `colors=` param wins.
const drawIn = (id, params) => {
  if (style === 'ansi' && values.colors !== undefined && params.colors === undefined && RECIPES[id]?.ansiOnly) params = { ...params, colors };
  try { return draw(id, params, { style, bgLimit: values.target && values.target !== 'classic' ? 16 : undefined, ...(RECIPES[id]?.needsAtlas ? { atlas: loadAtlas() } : {}) }); } catch (e) { console.error(e.message); process.exit(2); } };
const parse = (kv) => Object.fromEntries(kv.map((s) => { const [k, ...v] = s.split('='); const x = v.join('='); return [k, x === 'true' ? true : x === 'false' ? false : x !== '' && !isNaN(Number(x)) ? Number(x) : x]; }));
const need = (v, flag) => { if (!v) { console.error(`${cmd} needs ${flag}`); process.exit(2); } return v; };
const load = (file) => { const { w, h, rgba } = decodePng(readFileSync(file)); return Pixels.fromRgba(w, h, rgba); };
const sheetOf = (frames) => cellStyle ? rgbaSheet(frames.map(renderOf), { scale, cols: Math.min(frames.length, 4) }) : contactSheet(frames, { scale, cols: Math.min(frames.length, 10) });
const colors = values.colors === 'true' ? 'true' : Number(values.colors ?? 16);
const spacing = Number(values.spacing ?? 8);
const renderOf = (cells) => renderCells(cells, { atlas: loadAtlas(), letterSpacing: spacing });
const sizeOf = (f) => cellStyle ? `${f.w}x${f.h} cells` : `${f.w}x${f.h}`;
// One ANSI frame in the form its file name asks for: .ans (the writer, whose warnings go to stderr), .txt (characters) or .png (the render).
const ansiFile = (cells, file, sauce) => {
  if (style === 'text' && /\.txt$/i.test(file)) { const t = writeText(cells); for (const w of t.warnings) console.error(`warn ${w}`); return Buffer.from(t.bytes); }
  if (/\.txt$/i.test(file)) return Buffer.from(toPlainText(cells) + '\n');
  if (/\.ans$/i.test(file)) {
    const out = writeAnsi(cells, { target: values.target, charset: values.charset, colors, sauce });
    for (const w of out.warnings) console.error(`warn ${w}`);
    console.error(`wrote ${out.target} target, ${out.charset === 'cp437' ? 'CP437 bytes (a UTF-8 terminal shows them as mojibake; use --charset utf8 or --target modern to look)' : 'UTF-8'}, ${out.colors} colours${out.ice ? ', iCE' : ''}`);
    return Buffer.from(out.bytes);
  }
  return rgbaSheet([renderOf(cells)], { scale, cols: 1, gap: 0 });
};

if (cmd === 'kits') {
  let problems = 0;
  for (const k of coverage()) {
    console.log(`\n${k.title}\n  ${k.doc}\n  ${Object.entries(k.totals).filter(([, n]) => n).map(([s, n]) => `${n} ${s}`).join(', ')}`);
    for (const r of k.rows) {
      console.log(`  ${r.status.padEnd(14)} ${r.id}${r.note ? ` (${r.note})` : ''}${r.ansi ? ` [ansi: ${r.ansi}]` : ''}`);
      for (const p of r.problems) { console.log(`      PROBLEM ${p}`); problems++; }
      for (const w of r.warnings) console.log(`      warn ${w}`);
    }
  }
  const gaps = coverage().flatMap((k) => k.rows.filter((r) => r.status === 'gap').map((r) => `${k.id}/${r.id}`));
  console.log(`\n${KITS.length} kits. Gaps (nothing draws these yet): ${gaps.join(', ') || 'none'}`);
  if (values.strict && problems) process.exit(1);
} else if (cmd === 'draw') {
  const a = drawIn(need(target, '<recipe>'), parse(rest));
  const out = need(values.out, '--out');
  if (values.clip) {
    // A clip: .ans is an ANSImation (cursor-move deltas at --baud) with the finished picture as a still beside it; .png is its frames.
    const c = a.clips?.[values.clip]; if (!c) { console.error(`${a.id} has no clip "${values.clip}"; known: ${Object.keys(a.clips ?? {}).join(', ') || 'none'}`); process.exit(2); }
    if (style !== 'ansi' || !/\.ans$/i.test(out)) { writeFileSync(out, sheetOf(c.frames)); console.log(`${a.id}.${values.clip}: ${c.frames.length} frames -> ${out}`); process.exit(0); }
    const m = writeAnsimation(c.frames, { target: values.target, charset: values.charset, colors, baud: Number(values.baud ?? 2400), fps: c.fps || 8, sauce: a.sauce });
    for (const w of m.warnings) console.error(`warn ${w}`);
    const still = out.replace(/\.ans$/i, '.still.ans');
    writeFileSync(out, m.bytes); writeFileSync(still, m.still);
    console.log(`${a.id}.${values.clip}: ${c.frames.length} frames, ${m.bytes.length} bytes, ${m.seconds.toFixed(1)} s at ${m.baud} baud (${m.charset}) -> ${out}, still -> ${still}`);
    process.exit(0);
  }
  writeFileSync(out, cellStyle && /\.(ans|txt)$/i.test(out) ? ansiFile(a.frames[0], out, a.sauce) : sheetOf(a.frames));
  console.log(`${a.id}: ${a.frames.length} frame(s) ${sizeOf(a.frames[0])} -> ${out}`);
} else if (cmd === 'convert') {
  // A drafting aid: the converted cells go to a file for a person to study and redraw; they are not engine art. Needs an image you may use.
  const { cells, warnings } = fromImage(load(need(target, '<image.png>')), { columns: Number(values.columns ?? 80), colors, bgLimit: values.target === 'classic' || !values.target ? 8 : 16 });
  for (const w of warnings) console.error(`warn ${w}`);
  const out = need(values.out, '--out');
  writeFileSync(out, ansiFile(cells, out));
  console.log(`${target}: ${cells.w}x${cells.h} cells -> ${out}`);
} else if (cmd === 'sheet') {
  const r = RECIPES[need(target, '<recipe>')]; if (!r) { console.error(`unknown recipe; known: ${Object.keys(RECIPES).join(', ')}`); process.exit(2); }
  const items = r.variants().map((v) => drawIn(target, v).frames[0]);
  const wide = items[0].w > 64 || cellStyle;
  writeFileSync(need(values.out, '--out'), cellStyle ? sheetOf(items) : contactSheet(items, { scale: wide ? 1 : scale, cols: wide ? 4 : 10 }));
  console.log(`${items.length} variants -> ${values.out}`);
} else if (cmd === 'lint') {
  const isFile = /\.png$/i.test(need(target, '<recipe|file.png|file.ans>'));
  if (style === 'text') {
    const a = drawIn(target, parse(rest)), found = a.frames.flatMap((f, i) => lintText(f, { name: `${a.id}#${i}`, width: Number(values.width ?? 80) }));
    for (const f of found) console.log(`${f.level} [${f.rule}] ${f.message}`);
    console.log(found.length ? `${found.length} finding(s)` : 'no findings');
    process.exit(found.some((f) => f.level === 'error') ? 1 : 0);
  }
  if (style === 'ansi' || /\.ans$/i.test(target)) {
    // A recipe, or a .ans file read back (the reader needs no atlas). --target picks the rules; --width the column limit.
    const opts = { target: values.target ?? 'classic', depth: colors, width: Number(values.width ?? 80) };
    const found = /\.ans$/i.test(target) ? lintAnsi(readAnsi(new Uint8Array(readFileSync(target)), { width: opts.width }).cells, { ...opts, name: target }) : (() => { const a = drawIn(target, parse(rest)); return [...a.frames.flatMap((f, i) => lintAnsi(f, { ...opts, name: `${a.id}#${i}` })), ...Object.entries(a.clips ?? {}).flatMap(([n, c]) => lintAnsiFrames(c.frames, { name: `${a.id}.${n}`, fps: c.fps ?? a.fps, loop: c.loop }))]; })();
    for (const f of found) console.log(`${f.level} [${f.rule}] ${f.message}`);
    console.log(found.length ? `${found.length} finding(s)` : 'no findings');
    process.exit(found.some((f) => f.level === 'error') ? 1 : 0);
  }
  const found = isFile ? lint(load(target), { name: target }) : (() => { const a = drawIn(target, parse(rest)); return [...a.frames.flatMap((f, i) => lint(f, { name: `${a.id}#${i}` })), ...Object.entries(a.clips ?? {}).flatMap(([n, c]) => lintFrames(c.frames, { name: `${a.id}.${n}`, loop: c.loop }))]; })();
  for (const f of found) console.log(`${f.level} [${f.rule}] ${f.message}`);
  console.log(found.length ? `${found.length} finding(s)` : 'no findings');
  if (found.some((f) => f.level === 'error')) process.exit(1);
} else if (cmd === 'study') {
  const ref = load(need(target, '<ref.png>')), mine = load(need(rest[0], '<mine.png>'));
  writeFileSync(need(values.out, '--out'), contactSheet([ref, mine], { scale, cols: 2 }));
  console.log(JSON.stringify(compare(ref, mine)));
} else if (cmd === 'export') {
  const id = need(target, '<recipe>'), a = drawIn(id, parse(rest)), dir = need(values.dir, '--dir'), name = a.id.replace(/[^a-z0-9.-]/gi, '_');
  mkdirSync(dir, { recursive: true });
  const files = {};
  const put = (key, frames) => { files[key] = frames.flatMap((f, i) => {
    if (!cellStyle) { const file = `${name}.${key}.${i}.png`; writeFileSync(join(dir, file), pixelsToPng(f)); return [file]; }
    return (style === 'text' ? ['png', 'txt'] : ['png', 'ans', 'txt']).map((ext) => { const file = `${name}.${key}.${i}.${ext}`; writeFileSync(join(dir, file), ansiFile(f, file, a.sauce)); return file; });
  }); };
  put('frames', a.frames);
  for (const [n, c] of Object.entries(a.clips ?? {})) put(`clip-${n}`, c.frames);
  for (const [n, fr] of Object.entries(a.layers ?? {})) put(`layer-${n}`, fr);
  const kits = KITS.filter((k) => k.subjects.some((s) => s.recipes?.some((r) => r.recipe === id))).map((k) => k.id);
  const record = {
    id: a.id, recipe: id, params: parse(rest), style, ...(style === 'ansi' ? { ansi: { target: values.target ?? 'classic', colors, charset: values.charset ?? null, font: 'IBM VGA 8x16' } } : {}), size: { w: a.frames[0].w, h: a.frames[0].h }, frames: a.frames.length, fps: a.fps, loop: a.loop,
    anchors: a.anchors, events: Object.fromEntries(Object.entries(a.clips ?? {}).map(([n, c]) => [n, c.events])), files, modelledOn: kits,
    // Drawn by this project's recipes from a seed: nothing third party is in the pixels. The kits were studied, not copied.
    source: { creator: values.creator ?? 'Project recipes (sprites/)', packName: 'internal' }, license: { licenseName: 'Project-created', status: 'approved', attributionRequired: false, evidenceNotes: 'Generated by deterministic recipes from the DB32 palette (folded or drawn to cells for ANSI and text); no third-party pixels.' },
  };
  writeFileSync(join(dir, `${name}.json`), JSON.stringify(record, null, 2) + '\n');
  console.log(`${a.id}: ${Object.values(files).flat().length} files and ${name}.json -> ${dir}`);
} else if (cmd === 'pack') {
  const dir = need(values.dir, '--dir'); let list;
  try { list = JSON.parse(readFileSync(need(target, '<list.json>'), 'utf8')); } catch (e) { console.error(`cannot read the pack list: ${e.message}`); process.exit(2); }
  try { const m = writePack(list, dir, { creator: values.creator }); console.log(`${m.name}: ${m.entries.length} entries, atlas ${m.atlas.w}x${m.atlas.h}, manifest.json and goldens.json -> ${dir}`); } catch (e) { console.error(e.message); process.exit(2); }
} else {
  console.error('commands: kits, draw, sheet, convert, lint, study, export, pack (see the top of cli.js)');
  process.exit(2);
}
