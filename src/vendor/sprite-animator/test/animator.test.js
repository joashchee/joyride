import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as api from '../index.js';
import { rng } from '../rng.js';
import { pose, clip, fromKeys, durations, checkShape, writeClip, readClip, CHANNELS } from '../clip.js';
import { lintClip } from '../lint.js';
import { tempo, PPQ } from '../tempo.js';
import { bind } from '../bind.js';
import { RECIPES, RECIPE_IDS, animate } from '../recipes/index.js';
import { MOTION_TYPES, MOTION_OF, motionTypeOf, motionPresets, motionClip, lintMotionType } from '../types.js';
import { KITS, coverage, registerKit, subjectStatus } from '../kits/index.js';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const errors = (c, o) => lintClip(c, o).filter((f) => f.level === 'error');
const files = (dir = ROOT, ext = '.js') => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.name === 'node_modules' || e.name.startsWith('.') ? [] : e.isDirectory() ? files(join(dir, e.name), ext) : e.name.endsWith(ext) ? [join(dir, e.name)] : []));

test('same recipe, params and seed: the same clip, down to the bytes', () => {
  for (const id of RECIPE_IDS) for (const v of RECIPES[id].variants()) assert.equal(writeClip(animate(id, v)), writeClip(animate(id, v)), `${id} ${JSON.stringify(v)}`);
  const a = writeClip(animate('walk', { wobble: 2, seed: 'x' })), b = writeClip(animate('walk', { wobble: 2, seed: 'y' }));
  assert.notEqual(a, b, 'the seed changes the wobble');
  assert.deepEqual(animate('walk', { wobble: 0, seed: 'x' }).frames, animate('walk', { wobble: 0, seed: 'y' }).frames, 'no wobble draws no randomness');
});

test('every variant of every recipe is sound: no lint errors, integers only, JSON round trip', () => {
  for (const id of RECIPE_IDS) {
    assert.ok(RECIPES[id].variants().length >= 3, `${id} has variants`);
    for (const v of RECIPES[id].variants()) {
      const c = animate(id, v), name = `${id} ${JSON.stringify(v)}`;
      assert.deepEqual(errors(c), [], name);
      assert.deepEqual(checkShape(c), [], name);
      assert.deepEqual(readClip(writeClip(c)), c, `${name} round trips`);
    }
  }
});

test('the minimal loops: walk 4 frames, run 4, jump 6', () => {
  assert.equal(animate('walk').frames.length, 4);
  assert.equal(animate('run').frames.length, 4);
  assert.equal(animate('jump').frames.length, 6);
  assert.deepEqual([animate('walk').loop, animate('run').loop, animate('jump').loop], [true, true, false]);
});

test('a walk keeps a foot down and steps twice; a run has flight; a jump leaves and lands and ends at rest', () => {
  const w = animate('walk'), r = animate('run'), j = animate('jump');
  assert.ok(w.frames.every((f) => f.air === 0));
  assert.deepEqual(w.events.map((e) => e.name), ['step', 'step']);
  assert.ok(r.frames.some((f) => f.air === 1) && r.frames.some((f) => f.air === 0));
  assert.deepEqual(j.events.map((e) => e.name), ['takeoff', 'land']);
  const [off, land] = j.events.map((e) => e.frame);
  assert.ok(j.frames.slice(off, land).every((f) => f.air === 1) && j.frames[land].air === 0);
  assert.deepEqual(j.frames.at(-1), pose());
  assert.ok(Math.min(...j.frames.map((f) => f.dy)) === -5, 'the apex is `height`');
});

test('more frames blend the same keys: event and air frames stay in step, a loop still closes, a one-shot ends on its last key', () => {
  for (const n of [4, 5, 6, 8, 12, 16]) {
    assert.deepEqual(errors(animate('walk', { frames: n })), [], `walk ${n}`);
    assert.deepEqual(errors(animate('run', { frames: n })), [], `run ${n}`);
  }
  for (const n of [6, 7, 8, 12, 16]) {
    const j = animate('jump', { frames: n });
    assert.equal(j.frames.length, n);
    assert.deepEqual(errors(j), [], `jump ${n}`);
    assert.deepEqual(j.frames.at(-1), pose(), `jump ${n} ends at rest`);
  }
  const k = fromKeys([{ dy: 0 }, { dy: -4, at: 2 }], { loop: false, frames: 3 });
  assert.deepEqual(k.frames.map((f) => f.dy), [0, -2, -4]);
});

