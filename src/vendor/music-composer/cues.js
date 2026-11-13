// The cue sheet as data (the game-music reference kit: "build a cue sheet early"). A sheet lists every piece of music a game needs;
// `composeCues` composes each one, checks it, and returns the files a host plays plus a manifest saying how to play them:
// loops that seam, stingers that play once and ring out, vertical layers (stems a host fades in and out) and horizontal sections
// (order lists a host switches between at a pattern boundary). No Node built-ins: the CLI writes the files.
//
//   { "title": "Moss Keep",
//     "cues": [
//       { "id": "explore", "recipe": "song", "params": { "form": "verse-chorus", "key": "D", "seed": "keep" },
//         "layers": { "base": ["chords", "bass"], "melody": ["lead"], "percussion": ["drums"] }, "on": ["base"] },
//       { "id": "found-key", "recipe": "jingle", "params": { "kind": "pickup" }, "tail": 2 } ],
//     "transitions": [ { "from": "explore", "to": "battle", "at": "phrase", "via": "alarm" } ] }
import { compose, RECIPES, RECIPE_IDS } from './recipes/index.js';
import { lint } from './lint.js';
import { toModule } from './tracker.js';
import { toNtt } from './ntt.js';
import { toMidi } from './midi.js';
import { render } from './synth.js';
import { toWav } from './wav.js';

export const CUE_KINDS = ['loop', 'stinger'];
export const SWITCH_AT = ['bar', 'phrase', 'section'];
const PHRASE_BARS = 4;
const ID = /^[a-z0-9][a-z0-9._-]*$/i;
const ms = (beats, bpm) => Math.round((beats * 60000) / bpm);

/** The built files and manifest entry for one cue. Throws with every problem found, not just the first. */
export function buildCue(cue, { engine = 'sample', rowsPerBeat = 4 } = {}) {
  const problems = [], at = (m) => problems.push(`cue "${cue.id}": ${m}`);
  if (!ID.test(cue.id ?? '')) throw new Error(`cue id "${cue.id}" must be letters, digits, dots, dashes or underscores`);
  if (!RECIPES[cue.recipe]) throw new Error(`cue "${cue.id}": unknown recipe "${cue.recipe}"; known: ${RECIPE_IDS.join(', ')}`);
  const art = compose(cue.recipe, cue.params ?? {}), kind = cue.kind ?? (art.loop || art.score.bars > 2 ? 'loop' : 'stinger');   // a long piece repeats whole; a short one that does not loop plays once
  if (!CUE_KINDS.includes(kind)) at(`kind must be ${CUE_KINDS.join(' or ')}, not "${kind}"`);
  const tracks = [...art.score.tracks.keys()], layers = cue.layers ?? null;
  if (layers) {
    const seen = new Map();
    for (const [name, list] of Object.entries(layers)) for (const t of list) { if (!tracks.includes(t)) at(`layer "${name}" names track "${t}"; the music has ${tracks.join(', ')}`); else if (seen.has(t)) at(`track "${t}" is in layers "${seen.get(t)}" and "${name}"`); else seen.set(t, name); }
    for (const t of tracks) if (!seen.has(t)) at(`track "${t}" is in no layer`);
    for (const name of cue.on ?? []) if (!layers[name]) at(`"on" names layer "${name}", which is not one`);
  } else if (cue.on) at('"on" needs layers');
  if (kind === 'stinger' && art.score.bars > 4) at(`a stinger is a bar or four, not ${art.score.bars}`);
  if (problems.length) throw new Error(problems.join('\n'));

  const found = lint(art.score, { name: cue.id, loop: kind === 'loop', ...art.lint });
  if (found.some((f) => f.level === 'error')) throw new Error(found.filter((f) => f.level === 'error').map((f) => f.message).join('\n'));
  const tail = cue.tail ?? (kind === 'stinger' ? 2 : 0);
  const { module, report } = toModule(art.score, { rowsPerBeat, sections: art.sections, engine: cue.engine ?? engine, lists: true, layers, on: cue.on ?? null, tail: kind === 'stinger' ? tail : 0, end: kind === 'stinger' });
  module.title = module.title || cue.id;
  const s = art.score, sections = Object.entries(art.sections).map(([name, x]) => ({ name, fromBar: Math.round(x.from / s.barTicks), toBar: Math.round(x.to / s.barTicks) }));
  const withList = sections.length > 1 ? sections.map((x, i) => ({ ...x, list: i + 1 })) : sections;
  const switchBars = [...new Set([...sections.map((x) => x.fromBar), ...Array.from({ length: Math.ceil(s.bars / PHRASE_BARS) }, (_, i) => i * PHRASE_BARS)])].filter((b) => b < s.bars).sort((a, b) => a - b);
  const entry = {
    id: cue.id, kind, file: `${cue.id}.ntt`, midi: `${cue.id}.mid`,
    recipe: cue.recipe, params: cue.params ?? {}, key: s.key, scale: s.scale, bpm: s.bpm, beats: s.beats, bars: s.bars,
    lengthMs: ms(s.bars * s.beats, s.bpm), ...(kind === 'stinger' ? { tailMs: ms(tail, s.bpm) } : {}),
    sections: withList, switchBars,
    layers: layers ? Object.fromEntries(Object.entries(layers).map(([name, list]) => [name, { tracks: list, channels: report.layers[name] }])) : {},
    start: layers ? cue.on ?? Object.keys(layers) : [],
    provenance: 'Project-created',
    warnings: found.map((f) => f.message),
  };
  return { id: cue.id, art, module, report, ntt: toNtt(module), midi: toMidi(art.score), entry };
}

