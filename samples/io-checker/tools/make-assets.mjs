#!/usr/bin/env node
// Makes the IO Checker's assets, so every file in `assets/` has a recipe and nothing in it is anyone else's work except what
// `assets.tsv` says. Run from the repo root:
//
//     node samples/io-checker/tools/make-assets.mjs
//
// * Painter (art-sprites), Composer (music-composer) and Animator (sprite-animator), vendored in `src/vendor/`, draw the
//   sprites, tiles, jingle, song and clip: same recipe, parameters and seed, same bytes. Their output is CC0-1.0.
// * Hand-built files (compressed textures, EXR, tiny data files) are written here, byte by byte: CC0-1.0.
// * ffmpeg, magick, lame and flac turn the generated PNG and WAV into the other formats (BMP, TGA, JPEG, WebP, QOI, HDR,
//   Ogg, MP3, FLAC). The *input* is deterministic; the encoders' bytes can change between encoder versions, which is why the
//   result is committed and `.joyride/assets.toml` pins each file's hash.
// * The sister checkout `../r3i6nos/corpus/files` (MIT, made in code by the same owner) supplies the tracker modules, the Theora
//   clip and three tiny test fonts, because no tool here writes them. They are copied only when that checkout is beside this one.
// * The IBM VGA 8x16 font (VileR, CC BY-SA 4.0) is copied from `public/fonts/ansiapps/` unmodified; the bitmap-font versions
//   are cut from its glyph atlas, so they carry the same licence.
//
// Writes `assets.tsv`: path, kind, licence (SPDX), author, source, recipe. `asset-core/examples/sample_assets_toml.rs` turns it
// into `.joyride/assets.toml` with real content hashes through the Asset Library's own catalog.

import { mkdirSync, writeFileSync, readFileSync, copyFileSync, existsSync, readdirSync, statSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateRawSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const proj = join(here, '..');
const root = join(proj, '..', '..');
const out = join(proj, 'assets');
const vend = (p) => join(root, 'src', 'vendor', p);

const { draw } = await import(vend('art-sprites/index.js'));
const { pixelsToPng, encodePng } = await import(vend('art-sprites/png.js'));
const { loadAtlas } = await import(vend('art-sprites/atlas.js'));
const composer = await import(vend('music-composer/index.js'));
const { animate, writeClip } = await import(vend('sprite-animator/index.js'));
const painterVersion = JSON.parse(readFileSync(vend('art-sprites/package.json'))).version;
const composerVersion = JSON.parse(readFileSync(vend('music-composer/package.json'))).version;
const animatorVersion = JSON.parse(readFileSync(vend('sprite-animator/package.json'))).version;

rmSync(out, { recursive: true, force: true });
const rows = [];
const AUTHOR = 'JoyrIDE';
const SRC_GEN = 'samples/io-checker/tools/make-assets.mjs';

/** Writes a file under assets/ and records its licence row. */
function put(rel, bytes, { kind, licence = 'CC0-1.0', author = AUTHOR, source = SRC_GEN, recipe = '', credit = '' }) {
  const p = join(out, rel);
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, bytes);
  rows.push([`assets/${rel}`, kind, licence, author, source, recipe, credit]);
}
const sh = (cmd, args) => execFileSync(cmd, args, { stdio: ['ignore', 'pipe', 'pipe'] });
const scratch = join(proj, '.scratch');
mkdirSync(scratch, { recursive: true });

// ---- pictures: Painter ------------------------------------------------------------------------------------------------------
const hero = draw('character', { loadout: 'rogue' });
const heroPng = pixelsToPng(hero.frames[0], 6);
put('images/hero.png', heroPng, { kind: 'image', recipe: `art-sprites ${painterVersion} character {"loadout":"rogue"} frame 0 scale 6` });

// The walk clip as a horizontal strip: four 40x32 frames, scale 4.
const walk = hero.clips.walk;
{
  const fw = hero.frames[0].w * 4, fh = hero.frames[0].h * 4;
  const sheet = new Uint8Array(fw * walk.frames.length * fh * 4);
  walk.frames.forEach((f, i) => {
    const rgba = f.toRgba(4);
    for (let y = 0; y < fh; y++) sheet.set(rgba.subarray(y * fw * 4, (y + 1) * fw * 4), (y * fw * walk.frames.length + i * fw) * 4);
  });
  put('images/hero-walk.png', encodePng(fw * walk.frames.length, fh, sheet), { kind: 'image', recipe: `art-sprites ${painterVersion} character {"loadout":"rogue"} clip walk (${walk.frames.length} frames, ${walk.fps} fps) scale 4` });
}

