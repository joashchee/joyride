// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! DefleMask's player: rows at two alternating speeds (times the time
//! base), instruments' macros, ProTracker's effects and each system's own,
//! played on the system's chips. Written from DefleMask's manual and
//! module specification, its behaviour measured on modules' output (as
//! Furnace plays them, run as a program and never read), not from code.
//!
//! Pitch is kept in 128ths of a semitone above note 0 (C of octave 0);
//! slides move in 32nds, as DefleMask's do.

use std::sync::Arc;

use crate::deflemask::Instrument;
use crate::deflemask::{Cell, Macro, Module, System};
use crate::gb::Gb;
use crate::huc6280::HuC6280;
use crate::nes::Apu;
use crate::opll::Opll;
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};
use crate::sid::Sid;
use crate::sn76489::Sn76489;
use crate::ymfm::{Ym2151, Ym2612};

/// A DefleMask module ready to play.
pub struct Song {
    pub module: Arc<Module>,
    file: Arc<Vec<u8>>,
}

impl Song {
    /// Reads an unpacked module (the host inflates the .dmf).
    pub fn parse(d: &[u8]) -> Option<Song> {
        let module = Module::parse(d)?;
        if !supported(module.system) {
            return None;
        }
        Some(Song {
            module: Arc::new(module),
            file: Arc::new(d.to_vec()),
        })
    }
}

/// The systems played so far.
pub fn supported(s: System) -> bool {
    matches!(
        s,
        System::Sms | System::SmsOpll | System::Nes | System::Genesis { .. } | System::GameBoy | System::PcEngine | System::C64 { .. } | System::Arcade
    )
}

/// What a Genesis channel is: an FM channel, one of FM 3's operators
/// (in its extended mode) or a PSG channel.
#[derive(Clone, Copy, PartialEq)]
enum GenCh {
    Fm(usize),
    Op(usize),
    Psg(usize),
}

fn gen_ch(ext3: bool, c: usize) -> GenCh {
    match (ext3, c) {
        (false, 0..=5) => GenCh::Fm(c),
        (false, _) => GenCh::Psg(c - 6),
        (true, 0 | 1) => GenCh::Fm(c),
        (true, 2..=5) => GenCh::Op(c - 2),
        (true, 6..=8) => GenCh::Fm(c - 3),
        (true, _) => GenCh::Psg(c - 9),
    }
}

/// A channel's name, as DefleMask shows it.
pub fn channel_name(s: System, c: usize) -> &'static str {
    match s {
        System::Sms => ["Square 1", "Square 2", "Square 3", "Noise"].get(c).copied().unwrap_or(""),
        System::SmsOpll => ["Square 1", "Square 2", "Square 3", "Noise", "FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6", "FM 7", "FM 8", "FM 9"]
            .get(c)
            .copied()
            .unwrap_or(""),
        System::Nes => ["Pulse 1", "Pulse 2", "Triangle", "Noise", "PCM"].get(c).copied().unwrap_or(""),
        System::GameBoy => ["Pulse 1", "Pulse 2", "Wave", "Noise"].get(c).copied().unwrap_or(""),
        System::Arcade => ["FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6", "FM 7", "FM 8", "PCM 1", "PCM 2", "PCM 3", "PCM 4", "PCM 5"]
            .get(c)
            .copied()
            .unwrap_or(""),
        System::C64 { .. } => ["Voice 1", "Voice 2", "Voice 3"].get(c).copied().unwrap_or(""),
        System::PcEngine => ["Wave 1", "Wave 2", "Wave 3", "Wave 4", "Wave 5", "Wave 6"].get(c).copied().unwrap_or(""),
        System::Genesis { ext3 } => match gen_ch(ext3, c) {
            GenCh::Fm(k) => ["FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6"][k],
            GenCh::Op(k) => ["FM 3 op 1", "FM 3 op 2", "FM 3 op 3", "FM 3 op 4"][k],
            GenCh::Psg(k) => ["Square 1", "Square 2", "Square 3", "Noise"][k],
        },
        _ => "",
    }
}

/// What to add to a note as the file has it (octave × 12 + note, C-0 0,
/// A-3 45) for the MIDI note it sounds as: the PSG's A-3 sounds at 880 Hz
/// (MIDI's 81), the FM's and the 2A03's at 440.
pub fn midi_offset(s: System, c: usize) -> i32 {
    match s {
        System::Genesis { ext3 } if !matches!(gen_ch(ext3, c), GenCh::Psg(_)) => 24,
        // The YM2413's A-3 at 220 Hz.
        System::SmsOpll if c >= 4 => 12,
        // The Game Boy's A-3 at 220 Hz (its wave channel's an octave lower).
        System::GameBoy if c == 2 => 0,
        System::GameBoy => 12,
        System::PcEngine => 24,
        // The YM2151's A-4 at 440 Hz.
        System::Arcade => 12,
        // The SID's A-3 at 220 Hz.
        System::C64 { .. } => 12,
        // The 2A03's A-3 at 440 Hz; the triangle an octave below.
        System::Nes if c == 2 => 12,
        System::Nes => 24,
        _ => 36,
    }
}

/// A channel's loudest volume.
pub fn max_volume(s: System, c: usize) -> i32 {
    match s {
        System::Genesis { ext3 } if !matches!(gen_ch(ext3, c), GenCh::Psg(_)) => 127,
        System::PcEngine => 31,
        System::Arcade => 127,
        _ => 15,
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "dmf"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.module.clone()))
    }

    /// Each instrument: the system, then its FM operators or its macros
    /// (and the C64's or Game Boy's settings), and the waves its wave macro
    /// plays (their values, numbered within the patch). An instrument with
    /// nothing set (no FM, no macros, the system's defaults) is left out.
    fn patches(&self) -> Vec<Patch> {
        let m = &self.module;
        let sys = match m.system {
            System::Genesis { .. } => 2,
            System::Sms => 3,
            System::SmsOpll => 0x43,
            System::GameBoy => 4,
            System::PcEngine => 5,
            System::Nes => 6,
            System::C64 { .. } => 7,
            System::Arcade => 8,
            System::NeoGeo { .. } => 9,
        };
        let mut out = Vec::new();
        for (k, ins) in m.instruments.iter().enumerate() {
            let mut b = PatchBytes::default();
            b.u8(sys);
            if let Some(fm) = &ins.fm {
                b.u8(1).u8(fm.alg).u8(fm.fb).u8(fm.lfo).u8(fm.lfo2);
                for o in &fm.ops {
                    for v in [o.am, o.ar, o.dr, o.mult, o.rr, o.sl, o.tl, o.dt2, o.rs, o.dt, o.d2r, o.ssg] {
                        b.u8(v);
                    }
                }
            } else {
                let blank = [&ins.volume, &ins.arpeggio, &ins.duty, &ins.wave].iter().all(|x| x.values.is_empty()) && ins.c64.iter().all(|&v| v == 0) && ins.gb == [0; 4];
                if blank {
                    continue;
                }
                b.u8(0);
                // The wave macro's waves, renumbered as they first appear.
                let mut waves: Vec<usize> = Vec::new();
                let wave_mac: Vec<i32> = ins
                    .wave
                    .values
                    .iter()
                    .map(|&w| {
                        let w = w.max(0) as usize;
                        if w >= m.waves.len() {
                            return w as i32 + 1000;
                        }
                        (waves.iter().position(|&x| x == w).unwrap_or_else(|| {
                            waves.push(w);
                            waves.len() - 1
                        })) as i32
                    })
                    .collect();
                let mac = |b: &mut PatchBytes, values: &[i32], l: Option<usize>| {
                    b.u16(values.len() as u16);
                    for &v in values {
                        b.i32(v);
                    }
                    b.i32(l.map_or(-1, |x| x as i32));
                };
                mac(&mut b, &ins.volume.values, ins.volume.loop_at);
                mac(&mut b, &ins.arpeggio.values, ins.arpeggio.loop_at);
                b.u8(u8::from(ins.arp_fixed));
                mac(&mut b, &ins.duty.values, ins.duty.loop_at);
                mac(&mut b, &wave_mac, ins.wave.loop_at);
                b.bytes(&ins.c64).bytes(&ins.gb);
                b.u16(waves.len() as u16);
                for w in waves {
                    let v = &m.waves[w];
                    b.u32(v.len() as u32);
                    for &x in v {
                        b.u32(x);
                    }
                }
            }
            out.push(b.patch(k + 1, &ins.name));
        }
        out
    }
}

/// The YM2413's output (a channel's full scale 2048) to the tracker's level.
const OPLL_LEVEL: f32 = 0.3 / 2048.0;

/// The SN76489's clock on the Master System and Genesis (NTSC).
const PSG_CLOCK: f64 = 3_579_545.0;

/// A macro being played: its next step.
#[derive(Clone, Copy, Default)]
struct MacroPos {
    at: usize,
    /// Whether it has values left to play (or loops).
    running: bool,
}

impl MacroPos {
    fn start(m: &Macro) -> MacroPos {
        MacroPos { at: 0, running: !m.values.is_empty() }
    }

    /// This tick's value, if the macro has one.
    fn step(&mut self, m: &Macro) -> Option<i32> {
        if !self.running {
            return None;
        }
        let v = *m.values.get(self.at)?;
        self.at += 1;
        if self.at >= m.values.len() {
            match m.loop_at {
                Some(l) => self.at = l,
                None => self.running = false,
            }
        }
        Some(v)
    }
}

#[derive(Clone, Default)]
struct Ch {
    /// The playing note (semitones from C-0), None before the first.
    note: Option<i32>,
    instrument: usize,
    /// Whether the note sounds (not cut or off).
    on: bool,
    /// The volume column (0 to the system's most), and as slides keep it (times 4).
    vol: i32,
    vol4: i32,
    /// The chip's volume now (0 to the most), and whether a column set it.
    out_vol: i32,
    vol_set: bool,
    vol_mac: MacroPos,
    arp_mac: MacroPos,
    duty_mac: MacroPos,
    wave_mac: MacroPos,
    /// The arpeggio macro's offset now (semitones) or fixed note.
    arp_now: i32,
    arp_fixed: bool,
    /// The duty macro's value now (None before it plays), and 12xx's.
    duty_now: Option<i32>,
    duty_fx: i32,
    /// Pitch slides' offset (128ths), the 1xx/2xx speed (32nds a tick, + up).
    slide: i32,
    slide_speed: i32,
    /// 3xx: the speed (32nds) and the target note's pitch.
    porta_speed: i32,
    porta_target: Option<i32>,
    /// E1xy/E2xy: speed and target, as 3xx.
    note_slide: Option<(i32, i32)>,
    /// 0xy and its tick counter.
    arp: u8,
    arp_tick: u8,
    arp_step: u8,
    /// The note started this tick, and whether it followed one still
    /// sounding (then 0xy plays the note itself that tick, and doesn't step).
    fresh: bool,
    arp_hold: bool,
    /// 4xy: speed, depth, its phase (0-63), E3 mode, E4 fine depth.
    vib_speed: i32,
    vib_depth: i32,
    vib_pos: i32,
    vib_mode: u8,
    vib_fine: i32,
    /// 7xy.
    trem_speed: i32,
    trem_depth: i32,
    trem_pos: i32,
    /// Axy: quarter-volumes a tick (+ up).
    vol_slide: i32,
    /// E5xx: 128ths of a semitone, 0 at 80.
    fine: i32,
    /// ECxx and EDxx: ticks to go, and the row a delay holds back.
    cut: Option<u8>,
    delayed: Option<(u8, Cell)>,
    /// Cxx: retriggers every so many ticks.
    retrig: u8,
    retrig_tick: u8,
    /// EAxx.
    legato: bool,
    /// SMS noise: special mode (tone 3's period) and white noise.
    noise_special: bool,
    noise_white: bool,
    /// FM: the effects' changes to the instrument (12xx-15xx TL, 16xy MULT,
    /// 19xx-1Dxx AR, 11xx FB), until another instrument plays.
    fm_tl: [Option<u8>; 4],
    fm_mult: [Option<u8>; 4],
    fm_ar: [Option<u8>; 4],
    fm_fb: Option<u8>,
    /// 8xy: left and right on (FM).
    pan: (bool, bool),
    /// 17xx: the channel plays samples (the Genesis's FM 6).
    dac: bool,
    /// Game Boy: 10xx's wave, 11xx's 7-bit noise, 13xy/14xx's sweep (NR10).
    wave_fx: Option<usize>,
    noise_narrow: bool,
    sweep: u8,
    /// PC Engine: 8xy's left and right levels (also the arcade's PCM).
    balance: Option<u8>,
    /// Arcade PCM: 20xx's rate.
    pcm_rate: Option<f64>,
    /// Game Boy noise: steps a slide has moved it (1xx and 2xx alike).
    noise_slide: i32,
    /// The wave macro's value now, and the volume macro's.
    wave_now: Option<i32>,
    vol_mac_now: Option<i32>,
    /// The YM2413: the drum (0x0E's bit) the channel last struck and the
    /// volume it gave it, and whether an effect has changed the custom
    /// voice since it was written.
    drum: u8,
    drum_att: u8,
    voice_fx: bool,
}

impl Ch {
    fn new() -> Ch {
        Ch {
            vib_fine: 15,
            noise_special: true,
            noise_white: true,
            pan: (true, true),
            arp_step: 2,
            arp_tick: u8::MAX,
            ..Default::default()
        }
    }
}

/// The chips a system plays on.
enum Chips {
    Sms(Sn76489),
    Nes(NesOut),
    Genesis(Box<GenOut>),
    GameBoy(Box<GbOut>),
    PcEngine(Box<PceOut>),
    C64(Box<C64Out>),
    Arcade(Box<ArcOut>),
    SmsOpll(Box<OpllOut>),
}

/// The Master System with its FM Sound Unit: the PSG, the YM2413 and
/// what's been written to it.
struct OpllOut {
    sn: Sn76489,
    opll: Opll,
    rate: u32,
    regs: [u8; 0x40],
    keyed: [bool; 9],
    /// The instrument each of FM 7 to 9 last struck the drum kit with.
    kit: [Option<usize>; 3],
    /// The channel each drum last played on, by the chip's drum outputs
    /// (BD, HH, SD, TOM, CYM).
    drum_ch: [usize; 5],
}

/// The drums' bits in 0x0E, by the chip's drum outputs.
const DRUM_BITS: [u8; 5] = [0x10, 0x01, 0x08, 0x04, 0x02];

impl OpllOut {
    fn write(&mut self, r: u8, v: u8) {
        self.regs[usize::from(r)] = v;
        self.opll.write(usize::from(r), v);
    }

    /// Writes a register only when its value changes.
    fn set(&mut self, r: u8, v: u8) {
        if self.regs[usize::from(r)] != v {
            self.write(r, v);
        }
    }

