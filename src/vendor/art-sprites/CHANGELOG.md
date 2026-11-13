# Changelog

## Unreleased

- **Rec demo for the R36S build** (2026-11-12): `sprites-demo` from `r36s/commands.txt`'s `#@` lines; built and run in Debian 12 arm64 (`--no-video`); not run on a unit.

## 1.31.1 (first power section from the unit)

- **R36S: the fourth device report, the first with a `== power ==` section** (2026-11-12 16:31, the unit's clock right this time; `r36s/reports/2026-11-12-1631-r36s-report.txt`): 17 PASS, 0 FAIL, 2 SKIP (`kits`, `sheet item`, `!SLOW`); 88% charge, on battery. Times as before within 4%: `draw character --scale 2` 17.56 s (18.53), `draw scene --scale 3` 10.96 s (11.40), `sheet icon` 14.18 s (14.19), `convert` 13.99 s (14.78); total 59.0 s. **Power:** every command is 100% of one core (a tool is one thread), wake-ups 2 to 29 a second (goal 200: PASS), draw 1442-1545 mW against 1146 mW idle (+353 mW; 20.0 J over idle for the 4 commands of 1 s or more), 7.6 h left at that draw. The report's own "cut first": `draw character` is 30% of the CPU seconds. Peak memory is 12.4 MB (11.7 MB in the third report; the power sampling is the likely difference, not checked). No code changed. One run at 88% charge: repeat at 50% and 20% before trusting small differences.

## 1.31.1 (R36S command-line port, `convert` faster)

- **R36S: the third device report: `convert` is 7 times faster on the unit** (2026-11-12; `r36s/reports/2026-11-12-r36s-report.txt`): `convert` of the 16 x 16 PNG 14.78 s, was 105 s, so the precompute below is confirmed; total 61.2 s (was 150.8 s); 16 PASS, 0 FAIL, 2 SKIP (`kits`, `sheet item`); peak 11.66 MB; the other commands within 5% of 2026-11-11. No `== power ==` section yet. The unit's clock stamps it 2025-11-12, a year behind (set by hand on the unit).
- **`convert` (`fromImage`) no longer rebuilds its glyph and colour candidates per cell pair** (2026-11-11): `ansi-image.js` makes the 1,792 pairs and their two `halves` once, on first use, and `best` only measures distance; same order, so ties pick the same pair. Output byte-identical (a `.ans` from `convert` and a hash of `fromImage` cells over 12 draws, before and after); `npm test` 154 of 154. `convert` of an 80 x 40 item: 0.57 s to 0.22 s in Node. Not timed on the unit yet (105 s before).
- **R36S: the second device report, from the rebuilt package** (2026-11-11 12:29; `r36s/reports/2026-11-11-1229-r36s-report.txt`): 16 PASS, 0 FAIL, 2 SKIP (`kits` and `sheet item`, the `!SLOW` lines). `sheet icon` 14.3 s (predicted about 13 s), total 150.8 s against 435.8 s, peak memory 11.7 MB. `convert` still 105 s (its fix is open on `ROADMAP.md`); everything else within 10% of the first report. No code changed.
- **R36S: the package rebuilt with the faster deflate and the new `commands.txt`** (2026-11-11): `r36s/build-deb.sh` in Debian 12 arm64 under Docker made `sprites.deb` (658 KB), installed it with `deb_watcher.py` and ran its report there: 16 PASS, 0 FAIL. Not run on the unit yet: `sheet icon` is expected to pass in about 13 s.
- **`ROADMAP.md`** (2026-11-09): the R36S port's open items in full; the plan documents in `docs/` are listed there, their items not yet carried in.
- **R36S: the first device report from a unit, and what changed from it** (2026-11-09 22:31; `r36s/reports/2026-11-09-2231-r36s-report.txt`): 15 PASS, 1 FAIL on one R36S under r3i6nos. Every command ran 18 to 21 times as long as the QuickJS build on this Mac (Composer's and Neumetik's ratio too). Small drawings take 0.2-0.5 s; `draw character --scale 2` 17.7 s and `draw scene --scale 3` 12.8 s; `convert` of a 16 x 16 PNG 103 s (5 s on the Mac, 0.67 s under Node). **The FAIL: `sheet item`** (all 610 item variants on one 1302 x 7932 PNG) was still running at the report's 300 s limit, at 104 MB (every other command 11.6 MB). On the Mac it took 19.6 s, half of it the port's deflate (r3i6nos's `tools/qjs-node/shim/zlib.js`, now made faster with byte-identical output: `sheet item` 15.5 s here, still about 290 s on the unit). So `sheet item` is now a `!SLOW` line in `r36s/commands.txt` (run with `--all`), like `kits`, and `sheet icon` (67 variants, 0.7 s here) times the sheet and the PNG writer instead. `r36s/check.sh`: 11 of 11 identical to Node. Not rebuilt or packaged for the unit yet.
- **R36S command-line port** (`r36s/`, maintainer's request): the `sprites` command as one QuickJS executable for the R36S, built from `cli.js` unchanged by the r3i6nos repo's `tools/qjs-node/` (shims for the `node:` modules, bytecode only, libc and libm). `r36s/check.sh` compares it with Node on `r36s/commands.txt`: identical except PNG bytes, which have the same pixels and are larger. `r36s/build-deb.sh` packs a tool `.deb` under KlamANSI's free R36S grant (`r36s/package/copyright`), a **draft**: `PAINTER_RELEASE=1` refuses a build while it has its marker. No change to any recipe or output; no version bump. Not run on a real R36S or on GitHub. `sprites render` needs the VGA atlas and is not in the port yet.
- **License is proprietary again** (was Apache-2.0), following Composer and KlamANSI, since the three travel together (maintainer's decision, 2026-11-08): `LICENSE` (copyright ansiapps, all rights reserved), `package.json` `"license": "UNLICENSED"`, `"private": true`, `NOTICE`, the README section. No code changed. `NOTICE` keeps the CoffeeMUD attribution (Apache-2.0, Bo Zimmerman) and the VGA font's (CC BY-SA 4.0); the Apache-2.0 text is kept as `LICENSES/Apache-2.0.txt` because it must go with that data. Copies already distributed under Apache-2.0, such as the one Coupler ships, stay under it.

## 1.31.0 (biome layers and depth for `scene`)

- **`scene` layers** (`recipes/scene-layers.js`): `layers: true` adds the biome's weather, inhabitants, story evidence and depth; each is also a param on its own (`weather` by name: `rain`, `snow`, `fog`, `dust`, `motes`, `drip`; `inhabitants`, `evidence`, `depth`; a layer's own param overrides `layers`). `SCENE_LAYERS` names each of the 22 biomes' weather, who lives there (walkers on the path, animals, birds, bats) and its evidence (footprints, wheel ruts, scorch marks, graves, bones, skulls). Depth is a hazed background of far props, the midground as before, and dark cropped foreground shapes under a shadow dither. The biomes kit's `biome-layers` and `readability` rows are no longer `partial`.
- **Changes to existing output:** none; every layer is off by default and draws from its own forked stream. The golden hashes gained `layers: true` rows for every biome.

## 1.30.0 (scenery for the scene kits, kit notes brought up to date)

- **Scenery for the scene kits** (`recipes/scenery.js`, `prop` and `scene`): twelve 16 pixel props (`battlement`, `well`, `stall`, `signpost`, `dock`, `bridge`, `ropebridge`, `ruinarch`, `ruinwall`, `dune`, `mineentrance`, `portal`) and four 32 pixel ones (`castle`, `gatehouse`, `peak`, `floatisle`). Eight new `scene` biomes use them: `castle`, `town`, `harbor`, `river`, `ruins`, `mines`, `highlands`, `skyisle`. A scene's landmark is now placed by its own width, not by the name `house`. The kits `mountain-cave-dungeon`, `desert-ruins`, `village-castle`, `coast-river-swamp` and `magical` are `covered`. Changes to existing output: none; the golden hashes only gained rows.

- **Kit notes brought up to date:** in the paper-doll game kit, `semantic-clips` (the expanded list and weapon attacks landed in 1.x) and `portraits` (`emblem` landed in 1.28.0, now listed in the row) are `covered`. No drawing changed.

## 1.29.0 (splash screens)

- **`splash`** (new recipe): a splash screen from two params, `title` and `scene` (`meadow`, `forest`, `desert`, `mountains`, `sea`, `snow`, `night`; `seed` optional). Pixel: 256 by 144 on DB32, a dithered sky, ridges far to near and a dark plate with the title set from the VGA atlas (`atlas` option; without it the plate is drawn and the letters are not). ANSI (native): 80 by 24, a `vista` for the scene under block lettering that steps down `shaded`, `block`, `half` to fit, classic-legal. Exports `SPLASH_SCENES`, `SPLASH_W`, `SPLASH_H`, `SPLASH_MAX_TITLE`.
- **Splash Screen Reference Kit** (`kits/splash-screens.js`, `docs/cc0-splash-screen-reference-kit.md`): permissive sources only, each marked licence verified or unverified, with the excluded sources and why; the limits are recorded as `partial` notes.
- **Changes to existing output:** none; the golden hashes only gained rows.

## 1.28.0 (Tag Team: what was still "not done" in `docs/tag-team-gaps-plan.md`)

- **Eleven combat effects** (T19): `effect` takes `shield`, `shieldhit`, `buff`, `debuff`, `arrow`, `trail`, `rune`, `portal`, `levelup`, `crit` and `loot` (32 by 32, four frames, no full-frame flash). Judge them on a sheet.
- **`emblem`** (T9): a class emblem (16 by 16) for each loadout, the sigil shield in its cloth colour with the icon glyph of its weapon (`glyph` overrides).
- **`map` floor label** (T18): `label` (such as `3` or `B2`) with `atlas` sets the floor number and depth badge in the VGA font on a line above the pips; the header grows by 16 pixels. Without `label` the map is identical.
- **`furniture` from above** (T23): `view: 'top'` draws seventeen pieces (bed, table, chair, shelf, hearth, altar, workbench, anvil, barrel, rug, tub, counter, stove, fountain, piano, questboard, cage) on the 32 pixel grid as top-down props, so `prop` and `dress` take them too.
- **Changes to existing output:** none; the golden hashes only gained new rows.

## 1.27.0 (Tag Team leftovers: the "Left" row of `docs/tag-team-gaps-plan.md`)

- **`cloak`** (T7): a ramp name, drawn as a layer of its own over the torso and under the head, so a pack or a quiver in the `back` slot can be worn with it. Two panels from the shoulders in the front and quarter views, a sheet from behind, one behind the body from the side,
  a round mantle from above. It is a palette group (`dye` takes `cloak`). A character without one is unchanged.
- **Boss canvases 192 and 256** (T13), front and top-down: `canvas` takes them (`CANVASES`, `TOP_CANVASES`); the top-down plans draw again at that scale, the front body is enlarged as 96 and 128 are, and the segments still lay back into the still.
- **Top-down plans of their own** (T23) for `fish`, `horse`, `centaur`, `griffon`, `bird`, `naga`, `merfolk`, `floater`, `fey`, `elemental` and `multiarm`, which shared the snake, quad, winged, blob and ghost plans. Judge them on a sheet.
- **Building states as animation** (T20): `building` with a `service` takes `animate: true`: four frames at 4 fps, looping, for `new` (the pennant ripples), `upgrade` (the chevron bobs) and `locked` (a glint goes round the padlock). Frame 0 is the still; `available` stays one frame.
- **Town paths** (T20): `town` takes `paths`: `lattice` (the default), `rows` (a street along each row of buildings, joined at the two ends) or `cross` (the rows, joined by one street down the middle). Doors still open on a street, the streets are one network, and the grid marks the rest as grass.
- **Changes to existing output:** the top-down drawings of the eleven bodies above differ from before (they were the nearest shared plan). Everything else is identical, and the golden hashes did not change.

## 1.26.0 (Tag Team phase 6: bigger icons, the conveyor and the gate tile; plan in `docs/tag-team-gaps-plan.md`)

- **24 and 32 pixel item icons** (T10, T1): `item` takes `iconSize` 16 (the default, unchanged), 24 or 32 (`ICON_SIZES`). The shape's own strokes, ellipses and polygons are laid out again on the finer grid, so curves and diagonals are
  rasterised there rather than enlarged; strokes keep their weight, shading is in bands two pixels deep at 32 with a dithered seam, the metal glint is a streak with a point of light, wood and leather get grain strokes,
  the rim of a shield or helm is two pixels at 32, and the tier marks (wear, glint, runes, sparks) are bigger. `anchors.grip` is scaled. All 101 shapes draw at both sizes.
- **`conveyor`** (T16): a top-down prop, one tile, `east`, `west`, `north` or `south`, with a `run` loop (the slats slide, chevrons point the way).
- **`gate` tile** (T16): `part: 'gate'` (a portcullis in a doorway: `closed`, `open` drawn up into the lintel, `locked` with a padlock) and `=` in a `dress` grid, drawn in the new `gates` state (`closed` by default, `open`,
  `locked`) and counted as walkable (the game opens it).
- **Changes to existing output:** the lit hatch of `ladder-up` is pale sand instead of gold (all three styles, every biome); the `dress` pictures are identical. Everything else is identical.
- Not done: boss canvases above 128, plans of their own for the shared top-down bodies, building states as animation, a town path network other than the lattice, a cloak apart from the back slot.

## 1.25.0 (Tag Team phase 5: the rest of the dungeon, the props and the bosses; plan in `docs/tag-team-gaps-plan.md`)

- **Four new dungeon kits and five new tile parts** (T15): `fungal`, `arcane`, `void` and `mechanical` join the top-down biomes (`TOP_BIOMES` is twelve; `dress` takes them as themes). New `part`s: `ladder-down` and `ladder-up`
  (turned by `facing`), `cliff` (the drop is on the side `facing` names, a lit lip, a rock face, then dark), `chasm` (deeper than `pit`, with a rock rim and dark that deepens inward) and `water` (two frames, shore
  lipped and joined like `lava`). `dress` takes `hazard: 'water'` or `'chasm'` and picks `water` for `flooded` and `chasm` for `void` by default. Every part draws at 16 and 32 pixels.
- **Props** (T16): `gate` (2 by 1, `closed`, `open`, `locked`, with an `open` clip), `mushroom`, and harvest stages: `plant` and `mushroom` take `seedling` and `growing` as well as `ripe` and `picked`, with a `harvest`
  clip (events `ripe` and `picked`); `dust` (a `drift` loop), `gears` (`turn`), `piston` (`pump`) and `ventsteam` (`steam`). 31 top-down props.
- **Bosses** (T13): `canvas` 96 and 128 in both views (drawn again at that scale, not enlarged); `segments` 2 to 5 (`SEGMENT_IDS`: head, chest, body, hips, tail) cut along the long axis; the parts still lay back into the
  still exactly. Five new bodies: `flytrap`, `vine` (a plant `family` with the `treant`), and three swarms of their own, `vermin` (rats), `flock` (bats) and `wisps`, each with a top-down plan; the `swarm` family allows
  all four. New presets: `rat-swarm`, `bat-swarm`, `wisp-swarm`, `flytrap`, `vine-horror`.
- **Changes to existing output:** the `plant` prop's pixels are identical, but it now has a `harvest` clip and two more states, so the golden hashes of `plant` ripe and picked (pixel, text and ansi) changed. Every other
  existing drawing is identical.
- Not done: boss canvases above 128, a plan of its own for the shared top-down bodies, drawn (not scaled) 24 and 32 pixel item icons.

## 1.24.0 (Tag Team phase 4: the town, the Museum, items and the expanded clips; plan in `docs/tag-team-gaps-plan.md`)

- **Building grammar** (T20): `building` with a `service` draws from one grammar of `shell` (`cottage`, `hall`, `tower`, `vault`, `stall`, `wing`), `theme` (`village`, `stone`, `arcane`, `market`) and an `icon` `glyph`, for the fifteen
  services and a generic `marker` (`SERVICES` gives each its defaults; `shell`, `theme`, `roof` and `glyph` override). Front view 32 by 48 with the icon hung as a sign, top view 32 by 32 with the icon on the roof plan. `state`:
  `locked` (dithered, a padlock), `available`, `new` (a green pennant with a plus), `upgrade` (a gold pennant with chevrons). Returns a `hotspot` and a `door` anchor. The seven old kinds are unchanged.
- **`town`** (T20): `{ buildings: [{ service, state?, cell? }], seed, cols, rows }` lays buildings on a lattice of streets, with a lamp at each crossing and a tree in each empty slot; returns the `hotspots` (`id`, `service`,
  `state`, `rect`, `entry` on the street), a `grid` of 32 pixel cells (`#` building or tree, `.` street, `,` grass) and `alt`. Six new `room`s for the services that lacked one: `alchemist`, `bank`, `training`, `atelier`, `portal`, `museum`.
- **`exhibit` and `award`** (T21): `exhibit` frames the picture the game gives it (an art, a Pixels, or `{ recipe, params }`) in a `case`, `plinth`, `frame`, `page` or `board` (48 by 48), `empty`, `occupied` or `new`, and
  returns the `window` the picture sits in; with no picture an occupied slot shows a category placeholder (`category`) or the generic exhibit one, so an undiscovered piece gives nothing away. `award`: `plaque` (blank, with an `emblem` slot),
  `podium` (`place` 1 to 3, counted in pips), `ribbon`, `seal`, `medal` (`tier` bronze to platinum, the rim notches count it) and `rank` (1 to 5 chevrons). No text is drawn: the game owns the words.
- **Items** (T10): eleven `item` shapes (`crossbow`, `kiteshield`, `towershield`, `roundshield`, `magicshield`, `ceremonial`, `sheath`, `quiver`, `tent`, `bedroll`, `rope`); `material` (`ore`, `herb`, `hide`, `part`,
  `crystal`, each with a `variant` that wraps); `currency` (`coin` in `copper`, `silver`, `gold`, `platinum` with a mark that differs as well as a colour, a `stack` of 1 to 3, `purse`, `key`, `fragment`, `plan`, `token`).
  Not done: 24 and 32 pixel icons (a redraw with more detail, not a scale).
- **Expanded clips** (T8): `dodge`, `parry`, `charge`, `stun` (loop), `potion`, `buffed` (loop, with a flash), `lowhealth` (loop), `levelup` (with a flash), `tagin`, `tagout`, `equip`, each with an event and a reduced still, in every
  view; `shoot`, `thrust` and `slam`, and `weaponAttack(shape)` (and `art.weaponAttack`) names the attack a weapon takes (`cast` for a staff or wand; `attack` otherwise, unchanged).
- **More gear layers** (T7): `tabard`, `bracers` (ramp names, dye groups), `neck` (`amulet`, `scarf`, `collar`, with `neckRamp`), `scar` (`brow`, `cheek`, `mouth`), `jewellery` (`earring`, `brooch`) and `helmStyle` (`visored`,
  `crested`, `plumed`, `winged`; a helm or a horned helm only), each its own optional layer in every view. Scars and jewellery are a first pass: judge them on a sheet.
- `pack` writes `hotspots`, `hotspot`, `grid`, `window` and `weaponAttack` as data. A kit gains six subjects; `kits --strict` is green. `index.d.ts` and `index.js` export the new lists.
- **Changes to existing output** (no pixel changed: the stills, layers and the eleven old clips of all 206 `character` variants are identical to 1.23.0): the golden hashes of those variants changed because every character's art now
  carries fourteen more clips and `weaponAttack`. Every other existing drawing is identical.

## 1.23.0 (Tag Team phase 3: the dungeon, the enemies and the map; plan in `docs/tag-team-gaps-plan.md`)

- **Top-down creatures** (T23): `creature` takes `view: 'top'`, a `facing` (`north`, `east`, `south`, `west`, turned a quarter at a time and shaded after, so the light stays top left) and a
  `canvas` (40, or 48 and 64, which draw the plan again at that scale). 40 by 40, `projection: 'top-down'`, two stride frames, the head anchor where the creature faces. Nine plans of their own
  (`blob`, `biped`, `skeleton`, `quad`, `spider`, `snake`, `golem`, `winged`, `ghost`) plus `drake`, `insect`, `treant` and the five new bodies; `fish`, `naga`, `merfolk` share `snake`, `horse` and
  `centaur` share `quad`, `griffon`, `bird` and `fey` share `winged`, `elemental` shares `ghost`, `floater` shares `blob` (`TOP_PLAN`). Not done: a plan of their own for those.
- **Enemy families** (T13): `family` (`FAMILIES`: humanoid, bandit, goblinoid, undead, beast, insect, slime, elemental, demon, dragon, spirit, golem, mimic, fungus, construct, swarm, aberration)
  composes body, skin, eyes, jaw, horns, ears and gear by `seed`, and throws on a part the family does not allow. Five new bodies in both views: `mimic`, `fungus`, `construct`, `swarm`,
  `aberration`; presets `mimic`, `sporeling`, `automaton`, `swarm`, `tentacled` and three bandits, whose `gear` (`hood`, `bandana`, `blade`, `club`, `buckler`) is drawn on a biped. `accent` is the cloth colour.
- **Elite and boss** (T13): `rank` `elite` (a gold edge and a three-point crest) or `boss` (a taller crest and a `canvas` of 48 or 64; the front view is resampled and then re-textured so the 2 by 2
  blocks break, the top view is redrawn). A boss returns `parts` (`head`, `body`, each a full-canvas layer with a `joint`; laid together they are the still exactly). `pack` writes them.
  Not done: boss canvases larger than 64, parts beyond two.
- **Enemy clips and looks** (T14): every creature, in either view, has `telegraph` (`release`), `attack` (`impact`), `attack_ranged` (`fire`, with an `effect_origin`), `attack_magic` (`cast`), `stagger`, `block`, `enrage`,
  `summon` (`spawn`), `retreat`, `dissolve` (`gone`, seeded) and `phase`, each with events and a reduced still; `direction` (front view, `west` by default) or the facing (top) is the way it lunges. `flash: false`
  swaps every white silhouette for a gold edge. `statuses` (`poison`, `burn`, `freeze`, `stun`, `bleed`) is a transparent layer `layers.status`, one frame per frame of the creature; `intent` (`attack`,
  `defend`, `magic`, `flee`, `summon`) is a marker over its head in `layers.intent`, each a different shape.
- **Top-down dungeon tiles** (T15): `tile` with `kind: 'dungeon'`, `view: 'top'`: 8 biomes (the six, plus `ancient` and `volcanic`, which also draw in the front tiles), 16 or 32 pixels (the 32 is drawn again
  with more stones and grain), and 14 parts: `floor`, `wall` (a flat top: no face), `pillar`, `broken`, `doorway` (`open`, `closed`, `locked`), `stairs-down`, `stairs-up`, `pit`, `lava`, `toxic`, `bridge`, `rubble`,
  `cracks`, `shade`. The autotile is the 4-neighbour `mask` plus the corner `notch`: the 47 shapes of a blob, all different (tested). A wall's open sides get a dark edge and a rim; a floor takes the wall's shadow.
  Not done: ladders, cliffs, chasm edges, water edges, a fungal, arcane, void or mechanical biome.
- **Top-down props** (T16, `prop` with `view: 'top'`): 26 props on the 32 grid (door, exit, chest, torch, brazier, crate, urn, statue, coffin, lever, plate, trap, web, ore, crystal, plant, banner, bookshelf, lectern,
  scrollrack, portal, and the tree, pine, bush, rock and stump as crowns and tops), each with `footprint`, `occlusion`, `states` (door and chest `closed`, `open`, `locked`; trap `armed`, `triggered`, `disarmed`; lever;
  plate; plant) and clips: `flicker`, `glow`, `swirl`, `open` (`opened`), `trigger` (`sprung`) and `reveal` on every prop. Collision stays the game's. `building` takes `view: 'top'`: the seven roof plans.
- **`dress`** (T17): `{ grid, theme, template, props, doors }`. The grid is the game's (`#` wall, `.` floor, `+` door, `>` exit, `o` pit, `^` hazard); every cell is drawn as it says, with the autotile. Props come from the
  game's own list first and then from a `template` (`arena`, `corridor`, `junction`, `treasure`, `trap`, `shrine`, `safe`, `merchant`, `boss`, `exit`, `secret`), and one that would cut the walkable cells apart is not placed.
  Returns the `props` with footprints, `blocked`, `walkable`, `unreachable` (a fault in the grid, reported, not repaired) and `cells`; `lintDress(art)` lists cells whose picture does not match the grid.
- **`map`** (T18): `{ cells, party, markers, size, depth, progress }` from the cells the game supplies and what is known of each (`unknown`, `visible`, `remembered`, `explored`, told by pattern as well as tone). An
  unknown cell is drawn the same whatever it holds and a marker on one is not drawn (tested). Returns the fog `mask`, the `legend` of what is shown and `alt`. Mini (4 pixels a cell) and expanded (8).
- **`pack`** writes the world data: `data` on each entry (`footprint`, `occlusion`, `states`, `props`, `blocked`, `walkable`, `unreachable`, `entry`, `tile`, `legend`), a boss's parts and a map's mask as files.
- A **`paper-doll-game` kit** gains eight subjects; `kits --strict` is green. `index.d.ts` and `pack.d.ts` follow.
- **Changes to existing output** (no pixel changed: the stills, `idle`, `hit` and `defeat` of all 83 existing `creature` variants are identical to 1.22.0): the golden hashes of those variants changed (all three
  styles) because every creature's art now carries eleven more clips. Every other existing drawing is identical. `art-meta` alt text for a creature names its rank and view when it has them.

## 1.22.0 (Tag Team phase 2: the top-down party; plan in `docs/tag-team-gaps-plan.md`)

- **Top-down characters** (T23): `character` takes `view: 'top'` and `facing` `north`, `east`, `south` or `west` (`west` is `east` mirrored; `south` is the default), every loadout,
  pose, clip, `motion`, `back` slot, dye and `bodyProfile` (`recipes/topdown.js`). 40 by 40, the body centred on (20, 20), `projection: 'top-down'`, the weapon turned to point along
  the facing. The four facings keep the layer slots; from the north the back slot covers the torso. `lintSilhouettes(items, { floor })` (`lint.js`) fails a set of drawings that
  differ by less than the floor (0.15 of their union) as filled shapes: the five classes pass in every facing. The other views are unchanged.
- **Gear layers** (T7, the part built here): `armor` (`cloth` is the plain torso; `leather`, `chain`, `scale`, `plate` are a texture on the torso layer's own colours), `bootStyle`
  (`short`, `tall`, `greaves`), `gloves`, `belt`, `shoulders`, `eyewear` (`goggles`, `mask`), `hairStyle` (`long`, `ponytail`, `bun`, `topknot`, `mohawk`, `braids`, `pigtails`,
  `curly`; shown with a bare head or a circlet, so a helm is never hidden by a missing look) and `facialHair` (`beard`, `mustache`, `goatee`, `stubble`), each its own optional entry in
  `layers`, drawn from the named anchors in every view. `gloves`, `belt` and `shoulders` are dye groups. **`appearance`** is what is shown over what is equipped (a cosmetic override is a
  param, not a recipe); `resolveLook` keeps it. Not done from T7: bracers, neck, cloak and tabard as layers apart from the back slot, scars and jewellery.
- **Semantic clips** (T8 core): every character has `turn`, `block`, `cast`, `heal`, `celebrate` and `recall`, each with an event and a `reduced` still, in every view. They lift relative
  to a stance, so a `run` or `fall` character still moves. Not done: `dodge`, `parry`, the expanded list and weapon-specific attacks.
- **Portraits** (T9): `portrait` takes `size` (`party` 16 by 16, `status` 20 by 20 and the default, `card` 32 by 32), `expression` (the nine of `face`, over skin only), `state`
  (`defeated` in the stone ramp, `recalling` fading from the feet up) and returns `mask` and `safe`. The default pixels are unchanged. Not done: class emblems.
- **`overlay` recipe** (T11): a rarity frame (`common` to `legendary`, each its own shape), state markers (`new`, `equipped`, `locked`, `bound`, `crafted`, `enchanted`, `compat`, `socket`,
  `better`, `worse`, `same`, each its own shape, four at most), a five-pip durability row and a stack `count`. `size` 16 or 32. **`pixelText(atlas, text)`** and `draw(..., { atlas })`
  bring the VGA font to pixels (`count` needs `size: 32`, the 8 by 16 digits at their own size); the atlas is passed in, so the main entry still has no Node built-ins. The CLI and `pack`
  load it themselves.
- **`skillicon` recipe** (T12): a 24 by 24 skill icon from `category` (10 plates), an approved `motif`, `target` (7), `kind` (`active`, `passive`, `triggered`), `tier` and `cost` (0 to 3),
  `locked` and `cooldown` (a dithered sweep; `clips.cooldown` runs it down). Ten **panel icons** are new `icon` glyphs: `actions`, `chat`, `party`, `character`, `inventory`, `map`,
  `chronicle`, `charts`, `research`, `help`.
- **Combat feedback** (T19): `telegraph` (`circle`, `line`, `cone`, `rect`; `danger` hatched, `friendly` dotted, `neutral` checkered, so colour is not the only cue; two frames; top-down),
  `target` (`ally`, `enemy`, `neutral` rings of different shapes, `turn` brackets, `seat` diamond, `shadow` blob; top-down) and **`drawNumber(atlas, { value, kind })`** (`damage`, `crit`, `heal`,
  `miss`, `block`; a four-frame rise). The number is a function, not a registered recipe, because it needs the atlas.
- A **`paper-doll-game` kit** (`kits/paper-doll-game.js`) covers all of the above; `kits --strict` is green.
- **Changes to existing output** (no pixel changed: every frame, clip frame and layer of the 1.21.0 drawings is identical): the golden hashes of every `character` variant changed because
  its art now carries six more clips, and of `portrait` because it carries `mask` and `safe`. Existing `clips` key lists gain `turn block cast heal celebrate recall`.

## 1.21.0 (Tag Team phase 1: the contract with a compositor; plan in `docs/tag-team-gaps-plan.md`)

- **Every drawing carries metadata a compositor and a screen reader read** (`art-meta.js`, filled in by `draw()`): `alt` (a short factual phrase), `anchors` (`root`,
  `shadow_center`, `head`, `effect_origin` from the drawn pixels on every pixel recipe; a recipe's own anchors win), `bounds`, `reduced` (the still for an
  animation, on the art and on every clip, with `events` on every clip), and `projection` (`front` for character and creature). **No pixel changed**: all 4,280
  registered drawings were hashed by frames, clips and layers against 1.20.1 and are identical. The golden hashes in `test/golden.json` changed because they hash
  the whole art object, so they were regenerated on purpose.
- **Named character anchors** (T2): `root`, `head`, `neck`, `shoulder_left/right`, `hand_main`, `hand_off`, `waist`, `back`, `foot_left/right`, `weapon_grip`,
  `shield_grip`, `effect_origin`, `shadow_center`, for every pose, view (the back view and `facing: 'left'` mirror them with the art), clip frame and `motion` clip
  (`art.anchors.frames`, `clip.anchors`). `art.anchors.feet` is unchanged.
- **`bodyProfile`** on `character` (T1): `medium` (the default, unchanged), `small`, `slim`, `broad`, each layer scaled about the feet inside the same 32 by 32
  canvas. The canvas stays 32 by 32 (decided 2026-10-30; `docs/tag-team-handback.md`).
- **`paletteGroups`** on character and creature art, **`recolor` / `recolorArt`** (`recolor.js`: ramp to ramp, each pixel keeps its step) and **`dye`** (`look.js`:
  draws again with colour groups set, so shading and outline are exact) (T3).
- **`resolveLook`** (`look.js`): every default written into the params for character, creature and item, JSON-safe, with the engine version, so a stored look is
  not changed by a later default. **`ENGINE_VERSION`** (`version.js`, kept equal to package.json by a test) (T4).
- **`sprites pack <list.json> --dir d`** and `art-sprites/pack` (`pack.js`, Node only): a list of recipe, params entries in; every frame, clip and layer as PNG, an atlas
  of the stills, `manifest.json` (look, anchors, palette groups, alt, projection, clip events and reduced stills, per-file pixel hashes, the game's own fields untouched
  under `meta`, engine version, Project-created licence) and `goldens.json` out. Same list, same bytes: no timestamps, and the hashes are of pixels, not PNG bytes (T4).
- **`placeholder` recipe** (T5): `character`, `family` (12), `item` (8 categories), `building`, `exhibit` and `unknown` (by size class only: it reads nothing else, so
  it cannot show what an undiscovered thing is). Registered with variants; motion type `object`.
- **`lintArt`**: `alt`, anchors inside the canvas, and a `reduced` still for the art and every clip that moves; all errors (T6).
- **`effect` takes `flash: false`** (the brightest colours drop a step) and **`density: 'low'`** (half the small sparks) for the no-flash and low-effects settings (T6).

## 1.20.1 (no repeated poses)

- **No repeated poses in presets** (from Animator's still-rule warnings): `head.nod`, `object.pulse` and `effect.pulse` are now 2-frame loops (rest, then the move)
  instead of 3 frames that ended on their first pose. `structure.creak` and `lettering.wave` drop a pose equal to its neighbour (the loop's wrap included), so at
  small amounts they have fewer frames (creak at its defaults: 4, not 6). These presets' clips change; clips passed directly are untouched. The `range` warnings
  on doubled params are `lintMotion` working as intended, so unchanged.

## 1.20.0 (motion types)

- **Every recipe has a motion type** (`motion-types.js`: `MOTION_TYPES`, `MOTION_OF`, `motionType`, `motionPresets`): `figure` (character), `head` (face, portrait,
  bust, creatureface), `creature` (creature, creation), `object` (item, thing, icon, sigil), `limb` (hand), `plant` (farm, prop), `machine` (vehicle), `structure`
  (building, strategy, furniture, room), `tiling` (tile, pattern), `scenery` (scene, vista, landscape, locale, onplace, weatherloop), `cloth`, `effect`,
  `panel` (bar, mappane, sheet, menu, fight, artpack) and `lettering` (banner). A type lists the pose channels it uses and how far (a share of the art), and has presets
  with their own params: `wind` for a plant or cloth, `distance` for a panel, `reach` for a figure or creature, `direction` for a tile.
- **`motion` takes a preset:** a name (`'bob'`), `{ preset, id?, ...params }`, or a list mixed with clips. Unknown presets and params throw. `lintMotion`
  reports a channel a type does not use or a move past its limit (findings, never a throw). A clip given directly is untouched, so existing output is unchanged.
- **`wrap: true` on a clip** (and `moveFrame(still, pose, { wrap })`): `dx dy lift` carry what leaves one edge in at the other, so a scrolling tile stays tiled.

## 1.19.0 (motion on anything)

- **`motion` param on every recipe, in every style:** a clip (or list) of poses (`{ id, frames: [pose], fps, loop, events, holds?, reduced? }`, the shape of
  Animator's clips) adds a clip to the art's `clips` under its `id`. `character` draws each pose with its own joints, the loadout, view and stance. Every other
  recipe, in pixel, `ansi` and `text`, moves its first frame (`motion.js`: `moveFrame`, `withMotion`): `dx dy lift` move it, `squat` squashes it from the
  bottom, `lean` shears the top, `sway` shears the lower half, in the art's own units (pixels, or cells); `head drag legs arm weapon air` are ignored. `holds`
  repeat a frame and event frames follow; an unknown channel or a bad hold throws. Without `motion` nothing changes, so existing output is unchanged.
  Carry-over: Animator's hand-off is done (`docs/animator-plan.md` there).

## 1.18.0 (robe, three-quarter, foreshortening)

- **`character` poses `point` and `kick` (foreshortening):** an arm and a leg coming at the viewer on the front view. `point` drops the arm to a short
  stump ending in a large fanned hand; `kick` shortens the near leg and swells its boot, higher than the standing foot. New poses, so existing output is
  unchanged. The kit's `pose-foreshortening` row is now partial (front view only); no kit gaps remain.

- **`cloth` kind `robe` (24x24 pixel loop):** a skirt or robe through one full turn of the body. The folds sit at fixed angles round a flared
  cylinder and the turn carries them across the front, the hem lags the waist (drag) and scallops, and `wind` flares it wider; `ramp` is the cloth
  and `accent` the sash and hem trim. New kind, so existing output is unchanged. The kit's `motion-robes` row is now covered (gaps left:
  `pose-foreshortening`, `turn-three-quarter`).

- **`character` view `quarter` (three-quarter, facing right):** between front and side: narrower head and torso, eyes crowded to the facing edge with a
  nose past it, closed-up legs, the far arm and shield behind the body. Same layers, loadouts, poses, clips and back slots as the other views. New view,
  so existing output is unchanged. The kit's `turn-three-quarter` row is now covered (gap left: `pose-foreshortening`).

## 1.17.0 (farm, strategy, weapon family)

- **`farm` recipe (16x16 pixel):** wheat, carrot, pumpkin, corn, turnip, grapes, egg, mushroom, hen and sheep, shaded from the DB32 ramps; `ink`
  names the body ramp. New recipe, so existing output is unchanged. The kit's `reference-farm-food` row is now `farm-food` and partial.
- **`strategy` recipe (16x16 pixel):** six pointy-top hex terrain tiles (grass, forest, mountain, water, desert, snow) and four towers (archer, cannon,
  mage, frost), shaded from the DB32 ramps; `ink` names the body ramp. New recipe, so existing output is unchanged. The kit's
  `reference-tower-strategy` row is now `tower-strategy` and partial.
- **`weapon-family` kit row:** the kit's `reference-weapon-family` row now lists the 25 weapon kinds `item` draws. Nothing is drawn differently, so no output changes.

## 1.16.0 (colour variants)

- **Every smooth-colour recipe is registered at all three depths:** `vista`, `creatureface`, `thing`, `creation`, `onplace` and `fight` list each
  variant at 16 (as before), `colors: 256` and `colors: 'true'`, so `sprites sheet`, the lint tests and the golden hashes cover 256 and true colour
  too (988 new golden hashes; the 16-colour ones did not change). The `bg-limit` lint rule no longer counts a true-colour cell, whose exact RGB is not a classic index.

## 1.15.0 (vehicles, buildings, patterns)

- **`pattern` recipe (16x16 pixel, tileable, two colours):** checker, checker4, stripes, hstripes, diagonal, dots, grid, bricks, diamonds, waves,
  rings, weave, hatch and seeded noise (mirrored so its edges meet). `ink` and `paper` are ramp names. New recipe, so existing output is unchanged.
  The kit's `one-bit-patterns` row is now covered.

- **`building` recipe (32x32 pixel, front view):** barn, windmill, lighthouse, tower, chapel, inn and watchtower, shaded from the DB32 ramps;
  the seed picks the roof colour, `roof` names a ramp. New recipe, so existing output is unchanged. The kit's `buildings` row is partial.

- **`vehicle` recipe (32x32 pixel, side view):** wagon, sled, car, locomotive, airship, raft and cannon, shaded with `shade.js` from the DB32 ramps;
  the seed picks a car or airship body colour, `body` names a ramp. New recipe, so existing output is unchanged. The kit's `vehicles` row is now
  partial (it also counts `prop`'s boat, cart, ship, caravan and spaceship, which the first draft of the study note missed).

- **`docs/free-asset-sources-study.md`:** a study note on five free-asset lists (assethoard, jam-ready-assets, Tiddybub/2d-assets, two gists):
  what they hold, what Painter already covers, and the gaps (vehicles, buildings, weapons, farm and food, tower-defense, 1-bit patterns).
  No output changes.
- **`free-asset-sources` kit:** the same gaps as seven `reference-only` rows (`kits/free-asset-sources.js`); nothing is drawn, so no output changes.
  Licenses were checked on the source pages: only Kenney and Quaternius (3D) are verified CC0; assethoard, jam-ready's RUN-licence packs and
  Tiddybub's blanket CC0 claim are excluded.

## 1.14.0 (smooth colour for every recipe)

- **`colors: 16 | 256 | 'true'` on every picture Coupler draws:** `creatureface`, `thing`, `creation`, `onplace`, `fight` (and `figureOnPlace`, `placeLayer`)
  take it as `vista` already did; `vista` now takes `'true'` too (it stopped at 256). 16 is the default and **its output is byte-identical**: the
  golden hashes did not move, and the 16-colour code paths are the ones that were there (a smooth branch is taken only at 256 and true colour).
  An Art drawn at 256 or true colour has a `colors` key (and its `figure` a `depth`); a 16-colour one has neither, so its hash is the same.
- **True colour in the cells.** A `Cells` picture can carry the exact RGB beside its palette indexes: `cells.tc` (`{ fg, bg }`, 0xRRGGBB, -1 where
  the index stands), `cells.trueColor()`, and `fgRgb` / `bgRgb` on `set` and `get`. The index is the nearest of the 256, so a viewer that cannot
  show true colour reads the 256 picture. `clone`, `equals`, `blit`, `halves`, `tone` and `tones` carry and read it (so `tones` is the exact hex
  a true-colour viewer paints), `renderCells` draws it, `writeAnsi(..., { colors: 'true' })` writes `38;2` and `48;2` from it (and the index at
  16 and 256), `legalize` leaves a true-colour cell alone (a 24-bit colour has no background limit), and the `invisible` lint compares RGB. A
  16 or 256-colour picture has none of this and the same fields as before (`tc` is a private field behind a getter).
- **The pixel canvas has three depths.** `Px` takes `depth` (16, 256, 'true'; `true` and `false` still mean 256 and 16). A pixel is a handle: an
  index, or `EXACT | 0xRRGGBB`; `Px.a` is now a `Uint32Array`. `px.between`, `px.scaled` and `px.tint` blend at the canvas's depth: the nearest of
  the 256, or the exact colour. `px.mix` is still the Bayer dither at 16 and the blend at 256 and true, so a `mix` is the same code at all three.
  The rooms use these (`vista` and `placeLayer`), so true-colour rooms are the same pictures as 256 with exact blends and finer texture.
- **Faces.** At 256 and true colour the volume shading (`shade`) is a blend instead of a Bayer dither; then a last pass (`recipes/smooth.js`,
  `volume`) gives the skin light from the upper left and a darker lower right edge, strands down the hair, a grain over fur and feathers, and the
  backdrop falling off toward the bottom. It runs after the hurt steps and looks, which find colours by index.
- **Items.** The backdrop and the table front fade to black as blends (one `mix`, the same dither at 16); the glow aura is a falloff, `hidden` a
  blend toward black; the material's grain (a weave in cloth, a mottle in leather, long grain in wood, a brushed streak in metal, a coarse
  mottle in rock, sparkle in precious things, a sheen in glass) is added on the volume.
- **Creation pictures and the Fight panel.** The sky falls toward blue and the ground darkens with a grain; the figure gets the same volume pass.
  The Fight room's no-room gradient is a smooth gradient; the flash is a blend toward white.
- **The figure on a place.** At 256 and true colour the place recedes by a quarter (`scaled`, black staying), the rim light is a blend that falls off
  over two rows, and night takes a figure's colours down to half, in place of the `LIGHTER`/`RECEDE`/`darken` table steps and dither (the tables are
  exported and unchanged for 16). A figure carries its `depth`, and `figureOnPlace` draws the place at it unless `colors` says otherwise.
  A figure's `colors` is a `Uint32Array` at 256 and true colour (a handle can be an `EXACT` colour); `CLEAR` (255) stays the no-pixel sentinel, so a
  drawn colour of exactly 255 is moved to 254 there.
- CLI: `--colors 16|256|true` is also the depth an ANSI-only recipe draws at (a `colors=` param wins).
- `tools/vista-matrix.mjs` draws the matrix's true-colour pictures as true colour (they were the 256 drawing): 24 of the 260 hashes changed, the other
  236 did not; `tools/vista-baseline.json` is updated. The 39 `ramp` findings are the same.
- Tests: determinism at all three depths, 16 equal to the default, 256 and true colour different with more tones, the cells' RGB, the writer, a bad depth
  refused, the figure on a place, lint clean at 256 and true colour for every variant of the five recipes.
- Not done (still 16 or 256 only): `locale`, the `ui` recipes and the other native ANSI recipes; `light` and `resolveTones` read palette indexes, so
  they leave RGB alone (a true-colour picture is lit by drawing it lit, as `figureOnPlace` does).

- **Apache-2.0** (was proprietary): `LICENSE`, `NOTICE`, `package.json` `license`, a README section. No code changed.

## 1.13.0 (Coupler gaps, Phase G: the close)

- No recipe or output changed. Kit `coupler-painter`: `scene-input`, `accessibility-words` and `ansi-file` are covered (what stays on Coupler's
  side is in the row); `terrains-outdoor`, `terrains-indoor`, `time-of-day` and `weather` stay partial until Coupler judges them by eye.
- Decided with Coupler: `recede` follows the table (a place is a shade darker); the fixtures' unreceded look is not followed. Accepted as thin, kept
  in `terrains-outdoor`: mountains are one ridge, wooded snow has less texture, the swamp sky is a flat band.
- **Output changes (`vista`, deliberate, answering Coupler's review):** `mountains` is three layered ranges (far grey and hazy, middle brown with snow,
  near dark foothills), each a continuous ridge with a lit and a shade face to every peak, gullies down the lit face, snow on the two far ranges;
  `swamp` gets a far line of dead trees and a mist on the water in place of the flat sky band; at 256 colours and true colour, snow lying on the
  land (every terrain in snow or blizzard) is textured: soft drifts, bluish hollows, glints. 31 of the 260 matrix pictures changed: every
  `mountains` and `swamp` picture, and the `dusk-snow` pictures at 256 and true colour of `plains`, `woods`, `city` and `mountains`. Also changed
  by the same code: `locale` `frozenpeaks` and `onplace` on `mountains`. Golden hashes and `tools/vista-baseline.json` updated.
- **The rule applied to every picture** (answering "use a broader palette for texture"): all 48 matrix pictures at 256 and true colour changed, and
  every terrain at 256 now shows at least twice the tones it has at 16 (a test). 16-colour output is byte-identical (the golden hashes did not move).
  `Px.mix` gives the colour between where 16 colours dithered; a new `texture` pass (`recipes/vista-canvas.js`) grains the ground (coarse near the
  viewer, fine at the horizon), the sky (long soft streaks), water (horizontal shimmer) and rooms (even weathering), and at night lifts the peaks of
  the noise a cube step where dark colours have no near neighbour; night dims with real darker tones (no checker); fog, heat and torch glow
  blend instead of dithering. Indoors now draw at 256 (`drawIndoors` takes `depth`); before, an indoor 256 picture was the 16-colour one.
- Rule kept from now on: where a broader palette (256, true colour) is available, add texture with it, not just smoother colour.
- Matrix lint (`tools/vista-matrix.mjs`): 39 of 260 pictures with findings, all `ramp`.
- New `docs/coupler-handback.md` (per-gap status, lint, changed outputs, carry-over); parity doc, README, plan and Phase 7 brought up to date.

## 1.12.0 (Coupler gaps, Phase F: the figure on a place and the Fight panel)

- **Figure layer** (`recipes/figure.js`): a `Layer` canvas whose pixels carry a part (back, body, hair, eye, mouth, worn, brow, detail) and a
  lit flag; `lay` (offset, the body running off the right edge, `hide` for an invisible figure), `stamp` (left edge, mirror, flash),
  `lightFigures` (outdoors by day a figure pixel with none of the figure above it, not an eye, not lit, takes `LIGHTER`; at night outdoors every
  unlit one is `darken`ed; indoors nothing), the `LIGHTER`, `SHADE` and `RECEDE` tables, `wildTerrain` (an animal's place: woods, plains or hills
  chosen by FNV-1a of its key when its terrain is a room, city, spaceport or seaport) and `figureOnPlace(figure, place)`. Checked in the suite
  against Coupler's `figure.json`: the three tables, the eight `wild` examples and the three laid fixtures (day rim, night, indoors) match.
- **New recipe `onplace`** (ANSI only): a `face` or `thing` laid on a darker picture of a terrain (`terrain`, `time`, `weather`, `style`, `key`,
  `wild`, `invisible`, `offset`). Without a time the sun stands at one of four arcs picked from the key and is kept from behind the head.
  `creatureface` takes the same as a `place` param (Phase C step 8), clips included.
- **New recipe `fight`** (ANSI only, `recipes/fight.js`): the room with two faces, the player mirrored, the foe in front where they overlap;
  `layout: 'wide'` (25 by 12 cell faces, 40 to 160 columns) or `'phone'` (the small 16 by 8 faces at the ends of an 8-row strip, `words` between,
  32 to 160 columns; `pictures: false` gives four rows of words only). A big blow (`hit` and `fell` of 15 or more of 100) gives the `blow` clip
  (the one hit a cell back and flashed, the jolt alone, at rest; 100 ms a frame, flash only if `flash` is not false, so at most once a blow);
  `slide` gives the foe sliding in (24 ms a frame). `reducedMotion` takes out the jolt and the slide: the still, plus a flash alone where
  allowed. Frames pass `lintAnsiFrames`. New exports: `blowFrames`, `slideFrames`, `BIG_HIT`, `BLOW_MS`, `SLIDE_MS`, `FLASH_GAP_MS`.
- Kit `coupler-painter`: `figure-on-place` and `fight-panel` are covered; the kit has no gap rows left. 21 new variants; golden hashes added,
  none changed. `drawOutdoors` and `drawIndoors` take `raw` (return the pixel canvas); `vista` output is unchanged.
- To confirm with Coupler: `figure.json`'s fixtures show the place's colours unreceded (a light-blue sky stays light blue), though the table
  says `recede` ("a shade darker") is applied. Painter follows the table (`recede: false` turns it off); the phone strip's default width is 64.

## 1.11.0 (Coupler gaps, Phase E: portraits)

- **New recipe `creation`** (ANSI only, `recipes/portrait.js`; the pixel `portrait` bust keeps its name): a character-creation picture
  from 8 by 4 to 80 by 40 cells (two square pixels a cell). `kind: 'race'` stands a race beside a measuring stick marked every foot up to
  six, so its size shows against the others (height from inches, bulk from weight per inch, skin, hair or bald, clothes, a feature:
  pointed ears, tall hat, tusks, beard and helmet, bare feet or wings, red glowing eyes where it sees in the dark). `kind: 'class'` draws a
  person in the class's clothes with its tools: warrior (helmet, belt, sword, shield), caster (pointed hat, glowing staff), rogue (hood,
  dagger), priest (symbol, mace), nature (hood of leaves, growing staff), bard (feathered cap, lute), monk (sash, bare hands), crafter
  (apron and a hammer, book or rope by the trim). Robed kinds get legs in the clothes and a trim hem.
- **186 races and 49 classes** from Coupler's tables (`recipes/portrait-data.js`, generated by `tools/make-portrait-data.mjs`). A name the
  tables lack is made from the name by Coupler's own FNV-1a seed and xorshift64\* generator, draw for draw (`portraitSeed`,
  `couplerRng`): the test checks Coupler's test vectors and that the 150 races and the class the handover made this way come out the same.
- `says`: "Elf, 5 feet 5, pointed ears, sees in the dark." `facts` carries what it was drawn with. Params override the table (`inches`,
  `pounds`, `bulk`, `darkvision`, `skin`, `hair`, `clothes`, `feature`, `classKind`, `trim`). New exports: `raceFacts`, `classFacts`,
  `portraitSeed`, `couplerRng`, `bulkFor`, `CREATION_KINDS`, `CLASS_KINDS`, `RACE_FEATURES`, `RACE_NAMES`, `CLASS_NAMES`.
- Kit `coupler-painter`: `portraits` is covered. 20 new variants with golden hashes; no existing output changes.

## 1.10.0 (Coupler gaps, Phase D: item pictures)

- **New recipe `thing`** (ANSI only, `recipes/item-art.js`; `item` and `icon` stay the pixel icons): an item alone on a table, 25 by 24
  pixels on a half-block canvas (25 by 12 cells), its slot in words on the bottom row ("Wielded", "Worn on head"), drawn from its name,
  material, wear and auras. The kind is the first word of the name that names one (`thingKind`; a material word only if nothing else does;
  none gives a Bag for a container, else a Bundle). **56 kinds**: Coupler's 51 and the missing ones the kit listed, a jar, herbs, a map, a
  relic (idol, skull, orb) and a rune stone; spears and polearms (trident, halberd, glaive, lance, scythe), gauntlets and greaves, wands,
  and a torch as an item.
- Shape by cues in the name (scimitar curved, rapier thin, claymore long and broad, buckler small, crossbow, quiver, circlet, kilt,
  mail and plate); the name's seed picks the rest (a hilt, a pommel, a shield's shape, a rune's glyph).
- Material classes (`materialClass`, 217 materials to class and colour): cloth woven, leather stitched, wood grained, metal with a band of
  light, mithril sparkling, gems sparkling with facets, glass the backdrop seen through, rock speckled, the rest smooth. A handle keeps
  its wood and a fitting its metal; colour words in the name ("a black wool hood") win over the material's own.
- Wear (`thingWear`): worn, damaged, battered, broken (split in two along a jagged cut), rusty (metal only), crude (a rough outline),
  fine (gilt trim and a gem). Auras (`auraGlow`): glowing in its colour (a halo), magical (sparks), metallic or mithril (glints),
  invisible (half seen through), hidden (in shadow), floating or flying (lifted, its shadow below), bound (a rope).
- The drawing carries `figure` (colour and part per pixel) for Phase F and `says` ("Longsword, rusty iron, wielded.").
- Tables are generated by `tools/make-item-data.mjs` into `recipes/item-data.js` (the five added kinds and the polearm cues live in the
  script). New exports: `thingKind`, `materialClass`, `thingWear`, `auraGlow`, `readThing`, `THING_*`. 87 new variants; golden hashes
  added, none changed; no existing output changes.
- Kit `coupler-painter`: `item-pictures` is covered. Kit `fantasy-items-equipment`: the partial notes on polearms, gauntlets and greaves,
  jars and herbs, maps and relics, and wands and runes are gone; `camping-travel` stays partial (no tents, bedrolls or ropes). The painters
  are this package's own, judged by eye against Coupler's, not matched to them.

## 1.9.0 (Coupler gaps, Phase C: faces)

- **New recipe `creatureface`** (ANSI only, `recipes/face.js`; `face` stays the pixel face): a creature's head by race, 25 by 24 pixels
  on a half-block canvas (25 by 12 cells), or the **small 16 by 8 face drawn on its own strokes** (`size: 'small'`; never the big face
  scaled). 24 head shapes by Coupler's rule (`faceShape`: person, six beasts, bird, lizard, snake, frog, fish, dragon, insect, spider,
  worm, skull, fire, water, air, earth, golem, slime, plant); 242 races from the handover's table plus the category rule for the rest.
- Colours and cues from a description (`readFaceWords`): skin, hair, eyes and coat by the colour-word table, beard, mustache, scar, eye
  patch, freckles, tattoo, horns, tusks, fangs, glowing eyes, halo, old age; worn head, neck, ear, eye and back items.
- Six moods (`mood`: calm, angry, pain, afraid, asleep, dead) as eye, brow and mouth strokes at each shape's anchors, the evil-calm
  sneer, and the effect names that set a mood (`combat sleep`, `aura of fear`, `berzerk`).
- Hurt in five steps (`hurt`: a step, `{ health }` or CoffeeMUD health text through `hurtStep`) shown by material: flesh and scales bleed,
  fur mats, stone, bone and plants crack, ghosts and elementals fade, slime shrinks (`hurtClass`).
- Effects (`effects`: 160 names, or `looks`: 15 looks): halo, poisoned, sick, burning, frozen, blind, half-seen, outlined, webbed,
  stone, bark, glow. Burning and glowing are clips (`clips.loop`, a still in `reduced`) that pass `lintAnsiFrames`.
- The drawing carries `figure` (colour and part per pixel: back, body, hair, eye, mouth, worn, brow, detail) for Phase F, and `says`
  ("Orc, angry, bleeding, tusked.").
- Tables are generated by `tools/make-face-data.mjs` into `recipes/face-data.js`. New exports: `faceShape`, `readFace`,
  `readFaceWords`, `hurtStep`, `hurtClass`, `effectLook`, `FACE_*`. 74 new variants; golden hashes added, none changed; no existing
  output changes.
- Kit `coupler-painter`: `faces` is covered and `faces-small` (new row) is covered. The painters are this package's own, judged by eye
  against Coupler's grids, not matched to them; the face on its place (terrain) waits for Phase F.

## 1.8.0 (Coupler gaps, Phase B: the room picture)

- **`vista` outdoors redrawn on a half-block pixel canvas** (`recipes/vista-outdoors.js`),
  the way Coupler's painter draws: solid sky bands that dither only where
  they meet (Coupler's `skyStops`), a night that is the day in its night colours
  (`dim`) with the moon on the sun's arc, per-terrain ground with texture and
  a road in perspective, trees, towns lit at night, layered snow-capped
  mountains, dunes, swamp pools, sea floor and weed, a harbour with a ship,
  a spaceport with a tower and a ship, clouds below `air`. All 14 weathers
  show (windy, heat, drought and cold included); snow lies on land, not water.
  **Output changes for every outdoor `vista`** and for the four `locale`
  variants that draw one (`road`, short `road`, `iceplain`, `frozenpeaks`):
  golden hashes updated. 
- **Indoor `vista` rooms redrawn the same way** (`recipes/vista-indoors.js`;
  the canvas is shared in `recipes/vista-canvas.js`). Each material is laid as
  Coupler's `materials` table says, so stone, wooden, metal, magic, underwater
  and harbour rooms are told apart at a glance: bricks with mortar, planks,
  panels, runes; their own wall, side, floor and floor-joint colours; a torch
  with a flame and a glow on each side wall (braziers on the floor and banners
  on the back wall in high fantasy); the harbour room's channel and jetty,
  bubbles underwater, a glowing circle on the magic floor. Caves have a
  stalactite roof and a fire; the lake cave has water and the harbour cave a
  quay and a boat; the chasm has cliffs and a rope bridge. **Output changes for
  all ten indoor terrains**; golden hashes updated.
- **256 colours really use the colours between.** At `colors: 256` the sky
  and ground gradients are smooth runs of xterm-256 colours (nearest to the
  blend) instead of the 16-colour dither, and night darkens them by scaling
  the colour; before, 256 drew the same picture as 16. Underwater bubbles are
  small `o` characters, not blocks.
- Landforms tuned by eye against Coupler's pictures: mountains (more, steeper
  peaks, snow shaded by side, brown foothills), dunes (fewer, thicker), swamp
  (mud band, many small reeds), harbour water (lighter, fewer ripples),
  spaceport (near-parallel pad lines, a longer ship).
- Kit `coupler-painter`: `high-fantasy`, `colour-depths` and `no-sky-indoors`
  are covered; `terrains-*`, `time-of-day` and `weather` stay partial with what
  is still simpler than Coupler's.
- The `invisible` and `shade-noise` lint findings are gone from the whole
  260-picture matrix (74 and 4 pictures before): a half block carries both of
  its colours, so no glyph is drawn in its own background.
- **`vista` takes `bgLimit`** (default 8, a classic-legal picture as before;
  16 for the full picture with bright backgrounds, for iCE and modern files).
  `draw` hands `options.bgLimit` to a native recipe that takes it, and the CLI
  passes 16 for `--target ice|modern`.
- Tools (outside the package): `tools/vista-matrix.mjs`, `tools/compare.mjs`,
  `tools/check-coupler-data.mjs`.

## 1.7.0

- **CoffeeMUD gaps closed** (kit `coffeemud`: 26 subjects covered, none partial).
  Existing drawings are byte-identical; the golden file only gains variants.
  Creatures: 11 bodies (`horse`, `insect`, `ghost`, `elemental`, `fey`,
  `treant`, `griffon`, `naga`, `multiarm`, `centaur`, `merfolk`), about 35
  presets (undead, elementals, mephit, fey, mounts, the ten dragon colours)
  and `RACES`/`SIZES`: `draw('creature', { race })` for 25 races with a size
  table. Characters: 16 loadouts (shaman, archon, pirate, sailor, jester, six
  mage schools, shopkeeper, banker, librarian, smith). Items: 36 shapes
  (blades, thrown and cord weapons, arquebus, cannon, clothing, pills, bags,
  cards, instruments, household goods, food, apron, ledger). Props: 3 exits,
  6 traps, vehicles (boat, ship, cart, caravan, tent), tech and space (terminal,
  robot, scanner, spaceship, star, planet, gas giant, moonlet, asteroid), a
  clan hall. Furniture: stove, tub, fountain, piano, quest board, cage,
  counter. Rooms: tavern, shop, castle hall, ruin, sewer, stable, inn. Biomes:
  mountain, snow, jungle, road, plains, canopy. Effects: necromancy, illusion,
  blessing, disease. Icons: sleep, blind, silence, slow. New recipes: `locale`
  (ANSI; road lengths, ice plain, frozen peaks, tree surface, sewer, hot and
  ice rooms, maze, ship decks, starfield), `bar`, `mappane` and `sheet` (ANSI
  client surfaces), and `sigil` (clan sigils composed from field, ordinary and
  charge). `vista` is unchanged (its 24 terrains are Coupler's list).

- **Per-school spell effects** (CoffeeMUD gap 7, spells). Six new `effect`
  recipes, four frames each, no flashing: `fire`, `ice`, `bolt`, `curse`,
  `poison`, `summon`. Existing drawings are byte-identical; new golden hashes
  added. The `coffeemud` kit's `spells-and-effects` row lists them.

- **Status glyphs** (CoffeeMUD gap 7, statuses). Eight new `icon` glyphs:
  `freeze`, `stun`, `poison`, `disease`, `bleed`, `arrowdown` (debuff),
  `mana`, `curse`. Existing drawings are byte-identical; new golden hashes
  added. The `coffeemud` kit's `status-icons` row lists them. Per-school
  spell effects are still open.

- **Exit props** (CoffeeMUD gap 6). Nine new `prop` kinds: `gate`, `grate`,
  `curtain`, `ladder`, `stairs`, `pit`, `doorlocked`, `doortrapped`,
  `hiddendoor`. Existing drawings are byte-identical; new golden hashes added.
  The `coffeemud` kit's `exits` row lists them.

- **Item shapes and class loadouts** (CoffeeMUD gaps 2 and 3). 16 new item
  shapes: `spear`, `halberd`, `trident`, `whip`, `wand`, `gauntlet`,
  `bracers`, `cloak`, `robe`, `belt`, `lantern`, `waterskin`, `sack`, `map`,
  `bread`, `cheese` (each in four tiers), and six character loadouts: `bard`,
  `druid`, `paladin`, `ranger`, `monk`, `necromancer`. Existing drawings are
  byte-identical; new golden hashes added. The `coffeemud` kit rows for
  weapons, armor, magic and basic items and classes list them.

- **Creature plans: six new bodies and eight presets** (first CoffeeMUD gap).
  Bodies `bird`, `fish`, `snake`, `spider`, `floater` (a floating eye) and
  `golem`; presets `crow`, `fish`, `snake`, `spider`, `beholder`, `golem`,
  plus `bear` and `cat` on the existing `quad`. Existing drawings are
  byte-identical; new golden hashes added. The `coffeemud` kit rows for
  animal races, elementals and monsters now list them.

- **New kit `coffeemud`** (`kits/coffeemud.js`, from `docs/drawing-coffeemud-reference-kit.md`):
  26 subjects mapping CoffeeMUD's races, classes, MOBs, items, locales, exits,
  effects and UI to the recipes that draw them, with each gap named from the
  JavaDocs class lists. The doc gains section 9, the findings. No recipe or
  output changes.

- **ANSI plan, Phase 7 (started): the `vista` recipe and the Coupler parity
  inventory.** New ansiOnly recipe `vista` takes data, not Coupler's types:
  `{ terrain, time, weather, style, arc, colors, seed, columns, rows }`. All 24
  CoffeeMUD terrains (14 outdoor, 10 indoor), dawn/day/dusk/night with the sun
  and moon placed by `arc` (thousandths across the sky), 14 weather kinds
  (clouds, rain, snow, hail, fog, dust, a storm's lightning), `fantasy` or
  `high` style (castle, second moon, aurora, banners, braziers), 16 or 256
  colours (`resolveTones`). Sky shows only where there is one, so indoors and
  underwater ignore time and weather. Sizes are clamped as Coupler clamps
  them (8 to 160 by 4 to 45). The drawing returns `says`, the picture's facts
  in words. `roomSeed(world, room, again)` is Coupler's FNV-1a for the `seed`
  param. `hasSky` and `TERRAINS` are exported. New kit `coupler-painter` and
  `docs/coupler-painter-parity.md`: every file of Coupler's painter as a row
  with a status, so `sprites kits` is the migration list. Gaps: portraits,
  faces, item pictures, the figure-on-place layer and the Fight panel. The
  pictures are simpler than Coupler's and not byte-identical (judge by eye).
  Existing output is unchanged; the golden file gains the `vista` variants.
  `docs/coupler-handoff.md` is the file to drop into Coupler: what Painter
  already does, the data Coupler must export for the gaps, and the decisions
  that are Coupler's.

- **ANSI plan, Phase 6: the plain text style** (`style: 'text'`, chosen by the
  user 2026-10-29). Seven-bit ASCII on the same `Cells` grid with no colour,
  for MUD clients, logs and colourless terminals. `draw(id, params, { style:
  'text' })` uses a recipe's own `text` drawing if it has one, else its ANSI
  drawing read as ink (`asciiFromCells`: ASCII glyphs stay, box drawing
  becomes `- = | +`, spaces, blocks and shades become `. : - = + * # % @` by
  the brightness of their `tone`, other characters `*`), else its pixel
  drawing read as light and dark (`foldText`: two square pixels to a cell, an
  opaque pixel is never a space, a lone upper or lower pixel is `'` or `,`).
  `STYLES` is now `pixel`, `ansi`, `text`, and `recipeStyles` lists `ansi` and
  `text` for an ansiOnly recipe. `writeText(cells, { eol })` writes the file:
  `?` and a warning for anything outside 7-bit, trailing spaces cut, no SAUCE.
  `lintText` checks `ascii`, `width`, `height`, `blank`, `fill` and `colour`.
  CLI: `--style text` on `draw`, `sheet`, `lint` and `export` (`.txt`, or a
  `.png` render with the VGA font). Pixel and ANSI output is unchanged; the
  golden file gains a `text` hash for every variant.

- **Image-to-cells study converter** (the optional step after Phase 4).
  `fromImage(image, { columns, colors, bgLimit, contrast })` takes a `Pixels`
  canvas or `{ w, h, rgba }` and returns `{ cells, warnings }`: luminance
  stretched first (2nd to 98th percentile), Lanczos-3 resize, two square
  pixels to a cell so half blocks absorb the 2:1 ratio, then each cell is the
  glyph (space, `░ ▒ ▓ █ ▀ ▄`) and colour pair of least error against its two
  halves, legal for `bgLimit`; `colors: 256` takes the nearest of the 256 for
  each half. Pixels under half opacity are empty cells. A drafting aid, not a
  recipe: it never fetches, and its output is not engine art. CLI:
  `sprites convert <image.png> --out f.ans|png|txt [--columns 80 --colors
  --target]`. The `ansi-art` kit's `image-to-cells` subject is built in, so
  that kit has no gaps left. No existing output changes.

- **ANSI plan, Phase 5: depth, light and animation.**
  `gradient(cells, x, y, w, h, ramp, { dir, bgLimit })` fills a region with a
  ramp's `░ ▒ ▓` ladders (vertical by default; under four cells along it is an
  error, the kit's room rule). `bayer(...)` is a 4 by 4 ordered dither between
  two colours, solid cells, no randomness. `tones(cells)` is the in-between
  colour of every cell as `#rrggbb` (what a truecolor viewer paints), and
  `resolveTones(cells, { depth })` turns `░ ▒ ▓` cells into solid cells of the
  nearest of the 256 (or the 16, legalized), for `writeAnsi(..., { colors: 256 })`.
  Cells still hold palette indexes, so truecolor output reads `tones` rather
  than the cells. `light(cells, { ambient, sources, selfLit })` is `relight`'s
  counterpart: whole steps along the hand-made ramps (negative ambient for
  night), sources that fall off to nothing at their radius with a cell counted
  twice as tall as wide, and self-lit cells (a `Cells` mask, as a folded
  drawing's `mask`, or a predicate) that take no ambient; the result is
  legalized. `writeAnsimation(frames, { baud, fps, sauce, ... })` writes an
  ANSImation: a full first frame after `ESC[2J ESC[H`, then, per frame, a bare
  `ESC[H` mark and `ESC[row;colH` redraws of only the changed stretch of each
  changed row (cells that leave are blanked); it reports the bytes and seconds
  each frame takes at the stated baud (default 2400), warns when later frames are
  slower than the clip's fps, writes SAUCE with the ANSiMation file type, and
  returns `still`, the last frame as an ordinary `.ans` for reduced motion.
  `readAnsimation(bytes)` replays one into a `Cells` per frame mark (a plain
  `.ans` is one frame; without SAUCE the width is `width` or 80 and the height
  is the last row drawn). New ansiOnly recipe `weatherloop` (`weather` rain or
  snow, `frames`, `time`): a landscape with falling weather as a seamless clip
  `loop`, the first frame being the still. The `ansimation` subject of the
  `ansi-art` kit is now partial (falling weather only), leaving
  `image-to-cells` as its only gap. CLI: `sprites draw <recipe> --style ansi
  --clip <name> --out f.ans [--baud n]` writes the ANSImation and `f.still.ans`
  beside it. `writeAnsi`'s byte writer moved into `makeEncoder` in `ansi.js`
  (shared with the animation writer) and SAUCE sealing into `withSauce`; every
  existing recipe's pixel, folded and `.ans` output is unchanged (the golden
  file only gains the two `weatherloop` variants).

## 1.3.0 (2026-10-29)

- **ANSI plan, Phase 4: ANSI lint, the first native recipes, the `ansi-art` kit.**
  `lintAnsi(cells, { target, width, maxHeight, depth, skip })` checks width,
  height (over 24 rows for a classic target), colour indexes per depth,
  `bg-limit` (a bright background in a classic file), `blink`, `charset` (outside
  CP437), `invisible` text, `contrast`, `shade-noise` (lone shade glyphs),
  `ramp` (a gradient under four steps; dark gray beside white) and `outline`;
  `lintAnsiFrames` adds identical frames, a long loop and `flash` (over three
  flashes a second is an error). `gap` (shapes touching) is not written yet.
  `ansi-craft.js`: `paint` runs a drawing's steps in the artists' order (focal,
  outline, base, highlights, shadows, blend) so the outline survives; `runSteps`
  is the same order for a canvas that is not cells; `overlay` scatters `stars`,
  `rain`, `snow`, `waves` or `runes` (CP437 glyphs, by seed, only over empty
  cells); `lettering` makes block lettering from a hand-made 3 by 5 font in
  four faces (`half`, `block`, `outlined`, `shaded`). Five new recipes with no
  pixel drawing (`ansiOnly`: draw them with `style: 'ansi'`; the pixel style
  says so and throws): `banner`, `landscape` (banded sky by time of day,
  two ranges, a lake, a green ground, rain or snow), `menu` (80 by 24, double
  frame, shaded title, boxed list), `bust` (80 by 48 pixels folded to half
  blocks, three-value lighting from either side, dark gray outline) and
  `artpack` (title, framed landscape, credit, with SAUCE). The `ansi-art` kit
  lists the five practice briefs, block lettering and overlays as covered or
  partial, and ANSImation and the image converter as gaps. `sprites kits`
  shows `[ansi: native|folded|mixed]` per subject; `sprites lint` takes
  `--style ansi` or a `.ans` file (`--target`, `--colors`, `--width`); an
  artpack's SAUCE goes into its `.ans`. Every existing recipe's pixel and
  folded output is unchanged (golden hashes); the golden file only gains the
  new variants.

## 1.2.0 (2026-10-29)

- **ANSI plan, Phase 3: rendering cells to pixels, and the CLI.** `renderCells(cells,
  { atlas, theme, letterSpacing, bold })` (browser-safe) draws cells as an RGBA
  image with the standard font, IBM VGA 8x16: 8 by 16 pixels a cell, or 9 by 16
  with `letterSpacing: 9` (line and block characters repeat their eighth column).
  The glyphs come from `docs/fonts/ibm-vga-8x16.png`, an atlas of images of the
  unmodified font's glyphs (CP437 order) made by `tools/render-vga-atlas.py`,
  outside the package; the credit and VileR's licence sit beside it
  (`docs/fonts/`). `loadAtlas()` (`art-sprites/atlas`, Node) reads it; a
  character outside CP437 draws as `?` and is listed in `missing`. `rgbaSheet`
  (`art-sprites/sheet`) lays rendered cells on one PNG. CLI: `sprites draw
  <recipe> --style ansi --out f.ans|f.png|f.txt` (the file name picks the
  form), `--target classic|ice|modern`, `--colors 16|256|true`, `--charset
  cp437|utf8`, `--spacing 8|9`; `sprites sheet --style ansi` makes a PNG; `sprites
  export --style ansi` writes `.png`, `.ans` and `.txt` per frame and records
  the style and ANSI settings; writer warnings and the charset written go to
  stderr. `sprites lint --style ansi` says it waits for Phase 4. Nothing
  existing changes; golden hashes are untouched.

- **ANSI plan, Phase 2: the ANSI writer and reader** (`ansi.js`, browser-safe,
  no Node built-ins). `writeAnsi(cells, { target, charset, colors, sauce })`
  makes the bytes of a file: `classic` (CP437, 8 backgrounds, legalized
  first), `ice` (bright backgrounds as the blink bit; the SAUCE iCE flag only
  when the picture uses one) or `modern` (UTF-8, `90`/`100` codes). Reset
  first, an escape only when attributes change, bold for a bright
  foreground, trailing spaces cut only over no background, CRLF even on a
  full-width row, Ctrl-Z, comments, then the 128-byte SAUCE (title, author,
  group, date, size, iCE, letter spacing, aspect, font `IBM VGA`). The SAUCE
  date is a parameter, never a clock. Anything that had to change (depth
  reduced above 15, a character outside CP437 written as `?`, blink dropped
  in iCE) comes back in `warnings`. `readAnsi(bytes | string, { width,
  charset, ice })` inverts it: SGR (16, 256, truecolor, bold, blink, reverse),
  cursor moves, clear, line erase, deferred wrap, tabs, CP437 or UTF-8
  detection, SAUCE width and iCE, stop at Ctrl-Z, 400 by 1000 cap.
  Also `CP437`, `toSpans` (runs of one style, for a page that colours the
  text itself) and `toPlainText` (the fallback beside a `.ans`). Nothing
  existing changes; golden hashes are untouched. Tests round-trip only
  pictures this engine drew.

- **ANSI plan, Phase 1b: legal cells and a drawing kit.** A classic `.ans`
  has 8 background colours, so `legalize(cells, { bgLimit })` (8 classic, 16
  for iCE and UTF-8) reverses `▀`/`▄` and `░`/`▓`, turns a space on a bright
  background into a `█`, and where both colours are bright picks the glyph
  and colours that look least unlike the cell. `foldPixels`, `foldArt` and
  `draw(..., { style: 'ansi' })` take `bgLimit` and default to 8.
  **Folded (`ansi`) output changes** for every cell with a bright
  background, and the folded golden hashes were re-accepted; pixel output is
  unchanged (no `pixel` hash moved). Also: `tone(cell)` and `halves(cell)`
  (the effective colour of a cell), `legalCell`, `ladder(a, b)` (the five
  steps, legal at the limit), `rampCells`, `ANSI_RAMPS` (hand-made: gray,
  blue, fire, green, magenta), and on `Cells`: `box` (single, double,
  mixed, with CP437 corners and joins), `hline`, `vline` and `shadeFill`.

- **Standard font rule**: checked every ansiapps app; all use IBM VGA 8x16
  by VileR (CC BY-SA 4.0) on an 8 by 16 grid, 16px only. It is now
  Painter's standard font (`CLAUDE.md` rule 8, plan section "The standard
  font"). Phase 3 renders from a PNG glyph atlas of the unmodified font, not
  from a bundled font file or a typed glyph table.

- Studied `docs/cc0-ansi-art-learning-reference-kit.md` and the technique
  pages it links; `docs/ansi-expansion-plan.md` now records the findings and
  the phase changes. The main one: a classic `.ans` allows only 8 background
  colors (bright ones need iCE), which Phase 1's fold does not yet respect,
  so a new **Phase 1b** (legal cells, `tone`, box and ladder primitives,
  `ANSI_RAMPS`) comes before the writer. Phases 2 to 5 gain the kit's file
  rules, lint rules, practice-brief recipes and a block-lettering `banner`.

- **ANSI plan, Phase 1: the cell canvas.** `Cells` (`cells.js`): a grid of
  a character and two palette indexes (0 to 255) a cell, with `set`,
  `rect`, `fill`, `text`, `glyph`, `blit`, `clone`, `equals`, `toText`.
  `foldPixels` folds a pixel canvas to cells, two square pixels a cell
  as `▀`/`▄` half blocks, and `foldArt` folds a whole drawing (frames,
  clips, layers, masks; anchors move to cell units). `palette-ansi.js`:
  `VGA16`, `V` (named indexes), `XTERM256`, `nearest(hex, 16|256|'true')`,
  `indexHex`, and `DB32_VGA`, a hand-made table from DB32 to the 16.
- **`draw(id, params, { style: 'ansi' })` now works for every recipe**:
  the recipe's native `ansi` drawing if it names one, otherwise its pixel
  drawing folded (`folded: true`). `isNative(id, style)` tells which. The
  CLI's `--style ansi` says its output is not written yet (Phase 2).
  Golden hashes now cover the folded drawings too.

- **ANSI plan, Phase 0**: a drawing `style` (`pixel`, the default, and `ansi`,
  not drawable yet). `draw(id, params, { style })`, `STYLES`,
  `DEFAULT_STYLE` and `recipeStyles(id)` are exported, and every drawing
  command of the CLI takes `--style`. A recipe gains a style by naming a
  native drawing in its `ansi` field; asking for a style it lacks throws
  "recipe X has no ansi drawing yet". Pixel output is unchanged: a new
  golden test (`test/golden.test.ts`, `test/golden.json`) hashes every
  registered variant and fails if any drawing changes; accept a change on
  purpose with `UPDATE_GOLDEN=1 npm test`.

- **Renamed `pixel-sprites` to `art-sprites`**: the engine will draw
  styles other than pixel art. Import from `art-sprites`; the `sprites`
  command is unchanged. Idle Solo's copy keeps the old name until it is
  updated.
- Added `docs/ansi-expansion-plan.md`: what Coupler's ANSI painter does,
  and the phases to give the engine an ANSI output (cell canvas, ANSI and
  SAUCE writer and reader, CLI, lint, recipes, depth and ANSImation).
  Decided: pixel recipes fold to cells and native ANSI recipes are built
  too; Coupler's ideas are ported, not its code; icy_tools stays out;
  Painter replaces Coupler's painter and is the only art engine for all
  ansiapps apps (a new Phase 7 covers the move).

- Added `CLAUDE.md` (operating rules), this changelog and a `.gitignore`,
  following the conventions of the other apps in `~/Coding`.

## 1.0.0 (2026-10-29)

- **Modular sprite drawing system** and the hero paper doll (2026-10-28):
  seeded, deterministic recipes on an indexed DB32 canvas, with lint, a
  PNG codec, contact sheets and the `sprites` CLI.
- **Hands, poses, turnarounds, cloth motion and interiors** (2026-10-28):
  `hand`, character `pose` and `view`, `cloth`, `furniture`, `room` and
  `relight`.
- **Portable, self-contained package** (`pixel-sprites`, 2026-10-28):
  nothing imported from outside the folder, no Node built-ins behind the
  main entry, types for every entry.
- All game art is drawn with the engine (2026-10-28).
- The modular sprite system design doc lives in `docs/`, and the README
  points at it (2026-10-29).
