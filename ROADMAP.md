# JoyrIDE roadmap

Progress and next phases, in the order to do them. Update it in the same
commit as the work. The design is `docs/JoyrIDE_LÖVE-Implementation-Spec.md`
(section 6 is the milestone order); this file is the checklist.

Last reviewed: 2026-11-13.

## 0. Repository

- [x] Public repo, MIT licence, README, CLAUDE.md, CONTRIBUTING, SECURITY, templates. Spec only; no app code.
- [x] Linux and Windows build workflows and release scripts, from Stylus. `.github/workflows/`, `scripts/`.
- [ ] Push to GitHub as `joashchee/joyride` (public). Blocked on `gh` auth reaching GitHub from here.
- [ ] Mac App Store record for `com.ansiapps.joyride` alongside the direct download. Needs the maintainer's accounts.

## 1. Scaffold, toolchain manager, core editor (spec milestone 1)

- [ ] Tauri 2 + React + Vite scaffold with splash, `public/manifest`-free (desktop only). Spec 1.
- [ ] `runtime_manager.rs`: bundled LÖVE 11.5, `12.0+` downloads, signed manifest, `tauri-plugin-updater`. Spec 3A.
- [ ] Monaco editor, `lsp_bridge.rs` with `lua-language-server`, version-switched types, generated `.luarc.json`. Spec 3B.
- [ ] `docs/platform-parity.md` started.

## 2. Runner and console (milestone 2)

- [ ] `runner.rs`, log streaming, clickable stack traces. Spec 3C.
- [ ] Viewport, touch and gamepad simulator. Spec 5.6.

## 3. LÖVE tooling and docs (milestone 3)

- [ ] `love-api-11.5.json` bundled, `12.0+` sync, `DocsDrawer`. Spec 3B.
- [ ] Colour swatches, asset hover previews, version-aware `luacheck`. Spec 3B.

## 4. Package manager and GitHub (milestone 4)

- [ ] `love.toml` resolver and `love.lock`. Spec 3D.
- [ ] GitHub device-flow sign-in, git panel, release workflow generator. Spec 3E.

## 5. Auto-porter and patch engine (milestone 5)

- [ ] `auto_porter.rs` with `full-moon` scanner. Spec 4B.
- [ ] `.joyride/patches/` overlays and `_jr_shim.lua` (scaler, virtual mouse, touch overlay). Spec 4A, 4C.
- [ ] `.gptk` auto-mapper UI.

## 6. Exporters (milestone 6)

- [ ] Desktop (Windows, macOS universal, Linux). Spec 5.1.
- [ ] PortMaster zip. Spec 5.5.
- [ ] Web (`love.js`). Spec 5.4.
- [ ] iOS and iPadOS. Spec 5.2.
- [ ] Android. Spec 5.3.

## 7. Release

- [ ] First signed, notarized universal build with `scripts/build-release.sh`.
- [ ] Third-party licence file generated and shipped (`scripts/license-scan.py`; add the generator Coupler has).
