# What LÖVE loads natively, and what the Asset Library tracks

Measured on the real LÖVE 11.5 and 12.0 binaries on this Mac (2026-11-13). "Measured" means
`scripts/love-asset-formats.py` made a sample file and each LÖVE loaded or refused it;
"in the binary" means the format's handler class is present in `love.framework` but no
sample file was tried; "documented" means LÖVE's own documentation and no measurement here.
The Asset Library's idea of a kind is `asset-core/src/kind.rs` (`classify`, by extension).

## Tracked today

Images `png jpg jpeg gif bmp tga webp dds ktx`, audio `wav ogg oga mp3 flac mod xm it s3m mid midi`,
fonts `ttf otf fnt woff woff2 bdf`, shaders `glsl frag vert shader`, data `json toml csv tmx tsx tsv xml ldtk`,
video `ogv webm mp4`, Lua `lua`.

## LÖVE loads these natively and the library does not track them

| Kind | Extension | LÖVE loads it | How it is known |
|---|---|---|---|
| Image | `.hdr` (Radiance, 32-bit float) | 11.5 and 12.0 | measured |
| Image | `.exr` (OpenEXR) | 11.5 and 12.0 | in the binary (`EXRHandler`) |
| Image | `.qoi` | **12.0 only** | measured (11.5 refuses it) |
| Compressed texture | `.pkm` (ETC), `.pvr`, `.astc` | 11.5 and 12.0, through `love.image.newCompressedData` | in the binary (`PKMHandler`, `PVRHandler`, `ASTCHandler`); `.dds` and `.ktx` are tracked |
| Tracker music | `.abc .amf .ams .dbm .dmf .dsm .far .j2b .mdl .med .mt2 .mtm .okt .pat .psm .stm .ult .umx` and `.699` | 11.5 (libmodplug's extension table); 12.0 also links libmodplug | in the binary. LÖVE's table spells it `699`, not `669`. `mod xm it s3m` and `mid` are tracked |
| Font | `.ttc` (TrueType Collection) | 11.5 and 12.0 | measured |
| Font | Type 1 (`.pfb .pfa`), `.pcf` | probably, through FreeType | not measured |
| Bitmap font pages | the PNG that a `.fnt` (BMFont) points at, and an image font's glyph string | yes, through `love.graphics.newFont` and `newImageFont` | documented; the library tracks the `.fnt` but not its pages, so a copy can lose them |
| Archive | `.love`, `.zip` (and a folder), mounted with `love.filesystem.mount` | yes | documented; the file preview lists their members |
| Lua bytecode | `.luac`, any file LuaJIT precompiled | yes, `love.filesystem.load` | documented |
| Shader source | `.fs .vs .fsh .vsh .fx` or any name | yes, `love.graphics.newShader` reads text and does not care about the extension | documented |
| Controller mappings | `gamecontrollerdb.txt` | yes, `love.joystick.loadGamepadMappings` | documented |

## Tracked as a LÖVE asset, but LÖVE does not load it

| Extension | What LÖVE does |
|---|---|
| `.gif` | refused, on both (measured). Fine as source art; convert to PNG or a sprite sheet for the game |
| `.mp4`, `.webm` | LÖVE plays Ogg Theora `.ogv` only (documented; only the Theora library is bundled) |
| `.midi` | LÖVE's table has `mid`, not `midi` |
| `.woff2` | not measured; `.woff` loads (measured) |
| `.webp` | 12.0 only (`WEBPHandler` is in 12.0's binary and absent from 11.5's) |
| `.psd`, `.ppm`, `.pgm` | refused, on both (measured); not tracked, which is right |
| `json tmx tsx ldtk csv toml xml` | not loaded by LÖVE itself: it has no parser for them, a library or the game reads them as text. Tracked as data, which is useful, but they are not native assets |

## What is checked against the binaries

`scripts/love-asset-formats.py` repeats the measured rows: HDR, TGA, QOI, GIF, PSD, PPM and PGM as
images; TTF, OTF, TTC and WOFF as fonts. Results on 2026-11-13: HDR and TGA load on both, QOI only on
12.0, GIF PSD PPM PGM refused on both, and the four fonts load on both.

## To do (ROADMAP section 5)

Track the rows in the first table, mark the second table's rows as "not loadable by LÖVE" in the catalog
rather than silently accepting them, pair a BMFont `.fnt` with its pages, and read the version a format
needs (`.qoi`, `.webp`: 12.0) so a project on 11.5 is warned.
