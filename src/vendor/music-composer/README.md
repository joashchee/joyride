# music-composer

A dependency-free music composing engine: seeded, deterministic recipes write melodies, chords, basslines, drum grooves, whole songs,
game stingers and looping ambient beds onto a score, with a lint, a MIDI codec, a WAV preview synth and a CLI. Same recipe, params and
seed, same notes. It imports nothing outside this folder, and the main entry has no Node built-ins behind it, so it runs unchanged in
a page, a worker, Node or Deno. Two tests enforce both.

Composer does for music what Painter (`art-sprites`) does for art, and is built the same way: recipes, kits, lint, codecs, a CLI.

## Use it

```js
import { compose, toMidi, render, toWav, lint, transpose } from 'music-composer';

const art = compose('song', { form: 'verse-chorus', key: 'G', scale: 'major', seed: 'town' });  // { id, score, loop, sections, params }
const midi = toMidi(art.score);                  // Uint8Array: the exact artifact, integers only
const wav  = toWav(render(art.score));           // Uint8Array: a preview you can hear
lint(art.score, { loop: art.loop });             // [{ rule, level, message }]
const lower = transpose(art.score, -5);          // transforms return a new Score
```

In a page, `play(art.score, { loop: art.loop })` plays through Web Audio. A host that mixes stems itself can mute and cross-fade
`score.tracks` and use `art.sections` (`{ verse: { from, to } }` in ticks) as cue points.

```
node cli.js make song form=loop key=A scale=minor seed=night --out night.mid     # .wav by extension
node cli.js show bass pattern=walking --track bass                                # a piano roll in the terminal
node cli.js export jingle kind=win --dir out                                      # .mid, .wav and a provenance .json
node cli.js lint night.mid                                                        # the craft rules on any MIDI file
node cli.js variants drums --dir out                                              # every variant of a recipe
node cli.js kits --strict                                                         # what the kits ask for, and what is missing
```

## Recipes

| Recipe | Params | Makes |
|---|---|---|
| `melody` | `contour` (arch, rise, fall, wave, flat), `density`, `octave`, `span`, `progression`, `ending` | rhythm cells repeated A B A C, chord tones on strong beats, a tonic ending |
| `chords` | `progression`, `style` (block, pad, arp, strum, stab), `size` (3 or 4), `octave` | a voice-led progression, one chord a bar |
| `bass` | `pattern` (root, octave, root-fifth, walking, sustain), `progression` | a bassline in the octave below the chords |
| `drums` | `pattern` (rock, four, breakbeat, half, march, waltz), `fills`, `ghost`, `phrase` | a groove with ghost snares, phrase-end fills and crashes |
| `song` | `form` (verse-chorus, verse-chorus-bridge, AABA, rondo, loop), `drums`, `beats` | intro, verse, chorus, bridge and outro roles with all four parts; `bars` comes from the form |
| `blues` | `scale` (blues, minor, major, major-pentatonic, mixolydian) | twelve bars in three four-bar lines over I IV V |
| `modulation` | `to` (fifth, fourth, whole-step, semitone, relative, parallel), `bars`, `back` | home key, a pivot dominant, then the new key (and home again with `back`); `score.keyChanges` records it |
| `counterpoint` | `species` (1, 2, 3), `position` (above, below), `bars` | a cantus firmus and a counter-line to the strict rules; lint it with `voices: ['cantus', 'counter']` |
| `leitmotif` | `theme` (any name), `state` (calm, tense, triumph, lament, chase), `bars` | one theme, the same steps and rhythm, in a state's scale, tempo, instrument, octave and accompaniment |
| `phrase` | `type` (period, sentence), `cadence` (authentic, half, plagal, deceptive) | eight bars with the cadence you ask for |
| `jingle` | `kind` (pickup, win, lose, levelup, alert) | a one or two bar stinger |
| `ambient` | `mood` (calm, sad, tense, bright), `bars` | a looping pad, drone bass and sparse lead |

