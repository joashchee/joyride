# Expansion plan: from pixel art to other styles, starting with ANSI art

Status: plan, nothing built. Studied 2026-10-29 from `~/Coding/coupler`
(`src-tauri/src/`: `painter.rs`, `paint.rs`, `figure.rs`, `face.rs`,
`item_art.rs`, `fight_art.rs`, `ansi_art.rs`, `ansi.rs`) and, for the file
formats, `~/Coding/stylus` (its notes on SAUCE and icy_tools).

The package is now `art-sprites`. Its promises do not change: seeded and
deterministic, dependency-free, self-contained, main entry free of Node
built-ins, nothing fetched.

## What Coupler already does (the thing to learn from)

- **A cell is two square pixels.** Each character cell is `▀` with the top
  pixel as foreground and the bottom as background, so a canvas is `w` by
  `2h` pixels and half-blocks give square pixels. Where a pixel cannot show
  something (stars, rain, snow, waves, leaves, runes) a glyph from the IBM
  VGA font goes over it (`Canvas.glyphs`, one `(char, color)` per cell).
- **16 VGA colors as palette indexes, not RGB.** The view's theme decides
  what an index looks like. More depth is an upgrade: `paint::Colors`
  (`16`, `256`, `true`, read from the game's `ANSI 16|256|TRUE` answers)
  makes `Canvas::art_in` show gradients, night and Bayer dithers as the
  in-between color (`tones`), nearest-of-256 for 256 colors.
- **Per-pixel metadata beyond color:** `lit` (self-lit pixels night leaves
  alone) and `parts` (what each pixel belongs to, so a figure can be laid
  on a room and lit by it, `figure.rs`).
- **One output type.** Everything, painted or loaded from a file, becomes
  `Art { columns, rows, lines: Vec<Line> }`, a line being spans of text with
  one `Sgr` style (fg, bg, flags). The painter and `.ans` files share it.
- **Pure and seeded.** `paint::seed(world, room, again)` is FNV-1a; same
  scene, same picture. Tests have an ignored `show` test that writes a PPM
  to look at.
- **Reading the format, which writing must invert** (`ansi_art.rs`):
  CP437 bytes or UTF-8; 80 columns unless SAUCE says otherwise; cursor
  moves; iCE colors (blink means bright background) when SAUCE asks; the
  SAUCE record and its comments and everything after the Ctrl-Z end mark
  are not picture; ANSImations replay at 14,400 bps with flashes held to
  three a second; sizes capped (400 by 1000).

## What art-sprites has to add

Today the engine's unit is the **pixel on an indexed DB32 canvas** (`Pixels`,
`palette.js`, `shade.js`, `lint.js`, PNG out). ANSI art differs in four ways,
and each is a phase below:

1. The grid is **cells**, each with a character and two colors, not pixels.
2. The palette is **16 VGA colors** (with a 256 and a truecolor upgrade),
   not DB32, and a theme maps them to RGB at view time.
3. The output is **text with escape codes** (or a spans structure), not a PNG.
4. **Rules differ**: lint for pixel clusters means nothing; ANSI has its own
   (width, color count, legible in the terminal font).

Painter's recipes should not be rewritten. The goal is a second *target*
for the same recipe contract.

## What the ANSI learning kit adds (studied 2026-10-29)

Source: `docs/cc0-ansi-art-learning-reference-kit.md`, and the pages it links
that could be read as text: Roy/SAC's zeroVision and Newt shading tutorials,
bert.org (recreating ANSI art from a screenshot), the Maximus BBS ANSI primer,
the Hermes ANSI-editor page, the sdelic.org ANSI overview, and 247plan's
image-to-character guide. **Not readable:** the shading charts in the Roy/SAC
tutorials are images or `.ANS` files, so the exact glyph-and-color pairs they
teach were not seen; the teXt0wnz tutorial index gives titles only. The
tutorials themselves (Halshade, Curving and Shading, Color Science, the font
tutorials) still need reading in an ANSI viewer by a person.