// A tileset: four 16x16 tiles side by side, scale 4 (64x64 each).
{
  const seeds = ['grass', 'stone', 'water', 'sand'];
  const frames = seeds.map((s) => draw('tile', { seed: s }).frames[0]);
  const ts = 16 * 4;
  const sheet = new Uint8Array(ts * seeds.length * ts * 4);
  frames.forEach((f, i) => {
    const rgba = f.toRgba(4);
    for (let y = 0; y < ts; y++) sheet.set(rgba.subarray(y * ts * 4, (y + 1) * ts * 4), (y * ts * seeds.length + i * ts) * 4);
  });
  put('images/tiles.png', encodePng(ts * seeds.length, ts, sheet), { kind: 'image', recipe: `art-sprites ${painterVersion} tile seeds ${seeds.join(',')} scale 4` });
}

// ---- pictures: other containers, from hero.png -------------------------------------------------------------------------------
const heroPath = join(out, 'images/hero.png');
function magick(ext, args = [], extra = {}) {
  const tmp = join(scratch, `hero.${ext}`);
  sh('magick', [heroPath, ...args, tmp]);
  put(`images/hero.${ext}`, readFileSync(tmp), { kind: 'image', recipe: `magick hero.png ${args.join(' ')} hero.${ext}`.replace('  ', ' '), ...extra });
}
magick('bmp', ['-background', '#202030', '-alpha', 'remove', '-type', 'TrueColor', '-define', 'bmp:format=bmp3']);
magick('tga', ['-compress', 'none']);
magick('jpg', ['-background', '#202030', '-alpha', 'remove', '-quality', '82', '-sampling-factor', '4:2:0', '-strip']);
magick('webp', ['-define', 'webp:lossless=true'], { credit: 'LÖVE 12.0 only: LÖVE 11.5 refuses WebP' });
magick('qoi', [], { credit: 'LÖVE 12.0 only: LÖVE 11.5 refuses QOI' });
magick('hdr', ['-resize', '25%', '-define', 'hdr:format=rgbe']);

// ---- compressed textures and EXR, built by hand (headers only, one block each) ---------------------------------------------
const u32 = (...v) => Buffer.concat(v.map((x) => { const b = Buffer.alloc(4); b.writeUInt32LE(x >>> 0); return b; }));
const be16 = (...v) => Buffer.concat(v.map((x) => { const b = Buffer.alloc(2); b.writeUInt16BE(x); return b; }));
const zero = (n) => Buffer.alloc(n);
const dds = (four, w, h, data, dx10) => {
  const pf = Buffer.concat([u32(32, 4), Buffer.from(four), u32(0, 0, 0, 0, 0)]);
  const head = Buffer.concat([Buffer.from('DDS '), u32(124, 0x81007, h, w, data.length, 0, 1), zero(44), pf, u32(0x1000, 0, 0, 0, 0)]);
  return Buffer.concat([head, dx10 ? u32(dx10, 3, 0, 1, 0) : Buffer.alloc(0), data]);
};
const TEX = 'LÖVE reads the header and the block; a GPU may still refuse to draw the format';
put('textures/bc1.dds', dds('DXT1', 4, 4, zero(8)), { kind: 'image', recipe: 'hand-built DDS, BC1 (DXT1), 4x4, one block', credit: TEX });
put('textures/bc3.dds', dds('DXT5', 4, 4, zero(16)), { kind: 'image', recipe: 'hand-built DDS, BC3 (DXT5), 4x4, one block', credit: TEX });
put('textures/bc7.dds', dds('DX10', 4, 4, zero(16), 98), { kind: 'image', recipe: 'hand-built DDS, BC7 (DX10 header), 4x4, one block', credit: TEX });
put('textures/etc2.ktx', Buffer.concat([Buffer.from([0xab, 0x4b, 0x54, 0x58, 0x20, 0x31, 0x31, 0xbb, 0x0d, 0x0a, 0x1a, 0x0a]), u32(0x04030201, 0, 1, 0, 0x9278, 0x1908, 4, 4, 0, 0, 1, 1, 0), u32(16), zero(16)]), { kind: 'image', recipe: 'hand-built KTX 1, ETC2 RGBA8, 4x4, one block', credit: TEX });
put('textures/etc1.pkm', Buffer.concat([Buffer.from('PKM 10'), be16(0, 4, 4, 4, 4), zero(8)]), { kind: 'image', recipe: 'hand-built PKM, ETC1, 4x4, one block', credit: TEX });
put('textures/pvrtc.pvr', Buffer.concat([Buffer.from('PVR\x03', 'latin1'), u32(0, 3, 0, 0, 0, 8, 8, 1, 1, 1, 1, 0), zero(32)]), { kind: 'image', recipe: 'hand-built PVR 3, PVRTC 4bpp RGBA, 8x8, one level', credit: TEX });
put('textures/astc4.astc', Buffer.concat([Buffer.from([0x13, 0xab, 0xa1, 0x5c, 4, 4, 1, 4, 0, 0, 4, 0, 0, 1, 0, 0]), zero(16)]), { kind: 'image', recipe: 'hand-built ASTC, 4x4 blocks, one block', credit: TEX });

