// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Klystrack (Tero Lindeman, 2009-2018): a chiptune tracker whose sounds
//! are made by its own synth, klystron's "cyd". Each channel is three
//! oscillators (a pulse, saw, triangle, noise and "pokey" LFSR wave ANDed
//! together as the SID does, four times oversampled), a wavetable sample
//! beside them, a two-operator FM modulator, ring modulation and hard
//! sync, a SID-style ADSR or a YM2149 buzz envelope, a resonant filter,
//! and eight effect buses (chorus, a 16-tap reverb, a bitcrusher). Its
//! instruments run a 32-step program of commands each tick. Each channel
//! plays its own sequence of patterns placed on the song's rows; the song
//! ticks at its own rate (50 Hz unless it says).
//!
//! Ported from klystron, whose source its author released under the MIT
//! License: every step of its integer arithmetic in the same order and
//! width, at its own 44.1 kHz, so the samples it makes are its own. Its
//! tables are computed as it computes them (its note frequencies are
//! equal temperament written to the hundredth of a hertz). Its notice:
//!
//! > Copyright (c) 2009-2018 Tero Lindeman (kometbomb)
//! >
//! > Permission is hereby granted, free of charge, to any person
//! > obtaining a copy of this software and associated documentation files
//! > (the "Software"), to deal in the Software without restriction,
//! > including without limitation the rights to use, copy, modify, merge,
//! > publish, distribute, sublicense, and/or sell copies of the Software,
//! > and to permit persons to whom the Software is furnished to do so,
//! > subject to the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be
//! > included in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! > EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! > MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! > NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//! > LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
//! > OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
//! > WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//!
//! Where klystron reads past an array (a note a semitone below the top
//! with a fraction, a wave number out of range), this reads 0 instead,
//! and a bus number past the eight is taken modulo eight. Its vibrato's
//! random steps come from the C library's rand(), the Microsoft C
//! runtime's here, as klystrack's Windows builds have them.
//!
//! Its effect buses mix channels; each bus's output is the original's,
//! and each channel is given its share: what its own copy of the bus's
//! chorus and reverb make of it (those are linear), and of what the bus
//! makes beyond that (the bitcrusher's steps), a part in proportion.
//! Ticks played only to seek or measure run the envelopes alone (they
//! decide when a note ends), not the oscillators.

use std::sync::{Arc, OnceLock};

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// The rate klystron makes everything at.
pub const RATE: u32 = 44_100;
/// The newest song version klystron reads.
const VERSION: u8 = 27;
/// A channel at full volume panned to one side peaks at this.
const LEVEL: f32 = 0.4;
/// klystron's oscillator output before its final quarter, as a float at the Tracker's level.
const SCALE: f32 = LEVEL / 32768.0;

const RANDOM_SEED: u32 = 0xf31782ce;
const OVERSAMPLE: u32 = 2;
const ACC_LENGTH: u32 = 1 << 24;
const YM_LENGTH: u32 = ACC_LENGTH;
const MAX_VOLUME: i32 = 128;
const WAVE_ENTRIES: usize = 128;
const INSTRUMENTS: usize = 128;
const RES: u64 = 65536;
pub const MAX_CHANNELS: usize = 32;
const BUSES: usize = 8;
const SUBS: usize = 3;
const PAN_CENTER: i32 = 64;
const PAN_RIGHT: i32 = 128;
const STEREO_GAIN: f64 = 2048.0;
const WAVE_AMP: i32 = 1 << 16;
const LUT_SIZE: usize = 1024;
const FREQ_TAB_SIZE: i32 = 96;
const MIDDLE_C: i32 = 48;
const PROG_LEN: usize = 32;
const MODULATOR_MAX: u64 = 1024;
const RVB_SIZE: usize = 2000 * RATE as usize / 1000;
const RVB_TAPS: usize = 16;
/// `(int)(100.0 * log(1.0 / 2048))`: a tap this quiet is off.
const RVB_LOW_LIMIT: i32 = -762;
const CHR_BUF: usize = RATE as usize * 50 / 1000;
const CHR_ACC: i32 = 256;

// A channel's flags (cyd).
const NOISE: u32 = 1;
const PULSE: u32 = 2;
const TRIANGLE: u32 = 4;
const SAW: u32 = 8;
const SYNC: u32 = 16;
const GATE: u32 = 32;
const KEY_SYNC: u32 = 64;
const METAL: u32 = 128;
const RING_MOD: u32 = 256;
const FILTER: u32 = 512;
const FX: u32 = 1024;
const YM_ENV: u32 = 2048;
const WAVE: u32 = 4096;
const WAVE_OVERRIDE_ENV: u32 = 8192;
const LFSR: u32 = 16384;
const FM: u32 = 32768;
const WAVEFORMS: u32 = NOISE | PULSE | TRIANGLE | SAW | WAVE | LFSR;

const ATTACK: u8 = 0;
const DECAY: u8 = 1;
const SUSTAIN: u8 = 2;
const RELEASE: u8 = 3;
const DONE: u8 = 4;

const YM_ENV_ATT: u8 = 1;
const YM_ENV_ALT: u8 = 2;

const FLT_LP: u8 = 0;
const FLT_HP: u8 = 1;
const FLT_BP: u8 = 2;

// A wave's flags.
const WAVE_LOOP: u32 = 1;
const WAVE_PINGPONG: u32 = 2;
const WAVE_NO_INTERPOLATION: u32 = 4;

// A bus's effects.
const FX_REVERB: u32 = 1;
const FX_CRUSH: u32 = 2;
const FX_CHORUS: u32 = 4;
const FX_CRUSH_DITHER: u32 = 8;

// An instrument's flags.
const INST_DRUM: u32 = 2;
const INST_INVERT_VIBRATO_BIT: u32 = 4;
const INST_LOCK_NOTE: u32 = 8;
const INST_SET_PW: u32 = 16;
const INST_SET_CUTOFF: u32 = 32;
const INST_YM_BUZZ: u32 = 64;
const INST_RELATIVE_VOLUME: u32 = 128;
const INST_QUARTER_FREQ: u32 = 256;
const INST_WAVE_LOCK_NOTE: u32 = 512;
const INST_NO_PROG_RESTART: u32 = 1024;
const INST_MULTIOSC: u32 = 2048;

// A channel's state in the song.
const CHN_PLAYING: u32 = 1;
const CHN_PROGRAM_RUNNING: u32 = 2;

pub const NOTE_NONE: u8 = 0xff;
pub const NOTE_RELEASE: u8 = 0xfe;
const NO_INSTRUMENT: u8 = 0xff;

const CTRL_LEGATO: u8 = 1;
const CTRL_SLIDE: u8 = 2;
const CTRL_VIB: u8 = 4;

const SONG_REVERB: u32 = 1;
const SONG_CRUSH: u32 = 2;
const SONG_MULTIPLEX: u32 = 4;

// Commands.
const FX_ARPEGGIO: u16 = 0x0000;
const FX_ARPEGGIO_ABS: u16 = 0x4000;
const FX_SET_EXT_ARP: u16 = 0x1000;
const FX_PORTA_UP: u16 = 0x0100;
const FX_PORTA_DN: u16 = 0x0200;
const FX_PORTA_UP_LOG: u16 = 0x0500;
const FX_PORTA_DN_LOG: u16 = 0x0600;
const FX_SLIDE: u16 = 0x0300;
const FX_VIBRATO: u16 = 0x0400;
const FX_FADE_VOLUME: u16 = 0x0a00;
const FX_SET_VOLUME: u16 = 0x0c00;
const FX_LOOP_PATTERN: u16 = 0x0d00;
const FX_SKIP_PATTERN: u16 = 0x2d00;
const FX_EXT: u16 = 0x0e00;
const FX_EXT_PORTA_UP: u16 = 0x0e10;
const FX_EXT_PORTA_DN: u16 = 0x0e20;
const FX_EXT_RETRIGGER: u16 = 0x0e90;
const FX_EXT_FADE_VOLUME_DN: u16 = 0x0ea0;
const FX_EXT_FADE_VOLUME_UP: u16 = 0x0eb0;
const FX_EXT_NOTE_CUT: u16 = 0x0ec0;
const FX_EXT_NOTE_DELAY: u16 = 0x0ed0;
const FX_SET_SPEED: u16 = 0x0f00;
const FX_SET_RATE: u16 = 0x1f00;
const FX_PORTA_UP_SEMI: u16 = 0x1100;
const FX_PORTA_DN_SEMI: u16 = 0x1200;
const FX_SET_PANNING: u16 = 0x1800;
const FX_PAN_LEFT: u16 = 0x1700;
const FX_PAN_RIGHT: u16 = 0x1900;
const FX_FADE_GLOBAL_VOLUME: u16 = 0x1a00;
const FX_SET_GLOBAL_VOLUME: u16 = 0x1d00;
const FX_SET_CHANNEL_VOLUME: u16 = 0x1c00;
const FX_CUTOFF_UP: u16 = 0x2100;
const FX_CUTOFF_DN: u16 = 0x2200;
const FX_CUTOFF_SET: u16 = 0x2900;
const FX_RESONANCE_SET: u16 = 0x2a00;
const FX_FILTER_TYPE: u16 = 0x2b00;
const FX_CUTOFF_SET_COMBINED: u16 = 0x2c00;
const FX_BUZZ_UP: u16 = 0x3100;
const FX_BUZZ_DN: u16 = 0x3200;
const FX_BUZZ_SHAPE: u16 = 0x3f00;
const FX_BUZZ_SET: u16 = 0x3900;
const FX_BUZZ_SET_SEMI: u16 = 0x3a00;
const FX_FM_SET_MODULATION: u16 = 0x3300;
const FX_FM_SET_FEEDBACK: u16 = 0x3400;
const FX_FM_SET_HARMONIC: u16 = 0x3500;
const FX_FM_SET_WAVEFORM: u16 = 0x3600;
const FX_PW_DN: u16 = 0x0700;
const FX_PW_UP: u16 = 0x0800;
const FX_PW_SET: u16 = 0x0900;
const FX_SET_WAVEFORM: u16 = 0x0b00;
const FX_SET_FXBUS: u16 = 0x1b00;
const FX_SET_SYNCSRC: u16 = 0x7a00;
const FX_SET_RINGSRC: u16 = 0x7b00;
const FX_SET_WAVETABLE_ITEM: u16 = 0x3b00;
const FX_SET_DOWNSAMPLE: u16 = 0x1e00;
const FX_WAVETABLE_OFFSET: u16 = 0x5000;
const FX_CUTOFF_FINE_SET: u16 = 0x6000;
const FX_END: u16 = 0xffff;
const FX_JUMP: u16 = 0xff00;
const FX_LABEL: u16 = 0xfd00;
const FX_LOOP: u16 = 0xfe00;
const FX_TRIGGER_RELEASE: u16 = 0x7c00;
const FX_RESTART_PROGRAM: u16 = 0x7d00;
const FX_NOP: u16 = 0xfffe;

// Waveform bits in the set-waveform command.
const FXW_NOISE: u16 = 1;
const FXW_PULSE: u16 = 2;
const FXW_TRIANGLE: u16 = 4;
const FXW_SAW: u16 = 8;
const FXW_LFSR: u16 = 16;
const FXW_WAVE: u16 = 32;

const SHAPE_SINE: u8 = 0;
const SHAPE_RAMP_UP: u8 = 1;
const SHAPE_RAMP_DN: u8 = 2;
const SHAPE_SQUARE: u8 = 4;

/// The YM2149's sixteen volume steps, as klystron has them (step 0 silent).
const YM_TABLE: [i32; 16] = [0, 161, 265, 377, 580, 774, 1155, 1575, 2260, 3088, 4570, 6233, 9330, 13187, 21220, 32767];
const RESONANCE: [u16; 4] = [10, 512, 1300, 1950];
const FB_TAB: [u64; 8] = [0, 64, 32, 16, 8, 4, 2, 1];
/// FM harmonics, twice their ratio.
const HARMONIC: [u64; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

struct Tables {
    /// Each note's frequency, 16ths of a hertz: C-0 to B-7.
    freq: [u16; 96],
    /// The envelope's curve.
    lut: [u16; LUT_SIZE],
    /// The vibrato and PWM shapes: a sine, and random steps.
    sine: [i8; 128],
    random: [i8; 128],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut freq = [0u16; 96];
        for (n, f) in freq.iter_mut().enumerate() {
            // Its table is equal temperament written to the hundredth, times 16.
            let hz = 440.0 * 2f64.powf((n as f64 - 57.0) / 12.0);
            let written: f64 = format!("{hz:.2}").parse().unwrap_or(hz);
            *f = (written * 16.0) as u16;
        }
        let mut lut = [0u16; LUT_SIZE];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = (i * (i / 2) / 8) as u16;
        }
        // klystron makes these when its replayer starts: the sine as
        // sin() gives it, the random steps from the C library's rand()
        // (the low byte of each). That's the Microsoft C runtime's here,
        // from its first call, as klystrack's Windows builds have it.
        let mut sine = [0i8; 128];
        let mut random = [0i8; 128];
        let mut seed = 1u32;
        for i in 0..128 {
            sine[i] = ((f64::from(i as f32 / 128.0) * std::f64::consts::PI * 2.0).sin() * 127.0) as i8;
            seed = seed.wrapping_mul(214013).wrapping_add(2531011);
            random[i] = ((seed >> 16) & 0x7fff) as u8 as i8;
        }
        Tables { freq, lut, sine, random }
    })
}

/// A note's frequency (a note is 256ths of a semitone from C-0), 16ths of a hertz.
fn get_freq(note: i32) -> u16 {
    let t = &tables().freq;
    if note <= 0 {
        return t[0];
    }
    if note >= FREQ_TAB_SIZE << 8 {
        return t[95];
    }
    let i = (note >> 8) as usize;
    if note & 0xff == 0 {
        return t[i];
    }
    // Between B-7 and the top klystron reads past its table; that's 0 here.
    let (f1, f2) = (i32::from(t[i]), t.get(i + 1).map_or(0, |&f| i32::from(f)));
    (f1 + (f2 - f1) * (note & 0xff) / 256) as u16
}

fn envspd(slope: u8) -> u32 {
    let base = 0xff0000u64;
    let s = i32::from(slope);
    (if s != 0 { base / ((s * s * 256 / 4) as u64) * 22050 / u64::from(RATE) } else { base * 22050 / u64::from(RATE) }) as u32
}

fn shift_lfsr(v: &mut u32, tap0: u32, tap1: u32) {
    let fb = (1u32 << tap0) ^ (1u32 << tap1);
    *v = (*v >> 1) ^ (0u32.wrapping_sub(*v & 1) & fb);
}

// ---------------------------------------------------------------------
// The song file.

/// A wavetable entry: a sample the oscillators can play beside their waves.
#[derive(Clone, Debug, Default)]
pub struct Wave {
    pub flags: u32,
    pub sample_rate: u32,
    pub samples: u32,
    pub loop_begin: u32,
    pub loop_end: u32,
    pub base_note: u16,
    pub data: Vec<i16>,
    pub name: String,
}

