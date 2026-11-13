# Changelog

## Unreleased

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
