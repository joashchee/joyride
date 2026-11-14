# JoyrIDE

A zero-config desktop IDE for LÖVE (Love2D): Tauri 2 (Rust) + React /
TypeScript / Vite, Monaco with `lua-language-server`, LÖVE 11.5 bundled
and 12.0+ switchable, and non-destructive auto-porting to PortMaster, iOS,
iPadOS, Android, web and desktop. Free and open source (MIT), macOS first.
This file is the condensed operating rules and a map. The design is
**`docs/JoyrIDE_LÖVE-Implementation-Spec.md`**: read it before any
architectural change. Sister apps to copy from: Stylus (MIT, same
scripts and CI), Coupler (reference release script, CONTRIBUTING),
Diskette (ROADMAP format).

**`CHANGELOG.md`**: add a bullet whenever a change lands. **`ROADMAP.md`**:
tick and update it in the same commit as the work.

## Current milestone: the app shell, filling out

The Tauri-free core is built and tested (`asset-core/`, `joyride-core/`) and
the Tauri 2 + React + Monaco shell runs around it (`src-tauri/`, `src/`):
project files, editor with the language server, StyLua, lints and colour
swatches, Run and console, find in files, libraries, the LÖVE reference,
Auto-Port and Export (`.love`, PortMaster). `ROADMAP.md` has what is left
(Asset Library and generator panels, the toolchain download and updater,
the other exporters' UI, the first signed release). Nothing in the core
needs the network to build or test (`cargo ... --offline`); `npm` needs
`registry.npmjs.org` allowed.

## Non-negotiable rules

1. **No telemetry, no accounts of ours.** Network use is only what the
   user asks for or turns on: the IDE update check, the signed toolchain
   manifest, library downloads, GitHub.
2. **Open source and free, MIT** (`LICENSE`, `NOTICE`). **No
   GPL/AGPL/LGPL dependencies.** Run `scripts/license-scan.py` whenever
   `Cargo.lock` or `package-lock.json` changes. Bundled tools keep their
   own licences, listed in `NOTICE`. The global "closed source" rule
   does not apply; its other half (no user-visible text names a repo
   file) does.
3. **Bundle ID `com.ansiapps.joyride` on every platform** (decided
   2026-11-13, overriding the global `com.scorelessmusic` default for
   free Mac apps). Never change it once a build ships: it keys the
   app-data folder. (A project's own `bundle_id` in `love.toml` is the
   game's, not ours.)
4. **The user's source is never mutated.** Target adaptations live in
   `.joyride/patches/<target>/` and are merged into a staged copy at
   build time (spec section 4).
5. **No GitHub credentials of ours.** GitHub goes through the `gh` command
   line, as in Claude Code: the person runs `gh auth login` once, JoyrIDE
   only runs `gh` and `git` (git gets credentials from `gh auth
   git-credential`), and never sees, stores or logs a token
   (`joyride-core/src/github.rs`). Android keystore passwords stay in the
   OS keychain (`keyring`), never a file, setting or log.
6. **The toolchain manifest is signed**; unsigned or mismatched downloads
   are refused. User data and toolchains live under Tauri's app-data
   dir, never a hardcoded path (the spec's `~/.joyride/toolchains/` is
   resolved through it).
7. **Fast start**: a splash at once, heavy work off the UI thread, the
   rest loaded after the editor is usable; errors shown on the splash.
8. **macOS first.** Add a `docs/platform-parity.md` row for every
   macOS-specific mechanism.
9. **Accessible**: every control works by keyboard alone and is named for
   VoiceOver.
10. **No first-run warning dialog.**
13. **The UI is the ANSIapps text-mode look and nothing else** (decided
    2026-11-13): no graphical theme and no switch for one. The only
    appearance options are colours: the palette, True colour (default,
    with theme swatches in `src/swatches.ts`) or 16-colour (DOS mode, fixed).
    A new swatch must pass `node scripts/check-swatches.ts`. Build on the 8 by 16 grid: widths, paddings
    and gaps in whole cells, no borders on anything holding text, 16px
    VGA font only, no Mac key symbols (write `Cmd+S`), every text and
    background pair from `docs/ansiapps-color-contrast.md`. Run
    `checkGrid()` (dev console) on each panel after changing
    `src/ansiapps-theme.css`. Details: `docs/text-mode-ui.md`.
14. **Every library added to or updated in the curated list is audited first**
    (decided 2026-11-14). `python3 -I scripts/audit-libraries.py <names>` clones
    each at its exact pinned commit (nothing is run) and flags secrets and
    personal data, binaries, submodules, `os.execute`/`io.popen`/`ffi`,
    `loadstring`, network and file writes, obfuscation and foreign licences.
    **Read the findings before anything runs the code:** the audit comes before
    `curate_libraries -- --probe` (which executes it) and before a commit.
    Then `--stamp "what you reviewed" <names>` records it in
    `joyride-core/data/library-audits.toml`; a test fails for a pinned commit
    with no record, so an update (a new `rev`) must be audited again. Put real
    hazards in the entry's `caution` (the panel shows it). Re-probe an audited
    commit with `--probe --pinned --only <names>`, which never re-pins.
15. **New benchmark results always update the libraries' info and ratings**
    (decided 2026-11-14). Whenever Benchmark Libraries reports arrive (the maintainer
    pastes one, or a run is made), in the same commit: save the report under
    `samples/benchmark-libraries/results/`, run `python3 -I scripts/ingest-benchmark.py
    <report>` (it rewrites `joyride-core/data/library-benchmarks.json`: per-library
    load result per LÖVE version, scores and best marks per comparison, findings,
    and the one `benchmark` line the Libraries panel shows), then read what changed.
    A finding that is a real hazard also goes into the library's `caution` (both
    `library-seeds.toml` and `libraries.toml`); a library that no longer loads
    gets its badge from the ingest and is refused on that LÖVE. If the sample's
    own code changed since a report, `--skip name` leaves that library's stale
    result out until it is run again. `python3 -I scripts/ingest-benchmark.py
    --check` and the `every_library_has_benchmark_results...` test fail on
    missing data. Update `README.md` findings, `ROADMAP.md` and `CHANGELOG.md`.
16. **Code JoyrIDE inserts into the user's files is always shown** (decided
    2026-11-14). Any line JoyrIDE writes into a source file of the person's
    (today: the require-path line per library in `main.lua`,
    `joyride-core/src/integrate.rs`) is recorded in `.joyride/inserted.json`
    and the editor marks it with a colour bar in the left margin
    (`.joyride-inserted`, `Editor.tsx`). The bar goes the moment the person
    touches that line, the line is then theirs (never marked, re-added,
    rewritten or removed with its library), and nothing is inserted into a
    file with unsaved edits. A new kind of inserted code uses the same record
    and the same bar; it never writes silently.
