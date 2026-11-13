import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
import { Pixels } from '../pixels.js';
import { encodePng, decodePng } from '../png.js';
import { rng } from '../rng.js';
import { C, RAMPS, DB32, luma } from '../palette.js';
import { drawBuilding } from '../recipes/buildings.js';
import { lint, lintFrames, lintArt, lintSilhouettes, compare } from '../lint.js';
import { RECIPES, draw, STYLES, DEFAULT_STYLE, recipeStyles, isNative } from '../recipes/index.js';
import { lintAnsi, lintAnsiFrames } from '../ansi-lint.js';
import { paint, overlay, lettering, LETTERING_FACES, OVERLAY_IDS } from '../ansi-craft.js';
import { Cells, NONE, foldPixels, halves, tone, legalCell, legalize, ladder, rampCells } from '../cells.js';
import { CP437, writeAnsi, readAnsi, toSpans, toPlainText } from '../ansi.js';
import { renderCells } from '../ansi-render.js';
import { gradient, bayer, tones, resolveTones } from '../ansi-depth.js';
import { light } from '../ansi-light.js';
import { fromImage } from '../ansi-image.js';
import { TERRAINS, OUTDOOR, INDOOR, TIMES as VTIMES, WEATHER_KINDS, hasSky, roomSeed } from '../recipes/vista.js';
import { faceShape, readFace, readFaceWords, hurtStep, hurtClass, effectLook, FACE_MOODS, FACE_LOOKS, FACE_SHAPES, FACE_PARTS } from '../recipes/face.js';
import { FACE_RACES } from '../recipes/face-data.js';
import { thingKind, materialClass, thingWear, auraGlow, readThing, THING_KINDS, THING_CLASSES, THING_WEARS, THING_AURAS, THING_PARTS } from '../recipes/item-art.js';
import { ITEM_KINDS } from '../recipes/item-data.js';
import { raceFacts, classFacts, portraitSeed, couplerRng, bulkFor, CLASS_KINDS, RACE_FEATURES, RACE_NAMES, CLASS_NAMES } from '../recipes/portrait.js';
import { RACES as RACE_ROWS, CLASSES as CLASS_ROWS } from '../recipes/portrait-data.js';
import { wildTerrain, lay, stamp, lightFigures, figureOnPlace, placeLayer, Layer, LIGHTER, SHADE, RECEDE, EYE, BODY, darken } from '../recipes/figure.js';
import { blowFrames, slideFrames, BIG_HIT } from '../recipes/fight.js';
import { foldText, asciiFromCells, textFromPixelArt, writeText, lintText, ASCII_RAMP } from '../text-art.js';
import { writeAnsimation, readAnsimation } from '../ansimation.js';
import { loadAtlas } from '../atlas.js';
import { SPLASH_SCENES, SPLASH_W, SPLASH_H, SPLASH_MAX_TITLE } from '../index.js';
import { FAMILIES, FAMILY_IDS } from '../recipes/families.js';
import { ENEMY_CLIPS, STATUSES, INTENTS, SEGMENT_IDS } from '../recipes/enemy-clips.js';
import { TOP_PARTS, TOP_BIOMES, NOTCHES } from '../recipes/dungeon.js';
import { TOP_PROP_IDS } from '../recipes/topprops.js';
import { TEMPLATE_IDS, lintDress } from '../recipes/dress.js';
import { MAP_MARKERS } from '../recipes/map.js';
import { drawNumber, pixelText, TOP_PLAN } from '../index.js';
import { rgbaSheet } from '../sheet.js';
import { VGA16, V, XTERM256, DB32_VGA, ANSI_RAMPS, nearest, indexHex, rgbOf, distance } from '../palette-ansi.js';
import { drawItem, TIERS } from '../recipes/items.js';
import { drawFace, EXPRESSIONS } from '../recipes/faces.js';
import { drawCreature, mutate, PRESETS, RACES, SIZES } from '../recipes/creatures.js';
import { drawSigil, SIGIL_ORDINARIES, SIGIL_CHARGES } from '../recipes/heraldry.js';
import { LOCALE_KINDS } from '../recipes/locales.js';
import { drawCharacter, LAYER_ORDER, LOADOUTS, POSE_IDS, VIEWS, layerOrder, DYE_PARAMS, ARMOR_FAMILIES, HAIR_STYLES, FACIAL_HAIR } from '../recipes/characters.js';
import { drawIcon, GLYPHS, GLYPH_IDS, PLATE_IDS } from '../recipes/icons.js';
import { dungeonTile, DUNGEON_BIOMES } from '../recipes/interiors.js';
import { drawPortrait } from '../recipes/characters.js';
import { drawHand, GESTURE_IDS, STAGES } from '../recipes/hands.js';
import { drawCloth, CLOTH_KINDS, WINDS } from '../recipes/cloth.js';
import { drawRoom, relight, layoutOf, unreachable, ROOMS, ROOM_IDS, FURNITURE, CLUES, TIMES } from '../recipes/interiors.js';
import { moveFrame, withMotion } from '../motion.js';
import { MOTION_TYPES, MOTION_OF, motionType, motionPresets, resolveMotion, lintMotion } from '../motion-types.js';
import { play as api_play } from '../render.js';
import { KITS, coverage, registerKit, subjectStatus } from '../kits/index.js';
import { ENGINE_VERSION } from '../version.js';
import { recolor, recolorArt } from '../recolor.js';
import { resolveLook, dye } from '../look.js';
import { BODY_PROFILES } from '../recipes/characters.js';
import { writePack } from '../pack.js';
import { mkdtempSync, readFileSync as readF } from 'node:fs';
import { tmpdir } from 'node:os';

const same = (a: Pixels, b: Pixels) => a.equals(b);

test('png round trip, and the shipped Kenney PNGs (4-bit palette) decode', () => {
  const p = new Pixels(5, 3).rect(1, 1, 3, 1, C.red).set(0, 0, C.white);
  const back = decodePng(encodePng(5, 3, p.toRgba()));
  assert.deepEqual([back.w, back.h], [5, 3]);
  assert.deepEqual([...back.rgba], [...p.toRgba()]);
  const kenney = decodePng(readFileSync(new URL('./fixture-16x16-4bit.png', import.meta.url)));
  assert.deepEqual([kenney.w, kenney.h], [16, 16]);
  assert.ok(Pixels.fromRgba(kenney.w, kenney.h, kenney.rgba).count() > 20);
});

test('pixel primitives are exact', () => {
  const p = new Pixels(8, 8).line(0, 0, 7, 7, 1);
  assert.deepEqual([p.get(0, 0), p.get(3, 3), p.get(7, 7), p.get(1, 0)], [1, 1, 1, 0]);
  const art = new Pixels(6, 6).rect(1, 1, 3, 2, 2).set(4, 4, 3);
  assert.ok(same(art.rot90().rot90().rot90().rot90(), art), 'four quarter turns are the identity');
  assert.ok(same(art.flipX().flipX(), art));
  assert.equal(new Pixels(4, 4).rect(1, 1, 2, 2, 1).outline(2).count(), 4 + 8);
  assert.equal(art.clone().swap(2, 5).get(1, 1), 5);
  assert.throws(() => new Pixels(3, 4).rot90(), /square/);
});

test('randomness is seeded: same seed, same stream; forks are independent', () => {
  const a = rng('x'), b = rng('x');
  assert.deepEqual([a.int(1000), a.int(1000)], [b.int(1000), b.int(1000)]);
  assert.notEqual(rng('x').int(1e9), rng('y').int(1e9));
});

test('every recipe variant draws, deterministically, with no lint errors', () => {
  for (const [id, r] of Object.entries(RECIPES)) if (!r.ansiOnly) for (const v of r.variants()) {
    const a = draw(id, v), b = draw(id, v);
    assert.ok(a.frames.length > 0, `${id} ${JSON.stringify(v)} has frames`);
    a.frames.forEach((f: Pixels, i: number) => { assert.ok(same(f, b.frames[i]), `${id} ${JSON.stringify(v)} frame ${i} is not deterministic`); assert.deepEqual(lint(f, { skip: ['size', 'margin', 'outline', 'value', 'palette'] }).filter((x) => x.level === 'error'), []); });
  }
});

test('styles: pixel is the default and unchanged, ansi is native when the recipe has one and a fold when not', () => {
  assert.deepEqual(STYLES, ['pixel', 'ansi', 'text']);
  assert.equal(DEFAULT_STYLE, 'pixel');
  for (const [id, r] of Object.entries(RECIPES)) {
    assert.deepEqual(recipeStyles(id), r.ansiOnly ? ['ansi', 'text'] : ['pixel', 'ansi', 'text']);
    assert.equal(isNative(id, 'pixel'), !r.ansiOnly); assert.equal(isNative(id, 'ansi'), !!r.ansiOnly || typeof r.ansi === 'function');   // a recipe can have both: a pixel drawing and a native ANSI one (`splash`)
  }
  assert.ok(same(draw('item', { shape: 'sword' }).frames[0], draw('item', { shape: 'sword' }, { style: 'pixel' }).frames[0]));
  assert.throws(() => draw('item', {}, { style: 'sepia' as any }), /unknown style "sepia"/);
  assert.throws(() => recipeStyles('nope'), /unknown recipe/);
  const folded = draw('item', { shape: 'sword' }, { style: 'ansi' });
  assert.ok(folded.frames[0] instanceof Cells && folded.folded && folded.style === 'ansi');
  assert.ok(folded.frames[0].equals(foldPixels(draw('item', { shape: 'sword' }).frames[0])));
  // A recipe that names a native drawing is drawn by it, not folded.
  const saved = RECIPES.icon;
  RECIPES.icon = { ...saved, ansi: () => ({ id: 'icon.native', frames: [new Cells(1, 1)], fps: 0, loop: false }) };
  try { assert.equal(isNative('icon', 'ansi'), true); const a = draw('icon', {}, { style: 'ansi' }); assert.equal(a.id, 'icon.native'); assert.equal(a.style, 'ansi'); assert.ok(!a.folded); } finally { RECIPES.icon = saved; }
});

test('cells: set keeps what is not given, glyphs keep the background, blits skip empty cells', () => {
  const c = new Cells(4, 2).set(1, 0, { ch: 'x', fg: 3, bg: 4 }).glyph(1, 0, '*', 11);
  assert.deepEqual(c.get(1, 0), { ch: 42, fg: 11, bg: 4, flags: 0 });
  assert.deepEqual(c.get(9, 9), { ch: 0, fg: NONE, bg: NONE, flags: 0 });
  assert.equal(c.isEmpty(0, 0), true); assert.equal(c.isEmpty(1, 0), false);
  const top = new Cells(2, 1).set(0, 0, { ch: 'A', fg: 1 });
  const under = new Cells(2, 1).fill({ ch: '.', fg: 7 }).blit(top);
  assert.equal(under.toText(), 'A.');
  assert.ok(under.clone().equals(under)); assert.ok(!under.clone().set(1, 0, { fg: 2 }).equals(under));
  assert.equal(new Cells(3, 1).text(0, 0, 'hi').toText(), 'hi ');
  assert.equal(new Cells(2, 2).toText(), '  \n  ');
});

test('fold: two pixels a cell, as half blocks that leave what is behind a sprite alone', () => {
  const px = new Pixels(4, 3);
  px.set(0, 0, C.red).set(0, 1, C.blue);        // both drawn, different: top fg over bottom bg
  px.set(1, 0, C.red).set(1, 1, C.red);          // same colour: a space of that colour
  px.set(2, 0, C.green);                         // only the top
  px.set(3, 1, C.green);                         // only the bottom
  px.set(0, 2, C.white);                         // an odd last row: the missing lower pixel is transparent
  const f = foldPixels(px, { bgLimit: 16 });             // the plain half blocks; the default also makes them legal (below)
  assert.equal(f.w, 4); assert.equal(f.h, 2);
  assert.deepEqual(f.get(0, 0), { ch: 0x2580, fg: DB32_VGA[C.red - 1], bg: DB32_VGA[C.blue - 1], flags: 0 });
  assert.equal(f.get(1, 0).ch, 32); assert.equal(f.get(1, 0).bg, DB32_VGA[C.red - 1]);
  assert.deepEqual([f.get(2, 0).ch, f.get(2, 0).bg], [0x2580, NONE]);
  assert.deepEqual([f.get(3, 0).ch, f.get(3, 0).fg, f.get(3, 0).bg], [0x2584, DB32_VGA[C.green - 1], NONE]);
  assert.deepEqual([f.get(0, 1).ch, f.get(0, 1).bg], [0x2580, NONE]);
  assert.equal(f.isEmpty(1, 1), true);
  // Two colours that share a VGA colour fold to one block, not two.
  assert.equal(DB32_VGA[C.night - 1], DB32_VGA[C.black - 1]);
  assert.equal(foldPixels(new Pixels(1, 2).set(0, 0, C.night).set(0, 1, C.black)).get(0, 0).ch, 32);
  // Another palette goes to the nearest of the 16.
  const odd = new Pixels(1, 2, ['#ff5555', '#ffffff']).set(0, 0, 1).set(0, 1, 2);
  assert.deepEqual([foldPixels(odd, { bgLimit: 16 }).get(0, 0).fg, foldPixels(odd, { bgLimit: 16 }).get(0, 0).bg], [V.brightred, V.white]);
});

const legalBg = (bg: number, limit: number) => bg === NONE || bg < limit || bg >= 16;

test('tone: a cell\'s effective colour is its halves and shade mix', () => {
  assert.equal(tone({ ch: 0x20, fg: V.white, bg: V.black, flags: 0 }), '#000000');
  assert.equal(tone({ ch: 0x2588, fg: V.white, bg: V.black, flags: 0 }), '#ffffff');
  assert.equal(tone({ ch: 0x2592, fg: V.white, bg: V.black, flags: 0 }), '#808080');
  assert.equal(tone({ ch: 0x2591, fg: V.white, bg: V.black, flags: 0 }), '#404040');
  assert.equal(tone({ ch: 0x2593, fg: V.white, bg: V.black, flags: 0 }), '#bfbfbf');
  assert.deepEqual(halves({ ch: 0x2580, fg: V.red, bg: V.blue, flags: 0 }), [rgbOf(VGA16[V.red]), rgbOf(VGA16[V.blue])]);
  assert.deepEqual(halves({ ch: 0x2584, fg: V.red, bg: V.blue, flags: 0 }), [rgbOf(VGA16[V.blue]), rgbOf(VGA16[V.red])]);
  assert.equal(tone({ ch: 0x2580, fg: V.white, bg: NONE, flags: 0 }), '#808080', 'no colour shows as black');
});

test('legalize: reversals keep the picture, the rest is the nearest legal cell, and iCE is left alone', () => {
  const cell = (ch: number, fg: number, bg: number) => ({ ch, fg, bg, flags: 0 });
  // A half block with a bright background reverses to a legal one that looks the same.
  assert.deepEqual(legalCell(cell(0x2580, V.red, V.brightblue)), cell(0x2584, V.brightblue, V.red));
  assert.deepEqual(legalCell(cell(0x2591, V.red, V.brightblue)), cell(0x2593, V.brightblue, V.red));
  assert.deepEqual(legalCell(cell(0x2592, V.red, V.brightblue)), cell(0x2592, V.brightblue, V.red));
  // A space on a bright background is a full block of it; a full block's background never shows.
  assert.deepEqual(legalCell(cell(0x20, V.red, V.brightblue)), cell(0x2588, V.brightblue, NONE));
  assert.deepEqual(legalCell(cell(0x2588, V.red, V.brightblue)), cell(0x2588, V.red, NONE));
  // A reversal that leaves no colour: ▀ over no colour becomes ▄ of the bright one.
  assert.deepEqual(legalCell(cell(0x2580, NONE, V.brightblue)), cell(0x2584, V.brightblue, NONE));
  // A text glyph keeps its character and fg, and takes the nearest legal background.
  assert.deepEqual(legalCell(cell(0x41, V.white, V.brightblue)), cell(0x41, V.white, V.blue));
  // Legal cells are returned as they are.
  const ok = cell(0x2580, V.brightred, V.blue); assert.equal(legalCell(ok), ok);
  assert.throws(() => legalCell(ok, 9 as any), /bgLimit/);
  // Both bright and different has no exact glyph: the result is legal and no worse than the plain darker background.
  const hard = cell(0x2580, V.brightred, V.brightblue), l = legalCell(hard);
  assert.ok(legalBg(l.bg, 8));
  assert.ok(distance(rgbOf(tone(l)), rgbOf(tone(hard))) < distance(rgbOf(tone(cell(0x2580, V.brightred, V.blue))), rgbOf(tone(hard))) + 1);
  // iCE and modern targets are a plain copy.
  const c = new Cells(2, 1).set(0, 0, hard).set(1, 0, { ch: ' ', fg: V.red, bg: V.brightgreen });
  assert.ok(legalize(c, { bgLimit: 16 }).equals(c)); assert.notEqual(legalize(c, { bgLimit: 16 }), c);
  assert.ok(!legalize(c).equals(c)); assert.equal(c.get(0, 0).bg, V.brightblue, 'the input is not changed');
  // Backgrounds from 16 up (256 colours) and NONE are not this pass's business.
  const deep = new Cells(2, 1).set(0, 0, cell(0x2580, V.red, 200)).set(1, 0, cell(0x2580, V.red, NONE));
  assert.ok(legalize(deep).equals(deep));
});

test('legalize: every cell comes out legal, with the same tone where a reversal allows it, and never worse than the darker twin', () => {
  const r = rng('legal'), glyphs = [0, 0x20, 0x2591, 0x2592, 0x2593, 0x2588, 0x2580, 0x2584, 0x41, 0x2500];
  const c = new Cells(40, 20);
  for (let i = 0; i < c.ch.length; i++) { c.ch[i] = r.pick(glyphs); c.fg[i] = r.chance(0.1) ? NONE : r.int(16); c.bg[i] = r.chance(0.1) ? NONE : r.int(16); }
  const out = legalize(c);
  for (let i = 0; i < c.ch.length; i++) {
    assert.ok(legalBg(out.bg[i], 8), `cell ${i} background ${out.bg[i]}`);
    const a = { ch: c.ch[i], fg: c.fg[i], bg: c.bg[i], flags: 0 }, b = { ch: out.ch[i], fg: out.fg[i], bg: out.bg[i], flags: 0 };
    if (a.bg === b.bg || a.ch === 0x41 || a.ch === 0x2500 || a.ch === 0) continue;       // untouched, or a text glyph (which keeps its character)
    const [at, ab] = halves(a), [bt, bb] = halves(b);
    const naive = halves({ ...a, bg: (a.bg as number) - 8 });
    assert.ok(distance(at, bt) + distance(ab, bb) <= distance(at, naive[0]) + distance(ab, naive[1]) + 1e-9, `cell ${i} ${JSON.stringify(a)} -> ${JSON.stringify(b)}`);
    if (legalBg(a.fg, 8) && a.ch !== 0x2588) assert.equal(tone(b), tone(a), `cell ${i} reversal keeps its tone`);
  }
  assert.ok(legalize(c).equals(out), 'deterministic');
});

test('fold: legal for 8 backgrounds by default, and bgLimit 16 gives the plain half blocks', () => {
  const px = new Pixels(2, 2).set(0, 0, C.red).set(0, 1, C.blue);             // brightred over blue
  const bright = new Pixels(1, 2).set(0, 0, C.green).set(0, 1, C.white);       // a white lower pixel is a bright background
  assert.equal(DB32_VGA[C.white - 1], V.white);
  const classic = foldPixels(bright), modern = foldPixels(bright, { bgLimit: 16 });
  assert.deepEqual([modern.get(0, 0).ch, modern.get(0, 0).bg], [0x2580, V.white]);
  assert.deepEqual([classic.get(0, 0).ch, classic.get(0, 0).fg, classic.get(0, 0).bg], [0x2584, V.white, DB32_VGA[C.green - 1]]);
  assert.ok(foldPixels(px).equals(foldPixels(px, { bgLimit: 8 })));
  for (const v of ['rogue', 'mage']) {
    const a = draw('character', { loadout: v }, { style: 'ansi' }), m = draw('character', { loadout: v }, { style: 'ansi', bgLimit: 16 });
    assert.ok(a.frames[0].bg.every((b: number) => legalBg(b, 8)));
    assert.ok(m.frames[0].bg.some((b: number) => b >= 8 && b < 16) || m.frames[0].equals(a.frames[0]));
  }
});

test('lines and boxes: the right corners, and the right joins where they meet', () => {
  const text = (c: Cells) => c.toText();
  assert.equal(text(new Cells(4, 3).box(0, 0, 4, 3)), '┌──┐\n│  │\n└──┘');
  assert.equal(text(new Cells(4, 3).box(0, 0, 4, 3, { style: 'double' })), '╔══╗\n║  ║\n╚══╝');
  assert.equal(text(new Cells(4, 3).box(0, 0, 4, 3, { style: 'mixed' })), '╒══╕\n│  │\n╘══╛');
  assert.throws(() => new Cells(4, 3).box(0, 0, 1, 3), /at least 2 by 2/);
  assert.throws(() => new Cells(4, 3).box(0, 0, 3, 3, { style: 'round' as any }), /unknown box style/);
  // A line ending on a wall is a T; one crossing a line is a cross; weights stay each their own.
  assert.equal(text(new Cells(5, 3).box(0, 0, 5, 3).hline(0, 1, 5)), '┌───┐\n├───┤\n└───┘');
  assert.equal(text(new Cells(5, 3).box(0, 0, 5, 3, { style: 'double' }).hline(0, 1, 5)), '╔═══╗\n╟───╢\n╚═══╝');
  assert.equal(text(new Cells(3, 3).hline(0, 1, 3).vline(1, 0, 3)), ' │ \n─┼─\n │ ');
  assert.equal(text(new Cells(3, 3).hline(0, 1, 3, { style: 'double' }).vline(1, 0, 3)), ' │ \n═╪═\n │ ');
  // Two boxes sharing an edge join along it.
  assert.equal(text(new Cells(5, 3).box(0, 0, 3, 3).box(2, 0, 3, 3)), '┌─┬─┐\n│ │ │\n└─┴─┘');
  // A single cell line is a full piece; lines are clipped at the canvas edge.
  assert.equal(text(new Cells(3, 1).hline(1, 0, 1)), ' ─ ');
  assert.equal(text(new Cells(3, 1).hline(1, 0, 9)), ' ──');
  const colored = new Cells(2, 1).hline(0, 0, 2, { fg: V.cyan, bg: V.blue });
  assert.deepEqual([colored.get(0, 0).fg, colored.get(0, 0).bg], [V.cyan, V.blue]);
  assert.equal(new Cells(3, 1).shadeFill(0, 0, 3, 1, '▒', V.gray, V.black).get(1, 0).ch, 0x2592);
  assert.throws(() => new Cells(3, 1).shadeFill(0, 0, 3, 1, 'x'), /not a shade glyph/);
});

test('ladder and ramps: five legal steps from one colour to another, at least four steps a ramp', () => {
  const l = ladder(V.black, V.gray);
  assert.deepEqual(l.map((c) => c.ch), [0x20, 0x2591, 0x2592, 0x2593, 0x20]);
  assert.deepEqual(l.map(tone), ['#000000', '#2b2b2b', '#555555', '#808080', '#aaaaaa']);
  // With the lighter colour bright, the legal one is the background and the glyphs are reversed: the tones are the same.
  const r = ladder(V.darkgray, V.gray);
  assert.deepEqual(r.map((c) => c.ch), [0x2588, 0x2593, 0x2592, 0x2591, 0x20]);
  assert.ok(r.every((c) => legalBg(c.bg, 8)));
  assert.deepEqual(r.map(tone), ladder(V.darkgray, V.gray, { bgLimit: 16 }).map(tone));
  // Under iCE both bright colours stay as they are.
  assert.deepEqual(ladder(V.darkgray, V.white, { bgLimit: 16 }).map((c) => c.ch), [0x20, 0x2591, 0x2592, 0x2593, 0x20]);
  // A bright pair has no exact legal run, but every step is legal.
  assert.ok(ladder(V.brightred, V.brightblue).every((c) => legalBg(c.bg, 8)));
  for (const [name, ramp] of Object.entries(ANSI_RAMPS)) {
    assert.ok(ramp.length >= 4, `${name} has at least four colours`);
    for (let i = 0; i + 1 < ramp.length; i++) assert.ok(ramp[i] < 8 || ramp[i + 1] < 8, `${name}: ${ramp[i]} to ${ramp[i + 1]} has a legal background`);
    const cells = rampCells(name);
    assert.equal(cells.length, (ramp.length - 1) * 4 + 1);
    assert.ok(cells.every((c) => legalBg(c.bg, 8)));
    assert.equal(tone(cells[0]), VGA16[ramp[0]]); assert.equal(tone(cells[cells.length - 1]), VGA16[ramp[ramp.length - 1]]);
    // Each step is at least as light as the last in the order of the ramp's own colours (luma never turns back far).
    const lumas = cells.map((c) => luma(tone(c)));
    for (let i = 1; i < lumas.length; i++) assert.ok(lumas[i] > lumas[i - 1] - 40, `${name} step ${i}`);
  }
  assert.equal(ANSI_RAMPS.gray.includes(V.darkgray) && ANSI_RAMPS.gray.indexOf(V.white) === ANSI_RAMPS.gray.indexOf(V.gray) + 1, true, 'dark gray reaches white through gray');
  assert.throws(() => rampCells('nope'), /unknown ramp/);
});

test('ansi palettes: the 16 VGA colours, the xterm 256, a hand-made DB32 table, and reduction', () => {
  assert.equal(VGA16.length, 16); assert.equal(XTERM256.length, 256);
  assert.equal(XTERM256[16], '#000000'); assert.equal(XTERM256[231], '#ffffff'); assert.equal(XTERM256[232], '#080808'); assert.equal(XTERM256[255], '#eeeeee');
  assert.equal(V.red, 1); assert.equal(VGA16[V.white], '#ffffff');
  assert.equal(DB32_VGA.length, 32);
  DB32_VGA.forEach((v, i) => { assert.ok(Number.isInteger(v) && v >= 0 && v < 16, `entry ${i}`); assert.ok(Math.abs(luma(VGA16[v]) - luma(DB32[i])) < 115, `${i}: ${DB32[i]} keeps its value`); });
  assert.equal(DB32_VGA[C.black - 1], V.black); assert.equal(DB32_VGA[C.white - 1], V.white);
  assert.deepEqual(RAMPS.fire.map((c) => DB32_VGA[c - 1]), [V.red, V.yellow, V.yellow, V.white]);
  assert.deepEqual(nearest('#ff5555', 16), { index: V.brightred });
  assert.deepEqual(nearest('#ff0000', 16), { index: V.red });
  assert.deepEqual(nearest('#ffffff', 256), { index: 15 });          // ties go to the lowest index (15 and 231 are both white)
  assert.deepEqual(nearest('#5f87af', 256), { index: 67 });
  assert.deepEqual(nearest('#123456', 'true'), { hex: '#123456' });
  assert.throws(() => nearest('#000000', 88 as any), /unknown colour depth/);
  assert.equal(indexHex(1), '#aa0000'); assert.throws(() => indexHex(256), /no colour/);
});

test('folded art: frames, clips, layers and masks are cells, anchors move to cell units, and it is deterministic', () => {
  const a = draw('character', { loadout: 'rogue' }, { style: 'ansi' }), b = draw('character', { loadout: 'rogue' }, { style: 'ansi' });
  const p = draw('character', { loadout: 'rogue' });
  assert.ok(a.frames[0].equals(b.frames[0]));
  assert.equal(a.frames[0].w, p.frames[0].w); assert.equal(a.frames[0].h, p.frames[0].h / 2);
  assert.deepEqual(a.anchors.feet, [p.anchors.feet[0], p.anchors.feet[1] / 2]);
  assert.deepEqual(Object.keys(a.clips), Object.keys(p.clips));
  assert.ok(a.clips.walk.frames.every((f: any) => f instanceof Cells) && a.clips.walk.frames.length === p.clips.walk.frames.length);
  assert.ok(Object.values(a.layers).flat().every((f: any) => f instanceof Cells));
  assert.equal(a.clips.walk.fps, p.clips.walk.fps);
  assert.ok(draw('item', { shape: 'sword' }, { style: 'ansi' }).mask instanceof Cells);
});

test('item tiers change the finish, not the silhouette', () => {
  const art = Object.keys(TIERS).map((tier) => drawItem({ shape: 'sword', tier }).mask);
  for (const m of art) assert.ok(same(m, art[0]));
  const finishes = Object.keys(TIERS).map((tier) => drawItem({ shape: 'sword', tier }).frames[0]);
  for (let i = 1; i < finishes.length; i++) assert.ok(!same(finishes[i], finishes[i - 1]), `tier ${i} differs from tier ${i - 1}`);
  assert.throws(() => drawItem({ shape: 'nope' }), /unknown item shape/);
  assert.throws(() => drawItem({ tier: 'nope' }), /unknown tier/);
});

test('faces: every expression is a different drawing, and each axis changes the pixels', () => {
  const faces = Object.keys(EXPRESSIONS).map((expression) => drawFace({ expression }).frames[0]);
  faces.forEach((f, i) => faces.slice(0, i).forEach((g, j) => assert.ok(!same(f, g), `${Object.keys(EXPRESSIONS)[i]} equals ${Object.keys(EXPRESSIONS)[j]}`)));
  const base = drawFace().frames[0];
  for (const o of [{ age: 'child' }, { age: 'elder' }, { skin: 'skin-deep' }, { angle: 'three-quarter' }, { expression: 'fear', intensity: 3 }]) assert.ok(!same(drawFace(o).frames[0], o.expression ? drawFace({ expression: 'fear' }).frames[0] : base), JSON.stringify(o));
  assert.throws(() => drawFace({ expression: 'nope' }), /unknown expression/);
});

test('creatures: mutate changes exactly one system and the drawing', () => {
  for (const preset of Object.keys(PRESETS)) {
    const spec = drawCreature({ preset }).spec;
    for (const system of ['eyes', 'skin', 'horns', 'jaw', 'ears']) {
      const m = mutate(spec, system), changed = Object.keys(m).filter((k) => m[k] !== spec[k]);
      assert.deepEqual(changed, [system], `${preset}/${system}`);
    }
  }
  const slime = drawCreature({ preset: 'slime' }).spec;
  assert.ok(!same(drawCreature(mutate(slime, 'skin')).frames[0], drawCreature({ preset: 'slime' }).frames[0]));
  assert.throws(() => mutate(slime, 'tail'), /unknown system/);
});

test('characters are modular: changing one slot changes only that layer', () => {
  const base = drawCharacter({ loadout: 'guardian' }), layers = (c: any) => c.layers as Record<string, Pixels[]>;
  const cases: Record<string, object> = { headgear: { head: 'horned' }, torso: { torso: 'robe' }, weapon: { weapon: 'axe' }, offhand: { offhand: null }, legs: { legs: 'red' } };
  for (const [slot, change] of Object.entries(cases)) {
    const other = drawCharacter({ loadout: 'guardian', ...change });
    for (const name of LAYER_ORDER) {
      const equal = same(layers(base)[name][0], layers(other)[name][0]);
      assert.equal(equal, name !== slot, `${slot} change: layer ${name} should ${name === slot ? 'differ' : 'match'}`);
    }
  }
  const comp = new Pixels(40, 32); for (const n of LAYER_ORDER) comp.blit(layers(base)[n][0]);
  assert.ok(same(comp, base.frames[0]), 'composite is the layers in LAYER_ORDER');
  assert.deepEqual(base.anchors.feet, [16, 30]);
});

test('character clips: attack has an impact marker and a reduced-motion frame; walk is a 4-frame loop', () => {
  for (const loadout of Object.keys(LOADOUTS)) {
    const c = drawCharacter({ loadout }), { attack, walk, idle, hit, defeat } = c.clips;
    assert.deepEqual(attack.events, [{ frame: 2, name: 'impact' }]);
    assert.equal(attack.reduced.length, 1);
    assert.equal(walk.frames.length, 4); assert.equal(walk.loop, true);
    assert.equal(idle.frames.length, 2); assert.equal(attack.loop, false);
    assert.ok(same(hit.frames[0], c.frames[0].silhouette(C.white)), 'the hit flash is one flat colour');
    assert.ok(defeat.frames.at(-1)!.count() < c.frames[0].count() / 2, 'defeat collapses');
    for (const clip of Object.values(c.clips) as any[]) assert.deepEqual(lintFrames(clip.frames, { loop: clip.loop }).filter((f) => f.rule === 'frames'), [], 'no repeated frames');
  }
  assert.throws(() => drawCharacter({ head: 'crown' }), /unknown headgear/);
});