test('the lint catches a broken loop, a walk that leaves the ground, and a jump that never lands', () => {
  const w = animate('walk');
  const popped = { ...w, frames: w.frames.map((f, i) => (i === 3 ? { ...f, dy: -9, legs: [5, -5] } : f)) };
  assert.ok(errors(popped).some((f) => f.rule === 'seam'));
  const flying = { ...w, frames: w.frames.map((f, i) => (i === 1 ? { ...f, air: 1 } : f)) };
  assert.ok(errors(flying).some((f) => f.rule === 'kind'));
  assert.ok(errors({ ...w, events: [] }).some((f) => f.rule === 'kind'));
  assert.ok(errors({ ...animate('run'), frames: animate('run').frames.map((f) => ({ ...f, air: 0 })) }).some((f) => f.rule === 'kind'));
  const j = animate('jump');
  assert.ok(errors({ ...j, events: j.events.filter((e) => e.name !== 'land') }).some((f) => f.rule === 'kind'));
  assert.ok(errors({ ...j, loop: true }).some((f) => f.rule === 'kind'));
  assert.ok(errors(clip('x', [pose({ dy: 0.5 })])).some((f) => f.rule === 'shape'));
  assert.ok(errors(w, { max: 3 }).some((f) => f.rule === 'frames'));
  assert.ok(lintClip({ ...w, frames: w.frames.map((f) => ({ ...f, dx: 20 })) }).some((f) => f.rule === 'range'));
  assert.deepEqual(errors(popped, { skip: ['seam'] }), []);
});

test('bad params and bad clips are refused with a message', () => {
  assert.throws(() => animate('walk', { stride: 9 }), /stride/);
  assert.throws(() => animate('jump', { frames: 1 }), /frames/);
  assert.throws(() => animate('swim'), /unknown recipe/);
  assert.throws(() => pose({ wiggle: 1 }), /unknown pose channel/);
  assert.throws(() => readClip('{}'), /not an animator-clip/);
  assert.throws(() => readClip('nope'), /not JSON/);
  assert.throws(() => writeClip(clip('x', [pose({ dx: 1.5 })])), /integer/);
});

test('tempo: frame lengths add up exactly, steps land on the beat, ticks use the 96 ppq grid', () => {
  assert.equal(PPQ, 96);
  const t = tempo(animate('walk'), { bpm: 120 });
  assert.deepEqual([t.totalMs, t.totalTicks, t.beats], [1000, 192, 2]);
  assert.deepEqual(t.events.map((e) => [e.ms, e.tick]), [[0, 0], [500, 96]]);
  for (const [bpm, spb, id] of [[97, 1, 'walk'], [140, 2, 'run'], [133, 1, 'run'], [60, 2, 'walk']]) {
    const c = animate(id), x = tempo(c, { bpm, stepsPerBeat: spb });
    assert.equal(x.ms.reduce((a, b) => a + b, 0), x.totalMs, `${id} ${bpm}`);
    assert.ok(x.ms.every((m) => Number.isInteger(m) && m > 0));
    assert.ok(x.events.every((e, i, a) => !i || e.tick > a[i - 1].tick));
    assert.equal(x.events[1].tick - x.events[0].tick, Math.round(PPQ / spb), 'steps are a (fraction of a) beat apart');
  }
  const j = tempo(animate('jump'), { bpm: 120, beats: 2 });
  assert.equal(j.totalMs, 1000);
  assert.ok(j.ms[0] > j.ms[1], 'the held crouch stays longer than a single frame');
  assert.throws(() => tempo(animate('jump'), { bpm: 120 }), /pass beats/);
  assert.throws(() => tempo(animate('walk'), { bpm: 5 }), /bpm/);
  assert.equal(durations(animate('walk')).join(), '125,125,125,125');
});

