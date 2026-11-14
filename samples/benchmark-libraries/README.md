# Benchmark Libraries

A sample project that ships with JoyrIDE: it loads every library JoyrIDE offers, lines up the ones that do the same job in tables of features, speed and cost, rates each cell out of 5 (and lets you rate it yourself), marks the best library in every row, and writes a report for the platform it runs on. Open it from **Samples…** in the toolbar, press **Run the whole benchmark**, then open a comparison.

It is the library counterpart of the IO Checker: the same 48 by 25 text-mode screens, the same keys, pad, mouse and touch, the same platform detection (macOS, Windows, Linux, PortMaster, iOS, iPadOS, Android, web), the same report grammar (`PASS`, `FAIL`, `INFO`, `SKIP`).

## What it compares

All 50 curated libraries are in `libs/`, each at its pinned commit (`love.toml`, `love.lock`; `tools/vendor-libs.py` rebuilds them from `joyride-core/data/libraries.toml` and drops docs, pictures, tests and compiled files). The ones that overlap are compared with the same task, written the way each library wants:

| Comparison | Libraries | What is rated (features, then speed or size) |
|---|---|---|
| Classes (OOP) | middleclass, classic, hump.class, batteries.class | Define + new; Inherit + super; Type test (is-a); Mixins; __tostring etc.; Create objects; Method calls |
| Tweening | tween, flux, hump.timer | Tween a number; Named easing; Nested fields; Completion; Delay and chain; Stop a tween; Run 100 tweens |
| Timers | hump.timer, batteries.timer, cron | After a delay; Repeat; Repeat N times; Progress; Cancel; Wait in a script; 1000 timers |
| Events and messages | hump.signal, batteries.pubsub | Send + arguments; Remove a listener; Listen once; Wildcards; Change mid-send; 10000 sends |
| Game states | hump.gamestate, batteries.state_machine, roomy | Enter / leave; Pass arguments; Update current; Push and pop; Self-transition; Ask where we are; 20000 switches |
| 2D vectors | hump.vector, vector-light, batteries.vec2, cpml.vec2 | Add, scale; Length, distance; Normalise; Rotate; Dot, project; Interpolate; 1000 steps; Garbage made |
| Arrays and tables | lume, batteries | map; filter; reduce; find a value; slice; count / any / all; Remove duplicates; Group / partition; Zip; Shuffle; Copy (shallow); Copy (deep); Merge; Keys; map+filter+reduce; Copy 1000 |
| Maths helpers | lume, mathx | Clamp; Round; Lerp; Smoothstep; Ping-pong; Wrap; Remap a range; Distance, angle; Sign; 10000 clamp+lerp |
| String helpers | lume, stringx | Split; Trim; Prefix / suffix; Fill a template; Word wrap; Title case; Contains; Split 1000 fields |
| Serialising tables | binser, bitser, serpent, lume, json | Round trip; 1 and "1" apart; Full precision; inf and -inf; Cycles; Binary strings; Output size; Save speed; Load speed |
| Printing tables | inspect, batteries.pretty | Nested table; Stable key order; Cycles; Metatables; Depth limit; Print 200 units |
| Collision | bump, HC, intersect | Box vs box; Circle vs circle; Point in box; Polygon; Push apart; No tunnelling; Ray cast; Find all overlaps |
| Pathfinding | jumper, batteries.pathfind | Shortest path; No way across; Diagonal moves; Terrain costs; Jump point search; 40 by 40 maze |
| Cameras | gamera, hump.camera | Screen centre; Zoom; Rotate; There and back; World bounds; Follow a target; 10000 conversions |
| Assertions and tests | lust, batteries.assert, luaunit | Equal passes; Failure says why; Type check; Compare tables; Describe / it; Before / after; 10000 checks |
| Timing and profiling | profile, batteries.measure | Time a function; Which function; Memory used; Mean, min, max; Cost of measuring |
| Immediate-mode GUI | suit, Slab, nuklear, cimgui | One frame; Click a button; Checkbox, slider; 100 buttons |

Every comparison also rates three **costs** of having the library: load time, memory kept and lines of code. They are whole-library figures, so a bundle such as batteries pays for all of it even if you would use one module; they count a third as much as a feature row each.

The libraries with nothing to compare against (tiny-ecs, anim8, peachy, sti, baton, ripple, moonshine, g3d, push, love-loader, lurker, lovebird, sock, luajit-request, nativefs, semver, log, and the four compiled ones) get one check of their core job under **Libraries that stand alone**. Checks that would open a network port, need a person, or need a binary this project does not ship only confirm the API is there, and say so.

## Ratings

- A cell is **0** when the library has no such feature (shown `-`), **1** when it tried and was wrong or broke, **5** when a feature works. Speed, memory and size rows give 5 within 10% of the best library, 4 within half, 3 within a fifth, 2 within a twentieth, else 1. `~` means this run could not try it (no window, no FFI): it has no rating and is left out of every average.
- A row's best libraries are marked `*` when the row tells libraries apart. A comparison's **overall** score is the weighted mean of its rows (missing features count as 0), and the best overall is marked.
- **Rate any cell yourself**: move to it, then **X** (or `+`) goes up and **Y** (or `-`) goes down, **0** clears. Your rating (`4m`) replaces the automatic one in the row, the overall score and the report, and is kept in `benchmark-ratings.tsv` in the save folder.
- Speed is this machine's. LuaJIT's JIT is off on some builds (LÖVE on an Apple-silicon Mac, iOS), and the report says so, since it changes who wins; compare numbers only from the same platform.

## Findings from the first run (macOS, LÖVE 11.5 and 12.0)

