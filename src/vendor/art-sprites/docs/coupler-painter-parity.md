# Coupler's painter: the parity inventory (Phase 7, step one)

Read on 2026-10-29 from Coupler's Rust source (`src-tauri/src/`, read only from
here; nothing was copied, ideas only). Each row below is a row of the
`coupler-painter` kit (`kits/coupler-painter.js`), so `sprites kits` is the
migration list. Statuses: **covered** (a recipe draws it), **partial**,
**built-in** (a function, not a recipe), **gap**.

Coupler's own rules and docs (CLAUDE.md rule 11 "all ART is Coupler's painter's
first") are changed from a session in Coupler, never from here.

## What Coupler draws

| File | Lines | Draws |
|---|---|---|
| `paint.rs` | 218 | The input: `Scene` (world, room, name, zone, terrain, weather, time, `arc`, look, art), `Style` (fantasy, high), `Colors` (16, 256, true), `seed(world, room, again)` = FNV-1a. |
| `painter.rs` | 1906 | Rooms: 14 outdoor and 10 indoor terrains, sky by time and weather, sun and moon by `arc`, landforms, towns, ships, castles, caves, rooms in perspective in 6 materials, torches, braziers, banners; the `Canvas` with parts, lit cells, glyphs, dithers and gradients resolved per colour depth (`art_in`). |
| `portrait.rs` | 617 | A race beside a measuring stick, or a class with its tools, for character creation. |
| `face.rs` | 3080 | A creature's face, 25 by 24 pixels, from the Bestiary: head shape by race category, colours from words, moods, hurt in five steps, effects, small 16 by 8 faces. **Done in `creatureface` (Phase C); the painters are the engine's own.** |
| `item_art.rs` | 1938 | An item alone, 25 by 24: kind from its name, shape from numbers, material texture, wear, auras, slot in words. **Done in `thing` (Phase D); the painters are the engine's own.** |
| `figure.rs` | 198 | A figure laid on a place a shade darker, with a rim light from the place. |
| `fight_art.rs` | 334 | The Fight panel: the room with two fighters; frames (jolt, flash, slide in); a small strip. |
| `create.rs` | 333 | Words to a `Scene`, written as `.ans` with SAUCE in each of the three colour depths. |

## What the engine does now

`recipes/vista.js` (`vista`, ANSI only) takes `{ terrain, time, weather, style,
arc, colors, seed, columns, rows }` and draws all 24 terrains, the four times,
the 14 weather kinds, both styles and 16 or 256 colours; indoors and underwater
show nothing of the sky; `says` returns the picture's facts in words.
`roomSeed(world, room, again)` is Coupler's FNV-1a. The pictures are simpler than
Coupler's: no per-terrain detail such as roads' perspective, jungle canopy or
spaceport pads beyond a sketch, no `Part` layers, no per-pixel light.

## What is outside this engine's control

1. **Coupler's side of the move.** How Coupler runs the engine (the webview's
   JavaScript with Rust passing the scene, or another route), mapping its
   `paint::Scene` to the recipe's params, deleting `painter.rs`, and the
   changes to its rules 6, 11 and 12, are decisions and edits for a session in
   Coupler.
2. **Parity is judged by eye.** The output is not byte-identical to the Rust
   painter's. Someone has to put the same scenes side by side on a contact
   sheet and accept the differences.
3. **The Bestiary, items and EXAMINE text.** Faces, item pictures and
   portraits read creature and item data (`bestiary::MobEntry`, `Thing`,
   `materials.tsv`, `effect_looks.tsv`) that only Coupler has. The engine
   needs those tables handed over as data (or the recipes designed around a
   smaller input) before they can be ported.
4. **`true` colour.** Done in 1.14.0: `colors: 'true'` on every picture; the cells carry exact RGB beside their palette
   indexes (`cells.tc`; the index is the nearest of the 256). Coupler's viewer reads `tones(cells)` or `cells.get(x, y).fgRgb` and `bgRgb`.
5. **The 3 colour depths of `create.rs`** (16, 256, true files kept side by
   side) need Coupler's asset storage; the engine can write each file.
6. **Accessibility words** beside every picture are Coupler's UI; the engine
   offers `says`.

## Where it stands (Phase G, 2026-10-29)

`sprites kits` reads no gap row in `coupler-painter`. Four rows stay `partial` on purpose, because only Coupler's eye can close them:
`terrains-outdoor`, `terrains-indoor`, `time-of-day` and `weather` (the room picture is simpler than the Rust one in the details each row
names). `scene-input`, `accessibility-words` and `ansi-file` are covered; what they leave to Coupler's side is written in their rows.
The matrix lint (`node tools/vista-matrix.mjs`) reads 39 of 260 pictures with findings, all `ramp`; the 123 of Phase A are down to those.
`docs/coupler-handback.md` has the per-gap table and the changed outputs.
