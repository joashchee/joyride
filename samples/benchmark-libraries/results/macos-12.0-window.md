# Benchmark Libraries

2026-11-15 on macOS, LÖVE 12.0, with a window. Ratings are out of 5; `*` / bold marks the best in a row; `-` means the library has no such feature; `~` means this run could not try it; `[mine]` is a rating given by the person.

### Classes (OOP)

Define a class, inherit, call super, test the type, mix in methods.

| Feature | middleclass | classic | hump.class | batteries.class |
|---|:-:|:-:|:-:|:-:|
| Define + new | 5 | 5 | 5 | 5 |
| Inherit + super | 5 | 5 | 5 | 5 |
| Type test (is-a) | **5** | **5** | - | **5** |
| Mixins | 5 | 5 | 5 | 5 |
| __tostring etc. | 5 | 5 | 5 | 5 |
| Create objects | 4 (4.8M objects/s) | **5 (9.4M objects/s)** | **5 (9.4M objects/s)** | **5 (9.3M objects/s)** |
| Method calls | 4 (19.6M calls/s) | 4 (24.9M calls/s) | **5 (36.7M calls/s)** | 4 (25.2M calls/s) |
| Load time | 3 (0.59 ms) | **5 (0.28 ms)** | **5 (0.27 ms)** | 1 (6.39 ms) |
| Memory kept | 3 (12.3 KB) | **5 (3.13 KB)** | 4 (4.54 KB) | 1 (194 KB) |
| Code size | 3 (242 lines) | **5 (69.0 lines)** | 2 (1.2k lines) | 1 (6.2k lines) |
| **Overall** | 4.50 | **4.88 best** | 4.21 | 4.37 |

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
| Run 100 tweens | 4 (293k tweens/s) | **5 (421k tweens/s)** | 4 (300k tweens/s) |
| Load time | 3 (0.67 ms) | 4 (0.38 ms) | **5 (0.27 ms)** |
| Memory kept | 2 (24.9 KB) | 3 (21.7 KB) | **5 (4.54 KB)** |
| Code size | 4 (368 lines) | **5 (225 lines)** | 2 (1.2k lines) |
| **Overall** | 2.88 | 4.25 | **4.75 best** |

### Timers

Call something after a delay, every interval, or while time passes; stop it; follow progress.

| Feature | hump.timer | batteries.timer | cron | tick |
|---|:-:|:-:|:-:|:-:|
| After a delay | 5 | 5 | 5 | 5 |
| Repeat | 5 | 5 | 5 | 5 |
| Repeat N times | **5** | - | - | - |
| Progress | **5** | **5** | - | - |
| Cancel | **5** | - | - | **5** |
| Wait in a script | **5** | - | - | - |
| 1000 timers | 4 (762k timers/s) | 4 (667k timers/s) | **5 (1.1M timers/s)** | **5 (1.2M timers/s)** |
| Load time | **5 (0.27 ms)** | 1 (6.39 ms) | 4 (0.50 ms) | **5 (0.29 ms)** |
| Memory kept | **5 (4.54 KB)** | 1 (194 KB) | 4 (6.38 KB) | 4 (7.66 KB) |
| Code size | 2 (1.2k lines) | 1 (6.2k lines) | **5 (107 lines)** | 4 (167 lines) |
| **Overall** | **4.75 best** | 2.50 | 2.42 | 3.04 |

### Events and messages

Register a listener, send a message with arguments, remove the listener, listen to everything.

| Feature | hump.signal | batteries.pubsub |
|---|:-:|:-:|
| Send + arguments | 5 | 5 |
| Remove a listener | 5 | 5 |
| Listen once | - | **5** |
| Wildcards | 5 | 5 |
| Change mid-send | 5 | 5 |
| 10000 sends | **5 (7.7M sends/s)** | 3 (1.9M sends/s) |
| Load time | **5 (0.27 ms)** | 1 (6.39 ms) |
| Memory kept | **5 (4.54 KB)** | 1 (194 KB) |
| Code size | **5 (1.2k lines)** | 2 (6.2k lines) |
| **Overall** | **4.29 best** | 4.18 |

