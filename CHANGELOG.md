# Changelog

## Unreleased

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