11. **Asset licences are confirmed before use** (`docs/asset-library.md`):
    nothing is imported, added to a project or exported with an unknown
    licence unless the user overrides and says why. Imports never copy
    before confirmation and never delete originals. Generators are
    seeded and deterministic (own PRNG, integer math, recipe + version +
    seed stored). Painter, Composer and Animator are vendored under MIT in
    `src/vendor/` (decided 2026-11-13) by `scripts/vendor-sync.sh`: change
    them in their own repos and re-sync, never only here. Any other
    proprietary sister code (Neumetik chipbank, Diskette, Crunchy) still
    needs the maintainer's decision recorded in `NOTICE` first.

## Where things live

- `asset-core/`: the Tauri-free Rust crate for the Asset Library
  (`docs/asset-library.md`), written clean-room from the ideas in Diskette
  and Crunchy, no code from either: `catalog.rs` (SQLite + FTS5, licences,
  tags, uses, origins, hash cache), `licence.rs`, `dedup.rs` (staged
  duplicate finding), `import.rs` (plan, then apply with confirmed
  licences), `kind.rs`. `cargo test --offline --manifest-path
  asset-core/Cargo.toml`.
- `joyride-core/src/integrate.rs`: the lines JoyrIDE inserts into `main.lua` for a project's libraries and the record that lets the editor bar them (rule 16); `lint_libs.rs`: the lints that know the libraries (a `require` that cannot work, measured pitfalls, near-miss names); both read the library list, and the language server gets the same require path (`lsp::settings_with`).
- `joyride-core/src/detect.rs`: the libraries a project uses (files only), matched with the list; the person's own libraries (`Custom`) and dismissed offers live in app-data, never in a project. A library found in a project is never added to `data/libraries.toml` without rule 14's audit.
- `joyride-core/`: the other Tauri-free crate, one module per spec area:
  `manifest`/`pkg` (love.toml, installer), `toolchain` (versions, signed
  manifest, `.luarc.json`), `lua`/`scan`/`lint` (lexer, auto-porter analysis,
  version lints), `trace`/`runner`, `shim`/`overlay` (generated Lua shims,
  zero-mutation staging), `export`/`pipeline` (.love, desktop, PortMaster,
  one-call builds with the asset gate), `editor`/`docs` (Monaco provider data,
  API search). Tests under LuaJIT with a mocked `love` (`tests/shim_lua.rs`);
  `tests/e2e_love.rs` needs a display and skips without one.
  `cargo test --offline --manifest-path joyride-core/Cargo.toml`.