### Game states

Switch between states with enter and leave callbacks, pass arguments, push and pop, transition from inside a state.

| Feature | hump.gamestate | batteries.state_machine | roomy |
|---|:-:|:-:|:-:|
| Enter / leave | 5 | 5 | 5 |
| Pass arguments | **5** | - | **5** |
| Update current | 5 | 5 | 5 |
| Push and pop | **5** | - | **5** |
| Self-transition | 5 | 5 | 5 |
| Ask where we are | **5** | **5** | - |
| 20000 switches | **5 (7.9M switches/s)** | 3 (3.7M switches/s) | 4 (6.6M switches/s) |
| Load time | **5 (0.27 ms)** | 1 (6.39 ms) | 4 (0.33 ms) |
| Memory kept | **5 (4.54 KB)** | 1 (194 KB) | 4 (6.50 KB) |
| Code size | 2 (1.2k lines) | 1 (6.2k lines) | **5 (130 lines)** |
| **Overall** | **4.87 best** | 3.00 | 4.17 |

### 2D vectors

Add, scale, length, normalise, rotate, dot, distance and project: as operators, as functions, or in place.

| Feature | hump.vector | vector-light | batteries.vec2 | cpml.vec2 |
|---|:-:|:-:|:-:|:-:|
| Add, scale | 5 | 5 | 5 | 5 |
| Length, distance | 5 | 5 | 5 | 5 |
| Normalise | 5 | 5 | 5 | 5 |
| Rotate | 5 | 5 | 5 | 5 |
| Dot, project | **5** | **5** | **5** | - |
| Interpolate | - | - | **5** | **5** |
| 1000 steps | 2 (1.5M steps/s) | **5 (12.6M steps/s)** | 3 (4.1M steps/s) | 2 (1.3M steps/s) |
| Garbage made | 1 (438 KB) | **5 (0.00 KB)** | **5 (0.22 KB)** | 1 (328 KB) |
| Load time | **5 (0.27 ms)** | **5 (0.27 ms)** | 1 (6.39 ms) | 2 (4.71 ms) |
| Memory kept | **5 (4.54 KB)** | **5 (4.54 KB)** | 1 (194 KB) | 1 (166 KB) |
| Code size | **5 (1.2k lines)** | **5 (1.2k lines)** | 2 (6.2k lines) | 3 (5.5k lines) |
| **Overall** | 3.67 | **4.45 best** | 4.36 | 3.33 |

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
| map+filter+reduce | **5 (15.3M elements/s)** | 4 (11.0M elements/s) |
| Copy 1000 | 5 (144.0M elements/s) | 5 (140.3M elements/s) |
| Load time | **5 (0.76 ms)** | 2 (6.39 ms) |
| Memory kept | **5 (40.6 KB)** | 3 (194 KB) |
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
| 10000 clamp+lerp | 5 (19.9M ops/s) | 5 (18.9M ops/s) |
| Load time | **5 (0.76 ms)** | 2 (6.39 ms) |
| Memory kept | **5 (40.6 KB)** | 3 (194 KB) |
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
| Split 1000 fields | **5 (8.9M fields/s)** | 3 (4.2M fields/s) |
| Load time | **5 (0.76 ms)** | 2 (6.39 ms) |
| Memory kept | **5 (40.6 KB)** | 3 (194 KB) |
| Code size | **5 (773 lines)** | 2 (6.2k lines) |
| **Overall** | 3.34 | **3.92 best** |

### Serialising tables

Save a table to text or bytes and read it back: how much survives, how big, how fast.

