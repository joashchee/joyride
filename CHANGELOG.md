# Changelog

## Unreleased

- **input is in Libraries; Light World and Slicy are not** (2026-11-14): `input` (keyboard, mouse and gamepad as one state, with held-down delay and interval, pressed and released per frame, key sequences and a history) is MIT, audited and read; a frame advances only when `push_state` runs, which `bind_callbacks` arranges by wrapping `love.draw`, so call it once in `love.load` after `love.draw` exists. **Slicy** is GPL-3.0 and **Light World** ships a GPL-2.0-or-later shader that its code loads with the library, so both are refused under the no-GPL rule and recorded in the exclusion list with the evidence; `lighter` and `Patchy` cover lights and 9-patches. In the Benchmark sample (57 libraries) `input`'s check passes on LÖVE 11.5 and 12.0 headless.

- **tactile, lighter and Patchy are in Libraries** (2026-11-14), the first three from the awesome-love2d check: `tactile` (map keys, mouse buttons, gamepad buttons and axes to named controls; button pairs read as an axis from -1 to 1), `lighter` (dynamic 2D lights and shadows from polygon occluders) and `Patchy` (9-patch images). All three are MIT by their README or file header (none ships a LICENSE file) and were audited with their sources read. Cautions: lighter needs a window to load, uses stencil calls whose behaviour on LÖVE 12.0 was not measured, and bundles three MIT libraries and an OLC-3 licence file that must travel with it; Patchy runs a 9-patch's metadata file as Lua, so use only files you ship. In the Benchmark sample (56 libraries): tactile's check passes on LÖVE 11.5 and 12.0, Patchy's API check passes, and lighter is skipped without a window. None of the three has been drawn in a window.

- **Checked the curated list against awesome-love2d** (2026-11-14): the list the current LÖVE jam pointed to has 260 libraries; 43 are already in Libraries, 217 are not, and 17 of ours are not on it. The full comparison is `docs/awesome-love2d-check.md` and a shortlist of where it fills a gap is in the roadmap; nothing on the list has been audited or added.

- **lua-ljsqlite3 and xsys are in Libraries** (2026-11-14): a second SQLite binding through LuaJIT's FFI, and the small helper library it requires. MIT, audited, ljsqlite3 pinned to `master` (its last tag predates a fix) and xsys to its `master`. Compared with sqlua it keeps text with NUL bytes, reads real BLOBs, runs several statements in one `exec`, errors cleanly on a closed connection, supports prepared statements and Lua functions callable from SQL, and stores integers exactly up to 2^63; it has not changed since 2018. Pitfalls measured and recorded: integers come back as 64-bit cdata (not a table key), `exec` returns results by column, `rowexec` raises unless exactly one row comes back, booleans cannot be bound. Add both to a project; require it as `lua-ljsqlite3` (the library lint says so if you write `ljsqlite3`). Needs the SQLite shared library beside the game (the system one on macOS) and is not for the web build. In the Benchmark sample (53 libraries) both pass their checks on LÖVE 11.5 and 12.0 on this Mac.

- **sqlua is in Libraries** (2026-11-14): SQLite 3 through LuaJIT's FFI, MIT, pinned to `v0.1-3` and audited (its whole source read and run). Positional and named parameters, rows as tables, a statement cache. It needs the SQLite shared library beside the game (macOS has one in the system; ship `sqlite3.dll` or `libsqlite3` for Windows, Linux, Android and iOS), and not in the web build. The audit measured its pitfalls and they are in the entry's caution: integers read back as 64-bit cdata (equal to the number, but not a table key), a real BLOB reads as nil, text is cut at the first NUL byte, booleans cannot be bound, only the first statement of an `execute` runs, and `close` twice is unsafe. In the Benchmark sample (51 libraries) it passes a real round trip on LÖVE 11.5 and 12.0 on this Mac; the headless reports are ingested.

