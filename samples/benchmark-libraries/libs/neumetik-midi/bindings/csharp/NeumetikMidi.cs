// Neumetik MIDI for C#: General MIDI and GS music, no SoundFont. For
// MonoGame, FNA, Unity, Godot (C#), Stride or plain .NET.
// Copyright 2026 ansiapps. MIT licensed.
//
// Ship the native library with your game: neumetik_midi.dll (Windows),
// libneumetik_midi.dylib (macOS), libneumetik_midi.so (Linux). .NET finds
// it beside the executable; Unity, in Assets/Plugins.
//
//   using var music = new NeumetikMidi.Player(48000);
//   music.Load(File.ReadAllBytes("song.mid"));
//   music.Looping = true;
//   music.Play();
//   music.Render(buffer);   // float[] or short[], interleaved stereo
//
// To play through a SoundFont or DLS bank:
//   music.LoadSoundBank(File.ReadAllBytes("bank.sf2"));
//
// Every call takes a lock, so the game thread may control it while the
// audio thread renders (as in Unity's OnAudioFilterRead). Rendering
// itself never allocates.

using System;
using System.Runtime.InteropServices;

namespace NeumetikMidi
{
    public enum PlayState { Stopped = 0, Playing = 1, Paused = 2 }

    public sealed class MidiException : Exception
    {
        public int Code { get; }
        public MidiException(int code, string message) : base(message) { Code = code; }
    }

    internal static class Native
    {
        const string Lib = "neumetik_midi";

        [DllImport(Lib)] public static extern uint nm_version();
        [DllImport(Lib)] public static extern IntPtr nm_create(uint sampleRate);
        [DllImport(Lib)] public static extern void nm_destroy(IntPtr p);
        [DllImport(Lib)] public static extern uint nm_sample_rate(IntPtr p);
        [DllImport(Lib)] public static extern int nm_load(IntPtr p, byte[] data, UIntPtr size);
        [DllImport(Lib)] public static extern IntPtr nm_error_string(int code);
        [DllImport(Lib)] public static extern void nm_unload(IntPtr p);
        [DllImport(Lib)] public static extern IntPtr nm_soundbank_load(byte[] data, UIntPtr size, out int error);
        [DllImport(Lib)] public static extern void nm_soundbank_free(IntPtr bank);
        [DllImport(Lib)] public static extern void nm_set_soundbank(IntPtr p, IntPtr bank);
        [DllImport(Lib)] public static extern void nm_play(IntPtr p);
        [DllImport(Lib)] public static extern void nm_pause(IntPtr p);
        [DllImport(Lib)] public static extern void nm_stop(IntPtr p);
        [DllImport(Lib)] public static extern int nm_state(IntPtr p);
        [DllImport(Lib)] public static extern int nm_is_finished(IntPtr p);
        [DllImport(Lib)] public static extern void nm_seek(IntPtr p, double seconds);
        [DllImport(Lib)] public static extern double nm_position(IntPtr p);
        [DllImport(Lib)] public static extern double nm_duration(IntPtr p);
        [DllImport(Lib)] public static extern void nm_set_looping(IntPtr p, int looping);
        [DllImport(Lib)] public static extern int nm_is_looping(IntPtr p);
        [DllImport(Lib)] public static extern void nm_set_loop_points(IntPtr p, double start, double end);
        [DllImport(Lib)] public static extern double nm_loop_start(IntPtr p);
        [DllImport(Lib)] public static extern double nm_loop_end(IntPtr p);
        [DllImport(Lib)] public static extern void nm_set_speed(IntPtr p, double speed);
        [DllImport(Lib)] public static extern void nm_set_volume(IntPtr p, float volume);
        [DllImport(Lib)] public static extern float nm_volume(IntPtr p);
        [DllImport(Lib)] public static extern void nm_fade_to(IntPtr p, float volume, float seconds);
        [DllImport(Lib)] public static extern void nm_set_channel_muted(IntPtr p, int channel, int muted);
        [DllImport(Lib)] public static extern void nm_note_on(IntPtr p, int channel, int key, int velocity);
        [DllImport(Lib)] public static extern void nm_note_off(IntPtr p, int channel, int key);
        [DllImport(Lib)] public static extern void nm_control_change(IntPtr p, int channel, int controller, int value);
        [DllImport(Lib)] public static extern void nm_program_change(IntPtr p, int channel, int program);
        [DllImport(Lib)] public static extern void nm_set_instrument(IntPtr p, int channel, int bank, int program);
        [DllImport(Lib)] public static extern void nm_pitch_bend(IntPtr p, int channel, int value);
        [DllImport(Lib)] public static extern void nm_send_midi(IntPtr p, byte[] data, UIntPtr size);
        [DllImport(Lib)] public static extern void nm_all_notes_off(IntPtr p);
        [DllImport(Lib)] public static extern void nm_all_sound_off(IntPtr p);
        [DllImport(Lib)] public static extern void nm_reset(IntPtr p);
        [DllImport(Lib)] public static extern unsafe void nm_render_f32(IntPtr p, float* output, UIntPtr frames);
        [DllImport(Lib)] public static extern unsafe void nm_render_s16(IntPtr p, short* output, UIntPtr frames);
        [DllImport(Lib)] public static extern int nm_active_voices(IntPtr p);
        [DllImport(Lib)] public static extern int nm_instrument_name(IntPtr p, int channel, byte[] buffer, UIntPtr size);
    }

