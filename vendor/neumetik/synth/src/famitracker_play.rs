// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FamiTracker's player: the tempo system its documentation describes
//! (an accumulator decremented by tempo × 24 / speed every engine tick),
//! rows of notes, instruments run as sequences (volume, arpeggio, pitch,
//! hi-pitch, duty) and the effects, played on the 2A03 and the expansion
//! chips. Written from FamiTracker's and Dn-FamiTracker's documentation,
//! its timing measured on its modules' output, not from its code.

use std::sync::Arc;

use crate::ay::Ay;
use crate::famitracker::*;
use crate::nes::Apu;
use crate::nes_ext::Expansion;
use crate::opll::Opll;
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// A FamiTracker module ready to play.
pub struct Song {
    pub module: Arc<Module>,
    file: Arc<Vec<u8>>,
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        Some(Song { module: Arc::new(Module::parse(d)?), file: Arc::new(d.to_vec()) })
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "ftm"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn songs(&self) -> usize {
        self.module.tracks.len().max(1)
    }
    fn driver(&self, song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.module.clone(), song))
    }

    /// Each instrument: its chip, its sequences' contents (by type, not
    /// number), the DPCM samples its notes play (their bytes), and the
    /// VRC7 patch, FDS wave and modulation, or N163 waves it has.
    fn patches(&self) -> Vec<Patch> {
        let m = &self.module;
        let mut keys: Vec<&u8> = m.instruments.keys().collect();
        keys.sort_unstable();
        let mut out = Vec::new();
        let seq = |b: &mut PatchBytes, q: &crate::famitracker::Sequence| {
            b.u16(q.values.len() as u16);
            for &v in &q.values {
                b.u8(v as u8);
            }
            b.i32(q.loop_point).i32(q.release).u32(q.setting);
        };
        for &k in keys {
            let i = &m.instruments[&k];
            // A new 2A03 instrument (no sequences, no samples) is the chip as it is.
            if i.kind == crate::famitracker::InstrumentKind::Apu && (0..i.seq.len()).all(|t| m.sequence(i, t).is_none()) && i.dpcm.iter().all(|d| d.sample == 0) {
                continue;
            }
            let mut b = PatchBytes::default();
            b.u8(i.kind as u8);
            for t in 0..i.seq.len() {
                match m.sequence(i, t).filter(|_| i.kind != crate::famitracker::InstrumentKind::Fds) {
                    Some(q) => {
                        b.u8(1);
                        seq(&mut b, q);
                    }
                    None => {
                        b.u8(0);
                    }
                }
            }
            let mut samples: Vec<u8> = Vec::new();
            b.u16(i.dpcm.len() as u16);
            for d in &i.dpcm {
                let local = if d.sample == 0 {
                    0
                } else {
                    samples.iter().position(|&x| x == d.sample).unwrap_or_else(|| {
                        samples.push(d.sample);
                        samples.len() - 1
                    }) as u8
                        + 1
                };
                b.u8(local).u8(d.pitch).u8(d.delta as u8);
            }
            for n in samples {
                b.bytes(m.samples.get(&(n - 1)).map_or(&[][..], |s| s.as_slice()));
            }
            b.u8(i.vrc7_patch).bytes(&i.vrc7_regs).bytes(&i.fds_wave).bytes(&i.fds_mod).i32(i.fds_mod_speed).i32(i.fds_mod_depth).i32(i.fds_mod_delay);
            b.u16(i.fds_seqs.len() as u16);
            for q in &i.fds_seqs {
                seq(&mut b, q);
            }
            b.u32(i.n163_size as u32).u32(i.n163_pos as u32).u8(u8::from(i.n163_auto)).u16(i.n163_waves.len() as u16);
            for w in &i.n163_waves {
                b.bytes(w);
            }
            out.push(b.patch(usize::from(k) + 1, &i.name));
        }
        out
    }
}

/// The period of note `n` (C-0 is 0; A-3 is 440 Hz) on a pulse channel.
pub fn pulse_period(clock: f64, n: i32) -> i32 {
    let f = 440.0 * 2f64.powf(f64::from(n - 45) / 12.0);
    ((clock / 16.0 / f).round() as i32 - 1).clamp(0, 0x7FF)
}

/// How a channel turns notes into its pitch register.
#[derive(Clone, Copy)]
struct Pitch {
    clock: f64,
    chan: Chan,
    /// N163: the wave's length and the channels sounding.
    n163: (u32, u32),
    /// VRC7: the block (the playing note's octave) F-numbers are for.
    block: i32,
}

impl Pitch {
    fn hz(note: i32) -> f64 {
        440.0 * 2f64.powf(f64::from(note.clamp(0, 95) - 45) / 12.0)
    }

    /// The register value of a note (a period, or for the FDS, N163 and
    /// VRC7 a frequency).
    fn value(&self, note: i32) -> i32 {
        let f = Pitch::hz(note);
        let c = self.clock;
        match self.chan {
            Chan::Noise => 15 - (note & 15),
            Chan::Dpcm => note,
            Chan::Vrc6Pulse(_) => ((c / 16.0 / f).round() as i32 - 1).clamp(0, 0xFFF),
            Chan::Vrc6Saw => ((c / 14.0 / f).round() as i32 - 1).clamp(0, 0xFFF),
            // As FamiTracker modules' FDS pitches measure (a sixteenth of the hardware's wave rate).
            Chan::Fds => ((f * 262_144.0 / c).round() as i32).clamp(0, 0xFFF),
            Chan::N163(_) => {
                let (len, nch) = self.n163;
                ((f * 15.0 * f64::from(nch.max(1)) * f64::from(len.max(4)) * 65536.0 / c).round() as i32).clamp(0, 0x3FFFF)
            }
            // The F-number in the note's block, two octaves under the 2A03's
            // pitch for the note (as FamiTracker modules' VRC7 measures).
            Chan::Vrc7(_) => (f / 4.0 * 524_288.0 / 49_716.0 / 2f64.powi(self.block - 1)).floor() as i32,
            Chan::S5b(_) => ((c / 32.0 / f).round() as i32).clamp(1, 0xFFF),
            _ => pulse_period(c, note),
        }
    }

    /// Whether the register rises with the pitch.
    fn rising(&self) -> bool {
        matches!(self.chan, Chan::Fds | Chan::N163(_) | Chan::Vrc7(_))
    }

    fn limits(&self) -> (i32, i32) {
        match self.chan {
            Chan::Pulse(_) | Chan::Triangle | Chan::Mmc5Pulse(_) => (3, 0x7FF),
            Chan::Vrc6Pulse(_) | Chan::Vrc6Saw | Chan::Fds | Chan::S5b(_) => (0, 0xFFF),
            Chan::N163(_) => (0, 0x3FFFF),
            Chan::Vrc7(_) => (0, 1023),
            _ => (0, i32::MAX),
        }
    }
}

/// A running sequence.
#[derive(Clone, Copy, Debug, Default)]
struct SeqState {
    active: bool,
    pos: usize,
    value: i32,
}

