# Neumetik MIDI for Godot 4

A GDExtension that adds a **NeumetikMusic** node: a MIDI player for
GDScript, with its own `AudioStreamPlayer` fed from `_process`. Using C#?
You don't need this: see `../csharp/README.md`.

> **Status:** the C++ here is written against godot-cpp 4.1+ but hasn't
> been built in CI yet. Treat it as a starting point and report what
> breaks. Web exports aren't covered: on the web, use the JavaScript
> binding.

## Build

```sh
git clone -b 4.3 https://github.com/godotengine/godot-cpp   # beside SConstruct; match your Godot version
cargo build --release --manifest-path ../../capi/Cargo.toml  # libneumetik_midi.a
scons platform=macos target=template_debug                   # or windows / linux; template_release for exports
```

That builds into `project/addons/neumetik_midi/bin/`. Copy
`project/addons/neumetik_midi/` into your game's `addons/`, and restart the
editor: **NeumetikMusic** appears in Add Node.

## Use

```gdscript
@onready var music: NeumetikMusic = $Music

func _ready():
    var problem := music.load_file("res://music/overworld.mid")
    if problem != "":
        push_error(problem)
    music.looping = true
    music.play()

func on_boss_appears():
    music.fade_to(0.0, 1.0)
```

| Method | Does |
| --- | --- |
| `load_file(path)`, `load_bytes(bytes)` | Reads a MIDI file; returns `""` or why not |
| `play()`, `pause()`, `stop()`, `is_playing()`, `is_finished()` | Playback |
| `seek(seconds)`, `get_position()`, `get_duration()` | Where it is |
| `looping`, `set_loop_points(start, end)` | Loops (the file's `loopStart`/`loopEnd` markers by default; -1 for the file's own) |
| `set_speed(speed)`, `volume`, `fade_to(volume, seconds)` | Tempo, volume, fades |
| `set_channel_muted(channel, muted)` | Music in layers |
| `note_on`, `note_off`, `control_change`, `set_instrument`, `pitch_bend`, `all_notes_off` | Playing live, over the music |
| `get_active_voices()`, `get_instrument_name(channel)` | Looking in |
| `bus` | The audio bus it plays on |

Godot exports `.mid` files only if told to: in the export preset, add
`*.mid` under Resources → Filters to export non-resource files.

`project/main.gd` is a small example scene script.