    /// <summary>A MIDI player: load a song, play it, render its samples.</summary>
    public sealed class Player : IDisposable
    {
        IntPtr p;
        readonly object gate = new object();
        readonly byte[] name = new byte[64];

        public static string Version { get { var v = Native.nm_version(); return $"{v / 100}.{v % 100:00}"; } }

        /// <summary>A player at sampleRate frames a second (8000 to 192000).</summary>
        public Player(int sampleRate = 48000)
        {
            p = Native.nm_create((uint)Math.Max(sampleRate, 0));
        }

        public int SampleRate { get { lock (gate) return (int)Native.nm_sample_rate(p); } }

        /// <summary>Loads a MIDI file's bytes, stopped at its start. Throws a
        /// MidiException saying why if it can't.</summary>
        public void Load(byte[] midi)
        {
            int code;
            lock (gate) code = Native.nm_load(p, midi, (UIntPtr)midi.Length);
            if (code != 0) throw new MidiException(code, "Couldn't play that MIDI file: " + Marshal.PtrToStringAnsi(Native.nm_error_string(code)) + ".");
        }

        /// <summary>Plays through a SoundFont (.sf2) or DLS (.dls) bank's
        /// bytes: its instruments, and Neumetik's for what it hasn't. Throws a
        /// MidiException saying why if it can't. The bank is read before the
        /// lock is taken, so the audio thread doesn't wait for it.</summary>
        public void LoadSoundBank(byte[] bank)
        {
            IntPtr b = Native.nm_soundbank_load(bank, (UIntPtr)bank.Length, out int code);
            if (b == IntPtr.Zero) throw new MidiException(code, "Couldn't use that sound bank: " + Marshal.PtrToStringAnsi(Native.nm_error_string(code)) + ".");
            lock (gate) Native.nm_set_soundbank(p, b);
            Native.nm_soundbank_free(b);
        }

        /// <summary>Back to Neumetik's own instruments.</summary>
        public void ClearSoundBank() { lock (gate) Native.nm_set_soundbank(p, IntPtr.Zero); }

        public void Unload() { lock (gate) Native.nm_unload(p); }
        public void Play() { lock (gate) Native.nm_play(p); }
        public void Pause() { lock (gate) Native.nm_pause(p); }
        public void Stop() { lock (gate) Native.nm_stop(p); }
        public PlayState State { get { lock (gate) return (PlayState)Native.nm_state(p); } }
        public bool IsFinished { get { lock (gate) return Native.nm_is_finished(p) != 0; } }

        public void Seek(double seconds) { lock (gate) Native.nm_seek(p, seconds); }
        public double Position { get { lock (gate) return Native.nm_position(p); } }
        public double Duration { get { lock (gate) return Native.nm_duration(p); } }

        public bool Looping
        {
            get { lock (gate) return Native.nm_is_looping(p) != 0; }
            set { lock (gate) Native.nm_set_looping(p, value ? 1 : 0); }
        }

