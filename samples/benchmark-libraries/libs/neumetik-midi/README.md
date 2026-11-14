# Neumetik MIDI

**MIDI music for games, with no SoundFont.** A General MIDI and Roland GS
synthesizer where every instrument is synthesized as it plays (subtractive,
FM and plucked-string engines, and a recipe for every drum), so there are no
samples to download. It turns MIDI into samples, and your engine plays them.
When you do want sampled instruments, it plays through a SoundFont or a DLS
bank too.

- **Small:** 80 KB of WebAssembly, gzipped. Linked into a native program it
  adds about 440 KB.
- **Light:** under 1% of one CPU core for typical game music at 48 kHz, and
  about 1.2 MB of memory, allocated once. Rendering never allocates, locks
  or panics.
- **No dependencies:** no SoundFont, no audio API, nothing to install. You
  get float or 16-bit PCM, interleaved or not.
- **Or a SoundFont or DLS bank:** give it a `.sf2` or a `.dls` (DLS level 1
  or 2) and it plays that bank's instruments, its own filling in for any
  the bank hasn't. Still no dependencies: it reads both itself.
- **And makes SoundFonts:** its own instruments packed as an `.sf2` for any
  SoundFont player, a DLS bank turned into one, a bank trimmed to what a
  song plays, or a new one from your own WAV files.
