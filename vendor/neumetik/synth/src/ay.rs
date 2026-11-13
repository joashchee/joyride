// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from Ayumi, Peter Sovietov's AY-3-8910 and YM2149 emulator
// (github.com/true-grue/ayumi), under its MIT License:
//
// The MIT License
//
// Copyright (c) Peter Sovietov, http://sovietov.com
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! The General Instrument AY-3-8910 and Yamaha YM2149: three square-wave
//! tone channels, a 17-bit noise and a 16-shape envelope, each channel's
//! level through the chip's DAC (the AY's 16 steps, or the YM's 32). The
//! chip is stepped at its clock over 8, its output interpolated, then
//! decimated by an FIR filter to the output's rate, as Ayumi does; here
//! each tone channel through its own interpolator and filter (they're
//! linear, so the three add up to Ayumi's mix), so a host can pan, mute
//! and level them.

const DECIMATE: usize = 8;
const FIR: usize = 192;
const DC: usize = 1024;

const AY_DAC: [f64; 32] = [
    0.0,
    0.0,
    0.00999465934234,
    0.00999465934234,
    0.0144502937362,
    0.0144502937362,
    0.0210574502174,
    0.0210574502174,
    0.0307011520562,
    0.0307011520562,
    0.0455481803616,
    0.0455481803616,
    0.0644998855573,
    0.0644998855573,
    0.107362478065,
    0.107362478065,
    0.126588845655,
    0.126588845655,
    0.20498970016,
    0.20498970016,
    0.292210269322,
    0.292210269322,
    0.372838941024,
    0.372838941024,
    0.492530708782,
    0.492530708782,
    0.635324635691,
    0.635324635691,
    0.805584802014,
    0.805584802014,
    1.0,
    1.0,
];

const YM_DAC: [f64; 32] = [
    0.0,
    0.0,
    0.00465400167849,
    0.00772106507973,
    0.0109559777218,
    0.0139620050355,
    0.0169985503929,
    0.0200198367285,
    0.024368657969,
    0.029694056611,
    0.0350652323186,
    0.0403906309606,
    0.0485389486534,
    0.0583352407111,
    0.0680552376593,
    0.0777752346075,
    0.0925154497597,
    0.111085679408,
    0.129747463188,
    0.148485542077,
    0.17666895552,
    0.211551079576,
    0.246387426566,
    0.281101701381,
    0.333730067903,
    0.400427252613,
    0.467383840696,
    0.53443198291,
    0.635172045472,
    0.75800717174,
    0.879926756695,
    1.0,
];

/// The FIR's taps, symmetric about the middle (index 96); the ones not
/// listed are 0.
const TAPS: [(usize, f64); 84] = [
    (1, -0.0000046183113992051936),
    (2, -0.00001117761640887225),
    (3, -0.000018610264502005432),
    (4, -0.000025134586135631012),
    (5, -0.000028494281690666197),
    (6, -0.000026396828793275159),
    (7, -0.000017094212558802156),
    (9, 0.000023798193576966866),
    (10, 0.000051281160242202183),
    (11, 0.00007762197826243427),
    (12, 0.000096759426664120416),
    (13, 0.00010240229300393402),
    (14, 0.000089344614218077106),
    (15, 0.000054875700118949183),
    (17, -0.000069839082210680165),
    (18, -0.0001447966132360757),
    (19, -0.00021158452917708308),
    (20, -0.00025535069106550544),
    (21, -0.00026228714374322104),
    (22, -0.00022258805927027799),
    (23, -0.00013323230495695704),
    (25, 0.00016182578767055206),
    (26, 0.00032846175385096581),
    (27, 0.00047045611576184863),
    (28, 0.00055713851457530944),
    (29, 0.00056212565121518726),
    (30, 0.00046901918553962478),
    (31, 0.00027624866838952986),
    (33, -0.00032564179486838622),
    (34, -0.00065182310286710388),
    (35, -0.00092127787309319298),
    (36, -0.0010772534348943575),
    (37, -0.0010737727700273478),
    (38, -0.00088556645390392634),
    (39, -0.00051581896090765534),
    (41, 0.00059548767193795277),
    (42, 0.0011803558710661009),
    (43, 0.0016527320270369871),
    (44, 0.0019152679330965555),
    (45, 0.0018927324805381538),
    (46, 0.0015481870327877937),
    (47, 0.00089470695834941306),
    (49, -0.0010178225878206125),
    (50, -0.0020037400552054292),
    (51, -0.0027874356824117317),
    (52, -0.003210329988021943),
    (53, -0.0031540624117984395),
    (54, -0.0025657163651900345),
    (55, -0.0014750752642111449),
    (57, 0.0016624165446378462),
    (58, 0.0032591192839069179),
    (59, 0.0045165685815867747),
    (60, 0.0051838984346123896),
    (61, 0.0050774264697459933),
    (62, 0.0041192521414141585),
    (63, 0.0023628575417966491),
    (65, -0.0026543507866759182),
    (66, -0.0051990251084333425),
    (67, -0.0072020238234656924),
    (68, -0.0082672928192007358),
    (69, -0.0081033739572956287),
    (70, -0.006583111539570221),
    (71, -0.0037839040415292386),
    (73, 0.0042781252851152507),
    (74, 0.0084176358598320178),
    (75, 0.01172566057463055),
    (76, 0.013550476647788672),
    (77, 0.013388189369997496),
    (78, 0.010979501242341259),
    (79, 0.006381274941685413),
    (81, -0.007421229604153888),
    (82, -0.01486456304340213),
    (83, -0.021143584622178104),
    (84, -0.02504275058758609),
    (85, -0.025473530942547201),
    (86, -0.021627310017882196),
    (87, -0.013104323383225543),
    (89, 0.017065133989980476),
    (90, 0.036978919264451952),
    (91, 0.05823318062093958),
    (92, 0.079072012081405949),
    (93, 0.097675998716952317),
    (94, 0.11236045936950932),
    (95, 0.12176343577287731),
];