/// A channel's state.
#[derive(Clone, Debug)]
struct Ch {
    chan: Chan,
    instrument: Option<u8>,
    /// The note playing (C-0 is 0), and whether it sounds.
    note: i32,
    on: bool,
    released: bool,
    /// The period before effects and sequences.
    period: i32,
    volume: i32,
    duty: i32,
    seq: [SeqState; 5],
    // Effects.
    arp: (i32, i32),
    arp_phase: usize,
    slide_up: i32,
    slide_down: i32,
    porta_speed: i32,
    porta_target: i32,
    vib_speed: i32,
    vib_depth: i32,
    vib_phase: i32,
    trem_speed: i32,
    trem_depth: i32,
    trem_phase: i32,
    vol_slide: i32,
    /// The volume in quarter steps (for Axy).
    vol_fine: i32,
    fine_pitch: i32,
    note_slide: (i32, i32),
    cut: Option<i32>,
    release_in: Option<i32>,
    delay: Option<(i32, Row)>,
    transpose: Option<(i32, i32)>,
    delayed_volume: Option<(i32, i32)>,
    sweep: Option<u8>,
    sweep_written: bool,
    /// What was last written for a pulse's high period byte (-1 none).
    last_hi: i32,
    // DPCM.
    dpcm: Option<DpcmKey>,
    dpcm_offset: u8,
    dpcm_pitch: Option<u8>,
    dac: Option<u8>,
    retrigger: Option<(i32, i32)>,
    triggered: bool,
    /// Whether it has played a note yet (its period isn't written before).
    started: bool,
}

impl Ch {
    fn new(chan: Chan) -> Ch {
        Ch {
            chan,
            instrument: None,
            note: 0,
            on: false,
            released: false,
            period: 0,
            volume: 15,
            duty: 0,
            seq: [SeqState::default(); 5],
            arp: (0, 0),
            arp_phase: 0,
            slide_up: 0,
            slide_down: 0,
            porta_speed: 0,
            porta_target: 0,
            vib_speed: 0,
            vib_depth: 0,
            vib_phase: 0,
            trem_speed: 0,
            trem_depth: 0,
            trem_phase: 0,
            vol_slide: 0,
            vol_fine: 15 << 2,
            fine_pitch: 0x80,
            note_slide: (0, 0),
            cut: None,
            release_in: None,
            delay: None,
            transpose: None,
            delayed_volume: None,
            sweep: None,
            sweep_written: false,
            last_hi: -1,
            dpcm: None,
            dpcm_offset: 0,
            dpcm_pitch: None,
            dac: None,
            retrigger: None,
            triggered: false,
            started: false,
        }
    }
}

struct Player {
    m: Arc<Module>,
    track: usize,
    frame: usize,
    row: usize,
    /// Where the next row read comes from.
    next: (usize, usize),
    speed: i32,
    tempo: i32,
    accum: i32,
    groove: Option<(u8, usize)>,
    halted: bool,
    ch: Vec<Ch>,
    apu: Apu,
    ext: Expansion,
    opll: Option<Opll>,
    ay: Option<Ay>,
    bufs: [Vec<f32>; 5],
    ext_bufs: [Vec<f32>; 14],
    dc: Vec<(f32, f32)>,
    /// Where each DPCM sample sits in the DMC's memory ($C000 up): (address byte, length byte).
    sample_at: std::collections::HashMap<u8, (u8, u8)>,
    shown: (usize, usize),
    seen: Vec<Vec<bool>>,
    done: bool,
    regs: [u8; 0x18],
    ay_regs: [u8; 14],
    /// The FDS's registers $4080 to $408A, as last written.
    fds_regs: [u8; 11],
    /// The VRC7's registers, to set again when its rate changes, and its rate.
    opll_regs: [u8; 0x40],
    opll_rate: u32,
}

impl Player {
    fn new(m: Arc<Module>, track: usize) -> Player {
        let track = track.min(m.tracks.len().saturating_sub(1));
        let t = &m.tracks[track];
        let ch = m.channels.iter().map(|&c| Ch::new(c)).collect();
        let mut apu = Apu::new(m.pal);
        // The samples laid out from $C000, each on a 64-byte boundary.
        let mut sample_at = std::collections::HashMap::new();
        let mut at = 0usize;
        let mut keys: Vec<&u8> = m.samples.keys().collect();
        keys.sort();
        for &k in keys {
            let s = &m.samples[&k];
            if at + s.len() > 0x4000 {
                break;
            }
            apu.memory[0x4000 + at..0x4000 + at + s.len()].copy_from_slice(s);
            let len = (s.len().saturating_sub(1) / 16).min(255) as u8;
            sample_at.insert(k, ((at / 64) as u8, len));
            at = (at + s.len()).div_ceil(64) * 64;
        }
        let frames = t.frames.len();
        let mut p = Player {
            track,
            frame: 0,
            row: 0,
            next: (0, 0),
            speed: t.speed as i32,
            tempo: t.tempo as i32,
            accum: 0,
            groove: None,
            halted: false,
            ch,
            ext: Expansion::new(if m.pal { crate::nes::PAL_HZ } else { crate::nes::NTSC_HZ }, m.expansion & 1 != 0, m.expansion & 8 != 0, m.expansion & 16 != 0, m.expansion & 4 != 0),
            opll: (m.expansion & 2 != 0).then(|| {
                let mut o = Opll::new(3_579_545, 49_716);
                o.set_vrc7();
                o
            }),
            ay: (m.expansion & 32 != 0).then(|| Ay::new(true, (if m.pal { crate::nes::PAL_HZ } else { crate::nes::NTSC_HZ }) / 2.0, 44100)),
            apu,
            bufs: Default::default(),
            ext_bufs: Default::default(),
            dc: vec![(0.0, 0.0); 32],
            sample_at,
            shown: (0, 0),
            seen: vec![vec![false; 256]; frames.max(1)],
            done: false,
            regs: [0; 0x18],
            ay_regs: [0; 14],
            fds_regs: [0; 11],
            opll_regs: [0; 0x40],
            opll_rate: 49_716,
            m,
        };
        if p.m.tracks[track].groove {
            p.groove = Some((p.speed as u8, 0));
            p.speed = p.groove_speed();
        }
        p.write(0x15, 0x1F);
        p.write(0x17, 0x00);
        if let Some(m) = p.ext.mmc5.as_mut() {
            m.write(0x15, 0x03);
        }
        if let Some(n) = p.ext.n163.as_mut() {
            // The channels sounding, in register $7F.
            n.ram[0x7F] = ((p.m.n163_channels.clamp(1, 8) - 1) as u8) << 4;
        }
        if let Some(a) = p.ay.as_mut() {
            let mut r = [0u8; 14];
            r[7] = 0x3F;
            a.write_regs(&r, false);
        }
        p.ay_regs[7] = 0x3F;
        p
    }