Every recipe takes `key` (`C`, `F#`, `Bb`), `scale` (major, minor, dorian, phrygian, lydian, mixolydian, harmonic, major-pentatonic,
minor-pentatonic, blues), `bpm`, `beats` (3, 4, 5 or 7 for melodies and drums) and `seed`. Progressions: pop, axis, doowop, rock, canon,
jazz-251, minor-sad, andalusian, minor-rise, pedal, or an array of scale degrees.

## Game music: cue sheets

`compose cues sheet.json --dir d` (or `composeCues(sheet)`) composes every cue of a sheet and writes each as `<id>.ntt` and
`<id>.mid`, plus `cues.json`, which tells the game how to play them. A cue is a recipe and its params:

```json
{ "title": "Moss Keep",
  "cues": [
    { "id": "explore", "recipe": "song", "params": { "form": "verse-chorus", "key": "D", "seed": "keep" },
      "layers": { "base": ["chords", "bass"], "melody": ["lead"], "percussion": ["drums"] }, "on": ["base"] },
    { "id": "hero-chase", "recipe": "leitmotif", "params": { "theme": "hero", "state": "chase" } },
    { "id": "found-key", "recipe": "jingle", "params": { "kind": "pickup" }, "tail": 2 } ],
  "transitions": [ { "from": "explore:verse", "to": "hero-chase", "at": "phrase", "via": "found-key" } ] }
```

- **Loops and stingers.** A piece longer than two bars, or one that loops, is a `loop` (linted for its seam); a short one plays once:
  a stinger gets a `tail` of empty beats so its release rings out, then the `.ntt` ends instead of restarting.
- **Vertical layers.** `layers` groups tracks as stems (every track in exactly one); `on` says which start audible and the rest start
  muted. The `.ntt` keeps the channel groups, and the manifest lists each layer's channels.
- **Horizontal sections.** A song's named sections are also order lists in the `.ntt` (each loops back to its start), and the
  manifest gives each section's bars and the bars where a host may switch (`switchBars`: section starts and every fourth bar).
- **Transitions** are data: from a cue or `cue:section` to another, at a `bar`, `phrase` or `section`, optionally through a stinger.
  Composer checks the references; the game does the switching, with `createCuePlayer` (below) if it is a page.
- **A cue player for pages, with the audio baked ahead of time.** `compose cues sheet.json --dir d --audio` also writes each cue as
  `<id>.wav`, or one `<id>.<layer>.wav` stem a layer (same length, same scale, so they sum to the mix; `--rate` sets the sample rate;
  a loop is exactly its length with the tail wrapped onto the start, so it seams). Serve them (encode to ogg or mp3 if size matters;
  the player decodes through `decodeAudioData`) and:

  ```js
  const player = createCuePlayer(ctx, manifest, await loadCueAudio(ctx, manifest, '/music/'), { volume: 0.8 });
  player.play('explore');            // idle: starts now, with the layers in `start`
  player.layers(['base', 'rest']);   // fade a stem in at the next bar
  player.play('battle');             // switches where the sheet's transition says (default next phrase), through its `via` stinger
  player.stinger('pickup');          // once, over whatever plays
  ```

  `play` is idempotent (asking for what plays or is queued does nothing; a different request before a switch lands replaces it), so a
  host can call it on every state change. Switches land on the music's own bar to the audio clock (`nextSwitch`), the old music fades
  over 40 ms, and `play('explore:chorus')` seeks to a section's bar. For size, `tools/encode_cues.sh d` (ffmpeg with libopus) writes `.opus` beside each WAV (about 6x smaller at 32 kbps), and
  `loadCueAudio(ctx, manifest, base, { formats: ['opus', 'wav'] })` takes Opus where the browser decodes it and WAV where not. Any WAV of the right length works, so Neumetik's renders can stand
  in for the preview synth's. Not done: ducking under a stinger, crossfading over more than a bar line, stems from Neumetik.
- **Leitmotifs.** The `leitmotif` recipe is one theme (named by `theme`) in a state's palette, so the same cue sheet can carry a hero
  theme through calm, tense and chase music.

The same options are on `toModule`: `lists`, `layers`, `on`, `tail`, `end`.

## Text notation

