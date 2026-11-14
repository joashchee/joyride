// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The NES's sound: the 2A03's APU (two pulse channels with envelopes and
//! sweeps, the triangle, the noise and the delta modulation channel),
//! written from the console's published hardware documentation. Clocked
//! at the CPU's rate (NTSC or PAL), its frame counter giving envelopes,
//! lengths and sweeps their quarter and half frames; each channel's
//! output kept apart at the level the mixer's linear approximation gives
//! it.

/// The NTSC NES's CPU clock.
pub const NTSC_HZ: f64 = 236_250_000.0 / 11.0 / 12.0;
/// The PAL NES's CPU clock.
pub const PAL_HZ: f64 = 26_601_712.5 / 16.0;

const DUTY: [[u8; 8]; 4] = [[0, 1, 0, 0, 0, 0, 0, 0], [0, 1, 1, 0, 0, 0, 0, 0], [0, 1, 1, 1, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1, 1, 1]];
pub(crate) const LENGTHS: [u8; 32] = [10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30];
const TRIANGLE: [u8; 32] = [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const NOISE_NTSC: [u16; 16] = [4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];
const NOISE_PAL: [u16; 16] = [4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708, 944, 1890, 3778];
pub(crate) const DMC_NTSC: [u16; 16] = [428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54];
pub(crate) const DMC_PAL: [u16; 16] = [398, 354, 316, 298, 276, 236, 210, 198, 176, 148, 132, 118, 98, 78, 66, 50];
/// The frame counter's steps (4-step mode), in CPU cycles.
pub(crate) const FRAME_NTSC: [u32; 4] = [7457, 14913, 22371, 29829];
pub(crate) const FRAME_PAL: [u32; 4] = [8313, 16627, 24939, 33253];

#[derive(Clone, Copy, Default)]
struct Envelope {
    start: bool,
    divider: u8,
    decay: u8,
    period: u8,
    looping: bool,
    constant: bool,
}

impl Envelope {
    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.period;
        } else if self.divider == 0 {
            self.divider = self.period;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looping {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    fn volume(&self) -> u8 {
        if self.constant { self.period } else { self.decay }
    }
}

#[derive(Clone, Copy, Default)]
struct Pulse {
    second: bool,
    duty: u8,
    env: Envelope,
    halt: bool,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,
    period: u16,
    timer: u16,
    step: u8,
    length: u8,
    enabled: bool,
}

impl Pulse {
    fn target(&self) -> i32 {
        let change = i32::from(self.period >> self.sweep_shift);
        let p = i32::from(self.period);
        if self.sweep_negate {
            p - change - i32::from(!self.second)
        } else {
            p + change
        }
    }

    fn muted(&self) -> bool {
        self.period < 8 || self.target() > 0x7FF
    }

    fn clock_sweep(&mut self) {
        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0 && !self.muted() {
            self.period = self.target().max(0) as u16;
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.length == 0 || self.muted() || DUTY[usize::from(self.duty)][usize::from(self.step)] == 0 {
            0
        } else {
            self.env.volume()
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Triangle {
    control: bool,
    linear_reload_value: u8,
    linear: u8,
    linear_reload: bool,
    period: u16,
    timer: u16,
    step: u8,
    length: u8,
    enabled: bool,
}

#[derive(Clone, Copy, Default)]
struct Noise {
    env: Envelope,
    halt: bool,
    mode: bool,
    period: u16,
    timer: u16,
    shift: u16,
    length: u8,
    enabled: bool,
}

#[derive(Clone, Copy, Default)]
struct Dmc {
    looping: bool,
    rate: u16,
    timer: u16,
    level: u8,
    address: u16,
    length: u16,
    current: u16,
    remaining: u16,
    buffer: Option<u8>,
    shift: u8,
    bits: u8,
    silent: bool,
}

/// The 2A03's APU.
pub struct Apu {
    pal: bool,
    pulse: [Pulse; 2],
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,
    /// The DMC's memory, $8000 to $FFFF.
    pub memory: Vec<u8>,
    frame_cycle: u32,
    odd: bool,
    clock: f64,
    owed: f64,
    /// CPU cycles run since it started.
    pub cycles: u64,
    /// Writes and memory changes to make at a cycle still to come ([`Apu::at`]), in order.
    pending: std::collections::VecDeque<(u64, Pending)>,
}

/// A change made at its cycle: a register written, or the DMC's memory changed (bytes from an offset into
/// [`Apu::memory`], for a bank switched in or RAM written).
#[derive(Clone, Debug)]
pub enum Pending {
    Reg(u16, u8),
    Mem(usize, Vec<u8>),
}

impl Apu {
    pub fn new(pal: bool) -> Apu {
        let mut a = Apu {
            pal,
            pulse: [Pulse::default(), Pulse { second: true, ..Pulse::default() }],
            triangle: Triangle::default(),
            noise: Noise { shift: 1, period: 4, ..Noise::default() },
            dmc: Dmc { rate: 428, silent: true, bits: 8, ..Dmc::default() },
            memory: vec![0; 0x8000],
            frame_cycle: 0,
            odd: false,
            clock: if pal { PAL_HZ } else { NTSC_HZ },
            owed: 0.0,
            cycles: 0,
            pending: std::collections::VecDeque::new(),
        };
        a.dmc.rate = if pal { DMC_PAL[0] } else { DMC_NTSC[0] };
        a
    }

    pub fn clock_hz(&self) -> f64 {
        self.clock
    }

    /// A change to make when [`Apu::cycles`] reaches `cycle` while it renders (at once if that has gone by).
    pub fn at(&mut self, cycle: u64, p: Pending) {
        self.pending.push_back((cycle, p));
    }

    /// Makes now every change due before `cycle` (those rendering didn't reach).
    pub fn flush_before(&mut self, cycle: u64) {
        while self.pending.front().is_some_and(|&(c, _)| c < cycle) {
            let (_, p) = self.pending.pop_front().unwrap();
            self.apply(p);
        }
    }

    fn apply(&mut self, p: Pending) {
        match p {
            Pending::Reg(r, v) => self.write(r, v),
            Pending::Mem(at, b) => {
                let n = b.len().min(self.memory.len().saturating_sub(at));
                self.memory[at..at + n].copy_from_slice(&b[..n]);
            }
        }
    }

    /// Writes register $4000 + `reg` (0x00 to 0x17).
    pub fn write(&mut self, reg: u16, v: u8) {
        match reg {
            0x00..=0x07 => {
                let p = &mut self.pulse[usize::from(reg >> 2)];
                match reg & 3 {
                    0 => {
                        p.duty = v >> 6;
                        p.halt = v & 0x20 != 0;
                        p.env.looping = p.halt;
                        p.env.constant = v & 0x10 != 0;
                        p.env.period = v & 0x0F;
                    }
                    1 => {
                        p.sweep_enabled = v & 0x80 != 0;
                        p.sweep_period = (v >> 4) & 7;
                        p.sweep_negate = v & 0x08 != 0;
                        p.sweep_shift = v & 7;
                        p.sweep_reload = true;
                    }
                    2 => p.period = (p.period & 0x700) | u16::from(v),
                    _ => {
                        p.period = (p.period & 0xFF) | (u16::from(v & 7) << 8);
                        if p.enabled {
                            p.length = LENGTHS[usize::from(v >> 3)];
                        }
                        p.step = 0;
                        p.env.start = true;
                    }
                }
            }
            0x08 => {
                let t = &mut self.triangle;
                t.control = v & 0x80 != 0;
                t.linear_reload_value = v & 0x7F;
            }
            0x0A => self.triangle.period = (self.triangle.period & 0x700) | u16::from(v),
            0x0B => {
                let t = &mut self.triangle;
                t.period = (t.period & 0xFF) | (u16::from(v & 7) << 8);
                if t.enabled {
                    t.length = LENGTHS[usize::from(v >> 3)];
                }
                t.linear_reload = true;
            }
            0x0C => {
                let n = &mut self.noise;
                n.halt = v & 0x20 != 0;
                n.env.looping = n.halt;
                n.env.constant = v & 0x10 != 0;
                n.env.period = v & 0x0F;
            }
            0x0E => {
                self.noise.mode = v & 0x80 != 0;
                let t = if self.pal { &NOISE_PAL } else { &NOISE_NTSC };
                self.noise.period = t[usize::from(v & 0x0F)];
            }
            0x0F => {
                if self.noise.enabled {
                    self.noise.length = LENGTHS[usize::from(v >> 3)];
                }
                self.noise.env.start = true;
            }
            0x10 => {
                self.dmc.looping = v & 0x40 != 0;
                let t = if self.pal { &DMC_PAL } else { &DMC_NTSC };
                self.dmc.rate = t[usize::from(v & 0x0F)];
            }
            0x11 => self.dmc.level = v & 0x7F,
            0x12 => self.dmc.address = 0xC000 | (u16::from(v) << 6),
            0x13 => self.dmc.length = (u16::from(v) << 4) | 1,
            0x15 => {
                self.pulse[0].enabled = v & 1 != 0;
                self.pulse[1].enabled = v & 2 != 0;
                self.triangle.enabled = v & 4 != 0;
                self.noise.enabled = v & 8 != 0;
                for (k, p) in self.pulse.iter_mut().enumerate() {
                    if v >> k & 1 == 0 {
                        p.length = 0;
                    }
                }
                if v & 4 == 0 {
                    self.triangle.length = 0;
                }
                if v & 8 == 0 {
                    self.noise.length = 0;
                }
                if v & 0x10 == 0 {
                    self.dmc.remaining = 0;
                } else if self.dmc.remaining == 0 {
                    self.dmc.current = self.dmc.address;
                    self.dmc.remaining = self.dmc.length;
                }
            }
            0x17 => self.frame_cycle = 0,
            _ => {}
        }
    }

    fn quarter(&mut self) {
        for p in self.pulse.iter_mut() {
            p.env.clock();
        }
        self.noise.env.clock();
        let t = &mut self.triangle;
        if t.linear_reload {
            t.linear = t.linear_reload_value;
        } else if t.linear > 0 {
            t.linear -= 1;
        }
        if !t.control {
            t.linear_reload = false;
        }
    }

    fn half(&mut self) {
        for p in self.pulse.iter_mut() {
            if !p.halt && p.length > 0 {
                p.length -= 1;
            }
            p.clock_sweep();
        }
        if !self.triangle.control && self.triangle.length > 0 {
            self.triangle.length -= 1;
        }
        if !self.noise.halt && self.noise.length > 0 {
            self.noise.length -= 1;
        }
    }

    fn cycle(&mut self) {
        while self.pending.front().is_some_and(|&(c, _)| c <= self.cycles) {
            let (_, p) = self.pending.pop_front().unwrap();
            self.apply(p);
        }
        self.cycles += 1;
        // The frame counter.
        let steps = if self.pal { &FRAME_PAL } else { &FRAME_NTSC };
        self.frame_cycle += 1;
        if let Some(k) = steps.iter().position(|&s| s == self.frame_cycle) {
            self.quarter();
            if k == 1 || k == 3 {
                self.half();
            }
            if k == 3 {
                self.frame_cycle = 0;
            }
        }
        // Pulses tick every other CPU cycle.
        self.odd = !self.odd;
        if self.odd {
            for p in self.pulse.iter_mut() {
                if p.timer == 0 {
                    p.timer = p.period;
                    p.step = (p.step + 7) & 7;
                } else {
                    p.timer -= 1;
                }
            }
        }
        let t = &mut self.triangle;
        if t.timer == 0 {
            t.timer = t.period;
            if t.linear > 0 && t.length > 0 && t.period >= 2 {
                t.step = (t.step + 1) & 31;
            }
        } else {
            t.timer -= 1;
        }
        let n = &mut self.noise;
        if n.timer == 0 {
            n.timer = n.period.saturating_sub(1);
            let bit = if n.mode { 6 } else { 1 };
            let fb = (n.shift ^ (n.shift >> bit)) & 1;
            n.shift = (n.shift >> 1) | (fb << 14);
        } else {
            n.timer -= 1;
        }
        self.dmc_cycle();
    }

    fn dmc_cycle(&mut self) {
        // The sample buffer refills as soon as it's empty.
        if self.dmc.buffer.is_none() && self.dmc.remaining > 0 {
            let a = self.dmc.current;
            self.dmc.buffer = Some(self.memory[usize::from(a & 0x7FFF)]);
            self.dmc.current = if a == 0xFFFF { 0x8000 } else { a + 1 };
            self.dmc.remaining -= 1;
            if self.dmc.remaining == 0 && self.dmc.looping {
                self.dmc.current = self.dmc.address;
                self.dmc.remaining = self.dmc.length;
            }
        }
        let d = &mut self.dmc;
        if d.timer == 0 {
            d.timer = d.rate.saturating_sub(1);
            if !d.silent {
                if d.shift & 1 != 0 {
                    if d.level <= 125 {
                        d.level += 2;
                    }
                } else if d.level >= 2 {
                    d.level -= 2;
                }
            }
            d.shift >>= 1;
            d.bits -= 1;
            if d.bits == 0 {
                d.bits = 8;
                match d.buffer.take() {
                    Some(b) => {
                        d.shift = b;
                        d.silent = false;
                    }
                    None => d.silent = true,
                }
            }
        } else {
            d.timer -= 1;
        }
    }

    /// Whether the DMC is playing a sample.
    pub fn dmc_playing(&self) -> bool {
        self.dmc.remaining > 0
    }

    /// The next frames at `rate`, each channel's own (pulse 1, pulse 2,
    /// triangle, noise, DMC), as the mixer's linear approximation weighs
    /// them (a pulse at full volume 0.113, the DMC at its top 0.425).
    pub fn render(&mut self, rate: u32, out: &mut [&mut [f32]; 5]) {
        let frames = out.iter().map(|o| o.len()).min().unwrap_or(0);
        let step = self.clock / f64::from(rate.max(1));
        for i in 0..frames {
            self.owed += step;
            let n = self.owed.floor();
            self.owed -= n;
            let n = n as u32;
            let mut acc = [0u32; 5];
            for _ in 0..n {
                self.cycle();
                acc[0] += u32::from(self.pulse[0].output());
                acc[1] += u32::from(self.pulse[1].output());
                let t = &self.triangle;
                acc[2] += u32::from(TRIANGLE[usize::from(t.step)]);
                let nz = &self.noise;
                if nz.length > 0 && nz.shift & 1 == 0 {
                    acc[3] += u32::from(nz.env.volume());
                }
                acc[4] += u32::from(self.dmc.level);
            }
            let d = n.max(1) as f32;
            out[0][i] = acc[0] as f32 / d * 0.00752;
            out[1][i] = acc[1] as f32 / d * 0.00752;
            out[2][i] = acc[2] as f32 / d * 0.00851;
            out[3][i] = acc[3] as f32 / d * 0.00494;
            out[4][i] = acc[4] as f32 / d * 0.00335;
        }
    }
}