- **windfield is excluded for good** (2026-11-14, the maintainer's decision): `joyride-core/data/library-exclusions.toml` lists libraries that are never curated; a test fails if one is in the list, and `curate_libraries` refuses a seed that names one by name or git address. windfield's own code is MIT (a header in its `init.lua`; the roadmap had wrongly said it had no licence) and it has had no commit since 2018, but the `mlib` it bundles has a GPL-2.0-or-later header and README beside a zlib LICENSE.md (upstream relicensed in 2015, after the copy windfield took), so it stays out under the no-GPL rule; the evidence is in the exclusion record. The remaining candidates were looked at (SQLite bindings, native file dialogs) and wait for the compiled-library work; see the roadmap.

- **serpent is in Libraries** (2026-11-14): the Lua serialiser and pretty printer, MIT, pinned to `0.30` and audited. Its only file is `src/serpent.lua`, so it is required as `libs.serpent.src.serpent` (the library lint tells you when `require "serpent"` cannot find it). `serpent.load` runs the saved text as Lua with no globals and no function calls unless you pass `safe = false`, which is for files you wrote yourself; the benchmark found it correct on every feature (cycles, infinities, exact numbers, both key types) but slow to write, 30 to 90 saves a second against thousands for bitser. In the Benchmark sample (50 libraries) it joins the serialising comparison, and comparison pages with more than four libraries now scroll sideways (the table is 48 columns wide), with "Libraries 2-5 of 5" shown in the frame.

- **Four more libraries, audited** (2026-11-14): `Concord` (entity component system; its repository moved to `Concord-Studio/Concord`, pinned to `v4.0`), `cron` (timer clocks), `i18n` (translations with plurals and fallbacks) and `cargo` (assets as tables), each pinned to an exact commit and read before anything ran. Cautions: **cargo does not work on LÖVE 12.0** (it calls `love.filesystem.isDirectory` and `exists`, removed there; measured on the real 12.0) and runs every Lua file in the folder it is given; i18n's `loadFile` cannot read inside a packaged .love. In the Benchmark sample (49 libraries): cron joins the timers comparison (fastest, 1.1M to 3.5M timers/s, no cancel, progress or script), and Concord, i18n and cargo have stand-alone checks; the headless runs on LÖVE 11.5 and 12.0 are ingested.

- **Five more libraries, audited** (2026-11-14): `cpml` (vectors, matrices, quaternions, intersections, easing, noise), `luaunit` (xUnit testing), `bitser` (compact binary serialising, LuaJIT), `talkies` (dialogue boxes) and `roomy` (scene manager), each pinned to an exact commit and read before anything ran. Cautions where the audit found something to know: bitser needs the FFI (not the web build); luaunit's repository has its own test runner and its `run` reads the program's command line (a game started with `--report` printed luaunit's usage; use `runSuiteByInstancesNoCmdLineParsing`); talkies' example folder holds a Creative Commons font; cpml's FFI use is optional and its LICENSE.md lists parts from other authors. The curator now recognises ISC and takes a `tag` in a seed for a release tag that is not a version number. In the Benchmark sample (45 libraries): cpml joins the vectors, roomy the states, bitser the serialising (best on both LÖVE versions, 4.6 and 4.8 of 5) and luaunit the testing comparisons; talkies has a stand-alone check that needs a window. The headless runs on LÖVE 11.5 and 12.0 are ingested; `macos-11.5.md` is the earlier run's tables and was not regenerated.

- **Delete moves to the Trash on the Mac; format on save** (2026-11-14): deleting a file or folder from the tree now asks "Move to the Trash?" and moves it into `~/.Trash` (never replacing something already there; a Trash on another volume falls back to a permanent delete and says so). Settings has a **Saving** choice, off by default, that runs a Lua file through StyLua when you save; a file with a syntax error is saved as written and the error shows in the status bar. Neither has been tried in a window. The PortMaster exporter check was re-run against PortMaster-New: it still matches.

- **New windowed Benchmark Libraries run ingested** (2026-11-14): macOS, LÖVE 11.5, window, all 40 libraries (`samples/benchmark-libraries/results/macos-11.5-window.txt` replaces the earlier windowed report): 424 pass, 435 comparison cells, 61 findings, no new hazards. Slab, cimgui-love, g3d and lua-discordRPC reported "did not load" with `ffi.cdef` redefine errors; that is the interactive Report loading libraries a second time in one process (the no-window runs load Slab and discordRPC fine), so they were ingested with `--skip` and keep their earlier results until a fresh run. Scores moved by under 0.1 (load times are run to run noise).

- **JoyrIDE integrates libraries into the code, and the editor shows exactly where** (2026-11-14): adding a library from Libraries now also adds one line to `main.lua` (`love.filesystem.setRequirePath(...)`, so `require "bump"` finds `libs/bump`), and removing it takes the line out again while it is still as JoyrIDE wrote it. Every line JoyrIDE inserts is recorded in `.joyride/inserted.json` and marked in the editor with a green bar in the left margin (hover for what it is); **touch the line and its bar goes**, and the line is yours from then on: never marked, re-added, rewritten or removed with the library. Nothing is written under unsaved edits (Libraries says so, and an **Integrate** button does it once `main.lua` is saved); a project with no `main.lua` gets one. C modules and libraries required by their path (`libs.g3d.g3d`) need no line. **Linting now knows the libraries**: a `require` of g3d, neumetik-midi or cimgui-love by a name that cannot find it, a library that does not load on the project's LÖVE (lurker on 12.0), calls that the benchmark measured to be wrong (HC's `raycast`, hump.class's missing type test, `set_state` arguments), and a near-miss of a name a library defines (`bump.neWorld`: did you mean `newWorld`?), read from the library's source in `libs/` and never run; and the language server gets the project's require path, so `require` of a library resolves for completion, hover and its own checks. Not yet seen in a window.

- **Add a bundled SoundFont to a project in one click** (2026-11-14): Assets… has a **SoundFonts** tab listing the three banks that ship with JoyrIDE. **Use in project…** first shows what adding would do and what to know (nothing is written yet), then **Add to project** copies the bank to `assets/soundfonts/`, its licence beside it, records it in the project's asset record with the licence confirmed (the BSD bank's credit line goes into `CREDITS.txt` on export), and shows the Lua that plays a song through it with neumetik-midi (with a Copy button). Warnings: no project open, not a LÖVE project (no `main.lua`), a different file already at that path (never replaced; the same file is fine), other SoundFonts already in the project (each is read into memory whole), no player library in the project, a web or mobile build set up (neumetik-midi cannot run there, so the bank would only add its size), BSD's keep-the-notice terms, PocketSongsGM's unverifiable samples, ProtoSquare's chip sound, and the bank missing from the build. Nothing is replaced and nothing is downloaded.

- **PocketSongsGM is the default MIDI sound** (2026-11-14): until you choose, MIDI previews play through PocketSongsGM (chosen by the maintainer after listening to all four sounds); the built-in synthesizer is one click away in Settings, and a choice already made is kept.

- **Choose the sound MIDI has** (2026-11-14): Settings has a **MIDI sound** choice: the built-in synthesizer (as before), or one of three General MIDI SoundFonts that ship with JoyrIDE: Sony Ericsson P800 (335 KB, BSD-3-Clause), PocketSongsGM (671 KB, CC0) and ProtoSquare (364 KB, CC0, an old sound chip's square waves). The MIDI preview plays through the chosen one (in-process with the vendored synthesizer, since `nttplay` takes no bank); the choice is kept in the app-data folder as an id from the list, never a path. `joyride_core::soundfonts` holds the list, sizes, checksums and licences, a test checks every file against its recorded checksum and licence file, and another plays notes through each bank. Select a MIDI file again to hear a change.

- **A General MIDI SoundFont ships with the app** (2026-11-14, `src-tauri/resources/soundfonts/`): Fontastik's catalog (public read) was searched for the smallest General MIDI bank with a permissive licence: of 1,240 banks between 0.2 and 12 MB, 81 are complete GM sets, and the catalog's own licence field is not trustworthy (it says "Public Domain" for Nokia, Qualcomm and Beatnik banks), so each file's embedded copyright and comment were read. Chosen: **Sony Ericsson P800**, 334,920 bytes, 128 melodic programs and a drum kit, New BSD from Beatnik's miniBAE (attribution kept in `LICENSE.txt` and `NOTICE`). Smaller GM banks exist (0.21 to 0.30 MB) but say nothing permissive; the next permissive ones are ProtoSquare (0.36 MB, CC0, a chiptune sound) and PocketSongsGM (0.67 MB, CC0, no provenance beyond its own text). A test loads it with the vendored synthesizer and plays notes through it. Not used by anything yet: the MIDI preview does not take a bank.

- **neumetik-midi is in Libraries** (2026-11-14): ansiapps' own General MIDI and GS synthesizer with no SoundFont needed (it synthesizes every instrument; it plays a SoundFont or DLS bank too if you load one: checked with GeneralUser GS, 30 MB, in 13 ms). A LuaJIT FFI library for LÖVE with a queueable-source binding: loops, fades, seeking, muted channels, live notes. Pinned to the public repository's 0.3.0 sync (`e275a35`), audited (rule 14), and in the Benchmark Libraries sample with a built-from-source universal macOS library (`tools/build-binaries.sh`) and a check that synthesizes a one-note song. Desktop, Android and PortMaster only: the web has no FFI, and iOS needs the library embedded. Its Benchmark line is in the panel.

- **Vendored Neumetik refreshed** (2026-11-14, `scripts/vendor-neumetik.sh`): the MIDI synthesizer now has XG master tune and transpose and the drum-pattern readers' fixes; `synth` gains the NSF, MML and MGS updates and a 68000 interpreter used only to test the MDSDRV player; `nttplay` gets feature switches for BGN scenes and Visual. Still standard library only; `nttplay` builds offline and probes and decodes as before (the app's 30 tests pass).

- **Opening a project finds the libraries it uses** (2026-11-14): JoyrIDE reads the project's files (nothing is run, nothing fetched) for libraries: `love.toml` dependencies, folders and files under `libs/`, `vendor/`, `third_party/` and the like, licensed folders and files anywhere (your own `lib/core.lua` is not one), and its `require`s. Each is matched against Libraries by name, module, repository name and git address; the ones Libraries lacks are offered in a **Libraries found in this project** window with what was read (version, licence, source address, where, whether anything requires it), and **Add to Libraries** puts them in your own list (the app-data folder), marked *yours*, *not audited, not measured*, with **Forget** to take one out. **Never for this project** remembers the choice; **Find in this project** in the Libraries panel looks again. Modules required but provided by nothing in the project are listed, never offered. Not yet seen in a window.

- **Benchmark results now feed the Libraries panel** (2026-11-14): `scripts/ingest-benchmark.py` reads Benchmark Libraries reports into `joyride-core/data/library-benchmarks.json` (each library's load result per LÖVE version, its score and place in every comparison, its findings), and the panel shows a **Benchmark** line under each library; a library that fails to load on a LÖVE version (lurker on 12.0) is now refused there. New cautions: g3d (require it as `libs.g3d.g3d`), love-loader (its file must sit at the game's root), tween (nested tweens lose sibling fields), HC (`raycast` misses rays heading right or down). Rule 15 in `CLAUDE.md`: every new report is ingested in the same commit.

- **Benchmark Libraries sample** (2026-11-14, `samples/benchmark-libraries/`): loads all 39 curated libraries (vendored in `libs/` at their pinned commits, so it runs offline), compares the ones that overlap in 17 tables (classes, tweening, timers, events, states, vectors, collections, maths, strings, serialising, printing, collision, pathfinding, cameras, assertions, profiling, GUI) of features, speed and cost, rates every cell out of 5, marks the best in each row and overall, and lets you rate any cell yourself (kept in the save folder). One check each for the libraries that stand alone. Same text-mode screens, platform detection and report grammar as the IO Checker; `love . --report` writes a text report and Markdown tables. First run on macOS found real library defects (tween.lua drops nested siblings, HC's raycast misses rays heading right or down, lurker does not load on LÖVE 12.0, and more): `README.md` there. Windowed comparisons not yet seen on a screen. `tools/build-binaries.sh` builds the compiled libraries from source for macOS (love-nuklear, cimgui, discord-rpc, universal, into a git-ignored `bin/`): nuklear and discord-rpc then load and run on LÖVE 11.5 and 12.0; luasteam is never built (it needs Valve's Steamworks SDK).

- **Fourteen more libraries, all audited** (2026-11-14): `tiny-ecs` (entity-component-system), `Slab` and `suit` (immediate-mode UI), `gamera` (camera), `jumper` (pathfinding), `g3d` (simple 3D), `batteries` (toolkit), `sock` (networking over LÖVE's bundled enet), `lovebird` (browser debug console), `profile` (profiler), `lust` (tests), `peachy` (Aseprite animations), `love-loader` (threaded asset loading), `ripple` (audio tags) and `moonshine` (shader effects). Each is pinned to an exact commit and was required on real LÖVE 11.5 and 12.0; every older entry was probed too. Before anything is probed or committed, a library is now scanned at its pinned commit for secrets, binaries, process and network use, hidden code and foreign licences, and a test refuses a pinned commit with no reviewed audit. The panel shows a **Caution** line where the audit found something to know: `lovebird` opens a code-running web console (development only), `luasteam`'s repository carries Valve's Steamworks SDK under Valve's own agreement, and `Slab` and `sti` use the FFI.

- **C libraries in Libraries** (2026-11-14): six that LÖVE games use through LuaJIT's FFI or as compiled modules, each pinned to a commit with its licence: `nativefs` (files outside LÖVE's folders), `lua-discordRPC` (Discord Rich Presence), `luajit-request` (HTTP and HTTPS through libcurl), `luasteam` (the Steamworks API; Balatro and SNKRX use it) `love-nuklear` (an immediate-mode GUI) and `cimgui-love` (Dear ImGui, from Codeberg: the curated list now takes Codeberg as well as GitHub). The panel shows each one's kind and the binary it still needs next to the game; adding installs the source only, and no "loads" result is claimed for them. Desktop only (the web build has no FFI).

- **IO Checker: MIDI check measures the sound** (2026-11-14): the `timidity.cfg` line on the console is libmodplug finding no GUS patch set; the MIDI file still decodes and is audible (plain built-in tones, measured on macOS). The check now reports `INFO … SILENT` instead of `PASS` if a platform decodes it to silence, and the README explains the console line.

## 0.2.0 (2026-11-14)

- **Choose your coding font** (2026-11-14): Settings has an *Editor font* with 13 fonts that ship with the app: JetBrains Mono, Source Code Pro, Hack, IBM Plex Mono, Inconsolata, Fira Code, Intel One Mono, DejaVu Sans Mono and Spleen (a pixel font in five sizes up to 32 by 64 cells), besides the default VGA. Each is offered only at sizes where its characters are whole screen pixels wide (small ones for a laptop, large for a big monitor), and **the window snaps** after a resize to a size that shows whole characters and whole lines in that font. Seen measuring in Chrome; not yet in the desktop window.
- **IO Checker sample project and a Samples panel** (2026-11-14): `Samples…` in the toolbar copies a project that ships with the app to your app-data folder and opens it (an existing copy is kept with your edits; Fresh moves it aside). The first is **IO Checker** (`samples/io-checker/`): it detects whether it runs on macOS, Windows, Linux, PortMaster, iOS, iPadOS, Android or the web (or is a target's build simulated on a desktop), lets you pick another to check as, checks that platform's input, screen, sound and storage against what it expects, loads one of every kind of asset LÖVE supports, and writes a device report; `love . --report` does the checks that need nobody, `love . --selftest` drives every screen without a GPU. Its 47 assets (including an OpenType font, Source Code Pro) have confirmed licences in `.joyride/assets.toml`; two fonts need a credit. Not yet seen in a window or on a device.
- **Exports check asset licences** (2026-11-14): the Export panel's `.love` and PortMaster builds now stop when a library asset the project uses has an unconfirmed licence, rights that have lapsed, a share-alike licence, or (with "This game is sold" ticked) a licence that forbids sale, and add a `CREDITS.txt` for assets that need a credit line.
- **Asset library panel** (2026-11-14): `Assets…` in the toolbar. Browse and search the library (name, tag, licence, author; filter by kind, confirmed licences, can be sold) with thumbnails, the licence line and where each asset is used; **Use** copies a confirmed asset into the open project (never over a file, never outside the project, checked against its recorded hash); **Confirm…** records a person's name against an unconfirmed licence. **Import from this project** scans the project, shows each distinct file once with the licence guessed from its licence files, and copies nothing until you choose a licence for each and give your name; originals are never changed.
- **PortMaster's LÖVE ports are in the games catalogue** (2026-11-14): 68 handheld ports with their description, genres, free or paid, the LÖVE version and the PortMaster download, merged with the same game from other places.
- **Games and apps panel** (2026-11-14): `Games…` in the toolbar opens the LÖVE games and apps catalogue: search, filter by kind, platform and open source, see the licence and latest version, and open a page or download in your browser. It is empty until the catalogue is filled.
- **LÖVE games and apps catalogue, first part** (2026-11-14): a second library for what has been made with LÖVE: where to download, licence, latest version. Entries from several places are de-duplicated and the newest version kept; same-name games by different authors stay apart. The data model, search and GitHub reader are written and tested; the catalogue is empty until the fetcher is run with network access, and there is no panel yet. `docs/love-games-library.md`.
- **Insert from the Reference** (2026-11-14): the LÖVE reference panel has an Insert button that puts a function's snippet at the cursor, with Tab moving between the placeholders.
- **Compressed textures and EXR checked on the real LÖVE** (2026-11-13): DDS, KTX, PKM, PVR, ASTC and scanline EXR files load on LÖVE 11.5 and 12.0; a tiled EXR crashes both, so the preview warns about it.
- **Bitmap fonts are drawn** (2026-11-13): BDF, PSF and BMFont (`.fnt` with its page pictures) show a sample in the preview, with the font's name, glyph count and cell size; PCF and Type 1 are named.
- **Music written as text plays** (2026-11-13): clicking an ABC, MusicXML, RTTTL, Alda, kern or similar file plays it in the preview, with an Edit button to open it in the editor.
- **Warnings for files LÖVE cannot use** (2026-11-13): a file the project's LÖVE version refuses (a GIF, MP4, WebM, WOFF2, BDF; a QOI or WebP on 11.5) is marked `!` in the file tree, printed as a warning in the Console when you Run, and listed in the Export panel. Nothing is blocked.
- **Compressed textures are drawn** (2026-11-13): DDS, KTX, PKM and PVR files in the BC1 to BC5 (DXT) and ETC1 formats now show as a picture in the preview, decoded by our own code; other texture formats still show their facts and bytes.
- **Everything LÖVE loads is tracked and previewed** (2026-11-13): the library now tracks HDR, EXR, QOI, compressed-texture files, all the tracker formats LÖVE lists, TrueType Collections, Lua bytecode, `.love` and `.zip` archives and `gamecontrollerdb.txt`. Every previewed file says whether LÖVE loads it, measured on the real LÖVE 11.5 and 12.0 (QOI and WebP need 12.0; GIF, MP4, WebM, WOFF2 and BDF are refused). HDR, EXR, QOI, TGA and DDS pictures show; KTX, PVR, PKM and ASTC show their header facts. `docs/love-asset-types.md` also records which sister app can load each type.
- **Add Font** (2026-11-13): a toolbar panel that adds a font to the project after you confirm its licence by name. It shows the font's own details and a sample, guesses the licence from the font's note or a licence file beside it, never replaces a file, and records the font and its licence for the project's credits. Font previews show the same details. `docs/love-asset-types.md` lists, from measurements on the real LÖVE, what LÖVE loads that the library does not yet track.
- **File preview** (2026-11-13): clicking a file that is not text shows it instead of an error: pictures (now also WebP and TIFF), sound, MIDI and tracker music, Theora video, MP4 and WebM, fonts, `.love` and zip members, and a hex dump with the facts for anything else. Chosen by the file's bytes, never edited or written.
- **`ogv` crate** (2026-11-13): probes and decodes Ogg Theora video to RGBA frames over the vendored libogg and libtheora, for the video preview.
- **libogg and libtheora vendored** (2026-11-13): `vendor/xiph/`, BSD-3-Clause, the portable C decoder only, pinned by SHA-256, for previewing Ogg Theora video (`.ogv`).
- **Neumetik's players vendored** (2026-11-13): `vendor/neumetik/` (`midi`, `bgn`, `synth`, `visual`, `tracker`, `nttplay`), MIT, for previewing MIDI, tracker modules and music written as text. `nttplay` builds offline and plays a one-note MIDI.
- **Viewport simulator** (2026-11-13): a `Screen:` button in the toolbar runs the game with a target's overlay in a simulated screen (PortMaster 4:3, 1:1, 16:9; iPhone, iPad, Android). The project is staged to a scratch copy; your source is not changed.
- **Text-mode UI only** (2026-11-13): the whole interface follows the ANSIapps theme
  (IBM VGA 8x16 on an 8 by 16 grid, blue desktop, light-gray menu bar, double-line frames,
  flat one-row buttons, a black DOS console); there is no graphical theme and no switch for
  one. The appearance options are colours only, in Settings: the palette, **True colour** (default)
  or **16-colour (DOS mode)**, and in True colour a **theme swatch**: Classic blue, Dark,
  Light, Solarized dark and light, Amber and Green phosphor (each contrast-checked by
  `scripts/check-swatches.ts`). The editor has the block cursor and the VGA font in all.
  `docs/text-mode-ui.md`.

- **GitHub via `gh`** (2026-11-13): a GitHub panel (status, commit, push, pull, create repo,
  release workflow) over the `gh` and `git` command lines; no token of ours.
- **Auto-Port panel** (2026-11-13): findings per target, generate or refresh the overlay,
  open its `patch.toml`, `.gptk` and shim.
- **11.5 variant list** (2026-11-13): the Export panel shows each 12.0-only function a
  game uses and lets you accept the unsupported ones, remembered in `.joyride/variant.toml`.
- **Colour swatches** (2026-11-13): `love.graphics.setColor(0.2, 0.4, 1)` shows a swatch
  and a picker that rewrites the numbers.
- **LÖVE lints in the editor** (2026-11-13): 0-1 colours, boolean `vsync`, 12.0-only and
  removed APIs, shown as markers for the project's LÖVE version.
- **Format Document** (2026-11-13): the bundled StyLua through `joyride-core::format`;
  ⇧⌥F or the context menu; a project's `stylua.toml` is honoured.
- **LÖVE reference** (2026-11-13): searchable offline docs (⌘⇧D) built from the LÖVE types
  bundled with the language server (`Docs::from_luacats`).
- **Library manager** (2026-11-13): add or remove curated libraries from a Libraries
  panel; installs at the pinned commit and writes `love.toml` and `love.lock`.
- **Export panel** (2026-11-13): `.love` and PortMaster zip (with the 11.5 variant of a
  12.0 project) into `<project>/build/`; `pipeline::build_plain_love`.
- **Run uses the project's LÖVE version** (2026-11-13): from `love.toml`; an installed
  LÖVE is matched by the version in its own `Info.plist`.
- **App icon** (2026-11-13): original joystick art drawn by `scripts/build-icons.ts`;
  `.icns` and `.ico` made with `tauri icon`.
- **Restore the last project and confirm on close** (2026-11-13): the last folder
  reopens after the splash; closing with unsaved files asks first.
- **Find in files** (2026-11-13): `files::search` and a sidebar panel (⌘⇧F); plain
  text, capped, never reads through links.
- **New file, new folder, rename, delete** (2026-11-13): `files::create_file`,
  `create_dir`, `rename`, `delete` (confined to the project, never replace, refuse
  links out), Tauri commands that return the fresh listing, tree buttons and an
  inline name box. Delete is permanent after a confirm.
- **Language server** (2026-11-13): `joyride-core::lsp` (Content-Length framing,
  finding the bundled `lua-language-server`, a child process with a message
  callback), Tauri commands `lsp_start`/`lsp_send`/`lsp_stop`, and `src/lib/lsp.ts`,
  a small client feeding Monaco diagnostics, completion, hover, signature help and
  go-to-definition. Settings (LuaJIT, `love` global) are answered over the protocol,
  so the project gets no `.luarc.json` of ours; logs and metadata go in the app-data
  folder. Tested against the real bundled server. The LÖVE 11.5 types that ship inside it
  (`meta/3rd/love2d`) are the workspace library, so `love.*` completes and hovers.
- **Bundled tools** (2026-11-13): `scripts/fetch-toolchains.py` pins LÖVE 11.5,
  lua-language-server and StyLua by SHA-256; Run uses the bundled LÖVE. PortMaster
  keeps using its own runtimes (rule recorded in CLAUDE.md); `docs/platform-parity.md`.
- **Run and console** (2026-11-13): stoppable `runner::start`, `find_love`, Run/Stop
  buttons (⌘R, ⌘.), a console whose error lines open the file at the line.
- **Open a project** (2026-11-13): `joyride-core::files`, Tauri commands, file tree,
  tabs and save. File access is confined to the project folder, symlinks included.
- **Monaco editor** (2026-11-13): bundled `monaco-editor` 0.57, Lua only, own
  worker, lazy-loaded after the splash. No network use.
- **App shell scaffold** (2026-11-13): Tauri 2 + React 19 + Vite 8 + TypeScript.
  Static splash in `index.html` that paints first, a React `Splash` with
  progress and load errors, `src-tauri` over `joyride-core` and `asset-core`
  (`core_info` command), bundle id `com.ansiapps.joyride`. Placeholder icons.
- **Web demo tooling** (2026-11-13): `joyride-core/examples/web_demo.rs` and
  `scripts/serve-web.py` to try the love.js templates in a real browser.
- **Web exporter** (2026-11-13): `web.rs` and `pipeline::build_web` package a
  love.js page without Node, by reproducing the few steps of love.js's
  `index.js` (data file, filled-in `game.js` and `index.html`, runtime files,
  `_headers` for the standard version). Reproducible output; a 12.0 project
  uses its 11.5 variant. Saved games persist (love.js mounts IndexedDB
  itself and syncs on `beforeunload`); a first note here saying they did not
  was wrong and is corrected.
- **love.js fork survey** (2026-11-13): `docs/lovejs-forks.md`,
  `scripts/check-lovejs-forks.py` and a weekly workflow. Davidobot/love.js has
  had no commit since 2024-05-13; `pkhead/love.js` `lua52` (Oct 2026: Lua 5.2,
  Emscripten 6.0.1, WASM exceptions) is the most current engine. The exporter
  now packages its template layout too; Davidobot's stays the default.
- **iOS and iPadOS exporter** (2026-11-13): `apple_mobile.rs` and
  `pipeline::build_ios` stage an Xcode project around the mobile `.love`
  (or its 11.5 variant), written against the real `love` repository's
  project (`main` and `11.x`): the game in the `love-ios` target's Resources
  phase, the app's identity on its three build configurations only,
  orientation and full-screen in the plist, one-size icon. `xcodebuild -list`
  accepts the edited projects. The build itself needs LÖVE's iOS dependencies
  and is not run here.
- **Android exporter** (2026-11-13): `android.rs` and `pipeline::build_android`
  stage a `love-android` Gradle project around the mobile `.love` (or its
  11.5 variant), written against the real template's `12.x` and `11.x`
  branches. Gradle itself is not run (needs the Android SDK and executes the
  template's build scripts).
- **Third-party licence file and scan** (2026-11-13):
  `scripts/third-party-licenses.py` writes `docs/third-party-licenses.txt`
  (every Rust crate in `asset-core` and `joyride-core`, the vendored engines'
  notices, `NOTICE`; identical texts once, a crate without a licence file
  listed as declared only); `scripts/license-scan.py` now scans the real
  crates and passes; both run in the Tests workflow.
- **Asset previews and image facets** (2026-11-13): thumbnails (pixel art
  stays crisp), palette, size, alpha and a pixel-art guess as tags, and WAV
  waveforms, stored in the catalog on import (`asset-core/src/preview.rs`).
- **Curated libraries** (2026-11-13): `library.rs` and a generated
  `data/libraries.toml` of 18 LÖVE libraries pinned to exact commits with
  their licences read from the repositories (all MIT), search, per-version
  load badges and add-to-`love.toml`. The generator reads tags, commits and
  licence files only; loading each library on real LÖVE (`--probe`) runs
  their code, so it is opt-in. The licence guesser now finds
  `MIT-LICENSE.txt`-style names and ranks licence files above readmes.
- **The 11.5 variant of a 12.0 game, for every export** (2026-11-13):
  `compat.rs` classifies each 12.0-only API a project uses as exact,
  reduced or unsupported, and `pipeline::Compat` builds an 11.5 variant for
  any target (PortMaster included, which has no 12.0 runtime): conf pinned to
  11.5, a generated `_jr_compat.lua` with only the needed functions, and a
  hard stop listing every unsupported use by file and line unless accepted.
  `love.filesystem.openFile`, `love.event.restart`, `love.sensor.*` and
  `love.markDeprecated` behave identically to 12.0 (error messages and error
  positions included) on the real 11.5 and 12.0 binaries. The project is
  never modified.
- **PortMaster exporter corrected against PortMaster-New** (`40186ea1`,
  2026-11-13): `scripts/check-portmaster.py` now clones the real repo (shallow,
  blob-less, scripts and `port.json` only) and compares the exporter's actual
  output with its 68 LÖVE ports. It found: LÖVE ports use `$GPTOKEYB
  "$LOVE_GPTK"` (not `$GPTOKEYB2 "love"`), `port.json` is version 4 with
  `porter` as a list, `desc_md`, `inst_md`, `store`, `availability`, `arch`,
  `min_glibc` and no runtime attr, and **no `love_12.0` runtime exists**, so the
  exporter and pipeline refuse a 12.0 project for PortMaster. Docker
  (Colima) check passes on Debian 12 aarch64.
- **Real LÖVE 11.5 and 12.0 and PortMaster checks** (2026-11-13): staged
  builds run headless on r3i6nos's real LÖVE builds (wrapper chain, conf
  patch, version-mismatch warning measured); lint tables for removed and
  new APIs measured from the binaries (`scripts/love-api-diff.py`) and
  guarded by a test; the shim is safe in a headless run; the PortMaster
  launch script runs against a stub PortMaster in a test;
  `scripts/check-portmaster.py` and a weekly workflow compare the exporter
  with PortMaster's repository (not run: no GitHub from the sandbox);
  `scripts/check-linux.sh` for Docker, Debian 12 and arm64 (not run: no
  Docker from the sandbox).
- **`joyride-core`, the Tauri-free core** (2026-11-13), 66 tests: `love.toml`
  and `love.lock` with a git installer; LÖVE version rules, `.luarc.json` and
  a signed (Ed25519) toolchain manifest; a Lua lexer, the auto-porter's
  scanner and version-calibrated lints; the runner and stack-trace linking;
  generated shims for PortMaster (scaler, right-stick cursor, Select+Start),
  mobile (scaler, touch pad) and web (stubs), tested under LuaJIT with a
  mocked `love`; zero-mutation patch overlays with staging, conf patching
  and unified diffs; `.love`, Windows, Linux, macOS and PortMaster exporters;
  one-call builds that check asset licences and add `CREDITS.txt`; editor
  provider data (colour swatches, asset hovers) and the offline API search.
  Not run on a display or a handheld (see ROADMAP).
- **Asset Library, project side** (2026-11-13): `.joyride/assets.toml`,
  `CREDITS.txt`, the export gate, and recording generated assets with their
  recipe and seed. Audio assets after Neumetik's catalog: header probe,
  levels, game-audio file names to tags, tag lint, BPM and length filters,
  rights and expiry alerts.
- **Tests workflow** (`.github/workflows/test.yml`) for both crates and the
  vendored engines.
- **`asset-core`, the Asset Library's core** (2026-11-13), clean-room from
  Diskette's and Crunchy's designs (their docs and measured findings, not
  their code): catalog with full-text search, licence model with guessing
  from `LICENSE`/`CREDITS`/sidecar files, staged duplicate finding, a hash
  cache keyed on size and mtime, and a two-step import (plan, then apply
  only what a person confirmed) that stores identical content once and
  reports licence conflicts. 24 tests.
- **Painter, Composer and Animator vendored under MIT** (2026-11-13), in
  `src/vendor/art-sprites`, `music-composer` and `sprite-animator`, by
  `scripts/vendor-sync.sh` (runtime code, tests and third-party licences;
  not the R36S ports or tools). Their tests pass here. Chipbank is not
  included.
- **Asset Library planned** (2026-11-13): `docs/asset-library.md` (what to take from Diskette, Crunchy, Neumetik, Stylus and the Painter/Composer/Animator engines, and the licensing decision) and ROADMAP section 5.

- **Repository set up** (2026-11-13): MIT licence, README, CLAUDE.md,
  ROADMAP.md, contributing and security notes, issue and pull request
  templates, Linux and Windows build workflows, and the release, publish
  and licence-scan scripts, taken from Stylus and Coupler. The
  implementation spec is `docs/JoyrIDE_LÖVE-Implementation-Spec.md`.
  No app code yet.
