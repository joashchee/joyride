# CC0 ANSI Art Learning Reference Kit

A curated, license-aware reference kit for learning to draw **ANSI art**, with emphasis on CC0/public-domain materials where they exist.

> **License caveat:** Most classic ANSI tutorials and artworks are *not* CC0. They are BBS/demoscene art with artist-held rights. Use them to study technique; do not redistribute, modify, or remix them unless the specific artist or archive grants permission. The **awesome-textmode** repository itself is CC0-1.0, but its linked resources may use different licenses.

## Start Here

- **awesome-textmode** — The best single index of textmode/ANSI resources: archives, editors, fonts, communities, and libraries. Repository: https://github.com/humanbydefinition/awesome-textmode — CC0-1.0 for the list itself; linked resources retain their own licenses.
- **teXt0wnz ANSI drawing tutorials** — Built-in library of 20+ historical ANSI tutorials covering colors, shading, fonts, curves, and character drawing. https://github.com/xero/text0wnz/wiki/tutorials
- **16colo.rs** — Major archive of ANSI/ASCII artpacks, artist profiles, group histories, and forums. https://16colo.rs/
- **Demozoo** — Large demoscene database with ANSI, ASCII, and PETSCII graphics plus artist and release metadata. https://demozoo.org/

## ANSI Drawing Tutorials

Study these for technique. Unless a specific artist/page grants redistribution rights, do not redistribute or modify the source files.

- **Ansi Academy** — LDA of Mistigris, 2021; modern introductory ANSI tutorial.
- **Basic Colors** — Prisoner #1 of Fire, 1996; foundational color selection and use.
- **Color Science** — Avenger of Maiden Brazil, 1998; color relationships and palette thinking.
- **Curving and Shading** — Nootropic of Dark, 1996; curves and tonal transitions.
- **Picture Shading** — Sodium of Odium, 1996; shading representational/picture-style ANSI.
- **Advanced Picture Tutorial** — Lord Soth of iCE, 1996; advanced image-building workflow.
- **Shading Techniques** — Prisoner #1 of Fire, 1996; practical shading patterns.
- **The “Halshade” Shading Technique** — Halaster of Fire, 1996; named scene shading method.
- **How to Draw Like zeroVision** — zeroVision of Blocktronics, 2008; detailed artist-process tutorial.
- **zeroVision’s Font Tutorial** — zeroVision of Blocktronics, 2009; ANSI lettering and logotype construction.
- **Jazz’s Font Drawing, Parts 1–2** — Lord Jazz of 765, 1996; classic font-drawing instruction.
- **Draw Fonts the Gunthar Way** — Gunthar of Fire, 1997.
- **Flame’s Font Tutorial** — Flame of Humid, 1996.
- **rorshack’s wERiD fONt Tutorial** — rorshack of Maiden Brazil, 1997.
- **enz0’s Flying Eagle Tutorial** — enz0 of Blocktronics, 2013; animal/figure-oriented ANSI drawing.
- **Female Character Drawing** — enz0 of Blocktronics, 2014; character illustration workflow.
- **Hulk Face Tutorial** — Fever/TCF of Blocktronics, 1997; face construction and shading.
- **How to Draw Like Halaster** — Halaster of Fire, 1996.
- **Ansi Tutorial** — Bisounours of Tiny Toons, 1998.
- **14 Steps to Scene Domination, Parts 1–2** — Hennifer of Lazarus, 1997; broader scene-art process.
- **Phiber Optic** — Shaitan of FBK.Sargahd/Asgaard, 1996.

Tutorial index: https://github.com/xero/text0wnz/wiki/tutorials

## Web Tutorials and Explainers