    fn pitch(&self, c: usize) -> Pitch {
        let n163 = match self.ch[c].chan {
            Chan::N163(_) => {
                let len = self.instrument(c).filter(|i| i.kind == InstrumentKind::N163).map_or(32, |i| i.n163_size) as u32;
                (len, self.m.n163_channels.clamp(1, 8) as u32)
            }
            _ => (0, 0),
        };
        Pitch { clock: self.clock(), chan: self.ch[c].chan, n163, block: (self.ch[c].note / 12).clamp(0, 7) }
    }

    fn write(&mut self, reg: u16, v: u8) {
        if let Some(r) = self.regs.get_mut(usize::from(reg)) {
            *r = v;
        }
        self.apu.write(reg, v);
    }

    fn engine_hz(&self) -> f64 {
        match self.m.engine_speed {
            0 => {
                if self.m.pal {
                    50.0
                } else {
                    60.0
                }
            }
            hz => f64::from(hz),
        }
    }

    fn clock(&self) -> f64 {
        self.apu.clock_hz()
    }

    fn groove_speed(&self) -> i32 {
        let Some((g, pos)) = self.groove else { return self.speed };
        let Some(v) = self.m.grooves.get(&g).filter(|v| !v.is_empty()) else { return self.speed };
        i32::from(v[pos % v.len()]).max(1)
    }

    /// The tempo's decrement and remainder.
    fn tempo_step(&self) -> (i32, i32) {
        let speed = self.speed.max(1);
        let t = self.tempo.max(1) * 24;
        (t / speed, t % speed)
    }

    fn tick_once(&mut self) {
        if self.halted {
            return;
        }
        if self.accum <= 0 {
            let (_, rem) = self.tempo_step();
            let divider = (self.engine_hz() * 60.0) as i32;
            self.accum += divider - rem;
            self.read_row();
        }
        for c in 0..self.ch.len() {
            self.update_channel(c);
        }
        let (dec, _) = self.tempo_step();
        self.accum -= dec;
    }

    fn read_row(&mut self) {
        let (frame, row) = self.next;
        self.frame = frame;
        self.row = row;
        self.shown = (frame, row);
        if let Some(seen) = self.seen.get_mut(frame).and_then(|f| f.get_mut(row)) {
            if std::mem::replace(seen, true) {
                self.done = true;
            }
        }
        let m = self.m.clone();
        let t = &m.tracks[self.track];
        let rows: Vec<Row> = (0..self.ch.len()).map(|c| t.row(frame, c, row)).collect();
        // The row's global effects: speed, tempo, jumps, halt.
        let (mut jump, mut skip) = (None, None);
        for r in &rows {
            for &(e, p) in &r.fx {
                match e {
                    EF_SPEED => {
                        if u32::from(p) < m.speed_split || t.groove && false {
                            self.speed = i32::from(p).max(1);
                            self.groove = None;
                        } else {
                            self.tempo = i32::from(p);
                        }
                    }
                    EF_GROOVE => {
                        if m.grooves.contains_key(&p) {
                            self.groove = Some((p, 0));
                        }
                    }
                    EF_JUMP => jump = Some(usize::from(p)),
                    EF_SKIP => skip = Some(usize::from(p)),
                    EF_HALT => self.halted = true,
                    _ => {}
                }
            }
        }
        if self.groove.is_some() {
            self.speed = self.groove_speed();
            if let Some((g, pos)) = self.groove.as_mut() {
                let _ = g;
                *pos += 1;
            }
        }
        for (c, r) in rows.iter().enumerate() {
            self.play_row(c, *r);
        }
        // Where the next row comes from.
        let frames = t.frames.len().max(1);
        self.next = if let Some(f) = jump {
            (f.min(frames - 1), skip.unwrap_or(0))
        } else if let Some(r) = skip {
            ((frame + 1) % frames, r)
        } else if row + 1 >= t.rows {
            ((frame + 1) % frames, 0)
        } else {
            (frame, row + 1)
        };
        if self.next.1 >= t.rows {
            self.next.1 = 0;
        }
        if self.next.0 < frame || (jump.is_some() && self.next.0 <= frame) {
            self.done = true;
        }
    }

    fn instrument(&self, c: usize) -> Option<&Instrument> {
        self.ch[c].instrument.and_then(|i| self.m.instruments.get(&i))
    }

    fn base_period(&self, c: usize, note: i32) -> i32 {
        self.pitch(c).value(note)
    }

    fn play_row(&mut self, c: usize, r: Row) {
        // A delay (Gxx) holds the whole row back.
        if let Some(&(_, p)) = r.fx.iter().find(|f| f.0 == EF_DELAY && f.1 > 0) {
            self.ch[c].delay = Some((i32::from(p), r));
            return;
        }
        self.apply_row(c, r);
    }

    fn apply_row(&mut self, c: usize, r: Row) {
        let m = self.m.clone();
        // Effects first (some change how the note plays).
        let mut porta_now = self.ch[c].porta_speed > 0;
        for &(e, p) in &r.fx {
            let pi = i32::from(p);
            let ch = &mut self.ch[c];
            match e {
                EF_VOLUME => {}
                EF_PORTAMENTO => {
                    ch.porta_speed = pi;
                    porta_now = pi > 0;
                    if pi > 0 {
                        ch.slide_up = 0;
                        ch.slide_down = 0;
                    }
                }
                EF_ARPEGGIO => {
                    ch.arp = (pi >> 4, pi & 15);
                    ch.arp_phase = 0;
                }
                EF_VIBRATO => {
                    ch.vib_speed = pi >> 4;
                    ch.vib_depth = pi & 15;
                }
                EF_TREMOLO => {
                    ch.trem_speed = pi >> 4;
                    ch.trem_depth = pi & 15;
                }
                EF_PITCH => ch.fine_pitch = pi,
                EF_PORTA_UP => {
                    ch.slide_up = pi;
                    ch.slide_down = 0;
                    ch.porta_speed = 0;
                }
                EF_PORTA_DOWN => {
                    ch.slide_down = pi;
                    ch.slide_up = 0;
                    ch.porta_speed = 0;
                }
                EF_DUTY_CYCLE => ch.duty = pi,
                EF_VOLUME_SLIDE => ch.vol_slide = pi,
                EF_NOTE_CUT => ch.cut = (pi > 0).then_some(pi),
                EF_NOTE_RELEASE => ch.release_in = Some(pi),
                EF_SLIDE_UP => ch.note_slide = ((pi >> 4).max(1) * 4, pi & 15),
                EF_SLIDE_DOWN => ch.note_slide = ((pi >> 4).max(1) * 4, -(pi & 15)),
                EF_SWEEPUP | EF_SWEEPDOWN => {
                    let neg = if e == EF_SWEEPDOWN { 0 } else { 8 };
                    ch.sweep = Some(0x80 | ((p >> 4) & 7) << 4 | neg | (p & 7));
                    ch.sweep_written = false;
                }
                EF_SAMPLE_OFFSET => ch.dpcm_offset = p,
                EF_DPCM_PITCH => ch.dpcm_pitch = Some(p & 15),
                EF_DAC => ch.dac = Some(p & 0x7F),
                EF_RETRIGGER => ch.retrigger = Some((pi.max(1), pi.max(1))),
                EF_TRANSPOSE => ch.transpose = Some((pi >> 4, pi & 15)),
                EF_DELAYED_VOLUME => ch.delayed_volume = Some((pi >> 4, pi & 15)),
                _ => {}
            }
        }
        if r.volume != NO_VOLUME {
            self.ch[c].volume = i32::from(r.volume & 0x0F);
            self.ch[c].vol_fine = self.ch[c].volume << 2;
        }
        if r.instrument != NO_INSTRUMENT && r.instrument != HOLD_INSTRUMENT && m.instruments.contains_key(&r.instrument) {
            self.ch[c].instrument = Some(r.instrument);
        }
        match r.note {
            1..=12 => {
                let note = i32::from(r.octave) * 12 + i32::from(r.note) - 1;
                self.note_on(c, note, porta_now);
            }
            NOTE_HALT => {
                let ch = &mut self.ch[c];
                ch.on = false;
                ch.released = false;
                for s in ch.seq.iter_mut() {
                    s.active = false;
                }
            }
            NOTE_RELEASE => self.release(c),
            _ => {}
        }
    }