    /// The registers as text, as `opllframes.py` prints Furnace's.
    #[cfg(test)]
    fn state(&self) -> String {
        let r = |a: usize| self.regs[a];
        let mut t = vec![self.sn.state()];
        for k in 0..9 {
            let hi = r(0x20 + k);
            t.push(format!(
                "{}/{}/{}/{}/{}/{}",
                u16::from(r(0x10 + k)) | u16::from(hi & 1) << 8,
                (hi >> 1) & 7,
                (hi >> 4) & 1,
                (hi >> 5) & 1,
                r(0x30 + k) >> 4,
                r(0x30 + k) & 15
            ));
        }
        t.push(format!("{:02x} {:02x}{:02x}{:02x}", r(0x0E), r(0x36), r(0x37), r(0x38)));
        t.push((0..8).map(|a| format!("{:02x}", r(a))).collect());
        t.join(" ")
    }
}

/// The arcade board: its YM2151 and the sample channels (SegaPCM's, here
/// played straight: rate, volume and each side's level).
struct ArcOut {
    opm: Ym2151,
    loaded: [Option<usize>; 8],
    keyed: [bool; 8],
    pos: f64,
    pcm: [Option<DacPlay>; 5],
    pcm_level: [(f32, f32); 5],
    samples: Arc<Vec<(Vec<u8>, f64)>>,
}

/// The YM2151's clock.
const OPM_CLOCK: f64 = 3_579_545.0;

/// A YM2151 pitch's KC and KF: its note (A-4 at 440 Hz) and 64ths of a semitone above.
fn opm_kc(pitch: i32) -> (u8, u8) {
    const CODE: [u8; 12] = [0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14];
    let p = pitch.clamp(128, 96 * 128 + 127);
    let n = p.div_euclid(128) - 1;
    let kf = (p.rem_euclid(128) / 2) as u8;
    let oct = n.div_euclid(12).clamp(0, 7) as u8;
    (oct << 4 | CODE[n.rem_euclid(12) as usize], kf)
}

/// The C64's SID (PAL), the filter's settings the instruments and effects
/// share, and each voice's state.
struct C64Out {
    sid: Sid,
    rate: u32,
    owed: f64,
    cutoff: u16,
    resonance: u8,
    mode: u8,
    /// Each voice: pulse width (0-4095), waveform bits, ADSR, ring/sync,
    /// to the filter, and whether the ADSR is kept through notes (1Axx).
    pw: [u16; 3],
    wave: [u8; 3],
    ad: [u8; 3],
    sr: [u8; 3],
    ring_sync: [u8; 3],
    filt: [bool; 3],
    no_reset: [bool; 3],
    gate: [bool; 3],
    started: [bool; 3],
}

/// The PAL C64's clock.
const C64_CLOCK: f64 = 985_248.0;

/// The PC Engine's PSG, and each channel's wave loaded.
struct PceOut {
    psg: HuC6280,
    wave: [Option<usize>; 6],
    noise: [bool; 6],
    /// The samples each channel plays (17xx), and the samples (8 bits).
    dda: [Option<DacPlay>; 6],
    samples: Arc<Vec<(Vec<u8>, f64)>>,
}

/// A PC Engine pitch's period (A-3 at 440 Hz; the periods a little longer
/// than that, as DefleMask's are): straight lines between notes.
fn pce_period(pitch: i32) -> u16 {
    let at = |n: i32| (HuC6280::clock() * 1.000126 / 32.0 / (440.0 * 2f64.powf(f64::from(n - 45) / 12.0))).round().clamp(1.0, 4095.0);
    let n = pitch.div_euclid(128);
    let fr = f64::from(pitch.rem_euclid(128)) / 128.0;
    let (a, b) = (at(n), at(n + 1));
    (a + ((b - a) * fr).floor()).clamp(1.0, 4095.0) as u16
}

/// The noise's frequency for a note (by its place in the octave alone).
fn pce_noise(note: i32) -> u8 {
    const T: [u8; 12] = [0x04, 0x0D, 0x0F, 0x12, 0x15, 0x17, 0x19, 0x1B, 0x1D, 0x1F, 0x00, 0x02];
    T[note.rem_euclid(12) as usize]
}

/// The Game Boy's APU, and the wave its wave channel has loaded.
struct GbOut {
    gb: Gb,
    wave: Option<usize>,
    started: [bool; 4],
}

impl GbOut {
    /// The registers as text, as `gbframes.py` prints Furnace's (and how
    /// many times each channel's been triggered).
    #[cfg(test)]
    fn state(&self, trig: &[u32; 4]) -> String {
        let r = |a: u8| self.gb.reg(a);
        let sq = |b: u8, k: usize| format!("{}/{:02x}/{}/{}", u16::from(r(b + 3)) | (u16::from(r(b + 4) & 7) << 8), r(b + 2), r(b + 1) >> 6, trig[k]);
        format!(
            "{} {} {}/{}/{} {:02x}/{:02x}/{} {:02x}",
            sq(0, 0),
            sq(5, 1),
            u16::from(r(0x0D)) | (u16::from(r(0x0E) & 7) << 8),
            (r(0x0C) >> 5) & 3,
            trig[2],
            r(0x12),
            r(0x11),
            trig[3],
            r(0x15)
        )
    }
}

/// A Game Boy pitch's period register (A-3, note 45, at 220 Hz, the
/// periods a little longer than that, as DefleMask's are; notes below C-2
/// at 1): straight lines between notes.
fn gb_period(pitch: i32) -> u16 {
    let at = |n: i32| {
        if n < 24 {
            1.0
        } else {
            (2048.0 - 131_072.0 * 1.00019 / (440.0 * 2f64.powf(f64::from(n - 57) / 12.0))).round().clamp(1.0, 2047.0)
        }
    };
    let n = pitch.div_euclid(128);
    let fr = f64::from(pitch.rem_euclid(128)) / 128.0;
    let (a, b) = (at(n), at(n + 1));
    (a + ((b - a) * fr).floor()).clamp(1.0, 2047.0) as u16
}

/// The noise's NR43 for a note: from C-2 up, the clock shift (10 down to
/// 0) and divisor (7 down to 4 a shift, then 3 to 0) each semitone a step
/// higher.
fn gb_noise(note: i32) -> u8 {
    // (Below C-2 the shifts go on to 15, as version 17's notes reach.)
    let i = (note - 24).clamp(-23, 44);
    if i <= 40 {
        let k = i - 1;
        let shift = 9 - k.div_euclid(4);
        let r = 7 - k.rem_euclid(4);
        (shift as u8) << 4 | r as u8
    } else {
        (3 - (i - 41)) as u8
    }
}

/// The Genesis's YM2612 at its NTSC clock.
const YM2612_CLOCK: f64 = 7_670_454.0;

/// A sample the DAC plays: which, where it's up to and its step a chip sample.
struct DacPlay {
    sample: usize,
    pos: f64,
    step: f64,
}

/// The Genesis: its YM2612 and PSG, and what's been written to the FM.
struct GenOut {
    ym: Ym2612,
    sn: Sn76489,
    /// Each FM channel's instrument loaded, and keyed operators; FM 3's
    /// operators' instruments in its extended mode.
    loaded: [Option<usize>; 6],
    keyed: [u8; 6],
    ext_loaded: [Option<usize>; 4],
    dac: Option<DacPlay>,
    /// The samples as the DAC takes them (8 bits, unsigned).
    dac_samples: Arc<Vec<(Vec<u8>, f64)>>,
    /// How far into the next chip sample the output is.
    pos: f64,
}

impl GenOut {
    fn write(&mut self, hi: bool, reg: u8, v: u8) {
        self.ym.write(hi, reg, v);
    }

    fn read(&self, hi: bool, reg: u8) -> u8 {
        self.ym.fm.regs.read(u16::from(reg) | if hi { 0x100 } else { 0 })
    }

    /// Writes a register only when its value changes.
    fn set(&mut self, hi: bool, reg: u8, v: u8) {
        if self.read(hi, reg) != v {
            self.write(hi, reg, v);
        }
    }

    /// A frequency's two registers, when either changes: the high one
    /// first, as the chip latches it (one latch for a half's channels) for
    /// the low one's write.
    fn freq(&mut self, hi: bool, hreg: u8, lreg: u8, h: u8, l: u8) {
        if self.read(hi, hreg) != h || self.read(hi, lreg) != l {
            self.write(hi, hreg, h);
            self.write(hi, lreg, l);
        }
    }

    /// The registers as text, as `fmframes.py` prints Furnace's.
    #[cfg(test)]
    fn state(&self) -> String {
        let mut t = Vec::new();
        for c in 0..6 {
            let (hi, o) = (c >= 3, (c % 3) as u8);
            let fnum = u16::from(self.read(hi, 0xA0 + o)) | (u16::from(self.read(hi, 0xA4 + o) & 7) << 8);
            let blk = (self.read(hi, 0xA4 + o) >> 3) & 7;
            let tl: Vec<String> = [0u8, 4, 8, 12].iter().map(|x| (self.read(hi, 0x40 + o + x) & 0x7F).to_string()).collect();
            t.push(format!(
                "{fnum}/{blk}/{:x}/{}/{:02x}/{:x}",
                self.keyed[c],
                tl.join("."),
                self.read(hi, 0xB0 + o),
                self.read(hi, 0xB4 + o) >> 6
            ));
        }
        t.push(self.sn.state());
        t.join(" ")
    }
}

/// The chip's operators O1 to O4 as DefleMask's instruments number them.
const CHIP_OP: [usize; 4] = [0, 2, 1, 3];

/// Each DefleMask operator's register offset (they're in register order).
const OP_OFFSET: [u8; 4] = [0, 4, 8, 12];

/// The operators (DefleMask's numbering) each algorithm sounds.
fn carriers(alg: u8) -> &'static [usize] {
    match alg & 7 {
        0..=3 => &[3],
        4 => &[2, 3],
        5 | 6 => &[1, 2, 3],
        _ => &[0, 1, 2, 3],
    }
}

/// A sample as the DAC plays it: scaled by its amp (50 is as recorded),
/// 8 bits unsigned, and how fast (its rate, times 1/6 to 5 by its pitch).
fn dac_sample(s: &crate::deflemask::Sample) -> (Vec<u8>, f64) {
    let data = s.data.iter().map(|&v| {
        let v = if s.bits == 8 { (i32::from(v) - 128) << 8 } else { i32::from(v) };
        (((v * i32::from(s.amp) / 50) >> 8) + 128).clamp(0, 255) as u8
    });
    let rate = match s.rate {
        1 => 8000.0,
        2 => 11025.0,
        3 => 16000.0,
        4 => 22050.0,
        _ => 32000.0,
    };
    let factor = match s.pitch {
        p @ 0..=4 => 1.0 / f64::from(6 - p),
        p @ 6..=9 => f64::from(p - 4),
        _ => 1.0,
    };
    (data.collect(), rate * factor)
}

/// The 2A03 and the registers written to it (the period's high bits are
/// written only when they change, as a write restarts the wave).
struct NesOut {
    apu: Apu,
    /// The PCM channel's sample playing, and the samples (8 bits).
    pcm: Option<DacPlay>,
    samples: Arc<Vec<(Vec<u8>, f64)>>,
    regs: [u8; 0x18],
    /// Whether each pulse and the triangle have had a note's high write.
    started: [bool; 3],
}

impl NesOut {
    fn write(&mut self, reg: u16, v: u8) {
        self.regs[usize::from(reg)] = v;
        self.apu.write(reg, v);
    }

    /// The registers as text, as `nesframes.py` prints them.
    #[cfg(test)]
    fn state(&self) -> String {
        let r = &self.regs;
        let sq = |b: usize| format!("{}/{}/{}", u16::from(r[b + 2]) | (u16::from(r[b + 3] & 7) << 8), r[b] & 15, r[b] >> 6);
        format!(
            "{} {} {}/{} {}/{}/{}",
            sq(0),
            sq(4),
            u16::from(r[0xA]) | (u16::from(r[0xB] & 7) << 8),
            u8::from(r[8] & 0x7F != 0),
            r[0xE] & 15,
            r[0xC] & 15,
            r[0xE] >> 7
        )
    }
}

struct Player {
    m: Arc<Module>,
    ch: Vec<Ch>,
    chips: Chips,
    order: usize,
    row: usize,
    /// Ticks left in the row, which of the two speeds the row has.
    ticks_left: u32,
    odd: bool,
    tick_in_row: u32,
    speed1: u32,
    speed2: u32,
    /// The next row to play, set by a jump or break (order, row).
    next: Option<(usize, usize)>,
    shown: (usize, usize),
    /// Orders played, to tell when the song comes round.
    seen: Vec<bool>,
    done: bool,
    started: bool,
    /// EFxx: 128ths of a semitone added to every channel.
    global_fine: i32,
    /// E0xx: ticks each step of 0xy lasts.
    arp_speed: u8,
    /// EBxx: the sample bank (12 samples each).
    sample_bank: usize,
    /// Each channel's triggers this tick, for the Game Boy's register dump.
    trig: [u32; 4],
    bufs: Vec<Vec<f32>>,
    /// Each channel's DC filter (last input and output).
    dc: Vec<(f32, f32)>,
}

