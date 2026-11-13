// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// From 98fmplayer (github.com/takamichih/fmplayer), BSD 2-Clause; the notice is in `opna.rs`.

//! What the PC-98 drivers (PMD, FMP) play on: the OPNA, its timers and
//! PPZ8, wired as 98fmplayer wires them. The chips run between the
//! timers' interrupts; each interrupt runs the driver.

use crate::opna::{Opna, Taps, Timer, RATE};
use crate::ppz8::Ppz8;

pub fn u8s8(v: u8) -> i8 {
    v as i8
}

pub fn u16s16(v: u16) -> i16 {
    v as i16
}

/// The level 98fmplayer mixes PPZ8 at.
const PPZ8_MIX: u16 = 0xa000;

#[derive(Clone)]
pub struct Work {
    pub opna: Opna,
    pub timer: Timer,
    pub ppz8: Ppz8,
    /// False once the song has stopped itself.
    pub playing: bool,
    /// How often the song has come round.
    pub loop_cnt: u8,
    pub timerb: u8,
    pub timerb_cnt: u32,
    pub timerb_cnt_loop: u32,
    /// Only the timers are kept (for timing a song without playing it).
    pub dummy: bool,
}

impl Default for Work {
    fn default() -> Work {
        Work::new()
    }
}

impl Work {
    pub fn new() -> Work {
        Work {
            opna: Opna::new(),
            timer: Timer::default(),
            ppz8: Ppz8::new(RATE as u16, PPZ8_MIX),
            playing: false,
            loop_cnt: 0,
            timerb: 0,
            timerb_cnt: 0,
            timerb_cnt_loop: 0,
            dummy: false,
        }
    }

    pub fn dummy() -> Work {
        Work { dummy: true, ..Work::new() }
    }

    pub fn writereg(&mut self, addr: u32, data: u32) {
        let data = data & 0xff;
        if !self.dummy {
            self.opna.writereg(addr, data);
        }
        self.timer.writereg(addr, data);
    }

    pub fn readreg(&self, addr: u32) -> u32 {
        if self.dummy {
            0xff
        } else {
            self.opna.readreg(addr)
        }
    }

    /// The timers' status (masked to the timers and busy bit unless `a1`).
    pub fn status(&self, a1: bool) -> u8 {
        if a1 {
            self.timer.status
        } else {
            self.timer.status & 0x83
        }
    }

    /// Samples timer B takes now between interrupts.
    pub fn timerb_samples(&self) -> usize {
        (1 << 12) - (usize::from(self.timerb_reg()) << 4)
    }

    fn timerb_reg(&self) -> u8 {
        self.timer.timerb()
    }
}

/// A driver the OPNA's interrupt runs.
pub trait Interrupt {
    fn interrupt(&mut self, w: &mut Work);
}

/// The chips' next `samples` added into `buf` (interleaved stereo), exactly
/// as 98fmplayer mixes them, running the driver at each interrupt.
pub fn mix<D: Interrupt>(w: &mut Work, d: &mut D, buf: &mut [i16], samples: usize) {
    let mut at = 0;
    while at < samples {
        let n = w.timer.until(samples - at);
        w.opna.mix(&mut buf[at * 2..], n);
        w.ppz8.mix(&mut buf[at * 2..], n);
        at += n;
        if w.timer.advance_a(n) {
            d.interrupt(w);
        }
        if w.timer.advance_b(n) {
            d.interrupt(w);
        }
    }
}

/// Each part's samples as they reach the mix.
#[derive(Clone, Copy, Default)]
pub struct Out {
    pub opna: Taps,
    pub ppz8: [[i32; 2]; 8],
}

/// The chips up to and including the next timer-B interrupt, a sample at a
/// time into `f`; how many samples that was.
pub fn run_tick<D: Interrupt>(w: &mut Work, d: &mut D, mut f: impl FnMut(&Out)) -> usize {
    let mut total = 0;
    let mut out = Out::default();
    // A tick is at most 4096 samples; more means timer B is stopped.
    while total < 1 << 13 {
        let n = w.timer.until(1 << 13);
        for _ in 0..n {
            w.opna.sample(&mut out.opna);
            w.ppz8.sample(&mut out.ppz8);
            f(&out);
        }
        total += n;
        if w.timer.advance_a(n) {
            d.interrupt(w);
        }
        if w.timer.advance_b(n) {
            d.interrupt(w);
            break;
        }
    }
    total
}
