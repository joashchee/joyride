# Handback to Coupler: the gaps closed (1.8.0 to 1.13.0)

Painter 1.13.0. Coupler's bar is "as informative as the Rust painter's, judged by eye". Nothing here is byte-identical to the Rust painter,
except the two fallbacks (an unknown race or class made from its name) and `roomSeed`, which match Coupler's test vectors.

| Gap | Kit rows | Status | Recipe | Release |
|---|---|---|---|---|
| G2 room fidelity | `terrains-outdoor`, `terrains-indoor`, `time-of-day`, `weather` | partial: needs your eye | `vista` | 1.8.0 |
| G2 | `scene-input`, `no-sky-indoors`, `high-fantasy`, `colour-depths`, `accessibility-words`, `ansi-file` | covered | `vista` | 1.8.0 |
| G3 faces | `faces`, `faces-small` | covered | `creatureface` | 1.9.0 |
| G4 items | `item-pictures` | covered | `thing` | 1.10.0 |
| G1 portraits | `portraits` | covered | `creation` | 1.11.0 |
| G5 figure and Fight | `figure-on-place`, `fight-panel` | covered, awaiting your sign-off | `onplace`, `fight` | 1.12.0 |
| G6 | other kits' partials | not taken: none blocks a done-when | | |

**Lint.** `node tools/vista-matrix.mjs` on your 260-picture matrix: 39 pictures with findings, all `ramp`; no `invisible`, `shade-noise` or
`contrast`. Every kit variant of the new recipes is lint clean (`npm test`, `node cli.js kits --strict`).

**Changed outputs.** `vista` pictures changed deliberately in 1.8.0 (pixel-canvas rooms, indoor redraw); the manifest names are in the 1.8.0 changelog entry.
`tools/vista-baseline.json` holds the current hashes and `--check` reports 0 of 260 differing. Later entries name any other change.

**Decided.** The table rules: a place recedes ("a shade darker"), as Painter already does. The fixtures showing it unreceded are not followed
(`recede: false` still turns it off).

**Coupler's review (2026-10-29), answered.** Mountains are now three layered ranges; the swamp has a far treeline and mist instead of a flat sky
band; snow lying on the land is textured at 256 colours and true colour (16 colours keep the plain dither). 31 of the 260 pictures changed, and then all 48 at 256 and true colour (the broader-palette texture rule),
named in the 1.13.0 changelog entry. Please look at `mountains_*`, `swamp_*` and the `dusk-snow` 256 pictures again. `terrains-outdoor` stays
`partial` for the details its row names. The sign-off on the Fight panel is open.

**Your side, unchanged.** How Coupler runs the engine, mapping `paint::Scene` to the params, deleting `painter.rs`, rules 6, 11 and 12, and
showing `says` beside every picture.

**A fresh set to look at.** `docs/coupler-handover/painter-data/engine/` (git-ignored) holds the 260 pictures drawn now, beside `reference/`;
`node tools/compare.mjs --terrain city` pairs them.

**Carry-over.** Copies of this package elsewhere (Idle Solo's `sprites/`) are not touched. Everything since 1.7.0 is ANSI-only recipes and
tables; the pixel recipes and their output are unchanged, so a copy needs the new files from 1.8.0 on only if it wants Coupler's recipes.

**Smooth colour for every recipe (1.14.0, your plan).** `colors: 16 | 256 | 'true'` is a param of `creatureface`, `thing`, `creation`, `onplace`,
`fight` and, with `'true'` new, `vista`; `figureOnPlace(figure, place, { colors })` and `placeLayer({ colors })` take it too. 16 is the default and
byte-identical (the golden hashes and your 236 non-true matrix hashes did not move). At 256 the Bayer dithers and flat steps are blended tones
from the xterm set, with grain; in true colour they are exact. The cells carry the RGB: `cells.tc.fg` / `cells.tc.bg` (0xRRGGBB, -1 where the
index stands), or `cells.get(x, y).fgRgb` / `bgRgb`; the index beside it is the nearest of the 256. `deepArt` should read RGB when `tc` is there and
the index when it is null. `figure.colors` is a `Uint32Array` at 256 and true colour (an `EXACT` handle, `0x1000000 | 0xRRGGBB`, is a true colour
there) and carries `figure.depth`. `tools/vista-matrix.mjs` now draws your true-colour pictures as true colour: 24 hashes changed, and the baseline
is updated. For `scripts/sprites-ts-check.mjs`: for the recipes above at one set of params, 256 and `'true'` should differ from 16, and `'true'`
from 256, and the 16 hash should equal the one you have. Not done, so still 16 or 256 only: `locale` and the other native ANSI recipes.