    fn note_on(&mut self, c: usize, note: i32, porta: bool) {
        // The VRC7's F-numbers are for the new note's block.
        let old = std::mem::replace(&mut self.ch[c].note, note);
        let period = self.base_period(c, note);
        if matches!(self.ch[c].chan, Chan::Vrc7(_)) && porta && self.ch[c].on {
            self.ch[c].note = old;
        }
        let playing = self.ch[c].on;
        let has_seq: [bool; 5] = std::array::from_fn(|t| self.instrument(c).and_then(|i| self.m.sequence(i, t)).is_some());
        let ch = &mut self.ch[c];
        ch.note = note;
        if porta && playing {
            ch.porta_target = period;
        } else {
            ch.period = period;
            ch.porta_target = period;
        }
        let legato = porta && playing;
        ch.on = true;
        ch.started = true;
        ch.released = false;
        ch.note_slide_reset();
        if legato {
            // A portamento glides on: the instrument isn't started again.
            return;
        }
        ch.vib_phase = 0;
        ch.trem_phase = 0;
        ch.triggered = true;
        for (t, s) in ch.seq.iter_mut().enumerate() {
            *s = SeqState { active: has_seq[t], pos: 0, value: 0 };
        }
        if ch.chan == Chan::Dpcm {
            ch.dpcm = None;
        }
        if let Chan::Pulse(_) = ch.chan {
            ch.sweep_written = false;
        }
    }

    fn release(&mut self, c: usize) {
        if self.ch[c].released {
            return;
        }
        let rels: [i32; 5] = std::array::from_fn(|t| self.instrument(c).and_then(|i| self.m.sequence(i, t)).map_or(-1, |s| s.release));
        let ch = &mut self.ch[c];
        ch.released = true;
        // Each sequence goes straight to its release point.
        for (t, s) in ch.seq.iter_mut().enumerate() {
            if s.active && rels[t] >= 0 {
                s.pos = rels[t] as usize;
            }
        }
    }

    /// One tick of a sequence: its value, moving on. Until the note is
    /// released it loops from its loop point to its release point (or
    /// holds there); after, it runs on to its end, looping only when its
    /// loop point is past the release point.
    fn step_seq(s: &mut SeqState, seq: &Sequence, released: bool) -> Option<i32> {
        if !s.active || seq.values.is_empty() {
            return None;
        }
        let len = seq.values.len();
        let pos = s.pos.min(len - 1);
        let v = i32::from(seq.values[pos]);
        s.value = v;
        let (lp, rel) = (seq.loop_point, seq.release.min(len as i32 - 1));
        let mut next = pos + 1;
        if !released && rel >= 0 && next as i32 > rel {
            next = if lp >= 0 && lp <= rel { lp as usize } else { rel as usize };
        } else if next >= len {
            if lp >= 0 && !(rel >= 0 && lp <= rel) {
                next = lp as usize;
            } else if lp >= 0 && rel < 0 {
                next = lp as usize;
            } else {
                s.active = false;
                next = len - 1;
            }
        }
        s.pos = next;
        Some(v)
    }