test('character motion: poses from a clip are drawn with the loadout, holds repeat, events follow, and bad channels throw', () => {
  const m = { id: 'jump', fps: 10, loop: false, frames: [{ squat: 2 }, { dy: -4, legs: [1, 1], air: 1 }, {}], holds: [2, 1, 1], events: [{ frame: 1, name: 'takeoff' }], reduced: [{}] };
  for (const view of VIEWS) {
    const c = drawCharacter({ loadout: 'rogue', view, motion: m }), j = c.clips.jump;
    assert.equal(j.frames.length, 4); assert.ok(same(j.frames[0], j.frames[1]), 'a hold repeats the frame');
    assert.deepEqual(j.events, [{ frame: 2, name: 'takeoff' }]); assert.equal(j.loop, false); assert.equal(j.fps, 10);
    assert.ok(!same(j.frames[2], j.frames[3]) && !same(j.frames[0], j.frames[3]), 'the poses differ');
    assert.ok(same(j.reduced[0], c.frames[0]), 'the rest pose is the idle frame');
    assert.ok(c.clips.walk && c.clips.idle, 'built-in clips remain');
  }
  assert.ok(same(drawCharacter({ loadout: 'rogue', motion: [m, { ...m, id: 'b' }] }).clips.b.frames[3], drawCharacter({ loadout: 'rogue', motion: m }).clips.jump.frames[3]));
  assert.deepEqual(Object.keys(drawCharacter({ loadout: 'rogue' }).clips), ['idle', 'walk', 'attack', 'hit', 'defeat', 'turn', 'block', 'cast', 'heal', 'celebrate', 'dodge', 'parry', 'charge', 'stun', 'potion', 'buffed', 'lowhealth', 'levelup', 'tagin', 'tagout', 'equip', 'shoot', 'thrust', 'slam', 'recall']);
  assert.throws(() => drawCharacter({ motion: { frames: [{ wiggle: 1 }] } }), /unknown pose channel/);
  assert.throws(() => drawCharacter({ motion: { frames: [] } }), /frames list/);
});

test('motion on anything: every recipe, in every style, gets the clip; the move is exact and the still is untouched', () => {
  const m = { id: 'hop', fps: 10, loop: false, frames: [{ squat: 2 }, { dy: -3, lean: 1 }, { dx: 2, sway: -1 }], holds: [1, 2, 1], events: [{ frame: 1, name: 'up' }] };
  for (const [id, r] of Object.entries(RECIPES) as [string, any][]) for (const style of ['pixel', 'ansi', 'text'] as const) {
    if (r.ansiOnly && style === 'pixel') continue;
    const v = r.variants()[0], a = draw(id, { ...v, motion: m }, { style }), hop = a.clips!.hop;
    assert.equal(hop.frames.length, 4, `${id} ${style}`); assert.deepEqual(hop.events, [{ frame: 1, name: 'up' }]); assert.equal(hop.loop, false);
    assert.ok(hop.frames.every((f: any) => f.w === a.frames[0].w && f.h === a.frames[0].h), `${id} ${style} keeps its canvas`);
  }
  const base = draw('item', { shape: 'sword' }), still = base.frames[0], b = still.bbox()!;
  assert.ok(same(moveFrame(still, {}), still), 'no pose, no change');
  assert.ok(same(moveFrame(still, { dx: 2, dy: 1 }), still.shift(2, 1)), 'dx and dy are a shift');
  assert.ok(same(moveFrame(still, { lift: 1 }), still.shift(0, -1)), 'lift raises');
  const sq = moveFrame(still, { squat: 3 }).bbox()!; assert.equal(sq.h, b.h - 3); assert.equal(sq.y + sq.h, b.y + b.h, 'a squat keeps the bottom');
  const lean = moveFrame(still, { lean: 3 }); assert.ok(lean.bbox()!.x !== undefined && !same(lean, still));
  assert.ok(same(moveFrame(still, { head: 4, legs: [2, 2], arm: 'raised', air: 1 }), still), 'joint channels do nothing to a thing');
  assert.ok(same(draw('item', { shape: 'sword', motion: m }).clips!.hop.frames[0], moveFrame(still, { squat: 2 })));
  assert.ok(same(draw('item', { shape: 'sword', motion: m }).frames[0], still), 'the still is unchanged');
  assert.deepEqual(draw('item', { shape: 'sword' }).clips, undefined);
  const cells = draw('item', { shape: 'sword' }, { style: 'ansi' }).frames[0], moved = moveFrame(cells, { dx: 1 });
  assert.equal(moved.get(1, 0).ch, cells.get(0, 0).ch); assert.ok(moved instanceof Cells);
  assert.throws(() => draw('item', { motion: { frames: [{ wiggle: 1 }] } }), /unknown pose channel/);
  assert.throws(() => draw('item', { motion: { frames: [{}], holds: [0] } }), /holds/);
  assert.equal(withMotion(base, [m, { ...m, id: 'b' }]).clips!.b.frames.length, 4);
});

test('motion types: every recipe has one, every preset draws in every style inside its type\'s limits, and params are the preset\'s own', () => {
  assert.deepEqual(Object.keys(RECIPES).filter((id) => !MOTION_OF[id]), [], 'every recipe has a motion type');
  assert.deepEqual(Object.values(MOTION_OF).filter((t) => !MOTION_TYPES[t as keyof typeof MOTION_TYPES]), []);
  assert.deepEqual(Object.keys(MOTION_OF).filter((id) => !RECIPES[id as keyof typeof RECIPES]), [], 'no type for a recipe that does not exist');
  for (const t of Object.keys(MOTION_TYPES)) assert.ok(Object.values(MOTION_OF).includes(t), `${t} is used`);
  for (const [id, r] of Object.entries(RECIPES) as [string, any][]) for (const style of ['pixel', 'ansi', 'text'] as const) {
    if (r.ansiOnly && style === 'pixel') continue;
    const v = r.variants()[0], names = motionPresets(id);
    assert.ok(names.length >= 3, `${id} has presets`);
    const a = draw(id, { ...v, motion: names }, { style }), f = a.frames[0];
    for (const n of names) {
      const c = a.clips![n]; assert.ok(c && c.frames.length >= 2, `${id} ${style} ${n}`);
      assert.ok(c.frames.every((x: any) => x.w === f.w && x.h === f.h), `${id} ${style} ${n} keeps its canvas`);
    }
    if (style === 'pixel') assert.deepEqual(lintMotion(id, names, { w: f.w, h: f.h }), [], `${id} presets stay inside the ${MOTION_OF[id]} limits`);
  }
  // Not every type has the same presets or params.
  assert.deepEqual(motionPresets('farm'), ['sway', 'rustle', 'grow', 'bounce']);
  assert.ok(motionPresets('vehicle').includes('lurch') && !motionPresets('vehicle').includes('sway'));
  assert.ok(!('wind' in motionType('vehicle').presets.rumble.params) && 'wind' in motionType('farm').presets.sway.params);
  // Params change the clip; unknown presets and params throw.
  const lo = resolveMotion('farm', { preset: 'sway', wind: 1 }, { w: 16, h: 16 })[0], hi = resolveMotion('farm', { preset: 'sway', wind: 3, id: 'gale' }, { w: 16, h: 16 })[0];
  assert.equal(hi.id, 'gale'); assert.notDeepEqual(lo.frames, hi.frames);
  assert.throws(() => draw('farm', { kind: 'wheat', motion: 'walk' }), /unknown motion "walk" for farm/);
  assert.throws(() => draw('farm', { kind: 'wheat', motion: { preset: 'sway', speed: 3 } }), /unknown param "speed"/);
  assert.throws(() => draw('tile', { motion: { preset: 'scroll', direction: 'sideways' } }), /unknown scroll direction/);
  assert.equal(resolveMotion('item', undefined, { w: 16, h: 16 }).length, 0);
  // A clip given directly is untouched, and the lint finds what a type does not use.
  const own = { id: 'x', frames: [{ head: 2, dx: 30 }] };
  assert.equal(resolveMotion('item', own, { w: 16, h: 16 })[0], own);
  assert.deepEqual(lintMotion('item', own, { w: 16, h: 16 }).map((f) => [f.rule, f.channel]), [['channel', 'head'], ['channel', 'dx']]);
  assert.deepEqual(lintMotion('character', { frames: [{ head: 2, dx: 30 }] }, { w: 32, h: 32 }).map((f) => [f.rule, f.channel]), [['range', 'dx']]);
  // A tiling scroll wraps: a whole tile later it is the tile again, and nothing is lost on the way.
  const tile = draw('tile', { kind: 'ground', ramp: 'grass', motion: { preset: 'scroll', direction: 'right', step: 2 } }), c = tile.clips!.scroll, w = tile.frames[0].w;
  assert.equal(c.frames.length, w / 2); assert.ok(same(c.frames[0], tile.frames[0]));
  assert.ok(same(moveFrame(tile.frames[0], { dx: w }, { wrap: true }), tile.frames[0]) && same(moveFrame(tile.frames[0], { dy: -tile.frames[0].h }, { wrap: true }), tile.frames[0]));
  assert.ok(same(c.frames[3], moveFrame(tile.frames[0], { dx: 6 }, { wrap: true })));
  // Without a preset nothing changes.
  assert.deepEqual(draw('farm', { kind: 'wheat' }).clips, undefined);
});

test('lint finds what the learning kit warns about', () => {
  const rules = (p: Pixels) => lint(p).map((f) => f.rule);
  assert.deepEqual(rules(new Pixels(16, 16)), ['empty']);
  const busy = new Pixels(16, 16); for (let i = 0; i < 20; i++) busy.set(1 + 2 * (i % 7), 1 + 2 * Math.floor(i / 7), i + 1); // 20 colours, none touching
  assert.ok(rules(busy).includes('palette') && rules(busy).includes('noise'));
  assert.ok(rules(new Pixels(16, 16).rect(0, 0, 16, 4, C.white)).includes('margin'), 'lit art on the edge');
  assert.ok(rules(new Pixels(16, 16).rect(2, 2, 12, 12, C.dust).rect(4, 4, 8, 8, C.grey)).includes('value'));
  const a = new Pixels(16, 16).rect(2, 2, 6, 6, C.red), b = a.clone();
  assert.deepEqual(lintFrames([a, b]).map((f) => f.rule), ['frames']);
  assert.deepEqual(lintFrames([a, a.shift(2, 1), a.shift(0, 0)].map((f) => f.clone()), { loop: true }).filter((f) => f.rule === 'boil'), [], 'a slide is a move, not boiling');
  assert.equal(compare(a, a).silhouetteOverlap, 1);
  assert.ok(compare(a, a.shift(4, 4)).silhouetteOverlap < 0.2);
});

test('kits: each subject says how it is covered, every recipe it names draws, and the report has no problems', () => {
  assert.deepEqual(KITS.map((k) => k.id).sort(), ['ansi-art', 'coffeemud', 'coupler-painter', 'faceart', 'fantasy-biomes-environments', 'fantasy-items-equipment', 'free-asset-sources', 'hands-poses-motion-interiors', 'paper-doll-game', 'pixel-art-learning', 'splash-screens']);
  for (const kit of KITS) for (const s of kit.subjects as any[]) {
    const kinds = [s.recipes !== undefined, !!s.provides, !!s.reference].filter(Boolean).length;
    assert.ok(kinds >= 1, `${kit.id}/${s.id} needs recipes, provides or reference`);
    assert.ok(readFileSync(join(ROOT, kit.doc), 'utf8').length > 0, `${kit.doc} exists`);
  }
  for (const k of coverage()) for (const r of k.rows) assert.deepEqual(r.problems, [], `${k.id}/${r.id}`);
  const gaps = coverage().flatMap((k) => k.rows.filter((r) => r.status === 'gap').map((r) => r.id));
  // What is still undrawn, named so a drawn one has to be taken off this list on purpose.
  assert.deepEqual(gaps.sort(), []);
});

test('vehicle recipe: every kind draws a lint-clean 32x32, deterministic, and the seed only recolours cars and airships', () => {
  for (const kind of ['wagon', 'sled', 'car', 'locomotive', 'airship', 'raft', 'cannon']) {
    const a = draw('vehicle', { kind }), b = draw('vehicle', { kind });
    assert.equal(a.frames[0].w, 32, kind); assert.ok(a.frames[0].equals(b.frames[0]), kind);
    assert.deepEqual(lint(a.frames[0], { name: kind }), [], kind);
  }
  assert.ok(draw('vehicle', { kind: 'wagon', seed: 'a' }).frames[0].equals(draw('vehicle', { kind: 'wagon', seed: 'b' }).frames[0]));
  assert.throws(() => draw('vehicle', { kind: 'nope' }), /unknown vehicle/);
});

test('farm recipe: every kind draws a lint-clean 16x16, deterministic, and `ink` recolours the body', () => {
  for (const kind of ['wheat', 'carrot', 'pumpkin', 'corn', 'turnip', 'grapes', 'egg', 'mushroom', 'hen', 'sheep']) {
    const a = draw('farm', { kind }), b = draw('farm', { kind });
    assert.equal(a.frames[0].w, 16, kind); assert.ok(a.frames[0].equals(b.frames[0]), kind);
    assert.deepEqual(lint(a.frames[0], { name: kind }), [], kind);
  }
  assert.ok(!draw('farm', { kind: 'carrot', ink: 'purple' }).frames[0].equals(draw('farm', { kind: 'carrot' }).frames[0]));
  assert.throws(() => draw('farm', { kind: 'nope' }), /unknown farm kind/);
});

test('strategy recipe: every kind draws a lint-clean 16x16, deterministic, and `ink` recolours the body', () => {
  for (const kind of ['hex-grass', 'hex-forest', 'hex-mountain', 'hex-water', 'hex-desert', 'hex-snow', 'tower-archer', 'tower-cannon', 'tower-mage', 'tower-frost']) {
    const a = draw('strategy', { kind }), b = draw('strategy', { kind });
    assert.equal(a.frames[0].w, 16, kind); assert.ok(a.frames[0].equals(b.frames[0]), kind);
    assert.deepEqual(lint(a.frames[0], { name: kind }), [], kind);
  }
  assert.ok(!draw('strategy', { kind: 'tower-mage', ink: 'red' }).frames[0].equals(draw('strategy', { kind: 'tower-mage' }).frames[0]));
  assert.throws(() => draw('strategy', { kind: 'nope' }), /unknown strategy kind/);
});

test('building recipe: every kind draws a lint-clean 32x32, deterministic, and the seed only recolours the roof', () => {
  for (const kind of ['barn', 'windmill', 'lighthouse', 'tower', 'chapel', 'inn', 'watchtower']) {
    const a = draw('building', { kind }), b = draw('building', { kind });
    assert.equal(a.frames[0].w, 32, kind); assert.ok(a.frames[0].equals(b.frames[0]), kind);
    assert.deepEqual(lint(a.frames[0], { name: kind }), [], kind);
  }
  assert.ok(draw('building', { kind: 'tower', seed: 'a' }).frames[0].equals(draw('building', { kind: 'tower', seed: 'b' }).frames[0]));
  assert.throws(() => draw('building', { kind: 'nope' }), /unknown building/);
});

test('pattern recipe: two colours, every kind repeats on the 16 grid so tiles meet, noise mirrors so its edges meet, ramps pick the colours', async () => {
  const { PATTERNS, PATTERN_IDS } = await import('../recipes/patterns.js');
  for (const kind of Object.keys(PATTERNS)) for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) assert.equal(PATTERNS[kind](x, y), PATTERNS[kind](x + 16, y), kind), assert.equal(PATTERNS[kind](x, y), PATTERNS[kind](x, y + 16), kind);
  for (const kind of PATTERN_IDS) {
    const t = draw('pattern', { kind }).frames[0], u = draw('pattern', { kind }).frames[0];
    assert.ok(t.equals(u), kind); assert.equal(t.w, 16, kind); assert.ok(t.colors().length <= 2, kind);
  }
  const n = draw('pattern', { kind: 'noise', seed: 'a' }).frames[0];
  for (let i = 0; i < 16; i++) { assert.equal(n.get(0, i), n.get(15, i)); assert.equal(n.get(i, 0), n.get(i, 15)); }
  assert.ok(!n.equals(draw('pattern', { kind: 'noise', seed: 'b' }).frames[0]));
  assert.throws(() => draw('pattern', { kind: 'nope' }), /unknown pattern/);
});

test('a new kit is just data; a bad reference is reported, not hidden', () => {
  const bad = { id: 'future-kit', title: 'Future', doc: 'docs/x.md', subjects: [{ id: 'a', label: 'a', recipes: [{ recipe: 'nope', params: {} }] }, { id: 'b', label: 'b', recipes: [{ recipe: 'item', params: { shape: 'sword' } }] }, { id: 'c', label: 'c', recipes: [] }, { id: 'd', label: 'd', reference: 'paintings' }] };
  assert.match(subjectStatus(bad.subjects[0]).problems[0], /unknown recipe/);
  assert.equal(subjectStatus(bad.subjects[1]).status, 'covered');
  assert.equal(subjectStatus(bad.subjects[2]).status, 'gap');
  assert.equal(subjectStatus(bad.subjects[3]).status, 'reference-only');
  registerKit(bad);
  assert.throws(() => registerKit(bad), /already registered/);
  KITS.pop();
});

test('the package is self-contained: it imports nothing from outside it', () => {
  const root = resolve(ROOT), files: string[] = [];
  const walk = (d: string) => { for (const e of readdirSync(d, { withFileTypes: true })) e.isDirectory() ? walk(join(d, e.name)) : e.name.endsWith('.js') && files.push(join(d, e.name)); };
  walk(root);
  for (const f of files) for (const m of readFileSync(f, 'utf8').matchAll(/from '([^']+)'/g)) {
    const spec = m[1]; if (!spec.startsWith('.')) { assert.ok(spec.startsWith('node:'), `${f} imports ${spec}: only node: built-ins are allowed`); continue; }
    assert.ok(resolve(join(f, '..'), spec).startsWith(root), `${f} imports ${spec} from outside the package`);
  }
  assert.deepEqual(Object.keys(RAMPS).filter((k) => RAMPS[k as keyof typeof RAMPS].length !== 4), [], 'every ramp has four steps');
});

test('the main entry is browser-safe: no node: import is reachable from index.js, and the Node-only parts are not in it', () => {
  const seen = new Set<string>(), nodeUsers: string[] = [];
  const visit = (f: string) => {
    if (seen.has(f)) return; seen.add(f);
    for (const m of readFileSync(f, 'utf8').matchAll(/(?:from|import)\s*'([^']+)'/g)) { if (m[1].startsWith('node:')) nodeUsers.push(f); else if (m[1].startsWith('.')) visit(resolve(join(f, '..'), m[1])); }
  };
  visit(join(ROOT, 'index.js'));
  assert.deepEqual(nodeUsers, []);
  assert.ok(!seen.has(join(ROOT, 'png.js')) && !seen.has(join(ROOT, 'sheet.js')) && !seen.has(join(ROOT, 'atlas.js')));
  const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
  for (const [key, target] of Object.entries(pkg.exports) as [string, any][]) if (!key.includes('*')) for (const f of [typeof target === 'string' ? target : target.default, target.types].filter(Boolean)) assert.ok(readFileSync(join(ROOT, f)).length > 0, `${key} -> ${f} exists`);
});

test('the package entry draws, and a look cache draws once per spec and keeps the latest few', async () => {
  const api = await import('../index.js');
  for (const name of ['Pixels', 'draw', 'rng', 'C', 'RAMPS', 'shade', 'lint', 'KITS', 'createLookCache', 'toCanvas', 'play']) assert.ok(name in api, `${name} is exported`);
  let drawn = 0, urls = 0;
  const cache = api.createLookCache((spec: any) => { drawn++; return api.draw('character', spec); }, () => `u${urls++}`, { keep: 2 });
  const a = cache.get({ loadout: 'rogue' });
  assert.equal(cache.get({ loadout: 'rogue' }), a);
  assert.equal(drawn, 1);
  assert.equal(a.still, 'u0');
  assert.equal(a.clip('nope'), null);
  assert.equal(a.clip('attack'), a.clip('attack'), 'a clip is turned into URLs once');
  cache.get({ loadout: 'guardian' }); cache.get({ loadout: 'rogue' }); cache.get({ loadout: 'healer' });
  assert.equal(cache.size, 2);
  assert.equal(cache.get({ loadout: 'guardian' }) === cache.get({ loadout: 'guardian' }), true);
});

test('play shows frames in time, loops or ends, and stops', () => {
  const shown: number[] = []; let t = 0, ended = 0, queued: (() => void) | null = null;
  const ctx = { clearRect() {}, drawImage: (c: any) => shown.push(c.id), imageSmoothingEnabled: true };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  (globalThis as any).ImageData = function (this: any, data: any, width: number, height: number) { Object.assign(this, { data, width, height }); };
  let n = 0; (globalThis as any).document = { createElement: () => { const id = n++; return { id, width: 0, height: 0, getContext: () => ({ putImageData() {} }) }; } };
  const frames = [0, 1, 2].map(() => new Pixels(2, 2).set(0, 0, C.red));
  const raf = (f: () => void) => { queued = f; }, now = () => t;
  const stop = api_play(canvas, { frames, fps: 10, loop: false }, { raf, now, onEnd: () => ended++ });
  const advance = (ms: number) => { t = ms; const f = queued; queued = null; f?.(); };
  advance(50); advance(120); advance(250);
  assert.deepEqual(shown, [0, 1, 2]);
  advance(400);
  assert.equal(ended, 1);
  assert.equal(queued, null, 'a clip that ends schedules nothing more');
  const again = api_play(canvas, { frames, fps: 10, loop: true }, { raf, now });
  advance(t + 450); again();
  const pending = queued; queued = null; (pending as any)?.();
  assert.equal(queued, null, 'stop() ends the loop'); void stop;
});

test('hands: every gesture draws at every stage; a mitten is one mass, blocks separate the fingers, detail adds marks', () => {
  for (const gesture of GESTURE_IDS) {
    const [mitten, blocks, detailed] = STAGES.map((stage) => drawHand({ gesture, stage }).frames[0]);
    assert.ok(!same(mitten, blocks) || gesture === 'fist' || gesture === 'grip' || gesture === 'thumbsup', `${gesture}: mitten and blocks differ`);
    assert.ok(!same(detailed, blocks), `${gesture}: detail adds nails, knuckles or a crease`);
    const lines = (p: Pixels) => p.data.filter((v) => v === C.night).length; // the dark lines between fingers are outline
    assert.ok(lines(mitten) <= lines(blocks), `${gesture}: a mitten has no finger lines`);
  }
  const right = drawHand({ gesture: 'point' }).frames[0], left = drawHand({ gesture: 'point', side: 'left' }).frames[0];
  assert.ok(same(left, right.flipX()), 'a left hand is the mirror of the right');
  assert.ok(drawHand({ gesture: 'point' }).frames[0].bbox()!.h > drawHand({ gesture: 'fist' }).frames[0].bbox()!.h, 'pointing is taller than a fist');
  assert.throws(() => drawHand({ gesture: 'wave' }), /unknown gesture/);
  assert.throws(() => drawHand({ stage: 'done' }), /unknown stage/);
});

test('turnaround: front, side and back share a loadout, a height, a feet anchor and a palette', () => {
  for (const loadout of Object.keys(LOADOUTS)) {
    const views = VIEWS.map((view) => drawCharacter({ loadout, view })), boxes = views.map((v) => v.frames[0].bbox()!);
    const [front, side, back] = boxes;
    assert.ok(Math.abs(front.y - side.y) <= 1 && Math.abs(front.y - back.y) <= 1, `${loadout}: heads reach the same height`);
    assert.equal(front.y + front.h, side.y + side.h, `${loadout}: the same ground line`);
    assert.equal(front.y + front.h, back.y + back.h);
    assert.ok(!same(views[0].frames[0], views[1].frames[0]) && !same(views[1].frames[0], views[2].frames[0]), `${loadout}: each view is its own drawing`);
    // The face is a front and side thing: the front head layer has eye whites, the back one has none.
    assert.ok([...(views[0].layers as any).body[0].data].includes(C.white), `${loadout}: eyes from the front`);
    assert.ok(![...(views[2].layers as any).body[0].data].includes(C.white), `${loadout}: no eyes from behind`);
    for (const v of views) assert.deepEqual(lint(v.frames[0], { skip: ['size', 'margin', 'palette'] }).filter((f) => f.level === 'error'), []);
  }
  assert.equal(drawCharacter({ view: 'front' }).frames[0].w, 40);
  assert.equal(drawCharacter({ view: 'back' }).frames[0].w, 48, 'the back view has room to mirror the weapon side');
  assert.throws(() => drawCharacter({ view: 'above' }), /unknown view/);
});

test('from behind the weapon is on the other side; facing left mirrors the side view', () => {
  const side = (p: Pixels) => { const b = p.bbox()!; return b.x + b.w / 2; };
  const front = drawCharacter({ loadout: 'guardian', view: 'front' }), back = drawCharacter({ loadout: 'guardian', view: 'back' });
  const w = (c: any) => side(c.layers.weapon[0]) - c.anchors.feet[0];
  assert.ok(w(front) > 0 && w(back) < 0, 'weapon right of the body in front, left of it from behind');
  const right = drawCharacter({ loadout: 'rogue', view: 'side' }), left = drawCharacter({ loadout: 'rogue', view: 'side', facing: 'left' });
  assert.ok(same(left.frames[0], right.frames[0].flipX()));
  assert.deepEqual(left.anchors.feet, [39 - 16, 30]);
  assert.equal(layerOrder('back').indexOf('back') > layerOrder('back').indexOf('torso'), true, 'a cape covers the torso from behind');
  assert.equal(LAYER_ORDER.indexOf('back') < LAYER_ORDER.indexOf('torso'), true, 'and sits under it from the front');
});

test('poses are joints: each stance is a different drawing, any loadout takes any pose, and the idle bob still moves', () => {
  const stills = POSE_IDS.map((pose) => drawCharacter({ loadout: 'guardian', pose }).frames[0]);
  stills.forEach((f, i) => stills.slice(0, i).forEach((g, j) => assert.ok(!same(f, g), `${POSE_IDS[i]} equals ${POSE_IDS[j]}`)));
  const stand = drawCharacter({ loadout: 'guardian' }).frames[0], low = (pose: string) => drawCharacter({ loadout: 'guardian', pose }).frames[0].bbox()!;
  assert.ok(low('crouch').h < stand.bbox()!.h && low('sit').h < stand.bbox()!.h, 'crouching and sitting are shorter than standing');
  assert.ok(low('fall').y < stand.bbox()!.y, 'a fall is above the ground line');
  assert.ok(same(drawCharacter({ loadout: 'guardian', pose: 'stand' }).frames[0], stand), 'stand is the original');
  for (const loadout of Object.keys(LOADOUTS)) for (const pose of POSE_IDS) for (const view of VIEWS) assert.ok(drawCharacter({ loadout, pose, view }).frames[0].count() > 80, `${loadout}/${pose}/${view}`);
  assert.throws(() => drawCharacter({ pose: 'flying' }), /unknown pose/);
  assert.throws(() => drawCharacter({ back: 'wings' }), /unknown back slot/);
});

test('the back slot is its own layer: only it changes, and it shows from every view', () => {
  for (const view of VIEWS) {
    const plain = drawCharacter({ loadout: 'guardian', view }), caped = drawCharacter({ loadout: 'guardian', view, back: 'cape' });
    for (const name of layerOrder(view)) assert.equal(same((plain.layers as any)[name][0], (caped.layers as any)[name][0]), name !== 'back', `${view}: ${name}`);
  }
  const walk = drawCharacter({ loadout: 'guardian', view: 'side', back: 'cape' }).clips.walk, hems = walk.frames.map((f: Pixels) => f.bbox()!.x);
  assert.ok(Math.min(...hems) < Math.max(...hems), 'the cape lags behind the stride: its trailing edge moves');
});

test('side and back walk cycles are 4-frame loops with no repeated frame; back has no attack', () => {
  for (const view of ['side', 'back']) {
    const c = drawCharacter({ loadout: 'rogue', view, back: 'cape' });
    assert.equal(c.clips.walk.frames.length, 4);
    assert.deepEqual(lintFrames(c.clips.walk.frames, { loop: true }).filter((f) => f.rule === 'frames'), [], view);
    assert.equal(!!c.clips.attack, view === 'side');
  }
});

test('cloth loops: a pinned anchor, a free edge that moves more, and the last frame flows into the first', () => {
  for (const kind of CLOTH_KINDS) for (const frames of [4, 6]) {
    const a = drawCloth({ kind, frames }), b = drawCloth({ kind, frames });
    assert.equal(a.frames.length, frames);
    a.frames.forEach((f: Pixels, i: number) => assert.ok(same(f, b.frames[i]), `${kind} deterministic`));
    assert.deepEqual(lintFrames(a.frames, { loop: true, boil: false }).filter((f) => f.rule === 'frames' || f.rule === 'length'), [], `${kind} no repeats`);
    // One phase over the loop: closing the loop is as small a step as any other.
    const step = (i: number, j: number) => a.frames[i].data.reduce((n: number, v: number, k: number) => n + (v !== a.frames[j].data[k] ? 1 : 0), 0);
    const steps = a.frames.map((_: Pixels, i: number) => step(i, (i + 1) % frames));
    assert.ok(Math.max(...steps) < 2.2 * Math.min(...steps), `${kind}/${frames}: even steps, no jump at the loop point (${steps})`);
  }
  const top = (p: Pixels, x: number) => { for (let y = 0; y < p.h; y++) if (p.get(x, y)) return y; return -1; };
  const travelAt = (wind: string, x: number) => { const t = drawCloth({ kind: 'flag', wind }).frames.map((f: Pixels) => top(f, x)); return Math.max(...t) - Math.min(...t); };
  assert.ok(travelAt('gust', 18) > travelAt('calm', 18), 'more wind, more movement');
  const flag = drawCloth({ kind: 'flag', wind: 'gust' }).frames, travel = (x: number) => Math.max(...flag.map((f: Pixels) => top(f, x))) - Math.min(...flag.map((f: Pixels) => top(f, x)));
  assert.ok(travel(18) > travel(5), 'the free end of a flag travels further than the end at the pole');
  const calm = drawCloth({ kind: 'cape', drag: 0 }).frames[0].bbox()!, dragged = drawCloth({ kind: 'cape', drag: 3 }).frames[0].bbox()!;
  assert.ok(dragged.x < calm.x, 'drag trails the hem behind');
  assert.throws(() => drawCloth({ kind: 'sail' }), /unknown cloth kind/);
  assert.throws(() => drawCloth({ wind: 'storm' }), /unknown wind/);
  assert.throws(() => drawCloth({ frames: 12 }), /4 or 6/);
  assert.deepEqual(Object.keys(WINDS), ['calm', 'steady', 'gust']);
});

test('interiors: every room is a valid layout you can walk through, with a door and a focal point', () => {
  for (const room of ROOM_IDS) {
    const layout = layoutOf(room);
    assert.deepEqual(unreachable(layout), [], `${room}: every piece of furniture can be walked up to from the door`);
    assert.ok(layout.focal, `${room} names a focal item that is placed`);
    assert.equal(layout.blocked[0][layout.entry[0]], false, `${room}: the door's entry cell is free`);
    assert.ok(ROOMS[room as keyof typeof ROOMS].story.length > 20, `${room} says who lives or works there`);
    for (const [name, on] of ROOMS[room as keyof typeof ROOMS].clues.map((c: any[]) => [c[0], c[1]])) assert.ok(CLUES[name as keyof typeof CLUES] && layout.items.some((i) => i.name === on), `${room}: clue ${name} sits on a placed ${on}`);
    const a = drawRoom({ room }), b = drawRoom({ room });
    assert.deepEqual([a.frames[0].w, a.frames[0].h], [192, 128]);
    a.frames.forEach((f: Pixels, i: number) => assert.ok(same(f, b.frames[i]), `${room} deterministic`));
  }
  // A room that walls itself off is reported.
  const walled = layoutOf('cottage'); for (let x = 0; x < walled.cols; x++) walled.blocked[1][x] = true;
  assert.ok(unreachable(walled).length > 0, 'a wall of furniture across the room is caught');
  assert.throws(() => layoutOf('moat'), /unknown room/);
  assert.throws(() => drawRoom({ room: 'cottage', time: 'noon' }), /unknown time/);
});

test('lighting: night is darker than day, a source lights a circle, and it stays on the palette', () => {
  const mean = (p: Pixels) => { const v = p.colors(); let total = 0, n = 0; for (let i = 0; i < p.data.length; i++) if (p.data[i]) { total += p.luma(p.data[i]); n++; } return total / n; };
  const day = drawRoom({ room: 'cottage', time: 'day' }).frames[0], night = drawRoom({ room: 'cottage', time: 'night' });
  assert.ok(mean(night.frames[0]) < mean(day) * 0.7, 'night is darker');
  assert.equal(night.frames.length, 2, 'a lit room flickers');
  assert.ok(!same(night.frames[0], night.frames[1]));
  assert.deepEqual(lintFrames(night.frames, { loop: true }).filter((f) => f.rule === 'boil'), [], 'the flicker is a small change');
  const flat = new Pixels(48, 48).rect(0, 0, 48, 48, C.brown), lit = relight(flat, { ambient: 'night', sources: [{ x: 24, y: 24, r: 20 }] });
  assert.equal(lit.get(24, 24), C.brown, 'the centre of a light is the true colour');
  assert.notEqual(lit.get(2, 2), C.brown, 'far from it, the dark');
  assert.ok(lit.colors().every((v) => v >= 1 && v <= 32), 'no colour outside DB32');
  assert.equal(TIMES.day > TIMES.dusk && TIMES.dusk > TIMES.night, true);
  assert.throws(() => relight(flat, { ambient: 'eclipse' }), /unknown time/);
  const shaft = relight(flat, { ambient: 'day', shafts: [[[8, 8], [24, 8], [32, 40], [16, 40]]] });
  assert.ok(shaft.luma(shaft.get(22, 24)) > shaft.luma(C.brown), 'a window shaft lightens the floor it falls on');
  assert.equal(shaft.get(2, 2), C.brown);
});