test('motion types: every Painter recipe has a type, every type has a recipe for each preset, and a clip is the preset\'s own', () => {
  assert.equal(Object.keys(MOTION_TYPES).length, 14);
  for (const [painter, type] of Object.entries(MOTION_OF)) assert.ok(MOTION_TYPES[type], `${painter} names a real type`);
  for (const type of Object.keys(MOTION_TYPES)) assert.ok(Object.values(MOTION_OF).includes(type), `${type} belongs to a Painter recipe`);
  assert.equal(motionTypeOf('character'), 'figure');
  assert.equal(motionTypeOf('banner'), 'lettering');
  assert.throws(() => motionTypeOf('walk'), /no motion type/);
  for (const [type, t] of Object.entries(MOTION_TYPES)) {
    assert.deepEqual(motionPresets(type), Object.keys(t.presets));
    for (const name of Object.keys(t.presets)) {
      assert.ok(RECIPES[`${type}.${name}`], `${type}.${name} is registered`);
      const c = animate(`${type}.${name}`);
      assert.equal(c.id, name, 'the clip id is the preset name, as in Painter');
      assert.deepEqual(c.spec.recipe, `${type}.${name}`);
      assert.deepEqual(errors(c), [], `${type}.${name}`);
    }
  }
});

test('motion types: presets give the frames Painter 1.20 gives, as integers (a wave is a rational curve, not a sine)', () => {
  const dys = (c) => c.frames.map((f) => f.dy), at = (id, p) => animate(id, p).frames;
  assert.deepEqual(dys(animate('object.bob')), [0, 1, 0, -1]);
  assert.deepEqual(dys(animate('object.bob', { amount: 3, steps: 8 })), [0, 2, 3, 2, 0, -2, -3, -2]);
  assert.deepEqual(at('cloth.gust').map((f) => [f.sway, f.lean]), [[0, 0], [2, 1], [2, 0], [0, 1], [-2, 0], [-2, 1]]);
  assert.deepEqual(at('creature.slither').map((f) => [f.sway, f.dx]), [[0, 0], [2, 1], [0, 0], [-2, -1]]);
  const hop = animate('figure.hop');
  assert.deepEqual(hop.events, [{ frame: 1, name: 'takeoff' }, { frame: 3, name: 'land' }]);
  assert.deepEqual(hop.frames.map((f) => f.air), [0, 1, 1, 0, 0]);
  assert.deepEqual(animate('figure.bow').holds, [1, 3, 1]);
  assert.equal(animate('figure.lunge').frames[1].arm, 'forward');
  assert.deepEqual(Object.keys(animate('head.nod').frames[0]).sort(), [...CHANNELS].sort(), 'poses are whole');
  assert.ok(Object.is(animate('object.bob').frames[0].dy, 0), 'no -0');
});

test('motion types: size-led presets read w and h; a scroll wraps, and the file keeps wrap', () => {
  assert.deepEqual(animate('plant.grow', { h: 16 }).frames.map((f) => f.squat), [12, 9, 6, 3, 0]);
  assert.deepEqual(animate('plant.grow', { h: 16, from: 4 }).frames.map((f) => f.squat), [4, 3, 2, 1, 0]);
  assert.deepEqual(animate('panel.slide-in', { w: 32 }).frames.map((f) => f.dx), [-8, -5, -3, 0]);
  assert.deepEqual(animate('panel.drop-in', { h: 16, distance: 6 }).frames.map((f) => f.dy), [-6, -4, -2, 0]);
  const s = animate('tiling.scroll', { w: 8, step: 2 });
  assert.equal(s.wrap, true);
  assert.deepEqual(s.frames.map((f) => f.dx), [0, -2, -4, -6]);
  assert.deepEqual(animate('tiling.scroll', { h: 8, direction: 'down' }).frames.map((f) => f.dy), [0, 1, 2, 3, 4, 5, 6, 7]);
  assert.match(writeClip(s), /"wrap": true/);
  assert.equal(readClip(writeClip(s)).wrap, true);
  assert.doesNotMatch(writeClip(animate('walk')), /wrap/, 'a clip that does not wrap says nothing, so older files are unchanged');
  assert.equal(animate('figure.hop').wrap, undefined);
  assert.equal(animate('tiling.scroll', { w: 40 }).frames.length, 40, 'a scroll takes a frame a step; the frame limit allows it');
  assert.deepEqual(errors(animate('tiling.scroll', { w: 40 })), []);
  assert.ok(errors(clip('x', Array.from({ length: 20 }, (_, i) => pose({ dx: i })))).some((f) => f.rule === 'frames'), 'without wrap the limit is 16');
  assert.ok(checkShape({ ...s, wrap: false }).some((m) => /wrap/.test(m)));
});

