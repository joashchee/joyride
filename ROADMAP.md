# JoyrIDE roadmap

Progress and next phases, in the order to do them. Update it in the same
commit as the work. The design is `docs/JoyrIDE_LÖVE-Implementation-Spec.md`
(section 6 is the milestone order); this file is the checklist.

Last reviewed: 2026-11-13 (core logic built; shell, Monaco and project files built, `lsp_bridge` next, see section 1).

## 0. Repository

- [x] Public repo, MIT licence, README, CLAUDE.md, CONTRIBUTING, SECURITY, templates. Spec only; no app code.
- [x] Linux and Windows build workflows and release scripts, from Stylus. `.github/workflows/`, `scripts/`.
- [ ] Push to GitHub as `joashchee/joyride` (public). Blocked on `gh` auth reaching GitHub from here.
- [ ] Mac App Store record for `com.ansiapps.joyride` alongside the direct download. Needs the maintainer's accounts.

## 1. Scaffold, toolchain manager, core editor (spec milestone 1)

The Tauri-free logic is built and tested in `joyride-core/` (and `asset-core/`); the shell and editor now exist (npm is reachable when `registry.npmjs.org` is allowed for the command; use `--cache $TMPDIR/npm-cache`, since `~/.npm` has root-owned files). Next: `lsp_bridge` (the binary is now bundled), then wrap the crates in `src-tauri` commands.