| Feature | binser | bitser | serpent | smallfolk | ser | moonblob | lume | json |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| Round trip | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 5 |
| 1 and "1" apart | **5** | **5** | **5** | **5** | **5** | **5** | **5** | 1 (FAIL) |
| Full precision | **5** | **5** | **5** | **5** | **5** | **5** | 1 (FAIL) | 1 (FAIL) |
| inf and -inf | **5** | **5** | **5** | **5** | **5** | **5** | **5** | 1 (FAIL) |
| Cycles | **5** | **5** | **5** | **5** | **5** | 1 (FAIL) | 1 (FAIL) | 1 (FAIL) |
| Binary strings | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 5 |
| Output size | 4 (8.1k bytes) | **5 (6.6k bytes)** | 4 (13k bytes) | 3 (14k bytes) | 3 (18k bytes) | 4 (13k bytes) | 3 (20k bytes) | 3 (15k bytes) |
| Save speed | 4 (1.8k saves/s) | 4 (1.3k saves/s) | 1 (91.5 saves/s) | 4 (1.5k saves/s) | 4 (1.5k saves/s) | 3 (575 saves/s) | **5 (2.5k saves/s)** | 4 (1.7k saves/s) |
| Load speed | 4 (2.5k loads/s) | 3 (1.6k loads/s) | **5 (4.3k loads/s)** | 3 (1.3k loads/s) | 4 (3.8k loads/s) | 2 (691 loads/s) | **5 (4.0k loads/s)** | 2 (838 loads/s) |
| Load time | 3 (0.73 ms) | 4 (0.41 ms) | 4 (0.36 ms) | 4 (0.33 ms) | **5 (0.26 ms)** | 4 (0.39 ms) | 3 (0.76 ms) | 4 (0.34 ms) |
| Memory kept | 3 (24.4 KB) | 3 (23.4 KB) | 3 (25.8 KB) | 4 (11.1 KB) | **5 (7.11 KB)** | 3 (26.7 KB) | 2 (40.6 KB) | 3 (14.7 KB) |
| Code size | 3 (627 lines) | 3 (577 lines) | **5 (141 lines)** | 4 (214 lines) | 4 (208 lines) | 2 (1.0k lines) | 2 (773 lines) | 3 (611 lines) |
| **Overall** | 4.50 | 4.53 | 4.40 | 4.40 | **4.57 best** | 3.80 | 3.73 | 2.63 |

### Printing tables

Readable text for a table: every key, the same order every time, cycles, metatables, depth.

| Feature | inspect | batteries.pretty |
|---|:-:|:-:|
| Nested table | 5 | 5 |
| Stable key order | 5 | 5 |
| Cycles | 5 | 5 |
| Metatables | **5** | 1 (FAIL) |
| Depth limit | **5** | - |
| Print 200 units | **5 (812 prints/s)** | 4 (572 prints/s) |
| Load time | **5 (0.87 ms)** | 2 (6.39 ms) |
| Memory kept | **5 (16.5 KB)** | 2 (194 KB) |
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
| Find all overlaps | **5 (402k boxes/s)** | 2 (68k boxes/s) | 1 (7.2k boxes/s) |
| Load time | **5 (0.85 ms)** | 3 (1.83 ms) | 2 (6.39 ms) |
| Memory kept | **5 (25.6 KB)** | 3 (69.3 KB) | 2 (194 KB) |
| Code size | **5 (776 lines)** | 3 (1.8k lines) | 2 (6.2k lines) |
| **Overall** | **3.89 best** | 3.44 | 3.11 |

### Pathfinding

The shortest way across a grid with walls, no way at all, and a 40 by 40 maze.