impl Wave {
    fn at(&self, i: i64) -> i32 {
        if i < 0 {
            return 0;
        }
        self.data.get(i as usize).map_or(0, |&v| i32::from(v))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instrument {
    pub flags: u32,
    pub cydflags: u32,
    pub adsr: [u8; 4],
    pub sync_source: u8,
    pub ring_mod: u8,
    pub pw: u16,
    pub volume: u8,
    pub program: [u16; PROG_LEN],
    pub prog_period: u8,
    pub vibrato_speed: u8,
    pub vibrato_depth: u8,
    pub slide_speed: u8,
    pub pwm_speed: u8,
    pub pwm_depth: u8,
    pub base_note: u8,
    pub cutoff: u16,
    pub resonance: u8,
    pub flttype: u8,
    pub ym_env_shape: u8,
    pub buzz_offset: i16,
    pub fx_bus: u8,
    pub vib_shape: u8,
    pub vib_delay: u8,
    pub pwm_shape: u8,
    pub name: String,
    pub wavetable_entry: u8,
    pub lfsr_type: u8,
    pub finetune: i8,
    pub fm_flags: u32,
    pub fm_modulation: u8,
    pub fm_feedback: u8,
    pub fm_wave: u8,
    pub fm_harmonic: u8,
    pub fm_adsr: [u8; 4],
    pub fm_attack_start: u8,
}

impl Default for Instrument {
    /// klystron's default instrument, which a file's fields overwrite.
    fn default() -> Instrument {
        Instrument {
            flags: INST_DRUM | INST_SET_PW | INST_SET_CUTOFF,
            cydflags: TRIANGLE,
            adsr: [2, 24, 0, 0],
            sync_source: 0,
            ring_mod: 0,
            pw: 0x600,
            volume: MAX_VOLUME as u8,
            program: [FX_NOP; PROG_LEN],
            prog_period: 2,
            vibrato_speed: 0x20,
            vibrato_depth: 0x20,
            slide_speed: 0x80,
            pwm_speed: 0,
            pwm_depth: 0,
            base_note: MIDDLE_C as u8,
            cutoff: 2047,
            resonance: 0,
            flttype: 0,
            ym_env_shape: 0,
            buzz_offset: 0,
            fx_bus: 0,
            vib_shape: SHAPE_SINE,
            vib_delay: 0,
            pwm_shape: 0,
            name: String::new(),
            wavetable_entry: 0,
            lfsr_type: 0,
            finetune: 0,
            fm_flags: 0,
            fm_modulation: 0,
            fm_feedback: 0,
            fm_wave: 0,
            fm_harmonic: 0,
            fm_adsr: [0; 4],
            fm_attack_start: 0,
        }
    }
}

/// A pattern row of one channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// A note (0 is C-0, 48 C-4), `NOTE_RELEASE` or `NOTE_NONE`.
    pub note: u8,
    /// The instrument, or 0xff for none.
    pub instrument: u8,
    pub ctrl: u8,
    pub command: u16,
    /// 0 to 0x80, a pan or fade in the top nibble, or 0xff for none.
    pub volume: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub steps: Vec<Step>,
}

/// A pattern placed in a channel's sequence: from row `position`, transposed.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub position: u16,
    pub pattern: u16,
    pub note_offset: i8,
}

/// An effect bus as the file has it.
#[derive(Clone, Copy, Debug, Default)]
struct FxSer {
    flags: u32,
    bit_drop: u8,
    chr_rate: u8,
    chr_min: u8,
    chr_max: u8,
    chr_sep: u8,
    taps: [(u16, i16, u8, u8); RVB_TAPS],
    downsample: u8,
    gain: u8,
}

#[derive(Debug)]
struct Data {
    file: Arc<[u8]>,
    version: u8,
    channels: usize,
    time_signature: u16,
    sequence_step: u16,
    /// All 128; the file's are the first `named`.
    instruments: Vec<Instrument>,
    named: usize,
    patterns: Vec<Pattern>,
    sequences: Vec<Vec<Placed>>,
    song_length: u16,
    loop_point: u16,
    master_volume: u8,
    speed: u8,
    speed2: u8,
    rate: u8,
    flags: u32,
    multiplex_period: u8,
    pitch_inaccuracy: u8,
    title: String,
    fx: [FxSer; BUSES],
    default_volume: [u8; MAX_CHANNELS],
    default_panning: [i8; MAX_CHANNELS],
    /// All 128 entries; those the song doesn't have are empty.
    waves: Vec<Wave>,
}

/// A Klystrack song (`cyd!song`, versions 0 to 27).
#[derive(Clone, Debug)]
pub struct Song(Arc<Data>);

/// Reads the file as klystron's `fread`s do: a field past the end is left as it was.
struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.p + n <= self.d.len() {
            let s = &self.d[self.p..self.p + n];
            self.p += n;
            Some(s)
        } else {
            self.p = self.d.len();
            None
        }
    }
    fn u8(&mut self, v: &mut u8) {
        if let Some(b) = self.take(1) {
            *v = b[0];
        }
    }
    fn i8(&mut self, v: &mut i8) {
        if let Some(b) = self.take(1) {
            *v = b[0] as i8;
        }
    }
    fn u16(&mut self, v: &mut u16) {
        if let Some(b) = self.take(2) {
            *v = u16::from_le_bytes([b[0], b[1]]);
        }
    }
    fn i16(&mut self, v: &mut i16) {
        if let Some(b) = self.take(2) {
            *v = i16::from_le_bytes([b[0], b[1]]);
        }
    }
    fn u32(&mut self, v: &mut u32) {
        if let Some(b) = self.take(4) {
            *v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
    }
    fn byte(&mut self) -> u8 {
        let mut v = 0;
        self.u8(&mut v);
        v
    }
    /// A name of `len` bytes, of which klystron keeps `max` (and reads no more).
    fn name(&mut self, len: u8, max: usize) -> String {
        let n = usize::from(len).min(max);
        let b = self.take(n).unwrap_or(&[]);
        let b = &b[..b.len().min(max - 1)];
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        b[..end].iter().map(|&c| c as char).collect::<String>().trim_end().to_string()
    }
}

pub fn probe(d: &[u8]) -> bool {
    d.len() > 16 && &d[..8] == b"cyd!song" && d[8] <= VERSION
}

fn read_instrument(r: &mut Reader, version: u8) -> Instrument {
    let mut i = Instrument::default();
    r.u32(&mut i.flags);
    r.u32(&mut i.cydflags);
    for a in i.adsr.iter_mut() {
        r.u8(a);
    }
    r.u8(&mut i.sync_source);
    r.u8(&mut i.ring_mod);
    r.u16(&mut i.pw);
    r.u8(&mut i.volume);
    let steps = r.byte();
    for k in 0..usize::from(steps) {
        let mut v = 0u16;
        r.u16(&mut v);
        if k < PROG_LEN {
            i.program[k] = v;
        }
    }
    r.u8(&mut i.prog_period);
    r.u8(&mut i.vibrato_speed);
    r.u8(&mut i.vibrato_depth);
    r.u8(&mut i.pwm_speed);
    r.u8(&mut i.pwm_depth);
    r.u8(&mut i.slide_speed);
    r.u8(&mut i.base_note);
    if version >= 20 {
        r.i8(&mut i.finetune);
    }
    let mut len = 16u8;
    if version >= 11 {
        r.u8(&mut len);
    }
    if len != 0 {
        i.name = r.name(len, 33);
    }
    if version >= 1 {
        r.u16(&mut i.cutoff);
        r.u8(&mut i.resonance);
        r.u8(&mut i.flttype);
    }
    if version >= 7 {
        r.u8(&mut i.ym_env_shape);
        r.i16(&mut i.buzz_offset);
    }
    if version >= 10 {
        r.u8(&mut i.fx_bus);
    }
    if version >= 11 {
        r.u8(&mut i.vib_shape);
        r.u8(&mut i.vib_delay);
        r.u8(&mut i.pwm_shape);
    }
    if version >= 18 {
        r.u8(&mut i.lfsr_type);
    }
    if version >= 12 {
        r.u8(&mut i.wavetable_entry);
    }
    if version >= 23 {
        r.u32(&mut i.fm_flags);
        r.u8(&mut i.fm_modulation);
        r.u8(&mut i.fm_feedback);
        r.u8(&mut i.fm_harmonic);
        for a in i.fm_adsr.iter_mut() {
            r.u8(a);
        }
    }
    if version >= 25 {
        r.u8(&mut i.fm_attack_start);
    }
    if version >= 23 {
        r.u8(&mut i.fm_wave);
    }
    if version < 26 {
        for k in [0, 1, 3] {
            i.adsr[k] = i.adsr[k].wrapping_mul(2);
            i.fm_adsr[k] = i.fm_adsr[k].wrapping_mul(2);
        }
    }
    i
}

fn read_fx(r: &mut Reader, version: u8) -> FxSer {
    let mut fx = FxSer::default();
    if version >= 22 {
        let len = r.byte();
        if len != 0 {
            r.name(len, 33);
        }
    }
    r.u32(&mut fx.flags);
    r.u8(&mut fx.bit_drop);
    r.u8(&mut fx.chr_rate);
    r.u8(&mut fx.chr_min);
    r.u8(&mut fx.chr_max);
    r.u8(&mut fx.chr_sep);
    let mut spread = 0u8;
    if version < 27 {
        r.u8(&mut spread);
    }
    if version < 21 {
        r.byte();
    }
    let taps = if version < 27 { 8 } else { RVB_TAPS };
    for t in fx.taps.iter_mut().take(taps) {
        r.u16(&mut t.0);
        r.i16(&mut t.1);
        if version >= 27 {
            r.u8(&mut t.2);
            r.u8(&mut t.3);
        } else {
            t.3 = 1;
            t.2 = if spread > 0 { 0 } else { PAN_CENTER as u8 };
        }
    }
    if version < 27 {
        for i in 8..RVB_TAPS {
            fx.taps[i] = if spread == 0 {
                (1000, RVB_LOW_LIMIT as i16, 0, 0)
            } else {
                let (d, g) = (u32::from(fx.taps[i - 8].0), fx.taps[i - 8].1);
                ((2000u32).min(d + d * u32::from(spread) / 2000) as u16, g, PAN_RIGHT as u8, 1)
            };
        }
    }
    r.u8(&mut fx.downsample);
    if version < 19 {
        fx.gain = 128;
    } else {
        r.u8(&mut fx.gain);
    }
    fx
}

/// klystron's bit-plane packing of a wave's samples (its `bitunpack`).
fn bitunpack(packed: &[u8], bits: u32, n: usize, flags: u32) -> Option<Vec<i16>> {
    struct Bits<'a> {
        d: &'a [u8],
        byte: usize,
        bit: u32,
        size_byte: usize,
        size_bit: u32,
    }
    impl Bits<'_> {
        fn r(&mut self) -> Option<bool> {
            if self.size_byte <= self.byte && self.size_bit <= self.bit {
                return None;
            }
            let b = self.d.get(self.byte).copied().unwrap_or(0) & (1 << self.bit) != 0;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.byte += 1;
            }
            Some(b)
        }
        fn gamma(&mut self) -> u32 {
            let mut n = 0;
            loop {
                match self.r() {
                    None => return 0,
                    Some(true) => break,
                    Some(false) => n += 1,
                }
            }
            if n > 31 {
                return 0;
            }
            let mut v = 0u32;
            for a in (0..n).rev() {
                match self.r() {
                    None => return 0,
                    Some(true) => v |= 1 << a,
                    Some(false) => {}
                }
            }
            v | 1 << n
        }
    }
    let mut b = Bits { d: packed, byte: 0, bit: 0, size_byte: (bits / 8) as usize, size_bit: bits & 7 };
    let mut out = vec![0i16; n];
    for plane in 0..16 {
        let mask = (1u16 << plane) as i16;
        let t = u32::from(b.r()?) | u32::from(b.r()?) << 1;
        match t {
            2 => {
                for v in out.iter_mut() {
                    if b.r()? {
                        *v |= mask;
                    }
                }
            }
            1 => {
                for v in out.iter_mut() {
                    *v |= mask;
                }
            }
            3 => {
                let mut bit = if b.r()? { mask } else { 0 };
                if let Some(v) = out.first_mut() {
                    *v |= bit;
                }
                let mut i = 0;
                while i < n {
                    let mut ctr = b.gamma();
                    if ctr == 0 {
                        return None;
                    }
                    while i < n && ctr > 0 {
                        out[i] |= bit;
                        i += 1;
                        ctr -= 1;
                    }
                    if ctr != 0 {
                        return None;
                    }
                    bit ^= mask;
                }
            }
            _ => {}
        }
    }
    if flags & 2 != 0 {
        for v in out.iter_mut() {
            let mut g = *v as u16;
            g ^= g >> 8;
            g ^= g >> 4;
            g ^= g >> 2;
            g ^= g >> 1;
            *v = (i32::from(g) - 32768) as i16;
        }
    }
    if flags & 1 != 0 {
        let mut delta = 0i32;
        for v in out.iter_mut() {
            *v = (i32::from(*v) + delta) as i16;
            delta = i32::from(*v);
        }
    }
    Some(out)
}

