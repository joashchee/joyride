# JoyrIDE roadmap

Progress and next phases, in the order to do them. Update it in the same
commit as the work. The design is `docs/JoyrIDE_LÖVE-Implementation-Spec.md`
(section 6 is the milestone order); this file is the checklist.

Last reviewed: 2026-11-13 (core logic built; app shell blocked on network, see section 1).

## 0. Repository

- [x] Public repo, MIT licence, README, CLAUDE.md, CONTRIBUTING, SECURITY, templates. Spec only; no app code.
- [x] Linux and Windows build workflows and release scripts, from Stylus. `.github/workflows/`, `scripts/`.
- [ ] Push to GitHub as `joashchee/joyride` (public). Blocked on `gh` auth reaching GitHub from here.
- [ ] Mac App Store record for `com.ansiapps.joyride` alongside the direct download. Needs the maintainer's accounts.

## 1. Scaffold, toolchain manager, core editor (spec milestone 1)

The Tauri-free logic is built and tested in `joyride-core/` (and `asset-core/`); the app shell is not, because the npm registry is unreachable from the build sandbox (Vite, React and Monaco can't be installed). Do those from a machine with network, then wrap the crates in `src-tauri` commands.

- [ ] Tauri 2 + React + Vite scaffold with splash and fast start (CLAUDE.md rule 7). Blocked: needs `npm install` with network. Spec 1.
- [x] Toolchain core (`joyride-core/src/toolchain.rs`): LÖVE version rules (11.5, 12.0+), `.luarc.json` generation per version, **signed toolchain manifest** (Ed25519, refused if unsigned or altered) and download checking (size + BLAKE3).
- [ ] Toolchain downloads and `tauri-plugin-updater`: fetching the manifest from GitHub, the pinned public key, installing into the app-data dir. Needs network and a signing key; the maintainer generates the key.
- [ ] Bundle LÖVE 11.5 binaries, `lua-language-server`, `luacheck`, `stylua`, the 11.5 `love-api` and LuaCATS types in `src-tauri/resources` (NOTICE lists licences). Needs downloads.
- [ ] Monaco editor and `lsp_bridge.rs` (spawn `lua-language-server`, JSON-RPC over the Tauri channel). Blocked on npm; `.luarc.json` side is done.
- [ ] `docs/platform-parity.md` started.

## 2. Runner and console (milestone 2)

- [x] Runner core (`runner.rs`): spawns LÖVE on a project, streams stdout and stderr, links errors; links cannot escape the project.
- [x] Stack-trace linking (`trace.rs`): `Error: file:line:`, `[string "…"]:line:` and traceback frames.
- [ ] Console UI with clickable links (frontend; blocked on scaffold).
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
- [ ] Curated libraries list with LÖVE-version badges and the library manager UI.
- [ ] GitHub device-flow sign-in with the OS keychain (`keyring` is not available offline), git panel, `release.yml` generator.

## 5. Asset Library (design: `docs/asset-library.md`)

Tracks every usable asset with its licence, category, tags and preview, makes licence-free assets deterministically, and imports and de-duplicates from other projects. Draws on Diskette (catalog, FTS5, thumbnails), Crunchy (de-dup pipeline), Neumetik (tag and rights schema), Stylus (provenance records) and Painter, Composer, Animator and chipbank (generators). Needs the Package Manager's `love.toml`.

- [x] **Painter, Composer and Animator vendored under MIT** (2026-11-13): `src/vendor/`, `scripts/vendor-sync.sh`, `NOTICE`; tests pass (154, 62, 23). Chipbank (Neumetik) is still proprietary: its 758 sound recipes need a separate decision.
- [x] **Catalog** (clean-room, 2026-11-13): SQLite store, FTS5 search, hash cache valid while size+mtime match. `asset-core/src/catalog.rs`. [ ] Incremental rescan of watched library folders still to do.
- [x] **Licence record** per asset (`asset-core/src/licence.rs`: flags, guess from text/sidecars, confirmed-by status; unknown blocks use) (SPDX id, source, author, attribution, commercial/modify/redistribute, AI involvement, confirmed-by and when), from Neumetik's rights schema. Unknown is shown and blocks use.
- [x] **Audio asset management after Neumetik's catalog** (2026-11-13, `asset-core/src/audio.rs`): header probe for WAV/FLAC/Ogg/MP3, raw peak and RMS, game-audio filename convention (`MUS_Area_State_…_96_Dm_v03` → tags), Neumetik tag syntax with a linter, hierarchical tag / BPM / length filters, facet counts, rights (licensor, territory, expiry alert, AI involvement). LUFS, loop verification and waveform previews still to do.
- [ ] **Categories and tags**: controlled facets for images (style, size, palette) plus free tags; Smart Folders (saved searches).
- [ ] **Previews**: image and spritesheet thumbnails, audio player with waveform, font sample, Lua library readme, from Diskette's `thumbnails.rs`.
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
- [ ] Web shim: IndexedDB `FS.syncfs` persistence. The shim marks writes (`jr.dirty`); the page that flushes needs the love.js template.
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
- [ ] Web (`love.js`): needs the love.js template and its packager.
- [ ] iOS and iPadOS (`apple_mobile.rs`): needs the LÖVE iOS Xcode template; `plist` handling exists.
- [ ] Android (`android.rs`): needs the `love-android` Gradle template and the keystore flow.
- [x] One-call builds (`pipeline.rs`): scan, make the overlay if missing, stage, check asset licences, add `CREDITS.txt`, zip; PortMaster zip from a project folder.
- [ ] Export Center UI.

## 8. Release

- [ ] First signed, notarized universal build with `scripts/build-release.sh`.
- [ ] Third-party licence file generated and shipped (`scripts/license-scan.py`; add the generator Coupler has).
