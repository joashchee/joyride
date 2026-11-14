// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! The synthesizer: 16 MIDI channels, `POLYPHONY` voices, the chorus and
//! the reverb. Driven by MIDI messages, live or from a file, and rendered
//! a buffer at a time.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use crate::bank::SoundBank;
use crate::effects::{Chorus, Reverb, Variation};
use crate::output;
use crate::patch::Patch;
use crate::smf::Message;
use crate::voice::{Controls, Voice, BLOCK};
use crate::{drums, gm, patch, POLYPHONY};

/// The lowest and highest sample rates the synthesizer runs at; others
/// are clamped to them.
pub const MIN_RATE: u32 = 8_000;
pub const MAX_RATE: u32 = 192_000;

/// XG's default variation effect: a delay.
const DEFAULT_VARIATION: u8 = 5;

/// A sound bank's voices against Neumetik's own: the level that puts a
/// General MIDI SoundFont's instruments about as loud as Neumetik's.
const SAMPLED_LEVEL: f32 = 2.0;

#[derive(Clone, Copy)]
pub(crate) struct Channel {
    /// The bank the instrument is looked up in: Bank Select MSB, or in
    /// XG mode what MSB and LSB make of it ([`Mode::bank`]).
    bank: u8,
    msb: u8,
    lsb: u8,
    variation: f32,
    program: u8,
    patch: &'static Patch,
    drums: bool,
    kit: u8,
    volume: f32,
    expression: f32,
    pan: f32,
    modulation: f32,
    bend: f32,
    bend_range: f32,
    tuning: f32,
    /// XG: master tune and transpose, in semitones, for every part.
    fine: f32,
    /// XG: this part's note shift in semitones (melodic parts) and the notes it plays (its limits).
    shift: i8,
    low: u8,
    high: u8,
    sustain: bool,
    reverb: f32,
    chorus: f32,
    brightness: f32,
    resonance: f32,
    /// The RPN being set (CC 101, 100), or none.
    rpn: Option<(u8, u8)>,
}

impl Channel {
    fn new(drums: bool) -> Channel {
        Channel {
            bank: 0,
            msb: 0,
            lsb: 0,
            variation: 0.0,
            program: 0,
            patch: &gm::GM[0],
            drums,
            kit: 0,
            volume: 100.0 / 127.0,
            expression: 1.0,
            pan: 0.0,
            modulation: 0.0,
            bend: 0.0,
            bend_range: 2.0,
            tuning: 0.0,
            fine: 0.0,
            shift: 0,
            low: 0,
            high: 127,
            sustain: false,
            reverb: 40.0 / 127.0,
            chorus: 0.0,
            brightness: 0.0,
            resonance: 0.0,
            rpn: None,
        }
    }

    /// Reset All Controllers (CC 121), as GM says.
    fn reset_controllers(&mut self) {
        self.expression = 1.0;
        self.modulation = 0.0;
        self.bend = 0.0;
        self.sustain = false;
        self.rpn = None;
    }

    fn controls(&self) -> Controls {
        Controls { bend: self.bend * self.bend_range + self.tuning + self.fine, modulation: self.modulation, brightness: self.brightness, resonance: self.resonance }
    }
}

/// What the channels are set to: everything a MIDI file changes but its
/// notes. Kept apart from the voices so a player can work out where a
/// file stands at any moment (to seek, or to loop) without sounding it.
#[derive(Clone, Copy)]
pub(crate) struct State {
    channels: [Channel; 16],
    mode: Mode,
    master: f32,
    /// XG's master tune and transpose, in semitones.
    xg_tune: f32,
    xg_transpose: f32,
    /// XG's variation effect: its type (the MSB of the XG type number).
    variation_type: u8,
}

/// Which family of instruments the file talks to, set by its reset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    /// General MIDI (or nothing said yet).
    Gm,
    /// Roland GS: bank 127 is a GS bank, not XG's drums.
    Gs,
    /// Yamaha XG: Bank Select is MSB and LSB, 127 and 126 are drum kits,
    /// 64 the sound effects, and CC 94 sends to the variation effect.
    Xg,
}

impl Mode {
    /// The bank a Bank Select asks for, as a sound bank (a SoundFont
    /// keeps one bank number) knows it: in XG mode an MSB of 0 picks the
    /// variation by the LSB, and MSB 64 (SFX) stays 64.
    fn bank(self, msb: u8, lsb: u8) -> u8 {
        match self {
            Mode::Xg if msb == 0 => lsb,
            _ => msb,
        }
    }

    /// Neumetik's own banks: XG's variations aren't Neumetik's, so they
    /// play the capital tone, but the classic banks stay.
    fn builtin_bank(self, msb: u8) -> u8 {
        match self {
            Mode::Xg if !(80..=82).contains(&msb) => 0,
            _ => msb,
        }
    }
}