fn read_wave(r: &mut Reader, version: u8) -> Wave {
    let mut w = Wave::default();
    if version >= 12 {
        r.u32(&mut w.flags);
        r.u32(&mut w.sample_rate);
        r.u32(&mut w.samples);
        r.u32(&mut w.loop_begin);
        r.u32(&mut w.loop_end);
        r.u16(&mut w.base_note);
    }
    if w.samples > 0 {
        let n = w.samples as usize;
        if version < 15 {
            let raw = r.take(n * 2).unwrap_or(&[]);
            w.data = raw.as_chunks::<2>().0.iter().map(|&c| i16::from_le_bytes(c)).collect();
            w.data.resize(n, 0);
        } else {
            let mut bits = 0u32;
            r.u32(&mut bits);
            let packed = r.take((bits as usize).div_ceil(8)).unwrap_or(&[]);
            // A wave that won't unpack keeps its length and plays silence.
            w.data = if n <= 1 << 26 { bitunpack(packed, bits, n, (w.flags >> 3) & 3).unwrap_or_default() } else { Vec::new() };
        }
    }
    w
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let mut r = Reader { d, p: 8 };
        let version = r.byte();
        let mut channels = if version > 3 { 4 } else { 3 };
        if version >= 6 {
            channels = r.byte();
        }
        if channels == 0 || usize::from(channels) > MAX_CHANNELS {
            return None;
        }
        let nch = usize::from(channels);
        let mut time_signature = 0u16;
        r.u16(&mut time_signature);
        let mut sequence_step = 0u16;
        if version >= 17 {
            r.u16(&mut sequence_step);
        }
        let num_instruments = r.byte();
        let mut num_patterns = 0u16;
        r.u16(&mut num_patterns);
        let mut num_sequences = vec![0u16; nch];
        for n in num_sequences.iter_mut() {
            r.u16(n);
        }
        let (mut song_length, mut loop_point) = (0u16, 0u16);
        r.u16(&mut song_length);
        r.u16(&mut loop_point);
        // klystrack sets these before it reads a song; older songs don't have them.
        let mut master_volume = MAX_VOLUME as u8;
        if version >= 12 {
            r.u8(&mut master_volume);
        }
        let (speed, speed2, rate) = (r.byte(), r.byte(), r.byte());
        let mut flags = 0u32;
        if version > 2 {
            r.u32(&mut flags);
        }
        let mut multiplex_period = 3u8;
        if version >= 9 {
            r.u8(&mut multiplex_period);
        }
        let mut pitch_inaccuracy = 0u8;
        if version >= 16 {
            r.u8(&mut pitch_inaccuracy);
        }
        let mut title_len = 17u8;
        if version >= 11 {
            r.u8(&mut title_len);
        }
        let title = if version >= 5 { r.name(title_len, 65) } else { String::new() };
        let mut n_fx = 0u8;
        if version >= 10 {
            r.u8(&mut n_fx);
        } else if flags & SONG_REVERB != 0 {
            n_fx = 1;
        }
        let mut fx = [FxSer::default(); BUSES];
        if usize::from(n_fx) > BUSES {
            return None;
        }
        for f in fx.iter_mut().take(usize::from(n_fx)) {
            if version >= 10 {
                *f = read_fx(&mut r, version);
            } else {
                f.flags = FX_REVERB | if flags & SONG_CRUSH != 0 { FX_CRUSH } else { 0 };
                for t in f.taps.iter_mut().take(8) {
                    let (mut g, mut dl) = (0u32, 0u32);
                    r.u32(&mut g);
                    r.u32(&mut dl);
                    *t = (dl as u16, g as i16, PAN_CENTER as u8, 1);
                }
            }
        }
        let mut default_volume = [MAX_VOLUME as u8; MAX_CHANNELS];
        let mut default_panning = [0i8; MAX_CHANNELS];
        if version >= 13 {
            for v in default_volume.iter_mut().take(nch) {
                r.u8(v);
            }
            for p in default_panning.iter_mut().take(nch) {
                r.i8(p);
            }
        }
        let mut instruments: Vec<Instrument> = (0..num_instruments).map(|_| read_instrument(&mut r, version)).collect();
        let named = instruments.len();
        // klystrack has 128 instruments; those the file leaves out are its default.
        instruments.resize(INSTRUMENTS, Instrument::default());
        let mut sequences = Vec::with_capacity(nch);
        for &n in &num_sequences {
            let mut seq = Vec::with_capacity(usize::from(n));
            for _ in 0..n {
                let mut s = Placed { position: 0, pattern: 0, note_offset: 0 };
                r.u16(&mut s.position);
                r.u16(&mut s.pattern);
                r.i8(&mut s.note_offset);
                if version < 8 {
                    // Written as a C struct, padded to six bytes.
                    r.byte();
                }
                seq.push(s);
            }
            sequences.push(seq);
        }
        let mut patterns = Vec::with_capacity(usize::from(num_patterns));
        for _ in 0..num_patterns {
            let mut steps = 0u16;
            r.u16(&mut steps);
            if version >= 24 {
                r.byte();
            }
            let n = usize::from(steps);
            let mut p = Pattern { steps: Vec::with_capacity(n) };
            if version < 8 {
                for _ in 0..n {
                    let mut s = Step { note: 0, instrument: 0, ctrl: 0, command: 0, volume: NOTE_NONE };
                    r.u8(&mut s.note);
                    r.u8(&mut s.instrument);
                    r.u8(&mut s.ctrl);
                    if version >= 2 {
                        r.byte();
                        r.u16(&mut s.command);
                    }
                    // These had no volume column; klystron's left as its
                    // zeroed memory sets every row silent, so none is read.
                    p.steps.push(s);
                }
            } else {
                let packed = r.take(n / 2 + (n & 1)).unwrap_or(&[]).to_vec();
                for s in 0..n {
                    let byte = packed.get(s / 2).copied().unwrap_or(0);
                    let mut bits = if s & 1 == 1 || s == n - 1 { byte & 0xf } else { byte >> 4 };
                    let mut st = Step { note: NOTE_NONE, instrument: NO_INSTRUMENT, ctrl: 0, command: 0, volume: NOTE_NONE };
                    if bits & 1 != 0 {
                        r.u8(&mut st.note);
                    }
                    if bits & 2 != 0 {
                        r.u8(&mut st.instrument);
                    }
                    if bits & 4 != 0 {
                        r.u8(&mut st.ctrl);
                        if version >= 14 {
                            bits |= st.ctrl & !7;
                        }
                        st.ctrl &= 7;
                    }
                    if bits & 8 != 0 {
                        r.u16(&mut st.command);
                    }
                    if bits & 128 != 0 {
                        r.u8(&mut st.volume);
                    }
                    p.steps.push(st);
                }
            }
            patterns.push(p);
        }
        let mut waves = vec![Wave::default(); WAVE_ENTRIES];
        if version >= 12 {
            let max_wt = usize::from(r.byte()).min(WAVE_ENTRIES);
            for w in waves.iter_mut().take(max_wt) {
                *w = read_wave(&mut r, version);
            }
            if version >= 26 {
                for w in waves.iter_mut().take(max_wt) {
                    let len = r.byte();
                    w.name = r.name(len, 256);
                }
            }
        }
        Some(Song(Arc::new(Data {
            file: d.into(),
            version,
            channels: nch,
            time_signature,
            sequence_step,
            instruments,
            named,
            patterns,
            sequences,
            song_length,
            loop_point,
            master_volume,
            speed,
            speed2,
            rate,
            flags,
            multiplex_period,
            pitch_inaccuracy,
            title,
            fx,
            default_volume,
            default_panning,
            waves,
        })))
    }

    pub fn version(&self) -> u8 {
        self.0.version
    }
    pub fn title(&self) -> &str {
        &self.0.title
    }
    pub fn channels(&self) -> usize {
        self.0.channels
    }
    /// The instruments the file has.
    pub fn instruments(&self) -> &[Instrument] {
        &self.0.instruments[..self.0.named]
    }
    pub fn patterns(&self) -> &[Pattern] {
        &self.0.patterns
    }
    /// Each channel's patterns, in the order they're placed.
    pub fn sequences(&self) -> &[Vec<Placed>] {
        &self.0.sequences
    }
    /// The waves the song has (the rest of the 128 are empty).
    pub fn waves(&self) -> &[Wave] {
        let n = self.0.waves.iter().rposition(|w| w.samples > 0 || !w.name.is_empty()).map_or(0, |i| i + 1);
        &self.0.waves[..n]
    }
    /// How many rows the song has, and the row it loops back to.
    pub fn length(&self) -> (u16, u16) {
        (self.0.song_length, self.0.loop_point)
    }
    /// Ticks a row on even and odd rows, and ticks a second.
    pub fn speeds(&self) -> (u8, u8, u8) {
        (self.0.speed, self.0.speed2, self.0.rate)
    }
    /// Rows a beat (the high byte) and beats a measure (the low), as its editor highlights them.
    pub fn time_signature(&self) -> u16 {
        self.0.time_signature
    }
    /// The sequence editor's step, in rows.
    pub fn sequence_step(&self) -> u16 {
        self.0.sequence_step
    }
    /// Each channel's pan, -64 (left) to 64 (right).
    pub fn panning(&self, c: usize) -> i8 {
        self.0.default_panning.get(c).copied().unwrap_or(0)
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "kt"
    }
    fn file(&self) -> &[u8] {
        &self.0.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.clone()))
    }

    /// Each of the file's instruments: every field but its name, its
    /// program, then the wavetable entries it plays (its own, its FM
    /// modulator's and those its program sets), each with its number.
    fn patches(&self) -> Vec<Patch> {
        let d = &self.0;
        let mut out = Vec::new();
        let fresh = Instrument::default();
        for (k, i) in d.instruments.iter().enumerate().take(d.named) {
            // One left as a new instrument is (whatever it's called) isn't a sound anyone made.
            if (Instrument { name: String::new(), ..i.clone() }) == fresh {
                continue;
            }
            let mut b = PatchBytes::default();
            b.u32(i.flags).u32(i.cydflags).bytes(&i.adsr).u8(i.sync_source).u8(i.ring_mod).u16(i.pw).u8(i.volume);
            for &p in &i.program {
                b.u16(p);
            }
            for v in [i.prog_period, i.vibrato_speed, i.vibrato_depth, i.slide_speed, i.pwm_speed, i.pwm_depth, i.base_note] {
                b.u8(v);
            }
            b.u16(i.cutoff).u8(i.resonance).u8(i.flttype).u8(i.ym_env_shape).u16(i.buzz_offset as u16);
            for v in [i.fx_bus, i.vib_shape, i.vib_delay, i.pwm_shape, i.wavetable_entry, i.lfsr_type, i.finetune as u8] {
                b.u8(v);
            }
            b.u32(i.fm_flags).u8(i.fm_modulation).u8(i.fm_feedback).u8(i.fm_wave).u8(i.fm_harmonic).bytes(&i.fm_adsr).u8(i.fm_attack_start);
            let mut waves = Vec::new();
            if i.cydflags & WAVE != 0 {
                waves.push(i.wavetable_entry);
            }
            if i.fm_flags & WAVE != 0 {
                waves.push(i.fm_wave);
            }
            waves.extend(i.program.iter().filter(|&&p| p & 0x7f00 == FX_SET_WAVETABLE_ITEM).map(|&p| p as u8));
            waves.sort_unstable();
            waves.dedup();
            for n in waves {
                let Some(w) = d.waves.get(usize::from(n)).filter(|w| !w.data.is_empty()) else { continue };
                b.u8(n).u32(w.flags).u32(w.sample_rate).u32(w.samples).u32(w.loop_begin).u32(w.loop_end).u16(w.base_note).u32(w.data.len() as u32);
                for &x in &w.data {
                    b.u16(x as u16);
                }
            }
            out.push(b.patch(k + 1, &i.name));
        }
        out
    }
}

// ---------------------------------------------------------------------
// The synth (cyd).

#[derive(Clone, Copy, Default)]
struct Adsr {
    volume: u8,
    envelope: u32,
    env_speed: u32,
    state: u8,
    a: u8,
    d: u8,
    s: u8,
    r: u8,
}

#[derive(Clone, Copy, Default)]
struct WaveState {
    /// 0 forward, 1 back.
    direction: u8,
    playing: bool,
    acc: u64,
    frequency: u32,
}

#[derive(Clone, Copy, Default)]
struct Osc {
    frequency: u32,
    accumulator: u32,
    random: u32,
    lfsr: u32,
    lfsr_period: u32,
    lfsr_ctr: u32,
    lfsr_acc: u32,
    reg4: u32,
    reg5: u32,
    reg9: u32,
    wave: WaveState,
}

#[derive(Clone, Copy, Default)]
struct Filter {
    f: i32,
    q: i32,
    p: i32,
    b0: i32,
    b1: i32,
    b2: i32,
    b3: i32,
    b4: i32,
}

impl Filter {
    fn set(&mut self, frequency: u16, resonance: u16) {
        let fr = i32::from(frequency);
        let q = 2048 - fr;
        self.p = fr.wrapping_add((1638i32.wrapping_mul(fr) / 2048).wrapping_mul(q) / 2048);
        self.f = self.p.wrapping_add(self.p).wrapping_sub(2048);
        self.q = i32::from(resonance);
    }
    fn cycle(&mut self, input: i32) {
        let input = input.wrapping_sub(self.q.wrapping_mul(self.b4) / 2048);
        let t1 = self.b1;
        self.b1 = input.wrapping_add(self.b0).wrapping_mul(self.p) / 2048 - self.b1.wrapping_mul(self.f) / 2048;
        let t2 = self.b2;
        self.b2 = self.b1.wrapping_add(t1).wrapping_mul(self.p) / 2048 - self.b2.wrapping_mul(self.f) / 2048;
        let t1 = self.b3;
        self.b3 = self.b2.wrapping_add(t2).wrapping_mul(self.p) / 2048 - self.b3.wrapping_mul(self.f) / 2048;
        self.b4 = self.b3.wrapping_add(t1).wrapping_mul(self.p) / 2048 - self.b4.wrapping_mul(self.f) / 2048;
        self.b4 = self.b4.clamp(-32768, 32767);
        self.b0 = input;
    }
}

#[derive(Clone, Copy, Default)]
struct Fm {
    flags: u32,
    feedback: u8,
    harmonic: u8,
    adsr: Adsr,
    period: u32,
    accumulator: u32,
    wave_entry: Option<usize>,
    wave: WaveState,
    fb1: u32,
    fb2: u32,
    env_output: u32,
    current_modulation: u32,
    attack_start: u8,
}

#[derive(Clone, Copy, Default)]
struct Chan {
    flags: u32,
    pw: u16,
    sync_source: u8,
    ring_mod: u8,
    flttype: u8,
    adsr: Adsr,
    ym_env_shape: u8,
    panning: u8,
    gain_left: i32,
    gain_right: i32,
    sync_bit: u32,
    lfsr_type: u32,
    wave_entry: Option<usize>,
    sub: [Osc; SUBS],
    flt: Filter,
    fx_bus: usize,
    fm: Fm,
}

fn cycle_adsr(mut flags: u32, ym_shape: u8, a: &mut Adsr) -> u32 {
    if flags & YM_ENV == 0 {
        match a.state {
            ATTACK => {
                a.envelope = a.envelope.wrapping_add(a.env_speed);
                if a.envelope >= 0xff0000 {
                    a.state = DECAY;
                    a.envelope = 0xff0000;
                    a.env_speed = envspd(a.d);
                }
            }
            DECAY => {
                let s = u32::from(a.s) << 19;
                if a.envelope > s.wrapping_add(a.env_speed) {
                    a.envelope -= a.env_speed;
                } else {
                    a.envelope = s;
                    a.state = if a.s == 0 { RELEASE } else { SUSTAIN };
                    a.env_speed = envspd(a.r);
                }
            }
            RELEASE => {
                if a.envelope > a.env_speed {
                    a.envelope -= a.env_speed;
                } else {
                    a.state = DONE;
                    if flags & (WAVE | WAVE_OVERRIDE_ENV) != (WAVE | WAVE_OVERRIDE_ENV) {
                        flags &= !GATE;
                    }
                    a.envelope = 0;
                }
            }
            _ => {}
        }
    } else {
        match a.state {
            ATTACK => {
                a.envelope = a.envelope.wrapping_add(a.env_speed);
                if a.envelope >= YM_LENGTH {
                    if ym_shape & YM_ENV_ALT != 0 {
                        a.envelope = YM_LENGTH.wrapping_sub(a.envelope.wrapping_sub(YM_LENGTH));
                        a.state = DECAY;
                    } else {
                        a.envelope &= YM_LENGTH - 1;
                        a.state = ATTACK;
                    }
                }
            }
            DECAY => {
                if a.envelope >= a.env_speed {
                    a.envelope -= a.env_speed;
                } else if ym_shape & YM_ENV_ALT != 0 {
                    a.envelope = a.env_speed - a.envelope;
                    a.state = ATTACK;
                } else {
                    a.envelope = a.envelope.wrapping_sub(a.env_speed) & (YM_LENGTH - 1);
                    a.state = DECAY;
                }
            }
            RELEASE => {
                a.state = DONE;
                if flags & (WAVE | WAVE_OVERRIDE_ENV) != (WAVE | WAVE_OVERRIDE_ENV) {
                    flags &= !GATE;
                }
                a.envelope = 0;
            }
            _ => {}
        }
    }
    flags
}

fn env_output(flags: u32, a: &Adsr, input: i32) -> i32 {
    let vol = i64::from(a.volume);
    if flags & YM_ENV != 0 {
        let idx = (a.envelope.wrapping_mul(16) / YM_LENGTH) as usize % 16;
        input.wrapping_mul(YM_TABLE[idx]) / 32768 * i32::from(a.volume) / MAX_VOLUME
    } else if a.state == ATTACK {
        ((i64::from(input) * i64::from(a.envelope as i32 / 0x10000) / 256) * vol / 128) as i32
    } else {
        let l = tables().lut[(a.envelope / (65536 * 256 / LUT_SIZE as u32)) as usize & (LUT_SIZE - 1)];
        ((i64::from(input) * i64::from(l) / 65536) * vol / 128) as i32
    }
}

fn osc(flags: u32, acc: u32, pw: u32, random: u32, lfsr_acc: u32) -> i32 {
    let w = flags & WAVEFORMS & !WAVE;
    if w == 0 {
        return WAVE_AMP / 2;
    }
    let mut v = u32::MAX;
    if w & PULSE != 0 {
        v &= if (acc >> 8) >= (pw << 4) { 0xFFFF } else { 0 };
    }
    if w & SAW != 0 {
        v &= (acc >> 8) & 0xFFFF;
    }
    if w & TRIANGLE != 0 {
        v &= ((if acc & (ACC_LENGTH / 2) != 0 { !acc } else { acc }) >> 7) & 0x1FFFF;
    }
    if w & NOISE != 0 {
        v &= random & 0xFFFF;
    }
    if w & LFSR != 0 {
        v &= lfsr_acc;
    }
    v as i32
}

