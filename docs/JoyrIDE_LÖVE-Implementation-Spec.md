# JoyrIDE — LÖVE Implementation Specification

## 1. Objective & Tech Stack
Build **JoyrIDE**, a dedicated, zero-config desktop IDE for the **LÖVE (Love2D)** framework featuring non-destructive, automated multi-platform porting and seamless version switching (defaulting to **LÖVE 11.5**, with built-in support and self-updating tooling for **LÖVE 12.0+**).
- **Shell & Backend:** Tauri v2 (Rust) + React/TypeScript (Vite + Tailwind CSS) with `tauri-plugin-updater` for IDE self-updates.
- **Editor Engine:** Monaco Editor (`monaco-editor`) with Lua language support, connected via JSON-RPC/WebSocket bridge to an embedded `lua-language-server` (`lua-ls`) process.
- **Bundled Default Toolchain (`11.5`) & Managed Runtime Cache (`~/.joyride/toolchains/`):**
  - Bundled `love-11.5` desktop runtime binaries (Windows, macOS Universal, Linux AppImage) for instant offline startup, plus downloadable `12.0+` runtimes.
  - `lua-language-server` + version-indexed `love-api` LuaCATS type definitions (`11.5` bundled, `12.0+` auto-fetched/updated).
  - `luacheck` binary & `stylua` binary.
  - Versioned `love.js` WebAssembly template bundles.
  - Versioned mobile build templates (`love-ios` Xcode template & `love-android` Gradle AAR/APK template).
  - PortMaster runtime templates (`port.json`, launch `.sh`, `.gptk`) targeting `love_11.5` or `love_12.0` shared runtimes, plus Lua shim libraries (`_jr_shim.lua`).

---

## 2. Directory & Architecture Blueprint

```text
joyride/
├── src-tauri/                  # Rust backend (IPC commands, process & build pipelines)
│   ├── src/
│   │   ├── main.rs
│   │   ├── runtime_manager.rs  # Manages LÖVE versions (11.5 default, 12.0+), self-updates API/defs/templates
│   │   ├── lsp_bridge.rs       # Spawns lua-language-server, dynamically binds active LÖVE version types
│   │   ├── runner.rs           # Launches active `love` binary for <project_dir>, streams stdout/stderr
│   │   ├── pkg_manager.rs      # Parses love.toml, clones/downloads libs to libs/
│   │   ├── git_service.rs      # Git CLI / git2 wrapper + GitHub API device-flow OAuth
│   │   ├── auto_porter.rs      # AST/static analyzer + non-destructive overlay patch generator
│   │   └── exporters/
│   │       ├── love_archive.rs # Zips project + target overlay patches -> game.love
│   │       ├── desktop.rs      # Fuses game.love with Win/macOS/Linux binaries (11.5 or 12.0+)
│   │       ├── apple_mobile.rs # Packages iOS & iPadOS (.xcodeproj / unsigned or signed .ipa)
│   │       ├── android.rs      # Packages Android (.apk / .aab via Gradle + keystore signing)
│   │       ├── web.rs          # Packages love.js WASM bundle + static HTML
│   │       └── portmaster.rs   # Generates PortMaster zip (port.json, .sh, .gptk, gamedata/)
│   └── resources/              # Bundled default LÖVE 11.5 binaries, LuaCATS defs, shims, templates
└── src/                        # React + TypeScript Frontend
    ├── components/
    │   ├── EditorPane.tsx      # Monaco instance, inline color swatches, asset hover previews
    │   ├── Sidebar/            # FileTree, LibraryManager, GitPanel, ExportCenter, AutoPortPanel, ToolchainPanel
    │   ├── BottomDock/         # DebugConsole (clickable Lua stack traces), Profiler
    │   └── DocsDrawer.tsx      # Searchable offline LÖVE API reference (version-aware) + snippet inserter
    └── data/
        └── love-api-11.5.json  # Bundled default LÖVE 11.5 docs (12.0+ stored in ~/.joyride/toolchains/)
```

---

## 3. Core Feature Specifications

### A. Multi-Version Runtime & Self-Updating Toolchain (`runtime_manager.rs`)
1. **Zero-Config Default (`11.5`) + Seamless `12.0+` Reconfiguration:**
   - New projects default to `love_version = "11.5"` in `love.toml` and `t.version = "11.5"` in `conf.lua` using bundled offline assets.
   - Switching a project's target version to `"12.0"` (via `love.toml` or the status-bar version picker) automatically reconfigures:
     - Active `love` runner executable and desktop/mobile/web/PortMaster export templates.
     - `.luarc.json` `workspace.library` path to load LÖVE 12.0 LuaCATS definitions.
     - `DocsDrawer` API reference dataset (`love-api-12.0.json`).
     - Version-specific linter rules (e.g., enforcing `11.5` rules vs. enabling `12.0` APIs such as `love.graphics.newTexture`, updated `love.math` / shader APIs, and `12.0` deprecation warnings).