/** Build every cue of a sheet; `{ manifest, cues }` where `manifest` is plain JSON for the game and each cue has its `ntt` and `midi` bytes. */
export function composeCues(sheet, opts = {}) {
  if (!sheet || !Array.isArray(sheet.cues) || !sheet.cues.length) throw new Error('a cue sheet needs a "cues" list');
  const problems = [], ids = new Set(), built = [];
  for (const cue of sheet.cues) {
    if (ids.has(cue.id)) { problems.push(`cue id "${cue.id}" is used twice`); continue; }
    ids.add(cue.id);
    try { built.push(buildCue(cue, opts)); } catch (e) { problems.push(e.message); }
  }
  const byId = new Map(built.map((b) => [b.id, b.entry]));
  const sectionsOf = (ref) => { const [id, section] = String(ref).split(':'), e = byId.get(id); return !e ? `unknown cue "${id}"` : section && !e.sections.some((x) => x.name === section) ? `cue "${id}" has no section "${section}"` : null; };
  (sheet.transitions ?? []).forEach((t, i) => {
    for (const [side, ref] of Object.entries({ from: t.from, to: t.to })) { const bad = ids.has(String(ref).split(':')[0]) ? sectionsOf(ref) : `unknown cue "${ref}"`; if (bad && byId.size === ids.size) problems.push(`transition ${i + 1} ${side}: ${bad}`); }
    if (t.at !== undefined && !SWITCH_AT.includes(t.at)) problems.push(`transition ${i + 1}: "at" must be ${SWITCH_AT.join(', ')}, not "${t.at}"`);
    if (t.via !== undefined && byId.get(t.via)?.kind !== 'stinger') problems.push(`transition ${i + 1}: "via" must be a stinger cue, not "${t.via}"`);
  });
  if (problems.length) throw new Error(problems.join('\n'));
  return { manifest: { version: 1, title: sheet.title ?? '', cues: built.map((b) => b.entry), transitions: (sheet.transitions ?? []).map((t) => ({ at: 'phrase', ...t })) }, cues: built };
}

/** The bar a host may switch away from a cue playing at `bar` (it may be fractional): the next bar, phrase or section start, or the end of the loop when none is left. */
export function nextSwitch(entry, bar, at = 'phrase') {
  if (!SWITCH_AT.includes(at)) throw new Error(`"at" must be ${SWITCH_AT.join(', ')}, not "${at}"`);
  if (at === 'bar') return Math.min(Math.ceil(bar), entry.bars);
  const points = at === 'section' ? entry.sections.map((x) => x.fromBar) : entry.switchBars;
  return points.filter((b) => b >= bar).sort((a, b) => a - b)[0] ?? entry.bars;
}

/**
 * A play script (the text `ntt_play --script` reads) that follows a cue sheet through its switching, as a game would: start the first loop, bring
 * its quiet layers in, play each stinger over it, then walk the transitions in order, each queued `hold` bars into the music and landing at the
 * next legal bar (`nextSwitch`). Plain text; file names are the manifest's, read from the script's folder. A transition to `cue:section` is a
 * `sequence` when it stays in the same cue; to another cue it starts that cue from its top (the player queues a song, not a place in one).
 */