fn wave_sample(st: &WaveState, w: &Wave, acc: u64) -> i32 {
    if w.data.is_empty() {
        return 0;
    }
    if w.flags & WAVE_NO_INTERPOLATION != 0 {
        return w.at((acc / RES) as i64);
    }
    let a = (acc / RES) as i32;
    if a as u32 >= w.samples {
        return 0;
    }
    let (looped, pingpong) = (w.flags & WAVE_LOOP != 0, w.flags & WAVE_PINGPONG != 0);
    let frac = (acc as i64) % RES as i64;
    if st.direction == 0 {
        let mut b = a.wrapping_add(1);
        if looped && b as u32 >= w.loop_end {
            b = if !pingpong { (b as u32).wrapping_sub(w.loop_end).wrapping_add(w.loop_begin) as i32 } else { w.loop_end.wrapping_sub((b as u32).wrapping_sub(w.loop_end)) as i32 };
        }
        if b as u32 >= w.samples {
            return w.at(i64::from(a));
        }
        let (da, db) = (i64::from(w.at(i64::from(a))), i64::from(w.at(i64::from(b))));
        (da + (db - da) * frac / RES as i64) as i32
    } else {
        let mut b = a.wrapping_sub(1);
        if looped && b < w.loop_begin as i32 {
            b = if !pingpong { (b as u32).wrapping_sub(w.loop_begin).wrapping_add(w.loop_end) as i32 } else { w.loop_begin.wrapping_sub((b as u32).wrapping_sub(w.loop_begin)) as i32 };
        }
        if b < 0 {
            return w.at(i64::from(a));
        }
        let (da, db) = (i64::from(w.at(i64::from(a))), i64::from(w.at(i64::from(b))));
        (da + (db - da) * (RES as i64 - frac) / RES as i64) as i32
    }
}

fn wave_cycle(st: &mut WaveState, w: Option<&Wave>) {
    let Some(w) = w else { return };
    if !st.playing {
        return;
    }
    let (lb, le) = (u64::from(w.loop_begin) * RES, u64::from(w.loop_end) * RES);
    let looped = w.flags & WAVE_LOOP != 0 && w.loop_end != w.loop_begin;
    if st.direction == 0 {
        st.acc = st.acc.wrapping_add(u64::from(st.frequency));
        if looped {
            if st.acc / RES >= u64::from(w.loop_end) {
                if w.flags & WAVE_PINGPONG != 0 {
                    st.acc = le.wrapping_sub(st.acc.wrapping_sub(le));
                    st.direction = 1;
                } else {
                    st.acc = st.acc.wrapping_sub(le).wrapping_add(lb);
                }
            }
        } else if st.acc / RES >= u64::from(w.samples) {
            st.playing = false;
        }
    } else {
        st.acc = st.acc.wrapping_sub(u64::from(st.frequency));
        if looped {
            if (st.acc as i64) / (RES as i64) < i64::from(w.loop_begin) {
                if w.flags & WAVE_PINGPONG != 0 {
                    st.acc = (lb as i64).wrapping_sub((st.acc as i64).wrapping_sub(lb as i64)) as u64;
                    st.direction = 0;
                } else {
                    st.acc = st.acc.wrapping_sub(lb).wrapping_add(le);
                }
            }
        } else if (st.acc as i64) < 0 {
            st.playing = false;
        }
    }
}

impl Fm {
    fn modulator(&self, waves: &[Wave]) -> u32 {
        let fb = FB_TAB[usize::from(self.feedback).min(7)];
        let fbsum = u64::from(self.fb1.wrapping_add(self.fb2)) / 2;
        match self.wave_entry {
            Some(e) if self.flags & WAVE != 0 => {
                let w = &waves[e];
                let mut acc = self.wave.acc as u32;
                let length = u64::from(w.loop_end.wrapping_sub(w.loop_begin)) * RES;
                if length == 0 {
                    return 0;
                }
                if self.feedback != 0 {
                    acc = (u64::from(acc) + fbsum * (length * 4 / fb) / MODULATOR_MAX) as u32;
                }
                (i64::from(wave_sample(&self.wave, w, u64::from(acc) % length)) * i64::from(self.env_output) / 32768 + 65536) as u32
            }
            _ => {
                let mut acc = self.accumulator;
                if self.feedback != 0 {
                    acc = acc.wrapping_add((fbsum * (u64::from(ACC_LENGTH) * 4 / fb) / MODULATOR_MAX) as u32);
                }
                (osc(TRIANGLE, acc % ACC_LENGTH, 0, 0, 0) as u64 * u64::from(self.env_output) / WAVE_AMP as u64 + WAVE_AMP as u64 / 2) as u32
            }
        }
    }

    fn cycle(&mut self, waves: &[Wave]) {
        cycle_adsr(0, 0, &mut self.adsr);
        self.env_output = env_output(0, &self.adsr, MODULATOR_MAX as i32) as u32;
        wave_cycle(&mut self.wave, self.wave_entry.map(|e| &waves[e]));
        self.accumulator = self.accumulator.wrapping_add(self.period) % ACC_LENGTH;
        let m = self.modulator(waves);
        self.fb2 = self.fb1;
        self.fb1 = m;
        self.current_modulation = m;
    }

    fn set_frequency(&mut self, base: u32, waves: &[Wave]) {
        let (hn, hd) = (HARMONIC[usize::from(self.harmonic & 15)], HARMONIC[usize::from(self.harmonic >> 4)]);
        self.period = ((u64::from(ACC_LENGTH) / 16 * u64::from(base) / u64::from(RATE)) * hn / hd) as u32;
        if let Some(e) = self.wave_entry {
            let w = &waves[e];
            self.wave.playing = true;
            self.wave.frequency = ((RES * u64::from(w.sample_rate) / u64::from(RATE) * u64::from(base) / u64::from(get_freq(i32::from(w.base_note)))) * hn / hd) as u32;
        }
    }

    fn modulate(&self) -> u32 {
        ((u64::from(self.current_modulation) * u64::from(ACC_LENGTH) * 8 / MODULATOR_MAX) as u32) % ACC_LENGTH
    }

    fn modulate_wave(&self, w: &Wave, acc: u64) -> u64 {
        if w.loop_begin == w.loop_end {
            return acc;
        }
        let length = u64::from(w.loop_end.wrapping_sub(w.loop_begin)) * RES;
        let m = u64::from(self.current_modulation).wrapping_mul(length).wrapping_mul(8) / MODULATOR_MAX;
        m.wrapping_add(acc) % length
    }

    fn set_wave_entry(&mut self, e: Option<usize>) {
        self.wave_entry = e;
        self.wave.frequency = 0;
        self.wave.direction = 0;
    }
}

impl Chan {
    fn init(&mut self, index: usize) {
        *self = Chan::default();
        self.pw = 0x400;
        self.set_filter(2047, 0);
        self.set_panning(PAN_CENTER);
        for s in self.sub.iter_mut() {
            s.random = RANDOM_SEED;
            s.reg4 = 1;
            s.reg5 = 1;
            s.reg9 = 1;
        }
        self.sync_source = index as u8;
    }

    fn set_filter(&mut self, cutoff: u16, resonance: u8) {
        self.flt.set(cutoff, RESONANCE[usize::from(resonance & 3)]);
    }

    /// klystron's pan: `panning` 0 (left) to 128 (right), constant power.
    fn set_panning(&mut self, panning: i32) {
        let p = panning as u8;
        if self.panning == p {
            return;
        }
        self.panning = i32::from(p).clamp(0, PAN_RIGHT) as u8;
        let a = (std::f64::consts::PI / 2.0 * f64::from(f32::from(self.panning)) / 128.0) as f32;
        self.gain_left = (f64::from(a).cos() * STEREO_GAIN) as i32;
        self.gain_right = (f64::from(a).sin() * STEREO_GAIN) as i32;
    }

    fn set_frequency(&mut self, s: usize, frequency: u16, waves: &[Wave]) {
        if frequency != 0 {
            self.sub[s].frequency = (u64::from(ACC_LENGTH >> OVERSAMPLE) / 16 * u64::from(frequency) / u64::from(RATE)) as u32;
            self.sub[s].lfsr_period = (u64::from(RATE) * 16 / u64::from(frequency)) as u32;
        } else {
            self.sub[s].frequency = 0;
        }
        if s == 0 {
            self.fm.set_frequency(u32::from(frequency), waves);
        }
    }

    fn set_wavetable_frequency(&mut self, s: usize, frequency: u16, waves: &[Wave]) {
        match self.wave_entry {
            Some(e) if frequency != 0 => {
                let w = &waves[e];
                self.sub[s].wave.frequency = (RES * u64::from(w.sample_rate) / u64::from(RATE) * u64::from(frequency) / u64::from(get_freq(i32::from(w.base_note)))) as u32;
            }
            _ => {
                self.sub[s].wave.playing = false;
                self.sub[s].wave.frequency = 0;
            }
        }
    }

    fn set_env_frequency(&mut self, frequency: u16) {
        self.adsr.env_speed = (u64::from(YM_LENGTH) / 16 * u64::from(frequency) / u64::from(RATE)) as u32;
    }

    fn set_env_shape(&mut self, shape: u8) {
        self.ym_env_shape = shape;
        if self.flags & KEY_SYNC != 0 || self.adsr.state == DONE || self.adsr.state == SUSTAIN {
            if shape & YM_ENV_ATT != 0 {
                self.adsr.envelope = 0;
                self.adsr.state = ATTACK;
            } else {
                self.adsr.envelope = YM_LENGTH;
                self.adsr.state = DECAY;
            }
        }
    }

    fn enable_gate(&mut self, enable: bool) {
        if enable {
            if self.flags & YM_ENV == 0 {
                self.adsr.state = ATTACK;
                self.adsr.envelope = 0;
                self.adsr.env_speed = envspd(self.adsr.a);
                self.flags = cycle_adsr(self.flags, self.ym_env_shape, &mut self.adsr);
                self.fm.adsr.state = ATTACK;
                self.fm.adsr.envelope = u32::from(self.fm.attack_start) << 19;
                self.fm.adsr.env_speed = envspd(self.fm.adsr.a);
                cycle_adsr(0, 0, &mut self.fm.adsr);
            }
            if self.flags & KEY_SYNC != 0 {
                for s in self.sub.iter_mut() {
                    s.accumulator = 0;
                    s.reg4 = 1;
                    s.reg5 = 1;
                    s.reg9 = 1;
                    s.lfsr_ctr = 0;
                }
                self.fm.accumulator = 0;
                self.fm.wave.acc = 0;
            }
            self.flags |= GATE;
        } else {
            self.flags &= !WAVE_OVERRIDE_ENV;
            // A release from the attack starts where the attack had got to.
            if self.adsr.state == ATTACK {
                self.adsr.envelope = (self.adsr.envelope as f32 * 65536.0f32 * 256.0f32).sqrt() as u32;
            }
            self.adsr.state = RELEASE;
            self.adsr.env_speed = envspd(self.adsr.r);
            if self.fm.adsr.state == ATTACK {
                self.fm.adsr.envelope = (self.fm.adsr.envelope as f32 * 65536.0f32 * 256.0f32).sqrt() as u32;
            }
            self.fm.adsr.state = RELEASE;
            self.fm.adsr.env_speed = envspd(self.fm.adsr.r);
        }
    }

    fn set_waveform(&mut self, wave: u32) {
        self.flags = (self.flags & !WAVEFORMS) | (wave & WAVEFORMS);
    }

    fn set_wave_entry(&mut self, e: Option<usize>) {
        self.wave_entry = e;
        for s in self.sub.iter_mut() {
            s.wave = WaveState { direction: 0, playing: true, acc: 0, frequency: 0 };
        }
    }

    fn set_wavetable_offset(&mut self, offset: u16, waves: &[Wave]) {
        if let Some(e) = self.wave_entry {
            let n = u64::from(waves[e].samples);
            for s in self.sub.iter_mut() {
                s.wave.acc = u64::from(offset) * RES * n / 0x1000;
            }
        }
    }

    fn run_lfsrs(&mut self) {
        let wide = self.lfsr_type & 8 == 0;
        for s in self.sub.iter_mut() {
            shift_lfsr(&mut s.reg4, 4, 3);
            shift_lfsr(&mut s.reg5, 5, 3);
            if wide {
                shift_lfsr(&mut s.reg9, 17, 14);
            } else {
                shift_lfsr(&mut s.reg9, 9, 5);
            }
        }
    }

    fn advance_oscillators(&mut self) {
        for s in 0..SUBS {
            let o = &mut self.sub[s];
            let prev = o.accumulator;
            o.accumulator = o.accumulator.wrapping_add(o.frequency);
            if s == 0 {
                self.sync_bit |= o.accumulator & ACC_LENGTH;
            }
            o.accumulator &= ACC_LENGTH - 1;
            if (prev & (ACC_LENGTH / 32)) != (o.accumulator & (ACC_LENGTH / 32)) {
                if self.flags & METAL != 0 {
                    shift_lfsr(&mut o.random, 0xe, 8);
                    o.random &= (1 << (0xe + 1)) - 1;
                } else {
                    shift_lfsr(&mut o.random, 22, 17);
                    o.random &= (1 << (22 + 1)) - 1;
                }
            }
            if self.flags & LFSR != 0 {
                o.lfsr_acc = if o.lfsr & 1 != 0 { (WAVE_AMP - 1) as u32 } else { 0 };
                if o.lfsr_ctr >= o.lfsr_period {
                    o.lfsr_ctr = 0;
                    match self.lfsr_type & 3 {
                        0 => o.lfsr ^= u32::from(o.reg5 & o.reg9 & 1 != 0),
                        2 => o.lfsr ^= u32::from(o.reg5 & o.reg4 & 1 != 0),
                        _ => o.lfsr ^= u32::from(o.reg5 & 1 != 0),
                    }
                }
                o.lfsr_ctr = o.lfsr_ctr.wrapping_add(1);
                self.run_lfsrs();
            }
        }
    }

    fn output(&mut self) -> i32 {
        self.sync_bit = 0;
        let m = if self.flags & FM != 0 { self.fm.modulate() } else { 0 };
        let mut ovr = 0i32;
        for _ in 0..1 << OVERSAMPLE {
            for s in 0..SUBS {
                let o = &self.sub[s];
                if o.frequency != 0 {
                    let acc = o.accumulator.wrapping_add(m);
                    ovr = ovr.wrapping_add(osc(self.flags, acc % ACC_LENGTH, u32::from(self.pw), o.random, o.lfsr_acc) - WAVE_AMP / 2);
                }
            }
            self.advance_oscillators();
        }
        ovr >> OVERSAMPLE
    }

    /// The wave's samples, or 0 when the wave isn't on.
    fn wave_output(&self, waves: &[Wave], volume: Option<i32>) -> i32 {
        let Some(e) = self.wave_entry else { return 0 };
        let w = &waves[e];
        let mut o = 0i32;
        for s in &self.sub {
            if s.wave.playing && s.wave.frequency != 0 {
                let acc = if self.flags & FM != 0 { self.fm.modulate_wave(w, s.wave.acc) } else { s.wave.acc };
                let v = wave_sample(&s.wave, w, acc);
                o = o.wrapping_add(match volume {
                    Some(vol) => v.wrapping_mul(vol) / MAX_VOLUME,
                    None => v,
                });
            }
        }
        o
    }

    fn cycle(&mut self, waves: &[Wave]) {
        self.flags = cycle_adsr(self.flags, self.ym_env_shape, &mut self.adsr);
        if self.flags & WAVE != 0 {
            let w = self.wave_entry.map(|e| &waves[e]);
            for s in self.sub.iter_mut() {
                wave_cycle(&mut s.wave, w);
            }
        }
        if self.flags & FM != 0 {
            self.fm.cycle(waves);
        }
    }

    /// The envelopes alone, for ticks no one hears.
    fn cycle_envelopes(&mut self) {
        self.flags = cycle_adsr(self.flags, self.ym_env_shape, &mut self.adsr);
        if self.flags & FM != 0 {
            cycle_adsr(0, 0, &mut self.fm.adsr);
        }
    }