impl Player {
    fn new(m: Arc<Module>) -> Player {
        let n = m.system.channels();
        let chips = match m.system {
            System::Nes => {
                let mut n = NesOut {
                    apu: Apu::new(false),
                    pcm: None,
                    samples: Arc::new(m.samples.iter().map(dac_sample).collect()),
                    regs: [0; 0x18],
                    started: [false; 3],
                };
                for (r, v) in [(0x15, 0x0F), (0x00, 0x30), (0x04, 0x30), (0x01, 0x08), (0x05, 0x08), (0x08, 0x80), (0x0C, 0x30)] {
                    n.write(r, v);
                }
                Chips::Nes(n)
            }
            System::Genesis { ext3 } => {
                let mut g = GenOut {
                    ym: Ym2612::default(),
                    sn: Sn76489::new(PSG_CLOCK),
                    loaded: [None; 6],
                    keyed: [0; 6],
                    ext_loaded: [None; 4],
                    dac: None,
                    dac_samples: Arc::new(m.samples.iter().map(dac_sample).collect()),
                    pos: 0.0,
                };
                // Every operator released at once, then the LFO on at its slowest.
                for hi in [false, true] {
                    for r in 0x80..0x90u8 {
                        g.write(hi, r, 0xFF);
                    }
                }
                for c in [0u8, 1, 2, 4, 5, 6] {
                    g.write(false, 0x28, c);
                }
                g.write(false, 0x2B, 0);
                g.write(false, 0x22, 0x08);
                if ext3 {
                    g.write(false, 0x27, 0x40);
                }
                for c in 0..4 {
                    g.sn.set_att(c, 15);
                }
                g.sn.set_noise(7);
                Chips::Genesis(Box::new(g))
            }
            System::GameBoy => {
                let mut g = GbOut {
                    gb: Gb::default(),
                    wave: None,
                    started: [false; 4],
                };
                // Every channel triggered silent, then the sound on, both sides.
                for (r, v) in [
                    (0x02, 0),
                    (0x04, 0x80),
                    (0x07, 0),
                    (0x09, 0x80),
                    (0x0C, 0),
                    (0x0E, 0x80),
                    (0x11, 0),
                    (0x13, 0x80),
                    (0x00, 0),
                    (0x16, 0x8F),
                    (0x15, 0xFF),
                    (0x14, 0xFF),
                ] {
                    g.gb.write(r, v);
                }
                Chips::GameBoy(Box::new(g))
            }
            System::PcEngine => {
                let mut g = PceOut {
                    psg: HuC6280::default(),
                    wave: [None; 6],
                    noise: [false; 6],
                    dda: Default::default(),
                    samples: Arc::new(m.samples.iter().map(dac_sample).collect()),
                };
                // Each channel's wave cleared, off; the main levels full.
                for c in 0..6u8 {
                    g.psg.write(0, c);
                    g.psg.write(4, 0x5F);
                    g.psg.write(4, 0x1F);
                    for _ in 0..32 {
                        g.psg.write(6, 0);
                    }
                    g.psg.write(5, 0xFF);
                }
                g.psg.write(1, 0xFF);
                g.psg.write(8, 0xFF);
                Chips::PcEngine(Box::new(g))
            }
            System::C64 { .. } => {
                let mut g = C64Out {
                    sid: Sid::new(48_000),
                    rate: 48_000,
                    owed: 0.0,
                    cutoff: 0x7FF,
                    resonance: 0,
                    mode: 0,
                    pw: [2048; 3],
                    wave: [0; 3],
                    ad: [0; 3],
                    sr: [0xF0; 3],
                    ring_sync: [0; 3],
                    filt: [false; 3],
                    no_reset: [false; 3],
                    gate: [false; 3],
                    started: [false; 3],
                };
                g.sid.write(24, 15);
                Chips::C64(Box::new(g))
            }
            System::Arcade => {
                let mut g = ArcOut {
                    opm: Ym2151::default(),
                    loaded: [None; 8],
                    keyed: [false; 8],
                    pos: 0.0,
                    pcm: Default::default(),
                    pcm_level: [(1.0, 1.0); 5],
                    samples: Arc::new(m.samples.iter().map(dac_sample).collect()),
                };
                for c in 0..8u8 {
                    for k in 0..4u8 {
                        g.opm.write(0xE0 + c + 8 * k, 0xFF);
                    }
                    g.opm.write(8, c);
                }
                // The LFO held at its start, its depths full.
                for (r, v) in [(0x01, 0x02), (0x18, 0), (0x19, 0x7F), (0x19, 0xFF)] {
                    g.opm.write(r, v);
                }
                Chips::Arcade(Box::new(g))
            }
            System::SmsOpll => {
                let mut g = OpllOut {
                    sn: Sn76489::new(PSG_CLOCK),
                    opll: Opll::new(crate::opll::CLOCK, 48_000),
                    rate: 48_000,
                    regs: [0; 0x40],
                    keyed: [false; 9],
                    kit: [None; 3],
                    drum_ch: [10, 11, 11, 12, 12],
                };
                for c in 0..4 {
                    g.sn.set_att(c, 15);
                }
                g.sn.set_noise(7);
                Chips::SmsOpll(Box::new(g))
            }
            _ => Chips::Sms(Sn76489::new(PSG_CLOCK)),
        };
        let speed1 = u32::from(m.tick1.max(1)) * (u32::from(m.time_base) + 1);
        let speed2 = u32::from(m.tick2.max(1)) * (u32::from(m.time_base) + 1);
        let mut p = Player {
            ch: (0..n).map(|_| Ch::new()).collect(),
            chips,
            order: 0,
            row: 0,
            ticks_left: 0,
            odd: false,
            tick_in_row: 0,
            speed1,
            speed2,
            next: None,
            shown: (0, 0),
            seen: vec![false; m.orders().max(1)],
            done: false,
            started: false,
            global_fine: 0,
            arp_speed: 1,
            sample_bank: 0,
            trig: [u32::from(m.system == System::GameBoy); 4],
            bufs: vec![Vec::new(); n.max(12)],
            dc: vec![(0.0, 0.0); n],
            m,
        };
        for c in 0..n {
            p.ch[c].vol = p.max_vol(c);
            p.ch[c].vol4 = p.ch[c].vol * 4 + 3;
            p.ch[c].out_vol = p.ch[c].vol;
        }
        if let Chips::Sms(sn) = &mut p.chips {
            for c in 0..4 {
                sn.set_att(c, 15);
            }
            sn.set_noise(7);
        }
        p
    }

    fn max_vol(&self, c: usize) -> i32 {
        max_volume(self.m.system, c)
    }

    /// The row's speed once channel `c`'s row is read (its 9xx or Fxx counted).
    fn row_speed_after(&self, c: usize) -> u32 {
        let tb = u32::from(self.m.time_base) + 1;
        let mut s = (self.speed1, self.speed2);
        for ch in 0..=c {
            for &(code, v) in &self.m.patterns[ch][self.order][self.row].fx {
                match code {
                    0x09 if v > 0 => s.0 = v as u32 * tb,
                    0x0F if v > 0 => s.1 = v as u32 * tb,
                    _ => {}
                }
            }
        }
        if self.odd {
            s.1
        } else {
            s.0
        }
    }

    fn row_speed(&self) -> u32 {
        if self.odd {
            self.speed2
        } else {
            self.speed1
        }
    }

    fn tick_once(&mut self) {
        // (The first tick's count keeps the start's triggers.)
        if self.started {
            self.trig = [0; 4];
        }
        if self.ticks_left == 0 {
            if self.started {
                self.advance();
            }
            self.started = true;
            self.tick_in_row = 0;
            self.read_row();
            // 9xx and Fxx count from the row they're on.
            self.ticks_left = self.row_speed().max(1);
        } else {
            self.tick_in_row += 1;
        }
        self.kit_start();
        for c in 0..self.ch.len() {
            self.tick_channel(c);
        }
        self.ticks_left -= 1;
    }

    /// The YM2413's rhythm mode, before the tick's notes: each time one of
    /// FM 7 to 9 strikes a drum kit it hadn't, the drums' channels set to
    /// their pitches again.
    fn kit_start(&mut self) {
        let Chips::SmsOpll(g) = &mut self.chips else { return };
        for k in 6..9 {
            let ch = &self.ch[4 + k];
            let kit = self.m.instruments.get(ch.instrument).and_then(|i| i.fm.as_ref()).is_some_and(|f| f.ops[0].dt2 >= 16);
            if kit && ch.fresh && ch.on && g.kit[k - 6] != Some(ch.instrument) {
                g.kit[k - 6] = Some(ch.instrument);
                for (r, v) in [(0x16, 0x20), (0x26, 0x05), (0x17, 0x50), (0x27, 0x05), (0x18, 0xC0), (0x28, 0x01)] {
                    g.write(r, v);
                }
                g.set(0x0E, g.regs[0x0E] | 0x20);
            }
        }
    }

    /// Moves to the next row (or where a jump sends it).
    fn advance(&mut self) {
        self.odd = !self.odd;
        let orders = self.m.orders().max(1);
        let (o, r) = match self.next.take() {
            Some(n) => n,
            None if self.row + 1 < self.m.rows => (self.order, self.row + 1),
            None => (self.order + 1, 0),
        };
        let o = if o >= orders { 0 } else { o };
        if (o != self.order || r < self.row || (o, r) == (self.order, self.row)) && self.seen[o] {
            self.done = true;
            self.seen.iter_mut().for_each(|s| *s = false);
        }
        self.order = o;
        self.row = r.min(self.m.rows - 1);
    }

    fn read_row(&mut self) {
        let (o, r) = (self.order, self.row);
        self.seen[o] = true;
        self.shown = (o, r);
        for c in 0..self.ch.len() {
            let cell = self.m.patterns[c][o][r].clone();
            // A note delay holds the whole row back.
            if let Some(d) = cell.fx.iter().find(|e| e.0 == 0xED && e.1 > 0).map(|e| e.1 as u32) {
                if d < self.row_speed_after(c) {
                    self.ch[c].delayed = Some((d as u8, cell));
                    continue;
                }
            }
            self.ch[c].delayed = None;
            self.play_cell(c, &cell);
        }
    }

    fn play_cell(&mut self, c: usize, cell: &Cell) {
        let porta = cell.fx.iter().any(|e| (e.0 == 3 || e.0 == 5) && e.1 != 0) || (self.ch[c].porta_speed != 0 && cell.fx.iter().any(|e| e.0 == 5));
        if cell.instrument >= 0 {
            let ch = &mut self.ch[c];
            // Another instrument drops the effects' changes to the last (and
            // on the Game Boy the volume column's, its envelope's taking over).
            if ch.instrument != cell.instrument as usize {
                if self.m.system == System::GameBoy && c != 2 {
                    ch.vol_set = false;
                }
                ch.fm_tl = [None; 4];
                ch.fm_mult = [None; 4];
                ch.fm_ar = [None; 4];
                ch.fm_fb = None;
            }
            ch.instrument = cell.instrument as usize;
        }
        // The triangle has no volume: only its macro turns it off.
        let triangle = self.m.system == System::Nes && c == 2;
        if cell.volume >= 0 && !triangle {
            let max = self.max_vol(c);
            let v = i32::from(cell.volume).min(max);
            let ch = &mut self.ch[c];
            ch.vol = v;
            ch.vol4 = if v >= max { v * 4 + 3 } else { v * 4 };
            ch.out_vol = v;
            ch.vol_set = true;
        }
        let ch = &mut self.ch[c];
        ch.cut = None;
        ch.retrig = 0;
        if cell.note == 100 {
            ch.on = false;
            ch.vol_mac.running = false;
            ch.porta_target = None;
            ch.note_slide = None;
        } else if cell.note > 0 || cell.octave > 0 {
            let note = (i32::from(cell.octave) * 12 + i32::from(cell.note)).clamp(0, 127);
            if porta && ch.note.is_some() && ch.on {
                ch.porta_target = Some(note * 128);
            } else {
                ch.note = Some(note);
                ch.slide = 0;
                ch.porta_target = None;
                ch.note_slide = None;
                ch.vib_pos = 0;
                ch.arp_hold = ch.on;
                ch.noise_slide = 0;
                ch.on = true;
                if !ch.legato {
                    self.start_macros(c);
                }
            }
        }
        for &(code, val) in &cell.fx {
            if code < 0 {
                continue;
            }
            let v = if val < 0 { 0 } else { val as i32 };
            self.effect(c, code as i32, v);
        }
    }

    fn start_macros(&mut self, c: usize) {
        let ins = self.m.instruments.get(self.ch[c].instrument).cloned().unwrap_or_default();
        let ch = &mut self.ch[c];
        ch.vol_mac = MacroPos::start(&ins.volume);
        ch.arp_mac = MacroPos::start(&ins.arpeggio);
        ch.duty_mac = MacroPos::start(&ins.duty);
        ch.wave_mac = MacroPos::start(&ins.wave);
        ch.arp_now = 0;
        ch.arp_fixed = false;
        ch.duty_now = None;
        ch.vol_mac_now = None;
        // A note keeps the volume the last left, but the triangle (whose only
        // volume is its macro's on or off) sounds again.
        if self.m.system == System::Nes && c == 2 {
            ch.out_vol = ch.vol;
        }
        // 0xy's steps go on through new notes, but a note's first tick is the note.
        ch.fresh = true;
    }

    fn effect(&mut self, c: usize, code: i32, v: i32) {
        let ch = &mut self.ch[c];
        let (x, y) = (v >> 4, v & 15);
        match code {
            0x00 => {
                ch.arp = v as u8;
                if v == 0 {
                    // Back to before the note's step, to step onto it next.
                    ch.arp_step = 2;
                    ch.arp_tick = u8::MAX;
                }
            }
            // 1xx, 2xx and 3xx are one slide: each takes the others' place.
            0x01 | 0x02 => {
                ch.slide_speed = if code == 1 { v } else { -v };
                ch.porta_target = None;
            }
            0x03 => {
                ch.porta_speed = v;
                ch.slide_speed = 0;
                if v == 0 {
                    ch.porta_target = None;
                }
            }
            0x04 => {
                if x != 0 {
                    ch.vib_speed = x;
                }
                ch.vib_depth = y;
                if v == 0 {
                    ch.vib_speed = 0;
                }
            }
            // Down when y is set (A93 slides down by 3), else up by x.
            0x05 | 0x06 | 0x0A => {
                ch.vol_slide = if y > 0 { -y } else { x };
            }
            0x07 => {
                ch.trem_speed = x;
                ch.trem_depth = y;
            }
            0x09 => self.speed1 = (v.max(1) as u32) * (u32::from(self.m.time_base) + 1),
            0x0F => self.speed2 = (v.max(1) as u32) * (u32::from(self.m.time_base) + 1),
            0x0B => self.next = Some((v as usize, 0)),
            0x0D => {
                let o = self.next.map_or(self.order + 1, |n| n.0);
                self.next = Some((o, v as usize));
            }
            0x0C => {
                ch.retrig = v as u8;
                ch.retrig_tick = 0;
            }
            // The arpeggio's speed is the song's, not the channel's.
            0xE0 => self.arp_speed = (v as u8).max(1),
            0xE1 | 0xE2 => {
                if let Some(n) = ch.note {
                    let base = ch.porta_target.unwrap_or(n * 128 + ch.slide);
                    let to = if code == 0xE1 { base + y * 128 } else { base - y * 128 };
                    let cur = n * 128 + ch.slide;
                    ch.note_slide = Some((x * 4, to.clamp(0, 127 * 128)));
                    ch.slide = cur - n * 128;
                }
            }
            0xE3 => ch.vib_mode = v as u8,
            0xE4 => ch.vib_fine = v,
            0xE5 => ch.fine = (v - 0x80) * if self.m.system == System::Arcade { 2 } else { 1 },
            0xEA => ch.legato = v != 0,
            0xEC => ch.cut = Some(v as u8),
            0xEF => self.global_fine += v - 0x80,
            0x12 if matches!(self.m.system, System::Nes | System::GameBoy) => ch.duty_fx = v,
            0x10 if matches!(self.m.system, System::GameBoy | System::PcEngine) => ch.wave_fx = Some(v as usize),
            0x11 if self.m.system == System::PcEngine => ch.noise_narrow = v != 0,
            0x12 if self.m.system == System::PcEngine => {
                if let Chips::PcEngine(g) = &mut self.chips {
                    g.psg.write(9, v as u8);
                }
            }
            // 13xx: the LFO's speed, its register the complement (higher is faster).
            0x17 if self.m.system == System::PcEngine => ch.dac = v != 0,
            0x13 if self.m.system == System::PcEngine => {
                if let Chips::PcEngine(g) = &mut self.chips {
                    g.psg.write(8, 0xFF - v as u8);
                }
            }
            0x11 if self.m.system == System::GameBoy => ch.noise_narrow = v != 0,
            0x10..=0x1E if matches!(self.m.system, System::C64 { .. }) => self.c64_effect(c, code, v),
            0x10..=0x20 if self.m.system == System::Arcade => self.arcade_effect(c, code, v),
            0x13 if self.m.system == System::GameBoy => ch.sweep = (ch.sweep & 8) | ((x as u8 & 7) << 4) | (y as u8 & 7),
            0x14 if self.m.system == System::GameBoy => ch.sweep = (ch.sweep & 0x77) | if v != 0 { 8 } else { 0 },
            // 800 is both sides, as 811 is.
            0x08 if self.m.system == System::PcEngine => ch.balance = Some(v as u8),
            0x08 if self.m.system == System::Arcade && c >= 8 => ch.balance = Some(v as u8),
            0x08 => ch.pan = if v == 0 { (true, true) } else { (x != 0, y != 0) },
            0xEB => self.sample_bank = v as usize,
            0x10..=0x1D if matches!(self.m.system, System::Genesis { .. }) => self.fm_effect(c, code, v),
            0x11..=0x1B if self.m.system == System::SmsOpll && c >= 4 => self.opll_effect(c, code, v),
            0x20 => {
                // Only each digit's low bit counts (2002 is periodic noise).
                ch.noise_special = x & 1 != 0;
                ch.noise_white = y & 1 != 0;
            }
            _ => {}
        }
    }