    fn update_channel(&mut self, c: usize) {
        // A delayed row.
        if let Some((n, r)) = self.ch[c].delay {
            if n <= 0 {
                self.ch[c].delay = None;
                self.apply_row(c, r);
            } else {
                self.ch[c].delay = Some((n - 1, r));
            }
        }
        if let Some(n) = self.ch[c].cut {
            if n <= 0 {
                self.ch[c].cut = None;
                let ch = &mut self.ch[c];
                ch.on = false;
            } else {
                self.ch[c].cut = Some(n - 1);
            }
        }
        if let Some(n) = self.ch[c].release_in {
            if n <= 0 {
                self.ch[c].release_in = None;
                self.release(c);
            } else {
                self.ch[c].release_in = Some(n - 1);
            }
        }
        let m = self.m.clone();
        let ins = self.instrument(c).cloned();
        let released = self.ch[c].released;
        let mut seq_vals: [Option<i32>; 5] = [None; 5];
        if let Some(ins) = &ins {
            for t in 0..5 {
                if let Some(seq) = m.sequence(ins, t) {
                    seq_vals[t] = Player::step_seq(&mut self.ch[c].seq[t], seq, released).map(|v| v);
                    if seq_vals[t].is_some() || !self.ch[c].seq[t].active {
                        // Kept for the arpeggio's and duty's last value.
                    }
                }
            }
        }
        let pitch = self.pitch(c);
        let rising = pitch.rising();
        // Pitch changes as a period's: for a frequency register, the other way.
        let dir = if rising { -1 } else { 1 };
        let (lo, hi) = pitch.limits();
        let ch = &mut self.ch[c];
        // Volume slide.
        if ch.vol_slide != 0 {
            let up = ch.vol_slide >> 4;
            let down = ch.vol_slide & 15;
            ch.vol_fine = (ch.vol_fine + up - down).clamp(0, 15 << 2);
            ch.volume = ch.vol_fine >> 2;
            // It stops at either end.
            if (up > down && ch.vol_fine == 15 << 2) || (down > up && ch.vol_fine == 0) {
                ch.vol_slide = 0;
            }
        }
        // Pitch effects on the period.
        if ch.porta_speed > 0 && ch.period != ch.porta_target {
            if ch.period < ch.porta_target {
                ch.period = (ch.period + ch.porta_speed).min(ch.porta_target);
            } else {
                ch.period = (ch.period - ch.porta_speed).max(ch.porta_target);
            }
        }
        if ch.slide_up > 0 {
            ch.period -= ch.slide_up * dir;
        }
        if ch.slide_down > 0 {
            ch.period += ch.slide_down * dir;
        }
        // Slides stop at the periods the channel can take.
        if !matches!(ch.chan, Chan::Noise | Chan::Dpcm) {
            ch.period = ch.period.clamp(lo, hi);
        }
        if ch.note_slide.0 > 0 && ch.note_slide.1 != 0 {
            let target_note = ch.note + ch.note_slide.1;
            let target = pitch.value(target_note);
            if ch.period > target {
                ch.period = (ch.period - ch.note_slide.0).max(target);
            } else {
                ch.period = (ch.period + ch.note_slide.0).min(target);
            }
            if ch.period == target {
                ch.note = target_note;
                ch.note_slide = (0, 0);
            }
        }
        // Arpeggio and the instrument's notes.
        let mut note = ch.note;
        let mut arp_active = false;
        let arp_setting = ins.as_ref().and_then(|i| m.sequence(i, 1)).map(|s| s.setting);
        match (arp_setting, seq_vals[1]) {
            (Some(2), Some(a)) => {
                arp_active = true;
                ch.note += a;
                note = ch.note;
            }
            (Some(1), _) if seq_vals[1].is_some() || ch.seq[1].pos > 0 => {
                // A fixed arpeggio holds its last note once it ends.
                arp_active = true;
                note = ch.seq[1].value;
            }
            (Some(0) | Some(3), _) if seq_vals[1].is_some() || ch.seq[1].pos > 0 => {
                arp_active = true;
                note += ch.seq[1].value;
            }
            _ => {}
        }
        if ch.arp != (0, 0) {
            // With no first step, the second takes its place.
            let (x, y) = if ch.arp.0 == 0 { (ch.arp.1, 0) } else { ch.arp };
            let add = match ch.arp_phase % 3 {
                0 => 0,
                1 => x,
                _ => y,
            };
            note += add;
            ch.arp_phase = (ch.arp_phase + 1) % 3;
            arp_active = true;
        }
        let mut period = if arp_active || matches!(ch.chan, Chan::Noise) { pitch.value(note) } else { ch.period };
        if matches!(ch.chan, Chan::Noise) {
            period = 15 - (note & 15);
        } else if arp_active && ch.period != pitch.value(ch.note) {
            // Slides keep their offset under an arpeggio.
            period += ch.period - pitch.value(ch.note);
        }
        // The instrument's pitch.
        if let Some(p) = seq_vals[2] {
            ch.period += p * dir;
            period += p * dir;
        }
        if let Some(p) = seq_vals[3] {
            ch.period += p * 16 * dir;
            period += p * 16 * dir;
        }
        // Vibrato.
        if ch.vib_depth > 0 {
            ch.vib_phase = (ch.vib_phase + ch.vib_speed) & 63;
            period -= vibrato(ch.vib_phase, ch.vib_depth) * dir;
        }
        period -= (ch.fine_pitch - 0x80) * dir;
        // Volume.
        let max_seq = if matches!(ch.chan, Chan::Fds) { 32 } else if matches!(ch.chan, Chan::Vrc6Saw) && ins.as_ref().and_then(|i| m.sequence(i, 0)).is_some_and(|s| s.setting == 1) { 63 } else { 15 };
        let fds_seq = ins.as_ref().filter(|i| i.kind == InstrumentKind::Fds).and_then(|i| i.fds_seqs.first());
        let has_vol_seq = ins.as_ref().and_then(|i| m.sequence(i, 0)).is_some() || fds_seq.is_some();
        let seq_vol = if has_vol_seq { ch.seq[0].value } else { max_seq };
        let mut vol = seq_vol.clamp(0, max_seq) * ch.volume / 15;
        if ch.trem_depth > 0 {
            // A raised cosine over 128 steps, as deep as the depth less one.
            ch.trem_phase = (ch.trem_phase + ch.trem_speed) & 127;
            let cut = (f64::from(ch.trem_depth - 1) * (1.0 - (std::f64::consts::PI * f64::from(ch.trem_phase) / 64.0).cos()) / 2.0).round() as i32;
            vol = (vol - cut).max(0);
        }
        if let Some(d) = seq_vals[4] {
            ch.duty = d;
        }
        if ch.chan == Chan::Triangle && !has_vol_seq {
            // Without a volume sequence the triangle sounds whatever its volume.
            vol = 15;
        }
        let mut duty = ch.duty;
        if !ch.on {
            // A cut channel is written silent at its note's own period.
            vol = 0;
            duty = 0;
            period = pitch.value(ch.note);
        }
        let triggered = std::mem::take(&mut ch.triggered);
        let chan = ch.chan;
        if !matches!(chan, Chan::Noise | Chan::Dpcm) {
            period = period.clamp(lo.min(0), hi);
        }
        self.output(c, chan, period, vol, duty, triggered);
    }

    fn output(&mut self, c: usize, chan: Chan, period: i32, vol: i32, duty: i32, triggered: bool) {
        let started = self.ch[c].started;
        match chan {
            Chan::Pulse(n) => {
                let base = u16::from(n) * 4;
                let period = period.clamp(0, 0x7FF);
                self.write(base, ((duty as u8 & 3) << 6) | 0x30 | vol as u8);
                let sweep = self.ch[c].sweep;
                if let Some(s) = sweep {
                    if !self.ch[c].sweep_written {
                        self.write(base + 1, s);
                        self.ch[c].sweep_written = true;
                        self.write(base + 3, ((period >> 8) as u8) | 0xF8);
                        self.ch[c].last_hi = period >> 8;
                    }
                } else {
                    self.write(base + 1, 0x08);
                }
                if !started {
                    return;
                }
                self.write(base + 2, (period & 0xFF) as u8);
                let hi = period >> 8;
                if hi != self.ch[c].last_hi {
                    self.write(base + 3, (hi as u8) | 0xF8);
                    self.ch[c].last_hi = hi;
                }
            }
            Chan::Triangle => {
                let period = period.clamp(0, 0x7FF);
                self.write(0x08, if vol > 0 { 0xFF } else { 0x00 });
                if !started {
                    return;
                }
                self.write(0x0A, (period & 0xFF) as u8);
                self.write(0x0B, ((period >> 8) as u8) | 0xF8);
            }
            Chan::Noise => {
                self.write(0x0C, ((duty as u8 & 1) << 6) | 0x30 | vol as u8);
                if !started {
                    return;
                }
                self.write(0x0E, ((duty as u8 & 1) << 7) | (period & 15) as u8);
                if triggered {
                    self.write(0x0F, 0xF8);
                }
            }
            Chan::Dpcm => self.output_dpcm(c, triggered),
            Chan::Vrc6Pulse(n) => {
                let base = 0x9000 + u16::from(n) * 0x1000;
                if let Some(v) = self.ext.vrc6.as_mut() {
                    v.write(base, ((duty as u8 & 7) << 4) | vol as u8);
                    if started {
                        v.write(base + 1, (period & 0xFF) as u8);
                        v.write(base + 2, 0x80 | ((period >> 8) & 15) as u8);
                    }
                }
            }
            Chan::Vrc6Saw => {
                // Sixteen-step volumes are doubled into the saw's rate.
                let wide = self.instrument(c).and_then(|i| self.m.sequence(i, 0)).is_some_and(|s| s.setting == 1);
                let rate = if wide { vol } else { vol << 1 };
                if let Some(v) = self.ext.vrc6.as_mut() {
                    v.write(0xB000, rate.clamp(0, 63) as u8);
                    if started {
                        v.write(0xB001, (period & 0xFF) as u8);
                        v.write(0xB002, 0x80 | ((period >> 8) & 15) as u8);
                    }
                }
            }
            Chan::Mmc5Pulse(n) => {
                let base = u16::from(n) * 4;
                let last = self.ch[c].last_hi;
                if let Some(m) = self.ext.mmc5.as_mut() {
                    m.write(base, ((duty as u8 & 3) << 6) | 0x30 | vol as u8);
                    if started {
                        m.write(base + 2, (period & 0xFF) as u8);
                        if period >> 8 != last {
                            m.write(base + 3, ((period >> 8) as u8 & 7) | 0xF8);
                        }
                    }
                }
                if started {
                    self.ch[c].last_hi = period >> 8;
                }
            }
            Chan::N163(n) => self.output_n163(c, n, period, vol, duty, triggered),
            Chan::Fds => self.output_fds(c, period, vol, triggered),
            Chan::Vrc7(n) => self.output_vrc7(c, n, period, vol, triggered),
            Chan::S5b(n) => self.output_5b(c, n, period, vol, duty),
        }
    }