| Feature | jumper | batteries.pathfind | lua-star | luafinding |
|---|:-:|:-:|:-:|:-:|
| Shortest path | **5** | **5** | **5** | 1 (FAIL) |
| No way across | 5 | 5 | 5 | 5 |
| Diagonal moves | 5 | 5 | 5 | 5 |
| Terrain costs | - | **5** | - | - |
| Jump point search | **5** | - | - | - |
| 40 by 40 maze | 2 (467 paths/s) | 2 (155 paths/s) | 1 (15.3 paths/s) | **5 (2.9k paths/s)** |
| Load time | 2 (3.55 ms) | 1 (6.39 ms) | **5 (0.26 ms)** | 3 (0.75 ms) |
| Memory kept | 2 (57.6 KB) | 1 (194 KB) | **5 (7.74 KB)** | 4 (14.5 KB) |
| Code size | 2 (2.2k lines) | 1 (6.2k lines) | **5 (209 lines)** | 3 (678 lines) |
| **Overall** | **3.42 best** | 3.28 | 3.01 | 2.76 |

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
| 10000 conversions | **5 (10.7M points/s)** | 4 (6.0M points/s) |
| Load time | 3 (0.59 ms) | **5 (0.27 ms)** |
| Memory kept | 3 (11.3 KB) | **5 (4.54 KB)** |
| Code size | **5 (209 lines)** | 2 (1.2k lines) |
| **Overall** | **4.21 best** | 4.12 |

### Assertions and tests

Check a value, fail with a message that says what was wrong, group checks into tests.

| Feature | lust | batteries.assert | luaunit |
|---|:-:|:-:|:-:|
| Equal passes | 5 | 5 | 5 |
| Failure says why | 5 | 5 | 5 |
| Type check | 5 | 5 | 5 |
| Compare tables | **5** | 1 (FAIL) | **5** |
| Describe / it | **5** | - | **5** |
| Before / after | **5** | - | **5** |
| 10000 checks | 1 (1.0M checks/s) | **5 (64.2M checks/s)** | 3 (21.8M checks/s) |
| Load time | **5 (0.58 ms)** | 2 (6.39 ms) | 3 (1.39 ms) |
| Memory kept | **5 (13.8 KB)** | 2 (194 KB) | 2 (127 KB) |
| Code size | **5 (241 lines)** | 1 (6.2k lines) | 2 (4.6k lines) |
| **Overall** | **4.50 best** | 2.83 | 4.41 |

### Timing and profiling

Time a function, count the memory it uses, find which functions the time went to.

| Feature | profile | batteries.measure |
|---|:-:|:-:|
| Time a function | 5 | 5 |
| Which function | **5** | - |
| Memory used | - | **5** |
| Mean, min, max | - | **5** |
| Cost of measuring | 2 (623 runs/s) | **5 (8.2k runs/s)** |
| Load time | **5 (0.52 ms)** | 2 (6.39 ms) |
| Memory kept | **5 (8.49 KB)** | 1 (194 KB) |
| Code size | **5 (227 lines)** | 1 (6.2k lines) |
| **Overall** | 2.84 | **3.55 best** |

### Immediate-mode GUI

Build a frame of widgets, click a button with a simulated mouse, count how many widgets a second.

| Feature | suit | Slab | nuklear | cimgui |
|---|:-:|:-:|:-:|:-:|
| One frame | 5 | 5 | 5 | 5 |
| Click a button | 5 | ~ | 5 | 5 |
| Checkbox, slider | 5 | ~ | 5 | 5 |
| 100 buttons | 3 (736k widgets/s) | 2 (177k widgets/s) | 3 (754k widgets/s) | **5 (1.6M widgets/s)** |
| Load time | 3 (2.59 ms) | 2 (10.3 ms) | **5 (1.02 ms)** | 2 (6.08 ms) |
| Memory kept | 3 (57.8 KB) | 1 (418 KB) | **5 (19.5 KB)** | 1 (588 KB) |
| Code size | **5 (1.1k lines)** | 2 (18k lines) | ~ | 2 (7.7k lines) |
| **Overall** | 4.33 | 2.88 | **4.57 best** | 4.32 |

### Libraries that stand alone