What it changes, from firm facts to craft rules:

**Hard rules the engine must obey (the file format and the hardware)**

1. **Backgrounds are 8 colors (0 to 7) unless iCE is on.** The 16 foreground
   colors are free; bright foregrounds were the bold bit, and a bright
   background was the blink bit, honored only when the file's SAUCE says iCE.
   Phase 1's fold ignores this: it can write `▀` or a space with a bright
   background. That is fine for UTF-8 terminals and wrong for a classic `.ans`.
   Fix: a legality pass (Phase 1b).
2. **Reversal equivalences make the rule workable.** `▀` with fg A, bg B is
   `▄` with fg B, bg A. `░` with A on B equals `▓` with B on A; `▒` swaps with
   itself. A space on a bright bg is a `█` in that color. Each swap moves a
   bright color from the background to the foreground. When both halves are
   bright and different, no single glyph is legal: pick the legal candidate
   with the least error (bert.org's method: score every glyph and color pair
   against the target cell).
3. **Shade glyphs mix about 25, 50 and 75 percent** (`░ ▒ ▓`), so three colors
   give roughly 25 shades. The cell's effective color is the weighted mix;
   that is the `tone` Coupler keeps per pixel, and what 256 and truecolor
   output (Phase 5) needs.
4. **Cell geometry is 8 by 16 (9 by 16 in VGA text mode),** so a cell is twice
   as tall as wide; half blocks make square pixels and an 80 by 25 screen a
   80 by 50 canvas. Honor the SAUCE aspect-ratio and letter-spacing flags.
