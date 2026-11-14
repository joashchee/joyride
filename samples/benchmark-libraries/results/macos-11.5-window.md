# Benchmark Libraries

2026-11-14 on macOS, LÖVE 11.5, with a window. Ratings are out of 5; `*` / bold marks the best in a row; `-` means the library has no such feature; `~` means this run could not try it; `[mine]` is a rating given by the person.

### Classes (OOP)

Define a class, inherit, call super, test the type, mix in methods.

| Feature | middleclass | classic | hump.class | batteries.class |
|---|:-:|:-:|:-:|:-:|
| Define + new | 5 | 5 | 5 | 5 |
| Inherit + super | 5 | 5 | 5 | 5 |
| Type test (is-a) | **5** | **5** | - | **5** |
| Mixins | 5 | 5 | 5 | 5 |
| __tostring etc. | 5 | 5 | 5 | 5 |
| Create objects | 3 (4.8M objects/s) | **5 (9.8M objects/s)** | **5 (9.8M objects/s)** | **5 (9.8M objects/s)** |
| Method calls | 4 (19.5M calls/s) | 4 (25.5M calls/s) | **5 (37.1M calls/s)** | 4 (25.4M calls/s) |
| Load time | 3 (1.02 ms) | **5 (0.28 ms)** | **5 (0.30 ms)** | 1 (6.42 ms) |
| Memory kept | 2 (10.7 KB) | **5 (1.56 KB)** | 4 (2.98 KB) | 1 (192 KB) |
| Code size | 3 (242 lines) | **5 (69.0 lines)** | 2 (1.2k lines) | 1 (6.2k lines) |
| **Overall** | 4.33 | **4.88 best** | 4.21 | 4.37 |

### Tweening

Move a number to a target over time, with easing, callbacks and stopping.

| Feature | tween | flux | hump.timer |
|---|:-:|:-:|:-:|
| Tween a number | 5 | 5 | 5 |
| Named easing | 5 | 5 | 5 |
| Nested fields | 1 (FAIL) | - | **5** |
| Completion | 5 | 5 | 5 |
| Delay and chain | - | **5** | **5** |
| Stop a tween | - | **5** | **5** |
| Run 100 tweens | 4 (292k tweens/s) | **5 (412k tweens/s)** | 4 (310k tweens/s) |
| Load time | 3 (0.68 ms) | 4 (0.45 ms) | **5 (0.30 ms)** |
| Memory kept | 2 (23.4 KB) | 2 (20.1 KB) | **5 (2.98 KB)** |
| Code size | 4 (368 lines) | **5 (225 lines)** | 2 (1.2k lines) |
| **Overall** | 2.88 | 4.21 | **4.75 best** |

### Timers

Call something after a delay, every interval, or while time passes; stop it; follow progress.

| Feature | hump.timer | batteries.timer |
|---|:-:|:-:|
| After a delay | 5 | 5 |
| Repeat | 5 | 5 |
| Repeat N times | **5** | - |
| Progress | 5 | 5 |
| Cancel | **5** | - |
| Wait in a script | **5** | - |
| 1000 timers | **5 (787k timers/s)** | 4 (693k timers/s) |
| Load time | **5 (0.30 ms)** | 1 (6.42 ms) |
| Memory kept | **5 (2.98 KB)** | 1 (192 KB) |
| Code size | **5 (1.2k lines)** | 2 (6.2k lines) |
| **Overall** | **5.00 best** | 2.54 |

### Events and messages

Register a listener, send a message with arguments, remove the listener, listen to everything.

| Feature | hump.signal | batteries.pubsub |
|---|:-:|:-:|
| Send + arguments | 5 | 5 |
| Remove a listener | 5 | 5 |
| Listen once | - | **5** |
| Wildcards | 5 | 5 |
| Change mid-send | 5 | 5 |
| 10000 sends | **5 (7.7M sends/s)** | 3 (2.0M sends/s) |
| Load time | **5 (0.30 ms)** | 1 (6.42 ms) |
| Memory kept | **5 (2.98 KB)** | 1 (192 KB) |
| Code size | **5 (1.2k lines)** | 2 (6.2k lines) |
| **Overall** | **4.29 best** | 4.18 |

### Game states

Switch between states with enter and leave callbacks, pass arguments, push and pop, transition from inside a state.

