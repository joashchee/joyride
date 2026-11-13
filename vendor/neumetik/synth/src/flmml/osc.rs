// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
// Ported from FlMML on HTML5 (BSD-3-Clause, notice in mod.rs): MOscMod and
// its oscillators, MOscillator, and MOscOPM on our own YM2151 (ymfm.rs)
// in place of fmgen.

//! FlMML's oscillators. Phases are FlMML's 30-bit fixed point; where its
//! JavaScript turned a double into a 32-bit integer (`| 0`, `>>`), [`i32_of`]
//! does the same.

use super::Tables;
use crate::ymfm::Ym2151;
use std::rc::Rc;

pub const SAMPLE_RATE: f64 = 44100.0;
pub const TABLE_LEN: usize = 1 << 16;
pub const PHASE_SFT: u32 = 14;
pub const PHASE_LEN: i64 = (TABLE_LEN as i64) << PHASE_SFT;
pub const PHASE_MSK: i64 = PHASE_LEN - 1;

pub const SINE: usize = 0;
pub const PULSE: usize = 3;
pub const NOISE: usize = 4;
pub const FC_PULSE: usize = 5;
pub const FC_NOISE: usize = 7;
pub const FC_S_NOISE: usize = 8;
pub const FC_DPCM: usize = 9;
pub const GB_NOISE: usize = 11;
pub const GB_S_NOISE: usize = 12;
pub const OPM: usize = 14;
pub const MAX: usize = 15;

/// JavaScript's ToInt32: a double to a 32-bit integer, truncated and wrapped.
pub fn i32_of(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    let m = t.rem_euclid(4294967296.0);
    (m as u64 as u32) as i32
}

/// The fixed tables every song shares: sine, saw, triangle, the
/// Famicom triangles, the Game Boy noises, and the white noise (FlMML
/// fills it from `Math.random`; here from a fixed seed).
pub struct Fixed {
    sine: [Vec<f64>; 3],
    saw: [Vec<f64>; 2],
    tri: [Vec<f64>; 2],
    fc_tri: [[f64; 32]; 2],
    pub noise: Vec<f64>,
    gb_long: Vec<f64>,
    gb_short: Vec<f64>,
}

/// The random numbers standing for `Math.random()`: xorshift32 from a
/// fixed seed, the same in the reference harness.
pub struct Random(u32);

impl Random {
    pub fn new() -> Random {
        Random(2463534242)
    }
    pub fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        f64::from(x) / 4294967296.0
    }
}

impl Fixed {
    pub fn new() -> Fixed {
        let n = TABLE_LEN;
        let (mut sine, mut saw, mut tri) = ([vec![0.0; n], vec![0.0; n], vec![0.0; n]], [vec![0.0; n], vec![0.0; n]], [vec![0.0; n], vec![0.0; n]]);
        let d0 = 2.0 * std::f64::consts::PI / n as f64;
        let mut p0 = 0.0f64;
        for i in 0..n {
            let s = p0.sin();
            sine[0][i] = s;
            sine[1][i] = s.max(0.0);
            sine[2][i] = if s >= 0.0 { s } else { s * -1.0 };
            p0 += d0;
        }
        let d0 = 1.0 / n as f64;
        let mut p0 = 0.0f64;
        for i in 0..n {
            saw[0][i] = p0 * 2.0 - 1.0;
            saw[1][i] = if p0 < 0.5 { 2.0 * p0 } else { 2.0 * p0 - 2.0 };
            tri[0][i] = if p0 < 0.50 { 1.0 - 4.0 * p0 } else { 1.0 - 4.0 * (1.0 - p0) };
            tri[1][i] = if p0 < 0.25 {
                0.0 - 4.0 * p0
            } else if p0 < 0.75 {
                -2.0 + 4.0 * p0
            } else {
                4.0 - 4.0 * p0
            };
            p0 += d0;
        }
        let mut fc_tri = [[0.0; 32]; 2];
        for i in 0..16 {
            let v = i as f64 * 2.0 / 15.0 - 1.0;
            fc_tri[0][i] = v;
            fc_tri[0][31 - i] = v;
        }
        for (i, v) in fc_tri[1].iter_mut().enumerate() {
            let i = i as f64;
            *v = if i < 8.0 {
                i * 2.0 / 14.0
            } else if i < 24.0 {
                (8.0 - i) * 2.0 / 15.0 + 1.0
            } else {
                (i - 24.0) * 2.0 / 15.0 - 1.0
            };
        }
        let mut rnd = Random::new();
        let noise = (0..n).map(|_| rnd.next() * 2.0 - 1.0).collect();
        let gb = |len: usize, a: i32, b: i32| {
            let (mut gbr, mut output) = (0xffffi32, 1i32);
            (0..len)
                .map(|_| {
                    if gbr == 0 {
                        gbr = 1;
                    }
                    gbr = gbr.wrapping_add(gbr.wrapping_add(((gbr >> a) ^ (gbr >> b)) & 1));
                    output ^= gbr & 1;
                    f64::from(output * 2 - 1)
                })
                .collect()
        };
        Fixed { sine, saw, tri, fc_tri, noise, gb_long: gb(32767, 14, 13), gb_short: gb(127, 6, 5) }
    }
}

