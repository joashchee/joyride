# art-sprites

A dependency-free art drawing engine (pixel art today; ANSI art next, see docs/ansi-expansion-plan.md): seeded, deterministic recipes draw items, faces, creatures, modular
paper-doll characters, hands, cloth, rooms, biome scenes and effects onto an indexed DB32 canvas, with a lint, a PNG codec and
canvas helpers. Same recipe, params and seed, same pixels. It imports nothing outside this folder, and the main entry has no Node
built-ins behind it, so it runs unchanged in a page, a worker, Node or Deno. Two tests enforce both.

## Use it in another project (Tag Team, ...)

Pick one; nothing else needs installing.

| How | Command |
|---|---|
| Local dependency (same machine) | `npm install ../idlesolo/sprites`, then `import { draw } from 'art-sprites'` |
| Copy the folder | copy `sprites/` into the project and import `./sprites/index.js` (a browser takes it as plain ES modules, no bundler) |
| Git subtree (keeps history, pullable) | `git subtree split -P sprites -b sprites-only` in this repo, then `git subtree add -P sprites <this-repo> sprites-only` in the other |
| Tarball | `cd sprites && npm pack` and install the `.tgz` |

```js
import { draw, toCanvas, play, createLookCache, RAMPS, rng } from 'art-sprites';

const art = draw('character', { loadout: 'rogue' });          // { id, frames, fps, loop, clips, layers, anchors }
document.body.append(toCanvas(art.frames[0], { scale: 6 }));    // one frame, crisp
play(canvas, art.clips.idle, { scale: 6 });                     // a clip, until stop()
const run = draw('character', { loadout: 'rogue', motion: animatorClip }).clips[animatorClip.id];   // a clip of poses (Animator's), drawn with this loadout
const bob = draw('item', { shape: 'sword', motion: animatorClip }).clips[animatorClip.id];   // any recipe, any style: its still, moved by each pose
const gust = draw('cloth', { kind: 'flag', motion: { preset: 'gust', wind: 3 } }).clips.gust;   // or a preset of the recipe's motion type, with that type's own params

const looks = createLookCache((spec) => draw('character', spec), (px) => toDataUrl(px));   // draw once per distinct spec
const look = looks.get({ loadout: 'guardian', weapon: 'axe' }); // { still, clip(name), fps(name) }
```

| Entry | Contents | Runs in |
|---|---|---|
| `art-sprites` | `draw`, `RECIPES`, `Pixels`, `rng`, palette and ramps, `shade`, `lint`, kits, look cache, canvas helpers (`toCanvas`, `toDataUrl`, `drawFrame`, `play`) | anywhere |
| `art-sprites/png` | PNG encode and decode, `pixelsToPng` | Node (uses `node:zlib`) |
| `art-sprites/sheet` | contact sheets, the 1x/2x/4x readability strip, `rgbaSheet` for rendered cells | Node |
| `art-sprites/atlas` | `loadAtlas()`: the IBM VGA 8x16 glyph atlas for `renderCells` (credit in `docs/fonts/`) | Node (uses `node:fs`) |
| `art-sprites/recipes/*` | one recipe module, for a page that wants to load only the character drawing | anywhere |
| `sprites` (CLI) | `kits`, `draw`, `sheet`, `lint`, `study`, `export` (each takes `--style ansi` or `text`; `draw --clip <name> --out f.ans` writes an ANSImation) | Node |

TypeScript types are in `index.d.ts`, `png.d.ts` and `sheet.d.ts`. Tests: `npm test` here (they need no files from the host project).

**What stays a project's job:** which look a game character has (Idle Solo maps gear to a `character` spec in its own `public/hero-doll.js`),
where the art is served from, and any art-direction changes. To add recipes or kits for one game, register a kit with `registerKit` or
add a module under `recipes/`; do not fork the engine. **Limits:** the palette is DB32 and indexed (recipes pick colours from `palette.js` ramps, so another palette means editing that file, not a parameter); the art is
generated, so it is a drafting engine rather than a replacement for hand-polished sprites.

---

## How it is built