| Feature | hump.gamestate | batteries.state_machine |
|---|:-:|:-:|
| Enter / leave | 5 | 5 |
| Pass arguments | **5** | - |
| Update current | 5 | 5 |
| Push and pop | **5** | - |
| Self-transition | 5 | 5 |
| Ask where we are | 5 | 5 |
| 20000 switches | **5 (8.3M switches/s)** | 3 (3.6M switches/s) |
| Load time | **5 (0.30 ms)** | 1 (6.42 ms) |
| Memory kept | **5 (2.98 KB)** | 1 (192 KB) |
| Code size | **5 (1.2k lines)** | 2 (6.2k lines) |
| **Overall** | **5.00 best** | 3.04 |

### 2D vectors

Add, scale, length, normalise, rotate, dot, distance and project: as operators, as functions, or in place.

| Feature | hump.vector | vector-light | batteries.vec2 |
|---|:-:|:-:|:-:|
| Add, scale | 5 | 5 | 5 |
| Length, distance | 5 | 5 | 5 |
| Normalise | 5 | 5 | 5 |
| Rotate | 5 | 5 | 5 |
| Dot, project | 5 | 5 | 5 |
| Interpolate | - | - | **5** |
| 1000 steps | 2 (1.6M steps/s) | **5 (12.6M steps/s)** | 3 (4.2M steps/s) |
| Garbage made | 1 (438 KB) | **5 (0.00 KB)** | **5 (0.22 KB)** |
| Load time | **5 (0.30 ms)** | **5 (0.30 ms)** | 1 (6.42 ms) |
| Memory kept | **5 (2.98 KB)** | **5 (2.98 KB)** | 1 (192 KB) |
| Code size | **5 (1.2k lines)** | **5 (1.2k lines)** | 2 (6.2k lines) |
| **Overall** | 3.67 | **4.45 best** | 4.36 |

### Arrays and tables

Map, filter, reduce, search, copy and merge: lume against batteries' functional and tablex.