2. **Self-Updating Toolchain & IDE:**
   - **IDE Updates:** Uses `tauri-plugin-updater` against GitHub Releases to keep JoyrIDE binaries up to date.
   - **Framework & Definitions Sync:** Periodically checks a signed toolchain manifest (`manifest.json` hosted on GitHub) to download or update LÖVE `12.0+` runtime binaries, `love-api` JSON/LuaCATS type definitions, `love.js` builds, and mobile export templates into `~/.joyride/toolchains/<version>/` without requiring a full IDE reinstall.

### B. Editor, Linting & Contextual Help
1. **LSP & Diagnostics:**
   - Auto-generate `.luarc.json` in project workspace pointing `workspace.library` to the active version's LÖVE LuaCATS definitions and `./libs`.
   - Run `luacheck` on save/change with `love` read-only globals enabled; emit version-calibrated warnings (e.g., flagging `setColor` values `> 1.0` or boolean `t.window.vsync` in `11.5+`, and flagging `11.x`-removed APIs when targeting `12.0`).
2. **Inline Visual Decorators (Monaco Providers):**
   - `DocumentColorProvider`: Parse `love.graphics.setColor(r, g, b, a)` (`0.0–1.0` floats) and render native Monaco inline color pickers.
   - `HoverProvider`: On hover over `love.*` symbols, render Markdown docs from the active version's `love-api-<version>.json` (displaying version badges like `Added in 12.0` or `Deprecated in 12.0`). On hover over asset path strings (`*.png`, `*.ogg`, `*.wav`), resolve relative to project root and display image thumbnail/dimensions or HTML5 `<audio>` preview.
3. **Offline Docs Drawer:**
   - Instant fuzzy search over the active LÖVE version's API (modules, functions, callbacks, enums) with one-click `"Insert Snippet"` into active Monaco cursor.

### C. Runtime & Debug Console
- **Execution (`runner.rs`):** Spawns the project's configured LÖVE executable (`11.5` or `12.0+`) targeting project path (or builds a temporary patched overlay when testing target-specific shims).
- **Stack Trace Linking:** Regex-parse stderr lines matching `Error: ([^:]+):(\d+): (.*)` and `\[string "([^"]+)"\]:(\d+):` into clickable links that open the file and jump to line in Monaco.

### D. Dependency Manager (`love.toml`)
- **Manifest Schema (`love.toml`):**
  ```toml
  [project]
  name = "my-game"
  bundle_id = "dev.joyride.mygame"
  version = "1.0.0"
  love_version = "11.5" # Defaults to "11.5"; switchable to "12.0"

  [dependencies]
  hump = { git = "https://github.com/vrld/hump", tag = "v0.4" }
  bump = { git = "https://github.com/kikito/bump.lua", rev = "964d299" }
  ```
- **Resolution (`pkg_manager.rs`):** Fetches repositories into `./libs/<pkg_name>/` and writes `love.lock`. UI filters/badges curated libraries by compatible LÖVE version (`11.5`, `12.0`, or both) + supports custom Git URL installs.

### E. GitHub Integration
- Authenticate via GitHub Device Flow OAuth (store token in OS keychain via `keyring` crate).
- Scaffold `.gitignore` (ignoring `build/`, `*.love`, `.DS_Store`), stage/commit/push/pull, and optionally generate `.github/workflows/release.yml` parameterized with the project's configured `love_version` for automated multi-platform builds.

---

## 4. Non-Destructive Auto-Porting & Auto-Patching Engine (`auto_porter.rs`)

Designed so developers can take **any existing desktop-first LÖVE project** and ship it to PortMaster handhelds, iOS/iPadOS, Android, macOS, and Web **without modifying a single line of original source code**.

### A. Overlay Patch Architecture (Zero-Mutation Source Tree)
- Original project files (`main.lua`, `conf.lua`, `src/**`) remain 100% untouched.
- All target-specific adaptations are stored as isolated **Patch Overlays** inside `.joyride/patches/<target>/` (trackable in Git):
  ```text
  .joyride/
  └── patches/
      ├── portmaster/
      │   ├── patch.toml          # Declarative rules, detected input bindings, resolution mode
      │   ├── _jr_shim.lua        # Auto-generated runtime hook (scaling, mouse/gamepad bridge)
      │   ├── game.gptk           # Auto-generated gptokeyb2 controller-to-keyboard map
      │   └── overrides/          # Optional drop-in file replacements (e.g., low-res shaders)
      ├── mobile/                 # Shared iOS / iPadOS / Android virtual gamepad & safe-area shim
      │   ├── patch.toml
      │   └── _jr_shim.lua
      └── web/                    # love.js async FS & unsupported-API stub shim
          ├── patch.toml
          └── _jr_shim.lua
  ```
