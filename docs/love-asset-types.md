# What LÖVE loads natively, what the library tracks, and who can load it

Measured on the real LÖVE 11.5 and 12.0 on this Mac (2026-11-13) with `scripts/love-asset-formats.py`:
tiny files it writes, and the **real sample files of r3i6nos's corpus** (`corpus/files`, 251 files of
about 250 formats; many are small *identification stubs*, see "How far to trust a refusal" below).
`asset-core/src/kind.rs` holds the answer in code: `classify` (what the library tracks) and
`love_support` (what LÖVE does with each extension), and the file preview prints it as the
"In LÖVE" line.

## Corrections to the first version of this file

Measuring with real files overturned four things said earlier: LÖVE picks an **audio decoder by the
file's content, not its extension** (a MIDI named `.xyz` loads); **`.midi` loads** (it was listed as
refused); **`.webp` loads on 12.0** (measured now, not only seen in the binary); and **WOFF2 and BDF
are refused** (they were "not measured").

## Tracked now (`classify`)

| Kind | Extensions |
|---|---|
| Image | `png jpg jpeg gif bmp tga webp` and, new, `hdr exr qoi` and the compressed-texture containers `dds ktx pkm pvr astc` |
| Audio | `wav ogg oga mp3 flac midi` and, new, every extension in LÖVE's tracker table: `669 699 abc amf ams dbm dmf dsm far it j2b mdl med mid mod mt2 mtm okt pat psm ptm s3m stm ult umx xm` |
| Font | `ttf otf fnt woff woff2 bdf` and, new, `ttc pcf pfb pfa` |
| Shader | `glsl frag vert shader` and, new, `fsh vsh` |
| Data | `json toml csv tmx tsx tsv xml ldtk` and, new, a file named `gamecontrollerdb.txt` |
| Video | `ogv webm mp4` |
| Script | `lua` and, new, `luac` |
| Archive (new kind) | `love zip` |

`.fs`, `.vs` and `.fx` are not tracked as shaders: they are as often F#, Visual Studio and HLSL files.

## What LÖVE does with each (measured unless noted)

| Format | LÖVE 11.5 | LÖVE 12.0 | Evidence |
|---|---|---|---|
| PNG, JPEG, BMP | loads | loads | standard (not re-measured) |
| TGA, HDR | loads | loads | measured |
| QOI | refused | **loads** | measured, real file |
| WebP | refused | **loads** | measured, real file |
| DDS (BC1, BC3, BC7), KTX 1 (ETC2), PKM (ETC1), PVR 3 (PVRTC), ASTC 4x4 | loads | loads | measured 2026-11-13 with hand-built one-block files through `love.image.newCompressedData` (headers only; no GPU, so whether a device can *draw* the format is not tested) |
| EXR, scanline (half or float, raw or RLE) | loads | loads | measured with hand-built files |
| EXR, **tiled** | **aborts the game** | **aborts the game** | measured: `newImageData` on a tiled file (the Rust `image` crate writes these) kills LÖVE with SIGABRT, uncatchable by `pcall`; the preview warns |
| GIF, PSD, PCX, ICO, TIFF, PPM, PGM | refused | refused | measured |
| WAV, FLAC, MP3, Ogg Vorbis | loads | loads | measured |
| Opus, AIFF, WMA, WavPack | refused | refused | measured |
| M4A | loads (it was read as MP3) | refused | measured; do not rely on it |
| MOD, XM, IT, S3M | loads | loads | measured |
| MIDI (`.mid`, `.midi`, any name) | loads | loads | measured |
| RMI (RIFF MIDI) | refused | refused | measured |
| 669, AMF, DBM, FAR, MTM, MED, OKT, PTM, STM, ULT | **refused the sample** | **refused the sample** | the corpus samples are stubs (below); **not confirmed either way** |
| ABC, AMS, DMF, DSM, J2B, MDL, MT2, PAT, PSM, UMX | listed in LÖVE's table | listed | no sample tried |
| SID, NSF, HVL, AHX and the other chip and tracker formats Neumetik reads | refused | refused | measured; LÖVE never listed them |
| TTF, OTF, TTC, WOFF | loads | loads | measured |
| WOFF2, BDF | refused | refused | measured |
| PCF, Type 1 (`.pfb`), BMFont `.fnt`, image fonts | not tried | not tried | FreeType may read the first two |
| Lua bytecode from **LuaJIT** (`luajit -b`) | loads | loads | measured |
| Lua bytecode from standard Lua 5.x `luac` | refused | refused | measured (LÖVE runs LuaJIT) |
| `.love`, `.zip`, a folder (`love.filesystem.mount`) | documented | documented | not measured |
| Ogg Theora `.ogv` | documented | documented | `.mp4`, `.webm` are not played by LÖVE |

### How far to trust a refusal

The tracker samples in r3i6nos's corpus were made to *identify* formats. `nttplay probe` says of most
of them "it ends early" or "isn't a tracker module Neumetik knows" (`composer.669`, `multitracker.mtm`,
`octamed.med`, `ultratracker.ult`, `dsmi.amf`, `farandole.far`, `polytracker.ptm`, `screamtracker2.stm`),
so LÖVE's refusal of them says little. What is solid: the five formats that load on real files (MOD,
XM, IT, S3M and MIDI) and the formats LÖVE never lists. Fixing this needs full-length modules of each
format (roadmap).