// OpenEXR, scanline, uncompressed, one HALF channel set R,G,B (2x1). Built by hand; never tiled (a tiled EXR aborts LÖVE).
{
  const attr = (name, type, data) => Buffer.concat([Buffer.from(name + '\0' + type + '\0'), u32(data.length), data]);
  const i32 = (...v) => Buffer.concat(v.map((x) => { const b = Buffer.alloc(4); b.writeInt32LE(x); return b; }));
  const f32 = (x) => { const b = Buffer.alloc(4); b.writeFloatLE(x); return b; };
  const chan = (n) => Buffer.concat([Buffer.from(n + '\0'), i32(1), u32(0), i32(1, 1)]);
  const chlist = Buffer.concat([chan('B'), chan('G'), chan('R'), Buffer.from([0])]);
  const box = i32(0, 0, 1, 0);
  const header = Buffer.concat([
    attr('channels', 'chlist', chlist), attr('compression', 'compression', Buffer.from([0])), attr('dataWindow', 'box2i', box),
    attr('displayWindow', 'box2i', box), attr('lineOrder', 'lineOrder', Buffer.from([0])),
    attr('pixelAspectRatio', 'float', f32(1)), attr('screenWindowCenter', 'v2f', Buffer.concat([f32(0), f32(0)])),
    attr('screenWindowWidth', 'float', f32(1)), Buffer.from([0]),
  ]);
  const half = (x) => (x === 1 ? 0x3c00 : 0);
  const px = Buffer.alloc(2 * 3 * 2); // B row, G row, R row; two pixels each
  [[0, 0], [0, 0], [1, 0]].forEach((row, c) => row.forEach((v, i) => px.writeUInt16LE(half(v), (c * 2 + i) * 2)));
  const base = 8 + header.length + 8;
  const body = Buffer.concat([i32(0), u32(px.length), px]);
  const table = Buffer.alloc(8); table.writeBigUInt64LE(BigInt(base));
  put('textures/scanline.exr', Buffer.concat([Buffer.from([0x76, 0x2f, 0x31, 0x01, 2, 0, 0, 0]), header, table, body]), { kind: 'image', recipe: 'hand-built OpenEXR, scanline, uncompressed, HALF RGB, 2x1', credit: 'a tiled EXR aborts LÖVE; this one is scanline' });
}