| Library | What it is for | Result |
|---|---|---|
| tiny-ecs | entities, components, systems | works |
| anim8 | spritesheet animation | works |
| peachy | Aseprite animations | works |
| sti | Tiled maps | works |
| baton | input mapping (keys, pads, mouse) | works |
| talkies | dialogue boxes | works |
| Concord | entities, components, systems | works |
| i18n | translations, plurals, fallbacks | works |
| cargo | assets by name from a folder | FAIL indexing the folder raised: libs/cargo/cargo.lua:53: attempt to call field 'isDirectory' (a nil value) |
| sqlua | SQLite databases | works |
| lua-ljsqlite3 | SQLite databases | works |
| xsys | string and table helpers (needed by lua-ljsqlite3) | works |
| tactile | input mapping (keys, pads, mouse) | works |
| input | input state (keys, mouse, pads) with pressed, released and held | works |
| jprof | profiling zones and saving a profile | works |
| shash | a spatial hash | works |
| beehive | behaviour trees | works |
| breezefield | a love.physics wrapper | works |
| Lily | loading assets on background threads | works |
| lovebpm | beat events for a music track | works |
| cartographer | Tiled maps | works |
| 30log | classes and inheritance | works |
| knife | a base class (one of its many small modules) | works |
| nata | entity management | works |
| lua-state-machine | a finite state machine | works |
| Bresenham | cells along a line | works |
| trickle | a bitstream for network messages | works |
| HSLUV | a perceptual colour space | works |
| Vornmath | vectors, matrices and quaternions | works |
| Vivid | colour conversion | works |
| Luvent | events with actions | works |
| smiti18n | translations, plurals, fallbacks | works |
| AppleCake | a profiler with a trace file | works |
| Shove | resolution scaling | works |
| resolution_solution | resolution scaling | works |
| Maid64 | a low-resolution scaler | works |
| Brady | a camera with parallax layers | works |
| GamepadGuesser | button prompts for the right controller | works |
| slick | swept polygon collision | works |
| lighter | 2D lights and shadows | works |
| Patchy | 9-patch images | works |
| denver | generated sound waves and note names | works |
| splashy | a splash screen sequence | works |
| LoverNet | client and server over ENet | works |
| Inky | retained UI elements and a pointer | works |
| Helium | retained UI framework | works |
| anim9 | 3D skeletal animation (with cpml) | works |
| brinevector3D | 3D vectors | works |
| brinevector | 2D vectors with ffi speed | works |
| bump-3dpd | 3D box collision | works |
| iqe | Inter-Quake Export (text) models | works |
| iqm-exm | Inter-Quake Model (binary) models | works |
| Menori | 3D rendering and glTF | works |
| astar | A* on any graph | works |
| andross | DragonBones 2D skeletal animation | works |
| love-ase | Aseprite files | works |
| chiro | Spine skeletal animation | works |
| lovanim | stateful animations | works |
| lump | Flash animation | works |
| skeletor | 2D skeletal animation | works |
| Walt | animations from grids of frames | works |
| Editgrid | a scaling grid for cameras | works |
| parallax | parallax scrolling | works |
| roomshift | a room-based camera | works |
| debugGraph | FPS and value graphs | works |
| loveprofiler | logging and profiling | works |
| Artal | Photoshop .psd files | works |
| deep | draw calls in depth order | works |
| draft | complex shapes | works |
| grove | draw order (and other small modules) | works |
| lovector | SVG and vector paths | works |
| Runtime-TextureAtlas | packing images into an atlas at run time | works |
| sunscreen | a virtual resolution | works |
| turtle | turtle graphics | works |
| evolved | entity component system | works |
| astray | maze and dungeon generation | works |
| GameStateManager | game states | works |
| love-state-switcher | switching states | works |
| lovely-windows | virtual windows | works |
| lue | colour effects | works |
| narrator | Ink stories | works |
| reflowprint | text shown one character at a time | works |
| rotLove | roguelike toolkit | works |
| SceneMan | a stack of scenes | works |
| scenery | scenes and states | works |
| screenmanager | screens | works |
| shard | small modules (base class) | works |
| Kazari | multitouch gestures | works |
| love-microphone | microphone input | works |
| LoveKeys | key state and aliases | works |
| swipe | a radial keyboard | works |
| bitumbra | GPU shadows for many lights | works |
| Light | a dynamic light | works |
| simple-love-lights | raycast lights | works |
| delaunay | Delaunay triangulation | works |
| loaded_dice | weighted random choice | works |
| polysec | polygon and rectangle intersections | works |
| pvector | vectors after Processing's PVector | works |
| love2d-lua-websocket | a WebSocket client | works |
| NoobHub | multiplayer messaging | works |
| LowerClass | classes with mixins | works |
| muun | MoonScript-compatible classes | works |
| Object-Library | simple physical objects | works |
| Piefiller | a graphical profiler | works |
| arson | custom types for json.lua | works |
| cdata | Lua tables to C data | works |
| Lady | saving and loading savegames | works |
| ShaderScan | shader hot reload | works |
| lunatest | xUnit-style tests | works |
| Badr | UI components | works |
| CatUI | a light-weight GUI | works |
| gooi | a GUI for Android | works |
| LoveFrames | a GUI with many controls | works |
| plan | rule-based layout | works |
| TuxRedux | an immediate-mode UI | works |
| yalg | a reactive UI | works |
| bar128-love | Code 128 barcodes | works |
| GifCat | saving GIFs | works |
| ini_parser | .ini files | works |
| KeyedArray | ordered dictionary | works |
| love-qrcode | QR codes | works |
| love2d-assets-loader | loading assets from folders | works |
| LuaRequireExtended | require with parameters | works |
| ShakeDetectorLua | shake detection from the accelerometer | works |
| LUA-SNG-Parser | .sng song packages | works |
| fetch-lua | HTTP and HTTPS | works |
| love-dialogue | dialogue with a script language | works |
| safeword | overscan detection | works |
| Sysl-Pixel | a pixel-art scaler | works |
| loveblobs | softbody physics | works |
| SYSL-Text | animated text | works |
| 3DreamEngine | a 3D engine | works |
| Lovox | pseudo-3D voxels | works |
| FPSGraph | an FPS graph | works |
| LoveDebug | an inline debug console | works |
| shack | screen shake and rotation | works |
| ripple | sound tags, volume, fades | works |
| moonshine | post-processing shader effects | works |
| g3d | 3D models and a camera | works |
| push | a virtual resolution scaled to the window | FAIL libs/push/push.lua:113: attempt to call upvalue 'getDPI' (a nil value) |
| love-loader | loading assets on a thread | FAIL it reads its own source as `love-loader.lua` from the game's root folder, so it only works copied there, not from libs/ |
| lurker | hot reloading of changed files | FAIL did not load: libs/lurker/lurker.lua:55: attempt to call upvalue 'isdir' (a nil value) |
| lovebird | a web console for a running game | works |
| sock | networking (ENet) | works |
| luajit-request | HTTP through libcurl | works |
| nativefs | files anywhere on the computer | works |
| neumetik-midi | MIDI music with no SoundFont | works |
| semver | version numbers | works |
| log | levelled console logging | works |
| lua-discordRPC | Discord Rich Presence | works |
| luasteam | Steamworks | ~ needs Valve's Steamworks SDK, which cannot be redistributed or built here: Needs its compiled module (luasteam.so or .dll, from the repository's releases) and Valve's Steamworks library (libsteam_api, steam_api64.dll) from the Steamworks SDK, next to the game; Steam must be running. |
| love-nuklear | Nuklear GUI (compiled) | works |
| cimgui-love | Dear ImGui (FFI) | works |
| ArrayRotation | rotating 2D arrays | works |
| lynx | list-based menus | works |
| Lovely-Toasts | toast messages | works |
| Hanker | mixed immediate and retained UI | works |
| require-lua | requiring a whole folder | works |
| lovr-ui2d | immediate mode GUI | works |
| wave | sound manager with beat tracking | works |
| telescope | BDD-style tests | works |
| prod_ui | user interface library (unfinished) | works |
| Lua-Opus | Ogg Opus streaming | ~ its binary is not here: Needs the opus and ogg shared libraries (opus.dll and ogg.dll on Windows) next to the game. |
