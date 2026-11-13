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

## Current milestone: none started

Repository set up (2026-11-13); next is spec milestone 1, scaffold,
toolchain manager and core editor (`ROADMAP.md`).

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
5. **Credentials in the OS keychain** (`keyring`): the GitHub token and
   Android keystore passwords, never a file, setting or log.
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
- Tests: `cargo test --manifest-path src-tauri/Cargo.toml`, `npx tsc --noEmit`.