5. **File output rules** (Hermes, Maximus, bert.org): reset (`ESC[0m`) first,
   then background, then foreground, with bold for bright foreground; emit an
   escape only when attributes change; elide trailing spaces (only where the
   background is none); **CRLF line ends, never a bare LF**, including on a
   full-width row; Ctrl-Z, then a 128-byte SAUCE (title, author, group, date,
   width, height, iCE and spacing flags, font name) and its comments; **CP437
   by default, UTF-8 optional**, and flag any character outside CP437 on save.
   Dense output built from cursor save and restore garbles in some readers
   (Maximus's converter): static art is written as plain forward printing, and
   cursor tricks are for ANSImations only. Hosts strip SAUCE, so a `.ans` is
   best paired with a plain text fallback, and art taller than 24 rows scrolls
   awkwardly on a classic BBS. Full screens run 2 to 8 KB.
6. **Terminals default to UTF-8, so raw CP437 shows as mojibake** (macOS
   Terminal, xterm, PuTTY). The CLI must say which charset it wrote.

**Craft rules the lint and the native recipes should carry**

7. **Workflow order:** eyes or focal point first (it sets the scale), then a
   single-line outline in dark gray before black, then base color blocks,
   then highlights, then shadows, then blend last, keeping most of the
   outline. Rough shapes first; detail only once the smooth base is done.
8. **Gradients need room:** about four steps for a smooth one; a blend goes
   only one or two steps into an area; prefer vertical smears to long
   horizontal runs of shade glyphs; no random shade noise (the kit says
   deliberate placement beats density); heavy shading erases the outline.
9. **Color relations:** dark gray (8) is the main shadow color and shades up
   toward white but cannot reach it directly (a highlight may have to be
   left out); a bright version beside its dark partner adds interest;
   dark red and dark blue are common bases; overhangs take a shadow; keep a
   one-cell black gap between shapes. A toony bold border gives contrast.
   Two starter palettes in the kit: *Classic BBS* (black ground, white and
   cyan highlights, magenta accent, yellow warm, red emphasis, green nature,
   blue cool shadow) and *Low-contrast study* (black, dark blue, blue, cyan,
   light cyan, white).
10. **Image-to-character conversion** (247plan, bert.org): fix luminance
    first, stretch contrast so the ramp's ends are used, resize with a
    good filter (Lanczos), correct the 2:1 cell ratio, reverse the ramp on a
    dark background, and avoid busy sources. Short ramps (about 10) under 60
    columns. The shade blocks plus a space are five clean tone levels with no
    glyph noise.

**Practice briefs become the kit.** The kit's five briefs are the first
`ansi-art` kit subjects (Phase 4): a 40 by 20 logo in 4 colors, an 80 by 25
menu screen with a box-drawing frame, an 80 by 25 landscape with a `░▒▓` sky,
an 80 by 50 half-block portrait with three-value lighting, and an original
artpack piece with SAUCE. Fonts (the six font tutorials) add a subject the
pixel kits do not have: **block lettering** for logos, titles and menus.

**License posture.** The kit says artworks and most tutorials are not CC0.
So: no archived `.ANS` file is bundled, traced or used as a test fixture;
reader and round-trip tests use files this engine drew itself; nothing
learned from a tutorial is copied, only techniques. The render font is fixed by the ansiapps standard (next section), not chosen
from the kit's CC0 list.

## The standard font (decided 2026-10-29, from the ansiapps apps)

Checked in the CLAUDE.md files and theme docs of Diskette, Crunchy, Floppy,
Stylus, Coupler, MyAiTwin and the ansiapps site, and the font files in
Diskette and Coupler. Every ansiapps app uses one font: **IBM VGA 8x16 by
VileR** (The Ultimate Oldschool PC Font Pack v2.2, int10h.org), **CC BY-SA
4.0**, shipped as `WebPlus_IBM_VGA_8x16.woff` (the "Plus" variant, for wide
Unicode coverage) in `public/fonts/ansiapps/` with its `LICENSE.TXT` and
`README.txt`. It is used at 16px only (a bitmap font is crisp only at its
native size), with no bold or italic, font smoothing off, on an 8 by 16 cell
grid. Floppy and Coupler draw every glyph in the theme from it; Stylus
rasterizes it at run time. The Scoreless Music apps (Chordica, Cue,
Neumetik, Fontastik) are a separate family and are not covered.

**This is Painter's standard font.** For this engine:

1. **The cell is 8 by 16 pixels, and IBM VGA 8x16 is the only font that
   defines what a glyph looks like** in any render, contact sheet or
   review image (Phase 3). SAUCE's font name defaults to `IBM VGA`, which is
   the same face, so a file written by Painter names what it was drawn for.
2. **Painter does not ship the font file.** The apps already do, unmodified,
   with the license beside it. Painter ships what the theme docs allow in any
   app: **rendered images** of the unmodified font's glyphs (VileR's readme
   says images rendered from the fonts are not covered by ShareAlike). So
   the renderer's source is a **PNG glyph atlas** (CP437's 256 glyphs on a
   16 by 16 grid of 8 by 16 cells), rendered once from the unmodified
   `WebPlus_IBM_VGA_8x16.woff` by a script outside the package, never a font
   file, a converted font, or a glyph table typed into a JS module (which
   would be a converted copy of the font).
3. **The credit travels with the atlas:** `docs/` names the font, VileR,
   int10h.org and CC BY-SA 4.0, with VileR's readme text beside the atlas,
   and the README says so. If Painter is ever packaged for an app that shows
   an About, that app's own credit already covers it.
4. **No other font defines the look.** A glyph a cell uses must be one the
   font has (the apps' own rule: a glyph the font lacks falls back to a font
   of another width and shifts the line). The lint's `charset` rule is
   therefore "in IBM VGA / CP437", and ANSI output for the apps stays inside
   CP437 box, block, shade and symbol characters.
