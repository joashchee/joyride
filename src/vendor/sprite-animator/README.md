# sprite-animator

A dependency-free motion engine for sprite and cell animation: seeded, deterministic recipes make short clips (walk, run, jump) as integer
pose tracks with named events, with a lint of the craft rules, an exact JSON file format, a tempo fit for Composer and a bridge to Painter.
Same recipe, params and seed, same clip. It imports nothing outside this folder (not Painter or Composer either), and the main entry has
no Node built-ins behind it, so it runs unchanged in a page, a worker, Node or Deno. Tests enforce both. Vendored into JoyrIDE under MIT.

The three engines fit together like this:

| Engine | Package | Role |
|---|---|---|
| Painter | `art-sprites` | draws a frame (pixels or cells) |
| Composer | `music-composer` | writes the music, on a 96 ppq tick grid |
| Animator | `sprite-animator` | says what pose each frame has, and when events land |

## Use it

```js
import { animate, tempo, bind, lintClip, writeClip } from 'sprite-animator';

const walk = animate('walk');                         // 4 frames, loop; { id, kind, frames: [pose], fps, loop, events, holds, reduced, spec }
const smooth = animate('run', { frames: 8, lean: 3 }); // the same keys, blended to 8 frames
const jump = animate('jump', { height: 6 });           // one-shot: crouch, takeoff, apex, fall, land, recover

lintClip(walk);                                       // [] or findings { level, rule, message }
tempo(walk, { bpm: 120 });                            // { ms: [250,250,250,250], events: [{ name: 'step', frame: 0, ms: 0, tick: 0 }, ...] }
writeClip(walk);                                      // the exact artifact: animator-clip JSON text

// Painter (1.19+) takes a clip as its `motion` param, on any recipe: `character` draws each pose with its joints, every other recipe
// (pixel, ansi, text) moves its first frame by dx dy lift squat lean sway. The clip goes in as is:
//   draw('slime', { motion: walk }).clips.walk
// To draw the poses with anything else, `bind` takes your own function; the result has the shape of Painter's clip record.
const drawn = bind(walk, (pose) => myDrawFrame(pose));
```

| Recipe | Frames | Params |
|---|---|---|
| `walk` | 4, loop: contact, passing, contact, passing | `stride` 1-4, `bounce` 0-3, `frames`, `fps`, `wobble`, `seed` |
| `run` | 4, loop: contact, flight, contact, flight | `stride` 2-5, `lean` 0-4, `rise` 1-4, `frames`, `fps`, `wobble`, `seed` |
| `jump` | 6, one-shot: crouch, takeoff, apex, fall, land, recover | `height` 2-9, `crouch` 1-3, `frames`, `fps`, `wobble`, `seed` |

`frames: n` blends the same keys to `n` frames (loops are cleanest at a multiple of 4). `wobble` is a seeded head bob, off by default.

A pose is `{ dx dy lean head squat lift drag sway legs arm weapon air }`: Painter's `character` pose vocabulary plus `air` (1 while no foot is
down). All integers; y is up-negative; `legs` are the two feet's offsets.

## Motion types

Painter 1.20 gives every recipe a **motion type**, and a type its own presets, params and limits: a hen bobs, a tower settles, a flag gusts, a status bar
jolts. Animator has the same 14 types (`MOTION_TYPES`, `MOTION_OF` for the Painter recipe of each, `motionTypeOf('character')` is `'figure'`) and one recipe
per preset, id `<type>.<preset>`. A preset's clip matches Painter's own, frame for frame, so `draw('cloth', { motion: 'gust' })` and
`animate('cloth.gust')` agree; the Animator side adds `lintClip`, `writeClip`, `tempo` and the CLI (`animate types`).

```js
const gust = animate('cloth.gust', { wind: 3 });                 // the preset's own params
const grow = animate('plant.grow', { h: 16 });                   // w and h: the art's size, for the presets that depend on it (default 32 by 32)
const scroll = animate('tiling.scroll', { w: 16, direction: 'up' }); // wrap: true, one frame a step
```