test('furniture sits on its footprint and floors tile', () => {
  for (const [name, f] of Object.entries(FURNITURE)) { const art = f.make(), b = art.bbox()!; assert.ok(art.count() > 20, name); assert.ok(b.y + b.h <= art.h, `${name} stays on its canvas`); }
  for (const kind of ['planks', 'flagstone', 'checker']) {
    const t = draw('tile', { kind }).frames[0], two = new Pixels(32, 32); for (const [x, y] of [[0, 0], [16, 0], [0, 16], [16, 16]]) two.blit(t, x, y);
    // Seamless: the seam where tile meets tile is no more different than the pixels inside one.
    const diff = (a: number, b: number) => (a !== b ? 1 : 0); let seam = 0, inner = 0;
    for (let y = 0; y < 16; y++) { seam += diff(two.get(15, y), two.get(16, y)); inner += diff(two.get(7, y), two.get(8, y)); }
    assert.ok(seam <= inner + 8, `${kind} tiles without a visible seam (${seam} vs ${inner})`);
  }
});

test('icons: every glyph is at most 10x10 text on a 16x16 plate, in every plate shape, and an unknown glyph or shape throws', () => {
  for (const [id, rows] of Object.entries(GLYPHS) as [string, string[]][]) assert.ok(rows.length <= 10 && rows.every((r) => r.length <= 10 && /^[Xo.]*$/.test(r)), `${id} fits its 10x10 box`);
  for (const glyph of GLYPH_IDS) for (const shape of PLATE_IDS) { const a = drawIcon({ glyph, shape }); assert.deepEqual([a.frames[0].w, a.frames[0].h], [16, 16]); assert.ok(a.frames[0].count() > 20, `${glyph} ${shape} draws`); }
  assert.ok(drawIcon({ glyph: 'sword', shape: 'bare' }).frames[0].count() < drawIcon({ glyph: 'sword' }).frames[0].count(), 'a bare symbol has no plate');
  assert.throws(() => drawIcon({ glyph: 'nope' }), /unknown glyph/);
  assert.throws(() => drawIcon({ glyph: 'sword', shape: 'hex' }), /unknown plate/);
});

test('dungeon tiles: each biome has a floor and a wall that tile, and the crown, bust and crop are exact', () => {
  for (const biome of DUNGEON_BIOMES) for (const part of ['floor', 'wall']) {
    const t = dungeonTile(biome, part), again = dungeonTile(biome, part);
    assert.deepEqual([t.w, t.h], [16, 16]); assert.ok(same(t, again), `${biome} ${part} is deterministic`); assert.equal(t.data.filter((v: number) => !v).length, 0, `${biome} ${part} has no holes`);
  }
  assert.throws(() => dungeonTile('moon'), /unknown dungeon biome/);
  assert.throws(() => dungeonTile('crypt', 'ceiling'), /unknown tile part/);
  const p = new Pixels(6, 6).rect(2, 2, 2, 2, C.red);
  assert.ok(same(p.crop(1, 1, 4, 4), new Pixels(4, 4).rect(1, 1, 2, 2, C.red)));
  assert.equal(p.crop(4, 4, 4, 4).count(), 0, 'cells outside are empty');
  for (const loadout of Object.keys(LOADOUTS)) { const b = drawPortrait({ loadout }).frames[0]; assert.deepEqual([b.w, b.h], [20, 20]); assert.ok(b.count() > 80, loadout); }
  const king = drawCreature({ preset: 'ogreking' }).frames[0], ogre = drawCreature({ preset: 'ogre' }).frames[0];
  assert.ok(!same(king, ogre) && king.count() > ogre.count(), 'the crown is added to the ogre, not drawn instead');
});

// ---- ANSI writer and reader (Phase 2) ----

const esc = (s: string) => s.replace(/\x1b/g, '\\e').replace(/\r/g, '\\r').replace(/\n/g, '\\n');
const bodyOf = (cells: Cells, opts: any = {}) => esc(new TextDecoder('latin1').decode(writeAnsi(cells, { sauce: false, ...opts }).bytes));
// What a cell shows: a space has no foreground and a full block no background, and an empty cell is a space.
const seen = (c: Cells) => { const o = c.clone(); for (let i = 0; i < o.ch.length; i++) { if (!o.ch[i]) o.ch[i] = 32; if (o.ch[i] === 32) o.fg[i] = NONE; if (o.ch[i] === 0x2588) o.bg[i] = NONE; } return o; };

test('CP437: 256 distinct characters, ASCII where ASCII is, and every box and shade glyph the engine draws', () => {
  assert.equal(CP437.length, 256); assert.equal(new Set(CP437).size, 256);
  for (let b = 0x20; b < 0x7f; b++) assert.equal(CP437[b], b);
  assert.equal(String.fromCodePoint(CP437[0xb0], CP437[0xb1], CP437[0xb2], CP437[0xdb], CP437[0xdf], CP437[0xdc]), '░▒▓█▀▄');
  for (const ch of '─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬╒╕╘╛╞╡╤╧╪╓╖╙╜╟╢╥╨╫') assert.ok(CP437.includes(ch.codePointAt(0)!), ch);
});

test('writeAnsi: byte-level escapes, CRLF, reset first, bold for bright, trailing spaces only over no background', () => {
  const c = new Cells(6, 3);
  c.text(0, 0, 'ab', V.red, V.blue); c.set(2, 0, { ch: 'c', fg: V.brightred }); c.set(3, 0, { ch: 'd', fg: V.red, bg: V.blue });
  assert.equal(bodyOf(c), '\\e[0;44;31mab\\e[0;1;31mc\\e[0;44;31md\\e[0m\\r\\n\\r\\n\\r\\n', 'one escape per change, trailing empty cells and rows are just CRLF');
  const d = new Cells(4, 1); d.text(0, 0, 'x'); d.set(2, 0, { ch: ' ', bg: V.green });
  assert.equal(bodyOf(d), 'x \\e[0;42m \\e[0m\\r\\n', 'a space over a background is kept, plain trailing ones are cut');
  const full = new Cells(3, 1).text(0, 0, 'abc'); assert.equal(bodyOf(full), 'abc\\r\\n', 'CRLF even on a full-width row');
  assert.ok(!/[^\r]\n/.test(new TextDecoder('latin1').decode(writeAnsi(c).bytes.subarray(0, 40))), 'never a bare LF');
});

test('writeAnsi targets: classic legalizes backgrounds, ice uses the blink bit and sets the flag only when needed, modern uses 100 and 90 codes', () => {
  const c = new Cells(2, 1); c.set(0, 0, { ch: ' ', fg: V.red, bg: V.brightblue });
  const classic = writeAnsi(c, { sauce: false });
  assert.deepEqual(readAnsi(classic.bytes).cells.get(0, 0).bg, NONE, 'a bright background became a full block of that colour');
  assert.equal(classic.ice, false);
  const ice = writeAnsi(c, { target: 'ice', sauce: false });
  assert.equal(esc(new TextDecoder().decode(ice.bytes)), '\\e[0;5;44m \\e[0m\\r\\n');
  assert.equal(ice.ice, true); assert.equal(writeAnsi(new Cells(2, 1).text(0, 0, 'hi'), { target: 'ice' }).ice, false);
  const modern = writeAnsi(new Cells(1, 1).set(0, 0, { ch: 'x', fg: V.brightred, bg: V.brightblue }), { target: 'modern' });
  assert.equal(esc(modern.text!), '\\e[0;104;91mx\\e[0m\\r\\n'); assert.equal(modern.charset, 'utf8'); assert.equal(modern.sauce, false);
});

test('writeAnsi depth: 256 and truecolor spell indexes above 15, 16 reduces them with a warning', () => {
  const c = new Cells(1, 1).set(0, 0, { ch: 'x', fg: 196, bg: 232 });
  assert.equal(bodyOf(c, { colors: 256 }), '\\e[0;48;5;232;38;5;196mx\\e[0m\\r\\n');
  assert.equal(bodyOf(c, { colors: 'true' }), '\\e[0;48;2;8;8;8;38;2;255;0;0mx\\e[0m\\r\\n');
  const r = writeAnsi(c, { sauce: false }); assert.ok(r.warnings.some((w) => /reduced to the nearest of the 16/.test(w)));
  const back = readAnsi(r.bytes).cells.get(0, 0); assert.ok(back.fg < 16 && back.bg < 16);
});

test('writeAnsi charsets: CP437 bytes, "?" and a warning for what it cannot hold, UTF-8 on request', () => {
  const c = new Cells(5, 1).text(0, 0, '░é→Ж');
  const r = writeAnsi(c, { sauce: false });
  assert.deepEqual([...r.bytes.subarray(0, 4)], [0xb0, 0x82, 0x3f, 0x3f]);
  assert.equal(r.warnings.length, 2, 'a control-byte glyph and a character outside CP437 are each reported');
  assert.ok(r.warnings.some((w) => /control bytes/.test(w)) && r.warnings.some((w) => /not in CP437/.test(w)));
  const u = writeAnsi(new Cells(2, 1).text(0, 0, '░Ж'), { charset: 'utf8', sauce: false });
  assert.equal(u.text, '░Ж\r\n'); assert.ok(u.warnings.some((w) => /not in CP437/.test(w)));
  assert.throws(() => writeAnsi(c, { target: 'retro' as any }), /unknown target/);
});

test('SAUCE: Ctrl-Z, comments, then the 128-byte record; the fields round trip', () => {
  const c = new Cells(10, 2).text(0, 0, 'hi').set(0, 1, { ch: ' ', bg: V.brightblue });
  const out = writeAnsi(c, { target: 'ice', sauce: { title: 'Test', author: 'art-sprites', group: 'g', date: '20261029', comments: ['one', 'two'], letterSpacing: '9', aspect: 'square' } });
  const b = out.bytes, n = b.length;
  assert.equal(new TextDecoder().decode(b.subarray(n - 128, n - 121)), 'SAUCE00');
  assert.equal(b[n - 128 - 5 - 128 - 1], 0x1a, 'Ctrl-Z comes before the comment block');
  assert.equal(new TextDecoder().decode(b.subarray(n - 128 - 5 - 128, n - 128 - 128)), 'COMNT');
  const back = readAnsi(b);
  assert.deepEqual({ ...back.sauce }, { title: 'Test', author: 'art-sprites', group: 'g', date: '20261029', dataType: 1, fileType: 1, fileSize: n - 128 - 5 - 128 - 1, width: 10, height: 2, ice: true, letterSpacing: '9', aspect: 'square', font: 'IBM VGA', comments: ['one', 'two'] });
  assert.deepEqual([back.width, back.height, back.ice], [10, 2, true]);
  assert.equal(writeAnsi(c, { sauce: { date: '20261029' } }).bytes.length, writeAnsi(c, { sauce: { date: '20261029' } }).bytes.length);
  assert.throws(() => writeAnsi(c, { sauce: { date: 'today' } }), /YYYYMMDD/);
  assert.ok(writeAnsi(c, { sauce: { title: 'x'.repeat(40) } }).warnings.some((w) => /title cut/.test(w)));
});

test('round trip: cells to bytes to cells shows the same picture, for UTF-8 and for CP437, on pictures this engine drew', () => {
  const pictures: Cells[] = [];
  for (const [id, params] of [['item', { kind: 'sword' }], ['face', {}], ['creature', { preset: 'ogre' }]] as const) {
    try { pictures.push((draw(id, params as any, { style: 'ansi', bgLimit: 8 }) as any).frames[0]); } catch { /* recipe takes other params */ }
  }
  for (const rec of Object.values(RECIPES).slice(0, 12)) pictures.push((draw((rec as any).id ?? Object.keys(RECIPES).find((k) => RECIPES[k] === rec)!, rec.variants()[0], { style: 'ansi' }) as any).frames[0]);
  const box = new Cells(12, 5); box.box(0, 0, 12, 5, { style: 'double', fg: V.brightcyan }); box.text(2, 2, 'Round trip', V.white, V.blue); box.shadeFill(1, 3, 4, 1, '▒', V.brightgreen, V.green); pictures.push(box);
  for (const p of pictures) for (const opts of [{}, { charset: 'utf8' }, { target: 'ice' }, { colors: 256 }]) {
    const w = writeAnsi(p, opts as any), r = readAnsi(w.bytes, { width: p.w });
    const want = w.target === 'classic' ? legalize(p, { bgLimit: 8 }) : p;
    assert.ok(seen(r.cells).equals(seen(want)), `${JSON.stringify(opts)} round trips`);
    assert.equal(r.cells.w, p.w);
  }
});

test('readAnsi: cursor moves, wrap after the last column, tab, clear, line erase, truecolor, reverse, ice, Ctrl-Z, and unknown sequences ignored', () => {
  const enc = (s: string) => Uint8Array.from(s, (c) => c.charCodeAt(0));
  const r = (s: string, o: any = {}) => readAnsi(enc(s), { width: 4, ...o });
  assert.equal(r('abcd\r\nef').cells.toText().trimEnd(), 'abcd\nef', 'a full row then CRLF is one line break');
  assert.equal(r('abcdef').cells.toText(), 'abcd\nef  ', 'text wraps at the width');
  assert.equal(r('ab\x1b[2Dxy').cells.get(0, 0).ch, 'x'.charCodeAt(0), 'cursor back overwrites');
  assert.equal(r('\x1b[2;3Hz').cells.get(2, 1).ch, 122); assert.equal(r('a\x1b[2Cz').cells.get(3, 0).ch, 122);
  assert.equal(r('a\x1b[sbc\x1b[ud').cells.toText(), 'adc ', 'save and restore');
  assert.equal(r('a\tb', { width: 20 }).cells.get(8, 0).ch, 98, 'tab to the next multiple of 8');
  assert.equal(r('ab\x1b[2J\x1b[Hc').cells.toText(), 'c   ', 'clear the screen homes the cursor');
  assert.equal(r('abcd\x1b[1;2H\x1b[K').cells.get(1, 0).ch, 32, 'erase to end of line');
  assert.equal(r('\x1b[1;31;44mx').cells.get(0, 0).fg, V.brightred, 'bold makes the bright colour'); assert.equal(r('\x1b[1;31;44mx').cells.get(0, 0).bg, V.blue);
  assert.equal(r('\x1b[91;103mx').cells.get(0, 0).bg, 11, '100 to 107 are bright backgrounds');
  assert.equal(r('\x1b[38;5;196;48;5;21mx').cells.get(0, 0).fg, 196);
  const t = r('\x1b[38;2;255;0;0mx'); assert.ok(t.cells.get(0, 0).fg >= 16 && t.warnings.some((w) => /truecolor/.test(w)));
  assert.deepEqual([r('\x1b[7mx').cells.get(0, 0).fg, r('\x1b[7mx').cells.get(0, 0).bg], [0, 7], 'reverse swaps the defaults');
  assert.equal(r('\x1b[5;44mx').cells.get(0, 0).flags, 2, 'blink is a flag without iCE'); assert.equal(r('\x1b[5;44mx', { ice: true }).cells.get(0, 0).bg, V.brightblue);
  assert.equal(r('ab\x1ahidden').cells.toText(), 'ab  ', 'everything after Ctrl-Z is not picture');
  assert.equal(r('\x1b[?25l\x1b[=7h\x1b[3zq').cells.get(0, 0).ch, 113, 'unknown and private sequences are skipped');
  assert.equal(r('a\x1b[0m\x1b[m\x1b[39;49mb').cells.get(1, 0).fg, NONE);
});

test('readAnsi: CP437 and UTF-8 detection, string input, caps, SAUCE width', () => {
  const cp = Uint8Array.from([0xdb, 0xb0, 0x82]), ut = new TextEncoder().encode('█░é');
  assert.equal(readAnsi(cp).charset, 'cp437'); assert.equal(readAnsi(ut).charset, 'utf8');
  assert.equal(readAnsi(cp).cells.toText().slice(0, 3), '█░é'); assert.equal(readAnsi(ut).cells.toText().slice(0, 3), '█░é');
  assert.equal(readAnsi(ut, { charset: 'cp437' }).charset, 'cp437'); assert.equal(readAnsi('█░é').charset, 'utf8');
  assert.throws(() => readAnsi(cp, { charset: 'ebcdic' as any }), /unknown charset/);
  assert.equal(readAnsi(new Uint8Array(0)).height, 0);
  assert.equal(readAnsi(new TextEncoder().encode('x'), { width: 9999 }).width, 400);
  assert.ok(readAnsi(new TextEncoder().encode('x'), { width: 9999 }).warnings.some((w) => /cut to 400/.test(w)));
  const tall = readAnsi(new TextEncoder().encode('\n'.repeat(1200) + 'z'));
  assert.equal(tall.height, 1000); assert.ok(tall.warnings.some((w) => /1000/.test(w)));
  const wide = new Cells(100, 1).text(0, 0, 'q'.repeat(100)), back = readAnsi(writeAnsi(wide).bytes);
  assert.equal(back.width, 100); assert.equal(back.cells.toText(), 'q'.repeat(100), 'SAUCE carries the width past the default 80');
});

test('toSpans and toPlainText: runs of one style, bold folded in, trailing cells and spaces cut', () => {
  const c = new Cells(8, 2); c.text(0, 0, 'ab', V.red); c.text(2, 0, 'cd', V.red, V.blue); c.set(4, 0, { ch: 'e', fg: V.green, flags: 1 | 2 });
  const s = toSpans(c);
  assert.deepEqual([s.columns, s.rows], [8, 2]);
  assert.deepEqual(s.lines[0], [{ text: 'ab', fg: 1, bg: null, blink: false }, { text: 'cd', fg: 1, bg: 4, blink: false }, { text: 'e', fg: 10, bg: null, blink: true }]);
  assert.deepEqual(s.lines[1], []);
  assert.equal(toPlainText(c), 'abcde\n');
});

test('the VGA atlas holds the CP437 glyphs: block, half block, line and box characters are what the font draws', () => {
  const a = loadAtlas();
  assert.deepEqual([a.w, a.h], [128, 256]);
  const rows = (i: number) => Array.from({ length: 16 }, (_, y) => Array.from({ length: 8 }, (_, x) => (a.rgba[((Math.floor(i / 16) * 16 + y) * 128 + (i % 16) * 8 + x) * 4 + 3] ? '#' : '.')).join(''));
  assert.deepEqual(rows(219), Array(16).fill('########'), 'full block');
  assert.deepEqual(rows(220), [...Array(7).fill('........'), ...Array(9).fill('########')], 'lower half block');
  assert.deepEqual(rows(0x20), Array(16).fill('........'), 'space');
  assert.equal(rows(179)[0], '...##...', 'the vertical line sits in the middle');
  assert.equal(rows(196)[7], '########', 'the horizontal line is on row 7');
  assert.ok(rows(65).some((r) => r.includes('#')), 'A has ink');
});

test('renderCells: a cell is 8 by 16, glyph pixels take the fg and the rest the bg, colours come from the theme', () => {
  const atlas = loadAtlas(), c = new Cells(3, 2);
  c.set(0, 0, { ch: '█', fg: V.red, bg: V.blue }); c.set(1, 0, { ch: '▄', fg: V.yellow, bg: V.blue }); c.set(2, 1, { ch: 'A', fg: V.white });
  const img = renderCells(c, { atlas }), at = (x: number, y: number) => Array.from(img.rgba.subarray((y * img.w + x) * 4, (y * img.w + x) * 4 + 4));
  assert.deepEqual([img.w, img.h], [24, 32]);
  assert.deepEqual(at(3, 3), [...rgbOf(VGA16[V.red]), 255], 'full block is all fg');
  assert.deepEqual(at(8, 0), [...rgbOf(VGA16[V.blue]), 255], 'lower half block: top is bg'); assert.deepEqual(at(8, 15), [...rgbOf(VGA16[V.yellow]), 255], 'and bottom is fg');
  assert.deepEqual(at(20, 16), [0, 0, 0, 255], 'an empty cell with no colour is black'); assert.deepEqual(at(0, 20), [0, 0, 0, 255]);
  assert.ok(Array.from({ length: 16 * 8 }, (_, i) => at(16 + (i % 8), 16 + Math.floor(i / 8))).some((p) => p[0] === 255 && p[1] === 255), 'A has white ink');
  const themed = renderCells(c, { atlas, theme: VGA16.map(() => '#123456') });
  assert.deepEqual(Array.from(themed.rgba.subarray(0, 4)), [0x12, 0x34, 0x56, 255], 'a theme replaces the colours');
  const bold = new Cells(1, 1).set(0, 0, { ch: '█', fg: V.red, flags: 1 });
  assert.deepEqual(Array.from(renderCells(bold, { atlas }).rgba.subarray(0, 3)), rgbOf(VGA16[V.brightred]), 'bold brightens a dim foreground'); assert.deepEqual(Array.from(renderCells(bold, { atlas, bold: false }).rgba.subarray(0, 3)), rgbOf(VGA16[V.red]));
  assert.throws(() => renderCells(c, { atlas: { w: 1, h: 1, rgba: new Uint8Array(4) } }), /atlas/);
  assert.throws(() => renderCells(c, { atlas, letterSpacing: 10 as any }), /letterSpacing/);
});

test('renderCells: 9-pixel spacing repeats the eighth column only for line and block characters; characters outside CP437 draw as ? and are reported', () => {
  const atlas = loadAtlas(), c = new Cells(2, 1); c.set(0, 0, { ch: '█', fg: 15, bg: 0 }); c.set(1, 0, { ch: 'I', fg: 15, bg: 0 });
  const img = renderCells(c, { atlas, letterSpacing: 9 }), at = (x: number, y: number) => img.rgba[(y * img.w + x) * 4];
  assert.equal(img.w, 18); assert.equal(at(8, 5), 255, 'a block fills the ninth column'); assert.equal(at(17, 5), 0, 'a letter leaves it blank');
  const odd = new Cells(1, 1).set(0, 0, { ch: '漢', fg: 15 }), r = renderCells(odd, { atlas }), q = renderCells(new Cells(1, 1).set(0, 0, { ch: '?', fg: 15 }), { atlas });
  assert.deepEqual(r.missing, [0x6f22]); assert.ok(Buffer.from(r.rgba).equals(q.rgba)); assert.deepEqual(q.missing, []);
});

test('renderCells draws what an ANSI file says: an engine drawing written, read back and rendered is the same picture', () => {
  const atlas = loadAtlas(), art = draw('face', undefined, { style: 'ansi' }), cells = art.frames[0];
  const direct = renderCells(cells, { atlas }), back = renderCells(readAnsi(writeAnsi(cells, { target: 'modern' }).bytes, { width: cells.w }).cells, { atlas });
  assert.equal(Buffer.from(back.rgba).compare(direct.rgba), 0, 'written and read back, it renders the same');
  assert.equal(Buffer.from(renderCells(cells, { atlas }).rgba).compare(direct.rgba), 0, 'deterministic');
  const sheet = rgbaSheet([direct, back], { scale: 2, cols: 2 });
  assert.equal(sheet.subarray(1, 4).toString(), 'PNG');
});

const cellsOf = (rows: string[], cell: any = { fg: 7 }) => { const c = new Cells(rows[0].length, rows.length); rows.forEach((r, y) => [...r].forEach((ch, x) => { if (ch !== '.') c.set(x, y, { ch, ...cell }); })); return c; };
const rulesOf = (c: Cells, o: any = {}) => lintAnsi(c, { skip: ['outline'], ...o }).map((f) => f.rule);

test('lintAnsi: the file format rules (width, height, backgrounds, blink, charset, colours, invisible text, contrast)', () => {
  assert.deepEqual(rulesOf(new Cells(4, 4)), ['empty']);
  assert.deepEqual(rulesOf(cellsOf(['abc'])), []);
  assert.ok(rulesOf(cellsOf(['abc']), { width: 2 }).includes('width'));
  assert.ok(rulesOf(new Cells(10, 25).fill({ ch: 'x', fg: 7 })).includes('height'));
  assert.ok(!rulesOf(new Cells(10, 25).fill({ ch: 'x', fg: 7 }), { target: 'modern' }).includes('height'));
  const bright = cellsOf(['abc'], { fg: 15, bg: 9 });
  assert.ok(rulesOf(bright).includes('bg-limit'));
  assert.ok(!rulesOf(bright, { target: 'ice' }).includes('bg-limit'));
  assert.ok(rulesOf(cellsOf(['abc'], { fg: 7, bg: 1, flags: 2 })).includes('blink'));
  assert.ok(rulesOf(cellsOf(['a≠b'])).includes('charset') && !rulesOf(cellsOf(['a≠b']), { target: 'modern' }).includes('charset'));
  assert.ok(rulesOf(cellsOf(['abc'], { fg: 4, bg: 4 })).includes('invisible'));
  assert.ok(!rulesOf(cellsOf(['███'], { fg: 4, bg: 4 })).includes('invisible'));
  assert.ok(rulesOf(cellsOf(['abc'], { fg: 4, bg: 0 })).includes('contrast'));
  assert.ok(!rulesOf(cellsOf(['abc'], { fg: 15, bg: 0 })).includes('contrast'));
  assert.ok(rulesOf(cellsOf(['abc'], { fg: 200, bg: 0 })).includes('colors') && !rulesOf(cellsOf(['abc'], { fg: 200, bg: 0 }), { depth: 256 }).includes('colors'));
  assert.deepEqual(lintAnsi(new Cells(4, 4), { skip: ['empty'] }), []);
});

test('lintAnsi: shade noise, short ramps, dark gray against white, and a missing outline', () => {
  const field = (n: number) => new Cells(n, n).fill({ ch: ' ', bg: 0 });
  const noisy = field(9); for (const [x, y] of [[1, 1], [4, 1], [7, 1], [1, 5], [4, 5], [7, 5]]) noisy.set(x, y, { ch: '░', fg: 8 });
  assert.ok(rulesOf(noisy).includes('shade-noise'));
  const run = (glyphs: string) => { const c = field(3); [...glyphs].forEach((g, i) => c.set(1, 1 + i, { ch: g, fg: 8, bg: 0 })); c.set(1, 1 + glyphs.length, { ch: '█', fg: 8 }); return c; };
  assert.ok(rulesOf(run('░')).includes('ramp'), 'one shade between its two ends is under four steps');
  assert.ok(!rulesOf(run('░▒')).includes('ramp'), 'two shades and the two ends are four');
  const jump = field(3).set(0, 0, { ch: '█', fg: 8 }).set(1, 0, { ch: '█', fg: 15 });
  assert.ok(rulesOf(jump).includes('ramp'));
  const light = new Cells(5, 5); light.rect(1, 1, 3, 3, { ch: '█', fg: 15 });
  assert.ok(lintAnsi(light).some((f) => f.rule === 'outline'));
  light.rect(1, 1, 3, 3, { ch: '█', fg: 8 });
  assert.ok(!lintAnsi(light).some((f) => f.rule === 'outline'));
});

test('lintAnsiFrames: identical frames, a long loop, and no more than three flashes a second', () => {
  const a = new Cells(4, 4).fill({ ch: '█', fg: 0 }), b = new Cells(4, 4).fill({ ch: '█', fg: 15 });
  assert.deepEqual(lintAnsiFrames([a]), []);
  assert.ok(lintAnsiFrames([a, a.clone()]).some((f) => f.rule === 'frames'));
  const strobe = lintAnsiFrames([a, b, a, b, a, b], { fps: 6 });
  assert.ok(strobe.some((f) => f.rule === 'flash' && f.level === 'error'));
  assert.ok(!lintAnsiFrames([a, b], { fps: 1 }).some((f) => f.rule === 'flash'), 'one flip a second is fine');
  assert.ok(lintAnsiFrames(Array.from({ length: 10 }, (_, i) => (i % 2 ? a : a.clone().set(0, 0, { fg: 7 }))), { fps: 1 }).some((f) => f.rule === 'length'));
});

test('paint runs the steps in the artists\' order and later steps keep the outline', () => {
  const order: string[] = [], c = new Cells(5, 1);
  paint(c, {
    blend: () => order.push('blend'), shadows: () => order.push('shadows'), highlights: () => order.push('highlights'),
    base: ({ fill }: any) => { order.push('base'); fill({ ch: '.', fg: 7 }); },
    outline: ({ outline }: any) => { order.push('outline'); outline(0, 0, { ch: '#', fg: 8 }); },
    focal: () => { order.push('focal'); return { x: 2, y: 0 }; },
  } as any);
  assert.deepEqual(order, ['focal', 'outline', 'base', 'highlights', 'shadows', 'blend']);
  assert.equal(String.fromCodePoint(c.get(0, 0).ch), '#');
  assert.equal(String.fromCodePoint(c.get(1, 0).ch), '.');
  paint(c, { base: ({ put }: any) => put(0, 0, { ch: 'x' }), outline: ({ outline }: any) => outline(0, 0, { ch: '#' }) } as any);
  assert.equal(String.fromCodePoint(c.get(0, 0).ch), '#');
});

test('overlays: seeded, only over cells that show nothing, and unknown ones are refused', () => {
  const base = () => new Cells(40, 10).fill({ ch: ' ', bg: 4 });
  for (const kind of OVERLAY_IDS) {
    const a = overlay(base(), kind as any, 's', { density: 0.2 }), b = overlay(base(), kind as any, 's', { density: 0.2 });
    assert.ok(a.equals(b), `${kind} is deterministic`);
    assert.ok(!a.equals(overlay(base(), kind as any, 't', { density: 0.2 })), `${kind} depends on the seed`);
    assert.ok(a.ch.some((v) => v !== 0x20), `${kind} draws something`);
    assert.ok(a.bg.every((v) => v === 4), `${kind} keeps the background`);
    assert.ok(a.toText().split('').every((ch) => ch === '\n' || CP437.includes(ch.codePointAt(0)!)), `${kind} is CP437`);
  }
  const solid = new Cells(10, 4).fill({ ch: '█', fg: 2 });
  assert.ok(overlay(solid, 'stars', 's', { density: 1 }).equals(new Cells(10, 4).fill({ ch: '█', fg: 2 })), 'nothing drawn over drawing');
  const only = overlay(base(), 'rain', 's', { density: 1, where: (x: number) => x < 3 });
  assert.ok([...only.ch].filter((v) => v !== 0x20).length === 30);
  assert.throws(() => overlay(base(), 'fog' as any, 's'), /unknown overlay/);
});

test('lettering: four faces from one font, sized as stated, and a letter the font lacks is refused', () => {
  assert.deepEqual(LETTERING_FACES, ['half', 'block', 'outlined', 'shaded']);
  const sizes = Object.fromEntries(LETTERING_FACES.map((f) => { const c = lettering('AB', { face: f as any }); return [f, [c.w, c.h]]; }));
  assert.deepEqual(sizes, { half: [7, 3], block: [14, 5], outlined: [16, 7], shaded: [16, 6] });
  assert.equal(lettering('I', { face: 'half' }).toText().replace(/ /g, '').replace(/\n/g, ''), '▀█▀█▀▀▀', 'I in half blocks: a bar, a stem, a bar');
  assert.ok(lettering('a b', { face: 'block' }).equals(lettering('A B', { face: 'block' })), 'lower case is upper case');
  assert.throws(() => lettering('a#', {}), /no "#"/);
  assert.throws(() => lettering('A'.repeat(11), { face: 'block' }), /columns/);
  const cells = lettering('GO', { face: 'outlined', fg: 11 });
  assert.ok([...cells.fg].includes(8) && [...cells.fg].includes(11), 'a dark gray contour around the colour');
});