- **Roy/SAC: How to Draw Like Zerovision** — Step-by-step ANSI workflow in PabloDraw, including outlining, color blocking, and shading. http://www.roysac.com/tutorial/ansitut-zv.html
- **Roy/SAC: Newt’s ANSI Shading Tutorial** — Classic 1996 shading tutorial focused on outlines, shape building, and shade placement. https://www.roysac.com/tutorial/nt-how2shadingtutorial.html
- **Roy/SAC block-ASCII tutorials** — CP437 block-shading approaches. https://github.com/dwgx/SmartCLI/blob/main/knowledge/sources/works-galleries.md
- **Recreating ANSI Art from a Screenshot** — Technical explanation of CP437 glyphs, foreground/background colors, bold/bright colors, and ANSI escape sequences. https://bert.org/2023/02/27/recreating-ansi-art-from-a-screenshot/
- **Maximus BBS: ANSI Art & RIP Graphics** — Primer on the 80-column canvas, CP437 ranges, box drawing, half blocks, and shading characters. https://maximusng-bbs.github.io/maximus/display-ansi-rip/
- **ANSI Art: the BBS-era graphics standard, explained** — Historical and practical overview of editors, the character grid, CP437, and the 16-color palette. https://www.sdelic.org/ansi-art
- **How to Make ASCII Art That Doesn’t Look Bad** — Modern workflow for image-to-character conversion, cell aspect ratio, resizing, and brightness ramps. https://247plan.net/articles/how-to-make-ascii-art
- **ANSI Art Editor for Mac — Clean .ANS Files, SAUCE & iCE Colors** — Explains the half-block “two pixels per text cell” technique and CP437 glyph banks. https://www.hermesbbs.com/hermes4/ansi-editor

## Core Technique Cheat Sheet

ANSI art is a character grid: CP437 characters plus ANSI color/cursor escape sequences, traditionally displayed in an IBM PC-style font.

### Canvas

- Start at **80 columns**.
- **25 lines** approximates a full DOS screen; modern pieces may be taller.
- Treat each character cell as a pixel-like unit.

### Shading Ramp

Use these CP437 glyphs for density:

```text
(space) ░ ▒ ▓ █
```

- `░` — light shade
- `▒` — medium shade
- `▓` — dark shade
- `█` — full block
- Space — empty/background

### Half-Block Pixels

Use foreground and background color with:

```text
▀ ▄
```

- `▀` — upper half block
- `▄` — lower half block
- This turns an 80×25 text grid into an 80×50 pixel canvas.

### Lines and Frames

CP437 includes single and double box-drawing characters:

```text
─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼
═ ║ ╔ ╗ ╚ ╝ ╠ ╣ ╦ ╩ ╬
```

### Practical Workflow

1. Choose a reference image you have the right to use.
2. Block in large color areas.
3. Establish the outline.
4. Refine shading and highlights.
5. Avoid random shade noise; deliberate placement matters more than density.
6. Save as `.ANS` and preserve SAUCE metadata when possible.

## CC0 / Public-Domain Assets

These are the strongest clearly reusable items found. Always verify the license on the actual project page before redistribution.

### CC0 Resource Lists and Fonts

- **awesome-textmode repository** — CC0-1.0 curated list. https://github.com/humanbydefinition/awesome-textmode
- **ATASCII font** — Dmian; CC0.
- **BESCII font** — Dmian; CC0.
- **CPC464 font** — Dmian; CC0.
- **DUNGEON.mode font** — datagoblin; CC0.
- **Public Pixel Font** — GGBotNet; CC0, monospaced 8×8 with 1,324 glyphs.
- **Rook font** — outer spec; public domain.
- **UNSCII** — viznut; public domain; `unscii-16-full` variant is GPL.
- **UrsaFont** — UrsaFrank; CC0; covers ANSI-standard and extended symbols.

### CC0 / Public-Domain Reference Images

Use these as drawing references for original ANSI compositions, not as ANSI source files.

- **Smithsonian Open Access** — CC0 images and data. https://www.si.edu/openaccess
- **Library of Congress Free to Use and Reuse Sets** — predominantly public-domain/CC0 image sets. https://www.loc.gov/free-to-use/
- **Openverse** — Search layer for openly licensed and public-domain images and audio; filter carefully by license. https://openverse.org/
- **Wikimedia Commons** — Large repository of public-domain and freely licensed media; verify each file’s exact license. https://commons.wikimedia.org/
- **Pixabay** — Free-to-reuse images under the Pixabay license; permissive but not identical to CC0. https://pixabay.com/