- **Build-Time Virtual Merge (`love_archive.rs`):** When packaging for `<target>`, the bundler copies the source tree to a staging temp directory, applies unified `.patch` diffs (if any custom overrides exist), injects `_jr_shim.lua` at the top of `main.lua` via a lightweight wrapper (`require("_jr_shim"); require("_orig_main")`), and patches `conf.lua` values in memory before zipping `game.love`.

### B. Automated Static Analysis & Codebase Scanner
Before exporting (or on demand in the **Auto-Port Panel**), `auto_porter.rs` scans all `.lua` files using `full-moon` (Rust Lua AST parser) to detect platform blockers and input usage:
1. **Input Extraction:** Indexes every key constant in `love.keyboard.isDown(...)`, `love.keypressed(key)`, `love.keyreleased(key)`, mouse button checks (`love.mouse.isDown`), and `love.gamepadpressed` / `love.joystick` calls.
2. **Resolution & Scaling Detection:** Reads `conf.lua` (`t.window.width`, `t.window.height`, `t.window.resizable`, `t.window.highdpi`) and flags hardcoded screen pixel math (`love.graphics.getWidth()`).
3. **API Compatibility Checks:**
   - **PortMaster (ARM64 Linux / GLES):** Flags desktop-only X11 assumptions, `love.mouse` cursor dependencies (no physical mouse on handhelds), and shaders requiring desktop OpenGL features incompatible with OpenGL ES 2.0/3.0.
   - **iOS / iPadOS / Android:** Flags keyboard-only controls (`love.keyboard`), missing touch handlers (`love.touchpressed`), and `t.window.fullscreen` configuration.
   - **Web (`love.js`):** Flags `require("ffi")`, `love.thread.newThread`, and blocking `io.popen` / `os.execute` calls.

### C. Target-Specific Auto-Shim Capabilities (`_jr_shim.lua`)
1. **Universal Viewport & Aspect-Ratio Auto-Scaler:**
   - Intercepts `love.graphics.getDimensions()`, `getWidth()`, `getHeight()`, and wraps `love.draw()` in an automatic letterboxed/integer-scaled virtual canvas matching the game's native design resolution (e.g., scaling a `1280x720` or `800x600` desktop game cleanly onto a `640x480` 4:3 Anbernic screen or `2732x2048` iPad screen, while translating `love.mouse.getPosition()` / `love.touch` coordinates into virtual space).
2. **PortMaster Auto-Porting & Input Mapping Pipeline:**
   - **Automatic `.gptk` Generation:** Maps all AST-discovered `love.keyboard` keys onto standard PortMaster handheld controls (`up/down/left/right` -> D-pad & Left Analog, `a/b/x/y` -> primary action keys like `z/x/space/return`, `l1/r1/l2/r2` -> `lshift/tab/q/e`, `start/select` -> `return/escape`).
   - **Right-Stick Virtual Mouse Mode:** If AST analysis detects `love.mouse.getPosition` or `love.mousepressed`, the shim automatically injects a hardware-rendered cursor controlled by the right analog stick (`Right Analog` -> cursor delta, `R1`/`A` -> left click, `L1`/`B` -> right click) and suppresses desktop `love.mouse.setVisible(false)` issues.
   - **Native `love.joystick` Fallback & Hotkey Exit:** Injects `Select + Start` clean quit (`love.event.quit()`) if the game lacks a controller-accessible exit menu.
   - **Interactive Override UI:** Users can inspect/tweak auto-mapped buttons in a visual handheld diagram or edit `.joyride/patches/portmaster/game.gptk` directly.
3. **iOS, iPadOS & Android Auto-Touch Overlay:**
   - If a game uses `love.keyboard` without `love.touchpressed`, `_jr_shim.lua` injects a translucent, customizable multi-touch virtual D-pad + A/B/Start overlay that fires synthetic `love.keypressed` / `love.keyreleased` events and drives `love.keyboard.isDown()` seamlessly.
4. **Web (`love.js`) Compatibility Shim:**
   - Stubs unsupported `ffi` / OS calls safely and wraps `love.filesystem` writes to trigger `FS.syncfs` persistence in IndexedDB.

---

## 5. Multi-Platform Export Pipeline
All targets select the active toolchain matching `love.toml` (`11.5` or `12.0+`) and invoke `auto_porter.rs` to merge `.joyride/patches/<target>/` into a validated `build/<slug>-<target>.love` archive.