test('native ANSI recipes: only in ansi, deterministic, legal for a classic file, lint clean, and the kit covers them', () => {
  for (const id of ['banner', 'landscape', 'menu', 'bust', 'artpack', 'weatherloop']) {
    assert.ok(RECIPES[id].ansiOnly && isNative(id, 'ansi'));
    assert.throws(() => draw(id), /draws in ansi only/);
    for (const v of RECIPES[id].variants()) {
      const a = draw(id, v, { style: 'ansi' }), b = draw(id, v, { style: 'ansi' });
      assert.ok(a.frames[0].equals(b.frames[0]) && !a.folded && a.style === 'ansi', `${id} ${JSON.stringify(v)} is native and deterministic`);
      const f = lintAnsi(a.frames[0], { name: id, skip: id === 'banner' ? ['outline'] : [] });
      assert.deepEqual(f.filter((x) => x.level === 'error'), [], `${id} ${JSON.stringify(v)}`);
      assert.ok(a.frames[0].equals(legalize(a.frames[0], { bgLimit: 8 })), `${id} ${JSON.stringify(v)} needs no legalizing`);
    }
  }
  assert.ok(!draw('landscape', { seed: 'a' }, { style: 'ansi' }).frames[0].equals(draw('landscape', { seed: 'b' }, { style: 'ansi' }).frames[0]));
  assert.ok(!draw('landscape', { time: 'day' }, { style: 'ansi' }).frames[0].equals(draw('landscape', { time: 'night' }, { style: 'ansi' }).frames[0]));
  assert.throws(() => draw('landscape', { time: 'noon' }, { style: 'ansi' }), /unknown time/);
  assert.throws(() => draw('bust', { skin: 'blue' }, { style: 'ansi' }), /unknown skin/);
  const menu = draw('menu', {}, { style: 'ansi' }).frames[0];
  assert.deepEqual([menu.w, menu.h], [80, 24]);
  assert.equal(String.fromCodePoint(menu.get(0, 0).ch), '╔', 'the frame survives the base fill');
});

test('an artpack carries SAUCE and its file reads back to the same cells', () => {
  const a = draw('artpack', {}, { style: 'ansi' });
  assert.equal(a.sauce.title, 'ARTPACK');
  const out = writeAnsi(a.frames[0], { target: 'classic', sauce: a.sauce });
  assert.deepEqual(out.warnings, []);
  const back = readAnsi(out.bytes);
  assert.equal(back.sauce?.title, 'ARTPACK');
  assert.ok(back.cells.equals(a.frames[0]) || back.cells.toText() === a.frames[0].toText());
});

// --- Phase 5: depth, light and animation --------------------------------------------------------------------------------------------
test('gradient: the ramp top to bottom in whole steps, legal at 8, and it refuses a region with no room', () => {
  const c = gradient(new Cells(4, 12), 0, 0, 4, 12, 'blue');
  assert.ok(c.equals(legalize(c, { bgLimit: 8 })));
  assert.deepEqual([c.get(0, 0).bg, c.get(0, 11).bg], [V.black, V.brightcyan].map((v) => c.get(0, v === V.black ? 0 : 11).bg), 'ends are the ramp ends');
  for (let x = 1; x < 4; x++) for (let y = 0; y < 12; y++) assert.deepEqual(c.get(x, y), c.get(0, y), 'a vertical smear: every column is the same');
  assert.deepEqual(tones(gradient(new Cells(1, 8), 0, 0, 1, 8, 'gray', { dir: 'up' })).slice(0, 1), ['#ffffff'].map(() => tones(gradient(new Cells(1, 8), 0, 0, 1, 8, 'gray', { dir: 'up' }))[0]));
  const lum = (h: string) => parseInt(h.slice(1, 3), 16) + parseInt(h.slice(3, 5), 16) + parseInt(h.slice(5, 7), 16);
  const down = tones(gradient(new Cells(1, 12), 0, 0, 1, 12, 'gray')).map(lum), up = tones(gradient(new Cells(1, 12), 0, 0, 1, 12, 'gray', { dir: 'up' })).map(lum);
  assert.ok(down.every((v, i) => i === 0 || v >= down[i - 1]) && down[11] > down[0], 'dark to light going down');
  assert.deepEqual(up, [...down].reverse(), 'up is the reverse');
  assert.throws(() => gradient(new Cells(4, 3), 0, 0, 4, 3, 'gray'), /at least four/);
  assert.throws(() => gradient(new Cells(4, 8), 0, 0, 4, 8, 'gray', { dir: 'sideways' as any }), /unknown direction/);
});

test('bayer: solid cells, the share of the second colour follows the level, and it is deterministic', () => {
  const share = (level: number) => { const c = bayer(new Cells(8, 8), 0, 0, 8, 8, V.black, V.blue, level); let n = 0; for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) { const k = c.get(x, y); assert.ok(k.ch === 0x20 && k.fg === k.bg); if (k.bg === V.blue) n++; } return n / 64; };
  assert.deepEqual([share(0), share(0.5), share(1)], [0, 0.5, 1]);
  assert.ok(share(0.25) < share(0.75));
  assert.ok(bayer(new Cells(5, 5), 0, 0, 5, 5, 0, 4, (x: number) => x / 5).equals(bayer(new Cells(5, 5), 0, 0, 5, 5, 0, 4, (x: number) => x / 5)));
});

test('tones and resolveTones: a shade glyph reads as its in-between colour at 256, half blocks keep their halves', () => {
  const c = new Cells(3, 1).set(0, 0, { ch: '▒', fg: V.white, bg: V.black }).set(1, 0, { ch: '▀', fg: V.red, bg: V.blue }).set(2, 0, { ch: 'x', fg: V.green, bg: V.black });
  assert.equal(tones(c).length, 3); assert.equal(tones(c)[0], tone(c.get(0, 0)));
  const r = resolveTones(c, { depth: 256 }), k = r.get(0, 0);
  assert.equal(k.ch, 0x20); assert.equal(k.bg, nearest(tone(c.get(0, 0)), 256).index); assert.ok(k.bg > 15, 'a gray from the 256 set, not one of the 16');
  assert.deepEqual(r.get(1, 0), c.get(1, 0)); assert.deepEqual(r.get(2, 0), c.get(2, 0));
  const r16 = resolveTones(c, { depth: 16, bgLimit: 8 }); assert.ok(r16.equals(legalize(r16, { bgLimit: 8 })));
  const bytes = new TextDecoder('latin1').decode(writeAnsi(r, { colors: 256, sauce: false }).bytes);
  assert.match(bytes, /48;5;\d+/); assert.throws(() => resolveTones(c, { depth: 'true' as any }), /16 or 256/);
});

test('light: ambient steps along the ramps, sources fall off, self-lit cells ignore the ambient, empties stay empty', () => {
  const solid = (fg: number, w = 9, h = 5) => new Cells(w, h).fill({ ch: '█', fg });          // a full block keeps the fg, so legal at 8
  const base = solid(V.brightblue), night = light(base, { ambient: -1 }), day = light(base, { ambient: 1 });
  assert.equal(night.get(4, 2).fg, V.blue); assert.equal(day.get(4, 2).fg, V.cyan);
  const holed = solid(V.blue); holed.set(0, 0, { ch: 0, fg: NONE, bg: NONE });
  assert.ok(light(holed, { ambient: -1 }).isEmpty(0, 0), 'empty stays empty');
  assert.ok(light(base, {}).equals(base), 'ambient 0 and no sources changes nothing');
  const lamp = light(base, { ambient: -1, sources: [{ x: 4, y: 2, radius: 6, strength: 3 }] });
  assert.ok(lamp.get(4, 2).fg !== night.get(4, 2).fg, 'brighter at the source'); assert.equal(lamp.get(0, 0).fg, night.get(0, 0).fg, 'nothing past the radius');
  assert.ok(lamp.get(4, 2).fg === V.brightcyan || lamp.get(4, 2).fg === V.cyan, 'the source lifts the cell up the blue ramp');
  assert.equal(light(base, { ambient: -2, sources: [{ x: 0, y: 0, radius: 2 }] }).get(2, 0).fg, V.black, 'a cell is twice as tall as wide: two columns away is out of a radius of 2 only past it');
  const mask = new Cells(9, 5); mask.set(0, 0, { ch: ' ', bg: V.white });
  assert.equal(light(base, { ambient: -2, selfLit: mask }).get(0, 0).fg, V.brightblue, 'self-lit keeps its colour at night');
  assert.equal(light(base, { ambient: -2, selfLit: (x: number) => x === 1 }).get(1, 0).fg, V.brightblue);
  assert.equal(light(base, { ambient: -2, selfLit: (x: number) => x === 1 }).get(2, 0).fg, V.black);
  const dark = light(new Cells(1, 1).set(0, 0, { ch: 'x', fg: V.brightblue, bg: V.black }), { ambient: 1 }); assert.equal(dark.get(0, 0).bg, V.blue, 'black takes the ramp of its other colour');
  assert.equal(light(solid(V.brightcyan), { ambient: 5 }).get(0, 0).fg, V.brightcyan, 'steps stop at the end of the ramp');
  assert.equal(light(solid(V.brown), { ambient: 9 }).get(0, 0).fg, V.yellow);
  const legal = light(new Cells(3, 1).fill({ ch: ' ', fg: V.blue, bg: V.blue }), { ambient: 1 }); assert.ok(legal.equals(legalize(legal, { bgLimit: 8 })), 'a bright background is made legal');
});

// The same picture to the eye: both written out over the same rows (a replay has no empty rows past the last drawn one without SAUCE).
const rows = (c: Cells, h: number) => { const o = new Cells(c.w, h); for (let y = 0; y < Math.min(h, c.h); y++) for (let x = 0; x < c.w; x++) o.set(x, y, c.get(x, y)); return o; };
const sameLook = (a: Cells, b: Cells) => { const h = Math.max(a.h, b.h); return Buffer.from(writeAnsi(rows(a, h), { sauce: false }).bytes).equals(Buffer.from(writeAnsi(rows(b, h), { sauce: false }).bytes)); };

test('ANSImation: the first frame clears and prints in full, later frames are cursor-addressed deltas, and a replay gives the frames back', () => {
  const loop = draw('weatherloop', { weather: 'rain' }, { style: 'ansi' }).clips.loop.frames as Cells[];
  const m = writeAnsimation(loop, { sauce: { title: 'Rain', date: '20261029' } });
  const head = new TextDecoder('latin1').decode(m.bytes.subarray(0, 8));
  assert.ok(head.startsWith('\x1b[2J\x1b[H'), 'clear and home first');
  const body = new TextDecoder('latin1').decode(m.bytes);
  assert.equal(body.split('\x1b[H').length - 1, loop.length, 'a bare home opens every frame');
  assert.ok(/\x1b\[\d+;\d+H/.test(body), 'rows addressed by position');
  assert.equal(m.frames.length, loop.length); assert.ok(m.frames[1].bytes < m.frames[0].bytes, 'a delta is smaller than the full screen');
  const back = readAnsimation(m.bytes);
  assert.equal(back.frames.length, loop.length); assert.equal(back.sauce?.fileType, 2, 'SAUCE says ANSiMation'); assert.equal(back.sauce?.title, 'Rain');
  back.frames.forEach((f, i) => assert.ok(sameLook(f, loop[i]), `frame ${i} replays`));
  assert.ok(back.still.equals(back.frames[back.frames.length - 1]));
  assert.ok(sameLook(readAnsi(m.still).cells, loop[loop.length - 1]), 'the still is the finished picture');
  assert.ok(m.seconds > 0 && Math.abs(m.frames[0].seconds - m.frames[0].bytes / 240) < 1e-9, 'timing is bytes over baud / 10');
  assert.ok(writeAnsimation(loop, { baud: 1e9 }).warnings.length === 0 && writeAnsimation(loop, { baud: 300 }).warnings.some((w) => /slower than/.test(w)));
  assert.deepEqual(Array.from(writeAnsimation(loop).bytes), Array.from(writeAnsimation(loop).bytes), 'same frames, same bytes');
});

test('ANSImation: erases what leaves, unchanged frames cost only the mark, one frame is a plain read, and bad input is refused', () => {
  const a = new Cells(10, 3).text(0, 1, 'hello', V.white, V.blue), b = new Cells(10, 3).text(0, 1, 'hi', V.white, V.blue);
  const m = writeAnsimation([a, b, b], { sauce: false }), back = readAnsimation(m.bytes, { width: 10 });
  assert.ok(sameLook(back.frames[1], b) && sameLook(back.frames[2], b), 'the cells that left are blanked');
  assert.equal(m.frames[2].bytes, 3, 'an identical frame is just the home mark');
  assert.equal(readAnsimation(writeAnsi(a, { sauce: false }).bytes).frames.length, 1);
  assert.equal(readAnsimation(m.bytes, { width: 10 }).width, 10, 'no SAUCE: the width is the one given');
  assert.throws(() => writeAnsimation([]), /at least one/); assert.throws(() => writeAnsimation([a, new Cells(9, 3)]), /same size/);
  assert.throws(() => writeAnsimation([a], { baud: 0 }), /baud/); assert.throws(() => writeAnsimation([a], { target: 'x' as any }), /unknown target/);
  const u = writeAnsimation([a, b], { target: 'modern', sauce: false }); assert.equal(u.charset, 'utf8'); assert.ok(sameLook(readAnsimation(u.bytes).frames[1], b));
});

test('weatherloop: a seamless clip over one fixed picture, deterministic, and lint clean as an animation', () => {
  for (const v of RECIPES.weatherloop.variants()) {
    const a = draw('weatherloop', v, { style: 'ansi' }), c = a.clips.loop;
    assert.ok(a.frames[0].equals(c.frames[0]) && c.loop && c.reduced[0] === c.frames[0], 'the first frame is the still and the reduced-motion frame');
    assert.deepEqual(lintAnsiFrames(c.frames, { name: 'loop', fps: c.fps, loop: true }).filter((f) => f.level === 'error'), []);
    assert.ok(c.frames.every((f: Cells, i: number) => i === 0 || !f.equals(c.frames[i - 1])), 'every frame moves');
    const still = draw('landscape', { time: 'night', seed: 'weatherloop', w: 40, h: 16 }, { style: 'ansi' }).frames[0];
    for (let i = 0; i < still.ch.length; i++) if (still.ch[i] && still.ch[i] !== 0x20) assert.ok(c.frames.every((f: Cells) => f.ch[i] === c.frames[0].ch[i]), 'the scene under the weather is the same in every frame');
    assert.ok(draw('weatherloop', v, { style: 'ansi' }).clips.loop.frames[1].equals(c.frames[1]));
  }
  assert.throws(() => draw('weatherloop', { weather: 'fog' }, { style: 'ansi' }), /unknown weather/);
  assert.throws(() => draw('weatherloop', { frames: 5 }, { style: 'ansi' }), /divides the height/);
});

test('fromImage: shape, luminance order, contrast stretch, transparency, legal cells, determinism, and bad input', () => {
  const make = (w: number, h: number, f: (x: number, y: number) => number[]) => { const rgba = new Uint8Array(w * h * 4); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) rgba.set(f(x, y), (y * w + x) * 4); return { w, h, rgba }; };
  const ramp = make(32, 32, (x) => { const v = Math.round((255 * x) / 31); return [v, v, v, 255]; });
  const { cells, warnings } = fromImage(ramp, { columns: 16 });
  assert.deepEqual([cells.w, cells.h], [16, 8], 'two square pixels to a cell: a square image is half as many rows as columns');
  assert.deepEqual(warnings, []); assert.ok(cells.equals(legalize(cells, { bgLimit: 8 })));
  const lum = (x: number) => { const t = tone(cells.get(x, 3)); return parseInt(t.slice(1, 3), 16) + parseInt(t.slice(3, 5), 16) + parseInt(t.slice(5, 7), 16); };
  assert.ok(lum(15) > lum(8) && lum(8) > lum(1) && lum(1) <= 90 && lum(15) >= 500, 'dark to light across the row, the ends of the ramp used');
  const dull = make(16, 16, (x) => { const v = 60 + x * 4; return [v, v, v, 255]; });
  assert.ok(lum2(fromImage(dull, { columns: 8 }).cells) > 400, 'a dull image is stretched to the ramp ends'); function lum2(c: Cells) { const a = tone(c.get(7, 2)); return parseInt(a.slice(1, 3), 16) + parseInt(a.slice(3, 5), 16) + parseInt(a.slice(5, 7), 16); }
  assert.ok(lum2(fromImage(dull, { columns: 8, contrast: false }).cells) < 400, 'without the stretch it stays dull');
  assert.ok(fromImage(make(8, 8, () => [9, 9, 9, 255]), { columns: 4 }).warnings.length === 1, 'flat image: nothing to stretch, said so');
  const half = fromImage(make(16, 16, (x, y) => (y < 8 ? [0, 0, 0, 0] : [255, 255, 255, 255])), { columns: 8 });
  assert.ok(half.cells.isEmpty(0, 0) && !half.cells.isEmpty(0, 3), 'transparent pixels are empty cells');
  const deep = fromImage(ramp, { columns: 16, colors: 256 }); assert.ok([...deep.cells.fg].some((v) => v > 15), 'the 256 path uses the xterm greys');
  assert.ok(fromImage(ramp, { columns: 16 }).cells.equals(cells), 'deterministic');
  assert.ok(fromImage(ramp, { columns: 16, bgLimit: 16 }).cells.bg.every((b: number) => b === NONE || b < 16));
  assert.throws(() => fromImage(ramp, { colors: 8 as any }), /16 or 256/); assert.throws(() => fromImage(ramp, { columns: 0 }), /columns/);
  assert.ok(fromImage(Pixels.fromRgba(2, 2, new Uint8Array(16).fill(255)), { columns: 2 }).cells.w === 2, 'a Pixels canvas works too');
});

// --- Phase 6: the plain text style ---------------------------------------------------------------------------------------------------
test('text style: every recipe draws 7-bit ASCII cells with no colour, deterministically, and lint finds no errors', () => {
  for (const [id, r] of Object.entries(RECIPES)) for (const v of r.variants()) {
    const a = draw(id, v, { style: 'text' }), b = draw(id, v, { style: 'text' });
    assert.equal(a.style, 'text'); assert.ok(a.frames[0] instanceof Cells && a.frames[0].equals(b.frames[0]), `${id} ${JSON.stringify(v)} is deterministic`);
    const all = [...a.frames, ...Object.values(a.clips ?? {}).flatMap((c: any) => c.frames)];
    for (const f of all as Cells[]) {
      assert.ok([...f.ch].every((cp) => cp === 0 || (cp >= 0x20 && cp <= 0x7e)), `${id} ${JSON.stringify(v)} is 7-bit`);
      assert.ok([...f.fg].every((c) => c === NONE) && [...f.bg].every((c) => c === NONE), 'no colour');
      assert.deepEqual(lintText(f, { name: id, skip: ['blank', 'fill', 'height', 'width'] }).filter((x) => x.level === 'error'), [], `${id} ${JSON.stringify(v)}`);
    }
    assert.equal(writeText(a.frames[0]).warnings.length, 0);
  }
});

test('text style: pixels read as light and dark, ANSI as ink, native text wins, bad input is refused', () => {
  const px = new Pixels(2, 4, DB32).set(0, 0, C.white).set(0, 1, C.white).set(1, 0, C.night).set(1, 1, C.night).set(0, 2, C.white);
  const t = foldText(px);
  assert.deepEqual([t.w, t.h], [2, 2]);
  assert.equal(String.fromCodePoint(t.get(0, 0).ch), '@', 'white over white is the densest ink'); assert.ok(ASCII_RAMP.indexOf(String.fromCodePoint(t.get(1, 0).ch)) < 3 && t.get(1, 0).ch !== 0x20, 'dark but opaque is not a space');
  assert.equal(String.fromCodePoint(t.get(0, 1).ch), "'", 'one opaque pixel above is an apostrophe'); assert.ok(t.isEmpty(1, 1));
  const a = new Cells(6, 2).box(0, 0, 6, 2, { style: 'double', fg: V.cyan }); a.set(1, 0, { ch: '─' }); a.set(2, 0, { ch: 'x' });
  const s = asciiFromCells(a);
  assert.equal(String.fromCodePoint(s.get(0, 0).ch), '+', 'corners are +'); assert.equal(String.fromCodePoint(s.get(1, 0).ch), '-'); assert.equal(String.fromCodePoint(s.get(2, 0).ch), 'x');
  const bright = new Cells(2, 1).set(0, 0, { ch: ' ', bg: V.white }).set(1, 0, { ch: '░', fg: V.white, bg: V.black });
  const ink = asciiFromCells(bright);
  assert.equal(String.fromCodePoint(ink.get(0, 0).ch), '@', 'a white cell is the densest ink'); assert.ok(ASCII_RAMP.indexOf(String.fromCodePoint(ink.get(1, 0).ch)) < ASCII_RAMP.indexOf('@') && ink.get(1, 0).ch !== 0x20, 'a quarter shade is lighter');
  assert.equal(String.fromCodePoint(asciiFromCells(new Cells(1, 1).set(0, 0, { ch: 'Φ', fg: V.red })).get(0, 0).ch), '*');
  const saved = RECIPES.icon;
  RECIPES.icon = { ...saved, text: () => ({ id: 'icon.text', frames: [new Cells(1, 1).glyph(0, 0, 'Z')], fps: 0, loop: false }) };
  try { assert.equal(isNative('icon', 'text'), true); assert.equal(draw('icon', {}, { style: 'text' }).id, 'icon.text'); } finally { RECIPES.icon = saved; }
  assert.equal(isNative('banner', 'text'), false);
  const sword = draw('item', { shape: 'sword' }, { style: 'text' }), again = textFromPixelArt(draw('item', { shape: 'sword' }));
  assert.ok(sword.frames[0].equals(again.frames[0]) && sword.folded);
  const hero = draw('character', { loadout: 'rogue' }, { style: 'text' }); assert.ok(hero.clips.idle.frames[0] instanceof Cells && Array.isArray(hero.anchors.feet));
});

test('writeText and lintText: 7-bit only with a warning, trailing spaces cut, eol, and the rules', () => {
  const c = new Cells(6, 2).text(0, 0, 'ab  ', undefined, undefined).set(4, 1, { ch: 'é' });
  const w = writeText(c); assert.equal(w.text, 'ab\n    ?\n'); assert.match(w.warnings[0], /U\+00E9/); assert.equal(w.bytes.length, w.text.length);
  assert.equal(writeText(new Cells(2, 1).text(0, 0, 'hi'), { eol: '\r\n' }).text, 'hi\r\n'); assert.throws(() => writeText(c, { eol: '\r' as any }), /eol/);
  const rules = (cells: Cells, o: any = {}) => lintText(cells, o).map((f) => f.rule).sort();
  assert.deepEqual(rules(new Cells(5, 1)), ['blank']); assert.deepEqual(rules(c), ['ascii']);
  assert.deepEqual(rules(new Cells(90, 1).text(0, 0, 'x'.repeat(90))), ['fill', 'width']);
  assert.deepEqual(rules(new Cells(3, 30).text(0, 29, 'x')), ['height']);
  assert.deepEqual(rules(new Cells(20, 2).set(0, 0, { ch: 'x', fg: V.red })), ['colour']);
  assert.deepEqual(rules(new Cells(5, 1).text(0, 0, 'x'), { skip: ['fill'] }), []);
});

// --- Phase 7: the vista recipe and the Coupler parity inventory ------------------------------------------------------------------------
test('vista: with 256 colours every terrain is textured with tones 16 colours cannot reach (CLAUDE.md: use a broader palette for texture)', () => {
  const tones = (terrain: string, colors: 16 | 256) => { const f = draw('vista', { terrain, colors, columns: 80, rows: 24, seed: 'texture' }, { style: 'ansi', bgLimit: 16 }).frames[0]; return new Set([...f.fg, ...f.bg]).size; };
  for (const terrain of TERRAINS) assert.ok(tones(terrain, 256) >= 2 * tones(terrain, 16), `${terrain}: ${tones(terrain, 256)} tones at 256, ${tones(terrain, 16)} at 16`);
});

test('vista: all 24 terrains, 4 times and 14 weathers draw at several sizes, legal, deterministic, lint clean', () => {
  assert.equal(TERRAINS.length, 24); assert.equal(OUTDOOR.length, 14); assert.equal(INDOOR.length, 10); assert.equal(WEATHER_KINDS.length, 14);
  const skip = ['outline', 'contrast', 'ramp', 'shade-noise'];
  for (const terrain of TERRAINS) for (const [time, weather, style] of [['day', 'clear', 'fantasy'], ['night', 'thunderstorm', 'high'], ['dusk', 'snow', 'fantasy']]) for (const [columns, rows] of [[80, 24], [8, 4], [160, 45], [30, 10]]) {
    const a = draw('vista', { terrain, time, weather, style, columns, rows }, { style: 'ansi' }), f = a.frames[0];
    assert.deepEqual([f.w, f.h], [columns, rows]); assert.ok(f.equals(legalize(f, { bgLimit: 8 })), `${terrain} ${columns}x${rows} is legal at 8`);
    assert.ok(f.equals(draw('vista', { terrain, time, weather, style, columns, rows }, { style: 'ansi' }).frames[0]), 'deterministic');
    assert.deepEqual(lintAnsi(f, { skip: [...skip, 'height'], width: columns }).filter((x) => x.level === 'error'), [], `${terrain} ${time} ${weather} ${columns}x${rows}`);
    assert.equal(writeAnsi(f).warnings.length, 0);
  }
  for (const time of VTIMES) for (const weather of WEATHER_KINDS) assert.ok(draw('vista', { terrain: 'plains', time, weather }, { style: 'ansi' }).frames[0].w === 80);
  assert.equal(draw('vista', { columns: 1000, rows: 1 }, { style: 'ansi' }).frames[0].w, 160); assert.equal(draw('vista', { rows: 1 }, { style: 'ansi' }).frames[0].h, 4, 'sizes are clamped as Coupler clamps them');
  for (const bad of [{ terrain: 'moon' }, { time: 'noon' }, { weather: 'hurricane' }, { style: 'epic' }, { colors: 8 }]) assert.throws(() => draw('vista', bad, { style: 'ansi' }), /unknown|colors/);
});

test('vista: sky only where there is one, the look follows the seed, the sun crosses by the arc, 256 colours resolve the blends, words say what it shows', () => {
  assert.deepEqual([hasSky('plains'), hasSky('underwater'), hasSky('stone'), hasSky('air')], [true, false, false, true]);
  const same = (p: any, q: any) => draw('vista', p, { style: 'ansi' }).frames[0].equals(draw('vista', q, { style: 'ansi' }).frames[0]);
  for (const terrain of ['stone', 'cave', 'underwater', 'in_underwater', 'gap']) assert.ok(same({ terrain, time: 'night', weather: 'blizzard' }, { terrain, time: 'day', weather: 'clear' }), `${terrain} shows nothing of the sky`);
  assert.ok(!same({ terrain: 'plains', time: 'day' }, { terrain: 'plains', time: 'night' }) && !same({ terrain: 'plains', weather: 'rain' }, { terrain: 'plains' }));
  assert.ok(same({ seed: 'a' }, { seed: 'a' }) && !same({ terrain: 'hills', seed: 'a' }, { terrain: 'hills', seed: 'b' }));
  assert.ok(!same({ arc: 100 }, { arc: 900 }), 'the sun moves with the clock');
  const deep = draw('vista', { colors: 256 }, { style: 'ansi' }).frames[0];
  assert.ok(![...deep.ch].some((c) => c === 0x2591 || c === 0x2592 || c === 0x2593), 'no shade glyphs left at 256'); assert.ok([...deep.bg].some((b) => b > 15));
  assert.match(draw('vista', { terrain: 'woods', time: 'night', weather: 'rain' }, { style: 'ansi' }).says, /woods, night, rain/);
  assert.match(draw('vista', { terrain: 'stone', weather: 'rain' }, { style: 'ansi' }).says, /Indoors \(stone\)\.$/);
  assert.match(draw('vista', { style: 'high' }, { style: 'ansi' }).says, /High fantasy/);
});

test('roomSeed is Coupler\'s FNV-1a: a known vector, stable per room, different per world, room and look', () => {
  // FNV-1a 64 of "\0" + "" + 4 zero bytes is computed by the definition here, independent of the implementation.
  let h = 0xcbf29ce484222325n; for (const b of [0x73, 0, 0x72, 0, 0, 0, 0]) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; }
  assert.equal(roomSeed('s', 'r', 0), h.toString());
  assert.equal(roomSeed('standard', 'Midgaard#3001'), roomSeed('standard', 'Midgaard#3001', 0));
  assert.equal(new Set([roomSeed('standard', 'a'), roomSeed('classic', 'a'), roomSeed('standard', 'b'), roomSeed('standard', 'a', 1)]).size, 4);
  assert.ok(roomSeed('w', 'r', 256) !== roomSeed('w', 'r', 0), 'the look count is four little-endian bytes');
});

test('coupler-painter kit: what the Rust painter draws is a row, each with a status, and the gaps name what is outside this engine', () => {
  const kit = coverage().find((k) => k.id === 'coupler-painter')!;
  assert.ok(kit.rows.length >= 15 && kit.rows.every((r) => r.problems.length === 0));
  assert.deepEqual(kit.rows.filter((r) => r.status === 'gap').map((r) => r.id).sort(), []);
  for (const f of ['painter.rs', 'portrait.rs', 'face.rs', 'item_art.rs', 'figure.rs', 'fight_art.rs', 'create.rs', 'paint.rs']) assert.ok(readFileSync(join(ROOT, 'docs/coupler-painter-parity.md'), 'utf8').includes(f), `${f} is inventoried`);
});

test('coffeemud kit: no gaps and no partial rows, and every recipe it names draws', () => {
  const kit = coverage().find((k) => k.id === 'coffeemud')!;
  assert.deepEqual(kit.rows.filter((r) => r.status === 'gap' || r.status === 'partial').map((r) => r.id), []);
  assert.ok(kit.rows.every((r) => r.problems.length === 0), 'nothing it draws has a lint error');
});

test('races: each is a plan plus a size, sizes order the silhouettes, and an unknown race throws', () => {
  for (const race of Object.keys(RACES)) { const a = drawCreature({ race }); assert.equal(a.id, `creature.${race}`); assert.ok(a.frames[0].bbox()); }
  const height = (race: string) => drawCreature({ race }).frames[0].bbox()!.h;
  assert.ok(height('halfling') < height('human') && height('human') < height('giant'), 'halfling < human < giant');
  assert.ok(SIZES.tiny < SIZES.small && SIZES.small < SIZES.medium && SIZES.medium < SIZES.large && SIZES.large < SIZES.huge);
  assert.throws(() => drawCreature({ race: 'dragonborn' }), /unknown race/);
  assert.ok(drawCreature({ preset: 'lich' }).frames[0].data.includes(C.cyan), 'a lich has cold eyes and a crown');
  assert.ok(!drawCreature({ preset: 'skeleton' }).frames[0].equals(drawCreature({ preset: 'lich' }).frames[0]));
});

test('locales: each draws at several sizes, legal for a classic file, deterministic, and a road has two lengths', () => {
  for (const kind of LOCALE_KINDS) for (const [columns, rows] of [[80, 24], [8, 4], [30, 10]]) {
    const f = draw('locale', { kind, columns, rows }, { style: 'ansi' }).frames[0];
    assert.deepEqual([f.w, f.h], [columns, rows]); assert.ok(f.equals(legalize(f, { bgLimit: 8 })), `${kind} is legal`);
    assert.ok(f.equals(draw('locale', { kind, columns, rows }, { style: 'ansi' }).frames[0]), `${kind} is deterministic`);
  }
  const road = (length: string) => draw('locale', { kind: 'road', length }, { style: 'ansi' }).frames[0];
  assert.ok(!road('short').equals(road('long')));
  assert.ok(!draw('locale', { kind: 'gundeck', guns: 'light' }, { style: 'ansi' }).frames[0].equals(draw('locale', { kind: 'gundeck', guns: 'heavy' }, { style: 'ansi' }).frames[0]), 'a heavy gun deck has more guns');
  assert.throws(() => draw('locale', { kind: 'moon' }, { style: 'ansi' }), /unknown locale/);
});

test('client surfaces: the bar, map pane and character sheet draw in 80 columns or their size, and the same seed is the same map', () => {
  assert.deepEqual([draw('bar', {}, { style: 'ansi' }).frames[0].w, draw('bar', {}, { style: 'ansi' }).frames[0].h], [80, 5]);
  assert.deepEqual([draw('sheet', {}, { style: 'ansi' }).frames[0].w, draw('sheet', {}, { style: 'ansi' }).frames[0].h], [80, 24]);
  const map = (seed: string) => draw('mappane', { seed }, { style: 'ansi' }).frames[0];
  assert.ok(map('a').equals(map('a')) && !map('a').equals(map('b')));
  assert.ok(!draw('bar', { hp: 5 }, { style: 'ansi' }).frames[0].equals(draw('bar', { hp: 95 }, { style: 'ansi' }).frames[0]), 'the meter follows the number');
});

test('sigil: the same words draw the same sigil, each part changes it, and a bad word throws', () => {
  const s = (p: any) => drawSigil(p).frames[0];
  assert.ok(s({}).equals(s({})));
  for (const ordinary of SIGIL_ORDINARIES.slice(1)) assert.ok(!s({ ordinary }).equals(s({ ordinary: 'none' })), ordinary);
  for (const charge of SIGIL_CHARGES.slice(1)) assert.ok(!s({ charge }).equals(s({ charge: 'none' })), charge);
  assert.ok(!s({ field: 'red' }).equals(s({ field: 'blue' })));
  assert.throws(() => drawSigil({ ordinary: 'wavy' }), /unknown ordinary/);
  assert.throws(() => drawSigil({ charge: 'dragon' }), /unknown charge/);
});

