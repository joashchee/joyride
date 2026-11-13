// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The NES's expansion sound chips, written from their published
//! hardware documentation: Konami's VRC6 (two pulses of eight duties and
//! a sawtooth), Nintendo's MMC5 (two pulses as the 2A03's), Namco's 163
//! (up to eight wavetable channels sharing one output in turn) and the
//! Famicom Disk System's wavetable channel with its pitch modulator. Each
//! is clocked at the CPU's rate and keeps each channel's output apart.

/// Konami's VRC6.
#[derive(Clone, Default)]
pub struct Vrc6 {
    pulse: [Vrc6Pulse; 2],
    saw: Saw,
}

#[derive(Clone, Copy, Default)]
struct Vrc6Pulse {
    volume: u8,
    duty: u8,
    digital: bool,
    period: u16,
    enabled: bool,
    timer: u16,
    step: u8,
}

#[derive(Clone, Copy, Default)]
struct Saw {
    rate: u8,
    period: u16,
    enabled: bool,
    timer: u16,
    step: u8,
    acc: u8,
}

impl Vrc6 {
    /// Writes $9000 + `reg` (pulse 1 0-2, pulse 2 0x1000-0x1002, saw 0x2000-0x2002).
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr {
            0x9000 | 0xA000 => {
                let p = &mut self.pulse[usize::from(addr == 0xA000)];
                p.volume = v & 15;
                p.duty = (v >> 4) & 7;
                p.digital = v & 0x80 != 0;
            }
            0x9001 | 0xA001 => {
                let p = &mut self.pulse[usize::from(addr == 0xA001)];
                p.period = (p.period & 0xF00) | u16::from(v);
            }
            0x9002 | 0xA002 => {
                let p = &mut self.pulse[usize::from(addr == 0xA002)];
                p.period = (p.period & 0xFF) | (u16::from(v & 15) << 8);
                p.enabled = v & 0x80 != 0;
                if !p.enabled {
                    p.step = 15;
                }
            }
            0xB000 => self.saw.rate = v & 0x3F,
            0xB001 => self.saw.period = (self.saw.period & 0xF00) | u16::from(v),
            0xB002 => {
                let s = &mut self.saw;
                s.period = (s.period & 0xFF) | (u16::from(v & 15) << 8);
                s.enabled = v & 0x80 != 0;
                if !s.enabled {
                    s.acc = 0;
                    s.step = 0;
                }
            }
            _ => {}
        }
    }

    fn cycle(&mut self) -> [u8; 3] {
        let mut out = [0u8; 3];
        for (k, p) in self.pulse.iter_mut().enumerate() {
            if p.enabled {
                if p.timer == 0 {
                    p.timer = p.period;
                    p.step = p.step.wrapping_sub(1) & 15;
                } else {
                    p.timer -= 1;
                }
                if p.digital || p.step <= p.duty {
                    out[k] = p.volume;
                }
            }
        }
        let s = &mut self.saw;
        if s.enabled {
            if s.timer == 0 {
                s.timer = s.period;
                s.step += 1;
                if s.step & 1 == 0 {
                    s.acc = s.acc.wrapping_add(s.rate);
                }
                if s.step >= 14 {
                    s.step = 0;
                    s.acc = 0;
                }
            } else {
                s.timer -= 1;
            }
            out[2] = s.acc >> 3;
        }
        out
    }
}

/// Nintendo's MMC5: two pulses with constant volumes (as players use them).
#[derive(Clone, Default)]
pub struct Mmc5 {
    pulse: [Mmc5Pulse; 2],
    odd: bool,
}

#[derive(Clone, Copy, Default)]
struct Mmc5Pulse {
    duty: u8,
    volume: u8,
    period: u16,
    timer: u16,
    step: u8,
    enabled: bool,
}

const DUTY: [[u8; 8]; 4] = [[0, 1, 0, 0, 0, 0, 0, 0], [0, 1, 1, 0, 0, 0, 0, 0], [0, 1, 1, 1, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1, 1, 1]];

impl Mmc5 {
    /// Writes $5000 + `reg`.
    pub fn write(&mut self, reg: u16, v: u8) {
        match reg {
            0x00..=0x07 => {
                let p = &mut self.pulse[usize::from(reg >> 2)];
                match reg & 3 {
                    0 => {
                        p.duty = v >> 6;
                        p.volume = v & 15;
                    }
                    2 => p.period = (p.period & 0x700) | u16::from(v),
                    3 => {
                        p.period = (p.period & 0xFF) | (u16::from(v & 7) << 8);
                        p.step = 0;
                    }
                    _ => {}
                }
            }
            0x15 => {
                self.pulse[0].enabled = v & 1 != 0;
                self.pulse[1].enabled = v & 2 != 0;
            }
            _ => {}
        }
    }

    fn cycle(&mut self) -> [u8; 2] {
        self.odd = !self.odd;
        let mut out = [0u8; 2];
        for (k, p) in self.pulse.iter_mut().enumerate() {
            if self.odd {
                if p.timer == 0 {
                    p.timer = p.period;
                    p.step = (p.step + 7) & 7;
                } else {
                    p.timer -= 1;
                }
            }
            if p.enabled && p.period >= 8 && DUTY[usize::from(p.duty)][usize::from(p.step)] != 0 {
                out[k] = p.volume;
            }
        }
        out
    }
}

