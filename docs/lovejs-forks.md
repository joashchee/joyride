# love.js forks (surveyed 2026-11-13)

JoyrIDE's web exporter (`joyride-core/src/web.rs`) packages a game with a
love.js template. The template it was written against, `Davidobot/love.js`
(LÖVE 11.5, LuaJIT), has had no commit since **2024-05-13**. This is what the
fork network has since. Re-run `scripts/check-lovejs-forks.py` (weekly in CI)
and update this file.

Method: github.com's fork-network page (73 forks), each fork blob-less cloned
(history only; nothing run), every branch compared with Davidobot's tip.
**None of this was run in a browser, and no engine was rebuilt**: what each
fork does is from its commit messages, its readme and its files.

| Fork (branch) | Last commit | Ahead | What it adds |
|---|---|---|---|
| **pkhead/love.js** (`lua52`) | 2026-10-22 | +22 | The biggest change. **Lua 5.2 instead of LuaJIT** (adds `goto` and `bit32`; breaks LuaJIT-only code, `webcompat/` has reverse-polyfills that must be `require`d at the top of `conf.lua`), **Emscripten 6.0.1**, WASM exceptions (LÖVE errors work properly), `Module.AL` exported, audio paused when the tab loses focus, a way to link C libraries and build a custom runtime. Uses a LÖVE fork (`goodpants-games/love`, branch `emscripten-ext`). A second branch, `lua52-emcc4.0.19`, is the Emscripten 4 version. |
| dreagledr/love.js (`master`) | 2026-10-15 | +6 | An async **`http` Lua module** on the Emscripten Fetch API, a modernised engine build script and a Docker image, engine binaries rebuilt. |
| cptobakhmut925-glitch/love.js (`master`) | 2026-10-09 | +2 | **WebGL2** support and font fixes (a `patches/` folder tracks the changes to LÖVE). |
| jeduden/love.js (`master`) | 2025-11-04 | +8 | Mobile-responsive layout, fullscreen fixes for mobile, canvas resize, a `serve` option on the CLI. |
| itsthisjustin/love.js, dev10/wasm-love.js | 2025-10 / 2026-01 | +4 / +6 | Emscripten 2.0.34+ support and a **mobile keyboard**, canvas resize. |
| DVDAGames/lovely.js (`main`) | 2026-06-28 | +2 | Shims and a basic `index.html`, towards a toolchain. |
| pluma9, phiodetdiao-dot, 26F-Studio, tiomke, others | various | +1 to +11 | Small patches, readme changes, a game's `main.lua`; nothing engine-level. |

## What JoyrIDE does with this

- **Both layouts package.** The exporter reads Davidobot's template layout
  and the `lua52` fork's (one shared `html/index.html` and `html/theme/`); both
  real checkouts package in tests (`JOYRIDE_LOVEJS=<checkout>`).
- **Davidobot's stays the default.** It runs LuaJIT, the same Lua as desktop
  LÖVE 11.5, so a game behaves the same on the web as on every other export.
  The `lua52` fork is the newer engine but a different Lua; adopting it means
  the web build differs from every other export unless the project goes through
  its `webcompat` layer. That is a choice for the maintainer, not a drift to
  absorb silently.
- **Open:** try the `lua52` fork in a real browser with a real game, and decide
  whether to offer it as an option (with `webcompat` staged into the `.love`
  and required first in `conf.lua`). Take the WebGL2, mobile-keyboard and
  responsive-layout patches only after reading them.

## Saved games (corrected)

An earlier note here said love.js does not persist saves. **It does**: its own
startup code (in `love.js`, in every template) mounts IndexedDB at
`/home/web_user/love`, loads it before the game starts and syncs it back on
`beforeunload`. The limit is that nothing else triggers a sync: a tab that dies
without `beforeunload` (mobile browsers, a crash) can lose the latest writes,
and the page cannot sync sooner because `FS` is not exported.