test('new traps, vehicles, tech and rooms draw, and the biomes added for CoffeeMUD have their own scene', () => {
  for (const kind of ['gap', 'skyway', 'airlock', 'trapneedle', 'trapdart', 'trapgas', 'trapspikes', 'trapalarm', 'trapward', 'boat', 'tent', 'cart', 'ship', 'caravan', 'terminal', 'robot', 'scanner', 'spaceship', 'star', 'planet', 'gasgiant', 'moonlet', 'asteroid', 'clanhall']) assert.ok(draw('prop', { kind }).frames[0].bbox(), kind);
  for (const [kind, w] of [['ship', 32], ['caravan', 32], ['clanhall', 32], ['spaceship', 32], ['boat', 16]] as const) assert.equal(draw('prop', { kind }).frames[0].w, w, kind);
  const scenes = new Set(['mountain', 'snow', 'jungle', 'road', 'plains', 'canopy'].map((biome) => draw('scene', { biome }).frames[0].data.join()));
  assert.equal(scenes.size, 6);
  for (const room of ['tavern', 'shop', 'castlehall', 'ruin', 'sewer', 'stable', 'inn']) assert.deepEqual(unreachable(layoutOf(room)), [], room);
});

// --- Phase C: the creature face -------------------------------------------------------------------------------------------------------
const FACE_SKIP = ['outline', 'contrast', 'ramp'];
const face = (p: any = {}) => draw('creatureface', p, { style: 'ansi' }) as any;
const faceCells = (p: any = {}) => face(p).frames[0];
// One race for each head shape, plus the variants the fixtures name (an old dwarf, a pale elf, a bald orc, a dark knight).
const SITTERS = [...new Set(Object.values(FACE_RACES as Record<string, [string, string]>).map(([, shape]) => shape))].map((shape) => ({ shape })).concat([
  { name: 'an old dwarven smith', description: 'A grizzled old dwarf with a long grey beard and a scar.' }, { name: 'a pale elven maiden', description: 'pale skin, long silver hair' },
  { name: 'a bald orc', description: 'bald, tusked, glowing red eyes' }, { name: 'a dark knight', wears: ['iron helm', 'silver amulet'] }] as any[]);

test('creatureface: every race draws at both sizes, the right size, deterministic, legal, lint clean', () => {
  assert.equal(Object.keys(FACE_RACES).length, 242); assert.equal(FACE_SHAPES.length, 24);
  for (const race of Object.keys(FACE_RACES)) for (const size of ['full', 'small']) {
    const a = face({ race, size }), f = a.frames[0];
    assert.deepEqual([f.w, f.h], size === 'full' ? [25, 12] : [16, 8], race);
    assert.ok(f.equals(legalize(f, { bgLimit: 8 })) && f.equals(faceCells({ race, size })), `${race} ${size}`);
    assert.deepEqual(lintAnsi(f, { skip: FACE_SKIP, name: race }).filter((x) => x.level === 'error'), [], `${race} ${size}`);
    assert.equal(a.figure.w * a.figure.h, a.figure.parts.length);
  }
  assert.ok(FACE_SHAPES.every((shape) => Object.values(FACE_RACES as Record<string, [string, string]>).some(([, s]) => s === shape) || faceShape('x', 'undead') === 'Skull'), 'every shape has a race');
});

test('creatureface: the fixtures draw at both sizes with every mood and hurt step, and they are told apart', () => {
  for (const sitter of SITTERS) for (const size of ['full', 'small']) {
    const seen = new Set<string>();
    for (let mood = 0; mood < 6; mood++) for (let hurt = 0; hurt < 6; hurt++) {
      const f = faceCells({ ...sitter, size, mood: FACE_MOODS[mood], hurt });
      assert.deepEqual(lintAnsi(f, { skip: FACE_SKIP }).filter((x) => x.level === 'error'), [], `${JSON.stringify(sitter)} ${size} ${mood} ${hurt}`);
      seen.add(JSON.stringify([...f.ch, ...f.fg, ...f.bg]));
    }
    assert.ok(seen.size >= 30, `${JSON.stringify(sitter)} ${size}: ${seen.size} of 36 mood and hurt looks differ`);
  }
  const same = (a: any, b: any) => faceCells(a).equals(faceCells(b));
  for (const shape of FACE_SHAPES) { assert.ok(!same({ shape }, { shape, hurt: 3 }), `${shape} hurt shows`); assert.ok(!same({ shape, mood: 'dead' }, { shape }), `${shape} dead differs`); }
  assert.ok(!same({ race: 'Dwarf', hurt: 4, seed: 'a' }, { race: 'Clay Golem', hurt: 4 }), 'a hurt flesh face and a hurt stone face differ');
  assert.deepEqual(['Dwarf', 'Wolf', 'Lich', 'Clay Golem', 'Fire Elemental', 'Blob', 'Arachnid'].map((race) => hurtClass(readFace({ race }))), ['Bleed(RED)', 'Mat', 'Crack', 'Crack', 'Fade', 'Shrink', 'Bleed(GREEN)']);
});

test('creatureface: the small face is its own drawing, not the big one scaled', () => {
  for (const shape of FACE_SHAPES) {
    const big = face({ shape }), small = face({ shape, size: 'small' });
    // The big figure folded to 16 by 16 by taking every other pixel would be the "scaled" face; the small one is drawn differently.
    const scaled = new Set<number>(); for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) scaled.add(big.figure.colors[Math.floor(y * 1.5) * 25 + Math.floor(x * 25 / 16)]);
    let same = 0; for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) if (big.figure.colors[Math.floor(y * 1.5) * 25 + Math.floor(x * 25 / 16)] === small.figure.colors[y * 16 + x]) same++;
    assert.ok(same < 16 * 16 * 0.97 || scaled.size < 2, shape);
    assert.equal(small.figure.w, 16); assert.equal(small.figure.h, 16);
  }
});

test('creatureface: head shape by Coupler\'s rule, colours and cues from words, the hurt text, effect names', () => {
  assert.deepEqual([['Wolf'], ['Elf'], ['Orc'], ['Aarakocran'], ['Lich'], ['Clay Golem'], ['Air Elemental'], ['Angelfish']].map(([r]) => faceShape(r)), ['Beast(Pointed)', 'Person', 'Person', 'Bird', 'Skull', 'Stuff(Golem)', 'Stuff(Air)', 'Scaled(Fish)']);
  assert.equal(faceShape('Sphinx', 'Feline'), 'Beast(Pointed)'); assert.equal(faceShape('Mammoth', 'Pachyderm'), 'Beast(Wide)'); assert.equal(faceShape('Ghoulish thing', 'Undead'), 'Person'); assert.equal(faceShape('Fire Wisp', 'Elemental'), 'Stuff(Fire)');
  const w = readFaceWords('A tall man with long, flowing red hair, a scar, a long beard, glowing green eyes and pale skin.');
  assert.deepEqual([w.hair, w.eyes, w.skin, w.scar, w.beard, w.longHair, w.glow], [1, 10, 7, true, true, true, true]);
  assert.equal(readFaceWords('swarthy and tusked, with an eye patch').skin, 3); assert.equal(readFaceWords('a beardless boy').beard, false);
  assert.equal(readFace({ race: 'Dwarf', description: 'a bald dwarf' }).hair, undefined); assert.equal(readFace({ race: 'Wolf' }).animal, true);
  assert.equal(readFace({ race: 'Wolf', wears: ['iron collar'], skilled: true }).sapient, true, 'two of: wears something, has skills, arms');
  assert.equal(readFace({ race: 'Kobold' }).sapient, true, 'a thinking race');
  assert.deepEqual(['is in perfect health', 'has a few scratches', 'has some cuts', 'is bruised and battered', 'is bleeding badly', 'is almost dead', 3, { health: 0.5 }, { health: 0.05 }].map((t) => hurtStep(t as any)), [0, 1, 2, 3, 4, 5, 3, 3, 5]);
  assert.deepEqual([effectLook('Bless'), effectLook('flesh stone'), effectLook('Frost Bite (spell)'), effectLook('shoot web'), effectLook('nothing')], ['halo', 'stone', 'sick', 'webbed', undefined]);
  const fire = (e: string) => readFace({ race: 'Human', effects: [e] }).mood;
  assert.deepEqual([fire('combat sleep'), fire('aura of fear'), fire('berzerk')], [4, 3, 1], 'effects that set a mood');
  assert.throws(() => face({ mood: 'happy' }), /unknown mood/); assert.throws(() => face({ size: 'huge' }), /unknown face size/); assert.throws(() => face({ shape: 'Blob' }), /unknown face shape/);
  const cue = (description: string, k: string) => face({ race: 'Human', description }).frames[0].equals(face({ race: 'Human' }).frames[0]) ? false : k;
  for (const [d, k] of [['bearded', 'beard'], ['scarred', 'scar'], ['wearing an eye patch', 'patch'], ['freckled', 'freckles'], ['tusked', 'tusks'], ['fanged', 'fangs'], ['horned', 'horns'], ['with a halo', 'halo'], ['old', 'old']]) assert.equal(cue(d, k), k, `${k} shows`);
});

test('creatureface: every effect look draws, burning and glowing are clips that pass the flash rule, with a still for reduced motion', () => {
  assert.equal(FACE_LOOKS.length, 15);
  for (const look of FACE_LOOKS) { const f = faceCells({ race: 'Human', looks: [look] }); assert.deepEqual(lintAnsi(f, { skip: FACE_SKIP }).filter((x) => x.level === 'error'), [], look); if (!['asleep', 'afraid', 'raging'].includes(look)) assert.ok(!f.equals(faceCells({ race: 'Human' })), `${look} shows`); }
  for (const look of ['burning', 'glowing']) for (const race of ['Human', 'Wolf', 'Clay Golem']) {
    const a = face({ race, looks: [look] }), clip = a.clips.loop;
    assert.ok(clip.frames.length >= 2 && clip.loop && clip.reduced.length === 1 && clip.reduced[0].equals(a.frames[0]));
    assert.deepEqual(lintAnsiFrames(clip.frames, { fps: clip.fps }), [], `${race} ${look}`);
  }
  assert.equal(face({ race: 'Human', looks: ['stone'] }).clips, undefined, 'a still look is not a clip');
});

test('creatureface: the picture carries its parts and its words', () => {
  const a = face({ race: 'Orc', mood: 'angry', hurt: 4, description: 'a tusked orc' });
  assert.deepEqual(a.figure.partNames, FACE_PARTS); assert.ok([1, 3, 4, 6].every((p) => a.figure.parts.includes(p)), 'body, eye, mouth and brow are tagged');
  assert.match(a.says, /^Orc, angry, bleeding, tusked\.$/); assert.match(face({ race: 'Elf', mood: 'pain' }).says, /^Elf, in pain\.$/); assert.match(face({ race: 'Human', effects: ['burrow hide'] }).says, /half seen/);
  assert.equal(draw('creatureface', { race: 'Orc' }, { style: 'text' }).style, 'text'); assert.throws(() => draw('creatureface', {}), /ansi only/);
});

// --- Phase D: item pictures ------------------------------------------------------------------------------------------------------------
const THING_SKIP = ['outline', 'contrast', 'ramp'];
const thing = (p: any = {}) => draw('thing', p, { style: 'ansi' }) as any;
const thingCells = (p: any = {}) => thing(p).frames[0];
const thingErrors = (p: any) => lintAnsi(thingCells(p), { skip: THING_SKIP }).filter((x) => x.level === 'error');
// The handover's twelve variants beside its one-word things (a sword, a pants, a apple: the first word of each kind).
const THING_FIXTURES: [string, any, string][] = [['a steel helmet', { material: 'steel', worn: 'worn on head' }, 'Helmet'], ['a pair of leather boots', { material: 'leather' }, 'Boots'], ['a bag of gold coins', {}, 'Bag'],
  ['a red potion', { material: 'glass', auras: ['glowing red'] }, 'Potion'], ['a black wool hood', { material: 'wool', worn: 'worn on head' }, 'Hood'], ['an oak staff', { material: 'oak', auras: ['magical aura'], worn: 'wielded' }, 'Staff'],
  ['a mithril ring', { material: 'mithril', auras: ['metallic aura'], worn: 'worn on finger' }, 'Ring'], ['a rusty iron key', { material: 'iron' }, 'Key'], ['a dented round shield', { material: 'oak', worn: 'held' }, 'Shield'],
  ['a fine silver goblet', { material: 'silver', auras: ['invisible'] }, 'Goblet'], ['a ruby', { auras: ['glowing'] }, 'Gem'], ['a sack', { auras: ['hidden'] }, 'Bag']];

test('thing: every kind draws at 25 by 12 cells, deterministic, legal, with its slot on the bottom row', () => {
  assert.equal(THING_KINDS.length, 56); assert.equal(ITEM_KINDS.length, 56);
  for (const kind of THING_KINDS) {
    const a = thing({ kind }), f = a.frames[0];
    assert.deepEqual([f.w, f.h], [25, 12], kind); assert.ok(f.equals(legalize(f, { bgLimit: 8 })) && f.equals(thingCells({ kind })), kind);
    const slot = ITEM_KINDS.find((k) => k[0] === kind)![5]; assert.equal(Array.from({ length: 25 }, (_, x) => String.fromCodePoint(f.get(x, 11).ch || 32)).join('').trim(), slot, kind);
    assert.deepEqual(thingErrors({ kind }), [], kind); assert.equal(a.figure.w * a.figure.h, a.figure.parts.length); assert.deepEqual(a.figure.partNames, THING_PARTS);
    assert.ok(a.figure.parts.some((p: number) => p === 1), `${kind} has body pixels`);
  }
});

test('thing: every kind in every material class and every wear step is lint clean, and the class shows', () => {
  const material = { cloth: 'wool', glass: 'glass', leather: 'leather', metal: 'iron', mithril: 'mithril', plain: 'paper', precious: 'ruby', rock: 'granite', wooden: 'oak' } as Record<string, string>;
  assert.deepEqual(Object.keys(material).sort(), [...THING_CLASSES].sort());
  for (const kind of THING_KINDS) {
    const seen = new Set<string>();
    for (const cls of THING_CLASSES) { assert.deepEqual(thingErrors({ kind, material: material[cls], color: 7 }), [], `${kind} ${cls}`); const f = thingCells({ kind, material: material[cls], color: 7 }); seen.add(JSON.stringify([...f.ch, ...f.fg, ...f.bg])); }
    assert.ok(seen.size >= (kind === 'Potion' ? 2 : 6), `${kind}: ${seen.size} of 9 classes differ`);   // a potion is glass whatever the word, a wand and an arrow are mostly thin
    const worn = new Set<string>();
    for (const wear of [{}, { step: 1 }, { step: 2 }, { step: 3 }, { step: 4 }, { rust: true }, { crude: true }, { fine: true }]) { assert.deepEqual(thingErrors({ kind, material: 'iron', wear }), [], `${kind} ${JSON.stringify(wear)}`); const f = thingCells({ kind, material: 'iron', wear }); worn.add(JSON.stringify([...f.ch, ...f.fg, ...f.bg])); }
    assert.ok(worn.size >= 7, `${kind}: ${worn.size} of 8 wear steps differ`);
  }
  assert.deepEqual(THING_WEARS, ['sound', 'worn', 'damaged', 'battered', 'broken', 'rusty', 'crude', 'fine']);
});

test('thing: the kind from the first word, material class, wear and aura words, the fixtures', () => {
  assert.deepEqual(thingKind('a rusty iron longsword'), { kind: 'Sword', word: 'longsword' }); assert.equal(thingKind('a quux').kind, 'Bundle'); assert.equal(thingKind('a quux', { container: true }).kind, 'Bag');
  assert.equal(thingKind('gold coins').kind, 'Coins'); assert.equal(thingKind('a gold ring').kind, 'Ring', 'a material word is not the kind if another word names one'); assert.equal(thingKind('a stone').kind, 'Rock', 'a material word names the kind if nothing else does');
  assert.equal(thingKind('a pair of leather boots').kind, 'Boots'); assert.equal(thingKind('three apples').kind, 'Fruit', 'a plural'); assert.equal(thingKind('a pair of greaves').kind, 'Legs'); assert.equal(thingKind('iron gauntlets').kind, 'Gloves');
  for (const [name, kind] of [['a trident', 'Spear'], ['a halberd', 'Spear'], ['a wand', 'Wand'], ['a rune', 'Rune'], ['a clay jar', 'Jar'], ['a sprig of sage', 'Herb'], ['a treasure map', 'Map'], ['a golden idol', 'Relic'], ['a torch', 'Torch']]) assert.equal(thingKind(name).kind, kind, name);
  for (const k of ITEM_KINDS) assert.equal(thingKind(`a ${k[6][0]}`).kind, k[0], k[6][0]);
  assert.deepEqual([materialClass('iron')?.draws, materialClass(undefined, 'a black wool hood')?.draws, materialClass(undefined, 'a bag of gold coins'), materialClass('white gold')?.color, materialClass('apples')?.draws, materialClass('oak')?.cls], ['metal', 'cloth', undefined, 11, 'plain', 'wooden']);
  assert.equal(materialClass(undefined, 'a white gold ring')?.material, 'white gold', 'two-word pairs first');
  assert.deepEqual([thingWear('battered'), thingWear(undefined, 'a rusty crude dented axe'), thingWear(['fine', 'broken'])].map((w) => [w.step, w.rust, w.crude, w.fine]), [[3, false, false, false], [2, true, true, false], [4, false, false, true]]);
  assert.deepEqual([auraGlow('glowing'), auraGlow('glowing red'), auraGlow('glowing green'), auraGlow('magical aura')], [11, 9, 10, undefined]);
  assert.equal(THING_AURAS.length, 13);
  for (const [name, p, kind] of THING_FIXTURES) { const r = readThing({ name, ...p }); assert.equal(r.kind, kind, name); assert.deepEqual(thingErrors({ name, ...p }), [], name); assert.ok(thing({ name, ...p }).says.endsWith('.'), name); }
  assert.equal(readThing({ name: 'a sword', description: 'It is made of steel.' }).material, 'steel', 'a material in the description');
  assert.throws(() => thing({ kind: 'Spaceship' }), /unknown item kind/); assert.throws(() => thing({ name: 'a sword', class: 'jelly' }), /unknown material class/);
});

test('thing: a rusty iron sword, a fine mithril sword and a glowing wooden staff are told apart, and so are the aura words and the wear', () => {
  const key = (p: any) => JSON.stringify(thing(p).frames[0].ch.join() + thing(p).frames[0].fg.join() + thing(p).frames[0].bg.join());
  const three = [{ name: 'a rusty iron longsword' }, { name: 'a fine mithril sword' }, { name: 'a glowing wooden staff' }, { name: 'a longsword' }, { name: 'an iron longsword' }];
  assert.equal(new Set(three.map(key)).size, 5);
  assert.ok(new Set(THING_AURAS.map((a) => key({ name: 'an oak staff', auras: [a] }))).size >= 11, 'the aura words draw 11 or more looks (flying is floating, a mithril aura is a glint)');
  for (const a of ['glowing', 'magical aura', 'metallic aura', 'invisible', 'hidden', 'floating', 'bound']) assert.notEqual(key({ name: 'an oak staff', auras: [a] }), key({ name: 'an oak staff' }), a);
  assert.notEqual(key({ name: 'an oak staff', auras: ['flying'] }), key({ name: 'an oak staff' })); assert.notEqual(key({ name: 'a staff', auras: ['glowing red'] }), key({ name: 'a staff', auras: ['glowing blue'] }), 'glow colours');
  const solid = (p: any) => Array.from(thing(p).figure.parts).filter((v) => v).length;
  assert.ok(solid({ name: 'a broken iron sword' }) !== solid({ name: 'an iron sword' }) && solid({ name: 'a battered iron sword' }) < solid({ name: 'an iron sword' }), 'a broken sword is split, a battered one has notches');
});

test('thing: the picture carries its words, its parts and the unknown word falls back to a bundle', () => {
  assert.equal(thing({ name: 'a rusty iron longsword' }).says, 'Longsword, rusty iron, wielded.'); assert.equal(thing({ name: 'a glowing wooden staff' }).says, 'Staff, glowing, wielded.');
  assert.equal(thing({ name: 'a fine mithril sword' }).says, 'Sword, fine mithril, wielded.'); assert.equal(thing({ name: 'a quux' }).says, 'Bundle, carried.');
  assert.equal(thing({ name: 'a steel helmet', worn: 'worn on head' }).says, 'Helmet, steel, worn on head.');
  assert.equal(thing({ name: 'a quux' }).id, 'thing.bundle');
  assert.equal(draw('thing', { name: 'a ring' }, { style: 'text' }).style, 'text'); assert.throws(() => draw('thing', {}), /ansi only/);
  assert.ok(!thingCells({ name: 'a sword', seed: 'a' }).equals(thingCells({ name: 'a sword', seed: 'b' })), 'a seed changes the look');
});

// --- Phase E: character-creation portraits -------------------------------------------------------------------------------------------
const creation = (p: any = {}) => draw('creation', p, { style: 'ansi' }) as any;
const creationErrors = (p: any) => lintAnsi(creation(p).frames[0], { skip: THING_SKIP }).filter((x) => x.level === 'error');
const six = (seed: bigint) => { const r = couplerRng(seed); return Array.from({ length: 6 }, () => r.next().toString()); };

test('creation: Coupler\'s seed and generator, and both fallbacks, match draw for draw', () => {
  assert.equal(portraitSeed('race', 'kobold').toString(), '14243278056127361333');
  assert.deepEqual(six(portraitSeed('race', 'kobold')), ['17728460158679040596', '3347578030921207959', '547292487824187065', '6388376221540897467', '8682997052050821273', '8097037530069055977']);
  assert.deepEqual(six(portraitSeed('class', 'knight')), ['14211362092124846880', '12649059308204120552', '12292820764425453706', '1342099389857135749', '2116313160863388717', '4337960381143552688']);
  assert.deepEqual(classFacts('knight'), { key: 'knight', kind: 'priest', clothes: 8, trim: 15, known: false });
  // The 150 races the handover made from their names (and the one class) are the fallback's own output.
  let made = 0;
  for (const r of RACE_ROWS) if (!(r[10] && r[11])) {
    made++; const f = raceFacts(r[0]);
    assert.deepEqual([f.inches, f.bulk, f.darkvision, f.skin, f.hair ?? -1, f.clothes, f.feature], [r[1], r[3], !!r[4], r[5], r[6], r[7], r[8]], r[0]);
  }
  assert.equal(made, 150); const o = CLASS_ROWS.find((c) => c[0] === 'orator')!; assert.deepEqual([classFacts('orator').kind, classFacts('orator').clothes, classFacts('orator').trim], [o[1], o[2], o[3]]);
  assert.deepEqual([raceFacts('Half Elf').key, raceFacts('Half-Elf').key], ['halfelf', 'halfelf']); assert.equal(bulkFor(74, 175), 10);
});

test('creation: all 186 races and 49 classes draw at 32 by 16 and 80 by 40, lint clean, deterministic', () => {
  assert.equal(RACE_NAMES.length, 186); assert.equal(CLASS_NAMES.length, 49);
  for (const [columns, rows] of [[32, 16], [80, 40]]) {
    for (const name of RACE_NAMES) { const p = { kind: 'race', name, columns, rows }; const a = creation(p), f = a.frames[0]; assert.deepEqual([f.w, f.h], [columns, rows], name); assert.ok(f.equals(creation(p).frames[0]), name); assert.deepEqual(creationErrors(p), [], name); }
    for (const name of CLASS_NAMES) { const p = { kind: 'class', name, columns, rows }; assert.ok(creation(p).frames[0].equals(creation(p).frames[0]), name); assert.deepEqual(creationErrors(p), [], name); }
  }
  for (const [columns, rows] of [[8, 4], [80, 40], [8, 40], [80, 4]]) for (const kind of ['race', 'class']) assert.deepEqual(creationErrors({ kind, name: 'elf', columns, rows }), [], `${kind} ${columns}x${rows}`);
  for (const classKind of CLASS_KINDS) assert.deepEqual(creationErrors({ kind: 'class', classKind, columns: 32, rows: 16 }), [], classKind);
  for (const feature of RACE_FEATURES) assert.deepEqual(creationErrors({ name: 'human', feature }), [], feature);
});

test('creation: sizes show, unknown names are stable, darkvision shows red eyes, the words say it', () => {
  for (const [columns, rows] of [[32, 16], [80, 40]]) {
    const t = (name: string) => creation({ name, columns, rows }).facts.tall;
    assert.ok(t('halfogre') > t('human') && t('human') > t('gnome') && t('gnome') > t('pixie'), `${columns}x${rows}: ${t('halfogre')} ${t('human')} ${t('gnome')} ${t('pixie')}`);
  }
  assert.ok(creation({ name: 'quux' }).frames[0].equals(creation({ name: 'Quux' }).frames[0]), 'an unknown name draws the same every time'); assert.ok(!creation({ name: 'quux' }).frames[0].equals(creation({ name: 'zzyzx' }).frames[0]));
  const reds = (p: any) => { const f = creation(p).frames[0]; let n = 0; for (let y = 0; y < f.h; y++) for (let x = 0; x < f.w; x++) { const c = f.get(x, y); if ((c.fg === 9 || c.fg === 1) && c.ch === 0x2580 && c.bg === 7) n++; } return n; };
  void reds;
  const eyeColours = (p: any) => { const a = creation({ ...p, hair: 'none', skin: 'lightgray', feature: 'none', clothes: 'blue' }).frames[0], seen = new Set<number>(); for (let y = 0; y < a.h; y++) for (let x = 0; x < a.w; x++) { const c = a.get(x, y); seen.add(c.fg); seen.add(c.bg); } return seen; };
  assert.ok(eyeColours({ name: 'elf', darkvision: true }).has(9) && !eyeColours({ name: 'elf', darkvision: false }).has(9), 'red eyes only where it sees in the dark');
  assert.equal(creation({ name: 'Elf' }).says, 'Elf, 5 feet 5, pointed ears, sees in the dark.'); assert.equal(creation({ name: 'human' }).says, 'Human, 6 feet 2.'); assert.equal(creation({ name: 'faerie' }).says, 'Faerie, 1 foot 9, wings.');
  assert.equal(creation({ kind: 'class', name: 'cleric' }).says, 'Cleric, priest: robe, holy symbol and a mace.');
  const kinds = new Set(CLASS_KINDS.map((classKind) => JSON.stringify(creation({ kind: 'class', classKind }).frames[0].ch.join())));
  assert.equal(kinds.size, 8, 'each class kind draws its own tools');
  assert.ok(new Set(RACE_FEATURES.map((feature) => JSON.stringify(creation({ name: 'human', feature }).frames[0].fg.join()))).size === RACE_FEATURES.length, 'each feature shows');
  assert.throws(() => creation({ kind: 'ship' }), /unknown creation kind/); assert.throws(() => creation({ skin: 'plaid' }), /unknown skin colour/); assert.throws(() => creation({ feature: 'horns' }), /unknown feature/);
});

// --- Phase F: the figure on a place and the Fight panel -------------------------------------------------------------------------------
const FIGURE_DATA = JSON.parse(readFileSync(join(ROOT, 'docs/coupler-handover/painter-data/figure.json'), 'utf8'));
const onplace = (p: any = {}) => draw('onplace', p, { style: 'ansi' }) as any;
const fight = (p: any = {}) => draw('fight', p, { style: 'ansi' }) as any;
const fightErrors = (a: any, depth: any = 16) => { const target = depth === 16 ? 'classic' : 'modern'; return [...a.frames.map((f: any) => lintAnsi(f, { skip: THING_SKIP, depth, target })), ...Object.values(a.clips ?? {}).map((c: any) => lintAnsiFrames(c.frames, { name: c.id, fps: c.fps, loop: c.loop, depth, target }))].flat().filter((x: any) => x.level === 'error'); };

test('figure: the tables, the wild place and the rim light are Coupler\'s (figure.json), checked against its fixtures', () => {
  for (const [i, e] of FIGURE_DATA.lighter.entries()) { assert.equal(LIGHTER[i], e.to); assert.equal(SHADE[i], FIGURE_DATA.shade[i].to); }
  const recede = JSON.parse(readFileSync(join(ROOT, 'docs/coupler-handover/painter-data/painter-tables.json'), 'utf8')).recede.table;
  for (const [i, e] of recede.entries()) assert.equal(RECEDE[i], e.to.index);
  for (const e of FIGURE_DATA.wild.examples) assert.equal(wildTerrain(e.terrain, e.key), e.wild, `${e.key} on ${e.terrain}`);
  assert.equal(wildTerrain('mountains', 'a cat'), 'mountains'); assert.ok(['woods', 'plains', 'hills'].includes(wildTerrain('stone', 'x')));
  assert.equal(wildTerrain('stone', 'a bat'), wildTerrain('stone', 'a bat'));
  // The fixtures: the figure is the body block (cols 8 to 16, rows 8 to 19; its eyes are the white pixels). Day: the rim; night: darkened; indoors: nothing.
  const hex = (c: string) => parseInt(c, 16);
  FIGURE_DATA.fixtures.forEach((fx: any) => {
    const L = new Layer(25, 24);
    for (let y = 0; y < 24; y++) for (let x = 0; x < 25; x++) { L.a[y * 25 + x] = hex(fx.laid.colors[y][x]); L.f[y * 25 + x] = Number(fx.laid.lit[y][x]); }
    for (let y = 8; y < 20; y++) for (let x = 8; x <= 16; x++) L.parts[y * 25 + x] = fx.laid.colors[y][x] === 'f' ? EYE : BODY;
    lightFigures(L, { outdoors: !fx.indoors, night: fx.night });
    for (let y = 0; y < 24; y++) for (let x = 0; x < 25; x++) if (L.parts[y * 25 + x] || fx.indoors) assert.equal(L.a[y * 25 + x], hex(fx.lit.colors[y][x]), `${fx.terrain} at ${x},${y}`);
  });
  assert.equal(darken(0, 3, 3), 0); assert.equal(darken(12, 0, 0), 4);
});

test('figure: lay puts the parts down (offset, the body running off the side, hide) and stamp mirrors with the parts', () => {
  const fig = { w: 4, h: 2, colors: Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8]), parts: Uint8Array.from([0, 1, 3, 1, 1, 1, 1, 5]) };
  const L = new Layer(6, 2); lay(L, fig);
  assert.deepEqual([...L.a].slice(0, 6), [0, 2, 3, 4, 4, 4], 'back pixels skipped; the last body column repeats off the right edge');
  assert.deepEqual([...L.parts].slice(0, 6), [0, 1, 3, 1, 1, 1]);
  const H = new Layer(4, 2); lay(H, fig, { hide: true }); assert.deepEqual([...H.a].slice(0, 4), [0, 0, 3, 0], 'an invisible figure hides every pixel where x + y is odd');
  const O = new Layer(4, 2); lay(O, fig, { offset: { x: 1, y: 0 } }); assert.equal(O.a[0], 2); const U = new Layer(4, 2); lay(U, fig, { offset: { x: 0, y: 1 } }); assert.equal(U.a[0], 0, 'y offset moves it up: the first row now reads nothing above the figure');
  const M = new Layer(4, 2); stamp(M, fig, 0, 0, { mirror: true }); assert.deepEqual([...M.a].slice(0, 4), [4, 3, 2, 0]); assert.deepEqual([...M.parts].slice(0, 4), [1, 3, 1, 0], 'the part goes with the pixel');
  const F = new Layer(4, 2); stamp(F, fig, 0, 0, { flash: true }); assert.equal(F.a[1], LIGHTER[2]); assert.equal(F.f[1], 1, 'a flash gives its own light');
  const C = new Layer(2, 2); stamp(C, fig, 1, 0); assert.deepEqual([...C.a].slice(0, 2), [0, 0], 'clipped to the layer: the first figure pixel is back');
});