// ---- fonts -----------------------------------------------------------------------------------------------------------------
const VGA = { licence: 'CC-BY-SA-4.0', author: 'VileR (The Ultimate Oldschool PC Font Pack)', source: 'https://int10h.org/oldschool-pc-fonts/' };
const vgaSrc = join(root, 'public/fonts/ansiapps/WebPlus_IBM_VGA_8x16.woff');
put('fonts/vga.woff', readFileSync(vgaSrc), { kind: 'font', ...VGA, recipe: 'copied unmodified from public/fonts/ansiapps/', credit: 'IBM VGA 8x16 by VileR, CC BY-SA 4.0' });
copyFileSync(join(root, 'public/fonts/ansiapps/LICENSE.TXT'), join(proj, 'LICENSE-vga-font.txt'));
{
  // BMFont: the page is Painter's glyph atlas of the VGA font, copied byte for byte (no subsetting, no conversion; docs/fonts/CREDIT.md
  // there says how it was made from the unmodified font), and the .fnt is only a table of where each cell is: 16 by 16 cells of 8 by 16
  // in CP437 order, each given its Unicode code point.
  const { CP437 } = await import(vend('art-sprites/ansi.js'));
  const atlasPath = vend('art-sprites/docs/fonts/ibm-vga-8x16.png');
  const at = loadAtlas();
  let fnt = `info face="IBM VGA 8x16" size=16 bold=0 italic=0 charset="" unicode=1 stretchH=100 smooth=0 aa=1 padding=0,0,0,0 spacing=0,0 outline=0\n`;
  fnt += `common lineHeight=16 base=13 scaleW=${at.w} scaleH=${at.h} pages=1 packed=0\npage id=0 file="vga-bmfont.png"\nchars count=256\n`;
  const seen = new Set();
  for (let i = 0; i < 256; i++) {
    const id = CP437[i];
    if (seen.has(id)) continue; // a code point drawn twice in CP437 keeps its first cell
    seen.add(id);
    fnt += `char id=${id} x=${(i % 16) * 8} y=${Math.floor(i / 16) * 16} width=8 height=16 xoffset=0 yoffset=0 xadvance=8 page=0 chnl=15\n`;
  }
  fnt = fnt.replace('chars count=256', `chars count=${seen.size}`);
  put('fonts/vga-bmfont.png', readFileSync(atlasPath), { kind: 'image', ...VGA, recipe: 'copied unmodified from src/vendor/art-sprites/docs/fonts/ibm-vga-8x16.png', credit: 'BMFont page: the VGA glyph atlas; IBM VGA 8x16 by VileR, CC BY-SA 4.0' });
  put('fonts/vga-bmfont.fnt', Buffer.from(fnt), { kind: 'font', ...VGA, recipe: 'BMFont descriptor for vga-bmfont.png (CP437 cells mapped to Unicode)', credit: 'IBM VGA 8x16 by VileR, CC BY-SA 4.0' });

  // ImageFont: our own 3 by 5 hex font, drawn here (CC0). The top-left pixel is the separator colour; a separator column sits before
  // each glyph and after the last. Glyph string: "0123456789ABCDEF.:-+ ".
  const G = {
    '0': '111101101101111', '1': '010110010010111', '2': '111001111100111', '3': '111001111001111', '4': '101101111001001',
    '5': '111100111001111', '6': '111100111101111', '7': '111001010010010', '8': '111101111101111', '9': '111101111001111',
    A: '111101111101101', B: '110101110101110', C: '111100100100111', D: '110101101101110', E: '111100110100111', F: '111100110100100',
    '.': '000000000000010', ':': '000010000010000', '-': '000000111000000', '+': '000010111010000', ' ': '000000000000000',
  };
  const glyphs = Object.keys(G);
  const IW = 1 + glyphs.length * 4, IH = 5;
  const strip = new Uint8Array(IW * IH * 4);
  for (let y = 0; y < IH; y++) for (let x = 0; x < IW; x += 4) strip.set([255, 0, 255, 255], (y * IW + x) * 4);
  glyphs.forEach((ch, n) => {
    for (let y = 0; y < 5; y++) for (let x = 0; x < 3; x++) if (G[ch][y * 3 + x] === '1') strip.set([255, 255, 255, 255], (y * IW + 1 + n * 4 + x) * 4);
  });
  put('fonts/hex-imagefont.png', encodePng(IW, IH, strip), { kind: 'image', recipe: 'a 3x5 hex font drawn in make-assets.mjs; glyphs 0123456789ABCDEF.:-+ and a space, magenta separators', credit: 'ImageFont strip' });
}

// OpenType with CFF outlines (`OTTO`): Source Code Pro Regular by Adobe, SIL OFL 1.1, copied unmodified (the Reserved Font Name forbids
// changing it) from github.com/adobe-fonts/source-code-pro (branch `release`, OTF/SourceCodePro-Regular.otf), kept in tools/third-party/.
put('fonts/SourceCodePro-Regular.otf', readFileSync(join(here, 'third-party/SourceCodePro-Regular.otf')), { kind: 'font', licence: 'OFL-1.1', author: 'Adobe (Source Code Pro)', source: 'https://github.com/adobe-fonts/source-code-pro', recipe: 'copied unmodified from tools/third-party/', credit: 'Source Code Pro, Copyright Adobe, SIL Open Font License 1.1 (LICENSE-source-code-pro.txt)' });