| Feature | lume | batteries |
|---|:-:|:-:|
| map | 5 | 5 |
| filter | 5 | 5 |
| reduce | 5 | 5 |
| find a value | 5 | 5 |
| slice | **5** | - |
| count / any / all | 5 | 5 |
| Remove duplicates | 5 | 5 |
| Group / partition | - | **5** |
| Zip | - | **5** |
| Shuffle | 5 | 5 |
| Copy (shallow) | 5 | 5 |
| Copy (deep) | - | **5** |
| Merge | 5 | 5 |
| Keys | 5 | 5 |
| map+filter+reduce | **5 (16.2M elements/s)** | 4 (11.2M elements/s) |
| Copy 1000 | 5 (148.6M elements/s) | 5 (147.8M elements/s) |
| Load time | **5 (0.81 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (40.3 KB)** | 3 (192 KB) |
| Code size | **5 (773 lines)** | 2 (6.2k lines) |
| **Overall** | 4.12 | **4.49 best** |

### Maths helpers

Clamp, round, interpolate, smooth, ping-pong, wrap, distance and angles.

| Feature | lume | mathx |
|---|:-:|:-:|
| Clamp | 5 | 5 |
| Round | 5 | 5 |
| Lerp | 5 | 5 |
| Smoothstep | 5 | 5 |
| Ping-pong | 5 | 5 |
| Wrap | - | **5** |
| Remap a range | - | **5** |
| Distance, angle | 5 | 5 |
| Sign | 5 | 5 |
| 10000 clamp+lerp | 5 (19.1M ops/s) | 5 (19.2M ops/s) |
| Load time | **5 (0.81 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (40.3 KB)** | 3 (192 KB) |
| Code size | **5 (773 lines)** | 2 (6.2k lines) |
| **Overall** | 4.09 | **4.75 best** |

### String helpers

Split, trim, test prefixes, fill templates, wrap and title-case.

| Feature | lume | stringx |
|---|:-:|:-:|
| Split | 5 | 5 |
| Trim | 5 | 5 |
| Prefix / suffix | - | **5** |
| Fill a template | 5 | 5 |
| Word wrap | **5** | - |
| Title case | - | **5** |
| Contains | - | **5** |
| Split 1000 fields | **5 (8.8M fields/s)** | 3 (4.2M fields/s) |
| Load time | **5 (0.81 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (40.3 KB)** | 3 (192 KB) |
| Code size | **5 (773 lines)** | 2 (6.2k lines) |
| **Overall** | 3.34 | **3.92 best** |

### Serialising tables

Save a table to text or bytes and read it back: how much survives, how big, how fast.

| Feature | binser | lume | json |
|---|:-:|:-:|:-:|
| Round trip | 5 | 5 | 5 |
| 1 and "1" apart | **5** | **5** | 1 (FAIL) |
| Full precision | **5** | 1 (FAIL) | 1 (FAIL) |
| inf and -inf | **5** | **5** | 1 (FAIL) |
| Cycles | **5** | 1 (FAIL) | 1 (FAIL) |
| Binary strings | 5 | 5 | 5 |
| Output size | **5 (8.1k bytes)** | 3 (20k bytes) | 4 (15k bytes) |
| Save speed | 4 (1.8k saves/s) | **5 (2.5k saves/s)** | 4 (1.7k saves/s) |
| Load speed | 4 (2.5k loads/s) | **5 (4.0k loads/s)** | 3 (826 loads/s) |
| Load time | 4 (0.71 ms) | 3 (0.81 ms) | **5 (0.37 ms)** |
| Memory kept | 4 (22.8 KB) | 3 (40.3 KB) | **5 (13.3 KB)** |
| Code size | **5 (627 lines)** | 4 (773 lines) | **5 (611 lines)** |
| **Overall** | **4.73 best** | 3.83 | 3.00 |

### Printing tables

Readable text for a table: every key, the same order every time, cycles, metatables, depth.

| Feature | inspect | batteries.pretty |
|---|:-:|:-:|
| Nested table | 5 | 5 |
| Stable key order | 5 | 5 |
| Cycles | 5 | 5 |
| Metatables | **5** | 1 (FAIL) |
| Depth limit | **5** | - |
| Print 200 units | **5 (871 prints/s)** | 4 (601 prints/s) |
| Load time | **5 (0.78 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (16.3 KB)** | 2 (192 KB) |
| Code size | **5 (451 lines)** | 2 (6.2k lines) |
| **Overall** | **5.00 best** | 3.14 |

### Collision

Do boxes, circles and polygons touch; stop a mover at a wall; find every overlap in a crowd.

| Feature | bump | HC | intersect |
|---|:-:|:-:|:-:|
| Box vs box | 5 | 5 | 5 |
| Circle vs circle | - | **5** | **5** |
| Point in box | 5 | 5 | 5 |
| Polygon | - | **5** | **5** |
| Push apart | 5 | 5 | 5 |
| No tunnelling | **5** | - | - |
| Ray cast | **5** | 1 (FAIL) | - |
| Find all overlaps | **5 (406k boxes/s)** | 2 (70k boxes/s) | 1 (7.3k boxes/s) |
| Load time | **5 (0.72 ms)** | 3 (1.87 ms) | 2 (6.42 ms) |
| Memory kept | **5 (24.1 KB)** | 3 (68.0 KB) | 2 (192 KB) |
| Code size | **5 (776 lines)** | 3 (1.8k lines) | 2 (6.2k lines) |
| **Overall** | **3.89 best** | 3.44 | 3.11 |

### Pathfinding

The shortest way across a grid with walls, no way at all, and a 40 by 40 maze.

| Feature | jumper | batteries.pathfind |
|---|:-:|:-:|
| Shortest path | 5 | 5 |
| No way across | 5 | 5 |
| Diagonal moves | 5 | 5 |
| Terrain costs | - | **5** |
| Jump point search | **5** | - |
| 40 by 40 maze | **5 (480 paths/s)** | 3 (155 paths/s) |
| Load time | **5 (3.61 ms)** | 4 (6.42 ms) |
| Memory kept | **5 (59.1 KB)** | 3 (192 KB) |
| Code size | **5 (2.2k lines)** | 3 (6.2k lines) |
| **Overall** | **4.29 best** | 3.76 |

### Cameras

Screen to world and back with zoom and rotation; keep the view inside the world; follow a target.

| Feature | gamera | hump.camera |
|---|:-:|:-:|
| Screen centre | 5 | 5 |
| Zoom | 5 | 5 |
| Rotate | 5 | 5 |
| There and back | 5 | 5 |
| World bounds | **5** | - |
| Follow a target | - | **5** |
| 10000 conversions | **5 (10.5M points/s)** | 4 (6.1M points/s) |
| Load time | 3 (0.62 ms) | **5 (0.30 ms)** |
| Memory kept | 3 (9.77 KB) | **5 (2.98 KB)** |
| Code size | **5 (209 lines)** | 2 (1.2k lines) |
| **Overall** | **4.21 best** | 4.12 |

### Assertions and tests

Check a value, fail with a message that says what was wrong, group checks into tests.

| Feature | lust | batteries.assert |
|---|:-:|:-:|
| Equal passes | 5 | 5 |
| Failure says why | 5 | 5 |
| Type check | 5 | 5 |
| Compare tables | **5** | 1 (FAIL) |
| Describe / it | **5** | - |
| Before / after | **5** | - |
| 10000 checks | 1 (1.0M checks/s) | **5 (73.8M checks/s)** |
| Load time | **5 (0.62 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (12.4 KB)** | 2 (192 KB) |
| Code size | **5 (241 lines)** | 1 (6.2k lines) |
| **Overall** | **4.50 best** | 2.83 |

### Timing and profiling

Time a function, count the memory it uses, find which functions the time went to.

| Feature | profile | batteries.measure |
|---|:-:|:-:|
| Time a function | 5 | 5 |
| Which function | **5** | - |
| Memory used | - | **5** |
| Mean, min, max | - | **5** |
| Cost of measuring | 2 (903 runs/s) | **5 (8.4k runs/s)** |
| Load time | **5 (0.57 ms)** | 2 (6.42 ms) |
| Memory kept | **5 (6.92 KB)** | 1 (192 KB) |
| Code size | **5 (227 lines)** | 1 (6.2k lines) |
| **Overall** | 2.84 | **3.55 best** |

### Immediate-mode GUI

Build a frame of widgets, click a button with a simulated mouse, count how many widgets a second.

| Feature | suit | Slab | nuklear | cimgui |
|---|:-:|:-:|:-:|:-:|
| One frame | 5 | 5 | 5 | 5 |
| Click a button | 5 | ~ | ~ | ~ |
| Checkbox, slider | 5 | ~ | ~ | ~ |
| 100 buttons | 3 (634k widgets/s) | 2 (228k widgets/s) | 3 (616k widgets/s) | **5 (2.6M widgets/s)** |
| Load time | 3 (2.57 ms) | 2 (10.3 ms) | **5 (1.08 ms)** | 2 (6.87 ms) |
| Memory kept | 3 (56.4 KB) | 1 (414 KB) | **5 (18.0 KB)** | 1 (593 KB) |
| Code size | 1 (1.1k lines) | 1 (18k lines) | **5 (0.00 lines)** | 1 (7.7k lines) |
| **Overall** | 4.06 | 2.77 | **4.34 best** | 3.76 |

### Libraries that stand alone

| Library | What it is for | Result |
|---|---|---|
| tiny-ecs | entities, components, systems | works |
| anim8 | spritesheet animation | works |
| peachy | Aseprite animations | works |
| sti | Tiled maps | works |
| baton | input mapping (keys, pads, mouse) | works |
| ripple | sound tags, volume, fades | works |
| moonshine | post-processing shader effects | works |
| g3d | 3D models and a camera | FAIL did not load: libs/g3d/g3d/init.lua:48: Could not open file g3d/g3d.vert. Does not exist. |
| push | a virtual resolution scaled to the window | works |
| love-loader | loading assets on a thread | FAIL it reads its own source as `love-loader.lua` from the game's root folder, so it only works copied there, not from libs/ |
| lurker | hot reloading of changed files | works |
| lovebird | a web console for a running game | works |
| sock | networking (ENet) | works |
| luajit-request | HTTP through libcurl | works |
| nativefs | files anywhere on the computer | works |
| semver | version numbers | works |
| log | levelled console logging | works |
| lua-discordRPC | Discord Rich Presence | works |
| luasteam | Steamworks | ~ needs Valve's Steamworks SDK, which cannot be redistributed or built here: Needs its compiled module (luasteam.so or .dll, from the repository's releases) and Valve's Steamworks library (libsteam_api, steam_api64.dll) from the Steamworks SDK, next to the game; Steam must be running. |
| love-nuklear | Nuklear GUI (compiled) | works |
| cimgui-love | Dear ImGui (FFI) | works |
