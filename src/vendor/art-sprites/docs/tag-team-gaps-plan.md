# Tag Team: gaps in Painter, and how to fill them

**Written 2026-10-30**, from `games/Tag Team Visuals 2.3.md` (the catalogue,
sections 2 to 12) read against what Painter 1.20.1 draws. This is the
first list for the `games/` folder; later games get their own file here.
Same shape as `docs/coupler-gaps-plan.md`: a gap, what exists, what to
build, in an order.

## How to read it

Tag Team's document is half art catalogue and half game architecture
(manifest loader, compositor, chart panels, Museum data, admin
dashboard). Only the art that a recipe can draw is Painter's. The rest is
listed at the end so it is not mistaken for a gap.

Painter rule 6 holds throughout: **which look a Tag Team class has, which
theme a floor uses and every art-direction call stay in the game.**
Painter gains generic recipes, params and a pack export; Tag Team's
`fantasy_pixel_v1` profile, manifests and recipes-by-ID live in Tag Team.
Rules 1 to 5 hold too: seeded, dependency-free, DB32, nothing fetched.

## Phase 1 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T1 sizes and body profiles | **Part done.** `bodyProfile` (`medium`, `small`, `slim`, `broad`) is in. `large` is left out: the full-height body already fills 32 by 32. `iconSize` 24 and 32 and tile size 32 are **not done**: a larger icon or tile needs the shape redrawn with more detail, not scaled, so they move to T10, T12 and T15 where those shapes are redrawn. |
| T2 anchors | **Done** for front, side, back and quarter, every pose, clip frame and `motion` clip. The four top-down facings come with T23. |
| T3 palette groups and dye | **Done** for character and creature (`paletteGroups`, `dye`); `recolor` and `recolorArt` work on any drawing. Items have no groups yet (they take `materials`, which already names ramps). |
| T4 pack, frozen looks | **Done.** `sprites pack`, `resolveLook` (character, creature, item; other recipes come back as given with `explicit: false`), `ENGINE_VERSION`. Still the game's: the manifest field names it wants (they arrive under `meta`) and where it stores looks. |
| T5 placeholders | **Done.** |
| T6 alt and reduced | **Done.** `alt` is exact for character and creature and a plain phrase from the recipe id and params for the rest; `lintArt` guards it. Effects take `flash` and `density`. |

No existing pixel changed (4,280 drawings compared with 1.20.1). Next is Phase 2, which starts with T23 (the top-down characters).

## Phase 2 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T23 characters | **Done** (`view: 'top'`, four facings, every loadout, pose, clip, dye and profile; class silhouettes pass `lintSilhouettes`). **Not done here, left for phase 3:** top-down creatures, props, walls, buildings, tile checks. |
| T7 | **Part done.** `armor` families, boots (`bootStyle`), gloves, belt, shoulders, eyewear, 8 hair styles, 4 facial hairs, `appearance`. Not done: bracers, neck, separate cloak and tabard, scars and jewellery, the `helm` variants. Hair and eyewear are drawn from anchors, so they are a first pass: judge them on a sheet before the game builds on them. |
| T8 | **Core done** (`turn`, `block`, `cast`, `heal`, `celebrate`, `recall`). Expanded list and weapon-specific attacks are phase 4. |
| T9 | **Mostly done** (three sizes, nine expressions, two states, mask and safe box). Class emblems not done. |
| T11 | **Done** (`overlay`, `pixelText`, digits from the atlas). Icons are still 16 by 16, so a 32 overlay lies over a 2x icon until T10 redraws them. |
| T12 | **Done** (`skillicon`, ten panel icons). The `motif` list is 40 approved glyphs; new skills' pictures are new glyphs. |
| T19 | **Done for target, telegraph, damage.** The rest of T19 (shield, buff, projectile, trail, rune, portal, level-up, loot) is later. |

Painter's `drawNumber` and the overlay `count` need the VGA atlas passed in (`loadAtlas` from `art-sprites/atlas`): the main entry cannot read a file.

