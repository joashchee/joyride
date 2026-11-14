# Neumetik MIDI for C#

`NeumetikMidi.cs` is the whole binding: copy it into your project and ship
the native library beside your game (`neumetik_midi.dll`,
`libneumetik_midi.dylib` or `libneumetik_midi.so`; see the main README for
building them). It uses `unsafe` code, so set `<AllowUnsafeBlocks>true</AllowUnsafeBlocks>`
in your `.csproj` (in Unity: Player Settings → Allow 'unsafe' Code).

Every method locks, so the game thread can control the music while the
audio thread renders.

## MonoGame and FNA

`DynamicSoundEffectInstance` asks for buffers through `BufferNeeded`.

```csharp
using Microsoft.Xna.Framework.Audio;
using NeumetikMidi;

public class Music : IDisposable
{
    const int Rate = 44100, Frames = 1024;
    readonly Player player = new Player(Rate);
    readonly DynamicSoundEffectInstance output = new DynamicSoundEffectInstance(Rate, AudioChannels.Stereo);
    readonly byte[] buffer = new byte[Frames * 4];   // 16-bit stereo

    public Music(string path)
    {
        player.Load(File.ReadAllBytes(path));
        player.Looping = true;
        output.BufferNeeded += (_, _) => Submit();
    }

    void Submit()
    {
        player.Render(buffer);
        output.SubmitBuffer(buffer);
    }

    public void Play()
    {
        player.Play();
        for (int i = 0; i < 3; i++) Submit();     // a little ahead
        output.Play();
    }

    public Player Player => player;                // FadeTo, SetChannelMuted, NoteOn...
    public void Dispose() { output.Dispose(); player.Dispose(); }
}
```

## Unity

Put the native libraries in `Assets/Plugins` (one per platform), then add
this to a GameObject with an `AudioSource` (no clip). Unity calls
`OnAudioFilterRead` on its audio thread with interleaved floats.

```csharp
using UnityEngine;
using NeumetikMidi;

[RequireComponent(typeof(AudioSource))]
public class MidiMusic : MonoBehaviour
{
    public TextAsset song;   // rename song.mid to song.bytes so Unity imports it
    Player player;

    void Awake()
    {
        player = new Player(AudioSettings.outputSampleRate);
        player.Load(song.bytes);
        player.Looping = true;
        player.Play();
    }

    void OnAudioFilterRead(float[] data, int channels)
    {
        if (player == null || channels != 2) return;
        player.Render(data);
    }

    void OnDestroy() => player?.Dispose();
}
```

## Godot (C#)

An `AudioStreamPlayer` with an `AudioStreamGenerator` stream:

```csharp
using Godot;
using NeumetikMidi;

public partial class MidiMusic : AudioStreamPlayer
{
    Player player;
    AudioStreamGeneratorPlayback playback;
    float[] buffer = new float[2048];

    public override void _Ready()
    {
        var generator = new AudioStreamGenerator { MixRate = 44100, BufferLength = 0.1f };
        Stream = generator;
        player = new Player(44100);
        player.Load(FileAccess.GetFileAsBytes("res://music/song.mid"));
        player.Looping = true;
        player.Play();
        Play();
        playback = (AudioStreamGeneratorPlayback)GetStreamPlayback();
    }

    public override void _Process(double delta)
    {
        int frames = Mathf.Min(playback.GetFramesAvailable(), buffer.Length / 2);
        if (frames == 0) return;
        player.Render(buffer, 0, frames * 2);
        for (int i = 0; i < frames; i++)
            playback.PushFrame(new Vector2(buffer[2 * i], buffer[2 * i + 1]));
    }
}
```

Godot exports `.mid` files only if you tell it to: in the export preset,
add `*.mid` under Resources → Filters to export non-resource files.
