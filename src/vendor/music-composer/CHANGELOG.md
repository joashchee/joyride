# Changelog

## Unreleased

- **Rec demo for the R36S build** (2026-11-12, maintainer's request; r3i6nos `tools/qjs-node/demo.py`): `r36s/commands.txt` has `#@hero`, `#@feature` and `#@say` comment lines (the report and the Node comparison ignore them), so the package installs `compose-demo`: 11 steps of the tool's own commands drawn on the theme screen, about 60 s, with a `== power ==` section in its log. Composer's own .wav outputs (the jingle, the ambient loop) are its sound. The commands moved so each feature is contiguous (the same lines; `kits --strict`, `lint melody`, `lint nothing` and `make` stay after the tour). Built and run in Debian 12 arm64 (`r36s/build-deb.sh`, `--no-video`); **not run on a unit**.
- **R36S: the fourth device report, the first with a `== power ==` section** (2026-11-12 16:31, the unit's clock right this time; `r36s/reports/2026-11-12-1631-r36s-report.txt`): 21 PASS, 0 FAIL. Times as before within 4%: `kits` 3.95 s (4.25), the ambient loop 15.88 s (16.16), the jingle 1.62 s; total 27.8 s; peak 12.4 MB (11.7 MB before). **Power** (88% charge, on battery): each command is 100% of one core, wake-ups 2 to 24 a second (goal 200: PASS), draw 1417-1544 mW against 1224 mW idle (+265 mW; 7.2 J over idle for the 5 commands of 1 s or more), 7.7 h left at that draw. The report's "cut first": `make ambient` is 57% of the CPU seconds (15.9 of 27.7). No code changed. One run: repeat at 50% and 20% charge before trusting small differences.

- **R36S: a third device report, the same result** (2026-11-12; `r36s/reports/2026-11-12-r36s-report.txt`): 20 PASS, 0 FAIL, 11.66 MB peak; `kits` 4.25 s, the ambient loop 16.16 s, total 28.6 s, within about 5% of 2026-11-09's runs. The report has no `== power ==` section yet (it waits on r3i6nos's `tools/qjs-node/report.py`). The unit's clock stamps it 2025-11-12, a year behind (set by hand on the unit).
- **Dates corrected** (2026-11-09): entries dated 2026-11-10 are 2026-11-09, and the first report is renamed `r36s/reports/2026-11-09-r36s-report.txt`. The date came from the report's stamp, made while the unit's clock ran about 15 hours fast; the stamp itself is kept, quoted where it is explained.
- **`ROADMAP.md`** (2026-11-09): what is left, from `docs/composer-plan.md` (phase 9, the open parts of phases 6 and 7, listening), `docs/tag-team-music-plan.md` (its open decisions) and the R36S port.
- **R36S: a second device report, the same result** (2026-11-09 22:30; `r36s/reports/2026-11-09-2230-r36s-report.txt`): 20 PASS, 0 FAIL on the same unit, 11.6 MB peak (was 11.5). The times match the first run to within 2%: `kits` 4.04 s, a jingle 1.67 s, the ambient loop 16.08 s (16.35 s before), 28.3 s in all. So the first figures hold. The first report's stamp, 2026-11-10 03:36, came from the unit's clock running about 15 hours fast (it was committed 2026-11-09 12:34); its file is now named for the day it ran.
- **R36S: the first device report from a unit** (2026-11-09; `r36s/reports/2026-11-09-r36s-report.txt`): 20 PASS, 0 FAIL on one R36S under r3i6nos, at most 11.5 MB of memory. About 18 times this Mac's native QuickJS build: `kits` 4.05 s (0.23 s here), a 4.1 s jingle in 1.67 s, a 26.7 s ambient loop in 16.35 s (1.6x real time, so made ahead of use), MIDI and notation 0.03 to 0.19 s. Not compared with Node's output on the unit.
- **R36S command-line port** (`r36s/`, maintainer's request): the `compose` command as one QuickJS executable for the R36S, built from `cli.js` unchanged by the r3i6nos repo's `tools/qjs-node/` (shims for the `node:` modules, bytecode only, libc and libm). `r36s/check.sh` compares it with Node on `r36s/commands.txt`: 15 of 15 commands identical, the `.wav` byte for byte. `r36s/build-deb.sh` packs a tool `.deb` (no menu entry; the command on every app's `PATH`). No change to any recipe or output. It ships under KlamANSI's free R36S grant (`r36s/package/copyright`, holder ansiapps as KlamANSI's), a **draft**: `COMPOSER_RELEASE=1` refuses a build while it has its marker. `LICENSE` and `package.json` still name Joash Chee as holder. Not run on a real R36S or on GitHub.
- **License is now proprietary**: `package.json` sets `"license": "UNLICENSED"` (npm's value for a package no one is granted rights to use) and `"private": true` (npm refuses to publish it). `LICENSE`: copyright Joash Chee, all rights reserved.
- **`createCuePlayer(context, manifest, audio, opts)` and `loadCueAudio`** (`cueplayer.js`): a Web Audio cue player for a page. `play(ref)` starts a
  cue now, or switches at the sheet's `at` (default next phrase) through its `via` stinger, landing on the music's own bar on the audio clock;
  `layers([...])` fades stems at the next bar; `stinger(id)` plays over the music; `stop`, `setVolume`, `position`, `state`. Asking for what
  plays does nothing, and a new request before a switch lands replaces it. Browser-safe (no DOM at import, no Node built-ins). Tested against a recording fake context.
- **`compose cues --audio`**, `audioFiles`, `renderCue`, `cueAudio`: bake each cue to WAV with the preview synth, a mix or one stem per layer
  at one shared scale; loops are exactly their length (they seam), stingers keep their tail. `--rate` sets the sample rate.
- **Compressed cue audio.** `loadCueAudio(..., { formats: ['opus', 'wav'] })` tries each extension in order and falls through to the next when a
  file is missing or will not decode. The player now loops each source on the cue's exact length (`loopStart`/`loopEnd`), so the few
  milliseconds of padding a compressed file decodes with do not click at the seam. `tools/encode_cues.sh` encodes a cue folder's WAVs to Opus
  with ffmpeg (about 6.6x smaller at 32 kbps on the test stem; not listened to). Not in the package.
- `render()` takes `tracks` (only those tracks) and `normalize: false`. Defaults unchanged, so no existing output moves.
- Not done: ducking under a stinger, stems rendered by Neumetik (a Neumetik WAV of the right length plays as a mix today), the ear check of the player's seams and fades.
- **`playScript(manifest, { hold })`** and `compose cues` writing `play.txt`: a play script for `ntt_play --script` generated from the cue
  sheet's switching (start the first loop, bring its quiet layers in, play unbridged stingers, then each transition at the next legal bar,
  `sequence` within a cue or `transition`/`cue` to another). Nothing existing changes; `cues.json` is unchanged. Checked in Neumetik by the numbers (below), not yet by ear.
- **`tools/neumetik/eartest_cues.html` and `render_cues.sh`**: an Eartest adapter page for a cue sheet and the script that renders `compose cues` output to `audio/*.wav` through `ntt_play` (loops cut at one pass to hear the seam, stingers with tails, and the switching walkthrough). Source to carry over to Neumetik's `eartest/`; not in the package.
- **Checked with `ntt_play`** (a four-cue sheet, Neumetik `0.30.0`): the generated script's predicted landing times matched the player's (47.14, 49.63, 68.91 s against 47.14, 49.64, 68.93 s); layers came in as scripted; a free stinger added sound for exactly its length and nothing after, with the music underneath unchanged (rendered with and without it and subtracted). The stinger's tail is silent rows, not a ring-out.

## 1.2.0

- **Phase 6 leftovers.** The `song` recipe takes `theme`: the chorus states that leitmotif (`motifOf`, same steps and rhythm as the
  `leitmotif` recipe) as its lead in the song's own key and scale; four beats and a seven-note scale only (`addTheme` in
  `recipes/motif.js`). New song variants with a theme. `nextSwitch(entry, bar, at)` answers the switching rule for a cue's manifest
  entry: the next bar, phrase or section start a host may switch at, or the loop end. Songs without `theme` are unchanged.
- **`tools/neumetik/ntt_play.rs`** carries over Neumetik's `--script` mode: a play script drives the tracker's queued changes (sections, cues, transitions, stingers, layer fades) to a WAV. Source only; not in the package.

## 1.1.0

- **Phase 6: game-music structure** (`cues.js`, `recipes/motif.js`, `module.js`, `tracker.js`, `ntt.js`).
  `composeCues(sheet)` and `compose cues sheet.json --dir d`: a cue sheet (recipe and params per cue, `layers`, `on`, `tail`,
  `transitions`) becomes an `.ntt` and `.mid` per cue and a `cues.json` manifest (kind, length, sections with their bars and order
  list, `switchBars`, layer channels, starting layers, tail, warnings, `Project-created` provenance); every problem in a sheet is
  reported together. `toModule` gains `lists` (each named section as an order list that loops, written in the `SONG` chunk),
  `layers` and `on` (stems as channel groups, the others start muted through the channel flags), `tail` (empty rows after the last
  note) and `end` (an end-of-song order, so a stinger plays once); `report.stems` maps tracks to channels. `Module` gains `lists`,
  `meta`, `addList`, `mute` and `end`; `parseNtt` reads them back, and `meta` (layers, sections) rides in Composer's own `CMPR` chunk.
  New recipe `leitmotif` (`theme`, `state`: calm, tense, triumph, lament, chase): the theme's steps and rhythm are made from its name
  alone and stay the same in every state, while scale, tempo, instrument, octave, speed and accompaniment change. Checked in
  Neumetik's reader (`umf_player`): a multi-list cue with muted layers loads and plays, and a stinger stops at its end. Not done:
  the whole-song replayers (phase 9), and no leitmotif is yet stated inside a longer song.
- **Phase 8: text notation** (`notation.js`). `toNotation(score)` and `parseNotation(text)`: a line-based text format (headers, `track`
  lines, bars between `|`, sticky durations and velocities, chords, ties, rests, drum names, extra voices, `key@TICK:` for
  modulations) that round-trips every integer of a Score, checked for every registered variant (identical MIDI bytes). Bars that
  do not add up are refused with the line and bar. `compose notate`, and a `.cmp` file works wherever a recipe does in the CLI. The
  soundfont hook of phase 8 is not built: Neumetik plays General MIDI without one.
- **Phase 7: theory depth** (`recipes/theory.js`). New recipes, each with variants, kit rows and tests:
  `blues` (twelve bars, three four-bar lines over I IV V, in blues, minor, major, major-pentatonic or mixolydian);
  `modulation` (`to` fifth, fourth, whole-step, semitone, relative or parallel, through a pivot dominant one bar before the change;
  `back` returns home); `counterpoint` (cantus firmus plus a counter-line, species 1 to 3, above or below, found by a seeded depth-first
  search under the textbook rules: perfect ends, consonance on strong beats, passing dissonance only, no parallel fifths or octaves,
  no similar motion into a perfect interval, leaps recovered by step, the close approached by step); `phrase` (an eight-bar period or
  sentence ending on an authentic, half, plagal or deceptive cadence).
  A `Score` now has `keyChanges`, `keyAt(t)` and `modulate(t, key, scale)`; `lint`'s `scale` rule checks each note against the key in
  force where it starts, and `transpose`, `stretch`, `slice` and `concat` carry key changes. MIDI still writes only the first key.
  `lint` takes `voices: [a, b]` for two counterpoint tracks and adds `parallel` and `dissonance`; a recipe whose art has `lint` options
  (counterpoint) has them applied by `compose lint`, the kits report and the tests. `voicing.js` exports the shared interval rules.
  Phrase length is not a lint rule: the writers fix phrase length by construction.
- **Odd meters and more forms.** Melodies, songs and drums work in five (3 + 2) and seven (4 + 3) beats a bar (`quintuple` and
  `septuple` drum patterns; walking bass fills the extra beats). Songs gain `verse-chorus-bridge` (a bridge role with new chords and no
  drums) and `rondo` (A B A C A). No existing composition changed: the 50 earlier golden hashes are identical; the golden file gained
  the new variants.
- **Phase 5: the bake manifest and Neumetik's engines** (`bake.js`,
  `tools/neumetik/render_manifest.rs`). `engine: 'bake'` (per track, with
  `source`, `recipe`, `note`, `ms`, `loop`, `seed`) lists what each
  instrument should sound like; `toManifest` writes `instruments.json`, the
  Neumetik-side tool renders each slot to a WAV, and `bake` puts the
  waveforms in the module at the right pitch (`parseWav` reads them). Until
  a WAV exists an instrument keeps its own-synth sound, so the `.ntt` always
  plays. `compose bake <recipe> --dir d` does both halves (it writes
  `d/instruments.json` and `d/<name>.ntt`, baking whatever is in `d/wavs`).
  Sources: Neumetik's MIDI synthesizer (`midi`, `midi.drum`; GM and the
  classic banks), BGN beds (`bgn`), chipbank (`chipbank`), and the SN76489
  and NES chips (`chip.sn76489`, `chip.nes`); the tool names the rest of
  the chips as not yet rendered. Each is rendered near the middle of the
  notes it plays, beds and hits at C-5 whatever the score says. Checked in
  Neumetik: every source sounds at the right pitch (piano at 60 and 72 gives
  262 and 524 Hz, the chips 262 and 131), and a whole baked song plays in
  the tracker alone. No existing MIDI output changes.
- **Phase 4: samples from Composer's own synth** (`samples.js`).
  `engine: 'sample'` gives every instrument its own sound, so an `.ntt`
  plays in any tracker player with no MIDI host: a pitched track is one
  looped 64-frame cycle of its wave (`cycle`) under a volume envelope made
  from the track's attack/decay/sustain/release (written into `INST`, in
  ticks of the song's own tempo, held at the sustain until the note ends),
  and each drum key is a one-shot of the percussion table (`drumShot`).
  `compose make x.ntt --engine sample`. `synth.js` now exports `OSC`,
  `DRUMS`, `noise` and `DEFAULT_ENV`; its output is unchanged. Checked in
  Neumetik's player: all four waves sound at 262 Hz for C-5, hold the
  sustain and release on cue; the drums sound; a whole song plays alone
  (peak 0.85). The samples come from `Math.sin` and `Math.exp`, so they
  are exact on V8 but not guaranteed byte for byte on other engines; the
  MIDI stays the exact artifact.
- **Playback through Neumetik's MIDI synthesizer** (`tools/neumetik/ntt_play.rs`).
  A MIDI-engine `.ntt` is silent in the tracker alone, since the tracker only
  hands MIDI to a host. `ntt_play` plays an `.ntt` the way the Neumetik app
  does: the tracker (FM, samples, effects) mixed with `neumetik::Synth` fed
  the tracker's MIDI messages; a `.mid` goes straight to the synthesizer.
  Source only, to carry over to `neumetik/examples/`; not in the package and
  not imported. Checked: a MIDI-engine `.ntt` renders as loud as its `.mid`
  (peak 0.27 against 0.26) where the tracker alone gave silence.
- **Phase 3: Neumetik's own engines in the `.ntt`.** `toModule` takes
  `engine` (`'opl'`, `'midi'`, or per track `{ lead: { engine: 'midi',
  program, bank }, '*': 'opl' }`) and `effects`. `opl.js`: `fmPatch` builds
  the eleven OPL2 register bytes from named fields, with eight generic
  presets (`OPL_PATCHES`); written as `SYNV`. MIDI output is `INSX` (drums
  on channel 9 play their own key); effect slots (reverb, delay, chorus,
  flanger, distortion, compressor, equalizer, filter, gargle) are `EFCT`,
  routed per instrument. `Module` gains `addFm`, `addEffect`, `effects`.
  Kit rows may name an `engine`. `compose make x.ntt --engine opl|midi
  --reverb 0.3`. Checked in Neumetik's player: FM patches sound at the right
  pitch (262 Hz for C-5), reverb changes the render, and MIDI-out sends the
  expected program changes, channels and keys. No existing MIDI output
  changes.
- **Phase 2: `.ntt` writer and reader** (`ntt.js`). `toNtt(module)` writes
  Neumetik's UMF with its samples inside (INFO, CHAN, ORDR, PATT, SMPL, INST,
  SONG when rows a beat differ, a Composer `CMPR` provenance chunk, WAVE,
  END), with waveform, sample and instrument IDs computed as a repository
  would; `parseNtt` reads it back and checks every waveform against its ID.
  Streams are stored, not compressed. `compose make x.ntt` and
  `compose export --ntt` (`--rows` sets rows a beat). Until samples are filled
  in (phases 4 and 5) the instrument slots are empty and the file is silent.
  Checked against Neumetik's own `umf_player`: it reads the files and plays
  embedded 8- and 16-bit samples. `Module` gains `time`.