- `src/ansiapps-theme.css`, `src/lib/theme.ts` (the palette), `src/lib/grid.ts`,
  `src/monaco-theme.ts` (the editor's two palettes), `components/Panel.tsx`
  (a framed window): the text-mode UI, `docs/text-mode-ui.md`.
- `samples/`: whole projects that ship with the app (`joyride-core/src/samples.rs` copies one to the app-data folder; `Samples…` in the toolbar). `samples/io-checker/` detects the platform and checks its I/O and every asset type (README there; `joyride-core/tests/io_checker_sample.rs`). Its assets are made by `tools/make-assets.mjs`; `asset-core/examples/sample_assets_toml.rs` writes the licence record. Regenerate both when an asset changes or the licence test fails.
- `samples/benchmark-libraries/` loads every curated library (vendored in `libs/` by `tools/vendor-libs.py`; rerun it and `joyride-core/tests/benchmark_libraries_sample.rs` when `libraries.toml` changes) and rates the overlapping ones (`bench/groups/`, one file per comparison). Add a new library to a comparison or to `bench/specialists.lua`; a unit test fails for a library with neither.
- `src/vendor/`: Painter, Composer, Animator (MIT; `scripts/vendor-sync.sh`).
- `ogv/`: Ogg Theora probe and frame decoder over `vendor/xiph/` (`cargo test --offline --manifest-path ogv/Cargo.toml`).
- `vendor/xiph/`: libogg and libtheora's decoder (BSD-3-Clause, unmodified; `scripts/vendor-xiph.sh`).
- `vendor/neumetik/`: Neumetik's players and `nttplay` (MIT; `scripts/vendor-neumetik.sh`; change them in Neumetik and re-sync).

## love.js and its forks

The web exporter targets `Davidobot/love.js` (LuaJIT, 11.5; untouched since
2024). Forks have newer work, notably `pkhead/love.js` `lua52` (Lua 5.2,
Emscripten 6.0.1): `docs/lovejs-forks.md`, re-checked weekly by
`scripts/check-lovejs-forks.py`. Do not switch templates silently: Lua 5.2 is
a different Lua, and the web build must behave like the other exports.

## The 11.5 variant of a 12.0 game

A 12.0 project can build an 11.5 variant for any target
(`pipeline::Compat`, `joyride-core/src/compat.rs`); it is the way a 12.0 game
reaches PortMaster, which has only `love_11.5`. Never define a 12.0 function in
the variant unless it was compared with the real 11.5 and 12.0 binaries
(`tests/e2e_love.rs`); anything else is unsupported and stops the build. An
unsupported API is never faked, so feature tests keep answering truthfully.

## Local LÖVE, Docker and PortMaster

- **Real LÖVE 11.5 and 12.0 are on this Mac** in r3i6nos's checkout
  (`../r3i6nos/vendor/love-mac/<version>/love.app`; the tests also look in
  `/Applications` and at `JOYRIDE_LOVE_11_5` / `JOYRIDE_LOVE_12_0`). Use
  them, not guesses: `tests/e2e_love.rs` runs staged builds on both and checks
  the lint tables against them; `scripts/love-api-diff.py` measures API
  differences. Under SDL's dummy drivers there is **no OpenGL**, so only
  headless modules run (no window, graphics, input); drawing and input are
  tested against a mocked `love` under LuaJIT, and on a Mac with a display
  `--graphics` widens the measurements.
- **Docker** (Colima, as r3i6nos uses) is for Debian 12 and linux/arm64:
  `scripts/check-linux.sh [--arm64]`. The build sandbox cannot reach it;
  the maintainer runs it.
- **12. Check PortMaster's GitHub repo before changing or trusting the
  PortMaster exporter, and keep it current.** `scripts/check-portmaster.py`
  runs the exporter and compares its launch script and `port.json` with
  PortMaster-New's real LÖVE ports; the weekly `portmaster-check` workflow
  runs it too. A failure means the exporter is out of date. `github.com` is
  allowed in `.claude/settings.local.json`, so run it here. As of PortMaster-New
  `40186ea1` (2026-11-13): 68 LÖVE ports, **only the `love_11.5` runtime**
  (so a 12.0 project cannot target PortMaster yet: the exporter refuses),
  `port.json` version 4, `$GPTOKEYB "$LOVE_GPTK"`.

- **PortMaster uses PortMaster's own LÖVE** (the maintainer, 2026-11-13:
  PortMaster recommends its runtimes over bundled binaries). The exporter's
  zip never contains a LÖVE binary: the launch script sources
  `runtimes/love_<ver>/love.txt`. `scripts/fetch-toolchains.py` and the
  toolchain manifest therefore have no PortMaster entry, and a PortMaster
  build must never add a LÖVE.

## Build notes

- macOS release: `scripts/build-release.sh` (universal, Developer ID,
  hardened runtime, notarized with the `scorelessmusic` notarytool
  profile, stapled, checked, zipped). Never on CI, never with Apple
  credentials in the repo. `JOYRIDE_UNSIGNED=1` for local trials. "Build
  the macOS app" means the `.app` only. Run it first when Tauri exists:
  it assumes `src-tauri/` and `package.json`.
- Linux and Windows build on GitHub-hosted runners (`.github/workflows`),
  published by `scripts/publish-release.sh`. Standard runners only, so
  it stays free. No AppImage of the IDE itself (it would ship LGPL
  WebKitGTK/GTK): `.deb` and `.rpm`.
- `scripts/fetch-toolchains.py` downloads the bundled tools (LÖVE 11.5, lua-language-server, StyLua) into `src-tauri/resources/toolchains/` (git-ignored), pinned by SHA-256 in `scripts/toolchains.lock.json`; `--update` re-pins, and you read the lock diff.
- Tests: `cargo test --manifest-path src-tauri/Cargo.toml`, `npx tsc --noEmit`.