#[derive(Clone, Copy, Default)]
struct Tone {
    period: i32,
    counter: i32,
    tone: i32,
    t_off: i32,
    n_off: i32,
    e_on: bool,
    volume: i32,
}

/// One channel's path from the chip's steps to the output's rate.
#[derive(Clone)]
struct Path {
    c: [f64; 3],
    y: [f64; 4],
    fir: Vec<f64>,
    dc_sum: f64,
    dc: Vec<f64>,
}

impl Path {
    fn new() -> Path {
        Path {
            c: [0.0; 3],
            y: [0.0; 4],
            fir: vec![0.0; FIR * 2],
            dc_sum: 0.0,
            dc: vec![0.0; DC],
        }
    }
}

/// An AY-3-8910 (or YM2149) and its three channels' outputs.
#[derive(Clone)]
pub struct Ay {
    tones: [Tone; 3],
    noise_period: i32,
    noise_counter: i32,
    noise: i32,
    env_counter: i32,
    env_period: i32,
    env_shape: usize,
    env_segment: usize,
    envelope: i32,
    dac: &'static [f64; 32],
    step: f64,
    x: f64,
    paths: [Path; 3],
    fir_index: usize,
    dc_index: usize,
    /// The three channels' last output samples (DC removed when asked).
    pub out: [f64; 3],
    /// A level (0 to 31) each channel outputs in place of its tone, noise
    /// and volume, as a player writing samples to the volume does.
    pub sample: [Option<u8>; 3],
    clock: f64,
    rate: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Seg {
    Down,
    Up,
    Bottom,
    Top,
}

const SHAPES: [[Seg; 2]; 16] = [
    [Seg::Down, Seg::Bottom],
    [Seg::Down, Seg::Bottom],
    [Seg::Down, Seg::Bottom],
    [Seg::Down, Seg::Bottom],
    [Seg::Up, Seg::Bottom],
    [Seg::Up, Seg::Bottom],
    [Seg::Up, Seg::Bottom],
    [Seg::Up, Seg::Bottom],
    [Seg::Down, Seg::Down],
    [Seg::Down, Seg::Bottom],
    [Seg::Down, Seg::Up],
    [Seg::Down, Seg::Top],
    [Seg::Up, Seg::Up],
    [Seg::Up, Seg::Top],
    [Seg::Up, Seg::Down],
    [Seg::Up, Seg::Bottom],
];

impl Ay {
    /// A chip clocked at `clock` Hz, its output at `rate`; `ym` for the
    /// YM2149's DAC.
    pub fn new(ym: bool, clock: f64, rate: u32) -> Ay {
        let mut ay = Ay {
            tones: [Tone::default(); 3],
            noise_period: 1,
            noise_counter: 0,
            noise: 1,
            env_counter: 0,
            env_period: 1,
            env_shape: 0,
            env_segment: 0,
            envelope: 0,
            dac: if ym { &YM_DAC } else { &AY_DAC },
            step: 0.0,
            x: 0.0,
            paths: [Path::new(), Path::new(), Path::new()],
            fir_index: 0,
            dc_index: 0,
            out: [0.0; 3],
            sample: [None; 3],
            clock,
            rate: 0,
        };
        ay.set_rate(rate);
        for c in 0..3 {
            ay.set_tone(c, 1);
        }
        ay
    }

    /// The output's rate (the chip carries on where it was).
    pub fn set_rate(&mut self, rate: u32) {
        if rate != self.rate {
            self.rate = rate;
            self.step = self.clock / (f64::from(rate.max(1)) * 8.0 * DECIMATE as f64);
        }
    }

    pub fn set_tone(&mut self, c: usize, period: i32) {
        let p = period & 0xfff;
        self.tones[c].period = i32::from(p == 0) | p;
    }

    pub fn set_noise(&mut self, period: i32) {
        let p = period & 0x1f;
        self.noise_period = i32::from(p == 0) | p;
    }