const FC_NOISE_SFT: u32 = 10;
const FC_NOISE_SEC: i64 = 1789773 << FC_NOISE_SFT;
const FC_NOISE_DLT: i64 = FC_NOISE_SEC / 44100;
const FC_NOISE_INTERVAL: [i64; 16] = [0x004, 0x008, 0x010, 0x020, 0x040, 0x060, 0x080, 0x0a0, 0x0ca, 0x0fe, 0x17c, 0x1fc, 0x2fa, 0x3f8, 0x7f2, 0xfe4];
const GB_SFT: u32 = 12;
const GB_DLT: f64 = (1 << GB_SFT) as f64;
const GB_INTERVAL: [f64; 64] = [
    2.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0, 56.0, 64.0, 80.0, 96.0, 112.0, 128.0, 160.0, 192.0, 224.0, 256.0, 320.0, 384.0, 448.0, 512.0, 640.0, 768.0, 896.0, 1024.0, 1280.0, 1536.0, 1792.0, 2048.0, 2560.0,
    3072.0, 3584.0, 4096.0, 5120.0, 6144.0, 7168.0, 8192.0, 10240.0, 12288.0, 14336.0, 16384.0, 20480.0, 24576.0, 28672.0, 32768.0, 40960.0, 49152.0, 57344.0, 65536.0, 81920.0, 98304.0, 114688.0, 131072.0, 163840.0, 196608.0,
    229376.0, 262144.0, 327680.0, 393216.0, 458752.0,
];
const DPCM_SFT: u32 = 2;
const DPCM_NEXT: i64 = 44100 << DPCM_SFT;
const DPCM_INTERVAL: [i64; 16] = [428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 85, 72, 54];

/// What each kind of oscillator keeps besides its phase.
enum Kind {
    Sine,
    Saw,
    Triangle,
    /// Width (in phase units), mixing noise in, and the noise it mixes.
    Pulse { pwm: f64, mix: bool },
    /// The counter, the step kept by @n, and whether a note resets it.
    Noise { counter: f64, noise_freq: f64, reset: bool },
    FcTri,
    FcNoise { fcr: i32, snz: i32, val: f64 },
    FcDpcm { address: usize, bit: u32, wav: i32, length: i32, ofs: f64 },
    GbWave,
    GbNoise { long: bool, sum: f64, skip: i32 },
    Wave,
    Opm(Box<Opm>),
}

/// One of FlMML's oscillators (MOscMod and its kinds).
pub struct Osc {
    kind: Kind,
    pub frequency: f64,
    /// The phase step: an integer but for Game Boy short noise and noise.
    shift: f64,
    phase: f64,
    wave_no: usize,
    fixed: Rc<Fixed>,
    tables: Rc<Tables>,
}

fn table_at(t: &[f64], i: i64) -> f64 {
    t.get(i as usize).copied().unwrap_or(0.0)
}

impl Osc {
    fn new(kind: Kind, fixed: Rc<Fixed>, tables: Rc<Tables>) -> Osc {
        let mut o = Osc { kind, frequency: 0.0, shift: 0.0, phase: 0.0, wave_no: 0, fixed, tables };
        // MOscMod's constructor sets 440 Hz (the FM voice isn't made yet
        // then, so it starts at 0); the noise then sets its own step.
        if !matches!(o.kind, Kind::Opm(_)) {
            o.set_frequency(440.0);
        }
        if matches!(o.kind, Kind::Noise { .. }) {
            o.set_noise_freq_white(1.0);
        }
        // FlMML's Famicom noise never sets its phase (its resetPhase is
        // empty): the first step adds to undefined and comes out 0.
        if matches!(o.kind, Kind::FcNoise { .. }) {
            o.phase = f64::NAN;
        }
        o
    }