/// Namco's 163: 128 bytes of RAM holding the waves and each channel's
/// registers; one channel updated every 15 CPU cycles, in turn.
#[derive(Clone)]
pub struct N163 {
    pub ram: [u8; 128],
    addr: u8,
    increment: bool,
    timer: u8,
    current: usize,
    level: [i32; 8],
}

impl Default for N163 {
    fn default() -> N163 {
        N163 { ram: [0; 128], addr: 0, increment: false, timer: 0, current: 7, level: [0; 8] }
    }
}

impl N163 {
    /// The address port ($F800): the RAM address, bit 7 auto-increment.
    pub fn set_address(&mut self, v: u8) {
        self.addr = v & 0x7F;
        self.increment = v & 0x80 != 0;
    }

    /// The RAM address the data port is at, and whether it increments.
    pub fn addr(&self) -> u8 {
        self.addr
    }

    pub fn increments(&self) -> bool {
        self.increment
    }

    /// The data port ($4800).
    pub fn write_data(&mut self, v: u8) {
        self.ram[usize::from(self.addr)] = v;
        if self.increment {
            self.addr = (self.addr + 1) & 0x7F;
        }
    }

    /// How many channels sound (1-8).
    pub fn channels(&self) -> usize {
        usize::from((self.ram[0x7F] >> 4) & 7) + 1
    }

    fn cycle(&mut self) {
        self.timer += 1;
        if self.timer < 15 {
            return;
        }
        self.timer = 0;
        let n = self.channels();
        // Channels run from 7 down to 8 - n.
        let ch = self.current;
        let base = 0x40 + ch * 8;
        let r = &mut self.ram;
        let freq = u32::from(r[base]) | u32::from(r[base + 2]) << 8 | u32::from(r[base + 4] & 3) << 16;
        let mut phase = u32::from(r[base + 1]) | u32::from(r[base + 3]) << 8 | u32::from(r[base + 5]) << 16;
        let len = 256 - u32::from(r[base + 4] & 0xFC);
        phase = (phase + freq) % (len << 16);
        r[base + 1] = phase as u8;
        r[base + 3] = (phase >> 8) as u8;
        r[base + 5] = (phase >> 16) as u8;
        let index = (u32::from(r[base + 6]) + (phase >> 16)) & 0xFF;
        let byte = r[(index >> 1) as usize & 0x7F];
        let sample = if index & 1 == 0 { byte & 15 } else { byte >> 4 };
        let volume = i32::from(r[base + 7] & 15);
        self.level[ch] = (i32::from(sample) - 8) * volume;
        self.current = if ch == 8 - n || ch == 0 { 7 } else { ch - 1 };
    }
}

/// The Famicom Disk System's sound: a 64-step wave of 6-bit samples, a
/// volume, and a modulator bending its pitch by a 32-step table.
#[derive(Clone)]
pub struct Fds {
    pub wave: [u8; 64],
    wave_write: bool,
    volume: u8,
    pitch: u16,
    halt: bool,
    master: u8,
    acc: u32,
    pos: u8,
    mod_table: [u8; 64],
    mod_write: usize,
    mod_pitch: u16,
    mod_halt: bool,
    mod_acc: u32,
    mod_pos: u8,
    mod_counter: i8,
    mod_gain: u8,
}

impl Default for Fds {
    fn default() -> Fds {
        Fds { wave: [0; 64], wave_write: false, volume: 0, pitch: 0, halt: true, master: 0, acc: 0, pos: 0, mod_table: [0; 64], mod_write: 0, mod_pitch: 0, mod_halt: true, mod_acc: 0, mod_pos: 0, mod_counter: 0, mod_gain: 0 }
    }
}