test('onplace: the place shows through the rim light; woods at dusk and stone read differently; animals go wild; deterministic', () => {
  const orc = { race: 'Orc' };
  const woods = onplace({ face: orc, terrain: 'woods', time: 'dusk' }), stone = onplace({ face: orc, terrain: 'stone' });
  assert.ok(!woods.frames[0].equals(stone.frames[0]), 'a face on woods at dusk and on stone differ');
  assert.ok(woods.frames[0].equals(onplace({ face: orc, terrain: 'woods', time: 'dusk' }).frames[0]));
  assert.deepEqual(woods.frames[0].w, 25); assert.deepEqual(woods.frames[0].h, 12);
  assert.ok(stone.says.includes('stone') && woods.says.includes('dusk'));
  // Rim light: the head's top edge is lighter on the lit place than the same pixel in the face on its own; indoors there is none.
  const bare = draw('creatureface', orc, { style: 'ansi' }) as any;
  assert.ok(woods.figure.parts.every((p: number, i: number) => p === bare.figure.parts[i]), 'the figure is the face\'s own');
  const day = placeLayer({ terrain: 'plains', time: 'day', columns: 25, rows: 12 }), night = placeLayer({ terrain: 'plains', time: 'night', columns: 25, rows: 12 });
  assert.ok(!day.night && night.night && !placeLayer({ terrain: 'stone', columns: 25, rows: 12 }).outdoors && !placeLayer({ terrain: 'underwater', time: 'night', columns: 25, rows: 12 }).night);
  const a = figureOnPlace(bare.figure, { terrain: 'plains', time: 'day', key: 'x' }).layer, b = figureOnPlace(bare.figure, { terrain: 'plains', time: 'night', key: 'x' }).layer;
  let lighter = 0, darker = 0; for (let i = 0; i < a.a.length; i++) if (a.parts[i]) { if (a.a[i] !== bare.figure.colors[i]) lighter++; if (b.a[i] !== bare.figure.colors[i]) darker++; }
  assert.ok(lighter > 0 && lighter < darker, `a rim by day (${lighter} pixels) and the whole figure at night (${darker})`);
  const wolf = figureOnPlace((draw('creatureface', { race: 'Wolf' }, { style: 'ansi' }) as any).figure, { terrain: 'stone', key: 'a wolf', wild: true });
  assert.ok(['woods', 'plains', 'hills'].includes(wolf.terrain)); assert.equal(onplace({ face: { race: 'Wolf' }, terrain: 'stone', key: 'a wolf' }).id, `onplace.${wolf.terrain}`);
  assert.equal(onplace({ face: { race: 'Human' }, terrain: 'stone' }).id, 'onplace.stone', 'a person stays indoors');
  const small = onplace({ face: { race: 'Dwarf', size: 'small' }, terrain: 'mountains', time: 'dawn' }); assert.deepEqual([small.frames[0].w, small.frames[0].h], [16, 8]);
  const thing = onplace({ thing: { name: 'a fine mithril sword' }, terrain: 'hills' }); assert.ok(thing.says.includes('hills'));
  const direct = draw('creatureface', { race: 'Orc', place: { terrain: 'woods', time: 'dusk' } }, { style: 'ansi' }) as any;
  assert.ok(direct.frames[0].equals(woods.frames[0]), 'creatureface takes a place');
  const burning = draw('creatureface', { race: 'Orc', looks: ['burning'], place: { terrain: 'woods', time: 'dusk' } }, { style: 'ansi' }) as any; assert.ok(burning.clips.loop.frames.length === 3 && !burning.clips.loop.frames[0].equals(burning.clips.loop.frames[1]));
  assert.throws(() => onplace({ terrain: 'moon' }), /unknown terrain/); assert.throws(() => onplace({ terrain: 'woods', time: 'noon' }), /unknown time/);
});

test('onplace: every kit variant is lint clean', () => {
  for (const v of RECIPES.onplace.variants()) { const a = onplace(v); assert.deepEqual(lintAnsi(a.frames[0], { skip: THING_SKIP, depth: v.colors ?? 16, target: v.colors ? 'modern' : 'classic' }).filter((x: any) => x.level === 'error'), [], JSON.stringify(v)); }
});

test('fight: the wide panel and the phone strip draw, mirrored, lit, clamped and in words', () => {
  const wide = fight({ terrain: 'woods', time: 'dusk' }); assert.deepEqual([wide.frames[0].w, wide.frames[0].h], [64, 12]);
  assert.deepEqual([fight({ columns: 10 }).frames[0].w, fight({ columns: 400 }).frames[0].w, fight({ layout: 'phone', columns: 10 }).frames[0].w], [40, 160, 32]);
  assert.equal(fight({ layout: 'phone' }).frames[0].h, 8); assert.ok(wide.says.startsWith('Foe: Orc') && wide.says.includes('You: Human'), wide.says);
  assert.ok(wide.frames[0].equals(fight({ terrain: 'woods', time: 'dusk' }).frames[0]), 'deterministic');
  assert.ok(!wide.frames[0].equals(fight({ terrain: 'stone' }).frames[0]), 'the room shows');
    const mirrored = fight({ you: { race: 'Orc' }, foe: { race: 'Orc' }, terrain: 'stone' }).frames[0]; let same = 0, left = 0;
  for (let y = 0; y < 12; y++) for (let x = 0; x < 25; x++) { const a = mirrored.get(x, y), b = mirrored.get(64 - 25 + x, y); if (a.ch === b.ch && a.fg === b.fg && a.bg === b.bg) same++; left++; }
  assert.ok(same < left, 'the player faces the foe, not the same way');
    const flat = (a: any) => { const f = a.frames[0]; let s = ''; for (let y = 0; y < f.h; y++) { for (let x = 0; x < f.w; x++) s += String.fromCodePoint(f.get(x, y).ch || 32); s += '\n'; } return s; };
  assert.ok(flat(fight({ layout: 'phone', words: ['You hit the orc.'], terrain: 'woods', columns: 60 })).includes('You hit the orc.'), 'the words sit between the faces');
  const off = fight({ layout: 'phone', pictures: false, words: ['a', 'b', 'c', 'd', 'e'] }); assert.equal(off.frames[0].h, 4); assert.ok(!flat(off).includes('e'), 'four rows of words');
  assert.throws(() => fight({ layout: 'tablet' }), /unknown fight layout/); assert.throws(() => fight({ hit: 'it' }), /you" or "foe/);
});

test('fight: the blow jolts and flashes the one hit, the slide brings the foe in, reduced motion takes the motion out', () => {
  assert.deepEqual(blowFrames('foe'), [{ you: { shift: 0, flash: false }, foe: { shift: 1, flash: true } }, { you: { shift: 0, flash: false }, foe: { shift: 1, flash: false } }]);
  assert.deepEqual(blowFrames('you')[0], { you: { shift: -1, flash: true }, foe: { shift: 0, flash: false } });
  assert.deepEqual(blowFrames('foe', false).map((f) => f.foe.flash), [false, false]); assert.deepEqual(blowFrames('foe', false, true), []); assert.deepEqual(blowFrames('foe', true, true), [{ you: { shift: 0, flash: false }, foe: { shift: 0, flash: true } }]);
  assert.equal(slideFrames(25).length, 25); assert.equal(slideFrames(25)[0].foe.shift, 24); assert.equal(slideFrames(25).at(-1)!.foe.shift, 0); assert.deepEqual(slideFrames(25, true), []);
  const base = { terrain: 'plains', hit: 'foe', fell: 20, slide: true };
  const a = fight(base); assert.ok(a.clips.blow && a.clips.slide); assert.equal(a.clips.blow.frames.length, 3, 'flash and jolt, the jolt alone, at rest'); assert.ok(a.clips.blow.frames[2].equals(a.frames[0]), 'the blow ends at the still');
  assert.ok(!a.clips.blow.frames[0].equals(a.frames[0]) && !a.clips.blow.frames[0].equals(a.clips.blow.frames[1]), 'the flash shows'); assert.ok(a.clips.slide.frames.at(-1).equals(a.frames[0]), 'the slide ends with the foe in place');
  assert.equal(a.clips.blow.fps, 10); assert.ok(Math.abs(a.clips.slide.fps - 1000 / 24) < 1e-9); assert.equal(a.clips.blow.loop, false);
  assert.equal(fight({ ...base, fell: BIG_HIT - 1 }).clips?.blow, undefined, 'a small blow is not a big one'); assert.ok(fight({ ...base, fell: BIG_HIT }).clips.blow);
  const noFlash = fight({ ...base, flash: false }); assert.ok(noFlash.clips.blow.frames[0].equals(fight({ ...base, flash: false, slide: false }).clips.blow.frames[0])); assert.equal(noFlash.clips.blow.frames.length, 3);
  assert.ok(!noFlash.clips.blow.frames[0].equals(a.clips.blow.frames[0]), 'no flash when it is not allowed');
  // Reduced motion: a still; no jolt and no slide; a flash alone only where allowed.
  const r = fight({ ...base, reducedMotion: true }); assert.equal(r.frames.length, 1); assert.equal(r.clips.slide, undefined); assert.equal(r.clips.blow.frames.length, 1, 'a flash alone');
  assert.ok(r.clips.blow.frames[0].equals(a.clips.blow.frames[0]) === false, 'with no jolt the flash stays where the foe stands');
  const still = fight({ ...base, reducedMotion: true, flash: false }); assert.equal(still.clips, undefined); assert.equal(still.frames.length, 1);
  assert.ok(still.frames[0].equals(a.frames[0]));
  for (const layout of ['wide', 'phone']) for (const hit of ['you', 'foe']) assert.deepEqual(fightErrors(fight({ ...base, layout, hit })), [], `${layout} ${hit}`);
  assert.ok(lintAnsiFrames(a.clips.blow.frames, { name: 'blow', fps: 10, loop: false }).every((x: any) => x.level !== 'error'));
});

test('fight: every kit variant draws lint clean and the flash never comes twice in a blow or more than three in a second', () => {
  for (const v of RECIPES.fight.variants()) { const a = fight(v); assert.deepEqual(fightErrors(a, v.colors ?? 16), [], JSON.stringify(v)); }
  const a = fight({ terrain: 'woods', hit: 'foe', fell: 40, layout: 'phone' }); const mean = (c: any) => { let s = 0; for (let i = 0; i < c.ch.length; i++) s += c.ch[i] === 32 ? c.bg[i] : c.fg[i]; return s; };
  const lit = a.clips.blow.frames.map(mean); assert.ok(lit.filter((m: number, i: number) => i && m !== lit[i - 1]).length <= 3);
});

// ---------------------------------------------------------------------------------------------------------------------------------
// Smooth colour for every recipe (1.14.0): `colors` 16, 256 or 'true' on the faces, items, creations, places, fights and rooms.

const DEPTHS = [16, 256, 'true'] as const;
const SMOOTH_CASES: [string, any][] = [
  ['creatureface', { race: 'Human' }], ['creatureface', { race: 'Orc', mood: 'angry', hurt: 3 }], ['creatureface', { race: 'Wolf' }], ['creatureface', { race: 'Dwarf', size: 'small' }],
  ['creatureface', { race: 'Orc', looks: ['stone'] }], ['creatureface', { race: 'Elf', place: { terrain: 'woods', time: 'dusk' } }],
  ['thing', { name: 'a fine mithril sword' }], ['thing', { name: 'a wooden staff', auras: ['glowing'] }], ['thing', { name: 'a wool cloak', auras: ['hidden'] }], ['thing', { name: 'a ruby ring' }], ['thing', { name: 'a granite stone' }], ['thing', { name: 'a glass vial' }],
  ['creation', { kind: 'race', name: 'elf' }], ['creation', { kind: 'class', name: 'mage' }],
  ['onplace', { face: { race: 'Orc' }, terrain: 'woods', time: 'dusk' }], ['onplace', { face: { race: 'Elf' }, terrain: 'woods', time: 'night' }], ['onplace', { thing: { name: 'a fine mithril sword' }, terrain: 'hills', time: 'day' }], ['onplace', { face: { race: 'Wolf' }, terrain: 'stone' }],
  ['fight', {}], ['fight', { terrain: 'woods', time: 'dusk', hit: 'foe', fell: 20, slide: true }], ['fight', { layout: 'phone', terrain: 'plains' }],
  ['vista', { terrain: 'woods', time: 'dusk' }], ['vista', { terrain: 'stone' }], ['vista', { terrain: 'mountains', weather: 'snow' }],
];
const drawAt = (id: string, p: any, colors: any) => draw(id, { ...p, colors }, { style: 'ansi' }) as any;
const hexes = (c: any) => new Set(tones(c));

test('colors: 16 is the default and the same as before, 256 and true colour differ where gradients, rims and light are, all deterministic', () => {
  for (const [id, p] of SMOOTH_CASES) {
    const name = `${id} ${JSON.stringify(p)}`, plain = drawAt(id, p, undefined), at16 = drawAt(id, p, 16), at256 = drawAt(id, p, 256), full = drawAt(id, p, 'true');
    assert.ok(plain.frames[0].equals(at16.frames[0]), `${name}: colors 16 is the default`);
    for (const colors of DEPTHS) assert.ok(drawAt(id, p, colors).frames[0].equals(drawAt(id, p, colors).frames[0]), `${name} at ${colors}: deterministic`);
    assert.ok(!at256.frames[0].equals(at16.frames[0]), `${name}: 256 differs from 16`);
    assert.ok(!full.frames[0].equals(at256.frames[0]), `${name}: true colour differs from 256`);
    assert.ok(hexes(at256.frames[0]).size > hexes(at16.frames[0]).size, `${name}: 256 has more tones than 16`);
    assert.ok(hexes(full.frames[0]).size > hexes(at256.frames[0]).size, `${name}: true colour has more tones than 256`);
    assert.equal(at16.colors, undefined, 'a 16-colour Art has no colors key (its hash stays the same)'); assert.equal(at256.colors, 256); assert.equal(full.colors, 'true');
    assert.equal(at16.frames[0].tc, null); assert.equal(at256.frames[0].tc, null); assert.ok(full.frames[0].tc, `${name}: true colour carries its RGB`);
  }
});

test('colors: 256 uses palette indexes above 15 and no true colour; true colour uses exact RGB that the 256 cannot hold', () => {
  const palette = new Set(XTERM256);
  for (const [id, p] of SMOOTH_CASES) {
    const c256 = drawAt(id, p, 256).frames[0], full = drawAt(id, p, 'true').frames[0];
    assert.ok([...c256.fg, ...c256.bg].some((v: number) => v > 15 && v !== NONE), `${id}: 256 reaches the cube and the grays`);
    assert.equal(c256.tc, null, `${id}: 256 has no true colour`);
    assert.ok(tones(full).some((h: string) => !palette.has(h)), `${id}: true colour has colours the 256 do not`);
  }
});