/// What a message asks of the voices, once the channels have taken it.
enum Effect {
    None,
    NoteOn(usize, u8, u8),
    NoteOff(usize, u8),
    /// The pedal let up: release what it held.
    PedalUp(usize),
    /// All Sound Off (CC 120).
    Cut(usize),
    /// All Notes Off and the modes that imply it (CC 123 to 127).
    Release(usize),
    /// A GM or GS reset: let everything go.
    ReleaseAll,
}

impl State {
    pub(crate) fn new() -> State {
        State { channels: std::array::from_fn(|i| Channel::new(i == 9)), mode: Mode::Gm, master: 1.0, xg_tune: 0.0, xg_transpose: 0.0, variation_type: DEFAULT_VARIATION }
    }

    fn apply_fine(&mut self) {
        let fine = self.xg_tune + self.xg_transpose;
        for c in &mut self.channels {
            c.fine = fine;
        }
    }

    fn reset(&mut self, mode: Mode) {
        *self = State::new();
        self.mode = mode;
        if mode == Mode::Xg {
            self.channels[9].msb = 127;
            self.channels[9].bank = 127;
        }
    }

    /// Takes a message into the channels; says what it asks of the voices.
    fn apply(&mut self, message: &Message) -> Effect {
        match message {
            Message::Channel(status, a, b) => self.channel(*status, *a, *b),
            Message::SysEx(data) => self.sysex(data),
        }
    }

    /// A message for where a file stands, never its notes: for seeking.
    pub(crate) fn chase(&mut self, message: &Message) {
        if let Message::Channel(status, _, _) = message {
            if matches!(status & 0xf0, 0x80 | 0x90) {
                return;
            }
        }
        let _ = self.apply(message);
    }

    /// What a channel plays: whether it's a drum part, its bank and its
    /// program.
    pub(crate) fn instrument(&self, ch: usize) -> (bool, u8, u8) {
        let c = &self.channels[ch & 15];
        (c.drums, c.bank, c.program)
    }

    /// A key after XG's note shift (melodic parts only).
    fn shifted(&self, ch: usize, key: u8) -> u8 {
        let c = &self.channels[ch];
        if c.drums || c.shift == 0 {
            key
        } else {
            (i32::from(key) + i32::from(c.shift)).clamp(0, 127) as u8
        }
    }