// ---- sound: Composer -------------------------------------------------------------------------------------------------------
{
  const jingle = composer.compose('jingle', { kind: 'win', key: 'C', scale: 'major', seed: 'io-checker' });
  const wav = Buffer.from(composer.toWav(composer.render(jingle.score), 22050));
  const rec = `music-composer ${composerVersion} jingle {"kind":"win","key":"C","scale":"major"} seed io-checker`;
  put('audio/jingle.wav', wav, { kind: 'audio', recipe: rec + ', rendered at 22050 Hz' });
  const w = join(scratch, 'jingle.wav');
  writeFileSync(w, wav);
  sh('ffmpeg', ['-y', '-loglevel', 'error', '-i', w, '-c:a', 'vorbis', '-strict', '-2', '-ar', '44100', '-ac', '2', join(scratch, 'jingle.ogg')]);
  put('audio/jingle.ogg', readFileSync(join(scratch, 'jingle.ogg')), { kind: 'audio', recipe: rec + ', then ffmpeg native Vorbis' });
  sh('lame', ['--silent', '-b', '96', '--nohist', w, join(scratch, 'jingle.mp3')]);
  put('audio/jingle.mp3', readFileSync(join(scratch, 'jingle.mp3')), { kind: 'audio', recipe: rec + ', then lame 96 kbps' });
  sh('flac', ['--silent', '-f', '-o', join(scratch, 'jingle.flac'), w]);
  put('audio/jingle.flac', readFileSync(join(scratch, 'jingle.flac')), { kind: 'audio', recipe: rec + ', then flac' });
  const song = composer.compose('song', { form: 'loop', key: 'A', scale: 'minor', seed: 'io-checker' });
  put('audio/theme.mid', Buffer.from(composer.toMidi(song.score)), { kind: 'audio', recipe: `music-composer ${composerVersion} song {"form":"loop","key":"A","scale":"minor"} seed io-checker` });
}

// ---- animation data: Animator ----------------------------------------------------------------------------------------------
{
  const clip = animate('walk');
  put('data/walk.clip.json', Buffer.from(writeClip(clip)), { kind: 'data', recipe: `sprite-animator ${animatorVersion} walk {} (the exact animator-clip JSON)` });
}

// ---- from the sister checkout: what no tool here writes ---------------------------------------------------------------------
const corpus = join(root, '..', 'r3i6nos', 'corpus', 'files');
const MIT = { licence: 'MIT', author: 'r3i6nos contributors', source: 'r3i6nos corpus/files (made in code: Neumetik, Composer, scripts/corpus_formats.py)' };
if (existsSync(corpus)) {
  for (const [from, to, kind] of [
    ['music/trackers/protracker.mod', 'audio/protracker.mod', 'audio'], ['music/trackers/fasttracker2.xm', 'audio/fasttracker2.xm', 'audio'],
    ['music/trackers/impulsetracker.it', 'audio/impulsetracker.it', 'audio'], ['music/trackers/screamtracker.s3m', 'audio/screamtracker.s3m', 'audio'],
    ['video/clip.ogv', 'video/clip.ogv', 'video'],
    ['fonts/corpus.ttf', 'fonts/corpus.ttf', 'font'], ['fonts/corpus.ttc', 'fonts/corpus.ttc', 'font'],
  ]) put(to, readFileSync(join(corpus, from)), { kind, ...MIT, recipe: `copied from ${from}` });
} else {
  console.error('note: ../r3i6nos/corpus/files not found; the tracker modules, clip.ogv and corpus fonts were not made.');
}

// ---- shaders, data, Lua ----------------------------------------------------------------------------------------------------
const text = (rel, body, kind, recipe = 'written by hand for the IO Checker') => put(rel, Buffer.from(body), { kind, recipe });

text('shaders/tint.glsl', `// One file, both stages (LÖVE 11 and 12). GLSL 1: runs on desktop OpenGL, OpenGL ES (PortMaster, mobile) and WebGL.
extern vec4 tint;
extern number wobble;
vec4 position(mat4 transform_projection, vec4 vertex_position) {
  vertex_position.y += sin(vertex_position.x * 0.05) * wobble;
  return transform_projection * vertex_position;
}
vec4 effect(vec4 color, Image tex, vec2 texture_coords, vec2 screen_coords) {
  vec4 texel = Texel(tex, texture_coords);
  float grey = dot(texel.rgb, vec3(0.299, 0.587, 0.114));
  return vec4(mix(texel.rgb, vec3(grey) * tint.rgb, tint.a), texel.a) * color;
}
`, 'shader');