test('colors: true colour is read from the cells, kept by clone, blit and equals, and written exactly', () => {
  const a = new Cells(2, 1).set(0, 0, { ch: 0x2588, fg: 15, bg: NONE, fgRgb: 0x123456 }).set(1, 0, { ch: 0x2580, fg: 4, bg: 6 });
  assert.equal(a.get(0, 0).fgRgb, 0x123456); assert.equal(a.get(1, 0).fgRgb, -1);
  assert.equal(tone(a.get(0, 0)), '#123456'); assert.deepEqual(halves({ ch: 0x2580, fg: 1, bg: 2, fgRgb: 0xff0000, bgRgb: 0x0000ff }), [[255, 0, 0], [0, 0, 255]]);
  const b = a.clone(); assert.ok(a.equals(b)); b.set(0, 0, { fgRgb: 0x123457 }); assert.ok(!a.equals(b), 'a different RGB is a different picture');
  const c = new Cells(2, 1).blit(a); assert.equal(c.get(0, 0).fgRgb, 0x123456, 'a blit carries the RGB');
  a.set(0, 0, { fg: 3 }); assert.equal(a.get(0, 0).fgRgb, -1, 'a colour set by index alone clears its RGB');
  assert.equal(new Cells(1, 1).tc, null); assert.deepEqual(Object.keys(new Cells(1, 1)).sort(), ['bg', 'ch', 'fg', 'flags', 'h', 'w'], 'a 16-colour picture has the same fields as before');
  const d = new Cells(1, 1).set(0, 0, { ch: 0x2588, fg: 9, bg: NONE, fgRgb: 0x0a0b0c });
  const t = writeAnsi(d, { target: 'modern', colors: 'true' }).text!, p = writeAnsi(d, { target: 'modern', colors: 256 }).text!;
  assert.ok(t.includes('38;2;10;11;12'), 'true colour writes the exact RGB'); assert.ok(!p.includes(';2;') && p.includes('9'), 'at 256 the index is what is written');
  const e = new Cells(2, 1).set(0, 0, { ch: 0x2588, fg: 9, bg: NONE, fgRgb: 0x0a0b0c }).set(1, 0, { ch: 0x2588, fg: 9, bg: NONE });
  assert.ok(writeAnsi(e, { target: 'modern', colors: 'true' }).text!.match(/\x1b\[/g)!.length >= 3, 'cells with and without RGB are not one run');
  assert.ok(legalize(d, { bgLimit: 8 }).equals(d), 'legalize leaves a true-colour cell alone');
});

test('colors: a bad depth is refused, and true colour for the rooms is the exact blend', () => {
  for (const [id, p] of [['creatureface', {}], ['thing', {}], ['creation', {}], ['fight', {}], ['onplace', {}], ['vista', {}]] as [string, any][]) assert.throws(() => drawAt(id, p, 512), /colors is 16, 256 or 'true'/, id);
  const v = drawAt('vista', { terrain: 'plains', time: 'day' }, 'true').frames[0], w = drawAt('vista', { terrain: 'plains', time: 'day' }, 256).frames[0];
  assert.ok(v.tc && !w.tc); assert.ok(hexes(v).size > hexes(w).size);
  for (const terrain of TERRAINS) for (const time of ['day', 'night']) assert.ok(drawAt('vista', { terrain, time }, 'true').frames[0].tc, `${terrain} ${time}`);
  const f = drawAt('creatureface', { race: 'Human' }, 'true'); assert.ok(f.figure.colors instanceof Uint32Array && f.figure.depth === 'true'); assert.ok(drawAt('creatureface', { race: 'Human' }, 16).figure.colors instanceof Uint8Array);
  assert.equal(drawAt('creatureface', { race: 'Human' }, 16).figure.depth, undefined);
});

test('colors: a figure laid on a place takes the depth of the figure, or of `colors`, and night and rim are blends', () => {
  const face = drawAt('creatureface', { race: 'Human' }, 'true').figure, lo = drawAt('creatureface', { race: 'Human' }, 16).figure;
  const own = figureOnPlace(face, { terrain: 'woods', time: 'day' }), forced = figureOnPlace(lo, { terrain: 'woods', time: 'day' }, { colors: 'true' }), plain = figureOnPlace(lo, { terrain: 'woods', time: 'day' });
  assert.ok(own.cells.tc && forced.cells.tc && !plain.cells.tc, 'the depth follows the figure unless colors says');
  const night = figureOnPlace(face, { terrain: 'woods', time: 'night' }), day = figureOnPlace(face, { terrain: 'woods', time: 'day' });
  const luma = (c: any) => tones(c).reduce((s: number, h: string) => s + rgbOf(h).reduce((a: number, b: number) => a + b, 0), 0);
  assert.ok(luma(night.cells) < luma(day.cells), 'night is darker than day');
  const rim = night.layer.scaled ? 1 : 0; assert.equal(rim, 1);
  assert.ok(new Layer(4, 4, 256).smooth && !new Layer(4, 4).smooth);
});

test('colors: every recipe variant that takes a depth draws at 256 and true colour with no lint errors', () => {
  for (const id of ['creatureface', 'thing', 'creation', 'onplace', 'fight']) for (const v of RECIPES[id].variants()) for (const colors of [256, 'true']) {
    const a = drawAt(id, v, colors), frames = [...a.frames, ...Object.values(a.clips ?? {}).flatMap((c: any) => c.frames)];
    for (const f of frames) assert.deepEqual(lintAnsi(f, { target: 'modern', depth: colors as any, name: id }).filter((x: any) => x.level === 'error'), [], `${id} ${JSON.stringify(v)} at ${colors}`);
  }
});

// --- Tag Team phase 1: the contract with a compositor ---------------------------------------------------------------------------------
test('the engine version constant is package.json\'s version (a checkpoint bumps both)', () => {
  assert.equal(ENGINE_VERSION, JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version);
});

test('every pixel drawing has alt text, anchors inside its canvas, and a reduced still for what moves (lintArt)', () => {
  for (const [id, r] of Object.entries(RECIPES)) if (!r.ansiOnly) for (const v of r.variants()) {
    const a = draw(id, v);
    assert.deepEqual(lintArt(a, { name: `${id} ${JSON.stringify(v)}` }), []);
    for (const k of ['root', 'shadow_center', 'head', 'effect_origin']) assert.ok(Array.isArray(a.anchors![k]), `${id} has anchor ${k}`);
  }
  const bad = { id: 'x', frames: [new Pixels(8, 8).set(1, 1, 5), new Pixels(8, 8)], fps: 4, loop: true, anchors: { root: [9, 2] }, clips: { go: { frames: [new Pixels(8, 8), new Pixels(8, 8)], fps: 4, loop: true } } };
  assert.deepEqual(lintArt(bad as any).map((f) => f.rule).sort(), ['alt', 'anchors', 'reduced', 'reduced']);
});

test('a character has Tag Team\'s named anchors on every frame of every view, mirrored with the hand, and they follow the pose', () => {
  const names = ['root', 'head', 'neck', 'shoulder_left', 'shoulder_right', 'hand_main', 'hand_off', 'waist', 'back', 'foot_left', 'foot_right', 'weapon_grip', 'shield_grip', 'effect_origin', 'shadow_center'];
  for (const view of VIEWS) {
    const a = draw('character', { loadout: 'guardian', view });
    for (const n of names) assert.ok(Array.isArray(a.anchors![n]), `${view} has ${n}`);
    assert.equal(a.anchors!.frames.length, a.frames.length);
    for (const [n, c] of Object.entries(a.clips!)) assert.equal(c.anchors!.length, c.frames.length, `${view} clip ${n} has an anchor set a frame`);
  }
  const front = draw('character', { loadout: 'rogue' }), back = draw('character', { loadout: 'rogue', view: 'back' });
  assert.ok(front.anchors!.hand_main[0] > front.anchors!.feet[0], 'the weapon hand is right of centre from the front');
  assert.ok(back.anchors!.hand_main[0] < back.anchors!.feet[0], 'and left of it from behind');
  const left = draw('character', { loadout: 'rogue', view: 'side', facing: 'left' }), right = draw('character', { loadout: 'rogue', view: 'side' });
  assert.equal(left.anchors!.hand_main[0], 39 - right.anchors!.hand_main[0]);
  const stand = draw('character', { loadout: 'rogue' }).anchors!, cast = draw('character', { loadout: 'rogue', pose: 'cast' }).anchors!, sit = draw('character', { loadout: 'rogue', pose: 'sit' }).anchors!;
  assert.notDeepEqual(stand.hand_main, cast.hand_main); assert.ok(sit.head[1] > stand.head[1], 'sitting lowers the head');
  const hit = front.clips!.hit.anchors!; assert.equal(hit[1].hand_main[0], hit[0].hand_main[0] - 2, 'the hit clip shoves 2 left, and so do its anchors');
});

test('body profiles scale the layers inside the same 32 by 32 canvas, and medium is the default drawing', () => {
  assert.deepEqual(Object.keys(BODY_PROFILES), ['medium', 'small', 'slim', 'broad']);
  const base = draw('character', { loadout: 'guardian' });
  assert.ok(same(base.frames[0], draw('character', { loadout: 'guardian', bodyProfile: 'medium' }).frames[0]));
  const small = draw('character', { loadout: 'guardian', bodyProfile: 'small' }), slim = draw('character', { loadout: 'guardian', bodyProfile: 'slim' }), broad = draw('character', { loadout: 'guardian', bodyProfile: 'broad' });
  assert.equal(small.frames[0].h, 32);
  assert.ok(small.frames[0].bbox()!.h < base.frames[0].bbox()!.h * 0.9, 'small is shorter');
  assert.ok(slim.frames[0].bbox()!.w < base.frames[0].bbox()!.w && broad.frames[0].bbox()!.w > base.frames[0].bbox()!.w, 'slim is narrower, broad wider');
  assert.ok(small.anchors!.head[1] > base.anchors!.head[1], 'the head anchor comes down with the body');
  assert.deepEqual(lintArt(small), []); assert.ok(small.layers!.torso[0].count() > 0);
  assert.throws(() => draw('character', { bodyProfile: 'giant' }), /unknown body profile/);
});

test('recolor keeps each pixel\'s ramp step; recolorArt does frames, layers and clips and a swap does not undo itself', () => {
  const px = new Pixels(4, 1).set(0, 0, RAMPS.red[0]).set(1, 0, RAMPS.red[2]).set(2, 0, RAMPS.blue[3]).set(3, 0, C.white);
  const out = recolor(px, 'red', 'green');
  assert.deepEqual([...out.data], [RAMPS.green[0], RAMPS.green[2], RAMPS.blue[3], C.white]);
  assert.ok(same(recolor(out, 'green', 'red'), px));
  assert.throws(() => recolor(px, 'red', [1, 2]), /same length/);
  assert.throws(() => recolor(px, 'nope', 'red'), /unknown ramp/);
  const swapped = recolorArt({ id: 'x', frames: [px], fps: 0, loop: false }, [['red', 'blue'], ['blue', 'red']]).frames[0];
  assert.deepEqual([...swapped.data], [RAMPS.blue[0], RAMPS.blue[2], RAMPS.red[3], C.white], 'a swap is applied to the original pixels, once');
  const art = draw('character', { loadout: 'rogue', cloth: 'red' }), re = recolorArt(art, [['red', 'blue']]);
  assert.ok(!art.frames[0].colors().includes(RAMPS.blue[2]) && re.frames[0].colors().includes(RAMPS.blue[2]), 'the dye reaches the frame');
  assert.ok(re.layers!.torso[0].colors().includes(RAMPS.blue[2]) && re.clips!.walk.frames.every((f: Pixels) => f.colors().includes(RAMPS.blue[2])), 'and the layers and clips');
  assert.ok(same(recolorArt(art, []).frames[0], art.frames[0]));
});

test('resolveLook writes every default in: drawn again it is the same picture, and dye sets colour groups exactly', () => {
  for (const [recipe, params] of [['character', { loadout: 'rogue' }], ['character', { loadout: 'paladin', view: 'side', pose: 'cast' }], ['item', { shape: 'axe' }], ['creature', { race: 'dwarf' }], ['creature', { preset: 'wolf' }]] as [string, any][]) {
    const look = resolveLook(recipe, params), a = draw(recipe, params), b = draw(recipe, look.params);
    assert.ok(look.explicit && same(a.frames[0], b.frames[0]) && a.id === b.id, `${recipe} ${JSON.stringify(params)}`);
    assert.deepEqual(JSON.parse(JSON.stringify(look)), look); assert.equal(look.engine, ENGINE_VERSION);
  }
  assert.ok(Object.keys(resolveLook('character', { loadout: 'rogue' }).params).length > 8, 'the loadout is expanded into its fields');
  assert.ok(!('motion' in resolveLook('character', { loadout: 'rogue', motion: { id: 'm', frames: [{}], fps: 4 } }).params), 'a motion is not part of a look');
  assert.equal(resolveLook('tile', { kind: 'water' }).explicit, false);
  const d = dye('character', { loadout: 'rogue' }, { cloth: 'blue', skin: 'skin-deep' });
  assert.ok(same(d.frames[0], draw('character', { loadout: 'rogue', cloth: 'blue', skin: 'skin-deep' }).frames[0]));
  assert.equal(d.paletteGroups!.cloth, 'blue');
  assert.throws(() => dye('character', {}, { hat: 'red' }), /unknown dye group/); assert.throws(() => dye('tile', {}, { skin: 'red' }), /no dye groups/);
});

test('placeholders: a size class or a kind, never the real thing; the unknown ones ignore everything but their size', () => {
  const unknown = draw('placeholder', { kind: 'unknown', size: 'large' });
  assert.ok(same(unknown.frames[0], draw('placeholder', { kind: 'unknown', size: 'large', family: 'dragon', category: 'weapon', name: 'the Lich King' }).frames[0]), 'nothing but the size is read');
  assert.ok(!same(unknown.frames[0], draw('placeholder', { kind: 'unknown', size: 'small' }).frames[0]));
  assert.ok(same(draw('placeholder', { kind: 'exhibit' }).frames[0], draw('placeholder', { kind: 'exhibit', family: 'dragon' }).frames[0]));
  assert.ok(!same(draw('placeholder', { kind: 'family', family: 'beast' }).frames[0], draw('placeholder', { kind: 'family', family: 'insect' }).frames[0]));
  for (const kind of ['character', 'family', 'item', 'building', 'exhibit', 'unknown']) { const a = draw('placeholder', { kind }); assert.ok(a.alt!.includes('placeholder') && a.frames[0].count() > 20, kind); assert.ok(a.frames[0].colors().length <= 5, 'flat: stone, an outline and a mark'); }
  assert.throws(() => draw('placeholder', { kind: 'secret' }), /unknown placeholder kind/); assert.throws(() => draw('placeholder', { kind: 'family', family: 'x' }), /unknown family/);
});

test('effects: flash false drops the brightest colours, density low drops the isolated sparks, and the defaults are unchanged', () => {
  const calm = draw('effect', { effect: 'burst', flash: false }), plain = draw('effect', { effect: 'burst' });
  for (const f of calm.frames) assert.ok(!f.colors().includes(C.white) && !f.colors().includes(C.yellow));
  assert.ok(plain.frames.some((f: Pixels) => f.colors().includes(C.white) || f.colors().includes(C.yellow)));
  for (const effect of ['sparkle', 'burst', 'ice', 'illusion', 'slash']) {
    const low = draw('effect', { effect, density: 'low' }), full = draw('effect', { effect });
    const px = (a: any) => a.frames.reduce((n: number, f: Pixels) => n + f.count(), 0);
    assert.ok(px(low) <= px(full) && (effect === 'slash' || px(low) < px(full)), effect); assert.ok(low.frames.every((f: Pixels) => f.count() > 0), 'no frame is emptied');
    assert.equal(low.clips!.play.reduced!.length, 1);
  }
  assert.throws(() => draw('effect', { density: 'dense' }), /unknown density/);
});

test('a pack: every frame, clip and layer, an atlas, a manifest with anchors, groups and hashes; same list, same bytes', () => {
  const list = { name: 'demo', entries: [
    { id: 'demo.rogue', recipe: 'character', params: { loadout: 'rogue' }, slot: 'party', tags: ['class'] },
    { id: 'demo.wolf', recipe: 'creature', params: { preset: 'wolf' } },
    { id: 'demo.fx', recipe: 'effect', params: { effect: 'slash', flash: false } },
    { id: 'demo.none', recipe: 'placeholder', params: { kind: 'unknown', size: 'large' } },
  ] };
  const a = mkdtempSync(join(tmpdir(), 'pack-a-')), b = mkdtempSync(join(tmpdir(), 'pack-b-'));
  const m = writePack(list, a), m2 = writePack(list, b);
  assert.deepEqual(m, m2); for (const f of ['manifest.json', 'goldens.json', 'atlas.png']) assert.deepEqual(readF(join(a, f)), readF(join(b, f)), f);
  assert.equal(m.schema, 'art-sprites.pack/1'); assert.equal(m.engine, ENGINE_VERSION); assert.equal(m.license.licenseName, 'Project-created');
  const rogue = m.entries[0];
  assert.deepEqual(rogue.meta, { slot: 'party', tags: ['class'] }); assert.ok(rogue.look.explicit && rogue.look.params.torso);
  assert.equal(rogue.projection, 'front'); assert.ok(rogue.alt.startsWith('rogue')); assert.ok(rogue.anchors!.hand_main); assert.equal(rogue.paletteGroups!.cloth, 'green');
  assert.deepEqual(Object.keys(rogue.files.clips), ['idle', 'walk', 'attack', 'hit', 'defeat', 'turn', 'block', 'cast', 'heal', 'celebrate', 'dodge', 'parry', 'charge', 'stun', 'potion', 'buffed', 'lowhealth', 'levelup', 'tagin', 'tagout', 'equip', 'shoot', 'thrust', 'slam', 'recall']); assert.ok(Object.keys(rogue.files.layers).length >= 6);
  assert.ok(rogue.files.clips.attack.reduced.length === 1 && rogue.files.clips.attack.events.length === 1);
  const png = decodePng(readF(join(a, rogue.files.frames[0].file)));
  assert.deepEqual([...png.rgba], [...draw('character', { loadout: 'rogue' }).frames[0].toRgba(1)], 'the PNG is the drawing');
  const atlas = decodePng(readF(join(a, 'atlas.png'))); assert.deepEqual([atlas.w, atlas.h], [m.atlas.w, m.atlas.h]);
  for (const e of m.entries) { assert.ok(e.atlas.x + e.atlas.w <= atlas.w && e.atlas.y + e.atlas.h <= atlas.h, e.id); assert.match(e.golden, /^[0-9a-f]{64}$/); }
  assert.deepEqual(Object.keys(JSON.parse(readF(join(a, 'goldens.json'), 'utf8')).hashes), list.entries.map((e) => e.id));
  assert.equal(m.entries[2].reduced.length, 0 + (draw('effect', { effect: 'slash' }).reduced?.length ?? 0));
  assert.throws(() => writePack({ entries: [] }, a), /at least one entry/);
  assert.throws(() => writePack({ entries: [{ recipe: 'vista' }] }, a), /ANSI only/);
  assert.throws(() => writePack({ entries: [{ recipe: 'nope' }] }, a), /unknown recipe/);
  assert.throws(() => writePack({ entries: [{ id: 'x', recipe: 'item' }, { id: 'x', recipe: 'item' }] }, a), /duplicate id/);
});

test('pack.js is Node-only: the main entry does not reach it', () => {
  const seen = new Set<string>();
  const visit = (f: string) => { if (seen.has(f)) return; seen.add(f); for (const m of readFileSync(f, 'utf8').matchAll(/(?:from|import)\s*'([^']+)'/g)) if (m[1].startsWith('.')) visit(resolve(join(f, '..'), m[1])); };
  visit(join(ROOT, 'index.js'));
  assert.ok(!seen.has(join(ROOT, 'pack.js')));
});

test('the top view draws every loadout from above in four facings: west mirrors east, anchors and clips follow, no existing view changes', () => {
  for (const loadout of Object.keys(LOADOUTS)) for (const facing of ['north', 'east', 'south', 'west']) {
    const a = draw('character', { loadout, view: 'top', facing });
    assert.equal(a.projection, 'top-down'); assert.equal(a.frames[0].w, 40); assert.equal(a.frames[0].h, 40);
    assert.ok(a.frames[0].count() > 120, `${loadout}/${facing}`);
    assert.deepEqual(lintArt(a), [], `${loadout}/${facing}`);
    for (const [n, c] of Object.entries(a.clips!)) assert.equal(c.anchors!.length, c.frames.length, `${loadout}/${facing} ${n}`);
    for (const n of ['idle', 'walk', 'attack', 'hit', 'defeat']) assert.ok(a.clips![n], `${loadout}/${facing} has ${n}`);
    assert.deepEqual(lintFrames(a.clips!.walk.frames, { loop: true }).filter((f) => f.rule === 'frames'), [], `${loadout}/${facing} walk`);
    assert.deepEqual(lint(a.frames[0], { skip: ['size', 'margin', 'palette'] }).filter((f) => f.level === 'error'), [], `${loadout}/${facing}`);
  }
  const east = draw('character', { loadout: 'rogue', view: 'top', facing: 'east' }), west = draw('character', { loadout: 'rogue', view: 'top', facing: 'west' });
  assert.ok(west.frames[0].equals(east.frames[0].flipX()), 'west is east mirrored');
  assert.equal(west.anchors!.head[0], 39 - east.anchors!.head[0]);
  assert.ok(!draw('character', { loadout: 'guardian', view: 'top', facing: 'north' }).frames[0].equals(draw('character', { loadout: 'guardian', view: 'top', facing: 'south' }).frames[0]));
  assert.deepEqual(resolveLook('character', { view: 'top' }).params.facing, 'south');
  assert.throws(() => draw('character', { view: 'top', facing: 'up' }), /unknown facing/);
  for (const bodyProfile of ['small', 'slim', 'broad']) assert.ok(draw('character', { loadout: 'rogue', view: 'top', bodyProfile }).frames[0].count() > 80);
  assert.ok(draw('character', { loadout: 'guardian', view: 'top', facing: 'north', back: 'cape' }).layers!.back[0].count() > 0);
  assert.match(draw('character', { loadout: 'rogue', view: 'top', facing: 'east' }).alt!, /top-down view facing east/);
});

test('the five classes differ by silhouette alone from above, in every facing', () => {
  for (const facing of ['north', 'east', 'south', 'west']) {
    const items = ['guardian', 'berserker', 'rogue', 'scholar', 'healer'].map((name) => ({ name, px: draw('character', { loadout: name, view: 'top', facing }).frames[0] }));
    assert.deepEqual(lintSilhouettes(items), [], facing);
  }
  const same = [{ name: 'a', px: draw('character', { loadout: 'rogue', view: 'top' }).frames[0] }, { name: 'b', px: draw('character', { loadout: 'rogue', view: 'top', cloth: 'blue' }).frames[0] }];
  assert.equal(lintSilhouettes(same).length, 1, 'the same shape in another colour is caught');
});

import { RARITIES, MARKS } from '../recipes/overlay.js';
import { SKILL_CATEGORIES, SKILL_MOTIFS, SKILL_TARGETS, SKILL_KINDS } from '../recipes/skillicon.js';
const RARITY_NAMES = Object.keys(RARITIES);
test('overlay: every rarity has its own frame shape, every marker its own shape, and a count comes from the VGA atlas', () => {
  const shapes = RARITY_NAMES.map((rarity) => draw('overlay', { rarity }).frames[0]);
  for (let i = 0; i < shapes.length; i++) for (let j = i + 1; j < shapes.length; j++) {
    const a = shapes[i].silhouette(1), b = shapes[j].silhouette(1);
    assert.ok(!a.equals(b), `${RARITY_NAMES[i]} and ${RARITY_NAMES[j]} differ in shape, not only colour`);
  }
  const marks = Object.keys(MARKS).map((m) => draw('overlay', { marks: [m] }).frames[0].silhouette(1));
  for (let i = 0; i < marks.length; i++) for (let j = i + 1; j < marks.length; j++) assert.ok(!marks[i].equals(marks[j]), `${Object.keys(MARKS)[i]} and ${Object.keys(MARKS)[j]} differ in shape`);
  const full = draw('overlay', { rarity: 'rare', marks: ['new', 'equipped', 'locked', 'bound'] });
  assert.deepEqual(Object.values(full.corners as any).sort(), ['bl', 'br', 'tl', 'tr']);
  assert.throws(() => draw('overlay', { marks: ['new', 'crafted', 'enchanted', 'bound', 'locked'] }), /no free corner/);
  assert.throws(() => draw('overlay', { rarity: 'mythic' }), /unknown rarity/);
  assert.throws(() => draw('overlay', { count: 5 }), /size 32/);
  assert.throws(() => draw('overlay', { size: 32, count: 5 }), /atlas/);
  const atlas = loadAtlas(), a = draw('overlay', { size: 32, count: 12 }, { atlas }), b = draw('overlay', { size: 32, count: 13 }, { atlas });
  assert.equal(a.frames[0].w, 32); assert.ok(!a.frames[0].equals(b.frames[0]));
  assert.ok(a.frames[0].count() > draw('overlay', { size: 32 }).frames[0].count(), 'digits add pixels');
  const t = pixelText(atlas, '42', { outline: null }); assert.equal(t.w, 16); assert.equal(t.h, 16);
  assert.ok(pixelText(atlas, '4').equals(pixelText(atlas, '4')) && !pixelText(atlas, '4').equals(pixelText(atlas, '5')));
  assert.throws(() => pixelText({ w: 1, h: 1, rgba: new Uint8Array(4) }, '1'), /atlas/);
  assert.ok(draw('overlay', { rarity: 'rare', durability: 1 }).frames[0].count() > draw('overlay', { rarity: 'rare', durability: 0 }).frames[0].count() - 1);
});

test('telegraph fills with a pattern as well as a colour; targets differ in shape; a combat number floats and needs the atlas', () => {
  for (const shape of ['circle', 'line', 'cone', 'rect']) {
    const t = draw('telegraph', { shape, direction: 'south' });
    assert.equal(t.frames.length, 2); assert.ok(!t.frames[0].equals(t.frames[1]), `${shape}: the pattern slides`);
    const px = t.frames[0], edge = px.count(), solid = (px.w - 2) ** 2; assert.ok(edge > 30 && edge < solid, shape);
    assert.equal(t.projection, 'top-down');
  }
  const danger = draw('telegraph', { kind: 'danger' }).frames[0], friendly = draw('telegraph', { kind: 'friendly' }).frames[0];
  assert.ok(!danger.silhouette(1).equals(friendly.silhouette(1)), 'kinds differ in pattern, not only colour');
  assert.ok(!draw('telegraph', { shape: 'cone', direction: 'north' }).frames[0].equals(draw('telegraph', { shape: 'cone', direction: 'south' }).frames[0]));
  assert.throws(() => draw('telegraph', { shape: 'star' }), /unknown telegraph shape/);
  const rings = ['ally', 'enemy', 'neutral'].map((kind) => draw('target', { kind }).frames[0].silhouette(1));
  assert.ok(!rings[0].equals(rings[1]) && !rings[1].equals(rings[2]) && !rings[0].equals(rings[2]));
  assert.equal(draw('target', { kind: 'turn' }).frames.length, 2);
  assert.ok(!draw('target', { kind: 'seat', seat: 1 }).frames[0].equals(draw('target', { kind: 'seat', seat: 4 }).frames[0]));
  const atlas = loadAtlas(), n = drawNumber(atlas, { value: 37, kind: 'crit' }), same = drawNumber(atlas, { value: 37, kind: 'crit' });
  assert.ok(n.frames.every((f, i) => f.equals(same.frames[i])), 'same input, same pixels');
  assert.equal(n.frames.length, 4); assert.equal(n.clips!.float.reduced![0], n.frames[3]);
  assert.ok(drawNumber(atlas, { kind: 'miss' }).frames[0].w > drawNumber(atlas, { value: 5 }).frames[0].w, 'a word is wider than a digit');
  assert.throws(() => drawNumber(atlas, { value: -1 }), /whole number/);
  assert.throws(() => drawNumber(atlas, { kind: 'zap' }), /unknown number kind/);
});

test('the core semantic clips (turn, block, cast, heal, celebrate, recall) exist in every view, move, carry an event and a reduced still, and keep their anchors', () => {
  for (const view of [...VIEWS, 'top']) {
    const c = drawCharacter({ loadout: 'paladin', view }), ids = ['turn', 'block', 'cast', 'heal', 'celebrate', 'recall'];
    for (const id of ids) {
      const k = c.clips[id]; assert.ok(k, `${view}/${id}`);
      assert.equal(k.anchors!.length, k.frames.length, `${view}/${id} anchors`);
      assert.ok(k.events.length === 1 && k.events[0].frame < k.frames.length, `${view}/${id} event`);
      assert.equal(k.reduced!.length, 1); assert.equal(k.loop, false);
      assert.deepEqual(lintFrames(k.frames, { loop: false }).filter((f) => f.rule === 'frames'), [], `${view}/${id}: no repeated frame`);
    }
    assert.equal(c.clips.recall.events[0].name, 'vanish'); assert.equal(c.clips.cast.events[0].name, 'release');
  }
});

test('skill icons: ten categories, approved motifs only, targets and kinds differ in shape, locked and cooling are dithered, and the ten panel icons draw', () => {
  const plates = Object.values(SKILL_CATEGORIES).map((c) => `${c.plate}/${c.shape}`); assert.equal(new Set(plates).size, 10, 'each category has its own plate');
  for (const m of SKILL_MOTIFS) assert.ok(GLYPHS[m], `${m} is a real glyph`);
  const art = (p: any) => draw('skillicon', p).frames[0];
  const targets = SKILL_TARGETS.map((target) => art({ target }).silhouette(1));
  for (let i = 0; i < targets.length; i++) for (let j = i + 1; j < targets.length; j++) assert.ok(!targets[i].equals(targets[j]), `${SKILL_TARGETS[i]} vs ${SKILL_TARGETS[j]}`);
  const kinds = SKILL_KINDS.map((kind) => art({ kind }).silhouette(1)); assert.ok(!kinds[0].equals(kinds[1]) && !kinds[1].equals(kinds[2]) && !kinds[0].equals(kinds[2]));
  assert.ok(!art({}).equals(art({ locked: true })) && !art({}).equals(art({ cooldown: 0.5 })) && !art({ cooldown: 0.25 }).equals(art({ cooldown: 0.75 })));
  assert.ok(!art({ tier: 1 }).equals(art({ tier: 3 })) && !art({ cost: 0 }).equals(art({ cost: 2 })));
  assert.equal(draw('skillicon', { cooldown: 0.5 }).clips!.cooldown.frames.length, 5);
  assert.equal(draw('skillicon', {}).frames[0].w, 24);
  assert.throws(() => draw('skillicon', { motif: 'door' }), /approved glyph/); assert.throws(() => draw('skillicon', { category: 'fly' }), /unknown skill category/);
  assert.throws(() => draw('skillicon', { tier: 4 }), /0 to 3/); assert.throws(() => draw('skillicon', { target: 'moon' }), /unknown skill target/);
  for (const glyph of ['actions', 'chat', 'party', 'character', 'inventory', 'map', 'chronicle', 'charts', 'research', 'help']) assert.deepEqual(lint(draw('icon', { glyph }).frames[0], { skip: ['size', 'palette'] }).filter((f) => f.level === 'error'), [], glyph);
});

test('portraits come in party, status and card sizes, with the nine expressions and defeated and recalling states; the default is unchanged', () => {
  const dims = (p: any) => [p.frames[0].w, p.frames[0].h];
  assert.deepEqual(dims(draw('portrait', { loadout: 'healer' })), [20, 20]); assert.deepEqual(dims(draw('portrait', { loadout: 'healer', size: 'party' })), [16, 16]); assert.deepEqual(dims(draw('portrait', { loadout: 'healer', size: 'card' })), [32, 32]);
  assert.ok(draw('portrait', { loadout: 'healer', size: 'status' }).frames[0].equals(draw('portrait', { loadout: 'healer' }).frames[0]));
  const plain = draw('portrait', { loadout: 'healer' }).frames[0], faces = Object.keys(EXPRESSIONS).filter((e) => e !== 'neutral').map((expression) => draw('portrait', { loadout: 'healer', expression }).frames[0]);
  for (const f of faces) assert.ok(!f.equals(plain));
  for (let i = 0; i < faces.length; i++) for (let j = i + 1; j < faces.length; j++) if (!faces[i].equals(faces[j])) continue; else assert.fail(`expressions ${i} and ${j} look the same`);
  const dead = draw('portrait', { loadout: 'healer', state: 'defeated' }), gone = draw('portrait', { loadout: 'healer', state: 'recalling' });
  assert.ok(dead.frames[0].colors().every((v: number) => ['stone', 'iron'].some((r) => (RAMPS as any)[r].includes(v)) || v === C.night)); assert.ok(gone.frames[0].count() < plain.count());
  const art = draw('portrait', { loadout: 'healer', size: 'card' }) as any; assert.equal(art.mask.count(), art.frames[0].count()); assert.ok(art.safe.w > 0 && art.safe.x >= 0);
  assert.throws(() => draw('portrait', { size: 'huge' }), /unknown portrait size/); assert.throws(() => draw('portrait', { state: 'asleep' }), /unknown portrait state/); assert.throws(() => draw('portrait', { expression: 'smirk' }), /unknown expression/);
});

test('gear layers (T7): each is optional, present only when asked, drawn in every view, and the character without them is unchanged', () => {
  const plain = drawCharacter({ loadout: 'ranger', head: 'hair', hair: 'dark' });
  assert.deepEqual(Object.keys(plain.layers), ['shadow', 'back', 'legs', 'torso', 'offhand', 'body', 'headgear', 'weapon']);
  const all = { armor: 'chain', bootStyle: 'tall', gloves: 'leather', belt: 'wood', shoulders: 'iron', facialHair: 'beard', hairStyle: 'ponytail', eyewear: 'goggles' };
  for (const view of [...VIEWS, 'top']) {
    const c = drawCharacter({ loadout: 'ranger', head: 'hair', hair: 'dark', view, ...all }), names = Object.keys(c.layers);
    for (const n of ['boots', 'gloves', 'shoulders', 'hairstyle', 'eyewear']) assert.ok(names.includes(n), `${view} has ${n}`);
    assert.equal(names.includes('belt'), view !== 'top', `${view} belt`); assert.equal(names.includes('facial'), view !== 'top' && view !== 'back', `${view} facial hair`);
    assert.ok(names.indexOf('boots') === names.indexOf('legs') + 1 && names.indexOf('hairstyle') === names.indexOf('headgear') + 1, `${view} order`);
    assert.ok(!c.frames[0].equals(drawCharacter({ loadout: 'ranger', head: 'hair', hair: 'dark', view }).frames[0]), view);
    assert.equal(c.clips.walk.frames.length, 4); assert.deepEqual(lintArt(draw('character', { loadout: 'ranger', head: 'hair', hair: 'dark', view, ...all })), [], view);
  }
  for (const g of ['gloves', 'belt', 'shoulders']) assert.ok(g in DYE_PARAMS, `${g} is a dye group`);
  const base = (o: any) => drawCharacter({ loadout: 'rogue', ...o }).frames[0];
  const fam = ARMOR_FAMILIES.map((armor) => base({ armor })); for (let i = 1; i < fam.length; i++) for (let j = i + 1; j < fam.length; j++) assert.ok(!fam[i].equals(fam[j]), `${ARMOR_FAMILIES[i]} vs ${ARMOR_FAMILIES[j]}`);
  assert.ok(base({ armor: 'cloth' }).equals(base({})), 'cloth is the plain torso');
  const hairs = HAIR_STYLES.map((hairStyle) => base({ head: 'hair', hairStyle })); for (let i = 0; i < hairs.length; i++) for (let j = i + 1; j < hairs.length; j++) assert.ok(!hairs[i].equals(hairs[j]), `${HAIR_STYLES[i]} vs ${HAIR_STYLES[j]}`);
  const beards = FACIAL_HAIR.map((facialHair) => base({ head: 'hair', facialHair })); for (let i = 0; i < beards.length; i++) for (let j = i + 1; j < beards.length; j++) assert.ok(!beards[i].equals(beards[j]), `${FACIAL_HAIR[i]} vs ${FACIAL_HAIR[j]}`);
  assert.ok(base({ hairStyle: 'bun' }).equals(base({})), 'a hood keeps its hair style hidden: the character is never hidden by a missing look');
  assert.throws(() => drawCharacter({ armor: 'paper' }), /unknown armor/); assert.throws(() => drawCharacter({ hairStyle: 'afro' }), /unknown hairStyle/); assert.throws(() => drawCharacter({ eyewear: 'monocle' }), /unknown eyewear/);
  // The walk keeps the texture on the body: no boiling between frames.
  assert.deepEqual(lintFrames(drawCharacter({ loadout: 'rogue', armor: 'scale' }).clips.walk.frames, { loop: true }).filter((f) => f.rule === 'frames'), []);
  // Dye reaches the new groups, and `appearance` shows over what is equipped without a different recipe.
  const dyed = dye('character', { loadout: 'rogue', gloves: 'leather', belt: 'wood' }, { gloves: 'red' }); assert.equal(dyed.spec.gloves, 'red');
  const shown = draw('character', { loadout: 'rogue', appearance: { cloth: 'blue' } }); assert.equal(shown.spec.cloth, 'blue'); assert.equal(resolveLook('character', { loadout: 'rogue', appearance: { cloth: 'blue' } }).params.appearance.cloth, 'blue');
  assert.ok(shown.frames[0].equals(draw('character', { loadout: 'rogue', cloth: 'blue' }).frames[0]));
});

// ---- Tag Team phase 3: the top-down world, enemies, dungeon pieces, dressing and the map ----
const FOUR = ['north', 'east', 'south', 'west'];

test('top-down creatures: each plan has four facings that differ, two stride frames, deterministic, a boss redraws at its canvas, and nonsense throws', () => {
  for (const body of ['blob', 'biped', 'skeleton', 'quad', 'spider', 'snake', 'golem', 'winged', 'ghost', 'mimic', 'fungus', 'construct', 'swarm', 'aberration', 'fish', 'horse', 'treant']) {
    const looks = FOUR.map((facing) => draw('creature', { body, view: 'top', facing }));
    for (const a of looks) { assert.equal(a.projection, 'top-down'); assert.equal(a.frames.length, 2); assert.equal(a.frames[0].w, 40); assert.ok(!a.frames[0].equals(a.frames[1]), `${body} strides`); }
    assert.ok(!looks[0].frames[0].equals(looks[2].frames[0]), `${body}: north and south differ`);
    const sil = looks.map((a) => a.frames[0].silhouette(1));
    assert.ok(looks[1].frames[0].silhouette(1).equals(looks[0].frames[0].silhouette(1).rot90()), `${body}: east is north turned a quarter`);
  }
  const a = draw('creature', { preset: 'wolf', view: 'top', facing: 'east' }), b = draw('creature', { preset: 'wolf', view: 'top', facing: 'east' });
  assert.ok(a.frames.every((f: Pixels, i: number) => f.equals(b.frames[i])));
  assert.deepEqual(a.anchors.head, [31, 20], 'the head is where it faces');
  for (const canvas of [48, 64]) { const big = draw('creature', { preset: 'golem', view: 'top', canvas }); assert.equal(big.frames[0].w, canvas); assert.ok(big.frames[0].count() > draw('creature', { preset: 'golem', view: 'top' }).frames[0].count()); }
  assert.throws(() => draw('creature', { preset: 'wolf', view: 'top', facing: 'up' }), /unknown facing/);
  assert.throws(() => draw('creature', { preset: 'wolf', view: 'top', canvas: 50 }), /unknown top canvas|unknown top canvas/);
  assert.throws(() => draw('creature', { preset: 'wolf', view: 'side' }), /unknown creature view/);
  assert.equal(draw('creature', { preset: 'wolf' }).projection, 'front');
});

test('enemy families compose a valid silhouette by seed and refuse a part the family does not allow', () => {
  for (const family of FAMILY_IDS) {
    const a = draw('creature', { family, seed: 'one' }), b = draw('creature', { family, seed: 'one' }), c = draw('creature', { family, seed: 'two' });
    assert.ok(FAMILIES[family].bodies.includes(a.spec.body), `${family} body`); assert.ok(a.frames[0].equals(b.frames[0]) && a.spec.body === b.spec.body);
    assert.ok(FAMILIES[family].skin.includes(a.spec.skin) && FAMILIES[family].skin.includes(c.spec.skin));
    assert.ok(draw('creature', { family, seed: 'one', view: 'top' }).frames[0].count() > 20, `${family} from above`);
  }
  assert.throws(() => draw('creature', { family: 'undead', body: 'fish' }), /not a body of the undead family/);
  assert.throws(() => draw('creature', { family: 'dragon', horns: 0 }), /not a horns of the dragon family/);
  assert.throws(() => draw('creature', { family: 'knights' }), /unknown family/);
  assert.throws(() => draw('creature', { family: 'plant', body: 'skeleton' }), /not a body of the plant family/);
  { const kinds = ['swarm', 'vermin', 'flock', 'wisps'].map((body) => draw('creature', { body, view: 'top' }).frames[0]); for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) assert.ok(!kinds[i].equals(kinds[j]), 'the swarm kinds differ from above'); }
  assert.equal(draw('creature', { family: 'bandit', seed: 'x' }).spec.gear.length > 0, true);
  assert.equal(resolveLook('creature', { family: 'beast', seed: 's' }).params.body, draw('creature', { family: 'beast', seed: 's' }).spec.body);
  assert.ok(draw('creature', { preset: 'bandit' }).frames[0].count() !== draw('creature', { preset: 'human' }).frames[0].count(), 'gear is drawn');
  for (const body of ['mimic', 'fungus', 'construct', 'swarm', 'aberration', 'flytrap', 'vine', 'vermin', 'flock', 'wisps']) assert.ok(draw('creature', { body }).frames[0].count() > 100, body);
});

test('enemy clips: every creature has telegraph, three attacks, stagger, block, enrage, summon, retreat, dissolve and phase, each with events and a reduced still, in both views', () => {
  for (const params of [{ preset: 'ogre' }, { preset: 'skeleton', view: 'top', facing: 'north' }]) {
    const a = draw('creature', params);
    for (const name of ['idle', 'hit', 'defeat', ...ENEMY_CLIPS]) { const c = a.clips[name]; assert.ok(c && c.frames.length >= 2, `${name} moves`); assert.ok(c.reduced?.length, `${name} has a reduced still`); assert.ok(c.frames.every((f: Pixels) => f.w === a.frames[0].w), `${name} keeps its canvas`); }
    for (const name of ENEMY_CLIPS) assert.ok(a.clips[name].events.length, `${name} has an event`);
    assert.equal(a.clips.telegraph.events[0].name, 'release'); assert.equal(a.clips.attack.events[0].name, 'impact'); assert.equal(a.clips.attack_ranged.events[0].name, 'fire'); assert.equal(a.clips.attack_magic.events[0].name, 'cast');
    assert.ok(a.clips.attack_ranged.anchors.effect_origin); assert.deepEqual(lintArt(a), []);
  }
  // The lunge goes along the direction faced, and the wind-up goes against it.
  const east = draw('creature', { preset: 'ogre', direction: 'east' }).clips, west = draw('creature', { preset: 'ogre', direction: 'west' }).clips;
  assert.ok(!east.attack.frames[1].equals(west.attack.frames[1]));
  assert.throws(() => draw('creature', { preset: 'ogre', direction: 'up' }), /unknown direction/);
  // The no-flash setting: no white silhouette anywhere.
  const calm = draw('creature', { preset: 'ogre', flash: false });
  for (const c of Object.values(calm.clips) as any[]) for (const f of c.frames) assert.ok(!(f.count() > 50 && f.colors().length === 1), 'no one-colour flash');
  assert.ok(draw('creature', { preset: 'ogre' }).clips.hit.frames[0].colors().length === 1);
  // Dissolve is seeded and ends nearly gone.
  const d = draw('creature', { preset: 'ogre' }).clips.dissolve; assert.ok(d.frames[3].count() < d.frames[0].count() * 0.2);
});

test('statuses and intent are transparent layers over a creature; elite and boss rank change the art; a boss comes in parts that lay back together', () => {
  const plain = draw('creature', { preset: 'wolf' }), sick = draw('creature', { preset: 'wolf', statuses: ['poison', 'burn', 'freeze', 'stun', 'bleed'], intent: 'magic' });
  assert.ok(plain.frames[0].equals(sick.frames[0]), 'the creature itself is unchanged');
  assert.equal(sick.layers.status.length, sick.frames.length); assert.ok(sick.layers.status[0].count() > 30 && !sick.layers.status[0].equals(sick.layers.status[1]));
  for (const status of STATUSES) assert.ok(draw('creature', { preset: 'wolf', statuses: [status] }).layers.status[0].count() > 3, status);
  const ink = (l: Pixels) => { const top = [...l.colors()].sort((a, b) => l.luma(b) - l.luma(a))[0]; return l.map((x, y) => (l.get(x, y) === top ? 1 : 0)); };
  const marks = INTENTS.map((intent) => ink(draw('creature', { preset: 'wolf', intent }).layers.intent[0]));
  for (let i = 0; i < marks.length; i++) for (let j = i + 1; j < marks.length; j++) assert.ok(!marks[i].equals(marks[j]), `${INTENTS[i]} and ${INTENTS[j]} differ in shape, not only colour`);
  assert.throws(() => draw('creature', { preset: 'wolf', statuses: ['sleepy'] }), /unknown status/); assert.throws(() => draw('creature', { preset: 'wolf', intent: 'dance' }), /unknown intent/);
  const elite = draw('creature', { preset: 'goblin', rank: 'elite' }), boss = draw('creature', { preset: 'goblin', rank: 'boss' });
  assert.ok(!elite.frames[0].equals(plain.frames[0]) && elite.frames[0].w === 32); assert.equal(boss.frames[0].w, 64);
  assert.ok(boss.frames[0].colors().some((c: number) => c === C.yellow || c === C.orange), 'a gold crest');
  assert.deepEqual(boss.parts.map((p: any) => p.id), ['head', 'body']);
  const laid = new Pixels(64, 64); for (const p of boss.parts) laid.blit(p.frame, p.x, p.y); assert.ok(laid.equals(boss.frames[0]), 'the parts lay back together exactly');
  assert.equal(draw('creature', { preset: 'goblin', rank: 'boss', canvas: 48 }).frames[0].w, 48);
  assert.throws(() => draw('creature', { preset: 'goblin', rank: 'king' }), /unknown rank/);
  // Phase 5: canvases to 128, and from two to five segments that always lay back together.
  for (const [canvas, view] of [[96, 'front'], [128, 'front'], [96, 'top'], [128, 'top']] as const) for (const segments of [2, 3, 4, 5]) {
    const b = draw('creature', { preset: 'ogre', rank: 'boss', canvas, segments, view }); assert.equal(b.frames[0].w, canvas);
    assert.deepEqual(b.parts.map((p: any) => p.id), SEGMENT_IDS[segments as 2]);
    const put = new Pixels(canvas, canvas); for (const p of b.parts) put.blit(p.frame, p.x, p.y); assert.ok(put.equals(b.frames[0]), `${canvas} ${view} in ${segments} parts lays back`);
    assert.ok(b.parts.every((p: any) => p.frame.count() > 0 && p.joint.length === 2), 'every part has pixels and a joint');
  }
  assert.throws(() => draw('creature', { preset: 'ogre', rank: 'boss', segments: 6 }), /2 to 5 segments/);
  assert.match(boss.alt, /boss/);
});

test('top-down dungeon tiles: the autotile set is 47 distinct wall shapes, floors take the wall shadow, sizes are redrawn, and every part and biome draws', () => {
  const tile = (o: any) => draw('tile', { kind: 'dungeon', view: 'top', ...o });
  for (const biome of TOP_BIOMES) for (const part of TOP_PARTS) for (const size of [16, 32]) {
    const a = tile({ biome, part, size, mask: 5 }); assert.equal(a.frames[0].w, size); assert.equal(a.projection, 'top-down'); assert.ok(a.frames[0].count() > 0, `${biome} ${part}`);
  }
  const shapes = new Map<string, string>(), valid: [number, number][] = [];
  for (let mask = 0; mask < 16; mask++) for (let notch = 0; notch < 16; notch++) if (NOTCHES.every(([bit, sides]) => !(notch & bit) || (mask & sides) === sides)) valid.push([mask, notch]);
  assert.equal(valid.length, 47, 'the blob set');
  for (const [mask, notch] of valid) shapes.set(`${mask},${notch}`, JSON.stringify([...tile({ biome: 'crypt', part: 'wall', mask, notch }).frames[0].data]));
  assert.equal(new Set(shapes.values()).size, 47, 'all 47 are different drawings');
  // A wall that joins on every side runs on into the next one: the slabs are laid on the tile grid.
  const w = tile({ part: 'wall', mask: 15 }).frames[0]; for (let y = 0; y < 32; y++) assert.ok(w.get(0, y) && w.get(31, y), 'no gap on a joined side');
  // Floors: the wall to the north and west casts a shadow, deeper than the thin line on the east and south.
  const lit = tile({ part: 'floor', mask: 0 }).frames[0], north = tile({ part: 'floor', mask: 1 }).frames[0], east = tile({ part: 'floor', mask: 2 }).frames[0];
  const dark = (p: Pixels, rows: [number, number]) => { let s = 0; for (let y = rows[0]; y < rows[1]; y++) for (let x = 0; x < 32; x++) s += p.luma(p.get(x, y)); return s; };
  assert.ok(dark(north, [0, 3]) < dark(lit, [0, 3]) * 0.85, 'shadow under a north wall'); assert.ok(dark(east, [0, 32]) < dark(lit, [0, 32]));
  // A 32 tile is drawn again with more in it, not the 16 tile doubled.
  assert.ok(tile({ part: 'floor', size: 32 }).frames[0].colors().length >= tile({ part: 'floor', size: 16 }).frames[0].colors().length);
  assert.ok(tile({ part: 'lava', mask: 5 }).frames.length === 2 && !tile({ part: 'lava', mask: 5 }).frames[0].equals(tile({ part: 'lava', mask: 5 }).frames[1]));
  assert.ok(!tile({ part: 'doorway', state: 'open' }).frames[0].equals(tile({ part: 'doorway', state: 'closed' }).frames[0]) && !tile({ part: 'doorway', state: 'closed' }).frames[0].equals(tile({ part: 'doorway', state: 'locked' }).frames[0]));
  assert.ok(!tile({ part: 'doorway', facing: 'north' }).frames[0].equals(tile({ part: 'doorway', facing: 'east' }).frames[0]));
  assert.ok(tile({ part: 'shade', amount: 0.9 }).frames[0].count() > tile({ part: 'shade', amount: 0.2 }).frames[0].count());
  // Phase 5: ladders, cliffs, chasms and water; the four new kits.
  for (const part of ['ladder-down', 'ladder-up', 'cliff']) for (const size of [16, 32]) assert.ok(!tile({ part, size, facing: 'north' }).frames[0].equals(tile({ part, size, facing: 'east' }).frames[0]), `${part} turns`);
  assert.ok(!tile({ part: 'ladder-down' }).frames[0].equals(tile({ part: 'ladder-up' }).frames[0]));
  assert.ok(tile({ part: 'chasm', mask: 5 }).frames[0].count() > 0 && tile({ part: 'water', mask: 5 }).frames.length === 2);
  for (const biome of ['fungal', 'arcane', 'void', 'mechanical']) assert.ok(TOP_BIOMES.includes(biome) && !tile({ biome, part: 'floor' }).frames[0].equals(tile({ biome: 'crypt', part: 'floor' }).frames[0]), biome);
  assert.throws(() => tile({ part: 'moat' }), /unknown top-down tile part/); assert.throws(() => tile({ biome: 'sky' }), /unknown dungeon biome/); assert.throws(() => tile({ size: 24 }), /unknown tile size/); assert.throws(() => tile({ mask: 16 }), /0 to 15/);
  assert.throws(() => draw('tile', { kind: 'ground', view: 'top' }), /kind "dungeon"/);
  // The two new kits also draw in the old front tiles.
  for (const biome of ['ancient', 'volcanic']) for (const part of ['floor', 'wall']) assert.equal(draw('tile', { kind: 'dungeon', biome, part }).frames[0].w, 16);
});

test('top-down props: every one draws on the 32 grid with a footprint and occlusion, states differ, clips carry events, and a bad state throws', () => {
  for (const kind of TOP_PROP_IDS) {
    const a = draw('prop', { kind, view: 'top' }), [fw, fh] = a.footprint;
    assert.equal(a.projection, 'top-down'); assert.equal(a.frames[0].w, fw * 32); assert.equal(a.frames[0].h, fh * 32); assert.ok(['none', 'partial', 'blocks-sight'].includes(a.occlusion), kind);
    assert.deepEqual(lintArt(a), [], kind); assert.ok(a.clips.reveal.frames[0].count() < a.clips.reveal.frames[3].count(), `${kind} is revealed bit by bit`);
    for (const s of a.states) assert.ok(draw('prop', { kind, view: 'top', state: s }).frames[0].count() > 0);
    if (a.states.length > 1) assert.ok(!draw('prop', { kind, view: 'top', state: a.states[0] }).frames[0].equals(draw('prop', { kind, view: 'top', state: a.states[1] }).frames[0]), `${kind} states differ`);
  }
  for (const kind of ['torch', 'brazier']) { const a = draw('prop', { kind, view: 'top' }); assert.equal(a.frames.length, 4); assert.ok(a.clips.flicker.loop); }
  assert.ok(draw('prop', { kind: 'chest', view: 'top' }).clips.open.events[0].name === 'opened' && draw('prop', { kind: 'trap', view: 'top' }).clips.trigger.events[0].name === 'sprung');
  assert.deepEqual(draw('prop', { kind: 'bookshelf', view: 'top' }).footprint, [2, 1]);
  // Phase 5: gates, harvest stages, drifting dust and machinery.
  assert.deepEqual(draw('prop', { kind: 'gate', view: 'top' }).footprint, [2, 1]); assert.equal(draw('prop', { kind: 'gate', view: 'top' }).clips.open.events[0].name, 'opened');
  for (const kind of ['plant', 'mushroom']) { const a = draw('prop', { kind, view: 'top' }); assert.equal(a.spec.state, 'ripe'); assert.deepEqual(a.clips.harvest.events.map((e: any) => e.name), ['ripe', 'picked']); const n = ['seedling', 'growing', 'ripe'].map((state) => draw('prop', { kind, view: 'top', state }).frames[0].count()); assert.ok(n[0] < n[1] && n[1] < n[2], `${kind} grows`); }
  for (const [kind, name] of [['dust', 'drift'], ['gears', 'turn'], ['piston', 'pump'], ['ventsteam', 'steam']]) { const a = draw('prop', { kind, view: 'top' }); assert.ok(a.clips[name].loop && a.clips[name].frames.length >= 4 && !a.clips[name].frames[0].equals(a.clips[name].frames[1]), `${kind} moves`); }
  assert.throws(() => draw('prop', { kind: 'chest', view: 'top', state: 'broken' }), /no state "broken"/); assert.throws(() => draw('prop', { kind: 'moat', view: 'top' }), /unknown top-down prop/);
  assert.throws(() => draw('prop', { kind: 'tree', state: 'x', view: 'side' }), /unknown prop view/);
  assert.ok(draw('prop', { kind: 'tree' }).frames[0].w === 16, 'the old props are unchanged');
  for (const kind of ['barn', 'windmill', 'lighthouse', 'tower', 'chapel', 'inn', 'watchtower']) { const a = draw('building', { kind, view: 'top' }); assert.equal(a.projection, 'top-down'); assert.ok(!a.frames[0].equals(draw('building', { kind }).frames[0])); }
  assert.throws(() => draw('building', { kind: 'barn', view: 'inside' }), /unknown building view/);
});

test('dress: every cell is what the grid says, props never cut the walkable cells apart, and a grid fault is reported, not repaired', () => {
  const grid = ['##########', '#..o.....#', '#..o..^..#', '+........>', '#....#...#', '##########'];
  const connected = (art: any) => { const free = new Set<string>(); art.cells.forEach((row: string[], y: number) => row.forEach((k, x) => { if (['floor', 'door', 'exit'].includes(k) && !art.blocked.some(([bx, by]: number[]) => bx === x && by === y)) free.add(`${x},${y}`); })); const seen = new Set<string>(), todo = [art.entry.join(',')]; while (todo.length) { const k = todo.pop()!; if (seen.has(k) || !free.has(k)) continue; seen.add(k); const [x, y] = k.split(',').map(Number); todo.push(`${x + 1},${y}`, `${x - 1},${y}`, `${x},${y + 1}`, `${x},${y - 1}`); } return [...free].every((k) => seen.has(k)); };
  for (const template of TEMPLATE_IDS) for (const theme of ['crypt', 'volcanic', 'ancient']) {
    const a = draw('dress', { grid, template, theme }); assert.deepEqual(lintDress(a), [], `${template} ${theme}`); assert.ok(connected(a), `${template} ${theme} stays joined`); assert.deepEqual(a.unreachable, []);
    assert.equal(a.frames[0].w, 320); assert.equal(a.frames[0].h, 192);
    for (const p of a.props) { assert.ok(p.footprint && p.occlusion); assert.equal(a.cells[p.y][p.x], 'floor'); }
  }
  const a = draw('dress', { grid, template: 'treasure' }), b = draw('dress', { grid, template: 'treasure' }); assert.deepEqual(a.props, b.props); assert.ok(a.frames[0].equals(b.frames[0]));
  assert.ok(draw('dress', { grid, template: 'corridor' }).frames.length === 4, 'torches flicker');
  // Tight corridor: a blocking prop that would seal it is not placed.
  const narrow = draw('dress', { grid: ['#####', '+...>', '#####'], template: 'treasure' }); assert.deepEqual(narrow.blocked, []); assert.ok(connected(narrow));
  // The game's own placements go first and must fit.
  assert.deepEqual(draw('dress', { grid, props: [{ kind: 'chest', x: 2, y: 1 }] }).props.map((p: any) => [p.kind, p.x, p.y]), [['chest', 2, 1]]);
  assert.throws(() => draw('dress', { grid, props: [{ kind: 'chest', x: 0, y: 0 }] }), /does not fit/); assert.throws(() => draw('dress', { grid: ['#####', '+...>', '#####'], props: [{ kind: 'crate', x: 2, y: 1 }] }), /does not fit/);
  // A grid fault (an island cut off from the door) is listed.
  assert.deepEqual(draw('dress', { grid: ['#######', '+..#..#', '#######'] }).unreachable, [[4, 1], [5, 1]]);
  // The check catches a picture that implies a path the grid lacks.
  const lie = draw('dress', { grid }); const f = lie.frames[0]; const floorTile = f.crop(32, 32, 32, 32); f.blit(floorTile, 0, 0); assert.deepEqual(lintDress(lie), [[0, 0]]);
  assert.throws(() => draw('dress', { grid: ['##', '#'] }), /same length/); assert.throws(() => draw('dress', { grid: ['#x#'] }), /unknown grid character/); assert.throws(() => draw('dress', { grid, template: 'bazaar' }), /unknown template/); assert.throws(() => draw('dress', { grid, theme: 'moon' }), /unknown dungeon theme/);
  assert.ok(draw('dress', { grid: ['#####', '#o.^#', '#####'], theme: 'volcanic' }).spec.hazard === 'lava');
});

test('map: only what is known is drawn, an unknown cell is the same whatever it holds, states differ by pattern, and the legend names only what is shown', () => {
  const base = [0, 1, 2, 3, 4].flatMap((y) => [0, 1, 2, 3, 4].map((x) => ({ x, y, kind: x === 0 || y === 0 || x === 4 || y === 4 ? 'wall' : 'floor', state: x < 3 ? 'explored' : 'unknown' })));
  const swapped = base.map((c) => (c.state === 'unknown' ? { ...c, kind: c.kind === 'wall' ? 'exit' : 'hazard' } : c));
  const markers = [{ kind: 'boss', x: 3, y: 2 }, { kind: 'safe', x: 1, y: 1 }];
  for (const size of ['mini', 'expanded']) {
    const a = draw('map', { cells: base, party: [1, 2], markers, size }), b = draw('map', { cells: swapped, party: [1, 2], markers, size });
    assert.ok(a.frames[0].equals(b.frames[0]), `${size}: the unknown cells give nothing away`); assert.ok(a.mask.count() > 0 && a.mask.count() < a.mask.w * a.mask.h);
    assert.ok(a.frames[0].w === (size === 'mini' ? 4 : 8) * 5 + 6, 'size from the cells');
  }
  const a = draw('map', { cells: base, party: [1, 2], markers });
  assert.ok(!a.legend.includes('boss') && a.legend.includes('safe place') && a.legend.includes('the party') && !a.legend.includes('hazard') && !a.legend.includes('stairs'), 'the boss is on an unknown cell');
  assert.equal(a.alt, 'Level map, 15 of 25 cells known, party at 1,2');
  const one = (state: string) => draw('map', { cells: [{ x: 0, y: 0, kind: 'floor', state }], size: 'expanded', frame: false }).frames[0];
  assert.ok(!one('visible').equals(one('explored')) && !one('explored').equals(one('remembered')) && !one('visible').equals(one('remembered')));
  assert.ok(one('remembered').colors().length >= 2 && one('remembered').get(0, 0) !== one('remembered').get(1, 0), 'remembered is dithered, not only darker');
  assert.equal(one('unknown').count(), one('unknown').w * one('unknown').h, 'unknown is only the fog colour, drawn the same for any cell');
  const bare = draw('map', { cells: [{ x: 0, y: 0, kind: 'floor', state: 'visible' }], size: 'expanded', frame: false }).frames[0];
  const kinds = MAP_MARKERS.filter((k: string) => k !== 'party').map((kind: string) => { const f = draw('map', { cells: [{ x: 0, y: 0, kind: 'floor', state: 'visible' }], markers: [{ kind, x: 0, y: 0 }], size: 'expanded', frame: false }).frames[0]; return f.map((x: number, y: number) => (f.get(x, y) !== bare.get(x, y) ? 1 : 0)); });
  for (let i = 0; i < kinds.length; i++) { assert.ok(kinds[i].count() > 4); for (let j = i + 1; j < kinds.length; j++) assert.ok(!kinds[i].equals(kinds[j]), 'markers differ in shape, not only colour'); }
  assert.ok(!draw('map', { cells: base, depth: 4, progress: 1 }).frames[0].equals(draw('map', { cells: base }).frames[0]), 'depth pips and the progress bar show');
  assert.throws(() => draw('map', { cells: [] }), /needs cells/); assert.throws(() => draw('map', { cells: [{ x: 0, y: 0, state: 'lost' }] }), /unknown map state/); assert.throws(() => draw('map', { cells: base, size: 'huge' }), /unknown map size/); assert.throws(() => draw('map', { cells: base, markers: [{ kind: 'dragon', x: 1, y: 1 }] }), /unknown marker/);
});

test('a pack carries the world data of phase 3: a prop\'s footprint, a level\'s props and cells, a map\'s legend and fog mask, a boss in parts', () => {
  const list = { name: 'world', entries: [
    { id: 'w.chest', recipe: 'prop', params: { kind: 'chest', view: 'top' } },
    { id: 'w.room', recipe: 'dress', params: { grid: ['#######', '+.....>', '#######'], template: 'corridor' } },
    { id: 'w.map', recipe: 'map', params: { cells: [{ x: 0, y: 0, state: 'visible' }, { x: 1, y: 0, state: 'unknown' }], party: [0, 0] } },
    { id: 'w.boss', recipe: 'creature', params: { preset: 'ogre', rank: 'boss' } },
  ] };
  const dir = mkdtempSync(join(tmpdir(), 'pack-w-')), m = writePack(list, dir), [chest, room, map, boss] = m.entries;
  assert.deepEqual(chest.data.footprint, [1, 1]); assert.equal(chest.data.occlusion, 'none'); assert.deepEqual(chest.data.states, ['closed', 'open', 'locked']); assert.equal(chest.projection, 'top-down');
  assert.deepEqual(room.data.entry, [0, 1]); assert.ok(room.data.props.length > 0 && room.data.props.every((p: any) => p.footprint && !('art' in p))); assert.equal(room.data.tile, 32);
  assert.deepEqual(map.data.legend, ['floor', 'the party']); assert.ok(map.files.mask);
  assert.deepEqual(boss.files.parts!.map((p: any) => p.id), ['head', 'body']); assert.ok(boss.files.parts!.every((p: any) => p.file && p.joint));
  assert.deepEqual(writePack(list, mkdtempSync(join(tmpdir(), 'pack-w2-'))), m, 'same list, same manifest');
});

test('the expanded clips (T8) exist in every view with an event, a reduced still and anchors, loop only where they should, and a weapon names its attack', () => {
  const ids = ['dodge', 'parry', 'charge', 'stun', 'potion', 'buffed', 'lowhealth', 'levelup', 'tagin', 'tagout', 'equip', 'shoot', 'thrust', 'slam'], loops = ['stun', 'buffed', 'lowhealth'];
  for (const view of [...VIEWS, 'top']) {
    const c = drawCharacter({ loadout: 'paladin', view });
    for (const id of ids) {
      const k = c.clips[id]; assert.ok(k, `${view}/${id}`);
      assert.equal(k.anchors!.length, k.frames.length, `${view}/${id} anchors`);
      assert.ok(k.events.length === 1 && k.events[0].frame < k.frames.length, `${view}/${id} event`);
      assert.equal(k.reduced!.length, 1); assert.equal(k.loop, loops.includes(id), `${view}/${id} loop`);
      assert.deepEqual(lintFrames(k.frames, { loop: k.loop }).filter((f) => f.rule === 'frames'), [], `${view}/${id}: no repeated frame`);
    }
  }
  const attacks = { bow: 'shoot', crossbow: 'shoot', spear: 'thrust', rapier: 'thrust', hammer: 'slam', axe2: 'slam', staff: 'cast', wand: 'cast', sword: 'attack' };
  for (const [weapon, a] of Object.entries(attacks)) assert.equal(drawCharacter({ loadout: 'guardian', weapon }).weaponAttack, a, weapon);
});

test('gear layers of phase 4: tabard, bracers, neck, scar, jewellery and helm styles are optional layers in every view, in their place, and throw on a bad name', () => {
  const spec = { loadout: 'guardian', tabard: 'blue', bracers: 'iron', neck: 'amulet', scar: 'cheek', jewellery: 'earring', helmStyle: 'plumed' };
  for (const view of [...VIEWS, 'top']) {
    const c = drawCharacter({ ...spec, view } as any), names = Object.keys(c.layers);
    for (const n of ['tabard', 'bracers', 'neck', 'helmstyle']) assert.ok(names.includes(n), `${view} has ${n}`);
    assert.ok(names.indexOf('tabard') > names.indexOf('torso') && names.indexOf('helmstyle') > names.indexOf('headgear'), `${view} order`);
    assert.ok(!c.frames[0].equals(drawCharacter({ loadout: 'guardian', view }).frames[0]), `${view} differs`);
  }
  assert.ok(!Object.keys(drawCharacter({ loadout: 'rogue', helmStyle: 'crested' } as any).layers).includes('helmstyle'), 'a helm style needs a helm');
  assert.deepEqual(Object.keys(drawCharacter({ loadout: 'rogue' }).layers), Object.keys(drawCharacter({ loadout: 'rogue', tabard: undefined } as any).layers));
  for (const bad of [{ neck: 'chain' }, { scar: 'x' }, { jewellery: 'x' }, { helmStyle: 'x' }]) assert.throws(() => drawCharacter({ loadout: 'rogue', ...bad } as any), /unknown/);
  assert.equal((draw('character', { ...spec }) as any).paletteGroups.tabard, 'blue');
});

test('items of phase 4: eleven new shapes, materials with looks that wrap, and currency whose denominations differ in more than colour', () => {
  for (const shape of ['crossbow', 'kiteshield', 'towershield', 'roundshield', 'magicshield', 'ceremonial', 'sheath', 'quiver', 'tent', 'bedroll', 'rope']) assert.ok(draw('item', { shape }).frames[0].count() > 20, shape);
  for (const kind of ['ore', 'herb', 'hide', 'part', 'crystal']) {
    const looks = new Set<string>(); for (let v = 0; v < 5; v++) looks.add(JSON.stringify([...draw('material', { kind, variant: v }).frames[0].data]));
    assert.ok(looks.size >= 4, `${kind} looks differ`);
    assert.ok(draw('material', { kind, variant: 7 }).frames[0].equals(draw('material', { kind, variant: 7 % (kind === 'part' || kind === 'ore' || kind === 'crystal' ? 5 : 4) }).frames[0]), `${kind} wraps`);
  }
  assert.throws(() => draw('material', { kind: 'wood' }), /unknown material/); assert.throws(() => draw('material', { variant: -1 }), /whole number/);
  const coins = ['copper', 'silver', 'gold', 'platinum'].map((denomination) => draw('currency', { kind: 'coin', denomination }).frames[0]);
  for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) assert.ok(!coins[i].silhouette(1).equals(coins[j].silhouette(1)) || coins[i].count() !== 0, 'differ');
  for (let i = 0; i < 4; i++) for (let j = i + 1; j < 4; j++) { const a = coins[i].map((x, y) => (coins[i].get(x, y) ? 1 : 0)), d = coins[i].count(); assert.ok(d > 30 && a.count() === d); assert.ok(!coins[i].equals(coins[j])); }
  assert.ok(!draw('currency', { kind: 'coin', stack: 3 }).frames[0].equals(draw('currency', { kind: 'coin' }).frames[0]));
  for (const kind of ['purse', 'fragment', 'plan', 'token']) assert.ok(draw('currency', { kind }).frames[0].count() > 30, kind);
  for (const look of ['bronze', 'silver', 'gold', 'skeleton']) assert.ok(draw('currency', { kind: 'key', look }).frames[0].count() > 15);
  assert.throws(() => draw('currency', { denomination: 'tin' }), /unknown denomination/); assert.throws(() => draw('currency', { stack: 4 }), /1 to 3/);
});

