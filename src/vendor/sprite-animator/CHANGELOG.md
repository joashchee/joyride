# Changelog

## Unreleased (R36S command-line port)

- **R36S: a third device report, the same result** (2026-11-12; `r36s/reports/2026-11-12-r36s-report.txt`): 15 PASS, 0 FAIL, 0.4 s for all ten commands, 11.66 MB peak, as on 2026-11-09. No `== power ==` section yet (it waits on r3i6nos's `tools/qjs-node/report.py`). The unit's clock stamps it 2025-11-12, a year behind (set by hand on the unit).
- **Dates corrected** (2026-11-09): entries dated 2026-11-10 are 2026-11-09, and the first report is renamed `r36s/reports/2026-11-09-r36s-report.txt`. The date came from the report's stamp, made while the unit's clock ran about 15 hours fast; the stamp itself is kept, quoted where it is explained.
- **`ROADMAP.md`** (2026-11-09): what is done and left, from `docs/animator-plan.md` (the phases, the open question) and the R36S port (the workflow, a LÖVE app calling `animate`, output compared on the unit).
- **R36S: a second device report, the same result** (2026-11-09 22:30; `r36s/reports/2026-11-09-2230-r36s-report.txt`): 15 PASS, 0 FAIL on the same unit. Every command took the same time as the first run (0.02 to 0.05 s, 0.3 s in all); peak memory 11.6 MB (was 11.5). So the first figures hold. The first report's stamp, 2026-11-10 03:34, came from the unit's clock running about 15 hours fast (it was committed 2026-11-09 12:34); its file is now named for the day it ran. This one's clock was right.
- **R36S: the first device report from a unit** (2026-11-09; `r36s/reports/2026-11-09-r36s-report.txt`): 15 PASS, 0 FAIL on one R36S under r3i6nos. The package installed from `native_deb/` and its report ran from Settings > Device reports; all 10 commands in `commands.txt` ran in 0.02 to 0.05 s each (0.3 s in all) with at most 11.5 MB of memory, and the unknown-recipe case exits 2 with its message. Not compared with Node's output on the unit.
- **R36S command-line port** (`r36s/`, maintainer's request): the `animate` command as one QuickJS executable for the R36S, built from `cli.js` unchanged by the r3i6nos repo's `tools/qjs-node/` (shims for the `node:` modules, bytecode only, libc and libm). `r36s/check.sh` compares it with Node on `r36s/commands.txt`: 10 of 10 commands identical. `r36s/build-deb.sh` packs a tool `.deb` under KlamANSI's free R36S grant (`r36s/package/copyright`), a **draft**: `ANIMATOR_RELEASE=1` refuses a build while it has its marker. No change to any recipe or output; no version bump. Not run on a real R36S or on GitHub.
- **License is proprietary again** (was Apache-2.0), following Composer and KlamANSI, since the three travel together (maintainer's decision, 2026-11-08): `LICENSE` (copyright ansiapps, all rights reserved), `package.json` `"license": "UNLICENSED"` (already private), `NOTICE`, the README section. No code changed. Copies already distributed under Apache-2.0 stay under it.

## 0.3.1 (repeated-pose presets)

- **Five type presets regenerated from Painter** (`types.js`, following Painter's fix): `head.nod`, `object.pulse` and `effect.pulse` are now 2-frame loops (`[{}, move]`) instead of 3
  frames ending on their first pose; `structure.creak` and `lettering.wave` drop a pose that repeats the one before it (a new integer `distinct` helper), so `creak` at its defaults is 0, 1, 0, -1
  (4 frames, was 6). **Output changes** for these five (15 golden entries updated); walk, run, jump and the other type presets are unchanged. They now lint clean on `still`.
  This removes the "Known" note under 0.3.0.

## 0.3.0 (motion types, Painter 1.20.0)

- **Motion types** (`types.js`: `MOTION_TYPES`, `MOTION_OF`, `motionTypeOf`, `motionPresets`, `motionClip`, `lintMotionType`): Painter's 14 types (`figure`, `head`, `creature`,
  `object`, `limb`, `plant`, `machine`, `structure`, `tiling`, `scenery`, `cloth`, `effect`, `panel`, `lettering`) with their presets, params and channel limits. Every preset
  is a recipe `<type>.<preset>` (`figure.hop`, `cloth.gust`, `tiling.scroll`; 57 in all), registered with variants, and makes the clip Painter's own preset makes. Params are the
  preset's own plus `w` and `h` (the art's size, default 32 by 32) for the size-led presets. Integer-only: the waves are a rational curve, rounded, not `Math.sin`.
- **`wrap` on a clip** (`clip(..., { wrap })`, `Clip.wrap`): written to the file only when true, so every existing file is byte for byte as before. `tiling` presets set it. A wrap clip has
  no seam rule, and its frame limit is 64 (16 otherwise).
- **Lint:** a clip made by a type recipe is checked against its type: `channel` (error, a channel the type does not use) and `range` (warn, past a share of the art's size, replacing the
  sprite limits for these clips). Locomotion clips are checked as before.
- **CLI:** `animate types [type]`; `show` also prints `dx` and `lift` and says when a clip wraps.
- No existing output changes (walk, run, jump hashes are the same); 172 new golden entries.

## 0.2.0 (license, Painter motion)

- **License:** Apache 2.0, Copyright 2026 ansiapps, matching Painter (`LICENSE`, `NOTICE`; `package.json` `license` was `UNLICENSED`). The package stays private.
- **Painter integration (Painter 1.19.0):** a clip goes straight into Painter's `motion` param on any recipe (`character` draws each pose, other art moves its first frame by
  `dx dy lift squat lean sway`); `bind` stays for custom drawing. Docs and plan updated, and a test checks every variant's clip against that contract. No clip output changes.

## 0.1.0 (first cut)

- **Scaffold,** modelled on Painter and Composer: seeded `rng`, a self-contained package with a browser-safe main entry, recipes with
  `variants()`, a data-only kit, a lint of craft rules, an exact JSON artifact (`animator-clip`), the `animate` CLI, a golden test and
  the rules in `CLAUDE.md`.
- **Clip model:** integer pose tracks on Painter's `character` channels plus `air`; `fromKeys` blends keys into any frame count.
- **Recipes `walk` (4 frames), `run` (4) and `jump` (6, one-shot):** the minimal common loops. New recipes, so no existing output changes.
- **Lint:** `shape`, `frames`, `seam`, `still`, `range` and per-kind rules (a walk keeps a foot down, a run has flight, a jump leaves and lands).
- **`tempo`:** fit a clip to a bpm for Composer: whole-ms frame lengths that sum exactly, event times and ticks on the 96 ppq grid.
- **`bind`:** draw each pose with a function the caller passes (Painter), giving a clip in the shape of Painter's own clip record.
- **Kit `locomotion`:** walk, run and jump covered; idle, jump with travel, sneak, fall and roll, climb and swim, dash and slide are gaps.