- **Phase 1: `toModule(score, opts)`** (`tracker.js`) turns a Score into a
  `Module`: notes quantised to rows (`rowsPerBeat`, default 4), one voice per
  simultaneous note, one instrument slot per pitched track and drum key,
  velocity to volume column, speed and tempo chosen so a row lasts a
  sixteenth, patterns cut at whole bars or section starts and identical ones
  kept once. Returns a report of what moved or was dropped. `Module` gains
  `restart`. No existing output changes.
- **Phase 0 of the `.ntt` output** (`docs/composer-plan.md`): `sha256.js`
  (FIPS 180-4, for waveform ids), `bytes.js` (little-endian `ByteWriter` and
  `ByteReader`, strings and chunks as Neumetik's UMF has them), `nlz.js`
  (stored NLZ1 streams; coded ones are refused) and `module.js` (`Module`:
  patterns of rows x channels, orders, samples; the model an `.ntt` is
  written from). No existing output changes; golden hashes are untouched.
- **Own repo.** Composer moved out of `neumetik/composer/` into its own
  private repo, `~/Coding/composer`. No code changed; the docs no longer
  say it lives inside neumetik. Version 1.0.1.

- **Composer 1.0.0, first cut**, modelled on Painter. `Score` (integer notes
  on a 96 ppq grid), scales, diatonic chords and progressions, voice
  leading and arpeggios, transforms (transpose, invert, retrograde, stretch,
  repeat, slice, concat, layer, swing, humanize), a lint (`empty`, `length`,
  `overlap`, `range`, `scale`, `dynamics`, `leap`, `repeat`, `seam`,
  `density`), a Standard MIDI File codec, a preview synth with a WAV
  writer, Web Audio helpers and a terminal piano roll.
- **Recipes**: `melody`, `chords`, `bass`, `drums`, `song` (verse-chorus,
  AABA, loop), `jingle` (pickup, win, lose, levelup, alert) and `ambient`
  (calm, sad, tense, bright).
- **Kits**: `game-music` and `song-craft`; `compose kits` lists what is
  covered and which rows are gaps (modulation, counterpoint).
- **CLI** `compose`: `kits`, `make`, `show`, `variants`, `lint`, `export`.
- Tests: determinism, MIDI round trip and lint for every variant, music
  rules, kit integrity, the self-containment rules, and golden MIDI hashes.