## Phase 3 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T23 | **Mostly done.** Top-down creatures (nine plans of their own, the rest sharing the nearest), roof plans for all seven buildings, top-down props, walls as flat tops. Not done: a plan of its own for fish, horse, griffon and the others that share one; `furniture` from above. |
| T13 | **Part done.** Family grammar (17 families, seeded, refuses nonsense), five new bodies, bandit gear, elite and boss rank, a boss in two parts. Not done: boss canvases above 64, more than two parts, more bodies (swarm variants, plants beyond the treant). |
| T14 | **Done.** `telegraph`, three attacks, `stagger`, `block`, `enrage`, `summon`, `retreat`, `dissolve`, `phase`; status looks and intent markers as layers. Intent and target markers beside the creature (with T19's rings) are the game's to place. |
| T15 | **Part done.** The 47-shape autotile, 14 parts, 8 biomes, 16 and 32 pixel tiles. Not done: ladders, cliffs, chasm and water edges, and the fungal, arcane, void and mechanical biomes (phase 5). |
| T16 | **Core done.** 26 props with states, footprint, occlusion and clips. Not done: gates, harvest states beyond `ripe` and `picked`, drifting dust, machinery. |
| T17 | **Done.** `dress` from a game grid, templates, walkability kept, `lintDress`. Props are placed on the 32 grid only. |
| T18 | **Done.** `map` with fog of war and no hint. Not done: a text depth badge (the depth is pips), floor numbers. |

`pack` writes the new world data. Phase 4 is T20 (town), T21, T10, T7's armour families and T8's expanded list.

## Phase 4 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T20 | **Mostly done.** One grammar for the fifteen services and a marker (six shells, four themes, four states), `town` with hotspots and a walkable grid, six interiors. Not done: building states drawn as animation, a town path network other than the lattice. |
| T21 | **Done** for `exhibit` (five styles, empty, occupied, new, placeholder) and `award` (plaque, podium, ribbon, seal, medal, rank). Achievement tiers are the medal tiers. |
| T10 | **Part done.** Eleven shapes, `material` (five families with looks), `currency`. **Not done:** 24 and 32 pixel icons (a redraw, not a scale). |
| T7 | **More done:** tabard, bracers, neck, scar, jewellery, helm styles. Cloak apart from the back slot is still the back slot. |
| T8 | **Done:** the expanded list and the weapon attacks (`shoot`, `thrust`, `slam`; `cast` for staff and wand). |

Phase 5 is T15 rest, T16 rest and T13 rest.

## Phase 5 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T15 | **Done** for the list: ladders, cliffs, chasm and water edges (`ladder-down`, `ladder-up`, `cliff`, `chasm`, `water`), and the fungal, arcane, void and mechanical biomes. Judge the cliff and ladder-up on a sheet before the game builds on them. |
| T16 | **Mostly done.** Gate, mushroom, harvest stages (`seedling`, `growing`, `ripe`, `picked`) with a clip, drifting dust, gears, piston, steam vent. Not done: more machinery (conveyors), a gate that opens as a tile in `dress`. |
| T13 | **Mostly done.** Boss canvases 96 and 128, 2 to 5 segments, plant family (treant, flytrap, vine), three swarm kinds. Not done: canvases above 128, plans of their own for the bodies that share one from above. |

## Phase 6 status (built 2026-10-30, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T10 / T1 | **Done:** `item` takes `iconSize` 16, 24 or 32. The 24 and 32 icons draw the shape's own strokes again on the finer grid (not a scale), shaded in wider bands, with a longer glint, more grain and a bigger tier mark. Not done: hand-tuned per-shape detail (a rune on a hilt, studs on a shield) beyond what the 16 pixel shape has. |
| T16 | **Done:** `conveyor` (four directions, a `run` loop). `gate` as a tile: part `gate` (`closed`, `open`, `locked`) and `=` in a `dress` grid with `gates`. |
| T15 | **Reviewed:** the cliff reads from all four sides; the lit hatch of `ladder-up` was a hot orange that shouted over the room, now pale sand (its goldens changed). |
| Left | Boss canvases above 128, plans of their own for the shared top-down bodies, building states as animation, a town path network other than the lattice, a cloak apart from the back slot. **All five done afterwards** (see the changelog, "Tag Team leftovers"): canvases 192 and 256, eleven new top-down plans, `animate` on a building, `paths` (`rows`, `cross`) on a town, and `cloak`. |

## Phase 7 status (built 2026-10-31, unreleased; see CHANGELOG)

| Gap | State |
|---|---|
| T19 | **Done:** the eleven effects that were left (`shield`, `shieldhit`, `buff`, `debuff`, `arrow`, `trail`, `rune`, `portal`, `levelup`, `crit`, `loot`). |
| T9 | **Done:** `emblem` for every loadout. |
| T18 | **Done:** `label` (floor number, depth badge in text) with the VGA atlas. |
| T23 | **Done:** `furniture` with `view: 'top'`. |
| Left | Hand-tuned per-shape detail on the 24 and 32 pixel item icons (a rune on a hilt, studs on a shield): an art pass to judge on a sheet, not a gap in the engine. |

## What already covers Tag Team

| Tag Team need (section) | Painter today |
|---|---|
| Five class silhouettes, paper-doll layers (2.1B, 5.2) | `character`: `guardian`, `berserker`, `rogue`, `scholar`, `healer` plus 21 more loadouts; 4 views, 16 poses, 3 back slots, layers by slot, `idle` `walk` `attack` `hit` `defeat` clips |
| Weapons, shields, armour pieces (2.2) | `item`: 90 shapes by 4 tiers; the same shapes are the worn weapons |
| Enemy families (2.3A) | `creature`: 58 presets (undead, beasts, insects, slimes, elementals, dragons, golems, fey, demons), 25 races, `mutate()` |
| Dungeon tiles and props (2.4) | `tile` (6 dungeon biomes, floor and wall), `prop` (64, incl. doors, stairs, traps, chests), `furniture` (24), `room` (12) |
| Status and skill glyphs (2.10) | `icon`: 57 glyphs, 14 of them statuses |
| Combat effects (3.4) | `effect`: 14, four frames, `reduced` frame set |
| Town buildings (2.6) | `building`: 7 kinds; `room` smithy, library, shrine, tavern, shop, inn |
| Faction and clan marks (2.12) | `sigil` |
| Character and portrait bust (2.1D) | `portrait` (20 by 20 bust) |
| Cache, seeds, motion (4.4, 8.3, 3.1) | `createLookCache`, `rng`, `motion` presets (14 types) |
| Provenance and lint (7.2, 10) | `sprites export` writes a `Project-created` record; `lint` and `lintFrames` |
| Future ANSI or mono profile (1.2) | every recipe also draws as `ansi` and `text` |

Everything below is what is left.

---

## The gaps

Size: **S** a day or less, **M** a few days, **L** a week or more.
**MVP** is Tag Team's own checklist (section 12); the rest follows its
phases V2 to V8.

### A. The contract between Painter and a compositor

These come first: they change what every later recipe has to return.

**T1. Icon sizes and body profiles** (8.4, 5.4, Appendix A). MVP. S.
- **Decided (D1): the character canvas stays 32 by 32** (40 wide for a
  swing). Tag Team's 32 by 48 and 48 by 64 were "test starting points", and
  the reasons for staying are in `docs/tag-team-handback.md`.
- Build, inside 32 by 32: a `bodyProfile` param on `character`
  (`humanoid_medium` today's; `humanoid_small`, `humanoid_large`, `slim`,
  `broad`) with the joints scaled by a share of the canvas, the way the
  creature size table already does; an `iconSize` of 16, 24 or 32 on `item`
  and `icon`; tile size 16 or 32 on `tile`. Pixels stay logical;
  nearest-neighbour scaling is the game's job.
- If a phone test later shows 32 by 32 too small, a taller profile is added
  as a new `bodyProfile`, never by changing the default.

**T2. Named anchors on every art** (5.3). MVP. S.
- Today `anchors` is `{ feet }`. Tag Team wants `root`, `head`, `neck`,
  `shoulder_left/right`, `hand_main`, `hand_off`, `waist`, `back`,
  `foot_left/right`, `weapon_grip`, `shield_grip`, `effect_origin`,
  `shadow_center`.
- Build: `character` already knows its joints per pose and frame; return
  them per frame (`anchors.frames[i]`). Creatures return `head`,
  `effect_origin`, `shadow_center`, `feet`; props and buildings return
  `root` and a footprint. A lint rule: anchors inside the canvas.

**T3. Palette groups and dye** (5.4, 6.1, 4.1). MVP for classes. M.
- Tag Team recolours within a style profile (skin, cloth, leather, metal,
  hair; faction and cosmetic dyes; palette variants of one enemy).
  Painter has ramps and `Pixels.swap` for one index, but no ramp-aware
  swap that keeps each shade's step.
- Build: `recolor(art, { cloth: 'blue' })` in `shade.js`, and every
  layered recipe reports its `paletteGroups` (which ramp each layer
  uses). Same ramp length contract, tested.

**T4. Pack export: manifest, hashes, atlas, version** (6, 7.2, 10.2). MVP. M.
- `sprites export` writes one art with a provenance record. Tag Team needs
  a whole **pack**: a list of recipe, params, seed entries in, one atlas
  PNG and a manifest out. The record carries no content hash and no
  engine version.
- Build: `sprites pack <list.json> --dir d`: per entry the PNGs, layer
  files, clips with events and `reduced`, anchors, paletteGroups, size, a
  `sha256` of the pixels, the `art-sprites` version, the licence block, and
  a golden-hash list for Tag Team's visual regression (10.2). The list
  and the manifest field names come from Tag Team (rule 6); Painter
  emits them.
- **Frozen looks** (4.4, 6.5, invariant 7). Painter's rule 1 is
  same-input-same-pixels *within a version*. A new default in 1.22 can
  change a saved character. Build `resolveLook(spec)`: returns every
  param explicit (loadout expanded, defaults filled) so Tag Team stores
  the resolved spec, plus a changelog line "changes existing output" that
  the pack can check against. Recommend for **historical Museum
  exhibits**: bake the PNG into the manifest rather than redraw.

**T5. Silhouettes and placeholders that do not leak** (5.5, invariant 8). MVP. S.
- `Pixels.silhouette` exists; no recipe. Tag Team needs a character
  silhouette fallback, an enemy-family silhouette, an item category
  placeholder, a generic building marker, and "undiscovered" shapes.
- Build: recipe `placeholder` (`kind`: character, family, item, building,
  exhibit, unknown). The "unknown" ones are drawn by size class only, never
  from the real creature or item, so they cannot reveal content.

**T6. Alt text and reduced motion on every art** (9, 3.6). MVP. S.
- Only `effect` and the character `attack` clip carry `reduced`; no art
  carries words.
- Build: every recipe returns `alt` (a short factual phrase: "rogue, front,
  hooded, dagger") and every clip gets `reduced` (one still frame) and
  `events`. `lint` fails an animation with neither. Effects take
  `flash: false` and `density` for the no-flash and low-effects settings.

### B. Characters

**T7. Equipment slots, armour families, hair, cosmetic override** (2.1C, 2.2B, 5.2). MVP basics, V2 rest. L.
- Today: layers `shadow back legs torso offhand body headgear weapon`;
  torso is `tunic`, `plate` or `robe`; headgear six kinds; hair is a
  colour only. Missing as layers: gloves and bracers, boots, belt,
  shoulders, neck (amulet), separate cloak and tabard, mask and goggles,
  hair styles and facial hair, scars and markings, jewellery.
- Build in order: boots, gloves, belt, shoulders (they use the existing
  anchors); torso families `cloth leather chain scale plate` (a texture on
  the same torso shape, drawn as DB32 texture with dither);
  8 hair styles and 4 facial hairs; headwear `mask`, `goggles`, `helm`
  variants. Each as an optional layer in `layers`, so a missing one never
  hides the character.
- **Cosmetic override** (invariants 3, 4): Painter already draws whatever
  spec it is given. Add `appearance` (what is shown) beside `gear` (what is
  equipped) in `resolveLook`, so the override is a different param, never a
  different recipe. The persistence is Tag Team's.

**T8. The semantic clip set** (3.1). MVP core, V2 expanded. M.
- Today: `idle walk attack hit defeat`, plus anything Animator sends.
- Build the core minimum as clips with `events` and `reduced`: `turn`,
  `block`, `cast`, `heal`, `celebrate`, `recall`. Then the expanded list:
  `dodge`, `parry`, `charge`, `stun`, `potion`, `buffed`, `lowhealth`,
  `levelup`, `tagin`, `tagout`, `equip`. Weapon-specific attacks (`shoot`
  for bow and sling, `thrust` for spear, `slam` for hammer and axe2,
  `cast` for staff and wand), chosen by weapon shape. All from the
  existing joints and poses (`cast`, `brace`, `reach` exist).

**T9. Portrait set and frames** (2.1D). MVP for two sizes. M.
- Today one 20 by 20 bust of the front view.
- Build: `portrait` sizes `party` (small), `status` (compact), `card`
  (large); a `defeated` and a `recalling` state; a mask and safe-area
  returned as data; class emblems from `sigil` plus `icon`. The
  expression set (neutral, joy, pain...) reuses `face`'s nine.

### C. Items, skills and icons

**T10. Item shapes, materials and currency** (2.2A, 2.9A). V2. M.
- Missing shapes: crossbow, kite shield, tower shield, round shield,
  magical shield, ceremonial sword, double-headed axe by name (axe2
  exists), sheaths and quivers as items, tent, bedroll, rope (the camping
  kit row is partial).
- Missing families in 2.9A: ores, herbs, hides, monster parts, crystals
  (partial in `thing`); currency denominations; keys and fragments;
  recipes and plans; quest tokens.
- Build: add the shapes to `items.js`; a `material` family
  (`ore herb hide part crystal`) with a `variant` index; coin
  denominations as `tier`-like steps. All at the 16, 24 and 32 sizes
  from T1.

**T11. Rarity, state overlays and pixel digits** (2.2D, 2.9B). MVP (rarity, new, equipped). M.
- Today `item` has four tiers and no frames.
- Build: recipe `overlay` with `rarity` (common to legendary: frame
  **shape** as well as colour, per section 9: colour is never the only cue),
  `new`, `equipped`, `locked`, `bound`, `crafted`, `enchanted`, `compat`
  markers, comparison arrows, a socket marker, a stack count and a
  durability pip row. The digits need a pixel glyph source: **use the VGA
  atlas rule 8 already sets** (render digits from the shipped PNG atlas,
  not a typed table). That is also T22's text.

**T12. Skill icon grammar and panel icons** (2.10, 2.11C). MVP (basic class skills). M.
- Today 57 glyphs, with statuses. Missing: targeting icons (self, ally,
  enemy, party, area, line, cone), skill categories (attack, defence,
  mobility, control, heal, cleanse, buff, debuff, summon, utility), cost
  and cooldown markers, active, passive and triggered markers, locked and
  unlocked, and the ten carousel panel icons (actions, chat, party,
  character, inventory, map, chronicle, charts, research, help).
- Build: `skillicon` recipe from `category` + `motif` + `target` + `tier`
  on the `icon` plate, bounded to approved motifs (4.1, 10 invariant 15).
  Cooldown as a sweep overlay frame. Panel and chart icons as new glyphs.

### D. Enemies and bosses

**T13. Family grammar, missing families, boss scale** (2.3, 4.5). MVP (a few silhouettes), V3 rest. L.
- Covered: humanoids, undead, goblins and orcs, beasts, insects, slimes,
  elementals, demons, dragons, spirits, golems. Missing: **mimic**,
  **fungus** and plant creatures beyond the treant, **mechanical
  constructs** (a golem is stone), **swarms**, **aberrations** beyond the
  beholder, and **bandit** variants with gear.
- Build: `family` presets so a generated enemy has a valid silhouette: a
  family names allowed parts (head, appendages, overlays) and `creature`
  composes from them by seed, rejecting nonsense anatomy. Add the missing
  bodies to `BODIES`. **Elite and boss treatment** (a frame, a crest, a
  size step) and a **canvas larger than 32** (48, 64, and a multi-part
  boss: `parts` returned with offsets so the game can animate segments).

**T14. Enemy clips and state overlays** (2.3C, 3.2). MVP (telegraph). M.
- Today creatures have `idle`, `hit`, `defeat`.
- Build: `telegraph` (wind-up with the direction the blow comes), `attack`
  (melee, ranged, magic), `stagger`, `block`, `enrage`, `summon` (spawn),
  `retreat`, `dissolve`, `phase` (boss), all with `events` and `reduced`.
  Status overlays (poison, burn, freeze...) as a transparent layer over a
  creature using the `icon` and `effect` look, plus intent and target
  markers (with T19).

### E. Dungeon and combat scene

**T15. Theme kits and autotiling** (2.4A, 2.4C). MVP (one kit), V3. L.
- Tag Team lists 11 kits; Painter has 6 dungeon biomes (crypt, cave,
  forge, flooded, frozen, overgrown) with a floor and a plain wall.
  Missing kits: ancient stone, fungal, volcanic, arcane or library,
  corrupted or void, mechanical. Missing tile roles: wall corner,
  inner and outer corner, junction, pillar, broken wall, doorway and arch,
  stairs and ladder as tiles, cliffs and pits, chasm edge, bridge,
  water-edge, lava, toxic pool, cracks, rubble.
- Build: `tile` `kind: 'dungeon'` takes `part` from a full autotile set
  (a 47-tile blob or the 16-tile edge set, decide in the build) and a
  `hazard` kind; five new biomes; a light and shadow overlay tile. The pixel profile stays DB32;
  the ANSI form of each kit may use the broader palette for texture.

**T16. Props and animated environment** (2.4B, 3.3). MVP (door, exit, chest, torch). M.
- Not in `prop` or `furniture`: crate, urn, brazier, statue, coffin,
  lever, switch, pressure plate, web, ore node, banner (a `cloth` exists),
  bookshelf wall, lectern, scroll rack, harvestable plant. Open and closed
  **states** for chest, door and gate; trap armed, triggered, disarmed.
- Build: add the props with a `footprint` and `occlusion` returned (6.1:
  collision stays the game's). Looped clips for torch and fire flicker,
  lava, crystal glow, portal, drifting dust, machinery, trap activation,
  door and chest opening, discovery reveal.

**T17. Room dressing from game geometry** (2.4B, 4.5 room grammar). MVP (two templates). L.
- `room` and `scene` draw a fixed layout from a seed. Tag Team's rule is
  that the engine decides geometry and walkability and the art follows
  it, so Painter must take a grid in, not choose one.
- Build: `dress` recipe: `{ theme, grid }` where `grid` is a rows array of
  floor, wall, door, exit, pit, hazard cells from the game; returns the
  composed picture, the props it placed with footprints, and a check that
  no art implies a walkable path the grid lacks (`unreachable` already
  tested for rooms). Templates as parameters the game supplies, not
  Painter's choice: arena, corridor, junction, treasure, trap, shrine,
  safe, merchant, boss, exit, secret.

**T18. Pixel map and fog of war** (2.4D). MVP (minimal map). M.
- `mappane` is ANSI only.
- Build: `map` recipe from a game-supplied list of cells with
  `state` (unknown, visible, remembered, explored): cells, outlines, party
  marker, objective, boss, safe-zone, trade, exit and teleport markers,
  floor progress, depth badge, frame, mini and expanded sizes, fog mask.
  **Unknown cells are drawn with no hint** (invariant 8). Text legend and
  `alt` returned with it.

**T19. Combat feedback effects** (2.5, 3.4). MVP (telegraph, target, damage). M.
- Today 14 effects: slash, burst, sparkle, heal, fire, ice, bolt, curse,
  poison, necromancy, illusion, blessing, disease, summon.
- Missing: shield or barrier and its impact, buff and debuff apply and
  expire, projectile and arrow paths, weapon trail, magic circle or rune,
  portal, level-up, critical emphasis, **area telegraph shapes** (circle,
  line, cone, rectangle, with a pattern fill, not colour alone),
  **target and selection rings**, turn highlight, shadow blob, party seat
  marker, hit flash, loot reveal.
- Build: new `effect` ids and a `telegraph` recipe with `shape`, `size`.
  Floating numbers and the crit text use T11's digit source.

### F. Town, Museum and community

**T20. Building grammar and town map** (2.6, 4.5 building grammar). MVP (Portal Tower, Registry, Atelier, Training Hall, Archive, generic marker), V4 rest. L.
- `building` draws 7 kinds; Tag Team has 15 services.
- Build as **one grammar, not 15 drawings**: `{ shell, roof, sign, serviceIcon, theme }`
  where `shell` is the footprint (cottage, hall, tower, vault, market
  stall, wing) and `serviceIcon` comes from `icon`. That draws registry,
  atelier, inn, training hall, archive, blacksmith, alchemist, store,
  trading post, bank, portal tower, chronicle hall, hall of fame,
  research library, museum, plus the generic marker. States: locked,
  available, new, upgrade (a pennant and a pip). A `town` recipe lays
  buildings from a list on paths, lamps, trees and signs, returning
  hotspots. Missing interiors: alchemist, bank, training hall, atelier,
  portal tower, museum (the other six `room`s exist).

**T21. Museum, Hall of Fame and badges** (2.7, 2.12). V4 and V6. M.
- Nothing for display cases, pedestals, plaques, frames, banners or podiums.
- Build: `exhibit` recipe: **frames** the picture the game gives it (a
  `Pixels` from any recipe) in a case, plinth, framed picture, manuscript
  page or specimen board, with a generic category placeholder (T5) when
  there is no picture, and an empty slot, an occupied slot and a "new"
  mark. Text never drawn into it; the game owns the words. Hall of Fame
  plaques, podium, ribbon, seal, milestone medal, achievement tiers and
  rank markers from `sigil` and `icon`.

**T22. Pixel UI frames and text.** **Dropped (D3).** Panels, buttons, bars,
seat frames, the turn strip, viewport frame and focus rings are CSS in the
game, with the VGA font. Painter keeps only what needs pixel art: the digits
and rarity frames in T11, the panel icons in T12 and the badges in T21.

### G. The top-down set (D2)

**T23. True top-down projection for the world** (1.2, 5.2, Appendix A). MVP. L.
- **Decided (D2):** Tag Team's world sprites are drawn looking straight
  down. Today `character` has front, side, back and quarter views; creatures,
  props and buildings are drawn from the side or front. None is top-down.
- Characters: `view: 'top'`, four facings (`north` `east` `south` `west`;
  `west` mirrors `east` like `facing: 'left'` does). Seen from above: the
  head and hair or headgear as a disc and crest, shoulders and arms, the
  weapon and shield held out, the back slot (cape, quiver, pack) behind,
  the feet showing at the stride. Same layer slots and loadouts, so the
  existing 26 loadouts, armour layers (T7) and dye (T3) work in it. Clips:
  `idle`, `walk`, `attack`, `hit`, `defeat` per facing; the poses that need
  a side view (`kick`, `fall`) get a top-down reading or are left out.
- Creatures: `view: 'top'` per body plan in `BODIES`, starting with
  `blob`, `biped`, `quad`, `spider`, `snake`, `skeleton`, `golem`,
  `winged`, `ghost`, then the rest.
- World pieces: `prop` and `furniture` from above (trees and rocks as
  crowns and tops, chests and doors with their hinge side, traps as floor
  marks); walls as flat tops with no face; `building` as a roof plan;
  `tile` edges, doorways and pits checked against it (T15). Items on the
  floor and effects stay as drawn; shadows are drawn under, not beside.
- Every art declares `projection: 'top-down'` (or `'front'`) in its
  returned data and in the pack manifest (T4), so the game can refuse to
  mix them in the world. The front, side, back and quarter views stay for
  portraits, the Looks Atelier mannequin and the Museum, which Tag Team
  puts on separate canvases anyway (2.1D).
- **Risk:** from above, class identity (2.1B, "readable without colour")
  rests on the headgear shape, weapon length, shield, and back slot.
  Add a lint check that the five classes differ by silhouette alone (a
  pixel-difference floor between their shapes) before this phase is done.

---

## Not Painter's (so not gaps)

Panels, buttons, health bars, seat frames, the turn strip and the viewport
frame (CSS, D3); the manifest schema, loader, compositor and fallback resolver (sections 5,
6, 14); content-to-visual mapping and historical version retention; licence
and ingestion workflow for third-party packs (Painter's output is
`Project-created`, so no ingestion applies); charts and chart panels
(rendered from data); research and admin dashboards; the carousel, layout
and responsive rules; screen-reader semantics, high contrast and text
descriptions (Painter only supplies `alt`, T6); nearest-neighbour scaling,
rendering engine, atlas format and memory budgets; audio-linked cues
(Chip Sounds); event-to-clip mapping; the onboarding tour; optional
image-generation assistance (Painter has no AI at draw time, rule 5).

## Order

| Phase | Gaps | Why this order | Release |
|---|---|---|---|
| 1. Contract | T1, T2 (with four facings), T3, T4, T5, T6 | every later recipe returns anchors, groups, projection, alt and goes through the pack | 1.21.0 |
| 2. Top-down party | T23 (characters, four facings, class-silhouette lint), T7 (boots to hair), T8 core, T9, T11 (rarity and digits), T12 (class skills), T19 (target, telegraph, damage) | Tag Team's MVP characters, items, skills and feedback, in the chosen projection | 1.22.0 |
| 3. Dungeon and enemies | T23 (creatures, props, walls, roofs), T13, T14, T15 (autotile + 2 kits), T16 core, T17, T18 | MVP corridor, two rooms, enemies and map, all from above | 1.23.0 |
| 4. Town and community | T20, T21, T10, T7 (armour families), T8 expanded | phases V2 and V4 | 1.24.0 |
| 5. Breadth | T15 rest, T16 rest, T13 rest | phases V3, V7 | 1.25.0 |

Each phase ends with `npm test` and `node cli.js kits --strict` green, a
kit row per new subject (a generic "paper-doll game" kit; no Tag Team
names inside Painter), `index.d.ts` in step, a CHANGELOG bullet per change,
and a suggested checkpoint (never automatic). Anything that changes
existing output (T7 layer order) says so in the changelog,
and `resolveLook` (T4) is what keeps Tag Team's saved characters safe.

## Decisions (settled 2026-10-30)

- **D1. Character canvas: stay 32 by 32.** Tag Team is told why in
  `docs/tag-team-handback.md`. A taller profile can be added later without
  touching the default.
- **D2. Projection: a true top-down set** for the world (T23). The other
  views remain for portraits and previews.
- **D3. Pixel UI and text:** digits and rarity frames in Painter (T11);
  panels, buttons, bars and seat frames in the game's CSS (T22 dropped).
- **D4. Pinned dependency, from launch.** During development Tag Team may
  use a local dependency (`npm install ../painter`) to follow changes. When
  the project launches it pins an exact released version (no `^` or `~`),
  from a tagged checkpoint or a tarball, with the lockfile committed. An
  upgrade is a deliberate step: read the CHANGELOG lines that say "changes
  existing output", store `resolveLook` specs (T4), and rerun Tag Team's
  golden hashes. Copies are no longer how Tag Team gets Painter.