## Editors and Tools

- **Moebius** — Modern open-source ANSI/ASCII editor by Blocktronics; Apache-2.0; half-block brushes and collaboration. https://github.com/blocktronics/moebius
- **PabloDraw** — Cross-platform ANSI/ASCII/RIPscrip editor; MIT. https://github.com/blocktronics/pablodraw
- **teXt0wnz** — Offline-first web app for ANSI, ASCII, NFO, and XBIN; MIT; half-block brushes, vintage fonts, SAUCE metadata, and tutorial library. https://github.com/xero/text0wnz
- **create.textmode.art** — Browser-based grid/glyph drawing editor. https://create.textmode.art/
- **editor.textmode.art** — Live-coding browser editor built on textmode.js. https://editor.textmode.art/
- **GlyphDrawing.Club** — Web editor for modular type and text art with adjustable grids. https://glyphdrawing.club/
- **lvllvl** — Web textmode editor with PETSCII, teletext, layers, animation, and custom fonts. https://lvllvl.com/
- **REXPaint** — Windows-friendly ASCII editor with layers, custom fonts, and ANSI/PNG export; freeware, not open source. https://github.com/maikklein/REXPaint
- **Playscii** — Open-source ASCII art, animation, and game creation program; MIT. https://jp.itch.io/playscii
- **Durdraw** — Terminal ANSI/Unicode/ASCII editor and animator; BSD-3-Clause. https://github.com/cmang/durdraw
- **Textual Paint** — MS Paint-inspired terminal image editor with ANSI support; MIT. https://github.com/1j01/textual-paint
- **DarkDraw** — Terminal Unicode art/animation editor with 256-color support and custom palettes; check repository license before redistribution.
- **TheDraw** — Classic DOS-era ANSI editor; historically important and referenced throughout older tutorials, but not an open-license modern tool.

## Archives for Study

Use these to reverse-engineer composition, palette choices, font design, and shading. Do not treat archived artwork as CC0 unless explicitly stated.

- **16colo.rs** — ANSI/ASCII artpacks from the BBS era to today. https://16colo.rs/
- **Demozoo** — 62,000+ graphics including ANSI, ASCII, and PETSCII works. https://demozoo.org/
- **pouët.net** — Major demoscene portal with 100,000+ productions. https://www.pouet.net/
- **ASCII Art Archive** — Large ASCII archive with drawing tools, games, animations, and a searchable Usenet mirror. https://www.asciiart.eu/
- **text-mode.org** — Curated archive spanning ASCII, PETSCII, teletext, and other text graphics. https://text-mode.org/
- **Lospec textmode gallery** — Curated user-submitted textmode art. https://lospec.com/gallery?tag=textmode
- **C64GFX** — 33,000+ Commodore 64 graphics, including PETSCII art. https://c64gfx.com/
- **CSDb** — Commodore 64 Scene Database with 250,000+ releases. https://csdb.dk/
- **Asciiville** — Includes nearly 1,000 ASCII/ANSI works; project says it includes only freely redistributable art, but confirm the license for any specific piece. https://asciiville.dev/
- **Ansilove examples** — Sample ANSI/ASCII works created for Ansilove by different artists. https://www.ansilove.org/examples.html
- **Roy/SAC ANSI gallery** — Authentic hand-made ANSI examples with original `.ANS` links; explicitly **not public domain**. http://www.roysac.com/otheransi.html

## Communities

- **Sixteen Colors Discord** — Community around the ANSI/ASCII artpack archive. https://16colo.rs/
- **aSCIIaRENA Discord** — ASCII archive and colly community rooted in BBS/demoscene traditions. https://asciinet.net/
- **textmode.art Discord** — General textmode hub covering art, fonts, libraries, and tools. https://textmode.art/
- **GlyphDrawing.Club Discord** — Community for the browser text-art/type editor. https://glyphdrawing.club/
- **Lospec Discord** — Low-spec art community covering pixel art, voxel art, and textmode. https://lospec.com/
- **Mistigris computer arts Discord** — Long-running computer-art collective, founded in 1994. https://mistigris.org/
- **PETSCII WORLD Discord** — PETSCII-focused community; useful for transferable grid-art skills.