    fn channel(&mut self, status: u8, a: u8, b: u8) -> Effect {
        let ch = usize::from(status & 0x0f);
        let (a, b) = (a & 0x7f, b & 0x7f);
        match status & 0xf0 {
            0x80 => Effect::NoteOff(ch, self.shifted(ch, a)),
            0x90 if b == 0 => Effect::NoteOff(ch, self.shifted(ch, a)),
            0x90 if (self.channels[ch].low..=self.channels[ch].high).contains(&a) => Effect::NoteOn(ch, self.shifted(ch, a), b),
            0x90 => Effect::None,
            0xb0 => self.control(ch, a, b),
            0xc0 => {
                self.program(ch, a);
                Effect::None
            }
            0xe0 => {
                let raw = (i32::from(b) << 7 | i32::from(a)) - 8192;
                self.channels[ch].bend = raw as f32 / 8192.0;
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn sysex(&mut self, d: &[u8]) -> Effect {
        match d {
            // GM System On (and GM2's).
            [0x7e, _, 0x09, 0x01 | 0x03, ..] => {
                self.reset(Mode::Gm);
                Effect::ReleaseAll
            }
            // Master volume.
            [0x7f, _, 0x04, 0x01, _, msb, ..] => {
                self.master = f32::from(*msb) / 127.0;
                Effect::None
            }
            // Roland GS: a data set to the Sound Canvas.
            [0x41, _, 0x42, 0x12, 0x40, 0x00, 0x7f, 0x00, ..] => {
                self.reset(Mode::Gs);
                Effect::ReleaseAll
            }
            // Yamaha XG: a parameter change to the XG model (4C).
            [0x43, dev, 0x4c, a, b, c, data @ ..] if dev & 0xf0 == 0x10 && !data.is_empty() => return self.xg(*a, *b, *c, data),
            // Use For Rhythm Part: parts 1 to 9 at 1 to 9, part 10 at 0,
            // 11 to 16 at A to F.
            [0x41, _, 0x42, 0x12, 0x40, addr, 0x15, map, ..] if addr & 0xf0 == 0x10 => {
                let part = usize::from(addr & 0x0f);
                let ch = match part {
                    0 => 9,
                    1..=9 => part - 1,
                    _ => part,
                };
                self.channels[ch].drums = *map != 0;
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn program(&mut self, ch: usize, program: u8) {
        let mode = self.mode;
        let c = &mut self.channels[ch];
        c.program = program;
        c.bank = mode.bank(c.msb, c.lsb);
        match mode {
            // XG: the Bank Select says whether it's a kit, and says it
            // again whenever it changes.
            Mode::Xg => c.drums = matches!(c.msb, 126 | 127),
            Mode::Gm if c.msb == 127 => c.drums = true,
            _ => {}
        }
        if c.drums {
            // XG's 126 is the sound effect kits: Neumetik's SFX kit.
            c.kit = if mode == Mode::Xg && c.msb == 126 { 56 } else { drums::kit_for(program) };
        } else {
            c.patch = patch(mode.builtin_bank(c.msb), program);
        }
    }

    /// XG Parameter Change (address `a b c`, the XG model's).
    fn xg(&mut self, a: u8, b: u8, c: u8, data: &[u8]) -> Effect {
        let v = data[0];
        match (a, b, c) {
            // XG System On, and All Parameter Reset.
            (0x00, 0x00, 0x7e | 0x7f) => {
                self.reset(Mode::Xg);
                return Effect::ReleaseAll;
            }
            // Master tune: four bytes, a nibble each, 0x400 the middle, in tenths of a cent (-102.4 to +102.3).
            (0x00, 0x00, 0x00) if data.len() >= 4 => {
                let n = data[..4].iter().fold(0i32, |n, &d| n << 4 | i32::from(d & 15));
                self.xg_tune = (n - 0x400) as f32 / 1000.0;
                self.apply_fine();
            }
            // Master volume.
            (0x00, 0x00, 0x04) => self.master = f32::from(v) / 127.0,
            // Transpose: 0x28 to 0x58, 0x40 none, in semitones.
            (0x00, 0x00, 0x06) => {
                self.xg_transpose = (f32::from(v) - 64.0).clamp(-24.0, 24.0);
                self.apply_fine();
            }
            // Variation effect type (MSB).
            (0x02, 0x01, 0x40) => self.variation_type = v,
            // Multi part, one parameter each: only in XG mode.
            (0x08, part, p) if self.mode == Mode::Xg && part < 16 => {
                let ch = usize::from(part);
                let ch_c = &mut self.channels[ch];
                match p {
                    0x01 => ch_c.msb = v,
                    0x02 => ch_c.lsb = v,
                    0x03 => self.program(ch, v),
                    // Part mode: 0 is melodic, the rest drum kits.
                    0x07 => {
                        ch_c.drums = v != 0;
                        ch_c.msb = if v != 0 { 127 } else { 0 };
                        let program = ch_c.program;
                        self.program(ch, program);
                    }
                    // Note shift: 0x28 to 0x58, 0x40 none, in semitones; the note limits.
                    0x08 => ch_c.shift = (i16::from(v) - 64).clamp(-24, 24) as i8,
                    0x0f => ch_c.low = v,
                    0x10 => ch_c.high = v,
                    0x0b => ch_c.volume = f32::from(v) / 127.0,
                    // Pan: 0 is random in XG; take it as the middle.
                    0x0e => ch_c.pan = if v == 0 { 0.0 } else { (f32::from(v) - 64.0) / 63.0 },
                    0x12 => ch_c.chorus = f32::from(v) / 127.0,
                    0x13 => ch_c.reverb = f32::from(v) / 127.0,
                    0x14 => ch_c.variation = f32::from(v) / 127.0,
                    // 15 to 17 are the vibrato's rate, depth and delay (not read).
                    0x18 => ch_c.brightness = (f32::from(v) - 64.0) / 64.0 * 2.0,
                    0x19 => ch_c.resonance = ((f32::from(v) - 64.0) / 64.0 * 0.5).max(0.0),
                    _ => {}
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn control(&mut self, ch: usize, cc: u8, value: u8) -> Effect {
        let v = f32::from(value) / 127.0;
        let mode = self.mode;
        let c = &mut self.channels[ch];
        match cc {
            0 => {
                c.msb = value;
                c.bank = mode.bank(c.msb, c.lsb);
            }
            32 => {
                c.lsb = value;
                c.bank = mode.bank(c.msb, c.lsb);
            }
            1 => c.modulation = v,
            6 => match c.rpn {
                Some((0, 0)) => c.bend_range = f32::from(value),
                Some((0, 2)) => c.tuning = f32::from(value) - 64.0,
                _ => {}
            },
            38 => {
                if c.rpn == Some((0, 1)) {
                    c.tuning = c.tuning.trunc() + (f32::from(value) - 64.0) / 64.0;
                }
            }
            7 => c.volume = v,
            10 => c.pan = (f32::from(value) - 64.0) / 63.0,
            11 => c.expression = v,
            64 => {
                c.sustain = value >= 64;
                if !c.sustain {
                    return Effect::PedalUp(ch);
                }
            }
            71 => c.resonance = ((f32::from(value) - 64.0) / 64.0 * 0.5).max(0.0),
            74 => c.brightness = (f32::from(value) - 64.0) / 64.0 * 2.0,
            91 => c.reverb = v,
            93 => c.chorus = v,
            94 => c.variation = v,
            // An NRPN: what follows isn't for an RPN.
            98 | 99 => c.rpn = None,
            100 => c.rpn = Some((c.rpn.map_or(127, |r| r.0), value)),
            101 => c.rpn = Some((value, c.rpn.map_or(127, |r| r.1))),
            120 => return Effect::Cut(ch),
            121 => c.reset_controllers(),
            123..=127 => return Effect::Release(ch),
            _ => {}
        }
        Effect::None
    }
}

/// The synthesizer. Play it live with [`note_on`](Synth::note_on) and the
/// rest, or feed it a MIDI file's messages; call
/// [`render`](Synth::render) (or [`render_interleaved`](Synth::render_interleaved),
/// [`render_i16`](Synth::render_i16)) for every buffer of sound.
///
/// Channels count from 0: channel 9 (MIDI's channel 10) is the drums.
/// Out-of-range channels, keys and values are ignored or clamped, never a
/// panic. Nothing allocates after [`Synth::new`].
pub struct Synth {
    state: State,
    voices: Vec<Voice>,
    clock: u64,
    rate: u32,
    chorus: Chorus,
    reverb: Reverb,
    variation: Variation,
    muted: [bool; 16],
    /// The player's volume, now and where a fade is taking it, a step a
    /// frame.
    volume: f32,
    fade_to: f32,
    fade_step: f32,
    /// Each channel's peak since [`Synth::take_levels`], dry (before the
    /// effects and the player's volume), for meters.
    levels: [f32; 16],
    /// A pan added to each drum key on the drum channel, and each key's peak
    /// since [`Synth::take_drum_levels`]: a drum machine's pads.
    drum_pan: [f32; 128],
    drum_levels: [f32; 128],
    /// The SoundFont or DLS bank it plays through, if any.
    bank: Option<Arc<SoundBank>>,
}

impl Synth {
    /// A synthesizer at `sample_rate` frames a second (clamped to
    /// [`MIN_RATE`]..=[`MAX_RATE`]). This allocates its voices and effects,
    /// about 1 MB at 48 kHz, once.
    pub fn new(sample_rate: u32) -> Synth {
        Synth::with_polyphony(sample_rate, POLYPHONY)
    }

    /// Like [`Synth::new`] with only `voices` voices (1 to [`POLYPHONY`]).
    /// A player who never holds more than a few notes saves the memory of
    /// the rest, and a pad's long releases can never pile up past the cap:
    /// a note past it takes the quietest voice, a released one first.
    pub fn with_polyphony(sample_rate: u32, voices: usize) -> Synth {
        let rate = sample_rate.clamp(MIN_RATE, MAX_RATE);
        let r = rate as f32;
        Synth {
            state: State::new(),
            voices: (0..voices.clamp(1, POLYPHONY)).map(|i| Voice::new(r, i as u32 + 1)).collect(),
            clock: 0,
            rate,
            chorus: Chorus::new(r),
            reverb: Reverb::new(r),
            variation: Variation::new(r),
            muted: [false; 16],
            volume: 1.0,
            fade_to: 1.0,
            fade_step: 0.0,
            levels: [0.0; 16],
            drum_pan: [0.0; 128],
            drum_levels: [0.0; 128],
            bank: None,
        }
    }

    /// Plays through a SoundFont or DLS bank ([`SoundBank`]), or with
    /// `None`, Neumetik's own instruments again. What the bank hasn't
    /// (an instrument, a kit, a bank of Neumetik's classics) Neumetik
    /// plays itself, so nothing goes silent. Notes sounding from the bank
    /// that was set stop at once.
    pub fn set_sound_bank(&mut self, bank: Option<Arc<SoundBank>>) {
        for v in self.voices.iter_mut().filter(|v| v.is_sampled()) {
            v.active = false;
        }
        self.bank = bank;
    }

    /// The sound bank it plays through, if any.
    pub fn sound_bank(&self) -> Option<&Arc<SoundBank>> {
        self.bank.as_ref()
    }

    /// The sample rate it runs at.
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Starts a note. A velocity of 0 is a note off, as in MIDI.
    pub fn note_on(&mut self, channel: u8, key: u8, velocity: u8) {
        if channel < 16 {
            self.message(&Message::Channel(0x90 | channel, key, velocity));
        }
    }

    /// Ends a note (it rings on while the sustain pedal is down).
    pub fn note_off(&mut self, channel: u8, key: u8) {
        if channel < 16 {
            self.message(&Message::Channel(0x80 | channel, key, 0));
        }
    }

    /// A control change: 0 bank select, 1 modulation, 7 volume, 10 pan,
    /// 11 expression, 64 sustain, 71 resonance, 74 brightness, 91 reverb,
    /// 93 chorus, 100/101 and 6/38 RPNs (bend range, tuning), 120 all sound
    /// off, 121 reset controllers, 123 all notes off.
    pub fn control_change(&mut self, channel: u8, controller: u8, value: u8) {
        if channel < 16 {
            self.message(&Message::Channel(0xb0 | channel, controller, value));
        }
    }

    /// Picks the instrument a channel plays from its bank (CC 0).
    pub fn program_change(&mut self, channel: u8, program: u8) {
        if channel < 16 {
            self.message(&Message::Channel(0xc0 | channel, program, 0));
        }
    }

    /// Bank select and program change in one: bank 0 is General MIDI, 1 to
    /// 63 the GS variations, 80 to 82 the classic synth banks. On the drum
    /// channel the program picks the kit.
    pub fn set_instrument(&mut self, channel: u8, bank: u8, program: u8) {
        self.control_change(channel, 0, bank);
        self.program_change(channel, program);
    }

    /// Bends a channel's pitch: -8192 to 8191, 0 in tune; by default the
    /// whole range is two semitones each way (RPN 0 changes it).
    pub fn pitch_bend(&mut self, channel: u8, value: i16) {
        if channel < 16 {
            let raw = (i32::from(value).clamp(-8192, 8191) + 8192) as u16;
            self.message(&Message::Channel(0xe0 | channel, (raw & 0x7f) as u8, (raw >> 7) as u8));
        }
    }

    /// Raw MIDI bytes: one message or many, running status allowed, system
    /// exclusive from `F0` to `F7`. What it doesn't use it skips.
    pub fn send(&mut self, bytes: &[u8]) {
        let mut i = 0;
        let mut running = 0u8;
        while i < bytes.len() {
            let b = bytes[i];
            match b {
                0xf0 => {
                    let rest = &bytes[i + 1..];
                    let len = rest.iter().position(|&x| x == 0xf7).unwrap_or(rest.len());
                    self.send_sysex(&rest[..len]);
                    running = 0;
                    i += len + 2;
                    continue;
                }
                0xff => {
                    // System Reset.
                    self.reset();
                    running = 0;
                    i += 1;
                    continue;
                }
                0xf1..=0xfe => {
                    // System common and real-time messages: not for a synth.
                    i += match b {
                        0xf1 | 0xf3 => 2,
                        0xf2 => 3,
                        _ => 1,
                    };
                    if b < 0xf8 {
                        running = 0;
                    }
                    continue;
                }
                _ => {}
            }
            let status = if b >= 0x80 {
                i += 1;
                running = b;
                b
            } else if running != 0 {
                running
            } else {
                i += 1;
                continue;
            };
            let wants = if matches!(status & 0xf0, 0xc0 | 0xd0) { 1 } else { 2 };
            let mut d = [0u8; 2];
            for slot in d.iter_mut().take(wants) {
                match bytes.get(i) {
                    Some(&x) if x < 0x80 => {
                        *slot = x;
                        i += 1;
                    }
                    _ => return,
                }
            }
            self.message(&Message::Channel(status, d[0], d[1]));
        }
    }

    /// Lets every note go, as if every key were lifted and the pedal up.
    pub fn all_notes_off(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.release();
        }
    }

    /// Silences everything at once, reverb and chorus left to fade.
    pub fn all_sound_off(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.choke_now();
        }
    }

    /// Back to how it started: every channel's instrument and controllers
    /// as General MIDI resets them, and silence. Mutes and the volume stay.
    pub fn reset(&mut self) {
        self.state = State::new();
        self.all_sound_off();
    }

    /// Mutes or unmutes a channel; its notes play on, unheard, so it comes
    /// back in time. For music in layers that come and go.
    pub fn set_channel_muted(&mut self, channel: u8, muted: bool) {
        if let Some(m) = self.muted.get_mut(usize::from(channel)) {
            *m = muted;
        }
    }

    pub fn channel_muted(&self, channel: u8) -> bool {
        self.muted.get(usize::from(channel)).copied().unwrap_or(false)
    }

    /// The overall volume, 0 to 1 (up to 4 for quiet music), at once.
    pub fn set_volume(&mut self, volume: f32) {
        let v = if volume.is_finite() { volume.clamp(0.0, 4.0) } else { 1.0 };
        self.volume = v;
        self.fade_to = v;
        self.fade_step = 0.0;
    }

    /// The volume now (partway through a fade, where it's got to).
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Fades the volume to `volume` over `seconds`, smoothly.
    pub fn fade_to(&mut self, volume: f32, seconds: f32) {
        let v = if volume.is_finite() { volume.clamp(0.0, 4.0) } else { 1.0 };
        let frames = if seconds.is_finite() { seconds.max(0.0) * self.rate as f32 } else { 0.0 };
        if frames < 1.0 {
            self.set_volume(v);
        } else {
            self.fade_to = v;
            self.fade_step = (v - self.volume) / frames;
        }
    }

    /// Each channel's peak level since the last time they were taken
    /// (dry, as the channel's volume and pan leave it), and starts again.
    pub fn take_levels(&mut self) -> [f32; 16] {
        std::mem::take(&mut self.levels)
    }

    /// Sets a pan (-1 left to 1 right) added to one drum key on the drum
    /// channels, as a drum machine's pad pan.
    pub fn set_drum_pan(&mut self, key: u8, pan: f32) {
        self.drum_pan[usize::from(key & 127)] = pan.clamp(-1.0, 1.0);
    }

    /// Each drum key's peak since the last call, dry like [`Synth::take_levels`].
    pub fn take_drum_levels(&mut self) -> [f32; 128] {
        std::mem::replace(&mut self.drum_levels, [0.0; 128])
    }

    /// Voices sounding now (at most [`POLYPHONY`], or the cap of [`Synth::with_polyphony`]).
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// The name of the instrument (or drum kit) a channel plays: the sound
    /// bank's, if it has it.
    pub fn instrument_name(&self, channel: u8) -> &str {
        let Some(c) = self.state.channels.get(usize::from(channel)) else { return "" };
        let (bank, program) = if c.drums { (0, c.program) } else { (c.bank, c.program) };
        if let Some(name) = self.bank.as_ref().and_then(|b| b.preset_name(c.drums, bank, program)) {
            return name;
        }
        if c.drums {
            drums::KITS.iter().find(|(p, _)| *p == c.kit).map_or("Standard", |(_, name)| name)
        } else {
            c.patch.name
        }
    }

    /// Roughly how much memory it holds, in bytes (not counting its sound
    /// bank's, which is shared: [`SoundBank::memory_bytes`]).
    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of::<Synth>() + self.voices.iter().map(Voice::memory_bytes).sum::<usize>() + self.chorus.memory_bytes() + self.reverb.memory_bytes() + self.variation.memory_bytes()
    }

    /// Renders the next frames into `left` and `right` (the shorter's
    /// length).
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let mut done = 0;
        while done < n {
            let step = (n - done).min(BLOCK);
            self.render_block(&mut left[done..done + step], &mut right[done..done + step]);
            done += step;
        }
    }

    /// Renders interleaved stereo, left then right: `out.len() / 2` frames.
    pub fn render_interleaved(&mut self, out: &mut [f32]) {
        output::interleaved(out, |l, r| self.render(l, r));
    }

    /// Renders interleaved stereo as 16-bit samples, clipped at full scale.
    pub fn render_i16(&mut self, out: &mut [i16]) {
        output::interleaved_i16(out, |l, r| self.render(l, r));
    }

    pub(crate) fn message(&mut self, message: &Message) {
        let effect = self.state.apply(message);
        self.effect(effect);
    }

    pub(crate) fn send_sysex(&mut self, data: &[u8]) {
        let effect = self.state.sysex(data);
        self.effect(effect);
    }

    /// Sets the channels to `state`: where a file stands.
    pub(crate) fn set_state(&mut self, state: State) {
        self.state = state;
    }

    fn effect(&mut self, effect: Effect) {
        let voices = |ch: usize| move |v: &&mut Voice| v.active && usize::from(v.channel) == ch;
        match effect {
            Effect::None => {}
            Effect::NoteOn(ch, key, vel) => self.note_start(ch, key, vel),
            Effect::NoteOff(ch, key) => {
                let pedal = self.state.channels[ch].sustain;
                for v in self.voices.iter_mut().filter(voices(ch)).filter(|v| v.key == key && !v.released) {
                    v.note_off(pedal);
                }
            }
            Effect::PedalUp(ch) => {
                for v in self.voices.iter_mut().filter(voices(ch)).filter(|v| v.sustained) {
                    v.release();
                }
            }
            Effect::Cut(ch) => {
                for v in self.voices.iter_mut().filter(voices(ch)) {
                    v.choke_now();
                }
            }
            Effect::Release(ch) => {
                for v in self.voices.iter_mut().filter(voices(ch)).filter(|v| !v.released) {
                    v.note_off(false);
                }
            }
            Effect::ReleaseAll => self.all_notes_off(),
        }
    }

    fn note_start(&mut self, ch: usize, key: u8, vel: u8) {
        self.clock += 1;
        let c = self.state.channels[ch];
        if self.bank.as_ref().is_some_and(|b| b.has(c.drums, c.bank, c.program)) {
            self.sampled_start(ch, key, vel);
            return;
        }
        if c.drums {
            let Some(drum) = drums::drum(c.kit, key) else { return };
            if drum.choke != 0 {
                for v in self.voices.iter_mut().filter(|v| v.active && v.channel as usize == ch && v.choke == drum.choke) {
                    v.choke_now();
                }
            }
            let i = self.free_voice();
            self.voices[i].start_drum(ch as u8, key, vel, drum, self.clock);
        } else {
            // The same key again ends the last one.
            for v in self.voices.iter_mut().filter(|v| v.active && v.channel as usize == ch && v.key == key && !v.released) {
                v.note_off(false);
            }
            let i = self.free_voice();
            self.voices[i].start(ch as u8, key, vel, c.patch, self.clock);
        }
    }

    /// A note from the sound bank: a voice for each region its key and
    /// velocity fall in (a stereo sample is two, a layered sound more).
    fn sampled_start(&mut self, ch: usize, key: u8, vel: u8) {
        // Holding the bank here only counts it: the synth's own keeps it,
        // so nothing is freed while rendering.
        let Some(bank) = self.bank.clone() else { return };
        let c = self.state.channels[ch];
        let Some(regions) = bank.regions_for(c.drums, c.bank, c.program, key, vel) else { return };
        let mut first = true;
        for i in regions {
            let region = &bank.regions[i];
            if first {
                first = false;
                // The same key again ends the last one, but a drum's,
                // which rings on as drums do.
                if !c.drums {
                    for v in self.voices.iter_mut().filter(|v| v.active && v.channel as usize == ch && v.key == key && !v.released) {
                        v.note_off(false);
                    }
                }
            }
            if region.exclusive != 0 {
                for v in self.voices.iter_mut().filter(|v| v.active && v.channel as usize == ch && v.exclusive() == region.exclusive && v.started != self.clock) {
                    v.choke_now();
                }
            }
            let i = self.free_voice();
            self.voices[i].start_sampled(ch as u8, key, vel, region, self.clock);
        }
    }

    /// A voice to play: a silent one, else the quietest of those let go,
    /// else the quietest, but never one the note just started.
    fn free_voice(&mut self) -> usize {
        if let Some(i) = self.voices.iter().position(|v| !v.active) {
            return i;
        }
        let clock = self.clock;
        let quietest = |released: bool| {
            self.voices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.started != clock)
                .filter(|(_, v)| !released || v.released)
                .min_by(|(_, a), (_, b)| a.loudness().total_cmp(&b.loudness()).then(a.started.cmp(&b.started)))
                .map(|(i, _)| i)
        };
        quietest(true).or_else(|| quietest(false)).unwrap_or(0)
    }

    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        let mut chorus_in = [0f32; BLOCK];
        let mut reverb_in = [0f32; BLOCK];
        let mut variation_in = [0f32; BLOCK];
        let mut one = [0f32; BLOCK];
        left.fill(0.0);
        right.fill(0.0);
        let samples: &[i16] = self.bank.as_ref().map_or(&[], |b| &b.samples);
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let c = &self.state.channels[usize::from(v.channel)];
            one[..n].fill(0.0);
            v.render(&mut one[..n], &c.controls(), samples);
            let mute = if self.muted[usize::from(v.channel)] { 0.0 } else { 1.0 };
            let level = if v.is_sampled() { SAMPLED_LEVEL } else { 1.0 };
            let gain = c.volume * c.volume * c.expression * c.expression * 0.25 * mute * level;
            let key_pan = if c.drums { self.drum_pan[usize::from(v.key & 127)] } else { 0.0 };
            let pan = (c.pan + v.own_pan() + key_pan).clamp(-1.0, 1.0);
            let angle = (pan + 1.0) * 0.5 * FRAC_PI_2;
            let (gl, gr) = (angle.cos() * gain, angle.sin() * gain);
            let chorus = (c.chorus + v.own_chorus()).min(1.0) * gain;
            let reverb = (c.reverb + v.own_reverb()).min(1.0) * gain;
            let variation = c.variation * gain;
            let mut peak = 0f32;
            for i in 0..n {
                let s = one[i];
                left[i] += s * gl;
                right[i] += s * gr;
                chorus_in[i] += s * chorus;
                reverb_in[i] += s * reverb;
                variation_in[i] += s * variation;
                peak = peak.max(s.abs());
            }
            let level = &mut self.levels[usize::from(v.channel) & 15];
            *level = level.max(peak * gl.max(gr));
            if c.drums {
                let d = &mut self.drum_levels[usize::from(v.key & 127)];
                *d = d.max(peak * gl.max(gr));
            }
        }
        self.chorus.process(&chorus_in[..n], left, right);
        self.reverb.process(&reverb_in[..n], left, right);
        self.variation.process(self.state.variation_type, &variation_in[..n], left, right);
        let master = self.state.master;
        if master != 1.0 || self.volume != 1.0 || self.fade_step != 0.0 {
            for i in 0..n {
                if self.fade_step != 0.0 {
                    self.volume += self.fade_step;
                    if (self.fade_step > 0.0) == (self.volume >= self.fade_to) {
                        self.volume = self.fade_to;
                        self.fade_step = 0.0;
                    }
                }
                let g = master * self.volume;
                left[i] *= g;
                right[i] *= g;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn channel_state(&self, ch: usize) -> (bool, f32, bool) {
        let c = &self.state.channels[ch];
        (c.drums, c.bend_range, c.sustain)
    }

    #[cfg(test)]
    pub(crate) fn voices(&self) -> &[Voice] {
        &self.voices
    }

    #[cfg(test)]
    pub(crate) fn play_patch(&mut self, p: &'static Patch, key: u8) {
        self.state.channels[0].patch = p;
        self.note_on(0, key, 100);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // XG's multi part addresses as Yamaha lists them: 08 nn 17 vibrato delay, 18 cutoff, 19 resonance.
    #[test]
    fn xg_cutoff_and_resonance_are_at_18_and_19() {
        let mut s = State::new();
        s.sysex(&[0x43, 0x10, 0x4c, 0x00, 0x00, 0x7e, 0x00, 0xf7]);
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x17, 0x7f, 0xf7]);
        assert_eq!((s.channels[2].brightness, s.channels[2].resonance), (0.0, 0.0));
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x18, 0x00, 0xf7]);
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x19, 0x60, 0xf7]);
        assert_eq!(s.channels[2].brightness, -2.0);
        assert!(s.channels[2].resonance > 0.0);
    }

    // Yamaha's XG tables (DB50XG data list, 2026-11-14): master tune 00 00 00 (four nibbles, 0x400 the middle, tenths of a cent),
    // transpose 00 00 06, and per part 08 nn 08 note shift, 0F and 10 the note limits.
    #[test]
    fn xg_master_tune_transpose_note_shift_and_limits() {
        let mut s = State::new();
        s.sysex(&[0x43, 0x10, 0x4c, 0x00, 0x00, 0x7e, 0x00, 0xf7]);
        // +50.0 cents: 0x400 + 500 = 0x5f4.
        s.sysex(&[0x43, 0x10, 0x4c, 0x00, 0x00, 0x00, 0x00, 0x05, 0x0f, 0x04, 0xf7]);
        assert!((s.channels[3].controls().bend - 0.5).abs() < 1e-4);
        // Transpose +2 semitones.
        s.sysex(&[0x43, 0x10, 0x4c, 0x00, 0x00, 0x06, 0x42, 0xf7]);
        assert!((s.channels[3].controls().bend - 2.5).abs() < 1e-4);
        // Part 2: note shift -12, notes 48 to 72 only.
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x08, 0x34, 0xf7]);
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x0f, 0x30, 0xf7]);
        s.sysex(&[0x43, 0x10, 0x4c, 0x08, 0x02, 0x10, 0x48, 0xf7]);
        assert!(matches!(s.channel(0x92, 60, 100), Effect::NoteOn(2, 48, 100)));
        assert!(matches!(s.channel(0x92, 40, 100), Effect::None), "below the limit");
        assert!(matches!(s.channel(0x92, 80, 100), Effect::None), "above the limit");
        assert!(matches!(s.channel(0x82, 60, 0), Effect::NoteOff(2, 48)));
        // Other parts and the drums are not shifted.
        assert!(matches!(s.channel(0x93, 60, 100), Effect::NoteOn(3, 60, 100)));
        assert!(matches!(s.channel(0x99, 36, 100), Effect::NoteOn(9, 36, 100)));
    }

    #[test]
    fn a_drum_keys_own_pan_and_level() {
        let mut s = Synth::new(22_050);
        s.set_drum_pan(38, 1.0);
        s.note_on(9, 38, 110);
        s.note_on(9, 36, 110);
        let (mut l, mut r) = (vec![0f32; 2048], vec![0f32; 2048]);
        s.render(&mut l, &mut r);
        let levels = s.take_drum_levels();
        assert!(levels[38] > 0.0 && levels[36] > 0.0 && levels[42] == 0.0);
        assert_eq!(s.take_drum_levels()[38], 0.0, "taken");
        // The snare is hard right: the right channel has more than the left from it.
        let mut s = Synth::new(22_050);
        s.set_drum_pan(38, 1.0);
        s.note_on(9, 38, 110);
        let (mut l, mut r) = (vec![0f32; 2048], vec![0f32; 2048]);
        s.render(&mut l, &mut r);
        let (el, er): (f32, f32) = (l.iter().map(|x| x * x).sum(), r.iter().map(|x| x * x).sum());
        assert!(er > el * 20.0, "{el} {er}");
    }
}