    fn output_n163(&mut self, c: usize, n: u8, freq: i32, vol: i32, wave: i32, triggered: bool) {
        let ins = self.instrument(c).filter(|i| i.kind == InstrumentKind::N163).cloned();
        let nch = self.m.n163_channels.clamp(1, 8);
        let Some(chip) = self.ext.n163.as_mut() else { return };
        let base = 0x78 - usize::from(n) * 8;
        if let Some(ins) = &ins {
            let w = if self.ch[c].seq[4].active || self.ch[c].seq[4].pos > 0 { wave } else { 0 };
            if triggered || w != self.ch[c].last_hi {
                // The wave, two samples a byte, at its place in the RAM.
                if let Some(data) = ins.n163_waves.get(w.clamp(0, 63) as usize).or_else(|| ins.n163_waves.first()) {
                    for (k, &smp) in data.iter().enumerate() {
                        let at = ins.n163_pos + k;
                        let byte = &mut chip.ram[(at >> 1) & 0x7F];
                        *byte = if at & 1 == 0 { (*byte & 0xF0) | (smp & 15) } else { (*byte & 0x0F) | (smp << 4) };
                    }
                }
                self.ch[c].last_hi = w;
            }
            chip.ram[base + 6] = ins.n163_pos as u8;
            chip.ram[base + 4] = (((256 - ins.n163_size.clamp(4, 252)) as u8) & 0xFC) | ((freq >> 16) & 3) as u8;
        }
        chip.ram[base] = (freq & 0xFF) as u8;
        chip.ram[base + 2] = ((freq >> 8) & 0xFF) as u8;
        let top = if n == 0 { ((nch - 1) as u8) << 4 } else { chip.ram[base + 7] & 0xF0 };
        chip.ram[base + 7] = top | vol.clamp(0, 15) as u8;
        if n == 0 {
            chip.ram[0x7F] = (((nch - 1) as u8) << 4) | vol.clamp(0, 15) as u8;
        }
    }

    fn output_fds(&mut self, c: usize, freq: i32, vol: i32, triggered: bool) {
        let ins = self.instrument(c).filter(|i| i.kind == InstrumentKind::Fds).cloned();
        if self.ext.fds.is_none() {
            return;
        }
        let on = self.ch[c].on;
        if triggered {
            if let Some(ins) = &ins {
                self.fds_write(0x4089, 0x80);
                for (k, &v) in ins.fds_wave.iter().enumerate().take(64) {
                    self.fds_write(0x4040 + k as u16, v);
                }
                self.fds_write(0x4089, 0x00);
                self.fds_write(0x4087, 0x80);
                for k in 0..32 {
                    self.fds_write(0x4088, ins.fds_mod.get(k).copied().unwrap_or(0));
                }
                self.fds_write(0x4085, 0);
                let speed = ins.fds_mod_speed.clamp(0, 0xFFF);
                self.fds_write(0x4084, 0x80 | ins.fds_mod_depth.clamp(0, 63) as u8);
                self.fds_write(0x4086, (speed & 0xFF) as u8);
                self.fds_write(0x4087, ((speed >> 8) & 15) as u8);
            }
        }
        self.fds_write(0x4080, 0x80 | vol.clamp(0, 32) as u8);
        self.fds_write(0x4082, (freq & 0xFF) as u8);
        let _ = on;
        self.fds_write(0x4083, ((freq >> 8) & 15) as u8);
    }

    fn fds_write(&mut self, addr: u16, v: u8) {
        if (0x4080..=0x408A).contains(&addr) {
            self.fds_regs[usize::from(addr - 0x4080)] = v;
        }
        if let Some(f) = self.ext.fds.as_mut() {
            f.write(addr, v);
        }
    }

    fn output_vrc7(&mut self, c: usize, n: u8, value: i32, vol: i32, triggered: bool) {
        let ins = self.instrument(c).filter(|i| i.kind == InstrumentKind::Vrc7).cloned();
        let on = self.ch[c].on;
        let volume = self.ch[c].volume;
        if self.opll.is_none() {
            return;
        }
        let _ = vol;
        // The note's octave is the block, moved up while the F-number is past 511.
        let mut block = (self.ch[c].note / 12).clamp(0, 7);
        let mut fnum = value.max(0);
        while fnum >= 512 && block < 7 {
            fnum >>= 1;
            block += 1;
        }
        let fnum = fnum.min(511) as u8 as usize | ((fnum.min(511) as usize) & 0x100);
        let patch = ins.as_ref().map_or(1, |i| i.vrc7_patch & 15);
        if triggered {
            if let Some(i) = &ins {
                if i.vrc7_patch == 0 {
                    for (r, &v) in i.vrc7_regs.iter().enumerate() {
                        self.opll_write(r, v);
                    }
                }
            }
            // Keyed off first, so the note starts again.
            self.opll_write(0x20 + usize::from(n), 0);
        }
        self.opll_write(0x30 + usize::from(n), (patch << 4) | (15 - volume.clamp(0, 15)) as u8);
        self.opll_write(0x10 + usize::from(n), (fnum & 0xFF) as u8);
        let key = if on { 0x10 } else { 0 };
        self.opll_write(0x20 + usize::from(n), key | ((block as u8) << 1) | ((fnum >> 8) & 1) as u8);
    }

    fn opll_write(&mut self, r: usize, v: u8) {
        self.opll_regs[r & 0x3F] = v;
        if let Some(o) = self.opll.as_mut() {
            o.write(r, v);
        }
    }