    /// The arcade board's effects (10xx to 1Exx on the FM, 20xx on the PCM).
    fn arcade_effect(&mut self, c: usize, code: i32, v: i32) {
        let (x, y) = (v >> 4, v & 15);
        let ch = &mut self.ch[c];
        match code {
            0x10 => {
                if let Chips::Arcade(g) = &mut self.chips {
                    g.opm.write(0x0F, if v == 0 { 0 } else { 0x80 | ((v - 1).clamp(0, 31) as u8) });
                }
            }
            0x11 if v <= 7 => ch.fm_fb = Some(v as u8),
            0x12..=0x15 if v <= 0x7F => ch.fm_tl[CHIP_OP[(code - 0x12) as usize]] = Some(v as u8),
            0x16 if (1..=4).contains(&x) => ch.fm_mult[CHIP_OP[(x - 1) as usize]] = Some(y as u8),
            0x17 | 0x18 | 0x1E => {
                if let Chips::Arcade(g) = &mut self.chips {
                    match code {
                        0x17 => {
                            g.opm.write(0x01, 0);
                            g.opm.write(0x18, v as u8);
                        }
                        0x18 => g.opm.write(0x1B, (v & 3) as u8),
                        _ => g.opm.write(0x19, v as u8),
                    }
                }
            }
            0x19 if v <= 0x1F => ch.fm_ar = [Some(v as u8); 4],
            0x1A..=0x1D if v <= 0x1F => ch.fm_ar[CHIP_OP[(code - 0x1A) as usize]] = Some(v as u8),
            // 20xx: a PCM channel's rate, xx × 31250 / 255 Hz.
            0x20 => ch.pcm_rate = Some(f64::from(v) * 31250.0 / 255.0),
            _ => {}
        }
    }

    /// The C64's effects (10xx to 1Exy).
    fn c64_effect(&mut self, c: usize, code: i32, v: i32) {
        let Chips::C64(g) = &mut self.chips else { return };
        let (x, y) = (v >> 4, v & 15);
        match code {
            0x10 => g.wave[c] = (v & 15) as u8,
            0x11 => g.cutoff = (v.min(100) * 2047 / 100) as u16,
            0x12 => g.pw[c] = (v.min(100) * 4095 / 100) as u16,
            0x13 => g.resonance = (v & 15) as u8,
            0x14 => g.mode = (v & 7) as u8,
            0x1A => g.no_reset[c] = v != 0,
            0x1E => match x {
                0 => g.ad[c] = (g.ad[c] & 0x0F) | (y as u8) << 4,
                1 => g.ad[c] = (g.ad[c] & 0xF0) | y as u8,
                2 => g.sr[c] = (g.sr[c] & 0x0F) | (y as u8) << 4,
                3 => g.sr[c] = (g.sr[c] & 0xF0) | y as u8,
                4 => g.ring_sync[c] = (g.ring_sync[c] & !4) | if y != 0 { 4 } else { 0 },
                5 => g.ring_sync[c] = (g.ring_sync[c] & !2) | if y != 0 { 2 } else { 0 },
                6 => g.mode = (g.mode & 7) | if y != 0 { 8 } else { 0 },
                _ => {}
            },
            _ => {}
        }
    }

    /// The YM2413's effects (11xx to 1Bxx), on the custom voice (13xx the
    /// volume's floor), written at once.
    fn opll_effect(&mut self, c: usize, code: i32, v: i32) {
        let (x, y) = (v >> 4, v & 15);
        let ch = &mut self.ch[c];
        match code {
            0x11 if v <= 7 => ch.fm_fb = Some(v as u8),
            0x12 if v <= 0x3F => ch.fm_tl[0] = Some(v as u8),
            0x13 if v <= 0x0F => ch.fm_tl[1] = Some(v as u8),
            0x16 if (1..=2).contains(&x) => ch.fm_mult[(x - 1) as usize] = Some(y as u8),
            0x19 if v <= 0x0F => {
                ch.fm_ar[0] = Some(v as u8);
                ch.fm_ar[1] = Some(v as u8);
            }
            0x1A | 0x1B if v <= 0x0F => ch.fm_ar[(code - 0x1A) as usize] = Some(v as u8),
            _ => return,
        }
        ch.voice_fx = code != 0x13;
    }

    /// The Genesis's FM effects (10xx to 1Dxx).
    fn fm_effect(&mut self, c: usize, code: i32, v: i32) {
        let (x, y) = (v >> 4, v & 15);
        let ch = &mut self.ch[c];
        match code {
            0x10 => {
                if let Chips::Genesis(g) = &mut self.chips {
                    g.set(false, 0x22, if x != 0 { 8 | (y & 7) as u8 } else { 0 });
                }
            }
            0x11 if v <= 7 => ch.fm_fb = Some(v as u8),
            // The effects number the operators as the chip does (O1 to O4),
            // not as the instrument lists them: 2 and 3 swap.
            0x12..=0x15 if v <= 0x7F => ch.fm_tl[CHIP_OP[(code - 0x12) as usize]] = Some(v as u8),
            0x16 if (1..=4).contains(&x) => ch.fm_mult[CHIP_OP[(x - 1) as usize]] = Some(y as u8),
            0x17 => ch.dac = v != 0,
            0x18 => {
                if let Chips::Genesis(g) = &mut self.chips {
                    g.set(false, 0x27, if v != 0 { 0x40 } else { 0 });
                }
            }
            0x19 if v <= 0x1F => ch.fm_ar = [Some(v as u8); 4],
            0x1A..=0x1D if v <= 0x1F => ch.fm_ar[CHIP_OP[(code - 0x1A) as usize]] = Some(v as u8),
            _ => {}
        }
    }

    fn tick_channel(&mut self, c: usize) {
        let t = self.tick_in_row;
        if let Some((d, _)) = &self.ch[c].delayed {
            if u32::from(*d) == t {
                let (_, cell) = self.ch[c].delayed.take().unwrap();
                self.play_cell(c, &cell);
            }
        }
        let ins = self.m.instruments.get(self.ch[c].instrument).cloned().unwrap_or_default();
        let max = self.max_vol(c);
        let gb_noise_ch = self.m.system == System::GameBoy && c == 3;
        let mult_vol = self.m.system == System::PcEngine;
        // The YM2151's slides go in 64ths of a semitone, the others' in 32nds.
        let unit = if self.m.system == System::Arcade { 2 } else { 1 };
        let slide_range = match self.m.system {
            System::Genesis { ext3 } if !matches!(gen_ch(ext3, c), GenCh::Psg(_)) => Some((0, 96 * 128)),
            System::SmsOpll if c >= 4 => Some((0, 96 * 128)),
            System::GameBoy => Some((24 * 128, 95 * 128)),
            _ => None,
        };
        let ch = &mut self.ch[c];
        if let Some(k) = ch.cut {
            if u32::from(k) == t {
                ch.on = false;
                ch.cut = None;
            }
        }
        if ch.retrig > 0 && t > 0 {
            ch.retrig_tick += 1;
            if ch.retrig_tick >= ch.retrig {
                ch.retrig_tick = 0;
                ch.on = true;
                ch.vol_mac = MacroPos::start(&ins.volume);
                ch.arp_mac = MacroPos::start(&ins.arpeggio);
                ch.duty_mac = MacroPos::start(&ins.duty);
                ch.wave_mac = MacroPos::start(&ins.wave);
            }
        }
        // Volume: slides, then the macro over the column.
        if ch.vol_slide != 0 {
            ch.vol4 = (ch.vol4 + ch.vol_slide).clamp(0, max * 4 + 3);
            ch.vol = ch.vol4 >> 2;
            ch.out_vol = ch.vol;
        }
        if let Some(m) = ch.vol_mac.step(&ins.volume) {
            // The macro takes the column's volume down: by its distance from
            // the top, or (the PC Engine) in proportion.
            ch.out_vol = if mult_vol { m.clamp(0, max) * ch.vol / max } else { (m - (max - ch.vol)).clamp(0, max) };
            ch.vol_mac_now = Some(m);
        }
        if let Some(a) = ch.arp_mac.step(&ins.arpeggio).map(|a| a.clamp(-128, 255)) {
            if ins.arp_fixed {
                ch.arp_fixed = true;
                ch.arp_now = a;
            } else {
                ch.arp_now = a - 12;
            }
        } else if ch.arp_fixed {
            // A fixed arpeggio's end gives the note back.
            ch.arp_fixed = false;
            ch.arp_now = 0;
        }
        if let Some(d) = ch.duty_mac.step(&ins.duty) {
            ch.duty_now = Some(d);
        }
        if let Some(w) = ch.wave_mac.step(&ins.wave) {
            ch.wave_now = Some(w);
        }
        // The Game Boy's noise slides up its steps 4 a tick for each of 1xx's
        // or 2xx's, either way.
        if gb_noise_ch && ch.slide_speed != 0 {
            ch.noise_slide = (ch.noise_slide + 4 * ch.slide_speed.abs()).min(64);
        }
        // Pitch. On FM a slide that reaches C-0 or C-8 stops there, and
        // ends; on the Game Boy at C-2 or B-7.
        if ch.slide_speed != 0 && !gb_noise_ch {
            ch.slide = (ch.slide + ch.slide_speed * 4 * unit).clamp(-(1 << 16), 1 << 16);
            if let Some((lo, hi)) = slide_range {
                if let Some(n) = ch.note {
                    let p = n * 128 + ch.slide;
                    if !(lo..=hi).contains(&p) {
                        ch.slide = p.clamp(lo, hi) - n * 128;
                        ch.slide_speed = 0;
                    }
                }
            }
        }
        if let Some(n) = ch.note {
            let cur = n * 128 + ch.slide;
            if let Some(target) = ch.porta_target {
                let s = ch.porta_speed * 4 * unit;
                let next = if cur < target { (cur + s).min(target) } else { (cur - s).max(target) };
                ch.slide = next - n * 128;
                if next == target {
                    ch.note = Some(target / 128);
                    ch.slide = target % 128;
                    ch.porta_target = None;
                }
            } else if let Some((s, target)) = ch.note_slide {
                let next = if cur < target { (cur + s * 4 * unit).min(target) } else { (cur - s * 4 * unit).max(target) };
                ch.slide = next - n * 128;
                if next == target {
                    ch.note_slide = None;
                }
            }
        }
        if ch.vib_speed != 0 && ch.vib_depth != 0 {
            ch.vib_pos = (ch.vib_pos + ch.vib_speed) & 63;
        }
        if ch.trem_speed != 0 && ch.trem_depth != 0 {
            ch.trem_pos = (ch.trem_pos + ch.trem_speed) & 63;
        }
        // 0xy steps on each tick but the first of a note that follows one.
        if ch.arp != 0 && !(ch.fresh && ch.arp_hold) {
            ch.arp_tick = ch.arp_tick.saturating_add(1);
            if ch.arp_tick >= self.arp_speed.max(1) {
                ch.arp_tick = 0;
                ch.arp_step = (ch.arp_step + 1) % 3;
            }
        }
        self.output(c);
        self.ch[c].fresh = false;
    }

    /// The channel's pitch now (128ths of a semitone), None when silent.
    fn pitch(&self, c: usize) -> Option<i32> {
        let ch = &self.ch[c];
        let n = ch.note?;
        let base = if ch.arp_fixed { ch.arp_now * 128 } else { (n + ch.arp_now) * 128 + ch.slide };
        let arp = if ch.fresh && ch.arp_hold {
            0
        } else {
            match ch.arp_step {
                1 => i32::from(ch.arp >> 4),
                2 => i32::from(ch.arp & 15),
                _ => 0,
            }
        };
        let mut vib = 0;
        if ch.vib_speed != 0 && ch.vib_depth != 0 {
            let s = (f64::from(ch.vib_pos) * std::f64::consts::TAU / 64.0).sin();
            let s = match ch.vib_mode {
                1 => s.max(0.0),
                2 => s.min(0.0),
                _ => s,
            };
            vib = (s * f64::from(ch.vib_depth * 8 * ch.vib_fine) / 15.0).round() as i32;
        }
        Some(base + arp * 128 + vib + ch.fine + self.global_fine)
    }