    pub fn set_mixer(&mut self, c: usize, t_off: bool, n_off: bool, e_on: bool) {
        let t = &mut self.tones[c];
        t.t_off = i32::from(t_off);
        t.n_off = i32::from(n_off);
        t.e_on = e_on;
    }

    pub fn set_volume(&mut self, c: usize, volume: i32) {
        self.tones[c].volume = volume & 0xf;
    }

    pub fn set_envelope(&mut self, period: i32) {
        let p = period & 0xffff;
        self.env_period = i32::from(p == 0) | p;
    }

    pub fn set_envelope_shape(&mut self, shape: i32) {
        self.env_shape = (shape & 0xf) as usize;
        self.env_counter = 0;
        self.env_segment = 0;
        self.reset_segment();
    }

    /// The chip's registers 0 to 13 as a player writes them (13, the
    /// envelope's shape, only when `shape` says it was written).
    pub fn write_regs(&mut self, r: &[u8; 14], shape: bool) {
        for c in 0..3 {
            self.set_tone(c, i32::from(r[c * 2]) | (i32::from(r[c * 2 + 1] & 0x0f) << 8));
            self.set_mixer(c, r[7] >> c & 1 != 0, r[7] >> (c + 3) & 1 != 0, r[8 + c] & 0x10 != 0);
            self.set_volume(c, i32::from(r[8 + c]));
        }
        self.set_noise(i32::from(r[6]));
        self.set_envelope(i32::from(r[11]) | (i32::from(r[12]) << 8));
        if shape {
            self.set_envelope_shape(i32::from(r[13]));
        }
    }

    fn reset_segment(&mut self) {
        let s = SHAPES[self.env_shape][self.env_segment];
        self.envelope = if s == Seg::Down || s == Seg::Top { 31 } else { 0 };
    }

    fn update_mixer(&mut self) -> [f64; 3] {
        self.noise_counter += 1;
        if self.noise_counter >= self.noise_period << 1 {
            self.noise_counter = 0;
            let bit = (self.noise ^ (self.noise >> 3)) & 1;
            self.noise = (self.noise >> 1) | (bit << 16);
        }
        let noise = self.noise & 1;
        self.env_counter += 1;
        if self.env_counter >= self.env_period {
            self.env_counter = 0;
            match SHAPES[self.env_shape][self.env_segment] {
                Seg::Up => {
                    self.envelope += 1;
                    if self.envelope > 31 {
                        self.env_segment ^= 1;
                        self.reset_segment();
                    }
                }
                Seg::Down => {
                    self.envelope -= 1;
                    if self.envelope < 0 {
                        self.env_segment ^= 1;
                        self.reset_segment();
                    }
                }
                _ => {}
            }
        }
        let envelope = self.envelope;
        let sample = self.sample;
        let mut out = [0.0; 3];
        for (c, t) in self.tones.iter_mut().enumerate() {
            t.counter += 1;
            if t.counter >= t.period {
                t.counter = 0;
                t.tone ^= 1;
            }
            let o = (t.tone | t.t_off) & (noise | t.n_off);
            let level = match sample[c] {
                Some(l) => i32::from(l.min(31)),
                None => o * if t.e_on { envelope } else { t.volume * 2 + 1 },
            };
            out[c] = self.dac[level as usize];
        }
        out
    }

    /// The next output sample of each channel, into `out`; with `dc`, the
    /// DC taken away (over the last 1,024 samples).
    pub fn process(&mut self, dc: bool) {
        let base = FIR - self.fir_index * DECIMATE;
        self.fir_index = (self.fir_index + 1) % (FIR / DECIMATE - 1);
        for i in (0..DECIMATE).rev() {
            self.x += self.step;
            if self.x >= 1.0 {
                self.x -= 1.0;
                let o = self.update_mixer();
                for (c, p) in self.paths.iter_mut().enumerate() {
                    p.y = [p.y[1], p.y[2], p.y[3], o[c]];
                    let y1 = p.y[2] - p.y[0];
                    p.c = [0.5 * p.y[1] + 0.25 * (p.y[0] + p.y[2]), 0.5 * y1, 0.25 * (p.y[3] - p.y[1] - y1)];
                }
            }
            let x = self.x;
            for p in &mut self.paths {
                p.fir[base + i] = (p.c[2] * x + p.c[1]) * x + p.c[0];
            }
        }
        for c in 0..3 {
            let p = &mut self.paths[c];
            let f = &p.fir[base..];
            let mut y = 0.125 * f[96];
            for &(k, t) in &TAPS {
                y += t * (f[k] + f[FIR - k]);
            }
            p.fir.copy_within(base..base + DECIMATE, base + FIR - DECIMATE);
            if dc {
                p.dc_sum += -p.dc[self.dc_index] + y;
                p.dc[self.dc_index] = y;
                y -= p.dc_sum / DC as f64;
            }
            self.out[c] = y;
        }
        if dc {
            self.dc_index = (self.dc_index + 1) & (DC - 1);
        }
    }
}