test('motion types: bad types, presets and params are refused with a message', () => {
  assert.throws(() => motionClip('figure', 'dance'), /unknown motion "dance" for a figure; known: breathe/);
  assert.throws(() => motionClip('mineral', 'bob'), /unknown motion type/);
  assert.throws(() => animate('object.bob', { wind: 1 }), /unknown param "wind"/);
  assert.throws(() => animate('object.bob', { amount: 1.5 }), /amount must be an integer/);
  assert.throws(() => animate('object.bob', { steps: 1 }), /steps must be an integer from 2 to 32/);
  assert.throws(() => animate('tiling.scroll', { direction: 'sideways' }), /unknown scroll direction/);
  assert.throws(() => animate('tiling.scroll', { direction: 3 }), /must be a string/);
  assert.throws(() => animate('panel.slide-in', { w: 1 }), /w must be an integer/);
  assert.throws(() => lintMotionType(animate('walk'), 'mineral'), /unknown motion type/);
});

test('motion types: the lint reads a type\'s channels and limits, and a wrap has no seam', () => {
  const c = animate('panel.jolt');
  assert.deepEqual(lintClip(c), [], 'a panel moves by dx');
  const leaning = { ...c, frames: c.frames.map((f) => ({ ...f, lean: 1 })) };
  assert.ok(lintClip(leaning).some((f) => f.level === 'error' && f.rule === 'channel' && /panel does not use "lean"/.test(f.message)));
  assert.equal(lintClip(leaning).filter((f) => f.rule === 'channel').length, 1, 'one finding per channel, not per frame');
  assert.ok(lintClip(animate('structure.quake', { amount: 3 })).some((f) => f.level === 'warn' && f.rule === 'range'), 'a quake of 3 is past a 32-row tower\'s 1');
  assert.deepEqual(lintClip(animate('structure.quake', { amount: 1 })), []);
  assert.ok(lintClip(animate('structure.quake', { amount: 3, w: 128 })).every((f) => f.rule !== 'range'), 'the limit is a share of the art');
  assert.ok(lintMotionType(animate('figure.hop'), 'head').some((f) => f.rule === 'channel'), 'a head does not use legs or air');
  assert.deepEqual(lintMotionType(animate('figure.hop'), 'figure'), []);
  assert.deepEqual(lintClip(animate('tiling.scroll')).filter((f) => f.level === 'error'), [], 'the wrap from the last frame to the first is not a pop');
  const walk = animate('walk');
  assert.deepEqual(lintClip({ ...walk, frames: walk.frames.map((f) => ({ ...f, dx: 20 })) }).map((f) => f.rule), ['range', 'range', 'range', 'range'], 'locomotion keeps the sprite limits');
});

test('bind: poses drawn into a clip with the shape of Painter\'s clip record', () => {
  const calls = [];
  const b = bind(animate('walk'), (p, i) => { calls.push(i); return `frame${i}:${p.legs}`; });
  assert.deepEqual(Object.keys(b).sort(), ['events', 'fps', 'frames', 'holds', 'id', 'loop', 'reduced']);
  assert.equal(b.frames.length, 4);
  assert.deepEqual(calls.slice(0, 4), [0, 1, 2, 3]);
  assert.equal(b.frames[0], 'frame0:-2,2');
  assert.throws(() => bind(animate('walk'), () => null), /returned nothing/);
  assert.throws(() => bind(animate('walk')), /drawFrame/);
});

test('motion: every variant\'s clip is a valid `motion` for any Painter recipe', () => {
  // Painter's motionClips (1.19) takes a clip as is: poses on its channels, one integer hold per frame, events on frames.
  const keys = ['dx', 'dy', 'lean', 'head', 'squat', 'lift', 'drag', 'sway', 'legs', 'arm', 'weapon', 'air'];
  assert.deepEqual([...CHANNELS].sort(), [...keys].sort());
  for (const id of RECIPE_IDS) for (const v of RECIPES[id].variants()) {
    const kind = id, c = animate(id, v);
    for (const f of [...c.frames, ...(c.reduced ?? [])]) for (const k of Object.keys(f)) assert.ok(keys.includes(k), `${kind}: ${k}`);
    assert.equal(c.holds.length, c.frames.length);
    assert.ok(c.holds.every((h) => Number.isInteger(h) && h >= 1));
    assert.ok(c.events.every((e) => e.frame >= 0 && e.frame < c.frames.length));
  }
});