    fn output_5b(&mut self, c: usize, n: u8, period: i32, vol: i32, mode: i32) {
        let has_mode = self.ch[c].seq[4].active || self.ch[c].seq[4].pos > 0;
        let r = &mut self.ay_regs;
        let k = usize::from(n);
        r[k * 2] = (period & 0xFF) as u8;
        r[k * 2 + 1] = ((period >> 8) & 15) as u8;
        // The instrument's mode: tone (0x40), noise (0x80, its period below), envelope (0x20).
        let (tone, noise, env) = if has_mode { (mode & 0x40 != 0, mode & 0x80 != 0, mode & 0x20 != 0) } else { (true, false, false) };
        if noise {
            r[6] = (mode & 0x1F) as u8;
        }
        let mut mixer = r[7];
        mixer = if tone { mixer & !(1 << k) } else { mixer | (1 << k) };
        mixer = if noise { mixer & !(8 << k) } else { mixer | (8 << k) };
        r[7] = mixer;
        r[8 + k] = (vol.clamp(0, 15) as u8) | if env { 0x10 } else { 0 };
        let regs = *r;
        if let Some(a) = self.ay.as_mut() {
            a.write_regs(&regs, false);
        }
    }

    fn output_dpcm(&mut self, c: usize, triggered: bool) {
        if let Some(v) = self.ch[c].dac.take() {
            self.write(0x11, v);
        }
        if !triggered {
            if !self.ch[c].on && self.apu.dmc_playing() {
                self.write(0x15, 0x0F);
                self.write(0x15, 0x1F);
            }
            return;
        }
        let note = self.ch[c].note.clamp(0, 95) as usize;
        let key = self.instrument(c).and_then(|i| i.dpcm.get(note)).copied().filter(|k| k.sample > 0);
        let Some(key) = key else { return };
        let Some(&(addr, len)) = self.sample_at.get(&(key.sample - 1)) else { return };
        let pitch = self.ch[c].dpcm_pitch.take().unwrap_or(key.pitch & 15);
        let looping = key.pitch & 0x80 != 0;
        self.write(0x10, (if looping { 0x40 } else { 0 }) | pitch);
        if key.delta >= 0 {
            self.write(0x11, key.delta as u8 & 0x7F);
        }
        let off = std::mem::take(&mut self.ch[c].dpcm_offset);
        self.write(0x12, addr.wrapping_add(off));
        self.write(0x13, len.saturating_sub(off.saturating_mul(4)));
        self.write(0x15, 0x0F);
        self.write(0x15, 0x1F);
    }

    /// The registers last written, $4000 to $4017.
    #[cfg(test)]
    fn registers(&self) -> [u8; 0x18] {
        self.regs
    }
}

impl Ch {
    fn note_slide_reset(&mut self) {}
}

/// The vibrato's first quarter wave for each depth 1-15 (a phase 1-16):
/// FamiTracker modules' vibrato as measured on their output.
const VIBRATO: [[u8; 16]; 15] = [
    [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4],
    [0, 1, 1, 2, 2, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6],
    [0, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 7, 8, 8, 8, 8],
    [1, 2, 3, 4, 4, 5, 6, 7, 8, 8, 9, 9, 10, 10, 10, 10],
    [1, 2, 3, 4, 5, 7, 8, 8, 9, 10, 11, 11, 12, 12, 12, 12],
    [1, 2, 4, 5, 6, 8, 9, 10, 11, 12, 13, 13, 14, 14, 14, 14],
    [1, 3, 4, 6, 7, 9, 10, 11, 13, 14, 14, 15, 16, 16, 16, 16],
    [1, 3, 5, 7, 8, 10, 12, 13, 14, 15, 16, 17, 18, 18, 18, 19],
    [2, 4, 6, 8, 9, 11, 13, 14, 16, 17, 18, 19, 20, 20, 21, 21],
    [2, 4, 6, 8, 10, 12, 14, 16, 17, 19, 20, 21, 22, 22, 23, 23],
    [2, 4, 7, 9, 11, 14, 16, 17, 19, 21, 22, 23, 24, 24, 25, 25],
    [2, 5, 7, 10, 12, 15, 17, 19, 21, 22, 24, 25, 26, 26, 27, 27],
    [2, 5, 8, 11, 13, 16, 18, 20, 22, 24, 26, 27, 28, 28, 29, 29],
    [3, 6, 9, 12, 14, 17, 20, 22, 24, 26, 28, 29, 30, 31, 31, 31],
];