## What the library marks now

The preview and `love_support` say, per file: "yes (checked)", "LÖVE 12.0 only" (QOI, WebP: a project on 11.5
is told it cannot use it), "no: LÖVE refuses GIF; convert it to PNG" (GIF, MP4, WebM, WOFF2, BDF, Opus,
AIFF, PCX, ICO, TIFF, PSD), "not confirmed" (the tracker formats above, PCF, Type 1, Lua `luac`) and "not
by LÖVE itself" (JSON, TMX, LDtk and the other data formats a library reads).

## Who can load it: every sister app checked (2026-11-13)

Searched in Diskette, Crunchy, Commander (r3i6nos), Neumetik and its tracker, Stylus, Coupler, Painter,
Composer, Animator, Floppy, Klamansi, Fontastik, TinyNTT, Cue and NoCoGaMa.

| LÖVE type | Sister app that loads or identifies it | Used in JoyrIDE |
|---|---|---|
| PNG, JPEG, GIF, BMP, TIFF, WebP | **Diskette** (`image` crate: jpeg png gif webp bmp tiff, for thumbnails); **Crunchy** (`image` with `png jpeg gif bmp ico tiff webp pnm tga qoi hdr exr dds ff`); **Commander** (its own C decoders: PNG, JPEG, GIF, BMP, ICO, TGA, QOI, PCX, PNM; WebP, TIFF and PSD through ffmpeg) | the `image` crate with Crunchy's HDR, EXR, QOI, DDS and TGA features added to Diskette's list |
| TGA, QOI | Crunchy, Commander (C decoders), **Stylus** (`IMAGE_EXTENSIONS` has `tga`, `qoi`) | yes |
| HDR, EXR | **Crunchy only** (it decodes them to compress) | yes, tone-mapped for the preview |
| DDS | Crunchy (the `dds` feature of `image`, which reads the simple cases) | header facts always; a picture when `image` decodes it |
| KTX, PVR, PKM, ASTC, and block-compressed DDS | **no sister app** (none mentions them) | header facts only: container, size, format, mip levels. **No picture: roadmap** |
| AVIF, HEIC | Crunchy (macOS ImageIO), Commander (a stub) | not decoded; LÖVE refuses them anyway |
| WAV, FLAC, MP3, Ogg, Opus, AIFF | Diskette, Commander (own decoders; ffmpeg-min for the rest) | the webview plays WAV, FLAC, MP3, Ogg; a header probe gives the facts |
| MOD, S3M, XM, IT, MIDI and about 80 more tracker formats | **Neumetik tracker** (83 formats into one model; `docs/tracker-formats.md`) and **Neumetik MIDI** (MIT); Commander shows a module's names | `nttplay` (vendored, plays the first 60 seconds). **Verified on the corpus: MOD, S3M, XM, IT, DBM, OKT, NSF, AHX, KT, ML, PTK, ST, V2M, MIDI and RMI play; the 669, MTM, MED and ULT stubs "end early"** |
| Music as text (ABC, MusicXML, MML, RTTTL, Alda, kern, ...) | Neumetik (`nttplay` reads 20 such formats: verified on the corpus) | `nttplay` reads them, but the preview opens a text file in the editor and has no Play for it: **roadmap** |
| TTF, OTF, WOFF, WOFF2, TTC | Diskette identifies them; Commander identifies WOFF, WOFF2, PSF and BDF | `asset-core/src/font.rs` reads their facts; a TTC shows facts and bytes (a browser cannot draw one) |
| Bitmap fonts: BDF, PSF, PCF, BMFont | Commander reads PSF and BDF text | not drawn; LÖVE refuses BDF. **Roadmap** |
| Ogg Theora `.ogv` | none: **Commander's ffmpeg-min has no Theora decoder** (checked) | `ogv` (libtheora, vendored): plays on a canvas |
| `.love`, `.zip` | Commander lists zip and `.love` members | `zip_members` in the preview |
| Lua bytecode | r3i6nos corpus has samples; Commander identifies "Lua bytecode" | named by its header: LuaJIT loads, Lua 5.x is refused |
| Shaders, `gamecontrollerdb.txt` | none (NoCoGaMa's design doc only sets an SDL variable) | tracked by extension and name |

## Not handled yet (all on the roadmap, section 5)

1. **Pictures of block-compressed textures** (DDS BC1 to BC7, KTX, PVR, PKM, ASTC): no sister app has a decoder.
2. **Real full-length samples** of the tracker formats LÖVE lists but the corpus has only as stubs, to settle both LÖVE's and Neumetik's support.
3. **A Play button for music written as text** (the editor opens the text; `nttplay` can play 20 notations).
4. **BMFont pairing**: a `.fnt` and the page images it names travel together; image fonts' glyph strings.
5. **PCF, Type 1, PSF and BDF** bitmap fonts: a sample drawn from their glyphs (Commander has C code for PSF and BDF).
6. **Telling a project it cannot use a file** (`LoveSupport::blocks`): the export gate and Run should warn when a project on 11.5 holds a `.qoi` or `.webp`, or any project holds a `.gif` it loads.
7. **EXR, DDS, KTX, PKM, PVR, ASTC loaded by a real LÖVE**: only their handlers were seen; make sample files and add them to the measurement script.
