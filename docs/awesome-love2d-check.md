# awesome-love2d, checked against the curated list

Checked 2026-11-14 against [love2d-community/awesome-love2d](https://github.com/love2d-community/awesome-love2d) at `8477dcd` (2026-09-26), the list the current LÖVE jam pointed to.
Matched by repository address and name. The list has 260 library entries (tutorials, IDEs, distribution and related links left out): **43 are in `joyride-core/data/libraries.toml`** (hump's four modules count as `hump`), **217 are not**.

Nothing here has been audited, probed or licence-checked: a name on this list is only a candidate. Rule 14 (`CLAUDE.md`) applies before any is added: `scripts/audit-libraries.py`, read the findings, then `curate_libraries`, the Benchmark sample and the ingest. `windfield` is excluded for good (`joyride-core/data/library-exclusions.toml`); `MLib` is windfield's bundled math library, whose older copies carry GPL-2.0-or-later headers, so read its licence at the pinned commit first.

## Ours, on the list

- anim8 (Animation)
- baton (Input)
- batteries (Helpers)
- binser (Serialization)
- bitser (Serialization)
- Bump (Physics)
- cargo (Utilities)
- cimgui-love (UI)
- Cirno's Perfect Math Library (Math)
- classic (OO)
- Concord (Entity)
- Flux (Tweening)
- g3d (3D)
- gamera (Camera)
- HC (Physics)
- hump (Helpers)
- hump.camera (Camera)
- hump.class (OO)
- hump.timer (Tweening)
- hump.vector (Math)
- i18n (Utilities)
- Jumper (AI)
- log.lua (Utilities)
- love-loader (Utilities)
- Lovebird (Development)
- luasteam (Utilities)
- lume (Helpers)
- lurker (Development)
- lust (Testing)
- Löve-Nuklear (UI)
- middleclass (OO)
- Moonshine (Shaders)
- nativefs (Utilities)
- Peachy (Animation)
- Push (Drawing)
- ripple (Music)
- roomy (Helpers)
- Simple Tiled Implementation (Helpers)
- Slab (UI)
- Sock.lua (Networking)
- SUIT (UI)
- tiny-ecs (Entity)
- tween.lua (Tweening)

## Ours, not on the list

`Concord` (listed under its old address), `HC`, `cron`, `i18n`, `inspect`, `json`, `lua-discordRPC`, `lua-ljsqlite3`, `luajit-request`, `luaunit`, `neumetik-midi`, `profile`, `semver`, `serpent`, `sqlua`, `talkies`, `xsys`. Several are listed under other addresses or not at all.

## On the list, not curated

### 3D

- [3DreamEngine](https://github.com/3dreamengine/3DreamEngine) - An awesome 3D engine for LÖVE.
- [anim9](https://github.com/excessive/anim9) - 3D skeletal animation library (design to be used with IQM and IQE).
- [Brinevector3D](https://github.com/flamendless/brinevector3D) - FFI-enabled vector library for 3D (x,y,z).
- [bump-3dpd](https://github.com/oniietzschan/bump-3dpd) - A 3D collision detection library for Lua.
- [IQE](https://github.com/excessive/iqe) - Inter-Quake Export loader (text).
- [IQM](https://github.com/excessive/iqm-exm) - Inter-Quake Model loader (binary).
- [love-gltf](https://gitlab.com/Alloyed/love-gltf) - Gltf asset loader and test renderer; Supports skeletal animations and morph targets.
- [Lovox](https://github.com/Keyslam/Lovox) - Pseudo-3D library for working with voxels.
- [Menori](https://github.com/rozenmad/Menori) - Library for 3D rendering with LÖVE.

### AI

- [astar](https://github.com/xiejiangzhi/astar) - An other A* library. Any map (grid, point, mesh or infinite map) and support path cost.
- [beehive.lua](https://github.com/drhayes/beehive.lua) - A functional behavior tree implementation.
- [FluadHTN](https://github.com/Safebox36/OdaiHTN) - A powerful hierarchical task network for AI planning.
- [LÖVElyTrees](https://github.com/Nrosa01/LOVElyTrees) - Fully featured behaviour tree implementation with tree rendering.
- [Lua-star](https://github.com/wesleywerner/lua-star) - Easy and pure Lua A* path finding.
- [Luafinding](https://github.com/GlorifiedPig/Luafinding) - Class-based A* implementation written purely in Lua.
- [pathfun](https://codeberg.org/apicici/pathfun) - Pure Lua library for 2D pathfinding using the funnel algorithm.

### Animation

- [andross](https://github.com/pfirsich/andross) - A Lua library for 2D skeletal/bone animations with a Löve backend.
- [animx](https://github.com/besnoi/animX) - A featureful Animation library for loading animations from XML files!.
- [Aseprite loader](https://github.com/elloramir/love-ase) - Complete ase/aseprite file loader (no json).
- [chiro](https://github.com/bjornbytes/chiro) - Convenience wrapper around [Spine](http://esotericsoftware.com).
- [lovanim](https://github.com/patrixr/love-animation) - A minimal stateful animation library.
- [Lump](https://github.com/sixFingers/lump) - Adobe Flash animation runtime.
- [skeletor](https://github.com/pelevesque/skeletor) - 2D skeletal animation system.
- [SYSL-Text](https://github.com/sysl-dev/SYSL-Text) - Text animation and automatic wrapping library based on tags.
- [Walt](https://github.com/davisdude/Walt) - Animation library.

### Camera

- [Brady](https://github.com/davisdude/Brady) - Camera library with parallax scrolling.
- [Editgrid](https://github.com/bakpakin/Editgrid) - Gamera and HUMP compatible scaling grid.
- [parallax](https://github.com/idbrii/love-parallax) - Scrolling library for any camera system; seamlessly tile background images.
- [roomshift](https://github.com/mshiplet/roomshift) - Room-based camera with smooth transitions.

### Development

- [debugGraph](https://github.com/Mechazawa/Love-Debug-Graph) - Small OO FPS graphing utillity based on FPSGraph.
- [FPSGraph](https://github.com/icrawler/FPSGraph) - Small FPS graphing utility.
- [LÖVE API](https://github.com/love2d-community/love-api) - The complete API documentation of LÖVE in a Lua table.
- [LÖVE Build](https://github.com/nhartland/love-build) - GitHub Action for automated cross-platform builds.
- [LÖVE TypeScript Definitions](https://github.com/hazzard993/love-typescript-definitions) - Write LÖVE games with TypeScript.
- [LoveDebug](https://github.com/flamendless/lovedebug) - Inline console-like debugger utility.
- [lovelier](https://github.com/patrixr/lovelier) - A LÖVE live reloader with Moonscript support.
- [loveprofiler](https://github.com/dknight/loveprofiler) - An extremely simple logger and profiler.
- [vudu](https://github.com/deltadaedalus/vudu) - Broad in-game debugging gui with a console, variable browser/editor, speed controls, and more.

### Drawing

- [Artal](https://github.com/EvineDev/Artal) - A .PSD parsing library for LÖVE.
- [Autobatch](https://github.com/rxi/autobatch) - Small LÖVE module to automate the use of SpriteBatches.
- [center](https://github.com/swalrus1/center) - A simple module to dynamically align and fit content to screens of any size.
- [deep](https://github.com/Nikaoto/deep) - Adds Z axis; allows you to queue actions and draw calls.
- [draft](https://github.com/pelevesque/draft) - A module for drawing complex shapes.
- [Export-TextureAtlas](https://github.com/EngineerSmith/Export-TextureAtlas) - Extends Runtime-TextureAtlas as a commandline tool to export an atlas.
- [grove.draworder](https://github.com/FloatingBanana/Grove/blob/master/grove/draworder.lua) - Similar to deep, but you can set a custom sorting method and create multiple, nestable instances.
- [Hot particles](https://github.com/ReFreezed/HotParticles) - "Hot Particles is a particle editor for the LÖVE game framework."
- [HSLUV](https://github.com/hsluv/hsluv-lua) - Lua implementation of HSLuv (a human-friendly alternative to HSL).
- [lovector](https://github.com/nasso/lovector) - A pure-lua vector graphics processing and rendering library.
- [Maid64](https://github.com/adekto/maid64) - Low resolution scaler for LÖVE.
- [renderplanet](https://github.com/meric/renderplanet/) - Realistic orthographic planet rendering.
- [Resolution Solution](https://github.com/grif-on/love2d_resolution_solution) - Scale library, that help you add resolution support to your games!
- [Runtime-TextureAtlas](https://github.com/EngineerSmith/Runtime-TextureAtlas) - Texture atlas packer and renderer at runtime; no external tools.
- [Shöve](https://github.com/Oval-Tutu/shove) - A powerful resolution-handler and rendering library for LÖVE.
- [Sölar](https://github.com/JanWerder/soelar) - A fairly simple solar system simulator.
- [Sunscreen](https://github.com/challacade/sunscreen) - A small, dependency-free virtual-resolution helper for LÖVE.
- [svglover](https://github.com/globalcitizen/svglover) - Library to import and display simple SVGs in LÖVE.
- [SYSL-Pixel](https://github.com/sysl-dev/Sysl-Pixel) - Pixel art focused scaler for graphics, position, screen-captures and shaders.
- [turtle.lua](https://github.com/arthurealike/turtle.lua) - Turtle graphics library for LÖVE.

### Entity

- [adorbs](https://github.com/JosephShering/adorbs) - Minimal, Functional Entity Component System.
- [ecs-lua](https://github.com/nidorx/ecs-lua) - ECS Lua is a fast and easy to use ECS (Entity Component System) engine for game development.
- [evolved.lua](https://github.com/BlackMATov/evolved.lua) - Evolved ECS (Entity-Component-System) for Lua.
- [knife.system](https://github.com/airstruck/knife/blob/master/readme/system.md) - Minimalist functional ECS.
- [nata](https://github.com/tesselode/nata) - Hybrid OOP/ECS entity management.

### Helpers

- [astray](https://github.com/SiENcE/astray) - A Lua-based maze, room and dungeon generation library for dungeon crawlers and roguelike video games.
- [cartographer](https://github.com/tesselode/cartographer) - Small Tiled map loader and drawer.
- [clove](https://github.com/besnoi/clove) - A helper library which allows you to quickly loads huge amount of assets.
- [GameStateManager](https://github.com/GwyrddGlas/GameStateManager) - A lightweight, optimized, and easy-to-implement solution for efficient game state management.
- [grove.color](https://github.com/FloatingBanana/Grove/blob/master/grove/color.lua) - Blend, convert and interpolate colors using 'color objects'.
- [iffy](https://github.com/besnoi/iffy) - A SpriteSheet and Tileset helper library for LÖVE.
- [knife](https://github.com/airstruck/knife) - Collection of useful micro-modules for Lua (Class, State Machines, Bind, Chain, Coroutines, Event, Memoize, Entity, Tests, Timer).
- [Love dialogue](https://github.com/Miisan-png/Love-Dialogue) - Simple to use Dialogue Library for Love2d with custom scripting language for dialogues .
- [love-state-switcher](https://github.com/nekromoff/love-state-switcher) - State switcher class for Lua LÖVE Engine.
- [lovely-windows](https://github.com/flamendless/lovely-windows) - A Window/Screen Manager Module for virtual windows.
- [lua-state-machine](https://github.com/kyleconroy/lua-state-machine) - Lua Finite State Machine.
- [lue](https://github.com/Ulydev/lue) - A LÖVE library that allows you to display hue color effects in your game.
- [Manami](https://github.com/MikuAuahDark/NPad93/blob/master/manami.lua) - An improved "reflowprint" with UTF-8, multicolor, and justify support.
- [narrator](https://github.com/astrochili/narrator) - An Ink narrative scripting language parser and runtime implementation.
- [prism](https://github.com/PrismRL/prism) - A comprehensive traditional roguelike engine.
- [reflowprint](https://github.com/josefnpat/reflowprint) - A library designed for alignment of text that is shown one character at a time.
- [rotLove](https://github.com/paulofmandown/rotLove) - Roguelike Toolkit in LÖVE. A LÖVE/lua port of rot.js.
- [SceneMan](https://github.com/KINGTUT10101/SceneMan) - A simple, but powerful scene/gamestate manager that uses a stack, allowing you to run multiple scenes at once.
- [Scenery](https://github.com/paltze/scenery) - A dead simple scene/state management system.
- [ScreenManager](https://github.com/rm-code/screenmanager) - Screen/State Management for the LÖVE framework.
- [shack](https://github.com/Ulydev/shack) - A LÖVE library that lets you easily add screen effects such as shake and rotation.
- [shard](https://github.com/MineGame159/shard) - Collection of useful mini modules.
- [Vivid](https://github.com/WetDesertRock/vivid) - Color math, manipulation and conversion library.

### Input

- [GamepadGuesser](https://github.com/idbrii/love-gamepadguesser) - Get the right button icons for gamepads; load updated gamepad db.
- [input](https://github.com/xiejiangzhi/input) - Simple and powerful input library. support check operation time, sequence.
- [Kazari](https://github.com/MikuAuahDark/Kazari) - Multitouch gesture and input library.
- [love-microphone](https://github.com/LPGhatguy/love-microphone) - Simple microphone support for LÖVE.
- [LoveKeys](https://github.com/SpaceCat-Chan/LoveKeys) - A simple input handeling library that keeps track of things for you.
- [Swipe](https://github.com/zombrodo/swipe) - A simple radial keyboard library.
- [tactile](https://github.com/tesselode/tactile) - A straightforward and flexible input library.

### Lighting

- [bitumbra](https://github.com/a13X-B/bitumbra) - GPU accelerated many lights 2D shadows.
- [Light](https://github.com/xiejiangzhi/light) - A simple dynamic light implementation.
- [Light World](https://github.com/tanema/light_world.lua) - A lighting model.
- [lighter](https://github.com/speakk/lighter) - A performant dynamic light implementation with a simple API.
- [Shädows](https://github.com/matiasah/shadows) - A Shadows & Lights engine for LÖVE.
- [Simple Love Lights](https://github.com/dylhunn/simple-love-lights) - A very simple raycasting light implementation.

### Math

- [Bresenham](https://github.com/rm-code/Bresenham) - Bresenham's line algorithm written in Lua.
- [brinevector](https://github.com/novemberisms/brinevector) - Standalone lightweight luajit ffi-accelerated 2D vector library for great performance.
- [delaunay](https://github.com/Yonaba/delaunay) - Triangulation library for convex polygons.
- [loaded_dice](https://github.com/a13X-B/loaded_dice) - Walker-Vose alias method implementation (loaded dice-like RNG) useful for rolling loot tables, etc.
- [MLib](https://github.com/davisdude/mlib) - Math and shape-intersection detection library written in Lua. It's aim is to be robust and easy to use.
- [nvec](https://github.com/MikuAuahDark/NPad93/blob/master/nvec.lua) - Hump.vector-compatible LuaJIT FFI-accelerated 2D vector library.
- [polysec](https://github.com/dknight/polysec) - A performant and lightweight library to detect polygon and rectangle intersections.
- [shash](https://github.com/rxi/shash) - A simple, lightweight spatial hash for Lua.
- [vector.lua](https://github.com/automattf/vector.lua) - A simple vector library based on the PVector class from processing.
- [Vornmath](https://github.com/DUznanski/vornmath) - The most comprehensive small vector & matrix, complex number, and quaternion library for Lua.

### Music

- [denver](https://github.com/superzazu/denver.lua) - A Löve custom waveform generation library.
- [lovebpm](https://github.com/rxi/lovebpm) - A LÖVE library for syncing events to the BPM of an audio track.
- [wave](https://github.com/Ulydev/wave) - A sound manager with audio parsing and rhythm functionalities.
- [Lua-Opus](https://github.com/GwyrddGlas/Lua-Opus) - A module for streaming and decoding Ogg Opus audio files with queueable playback and PCM integration.

### Networking

- [fetch-lua](https://github.com/elloramir/fetch-lua) - An HTTPS/HTTP requests library made only with luajjit.
- [Grease](https://github.com/bartbes/love-misc-libs/tree/master/grease) - Networking library intended to make networking easy for lovers (TCP, UDP, Enet).  (IPv6)
- [löve-ws](https://github.com/holywyvern/love-ws) - A WebSocket client and server library.
- [love2d-lua-websocket](https://github.com/flaribbit/love2d-lua-websocket) - A simple event-driven WebSocket client library. (IPv4)
- [LoverNet](https://github.com/josefnpat/LoverNet) - A networking library that leverages bitser and enet. (IPv4)
- [NoobHub](https://github.com/Overtorment/NoobHub) - OpenSource multiplayer and network messaging. (IPv6)

### OO

- [30log](https://github.com/Yonaba/30log) - Minified framework for object-orientation in Lua. It features named (and unnamed) classes, single inheritance and a basic support for mixins.
- [knife.base](https://github.com/airstruck/knife/blob/master/readme/base.md) - Extremely minimal base class providing single inheritance and constructors.
- [LowerClass](https://github.com/DevonPalma/LowerClass) - A MiddleClass Inspired library with extended features.
- [muun](https://github.com/megagrump/muun) - Moonscript compatible class implementation.
- [Object-Library](https://github.com/Virus01Official/Object-Library) - ObjectLibrary is a simple library for the LÖVE2D game engine that allows users to create objects with collision detection and optional textures.
- [selene](https://github.com/novafacing/selene) - Project template for writing games in Moonscript instead of Lua without precompiling.

### Performance

- [AppleCake](https://github.com/EngineerSmith/AppleCake) - Profiling library for LÖVE, with detailed profiling and thread support.
- [jprof](https://github.com/pfirsich/jprof) - Profiling library/tool for LÖVE.
- [Piefiller](https://github.com/Polynominal/Piefiller) - Graphical profiler for LÖVE.

### Physics

- [breezefield](https://github.com/HDictus/breezefield) - A lightweight and easy love.physics wrapper (windfield alternative).
- [loveblobs](https://github.com/zhangf911/loveblobs) - A softbody library with support for both dynamic and static arbitrary softbodies.
- [slick](https://github.com/erinmaus/slick/) - A simple to use polygon collision library inspired by bump.lua.
- [Strike](https://github.com/Aweptimum/Strike) - 2D collision detection library. Extendable, based on Separating-Axis-Theorem.

### Platforms

- [LÖVE Potion](https://github.com/lovebrew/lovepotion) - Unofficial implementation of the LÖVE for Nintendo (3DS, Switch and Wii U) Homebrew.
- [LOVE-WrapLua](https://github.com/LukeZGD/LOVE-WrapLua) - A small and simple wrapper for OneLua, lpp-vita, and Lua Player PS3.
- [lovejs-player](https://codeberg.org/alexjgriffith/lovejs-player) - LÖVE ported to the web using Emscripten.
- [LoveDos](https://github.com/rxi/lovedos) - A Lua framework for 2D DOS games, implementing a subset of the LÖVE API.
- [LoveFTW](https://bitbucket.org/T-BoneISS/l-veftw) - Work-in-progress port to Windows phone 8.1.
- [LÖVR](https://github.com/bjornbytes/lovr) - LÖVE for virtual reality devices.
- [Lutro](https://lutro.libretro.com/) - Portable game engine implementing a subset of the LÖVE API for retro games, runnable through RetroArch on many platforms.

### Serialization

- [arson](https://github.com/flamendless/arson.lua) - Companion library for registering custom data types that can be encoded and decoded by json.lua.
- [cdata](https://github.com/excessive/cdata) - Serialize between Lua data and C data using LuaJIT's FFI.
- [knife.serialize](https://github.com/airstruck/knife/blob/master/readme/serialize.md) - Serialize data as a Lua script.
- [Lady](https://github.com/gvx/Lady) - Saving and loading savegames; based on Ser.
- [moonblob](https://github.com/megagrump/moonblob) - Binary serialization and data parsing library.
- [Ser](https://github.com/gvx/Ser) - Fast, robust, richly-featured table serialization library for Lua.
- [Smallfolk](https://github.com/gvx/Smallfolk) - A fast, robust, secure, richly-featured table serialization library for Lua.
- [trickle](https://github.com/bjornbytes/trickle) - A bitstream library focused on high compression for use in networking.

### Shaders

- [LoveShaderConverter](https://github.com/tsteinholz/LoveShaderConverter) - Convert Shadertoy Shader files to LÖVE GLSL Files with handy utilities for infinite purposes.
- [ngrading](https://github.com/MikuAuahDark/NPad93/tree/master/ngrading) - Simple color grading library.
- [ShaderScan](https://github.com/idbrii/love-shaderscan) - Adds hot reload, includes, and better error messages for faster shader iteration.

### Testing

- [busted](https://github.com/lunarmodules/busted) - Simple unit-testing framework with customizable assertions.
- [knife.test](https://github.com/airstruck/knife/blob/master/readme/test.md) - Fixture-free test framework.
- [Luassert](https://github.com/lunarmodules/luassert) - Extends `assert()` with additional, customizable tests.
- [Lunatest](https://github.com/silentbicycle/lunatest) - xUnit-style randomized unit testing framework.
- [Specl](http://gvvaughan.github.io/specl/) - Behavior Driven Development (BDD) tool.
- [Telescope](http://norman.github.io/telescope/) - Highly-customizable BDD-style testing library.

### Tweening

- [knife.timer](https://github.com/airstruck/knife/blob/master/readme/timer.md) - Create timers and tweens with ease.
- [NAniTe](https://github.com/MikuAuahDark/NPad93/blob/master/nanite.lua) - Timeline-based animation system that supports forward and backward update.
- [tick](https://github.com/rxi/tick) - Lua module for delaying function calls.

### UI

- [Badar 🌕](https://github.com/Nabeel20/Badr) - Simple custom UI syntax for easy components compositions.
- [CatUI](https://github.com/wilhantian/catui) - A very light-weight GUI library for LÖVE.
- [FlexLöve](https://github.com/mikefreno/FlexLove) - A fully featured GUI library, supporting flexbox/grid/absolute layouts, both immediate and retained mode, 9patch theming and advanced event support.
- [GOOi](https://github.com/gustavostuff/gooi) - Android-oriented GUI library.
- [Hanker](https://gitlab.com/Alloyed/hanker) - Maximalist immediate mode-ish UI elements; gamepad-friendly.
- [Helium](https://github.com/qeffects/helium) - A modern, customizable, high performance retained UI framework.
- [Inky](https://github.com/Keyslam/Inky) - Any-purpose GUI framework.
- [Layouter](https://github.com/nekromoff/layouter) - A simple UI grid layout library for LÖVE 2D game engine.
- [likelihud](https://github.com/LRDPRDX/LikeliHUD) - A simple UI library with a declarative QML-like approach of defining UI.
- [ListBox](https://github.com/darkmetalic/ListBox) - A dynamic ListBox for LÖVE that supports touch, mouse, and keyboard inputs.
- [Love Imgui](https://github.com/slages/love-imgui) - Imgui module for the LÖVE game engine.
- [LoveFrames Fork](https://github.com/linux-man/LoveFrames) - 11.2 Fork of a GUI library.
- [LOVELi](https://github.com/mtanksl/LOVELi) - Retained mode layout and GUI library inspired by .NET's MAUI controls with StackLayout, AbsoluteLayout, Grid, Label, Button, Image, CheckBox, RadioButton, Switch, ProgressBar, Slider, TextBox and Border.
- [Lovely Toasts](https://codeberg.org/togfox/Lovely-Toasts) - Floating speech bubbles with text of your choice.
- [lovepatch](https://github.com/notcl4y14/lovepatch) - A simple 9-patch library for LÖVE.
- [lovr-ui2d](https://github.com/immortalx74/lovr-ui2d) - An immediate mode GUI library for LÖVR and LÖVE.
- [LuaJIT-ImGui](https://github.com/sonoro1234/LuaJIT-ImGui) - LuaJIT bindings for Dear ImGui with a LÖVE module; supports all cimgui widgets using cpp names for easy example copying.
- [Luis](https://github.com/SiENcE/luis) - Love UI System - A retained mode UI framework for LÖVE with input processing (mouse, keyboard, touch, gamepad), layer-, grid-, state-, theming-system, UI editor, 16+ widgets (for desktop & mobile).
- [Lynx](https://gitlab.com/TSnake41/lynx) - Very-lightweight list-based UI library.
- [MeowUI](https://github.com/MoonGameLab/MeowUI) - Extensible library written in MoonScript that enables you to create your own GUI controls based on provided core modules for Löve2D.
- [NLay](https://github.com/MikuAuahDark/NPad93#nlay) - Flexible layouting library.
- [Patchy](https://github.com/excessive/patchy) - 9patch library.
- [Plan](https://github.com/zombrodo/plan) - A super simple Rule-based layout library.
- [Polywell](https://gitlab.com/technomancy/polywell) - A highly-configurable text editor / coding tool written in Lua that runs on the LÖVE game engine.
- [ProdUI](https://github.com/frank-f-trafton/prod_ui_wip) - A user interface library for the LÖVE Framework.
- [SafeWord](https://github.com/josefnpat/safeword) - An overscan detection library for LÖVE.
- [Slicy](https://github.com/wqferr/slicy) - A newer 9patch/9slice library fixing some issues with Patchy.
- [TuxRedux](https://github.com/KINGTUT10101/TuxRedux) - An immediate-mode UI system for LOVE2D inspired by SUIT.
- [YALG](https://github.com/sasszem/yalg) - A very simple, cross-platform, reactive UI for last minute UIs.

### Utilities

- [andromeda](https://github.com/itzKiwiSky/Andromeda) - Asset database and loader for love.
- [ArrayRotation](https://gist.github.com/rm-code/4118d4a97d8cde16952199d94b84ead0) - Rotation of two dimensional arrays (square and non-square).
- [bar128-love](https://github.com/Nawias/bar128-love) - Barcode rendering library for LÖVE.
- [chance.lua](http://ejmr.github.io/chance.lua/) - Library for generating random data.
- [colorchanger](https://github.com/santoslove/colorchanger) - Change the colors in your LÖVE game.
- [Flirt](https://github.com/Alloyed/flirt) - LÖVE version manager; allows to switch between multiple versions of engine on same machine.
- [GifCat](https://github.com/WetDesertRock/GifCat) - A simple module for saving gifs from LÖVE.
- [hex2color](https://github.com/swalrus1/hex2color) - A single function to use HEX color values.
- [Ini Parser](https://github.com/nobytesgiven/ini_parser) - General purpose ini configuration parser.
- [KeyedArray](https://github.com/KINGTUT10101/KeyedArray) - A special data structure where items can be found by their position or key. Similar to an ordered dictionary.
- [L5](https://l5lua.org/) - A lightweight creative coding library that ports Processing / p5.js into Lua.
- [Lily](https://github.com/MikuAuahDark/lily) - Async Asset Loader.
- [love-pe](https://github.com/Rami-Sabbagh/love-pe) - A tool made in LÖVE for changing LÖVE icon itself.
- [love-qrcode](https://github.com/Nawias/love-qrcode) - QR Code rendering library for LÖVE.
- [love2d-assets-loader](https://github.com/Yonaba/love2d-assets-loader) - Assets Loader.
- [lovely-engine](https://github.com/vinnyhorgan/lovely-engine) - A tool that makes setting up and using LÖVE easier and with Moonscript support.
- [LuaRequireExtended](https://github.com/KINGTUT10101/LuaRequireExtended) - A Lua module that lets you require files with input parameters and multiple output values.
- [Luvent](https://github.com/ejmr/Luvent) - Simple event-driven programming.
- [nativefiledialog](https://github.com/Alloyed/nativefiledialog/tree/master/lua) - Open a file picker on Windows/Mac/Linux.
- [particle-system-playground](https://github.com/santoslove/particle-system-playground) - A basic LÖVE particle system editor.
- [ParticleEditor](https://github.com/MusouCrow/ParticleEditor) - An editor of particle for LÖVE.
- [require.lua](https://github.com/premek/require.lua) - Require all files in a folder.
- [servelove](https://github.com/YellowButShort/servelove/) - A web server library that runs on love.
- [ShakeDetectorLua](https://github.com/azoyan/ShakeDetectorLua) - Uses accelerometer data for shake device detection written in Lua.
- [smiti18n](https://github.com/Oval-Tutu/smiti18n) - A very complete internationalization library for Lua with LÖVE support.
- [splashy](https://github.com/videah/splashy) - Splash Screen Library.
- [text2.love](https://git.sr.ht/~akkartik/text2.love) - Full-featured text editor for prose (not code).
- [tick](https://github.com/bjornbytes/tick) - Useful timing tweaks for LÖVE's run loop.
- [LUA-SNG-Parser](https://github.com/GwyrddGlas/LUA-SNG-Parser) - A Lua library for parsing and extracting custom .sng song package files, supporting metadata reading, file listing, and batch extraction. Ideal for rhythm games, audio-based projects.