    fn int_phase(&self) -> i64 {
        self.phase as i64
    }

    pub fn reset_phase(&mut self) {
        match &mut self.kind {
            Kind::Noise { reset, .. } => {
                if *reset {
                    self.phase = 0.0;
                }
            }
            Kind::FcNoise { .. } => {}
            Kind::FcDpcm { address, bit, wav, length, ofs } => {
                self.phase = 0.0;
                *address = 0;
                *bit = 0;
                *ofs = 0.0;
                let w = &self.tables.dpcm[self.wave_no];
                *wav = w.int_vol;
                *length = w.length;
            }
            _ => self.phase = 0.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f64) {
        match &mut self.kind {
            Kind::Noise { .. } | Kind::GbNoise { .. } => self.frequency = frequency,
            Kind::FcNoise { .. } => {
                self.shift = f64::from(i32_of(FC_NOISE_SEC as f64 / frequency));
            }
            Kind::FcDpcm { .. } => {
                self.shift = f64::from(i32_of(frequency * f64::from(1 << (DPCM_SFT + 4))));
            }
            Kind::Opm(_) => self.opm_frequency(frequency),
            _ => {
                self.frequency = frequency;
                self.shift = f64::from(i32_of(frequency * (PHASE_LEN as f64 / SAMPLE_RATE)));
            }
        }
    }

    pub fn add_phase(&mut self, time: i64) {
        match &mut self.kind {
            Kind::Noise { counter, .. } => {
                let c = i32_of(*counter + self.shift * time as f64);
                self.phase = ((self.phase as i64 + i64::from(c >> 30)) & (TABLE_LEN as i64 - 1)) as f64;
                *counter = f64::from(c & ((1 << 30) - 1));
            }
            Kind::FcNoise { .. } => {
                self.phase = f64::from(i32_of(self.phase + (FC_NOISE_DLT * time) as f64));
                while self.phase >= self.shift {
                    self.phase -= self.shift;
                    let v = self.fc_value();
                    if let Kind::FcNoise { val, .. } = &mut self.kind {
                        *val = v;
                    }
                }
            }
            _ => self.phase = ((self.int_phase() + self.shift as i64 * time) & PHASE_MSK) as f64,
        }
    }

    pub fn set_wave_no(&mut self, n: i64) {
        let n = n.max(0) as usize;
        match &mut self.kind {
            Kind::Sine => self.wave_no = n.min(2),
            Kind::Saw | Kind::Triangle | Kind::FcTri => self.wave_no = n.min(1),
            Kind::GbWave => {
                let n = n.min(31);
                self.wave_no = if self.tables.gb_wave[n].is_some() { n } else { 0 };
            }
            Kind::Wave => {
                let n = n.min(31);
                self.wave_no = if self.tables.wave[n].is_some() { n } else { 0 };
            }
            Kind::FcDpcm { .. } => {
                let n = n.min(15);
                self.wave_no = if self.tables.dpcm_set[n] { n } else { 0 };
            }
            Kind::Opm(_) => self.opm_wave(n),
            _ => {}
        }
    }

    pub fn set_note_no(&mut self, note: i64) {
        match &mut self.kind {
            Kind::FcNoise { .. } => self.set_noise_no(note),
            Kind::GbNoise { .. } => self.set_gb_noise(note),
            Kind::FcDpcm { .. } => {
                let no = note.clamp(0, 15) as usize;
                self.shift = f64::from(i32_of(((1789773i64 << DPCM_SFT) as f64) / DPCM_INTERVAL[no] as f64));
            }
            Kind::Opm(o) => o.note_on(),
            _ => {}
        }
    }

    // ---- Noise (MOscNoise).

    pub fn set_noise_freq_white(&mut self, frequency: f64) {
        if let Kind::Noise { noise_freq, .. } = &mut self.kind {
            *noise_freq = frequency * f64::from(1u32 << 30);
            self.shift = *noise_freq;
        }
    }
    pub fn restore_freq(&mut self) {
        if let Kind::Noise { noise_freq, .. } = &self.kind {
            self.shift = *noise_freq;
        }
    }
    pub fn disable_reset_phase(&mut self) {
        if let Kind::Noise { reset, .. } = &mut self.kind {
            *reset = false;
        }
    }

    // ---- Famicom noise.

    fn fc_value(&mut self) -> f64 {
        if let Kind::FcNoise { fcr, snz, .. } = &mut self.kind {
            *fcr >>= 1;
            *fcr |= ((*fcr ^ (*fcr >> *snz)) & 1) << 15;
            if *fcr & 1 != 0 {
                1.0
            } else {
                -1.0
            }
        } else {
            0.0
        }
    }
    pub fn set_fc_mode(&mut self, short: bool) {
        if let Kind::FcNoise { snz, .. } = &mut self.kind {
            *snz = if short { 6 } else { 1 };
        }
    }
    fn set_noise_no(&mut self, no: i64) {
        let no = no.clamp(0, 15) as usize;
        self.shift = f64::from(i32_of((FC_NOISE_INTERVAL[no] << FC_NOISE_SFT) as f64));
    }

    // ---- Game Boy noise.

    fn set_gb_noise(&mut self, no: i64) {
        let no = no.clamp(0, 63) as usize;
        let long = matches!(self.kind, Kind::GbNoise { long: true, .. });
        let v = f64::from(1048576i32 << (GB_SFT - 2)) / (GB_INTERVAL[no] * 11025.0);
        self.shift = if long { f64::from(i32_of(v)) } else { v };
    }

    // ---- Pulse.

    pub fn set_pwm(&mut self, pwm: f64) {
        if let Kind::Pulse { pwm: p, .. } = &mut self.kind {
            *p = pwm * PHASE_LEN as f64;
        }
    }
    pub fn set_mix(&mut self, m: bool) {
        if let Kind::Pulse { mix, .. } = &mut self.kind {
            *mix = m;
        }
    }

    /// The next sample (with the phase moved on by `ofs`, from a pipe:
    /// `Some`), for the pulse's noise passed in.
    pub fn next(&mut self, ofs: Option<f64>, noise: Option<&mut Osc>) -> f64 {
        let fixed = self.fixed.clone();
        let shift_i = self.shift as i64;
        let ph = self.int_phase();
        let at = |o: Option<f64>| match o {
            Some(o) => (i64::from(i32_of(ph as f64 + o)) & PHASE_MSK) >> PHASE_SFT,
            None => ph >> PHASE_SFT,
        };
        let step = |s: &mut Osc| s.phase = ((ph + shift_i) & PHASE_MSK) as f64;
        match &mut self.kind {
            Kind::Sine => {
                let v = fixed.sine[self.wave_no][at(ofs) as usize];
                step(self);
                v
            }
            Kind::Saw => {
                let v = fixed.saw[self.wave_no][at(ofs) as usize];
                step(self);
                v
            }
            Kind::Triangle => {
                let v = fixed.tri[self.wave_no][at(ofs) as usize];
                step(self);
                v
            }
            Kind::FcTri => {
                let v = fixed.fc_tri[self.wave_no][(at(ofs) >> 11) as usize];
                step(self);
                v
            }
            Kind::GbWave => {
                let t = self.tables.gb_wave[self.wave_no].as_ref().map(|t| table_at(t, at(ofs) >> 11)).unwrap_or(0.0);
                step(self);
                t
            }
            Kind::Wave => {
                let p = match ofs {
                    Some(o) => i64::from(i32_of(ph as f64 + o)) & PHASE_MSK,
                    None => ph,
                };
                let v = self.tables.wave[self.wave_no].as_ref().map(|(t, len)| table_at(t, (p as f64 / len).floor() as i64)).unwrap_or(0.0);
                step(self);
                v
            }
            Kind::Pulse { pwm, mix } => {
                let p = match ofs {
                    Some(o) => (i64::from(i32_of(ph as f64 + o)) & PHASE_MSK) as f64,
                    None => ph as f64,
                };
                let v = if p < *pwm {
                    1.0
                } else if *mix {
                    noise.map(|n| n.next(ofs, None)).unwrap_or(-1.0)
                } else {
                    -1.0
                };
                step(self);
                v
            }
            Kind::Noise { counter, .. } => {
                let idx = match ofs {
                    Some(o) => (ph + i64::from(i32_of(o).wrapping_shl(PHASE_SFT))) & (TABLE_LEN as i64 - 1),
                    None => ph,
                };
                let v = fixed.noise[(idx & (TABLE_LEN as i64 - 1)) as usize];
                let c = i32_of(*counter + self.shift);
                self.phase = ((ph + i64::from(c >> 30)) & (TABLE_LEN as i64 - 1)) as f64;
                *counter = f64::from(c & ((1 << 30) - 1));
                v
            }
            Kind::FcNoise { .. } => match ofs {
                None => self.fc_next(FC_NOISE_DLT as f64),
                Some(o) => {
                    let (fcr0, phase0) = match &self.kind {
                        Kind::FcNoise { fcr, .. } => (*fcr, self.phase),
                        _ => unreachable!(),
                    };
                    let val0 = match &self.kind {
                        Kind::FcNoise { val, .. } => *val,
                        _ => 0.0,
                    };
                    // FlMML's getNextSampleOfs: runs ahead by the offset, keeps
                    // the value it made, then puts the register and phase back
                    // and steps once as usual.
                    let mut delta = FC_NOISE_DLT as f64 + o;
                    let (mut sum, mut cnt) = (0.0, 0.0);
                    while delta >= self.shift {
                        delta -= self.shift;
                        self.phase = 0.0;
                        sum += self.fc_value();
                        cnt += 1.0;
                    }
                    if cnt > 0.0 {
                        if let Kind::FcNoise { val, .. } = &mut self.kind {
                            *val = sum / cnt;
                        }
                    }
                    self.phase = f64::from(i32_of(self.phase + delta));
                    if self.phase >= self.shift {
                        self.phase = self.shift;
                        let v = self.fc_value();
                        if let Kind::FcNoise { val, .. } = &mut self.kind {
                            *val = v;
                        }
                    }
                    if let Kind::FcNoise { fcr, .. } = &mut self.kind {
                        *fcr = fcr0;
                    }
                    self.phase = phase0;
                    self.fc_next(FC_NOISE_DLT as f64);
                    val0
                }
            },
            Kind::GbNoise { long, sum, skip } => {
                let (table, modulo) = if *long { (&fixed.gb_long, (32767i64 << GB_SFT) - 1) } else { (&fixed.gb_short, (127i64 << GB_SFT) - 1) };
                let m = modulo as f64;
                match ofs {
                    Some(o) => {
                        let p = (self.phase + o) % m;
                        let pi = i32_of(p);
                        let idx = i64::from(i32_of(p + f64::from((pi >> 31) & modulo as i32))) >> GB_SFT;
                        let v = table_at(table, idx);
                        self.phase = (self.phase + self.shift) % m;
                        v
                    }
                    None => {
                        let mut v = table_at(table, i64::from(i32_of(self.phase)) >> GB_SFT);
                        if *skip > 0 {
                            v = (v + *sum) / f64::from(*skip + 1);
                        }
                        *sum = 0.0;
                        *skip = 0;
                        let mut fs = self.shift;
                        while fs > GB_DLT {
                            self.phase = (self.phase + GB_DLT) % m;
                            fs -= GB_DLT;
                            *sum += table_at(table, i64::from(i32_of(self.phase)) >> GB_SFT);
                            *skip += 1;
                        }
                        self.phase = (self.phase + fs) % m;
                        v
                    }
                }
            }
            Kind::FcDpcm { .. } => self.dpcm_next(ofs),
            Kind::Opm(o) => o.sample(),
        }
    }

    fn fc_next(&mut self, dlt: f64) -> f64 {
        let val0 = match &self.kind {
            Kind::FcNoise { val, .. } => *val,
            _ => 0.0,
        };
        let mut delta = dlt;
        let (mut sum, mut cnt) = (0.0, 0.0);
        while delta >= self.shift {
            delta -= self.shift;
            self.phase = 0.0;
            sum += self.fc_value();
            cnt += 1.0;
        }
        if cnt > 0.0 {
            if let Kind::FcNoise { val, .. } = &mut self.kind {
                *val = sum / cnt;
            }
        }
        self.phase = f64::from(i32_of(self.phase + delta));
        if self.phase >= self.shift {
            self.phase -= self.shift;
            let v = self.fc_value();
            if let Kind::FcNoise { val, .. } = &mut self.kind {
                *val = v;
            }
        }
        val0
    }

    fn dpcm_next(&mut self, ofs: Option<f64>) -> f64 {
        let tables = self.tables.clone();
        let w = &tables.dpcm[self.wave_no];
        let shift = self.shift as i64;
        let Kind::FcDpcm { address, bit, wav, length, ofs: last } = &mut self.kind else { return 0.0 };
        let mut val = f64::from(*wav - 64) / 64.0;
        let add = match ofs {
            Some(o) => shift + (i64::from(i32_of(o - *last)) >> (PHASE_SFT - 7)),
            None => shift,
        };
        let mut phase = ((self.phase as i64) + add) & PHASE_MSK;
        while DPCM_NEXT <= phase {
            phase -= DPCM_NEXT;
            if *length > 0 {
                if (w.table.get(*address).copied().unwrap_or(0) >> *bit) & 1 != 0 {
                    if *wav < 126 {
                        *wav += 2;
                    }
                } else if *wav > 1 {
                    *wav -= 2;
                }
                *bit += 1;
                if *bit >= 32 {
                    *bit = 0;
                    *address += 1;
                }
                *length -= 1;
                if *length == 0 && w.looped {
                    *address = 0;
                    *bit = 0;
                    *length = w.length;
                }
            }
            val = f64::from(*wav - 64) / 64.0;
        }
        if let Some(o) = ofs {
            *last = o;
        }
        self.phase = phase as f64;
        val
    }

    /// Samples into `out[start..end]` (FlMML's getSamples), with sync in
    /// (`Some(line, false)`) or out (`Some(line, true)`).
    pub fn samples(&mut self, out: &mut [f32], start: usize, end: usize, mut noise: Option<&mut Osc>, sync: Option<(&mut [bool], bool)>) {
        let sync_kind = !matches!(self.kind, Kind::Noise { .. } | Kind::FcNoise { .. } | Kind::FcDpcm { .. } | Kind::GbNoise { .. } | Kind::Opm(_));
        match (sync, sync_kind) {
            (Some((line, false)), true) => {
                for i in start..end {
                    if line[i] {
                        self.reset_phase();
                    }
                    out[i] = self.next(None, noise.as_deref_mut()) as f32;
                }
            }
            (Some((line, true)), true) => {
                for i in start..end {
                    let before = self.int_phase();
                    out[i] = self.next(None, noise.as_deref_mut()) as f32;
                    line[i] = before + self.shift as i64 > PHASE_MSK;
                }
            }
            _ => {
                if let Kind::Opm(o) = &mut self.kind {
                    for v in &mut out[start..end] {
                        *v = o.sample() as f32;
                    }
                    return;
                }
                for v in &mut out[start..end] {
                    *v = self.next(None, noise.as_deref_mut()) as f32;
                }
            }
        }
    }

    // ---- FM (MOscOPM on the YM2151).

    fn opm_frequency(&mut self, frequency: f64) {
        let Kind::Opm(o) = &mut self.kind else { return };
        if self.frequency == frequency {
            return;
        }
        self.frequency = frequency;
        self.shift = f64::from(i32_of(frequency * (PHASE_LEN as f64 / SAMPLE_RATE)));
        let n = i32_of(1200.0 * (frequency / 440.0).ln() * std::f64::consts::LOG2_E + 5700.0 + 0.5);
        let note = n / 100;
        let cent = n % 100;
        let kf = i32_of(64.0 * f64::from(cent) / 100.0 + 0.5);
        const KCTABLE: [i32; 12] = [0xE, 0x0, 0x1, 0x2, 0x4, 0x5, 0x6, 0x8, 0x9, 0xA, 0xC, 0xD];
        let kc = (((note - 1) / 12) << 4) | KCTABLE[((note + 1200) % 12) as usize];
        o.reg(0x30, (kf << 2) as u8);
        o.reg(0x28, kc as u8);
    }

    fn opm_wave(&mut self, n: usize) {
        let tables = self.tables.clone();
        let Kind::Opm(o) = &mut self.kind else { return };
        let n = n.min(127);
        let t = tables.opm[n].clone().unwrap_or_else(|| tables.opm[0].clone().unwrap_or_default());
        o.load(&t);
    }

    pub fn opm_ref(&self) -> Option<&Opm> {
        match &self.kind {
            Kind::Opm(o) => Some(o),
            _ => None,
        }
    }

    pub fn opm(&mut self) -> Option<&mut Opm> {
        match &mut self.kind {
            Kind::Opm(o) => Some(o),
            _ => None,
        }
    }

}

/// FlMML's FM voice (MOscOPM) on the YM2151: channel 0, its output brought
/// from the chip's rate to 44.1 kHz, at FlMML's level (its common gain in
/// dB and the expression).
pub struct Opm {
    chip: Box<Ym2151>,
    /// The chip's last two samples (channel 0), and how far between them.
    prev: f64,
    cur: f64,
    frac: f64,
    step: f64,
    op_mask: u8,
    velocity: i32,
    al: u8,
    tl: [i32; 4],
    gain: f64,
    expression: f64,
    /// Keyed on and not yet clocked: fmgen starts the attack when the
    /// register is written, the YM2151 at its next sample.
    keyed: bool,
}

const SLOT: [u8; 4] = [0, 2, 1, 3];
const CARRIER: [u8; 8] = [0x40, 0x40, 0x40, 0x40, 0x50, 0x70, 0x70, 0x78];
/// FlMML's clock for its OPM.
const OPM_CLOCK: f64 = 3_580_000.0;

impl Opm {
    pub fn new(gain_db: f64) -> Opm {
        let mut chip = Box::new(Ym2151::default());
        // Every channel's key off; channel 0 plays.
        chip.write(0x01, 0);
        Opm { chip, prev: 0.0, cur: 0.0, frac: 1.0, step: Ym2151::rate(OPM_CLOCK) / SAMPLE_RATE, op_mask: 15, velocity: 127, al: 0, tl: [0; 4], gain: gain_db, expression: 1.0, keyed: false }
    }
    fn reg(&mut self, r: u8, v: u8) {
        self.chip.write(r, v);
    }
    pub fn load(&mut self, p: &[i32]) {
        let g = |i: usize| p.get(i).copied().unwrap_or(0);
        self.al = (g(0) & 7) as u8;
        self.reg(0x20, ((3 << 6) | ((g(1) & 7) << 3) | (g(0) & 7)) as u8);
        let mut i = 2;
        for (s, &slot) in SLOT.iter().enumerate() {
            let off = slot << 3;
            self.reg(0x40 | off, (((g(i + 8) & 7) << 4) | (g(i + 7) & 15)) as u8);
            self.tl[s] = g(i + 5);
            self.set_tl(slot, g(i + 5));
            self.reg(0x80 | off, (((g(i + 6) & 3) << 6) | (g(i) & 0x1f)) as u8);
            self.reg(0xa0 | off, (((g(i + 10) & 1) << 7) | (g(i + 1) & 0x1f)) as u8);
            self.reg(0xc0 | off, (((g(i + 9) & 3) << 6) | (g(i + 2) & 0x1f)) as u8);
            self.reg(0xe0 | off, (((g(i + 4) & 15) << 4) | (g(i + 3) & 15)) as u8);
            i += 11;
        }
        self.set_velocity(self.velocity);
        self.op_mask = (g(i) & 15) as u8;
        self.set_wf(g(i + 1));
        self.set_lfrq(g(i + 2));
        self.set_pmd(g(i + 3));
        self.set_amd(g(i + 4));
        self.set_pms_ams(g(i + 5), g(i + 6));
        self.reg(0x0f, (((g(i + 7) & 1) << 7) | (g(i + 8) & 0x1f)) as u8);
    }
    fn set_tl(&mut self, slot: u8, tl: i32) {
        self.reg(0x60 | (slot << 3), tl.clamp(0, 127) as u8);
    }
    pub fn set_velocity(&mut self, vel: i32) {
        self.velocity = vel;
        let c = CARRIER[usize::from(self.al)];
        for (s, bit) in [0x08u8, 0x10, 0x20, 0x40].iter().enumerate() {
            let add = if c & bit != 0 { 127 - vel } else { 0 };
            self.set_tl(SLOT[s], self.tl[s] + add);
        }
    }
    pub fn set_expression(&mut self, ex: f64) {
        self.expression = ex;
    }
    pub fn set_pms_ams(&mut self, pms: i32, ams: i32) {
        self.reg(0x38, (((pms & 7) << 4) | (ams & 3)) as u8);
    }
    pub fn set_pmd(&mut self, pmd: i32) {
        self.reg(0x19, (0x80 | (pmd & 0x7f)) as u8);
    }
    pub fn set_amd(&mut self, amd: i32) {
        self.reg(0x19, (amd & 0x7f) as u8);
    }
    pub fn set_lfrq(&mut self, f: i32) {
        self.reg(0x18, (f & 0xff) as u8);
    }
    pub fn set_wf(&mut self, w: i32) {
        self.reg(0x1b, (w & 3) as u8);
    }
    pub fn note_on(&mut self) {
        self.reg(0x01, 0x02);
        self.reg(0x01, 0x00);
        self.reg(0x08, self.op_mask << 3);
        self.keyed = true;
    }
    pub fn note_off(&mut self) {
        self.reg(0x08, 0x00);
    }
    /// Whether its carriers still sound (fmgen's IsOn).
    pub fn playing(&self) -> bool {
        if self.keyed {
            return true;
        }
        let on = |i: usize| self.chip.fm.operator_on(0, i);
        match self.al {
            0..=3 => on(3),
            4 => on(1) || on(3),
            5 | 6 => on(1) || on(2) || on(3),
            _ => on(0) || on(1) || on(2) || on(3),
        }
    }
    fn sample(&mut self) -> f64 {
        // The chip at its own rate, read between its samples.
        while self.frac >= 1.0 {
            let mut out = [[0i32; 2]; 8];
            self.chip.generate(&mut out);
            self.keyed = false;
            self.prev = self.cur;
            self.cur = f64::from(out[0][0]);
            self.frac -= 1.0;
        }
        let r = self.prev + (self.cur - self.prev) * self.frac;
        self.frac += self.step;
        // fmgen: r * 10^(dB/40) * expression / 8192.
        r * 10f64.powf(self.gain / 40.0) * self.expression / 8192.0
    }
}

/// Every oscillator a channel has, one current (MOscillator).
pub struct Oscillator {
    pub mods: Vec<Osc>,
    pub form: usize,
}

impl Oscillator {
    pub fn new(fixed: &Rc<Fixed>, tables: &Rc<Tables>) -> Oscillator {
        let mk = |k: Kind| Osc::new(k, fixed.clone(), tables.clone());
        let mods = vec![
            mk(Kind::Sine),
            mk(Kind::Saw),
            mk(Kind::Triangle),
            mk(Kind::Pulse { pwm: 0.5 * PHASE_LEN as f64, mix: false }),
            mk(Kind::Noise { counter: 0.0, noise_freq: 0.0, reset: true }),
            mk(Kind::Pulse { pwm: 0.5 * PHASE_LEN as f64, mix: false }),
            mk(Kind::FcTri),
            mk(Kind::FcNoise { fcr: 0x8000, snz: 1, val: 0.0 }),
            // FC_S_NOISE is the same oscillator as FC_NOISE (short mode).
            mk(Kind::Sine),
            mk(Kind::FcDpcm { address: 0, bit: 0, wav: 0, length: 0, ofs: 0.0 }),
            mk(Kind::GbWave),
            mk(Kind::GbNoise { long: true, sum: 0.0, skip: 0 }),
            mk(Kind::GbNoise { long: false, sum: 0.0, skip: 0 }),
            mk(Kind::Wave),
            mk(Kind::Opm(Box::new(Opm::new(tables.fm_gain)))),
        ];
        let mut o = Oscillator { mods, form: PULSE };
        // The FC noise's own constructor: a value, then interval 0.
        o.mods[FC_NOISE] = {
            let mut n = mk(Kind::FcNoise { fcr: 0x8000, snz: 1, val: 0.0 });
            let v = n.fc_value();
            if let Kind::FcNoise { val, .. } = &mut n.kind {
                *val = v;
            }
            n.set_noise_no(0);
            n
        };
        o.mods[OPM].opm_wave(0);
        o.set_form(PULSE);
        o
    }
    pub fn index(form: usize) -> usize {
        if form == FC_S_NOISE {
            FC_NOISE
        } else {
            form
        }
    }
    pub fn set_form(&mut self, form: usize) -> usize {
        let form = form.min(MAX - 1);
        self.form = form;
        match form {
            NOISE => self.mods[NOISE].restore_freq(),
            FC_NOISE => self.mods[FC_NOISE].set_fc_mode(false),
            FC_S_NOISE => self.mods[FC_NOISE].set_fc_mode(true),
            _ => {}
        }
        Self::index(form)
    }
    pub fn current(&self) -> usize {
        Self::index(self.form)
    }
}