    fn output(&mut self, c: usize) {
        if let System::Genesis { ext3 } = self.m.system {
            match gen_ch(ext3, c) {
                GenCh::Psg(k) => self.output_psg(c, k),
                GenCh::Fm(k) => self.output_fm(c, k),
                GenCh::Op(k) => self.output_op(c, k),
            }
            return;
        }
        if self.m.system == System::Sms {
            self.output_psg(c, c);
            return;
        }
        if self.m.system == System::SmsOpll {
            if c < 4 {
                self.output_psg(c, c);
            } else {
                self.output_opll(c, c - 4);
            }
            return;
        }
        if self.m.system == System::GameBoy {
            self.output_gb(c);
            return;
        }
        if self.m.system == System::PcEngine {
            self.output_pce(c);
            return;
        }
        if matches!(self.m.system, System::C64 { .. }) {
            self.output_c64(c);
            return;
        }
        if self.m.system == System::Arcade {
            self.output_arcade(c);
            return;
        }
        let pitch = self.pitch(c);
        let ch = &self.ch[c];
        let vol = if ch.on { ch.out_vol } else { 0 };
        let duty = ch.duty_now;
        if let Chips::Nes(n) = &mut self.chips {
            {
                let vol = vol.clamp(0, 15) as u8;
                let trigger = ch.fresh;
                match c {
                    0 | 1 => {
                        let b = (c * 4) as u16;
                        // A note off writes the duty back to 0 with the volume.
                        let d = if ch.on { duty.unwrap_or(ch.duty_fx).clamp(0, 3) as u8 } else { 0 };
                        n.write(b, d << 6 | 0x30 | vol);
                        if let Some(p) = pitch {
                            let per = nes_period(p);
                            n.write(b + 2, per as u8);
                            let hi = (per >> 8) as u8 | 0xF8;
                            if trigger || !n.started[c] || n.regs[usize::from(b + 3)] != hi {
                                n.write(b + 3, hi);
                                n.started[c] = true;
                            }
                        }
                    }
                    2 => {
                        n.write(8, if vol > 0 { 0xFF } else { 0x80 });
                        if let Some(p) = pitch {
                            let per = nes_period(p);
                            n.write(0xA, per as u8);
                            let hi = (per >> 8) as u8 | 0xF8;
                            if trigger || !n.started[2] || n.regs[0xB] != hi || (vol > 0 && n.regs[8] != 0xFF) {
                                n.write(0xB, hi);
                                n.started[2] = true;
                            }
                        }
                    }
                    // The PCM channel: its notes choose samples (by their place in
                    // the octave and EBxx's bank), played as recorded.
                    4 => {
                        if trigger {
                            let s = ch.note.unwrap_or(0).rem_euclid(12) as usize + 12 * self.sample_bank;
                            n.pcm = (s < n.samples.len()).then(|| DacPlay {
                                sample: s,
                                pos: 0.0,
                                step: n.samples[s].1,
                            });
                        }
                        if !ch.on {
                            n.pcm = None;
                        }
                    }
                    3 => {
                        let mode = duty.unwrap_or(ch.duty_fx) & 1;
                        n.write(0xC, 0x30 | vol);
                        if let Some(p) = pitch {
                            n.write(0xE, (mode as u8) << 7 | nes_noise(p.div_euclid(128)));
                            n.write(0xF, 0xF8);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// A PSG channel (tone `k` 0-2, or the noise, 3).
    fn output_psg(&mut self, c: usize, k: usize) {
        let pitch = self.pitch(c);
        let ch = &self.ch[c];
        let note = ch.note.unwrap_or(0);
        let vol = if ch.on { ch.out_vol } else { 0 };
        let (special, white) = match ch.duty_now {
            Some(d) => (d & 2 != 0, d & 1 != 0),
            None => (ch.noise_special, ch.noise_white),
        };
        let genesis = matches!(self.chips, Chips::Genesis(_));
        let sn = match &mut self.chips {
            Chips::Sms(sn) => sn,
            Chips::Genesis(g) => &mut g.sn,
            Chips::SmsOpll(g) => &mut g.sn,
            _ => return,
        };
        if k < 3 {
            if let Some(p) = pitch {
                sn.set_period(
                    k,
                    if genesis {
                        gen_psg_period(p, 90)
                    } else if note >= 95 {
                        1
                    } else {
                        sn_period(p)
                    },
                );
            }
            sn.set_att(k, (15 - vol.clamp(0, 15)) as u8);
        } else {
            // Without the special mode only C, C# and D set the rate
            // (2, 1, 0); other notes leave it as it was.
            let mut rate = sn.noise() & 3;
            if let Some(p) = pitch {
                let k = p.div_euclid(128).rem_euclid(12);
                if k < 3 {
                    rate = (2 - k) as u8;
                }
            }
            let reg = if special { 3 } else { rate } | if white { 4 } else { 0 };
            if reg != sn.noise() {
                sn.set_noise(reg);
            }
            if special {
                if let Some(p) = pitch {
                    // The noise's top note is A-7 (tone channels' A#7): a note
                    // above plays period 1 (a fixed arpeggio there doesn't).
                    sn.set_period(
                        2,
                        if genesis {
                            gen_psg_period(p, 94)
                        } else if note >= 94 {
                            1
                        } else {
                            sn_period(p)
                        },
                    );
                }
            }
            sn.set_att(3, (15 - vol.clamp(0, 15)) as u8);
        }
    }

    /// An FM channel (0-5).
    fn output_fm(&mut self, c: usize, k: usize) {
        let pitch = self.pitch(c);
        let Chips::Genesis(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        let ins: &Instrument = match self.m.instruments.get(ch.instrument) {
            Some(i) => i,
            None => return,
        };
        let (hi, o) = (k >= 3, (k % 3) as u8);
        let key = 0x28;
        let sel = (if hi { 4 } else { 0 }) | o;
        // FM 6 plays samples instead while 17xx says so (its frequency still
        // written, but never keyed).
        let dac = k == 5 && ch.dac;
        if dac {
            // (Furnace leaves FM 6 at C-4, unkeyed and its instrument unwritten.)
            if !g.ym.dac_enabled() {
                g.freq(true, 0xA6, 0xA2, 4 << 3 | (643u16 >> 8) as u8, 643u16 as u8);
            }
            g.set(false, 0x2B, 0x80);
            if ch.fresh {
                let s = (ch.note.unwrap_or(0).rem_euclid(12)) as usize + 12 * self.sample_bank;
                g.dac = (s < g.dac_samples.len()).then(|| DacPlay {
                    sample: s,
                    pos: 0.0,
                    step: g.dac_samples[s].1 / Ym2612::rate(YM2612_CLOCK),
                });
            }
            if !ch.on {
                g.dac = None;
            }
            return;
        } else if k == 5 && g.ym.dac_enabled() {
            g.write(false, 0x2B, 0);
            g.dac = None;
        }
        let Some(fm) = ins.fm.as_ref() else { return };
        // Nothing's written until the channel's first note.
        if g.loaded[k].is_none() && !ch.fresh {
            return;
        }
        if ch.fresh && g.loaded[k] != Some(ch.instrument) {
            g.loaded[k] = Some(ch.instrument);
            for (i, op) in fm.ops.iter().enumerate() {
                let r = OP_OFFSET[i] + o;
                let dt = if op.dt >= 3 { op.dt - 3 } else { 4 + (3 - op.dt) } & 7;
                g.write(hi, 0x30 + r, dt << 4 | (op.mult & 15));
                g.write(hi, 0x50 + r, (op.rs & 3) << 6 | (op.ar & 31));
                g.write(hi, 0x60 + r, (op.am & 1) << 7 | (op.dr & 31));
                g.write(hi, 0x70 + r, op.d2r & 31);
                g.write(hi, 0x80 + r, (op.sl & 15) << 4 | (op.rr & 15));
                g.write(hi, 0x90 + r, op.ssg & 15);
            }
        }
        // The instrument with the effects' changes, its carriers at the volume.
        let alg = fm.alg & 7;
        let vol = ch.vol.clamp(0, 127) as u32;
        for (i, op) in fm.ops.iter().enumerate() {
            let r = OP_OFFSET[i] + o;
            let tl = u32::from(ch.fm_tl[i].unwrap_or(op.tl) & 0x7F);
            // (Before version 18 the volume took every operator down.)
            let tl = if carriers(alg).contains(&i) || self.m.version < 18 { 127 - (127 - tl) * vol / 127 } else { tl };
            g.set(hi, 0x40 + r, tl as u8);
            let mult = ch.fm_mult[i].unwrap_or(op.mult) & 15;
            let dt = if op.dt >= 3 { op.dt - 3 } else { 4 + (3 - op.dt) } & 7;
            g.set(hi, 0x30 + r, dt << 4 | mult);
            let ar = ch.fm_ar[i].unwrap_or(op.ar) & 31;
            g.set(hi, 0x50 + r, (op.rs & 3) << 6 | ar);
        }
        g.set(hi, 0xB0 + o, (ch.fm_fb.unwrap_or(fm.fb) & 7) << 3 | alg);
        let b4 = (u8::from(ch.pan.0) << 7) | (u8::from(ch.pan.1) << 6) | (fm.lfo2 & 3) << 4 | (fm.lfo & 7);
        g.set(hi, 0xB4 + o, b4);
        // A note: its key released, its frequency, its key on.
        if ch.fresh && g.keyed[k] != 0 {
            g.write(false, key, sel);
            g.keyed[k] = 0;
        }
        if let Some(p) = pitch {
            let (blk, fnum) = fm_freq(p.clamp(0, 96 * 128));
            g.freq(hi, 0xA4 + o, 0xA0 + o, blk << 3 | (fnum >> 8) as u8, fnum as u8);
        }
        if ch.on && ch.fresh && !dac {
            g.write(false, key, 0xF0 | sel);
            g.keyed[k] = 0xF;
        } else if !ch.on && g.keyed[k] != 0 {
            g.write(false, key, sel);
            g.keyed[k] = 0;
        }
    }

    /// A YM2413 channel (0-8). Operator 1's DT2 is the instrument's voice:
    /// 0 the custom voice (one for the chip, written when a note or an
    /// effect needs it), 1-15 the chip's own, 16 the drum kit (FM 7 to 9 in
    /// the rhythm mode, the note choosing the drum). Operator 2's TL is the
    /// volume's floor.
    fn output_opll(&mut self, c: usize, k: usize) {
        let pitch = self.pitch(c);
        let Chips::SmsOpll(g) = &mut self.chips else { return };
        let ch = &mut self.ch[c];
        let Some(fm) = self.m.instruments.get(ch.instrument).and_then(|i| i.fm.as_ref()) else { return };
        let preset = fm.ops[0].dt2;
        let tl2 = i32::from(ch.fm_tl[1].unwrap_or(fm.ops[1].tl) & 15);
        let vol = ch.out_vol.clamp(0, 15);
        if preset >= 16 {
            // (The rhythm mode's set by `kit_start`.)
            if k < 6 || g.regs[0x0E] & 0x20 == 0 {
                return;
            }
            if ch.fresh && ch.on {
                // C the bass drum, C# the snare, D the tom, D# the top cymbal, E the hi-hat.
                ch.drum = [0x10, 0x08, 0x04, 0x02, 0x01].get(ch.note.unwrap_or(0).rem_euclid(12) as usize).copied().unwrap_or(0);
            }
            let d = ch.drum;
            if ch.note.is_some() {
                // Each drum its own volume: the drum's TL less the volume's
                // distance from the top (F to B's on the hi-hat's), written
                // when the channel strikes or its volume changes (another
                // channel's may be the drum's now).
                let att = (15 - vol + tl2).clamp(0, 15) as u8;
                let (r, high) = match d {
                    0x10 => (0x36, false),
                    0x08 => (0x37, false),
                    0x04 => (0x38, true),
                    0x02 => (0x38, false),
                    _ => (0x37, true),
                };
                if (ch.fresh && ch.on) || att != ch.drum_att {
                    ch.drum_att = att;
                    let old = g.regs[usize::from(r)];
                    g.set(r, if high { (old & 0x0F) | att << 4 } else { (old & 0xF0) | att });
                }
            }
            if let Some(out) = DRUM_BITS.iter().position(|&b| b == d) {
                if ch.fresh && ch.on {
                    let rhythm = g.regs[0x0E];
                    if rhythm & d != 0 {
                        g.write(0x0E, rhythm & !d);
                    }
                    g.write(0x0E, (rhythm & 0x3F) | d | 0x20);
                    g.drum_ch[out] = c;
                }
            }
            // A note off releases the channel's drum, and writes the
            // channel's block (the note's) over the drums' pitch, unkeyed.
            if ch.fresh && ch.on {
                g.keyed[k] = true;
            } else if !ch.on && g.keyed[k] {
                g.keyed[k] = false;
                let rhythm = g.regs[0x0E];
                if rhythm & d != 0 {
                    g.write(0x0E, rhythm & !d);
                }
                if let Some(p) = pitch {
                    let (blk, fnum) = opll_freq(p);
                    g.write(0x20 + k as u8, blk << 1 | (fnum >> 8) as u8);
                }
            }
            return;
        }
        // Nothing's written until the channel's first note.
        if !g.keyed[k] && g.regs[0x20 + k] == 0 && !ch.fresh {
            return;
        }
        let preset = preset & 15;
        if (ch.fresh && preset == 0) || ch.voice_fx {
            ch.voice_fx = false;
            let o = &fm.ops;
            let r0 = |i: usize| (o[i].am & 1) << 7 | (o[i].dt & 1) << 6 | (o[i].ssg >> 3 & 1) << 5 | (o[i].rs & 1) << 4 | (ch.fm_mult[i].unwrap_or(o[i].mult) & 15);
            let r4 = |i: usize| (ch.fm_ar[i].unwrap_or(o[i].ar) & 15) << 4 | (o[i].dr & 15);
            let voice = [
                r0(0),
                r0(1),
                (o[0].d2r & 3) << 6 | (ch.fm_tl[0].unwrap_or(o[0].tl) & 63),
                (o[1].d2r & 3) << 6 | u8::from(fm.lfo != 0) << 4 | u8::from(fm.lfo2 != 0) << 3 | (ch.fm_fb.unwrap_or(fm.fb) & 7),
                r4(0),
                r4(1),
                (o[0].sl & 15) << 4 | (o[0].rr & 15),
                (o[1].sl & 15) << 4 | (o[1].rr & 15),
            ];
            for (r, v) in voice.into_iter().enumerate() {
                g.set(r as u8, v);
            }
        }
        let (kr, fr, vr) = (0x20 + k as u8, 0x10 + k as u8, 0x30 + k as u8);
        g.set(vr, preset << 4 | (15 - vol).max(tl2) as u8);
        // A note: its key released, its frequency, its key on (the
        // algorithm's bit the sustain).
        if ch.fresh && ch.on && g.keyed[k] {
            g.write(kr, g.regs[usize::from(kr)] & !0x10);
            g.keyed[k] = false;
        }
        let sus = u8::from(fm.alg != 0) << 5;
        if let Some(p) = pitch {
            let (blk, fnum) = opll_freq(p);
            g.set(fr, fnum as u8);
            g.set(kr, sus | u8::from(g.keyed[k]) << 4 | blk << 1 | (fnum >> 8) as u8);
        }
        if ch.on && ch.fresh {
            g.write(kr, g.regs[usize::from(kr)] | 0x10);
            g.keyed[k] = true;
        } else if !ch.on && g.keyed[k] {
            g.write(kr, g.regs[usize::from(kr)] & !0x10);
            g.keyed[k] = false;
        }
    }

    /// One of FM 3's operators, in its extended mode (each its own pitch).
    /// The four channels are the chip's operators in its own order (O1 to
    /// O4: registers +0, +8, +4, +12, DefleMask's operators 1, 3, 2, 4);
    /// each loads its operator from its instrument, and the algorithm,
    /// feedback and panning.
    fn output_op(&mut self, c: usize, k: usize) {
        const REG: [u8; 4] = [0, 8, 4, 12];
        const OP: [usize; 4] = [0, 2, 1, 3];
        const FREQ: [(u8, u8); 4] = [(0xAD, 0xA9), (0xAE, 0xAA), (0xAC, 0xA8), (0xA6, 0xA2)];
        const KEY: [u8; 4] = [0x10, 0x20, 0x40, 0x80];
        let pitch = self.pitch(c);
        let Chips::Genesis(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        let Some(fm) = self.m.instruments.get(ch.instrument).and_then(|i| i.fm.as_ref()) else { return };
        if g.ext_loaded[k].is_none() && !ch.fresh {
            return;
        }
        let r = REG[k] + 2;
        let op = &fm.ops[OP[k]];
        if ch.fresh && g.ext_loaded[k] != Some(ch.instrument) {
            g.ext_loaded[k] = Some(ch.instrument);
            let dt = if op.dt >= 3 { op.dt - 3 } else { 4 + (3 - op.dt) } & 7;
            g.write(false, 0x30 + r, dt << 4 | (op.mult & 15));
            g.write(false, 0x50 + r, (op.rs & 3) << 6 | (op.ar & 31));
            g.write(false, 0x60 + r, (op.am & 1) << 7 | (op.dr & 31));
            g.write(false, 0x70 + r, op.d2r & 31);
            g.write(false, 0x80 + r, (op.sl & 15) << 4 | (op.rr & 15));
            g.write(false, 0x90 + r, op.ssg & 15);
        }
        // Each operator's channel's volume takes its operator down, carrier or not.
        let tl = u32::from(ch.fm_tl[OP[k]].unwrap_or(op.tl) & 0x7F);
        let tl = 127 - (127 - tl) * ch.vol.clamp(0, 127) as u32 / 127;
        g.set(false, 0x40 + r, tl as u8);
        {
            g.set(false, 0xB2, (ch.fm_fb.unwrap_or(fm.fb) & 7) << 3 | (fm.alg & 7));
            let b4 = (u8::from(ch.pan.0) << 7) | (u8::from(ch.pan.1) << 6) | (fm.lfo2 & 3) << 4 | (fm.lfo & 7);
            g.set(false, 0xB6, b4);
        }
        let bit = KEY[k];
        let mut keys = g.keyed[2] << 4;
        if ch.fresh && keys & bit != 0 {
            keys &= !bit;
            g.write(false, 0x28, keys | 2);
        }
        if let Some(p) = pitch {
            let (blk, fnum) = fm_freq(p.clamp(0, 96 * 128));
            let (hr, lr) = FREQ[k];
            g.freq(false, hr, lr, blk << 3 | (fnum >> 8) as u8, fnum as u8);
        }
        if ch.on && ch.fresh {
            keys |= bit;
            g.write(false, 0x28, keys | 2);
        } else if !ch.on && keys & bit != 0 {
            keys &= !bit;
            g.write(false, 0x28, keys | 2);
        }
        g.keyed[2] = keys >> 4;
    }

    /// An arcade channel: one of the YM2151's eight (as the Genesis's FM,
    /// in the OPM's registers) or a PCM channel.
    fn output_arcade(&mut self, c: usize) {
        let pitch = self.pitch(c);
        let Chips::Arcade(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        if c >= 8 {
            let k = c - 8;
            if ch.fresh {
                let s = ch.note.unwrap_or(0).rem_euclid(12) as usize + 12 * self.sample_bank;
                g.pcm[k] = (s < g.samples.len()).then(|| DacPlay {
                    sample: s,
                    pos: 0.0,
                    step: ch.pcm_rate.unwrap_or(g.samples[s].1),
                });
            }
            if !ch.on {
                g.pcm[k] = None;
            }
            let v = ch.vol.clamp(0, 127) as f32 / 127.0;
            let bal = ch.balance.unwrap_or(0xFF);
            g.pcm_level[k] = (v * f32::from(bal >> 4) / 15.0, v * f32::from(bal & 15) / 15.0);
            return;
        }
        let Some(fm) = self.m.instruments.get(ch.instrument).and_then(|i| i.fm.as_ref()) else { return };
        if g.loaded[c].is_none() && !ch.fresh {
            return;
        }
        let o = c as u8;
        if ch.fresh && g.loaded[c] != Some(ch.instrument) {
            g.loaded[c] = Some(ch.instrument);
            for (i, op) in fm.ops.iter().enumerate() {
                let r = o + 8 * i as u8;
                g.opm.write(0x80 + r, (op.rs & 3) << 6 | (op.ar & 31));
                g.opm.write(0xA0 + r, (op.am & 1) << 7 | (op.dr & 31));
                g.opm.write(0xC0 + r, (op.dt2 & 3) << 6 | (op.d2r & 31));
                g.opm.write(0xE0 + r, (op.sl & 15) << 4 | (op.rr & 15));
            }
        }
        let alg = fm.alg & 7;
        let vol = ch.vol.clamp(0, 127) as u32;
        for (i, op) in fm.ops.iter().enumerate() {
            let r = o + 8 * i as u8;
            let tl = u32::from(ch.fm_tl[i].unwrap_or(op.tl) & 0x7F);
            let tl = if carriers(alg).contains(&i) || self.m.version < 18 { 127 - (127 - tl) * vol / 127 } else { tl };
            g.opm.write(0x60 + r, tl as u8);
            let dt = if op.dt >= 3 { op.dt - 3 } else { 4 + (3 - op.dt) } & 7;
            g.opm.write(0x40 + r, dt << 4 | (ch.fm_mult[i].unwrap_or(op.mult) & 15));
            g.opm.write(0x80 + r, (op.rs & 3) << 6 | (ch.fm_ar[i].unwrap_or(op.ar) & 31));
        }
        let rl = if ch.pan.0 { 0x40 } else { 0 } | if ch.pan.1 { 0x80 } else { 0 };
        g.opm.write(0x20 + o, rl | (ch.fm_fb.unwrap_or(fm.fb) & 7) << 3 | alg);
        g.opm.write(0x38 + o, (fm.lfo & 7) << 4 | (fm.lfo2 & 3));
        if ch.fresh && g.keyed[c] {
            g.opm.write(8, o);
            g.keyed[c] = false;
        }
        if let Some(p) = pitch {
            let (kc, kf) = opm_kc(p);
            g.opm.write(0x28 + o, kc);
            g.opm.write(0x30 + o, kf << 2);
        }
        if ch.on && ch.fresh {
            g.opm.write(8, 0x78 | o);
            g.keyed[c] = true;
        } else if !ch.on && g.keyed[c] {
            g.opm.write(8, o);
            g.keyed[c] = false;
        }
    }

    /// A C64 voice. Its instrument sets the waveform, ADSR, pulse width,
    /// ring and sync and (when it says so) the shared filter; its wave
    /// macro the waveform, its duty macro the pulse width (in percent);
    /// the volume column and macro are the chip's volume, which all share.
    fn output_c64(&mut self, c: usize) {
        let pitch = self.pitch(c);
        let Chips::C64(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        let ins = self.m.instruments.get(ch.instrument).cloned().unwrap_or_default();
        let b = |i: usize| ins.c64.get(i).copied().unwrap_or(0);
        let base = (c * 7) as u8;
        if ch.fresh {
            g.wave[c] = (0..4).fold(0, |a, k| a | u8::from(b(k) != 0) << k);
            g.ad[c] = (b(4) & 15) << 4 | (b(5) & 15);
            g.sr[c] = (b(6) & 15) << 4 | (b(7) & 15);
            g.pw[c] = (u32::from(b(8).min(100)) * 4095 / 100).max(1) as u16;
            g.ring_sync[c] = if b(9) != 0 { 4 } else { 0 } | if b(10) != 0 { 2 } else { 0 };
            g.filt[c] = b(11) != 0;
            if b(13) != 0 {
                g.resonance = b(14) & 15;
                g.cutoff = (u32::from(b(15).min(100)) * 2047 / 100).min(2047) as u16;
                g.mode = (g.mode & 8) | if b(16) != 0 { 4 } else { 0 } | if b(17) != 0 { 1 } else { 0 } | if b(18) != 0 { 2 } else { 0 } | if b(19) != 0 { 8 } else { 0 };
            }
            g.started[c] = true;
        }
        if !g.started[c] {
            return;
        }
        if let Some(w) = ch.wave_now {
            g.wave[c] = (w & 15) as u8;
        }
        if let Some(d) = ch.duty_now {
            g.pw[c] = (d.clamp(0, 100) * 4095 / 100) as u16;
        }
        if let Some(p) = pitch {
            let f = 440.0 * 2f64.powf((f64::from(p) / 128.0 - 57.0) / 12.0);
            let reg = (f * 16_777_216.0 / C64_CLOCK).round().clamp(0.0, 65535.0) as u16;
            g.sid.write(base, reg as u8);
            g.sid.write(base + 1, (reg >> 8) as u8);
        }
        g.sid.write(base + 2, g.pw[c] as u8);
        g.sid.write(base + 3, (g.pw[c] >> 8) as u8);
        // A note: its ADSR, and the gate off and on again (unless 1Axx keeps it).
        if ch.fresh && !(g.no_reset[c] && g.gate[c]) {
            g.sid.write(base + 4, g.wave[c] << 4 | g.ring_sync[c]);
            g.sid.write(base + 5, g.ad[c]);
            g.sid.write(base + 6, g.sr[c]);
        }
        let gate = ch.on;
        g.sid.write(base + 4, g.wave[c] << 4 | g.ring_sync[c] | u8::from(gate));
        g.gate[c] = gate;
        // The filter and volume, shared: the volume the last voice's.
        // (With no filter mode set, nothing goes through the filter, which
        // would silence it.)
        let route = if g.mode & 7 == 0 { 0 } else { (0..3).fold(0u8, |a, k| a | if g.filt[k] { 1 << k } else { 0 }) };
        g.sid.write(21, (g.cutoff & 7) as u8);
        g.sid.write(22, (g.cutoff >> 3) as u8);
        g.sid.write(23, g.resonance << 4 | route);
        // An instrument whose volume macro is the filter's cutoff (its byte
        // 12) leaves the volume to the column.
        let vol = if b(12) != 0 {
            if let Some(m) = ch.vol_mac_now {
                g.cutoff = (m.clamp(0, 100) * 2047 / 100) as u16;
                g.sid.write(21, (g.cutoff & 7) as u8);
                g.sid.write(22, (g.cutoff >> 3) as u8);
            }
            ch.vol.clamp(0, 15) as u8
        } else {
            ch.out_vol.clamp(0, 15) as u8
        };
        g.sid.write(24, (g.mode & 15) << 4 | vol);
    }

    /// A PC Engine channel.
    fn output_pce(&mut self, c: usize) {
        let pitch = self.pitch(c);
        let Chips::PcEngine(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        if ch.note.is_none() {
            return;
        }
        g.psg.write(0, c as u8);
        // The wave: 10xx's, else the macro's (whose 63 and 62 turn the noise on and off).
        let mut noise = ch.noise_narrow;
        // (Without one, wave 0.)
        let mut wave = ch.wave_fx.or(Some(0));
        if let Some(w) = ch.wave_now {
            match w {
                63 => noise = true,
                62 => noise = false,
                _ if ch.wave_fx.is_none() || ch.fresh => wave = Some(w.max(0) as usize),
                _ => {}
            }
        }
        let vol = if ch.on { ch.out_vol.clamp(0, 31) as u8 } else { 0 };
        // 17xx: the channel plays samples, in DDA mode, its notes choosing them.
        if ch.dac {
            if ch.fresh {
                let s = ch.note.unwrap_or(0).rem_euclid(12) as usize + 12 * self.sample_bank;
                g.dda[c] = (s < g.samples.len()).then(|| DacPlay {
                    sample: s,
                    pos: 0.0,
                    step: g.samples[s].1,
                });
            }
            if !ch.on {
                g.dda[c] = None;
            }
            // (Furnace leaves such a channel at full volume, its period C-4's.)
            let per = pce_period(48 * 128);
            g.psg.write(2, per as u8);
            g.psg.write(3, (per >> 8) as u8);
            g.psg.write(4, if ch.on { 0xDF } else { 0 });
            g.psg.write(5, ch.balance.unwrap_or(0xFF));
            return;
        }
        g.dda[c] = None;
        let ctrl = if ch.on { 0x80 | vol } else { 0 };
        if let Some(w) = wave.filter(|&w| g.wave[c] != Some(w)) {
            if let Some(data) = self.m.waves.get(w) {
                g.psg.write(4, 0x40 | (ctrl & 0x1F));
                g.psg.write(4, ctrl & 0x1F);
                for k in 0..32 {
                    g.psg.write(6, (data.get(k).copied().unwrap_or(0) & 31) as u8);
                }
                g.wave[c] = Some(w);
            }
        }
        if let Some(p) = pitch {
            let per = pce_period(p);
            g.psg.write(2, per as u8);
            g.psg.write(3, (per >> 8) as u8);
        }
        g.psg.write(4, ctrl);
        g.psg.write(5, ch.balance.unwrap_or(0xFF));
        if c >= 4 {
            let reg = if noise { 0x80 | pitch.map_or(0, |p| pce_noise(p.div_euclid(128))) } else { 0 };
            g.psg.write(7, reg);
            g.noise[c] = noise;
        }
    }

    /// A Game Boy channel.
    fn output_gb(&mut self, c: usize) {
        let pitch = self.pitch(c);
        let Chips::GameBoy(g) = &mut self.chips else { return };
        let ch = &self.ch[c];
        let ins = self.m.instruments.get(ch.instrument).cloned().unwrap_or_default();
        let [env_vol, env_up, env_len, sound_len] = ins.gb;
        let len_on = sound_len < 64;
        let len = if len_on { 63 - sound_len.min(63) } else { 63 };
        // 8xy: NR51's bits for the channel (left 4 up, right the low 4).
        let pan = (u8::from(ch.pan.0) << (4 + c)) | (u8::from(ch.pan.1) << c);
        let nr51 = (g.gb.reg(0x15) & !(0x11 << c)) | pan;
        if nr51 != g.gb.reg(0x15) {
            g.gb.write(0x15, nr51);
        }
        if !ch.fresh && !g.started[c] {
            return;
        }
        // The note's volume: the column's when it has one, the instrument's
        // otherwise. Before version 18 instruments had a volume macro instead
        // of the envelope: its steps written as a steady envelope (going up,
        // which holds), the note not triggered again.
        let old = self.m.version < 18;
        let nrx2 = if old {
            // The macro's steps times the column (15 unless set), out of 15;
            // without a macro the column's, else nothing.
            let v = match ch.vol_mac_now {
                Some(m) => m.clamp(0, 15) * ch.vol.clamp(0, 15) / 15,
                None if ch.vol_set => ch.vol.clamp(0, 15),
                None => 0,
            } as u8;
            v << 4 | 8
        } else {
            let v = if ch.vol_set { ch.vol as u8 } else { env_vol } & 15;
            v << 4 | if env_up != 0 { 8 } else { 0 } | (env_len & 7)
        };
        match c {
            0 | 1 => {
                let b = if c == 0 { 0x00 } else { 0x05 };
                let duty = (ch.duty_now.unwrap_or(ch.duty_fx) & 3) as u8;
                let per = pitch.map(gb_period).unwrap_or(0);
                if ch.fresh {
                    if c == 0 {
                        g.gb.write(0x00, ch.sweep);
                    }
                    g.gb.write(b + 1, duty << 6 | len);
                    g.gb.write(b + 2, if ch.on { nrx2 } else { 8 });
                    g.gb.write(b + 3, per as u8);
                    g.gb.write(b + 4, 0x80 | if len_on { 0x40 } else { 0 } | (per >> 8) as u8);
                    g.started[c] = true;
                    self.trig[c] += 1;
                } else {
                    if g.gb.reg(b + 1) >> 6 != duty {
                        g.gb.write(b + 1, duty << 6 | len);
                    }
                    if old && ch.on && g.gb.reg(b + 2) != nrx2 {
                        g.gb.write(b + 2, nrx2);
                    }
                    // A note off: silent, the DAC left on (NRx2 8), triggered.
                    if !ch.on && g.gb.reg(b + 2) != 8 {
                        g.gb.write(b + 2, 8);
                        g.gb.write(b + 4, 0x80 | (g.gb.reg(b + 4) & 0x47));
                        self.trig[c] += 1;
                    } else if ch.on && (g.gb.reg(b + 3) != per as u8 || g.gb.reg(b + 4) & 7 != (per >> 8) as u8) {
                        g.gb.write(b + 3, per as u8);
                        g.gb.write(b + 4, if len_on { 0x40 } else { 0 } | (per >> 8) as u8);
                    }
                }
            }
            2 => {
                let wave = ch.wave_fx.or(ch.wave_now.map(|w| w.max(0) as usize));
                // The wave channel's volume (0-15) in quarters: off, 25%, 50%,
                // full; before version 18 its column was 0-3 itself.
                let wv = if old {
                    // (Its macro's steps, 0-15, times the 0-3 column, in quarters.)
                    let col = ch.vol.clamp(0, 3);
                    match ch.vol_mac_now {
                        Some(m) => ((m.clamp(0, 15) * col / 3) >> 2) as usize,
                        None => col as usize,
                    }
                } else {
                    (ch.out_vol.clamp(0, 15) >> 2) as usize
                };
                let nr32 = [0u8, 0x60, 0x40, 0x20][if ch.on { wv } else { 0 }];
                let per = pitch.map(gb_period).unwrap_or(0);
                if ch.fresh {
                    if let Some(w) = wave.filter(|&w| g.wave != Some(w)) {
                        if let Some(data) = self.m.waves.get(w) {
                            g.gb.write(0x0A, 0);
                            for k in 0..16 {
                                let at = |i: usize| 15 - (data.get(i).copied().unwrap_or(0) & 15) as u8;
                                g.gb.write(0x20 + k as u8, at(2 * k) << 4 | at(2 * k + 1));
                            }
                            g.wave = Some(w);
                        }
                    }
                    g.gb.write(0x0A, 0x80);
                    g.gb.write(0x0C, nr32);
                    g.gb.write(0x0D, per as u8);
                    g.gb.write(0x0E, 0x80 | if len_on { 0x40 } else { 0 } | (per >> 8) as u8);
                    g.gb.write(0x0B, len);
                    g.started[2] = true;
                    self.trig[2] += 1;
                } else {
                    if g.gb.reg(0x0C) != nr32 {
                        g.gb.write(0x0C, nr32);
                    }
                    if g.gb.reg(0x0D) != per as u8 || g.gb.reg(0x0E) & 7 != (per >> 8) as u8 {
                        g.gb.write(0x0D, per as u8);
                        g.gb.write(0x0E, if len_on { 0x40 } else { 0 } | (per >> 8) as u8);
                    }
                }
            }
            _ => {
                // The duty macro's any value but 0 (or 11xx) makes the noise 7-bit.
                let narrow = match ch.duty_now {
                    Some(d) => d != 0,
                    None => ch.noise_narrow,
                };
                // A note's noise counts from C-2 (a fixed arpeggio's value is the
                // step itself); vibrato doesn't move it, slides as above.
                let arp = match ch.arp_step {
                    1 if !(ch.fresh && ch.arp_hold) => i32::from(ch.arp >> 4),
                    2 if !(ch.fresh && ch.arp_hold) => i32::from(ch.arp & 15),
                    _ => 0,
                };
                // (Version 17's noise notes are two octaves below later versions'.)
                let step = if ch.arp_fixed { Some(ch.arp_now + 24) } else { ch.note.map(|n| n + ch.arp_now + arp) }.map(|n| n + ch.noise_slide - if old { 24 } else { 0 });
                let nr43 = step.map(gb_noise).unwrap_or(0) | if narrow { 8 } else { 0 };
                if ch.fresh {
                    g.gb.write(0x10, len);
                    g.gb.write(0x11, if ch.on { nrx2 } else { 8 });
                    g.gb.write(0x12, nr43);
                    g.gb.write(0x13, 0x80 | if len_on { 0x40 } else { 0 });
                    g.started[3] = true;
                    self.trig[3] += 1;
                } else if !ch.on && g.gb.reg(0x11) != 8 {
                    g.gb.write(0x11, 8);
                    g.gb.write(0x13, 0x80);
                    self.trig[3] += 1;
                } else {
                    if old && ch.on && g.gb.reg(0x11) != nrx2 {
                        g.gb.write(0x11, nrx2);
                    }
                    if g.gb.reg(0x12) != nr43 {
                        g.gb.write(0x12, nr43);
                    }
                }
            }
        }
    }

    /// The chips' registers as text, as the probe scripts print Furnace's.
    #[cfg(test)]
    fn state(&self) -> String {
        match &self.chips {
            Chips::Sms(sn) => sn.state(),
            Chips::SmsOpll(g) => g.state(),
            Chips::Nes(n) => n.state(),
            Chips::Genesis(g) => g.state(),
            Chips::GameBoy(g) => g.state(&self.trig),
            Chips::PcEngine(g) => g.psg.state(),
            Chips::C64(g) => format!("{:?}", &g.sid.regs[..25]),
            Chips::Arcade(g) => {
                let r = |a: u8| g.opm.fm.regs.read(a);
                (0..8u8)
                    .map(|c| {
                        format!(
                            "{:02x}/{:02x}/{:x}/{}/{:02x}",
                            r(0x28 + c),
                            r(0x30 + c) >> 2,
                            if g.keyed[usize::from(c)] { 15 } else { 0 },
                            (0..4u8).map(|k| (r(0x60 + c + 8 * k) & 0x7F).to_string()).collect::<Vec<_>>().join("."),
                            r(0x20 + c)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }
}

/// An FM pitch's block and F-number: DefleMask's notes (A-3 at 440 Hz)
/// kept as one linear number (an octave's F-numbers times 2^block),
/// straight lines between notes, the block the lower note's octave.
fn fm_freq(pitch: i32) -> (u8, u16) {
    let lin = |n: i32| (440.0 * 2f64.powf(f64::from(n - 45) / 12.0) * f64::from(1 << 20) / Ym2612::rate(YM2612_CLOCK)).round();
    let n = pitch.div_euclid(128);
    let fr = f64::from(pitch.rem_euclid(128)) / 128.0;
    let (a, b) = (lin(n), lin(n + 1));
    let f = (a + ((b - a) * fr).floor()) as i64;
    let blk = n.div_euclid(12).clamp(0, 7);
    ((blk as u8), ((f >> blk) as u16).min(0x7FF))
}

/// The YM2413's F-numbers for each octave's notes (at the octave's block),
/// as DefleMask's modules play them; 345 is the next C.
const OPLL_FNUM: [[u16; 12]; 4] = [
    [173, 183, 194, 205, 217, 230, 244, 258, 274, 290, 307, 326],
    [172, 183, 193, 205, 217, 230, 244, 258, 274, 290, 307, 325],
    [172, 182, 193, 205, 217, 230, 244, 258, 273, 290, 307, 325],
    [172, 182, 193, 205, 217, 230, 243, 258, 273, 290, 307, 325],
];

/// A YM2413 pitch's block and F-number: the block the octave, between
/// notes a straight line between theirs, C-8 the highest.
fn opll_freq(pitch: i32) -> (u8, u16) {
    let pitch = pitch.clamp(0, 96 * 128);
    let n = pitch.div_euclid(128);
    if n >= 96 {
        return (7, 345);
    }
    let (o, k) = ((n / 12) as usize, (n % 12) as usize);
    let t = &OPLL_FNUM[[0, 1, 2, 2, 3, 3, 3, 3][o]];
    let (a, b) = (i32::from(t[k]), if k == 11 { 345 } else { i32::from(t[k + 1]) });
    let fr = pitch.rem_euclid(128);
    (o as u8, (a + (b - a) * fr / 128) as u16)
}

/// A NES pitch's frequency: A of octave 3 (note 45) at 440 Hz.
fn nes_hz(pitch: i32) -> f64 {
    440.0 * 2f64.powf((f64::from(pitch) / 128.0 - 45.0) / 12.0)
}

/// A pulse's (and the triangle's) period: between notes, a straight line
/// between theirs.
fn nes_period(pitch: i32) -> u16 {
    let at = |n: i32| (crate::nes::NTSC_HZ / 16.0 / nes_hz(n * 128) - 1.0).round().clamp(0.0, 2047.0);
    let n = pitch.div_euclid(128);
    let fr = f64::from(pitch.rem_euclid(128)) / 128.0;
    let (a, b) = (at(n), at(n + 1));
    (a + ((b - a) * fr).floor()).clamp(0.0, 2047.0) as u16
}

/// The noise's period index for a note: 15 for C, down a step a semitone
/// to 4 for B, except that from octave 3 the odd octaves' C to D# are 3 to
/// 0 (as DefleMask's modules play).
fn nes_noise(note: i32) -> u8 {
    let (o, k) = (note.div_euclid(12), note.rem_euclid(12));
    if o >= 3 && o % 2 == 1 && k < 4 {
        (3 - k) as u8
    } else {
        (15 - k) as u8
    }
}

/// A pitch's frequency (A of octave 3, note 45, at 880 Hz as DefleMask's
/// PSG plays it).
fn hz(pitch: i32) -> f64 {
    440.0 * 2f64.powf((f64::from(pitch) / 128.0 - 33.0) / 12.0)
}

/// A note's tone period. DefleMask's periods are a little longer than the
/// clock gives (each of its notes' is the clock times 1.00023 to 1.00043).
fn sn_note_period(note: i32) -> f64 {
    (PSG_CLOCK * 1.00033 / 32.0 / hz(note * 128)).round().min(1023.0)
}

/// The Genesis's PSG period: as the Master System's to C#7, then down a
/// period a semitone (97 less the note) until `top` (F#7 for a tone
/// channel, A#7 for the noise's), and 1 from there.
fn gen_psg_period(pitch: i32, top: i32) -> u16 {
    match pitch.div_euclid(128) {
        n if n >= top => 1,
        n if n >= 85 => (97 - n) as u16,
        _ => sn_period(pitch),
    }
}

/// A pitch's tone period: between notes, a straight line between theirs.
fn sn_period(pitch: i32) -> u16 {
    let n = pitch.div_euclid(128);
    let fr = f64::from(pitch.rem_euclid(128)) / 128.0;
    let (a, b) = (sn_note_period(n), sn_note_period(n + 1));
    (a + ((b - a) * fr).floor()).clamp(1.0, 1023.0) as u16
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.ch.len()
    }

    fn tick(&mut self) {
        self.tick_once();
    }

    fn tick_seconds(&self) -> f64 {
        1.0 / self.m.hz()
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for b in self.bufs.iter_mut() {
            b.clear();
            b.resize(frames, 0.0);
        }
        match &mut self.chips {
            Chips::Sms(sn) => {
                let [a, b, c, d, ..] = &mut self.bufs[..] else { return };
                sn.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..]]);
                for ch in 0..4 {
                    for i in 0..frames {
                        let s = self.bufs[ch][i] * 0.2;
                        out.left[ch][i] = s;
                        out.right[ch][i] = s;
                    }
                }
            }
            Chips::SmsOpll(g) => {
                if g.rate != rate {
                    // The YM2413 made again at the new rate, its registers as they were.
                    g.opll = Opll::new(crate::opll::CLOCK, rate);
                    g.rate = rate;
                    let regs = g.regs;
                    for (r, &v) in regs.iter().enumerate() {
                        g.opll.write(r, v);
                    }
                }
                {
                    let [a, b, c, d, ..] = &mut self.bufs[..] else { return };
                    g.sn.render(rate, &mut [&mut a[..frames], &mut b[..frames], &mut c[..frames], &mut d[..frames]]);
                }
                for ch in 0..4 {
                    for i in 0..frames {
                        let s = self.bufs[ch][i] * 0.2;
                        out.left[ch][i] = s;
                        out.right[ch][i] = s;
                    }
                }
                for ch in 4..13 {
                    out.left[ch][..frames].fill(0.0);
                    out.right[ch][..frames].fill(0.0);
                }
                for i in 0..frames {
                    let v = g.opll.calc_channels();
                    for k in 0..9 {
                        let s = f32::from(v[k]) * OPLL_LEVEL;
                        out.left[4 + k][i] = s;
                        out.right[4 + k][i] = s;
                    }
                    for (d, &c) in g.drum_ch.iter().enumerate() {
                        let s = f32::from(v[9 + d]) * OPLL_LEVEL;
                        out.left[c][i] += s;
                        out.right[c][i] += s;
                    }
                }
            }
            Chips::Genesis(g) => {
                let ext3 = matches!(self.m.system, System::Genesis { ext3: true });
                // The PSG into its four channels' buffers.
                {
                    let [a, b, c, d, ..] = &mut self.bufs[..] else { return };
                    g.sn.render(rate, &mut [&mut a[..frames], &mut b[..frames], &mut c[..frames], &mut d[..frames]]);
                }
                let per = Ym2612::rate(YM2612_CLOCK) / f64::from(rate.max(1));
                let mut o = [[0i32; 2]; 6];
                // The YM2612's output (each FM channel 1/6 of the chip's full scale).
                let level = 128.0 * 64.0 / (6.0 * 65.0) / 32768.0 * 2.4;
                let mut fm = vec![[0f32; 2]; 6 * frames];
                for i in 0..frames {
                    g.pos += per;
                    let mut n = 0;
                    let mut acc = [[0f32; 2]; 6];
                    while g.pos >= 1.0 {
                        g.pos -= 1.0;
                        if let Some(d) = &mut g.dac {
                            let data = &g.dac_samples[d.sample].0;
                            match data.get(d.pos as usize) {
                                Some(&v) => {
                                    g.ym.write(false, 0x2A, v);
                                    d.pos += d.step;
                                }
                                None => g.dac = None,
                            }
                        }
                        g.ym.generate(&mut o);
                        for c in 0..6 {
                            acc[c][0] += o[c][0] as f32;
                            acc[c][1] += o[c][1] as f32;
                        }
                        n += 1;
                    }
                    let k = level / n.max(1) as f32;
                    for c in 0..6 {
                        fm[i * 6 + c] = [acc[c][0] * k, acc[c][1] * k];
                    }
                }
                for c in 0..self.ch.len() {
                    match gen_ch(ext3, c) {
                        GenCh::Fm(f) | GenCh::Op(f @ 0) => {
                            let f = if matches!(gen_ch(ext3, c), GenCh::Op(_)) { 2 } else { f };
                            for i in 0..frames {
                                out.left[c][i] = fm[i * 6 + f][0];
                                out.right[c][i] = fm[i * 6 + f][1];
                            }
                        }
                        GenCh::Op(_) => {}
                        GenCh::Psg(k) => {
                            for i in 0..frames {
                                let s = self.bufs[k][i] * 0.12;
                                out.left[c][i] = s;
                                out.right[c][i] = s;
                            }
                        }
                    }
                }
            }
            Chips::Arcade(g) => {
                let per = Ym2151::rate(OPM_CLOCK) / f64::from(rate.max(1));
                let mut o = [[0i32; 2]; 8];
                let level = 1.0 / 32768.0 * 0.5;
                for i in 0..frames {
                    g.pos += per;
                    let mut n = 0;
                    let mut acc = [[0f32; 2]; 8];
                    while g.pos >= 1.0 {
                        g.pos -= 1.0;
                        g.opm.generate(&mut o);
                        for c in 0..8 {
                            acc[c][0] += o[c][0] as f32;
                            acc[c][1] += o[c][1] as f32;
                        }
                        n += 1;
                    }
                    let k = level / n.max(1) as f32;
                    for c in 0..8 {
                        out.left[c][i] = acc[c][0] * k;
                        out.right[c][i] = acc[c][1] * k;
                    }
                    for p in 0..5 {
                        let mut v = 0.0;
                        if let Some(d) = &mut g.pcm[p] {
                            let data = &g.samples[d.sample].0;
                            match data.get(d.pos as usize) {
                                Some(&s) => {
                                    v = (f32::from(s) - 128.0) / 128.0 * 0.3;
                                    d.pos += d.step / f64::from(rate.max(1));
                                }
                                None => g.pcm[p] = None,
                            }
                        }
                        out.left[8 + p][i] = v * g.pcm_level[p].0;
                        out.right[8 + p][i] = v * g.pcm_level[p].1;
                    }
                }
            }
            Chips::C64(g) => {
                if rate != g.rate {
                    // The filter's made for the output rate: made again, its registers as they were.
                    let regs = g.sid.regs;
                    g.sid = Sid::new(rate);
                    for (i, &v) in regs.iter().enumerate().take(25) {
                        g.sid.write(i as u8, v);
                    }
                    g.rate = rate;
                }
                let per = C64_CLOCK / f64::from(rate.max(1));
                for i in 0..frames {
                    g.owed += per;
                    let n = g.owed as usize;
                    g.owed -= n as f64;
                    let mut acc = [0f32; 3];
                    for _ in 0..n {
                        let o = g.sid.clock();
                        for k in 0..3 {
                            acc[k] += o[k];
                        }
                    }
                    for k in 0..3 {
                        let v = if n > 0 { acc[k] / n as f32 } else { 0.0 } * 1.2;
                        out.left[k][i] = v;
                        out.right[k][i] = v;
                    }
                }
            }
            Chips::PcEngine(g) => {
                let (lo, hi) = self.bufs.split_at_mut(6);
                let [a, b, c, d, e, f] = lo else { return };
                let [h, i, j, k, l, m, ..] = hi else { return };
                if g.dda.iter().all(Option::is_none) {
                    g.psg.render(
                        rate,
                        &mut [&mut a[..frames], &mut b[..frames], &mut c[..frames], &mut d[..frames], &mut e[..frames], &mut f[..frames]],
                        &mut [&mut h[..frames], &mut i[..frames], &mut j[..frames], &mut k[..frames], &mut l[..frames], &mut m[..frames]],
                    );
                } else {
                    // Samples playing: their next value set a frame at a time.
                    for x in 0..frames {
                        for ch in 0..6 {
                            if let Some(p) = &mut g.dda[ch] {
                                let data = &g.samples[p.sample].0;
                                match data.get(p.pos as usize) {
                                    Some(&v) => {
                                        g.psg.set_dda(ch, v >> 3);
                                        p.pos += p.step / f64::from(rate.max(1));
                                    }
                                    None => g.dda[ch] = None,
                                }
                            }
                        }
                        g.psg.render(
                            rate,
                            &mut [&mut a[x..x + 1], &mut b[x..x + 1], &mut c[x..x + 1], &mut d[x..x + 1], &mut e[x..x + 1], &mut f[x..x + 1]],
                            &mut [&mut h[x..x + 1], &mut i[x..x + 1], &mut j[x..x + 1], &mut k[x..x + 1], &mut l[x..x + 1], &mut m[x..x + 1]],
                        );
                    }
                }
                for ch in 0..6 {
                    for x in 0..frames {
                        out.left[ch][x] = self.bufs[ch][x] * 0.15;
                        out.right[ch][x] = self.bufs[6 + ch][x] * 0.15;
                    }
                }
            }
            Chips::GameBoy(g) => {
                let (lo, hi) = self.bufs.split_at_mut(4);
                let [a, b, c, d] = lo else { return };
                let [e, f, h, i, ..] = hi else { return };
                g.gb.render(
                    rate,
                    &mut [&mut a[..frames], &mut b[..frames], &mut c[..frames], &mut d[..frames]],
                    &mut [&mut e[..frames], &mut f[..frames], &mut h[..frames], &mut i[..frames]],
                );
                for ch in 0..4 {
                    for k in 0..frames {
                        out.left[ch][k] = self.bufs[ch][k] * 0.12;
                        out.right[ch][k] = self.bufs[4 + ch][k] * 0.12;
                    }
                }
            }
            Chips::Nes(n) => {
                let [a, b, c, d, e, ..] = &mut self.bufs[..] else { return };
                n.apu.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..], &mut e[..]]);
                // The APU's output has its DC: taken off as FamiTracker's player does.
                let r = 1.0 - (2.0 * std::f64::consts::PI * 10.0 / f64::from(rate.max(1))) as f32;
                // The PCM channel's sample, as the DMC's 7-bit level would play it.
                if let Some(p) = &mut n.pcm {
                    let data = &n.samples[p.sample].0;
                    for i in 0..frames {
                        match data.get(p.pos as usize) {
                            Some(&v) => {
                                self.bufs[4][i] += f32::from(v >> 1) / 127.0 * 0.25;
                                p.pos += p.step / f64::from(rate.max(1));
                            }
                            None => {
                                n.pcm = None;
                                break;
                            }
                        }
                    }
                }
                for ch in 0..5 {
                    let (mut x1, mut y1) = self.dc[ch];
                    for i in 0..frames {
                        let x = self.bufs[ch][i];
                        let y = x - x1 + r * y1;
                        x1 = x;
                        y1 = y;
                        out.left[ch][i] = y * 1.7;
                        out.right[ch][i] = y * 1.7;
                    }
                    self.dc[ch] = (x1, y1);
                }
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let per_row = (self.speed1 + self.speed2) as f64 / 2.0;
        let bpm = self.m.hz() * 60.0 / (per_row * 4.0);
        (self.speed1, bpm.round().max(1.0) as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 24 module for `system` of one pattern of `rows` rows at
    /// speed 1, its rows for channel 0 given as (note, octave, volume,
    /// effect, value); no instruments but a blank one.
    fn module(system: u8, channels: usize, cells: &[(i16, i16, i16, i16, i16)]) -> Vec<u8> {
        let mut b = b".DelekDefleMask.".to_vec();
        b.extend_from_slice(&[24, system, 0, 0, 4, 16, 0, 1, 1, 1, 0, 0, 0, 0]);
        b.extend_from_slice(&(cells.len() as u32).to_le_bytes());
        b.push(1);
        b.extend(std::iter::repeat_n(0u8, channels));
        b.extend_from_slice(&[1, 1, b'i', 0]);
        if system != 4 {
            b.push(0);
        }
        b.extend_from_slice(&[0, 0, 0, 0]);
        if system == 4 {
            b.extend_from_slice(&[15, 0, 0, 64]);
        }
        b.push(0);
        for c in 0..channels {
            b.push(1);
            for k in 0..cells.len() {
                let (n, o, v, e, x) = if c == 0 { cells[k] } else { (0, 0, -1, -1, -1) };
                for w in [n, o, v, e, x, if n > 0 { 0 } else { -1 }] {
                    b.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
        b.push(0);
        b
    }

    #[test]
    fn plays_a_master_system_note_and_arpeggio() {
        // C-4 (stored as C of octave 3) with arpeggio 37.
        let d = module(3, 4, &[(12, 3, 15, 0, 0x37), (0, 0, -1, -1, -1), (0, 0, -1, -1, -1), (0, 0, -1, -1, -1)]);
        let s = Song::parse(&d).expect("read");
        let mut p = Player::new(s.module.clone());
        let mut periods = Vec::new();
        for _ in 0..4 {
            p.tick();
            let Chips::Sms(sn) = &p.chips else { panic!() };
            periods.push(sn.state().split('/').next().unwrap().to_string());
        }
        // C-4, then 3 and 7 semitones up, as Furnace plays DefleMask's.
        assert_eq!(periods, ["107", "90", "71", "107"]);
    }

    #[test]
    fn plays_the_fm_sound_units_voices_and_drums() {
        // Master System + FM: instrument 0 the chip's piano (operator 1's
        // DT2 3, operator 2's TL 2), 1 the drum kit (DT2 16). C-4 on FM 1,
        // then a bass drum and a snare on FM 7.
        let mut b = b".DelekDefleMask.".to_vec();
        b.extend_from_slice(&[24, 0x43, 0, 0, 4, 16, 0, 1, 1, 1, 0, 0, 0, 0]);
        b.extend_from_slice(&2u32.to_le_bytes());
        b.push(1);
        b.extend(std::iter::repeat_n(0u8, 13));
        b.push(2);
        for (dt2, tl2) in [(3, 2), (16, 0)] {
            b.extend_from_slice(&[1, b'i', 1, 0, 0, 0, 0]);
            for op in 0..4 {
                b.extend_from_slice(&[0, 15, 5, 1, 5, 2, if op == 1 { tl2 } else { 0 }, if op == 0 { dt2 } else { 0 }, 0, 0, 0, 0]);
            }
        }
        b.push(0);
        let rows: [[(i16, i16, i16); 2]; 13] = std::array::from_fn(|c| match c {
            4 => [(12, 3, 0), (0, 0, -1)],
            10 => [(12, 3, 1), (1, 4, 1)],
            _ => [(0, 0, -1); 2],
        });
        for row in rows {
            b.push(1);
            for (n, o, i) in row {
                for w in [n, o, -1, -1, -1, i] {
                    b.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
        b.push(0);
        let s = Song::parse(&b).expect("read");
        let mut p = Player::new(s.module.clone());
        p.tick();
        let Chips::SmsOpll(g) = &p.chips else { panic!() };
        // FM 1: F-number 172 at block 4, keyed; the piano at TL 2's volume.
        assert_eq!((g.regs[0x10], g.regs[0x20], g.regs[0x30]), (0xAC, 0x18, 0x32));
        // The rhythm mode, the bass drum struck at full volume.
        assert_eq!((g.regs[0x0E], g.regs[0x36] & 15), (0x30, 0));
        p.tick();
        let Chips::SmsOpll(g) = &p.chips else { panic!() };
        // The snare too; the bass drum still sounding.
        assert_eq!(g.regs[0x0E], 0x38);
    }

    #[test]
    fn note_tables() {
        assert_eq!(sn_period(48 * 128), 107);
        assert_eq!(nes_period(45 * 128), 253);
        assert_eq!(gb_period(45 * 128), 1452);
        assert_eq!(pce_period(45 * 128), 254);
        assert_eq!(fm_freq(45 * 128), (3, 1082));
        // The YM2151's C-4 is the C at the top of KC octave 3.
        assert_eq!(opm_kc(48 * 128), (0x3E, 0));
        assert_eq!(gb_noise(24), 0xA4);
        assert_eq!(gb_noise(48), 0x44);
    }

    #[test]
    fn reads_every_version_layout() {
        let d = module(4, 4, &[(9, 3, -1, -1, -1)]);
        let m = Module::parse(&d).expect("read");
        assert_eq!(m.system, System::GameBoy);
        assert_eq!(m.instruments[0].gb, [15, 0, 0, 64]);
        assert_eq!(m.patterns[0][0][0].note, 9);
        // Cut short anywhere it isn't read, and doesn't panic.
        for n in 0..d.len() {
            let _ = Module::parse(&d[..n]);
        }
    }

    #[test]
    fn keeps_what_a_file_cut_off_in_its_patterns_has() {
        let d = module(3, 4, &[(12, 3, 15, -1, -1), (1, 4, -1, -1, -1)]);
        // Cut off in the last channel's second row: that row is empty, as
        // are the samples.
        let m = Module::parse(&d[..d.len() - 10]).expect("read");
        assert_eq!((m.patterns[0][0][1].note, m.patterns[3][0][1].instrument), (1, -1));
        assert_eq!(m.patterns[3][0][1].fx, [(-1, -1)]);
        // An effect column count DefleMask doesn't have is damage.
        let at = d.len() - 1 - 2 * 12 - 1;
        assert_eq!(d[at], 1);
        let mut bad = d.clone();
        bad[at] = 0;
        assert!(Module::parse(&bad).is_none());
    }

    /// Renders each channel of DMF_PROBE alone (unpacked) to
    /// DMF_SOLO/<n>.wav (44.1 kHz, 16-bit stereo, DMF_SECS long), for
    /// comparing with Furnace's per-channel output.
    #[test]
    #[ignore]
    fn solo() {
        let p = std::env::var("DMF_PROBE").expect("DMF_PROBE");
        let dir = std::env::var("DMF_SOLO").expect("DMF_SOLO");
        let secs: f64 = std::env::var("DMF_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let d = std::fs::read(p).unwrap();
        let tune: Arc<dyn Tune> = Arc::new(Song::parse(&d).expect("not read"));
        let n = tune.driver(0).channels();
        let frames = (secs * 44100.0) as usize;
        for c in 0..n {
            let mut m = crate::replay::Machine::new(tune.clone(), 0);
            for k in 0..n {
                m.set_muted(k, k != c);
            }
            let (mut l, mut r) = (vec![0f32; frames], vec![0f32; frames]);
            m.render(44100, 1.0, 1.0, &mut l, &mut r);
            let mut w = Vec::new();
            let len = (frames * 4) as u32;
            w.extend_from_slice(b"RIFF");
            w.extend_from_slice(&(36 + len).to_le_bytes());
            w.extend_from_slice(b"WAVEfmt ");
            w.extend_from_slice(&16u32.to_le_bytes());
            w.extend_from_slice(&[1, 0, 2, 0]);
            w.extend_from_slice(&44100u32.to_le_bytes());
            w.extend_from_slice(&(44100u32 * 4).to_le_bytes());
            w.extend_from_slice(&[4, 0, 16, 0]);
            w.extend_from_slice(b"data");
            w.extend_from_slice(&len.to_le_bytes());
            for i in 0..frames {
                for v in [l[i], r[i]] {
                    w.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
            }
            std::fs::write(format!("{dir}/{:02}.wav", c + 1), w).unwrap();
        }
    }

    /// Prints the SN76489's registers per tick (period/volume of the four
    /// channels) of DMF_PROBE (an unpacked or packed module), as
    /// `snframes.py` prints Furnace's, for comparing.
    #[test]
    #[ignore]
    fn dump() {
        let p = std::env::var("DMF_PROBE").expect("DMF_PROBE");
        let n: usize = std::env::var("DMF_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        let d = std::fs::read(p).unwrap();
        let s = Song::parse(&d).expect("not read");
        let mut pl = Player::new(s.module.clone());
        for f in 0..n {
            pl.tick();
            // Once round, the reference (the song played once) has no more.
            if pl.take_done() {
                break;
            }
            if std::env::var("DMF_POS").is_ok() {
                eprintln!("{f} {:?} {}", pl.shown, pl.tick_in_row);
            }
            println!("{f:3} {}", pl.state());
        }
    }
}