        /// <summary>Loop points in seconds; null goes back to the file's own.</summary>
        public void SetLoopPoints(double? start, double? end) { lock (gate) Native.nm_set_loop_points(p, start ?? -1, end ?? -1); }
        public (double Start, double End) LoopPoints { get { lock (gate) return (Native.nm_loop_start(p), Native.nm_loop_end(p)); } }

        /// <summary>Tempo: 1 as written, 0.25 to 4; the pitch stays.</summary>
        public double Speed { set { lock (gate) Native.nm_set_speed(p, value); } }

        public float Volume
        {
            get { lock (gate) return Native.nm_volume(p); }
            set { lock (gate) Native.nm_set_volume(p, value); }
        }

        public void FadeTo(float volume, float seconds) { lock (gate) Native.nm_fade_to(p, volume, seconds); }
        public void SetChannelMuted(int channel, bool muted) { lock (gate) Native.nm_set_channel_muted(p, channel, muted ? 1 : 0); }

        public void NoteOn(int channel, int key, int velocity = 100) { lock (gate) Native.nm_note_on(p, channel, key, velocity); }
        public void NoteOff(int channel, int key) { lock (gate) Native.nm_note_off(p, channel, key); }
        public void ControlChange(int channel, int controller, int value) { lock (gate) Native.nm_control_change(p, channel, controller, value); }
        public void ProgramChange(int channel, int program) { lock (gate) Native.nm_program_change(p, channel, program); }
        public void SetInstrument(int channel, int bank, int program) { lock (gate) Native.nm_set_instrument(p, channel, bank, program); }
        /// <summary>-8192 to 8191, 0 in tune.</summary>
        public void PitchBend(int channel, int value) { lock (gate) Native.nm_pitch_bend(p, channel, value); }
        public void SendMidi(byte[] data) { lock (gate) Native.nm_send_midi(p, data, (UIntPtr)data.Length); }
        public void AllNotesOff() { lock (gate) Native.nm_all_notes_off(p); }
        public void AllSoundOff() { lock (gate) Native.nm_all_sound_off(p); }
        public void Reset() { lock (gate) Native.nm_reset(p); }

        public int ActiveVoices { get { lock (gate) return Native.nm_active_voices(p); } }

        public string InstrumentName(int channel)
        {
            lock (gate)
            {
                int n = Math.Min(Native.nm_instrument_name(p, channel, name, (UIntPtr)name.Length), name.Length - 1);
                return System.Text.Encoding.ASCII.GetString(name, 0, n);
            }
        }

        /// <summary>Renders interleaved stereo floats: buffer.Length / 2 frames.</summary>
        public void Render(float[] buffer) => Render(buffer, 0, buffer.Length);

        public unsafe void Render(float[] buffer, int offset, int count)
        {
            if (offset < 0 || count < 0 || offset + count > buffer.Length) throw new ArgumentOutOfRangeException(nameof(count));
            if (count < 2) return;
            fixed (float* at = &buffer[0]) lock (gate) Native.nm_render_f32(p, at + offset, (UIntPtr)(count / 2));
        }

        /// <summary>Renders interleaved stereo 16-bit samples: buffer.Length / 2 frames.</summary>
        public unsafe void Render(short[] buffer)
        {
            if (buffer.Length < 2) return;
            fixed (short* at = &buffer[0]) lock (gate) Native.nm_render_s16(p, at, (UIntPtr)(buffer.Length / 2));
        }

        /// <summary>Renders 16-bit stereo into bytes, as MonoGame's
        /// DynamicSoundEffectInstance wants them: buffer.Length / 4 frames.</summary>
        public unsafe void Render(byte[] buffer)
        {
            if (buffer.Length < 4) return;
            fixed (byte* at = &buffer[0]) lock (gate) Native.nm_render_s16(p, (short*)at, (UIntPtr)(buffer.Length / 4));
        }

        public void Dispose()
        {
            lock (gate)
            {
                if (p != IntPtr.Zero) Native.nm_destroy(p);
                p = IntPtr.Zero;
            }
            GC.SuppressFinalize(this);
        }

        ~Player() { if (p != IntPtr.Zero) Native.nm_destroy(p); }
    }
}