text('data/config.json', JSON.stringify({ name: 'IO Checker', version: '1.0.0', checks: ['input', 'video', 'sound', 'storage', 'system', 'assets'], limits: { fps: 55, lateFramePercent: 1 }, unicode: 'Löve ♥ 日本語' }, null, 2) + '\n', 'data');
text('data/levels.csv', 'id,name,tiles,seed\n1,Meadow,grass;stone,io-checker-1\n2,Lake,water;sand,io-checker-2\n3,"Ridge, high",stone;stone,io-checker-3\n', 'data');
text('data/strings.toml', '# TOML strings, as a localisation file would hold them.\n[en]\ntitle = "IO Checker"\nbye = "Goodbye"\n\n[fr]\ntitle = "Vérificateur E/S"\nbye = "Au revoir"\n', 'data');
text('data/tiles.tsx', '<?xml version="1.0" encoding="UTF-8"?>\n<tileset version="1.10" tiledversion="1.10.2" name="tiles" tilewidth="64" tileheight="64" tilecount="4" columns="4">\n <image source="../images/tiles.png" width="256" height="64"/>\n</tileset>\n', 'data', 'Tiled tileset for images/tiles.png');
text('data/map.tmx', '<?xml version="1.0" encoding="UTF-8"?>\n<map version="1.10" tiledversion="1.10.2" orientation="orthogonal" renderorder="right-down" width="4" height="2" tilewidth="64" tileheight="64" infinite="0" nextlayerid="2" nextobjectid="1">\n <tileset firstgid="1" source="tiles.tsx"/>\n <layer id="1" name="ground" width="4" height="2">\n  <data encoding="csv">\n1,1,2,2,\n3,3,4,4\n</data>\n </layer>\n</map>\n', 'data', 'Tiled map using tiles.tsx');
text('data/world.ldtk', JSON.stringify({ __header__: { fileType: 'LDtk Project JSON', app: 'LDtk', appVersion: '1.5.3' }, jsonVersion: '1.5.3', defaultGridSize: 16, levels: [{ identifier: 'Level_0', worldX: 0, worldY: 0, pxWid: 64, pxHei: 32, layerInstances: [] }] }, null, 2) + '\n', 'data', 'LDtk world with one empty level');
text('data/gamecontrollerdb.txt', '# SDL game controller mappings: guid,name,mappings,platform. This one is for a made-up pad, so the file is valid and harmless.\n030000004a6f79726964652000000001,Joyride IO Checker Virtual Pad,a:b0,b:b1,x:b2,y:b3,back:b6,start:b7,leftshoulder:b4,rightshoulder:b5,dpup:h0.1,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,leftx:a0,lefty:a1,rightx:a2,righty:a3,platform:Mac OS X,\n', 'data');
text('data/notes.txt', 'Plain text, read with love.filesystem.read and shown by the Assets page.\nA second line, with an accent: café.\n', 'data');

text('lib/json.lua', `-- A small JSON reader (decode only), so the Assets page can read data/*.json without a dependency. Written for the IO Checker.
local M = {}
local esc = { b = "\\b", f = "\\f", n = "\\n", r = "\\r", t = "\\t", ['"'] = '"', ["\\\\"] = "\\\\", ["/"] = "/" }
local function utf8char(cp) return utf8 and utf8.char(cp) or string.char(cp % 256) end
function M.decode(s)
  local i = 1
  local function ws() i = s:find("%S", i) or #s + 1 end
  local value
  local function str()
    local out = {}
    i = i + 1
    while true do
      local c = s:sub(i, i)
      if c == "" then error("unterminated string at " .. i) end
      if c == '"' then i = i + 1 return table.concat(out) end
      if c == "\\\\" then
        local n = s:sub(i + 1, i + 1)
        if n == "u" then
          out[#out + 1] = utf8char(tonumber(s:sub(i + 2, i + 5), 16) or 63)
          i = i + 6
        else out[#out + 1] = esc[n] or n i = i + 2 end
      else out[#out + 1] = c i = i + 1 end
    end
  end
  function value()
    ws()
    local c = s:sub(i, i)
    if c == "{" then
      local t = {}
      i = i + 1 ws()
      if s:sub(i, i) == "}" then i = i + 1 return t end
      repeat
        ws()
        local k = str() ws()
        if s:sub(i, i) ~= ":" then error("expected : at " .. i) end
        i = i + 1
        t[k] = value() ws()
        local d = s:sub(i, i) i = i + 1
      until d ~= ","
      return t
    elseif c == "[" then
      local t = {}
      i = i + 1 ws()
      if s:sub(i, i) == "]" then i = i + 1 return t end
      repeat
        t[#t + 1] = value() ws()
        local d = s:sub(i, i) i = i + 1
      until d ~= ","
      return t
    elseif c == '"' then return str()
    elseif s:find("^true", i) then i = i + 4 return true
    elseif s:find("^false", i) then i = i + 5 return false
    elseif s:find("^null", i) then i = i + 4 return nil
    else
      local num = s:match("^-?%d+%.?%d*[eE]?[+-]?%d*", i)
      if not num or num == "" then error("unexpected character at " .. i) end
      i = i + #num
      return tonumber(num)
    end
  end
  local v = value()
  return v
end
return M
`, 'script');