5. A 9 by 16 letter-spaced mode (SAUCE's flag) is derived from the same
   atlas by repeating the eighth column for the line and block characters,
   as VGA hardware did; it is optional.

## Phases

Each phase ends with `npm test` passing, a changelog bullet, a `kits --strict`
run, and a look at the output (a PNG contact sheet, and for ANSI also the
text in a terminal).

### Phase 0: groundwork (done 2026-10-29)

- Vocabulary settled: a **style** is a drawing language (`pixel`, then
  `ansi`); a **target** is an output form. Recipes keep their
  `(params) => { id, frames, fps, loop, clips?, layers?, anchors? }`
  contract.
- `draw(id, params, { style })`, default `pixel`, so every existing call is
  unchanged. `STYLES`, `DEFAULT_STYLE`, `recipeStyles(id)`. A recipe offers
  a style by naming a native drawing in its field of that name (`ansi`);
  without one, `draw` throws "recipe X has no ansi drawing yet" (Phase 1's
  fold will make that the fallback instead). Types and `--style` in the CLI
  done.
- Golden hashes (`test/golden.test.ts`, `test/golden.json`): every
  registered variant, so the expansion cannot change pixel art.
- Moved to Phase 4: `sprites kits` reporting ANSI gaps. It needs the
  `ansi-art` kit, which does not exist before there are ANSI recipes.

### Phase 1: the cell canvas (done 2026-10-29)

- `cells.js`: `Cells` with `w`, `h` and per cell a code point, fg and bg as
  palette indexes (0 to 255, `NONE` for no colour) and flags (`BOLD`,
  `BLINK`). Primitives: `set` (keeps what is not given), `rect`, `fill`,
  `text`, `glyph` (a character over a cell, keeping its background),
  `blit` (skips empty cells), `clone`, `equals`, `toText`.
- **Half-block fold** (`foldPixels`, `foldArt`): `w` by `h` pixels become `w`
  by `ceil(h/2)` cells. Both pixels drawn and different: `▀` with the top as
  foreground and the bottom as background. Same colour: a space of that
  colour. One pixel only: `▀` or `▄` with no background, so a sprite leaves
  what is behind it. `foldArt` folds frames, clips, layers and masks, and
  moves anchors to cell units (`y / 2`). `draw(..., { style: 'ansi' })` uses
  it for every recipe without a native `ansi` drawing.
- `palette-ansi.js`: `VGA16` (ANSI index order), `XTERM256`, `nearest`,
  `indexHex`, and `DB32_VGA`, written by hand because nearest colour sends
  a dozen DB32 colours to dark gray.
- Not yet: the optional `parts` and `lit` layers (Phase 5 needs them for
  lighting), the 256 and truecolor per-cell tones (Phase 5), and a way to
  choose another DB32 table per call beyond the `map` option.
- Tests: the fold on known pixels, palette ranges and the table's values,
  folded characters (frames, clips, layers, anchors), determinism, and
  golden hashes for every folded variant.

### Phase 1b: legal cells and a drawing kit (done 2026-10-29; Phase 2 is next)

Phase 1's fold is right for UTF-8 and wrong for a classic `.ans` (finding 1).
Fix that and add the primitives native recipes and the writer need.

- **`bgLimit`** (`8` classic, `16` for iCE, UTF-8 and modern): a property of
  the output target, kept on `Cells` or passed to `legalize`.
- **`legalize(cells, { bgLimit })`**: for each cell with a background over
  the limit, apply the reversal swaps (finding 2); when none is legal, pick
  the candidate (glyph in space, `░ ▒ ▓ █ ▀ ▄`; any fg; legal bg) with the
  least color error against the cell's effective colors. `foldPixels` takes
  `{ bgLimit }` (default 8, the classic target) and runs it. Property test:
  after it, every background is under the limit and the cell shows the same
  colors, or the nearest legal ones.
- **`tone(cell)`**: the effective color of a cell (`░ ▒ ▓` mix 25, 50, 75
  percent of fg over bg; half blocks are the two halves), used by the
  legalizer, by Phase 5, and by the lint. The 256 and truecolor path reads
  it.
- **Primitives:** `box(x, y, w, h, { style: 'single' | 'double' | 'mixed' })`
  with the right corners and joins, `hline`/`vline`, `shadeFill` with a
  chosen glyph, and `ladder(a, b)`: the five-step run `a, a/b ░, ▒, ▓, b`
  oriented so every step is legal.
- **`ANSI_RAMPS`**: shade ladders by hand, like `DB32_VGA`, from the kit's
  two starter palettes and the dark-gray rule (finding 9); each ramp has at
  least four steps and is legal at 8 backgrounds.
- Re-accept the folded golden hashes (`UPDATE_GOLDEN=1`) and say so in the
  changelog: folded output changes, pixel output must not.
- As built: `legalize`/`legalCell`/`tone`/`halves`/`ladder`/`rampCells` are in
  `cells.js` and the box-drawing tables in `boxes.js`. `bgLimit` is passed
  per call (`foldPixels`, `foldArt`, `draw`), not kept on `Cells`. A cell with
  no colour counts as black in `tone`. Backgrounds of 16 and up (256-colour
  mode) are left to the writer's depth reduction. `mixed` boxes are double
  horizontals with single verticals. Ramps stop where both colours would be
  bright (the blue ramp ends at light cyan; white is placed by hand).

### Phase 2: ANSI writer and reader (`ansi.js`, no Node built-ins) (done 2026-10-29; Phase 3 is next)

As built: `writeAnsi` takes a `target` (`classic`, `ice`, `modern`) that picks the charset, the background limit and the SGR dialect, with `charset`, `colors` and `sauce` overrides; it returns `{ bytes, warnings, ... }`. `readAnsi` defers the wrap after the last column, so a full row plus CRLF is one break. Bold folds into the bright colour on read. SAUCE letter spacing defaults to `8` (the standard font's cell). ANSImation is in `ansimation.js` (Phase 5).

- **Writer:** cells to a string of SGR (16, 256, truecolor selectable via a
  `colors` option, as Coupler's `Colors`), run-length merged like
  `art_in`'s spans, lines ended with a reset. Targets: UTF-8 (modern
  terminals and MUD clients) and **CP437 bytes** (classic `.ans`, with
  `Uint8Array` out so the main entry stays browser-safe).
- **SAUCE:** write a record (title, author, group, date, width, height,
  iCE flag, font) and the Ctrl-Z mark. The creator is the project, as
  `export` already records provenance. Respect SAUCE when reading.
- **Reader:** the inverse, mirroring `ansi_art.rs` (CP437 versus UTF-8
  detection, cursor moves, SAUCE width, iCE). A round trip
  (cells to bytes to cells) is the test; Stylus's notes call this "the
  corpus round trip". Cap sizes as Coupler does.
- **Writer rules from the kit** (findings 5 and 6): `ESC[0m`, background,
  foreground, bold for a bright foreground; an escape only when attributes
  change; trailing spaces elided only over no background; CRLF, never a bare
  LF, even on a full-width row; plain forward printing for stills (cursor
  moves only in ANSImations); `legalize` is run for the 8-background target
  and the iCE flag is set only when the file uses bright backgrounds; a
  warning, never silence, for a character outside CP437 and for a depth
  reduction; the result says its charset.
- **SAUCE completeness:** the iCE bit, the letter-spacing and aspect-ratio
  flags, and the font name (`IBM VGA` by default), with the comment block;
  Ctrl-Z first. A `.txt` fallback (the characters alone) is offered beside a
  `.ans` for hosts that strip SAUCE and for plain terminals.
- **Reader tests use only files this engine drew** (license posture): no
  archived `.ANS` fixtures, so the "corpus round trip" is a corpus of our
  own recipes' output.
- **Spans output** (`toSpans`): the same `{columns, rows, lines}` shape
  Coupler uses, for a page or app that colors the text itself.
- Tests: byte-level cases for each escape, CP437 table, a SAUCE round
  trip, width limits.

### Phase 3: rendering ANSI back to pixels and the CLI (done 2026-10-29; Phase 4 is next)

As built: `renderCells` (`ansi-render.js`) takes the atlas as a parameter and returns `{ w, h, rgba, missing }`; `atlas.js` (Node) loads `docs/fonts/ibm-vga-8x16.png`, rendered by `tools/render-vga-atlas.py` (not in the package) from the unmodified woff. 9 by 16 mode is in. CLI: `draw`, `sheet` and `export` take `--style ansi` with `--target`, `--colors`, `--charset`, `--spacing`; `lint --style ansi` waits for Phase 4's `lintAnsi`. Not done: a `--theme`, `study` against a cell render (it compares PNGs, so render first), VileR's own readme (only the note the apps ship is beside the atlas; add the pack's readme when the pack is to hand).

- `renderCells(cells, { font, theme })` to `Pixels`/PNG, using the
  standard font's **PNG glyph atlas** (IBM VGA 8x16 by VileR, see "The
  standard font"; an optional 9x16 letter-spaced mode) and a theme palette. This is the
  review tool: contact sheets of ANSI art as an image, the equivalent of
  Coupler's ignored PPM `show` test, and how a person looks at the result
  without a terminal.
- CLI: `sprites draw <id> --style ansi --colors 16|256|true --charset
  utf8|cp437 --out x.ans`, `sprites sheet --style ansi` (PNG), and
  `sprites lint --style ansi`. The CLI still writes only where `--out` or
  `--dir` says.
- `sprites study` already compares two PNGs; with `renderCells` it
  compares a drawn ANSI piece against a reference *image the user has the
  right to use*, the kit's recreate-and-compare step. Nothing from a
  reference is stored or shipped.
- Export records keep `Project-created` provenance, plus the style and
  colors.

### Phase 4: ANSI lint and the first ANSI recipes (done 2026-10-29)

As built: `ansi-lint.js` (`lintAnsi`, `lintAnsiFrames`), `ansi-craft.js` (`paint`, `runSteps`, `overlay`, `lettering`), `recipes/ansi.js` (`banner`, `landscape`, `menu`, `bust`, `artpack`, all `ansiOnly`: registered with an `ansi` drawing and no pixel one) and `kits/ansi-art.js`. Not done: the `gap` lint rule (it needs a notion of a shape), a native `face`, `scene` or `item` (the folded pixel drawings stand in; `sprites kits` marks them `[ansi: folded]`), the tutorials' own shading pairs (not readable). Changes from the brief: the menu and landscape are 80 by 24, not 25 (the `height` rule: a classic screen scrolls past 24), and the bust is 80 by 48 pixels (80 by 24 cells). A `ramp` finding counts only a run whose ends are the run's own two colours, so a gradient cut off by a mountain is not one.

- `lintAnsi` (rules from findings 1, 8 and 9 are in the list that follows
  this one): width at most 80 (or the stated width), the color count per
  depth, no cell whose fg equals bg with a glyph (invisible text), contrast
  per cell against the 16-color rule, iCE only when declared, and for
  animation the `frames`/`length` checks plus a flash-rate check (no more
  than three a second, as Coupler holds).
- First recipes, taken from what Coupler's painter proved: **scenes**
  (sky by time and weather, terrain, indoor room in perspective: the
  `fantasy-biomes-environments` kit's subjects), **faces/portraits**, and
  **items**, each as a recipe with an `ansi` drawing. Start with one scene
  and one portrait; the rest follow the kit gap list.
- **More lint rules, from the tutorials:** `bg-limit` (a background over the
  target's limit), `shade-noise` (an isolated shade glyph whose neighbors are
  solid or empty, the "random shade noise" the kit warns about), `ramp`
  (a gradient under four steps, or a dark-gray-to-white jump), `outline` (a
  figure with no darker contour cell around it), `gap` (shapes touching
  with no one-cell separation, a warning), `height` (over 24 rows for a
  classic target), and `charset` (a glyph outside CP437 for a CP437 target).
- **Recipes are written in the artists' order** (finding 7): a helper API
  for focal point and outline, then base fills, highlights, shadows, and
  blend last, so the code reads as the workflow and the outline survives.
- **The `ansi-art` kit starts from the five practice briefs** (logo, menu
  screen with a box-drawing frame, shaded-sky landscape, three-value
  half-block portrait, SAUCE artpack piece) plus **block lettering**:
  a `banner` recipe building letters from half and full blocks in a few
  hand-made faces (single-color, outlined, shaded), used by logos, titles
  and menus across the ansiapps apps.
- Glyph overlays (`stars`, `rain`, `snow`, `waves`, `runes`) as a small
  table from the IBM VGA set, chosen by seed.
- Kits: add an `ansi-art` kit (data, with the statuses already used) so
  `sprites kits` lists ANSI gaps as the work list.

### Phase 5: depth, light and animation (done 2026-10-29)

As built: `ansi-depth.js` (`gradient`, `bayer`, `tones`, `resolveTones`), `ansi-light.js` (`light`) and `ansimation.js` (`writeAnsimation`, `readAnsimation`); the recipe `weatherloop` and the CLI's `--clip`/`--baud`. Cells hold palette indexes, so "truecolor tones" are `tones(cells)` (hex per cell) for a viewer, and `resolveTones` is the 256-colour path; the writer does not take hex. `light` works on the finished cells by stepping colours along the ramps, with `selfLit` standing in for the `parts`/`lit` layers (no such layers exist on cells; the folded drawings' `mask` works as a `selfLit` mask). Frame marks are a bare `ESC[H`; a delta addresses with `ESC[row;colH`. Not done: a native character or scene animation (only falling weather), a `lint` of a `.ans` animation file (lint the clip's frames), loops written into the file (a player repeats the file).

- Gradients follow the kit: at least four steps, vertical smears over
  horizontal runs, one or two steps of blend into an area.
- 256 and truecolor: gradients, night and Bayer dithers resolved to the
  in-between color per cell (`tones`), nearest-of-256 for 256 colors.
- Lighting for cells (`relight`'s counterpart): self-lit cells, ambient and
  sources, using `parts`/`lit`.
- **ANSImation:** clips as frames, written as cursor-move deltas (only the
  changed rows), playable at a stated baud; plus a reader that replays one.
  Reduced-motion rule: the finished picture is always available as a still.

### Optional, after Phase 4: an image-to-cells study converter (done 2026-10-29)

As built: `ansi-image.js` `fromImage` and `sprites convert`. 16 colours choose by least error over glyph and colour pairs; 256 colours are the nearest of the 256 per half block.

`fromImage(pixels, { columns, colors, bgLimit })` for the study workflow only
(an image the user has the right to use): luminance and contrast first,
Lanczos resize, the 2:1 cell ratio absorbed by half blocks, and each cell
chosen by least error over glyph and color pairs (the bert.org method, with
`tone` and `legalize`). It is a drafting aid, not a recipe: output is not
committed as engine art, and it never fetches an image.

### Phase 6: the rest of the styles (the plain text style is done 2026-10-29)

As built: `text-art.js` (`foldText`, `asciiFromCells`, `writeText`, `lintText`), `STYLES` gains `text`, and the CLI takes `--style text`. It reuses `Cells` as the canvas (no new canvas type was needed), a 7-bit ASCII "palette" of ten ink levels, a writer and a lint. No recipe has a native `text` drawing yet. Vector and braille/sextant styles were offered and not chosen; the same split applies when they are.

The `style` and `target` split from Phase 0 is what makes this cheap. Later
styles (the user has said others are coming; not decided) each add a canvas
type, a palette, a writer and a lint, and reuse the recipe contract, the
seed, the kits and the CLI. Do not start one until ANSI is through Phase 4.

## Decisions (made 2026-10-29)

1. **Both.** Existing pixel recipes fold to cells (Phase 1's half-block
   mode), and native ANSI recipes are hand-built for scenes, portraits and
   items (Phase 4 on). A recipe may offer either; `draw` prefers the native
   drawing when the recipe has one and folds otherwise.
2. **The 16-color palette is themed by the viewer.** Keep indexes in the
   engine and leave RGB to the viewer (Coupler's rule: which RGB an index
   means is the view's call); the PNG renderer takes a theme only for
   review.
3. **Port ideas, not code.** Coupler's painter is Rust and this engine is
   JavaScript: port the ideas, and cross-check against the output of
   Coupler's tests. No code is shared, and **icy_tools stays out** of this
   dependency-free package. Stylus remains the editor and the authority
   for what a saved file looks like.
4. **art-sprites replaces Coupler's painter and is the only art engine for
   every ansiapps app from now on.** New art work in any ansiapps app
   starts here, not in the app. This changes Coupler's rule 11 ("all ART is
   Coupler's painter's first"); that edit is Coupler's own session's step
   and has not been made.

## Phase 7: replacing Coupler's painter (started 2026-10-29: inventory and the room recipe)

As built: `docs/coupler-painter-parity.md` and `kits/coupler-painter.js` (the inventory, one row per thing); `recipes/vista.js` (`vista`, `roomSeed`, `hasSky`) covers rooms. Faces, portraits, item pictures, the figure layer and the Fight panel are built too (the Coupler gaps plan, Phases B to G; `docs/coupler-handback.md`); only the room picture's finer detail waits on Coupler's by-eye judgement. Wiring Coupler to the engine is Coupler's session's work.

Last, because it needs everything above. Coupler is Rust/Tauri and this
engine is JavaScript, so the two must meet without making Coupler lose its
guarantees (rule 12: sound and pictures never break; rule 6: pure Rust,
unit-tested).

- **Parity inventory first.** List what `painter.rs`, `portrait.rs`,
  `face.rs`, `item_art.rs`, `figure.rs`, `fight_art.rs` and `create.rs`
  draw: the 24 CoffeeMUD terrains, indoor materials, time and weather, the
  two styles (fantasy, high fantasy), faces and the small 16 by 8 face,
  items and effect looks, The Fight's frames. Each becomes a row in a kit
  (`coupler-painter`) with a status, so `sprites kits` is the migration
  list.
- **Scene input is data, not Coupler's types.** The recipe takes
  `{ terrain, indoors, time, weather, style, colors, seed }`; Coupler maps
  its `paint::Scene` to that and keeps `paint::seed`'s FNV-1a so a room
  keeps its look from visit to visit. Never pass talk, players' names or
  anything the player typed (paint.rs's rule).
- **How Coupler runs it** is a decision for Coupler's session: the
  JavaScript engine in the webview (it is browser-safe) with Rust passing
  the scene, or another route. The web build and the phone must draw the
  same pictures. Whatever the route, Coupler's accessibility rule stands:
  a picture's facts are always in words beside it.
- **Golden tests across the move:** the same scene before and after, with
  differences judged by eye on a contact sheet, since the output will not
  be byte-identical to the Rust painter's.
- Coupler's own docs and rules are changed from a session in Coupler, not
  from here.

## Risks

- **Rule drift:** the engine's guarantees (determinism, no Node built-ins in
  `index.js`, no outside imports) must hold for every new module. Both are
  already tested; the new entries (`ansi.js`, `cells.js`) must be covered.
- **Pixel output changing by accident:** guarded by the Phase 0 golden hash.
- **One engine for every ansiapps app** makes this package a shared
  dependency: a change to existing output reaches every app. Keep the
  golden hashes, version with semver, and say in the changelog when a
  look changes.
- **Copies elsewhere:** Idle Solo's `sprites/` is a separate copy still named
  `pixel-sprites`; carrying the rename over is that project's step.