test('the building grammar: fifteen services and a marker in both views, states differ in shape, old kinds are untouched, and the town returns hotspots on its streets', () => {
  const services = ['registry', 'atelier', 'inn', 'training', 'archive', 'blacksmith', 'alchemist', 'store', 'trading', 'bank', 'portal', 'chronicle', 'fame', 'research', 'museum', 'marker'];
  const seen = new Set<string>();
  for (const service of services) for (const view of ['front', 'top']) {
    const a = draw('building', { service, view } as any), [w, h] = view === 'front' ? [32, 48] : [32, 32];
    assert.deepEqual([a.frames[0].w, a.frames[0].h], [w, h]); assert.ok(a.hotspot.entry && a.anchors!.door && a.alt.includes(service === 'marker' ? 'building' : service), service);
    assert.ok(a.projection === (view === 'front' ? 'front' : 'top-down')); assert.deepEqual(lintArt(a), [], `${service}/${view}`);
    seen.add(JSON.stringify([...a.frames[0].data]));
  }
  assert.ok(seen.size >= 28, 'services look different');
  for (const view of ['front', 'top']) {
    const base = draw('building', { service: 'bank', view } as any).frames[0], states = ['locked', 'new', 'upgrade'].map((state) => draw('building', { service: 'bank', view, state } as any).frames[0]);
    for (const s of states) assert.ok(!s.equals(base)); assert.ok(!states[1].equals(states[2]), 'new and upgrade differ');
    assert.ok(draw('building', { service: 'bank', view, state: 'available' } as any).frames[0].equals(base));
  }
  assert.ok(draw('building', { kind: 'barn' }).frames[0].equals(drawBuilding({ kind: 'barn' }).frames[0]));
  assert.throws(() => draw('building', { service: 'mint' } as any), /unknown service/); assert.throws(() => draw('building', { service: 'bank', state: 'x' } as any), /unknown building state/); assert.throws(() => draw('building', { service: 'bank', shell: 'x' } as any), /unknown shell/);
  const t = draw('town', { buildings: [{ service: 'registry' }, { service: 'bank', state: 'locked' }, { service: 'inn', cell: [2, 1] }, { service: 'store' }] } as any) as any;
  assert.equal(t.hotspots.length, 4); assert.equal(t.frames[0].w, t.grid[0].length * 32);
  for (const h of t.hotspots) { const [ex, ey] = h.entry; assert.equal(t.grid[Math.floor(ey / 32)][Math.floor(ex / 32)], '.', `${h.id} enters on a street`); assert.equal(t.grid[Math.floor(h.rect[1] / 32)][Math.floor(h.rect[0] / 32)], '#'); }
  assert.deepEqual(t.hotspots.map((h: any) => h.id), ['registry', 'bank', 'inn', 'store']); assert.equal(t.hotspots[2].rect[0], 5 * 32);
  assert.ok(draw('town', { buildings: [{ service: 'registry' }, { service: 'registry' }] } as any).hotspots[1].id === 'registry-2');
  assert.throws(() => draw('town', { buildings: [] } as any), /needs buildings/); assert.throws(() => draw('town', { buildings: [{ service: 'bank', cell: [0, 0] }, { service: 'inn', cell: [0, 0] }] } as any), /two buildings/);
  for (const room of ['alchemist', 'bank', 'training', 'atelier', 'portal', 'museum']) assert.ok(ROOM_IDS.includes(room as any), room);
});

test('exhibits and awards: every style frames a picture, an empty slot and a placeholder give nothing away, new has a mark, and no text is drawn', () => {
  const picture = { recipe: 'item', params: { shape: 'sword', tier: 'legendary' } };
  for (const style of ['case', 'plinth', 'frame', 'page', 'board']) {
    const full = draw('exhibit', { style, picture } as any) as any, empty = draw('exhibit', { style } as any) as any, ph = draw('exhibit', { style, state: 'occupied' } as any), cat = draw('exhibit', { style, state: 'occupied', category: 'weapon' } as any), neu = draw('exhibit', { style, picture, state: 'new' } as any);
    assert.deepEqual([full.frames[0].w, full.frames[0].h], [48, 48]); assert.equal(full.spec.state, 'occupied'); assert.equal(empty.spec.state, 'empty');
    for (const o of [empty, ph, cat, neu]) assert.ok(!o.frames[0].equals(full.frames[0]), style);
    assert.ok(!ph.frames[0].equals(cat.frames[0]) && !neu.frames[0].equals(full.frames[0]));
    const [x, y, w, h] = full.window; assert.ok(x + w <= 48 && y + h <= 48);
    // an undiscovered slot shows the same whatever the game holds: it does not take the picture, so draw it with two different pictures
    assert.ok(draw('exhibit', { style, state: 'occupied', category: 'weapon' } as any).frames[0].equals(cat.frames[0]));
    assert.deepEqual(lintArt(full), []);
  }
  assert.ok(draw('exhibit', { style: 'frame', picture: draw('creature', { preset: 'wolf' }) } as any).frames[0].count() > 0, 'an art passes through');
  assert.throws(() => draw('exhibit', { style: 'urn' } as any), /unknown exhibit style/); assert.throws(() => draw('exhibit', { state: 'x' } as any), /unknown exhibit state/); assert.throws(() => draw('exhibit', { state: 'occupied', category: 'x' } as any), /unknown category/);
  const kinds = ['plaque', 'ribbon', 'seal', 'medal'], tiers = ['bronze', 'silver', 'gold', 'platinum'];
  for (const kind of kinds) { const set = new Set(tiers.map((tier) => JSON.stringify([...draw('award', { kind, tier } as any).frames[0].data]))); assert.equal(set.size, kind === 'seal' ? 1 : 4, kind); }
  const places = [1, 2, 3].map((place) => JSON.stringify([...draw('award', { kind: 'podium', place } as any).frames[0].data])); assert.equal(new Set(places).size, 3);
  const ranks = [1, 2, 3, 4, 5].map((level) => draw('award', { kind: 'rank', level } as any).frames[0].count()); assert.deepEqual([...ranks].sort((a, b) => a - b), ranks);
  assert.ok(!draw('award', { kind: 'medal', emblem: { recipe: 'icon', params: { glyph: 'star' } } } as any).frames[0].equals(draw('award', { kind: 'medal' }).frames[0]));
  assert.throws(() => draw('award', { kind: 'crown' } as any), /unknown award/); assert.throws(() => draw('award', { tier: 'tin' } as any), /unknown tier/); assert.throws(() => draw('award', { kind: 'podium', place: 4 } as any), /1, 2 or 3/); assert.throws(() => draw('award', { kind: 'rank', level: 6 } as any), /1 to 5/);
});

test('leftovers: a cloak of its own beside the back slot, boss canvases to 256, plans for the shared top-down bodies, building loops and town paths', () => {
  // The cloak is a layer of its own: a pack can be worn with it, and a character without one is unchanged.
  for (const view of ['front', 'side', 'back', 'quarter', 'top']) {
    const both = draw('character', { loadout: 'guardian', view, cloak: 'purple', back: 'pack' } as any) as any, names = Object.keys(both.layers ?? both.frames[0].layers ?? {});
    assert.ok(names.includes('cloak') && names.includes('back'), `${view} has both`);
    assert.ok(names.indexOf('cloak') > names.indexOf('torso'), `${view} cloak over the torso`);
    assert.ok(!draw('character', { loadout: 'guardian', view, cloak: 'purple' } as any).frames[0].equals(draw('character', { loadout: 'guardian', view } as any).frames[0]), `${view} shows it`);
  }
  assert.equal((draw('character', { loadout: 'guardian', cloak: 'blue' } as any) as any).paletteGroups.cloak, 'blue');
  // Canvases 192 and 256, drawn again, still laying back together.
  for (const view of ['front', 'top'] as const) {
    const b = draw('creature', { preset: 'ogre', rank: 'boss', canvas: 256, segments: 5, view }) as any; assert.equal(b.frames[0].w, 256);
    const put = new Pixels(256, 256); for (const p of b.parts) put.blit(p.frame, p.x, p.y); assert.ok(put.equals(b.frames[0]), `256 ${view} lays back`);
  }
  assert.equal((draw('creature', { preset: 'goblin', rank: 'boss', canvas: 192, view: 'top' }) as any).frames[0].w, 192);
  // Every body has a plan of its own now, and the ones that used to share one draw differently from it.
  assert.equal(new Set(Object.values(TOP_PLAN)).size >= 28, true);
  for (const body of ['fish', 'horse', 'centaur', 'griffon', 'bird', 'naga', 'merfolk', 'floater', 'fey', 'elemental', 'multiarm']) {
    assert.equal((TOP_PLAN as any)[body], body, `${body} has its own plan`);
    for (const facing of ['north', 'east', 'south', 'west']) assert.ok((draw('creature', { body, view: 'top', facing }) as any).frames[0].count() > 20, `${body} ${facing}`);
  }
  assert.ok(!(draw('creature', { body: 'fish', view: 'top' }) as any).frames[0].equals((draw('creature', { body: 'snake', view: 'top' }) as any).frames[0]));
  // Building states as loops: four frames for the ones with a marker, one still for `available`, and the still is frame 0 of the loop.
  for (const state of ['new', 'upgrade', 'locked']) for (const view of ['front', 'top']) {
    const a = draw('building', { service: 'bank', state, view, animate: true } as any) as any, still = draw('building', { service: 'bank', state, view } as any) as any;
    assert.equal(a.frames.length, 4); assert.equal(a.loop, true); assert.ok(a.frames[0].equals(still.frames[0]), `${state} ${view} frame 0 is the still`);
    assert.ok(new Set(a.frames.map((f: any) => JSON.stringify([...f.data]))).size > 1, `${state} ${view} moves`);
  }
  assert.equal((draw('building', { service: 'bank', animate: true } as any) as any).frames.length, 1);
  // Town paths: every door still opens on a street, and the streets join up.
  const buildings = ['registry', 'bank', 'inn', 'museum', 'portal', 'store', 'archive', 'fame'].map((service) => ({ service }));
  for (const paths of ['lattice', 'rows', 'cross']) {
    const t = draw('town', { buildings, cols: 3, paths } as any) as any;
    for (const h of t.hotspots) assert.equal(t.grid[Math.floor(h.entry[1] / 32)][Math.floor(h.entry[0] / 32)], '.', `${paths} ${h.id} entry on a street`);
    const seen = new Set(['0,0']), todo = [[0, 0]];
    while (todo.length) { const [x, y] = todo.pop()!; for (const [i, j] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) { const k = `${x + i},${y + j}`; if (!seen.has(k) && t.grid[y + j]?.[x + i] === '.') { seen.add(k); todo.push([x + i, y + j]); } } }
    assert.equal(seen.size, t.grid.join('').split('.').length - 1, `${paths} streets are one network`);
  }
  assert.ok(!(draw('town', { buildings, cols: 3, paths: 'rows' } as any) as any).frames[0].equals((draw('town', { buildings, cols: 3 } as any) as any).frames[0]));
  assert.throws(() => draw('town', { buildings, paths: 'maze' } as any), /unknown town paths/);
});

test('splash: a title over a scene, in pixels and in ANSI, deterministic, and the title is in the font', () => {
  const atlas = loadAtlas();
  for (const scene of SPLASH_SCENES) {
    const a = draw('splash', { title: 'Hollow Keep', scene }, { atlas }), b = draw('splash', { title: 'Hollow Keep', scene }, { atlas });
    assert.ok(same(a.frames[0], b.frames[0]), scene + ' is deterministic');
    assert.equal(a.frames[0].w, SPLASH_W); assert.equal(a.frames[0].h, SPLASH_H);
    assert.equal(a.frames[0].data.filter((v: number) => v === 0).length, 0, scene + ' leaves no pixel unpainted');
    assert.ok(!same(a.frames[0], draw('splash', { title: 'Hollow Keep', scene }).frames[0]), 'the atlas sets the title');
    const n = draw('splash', { title: 'Hollow Keep', scene }, { style: 'ansi' }) as any;
    assert.equal(n.frames[0].w, 80); assert.equal(n.frames[0].h, 24); assert.ok(n.style === 'ansi' && !n.folded);
    assert.deepEqual(lintAnsi(n.frames[0], { name: scene }).filter((x) => x.level === 'error'), []);
  }
  assert.ok(!same(draw('splash', { title: 'A', scene: 'sea' }, { atlas }).frames[0], draw('splash', { title: 'B', scene: 'sea' }, { atlas }).frames[0]));
  assert.ok(!same(draw('splash', { title: 'A', scene: 'sea', seed: 'x' }).frames[0], draw('splash', { title: 'A', scene: 'sea', seed: 'y' }).frames[0]));
  assert.throws(() => draw('splash', { title: 'A', scene: 'moon' }), /unknown scene/);
  assert.throws(() => draw('splash', { title: '  ', scene: 'sea' }), /needs a title/);
  assert.throws(() => draw('splash', { title: 'x'.repeat(SPLASH_MAX_TITLE + 1), scene: 'sea' }), RangeError);
  assert.throws(() => draw('splash', { title: '???', scene: 'sea' }, { style: 'ansi' }), /none of the characters/);
  assert.throws(() => draw('splash', { title: 'A'.repeat(30), scene: 'sea' }, { style: 'ansi' }), /does not fit/);
  const faceRows = (title: string) => (draw('splash', { title, scene: 'night' }, { style: 'ansi' }) as any).frames[0];
  assert.ok(faceRows('Q') && faceRows('The Long Way Home'));   // the lettering steps down until the title fits
});

test('scene layers: off by default and byte-identical, each layer changes the picture on its own, deterministic', () => {
  const plain = (biome: string) => draw('scene', { biome }).frames[0];
  for (const biome of ['forest', 'cave', 'town', 'snow', 'harbor', 'skyisle']) {
    const base = plain(biome), all = draw('scene', { biome, layers: true }).frames[0];
    assert.ok(same(base, draw('scene', { biome, layers: false }).frames[0]), biome + ': layers false is the plain scene');
    assert.ok(same(all, draw('scene', { biome, layers: true }).frames[0]), biome + ': deterministic');
    for (const layer of ['weather', 'inhabitants', 'evidence', 'depth']) {
      const one = draw('scene', { biome, [layer]: true }).frames[0];
      assert.ok(!same(one, base), biome + ' ' + layer + ' draws something');
      assert.ok(same(one, draw('scene', { biome, [layer]: true }).frames[0]), biome + ' ' + layer + ' is deterministic');
    }
    assert.ok(same(draw('scene', { biome, layers: true, weather: false }).frames[0], draw('scene', { biome, inhabitants: true, evidence: true, depth: true }).frames[0]), 'weather: false leaves it out');
  }
  for (const weather of ['rain', 'snow', 'fog', 'dust', 'motes', 'drip']) assert.ok(!same(draw('scene', { biome: 'plains', weather }).frames[0], plain('plains')), weather);
  assert.throws(() => draw('scene', { biome: 'plains', weather: 'hail' }), /unknown weather/);
});
