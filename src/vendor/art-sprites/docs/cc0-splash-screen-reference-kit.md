# Splash Screen Reference Kit (pixel art and ANSI art)

Compiled 2026-11-01 for the `splash` recipe: **two parameters, a `title` and a `scene`**, drawn as pixel art (256 by 144, DB32) or as ANSI art (80 by 24, classic-legal).
**Permissive material only.** A source is listed as a source only if its own page states CC0 or another permissive licence. Everything else is under *Excluded*. Nothing here is bundled, traced or used as a fixture (rule 10: reference rows are never drawn or shipped); the pictures are drawn by the recipe from a seed.

## How a licence was checked

| Mark | Meaning |
| --- | --- |
| **verified** | The licence was read on the item's own page (or the publisher's own licence page) on 2026-11-01. |
| **unverified** | A search listing says CC0, but the page itself could not be fetched (OpenGameArt answers a fetch with HTTP 403). Not relied on; check the page in a browser before taking anything from it. |

## Permissive sources

### Licence verified

- **IAMST, Commercial Title Asset Pack** (itch.io) - separate PNG title graphics to arrange in an image editor under a game's name. Licence on the page: *Creative Commons Zero v1.0 Universal*, "Use for anything"; $1 for the full pack, free sample. https://iamst.itch.io/commercial-title-asset-pack. The page names the font Luckiest Guy and gives no licence for it: **do not take the font from here**. Study what a title lockup is made of (a heavy face, a light-to-dark ramp, a dark edge, a drop shadow), then draw your own.
- **Kenney** (UI Pack, Pixel UI Pack, UI Pack Adventure, Pixel Adventure, Background Elements): every asset on the asset pages is CC0 per https://kenney.nl/support ("public domain licensed (CC0)", free for commercial use, credit optional). **Kenney's logo is reserved**: never use it. Use for how menu plates, buttons and backdrops are proportioned. https://kenney.nl/assets/ui-pack, https://kenney.nl/assets/pixel-ui-pack, https://kenney.nl/assets/background-elements
- **awesome-textmode** (the index; CC0-1.0, already in `docs/cc0-ansi-art-learning-reference-kit.md`): the list is CC0, the things it links to are not. https://github.com/humanbydefinition/awesome-textmode
- **IBM VGA 8x16 by VileR** (CC BY-SA 4.0, `docs/fonts/CREDIT.md`): the project's standard font; the pixel splash title is set from its atlas, unmodified.

### Licence unverified (OpenGameArt, listed CC0 in search; pages not fetchable)

- *Pixel-Art Backgrounds* - nine images, some loopable, hand drawn in the Endesga 32 palette. https://opengameart.org/content/pixel-art-backgrounds-0 (Endesga 32 is a 32 colour palette like DB32: a good yardstick for how many colours a scene needs.)
- *Good CC0 Pixel Game Backgrounds* - pixel landscapes; the page says no AI was used. https://opengameart.org/content/good-cc0-pixel-game-backgrounds
- *Simple title screen* - a title background with and without text. https://opengameart.org/content/simple-title-screen
- *Menu Backdrop* - used as a main-menu backdrop. https://opengameart.org/content/menu-backdrop
- *Cityscape* - a pixel city made as a game's intro screen. https://opengameart.org/content/cityscape

## Excluded

- **Attribution licences** (listed to stop anyone using them by mistake, not studied): *Grungy Title Screen Background* (CC-BY 3.0), *Logo* by KBArt (CC-BY 4.0). Both listed on OpenGameArt; unverified.
- **BBS and demoscene ANSI** (16colo.rs, artpacks): not CC0; artists hold the rights. Never bundled, traced or used as a test file (rule 8).
- **ANSI generators with copyleft licences** (Welcome2L, a BBS-style login logo maker, GPL): not permissive; not read for code.
- **Articles on splash and title design** (UXPin, UX Planet, eDirectory, Playgama, Unity docs, Lenovo glossary): all rights reserved. They were read for orientation only; no text, figure or example was copied, and none is a source for this kit. The rules below are stated independently and are common practice.
- **Wikipedia pages on splash screens and ANSI art**: CC BY-SA, share-alike; not permissive in the sense this kit uses.
- **ansiart (MIT, PyPI), ansi-viewer (MIT)**: permissive, but they convert or view images; the engine already has `fromImage` for study. No code taken.

## What a splash is (the rules the recipe follows)

1. **One focal point: the title.** Everything else is quieter than the letters.
2. **A plate under the letters.** The title sits on a dark band across the upper third, so it reads over any scene (pixel: solid `night`, dithered edge rows, a `plum` rule; ANSI: solid black cells and a box line).
3. **A quiet scene.** Soft value steps and one light source; the busiest area is away from the title. Layers recede: lighter and bluer far, darker and warmer near.
4. **Margin on every side.** Letters never touch an edge. Pixel title: at most 28 characters (8 pixels each in a 256 wide canvas).
5. **Dither the seams, not the surfaces.** Sky bands blend with a 4 by 4 Bayer dither; ridges keep a one-pixel lit edge.
6. **One scene, one palette family.** A sunset scene is violet, rose, orange, peach; a night scene is night, indigo, blue with a few bright points.
7. **Lettering from the standard font.** Pixel text comes from the VGA atlas, 16 pixels, no bold or italic, passed in as `atlas`. ANSI lettering is the engine's own 3 by 5 block face, falling back from `shaded` to `block` to `half` as the title grows.
8. **Short.** A splash is a still. Timing and fades belong to the game.

## The two parameters

- **`title`**: text. Pixel: any CP437 text up to 28 characters; without `atlas` the plate is drawn and the letters are not (like `map`'s `label`). ANSI: upper-cased; the lettering font has `A-Z 0-9 - ! . :` and space, other characters are dropped; fails if nothing is left or the title cannot fit 80 columns in `half`.
- **`scene`**: `meadow`, `forest`, `desert`, `mountains`, `sea`, `snow`, `night`. In ANSI each is a `vista` (plains day, woods dusk, desert day, mountains dawn, watersurface dusk, hills with snow, plains night).
- Optional: `seed` (default `splash`), same title, scene and seed, same picture.

## Briefs (study order)

1. Pixel: draw the sky as four dithered bands, then the sun or moon, then three ridges far to near.
2. Pixel: set the title on the plate with the atlas, white with a black contour.
3. ANSI: let `vista` draw the scene, then lay the plate and the lettering over it.
4. Judge all seven scenes on a contact sheet (`sprites sheet`) for the same title.

## Left to do

- Subtitle, credit and "press any key" lines (a third text param would break the two-parameter contract; a game sets these itself).
- A town, castle or ship scene; a scene with a character in it.
- A faded or ansimation splash.