    fn sync(&mut self) {
        for s in self.sub.iter_mut() {
            s.wave.acc = 0;
            s.wave.direction = 0;
            s.accumulator = 0;
            s.random = RANDOM_SEED;
            s.reg4 = 1;
            s.reg5 = 1;
            s.reg9 = 1;
            s.lfsr_ctr = 0;
        }
    }
}

// The effect buses.

#[derive(Clone)]
struct Reverb {
    buffer: Vec<i32>,
    position: usize,
    /// Each tap's place in the buffer and its gain to the left and right.
    taps: [(usize, i32, i32); RVB_TAPS],
}

impl Reverb {
    fn new() -> Reverb {
        Reverb { buffer: vec![0; RVB_SIZE * 2], position: 0, taps: [(0, 0, 0); RVB_TAPS] }
    }

    fn set_tap(&mut self, i: usize, delay_ms: i32, gain_db: i32, panning: i32) {
        let size = RVB_SIZE as i32;
        let pos = (self.position as i32).wrapping_sub(delay_ms.wrapping_mul(RATE as i32) / 1000).wrapping_add(size);
        self.taps[i].0 = pos.rem_euclid(size) as usize;
        if gain_db <= RVB_LOW_LIMIT {
            self.taps[i].1 = 0;
            self.taps[i].2 = 0;
        } else {
            let p = panning.clamp(0, PAN_RIGHT);
            let a = (std::f64::consts::PI / 2.0 * f64::from(p as f32) / 128.0) as f32;
            let gain = (10f64.powf(f64::from(gain_db) * 0.01) * 2048.0) as i32;
            self.taps[i].1 = (f64::from(gain) * f64::from(a).cos()) as i32;
            self.taps[i].2 = (f64::from(gain) * f64::from(a).sin()) as i32;
        }
    }

    fn cycle(&mut self, left: i32, right: i32) {
        for t in self.taps.iter_mut() {
            t.0 += 1;
            if t.0 >= RVB_SIZE {
                t.0 = 0;
            }
        }
        self.position += 1;
        if self.position >= RVB_SIZE {
            self.position = 0;
        }
        self.buffer[self.position * 2] = left;
        self.buffer[self.position * 2 + 1] = right;
    }

    fn output(&self) -> (i32, i32) {
        let (mut l, mut r) = (0i32, 0i32);
        for &(p, gl, gr) in &self.taps {
            if gl != 0 || gr != 0 {
                l = l.wrapping_add(gl.wrapping_mul(self.buffer[p * 2]) / 2048);
                r = r.wrapping_add(gr.wrapping_mul(self.buffer[p * 2 + 1]) / 2048);
            }
        }
        (l, r)
    }
}

#[derive(Clone)]
struct Chorus {
    buffer: Vec<i32>,
    lut: Arc<Vec<i32>>,
    lut_size: usize,
    pos_l: usize,
    pos_r: usize,
    pos_buf: usize,
}

impl Chorus {
    fn new() -> Chorus {
        Chorus { buffer: vec![0; CHR_BUF * 2], lut: Arc::new(vec![0]), lut_size: 0, pos_l: 0, pos_r: 0, pos_buf: 0 }
    }

    fn set(&mut self, rate: i32, min_delay: i32, max_delay: i32, sep: i32) {
        let sr = RATE as i32;
        if rate != 0 {
            let size = (sr * 4 * 10 / (10 + (rate - 1))) as usize;
            self.lut_size = size;
            self.pos_l = 0;
            self.pos_r = (sep as usize * size / 2 / 64) % size;
            let mut lut = vec![0i32; size.max(1)];
            for (i, v) in lut.iter_mut().enumerate().take(size) {
                let s = ((i as f64 / size as f64 * std::f64::consts::PI * 2.0).sin() * 0.5 + 0.5) * f64::from(max_delay - min_delay) + f64::from(min_delay);
                *v = (s * f64::from(CHR_ACC) * f64::from(sr) / 10000.0) as i32 % (CHR_BUF as i32 * CHR_ACC);
            }
            self.lut = Arc::new(lut);
        } else {
            self.pos_l = 0;
            self.pos_r = 0;
            self.lut_size = 0;
            self.lut = Arc::new(vec![sr * min_delay / 10000]);
        }
    }

    fn output(&mut self, in_l: i32, in_r: i32) -> (i32, i32) {
        self.pos_buf += 1;
        if self.pos_buf >= CHR_BUF {
            self.pos_buf = 0;
        }
        let in_c = in_l.wrapping_add(in_r) / 2;
        self.buffer[self.pos_buf] = in_c;
        self.buffer[self.pos_buf + CHR_BUF] = in_c;
        self.pos_l += 1;
        if self.pos_l >= self.lut_size {
            self.pos_l = 0;
        }
        self.pos_r += 1;
        if self.pos_r >= self.lut_size {
            self.pos_r = 0;
        }
        let tap = |pos: usize| -> i32 {
            let d = self.lut[pos];
            let at = |k: i32| self.buffer.get((self.pos_buf as i32 - k + CHR_BUF as i32) as usize).copied().unwrap_or(0);
            let (a, b) = (at(d / CHR_ACC), at(d / CHR_ACC + 1));
            a.wrapping_add(b.wrapping_sub(a).wrapping_mul(d % CHR_ACC) / CHR_ACC)
        };
        let l = if self.lut_size != 0 { tap(self.pos_l) } else { in_c };
        (l, tap(self.pos_r))
    }
}

#[derive(Clone, Copy, Default)]
struct Crush {
    hold_l: i32,
    hold_r: i32,
    error_l: i32,
    error_r: i32,
    counter: i32,
    bit_drop: i32,
    downsample: i32,
    dither: bool,
    gain: i32,
}

impl Crush {
    fn output(&mut self, l: i32, r: i32) -> (i32, i32) {
        let c = self.counter;
        self.counter += 1;
        if c >= self.downsample {
            self.counter = 0;
            if !self.dither {
                self.hold_l = (l.wrapping_add(32768) & self.bit_drop) - 32768;
                self.hold_r = (r.wrapping_add(32768) & self.bit_drop) - 32768;
            } else {
                self.hold_l = (l.wrapping_add(32768).wrapping_add(self.error_l).clamp(0, 65535) & self.bit_drop) - 32768;
                self.hold_r = (r.wrapping_add(32768).wrapping_add(self.error_r).clamp(0, 65535) & self.bit_drop) - 32768;
                self.error_l = self.error_l.wrapping_add(l - self.hold_l);
                self.error_r = self.error_r.wrapping_add(r - self.hold_r);
            }
        }
        (self.hold_l.wrapping_mul(self.gain) / 128, self.hold_r.wrapping_mul(self.gain) / 128)
    }
}

#[derive(Clone)]
struct Bus {
    flags: u32,
    crush: Crush,
    rvb: Reverb,
    chr: Chorus,
}

impl Bus {
    fn new(s: &FxSer) -> Bus {
        let mut b = Bus { flags: s.flags, crush: Crush { gain: 128, ..Crush::default() }, rvb: Reverb::new(), chr: Chorus::new() };
        for (i, t) in s.taps.iter().enumerate() {
            if t.3 & 1 != 0 {
                b.rvb.set_tap(i, i32::from(t.0), i32::from(t.1), i32::from(t.2));
            } else {
                b.rvb.set_tap(i, 0, RVB_LOW_LIMIT, 0);
            }
        }
        b.chr.set(i32::from(s.chr_rate), i32::from(s.chr_min), i32::from(s.chr_max), i32::from(s.chr_sep));
        b.crush.downsample = i32::from(s.downsample);
        b.crush.bit_drop = (0xffffffffu32 << s.bit_drop.min(31)) as i32;
        b.crush.dither = s.flags & FX_CRUSH_DITHER != 0;
        b.crush.gain = i32::from(s.gain);
        b
    }

    /// What the chorus and reverb make of the input (the linear part).
    fn linear(&mut self, l: i32, r: i32) -> (i32, i32) {
        let (mut ol, mut or) = (l, r);
        if self.flags & FX_CHORUS != 0 {
            (ol, or) = self.chr.output(l, r);
        }
        if self.flags & FX_REVERB != 0 {
            // klystron feeds its reverb the left side twice.
            self.rvb.cycle(l, l);
            let (rl, rr) = self.rvb.output();
            ol = ol.wrapping_add(rl);
            or = or.wrapping_add(rr);
        }
        (ol, or)
    }

    fn output(&mut self, l: i32, r: i32) -> (i32, i32) {
        let (ol, or) = self.linear(l, r);
        if self.flags & FX_CRUSH != 0 {
            self.crush.output(ol, or)
        } else {
            (ol, or)
        }
    }

    /// A copy for one channel's share: the same settings and places, nothing heard yet.
    fn share(&self) -> Bus {
        let mut b = self.clone();
        b.rvb.buffer.iter_mut().for_each(|v| *v = 0);
        b.chr.buffer.iter_mut().for_each(|v| *v = 0);
        b
    }
}

// ---------------------------------------------------------------------
// The replayer (klystron's music routine).

#[derive(Clone, Copy, Default)]
struct MusChannel {
    instrument: Option<usize>,
    note: u16,
    volume: u8,
    arpeggio_note: u8,
    target_note: u16,
    last_note: u16,
    fixed_note: u16,
    flags: u32,
    current_tick: u32,
    program_counter: u8,
    program_tick: u8,
    program_loop: u8,
    prog_period: u8,
    buzz_offset: i16,
}

#[derive(Clone, Copy, Default)]
struct Track {
    pattern: Option<usize>,
    pw: u16,
    pattern_step: u16,
    sequence_position: u16,
    slide_speed: u16,
    vibrato_position: u16,
    pwm_position: u16,
    note_offset: i8,
    filter_cutoff: u16,
    filter_resonance: u8,
    extarp1: u8,
    extarp2: u8,
    volume: u8,
    vib_delay: u8,
}

struct Engine {
    song: Song,
    ch: Vec<Chan>,
    mus: Vec<MusChannel>,
    tracks: Vec<Track>,
    buses: Vec<Bus>,
    speed: u8,
    speed2: u8,
    rate: u8,
    song_counter: u8,
    song_position: u16,
    volume: u8,
    play_volume: u8,
    multiplex_ctr: u8,
    pitch_mask: u32,
    /// Samples a tick, and how many to go before the next tick, less one.
    period: u32,
    counter: u32,
    /// The song came back round to its loop point.
    looped: bool,
}

impl Engine {
    fn new(song: Song) -> Engine {
        let d = song.0.clone();
        let n = d.channels;
        let mut ch = vec![Chan::default(); n];
        for (i, c) in ch.iter_mut().enumerate() {
            c.init(i);
        }
        let buses = d.fx.iter().map(Bus::new).collect();
        let pitch_mask = u32::MAX.checked_shl(u32::from(d.pitch_inaccuracy)).unwrap_or(0);
        let rate = d.rate.max(1);
        let mut e = Engine {
            song,
            ch,
            mus: vec![MusChannel { volume: MAX_VOLUME as u8, ..Default::default() }; n],
            tracks: vec![Track::default(); n],
            buses,
            speed: d.speed,
            speed2: d.speed2,
            rate,
            song_counter: 0,
            song_position: 0,
            volume: d.master_volume,
            play_volume: MAX_VOLUME as u8,
            multiplex_ctr: 0,
            pitch_mask,
            period: RATE / u32::from(rate),
            counter: 0,
            looped: false,
        };
        for i in 0..n {
            e.mus[i].volume = d.default_volume[i];
            e.ch[i].set_panning(i32::from(d.default_panning[i]) + PAN_CENTER);
        }
        e
    }

    fn data(&self) -> &Data {
        &self.song.0
    }

    fn update_volumes(&mut self, c: usize, volume: i32) {
        let rel = self.mus[c].instrument.is_some_and(|i| self.data().instruments[i].flags & INST_RELATIVE_VOLUME != 0);
        let (mv, pv, cv) = (i32::from(self.volume), i32::from(self.play_volume), i32::from(self.mus[c].volume));
        self.tracks[c].volume = volume as u8;
        let v = if rel {
            let iv = i32::from(self.data().instruments[self.mus[c].instrument.unwrap_or(0)].volume);
            iv * volume / MAX_VOLUME * mv / MAX_VOLUME * pv / MAX_VOLUME * cv / MAX_VOLUME
        } else {
            i32::from(self.tracks[c].volume) * mv / MAX_VOLUME * pv / MAX_VOLUME * cv / MAX_VOLUME
        };
        self.ch[c].adsr.volume = v as u8;
    }

    fn update_all_volumes(&mut self) {
        for c in 0..self.ch.len() {
            let v = i32::from(self.tracks[c].volume);
            self.update_volumes(c, v);
        }
    }

    fn instrument(&self, c: usize) -> Option<&Instrument> {
        self.mus[c].instrument.map(|i| &self.data().instruments[i])
    }

    fn wave_index(i: u8) -> Option<usize> {
        (usize::from(i) < WAVE_ENTRIES).then_some(usize::from(i))
    }

    fn set_buzz_frequency(&mut self, c: usize, note: u16) {
        if self.instrument(c).is_some_and(|i| i.flags & INST_YM_BUZZ != 0) {
            let f = (u32::from(get_freq(i32::from(note) + i32::from(self.mus[c].buzz_offset))) & self.pitch_mask) as u16;
            self.ch[c].set_env_frequency(f);
        }
    }

    /// Each oscillator's note: the first the note, the other two the
    /// extended arpeggio's (when the instrument sounds all three).
    fn sub_note(&self, c: usize, s: usize, note: u16, multi: bool) -> u16 {
        let t = &self.tracks[c];
        match s {
            0 => note,
            1 if multi && t.extarp1 != 0 => note.wrapping_add(u16::from(t.extarp1) << 8),
            2 if multi && t.extarp2 != 0 => note.wrapping_add(u16::from(t.extarp2) << 8),
            _ => 0,
        }
    }

    fn set_wavetable_frequency(&mut self, c: usize, note: u16) {
        let Some(ins) = self.instrument(c) else { return };
        let (cydflags, flags) = (ins.cydflags, ins.flags);
        let Some(e) = self.ch[c].wave_entry else { return };
        if cydflags & WAVE == 0 {
            return;
        }
        let base = self.data().waves[e].base_note;
        for s in 0..SUBS {
            let f = self.sub_note(c, s, note, flags & INST_MULTIOSC != 0);
            let wf = if f != 0 { (u32::from(get_freq(if flags & INST_WAVE_LOCK_NOTE != 0 { i32::from(base) } else { i32::from(f) })) & self.pitch_mask) as u16 } else { 0 };
            let song = self.song.clone();
            self.ch[c].set_wavetable_frequency(s, wf, &song.0.waves);
        }
    }

    fn set_frequency(&mut self, c: usize, note: u16, divider: u16) {
        let multi = self.instrument(c).is_some_and(|i| i.flags & INST_MULTIOSC != 0);
        let song = self.song.clone();
        for s in 0..SUBS {
            let f = self.sub_note(c, s, note, multi);
            let fr = if f != 0 { (u32::from(get_freq(i32::from(f))) & self.pitch_mask) as u16 } else { 0 };
            self.ch[c].set_frequency(s, fr / divider, &song.0.waves);
        }
    }

    fn set_note(&mut self, c: usize, note: u16, update: bool, divider: u16) {
        if update {
            self.mus[c].note = note;
        }
        self.set_frequency(c, note, divider);
        self.set_wavetable_frequency(c, note);
        self.set_buzz_frequency(c, note);
    }