export function playScript(manifest, { hold = 8 } = {}) {
  const by = new Map(manifest.cues.map((c) => [c.id, c])), id = (ref) => String(ref).split(':')[0], sec = (ref) => String(ref).split(':')[1];
  const trs = manifest.transitions ?? [], first = by.get(trs.length ? id(trs[0].from) : manifest.cues.find((c) => c.kind === 'loop')?.id);
  if (!first || first.kind !== 'loop') throw new Error('a play script needs a loop cue to start with');
  const secs = (e, bars) => (bars * e.beats * 60) / e.bpm, t2 = (t) => t.toFixed(2);
  const out = [`# ${manifest.title || 'cue sheet'}: generated by compose; times in seconds`, `play ${first.file}`];
  let cur = first, t = 0, origin = 0;   // `origin`: when the cue now playing began
  const wait = (bars) => { t += secs(cur, bars); };
  const bar = () => (((t - origin) * cur.bpm) / (60 * cur.beats)) % cur.bars;   // where the loop is, in bars, at the time `t`
  // The first cue's quiet layers come in one at a time, a measure apart.
  for (const [name, l] of Object.entries(first.layers)) {
    if (first.start.includes(name)) continue;
    wait(hold / 2);
    out.push(`at ${t2(t)} channels ${l.channels.map((c) => c + 1).join(',')} on pattern 2   # layer ${name}`);
  }
  // Every stinger no transition uses as a bridge plays once over the music.
  const bridges = new Set(trs.map((x) => x.via));
  for (const s of manifest.cues.filter((c) => c.kind === 'stinger' && !bridges.has(c.id))) {
    wait(hold / 2);
    out.push(`at ${t2(t)} stinger ${s.file} now   # ${s.id}`);
    t += (s.lengthMs + (s.tailMs ?? 0)) / 1000;
  }
  for (const x of trs) {
    const from = by.get(id(x.from)), to = by.get(id(x.to)), at = x.at ?? 'phrase';
    if (from !== cur) { out.push(`at ${t2(t)} cue ${from.file} now   # to start ${from.id}`); cur = from; origin = t; }
    wait(hold);
    out.push(`# ${x.from} -> ${x.to}${x.via ? ` via ${x.via}` : ''}`);
    if (to === cur && sec(x.to)) out.push(`at ${t2(t)} sequence ${to.file} ${sec(x.to)} ${at}`);
    else out.push(`at ${t2(t)} ${x.via ? `transition ${by.get(x.via).file} ${to.file}` : `cue ${to.file}`} ${at}`);
    const lands = t + secs(cur, nextSwitch(cur, bar(), at) - bar());   // the player takes it at the next legal bar of the music now playing
    origin = t = lands + (x.via ? (by.get(x.via).lengthMs + (by.get(x.via).tailMs ?? 0)) / 1000 : 0);
    cur = to;
  }
  wait(hold);
  out.push(`end ${t2(t)}`);
  return out.join('\n') + '\n';
}

const AUDIO_NAME = /^[a-z0-9][a-z0-9_-]*$/i;
/** The audio files a cue is baked to: one mix, or one stem per layer (all the same length, so they start together and stay in step). */
export function audioFiles(entry) {
  const names = Object.keys(entry.layers);
  for (const n of names) if (!AUDIO_NAME.test(n)) throw new Error(`cue "${entry.id}": layer "${n}" must be letters, digits, dashes or underscores to name a file`);
  return names.length ? { layers: Object.fromEntries(names.map((n) => [n, `${entry.id}.${n}.wav`])) } : { mix: `${entry.id}.wav` };
}

/**
 * A built cue rendered to audio with the preview synth: `{ sampleRate, mix }` or `{ sampleRate, layers }` (Float32Arrays). A loop is exactly
 * its length, its tail wrapped onto the start, so it seams; a stinger is its length plus its tail. Stems share the full mix's scale, so
 * they sum to it. Floats from `Math.sin`: for listening, never hashed.
 */
export function renderCue(built, { sampleRate = 22050 } = {}) {
  const { art, entry } = built, loop = entry.kind === 'loop', o = { sampleRate, loop, tail: (entry.tailMs ?? 0) / 1000, normalize: false };
  const full = render(art.score, o);
  let peak = 0;
  for (let i = 0; i < full.length; i++) peak = Math.max(peak, Math.abs(full[i]));
  const k = peak > 0.9 ? 0.9 / peak : 1, scaled = (a) => { for (let i = 0; i < a.length; i++) a[i] *= k; return a; };
  if (!Object.keys(entry.layers).length) return { sampleRate, mix: scaled(full) };
  return { sampleRate, layers: Object.fromEntries(Object.entries(entry.layers).map(([name, l]) => [name, scaled(render(art.score, { ...o, tracks: l.tracks }))])) };
}

/** `{ file: Uint8Array }`: a built cue baked to WAV files named by `audioFiles`. */
export function cueAudio(built, opts = {}) {
  const r = renderCue(built, opts), f = audioFiles(built.entry);
  return r.mix ? { [f.mix]: toWav(r.mix, r.sampleRate) } : Object.fromEntries(Object.entries(r.layers).map(([n, a]) => [f.layers[n], toWav(a, r.sampleRate)]));
}