Tables: `results/macos-11.5.md`; reports: `results/macos-11.5.txt`, `results/macos-12.0.txt` (no window, in a sandbox) and `results/macos-11.5-window.txt` / `.md` (the maintainer's run on a Mac with a window, with the compiled libraries built: the immediate-mode GUIs, anim8, moonshine and push all ran, no FAIL; suit's click rows work after the fixes). On battery: indicative only. The compiled GUIs are fast because their work happens in C (cimgui: 2.6M widgets/s against suit's 0.6M), so their speed is not comparable with the Lua ones; nuklear has no Lua lines to count, so it is left out of the code-size row from now on (the saved run predates that and rates it 5). A few things the libraries do at their pinned commits (pinned by the tests, so an update shows):

- tween.lua loses the sibling fields of a nested table when a tween ends (`pos.y` is gone after tweening `pos.x`).
- json and lume.serialize lose digits (`1/3` comes back as `0.33333333333333`) and refuse cycles; json also refuses `inf` and mixed keys; binser keeps all of them.
- HC's `raycast` finds nothing for rays heading right or down; only bump moves a fast box without tunnelling through a thin wall.
- hump.class has no type test; batteries.state_machine cannot pass arguments to a state; batteries.pretty does not show metatables; batteries.assert compares tables by identity.
- lurker does not load on LÖVE 12.0 (it calls `love.filesystem.isDirectory`, removed there); love-loader and g3d only work with their files at the game's root, not in `libs/`.

## The compiled libraries

`sh tools/build-binaries.sh` builds, from source and universal (arm64 and x86_64), **love-nuklear** (a C module), **cimgui** (Dear ImGui 1.92 by CMake) and Discord's **discord-rpc**, into `bin/` (git-ignored: a binary is per platform). With them, nuklear and discord-rpc load and discord-rpc starts and stops (checked on LÖVE 11.5 and 12.0); cimgui loads its binary and then needs a window. Without them those libraries are `SKIP`, as before. macOS only so far; Linux and Windows need the same three builds. luasteam is not built (Steamworks SDK).

A compiled library can abort the whole program (nuklear does, if a frame is not ended with `draw`) and Lua cannot catch that, so before each such cell the benchmark writes a marker to the save folder; if the program dies there, the next start reports that cell as having aborted the program and skips it.

## After a run

A report is not finished until it is ingested: save it in `results/`, then `python3 -I scripts/ingest-benchmark.py results/<report>.txt` from the repository root updates the library list (the Benchmark line and load badges in JoyrIDE's Libraries panel). `CLAUDE.md` rule 15 has the whole routine.

## Run it

- In JoyrIDE: **Samples… > Benchmark Libraries**, then **Run**. Comparisons that draw (the immediate-mode GUIs, anim8, moonshine, g3d, push, peachy, sti) only run with a window.
- With a LÖVE on the command line: `love samples/benchmark-libraries`.
- Without a window: `love samples/benchmark-libraries --report` runs everything that needs no screen, prints the report, saves it and the Markdown tables (`benchmark-libraries-<platform>-<LÖVE version>.txt` and `.md`, in the save folder; the paths go to stderr) and exits 1 only if a library that should load did not. A comparison cell that comes out wrong is a *result*, shown as `INFO`, not a failure.
- Without a GPU: `love samples/benchmark-libraries --selftest` drives every screen against mocks (1095 checks).
- Plain LuaJIT: `luajit tests/unit.lua` (2300 checks: the harness, the ratings, every group's shape, what each library does).

Auto-Port may flag `ffi` use in nativefs, luajit-request and the other FFI libraries for the web: they are skipped there on purpose (the web profile has no FFI).

## Tested, and not

Tested here (`joyride-core/tests/benchmark_libraries_sample.rs`, `tests/unit.lua`, `tests/selftest.lua`): `libs/` is the curated list at its pinned commits with no compiled files or SDK; the headless report on the real LÖVE 11.5 and 12.0 (no FAIL, every library accounted for, the findings above); the project rebuilt by JoyrIDE's pipeline for PortMaster, mobile and web and recognised under each shim (FFI libraries skipped on the web); every screen driven against mocks; lint-clean at 11.5 and 12.0.

**Not tested** (needs a display or a device): the windowed comparison and window-only checks have been run once, by the maintainer on a Mac (suit, Slab, anim8, push worked; the suit click rows, the camera rows with a real window, moonshine and g3d were then fixed or recorded as findings and are not re-seen); peachy and sti only confirm their API; real drawing; a controller, touch screen or phone; luasteam, which needs Valve's Steamworks SDK (a partner login; it may not be redistributed), so it is never built; the windowed rows of love-nuklear and cimgui-love, whose binaries load but whose drawing has not been seen; any platform other than macOS.

## Licences

The project's own code is MIT. Each library keeps its own licence file in `libs/<name>/` (all MIT or Zlib; `moonshine` and `hump` carry theirs in the README and the file headers). The UI font (`assets/fonts/vga.woff`) is IBM VGA 8x16 by VileR, CC BY-SA 4.0 (`LICENSE-vga-font.txt`).

## The GUI click rows

suit, nuklear and cimgui are clicked by a simulated mouse (a press and release on a button, a tick on a checkbox, a drag across a slider), so they are compared on whether input works, not only on drawing. nuklear's button reports on the frame after the release and its slider moves only when the press starts on the knob; cimgui is fed straight through ImGui's own input queue (`imgui.love`'s handlers ignore a mouse the window has not focused). Both were worked out against mocked drawing under real LÖVE (nuklear's are in the selftest; cimgui's are not, since the selftest does not load it). Slab has no row: its own mouse module is not driven. The saved window run predates these rows.