| Type | Painter recipes | Presets (own params) |
|---|---|---|
| `figure` | `character` | `breathe` (amount), `hop` (height), `lunge` (reach), `recoil` (push), `bow` (depth), `cheer` (height) |
| `head` | `face`, `portrait`, `bust`, `creatureface` | `breathe` (amount), `nod` (depth), `tilt` (amount), `flinch` (push) |
| `creature` | `creature`, `creation` | `breathe` (amount), `hop` (height), `pounce` (reach), `slither` (sway), `recoil` (push) |
| `object` | `item`, `thing`, `icon`, `sigil` | `bob` (amount), `pulse` (amount), `swing` (angle), `drop` (height) |
| `limb` | `hand` | `wave` (angle), `beckon` (amount), `tremble` (amount) |
| `plant` | `farm`, `prop` | `sway` (wind), `rustle` (wind), `grow` (from), `bounce` (height) |
| `machine` | `vehicle` | `rumble` (amount), `roll` (distance), `bump` (height), `lurch` (push) |
| `structure` | `building`, `strategy`, `furniture`, `room` | `settle` (amount), `quake` (amount), `creak` (angle), `build` (from), `crumble` (to) |
| `tiling` | `tile`, `pattern` | `scroll` (direction, step), `drift` (step), `ripple` (amount) |
| `scenery` | `scene`, `vista`, `landscape`, `locale`, `onplace`, `weatherloop` | `tremor` (amount), `shimmer` (amount), `breathe` (amount) |
| `cloth` | `cloth` | `gust` (wind), `billow` (wind), `snap` (wind), `flutter` (wind) |
| `effect` | `effect` | `pulse` (amount), `burst` (from), `rise` (height), `shake` (amount) |
| `panel` | `bar`, `mappane`, `sheet`, `menu`, `fight`, `artpack` | `jolt` (push), `shake` (amount), `slide-in` (distance), `drop-in` (distance) |
| `lettering` | `banner` | `wave` (amount), `wobble` (amount), `slide-in` (distance), `drop-in` (distance) |

Every param is a whole number (`steps` 2-32, `fps` 1-60); an unknown preset or param throws. The clip id is the preset's name, as in Painter. A `tiling` clip
has `wrap: true` (kept in the file), so a scrolling tile stays tiled, and the lint gives it no seam. The lint reads a type's own channels and limits (a share of
the art, from `w` and `h`): `channel` (error) for a channel the type does not use, `range` (warn) for a move past its limit. `lintMotionType(clip, type, size)` checks
any clip against any type. The waves are a rational curve rounded (no `Math.sin`), within 0.002 of a sine.

## The CLI

```
animate kits [--strict]                       what the reference kits ask for, and what can be made
animate types [type]                          the motion types, their Painter recipes, channels and presets
animate variants [recipe]                     every variant of a recipe (or all), linted
animate show walk [stride=3]                  a clip as a table of its poses
animate make run frames=8 --out run.json      a clip as an animator-clip file
animate lint walk | file.json                 the motion rules as checks
animate tempo run --bpm 140 --steps-per-beat 2   frame lengths and event ticks on a beat
animate export jump --dir out                 the clip file plus a provenance record
```

Output goes only where `--out` or `--dir` says.

## The lint

`shape` (integers, one hold per frame, events on frames), `frames` (count), `seam` (a loop must not pop at the wrap), `still` (two equal
neighbours: hold instead), `range`, and the motion type's `channel` and `range` (see Motion types), and per-kind rules: a **walk** keeps a foot down and steps twice; a **run** has both flight and contact
frames, with steps on the ground; a **jump** is a one-shot with one `takeoff` and one `land`, airborne between, ending at rest.

## What stays a project's job

Which clip a character plays, its speed in world units, and art direction. To add motion for one game, register a kit with `registerKit`
or add a module under `recipes/`; do not fork the engine. **Limits:** it makes poses, not pictures. Painter draws them (its `motion` param,
`docs/animator-plan.md`, "The Painter hand-off"); `head drag legs arm weapon air` only move a `character`, and are ignored on other art.

Types are in `index.d.ts`. Tests: `npm test` (Node 22+, no files from a host project). Rules for contributors: `CLAUDE.md`.

## License

MIT, Copyright 2026 ansiapps (see `LICENSE`); see `NOTICE` for what it includes from others.
