// Pack export (Node only: it writes files and hashes them). A pack is a list of drawings in, a folder out: every frame, clip and layer as
// a PNG, one atlas of the stills, a manifest that says everything a compositor needs, and a hash list for visual regression tests.
//
//   { "name": "my-pack", "entries": [ { "id": "game.char.guardian", "recipe": "character", "params": { "loadout": "guardian" }, ...anything else } ] }
//
// `id` and every field other than `recipe` and `params` are the game's: they come back under `meta` untouched (which slot, tags, which
// style profile), so the game's names never live inside the engine. Pixel art only. Nothing in the output varies from run to run: no
// timestamps, and the hashes are of the pixels, not of the PNG bytes (a zlib version may compress the same pixels differently).
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Pixels } from './pixels.js';
import { pixelsToPng } from './png.js';
import { draw, RECIPES } from './recipes/index.js';
import { loadAtlas } from './atlas.js';
import { resolveLook } from './look.js';
import { ENGINE_VERSION } from './version.js';
import { KITS } from './kits/index.js';

export const PACK_SCHEMA = 'art-sprites.pack/1';
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const pixelHash = (px) => sha(Buffer.from(px.toRgba(1)));
const safe = (id) => String(id).replace(/[^a-z0-9.-]/gi, '_');

/** Write the pack for `list` into `dir`; returns the manifest. `creator` names who made the pixels (default: this project's recipes). */
export function writePack(list, dir, { creator } = {}) {
  const entries = list?.entries;
  if (!Array.isArray(entries) || !entries.length) throw new Error('a pack list needs an "entries" array with at least one entry');
  mkdirSync(dir, { recursive: true });
  const seen = new Set(), records = [], stills = [], goldens = {};
  for (const [i, entry] of entries.entries()) {
    const { id: given, recipe, params = {}, ...meta } = entry, where = `entry ${i}${given ? ` "${given}"` : ''}`;
    if (!RECIPES[recipe]) throw new Error(`${where}: unknown recipe "${recipe}"`);
    if (RECIPES[recipe].ansiOnly) throw new Error(`${where}: recipe "${recipe}" is ANSI only; a pack holds pixel art`);
    const art = draw(recipe, params, RECIPES[recipe].needsAtlas ? { atlas: loadAtlas() } : {}), id = given ?? art.id;
    if (seen.has(id)) throw new Error(`${where}: duplicate id "${id}"`);
    seen.add(id);
    const name = safe(id), files = { frames: [], clips: {}, layers: {} }, hashes = [];
    const put = (key, frames, tag) => frames.map((px, k) => { const file = `${name}.${tag}.${k}.png`; writeFileSync(join(dir, file), pixelsToPng(px)); const h = pixelHash(px); hashes.push(h); return { file, sha256: h }; });
    files.frames = put('frames', art.frames, 'frames');
    for (const [n, c] of Object.entries(art.clips ?? {})) files.clips[n] = { fps: c.fps, loop: c.loop, events: c.events, anchors: c.anchors ?? null, frames: put(n, c.frames, `clip-${n}`), reduced: put(n, c.reduced ?? [], `clip-${n}.reduced`) };
    for (const [n, fr] of Object.entries(art.layers ?? {})) files.layers[n] = put(n, fr, `layer-${n}`);
    const reduced = art.reduced ? put('reduced', art.reduced, 'reduced') : [];
    // The world data a game reads beside the pixels: the footprint and occlusion of a prop, the props and cells of a dressed level, the legend of a map; a boss in parts and a fog mask as files.
    const data = {}; for (const k of ['footprint', 'occlusion', 'states', 'props', 'blocked', 'walkable', 'unreachable', 'entry', 'tile', 'legend', 'cell', 'origin', 'hotspots', 'hotspot', 'grid', 'window', 'weaponAttack']) if (art[k] !== undefined) data[k] = art[k];
    if (art.parts) files.parts = art.parts.map((p) => ({ id: p.id, x: p.x, y: p.y, joint: p.joint, ...put(p.id, [p.frame], `part-${p.id}`)[0] }));
    if (art.mask) files.mask = put('mask', [art.mask], 'mask')[0];
    goldens[id] = sha(hashes.join(''));
    const f = art.frames[0]; stills.push({ id, px: f });
    records.push({
      id, recipe, params, look: resolveLook(recipe, params), meta, style: 'pixel', projection: art.projection ?? null, size: { w: f.w, h: f.h },
      alt: art.alt, anchors: art.anchors ?? null, bounds: art.bounds ?? null, paletteGroups: art.paletteGroups ?? null, fps: art.fps, loop: art.loop,
      reduced, files, data, golden: goldens[id],
      modelledOn: KITS.filter((k) => k.subjects.some((s) => s.recipes?.some((r) => r.recipe === recipe))).map((k) => k.id),
    });
  }
  // The atlas: the first frame of every entry in rows, 1 pixel apart, at most 512 wide.
  const MAX = 512; let x = 0, y = 0, rowH = 0, atlasW = 0; const rects = {};
  for (const { id, px } of stills) {
    if (x && x + px.w > MAX) { x = 0; y += rowH + 1; rowH = 0; }
    rects[id] = { x, y, w: px.w, h: px.h }; x += px.w + 1; rowH = Math.max(rowH, px.h); atlasW = Math.max(atlasW, x - 1);
  }
  const atlas = new Pixels(atlasW, y + rowH);
  for (const { id, px } of stills) atlas.blit(px, rects[id].x, rects[id].y);
  writeFileSync(join(dir, 'atlas.png'), pixelsToPng(atlas));
  for (const r of records) r.atlas = rects[r.id];
  const manifest = {
    schema: PACK_SCHEMA, name: list.name ?? 'pack', engine: ENGINE_VERSION, palette: 'DB32',
    atlas: { file: 'atlas.png', w: atlas.w, h: atlas.h, sha256: pixelHash(atlas) },
    source: { creator: creator ?? 'Project recipes (sprites/)', packName: 'internal' },
    // Drawn by this project's recipes from a seed: nothing third party is in the pixels.
    license: { licenseName: 'Project-created', status: 'approved', attributionRequired: false, evidenceNotes: 'Generated by deterministic recipes from the DB32 palette; no third-party pixels.' },
    entries: records,
  };
  writeFileSync(join(dir, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  writeFileSync(join(dir, 'goldens.json'), JSON.stringify({ engine: ENGINE_VERSION, hashes: goldens }, null, 2) + '\n');
  return manifest;
}
