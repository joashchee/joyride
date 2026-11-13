# Plan: closing the Coupler gaps (`coupler-painter` kit)

**Written 2026-10-29**, from everything in `docs/coupler-handover/`:
`docs/coupler-handoff.md`, `docs/gaps-list-from-coupler.md`, and
`painter-data/` (B2 to B6 JSON, 260 Rust-painter pictures in `reference/`,
the engine's 1.3.0 pictures in `engine/`). Order is Coupler's (decision C7):
**G2, G3, G4, G1, G5**, with G1 allowed in parallel. Coupler's bar is "as
informative as the Rust painter's, judged by eye", not byte-identical.

## What is being fixed

| Gap | Kit row(s) | Size in Coupler | Data |
|---|---|---|---|
| G2 room fidelity | `terrains-*`, `time-of-day`, `weather`, `high-fantasy`, `colour-depths` (partial) | `painter.rs` | `painter-tables.json`, `reference/` |
| G3 faces + small face | `faces` (gap) | `face.rs`, 3080 lines | `shapes words moods hurt effects small-face sitters faces` |
| G4 item pictures | `item-pictures` (gap) | `item_art.rs`, 1938 | `kinds materials shape-numbers wear things` |
| G1 portraits | `portraits` (gap) | `portrait.rs`, 617 | `races classes portrait` |
| G5 figure + Fight | `figure-on-place`, `fight-panel` (gap) | 198 + 334 | `figure fight` |
| G6 supporting kit rows | partial rows the above lean on | n/a | none |

Done for the whole plan = the kit reads **0 gap, 0 partial** for
`coupler-painter`, `lintAnsi` clean on the matrix, every changed existing
output named in `CHANGELOG.md`, a fresh contact sheet beside `reference/`.

## Rules that shape every phase

- Recipes take **data, not Coupler types**; the JSON becomes engine tables
  in `recipes/` (or a `data/` module of plain JS objects, since the main
  entry cannot read files). Each table is a literal in source, generated
  once by a script outside the package and reviewed, never fetched or
  `import`ed from `docs/` (rule 2, and the docs folder is not shipped).
- Rule 1: all randomness from `rng(seed)`. The handover gives Coupler's
  FNV-1a and xorshift64\* with test vectors (`rngFirstSix...`); implement
  them exactly **only** where a fallback must match (unknown race or class
  made "from its name"); elsewhere use `rng`.
- Every recipe is registered with `variants()` and a kit row (rule 7),
  ANSI-native (`ansiOnly` where no pixel form makes sense), linted with
  `lintAnsi`, and covered by golden hashes. Words beside the picture come
  back as `art.says`.
- `vista` output changes in G2 are deliberate: say so in the changelog with
  the manifest file names that changed (Coupler's rule 12).
- Version: each phase ends in a minor bump (1.8.0 ...) and a "checkpoint"
  suggestion; never automatic.

---

## Phase A: groundwork and the review harness (done 2026-10-29; Phase B is next)

As built, in `tools/` (outside the package, like `render-vga-atlas.py`): `vista-matrix.mjs` (the 260-picture matrix, per-picture lint with the right target, `--baseline`/`--check` against `tools/vista-baseline.json`), `compare.mjs` (reference left, engine right), `check-coupler-data.mjs` (the data's counts, vectors and a no-dev-path check). Lint on the matrix reads 123 of 260 pictures with findings: `invisible` 74, `shade-noise` 4, plus `contrast` 93 and `ramp` 7, which Coupler's report (made with an older lint) did not list; they are Phase B's to judge. The scripts the plan names below as `scripts/` are these `tools/` files.

1. **Decide where the handover lives.** It is 12 MB, untracked. The handoff
   asked for `docs/coupler-data/`; the folder is `docs/coupler-handover/`.
   Keep that name, record it in CLAUDE.md "Where things live", and decide
   whether the 260 reference `.ans`/`.png` are committed (recommend: commit
   `docs/` and the JSON, keep `reference/` and `engine/` as the user's local
   comparison set, `.gitignore` them, since they are Coupler's output and
   bloat the repo). The security check found no `/Users/` paths in the data.
2. **Lint false alarms first** (so later counts mean something): lint the
   matrix per target (`width`/`height` on 160 by 45 and `bg-limit` on 256 and
   true are the harness's fault, per the gaps list). Add a script
   `tools/vista-matrix.mjs` that draws the same 260-picture matrix from
   `reference/manifest.json` (names match) with the right lint target and
   prints a per-file `lint` table.
3. **Side-by-side tool.** `tools/compare.mjs name` writes one PNG with
   `reference/<name>.png` and the engine's drawing beside each other, and a
   `--sheet` mode for a terrain row. This is the "judged by eye" loop for
   G2 to G5.
4. **Importer.** One script, outside the package, that turns the B2 to B6
   JSON into the literal tables each phase needs and checks counts (186
   races, 49 classes, 33 sitters, 51 kinds, 217 materials, 63 things) so a
   re-export from Coupler is a rerun, not a rewrite.
5. **Pin the baseline:** record the current `vista` golden hashes so G2's
   intentional changes are visible diffs.

Done when: the matrix script reproduces Coupler's 106-of-260 lint report and
the compare tool shows `city_night-thunderstorm` beside its reference.

## Phase B: G2, the room picture (`vista`), the most seen (done 2026-10-29 apart from what is listed below, awaiting Coupler's by-eye sign-off; Phase C is next; was: steps 1, 2, 3, 5 and most of 4 and 7 done for outdoors; step 6, indoors by material, done the same day)

As built: outdoors is a half-block pixel painter in `recipes/vista-outdoors.js` (the key decision: Coupler paints pixels two to a cell, so the engine does too, which also removes `invisible` by construction). `vista` takes `bgLimit` (8 default, 16 for iCE and modern). Indoors is `recipes/vista-indoors.js` on the shared `vista-canvas.js`. Tuned by eye (swamp, harbour, spaceport, mountains, dunes, underwater) and 256 colours now smooth natively. Still open: weather marks and fog/wind/heat not matched to Coupler's, no per-hour sky, the `ramp` lint warnings kept as warnings (39 pictures: white against dark gray in towns, snow and pad lines, which Coupler's pictures also have). Then Coupler judges the contact sheet and wires `vista` in behind a flag.

Order inside the phase is by how much each shows (Coupler's observations).
Do it as small steps, each re-running the matrix and the compare tool.

1. **Kill `invisible` and `shade-noise`** (74 and 4 pictures). Find the
   source (a glyph drawn in its own background colour: dither where fg ==
   bg). Fix at the helper (`ansi-craft`/`vista`) so it cannot recur; add a
   test that the matrix has zero. Changes output: changelog.
2. **Sky as solid bands, dither only at joins.** Use `skyStops` and the
   `dim` table from `painter-tables.json`; **night is the day drawn in its
   night colours with the moon on the sun's arc**, replacing "lit down with
   `light` plus stars" (keep stars as an overlay if the reference has
   them). 256 and true colour smooth between stops via `resolveTones`;
   confirm 256 really uses in-between colours.
3. **Ground and horizon shares** from `horizon` (45 percent, mountains 60,
   watersurface and seaport 50, air all). Replace the grey slab with
   per-terrain ground: dithered paving (`city`), grass (`plains`, `woods`,
   `hills`), sand, mud, rock; road as a perspective wedge (`recede`
   table); `city` lit windows and streets so night is lit, never black.
4. **Per-terrain landforms**, the checklist from the gaps list:
   jungle canopy, swamp water and reeds and mist, air with clouds below,
   underwater sea floor and weed and bubbles and light shafts (`underwater
   Stops`), seaport ships and jetties, spaceport pads and ships and lights,
   mountains as layered ranges with snow shaded by depth, desert dunes
   and ruins. These lean on G6 rows (`village-castle`, `coast-river-swamp`,
   `mountain-cave-dungeon`, `desert-ruins`): build the needed props there
   (as recipes in `locales`/`interiors`/`tiles`) and have `vista` call
   them, so the kit rows move too.
5. **Weather:** make `windy`, `heat`, `drought`, `cold` visible (`weather
   marks` table); snow lies on land only, not water or stone.
6. **Indoors by material** from `materials` rows: bricks with mortar
   (stone), planks (wooden), panels (metal), runes (magic), laid floors,
   own wall/joint/side/floor/ceiling colours; `innerseaport` channel and
   jetty, `in_underwater` bubbles, `magic` glowing floor circle, cave
   lake and quay, `gap` chasm; torches with flame and wall glow.
7. **High fantasy:** second small red moon on its own arc, castle towers on
   every landscape except `air`, `city`, `spaceport`; banners and braziers
   indoors.

Done when (Coupler's list): each terrain reads as itself at 80 by 24 and 160
by 45; stone/wooden/metal/magic rooms are told apart without names; night
city and night plain are lit; sky is solid bands; the four weathers show;
`invisible` and `shade-noise` are 0 on the matrix; contact sheet accepted by
eye. Kit rows `terrains-*`, `time-of-day`, `weather`, `high-fantasy`,
`colour-depths` become `covered`. Coupler wires `vista` in behind a flag
after this.

Release 1.8.0.

## Phase C: G3, faces (the biggest) with the small face (done 2026-10-29, awaiting Coupler's by-eye sign-off)

As built: recipe `creatureface` (the plan's `face` name is the pixel face's), `recipes/face.js` and the generated `recipes/face-data.js`. Steps 1 to 7 and 9 are done; step 8 (the face in its terrain) is Phase F's. Out of the plan's wording: the painters are one per shape (24), not per family; the `healthText` mapping is by keywords (`hurtStep`), since the handover names no exact strings, so Coupler should confirm it against the game's ten lines.

New module `recipes/face.js` (ANSI-native, `faceArt` recipe name `face`;
keep the existing `faces`/`bust` untouched).

1. **Shapes.** Table from `shapes.json`: 242 races to category to shape, the
   per-shape numbers (oval radii, ear, muzzle, beak, eye `Anchors`); the
   person/animal/sapient rules and the thinking-races list. A
   `faceShape(raceOrCategory)` helper exported for the recipes.
2. **Base heads, one painter per shape family** (person, canine, feline,
   ursine, rodent, equine, bovine, bird, reptile, fish, insect, spider,
   skull, elemental, golem, slime, plant, rest). 25 by 24 pixels on `Cells`,
   painted in artists' order (`paint`: focal, outline, base, highlights,
   shadows, blend). Check against `faces.json` ground-truth grids by eye.
3. **Words to look:** `words.json` colour-word table, stop-words, the cues
   (beard, scar, eye patch, freckles, horns, tusks, fangs, glowing eyes,
   halo, old age), item colours. `face` takes either explicit params
   (`skin`, `hair`, `beard`, ...) or a `description` string run through the
   cue parser, so Coupler passes the game's own description.
4. **Moods** (calm, angry, in pain, afraid, asleep, dead) as eye/brow/mouth
   strokes per shape, plus the evil-calm sneer; `effectsSetMood` map.
5. **Hurt steps 0..5**, per material class (flesh bleed, fur mat, stone and
   bone crack, ghost/elemental fade, slime shrink); the `healthText` to step
   mapping goes in as a pure function `hurtStep(text)`.
6. **Effect overlays** from `effect_looks` (160 effect names to 15 looks:
   halo, poison, burning, frost, blind, half-seen, outline, web, stone,
   bark, glow). Animated ones (burning, glow) are clips and must pass
   `lintAnsiFrames` (at most three flashes a second).
7. **Small 16 by 8 face** from `small-face.json`'s own strokes, never the big
   face scaled. This also closes kit row `faces-small`.
8. **The face in its terrain** (a shade back of the vista): hook it up here
   as an optional `place`, finished in Phase F.
9. `says`: e.g. "Orc, angry, battered." Tests: all 33 sitters at both sizes
   with every mood and hurt step, deterministic, lint clean; a hurt flesh
   face and hurt stone face differ; golden hashes; `kits --strict`.

Done when matches the gaps list. Release 1.9.0.

## Phase D: G4, item pictures (done 2026-10-29, awaiting Coupler's by-eye sign-off; Phase E is next)

As built: recipe `thing` (the plan's `item` name is the pixel icon's), `recipes/item-art.js` and the generated `recipes/item-data.js`. Steps 1 to 7 are done. The painters lay down parts (main, metal, wood, trim, gem) and one finish colours them, so every kind takes every class and wear step the same way. Out of the plan's wording: 56 kinds (Coupler's 51 plus jar, herb, map, relic, rune; gauntlets, greaves, wands and the polearms are words and cues of existing kinds); a material class changes the texture and colour of the kind's main part, and a handle keeps its wood and a fitting its metal; a potion is glass whatever the word. Wear step 4 splits the item in two along a jagged cut; `rust` marks metal only. The aura pictures are stills (no clip).

New module `recipes/item-art.js` (recipe `thing`; existing `items`/`icons`
stay pixel).

1. **Missing kinds first** (they are also kit gaps): spear and polearms,
   gauntlets, greaves, wands and runes, jars and herbs, maps and relics,
   torch outside camping. Drawn in native ANSI, 25 by 24.
2. **Kind from the first word** (`kinds.json`, 51 kinds, slot text), the
   unknown word falls back to a bundle.
3. **Shape numbers** per kind and per name word (scimitar curved, rapier
   thin, claymore long), with the name's seed filling the rest
   (`shape-numbers.json`).
4. **Material classes** (217 materials to 9 classes) and the texture each
   draws: cloth woven, leather stitched, wood grained, metal with a band of
   light, mithril and gems sparkling, glass see-through, stone speckled.
5. **Wear** (worn, damaged, battered, broken, rusty, crude, fine) and **aura
   overlays** (glowing in its colours, magical, metallic or mithril,
   invisible, hidden, floating, bound).
6. `says` carries slot in words: "Longsword, rusty iron, wielded."
7. Tests: the 63 `things` fixtures draw; every kind in every material class
   and wear step is lint clean; a rusty iron sword, a fine mithril sword and
   a glowing wooden staff differ. Items-equipment kit rows move.

Release 1.10.0.

## Phase E: G1, portraits (done 2026-10-29, awaiting Coupler's by-eye sign-off; Phase F is next)

As built: recipe `creation` (`kind: 'race' | 'class'`; the pixel `portrait` keeps its name), `recipes/portrait.js` and the generated `recipes/portrait-data.js`. Steps 1 to 5 are done; the 150 races and one class the handover made from their names are checked against the fallback in the suite.

New module `recipes/portrait.js` (recipe `portrait`; the old `portrait`
pixel recipe stays under its name in the pixel kit: rename the new one to
`race-portrait` / `class-portrait` or take a `kind` param, decide at the
start, since a name clash breaks rule 7).

1. Race table (186) and class table (49) with players, fixtures, bulk by
   kind, robed rule.
2. Builder per `portrait.json`: ground row, foot size, stick marked every
   foot to six, figure formulas, wings/feature/kind tools, paint order.
3. **Both fallbacks bit-exact:** FNV-1a seed, xorshift64\*, draw order;
   test against `rngFirstSixOfRaceKobold` and `...ClassKnight`.
4. Sizes 8 by 4 to 80 by 40; `says`: "Elf, 5 feet 5, pointed ears, sees in
   the dark."
5. Tests: all 186 races and 49 classes at 32 by 16 and 80 by 40, lint
   clean; Half Ogre taller than Human taller than Gnome taller than Pixie;
   unknown name stable; darkvision eyes red, others not.

Release 1.11.0 (or folded into the C or D release).

## Phase F: G5, figure on a place and the Fight panel (done 2026-10-29, awaiting Coupler's by-eye sign-off)

As built: `recipes/figure.js` (layer, `lay`, `stamp`, rim light, `wildTerrain`, `figureOnPlace`), recipes `onplace` and `fight` (`recipes/onplace.js`, `recipes/fight.js`), and a `place` param on `creatureface`. Step 1 was already done in C and D (`figure` on each drawing). The tables, `wild` examples and fixtures are checked against `figure.json` in the suite. Settled: the table rules, so a place recedes (Coupler, 2026-10-29).

1. **Part-tagged layer:** a `Figure` canvas where each pixel carries a part
   (back, body, hair, eye, mouth, worn, brow, detail); `face` and `thing`
   from C and D gain a `parts` output.
2. **`lay`** (offset, hide) and the rim-light pairs (`lighter`, `shade`
   16-entry tables), `wild(terrain, key)` for an animal's place, producing
   `figureOnPlace(figure, vista)` so the place shows through the rim light
   (done when a face on `woods` dusk and on `stone` look different).
3. **`fight` recipe:** layout from `fight.json` (columns, face size, mirror,
   the phone strip of eight rows with 16 by 8 faces and words between).
   Frames as clips: jolt shift and flash for a big blow, slide in for a new
   foe. Param `reducedMotion` returns a still (no jolt or slide, no
   flash). Flash at most once a blow, never more than three a second.
4. Tests: clips pass `lintAnsiFrames`; reduced-motion version is one frame.

Release 1.12.0.

## Phase G: G6 and the close (done 2026-10-29)

As built: nothing new was drawn. Step 1 took only what blocks a done-when, and none of the listed rows does: Coupler's kit has no gap, and the
other kits' partials (`pose-foreshortening`, `turn-three-quarter`, `motion-robes`, `range-angle`, `range-phenotype`, `historical-monsters`,
`magical`, `readability`, `interior-architecture`, `shaded-landscape`, `ansimation`) stay honestly partial or gap, reported in the README's
"Known gaps". Faces draw all their shapes without the demon, chimera and harpy body plans. In `coupler-painter`, `scene-input`,
`accessibility-words` and `ansi-file` moved to covered; `terrains-outdoor`, `terrains-indoor`, `time-of-day` and `weather` stay partial until
Coupler judges them by eye, so the plan's "0 partial" is not met and is said so. Steps 2 to 4: the parity doc, README, CHANGELOG, Phase 7 in
the expansion plan, `docs/coupler-handback.md` (the per-gap table, changed outputs, carry-over) and a fresh `engine/` set beside `reference/`.

1. Remaining partial kit rows not already moved by B to F: `pose-foreshortening`,
   `turn-three-quarter`, `motion-robes`, `range-angle`, `range-phenotype`,
   `historical-monsters` (demon, chimera, harpy body plans: needed by
   face shapes), `magical` (floating islands, portals), `readability`
   (foreground/midground/background layers), `interior-architecture`,
   `shaded-landscape`, `ansimation` (storm, fire). Take only what blocks a
   done-when; the rest stays honestly `partial` and is reported as such.
2. Update `docs/coupler-painter-parity.md`, the kit rows (`covered`), the
   README, `index.d.ts`, CHANGELOG, `docs/ansi-expansion-plan.md` Phase 7.
3. Fresh contact sheets and matrix lint report for Coupler; a handback note
   listing for each gap: kit row status, lint result, changed outputs.
4. Carry-over note for copies elsewhere (Idle Solo).

## Dependencies and risks

- B can start immediately; C needs A's importer; F needs B and C (and D for
  items); E is independent.
- **Biggest risk is B's scope** (24 terrains by weather by time). Mitigate by
  landing it in the numbered steps with a release at the end of 2 to 3 of
  them, and by judging by eye each time.
- Data fixtures are synthetic (not real Bestiary entries), so "done" is the
  fixtures, not Coupler's live game; ask Coupler to rerun
  `scripts/painter-data.sh` and render a new reference at any phase end.
- Rust `painter.rs` rules the picture; port ideas never code, never import
  Coupler (rule 6 of the repo), and the Coupler rule edits (6, 11, 12) are
  their session's.
- Every phase changes shipped surface: keep the d.ts in step, `npm test` and
  `node cli.js kits --strict` green before each checkpoint.