## License Practice

- **CC0** — “No rights reserved.” You can generally copy, modify, distribute, and use the work commercially without attribution, though credit is good practice.
- **Public domain** — Similar practical effect, but provenance matters; prefer explicitly marked CC0 where possible.
- **CC BY** — Requires attribution.
- **CC BY-SA** — Requires attribution and share-alike licensing for adaptations.
- **“Freely redistributable” is not CC0.** For example, Roy/SAC states that its ANSI gallery pieces may be copied or shown around but may not be modified or reused without the artist’s permission.
- **Fonts need separate checks.** Font licenses vary even when the surrounding project is open source.

## Practical Learning Path

1. Install **Moebius** or open **teXt0wnz**.
2. Work through **Ansi Academy**, **Basic Colors**, **Curving and Shading**, and **Shading Techniques**.
3. Practice the CP437 ramp `░ ▒ ▓ █` and half-blocks `▀ ▄` until you can create smooth gradients and clean edges.
4. Use a public-domain or CC0 reference image locally; do not trace or redistribute copyrighted source images.
5. Draw a simple 40×20 logo.
6. Draw an 80×25 border/menu screen.
7. Draw a shaded character or object.
8. Study one historical artist at a time on **16colo.rs** or **Demozoo**.
9. Recreate techniques in original compositions rather than editing archived files.
10. Save with SAUCE metadata where your editor supports it.

## Suggested Practice Briefs

- **Logo:** 40×20, 4 colors, one font, one outline color, one highlight color.
- **Menu screen:** 80×25, box-drawing frame, title, five menu items, decorative shading.
- **Landscape:** 80×25, sky gradient using `░ ▒ ▓`, foreground silhouette, one focal object.
- **Character portrait:** 80×50 using half-blocks, three-value lighting, clean contour.
- **Original artpack piece:** 80×25 or taller, consistent palette, readable font, SAUCE metadata.

## Quick Palette Starters

### Classic BBS

```text
Black background
Bright white — highlights
Bright cyan — secondary highlights
Bright magenta — accents
Bright yellow — warm accents
Bright red — emphasis
Bright green — nature / success
Bright blue — cool shadows
```

### Low-Contrast Study

```text
Black background
Dark blue — base shadow
Blue — mid shadow
Cyan — mid light
Light cyan — highlight
White — specular highlight
```

## Minimal ANSI Starter

```text
┌──────────────────────────────┐
│  ANSI ART PRACTICE           │
│  ░▒▓█                        │
│  ▀▄ half-block pixels        │
│  ─ │ ┌ ┐ └ ┘                 │
└──────────────────────────────┘
```

## Sources

- awesome-textmode: https://github.com/humanbydefinition/awesome-textmode
- teXt0wnz tutorial wiki: https://github.com/xero/text0wnz/wiki/tutorials
- teXt0wnz repository: https://github.com/xero/text0wnz
- Roy/SAC zeroVision tutorial: http://www.roysac.com/tutorial/ansitut-zv.html
- Roy/SAC Newt shading tutorial: https://www.roysac.com/tutorial/nt-how2shadingtutorial.html
- Recreating ANSI Art from a Screenshot: https://bert.org/2023/02/27/recreating-ansi-art-from-a-screenshot/
- Maximus BBS ANSI primer: https://maximusng-bbs.github.io/maximus/display-ansi-rip/
- ANSI Art explained: https://www.sdelic.org/ansi-art
- ASCII art workflow: https://247plan.net/articles/how-to-make-ascii-art
- ANSI editor / half-block technique: https://www.hermesbbs.com/hermes4/ansi-editor
- Asciiville: https://asciiville.dev/
- Ansilove examples: https://www.ansilove.org/examples.html
- Roy/SAC gallery and license note: http://www.roysac.com/otheransi.html