This folder draws pixel art from code. It is the drawing half of the modular sprite system in Painter's `docs/modular-sprite-system-design.md`
(that doc's non-goal "no procedural pixel drawing beyond compositing" is amended: seeded, deterministic recipes are in scope;
AI and LLMs still are not). The same recipe, params and seed always give the same pixels, nothing is fetched, and nothing in
here imports from a game (a test enforces it).

## How the reference kits fit

A kit is **data** (`sprites/kits/<id>.js`): the subjects its document asks for, the recipes that draw each one, and a status.
`sprites kits` draws every recipe a kit names, lints it, and reports each subject as:

| Status | Meaning |
|---|---|
| `covered` | recipes draw it |
| `partial` | recipes draw part of it; the note says what is missing |
| `built-in` | provided by the palette, shading or scene code rather than a recipe |
| `gap` | nothing draws it yet |
| `reference-only` | study material (paintings, photographs, 3D, HDRIs); not drawable, never shipped |

The five kits are `pixel-art-learning`, `faceart`, `fantasy-items-equipment`, `fantasy-biomes-environments` and
`hands-poses-motion-interiors`. Their tiers, axes and layers are parameters of the recipes (`tier`, `age/skin/angle/intensity`,
the `BIOMES` table, `stage`, `view`, `pose`, `wind`, a room's `time`), not prose.

**Adding a future kit:** write `sprites/kits/<id>.js` (copy a neighbour), import it in `kits/index.js`, run `kits`. Its `gap` rows
are the work list. A kit that names a recipe that does not exist, or one that throws, fails the test suite.

**Adding a recipe:** a function `(params) => { id, frames, fps, loop, clips?, layers?, anchors? }` in `sprites/recipes/`,
registered in `recipes/index.js` with a `variants()` list so `sheet` and the tests exercise it. Draw with the `Pixels`
primitives and `shade()`; take colours from `palette.js` ramps; take randomness from `rng(seed)`.

## The learning kit as code

| Kit rule | Where |
|---|---|
| Limited palette, hue-shifted ramps | `palette.js` (DB32, four-step ramps) |
| One light direction, cluster shading, outline | `shade.js` |
| Silhouette, three values, material cue, one accent | `recipes/items.js` |
| Readable at 100/200/400% | `sheet.js` `scaleStrip`; `lint` rules `outline`, `value`, `margin` |
| No pixel noise, 4 to 16 colours, standard sizes | `lint.js` |
| 4 to 6 frame cycles, intentional change, no boiling | `lintFrames` |
| Study: recreate, compare side by side | `sprites study ref.png mine.png` |

## Commands

```
sprites kits [--strict]
sprites draw character loadout=rogue --out /tmp/rogue.png --scale 6
sprites sheet item --out /tmp/items.png
sprites lint item shape=sword tier=legendary
sprites study ref.png /tmp/mine.png --out /tmp/side-by-side.png
sprites export character loadout=guardian --dir /tmp/guardian   # PNGs + provenance JSON
```

The CLI writes only where `--out` or `--dir` says. Exported records say `Project-created` with the recipe and params as evidence
(`--creator` names the project); wiring them into a game's asset manifest is that game's step.

## Motion types

Nothing moves the same way: a hen bobs, a tower settles, a flag gusts, a status bar jolts. Every recipe belongs to one of 14 motion types (`MOTION_OF`, `motionType(recipe)`), and a type has its own presets, params and limits. `motion` takes a clip, a preset name, `{ preset, id?, ...params }` or a list; an unknown preset or param throws. `lintMotion(recipe, motion, size)` reports a channel the type does not use or a move past its limit.

| Type | Recipes | Presets (own params) |
|---|---|---|
| `figure` | `character` | `breathe` (amount), `hop` (height), `lunge` (reach), `recoil` (push), `bow` (depth), `cheer` (height) |
| `head` | `face`, `portrait`, `bust`, `creatureface` | `breathe` (amount), `nod` (depth), `tilt` (amount), `flinch` (push) |
| `creature` | `creature`, `creation` | `breathe` (amount), `hop` (height), `pounce` (reach), `slither` (sway), `recoil` (push) |
| `object` | `item`, `thing`, `icon`, `sigil` | `bob` (amount), `pulse` (amount), `swing` (angle), `drop` (height) |
| `limb` | `hand` | `wave` (angle), `beckon` (amount), `tremble` (amount) |
| `plant` | `farm`, `prop` | `sway` (wind), `rustle` (wind), `grow` (from), `bounce` (height) |
| `machine` | `vehicle` | `rumble` (amount), `roll` (distance), `bump` (height), `lurch` (push) |
| `structure` | `building`, `strategy`, `furniture`, `room` | `settle` (amount), `quake` (amount), `creak` (angle), `build` (from), `crumble` (to) |
| `tiling` | `tile`, `pattern` | `scroll` (direction, step), `drift` (step), `ripple` (amount) |
| `scenery` | `scene`, `vista`, `landscape`, `locale`, `onplace`, `weatherloop` | `tremor` (amount), `shimmer` (amount), `breathe` (amount) |
| `cloth` | `cloth` | `gust` (wind), `billow` (wind), `snap` (wind), `flutter` (wind) |
| `effect` | `effect` | `pulse` (amount), `burst` (from), `rise` (height), `shake` (amount) |
| `panel` | `bar`, `mappane`, `sheet`, `menu`, `fight`, `artpack` | `jolt` (push), `shake` (amount), `slide-in` (distance), `drop-in` (distance) |
| `lettering` | `banner` | `wave` (amount), `wobble` (amount), `slide-in` (distance), `drop-in` (distance) |


`tiling` clips wrap round the edge (`wrap: true`), so a scrolling tile stays tiled. Values are in the art's own units: pixels, or cells for ANSI and text.

## Hands, poses, turnarounds, motion and interiors

`docs/cc0-hands-poses-turnarounds-motion-interiors-kit.md` closes the four gaps the first kits left. Each of its workflows is a parameter.

| Kit workflow | Recipe and parameter |
|---|---|
| Hand study: mitten, blocks, detail | `hand` (24x24): `gesture` (open, ward, fist, point, thumbsup, cast, grip), `stage` (mitten, blocks, detailed), `side` |
| Pose study: stances and postures | `character` `pose`: run, sit, crouch, reach, carry, cast, fall, brace, confident, regal, injured, sneak, exhausted, point and kick (an arm and a leg toward the viewer, foreshortened). A pose is a few joints (lean, head, squat, lift, feet, arm, weapon), not new art, so every loadout takes every pose |
| Turnaround: front, three-quarter, side, back | `character` `view`: `front`, `side` (`facing: 'left'` mirrors it), `back`, `quarter` (three-quarter, facing right). One spec drives all four; each view is its own set of layer drawings, so the face shows from front, three-quarter and side, and from behind the weapon and shield swap sides (the back canvas is 48 wide to mirror them) |
| Hidden-from-the-front details | `character` `back`: `cape`, `quiver`, `pack`, its own layer (under the torso from the front and side, over it from behind) |
| Secondary motion | `cloth` (flag, cape, banner, curtain, hair, robe): `wind`, `frames` (4 or 6), `drag`, `emblem`, `tie` |
| Items (ANSI) | `thing`: an item alone on a table, 25 by 12 cells with its slot in words; the kind from the name's first word (56 kinds), material class texture, wear (worn to broken, rusty, crude, fine) and auras (glowing, magical, metallic, invisible, hidden, floating, bound); `thingKind`, `materialClass`, `thingWear`. |
| Patterns | `pattern` (16x16, two colours, tileable: checker, checker4, stripes, hstripes, diagonal, dots, grid, bricks, diamonds, waves, rings, weave, hatch, noise; `kind`, `seed`, `ink` and `paper` ramps, `density` for noise) |
| Buildings | `building` (32x32 front view: barn, windmill, lighthouse, tower, chapel, inn, watchtower; `kind`, `seed`, `roof` ramp); `prop` also draws house, clanhall, shrine, tent, and scenery: castle, gatehouse, battlement, well, stall, dock, bridges, ruins, dune, peak, mine mouth, floating isle, portal; `scene` biomes castle, town, harbor, river, ruins, mines, highlands, skyisle; `scene` also takes `layers: true` (weather, inhabitants, story evidence, depth) or each as its own param |
| Farm and food | `farm` (16x16: wheat, carrot, pumpkin, corn, turnip, grapes, egg, mushroom, hen, sheep; `kind`, `ink` ramp); `items` also draws bread, cheese, apple, meat |
| Strategy | `strategy` (16x16: pointy-top hex tiles grass, forest, mountain, water, desert, snow; towers archer, cannon, mage, frost; `kind`, `ink` ramp) |
| Vehicles | `vehicle` (32x32 side view: wagon, sled, car, locomotive, airship, raft, cannon; `kind`, `seed`, `body` ramp); `prop` also draws boat, cart, ship, caravan, spaceship |
| Interiors | `tile` kinds `planks`, `flagstone`, `checker`; `furniture` (bed, table, chair, shelf, hearth, altar, workbench, anvil, barrel, candle, rug, window, doorway, stove, tub, fountain, piano, questboard, cage, counter and the prop shapes); `room` (cottage, smithy, library, shrine, cell, tavern, shop, castlehall, ruin, sewer, stable, inn) at a `time` of day. ANSI-only `locale`, `bar`, `mappane`, `sheet`, `creation` (a race beside a measuring stick, or a class with its tools: `kind`, `name`, `columns`, `rows`), `creatureface` (a creature's face by race, 25 by 12 cells or a small 16 by 8: `race`, `description`, `mood`, `hurt`, `effects`, `size`); `sigil` (clan shield) |

**Cloth** follows the kit's principles in code: an anchor that does not move (`anchors.pin`), a wave that grows with distance from it
(overlap) and lags behind it (drag), a `drag` offset for a cape trailing a body (follow-through), and one phase over the loop so the
last frame flows into the first. The wave changes most of a cloth's pixels by design, so the kit report does not apply the `boil` check to
it (`lintFrames(..., { boil: false })`); the `frames` and `length` checks still do.

**Rooms** are data (`ROOMS` in `recipes/interiors.js`): a floor, a wall, windows, furniture as `[name, column, row]`, clues that sit on a
piece of furniture, and a one-line `story`. The kit's readability checklist is testable: `layoutOf(room)` gives the door's entry cell, the
focal item and the blocked grid; `unreachable(layout)` lists furniture you cannot walk up to from the door (the test suite requires none).
`relight(px, { ambient, sources, shafts })` does the lighting without leaving DB32: night and dusk darken each colour to its nearest
darker palette colour, every source lights a circle in full and dithers out to its radius, and a window's light is a lightened, dithered
shaft on the floor. Lit rooms flicker (two frames).

## Coupler (ANSI rooms, faces, items, portraits, the Fight panel)

Coupler draws its pictures here, not in its own painter: `vista` (24 terrains, times, weather, both styles), `creatureface`,
`thing`, `creation`, `onplace` and `fight`, all ANSI-only, with the words beside each picture as `art.says`. Each takes `colors: 16 | 256 | 'true'`
(16 is the default and never changes): at 256 the dithers, rims and light are blended tones and grain from the xterm set, and in true colour
the exact colour, which the picture's cells carry as `cells.tc` (`tones(cells)` is the hex a viewer paints; `writeAnsi(..., { colors: 'true' })` writes it). `roomSeed(world, room, again)` is
Coupler's FNV-1a. `docs/coupler-painter-parity.md` is the status and `docs/coupler-handback.md` the note for Coupler.

## For a game that composes the art (Tag Team's contract)

Every drawing from `draw()` carries `alt`, named `anchors`, `bounds`, a `reduced` still (and `events` on each clip) and, for characters and creatures, `paletteGroups` and `projection`; `lintArt` checks them. `resolveLook(recipe, params)` writes every default into the params, so store that and not a loadout name; `dye(recipe, params, { cloth: 'blue' })` draws with colour groups set; `recolor` / `recolorArt` swap ramps in a finished picture. `placeholder` draws the fallbacks (`kind: 'unknown'` reads only its size). `character` takes `bodyProfile` (`medium`, `small`, `slim`, `broad`) inside the same 32 by 32 canvas. `sprites pack list.json --dir out` (or `writePack` from `art-sprites/pack`) writes a whole pack with an atlas, `manifest.json` and `goldens.json`; the pack and the `ENGINE_VERSION` it records are what a game that pins this package checks before it upgrades.

**A top-down world and what goes on it.** `character` takes `view: 'top'` with `facing` `north`, `east`, `south` or `west` (`west` is `east` mirrored): the same loadouts, poses, clips, dye and body profiles seen from above, on a 40 by 40 canvas centred on the body, with `projection: 'top-down'`. The five classes differ by silhouette alone (`lintSilhouettes` checks a set of drawings against a floor). Optional gear layers appear in `layers` only when asked: `armor` (`cloth`, `leather`, `chain`, `scale`, `plate`: a texture on the torso), `bootStyle`, `gloves`, `belt`, `shoulders`, `eyewear` (`goggles`, `mask`), `hairStyle` (8; under a bare head or a circlet) and `facialHair` (`beard`, `mustache`, `goatee`, `stubble`); `appearance` is what is shown over what is equipped. Every character has the clips `turn`, `block`, `cast`, `heal`, `celebrate` and `recall`. `portrait` takes `size` (`party`, `status`, `card`), `expression` (the nine of `face`) and `state` (`defeated`, `recalling`) and returns a `mask` and a `safe` box. `overlay` draws a rarity frame (each rarity its own shape), state markers, durability pips and, at `size: 32` with `draw(..., { atlas })`, a stack count in the VGA font; `skillicon` makes a class skill from a category, an approved motif, a target, a kind, a tier and a cost; `telegraph` and `target` mark a blow's area and who is selected; `drawNumber(atlas, { value, kind })` floats a combat number. Colour is never the only cue in any of them.

**Phase 6.** `item` takes `iconSize` 24 or 32 (drawn again from the shape on the finer grid, not scaled); `prop` with `view: 'top'` has `conveyor`; the dungeon tile has a `gate` part and a `dress` grid takes `=` for it (`gates` sets its state).

**The town, the Museum and the rest of the party's kit.** `building` with a `service` (`registry`, `atelier`, `inn`, `training`, `archive`, `blacksmith`, `alchemist`, `store`, `trading`, `bank`, `portal`, `chronicle`, `fame`, `research`, `museum`, `marker`) draws from one grammar (`shell`, `theme`, icon `glyph`) in the front view (32 by 48, a hanging sign) or from above (32 by 32, the icon on the roof) with a `state` (`locked`, `available`, `new`, `upgrade`, each its own shape); `town` lays a list of them on streets and returns `hotspots`, a walkable `grid` and `alt`. `exhibit` frames any picture (an art, or `{ recipe, params }`) in a case, on a plinth, as a framed picture, a manuscript page or a specimen board, empty, occupied or new, with a category placeholder when there is no picture; `award` draws plaques, podiums, ribbons, seals, medals and rank chevrons. No text is drawn into either. `material` (ore, herb, hide, part, crystal, each with a `variant`) and `currency` (coins in four denominations, purses, keys, fragments, plans, tokens) join eleven new `item` shapes. `character` has the expanded clips (`dodge`, `parry`, `charge`, `stun`, `potion`, `buffed`, `lowhealth`, `levelup`, `tagin`, `tagout`, `equip`) and `shoot`, `thrust` and `slam` (`weaponAttack` says which a weapon takes), and the extra layers `tabard`, `bracers`, `neck`, `scar`, `jewellery` and `helmStyle`.

## Known gaps

No kit subject is left with nothing that draws it (`sprites kits` lists none). Partial: foreshortening is front view only; hands are right hands seen from the back (no palm view, none holding an item); no
climbing pose; no stairs or ceilings; five rooms (no tavern, throne room or ruin); no spears or polearms, no scrolls or runes, no castles,
ruins, docks or floating islands, no demon, chimera or harpy body plan, no 8x8 faces, no profile or back head angle on faces. The art is
generated, so it is a drafting engine and a consistent baseline, not a replacement for hand-polished sprites; the intended use is draw
by recipe, then refine in a pixel editor.

## Hero features (the R36S Rec demo)

The R36S build records a tour of these (`sprites-demo`, or Settings > Rec demos; `--features a,b` over SSH). The tour is the commands under `#@feature` lines in `r36s/commands.txt`
(the same list the device report times), so a hero feature is a group of commands there:

- `draw`: Draw items, characters and scenes from recipes
- `ansi`: The same art as ANSI and text
- `sheet`: Sprite sheets
- `convert`: Any picture as ANSI art
- `check`: Lint and export

## License

MIT, Copyright 2026 ansiapps (see `LICENSE`); see `NOTICE` for what it includes from others.