// LuaJIT bytecode (what LÖVE's own Lua loads): from the homebrew luajit when present.
{
  const src = join(scratch, 'hello.lua');
  writeFileSync(src, 'return "hello from LuaJIT bytecode"\n');
  try {
    sh('luajit', ['-b', '-s', src, join(scratch, 'hello.luac')]);
    put('lib/hello.luac', readFileSync(join(scratch, 'hello.luac')), { kind: 'script', recipe: 'luajit -b -s (LuaJIT bytecode; refused by standard Lua and by builds with a different LuaJIT)', credit: 'bytecode is tied to the LuaJIT build that wrote it' });
  } catch { console.error('note: luajit not found; lib/hello.luac not made.'); }
}

// A zip, for love.filesystem.mount. Stored with a fixed date so the bytes repeat.
{
  const files = [['pack/readme.txt', 'Inside pack.zip, mounted with love.filesystem.mount.\n'], ['pack/numbers.csv', 'n,square\n1,1\n2,4\n3,9\n']];
  const crc = (b) => { let c = ~0; for (const x of b) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); } return ~c >>> 0; };
  const locals = [], central = [];
  let off = 0;
  for (const [name, body] of files) {
    const data = Buffer.from(body), nm = Buffer.from(name), c = crc(data);
    const h = Buffer.alloc(30); h.writeUInt32LE(0x04034b50); h.writeUInt16LE(20, 4); h.writeUInt16LE(0, 6); h.writeUInt16LE(0, 8); h.writeUInt16LE(0, 10); h.writeUInt16LE(0x21, 12);
    h.writeUInt32LE(c, 14); h.writeUInt32LE(data.length, 18); h.writeUInt32LE(data.length, 22); h.writeUInt16LE(nm.length, 26);
    locals.push(h, nm, data);
    const d = Buffer.alloc(46); d.writeUInt32LE(0x02014b50); d.writeUInt16LE(20, 4); d.writeUInt16LE(20, 6); d.writeUInt16LE(0x21, 14);
    d.writeUInt32LE(c, 16); d.writeUInt32LE(data.length, 20); d.writeUInt32LE(data.length, 24); d.writeUInt16LE(nm.length, 28); d.writeUInt32LE(off, 42);
    central.push(d, nm);
    off += 30 + nm.length + data.length;
  }
  const cd = Buffer.concat(central), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50); end.writeUInt16LE(files.length, 8); end.writeUInt16LE(files.length, 10); end.writeUInt32LE(cd.length, 12); end.writeUInt32LE(off, 16);
  put('archives/pack.zip', Buffer.concat([...locals, cd, end]), { kind: 'archive', recipe: 'stored zip of two small text files, fixed date' });
}

// ---- the licence table ------------------------------------------------------------------------------------------------------
rows.sort((a, b) => (a[0] < b[0] ? -1 : 1));
writeFileSync(join(proj, 'assets.tsv'), '# path\tkind\tlicence\tauthor\tsource\trecipe\tnote\n' + rows.map((r) => r.join('\t')).join('\n') + '\n');
rmSync(scratch, { recursive: true, force: true });
let total = 0;
const walkDir = (d) => { for (const f of readdirSync(d)) { const p = join(d, f); statSync(p).isDirectory() ? walkDir(p) : (total += statSync(p).size); } };
walkDir(out);
console.log(`${rows.length} assets, ${(total / 1024).toFixed(0)} KiB, written to ${relative(root, out)}; licences in samples/io-checker/assets.tsv`);
