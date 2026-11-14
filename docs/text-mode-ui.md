# JoyrIDE's text-mode UI

**Decided 2026-11-13.** JoyrIDE's interface is the ANSIapps theme and
nothing else. Unlike the other ansiapps apps there is no "modern" theme
and no switch for one; the graphical look does not exist in this app. The
rules are the ANSIapps theme's, as built on the grid in Coupler (Coupler's
`docs/ansiapps-theme.md`, "Design rules" and "The grid"); this file is
what JoyrIDE does with them, and where it differs.

## The one option: the colour palette

Settings (menu bar, `Settings…`) has a single choice, remembered across
launches and applied before first paint (`index.html`, key
`joyride.palette`, `src/lib/theme.ts`):

| Palette | What it means |
|---|---|
| **True colour** (default) | 24-bit colour with a choice of **theme swatches** (below), the way an editor offers dark, light and well-known schemes. |
| **16-colour (DOS mode)** | Every colour of the UI and the editor is one of the 16 `--dos-*` colours, fixed: no swatches, no tints, no translucent selection (it is black), no current-line highlight. |

### Theme swatches (True colour only)

`src/swatches.ts`: Classic blue (the default), Dark, Light, Solarized dark,
Solarized light, Amber phosphor and Green phosphor. A swatch is only
colours: the font, grid and frames never change. It sets the role tokens
the stylesheet reads (`--bg`, `--text`, `--r-win`, `--r-ink`, ...) and gives
Monaco its syntax tones, selection and tints (`src/monaco-theme.ts` builds
one Monaco theme per swatch). The choice is stored (`joyride.swatch`, with
the colours in `joyride.ui`) and `index.html` applies it before first
paint; switching to DOS mode keeps the swatch, and switching back restores it.

To add a swatch: add it to `SWATCHES`, then run
`node scripts/check-swatches.ts`; every text and background pair (UI,
console, syntax on the editor, the current line and the selection, line
numbers) must be 4.5:1, frame lines and the progress fill 3:1.

The user's own colours are not ours to restrict: the swatches Monaco
shows for `love.graphics.setColor(...)` and the game's own output keep
their real values in both palettes (as Stylus's art canvas and Coupler's
game output do).

Every text and background pair comes from
`docs/ansiapps-color-contrast.md` (DOS mode) or is 4.5:1 or better by
the same formula (every swatch, checked by `scripts/check-swatches.ts`).

## The screen

- A light-gray **menu bar** (Turbo Vision) across the top: Open Folder,
  Run, Stop, Reference, Libraries, GitHub, Auto-Port, Export, Settings.
- A **sidebar** of 38 cells (file buttons, find in files, the tree) with a
  double rule between it and the editor.
- **Tabs** that wrap to another row rather than scroll, the **editor**
  (Monaco, below), and the **console**: a black DOS prompt in a framed
  panel; errors are light red on black, links are underlined.
- A light-gray **status bar**.
- **Panels** (Reference, Libraries, GitHub, Auto-Port, Export, Settings)
  are double-line framed windows with the title in the top edge, a close
  box `[■]` at the top left and, where needed, buttons at the top right.
  Escape closes. `components/Panel.tsx`.
- The **window's content is cut to whole cells** (`.shell`), so a resized
  window never leaves a half cell.
- The **startup screen** (static in `index.html`, then `Splash.tsx`): blue,
  a small ANSI joystick, the name, a block progress bar that steps to each
  new width (indeterminate: a sweep in 12 whole steps), no fade.

## The editor

Monaco is set up as a text-mode screen (`src/monaco-theme.ts`): the VGA
font at 16px on 16px lines (so its cells are 8 by 16 like the rest), block
cursor, no minimap, no smooth scrolling, no folding column, 16px scroll
bars, and a Borland-style blue background. Popups (suggest, hover, find,
menus) are light-gray windows with black text and a green highlight and a
hard shadow. Monaco keeps its own scrolling and text metrics; the grid
check does not measure inside it.

### The editor's own font (Settings)

The one place the VGA font is not required: a person may pick a coding font for the editor in Settings (`src/lib/codefonts.ts`; the
maintainer's decision, 2026-11-14). Everything else stays VGA on the 8 by 16 grid. The default is still VGA. Offered: JetBrains Mono,
Source Code Pro, Hack, IBM Plex Mono, Inconsolata, Fira Code (ligatures off), Intel One Mono, DejaVu Sans Mono, and Spleen, a pixel font
in five cell sizes (6x12 to 32x64). All Regular, unmodified, licence beside each in `public/fonts/code/licences/`, listed in `NOTICE`.

- **Sizes are only the clean ones.** A vector font is offered at the pixel sizes where its character advance is within a quarter of a
  device pixel of whole, and Monaco is given the letter-spacing that makes it exactly whole (so columns do not drift); line height is whole
  device pixels. A pixel font is offered at whole multiples of the size it was drawn for. Laptop sizes are the small ones (10 to 16),
  desktop 17 to 24, large displays above that; Spleen's cells reach 32 by 64.
- **The window snaps.** When a resize ends (and when the font or size changes), the window moves by whole shell cells (8 by 16) to the
  nearest size where the editor's text area is whole characters wide and whole lines tall (`src/lib/snap.ts`, `windowsnap.ts`). Not when
  maximized or full screen, with no editor on screen, or when no nearby size lines up; two nudges at most, so it cannot fight the layout.
- `node scripts/check-code-fonts.ts` reads each file's own metrics and licence; `node scripts/check-snap.ts` checks the sums.

## What to remember when adding UI

- **Whole cells**: every width, padding, margin and gap a multiple of 8px
  across and 16px down; every line 16px; buttons and fields one row tall.
- **No border on anything holding text.** A frame is `::before` in a ring
  of padding (see `.panel`).
- **Characters the font has.** No `⌘ ⇧ ⌥ ⌃ ▸ ▾`; write `Cmd+Shift+D`, use
  `► ▼`. The font has box drawing, blocks, arrows `← → ↑ ↓`, `■ □ ▲ ▼ ► ◄ ○ • · × … ✓`.
- **No bold, italic, easing, fades or gradients** (the progress fill's
  hard-stop stripes are the one exception, as in the theme).
- **A mark that can land on several row colours** gets its own black cell
  (`.mark`, Auto-Port severities).
- **A row's own button comes first** (Libraries' Add/Remove).
- **Check it**: in `npm run tauri dev`, call `checkGrid()` in the web
  inspector on each panel; it lists and outlines what is off the grid.

## The font

IBM VGA 8x16 (the "Plus" variant) from The Ultimate Oldschool PC Font
Pack v2.2 by VileR, int10h.org, **CC BY-SA 4.0**, shipped unmodified as
its own file with its licence in `public/fonts/ansiapps/`, credited in
Settings and `NOTICE`. Never subset, convert (woff2), rename glyphs or
inline it as a data URI; the rest of the licensing reasoning is in
Coupler's `docs/ansiapps-theme.md`, "Licensing".

## Not done

- Seen in a real window and run through `checkGrid()`.
- The shared ANSI theme pack (button and frame art, icons) once Stylus
  renders it; until then frames and bars are CSS.
- An About box (the font credit is in Settings for now).