impl Fds {
    /// Writes $4040 to $408A.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr {
            0x4040..=0x407F => {
                if self.wave_write {
                    self.wave[usize::from(addr - 0x4040)] = v & 0x3F;
                }
            }
            0x4080 => {
                if v & 0x80 != 0 {
                    self.volume = (v & 0x3F).min(32);
                }
            }
            0x4082 => self.pitch = (self.pitch & 0xF00) | u16::from(v),
            0x4083 => {
                self.pitch = (self.pitch & 0xFF) | (u16::from(v & 15) << 8);
                self.halt = v & 0x80 != 0;
                if self.halt {
                    self.acc = 0;
                    self.pos = 0;
                }
            }
            0x4084 => {
                if v & 0x80 != 0 {
                    self.mod_gain = v & 0x3F;
                }
            }
            0x4085 => self.mod_counter = ((v & 0x7F) << 1) as i8 >> 1,
            0x4086 => self.mod_pitch = (self.mod_pitch & 0xF00) | u16::from(v),
            0x4087 => {
                self.mod_pitch = (self.mod_pitch & 0xFF) | (u16::from(v & 15) << 8);
                self.mod_halt = v & 0x80 != 0;
                if self.mod_halt {
                    self.mod_write = usize::from(self.mod_pos) & 62;
                }
            }
            0x4088 => {
                if self.mod_halt {
                    // Each entry fills two steps.
                    let i = self.mod_write & 63;
                    self.mod_table[i] = v & 7;
                    self.mod_table[(i + 1) & 63] = v & 7;
                    self.mod_write = (i + 2) & 63;
                }
            }
            0x4089 => {
                self.master = v & 3;
                self.wave_write = v & 0x80 != 0;
            }
            _ => {}
        }
    }

    /// The pitch with the modulator's bend, as the hardware works it out.
    fn bent(&self) -> u32 {
        let p = i32::from(self.pitch);
        let mut temp = i32::from(self.mod_counter) * i32::from(self.mod_gain);
        let rem = temp & 15;
        temp >>= 4;
        if rem > 0 && temp & 0x80 == 0 {
            temp += if self.mod_counter < 0 { -1 } else { 2 };
        }
        if temp >= 192 {
            temp -= 256;
        } else if temp < -64 {
            temp += 256;
        }
        let mut t = p * temp;
        let rem = t & 0x3F;
        t >>= 6;
        if rem >= 32 {
            t += 1;
        }
        (p + t).max(0) as u32
    }

    fn cycle(&mut self) -> i32 {
        if !self.mod_halt && self.mod_pitch > 0 {
            self.mod_acc += u32::from(self.mod_pitch);
            if self.mod_acc >= 0x10000 {
                self.mod_acc -= 0x10000;
                let step = self.mod_table[usize::from(self.mod_pos & 63)];
                self.mod_counter = match step {
                    4 => 0,
                    s => {
                        let d = [0, 1, 2, 4, 0, -4, -2, -1][usize::from(s)];
                        (((i32::from(self.mod_counter) + d) & 0x7F) << 1) as i8 >> 1
                    }
                };
                self.mod_pos = (self.mod_pos + 1) & 63;
            }
        }
        if !self.halt && !self.wave_write {
            self.acc += if self.mod_halt || self.mod_gain == 0 { u32::from(self.pitch) } else { self.bent() };
            if self.acc >= 0x10000 {
                self.pos = (self.pos + (self.acc >> 16) as u8) & 63;
                self.acc &= 0xFFFF;
            }
        }
        let master = [30, 20, 15, 12][usize::from(self.master)];
        i32::from(self.wave[usize::from(self.pos)]) * i32::from(self.volume.min(32)) * master / 30
    }
}

/// The expansion chips a module uses, clocked together.
#[derive(Default)]
pub struct Expansion {
    pub vrc6: Option<Vrc6>,
    pub mmc5: Option<Mmc5>,
    pub n163: Option<N163>,
    pub fds: Option<Fds>,
    clock: f64,
    owed: f64,
}

impl Expansion {
    pub fn new(clock: f64, vrc6: bool, mmc5: bool, n163: bool, fds: bool) -> Expansion {
        Expansion { vrc6: vrc6.then(Vrc6::default), mmc5: mmc5.then(Mmc5::default), n163: n163.then(N163::default), fds: fds.then(Fds::default), clock, owed: 0.0 }
    }

    /// The next frames at `rate`: VRC6 (2 pulses, saw), MMC5 (2 pulses),
    /// N163 (8 channels) and FDS, each from about -1 to 1.
    pub fn render(&mut self, rate: u32, out: &mut [Vec<f32>; 14], frames: usize) {
        for o in out.iter_mut() {
            o.clear();
            o.resize(frames, 0.0);
        }
        let step = self.clock / f64::from(rate.max(1));
        for i in 0..frames {
            self.owed += step;
            let n = self.owed.floor();
            self.owed -= n;
            let n = (n as u32).max(1);
            let mut acc = [0f32; 14];
            for _ in 0..n {
                if let Some(v) = self.vrc6.as_mut() {
                    let o = v.cycle();
                    acc[0] += f32::from(o[0]);
                    acc[1] += f32::from(o[1]);
                    acc[2] += f32::from(o[2]);
                }
                if let Some(m) = self.mmc5.as_mut() {
                    let o = m.cycle();
                    acc[3] += f32::from(o[0]);
                    acc[4] += f32::from(o[1]);
                }
                if let Some(c) = self.n163.as_mut() {
                    c.cycle();
                    let nch = c.channels() as f32;
                    for k in 0..8 {
                        // Channel k of the module is the chip's 7 - k.
                        acc[5 + k] += c.level[7 - k] as f32 / nch;
                    }
                }
                if let Some(f) = self.fds.as_mut() {
                    acc[13] += f.cycle() as f32;
                }
            }
            let d = n as f32;
            out[0][i] = acc[0] / d / 15.0;
            out[1][i] = acc[1] / d / 15.0;
            out[2][i] = acc[2] / d / 31.0;
            out[3][i] = acc[3] / d / 15.0;
            out[4][i] = acc[4] / d / 15.0;
            for k in 0..8 {
                out[5 + k][i] = acc[5 + k] / d / 120.0;
            }
            out[13][i] = acc[13] / d / (63.0 * 32.0);
        }
    }
}