/// The vibrato's offset at a phase (0-63) and depth (1-15).
fn vibrato(phase: i32, depth: i32) -> i32 {
    let t = &VIBRATO[(depth.clamp(1, 15) - 1) as usize];
    let p = phase & 63;
    let half = |p: i32| -> i32 {
        match p {
            0 => 0,
            1..=16 => i32::from(t[(p - 1) as usize]),
            _ => i32::from(t[(31 - p).clamp(0, 15) as usize]),
        }
    };
    if p < 32 { half(p) } else { -half(p - 32) }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.ch.len()
    }

    fn tick(&mut self) {
        self.tick_once();
    }

    fn tick_seconds(&self) -> f64 {
        1.0 / self.engine_hz()
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for b in self.bufs.iter_mut() {
            b.clear();
            b.resize(frames, 0.0);
        }
        {
            let [a, b, c, d, e] = &mut self.bufs;
            self.apu.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..], &mut e[..]]);
        }
        self.ext.render(rate, &mut self.ext_bufs, frames);
        for c in 0..self.ch.len() {
            out.left[c][..frames].fill(0.0);
            out.right[c][..frames].fill(0.0);
        }
        if self.opll.is_some() && self.opll_rate != rate {
            // The VRC7 made again at the new rate, its registers as they were.
            let mut o = Opll::new(3_579_545, rate);
            o.set_vrc7();
            for (r, &v) in self.opll_regs.iter().enumerate() {
                o.write(r, v);
            }
            self.opll = Some(o);
            self.opll_rate = rate;
        }
        if let Some(a) = self.ay.as_mut() {
            a.set_rate(rate);
        }
        // Each channel's source, its level, and whether its DC is taken off.
        let mut src: Vec<Option<(usize, f32)>> = vec![None; self.ch.len()];
        for (c, ch) in self.ch.iter().enumerate() {
            src[c] = match ch.chan {
                Chan::Pulse(n) => Some((usize::from(n), 1.7)),
                Chan::Triangle => Some((2, 1.7)),
                Chan::Noise => Some((3, 1.7)),
                Chan::Dpcm => Some((4, 1.7)),
                Chan::Vrc6Pulse(n) => Some((5 + usize::from(n), 0.19)),
                Chan::Vrc6Saw => Some((7, 0.19)),
                Chan::Mmc5Pulse(n) => Some((8 + usize::from(n), 0.19)),
                Chan::N163(n) => Some((10 + usize::from(n), 0.19)),
                Chan::Fds => Some((18, 0.19)),
                _ => None,
            };
        }
        let r = 1.0 - (2.0 * std::f64::consts::PI * 10.0 / f64::from(rate.max(1))) as f32;
        for (c, sv) in src.iter().enumerate() {
            let Some((k, level)) = *sv else { continue };
            let input: &[f32] = if k < 5 { &self.bufs[k] } else { &self.ext_bufs[k - 5] };
            let (mut x1, mut y1) = self.dc[k];
            for i in 0..frames {
                let x = input[i];
                let y = x - x1 + r * y1;
                x1 = x;
                y1 = y;
                let s = y * level;
                out.left[c][i] = s;
                out.right[c][i] = s;
            }
            self.dc[k] = (x1, y1);
        }
        let vrc7: Vec<(usize, usize)> = self.ch.iter().enumerate().filter_map(|(c, ch)| if let Chan::Vrc7(n) = ch.chan { Some((c, usize::from(n))) } else { None }).collect();
        let s5b: Vec<(usize, usize)> = self.ch.iter().enumerate().filter_map(|(c, ch)| if let Chan::S5b(n) = ch.chan { Some((c, usize::from(n))) } else { None }).collect();
        for i in 0..frames {
            if let Some(o) = self.opll.as_mut() {
                let v = o.calc_channels();
                for &(c, n) in &vrc7 {
                    let x = f32::from(v[n]) * (0.15 / 2048.0);
                    out.left[c][i] = x;
                    out.right[c][i] = x;
                }
            }
            if let Some(a) = self.ay.as_mut() {
                a.process(true);
                for &(c, n) in &s5b {
                    let x = a.out[n] as f32 * 0.28;
                    out.left[c][i] = x;
                    out.right[c][i] = x;
                }
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let bpm = 6 * self.tempo / self.speed.max(1);
        (self.speed.max(1) as u32, bpm.max(1) as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.halted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_vibrato_tempo() {
        let ntsc = crate::nes::NTSC_HZ;
        // A-3 is 440 Hz, A-0 55 Hz, C-3 the period 427.
        assert_eq!(pulse_period(ntsc, 45), 253);
        assert_eq!(pulse_period(ntsc, 9), 2033);
        assert_eq!(pulse_period(ntsc, 36), 427);
        for d in 1..16 {
            for p in 0..32 {
                assert_eq!(vibrato(p + 32, d), -vibrato(p, d));
            }
        }
        // Tempo 151, speed 5 at 60 Hz: five ticks a row, four on every thirtieth (Dn-Wiki's table).
        let (div, dec, rem) = (3600, 151 * 24 / 5, 151 * 24 % 5);
        let mut accum = 0;
        let mut ticks = Vec::new();
        let mut n = 0;
        for _ in 0..200 {
            if accum <= 0 {
                if n > 0 {
                    ticks.push(n);
                }
                n = 0;
                accum += div - rem;
            }
            n += 1;
            accum -= dec;
        }
        assert_eq!(&ticks[..31], &[[5; 30].as_slice(), &[4]].concat()[..]);
    }

    /// Prints the APU's registers each frame for FT_PROBE (a module), as the probe script does.
    #[test]
    #[ignore]
    fn dump() {
        let Ok(f) = std::env::var("FT_PROBE") else { return };
        let frames: usize = std::env::var("FT_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(40);
        let song = Song::parse(&std::fs::read(f).unwrap()).unwrap();
        let mut p = Player::new(song.module.clone(), 0);
        println!("  0 {}", "00000000 ".repeat(6).trim_end());
        for t in 1..frames {
            p.tick_once();
            let r = p.registers();
            let hex: String = r.iter().map(|b| format!("{b:02x}")).collect();
            let groups: Vec<&str> = (0..6).map(|k| &hex[k * 8..k * 8 + 8]).collect();
            let fds: String = p.fds_regs.iter().map(|b| format!("{b:02x}")).collect();
            let opll: String = [0x10usize, 0x20, 0x30].iter().map(|&b| p.opll_regs[b..b + 6].iter().map(|v| format!("{v:02x}")).collect::<String>()).collect::<Vec<_>>().join(" ");
            println!("{t:3} {} {fds} {opll}", groups.join(" "));
        }
    }
}

#[cfg(test)]
mod against {
    use super::*;

    /// Compares each module in FT_LIST (a file of paths) with Furnace's
    /// register log beside it in FT_REF (`<name>.ref`, a line a frame).
    #[test]
    #[ignore]
    fn against_furnace() {
        let (Ok(list), Ok(rf)) = (std::env::var("FT_LIST"), std::env::var("FT_REF")) else { return };
        let show = std::env::var("FT_SHOW").is_ok();
        let mut total = (0, 0);
        for path in std::fs::read_to_string(list).unwrap().lines() {
            let name = std::path::Path::new(path).file_stem().unwrap().to_string_lossy().to_string();
            let Ok(reference) = std::fs::read_to_string(format!("{rf}/{name}.ref")) else { continue };
            let Some(song) = Song::parse(&std::fs::read(path).unwrap()) else { continue };
            let mut p = Player::new(song.module.clone(), 0);
            let (mut bad, mut n, mut first) = (0, 0, None);
            for (t, line) in reference.lines().enumerate().skip(1) {
                p.tick_once();
                let want: Vec<u8> = line.split_whitespace().skip(1).flat_map(|g| (0..g.len() / 2).map(move |k| u8::from_str_radix(&g[k * 2..k * 2 + 2], 16).unwrap_or(0))).collect();
                if want.len() < 0x18 {
                    break;
                }
                let r = p.registers();
                n += 1;
                let per = |x: &[u8], b: usize| i32::from(x[b]) | i32::from(x[b + 1] & 7) << 8;
                // A silent channel's other registers don't matter.
                let quiet = |x: &[u8], k: usize| match k {
                    0..=3 => x[0] & 15 == 0,
                    4..=7 => x[4] & 15 == 0,
                    8..=0x0B => x[8] == 0 || x[8] == 0x80,
                    0x0C..=0x0F => x[0x0C] & 15 == 0,
                    _ => false,
                };
                let mut ok = (0..0x18).all(|k| matches!(k, 2 | 3 | 6 | 7 | 0x0A | 0x0B | 0x15 | 0x10..=0x13) || (quiet(&r, k) && quiet(&want, k)) || r[k] == want[k]);
                for b in [2, 6, 0x0A] {
                    ok &= (quiet(&r, b) && quiet(&want, b)) || (per(&r, b) - per(&want, b)).abs() <= 1;
                }
                if !ok {
                    bad += 1;
                    if first.is_none() {
                        first = Some((t, r, want));
                    }
                }
            }
            total.0 += n - bad;
            total.1 += n;
            match first {
                None => println!("{name}: all {n} frames"),
                Some((t, r, w)) => {
                    println!("{name}: {bad} of {n} differ, first at {t}");
                    if show {
                        let h = |x: &[u8]| x.iter().map(|b| format!("{b:02x}")).collect::<String>();
                        println!("   ours {}\n   ref  {}", h(&r), h(&w[..0x18]));
                    }
                }
            }
        }
        println!("{} of {} frames the same", total.0, total.1);
    }
}