test('rng: the same seed gives the same stream, and a fork leaves its parent alone', () => {
  assert.deepEqual([rng('a').int(1000), rng('a').int(1000)].length, 2);
  assert.equal(rng('a').next(), rng('a').next());
  const a = rng('s'), b = rng('s'); a.fork('x').next();
  assert.equal(a.next(), b.next());
});

test('kits: every row is covered, a gap or study-only, and none throws', () => {
  const [k] = coverage();
  assert.deepEqual(k.rows.flatMap((r) => r.problems), []);
  assert.deepEqual(k.rows.filter((r) => ['walk', 'run', 'jump'].includes(r.id)).map((r) => r.status), ['covered', 'covered', 'covered']);
  assert.ok(k.rows.find((r) => r.status === 'reference-only'));
  const bad = { id: 'bad', title: 'Bad', doc: 'x', subjects: [{ id: 'a', label: 'a', recipes: [{ recipe: 'nope' }] }, { id: 'b', label: 'b', recipes: [{ recipe: 'walk', params: {} }] }, { id: 'c', label: 'c' }, { id: 'd', label: 'd', reference: 'study' }] };
  assert.match(subjectStatus(bad.subjects[0]).problems[0], /unknown recipe/);
  assert.equal(subjectStatus(bad.subjects[1]).status, 'covered');
  assert.equal(subjectStatus(bad.subjects[2]).status, 'gap');
  assert.equal(subjectStatus(bad.subjects[3]).status, 'reference-only');
  registerKit(bad);
  assert.throws(() => registerKit(bad), /already registered/);
  KITS.pop();
});

test('the package is self-contained: it imports nothing from outside it', () => {
  for (const f of files().filter((x) => !x.includes('/test/'))) for (const m of readFileSync(f, 'utf8').matchAll(/from '([^']+)'/g)) {
    if (!m[1].startsWith('.')) { assert.ok(m[1].startsWith('node:'), `${f} imports ${m[1]}: only node: built-ins are allowed`); continue; }
    assert.ok(resolve(join(f, '..'), m[1]).startsWith(resolve(ROOT)), `${f} imports ${m[1]} from outside the package`);
  }
  const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
  assert.equal(pkg.dependencies, undefined);
  assert.equal(pkg.devDependencies, undefined);
});

test('the main entry is browser-safe: no node: import is reachable from index.js', () => {
  const seen = new Set(), users = [];
  const visit = (f) => {
    if (seen.has(f)) return; seen.add(f);
    for (const m of readFileSync(f, 'utf8').matchAll(/(?:from|import)\s*'([^']+)'/g)) { if (m[1].startsWith('node:')) users.push(f); else if (m[1].startsWith('.')) visit(resolve(join(f, '..'), m[1])); }
  };
  visit(join(ROOT, 'index.js'));
  assert.deepEqual(users, []);
  assert.ok(!seen.has(join(ROOT, 'cli.js')));
});

test('nothing varies between runs or machines: no Math.random, Date, clocks or trigonometry in the source', () => {
  for (const f of files().filter((x) => !x.includes('/test/'))) {
    const src = readFileSync(f, 'utf8').split('\n').filter((l) => !/^\s*\/\//.test(l)).join('\n');
    assert.doesNotMatch(src, /Math\.(random|sin|cos|tan|atan2?|exp|pow|sqrt|hypot)|\bDate\b|performance\.now|process\.hrtime|setTimeout/, f);
  }
});

test('no file leaks this machine: no /Users/ or home paths in source, docs or tests', () => {
  for (const f of [...files(ROOT, '.js'), ...files(ROOT, '.md'), ...files(ROOT, '.ts'), ...files(ROOT, '.json')]) {
    if (f.endsWith('animator.test.js')) continue;
    assert.doesNotMatch(readFileSync(f, 'utf8'), /\/Users\/|\/home\/[a-z]|C:\\Users/, f);
  }
});

test('index.d.ts names every export of index.js, and CHANNELS match the Pose type', () => {
  const d = readFileSync(join(ROOT, 'index.d.ts'), 'utf8');
  for (const k of Object.keys(api)) assert.match(d, new RegExp(`\\b${k}\\b`), `${k} is in index.d.ts`);
  const iface = d.match(/interface Pose \{([^}]*)\}/)[1];
  for (const c of CHANNELS) assert.match(iface, new RegExp(`\\b${c}\\b`), `${c} is in the Pose type`);
});