- [x] Tauri 2 + React + Vite scaffold with splash and fast start (CLAUDE.md rule 7), 2026-11-13: static splash in `index.html`, `components/Splash.tsx` (progress, error shown on it), `src-tauri` with a `core_info` command over the core crates; `npm run build` and `cargo build --offline` pass. **Not yet run as a window** (`npm run tauri dev`), icons are a placeholder PNG set (no `.icns`/`.ico`: run `tauri icon` on real art before the release build), no `lsp_bridge`, Monaco or menus yet. Spec 1.
- [x] Toolchain core (`joyride-core/src/toolchain.rs`): LÖVE version rules (11.5, 12.0+), `.luarc.json` generation per version, **signed toolchain manifest** (Ed25519, refused if unsigned or altered) and download checking (size + BLAKE3).
- [ ] Toolchain downloads and `tauri-plugin-updater`: fetching the manifest from GitHub, the pinned public key, installing into the app-data dir. Needs network and a signing key; the maintainer generates the key.
- [x] **Bundled tools** (2026-11-13, `scripts/fetch-toolchains.py`, `scripts/toolchains.lock.json`): LÖVE 11.5 (`love.app`), `lua-language-server` 3.19.2 and StyLua 2.6.0 (arm64 and x64 each), pinned by version and SHA-256 (GitHub's own digest cross-checked), unsafe archive paths refused, all three run on this Mac, listed in `bundle.resources`; Run uses the bundled LÖVE first. **PortMaster is the exception: its export uses PortMaster's own `love_<ver>` runtime and never carries a LÖVE** (CLAUDE.md). Still to do: Linux and Windows assets, signing the nested binaries at the first release build (`docs/platform-parity.md`), the `love-api` 11.5 JSON and LuaCATS types, **luacheck** (its release is a Linux/Windows executable only; needs a Lua to run it or a build from source).
- [x] **Monaco editor** (2026-11-13, `src/components/Editor.tsx`, `src/monaco-setup.ts`): `monaco-editor` 0.57 (MIT), bundled with its own worker (no CDN), Lua highlighting only (the CSS/HTML/JSON/TypeScript services and their 7 MB worker are left out), loaded after the splash so the first screen stays 224 kB. Follows the system light/dark theme. **Not run in a window yet.**
- [ ] `lsp_bridge.rs`: spawn `lua-language-server`, JSON-RPC over the Tauri channel into Monaco (0.57 ships `monaco-lsp-client`, try it first). Needs the binary; the `.luarc.json` side is done.
- [x] **Open a project folder** (2026-11-13): `joyride-core/src/files.rs` (listing with folders first, text read, atomic write; paths and symlinks cannot leave the project; 5 tests), `src-tauri/src/project.rs` commands, folder dialog, `FileTree` (keyboard and VoiceOver roles), tabs with unsaved marks, one Monaco model per file, ⌘S/Ctrl+S saves. **Not run in a window.** Still to do: new/rename/delete file, find in files, restore the last project, a confirm on quitting with unsaved files.
- [x] **Run, Stop and the console** (2026-11-13): `runner::start` (non-blocking, stoppable, exit status; test with a fake LÖVE) and `runner::find_love` (`JOYRIDE_LOVE`, the toolchains folder, then an installed LÖVE), `src-tauri/src/run.rs` (`run-line` and `run-exit` events; links resolved inside the project), `Console` with lines that open the file at the line, ⌘R and ⌘. **Not run in a window.** Uses a LÖVE found on the machine until the bundled one exists; it does not yet pick the project's version from `love.toml`.
- [x] `docs/platform-parity.md` started (bundled tools, signing, PortMaster, folder picker).

## 2. Runner and console (milestone 2)

- [x] Runner core (`runner.rs`): spawns LÖVE on a project, streams stdout and stderr, links errors; links cannot escape the project.
- [x] Stack-trace linking (`trace.rs`): `Error: file:line:`, `[string "…"]:line:` and traceback frames.
- [x] Console UI with clickable links (see Run, Stop and the console in section 1).
- [ ] Viewport, touch and gamepad simulator (spec 5.6): the shim already reads its design size and screen; a simulator needs the runner to set a window size and send events. Frontend plus a `JOYRIDE_SIM` env hook in the shim.
- [x] Shim behaviour tested under LuaJIT with a mocked `love` (`tests/shim_lua.rs`).
- [x] Real LÖVE 11.5 and 12.0 (r3i6nos's builds) run staged PortMaster, mobile and web builds headless: wrapper chain, conf patch, version pin (`tests/e2e_love.rs`). The dummy video driver has no OpenGL, so drawing is not exercised: on a Mac with a display, add a windowed run (`--graphics`).

## 3. LÖVE tooling and docs (milestone 3)

- [x] Version-calibrated lints (`lint.rs`): 0-1 colours, boolean `vsync`, 12.0-only APIs on 11.5, removed-in-12 APIs with their replacements, `.luacheckrc`. **The API tables were measured on the real 11.5 and 12.0 binaries** (`scripts/love-api-diff.py`, checked by `tests/e2e_love.rs`): 7 removed, 29 added, from the non-graphics modules. `love.graphics`, window and input need a display: rerun with `--graphics` on the Mac and extend the tables.
- [x] Colour swatch data (`editor.rs`): `setColor`/`setBackgroundColor` literals with ranges, and writing a colour back.
- [x] Asset hover data (`editor.rs`): image/audio path strings, safe resolution inside the project, PNG/GIF/JPEG sizes.
- [x] Docs search, snippets and hover with version badges (`docs.rs`) over the `love-api` JSON schema.
- [ ] Bundle `love-api-11.5.json` and sync 12.0+ (needs the download); `DocsDrawer` UI and Monaco providers (frontend).
- [ ] Run `luacheck` and `stylua` from the app (needs the binaries).

## 4. Package manager and GitHub (milestone 4)

- [x] `love.toml` and `love.lock` (`manifest.rs`): the spec's schema, validation (bundle id, versions, git URLs that cannot be options or remote helpers, refs).
- [x] Installer (`pkg.rs`): clones into `libs/<name>`, pins tag, rev or branch, drops `.git`, writes the lock, skips what is up to date; tested against real git repos.
- [x] **Curated libraries** (`joyride-core/src/library.rs`, `data/libraries.toml`): 18 libraries (hump, bump, anim8, lume, flux, push, STI...), each pinned to an exact commit (newest tag where there is one), licence read from its repository (all MIT), generated by `cargo run --example curate_libraries` from `data/library-seeds.toml`; add-to-manifest pins the commit; search; per-version badges.
- [ ] **Run the load test** (`cargo run --example curate_libraries -- --probe`): requires each library on real LÖVE 11.5 and 12.0 headless to fill the badges (now `Not tested`). It executes third-party library code, so it is opt-in and left for you to run.
- [ ] Library manager UI.
- [ ] GitHub device-flow sign-in with the OS keychain (`keyring` is not available offline), git panel, `release.yml` generator.

## 5. Asset Library (design: `docs/asset-library.md`)

Tracks every usable asset with its licence, category, tags and preview, makes licence-free assets deterministically, and imports and de-duplicates from other projects. Draws on Diskette (catalog, FTS5, thumbnails), Crunchy (de-dup pipeline), Neumetik (tag and rights schema), Stylus (provenance records) and Painter, Composer, Animator and chipbank (generators). Needs the Package Manager's `love.toml`.

- [x] **Painter, Composer and Animator vendored under MIT** (2026-11-13): `src/vendor/`, `scripts/vendor-sync.sh`, `NOTICE`; tests pass (154, 62, 23). Chipbank (Neumetik) is still proprietary: its 758 sound recipes need a separate decision.
- [x] **Catalog** (clean-room, 2026-11-13): SQLite store, FTS5 search, hash cache valid while size+mtime match. `asset-core/src/catalog.rs`. [ ] Incremental rescan of watched library folders still to do.
- [x] **Licence record** per asset (`asset-core/src/licence.rs`: flags, guess from text/sidecars, confirmed-by status; unknown blocks use) (SPDX id, source, author, attribution, commercial/modify/redistribute, AI involvement, confirmed-by and when), from Neumetik's rights schema. Unknown is shown and blocks use.
- [x] **Audio asset management after Neumetik's catalog** (2026-11-13, `asset-core/src/audio.rs`): header probe for WAV/FLAC/Ogg/MP3, raw peak and RMS, game-audio filename convention (`MUS_Area_State_…_96_Dm_v03` → tags), Neumetik tag syntax with a linter, hierarchical tag / BPM / length filters, facet counts, rights (licensor, territory, expiry alert, AI involvement). LUFS, loop verification and waveform previews still to do.
- [x] **Image facets and previews** (`asset-core/src/preview.rs`, stored in the catalog): size, size class, palette richness, alpha and a pixel-art guess become tags on import (`size:16x16`, `style:pixel-art`, `palette:limited`); a 128 px PNG thumbnail (pixel art upscaled by whole numbers, so it stays crisp); six-colour palette with near-duplicates merged; a 128-bin waveform for WAV. An image that will not decode is still imported.
- [ ] Smart Folders (saved searches); spritesheet frame detection; font samples; Lua library readme previews; waveform for FLAC, Ogg and MP3 (needs decoders).
- [ ] **Asset Library panel** in the sidebar: browse, search, filter by licence, "Add to project" (copies into the project, records the use), hover preview in Monaco for library assets.
- [x] **Import from a project** (`asset-core/src/import.rs`, plan then apply; 6 integration tests): scan, guess licences from `LICENSE`/`README`/`CREDITS`, **confirm licences before anything is copied**, then import. Spec 3D, `asset-library.md` rule 2.
- [x] **Automatic de-duplication on import** (`asset-core/src/dedup.rs` staged size → prefix → BLAKE3; import groups by content, stores once, records uses, flags licence conflicts): size → prefix → first+last → BLAKE3 pipeline from Crunchy; identical content becomes a new use of the existing record; conflicting licences ask; near-duplicates only suggested. Nothing deleted.
- [x] **`.joyride/assets.toml`**, **`CREDITS.txt`** and the **export gate** (`asset-core/src/project.rs`): each project lists what it uses; credits collected once per asset; an export stops on unconfirmed, non-commercial (for a commercial game), share-alike or expired assets. Wired into builds by `joyride-core/src/pipeline.rs`.
- [x] **Recording generated assets** (`asset-core/src/generated.rs`): engine + version + recipe + seed + params stored as the origin; output is CC0 and confirmed by the generator; the same pixels twice are one asset. The `CC0` choice for generated output is the maintainer's to confirm.
- [ ] **Running the generators** (Painter, Composer, Animator in `src/vendor/`, JavaScript) from the app's webview or a worker, and chip sound effects (chipbank, not vendored). Frontend work.
- [ ] **Generator UI**: pick a recipe, tweak params, reroll seed, preview, save to the library; lint results shown.

## 6. Auto-porter and patch engine (milestone 5)

- [x] Scanner (`scan.rs`, on a small Lua lexer `lua.rs`, since `full-moon` is not available offline): keys, mouse, touch, gamepad, `conf.lua` window, and per-target findings (ffi, threads, popen/execute, shader versions, keyboard-only, fullscreen). Swap in `full-moon` behind `lua.rs` if a rule ever needs real structure.
- [x] Overlays (`overlay.rs`): `.joyride/patches/<target>/` written and read; staging copies the project and merges the overlay without touching the source (tested by comparing the tree before and after); `conf.lua` values patched in the copy; unified diffs applied exactly or refused; drop-in overrides.
- [x] Shims (`shim.rs`): letterboxed integer or smooth scaler with mouse and touch mapping; PortMaster right-stick cursor, Select+Start quit; mobile touch pad that fires synthetic key events; web stubs. Tested under LuaJIT.
- [x] `.gptk` generator from the scanned keys.
- [ ] Overlay UI: Auto-Port panel, handheld diagram, editing the `.gptk`.
- [ ] Shim polish needing a real device: canvas stencil use, shaders on GLES, safe-area on notched phones.

## 7. Exporters (milestone 6)

- [x] `.love` archive (`export.rs`): reproducible bytes, `main.lua` at the root.
- [x] Windows (fused exe + DLLs), Linux (fused AppImage), macOS (`love.app` copy, game placed, `Info.plist` rewritten, symlinks kept). Signing is the caller's step. Icons (`.ico`, `.icns`) not generated yet.
- [x] **Checked against PortMaster-New** (`40186ea1`, 2026-11-13, `scripts/check-portmaster.py`): the first draft was wrong in several ways (GPTOKEYB2 vs `$GPTOKEYB "$LOVE_GPTK"`, `port.json` v3 vs 4 and its attr fields, a `love_12.0` runtime that does not exist). Fixed; the exporter now matches all 68 real LÖVE ports' conventions. The weekly workflow keeps watching.
- [x] **The 11.5 variant of a 12.0 game, for every export** (`joyride-core/src/compat.rs`, `pipeline::Compat`): PortMaster has no `love_12.0`, so a 12.0 project can build an 11.5 variant for any target. Built from the staged copy (source untouched), conf pinned to 11.5, and a generated `_jr_compat.lua` defining only the 12.0 functions the project uses: **exact** (`love.filesystem.openFile`, `love.event.restart`), **reduced** (`love.sensor.*` acts as no sensors, `love.markDeprecated` prints nothing) or **unsupported** (everything else 12-only: the build stops and lists each use by file and line unless the person accepts that API). Compared with the real 11.5 and 12.0 binaries, including error messages (`tests/e2e_love.rs`). The spec assumed `love_12.0` on PortMaster; it does not exist yet (the weekly check fails when it appears, then add it to `PORTMASTER_LOVE_VERSIONS`).
- [ ] **Variant UI**: offer "Also build for LÖVE 11.5" on a 12.0 project (`pipeline::compat_offer` has the report: exact, reduced, blockers), with per-API accept checkboxes saved in the project. `love.graphics` 12.0 functions are not classified (no display to measure): extend `compat::support` after `scripts/love-api-diff.py --graphics`.
- [ ] Variant for physics body helpers, `mountCommonPath` and the rest: only if exact emulations are found; they are unsupported today.
- [ ] **Run `scripts/check-linux.sh` and `--arm64`** in Docker (Colima): unrun, the sandbox cannot reach Docker.
- [x] PortMaster zip: layout, `port.json`, launch script, `.gptk`, matched to PortMaster-New's real LÖVE ports. **Not run on a handheld.**
- [x] **Web page** (`joyride-core/src/web.rs`, `pipeline::build_web`): love.js's packaging is a few plain steps (read from its `index.js`), done in Rust with no Node: `game.data` is the `.love`, `game.js` and `index.html` are its templates filled in (title HTML-escaped, `["./game.love"]`, memory sized to the game), plus `love.js`, `love.wasm`, `theme/`, and for the standard version `love.worker.js` and a `_headers` file with the two cross-origin headers; the compatibility page needs neither. Reproducible: the package id comes from the game's bytes. Both versions package from a real `Davidobot/love.js` checkout (env-gated test, every placeholder filled). **LÖVE 11.5 only** (the template's runtime), so a 12.0 project builds its 11.5 variant for the web.
- [ ] **Run the web page in a browser** (not done: no browser here): the standard version needs the two headers served; try `python3 -m http.server` plus headers, or the compatibility page on itch.io.
- [x] **Web saves**: corrected (an earlier note said they were not persisted). love.js mounts IndexedDB at `/home/web_user/love` itself, loads it before the game starts and syncs on `beforeunload`; the limit is that a tab that dies without `beforeunload` (mobile, crash) can lose the latest writes, and the page cannot sync sooner (`FS` is not exported). Needs a fork that exports `FS` or syncs on `visibilitychange`.
- [x] **love.js fork survey** (`docs/lovejs-forks.md`, `scripts/check-lovejs-forks.py`, weekly workflow): Davidobot/love.js has had no commit since 2024-05-13; of 73 forks, **pkhead/love.js `lua52`** (Oct 2026, +22: Lua 5.2 not LuaJIT, Emscripten 6.0.1, WASM exceptions) is the most current engine, then dreagledr (HTTP module), cptobakhmut925-glitch (WebGL2), jeduden (mobile layout). The exporter packages both Davidobot's and the `lua52` fork's template layouts (tested on real checkouts); Davidobot's stays the default because LuaJIT keeps the web build equal to the other exports.
- [ ] **Try the `lua52` fork in a real browser** (prepared, not run: Claude in Chrome was not linked to the session): `joyride-core/examples/web_demo.rs` builds a demo game (shows `_VERSION`/LuaJIT, a frame counter, the last key, and a save counter that must go up on each page reload) with either template, `scripts/serve-web.py` serves it with the two headers; the four pages (Davidobot and `lua52`, standard and compat) are built in `build/web-test/` (`scripts/serve-web.py build/web-test 8765`). Then try it with a real game and decide whether to offer it (staging its `webcompat/` and requiring it first in `conf.lua`); read the WebGL2, mobile-keyboard and responsive-layout patches before taking any.
- [x] **iOS and iPadOS Xcode project** (`joyride-core/src/apple_mobile.rs`, `pipeline::build_ios`): written against the real `love` repository's `platform/xcode/love.xcodeproj` (`main` and `11.x`, same ids). `game.love` is added to the project (file reference, build file, the `love-ios` Resources phase) by targeted text edits; bundle id, name, version, device family (iPhone, iPad, universal) and team are the target's three configurations (the macOS target keeps LÖVE's identity); orientation lock, full-screen and no `.love` registration are `ios/love-ios.plist`; one 1024 px icon. **Xcode's own `xcodebuild -list` accepts the edited project** for both branches (checked in the test). Also: unsigned `.ipa` packing for AltStore/SideStore, `ExportOptions.plist`, `xcodebuild` arguments. Works with the 11.5 variant.
- [ ] **Run the iOS build** (`xcodebuild -target love-ios … build`): compiles LÖVE's C++ and needs LÖVE's iOS dependencies (`love-apple-dependencies`: `iOS/libraries` and `shared`, from its GitHub release), so it is not run here. The signed archive needs a shared scheme (the template has none) or Xcode.
- [x] **Android project** (`joyride-core/src/android.rs`, `pipeline::build_android`): written against the real `love-android` `12.x`/`main` and `11.x` branches (read 2026-11-13; both stage correctly with their real `gradle.properties`): the game as `app/src/embed/assets/game.love`, app name (ASCII, `\uXXXX`, or 11.x's byte list), application id, orientation from the game's window, version code and name, launcher icons from one PNG, and the `assembleEmbedNoRecordRelease` / bundle task. Works with the 11.5 variant. Signing commands (`keytool`, `apksigner`) take passwords by environment variable, never as arguments.
- [ ] **Run the Android build** (`./gradlew assembleEmbedNoRecordRelease` on a staged project) and sign it: needs the Android SDK and NDK and executes the template's Gradle scripts, so it is not run here. Also the submodule checkout (`git submodule update --init --recursive`) and the keystore UI (password in the OS keychain).
- [x] One-call builds (`pipeline.rs`): scan, make the overlay if missing, stage, check asset licences, add `CREDITS.txt`, zip; PortMaster zip from a project folder.
- [ ] Export Center UI.

## 8. Release

- [ ] First signed, notarized universal build with `scripts/build-release.sh`.
- [x] Third-party licence file (`scripts/third-party-licenses.py` → `docs/third-party-licenses.txt`, 229 distinct texts for every crate, the vendored engines' notices and `NOTICE`) and the licence scan (`scripts/license-scan.py`: no copyleft-only dependency) run in CI; `--check` fails when the committed file is stale. Ship it in the app (About) with the shell; `build-release.sh` should run it before a release build.
