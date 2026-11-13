# JoyrIDE

A zero-config desktop IDE for [LÖVE](https://love2d.org) (Love2D). Free
and open source (MIT). macOS first, with Windows and Linux builds.

Open a folder and run it. JoyrIDE bundles LÖVE 11.5 and works offline,
switches a project to LÖVE 12.0+ with one setting, and ports an existing
desktop game to PortMaster handhelds, iOS, iPadOS, Android, the web and
desktop **without changing a line of its source**: target adaptations are
kept as overlays in `.joyride/patches/`.

- Monaco editor with `lua-language-server`, version-matched LÖVE types,
  `luacheck` and `stylua`
- Offline, version-aware API docs, colour swatches, asset hover previews
- Run with a console that links stack traces to the code
- `love.toml` dependency manager and GitHub sign-in
- Auto-porter and exporters for every target

**Status: pre-alpha, nothing built yet.** The plan is in
[`docs/JoyrIDE_LÖVE-Implementation-Spec.md`](docs/JoyrIDE_LÖVE-Implementation-Spec.md)
and the order of work in [`ROADMAP.md`](ROADMAP.md).

## Build

Tauri 2 (Rust) with React, TypeScript and Vite. Once the scaffold lands:

```sh
npm install
npm run tauri dev      # run the app
cargo test --manifest-path src-tauri/Cargo.toml
scripts/license-scan.py
```

See `CLAUDE.md` for the rules and `CONTRIBUTING.md` to help.

## License

MIT (`LICENSE`). Bundled LÖVE runtimes and tools keep their own licences,
listed in `NOTICE`.
