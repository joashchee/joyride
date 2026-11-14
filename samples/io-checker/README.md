# IO Checker

A sample project that ships with JoyrIDE: it works out which of JoyrIDE's platforms it is running on, checks that platform's input, screen, sound and storage, loads every kind of asset LÖVE supports, and writes a device report. Open it from **Samples…** in the toolbar (it is copied to a folder you own first), press **Run**, and try **Screen:** to see it as a handheld or a phone.

It is the LÖVE counterpart of r3i6nos's IO Checker (`r3i6nos/docs/IO-CHECKER.md`), rebuilt around JoyrIDE's targets instead of one handheld, and it uses the same report grammar (`PASS`, `FAIL`, `INFO`, `SKIP`).

## What it does

**It detects the platform** (`lib/platform.lua`). LÖVE only says `OS X`, `Windows`, `Linux`, `Android`, `iOS` or `Web`; the rest comes from evidence, all listed on the Platform page:

| Platform | How it is told |
|---|---|
| macOS, Windows | `love.system.getOS()` |
| Linux | `getOS()` is `Linux` and none of PortMaster's signals are there |
| PortMaster | `getOS()` is `Linux` and the launch script's variables (`CFW_NAME`, `DEVICE_NAME`, `GPTOKEYB`, `controlfolder`) or a `/ports/` game folder say so: two signals is certain, one is likely |
| iOS, iPadOS | `getOS()` is `iOS`; an iPad has 600 points or more on its short side (iPhones top out at 440, the smallest iPad is 744): marked *likely* |
| Android | `getOS()` is `Android` |
| Web | `getOS()` is `Web`, or there is no LuaJIT |
| a build on the wrong machine | JoyrIDE's auto-port shim (`_jr_shim`) says which build this is when the OS cannot, e.g. a simulated handheld screen on a Mac: shown as *simulated* |

**You can choose the platform** with left and right on the Platform page. Every check then judges against the chosen platform's expectations, so a Mac can be checked as a phone and report what is missing (a touch screen, a safe area, a vibration motor). A report says when the platform was chosen rather than detected.

**It checks that platform's I/O.** Each platform has an expectation per capability: `need` (a missing one is a FAIL), `want` (a missing one is only noted) or `none` (not normally there; present is information). `lib/platform.lua` holds all eight profiles.

| Page | What it checks |
|---|---|
| Keyboard and text | every key, modifiers, `love.textinput` (and the on-screen keyboard on a phone) |
| Mouse and touch | position, buttons, wheel, fingers at once, taps, pressure; a touch is not counted as a click |
| Game controllers | every pad: buttons pressed and never pressed, chatter, each axis's range and rest position, the hat, rumble |
| Sensors and vibration | `love.sensor` on 12.0 (accelerometer, gyroscope), the accelerometer-as-joystick on 11.5 phones, `love.system.vibrate` |
| Screen | ten full-window patterns (colour bars, ramps, a checker and grid with edge and corner marks, a circle and the safe area, a motion bar that **measures the frame rate**), window, scale, safe area, orientation, renderer, limits |
| Sound | six generated test tones (left, right, both, low, a sweep, quiet): play one (Enter, A or the Play button), then say **Heard** (Y, pad X, or the button) or **Silent** (N, pad Y, or the button); the output, the volume |
| Storage | write, read, append, list and delete in the save folder; 4 MB written and read back byte for byte; a run counter that proves saves survive a restart; plain `io` where the platform allows it |
| System | LÖVE, LuaJIT, processors, memory, power, locale, identity, source and save folders |
| Assets | every type below, loaded as a game would, judged against what this platform and LÖVE version are expected to do |
| Device report | everything above in one text file, in the save folder, copyable with X |

The input pages record everything and navigate nothing. Leave one by holding **Escape**, **Select+Start**, or the bar at the bottom for a second, or wait 15 seconds with no input. A held Escape leaves once and cannot run on into quitting; quitting from the list takes Back twice.

Everything is the same few actions (up, down, left, right, accept, back, page up, page down, X, Y) whether it comes from a keyboard, a pad, a mouse or a finger, so the same screens work on a laptop, a handheld, a phone and a web page. The screen is a 48 by 25 grid of 8 by 16 cells in the VGA colours, drawn with the IBM VGA 8x16 font from `assets/fonts/vga.woff`, scaled to the window.

## The assets: every type LÖVE supports

This is what exercises JoyrIDE's **Asset Library**. Every file under `assets/` has a recorded licence in `.joyride/assets.toml` (written by the library's own catalog, each confirmed), a kind the library classifies, and a check that loads it.