1. **Desktop (`desktop.rs`):**
   - **Windows:** Concatenate version-matched `love.exe` + `game.love` -> `<slug>.exe`, inject `.ico` resource, copy DLLs to `build/win64/`.
   - **macOS (`arm64` + `x86_64` Universal):** Clone version-matched `love.app` -> `<slug>.app`, place `game.love` in `Contents/Resources/`, patch `Info.plist` (`CFBundleIdentifier`, `CFBundleName`, `CFBundleIconFile` generated `.icns`), apply ad-hoc or Apple Developer ID `codesign`, and optionally package into `.dmg`.
   - **Linux:** Concatenate version-matched `love.AppImage` + `game.love` -> `<slug>.AppImage` (`chmod +x`).

2. **iOS & iPadOS (`apple_mobile.rs`):**
   - Clones version-matched LÖVE iOS Xcode project template, embeds patched `game.love` into app bundle resources, generates AppIcon asset catalog (`1024x1024` source -> all iPhone/iPad icon sizes), and configures `Info.plist` (`UIDeviceFamily` supporting iPhone `1`, iPad `2`, or Universal `1,2`, orientation locks, and `UIRequiresFullScreen`).
   - **Build Outputs:**
     - **Cross-Platform (Win/Linux/macOS):** Generates ready-to-build Xcode project archive (`build/ios/<slug>-xcode.zip`) for CI or Mac transfer.
     - **macOS Host (with Xcode CLI tools):** Invokes `xcodebuild` to produce simulator `.app`, unsigned `.ipa` (for AltStore/SideStore sideloading), or signed App Store `.ipa` using local provisioning profiles.

3. **Android (`android.rs`):**
   - Clones version-matched `love-android` Gradle wrapper template, injects patched `game.love` into `app/src/main/assets/game.love`, patches `AndroidManifest.xml` (`package`, `android:label`, screen orientation, immersive fullscreen flags), and generates adaptive `mipmap-*` launcher icons.
   - **Signing & Outputs:** Built-in Keystore Generator/Selector UI (stores keystore password in OS keychain); runs `./gradlew assembleRelease` -> `build/android/<slug>.apk` and `./gradlew bundleRelease` -> `build/android/<slug>.aab` (Google Play ready).

4. **Web (`web.rs`):**
   - Applies `web` shim overlay and packages `game.love` into version-matched `love.js` WASM distribution (`index.html`, `love.js`, `love.wasm`, `game.data`).

5. **PortMaster Handhelds (`portmaster.rs`):**
   - Applies `portmaster` overlay (`_jr_shim.lua` + `game.gptk` + `port.json` + `<Slug>.sh`), dynamically sets the PortMaster runtime (`love_11.5` or `love_12.0`), and outputs `build/<slug>-portmaster.zip`:
     ```text
     <slug>/
     ├── <Slug>.sh          # Sources $controlfolder/runtimes/love_<ver>/love.txt & gptokeyb2
     ├── port.json          # Metadata, genres, runtime: "love_<ver>"
     ├── game.gptk          # Auto-generated / user-tuned controller-to-key map
     └── gamedata/
         └── game.love      # Contains original source + injected _jr_shim.lua
     ```

6. **Mobile & Handheld Viewport & Shim Simulator (Editor Preview):**
   - Overlay selector in the Runner toolbar to run the project **with any target's patch overlay active** inside simulated screen dimensions, DPI scaling, and safe-area guides for **PortMaster (`640x480` 4:3, `720x720` 1:1, `1280x720` 16:9)**, **iPhone**, **iPad**, and **Android**, plus controller/touch event emulation.

---

## 6. Implementation Milestones (Execution Order)
1. **Scaffold, Toolchain Manager & Core Editor:** Initialize Tauri v2 + React + Monaco (`joyride`); implement `runtime_manager.rs` (defaulting to bundled `11.5`, supporting `12.0+` downloads & `tauri-plugin-updater` self-updates) and wire `lsp_bridge.rs` with `lua-language-server` + version-switched LÖVE LuaCATS types.
2. **Runner & Console:** Implement `runner.rs` process execution, log streaming, clickable error stack traces, and Viewport/Touch/Gamepad Simulator.
3. **LÖVE Tooling & Docs:** Bundle `love-api-11.5.json` + remote `12.0+` sync, build `DocsDrawer`, inline `setColor` swatches, asset hover previews, and version-aware `luacheck` rules.
4. **Package Manager & GitHub:** Implement `love.toml` resolver (`pkg_manager.rs`) and Git/GitHub OAuth panel.
5. **Auto-Porter & Patch Engine:** Implement `auto_porter.rs` (`full-moon` Lua AST scanner), `.joyride/patches/` overlay staging, `_jr_shim.lua` (viewport scaler, virtual mouse, virtual touch controls), and `.gptk` auto-mapper UI.
6. **Multi-Platform Exporters:** Implement Desktop (Win/macOS/Linux), PortMaster (`.zip`), Web (`love.js`), iOS/iPadOS (`apple_mobile.rs`), and Android (`android.rs`) builders using the project's active LÖVE toolchain.