    fn do_command(&mut self, c: usize, tick: i32, inst: u16, from_program: bool) {
        let song = self.song.clone();
        let waves = &song.0.waves;
        match inst & 0x7f00 {
            FX_PORTA_UP => {
                let prev = self.mus[c].note;
                self.mus[c].note = prev.wrapping_add((inst & 0xff) << 2);
                if prev > self.mus[c].note {
                    self.mus[c].note = 0xffff;
                }
                self.mus[c].target_note = self.mus[c].note;
            }
            FX_PORTA_DN => {
                let prev = self.mus[c].note;
                self.mus[c].note = prev.wrapping_sub((inst & 0xff) << 2);
                if prev < self.mus[c].note {
                    self.mus[c].note = 0;
                }
                self.mus[c].target_note = self.mus[c].note;
            }
            FX_PORTA_UP_LOG | FX_PORTA_DN_LOG => {
                let prev = self.mus[c].note;
                let step = (u32::from(tables().freq[MIDDLE_C as usize]) * u32::from(inst & 0xff) / u32::from(get_freq(i32::from(prev)))).max(1) as u16;
                if inst & 0x7f00 == FX_PORTA_UP_LOG {
                    self.mus[c].note = prev.wrapping_add(step);
                    if prev > self.mus[c].note {
                        self.mus[c].note = 0xffff;
                    }
                } else {
                    self.mus[c].note = prev.wrapping_sub(step);
                    if prev < self.mus[c].note {
                        self.mus[c].note = 0;
                    }
                }
                self.mus[c].target_note = self.mus[c].note;
            }
            FX_PW_DN => {
                let t = &mut self.tracks[c];
                t.pw = t.pw.wrapping_sub(inst & 0xff);
                if t.pw > 0xf000 {
                    t.pw = 0;
                }
            }
            FX_PW_UP => {
                let t = &mut self.tracks[c];
                t.pw = t.pw.wrapping_add(inst & 0xff);
                if t.pw > 0x7ff {
                    t.pw = 0x7ff;
                }
            }
            FX_CUTOFF_DN => {
                let t = &mut self.tracks[c];
                t.filter_cutoff = t.filter_cutoff.wrapping_sub(inst & 0xff);
                if t.filter_cutoff > 0xf000 {
                    t.filter_cutoff = 0;
                }
                let (fc, fr) = (t.filter_cutoff, t.filter_resonance);
                self.ch[c].set_filter(fc, fr);
            }
            FX_CUTOFF_UP => {
                let t = &mut self.tracks[c];
                t.filter_cutoff = t.filter_cutoff.wrapping_add(inst & 0xff);
                if t.filter_cutoff > 0x7ff {
                    t.filter_cutoff = 0x7ff;
                }
                let (fc, fr) = (t.filter_cutoff, t.filter_resonance);
                self.ch[c].set_filter(fc, fr);
            }
            FX_BUZZ_DN => {
                let m = &mut self.mus[c];
                if i32::from(m.buzz_offset) >= -32768 + i32::from(inst & 0xff) {
                    m.buzz_offset = (i32::from(m.buzz_offset) - i32::from(inst & 0xff)) as i16;
                }
                let n = m.note;
                self.set_buzz_frequency(c, n);
            }
            FX_BUZZ_UP => {
                let m = &mut self.mus[c];
                if i32::from(m.buzz_offset) <= 32767 - i32::from(inst & 0xff) {
                    m.buzz_offset = (i32::from(m.buzz_offset) + i32::from(inst & 0xff)) as i16;
                }
                let n = m.note;
                self.set_buzz_frequency(c, n);
            }
            FX_TRIGGER_RELEASE => {
                if tick == i32::from(inst & 0xff) {
                    self.ch[c].enable_gate(false);
                }
            }
            FX_FADE_VOLUME => {
                let t = &mut self.tracks[c];
                t.volume = t.volume.wrapping_sub((inst & 0xf) as u8);
                if i32::from(t.volume) > MAX_VOLUME {
                    t.volume = 0;
                }
                t.volume = t.volume.wrapping_add(((inst >> 4) & 0xf) as u8);
                if i32::from(t.volume) > MAX_VOLUME {
                    t.volume = MAX_VOLUME as u8;
                }
                let v = i32::from(t.volume);
                self.update_volumes(c, v);
            }
            FX_PAN_RIGHT | FX_PAN_LEFT => {
                let mut p = i32::from(self.ch[c].panning);
                if inst & 0xff00 == FX_PAN_LEFT {
                    p -= i32::from(inst & 0xff);
                } else {
                    p += i32::from(inst & 0xff);
                }
                self.ch[c].set_panning(p.clamp(0, PAN_RIGHT));
            }
            FX_EXT => match inst & 0xfff0 {
                FX_EXT_NOTE_CUT => {
                    if i32::from(inst & 0xf) <= tick {
                        self.ch[c].adsr.volume = 0;
                        self.tracks[c].volume = 0;
                    }
                }
                FX_EXT_RETRIGGER => {
                    let r = i32::from(inst & 0xf);
                    if r > 0 && tick % r == 0 {
                        if let Some(ins) = self.mus[c].instrument {
                            let (pt, pc) = (self.tracks[c].volume, self.ch[c].adsr.volume);
                            let n = self.mus[c].last_note;
                            self.trigger(c, ins, n, -1);
                            self.tracks[c].volume = pt;
                            self.ch[c].adsr.volume = pc;
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if tick != 0 {
            return;
        }
        // Commands that run only on the row's first tick.
        if inst & 0xff00 == FX_EXT {
            match inst & 0xfff0 {
                FX_EXT_FADE_VOLUME_DN => {
                    let t = &mut self.tracks[c];
                    t.volume = t.volume.wrapping_sub((inst & 0xf) as u8);
                    if i32::from(t.volume) > MAX_VOLUME {
                        t.volume = 0;
                    }
                    let v = i32::from(t.volume);
                    self.update_volumes(c, v);
                }
                FX_EXT_FADE_VOLUME_UP => {
                    let t = &mut self.tracks[c];
                    t.volume = t.volume.wrapping_add((inst & 0xf) as u8);
                    if i32::from(t.volume) > MAX_VOLUME {
                        t.volume = MAX_VOLUME as u8;
                    }
                    let v = i32::from(t.volume);
                    self.update_volumes(c, v);
                }
                FX_EXT_PORTA_UP => {
                    let prev = self.mus[c].note;
                    self.mus[c].note = prev.wrapping_add(inst & 0xf);
                    if prev > self.mus[c].note {
                        self.mus[c].note = 0xffff;
                    }
                    self.mus[c].target_note = self.mus[c].note;
                }
                FX_EXT_PORTA_DN => {
                    let prev = self.mus[c].note;
                    self.mus[c].note = prev.wrapping_sub(inst & 0xf);
                    if prev < self.mus[c].note {
                        self.mus[c].note = 0;
                    }
                    self.mus[c].target_note = self.mus[c].note;
                }
                _ => {}
            }
            return;
        }
        match inst & 0xf000 {
            FX_CUTOFF_FINE_SET => {
                let t = &mut self.tracks[c];
                t.filter_cutoff = (inst & 0xfff).min(0x7ff);
                let (fc, fr) = (t.filter_cutoff, t.filter_resonance);
                self.ch[c].set_filter(fc, fr);
            }
            FX_WAVETABLE_OFFSET => self.ch[c].set_wavetable_offset(inst & 0xfff, waves),
            _ => {}
        }
        match inst & 0x7f00 {
            FX_SET_GLOBAL_VOLUME => {
                self.play_volume = (inst & 0xff).min(MAX_VOLUME as u16) as u8;
                self.update_all_volumes();
            }
            FX_FADE_GLOBAL_VOLUME => {
                self.play_volume = self.play_volume.wrapping_sub((inst & 0xf) as u8);
                if i32::from(self.play_volume) > MAX_VOLUME {
                    self.play_volume = 0;
                }
                self.play_volume = self.play_volume.wrapping_add(((inst & 0xf0) >> 4) as u8);
                if i32::from(self.play_volume) > MAX_VOLUME {
                    self.play_volume = MAX_VOLUME as u8;
                }
                self.update_all_volumes();
            }
            FX_SET_CHANNEL_VOLUME => {
                self.mus[c].volume = (inst & 0xff).min(MAX_VOLUME as u16) as u8;
                let v = i32::from(self.tracks[c].volume);
                self.update_volumes(c, v);
            }
            FX_PW_SET => self.tracks[c].pw = (inst & 0xff) << 4,
            FX_BUZZ_SHAPE => self.ch[c].set_env_shape((inst & 3) as u8),
            FX_BUZZ_SET_SEMI => {
                self.mus[c].buzz_offset = ((i32::from(inst & 0xff) - 0x80) << 8) as i16;
                let n = self.mus[c].note;
                self.set_buzz_frequency(c, n);
            }
            FX_BUZZ_SET => {
                let m = &mut self.mus[c];
                m.buzz_offset = ((m.buzz_offset as u16 & 0xff00) | (inst & 0xff)) as i16;
                let n = m.note;
                self.set_buzz_frequency(c, n);
            }
            FX_FM_SET_MODULATION => self.ch[c].fm.adsr.volume = (inst % MAX_VOLUME as u16) as u8,
            FX_FM_SET_HARMONIC => self.ch[c].fm.harmonic = (inst % 256) as u8,
            FX_FM_SET_FEEDBACK => self.ch[c].fm.feedback = (inst % 8) as u8,
            FX_FM_SET_WAVEFORM => {
                if usize::from(inst & 255) < WAVE_ENTRIES {
                    self.ch[c].fm.wave_entry = Some(usize::from(inst & 255));
                }
            }
            FX_SET_PANNING => self.ch[c].set_panning(i32::from(inst & 0xff)),
            FX_FILTER_TYPE => self.ch[c].flttype = ((inst & 0xf) % 3) as u8,
            FX_CUTOFF_SET => {
                let t = &mut self.tracks[c];
                t.filter_cutoff = ((inst & 0xff) << 3).min(0x7ff);
                let (fc, fr) = (t.filter_cutoff, t.filter_resonance);
                self.ch[c].set_filter(fc, fr);
            }
            FX_CUTOFF_SET_COMBINED => {
                let v = inst & 0xff;
                let t = &mut self.tracks[c];
                if v < 0x80 {
                    t.filter_cutoff = v << 4;
                    self.ch[c].flttype = FLT_LP;
                } else {
                    t.filter_cutoff = (v - 0x80) << 4;
                    self.ch[c].flttype = FLT_HP;
                }
                let (fc, fr) = (t.filter_cutoff, t.filter_resonance);
                self.ch[c].set_filter(fc, fr);
            }
            FX_RESONANCE_SET => {
                self.tracks[c].filter_resonance = (inst & 3) as u8;
                let fc = self.tracks[c].filter_cutoff;
                self.ch[c].set_filter(fc, (inst & 3) as u8);
            }
            FX_SET_SPEED => {
                if from_program {
                    self.mus[c].prog_period = (inst & 0xff) as u8;
                } else {
                    self.speed = (inst & 0xf) as u8;
                    self.speed2 = if inst & 0xf0 == 0 { self.speed } else { ((inst >> 4) & 0xf) as u8 };
                }
            }
            FX_SET_RATE => {
                self.rate = ((inst & 0xff) as u8).max(1);
                self.period = RATE / u32::from(self.rate);
                self.counter %= self.period;
            }
            FX_PORTA_UP_SEMI => {
                let prev = self.mus[c].note;
                self.mus[c].note = prev.wrapping_add((inst & 0xff) << 8);
                if prev > self.mus[c].note || i32::from(self.mus[c].note) >= FREQ_TAB_SIZE << 8 {
                    self.mus[c].note = ((FREQ_TAB_SIZE - 1) << 8) as u16;
                }
                self.mus[c].target_note = self.mus[c].note;
            }
            FX_PORTA_DN_SEMI => {
                let prev = self.mus[c].note;
                self.mus[c].note = prev.wrapping_sub((inst & 0xff) << 8);
                if prev < self.mus[c].note {
                    self.mus[c].note = 0;
                }
                self.mus[c].target_note = self.mus[c].note;
            }
            FX_ARPEGGIO_ABS => {
                self.mus[c].arpeggio_note = 0;
                self.mus[c].fixed_note = (inst & 0xff) << 8;
            }
            FX_ARPEGGIO => {
                let m = &mut self.mus[c];
                if m.fixed_note != 0xffff {
                    m.note = m.last_note;
                    m.fixed_note = 0xffff;
                }
                let t = &self.tracks[c];
                m.arpeggio_note = match inst & 0xff {
                    0xf0 => t.extarp1,
                    0xf1 => t.extarp2,
                    v => v as u8,
                };
            }
            FX_SET_VOLUME => {
                let v = i32::from(inst & 0xff).min(MAX_VOLUME);
                self.tracks[c].volume = v as u8;
                self.update_volumes(c, v);
            }
            FX_SET_SYNCSRC => {
                if inst & 0xff != 0xff {
                    self.ch[c].sync_source = ((inst & 0xff) % BUSES as u16) as u8;
                    self.ch[c].flags |= SYNC;
                } else {
                    self.ch[c].flags &= !SYNC;
                }
            }
            FX_SET_RINGSRC => {
                if inst & 0xff != 0xff {
                    self.ch[c].ring_mod = ((inst & 0xff) % BUSES as u16) as u8;
                    self.ch[c].flags |= RING_MOD;
                } else {
                    self.ch[c].flags &= !RING_MOD;
                }
            }
            FX_SET_FXBUS => self.ch[c].fx_bus = usize::from(inst & 0xff) % BUSES,
            FX_SET_DOWNSAMPLE => {
                let b = self.ch[c].fx_bus;
                self.buses[b].crush.downsample = i32::from(inst & 0xff);
            }
            FX_SET_WAVETABLE_ITEM => {
                if usize::from(inst & 255) < WAVE_ENTRIES {
                    self.ch[c].wave_entry = Some(usize::from(inst & 255));
                }
            }
            FX_SET_WAVEFORM => {
                let mut f = 0;
                for (bit, flag) in [(FXW_NOISE, NOISE), (FXW_PULSE, PULSE), (FXW_TRIANGLE, TRIANGLE), (FXW_SAW, SAW), (FXW_WAVE, WAVE), (FXW_LFSR, LFSR)] {
                    if inst & bit != 0 {
                        f |= flag;
                    }
                }
                self.ch[c].set_waveform(f);
            }
            FX_RESTART_PROGRAM if !from_program => {
                let m = &mut self.mus[c];
                m.program_counter = 0;
                m.program_tick = 0;
                m.program_loop = 1;
            }
            _ => {}
        }
    }

    fn exec_track_command(&mut self, c: usize, first_tick: bool) {
        let Some(p) = self.tracks[c].pattern else { return };
        let st = self.data().patterns[p].steps[usize::from(self.tracks[c].pattern_step)];
        let (inst, vol) = (st.command, st.volume);
        let counter = i32::from(self.song_counter);
        match vol & 0xf0 {
            0xb0 => self.do_command(c, counter, FX_PAN_LEFT | u16::from(vol & 0xf), false),
            0xc0 => self.do_command(c, counter, FX_PAN_RIGHT | u16::from(vol & 0xf), false),
            0xa0 => {
                let v = u16::from(vol & 0xf);
                let pan = if v <= 8 { v * PAN_CENTER as u16 / 8 } else { (v - 8) * (PAN_RIGHT - PAN_CENTER) as u16 / 8 + PAN_CENTER as u16 };
                self.do_command(c, counter, FX_SET_PANNING | pan, false);
            }
            0xe0 => self.do_command(c, counter, FX_FADE_VOLUME | (u16::from(vol & 0xf) << 4), false),
            0xd0 => self.do_command(c, counter, FX_FADE_VOLUME | u16::from(vol & 0xf), false),
            _ => {
                if i32::from(vol) <= MAX_VOLUME {
                    self.do_command(c, if first_tick { 0 } else { counter }, FX_SET_VOLUME | u16::from(vol), false);
                }
            }
        }
        match inst & 0xff00 {
            FX_ARPEGGIO | FX_SET_EXT_ARP => {
                if inst & 0xff00 == FX_SET_EXT_ARP || inst & 0xff != 0 {
                    self.tracks[c].extarp1 = ((inst & 0xf0) >> 4) as u8;
                    self.tracks[c].extarp2 = (inst & 0xf) as u8;
                }
            }
            _ => self.do_command(c, counter, inst, false),
        }
    }

    fn exec_prog_tick(&mut self, c: usize, advance: bool) {
        let Some(ins) = self.mus[c].instrument else { return };
        let song = self.song.clone();
        let prog = &song.0.instruments[ins].program;
        let mut tick = i32::from(self.mus[c].program_tick);
        let mut visited = [false; PROG_LEN];
        // klystron would loop forever on a program of nothing but chained steps.
        for _ in 0..PROG_LEN * 64 {
            let inst = prog[tick as usize];
            if inst == FX_END {
                self.mus[c].flags &= !CHN_PROGRAM_RUNNING;
                return;
            }
            let mut dont_reloop = false;
            if inst != FX_NOP {
                match inst & 0xff00 {
                    FX_JUMP => {
                        if !visited[tick as usize] {
                            visited[tick as usize] = true;
                            tick = i32::from(inst) & (PROG_LEN as i32 - 1);
                        } else {
                            return;
                        }
                    }
                    FX_LABEL => {}
                    FX_LOOP => {
                        let m = &mut self.mus[c];
                        if m.program_loop == (inst & 0xff) as u8 {
                            if advance {
                                m.program_loop = 1;
                            }
                        } else {
                            if advance {
                                m.program_loop = m.program_loop.wrapping_add(1);
                            }
                            let mut l = 0;
                            while prog[tick as usize] & 0xff00 != FX_LABEL && tick > 0 {
                                tick -= 1;
                                if prog[tick as usize] & 0x8000 == 0 {
                                    l += 1;
                                }
                            }
                            tick -= 1;
                            dont_reloop = l <= 1;
                        }
                    }
                    _ => {
                        let pc = i32::from(self.mus[c].program_counter);
                        self.do_command(c, pc, inst, true);
                    }
                }
            }
            if inst == FX_NOP || inst & 0xff00 != FX_JUMP {
                tick += 1;
                if tick >= PROG_LEN as i32 {
                    tick = 0;
                }
            }
            if inst & 0x8000 != 0 && inst != FX_NOP && !dont_reloop {
                continue;
            }
            break;
        }
        if advance {
            self.mus[c].program_tick = tick as u8;
        }
    }

    fn do_pwm(&mut self, c: usize) {
        let Some(ins) = self.instrument(c) else { return };
        let (speed, shape, depth) = (ins.pwm_speed, ins.pwm_shape, i32::from(ins.pwm_depth));
        let t = &mut self.tracks[c];
        t.pwm_position = t.pwm_position.wrapping_add(u16::from(speed));
        self.ch[c].pw = (i32::from(t.pw) + i32::from(mus_shape(t.pwm_position >> 1, shape)) * depth / 32) as u16;
    }

    fn trigger(&mut self, c: usize, ins_index: usize, note: u16, panning: i32) {
        let song = self.song.clone();
        let ins = &song.0.instruments[ins_index];
        let waves = &song.0.waves;
        let nch = self.ch.len();
        let m = &mut self.mus[c];
        m.flags = CHN_PLAYING;
        if ins.prog_period > 0 {
            m.flags |= CHN_PROGRAM_RUNNING;
        }
        m.prog_period = ins.prog_period;
        m.instrument = Some(ins_index);
        if ins.flags & INST_NO_PROG_RESTART == 0 {
            m.program_counter = 0;
            m.program_tick = 0;
            m.program_loop = 1;
        }
        m.arpeggio_note = 0;
        m.fixed_note = 0xffff;
        let ch = &mut self.ch[c];
        ch.flags = ins.cydflags;
        ch.fx_bus = usize::from(ins.fx_bus) % BUSES;
        if ins.flags & INST_DRUM != 0 {
            ch.set_waveform(NOISE);
        }
        let note = if ins.flags & INST_LOCK_NOTE != 0 { u16::from(ins.base_note) << 8 } else { note.wrapping_add(((i32::from(ins.base_note) - MIDDLE_C) as u16).wrapping_shl(8)) };
        let divider = if ins.flags & INST_QUARTER_FREQ != 0 { 4 } else { 1 };
        let tuned = (i32::from(note) + i32::from(ins.finetune)) as u16;
        self.set_note(c, tuned, true, divider);
        let m = &mut self.mus[c];
        m.last_note = tuned;
        m.target_note = tuned;
        m.current_tick = 0;
        let t = &mut self.tracks[c];
        t.vibrato_position = 0;
        t.vib_delay = ins.vib_delay;
        t.slide_speed = 0;
        self.update_volumes(c, if ins.flags & INST_RELATIVE_VOLUME != 0 { MAX_VOLUME } else { i32::from(ins.volume) });
        let ch = &mut self.ch[c];
        ch.sync_source = if ins.sync_source == 0xff { c as u8 } else { ins.sync_source };
        ch.ring_mod = if ins.ring_mod == 0xff { c as u8 } else { ins.ring_mod };
        // klystron clamps these to its 32 channels.
        ch.ring_mod = ch.ring_mod.min(MAX_CHANNELS as u8 - 1);
        ch.sync_source = ch.sync_source.min(MAX_CHANNELS as u8 - 1);
        ch.flttype = ins.flttype;
        ch.lfsr_type = u32::from(ins.lfsr_type);
        if ins.cydflags & KEY_SYNC != 0 {
            self.tracks[c].pwm_position = 0;
        }
        if ins.flags & INST_SET_CUTOFF != 0 {
            self.tracks[c].filter_cutoff = ins.cutoff;
            self.tracks[c].filter_resonance = ins.resonance;
            self.ch[c].set_filter(ins.cutoff, ins.resonance);
        }
        if ins.flags & INST_SET_PW != 0 {
            self.tracks[c].pw = ins.pw;
            self.do_pwm(c);
        }
        let ch = &mut self.ch[c];
        if ins.flags & INST_YM_BUZZ != 0 {
            ch.flags |= YM_ENV;
            ch.set_env_shape(ins.ym_env_shape);
            self.mus[c].buzz_offset = ins.buzz_offset;
        } else {
            ch.flags &= !YM_ENV;
            ch.adsr.a = ins.adsr[0];
            ch.adsr.d = ins.adsr[1];
            ch.adsr.s = ins.adsr[2];
            ch.adsr.r = ins.adsr[3];
        }
        let ch = &mut self.ch[c];
        ch.set_wave_entry(if ins.cydflags & WAVE != 0 { Self::wave_index(ins.wavetable_entry) } else { None });
        ch.fm.set_wave_entry(if ins.fm_flags & WAVE != 0 { Self::wave_index(ins.fm_wave) } else { None });
        if panning != -1 {
            ch.set_panning(panning);
        }
        let fm = &mut ch.fm;
        fm.flags = ins.fm_flags;
        fm.harmonic = ins.fm_harmonic;
        fm.adsr.a = ins.fm_adsr[0];
        fm.adsr.d = ins.fm_adsr[1];
        fm.adsr.s = ins.fm_adsr[2];
        fm.adsr.r = ins.fm_adsr[3];
        fm.adsr.volume = ins.fm_modulation;
        fm.feedback = ins.fm_feedback;
        fm.attack_start = ins.fm_attack_start;
        ch.enable_gate(true);
        let _ = (waves, nch);
    }

    fn advance_channel(&mut self, c: usize) {
        if self.ch[c].flags & GATE == 0 {
            self.mus[c].flags &= !CHN_PLAYING;
            return;
        }
        let Some(ins_index) = self.mus[c].instrument else { return };
        let song = self.song.clone();
        let ins = &song.0.instruments[ins_index];
        if ins.flags & INST_DRUM != 0 && self.mus[c].current_tick == 1 {
            self.ch[c].set_waveform(ins.cydflags);
        }
        let ss = self.tracks[c].slide_speed;
        let m = &mut self.mus[c];
        if ss != 0 {
            if m.target_note > m.note {
                m.note += ss.min(m.target_note - m.note);
            } else if m.target_note < m.note {
                m.note -= ss.min(m.note - m.target_note);
            }
        }
        m.current_tick = m.current_tick.wrapping_add(1);
        if m.flags & CHN_PROGRAM_RUNNING != 0 {
            let u = u32::from(m.program_counter) + 1 >= u32::from(m.prog_period);
            self.exec_prog_tick(c, u);
            let m = &mut self.mus[c];
            m.program_counter = m.program_counter.wrapping_add(1);
            if u {
                m.program_counter = 0;
            }
        }
        let mut ctrl = 0u8;
        let t = self.tracks[c];
        let mut vibdep = (i32::from(ins.vibrato_depth) - i32::from(t.vib_delay)).max(0);
        let mut vibspd = i32::from(ins.vibrato_speed);
        if let Some(p) = t.pattern {
            let st = song.0.patterns[p].steps[usize::from(t.pattern_step)];
            ctrl = st.ctrl;
            if st.command & 0xff00 == FX_VIBRATO {
                ctrl |= CTRL_VIB;
                if st.command & 0xff != 0 {
                    vibdep = i32::from(st.command & 0xf) << 2;
                    vibspd = i32::from(st.command & 0xf0) >> 2;
                    if vibspd == 0 {
                        vibspd = i32::from(ins.vibrato_speed);
                    }
                    if vibdep == 0 {
                        vibdep = i32::from(ins.vibrato_depth);
                    }
                }
            }
        }
        let mut vib = 0i32;
        let on = ctrl & CTRL_VIB != 0;
        if on != (ins.flags & INST_INVERT_VIBRATO_BIT != 0) {
            let t = &mut self.tracks[c];
            t.vibrato_position = t.vibrato_position.wrapping_add(vibspd as u16);
            vib = i32::from((i32::from(mus_shape(t.vibrato_position >> 1, ins.vib_shape)) * vibdep / 64) as i16);
            if t.vib_delay != 0 {
                t.vib_delay -= 1;
            }
        }
        self.do_pwm(c);
        let m = &self.mus[c];
        let mut note = i32::from(if m.fixed_note != 0xffff { m.fixed_note } else { m.note }) + vib + (i32::from(m.arpeggio_note) << 8);
        if note < 0 {
            note = 0;
        }
        if note > FREQ_TAB_SIZE << 8 {
            note = (FREQ_TAB_SIZE - 1) << 8;
        }
        let divider = if ins.flags & INST_QUARTER_FREQ != 0 { 4 } else { 1 };
        self.set_note(c, note as u16, false, divider);
    }

    /// One tick of the song (klystron's `mus_advance_tick`).
    fn advance_tick(&mut self) {
        let song = self.song.clone();
        let d = &song.0;
        let nch = self.ch.len();
        for i in 0..nch {
            if self.song_counter == 0 {
                let seq = &d.sequences[i];
                let t = &mut self.tracks[i];
                while usize::from(t.sequence_position) < seq.len() && seq[usize::from(t.sequence_position)].position <= self.song_position {
                    let s = seq[usize::from(t.sequence_position)];
                    t.pattern_step = self.song_position.wrapping_sub(s.position);
                    t.pattern = d.patterns.get(usize::from(s.pattern)).filter(|p| usize::from(t.pattern_step) < p.steps.len()).map(|_| usize::from(s.pattern));
                    t.note_offset = s.note_offset;
                    t.sequence_position += 1;
                }
            }
            let step = self.tracks[i].pattern.map(|p| d.patterns[p].steps[usize::from(self.tracks[i].pattern_step)]);
            let delay = match step {
                Some(s) if s.command & 0x7ff0 == FX_EXT_NOTE_DELAY => (s.command & 0xf) as u8,
                _ => 0,
            };
            if self.song_counter == delay {
                if let Some(st) = step {
                    let note = if st.note < 0xf0 { (i32::from(st.note) + i32::from(self.tracks[i].note_offset)) as u8 } else { st.note };
                    let inst = st.instrument;
                    let pinst = if inst == NO_INSTRUMENT {
                        self.mus[i].instrument
                    } else if usize::from(inst) < d.instruments.len() {
                        self.mus[i].instrument = Some(usize::from(inst));
                        Some(usize::from(inst))
                    } else {
                        None
                    };
                    if note == NOTE_RELEASE {
                        self.ch[i].enable_gate(false);
                    } else if let (Some(pi), true) = (pinst, note != NOTE_NONE) {
                        let ins = &d.instruments[pi];
                        self.tracks[i].slide_speed = 0;
                        let mut speed = u16::from(ins.slide_speed | 1);
                        let mut ctrl = st.ctrl;
                        if st.command & 0xff00 == FX_SLIDE {
                            ctrl |= CTRL_SLIDE | CTRL_LEGATO;
                            speed = st.command & 0xff;
                        }
                        let target = (((i32::from(note) + i32::from(ins.base_note) - MIDDLE_C) << 8) + i32::from(ins.finetune)) as u16;
                        let divider = if ins.flags & INST_QUARTER_FREQ != 0 { 4 } else { 1 };
                        if ctrl & CTRL_SLIDE != 0 {
                            if ctrl & CTRL_LEGATO != 0 {
                                self.mus[i].target_note = target;
                            } else {
                                let old = self.mus[i].note;
                                self.trigger(i, pi, u16::from(note) << 8, -1);
                                self.mus[i].note = old;
                            }
                            self.tracks[i].slide_speed = speed;
                        } else if ctrl & CTRL_LEGATO != 0 {
                            self.set_note(i, target, true, divider);
                            self.mus[i].target_note = target;
                        } else {
                            let (pt, pc) = (self.tracks[i].volume, self.ch[i].adsr.volume);
                            self.trigger(i, pi, u16::from(note) << 8, -1);
                            self.mus[i].target_note = target;
                            if inst == NO_INSTRUMENT {
                                self.tracks[i].volume = pt;
                                self.ch[i].adsr.volume = pc;
                            }
                        }
                        if inst != NO_INSTRUMENT {
                            let (mv, pv, cv) = (i32::from(self.volume), i32::from(self.play_volume), i32::from(self.mus[i].volume));
                            self.tracks[i].volume = if ins.flags & INST_RELATIVE_VOLUME != 0 { MAX_VOLUME as u8 } else { ins.volume };
                            self.ch[i].adsr.volume = (i32::from(ins.volume) * mv / MAX_VOLUME * pv / MAX_VOLUME * cv / MAX_VOLUME) as u8;
                        }
                    }
                }
            }
            if self.tracks[i].pattern.is_some() {
                let first = self.song_counter == delay;
                self.exec_track_command(i, first);
            }
        }
        self.song_counter = self.song_counter.wrapping_add(1);
        let speed = if self.song_position & 1 == 0 { self.speed } else { self.speed2 };
        if self.song_counter >= speed {
            for i in 0..nch {
                let t = &mut self.tracks[i];
                let Some(p) = t.pattern else { continue };
                let steps = d.patterns[p].steps.len();
                let command = d.patterns[p].steps[usize::from(t.pattern_step)].command;
                if command & 0xff00 == FX_LOOP_PATTERN {
                    t.pattern_step = command & 0xff;
                } else if command & 0xff00 == FX_SKIP_PATTERN {
                    self.song_position = self.song_position.wrapping_add((steps as i32 - i32::from(t.pattern_step) - 1).max(0) as u16);
                    t.pattern = None;
                    t.pattern_step = 0;
                } else {
                    t.pattern_step = t.pattern_step.wrapping_add(1);
                }
                if t.pattern.is_some() && usize::from(t.pattern_step) >= steps {
                    t.pattern = None;
                    t.pattern_step = 0;
                }
            }
            self.song_counter = 0;
            self.song_position = self.song_position.wrapping_add(1);
            if self.song_position >= d.song_length {
                self.song_position = d.loop_point;
                self.looped = true;
                for t in self.tracks.iter_mut() {
                    t.pattern = None;
                    t.pattern_step = 0;
                    t.sequence_position = 0;
                }
            }
        }
        for i in 0..nch {
            if self.mus[i].flags & CHN_PLAYING != 0 {
                self.advance_channel(i);
            }
        }
        if d.flags & SONG_MULTIPLEX != 0 && d.multiplex_period > 0 {
            for i in 0..nch {
                if usize::from(self.multiplex_ctr / d.multiplex_period) == i {
                    let v = i32::from(self.tracks[i].volume);
                    self.update_volumes(i, v);
                } else {
                    self.ch[i].adsr.volume = 0;
                }
            }
            self.multiplex_ctr = self.multiplex_ctr.wrapping_add(1);
            if usize::from(self.multiplex_ctr) >= nch * usize::from(d.multiplex_period) {
                self.multiplex_ctr = 0;
            }
        }
    }
}

fn mus_shape(position: u16, shape: u8) -> i8 {
    let p = usize::from(position) % 128;
    match shape {
        SHAPE_SINE => tables().sine[p],
        SHAPE_SQUARE => {
            if p & 64 != 0 {
                -128
            } else {
                127
            }
        }
        SHAPE_RAMP_UP => (p as i32 * 2 - 128) as i8,
        SHAPE_RAMP_DN => (127 - p as i32 * 2) as i8,
        _ => tables().random[usize::from(position / 8) % 128],
    }
}

// ---------------------------------------------------------------------
// Playing.

/// A channel's own share of a bus: its copy of the bus's chorus and reverb.
struct Share {
    bus: Bus,
}

/// Plays a Klystrack song: klystron at 44.1 kHz, each tick's samples
/// made as the tick's rendered (or only its envelopes run, when it's
/// passed over), resampled to the host's rate.
pub struct Player {
    e: Engine,
    /// Samples in the tick now.
    n: usize,
    /// Whether the tick's samples are still to be made.
    fresh: bool,
    left: Vec<Vec<f32>>,
    right: Vec<Vec<f32>>,
    prev: Vec<(f32, f32)>,
    /// Each tick's mix, as klystron's output before its final quarter.
    mix: Vec<(i32, i32)>,
    /// Each (channel, bus) the channel's share of the bus.
    shares: Vec<Option<Box<Share>>>,
    /// The channel that last sent to each bus.
    last: [usize; BUSES],
    pos: f64,
    done: bool,
}

impl Player {
    pub fn new(song: Song) -> Player {
        let n = song.channels();
        Player {
            e: Engine::new(song),
            n: 0,
            fresh: false,
            left: vec![Vec::new(); n],
            right: vec![Vec::new(); n],
            prev: vec![(0.0, 0.0); n],
            mix: Vec::new(),
            shares: (0..n * BUSES).map(|_| None).collect(),
            last: [0; BUSES],
            pos: 0.0,
            done: false,
        }
    }

    /// The samples of the tick: klystron's `cyd_output` and `cyd_cycle` for each.
    fn make(&mut self) {
        let n = self.n;
        let nch = self.e.ch.len();
        let song = self.e.song.clone();
        let waves = &song.0.waves;
        for c in 0..nch {
            self.left[c].clear();
            self.left[c].resize(n, 0.0);
            self.right[c].clear();
            self.right[c].resize(n, 0.0);
        }
        self.mix.clear();
        let mut s = [0i32; MAX_CHANNELS];
        let mut send = [(0i32, 0i32, usize::MAX); MAX_CHANNELS];
        for k in 0..n {
            let ch = &mut self.e.ch;
            for c in 0..nch {
                s[c] = ch[c].output();
                if ch[c].flags & WAVE != 0 && ch[c].flags & WAVE_OVERRIDE_ENV == 0 {
                    s[c] = s[c].wrapping_add(ch[c].wave_output(waves, None));
                }
            }
            let (mut ml, mut mr) = (0i32, 0i32);
            for c in 0..nch {
                send[c] = (0, 0, usize::MAX);
                let chn = &mut ch[c];
                if chn.flags & GATE == 0 {
                    continue;
                }
                let input = if chn.flags & RING_MOD != 0 {
                    let rm = s.get(usize::from(chn.ring_mod)).copied().filter(|_| usize::from(chn.ring_mod) < nch).unwrap_or(0);
                    s[c].wrapping_mul(rm + WAVE_AMP / 2) / WAVE_AMP
                } else {
                    s[c]
                };
                let mut o = env_output(chn.flags, &chn.adsr, input);
                if chn.flags & WAVE != 0 && chn.flags & WAVE_OVERRIDE_ENV != 0 {
                    o = o.wrapping_add(chn.wave_output(waves, Some(i32::from(chn.adsr.volume))));
                }
                if chn.flags & FILTER != 0 {
                    chn.flt.cycle(o);
                    o = match chn.flttype {
                        FLT_BP => 3i32.wrapping_mul(chn.flt.b3.wrapping_sub(chn.flt.b4)),
                        FLT_HP => chn.flt.b0.wrapping_sub(chn.flt.b4),
                        _ => chn.flt.b4,
                    };
                }
                let ol = o.wrapping_mul(chn.gain_left) / 2048;
                let or = o.wrapping_mul(chn.gain_right) / 2048;
                if chn.flags & FX != 0 {
                    send[c] = (ol, or, chn.fx_bus);
                } else {
                    ml = ml.wrapping_add(ol);
                    mr = mr.wrapping_add(or);
                    self.left[c][k] += ol as f32 * SCALE;
                    self.right[c][k] += or as f32 * SCALE;
                }
            }
            for b in 0..BUSES {
                let (mut sl, mut sr) = (0i32, 0i32);
                for (c, &(l, r, bus)) in send.iter().enumerate().take(nch) {
                    if bus == b {
                        sl = sl.wrapping_add(l);
                        sr = sr.wrapping_add(r);
                        self.last[b] = c;
                    }
                }
                let bus = &mut self.e.buses[b];
                let (yl, yr) = bus.output(sl, sr);
                ml = ml.wrapping_add(yl);
                mr = mr.wrapping_add(yr);
                if bus.flags == 0 && sl == 0 && sr == 0 && yl == 0 && yr == 0 {
                    continue;
                }
                // Each channel's share: its own copy's chorus and reverb
                // (or what it sent, with neither), and of the rest, a part
                // as loud as its share.
                let linear = bus.flags & (FX_CHORUS | FX_REVERB) != 0;
                let mut parts = [(0i32, 0i32); MAX_CHANNELS];
                let (mut pl, mut pr, mut weight) = (0i64, 0i64, 0i64);
                for c in 0..nch {
                    let (l, r) = if send[c].2 == b { (send[c].0, send[c].1) } else { (0, 0) };
                    let part = if linear {
                        let slot = &mut self.shares[c * BUSES + b];
                        if slot.is_none() && (l != 0 || r != 0) {
                            *slot = Some(Box::new(Share { bus: self.e.buses[b].share() }));
                        }
                        match slot {
                            Some(sh) => sh.bus.linear(l, r),
                            None => (0, 0),
                        }
                    } else {
                        (l, r)
                    };
                    parts[c] = part;
                    pl += i64::from(part.0);
                    pr += i64::from(part.1);
                    weight += i64::from(part.0).abs() + i64::from(part.1).abs();
                }
                let (rl, rr) = ((i64::from(yl) - pl) as f32, (i64::from(yr) - pr) as f32);
                for (c, &(l, r)) in parts.iter().enumerate().take(nch) {
                    let w = if weight > 0 {
                        (i64::from(l).abs() + i64::from(r).abs()) as f32 / weight as f32
                    } else if c == self.last[b] {
                        1.0
                    } else {
                        0.0
                    };
                    self.left[c][k] += (l as f32 + rl * w) * SCALE;
                    self.right[c][k] += (r as f32 + rr * w) * SCALE;
                }
            }
            self.mix.push((ml, mr));
            let ch = &mut self.e.ch;
            for c in ch.iter_mut() {
                c.cycle(waves);
            }
            for c in 0..nch {
                if ch[c].flags & SYNC != 0 {
                    let src = usize::from(ch[c].sync_source);
                    if src < nch && ch[src].sync_bit != 0 {
                        ch[c].sync();
                    }
                }
            }
        }
    }

    /// The tick's envelopes alone, for a tick no one hears.
    fn pass(&mut self) {
        for _ in 0..self.n {
            for c in self.e.ch.iter_mut() {
                c.cycle_envelopes();
            }
        }
    }

    #[cfg(test)]
    fn make_mix(&mut self) -> &[(i32, i32)] {
        if self.fresh {
            self.make();
            self.fresh = false;
        }
        &self.mix
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.e.ch.len()
    }

    fn tick(&mut self) {
        if self.fresh {
            self.pass();
            for p in self.prev.iter_mut() {
                *p = (0.0, 0.0);
            }
        } else if self.n > 0 {
            for c in 0..self.prev.len() {
                self.prev[c] = (self.left[c][self.n - 1], self.right[c][self.n - 1]);
            }
        }
        self.pos = (self.pos - self.n as f64).max(0.0);
        self.e.counter = self.e.period - 1;
        self.e.advance_tick();
        if std::mem::take(&mut self.e.looped) {
            self.done = true;
        }
        self.n = self.e.counter as usize + 1;
        self.fresh = true;
    }

    fn tick_seconds(&self) -> f64 {
        self.n as f64 / f64::from(RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        if self.fresh {
            self.make();
            self.fresh = false;
        }
        let step = f64::from(RATE) / f64::from(rate.max(1));
        let n = self.n;
        for c in 0..self.left.len() {
            let (bl, br, prev) = (&self.left[c], &self.right[c], self.prev[c]);
            let at = |k: isize| if k < 0 || n == 0 { prev } else { (bl[(k as usize).min(n - 1)], br[(k as usize).min(n - 1)]) };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (a, b) = (at(k as isize), at(k as isize + 1));
                l[i] = a.0 + (b.0 - a.0) * f;
                r[i] = a.1 + (b.1 - a.1) * f;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        let rows = rows_per_pattern(&self.e.song);
        let p = usize::from(self.e.song_position);
        (p / rows, p % rows)
    }

    fn speed(&self) -> (u32, u32) {
        let speed = if self.e.song_position & 1 == 0 { self.e.speed } else { self.e.speed2 };
        (u32::from(speed), u32::from(self.e.rate) * 5 / 2)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

/// How the song's rows are cut into patterns for the screen: its
/// sequence editor's step, or for a song that doesn't say, the step
/// klystrack works out (the closest any two patterns are placed, or a
/// last one to the end), 16 at most 256.
pub fn rows_per_pattern(song: &Song) -> usize {
    let d = &song.0;
    let mut step = usize::from(d.sequence_step);
    if step == 0 {
        let len = usize::from(d.song_length);
        step = 1000;
        for seq in &d.sequences {
            for s in 1..seq.len() {
                if usize::from(seq[s - 1].position) >= len {
                    break;
                }
                step = step.min(usize::from(seq[s].position).saturating_sub(usize::from(seq[s - 1].position)));
            }
            if let Some(last) = seq.last() {
                if usize::from(last.position) < len {
                    step = step.min(len - usize::from(last.position));
                }
            }
        }
        if step == 1000 {
            step = 16;
        }
    }
    step.clamp(1, 256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::Machine;

    /// A version 27 song: one channel, a saw instrument held at full
    /// sustain, and a C-4 at the start of a 16-row pattern at speed 6, 50 Hz.
    fn song() -> Vec<u8> {
        let mut d = b"cyd!song".to_vec();
        d.extend([27, 1]); // version, channels
        d.extend([4, 4, 16, 0]); // time signature, sequence step
        d.extend([1, 1, 0, 1, 0]); // instruments, patterns, sequences
        d.extend([16, 0, 0, 0, 128, 6, 6, 50]); // length, loop, master volume, speeds, rate
        d.extend([0, 0, 0, 0, 3, 0]); // flags, multiplex period, pitch inaccuracy
        d.push(4);
        d.extend(b"Test");
        d.extend([0, 128, 0]); // effects, default volume and pan
        d.extend([48, 0, 0, 0, SAW as u8, 0, 0, 0]); // flags (set PW, cutoff), saw
        d.extend([0, 0, 31, 4, 0xff, 0xff, 0x00, 0x04, 128]); // ADSR, no sync or ring, PW, volume
        d.extend([0, 1, 0, 0, 0, 0, 0x80, 48, 0]); // no program, period, vibrato, PWM, slide, base note, finetune
        d.push(3);
        d.extend(b"Saw");
        d.extend([0xff, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // cutoff, resonance, type, buzz, bus, shapes, LFSR, wave
        d.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // FM
        d.extend([0, 0, 0, 0, 0]); // the pattern at row 0
        d.extend([4, 0, 0, 0x30, 0x00, 48, 0]); // 4 steps, colour, a note and instrument on the first
        d.push(0); // no waves
        d
    }

    #[test]
    fn plays_a_saw_at_c4() {
        let s = Song::parse(&song()).expect("parses");
        assert_eq!((s.channels(), s.length(), s.title()), (1, (16, 0), "Test"));
        assert_eq!(s.instruments()[0].name, "Saw");
        let tune: Arc<dyn Tune> = Arc::new(s);
        let mut m = Machine::new(tune.clone(), 0);
        let (mut l, mut r) = (vec![0f32; 44100], vec![0f32; 44100]);
        m.render(44100, 1.0, 1.0, &mut l, &mut r);
        // A saw falls through 0.15 once a cycle: C-4 is 261.63 Hz.
        let falls = l.windows(2).filter(|w| w[0] > 0.15 && w[1] <= 0.15).count();
        assert!((260..=263).contains(&falls), "{falls} cycles");
        let peak = l.iter().fold(0f32, |a, &v| a.max(v.abs()));
        assert!((0.2..0.4).contains(&peak), "peak {peak}");
        // 16 rows of 6 ticks at 50 Hz.
        assert!((Machine::length(&tune, 0, 60.0) - 1.92).abs() < 1e-9);
    }

    /// Compares each song in `KT_DIR` with klystron's own render beside
    /// it (`<song>.raw`, stereo 16-bit, in `KT_REF`), sample for sample.
    #[test]
    #[ignore]
    fn matches_klystron() {
        let (Ok(dir), Ok(refs)) = (std::env::var("KT_DIR"), std::env::var("KT_REF")) else { return };
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "kt")).collect();
        names.sort();
        for p in names {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if std::env::var("KT_ONLY").is_ok_and(|o| name != o) {
                continue;
            }
            let Ok(r) = std::fs::read(format!("{refs}/{name}.raw")) else { continue };
            let want: Vec<i16> = r.as_chunks::<2>().0.iter().map(|&c| i16::from_le_bytes(c)).collect();
            let song = Song::parse(&std::fs::read(&p).unwrap()).expect("parses");
            let mut pl = Player::new(song);
            let mut got: Vec<i16> = Vec::with_capacity(want.len());
            while got.len() < want.len() {
                pl.tick();
                for &(l, r) in pl.make_mix() {
                    got.push((l / 4).clamp(-32768, 32767) as i16);
                    got.push((r / 4).clamp(-32768, 32767) as i16);
                }
            }
            let n = want.len();
            let bad = (0..n).filter(|&i| got[i] != want[i]).count();
            let first = (0..n).find(|&i| got[i] != want[i]);
            let maxd = (0..n).map(|i| (i32::from(got[i]) - i32::from(want[i])).abs()).max().unwrap_or(0);
            println!("{name}: {} frames, {bad} differ, first {:?}, most {maxd}", n / 2, first.map(|i| (i / 2, got[i], want[i])));
        }
    }
}