- **And reads music written as text:** MusicXML, ABC, MML (Mabinogi,
  MapleStory 2 and the BASICs' `PLAY`), ChordPro, Humdrum `**kern`, Alda,
  RTTTL ring tones and Strudel/Tidal mini-notation, made into MIDI to play.
- **Made for games:** loops (from the file's markers or your own points),
  seeking, fades, tempo, channels muted and unmuted for music in layers, and
  notes played live over the music.
- **Everywhere:** Rust, a C API, WebAssembly, and bindings for JavaScript
  (Phaser and any Web Audio game), LÖVE, C# (MonoGame, FNA, Unity, Godot),
  Godot (GDScript), Qt and raylib.
- **Free:** MIT licensed.

Hear it, play it, compare it with a SoundFont or DLS bank and see the benchmarks on the
demo page (`demo/`; see [Building](#building)).

## Contents

- [Quick start](#quick-start)
- [Your engine](#your-engine)
- [How it works](#how-it-works) (players, loops, seeking, layers, playing live, threads, levels)
- [SoundFonts and DLS](#soundfonts-and-dls)
- [Making SoundFonts](#making-soundfonts)
- [Music as text](#music-as-text)
- [The C API](#the-c-api)
- [What it plays](#what-it-plays)
- [Size and speed](#size-and-speed)
- [Building](#building)
- [Limits](#limits)

## Quick start

### Rust

```toml
[dependencies]
neumetik-midi = "0.2"
```

```rust
use neumetik_midi::Player;

let mut music = Player::new(48_000);
music.load(&std::fs::read("overworld.mid")?)?;
music.set_looping(true);   // between the file's loop markers, or the whole song
music.play();

// In your audio callback (or whenever your engine wants more):
let mut buffer = [0f32; 1024];          // 512 frames, left and right interleaved
music.render_interleaved(&mut buffer);  // or render(&mut left, &mut right), render_i16
```

### C or C++

```c
#include "neumetik_midi.h"   /* link libneumetik_midi.a, or the .so / .dylib / .dll */

nm_player *music = nm_create(48000);
int32_t err = nm_load(music, midi_bytes, midi_size);
if (err != NM_OK) printf("%s\n", nm_error_string(err));
nm_set_looping(music, 1);
nm_play(music);

nm_render_f32(music, buffer, frames);   /* interleaved stereo; or nm_render_s16 */

nm_destroy(music);
```

`capi/examples/render_wav.c` plays a MIDI file into a WAV file using most of
the API.

### JavaScript

```js
import { createMusicNode } from './neumetik-midi.js';   // neumetik_midi.wasm beside it

const music = await createMusicNode(audioContext);       // an AudioWorklet
music.node.connect(audioContext.destination);
await music.load(await (await fetch('overworld.mid')).arrayBuffer());
music.setLooping(true);
music.play();
```

## Your engine

| Engine | Use | Where |
| --- | --- | --- |
| Any Rust engine (Bevy, macroquad, ggez, cpal) | the crate | this folder |
| C / C++ (SDL, SFML, miniaudio, your own) | `neumetik_midi.h` + the library | `capi/` |
| raylib | an `AudioStream` fed from the game loop | `bindings/raylib/raylib_music.c` |
| Phaser, PixiJS, three.js, plain Web Audio | `createMusicNode` (AudioWorklet) or `renderToAudioBuffer` | `bindings/js/` |
| LÖVE | `neumetik_midi.lua` (LuaJIT FFI, a `QueueableSource`) | `bindings/love/` |
| MonoGame, FNA, Unity, Godot C#, .NET | `NeumetikMidi.cs` (P/Invoke) | `bindings/csharp/` |
| Godot (GDScript) | the `NeumetikMusic` node, a GDExtension | `bindings/godot/` |
| Qt 6 | `NeumetikMidiDevice`, a `QIODevice` for `QAudioSink` | `bindings/qt/` |
| Anything with a C FFI (Python ctypes, Zig, Nim, Odin, Go) | the C API | `capi/include/neumetik_midi.h` |

The Rust crate, the C API, WebAssembly, the JavaScript binding and the LÖVE
binding are tested. The C#, Godot, Qt and raylib bindings are written to
those engines' APIs but haven't been built against them in CI yet.

### Phaser

```js
import { createMusicNode } from './neumetik-midi.js';

class Level extends Phaser.Scene {
  async create() {
    this.music = await createMusicNode(this.sound.context);
    this.music.node.connect(this.sound.masterVolumeNode);   // follows Phaser's volume and mute
    await this.music.load(await (await fetch('assets/overworld.mid')).arrayBuffer());
    this.music.setLooping(true);
    this.music.play();
  }
  bossAppears() {
    this.music.fadeTo(0, 1.5);
  }
}
```

Browsers start audio only after the player clicks or presses a key, and
Phaser resumes its context then. `createMusicNode` needs the page served
over HTTP (an AudioWorklet can't load from `file://`). For music as an
ordinary sound instead, `renderToAudioBuffer(context, midi, { loops: 2 })`
renders it whole.

### LÖVE

Put `neumetik_midi.lua` and the library for each platform
(`libneumetik_midi.dylib`, `.so`, `neumetik_midi.dll`) beside `main.lua`:

```lua
local NM = require("neumetik_midi")

function love.load()
  music = NM.new()                                   -- 44.1 kHz, a QueueableSource
  assert(music:load(love.filesystem.read("overworld.mid")))
  music:setLooping(true)
  music:play()
end

function love.update(dt)
  music:update()                                     -- keeps about 93 ms queued
end
```

`bindings/love/main.lua` is a whole example: pause, fades, muting the drums,
and notes played on the keyboard.

### MonoGame, Unity, Godot C#

See `bindings/csharp/README.md`. In short:

```csharp
var player = new NeumetikMidi.Player(44100);
player.Load(File.ReadAllBytes("overworld.mid"));
player.Looping = true;
player.Play();
player.Render(buffer);   // float[], short[] or byte[] (16-bit, for DynamicSoundEffectInstance)
```

### Godot (GDScript)

Build the GDExtension (`bindings/godot/README.md`), then:

```gdscript
@onready var music: NeumetikMusic = $Music

func _ready():
    music.load_file("res://music/overworld.mid")
    music.looping = true
    music.play()
```

### raylib

```c
AudioStream stream = LoadAudioStream(44100, 16, 2);
nm_player *music = nm_create(44100);
nm_load(music, data, size);
nm_play(music);
PlayAudioStream(stream);
/* each frame: */
while (IsAudioStreamProcessed(stream)) {
    nm_render_s16(music, buffer, 1024);
    UpdateAudioStream(stream, buffer, 1024);
}
```

Set `SetAudioStreamBufferSizeDefault(1024)` before `LoadAudioStream` so the
two agree. See `bindings/raylib/raylib_music.c`.

## How it works

### Three ways in

- **`Player`** plays a song: load, play, pause, stop, seek, loop, speed,
  volume, fades, mutes. Use it for a game's music.
- **`Synth`** is the synthesizer alone, driven by MIDI messages as they
  come. Every player has one (`player.synth()`, `player.synth_mut()`), so
  you can play notes over the music.
- **`Sequencer`** queues MIDI files back to back with no gap. Use it for
  music written as it plays.

`render(bytes, rate, tail, longest)` renders a whole file at once, and `wav`
turns samples into a WAV file.

### Loops

`set_looping(true)` loops between the song's loop points:

1. points you set with `set_loop_points(start, end)` (in seconds);
2. otherwise the file's own: a marker named `loopStart` and one named
   `loopEnd` (any case, with or without a space, `_` or `-`), or CC 111 at
   the loop's start (as RPG Maker writes it). A start with no end loops to
   the end of the song;
3. otherwise the whole song, from its start to its last End of Track.

At the loop's end, held notes are let go, every channel returns to the
instrument and controller settings it has at the loop's start, and playing
carries on from there with no gap. The reverb and the release of the last
notes ring on into the loop.

Tips for MIDI files that loop well:

- End the loop on a bar line: put the `loopEnd` marker there, or make the
  track's End of Track fall there.
- Most DAWs can export markers in a MIDI file. Name them `loopStart` and
  `loopEnd`.
- Set each channel's instrument and volume before the loop starts, or again
  at its start.

### Seeking

`seek(seconds)` silences what's sounding, then works out what every channel
plays at that moment (program changes, bank selects, controllers, pitch
bend, SysEx), so the music resumes with the right instruments. It doesn't
resume a note held across that moment: it waits for the next one.

### Layers, fades and speed

- `set_channel_muted(channel, true)` keeps a channel playing unheard, so it
  comes back in time when unmuted. Write music in layers (drums on 9, bass
  on 1, and so on) and bring them in and out as the game changes.
- `fade_to(volume, seconds)` fades smoothly. Fading to 0 keeps the song
  playing, silent.
- `set_speed(1.5)` plays half as fast again, from 0.25× to 4×, without
  changing the pitch.

### Playing live

`note_on`, `note_off`, `control_change`, `program_change`,
`set_instrument(channel, bank, program)` and `pitch_bend`, or raw MIDI bytes
with `send` (running status and SysEx included, so a MIDI keyboard's input
can go straight in). Over a song, use a channel the song doesn't use
(15 is usually free). Channels count from 0, and 9 is the drums.

```rust
let s = music.synth_mut();
s.set_instrument(15, 0, 11);   // a vibraphone
s.note_on(15, 84, 110);        // a sting over the music
```

### Threads and audio callbacks

A player has no locks of its own. Call it from one thread at a time. Most
engines feed audio from the game loop (LÖVE's `QueueableSource`, raylib's
`UpdateAudioStream`, Godot's `AudioStreamGenerator`, MonoGame's
`BufferNeeded`, Qt's `QAudioSink`), so there's nothing to do. If your audio
callback runs on its own thread (miniaudio, SDL's callback, cpal, Unity's
`OnAudioFilterRead`), guard the player with a mutex. The calls are short,
and rendering never blocks. The C# binding takes a lock for you. In the
browser, `createMusicNode` runs the player in the AudioWorklet and passes
your calls to it as messages.

`Song::from_midi` reads a file without a player, so a game can read its
songs while loading and hand them over with `load_song` later.

### Levels

The synthesizer leaves headroom: the demo songs peak between -11 and
-16 dBFS, and all 64 voices at once peak around -2.5 dBFS. Use `set_volume` (up to 4) if it's
quieter than your sound effects. `render_i16` clips at full scale.

### Determinism

The same MIDI at the same sample rate always makes the same samples on the
same platform. Its noise is seeded, not random. That holds through a sound
bank too.

## SoundFonts and DLS

Neumetik's own instruments need nothing. For sampled ones, read a bank and
hand it to the player:

```rust
use std::sync::Arc;
use neumetik_midi::{Player, SoundBank};

let bank = Arc::new(SoundBank::from_bytes(&std::fs::read("GeneralUser GS.sf2")?)?);
let mut music = Player::new(48_000);
music.set_sound_bank(Some(bank.clone()));   // or player.synth_mut().set_sound_bank(...)
music.load(&std::fs::read("overworld.mid")?)?;
music.play();
```

```c
int32_t err;
nm_soundbank *bank = nm_soundbank_load(bank_bytes, bank_size, &err);   /* NULL, and err, if it can't */
nm_set_soundbank(music, bank);      /* any number of players can share one */
nm_soundbank_free(bank);            /* the players keep their share */
/* or, for one player: nm_load_soundbank(music, bank_bytes, bank_size) */
```

```js
await music.loadSoundBank(await (await fetch('bank.sf2')).arrayBuffer());
```

LÖVE has `music:loadSoundBank(bytes)`, C# `LoadSoundBank(bytes)`, Godot
`load_sound_bank_file(path)`, Qt `loadSoundBank(bytes)`. Each has a way
back to Neumetik's own instruments (`set_sound_bank(None)`,
`nm_set_soundbank(p, NULL)`, `clearSoundBank()`).

**What it reads:**

- **SoundFont 2** (`.sf2`): presets, instruments and samples, with every
  generator that shapes a sound: key and velocity ranges, both envelopes,
  both LFOs, the resonant low-pass filter, tuning (coarse, fine, scale,
  root key), attenuation, pan, the reverb and chorus sends, loops
  (continuous, or until the key's let go), exclusive classes (an open
  hi-hat cut by a closed one), and fixed keys and velocities. Drum kits are
  bank 128.
- **DLS levels 1 and 2** (`.dls`): instruments, regions and waves (PCM of
  8, 16, 24 or 32 bits, or 32-bit float), and the articulation connections
  a General MIDI bank uses: the volume and modulation envelopes, the LFO
  and DLS 2's vibrato LFO, the filter, tuning, gain, pan, the sends, and
  velocity and key moving them. A region's articulation goes over its
  instrument's. Drum kits are instruments with the drum flag.

**What a channel plays:** the bank's preset for its bank and program, or
else the same program in the bank's bank 0 (as a Sound Canvas falls back
from a variation it hasn't). A drum channel plays the bank's kit, or else
its kit 0. Neumetik's classic banks (80 to 82) stay Neumetik's unless the
bank has those banks itself, and anything the bank hasn't at all Neumetik
plays, so nothing goes silent. `instrument_name` says the bank's name for
it.

**How it plays them:** as the SoundFont standard describes, and as
FluidSynth (which most SoundFonts are made on) plays them: the volume
envelope falls in decibels, the initial attenuation counts at 40% as
E-mu's hardware did, the filter's resonance is made up for in the level,
and the mod wheel adds vibrato. The standard's default modulators are
built in (velocity to level, the mod wheel to vibrato). Of a bank's own
modulators, the simple ones Neumetik writes are read (velocity and key
moving the cutoff, velocity the attack, the mod wheel's vibrato through
either LFO); the rest aren't. Samples are read between their points by
straight lines.

**Memory and time:** a bank's samples are kept as 16 bits, so a SoundFont
takes about its file's size (GeneralUser GS, 30 MB, takes 35 MB) and an
8-bit DLS twice its file's. Reading one is quick (14 ms for GeneralUser
GS) but allocates: do it while the game loads, not in the audio callback.
Rendering through it allocates nothing. Changing a player's bank stops the
notes sounding from the old one.

**Levels:** banks differ. GeneralUser GS plays about as loud as Neumetik's
own instruments; others are quieter or louder. `set_volume` evens them out.

Not read: compressed SoundFonts (SF3, Ogg Vorbis samples:
`Error::CompressedSoundFont`; use the SF2 they were made from), samples in
a sound card's ROM, ADPCM DLS waves (those regions are silent), and a
bank's own modulators (but the simple ones above) and controller
connections.

## Making SoundFonts

`neumetik_midi::soundfont` makes SoundFonts, still with no dependencies. A
`SoundFont` is a SoundFont 2 as its parts (samples, instruments, presets,
and the zones and generators between them); change any of them and
`to_bytes` writes a `.sf2` any SoundFont player reads.

**Neumetik's own instruments, packed**, so a game, a DAW or a tracker that
takes SoundFonts sounds like Neumetik:

```rust
use neumetik_midi::soundfont::{Pack, PackSet, SoundFont};

let sf = SoundFont::neumetik(&Pack::default());                // GM and the drum kits
std::fs::write("Neumetik.sf2", sf.to_bytes()?)?;
let all = SoundFont::neumetik(&Pack { set: PackSet::ALL, ..Pack::default() });  // GS and the classic banks too
```

Each patch is sampled at a root every few keys, and at two velocities where
velocity changes its sound and not only its level (FM, tines, plucked
strings, a filter it moves). What a SoundFont can do itself is left to it:
the amplitude envelope goes in the volume envelope, a low-pass filter in
the SoundFont's filter (its envelope in the modulation envelope, its key
tracking in each root's cutoff, its velocity in a modulator), vibrato in
the vibrato LFO. What changes on its own (an FM index falling, a tine, a
pitch sweep, a high-pass filter's envelope, tremolo, pulse-width
modulation) is sampled until it has settled, then loops round whole cycles,
crossfaded. A plucked string's decay is measured and taken out of its
sample to go in the volume envelope. Each instrument is as loud as 16 bits
hold, its attenuation set so it plays as loud as Neumetik's own.

| `Pack` | Rate | Roots | Layers | Before a loop | General MIDI and drums | Everything |
| --- | --- | --- | --- | --- | --- | --- |
| `Pack::SMALL` | 22.05 kHz | every 12 keys | 1 | 0.5 s | 28 MB | |
| `Pack::default()` | 32 kHz | every 6 keys | 2 | 1 s | 110 MB | 276 MB |

Packing General MIDI takes about 2 seconds. Through Neumetik MIDI the
packed bank plays within a decibel or two of Neumetik's own for most
instruments, in tune; through rustysynth, within 2 dB for nine in ten.
What differs: a 24 dB filter is sampled (a SoundFont's is 12 dB), so a
patch's 24 dB sweep is held once it loops; a plucked string's noise burst
is one typical one where Neumetik's own varies note to note; the lowest
basses are a few dB softer (they're louder than a SoundFont sample can play
through Neumetik); and what a sample holds once it loops doesn't change any
more.

**A DLS bank as a SoundFont:** `SoundFont::from_bytes` opens a `.dls` as
one, its articulation worked back into generators (and modulators, for
velocity and key moving the cutoff). Apple's GS bank plays within a
decibel of the DLS itself through Neumetik.

**A SoundFont opened as it is:** `SoundFont::from_bytes` keeps every
generator and modulator of an `.sf2`, and writing it again plays exactly
the same (GeneralUser GS, the SC-55 SoundFonts and Gervill's bank, sample
for sample). Names past 19 characters lose the rest, as the standard has
it.

**Only what a song plays:** `trim_to_midi` keeps the presets a MIDI file
plays (as Neumetik picks them: bank and program, else bank 0; a drum
part's kit, else kit 0), and in them only the zones its notes reach, then
drops the instruments and samples nothing plays. `trim` keeps the presets
you choose. A game's music can ship with a few hundred kilobytes of a
bank, not all of it.

```rust
let mut sf = SoundFont::from_bytes(&std::fs::read("GeneralUser GS.sf2")?)?;
sf.trim_to_midi(&std::fs::read("overworld.mid")?)?;
std::fs::write("overworld.sf2", sf.to_bytes()?)?;
```

**Your own:** samples from data or WAV files (`Sample::from_wav`, PCM or
float, mono or stereo, its root key and loop from a sampler's `smpl`
chunk), instruments of zones that play them, presets of zones that play
those:

```rust
use neumetik_midi::soundfont::{generator, Instrument, Preset, Sample, SoundFont, Zone};

let mut sf = SoundFont::new("My Bank");
let s = sf.add_sample(Sample::new("Saw C4", saw, 44_100, 60).looped(1_000, 1_441));
let mut inst = Instrument::new("Saw");
inst.zones.push(Zone::new(s).keys(0, 127).with(generator::SAMPLE_MODES, 1).with(generator::VOL_ENV_RELEASE, -1200));
let i = sf.add_instrument(inst);
sf.add_preset(Preset::new("Saw", 0, 81, i));
std::fs::write("My Bank.sf2", sf.to_bytes()?)?;
```

`to_bytes` refuses (`Error::Unwritable`) what doesn't fit together: a zone
playing a sample or instrument the SoundFont hasn't, a stereo pair that
doesn't point at each other, a loop outside its sample, or more than the
format holds.

**From the command line:** `cargo run --release --example soundfont --`
then `pack out.sf2 [--all] [--small] [--rate HZ] [--step KEYS] [--layers N]
[--seconds S]`, `convert in.dls out.sf2`, `trim in.sf2 song.mid out.sf2`,
`make out.sf2 a.wav b.wav ...` (each WAV a preset), or `info in.sf2`.

Written from the SoundFont 2.04 specification. It writes 16-bit SoundFont
2.04, not SF3 (Ogg Vorbis would need an encoder) or 24-bit samples; there's
no C API for it yet.

## Music as text

`Score` is a MIDI file as an editor has it: parts (a channel of a track
each), bars and notes, with everything else kept at its tick so
`Score::to_midi` writes it back with nothing lost. `tmf` writes a score as
TMF, a short run of plain characters that survives being typed into a game
and printed back wrapped (`TMF1`, groups of base64url, ` =`).

`text` reads the music people write as text into a score:

```rust
use neumetik_midi::text;

let abc = b"X:1\nT:Scale\nL:1/8\nK:D\nDEFG ABcd|]\n";
let (score, title) = text::read(abc, Some("abc"))?;   // the format by extension, else by what's in it
let midi = score.to_midi();                            // a format 1 MIDI file, to play or save
let xml = text::write(&score, &title, "musicxml");     // and back out as text (any score, a MIDI file's too)
```

| Format | Files | Reads | Writes |
| --- | --- | --- | --- |
| MusicXML 1.0 to 4.0 | `.musicxml` `.xml` `.mxl` | part-wise and time-wise; voices, chords, ties, tuplets, transposing instruments; repeats and endings played out; tempo, dynamics, chord symbols, lyrics | `musicxml::write` |
| ABC 2.1 | `.abc` | every tune in a file (`abc::tunes`); keys and modes, lengths, broken rhythm, tuplets, chords, repeats and endings, parts in `P:` order, voices, lyrics, `%%MIDI program` | `abc::write` |
| MML | `.mml` `.ms2mml` `.bas` | Mabinogi and ArcheAge `MML@`, MapleStory 2, GW-BASIC/QBasic, MSX and MSX-MUSIC, N88-BASIC, C128 and ZX Spectrum 128 `PLAY` (a BASIC program's string variables and `FOR` loops too); Sakura, by its own compiler (sakuramml-rust, MIT, in `text::sakura`) | `mml::write` |
| FlMML | `.flmml` `.mml` | a port of FlMML's own compiler (BSD-3): macros, repeats, group notes, chords, ties and slurs, gates; its tones as General MIDI stand-ins, its noise as drums | |
| SiON | `.sion` `.mml` | a port of SiON's own compiler (MIT): macros (`#A=`, `A(n)`), `![ !\|  !]` and `[ \| ]` repeats, ties and slurs, gates (`q`, `@q`), `v` `x` `p`, `t` with `#TMODE`, `#SIGN`, `#REV`, `#QUANT`; its channels as General MIDI stand-ins, its noise and sampler as drums | |
| ChordPro 6 | `.cho` `.chordpro` `.crd` `.pro` | chords voiced and played (a piano each beat, a bass each half bar), timed by `{tempo}`, `{time}`, grids and bar lines; lyrics | `chordpro::write` (the chords, or recognised from the notes) |
| Humdrum `**kern` | `.krn` | spines splitting and joining, chords, ties, `*MM`, `*M`, `*I`, section expansion lists | `kern::write` |
| Alda | `.alda` | parts and groups, voices, markers, variables, repeats with endings, cram, attributes and dynamics | `alda::write` |
| RTTTL | `.rtttl` `.rtx` | | `rtttl::write` |
| Strudel and Tidal mini-notation | `.mini` `.strudel` `.tidal` | `~ [ ] < > { } % * / ! @ _ , \| ? .`, Euclidean rhythms, `note()` and `s()` in Strudel or Tidal code, drum machine names as GM drums | `mini::write` |

MusicXML, ABC and `**kern` are written from one notation of the score
(`text::notation`): bars from its meter, voices, notes tied over bars and
beats, tuplets found from the times, keys spelled by the key signature.
Music read from text is written exactly; a played MIDI file's times are
rounded to the nearer 32nd or triplet 16th.

A text that can't be read is `Error::BadText`, with its line, column and
why ("line 3, column 7: a length is 1, 2, 4, 8, 16 or 32"). Nothing is
rounded: notes are placed in fractions of a whole note and made ticks at a
PPQ they all divide into. `text::program` finds a General MIDI program by
an instrument's name in any of these formats (`Violoncello`, `*Iflt`,
`midi-acoustic-guitar-nylon`).

The readers were written from each format's published description. They
were checked against black-box references: music21 for MusicXML (40 of 40
of its Bach `.mxl` files the same note for note) and `**kern` (59 of 60
files; the other is music21 not playing an expansion list), Alda's own parser (22 of
22 to the millisecond), and Strudel's (27 of 27 patterns). The differences
left with music21 on ABC and other MusicXML are where it doesn't follow the
standards: it ignores `<transpose>`, plays cue notes, repeats a second `:|`
from the start of the tune, and doesn't join tied notes. `text_check` and
`text_dump` (in `examples/`) are the tools.

## The C API

`capi/include/neumetik_midi.h` documents every function. In brief:

| | |
| --- | --- |
| Make | `nm_create(rate)`, `nm_destroy`, `nm_sample_rate` |
| Song | `nm_load(p, bytes, size)` → `NM_OK` or `NM_ERR_*`, `nm_error_string`, `nm_unload` |
| Play | `nm_play`, `nm_pause`, `nm_stop`, `nm_state`, `nm_is_finished` |
| Where | `nm_seek`, `nm_position`, `nm_duration` |
| Loop | `nm_set_looping`, `nm_is_looping`, `nm_set_loop_points(start, end)` (negative: the file's own), `nm_loop_start`, `nm_loop_end` |
| Shape | `nm_set_speed`, `nm_set_volume`, `nm_volume`, `nm_fade_to`, `nm_set_channel_muted` |
| Live | `nm_note_on`, `nm_note_off`, `nm_control_change`, `nm_program_change`, `nm_set_instrument`, `nm_pitch_bend`, `nm_send_midi`, `nm_all_notes_off`, `nm_all_sound_off`, `nm_reset` |
| Render | `nm_render_f32` (interleaved), `nm_render_s16`, `nm_render_f32_planar` |
| Banks | `nm_soundbank_load(bytes, size, &err)`, `nm_soundbank_free`, `nm_set_soundbank(p, bank)` (NULL: Neumetik's own), `nm_load_soundbank(p, bytes, size)` |
| Look | `nm_active_voices`, `nm_instrument_name(p, channel, buffer, size)` |

Every function accepts a NULL player and does nothing. Out-of-range values
are clamped or ignored. The WebAssembly module exports the same functions,
plus `nm_alloc` and `nm_free` for passing bytes in. It needs no imports.

## What it plays

- Standard MIDI Files, formats 0, 1 and 2, and RIFF RMID. A damaged track
  plays as far as it reads.
- Through Neumetik's own instruments (what follows), or a SoundFont or DLS
  bank's ([SoundFonts and DLS](#soundfonts-and-dls)).
- **Bank 0, General MIDI:** all 128 instruments.
- **Banks 1 to 63, Roland GS:** the SC-55's variation tones. A variation it
  doesn't have plays the GM tone, as the SC-55 did.
- **Banks 80, 81, 82:** classic synth sounds of the eighties, the nineties,
  and the 2000s on.
- **Drums** on channel 9 (MIDI's 10), or any channel GS makes a drum part.
  Every key 27 to 87 is covered. Kits by program: 0 Standard, 8 Room,
  16 Power, 24 Electronic, 25 Analog Boom, 26 Dance, 27 Rhythm Box,
  28 Analog Tick, 29 Digital Machine, 30 Analog Punch, 32 Jazz, 40 Brush,
  48 Orchestra, 56 SFX, 64 Studio Machine, 72 Hex Pads.
- **XG:** after an XG System On (`F0 43 1n 4C 00 00 7E 00 F7`) Bank Select
  is two bytes. MSB 127 is the drum kits (and the drum channel's default),
  126 the SFX kits (Neumetik's SFX kit), 64 the SFX voices, 0 the voices
  with the LSB picking the variation; any channel goes back to melodic with
  an MSB of 0. Through a SoundFont the bank is the LSB (or 64), falling
  back to bank 0, so an XG-mapped bank works. Neumetik's own tones play
  GM's for XG variations. CC 91, 93 and **94** send to the reverb, the
  chorus and the variation effect (a delay; types 5 to 8 of XG's variation
  list), and XG parameter changes set a part's bank, program, part mode,
  volume, pan, sends, cutoff and resonance, and the master volume.
- **Controllers:**

  | CC | Does |
  | --- | --- |
  | 0 | bank select |
  | 1 | modulation |
  | 6/38 | data entry |
  | 7 | volume |
  | 10 | pan |
  | 11 | expression |
  | 64 | sustain |
  | 71 | resonance |
  | 74 | brightness |
  | 91 | reverb |
  | 93 | chorus |
  | 100/101 | RPN: bend range, fine and coarse tuning |
  | 120 | all sound off |
  | 121 | reset controllers |
  | 123–127 | all notes off |

  Pitch bend is supported.
- **SysEx:** GM System On, GS reset, GS Use For Rhythm Part, master volume.
- 64 voices. When they run out, the quietest released voice is stolen
  first, then the quietest of all.

## Size and speed

Measured on Apple Silicon (aarch64, macOS) at 48 kHz in 512-frame buffers
by `cargo run --release --example bench`. `scripts/build.sh` measures
again, and the demo page shows the latest numbers along with a benchmark
run in your browser.

| | |
| --- | --- |
| WebAssembly | 224 KB, 80 KB gzipped |
| Added to a C program, linked statically (stripped) | about 441 KB |
| Shared library | 459 KB |
| Memory, one player | 1.17 MB (64 voices, chorus, reverb), and a sound bank's, shared |
| Typical game music | 0.25–0.7% of one core (140–400× real time) |
| Through GeneralUser GS (SoundFont) | 0.5–1.3% of one core (75–220× real time) |
| Through Apple's QuickTime bank (DLS) | 0.2–0.6% of one core (180–500× real time) |
| All 64 voices busy | 5.3% of one core (19× real time); through a bank, 2–2.4% |

The native figure includes the parts of Rust's standard library that get
linked in.

## Building

You need Rust (1.75 or newer). `rustup target add wasm32-unknown-unknown`
for the WebAssembly build.

```sh
cargo test                                         # the crate
cargo run --release --example make_demos -- songs  # the demo songs, as MIDI files
cargo run --release --example to_wav -- song.mid song.wav --loops 2
cargo run --release --example to_wav -- song.mid song.wav --bank bank.sf2   # or a .dls
cargo run --release --example live                 # live.wav: notes, bends, drums, no file
cargo run --release --example bench -- songs       # how fast, how loud (--bank bank.sf2 for a bank)
NEUMETIK_TEST_BANKS=a.sf2:b.dls cargo test real_banks   # the tests with real banks, too

scripts/build.sh                                   # everything: dist/ and the demo page
python3 -m http.server -d demo 8000                # then open http://localhost:8000
```

`dist/` gets the header, the static and shared libraries for the machine
you built on, the WebAssembly module and JavaScript, the LÖVE and C#
bindings, and the demo songs. For other platforms, build the C API with
their targets:

```sh
cargo build --release --manifest-path capi/Cargo.toml --target x86_64-pc-windows-msvc   # neumetik_midi.dll / .lib
cargo build --release --manifest-path capi/Cargo.toml --target x86_64-unknown-linux-gnu
cargo build --release --manifest-path capi/Cargo.toml --target aarch64-apple-ios        # a static library for iOS
cargo build --release --manifest-path capi/Cargo.toml --target aarch64-linux-android    # with the NDK's linker set up
```

On Windows, linking the static library also needs `ws2_32 userenv ntdll bcrypt advapi32`.
On Linux, it needs `pthread dl m`.

## Limits

- Sound banks: SoundFont 2 and DLS 1 and 2, without SF3's compression or
  most of a bank's own modulators (see [SoundFonts and DLS](#soundfonts-and-dls)).
  It writes 16-bit SoundFont 2 (see [Making SoundFonts](#making-soundfonts)).
- General MIDI, GS and the XG bank scheme, not GM2 or XG's own voices
  (their variations play GM's, or a SoundFont's). XG's variation effect
  is only the delays; its effect types, drum setup and master tune are
  not read.
  No NRPN sound editing, no MIDI 2.0.
- One song per player. For two at once (a crossfade, say), use two players
  and mix them.
- The sound is its own: close to a Sound Canvas in spirit, not a copy of
  one.

## License

MIT: see `LICENSE`. The demo songs were written for this project and are
free to use under the same license. "Für Elise" is Beethoven's, in the
public domain.