| Type | Files | Notes |
|---|---|---|
| Image | `png`, `jpg`, `bmp`, `tga`, `hdr`, `exr`; `webp` and `qoi` | `webp` and `qoi` are **LÖVE 12.0 only**: on 11.5 the check expects a refusal and passes when it gets one |
| Spritesheet, tileset | `hero-walk.png` (animated on the Assets page), `tiles.png` | Painter and Animator output |
| Compressed texture | `dds` (BC1, BC3, BC7), `ktx` (ETC2), `pkm` (ETC1), `pvr` (PVRTC), `astc` | one block each, built by hand; whether a GPU can draw each is a separate answer, shown when you open one |
| Sound | `wav`, `ogg`, `mp3`, `flac` | Composer's jingle, encoded four ways |
| Music | `mid` (MIDI), `mod`, `xm`, `it`, `s3m` | MIDI needs a GUS patch set (`timidity.cfg`); without one LÖVE prints `load_pat > can not open …/timidity.cfg` on the console and plays plain built-in tones. The check measures the decoded audio and reports `INFO … SILENT` if a platform gives none |
| Font | `woff` (the UI font), `otf` (Source Code Pro, CFF outlines), `ttf`, `ttc`, BMFont (`fnt` and its page picture), an image font (a 3x5 hex font drawn for this project) | |
| Video | `ogv` (Ogg Theora, no sound) | the only video LÖVE plays |
| Shader | `glsl` | vertex and pixel stage in one file; applied to the hero on the Assets page |
| Script | `lua` (a JSON reader), `luac` (LuaJIT bytecode) | bytecode is tied to the LuaJIT build that wrote it: *not confirmed* on a platform whose LuaJIT may differ |
| Data | `json`, `csv`, `toml`, `tmx`, `tsx`, `ldtk`, plain text, an Animator clip | |
| Gamepad mappings | `gamecontrollerdb.txt` | loaded with `love.joystick.loadGamepadMappings` |
| Archive | `zip` | mounted with `love.filesystem.mount`, read, unmounted |

**Not here, and why:** the tracker formats LÖVE lists but real files of which are not to be had (669, AMF, DBM, FAR, MTM, MED, OKT, PTM, STM, ULT; see `docs/love-asset-types.md`); and GIF, Opus and the other formats LÖVE refuses.

`assets.tsv` lists each file's kind, licence, author, source and recipe. Most are **CC0-1.0**, made by JoyrIDE's own seeded generators (`tools/make-assets.mjs` runs Painter, Composer and Animator, then ffmpeg, lame, flac and ImageMagick for the other containers). The tracker modules, the Theora clip and the three tiny test fonts are **MIT**, copied from the r3i6nos corpus. Source Code Pro (`.otf`, Adobe, **OFL-1.1**, unmodified; `LICENSE-source-code-pro.txt`) is the second asset that needs a credit. The VGA font (`vga.woff`, unmodified, as JoyrIDE ships it) and the BMFont page (Painter's glyph atlas of it, copied byte for byte) are **CC BY-SA 4.0** (VileR; licence text in `LICENSE-vga-font.txt`): the one asset that needs credit and is share-alike, so the Asset Library's export gate and `CREDITS.txt` have something real to work on.

## Run it

- In JoyrIDE: **Samples… > IO Checker**, then **Run**. The **Screen:** button runs it as a PortMaster handheld (640x480, 720x720, 1280x720), an iPhone, an iPad or an Android phone.
- With a LÖVE on the command line: `love samples/io-checker`.
- Without a person or a window: `love samples/io-checker --report` runs the checks that need nobody, prints the report, saves it as `io-checker-report-<platform>-<LÖVE version>.txt` in LÖVE's save folder for this project (on a Mac, `~/Library/Application Support/LOVE/joyride-io-checker/`; the path is printed on stderr as `Report file:`), and exits with 1 if anything failed. This is how to check a handheld over SSH or a build in CI. Checks that need a window or a human are `SKIP`.
- Without a GPU: `love samples/io-checker --selftest` drives every screen, every asset viewer and the report against a mock of LÖVE's drawing and a fake controller, and exits 1 on any failure.

Auto-Port will flag two things in this project, on purpose: `love.thread.newThread` and `io.popen` are *Blockers* for the web. Both are probes inside `pcall` that exist to find out whether the platform has them; the web shim makes them fail and the web profile expects exactly that.

## Tested, and not

Tested here (`joyride-core/tests/io_checker_sample.rs`, `tests/unit.lua`, `tests/selftest.lua`): platform detection in every case, the judging, the input record, the report; every asset loading on the real LÖVE **11.5 and 12.0** headless (and WebP and QOI refused on 11.5, loaded on 12.0); the project rebuilt by JoyrIDE's pipeline for PortMaster, mobile and web and recognised under each shim; every screen and every asset viewer driven against mocked drawing; the licence record matching every file's hash; the project lint-clean at 11.5 and 12.0.

**Not tested** (needs a display, a speaker, a person or a device): real drawing on a GPU, so what the screen *looks* like, the patterns on a panel, the measured frame rate; the shader compile; video playback; sound out of a speaker; a real controller, touch screen, accelerometer or vibration motor; the on-screen keyboard; the clipboard; saves surviving a restart on a phone or in a browser; the iPad-versus-iPhone guess on a real device; PortMaster's variables on a real handheld (the names come from its launch script and are unconfirmed on hardware); LÖVE 12.0's storage checks on this machine's sandbox (macOS gives 12.0 a save folder the sandbox forbids; 11.5's storage checks ran).

## Files

`conf.lua`, `main.lua`, `lib/` (`platform`, `model`, `probe`, `assets`, `report`, `core`, `optional`), `ui/` (the text grid, the app shell, one file per page in `ui/pages/`), `assets/`, `tools/make-assets.mjs`, `tests/`. The code is MIT with the rest of JoyrIDE; `assets.tsv` and `.joyride/assets.toml` say who made each asset and under what licence.