`toNotation(score)` writes a Score as plain text and `parseNotation(text)` reads it back, exactly (the MIDI bytes come out identical), so
a tune can be written by hand, kept in a repo and diffed. `compose notate <recipe|file.mid|file.cmp>` prints it, and a `name.cmp` file
works anywhere a recipe does (`compose make tune.cmp --out tune.mid`, `lint`, `show`, `export`).

```
title: Night walk
bpm: 96
key: A minor
track lead program=80 wave=square
| A4:q C5 E5 A5!70 | G5:h~ G5:h |
track chords
| [A3 C4 E4]:w | [G3 B3 D4]:w |
track drums drum
| kick:q hat snare hat | kick hat snare:q hat |
```

A note is a pitch name (`C4` = 60, `F#3`, `Bb2`, or `n60`; on a `drum` track the General MIDI names kick, snare, hat...) with an optional
`:duration`, which sticks until changed: `w h q e s t`, dotted with `.`, or a number of ticks. `[C4 E4 G4]` is a chord, `r` a rest, `!80`
sets the velocity from there on, `~` ties to the same pitch that follows, and a second voice line under a track plays with the first.
Every bar must add up to the meter; the error names the line and the bar. `bars:`, `meter:` (beats a bar), `bpm:`, `key:` and
`key@TICK:` (a modulation) are the headers.

## How it is built

- A `Score` holds named tracks of `{ p, t, d, v }` notes: pitch 0..127, start and length in ticks (`PPQ` 96, so `STEP` 24 is a
  sixteenth), velocity 1..127. Everything is an integer, so the MIDI bytes are identical on every machine.
- Recipes are `(params) => { id, score, loop, sections }`. They are built from four writers (`addMelody`, `addChords`, `addBass`,
  `addDrums`) that add a part at a bar offset, which is also how a song is built, section by section. Each takes a seeded `rng`.
- Randomness comes only from `rng(seed)`; `r.fork(label)` gives a child stream that depends on the seed and label alone, so adding a
  draw in one part never shifts another.
- Kits (`kits/`) are data: subjects, the recipes that compose each, and a status (`covered`, `partial`, `built-in`, `gap`,
  `reference-only`). `compose kits` composes and lints every recipe a kit names, so a kit that points at a missing recipe fails.

| Painter | Composer |
|---|---|
| `Pixels` canvas | `Score` |
| `palette.js` DB32 and ramps | `scales.js` scales, chords, progressions |
| `shade.js` | `voicing.js` |
| `anim.js` | `transform.js` |
| `lint.js` | `lint.js` |
| `png.js` | `midi.js`, `wav.js` |
| `render.js` canvas helpers | `render.js` Web Audio, `text.js` piano roll |
| `sprites` CLI | `compose` CLI |

## Limits

- The preview synth is oscillators and noise drums: a sketching voice, not a mix. Real sound is the host's job: send the MIDI to a
  sound bank, or mix the tracks yourself.
- MIDI key signatures only say major or minor, so a round trip keeps the notes but turns `dorian` into `minor`.
- Five forms, whole beats from three to seven, one chord a bar. Counterpoint is two voices in the first three species; modulation is
  a pivot dominant then a direct change. MIDI keeps the first key signature only (`score.keyChanges` is not written to the file).
- Generated music is a drafting engine for prototypes and placeholders, not a replacement for a composer.
- No copyrighted tunes, samples or recordings are used or encoded (`CLAUDE.md` rule 9).

Tests: `npm test` here (they need no files from a host project). Types: `index.d.ts`.

MIT, Copyright 2026 ansiapps (see `LICENSE`); see `NOTICE` for what it includes from others.

## Hero features (the R36S Rec demo)

The R36S build records a tour of these (`compose-demo`, or Settings > Rec demos; `--features a,b` over SSH). The tour is the commands under `#@feature` lines in `r36s/commands.txt`
(the same list the device report times), so a hero feature is a group of commands there:

- `kits`: The sound kits and their patterns
- `songs`: Songs from a seed, checked
- `sounds`: Jingles and ambience as audio files
- `notation`: Notation and variants
- `export`: Export for a game

