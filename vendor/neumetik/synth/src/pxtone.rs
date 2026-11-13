// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! pxtone (Piston Collage, Studio Pixel, 2005-): projects (.ptcop,
//! "PTCOLLAGE-") and tunes (.pttune, "PTTUNE--"), every version from the
//! first. A song is units (up to 50, each a voice playing one note at a
//! time) driven by one list of events on a clock (notes on for so long,
//! keys, portamento, volume, velocity, pans by volume and by time,
//! tunings, a voice and a group each), mixed into seven groups that pass
//! through overdrives and delays. A voice ("woice") is PCM, a waveform
//! drawn by points or by overtones (400 points), Ogg Vorbis, or a noise
//! built from up to four oscillator units (16 wave types, frequency and
//! volume modulated, under an envelope); each under its own envelope.
//!
//! A port of pxtone's own playback source (pxtone-source-code 220910a, the
//! `pxtone` folder, from pxtone.org), which Studio Pixel give away in its
//! readme: "再生に必要な ソースコード(pxtoneフォルダ内)は無償で使えます。改変も
//! OKです。特に許可をとる必要はありません。" (the source needed for playback,
//! in the pxtone folder, is free to use; changing it is fine too; no
//! permission is needed). Its quirks are kept so it plays the same; its
//! Ogg Vorbis voices are decoded by ours (`vorbis.rs`) where pxtone uses
//! libvorbis. It renders at 44.1 kHz, as pxtone's own player does, each
//! unit to its own channel (a group's overdrives' and delays' part shared
//! among its units by how much each has played lately).

use std::sync::Arc;

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

const RATE: i32 = 44_100;
const MAX_UNITS: usize = 50;
const MAX_WOICES: usize = 100;
const GROUPS: usize = 7;
const MAX_DELAYS: usize = 4;
const MAX_OVERDRIVES: usize = 2;
const UNIT_VOICES: usize = 2;
const TIMEPAN: usize = 0x40;

const KIND_NULL: u8 = 0;
const KIND_ON: u8 = 1;
const KIND_KEY: u8 = 2;
const KIND_PAN_VOLUME: u8 = 3;
const KIND_VELOCITY: u8 = 4;
const KIND_VOLUME: u8 = 5;
const KIND_PORTAMENT: u8 = 6;
const KIND_BEATCLOCK: u8 = 7;
const KIND_BEATTEMPO: u8 = 8;
const KIND_BEATNUM: u8 = 9;
const KIND_REPEAT: u8 = 10;
const KIND_LAST: u8 = 11;
const KIND_VOICENO: u8 = 12;
const KIND_GROUPNO: u8 = 13;
const KIND_TUNING: u8 = 14;
const KIND_PAN_TIME: u8 = 15;
const KIND_NUM: u8 = 16;

const DEFAULT_VOLUME: i32 = 104;
const DEFAULT_VELOCITY: i32 = 104;
const DEFAULT_KEY: i32 = 0x6000;
const DEFAULT_BASICKEY: i32 = 0x4500;
const DEFAULT_BEATNUM: i32 = 4;
const DEFAULT_BEATTEMPO: f32 = 120.0;
const DEFAULT_BEATCLOCK: i32 = 480;

const VOICEFLAG_WAVELOOP: u32 = 1;
const VOICEFLAG_SMOOTH: u32 = 2;
const VOICEFLAG_BEATFIT: u32 = 4;
const VOICEFLAG_UNCOVERED: u32 = 0xffff_fff8;
const DATAFLAG_WAVE: u32 = 1;
const DATAFLAG_ENVELOPE: u32 = 2;
const DATAFLAG_UNCOVERED: u32 = 0xffff_fffc;

fn is_tail(kind: u8) -> bool {
    kind == KIND_ON || kind == KIND_PORTAMENT
}

// ---- Reading ----

struct Cur<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Cur<'_> {
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let b = self.d.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(b)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }
    /// pxtone's variable-length number: 7 bits a byte, low first, at most 5.
    fn v(&mut self) -> Option<i32> {
        let mut v = 0u64;
        for k in 0..5 {
            let b = self.u8()?;
            v |= u64::from(b & 0x7F) << (7 * k);
            if b & 0x80 == 0 {
                return Some(v as u32 as i32);
            }
        }
        None
    }
    fn seek(&mut self, to: usize) -> Option<()> {
        (to <= self.d.len()).then(|| self.pos = to)
    }
    fn skip(&mut self, n: i64) -> Option<()> {
        let to = self.pos as i64 + n;
        (to >= 0).then_some(())?;
        self.seek(to as usize)
    }
}

// ---- The event list (a linked list in a fixed arena, as pxtone keeps it) ----

#[derive(Clone, Copy, Default, Debug)]
struct Rec {
    kind: u8,
    unit: u8,
    value: i32,
    clock: i32,
    prev: Option<u32>,
    next: Option<u32>,
}

#[derive(Clone, Default, Debug)]
struct Evelist {
    recs: Vec<Rec>,
    start: Option<u32>,
    linear: usize,
    x4x: Option<u32>,
}

fn priority(kind: u8) -> i32 {
    const T: [i32; 16] = [0, 50, 40, 60, 70, 80, 30, 0, 0, 0, 0, 255, 10, 20, 90, 100];
    T.get(usize::from(kind)).copied().unwrap_or(0)
}

impl Evelist {
    fn allocate(&mut self, n: usize) {
        self.recs = vec![Rec::default(); n];
        self.start = None;
    }
    fn clear(&mut self) {
        self.recs.iter_mut().for_each(|r| *r = Rec::default());
        self.start = None;
    }
    fn r(&self, i: u32) -> &Rec {
        &self.recs[i as usize]
    }
    fn rm(&mut self, i: u32) -> &mut Rec {
        &mut self.recs[i as usize]
    }
    fn iter(&self) -> impl Iterator<Item = &Rec> + '_ {
        let mut p = self.start;
        let mut guard = self.recs.len() + 1;
        std::iter::from_fn(move || {
            let i = p?;
            guard = guard.checked_sub(1)?;
            let r = &self.recs[i as usize];
            p = r.next;
            Some(r)
        })
    }
    fn max_clock(&self) -> i32 {
        self.iter().map(|p| if is_tail(p.kind) { p.clock.wrapping_add(p.value) } else { p.clock }).fold(0, i32::max)
    }
    fn count_unit_kind(&self, unit: u8, kind: u8) -> usize {
        self.iter().filter(|p| p.unit == unit && p.kind == kind).count()
    }
    fn set(&mut self, i: u32, prev: Option<u32>, next: Option<u32>, clock: i32, unit: u8, kind: u8, value: i32) {
        match prev {
            Some(p) => self.rm(p).next = Some(i),
            None => self.start = Some(i),
        }
        if let Some(n) = next {
            self.rm(n).prev = Some(i);
        }
        *self.rm(i) = Rec { kind, unit, value, clock, prev, next };
    }
    fn cut(&mut self, i: u32) {
        let Rec { prev, next, .. } = *self.r(i);
        match prev {
            Some(p) => self.rm(p).next = next,
            None => self.start = next,
        }
        if let Some(n) = next {
            self.rm(n).prev = prev;
        }
        self.rm(i).kind = KIND_NULL;
    }
    /// Where a record at `clock` goes among the others: (prev, next), the
    /// one it replaces marked empty. From `from` on, else the start.
    fn place(&mut self, clock: i32, unit: u8, kind: u8, from: Option<u32>) -> (Option<u32>, Option<u32>) {
        let Some(s) = self.start else { return (None, None) };
        if clock < self.r(s).clock {
            return (None, Some(s));
        }
        let mut p = from.or(Some(s));
        let mut guard = self.recs.len() * 2 + 2;
        while let Some(i) = p {
            guard = guard.saturating_sub(1);
            if guard == 0 {
                break;
            }
            let r = *self.r(i);
            if r.clock == clock {
                let mut q = Some(i);
                while let Some(j) = q {
                    guard = guard.saturating_sub(1);
                    if guard == 0 {
                        break;
                    }
                    let r = *self.r(j);
                    if r.clock != clock {
                        return (r.prev, Some(j));
                    }
                    if unit == r.unit && kind == r.kind {
                        self.rm(j).kind = KIND_NULL;
                        return (r.prev, r.next);
                    }
                    if priority(kind) - priority(r.kind) < 0 {
                        return (r.prev, Some(j));
                    }
                    if r.next.is_none() {
                        return (Some(j), None);
                    }
                    q = r.next;
                }
                return (None, None);
            } else if r.clock > clock {
                return (r.prev, Some(i));
            } else if r.next.is_none() {
                return (Some(i), None);
            }
            p = r.next;
        }
        (None, None)
    }
    fn add_i(&mut self, clock: i32, unit: u8, kind: u8, value: i32) -> bool {
        let Some(new) = self.recs.iter().position(|r| r.kind == KIND_NULL) else { return false };
        let new = new as u32;
        let (prev, next) = self.place(clock, unit, kind, None);
        self.set(new, prev, next, clock, unit, kind, value);
        if is_tail(kind) {
            let mut p = self.r(new).prev;
            while let Some(i) = p {
                let r = *self.r(i);
                if r.unit == unit && r.kind == kind {
                    if clock < r.clock.wrapping_add(r.value) {
                        self.rm(i).value = clock.wrapping_sub(r.clock);
                    }
                    break;
                }
                p = r.prev;
            }
            let mut p = self.r(new).next;
            while let Some(i) = p {
                let r = *self.r(i);
                if r.clock >= clock.wrapping_add(value) {
                    break;
                }
                if r.unit == unit && r.kind == kind {
                    self.cut(i);
                }
                p = r.next;
            }
        }
        true
    }
    fn value_change(&mut self, clock1: i32, clock2: i32, unit: u8, kind: u8, value: i32) {
        let (max, min) = match kind {
            KIND_ON => (120, 120),
            KIND_KEY => (0xbfff, 0),
            KIND_PAN_VOLUME | KIND_PAN_TIME | KIND_VELOCITY | KIND_VOLUME => (0x80, 0),
            _ => (0, 0),
        };
        let mut p = self.start;
        let mut guard = self.recs.len() + 1;
        while let Some(i) = p {
            guard = guard.saturating_sub(1);
            if guard == 0 {
                break;
            }
            let r = self.rm(i);
            if r.unit == unit && r.kind == kind && r.clock >= clock1 && (clock2 == -1 || r.clock < clock2) {
                r.value = r.value.wrapping_add(value).max(min).min(max);
            }
            p = r.next;
        }
    }
    fn linear_add(&mut self, clock: i32, unit: u8, kind: u8, value: i32) {
        if let Some(r) = self.recs.get_mut(self.linear) {
            r.clock = clock;
            r.unit = unit;
            r.kind = kind;
            r.value = value;
        }
        self.linear += 1;
    }
    fn linear_end(&mut self) {
        if self.recs.first().is_some_and(|r| r.kind != KIND_NULL) {
            self.start = Some(0);
        }
        for r in 1..self.recs.len() {
            if self.recs[r].kind == KIND_NULL {
                break;
            }
            self.recs[r].prev = Some(r as u32 - 1);
            self.recs[r - 1].next = Some(r as u32);
        }
    }
    fn x4x_add(&mut self, clock: i32, unit: u8, kind: u8, value: i32) {
        if self.linear >= self.recs.len() {
            return;
        }
        let new = self.linear as u32;
        self.linear += 1;
        let (prev, next) = self.place(clock, unit, kind, self.x4x);
        self.set(new, prev, next, clock, unit, kind, value);
        self.x4x = Some(new);
    }
    fn read_v5(&mut self, c: &mut Cur) -> Option<()> {
        c.i32()?;
        let n = c.i32()?;
        let mut absolute = 0i32;
        for _ in 0..n.max(0) {
            let clock = c.v()?;
            let unit = c.u8()?;
            let kind = c.u8()?;
            let value = c.v()?;
            absolute = absolute.wrapping_add(clock);
            self.linear_add(absolute, unit, kind, value);
        }
        Some(())
    }
    fn count_v5(c: &mut Cur) -> Option<i32> {
        c.i32()?;
        let n = c.i32()?;
        for _ in 0..n.max(0) {
            c.v()?;
            c.u8()?;
            c.u8()?;
            c.v()?;
        }
        Some(n)
    }
    fn read_x4x_unit(&mut self, c: &mut Cur, tail_absolute: bool, check_rrr: bool) -> Option<()> {
        c.i32()?;
        let unit = c.u16()?;
        let kind = c.u16()?;
        let data_num = c.u16()?;
        let rrr = c.u16()?;
        let n = c.u32()?;
        if data_num != 2 || kind >= u16::from(KIND_NUM) || (check_rrr && rrr != 0) {
            return None;
        }
        let mut absolute = 0i32;
        for _ in 0..n {
            let clock = c.v()?;
            let value = c.v()?;
            absolute = absolute.wrapping_add(clock);
            self.x4x_add(absolute, unit as u8, kind as u8, value);
            if tail_absolute && is_tail(kind as u8) {
                absolute = absolute.wrapping_add(value);
            }
        }
        self.x4x = None;
        Some(())
    }
}

// ---- The master: tempo, beats, measures ----

#[derive(Clone, Copy, Debug)]
pub struct Master {
    pub beat_num: i32,
    pub beat_tempo: f32,
    pub beat_clock: i32,
    pub meas_num: i32,
    pub repeat_meas: i32,
    pub last_meas: i32,
}

impl Default for Master {
    fn default() -> Master {
        Master {
            beat_num: DEFAULT_BEATNUM,
            beat_tempo: DEFAULT_BEATTEMPO,
            beat_clock: DEFAULT_BEATCLOCK,
            meas_num: 1,
            repeat_meas: 0,
            last_meas: 0,
        }
    }
}

impl Master {
    pub fn play_meas(&self) -> i32 {
        if self.last_meas != 0 {
            self.last_meas
        } else {
            self.meas_num
        }
    }
    fn last_clock(&self) -> i32 {
        self.last_meas.wrapping_mul(self.beat_clock).wrapping_mul(self.beat_num)
    }
    fn adjust_meas_num(&mut self, clock: i32) {
        if self.beat_clock == 0 || self.beat_num == 0 {
            return;
        }
        let b = (clock + self.beat_clock - 1) / self.beat_clock;
        let m = (b + self.beat_num - 1) / self.beat_num;
        if self.meas_num <= m {
            self.meas_num = m;
        }
        if self.repeat_meas >= self.meas_num {
            self.repeat_meas = 0;
        }
        if self.last_meas > self.meas_num {
            self.last_meas = self.meas_num;
        }
    }
    fn set_repeat_last(&mut self, repeat: i32, last: i32) {
        let d = self.beat_num.wrapping_mul(self.beat_clock);
        if d == 0 {
            return;
        }
        self.repeat_meas = (repeat / d).max(0);
        self.last_meas = (last / d).max(0);
    }
    fn read_v5(&mut self, c: &mut Cur) -> Option<()> {
        if c.u32()? != 15 {
            return None;
        }
        self.beat_clock = i32::from(c.u16()? as i16);
        self.beat_num = i32::from(c.u8()? as i8);
        self.beat_tempo = c.f32()?;
        let repeat = c.i32()?;
        let last = c.i32()?;
        self.set_repeat_last(repeat, last);
        Some(())
    }
    fn read_x4x(&mut self, c: &mut Cur) -> Option<()> {
        c.i32()?;
        let data_num = c.u16()?;
        let rrr = c.u16()?;
        let n = c.u32()?;
        if data_num != 3 || rrr != 0 {
            return None;
        }
        let (mut beat_clock, mut beat_num, mut beat_tempo, mut repeat, mut last) = (DEFAULT_BEATCLOCK, DEFAULT_BEATNUM, DEFAULT_BEATTEMPO, 0, 0);
        let mut absolute = 0i32;
        for _ in 0..n {
            let status = c.v()?;
            let clock = c.v()?;
            let volume = c.v()?;
            absolute = absolute.wrapping_add(clock);
            let clock = absolute;
            match status as u8 {
                KIND_BEATCLOCK if clock == 0 => beat_clock = volume,
                KIND_BEATTEMPO if clock == 0 => beat_tempo = f32::from_bits(volume as u32),
                KIND_BEATNUM if clock == 0 => beat_num = volume,
                KIND_REPEAT if volume == 0 => repeat = clock,
                KIND_LAST if volume == 0 => last = clock,
                _ => return None,
            }
        }
        self.beat_num = beat_num;
        self.beat_tempo = beat_tempo;
        self.beat_clock = beat_clock;
        self.set_repeat_last(repeat, last);
        Some(())
    }
}

// ---- Voices ----

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WoiceType {
    #[default]
    None,
    Pcm,
    Ptv,
    Ptn,
    Oggv,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VoiceType {
    #[default]
    Coordinate,
    Overtone,
    Noise,
    Sampling,
    OggVorbis,
}

/// PCM as pxtone keeps it: bytes, 8-bit unsigned or 16-bit signed.
#[derive(Clone, Debug, Default)]
struct Pcm {
    ch: i32,
    sps: i32,
    bps: i32,
    head: i32,
    body: i32,
    tail: i32,
    smp: Vec<u8>,
}

impl Pcm {
    fn create(ch: i32, sps: i32, bps: i32, num: i32) -> Option<Pcm> {
        if bps != 8 && bps != 16 {
            return None;
        }
        let size = (num as i64 * bps as i64 * ch as i64 / 8).clamp(0, 1 << 30) as usize;
        Some(Pcm {
            ch,
            sps,
            bps,
            head: 0,
            body: num,
            tail: 0,
            smp: vec![if bps == 8 { 128 } else { 0 }; size],
        })
    }
    fn size(&self) -> usize {
        ((self.head + self.body + self.tail) as i64 * self.ch as i64 * self.bps as i64 / 8).max(0) as usize
    }
    fn convert_ch(&mut self, new_ch: i32) -> bool {
        let size = self.size().min(self.smp.len());
        if self.ch == new_ch {
            return true;
        }
        let s = &self.smp;
        let work: Vec<u8> = if new_ch == 2 {
            match self.bps {
                8 => s[..size].iter().flat_map(|&b| [b, b]).collect(),
                _ => s[..size].chunks_exact(2).flat_map(|b| [b[0], b[1], b[0], b[1]]).collect(),
            }
        } else {
            match self.bps {
                8 => s[..size].chunks_exact(2).map(|b| ((i32::from(b[0]) + i32::from(b[1])) / 2) as u8).collect(),
                _ => s[..size]
                    .chunks_exact(4)
                    .flat_map(|b| {
                        let (t1, t2) = (i32::from(i16::from_le_bytes([b[0], b[1]])), i32::from(i16::from_le_bytes([b[2], b[3]])));
                        (((t1 + t2) / 2) as i16).to_le_bytes()
                    })
                    .collect(),
            }
        };
        self.smp = work;
        self.ch = new_ch;
        true
    }
    fn convert_bps(&mut self, new_bps: i32) -> bool {
        if self.bps == new_bps {
            return true;
        }
        let size = self.size().min(self.smp.len());
        let s = &self.smp[..size];
        self.smp = match new_bps {
            8 => s.chunks_exact(2).map(|b| ((i32::from(i16::from_le_bytes([b[0], b[1]])) / 0x100) + 128) as u8).collect(),
            16 => s.iter().flat_map(|&b| (((i32::from(b) - 128) * 0x100) as i16).to_le_bytes()).collect(),
            _ => return false,
        };
        self.bps = new_bps;
        true
    }
    fn convert_sps(&mut self, new_sps: i32) -> bool {
        if self.sps == new_sps {
            return true;
        }
        if self.sps == 0 || new_sps == 0 {
            return false;
        }
        let up = |x: i32| -> i32 {
            let b = (x as i64 * self.ch as i64 * self.bps as i64 / 8) as f64;
            ((b * f64::from(new_sps) + f64::from(self.sps) - 1.0) / f64::from(self.sps)) as i32
        };
        let (h, b, t) = (up(self.head), up(self.body), up(self.tail));
        let total = h + b + t;
        let unit: i32 = if self.ch == 2 && self.bps == 16 {
            4
        } else if self.ch == 1 && self.bps == 8 {
            1
        } else {
            2
        };
        self.head = h / unit;
        self.body = b / unit;
        self.tail = t / unit;
        let num = (total / unit).max(0) as usize;
        let u = unit as usize;
        let mut work = vec![0u8; num * u];
        for a in 0..num {
            let src = (a as f64 * f64::from(self.sps) / f64::from(new_sps)) as usize;
            if let (Some(from), Some(to)) = (self.smp.get(src * u..src * u + u), work.get_mut(a * u..a * u + u)) {
                to.copy_from_slice(from);
            }
        }
        self.smp = work;
        self.sps = new_sps;
        true
    }
    fn convert(&mut self, ch: i32, sps: i32, bps: i32) -> bool {
        self.convert_ch(ch) && self.convert_bps(bps) && self.convert_sps(sps)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct NoiseOsc {
    kind: i32,
    freq: f32,
    volume: f32,
    offset: f32,
    rev: bool,
}

#[derive(Clone, Debug, Default)]
struct NoiseUnit {
    enable: bool,
    enves: Vec<(i32, i32)>,
    pan: i32,
    main: NoiseOsc,
    freq: NoiseOsc,
    volu: NoiseOsc,
}

#[derive(Clone, Debug, Default)]
struct Noise {
    smp_num_44k: i32,
    units: Vec<NoiseUnit>,
}

const WAVETYPE_NUM: i32 = 17;

impl Noise {
    fn read(c: &mut Cur) -> Option<Noise> {
        if c.bytes(8)? != b"PTNOISE-" {
            return None;
        }
        if c.u32()? > 20120418 {
            return None;
        }
        let smp_num_44k = c.v()?;
        let unit_num = c.u8()? as i8;
        if !(0..=4).contains(&unit_num) {
            return None;
        }
        let mut units = Vec::new();
        for _ in 0..unit_num {
            let mut u = NoiseUnit { enable: true, ..Default::default() };
            let flags = c.v()? as u32;
            if flags & 0xffff_ff83 != 0 {
                return None;
            }
            if flags & 0x04 != 0 {
                let n = c.v()?;
                if n > 3 {
                    return None;
                }
                for _ in 0..n.max(0) {
                    let x = c.v()?;
                    let y = c.v()?;
                    u.enves.push((x, y));
                }
            }
            if flags & 0x08 != 0 {
                u.pan = i32::from(c.u8()? as i8);
            }
            let osc = |c: &mut Cur| -> Option<NoiseOsc> {
                let kind = c.v()?;
                if kind >= WAVETYPE_NUM {
                    return None;
                }
                let rev = c.v()? != 0;
                let freq = c.v()? as f32 / 10.0;
                let volume = c.v()? as f32 / 10.0;
                let offset = c.v()? as f32 / 10.0;
                Some(NoiseOsc { kind, freq, volume, offset, rev })
            };
            if flags & 0x10 != 0 {
                u.main = osc(c)?;
            }
            if flags & 0x20 != 0 {
                u.freq = osc(c)?;
            }
            if flags & 0x40 != 0 {
                u.volu = osc(c)?;
            }
            units.push(u);
        }
        Some(Noise { smp_num_44k, units })
    }

    /// Its limits, as pxtone sets them before building it.
    fn fix(&mut self) {
        self.smp_num_44k = self.smp_num_44k.min(48_000 * 10);
        let fix_osc = |o: &mut NoiseOsc| {
            if o.kind >= WAVETYPE_NUM {
                o.kind = 0;
            }
            if o.freq > 44100.0 {
                o.freq = 44100.0;
            }
            if o.freq <= 0.0 {
                o.freq = 0.0;
            }
            if o.volume > 200.0 {
                o.volume = 200.0;
            }
            if o.volume <= 0.0 {
                o.volume = 0.0;
            }
            if o.offset > 100.0 {
                o.offset = 100.0;
            }
            if o.offset <= 0.0 {
                o.offset = 0.0;
            }
        };
        for u in &mut self.units {
            if u.enable {
                for e in &mut u.enves {
                    e.0 = e.0.clamp(0, 10_000);
                    e.1 = e.1.clamp(0, 100);
                }
                u.pan = u.pan.clamp(-100, 100);
                fix_osc(&mut u.main);
                fix_osc(&mut u.freq);
                fix_osc(&mut u.volu);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct VoiceUnit {
    basic_key: i32,
    volume: i32,
    pan: i32,
    tuning: f32,
    voice_flags: u32,
    data_flags: u32,
    kind: VoiceType,
    pcm: Option<Pcm>,
    ptn: Option<Noise>,
    /// An Ogg Vorbis voice: its channels, rate, length and stream.
    oggv: Option<(i32, i32, i32, Vec<u8>)>,
    wave_reso: i32,
    wave: Vec<(i32, i32)>,
    env_fps: i32,
    env_head: i32,
    env_body: i32,
    env_tail: i32,
    env: Vec<(i32, i32)>,
}

impl VoiceUnit {
    fn new() -> VoiceUnit {
        VoiceUnit {
            basic_key: DEFAULT_BASICKEY,
            volume: 128,
            pan: 64,
            tuning: 1.0,
            voice_flags: VOICEFLAG_SMOOTH,
            data_flags: DATAFLAG_WAVE,
            ..Default::default()
        }
    }
}

/// A voice made ready to play: its points (16-bit stereo at 44.1 kHz) and
/// its envelope (one step a sample).
#[derive(Clone, Debug, Default)]
struct Instance {
    smp_body_w: i32,
    smp: Vec<i16>,
    env: Vec<u8>,
    env_size: i32,
    env_release: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Woice {
    pub kind: WoiceType,
    pub name: String,
    voices: Vec<VoiceUnit>,
    insts: Vec<Instance>,
    x3x_tuning: f32,
    x3x_basic_key: i32,
    /// Its chunk as the file has it (its code first), for its patch.
    raw: Vec<u8>,
}

impl Woice {
    fn read_pcm(c: &mut Cur) -> Option<Woice> {
        c.i32()?;
        let _unit_no = c.u16()?;
        let basic_key = c.u16()?;
        let voice_flags = c.u32()?;
        let ch = c.u16()?;
        let bps = c.u16()?;
        let sps = c.u32()?;
        let tuning = c.f32()?;
        let data_size = c.u32()?;
        if voice_flags & VOICEFLAG_UNCOVERED != 0 {
            return None;
        }
        let per = (i32::from(bps) / 8) * i32::from(ch);
        if per == 0 {
            return None;
        }
        let mut pcm = Pcm::create(i32::from(ch), sps as i32, i32::from(bps), (data_size / per as u32) as i32)?;
        let data = c.bytes(data_size as usize)?;
        let n = pcm.smp.len().min(data.len());
        pcm.smp[..n].copy_from_slice(&data[..n]);
        let mut v = VoiceUnit::new();
        v.kind = VoiceType::Sampling;
        v.pcm = Some(pcm);
        v.voice_flags = voice_flags;
        v.basic_key = i32::from(basic_key);
        v.tuning = tuning;
        Some(Woice {
            kind: WoiceType::Pcm,
            voices: vec![v],
            insts: vec![Instance::default()],
            x3x_basic_key: i32::from(basic_key),
            x3x_tuning: 0.0,
            ..Default::default()
        })
    }

    fn read_ptn(c: &mut Cur) -> Option<Woice> {
        c.i32()?;
        let _unit_no = c.u16()?;
        let basic_key = c.u16()?;
        let voice_flags = c.u32()?;
        let tuning = c.f32()?;
        let rrr = c.i32()?;
        if !(0..=1).contains(&rrr) {
            return None;
        }
        let mut v = VoiceUnit::new();
        v.kind = VoiceType::Noise;
        v.ptn = Some(Noise::read(c)?);
        v.voice_flags = voice_flags;
        v.basic_key = i32::from(basic_key);
        v.tuning = tuning;
        Some(Woice {
            kind: WoiceType::Ptn,
            voices: vec![v],
            insts: vec![Instance::default()],
            x3x_basic_key: i32::from(basic_key),
            x3x_tuning: 0.0,
            ..Default::default()
        })
    }

    fn read_ptv_body(c: &mut Cur) -> Option<Woice> {
        if c.bytes(8)? != b"PTVOICE-" {
            return None;
        }
        let version = c.i32()?;
        c.i32()?;
        if version > 20060111 {
            return None;
        }
        let x3x_basic_key = c.v()?;
        let (w1, w2) = (c.v()?, c.v()?);
        if w1 != 0 || w2 != 0 {
            return None;
        }
        let num = c.v()?;
        if !(0..=1024).contains(&num) {
            return None;
        }
        let mut voices = Vec::new();
        for _ in 0..num {
            let mut v = VoiceUnit::new();
            v.basic_key = c.v()?;
            v.volume = c.v()?;
            v.pan = c.v()?;
            v.tuning = f32::from_bits(c.v()? as u32);
            v.voice_flags = c.v()? as u32;
            v.data_flags = c.v()? as u32;
            if v.voice_flags & VOICEFLAG_UNCOVERED != 0 || v.data_flags & DATAFLAG_UNCOVERED != 0 {
                return None;
            }
            if v.data_flags & DATAFLAG_WAVE != 0 {
                let t = c.v()?;
                match t {
                    0 => {
                        v.kind = VoiceType::Coordinate;
                        let n = c.v()?;
                        v.wave_reso = c.v()?;
                        if !(0..=1 << 20).contains(&n) {
                            return None;
                        }
                        for _ in 0..n {
                            let x = i32::from(c.u8()?);
                            let y = i32::from(c.u8()? as i8);
                            v.wave.push((x, y));
                        }
                    }
                    1 => {
                        v.kind = VoiceType::Overtone;
                        let n = c.v()?;
                        if !(0..=1 << 20).contains(&n) {
                            return None;
                        }
                        for _ in 0..n {
                            let x = c.v()?;
                            let y = c.v()?;
                            v.wave.push((x, y));
                        }
                    }
                    _ => return None,
                }
            }
            if v.data_flags & DATAFLAG_ENVELOPE != 0 {
                v.env_fps = c.v()?;
                v.env_head = c.v()?;
                v.env_body = c.v()?;
                v.env_tail = c.v()?;
                if v.env_body != 0 || v.env_tail != 1 || !(0..=1 << 20).contains(&v.env_head) {
                    return None;
                }
                for _ in 0..v.env_head + v.env_body + v.env_tail {
                    let x = c.v()?;
                    let y = c.v()?;
                    v.env.push((x, y));
                }
            }
            voices.push(v);
        }
        let insts = vec![Instance::default(); voices.len()];
        Some(Woice {
            kind: WoiceType::Ptv,
            voices,
            insts,
            x3x_basic_key,
            ..Default::default()
        })
    }

    fn read_ptv(c: &mut Cur) -> Option<Woice> {
        c.i32()?;
        let _unit_no = c.u16()?;
        let rrr = c.u16()?;
        let x3x_tuning = c.f32()?;
        c.i32()?;
        if rrr != 0 {
            return None;
        }
        let mut w = Woice::read_ptv_body(c)?;
        w.x3x_tuning = if x3x_tuning != 1.0 { x3x_tuning } else { 0.0 };
        Some(w)
    }

    fn read_oggv(c: &mut Cur) -> Option<Woice> {
        c.i32()?;
        let _xxx = c.u16()?;
        let basic_key = c.u16()?;
        let voice_flags = c.u32()?;
        let tuning = c.f32()?;
        if voice_flags & VOICEFLAG_UNCOVERED != 0 {
            return None;
        }
        let ch = c.i32()?;
        let sps = c.i32()?;
        let num = c.i32()?;
        let size = c.i32()?;
        if size <= 0 {
            return None;
        }
        let data = c.bytes(size as usize)?.to_vec();
        let mut v = VoiceUnit::new();
        v.kind = VoiceType::OggVorbis;
        v.oggv = Some((ch, sps, num, data));
        v.voice_flags = voice_flags;
        v.basic_key = i32::from(basic_key);
        v.tuning = tuning;
        Some(Woice {
            kind: WoiceType::Oggv,
            voices: vec![v],
            insts: vec![Instance::default()],
            x3x_basic_key: i32::from(basic_key),
            x3x_tuning: 0.0,
            ..Default::default()
        })
    }

    /// Each voice's points and envelope (Tone_Ready).
    fn ready(&mut self, bldr: &NoiseBuilder) -> Option<()> {
        for (vc, vi) in self.voices.iter_mut().zip(self.insts.iter_mut()) {
            *vi = Instance::default();
            match vc.kind {
                VoiceType::OggVorbis => {
                    let (_, _, _, data) = vc.oggv.as_ref()?;
                    let d = crate::vorbis::decode_ogg(data)?;
                    let frames = d.pcm.len() / usize::from(d.channels.max(1));
                    let mut pcm = Pcm::create(i32::from(d.channels), d.rate as i32, 16, frames as i32)?;
                    for (k, s) in d.pcm.iter().enumerate() {
                        if let Some(b) = pcm.smp.get_mut(k * 2..k * 2 + 2) {
                            b.copy_from_slice(&s.to_le_bytes());
                        }
                    }
                    if !pcm.convert(2, RATE, 16) {
                        return None;
                    }
                    vi.smp_body_w = pcm.body;
                    vi.smp = pcm.smp.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
                }
                VoiceType::Sampling => {
                    let mut pcm = vc.pcm.clone()?;
                    if !pcm.convert(2, RATE, 16) {
                        return None;
                    }
                    vi.smp_body_w = pcm.body;
                    vi.smp = pcm.smp.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
                }
                VoiceType::Overtone | VoiceType::Coordinate => {
                    vi.smp_body_w = 400;
                    vi.smp = vec![0; 400 * 2];
                    let mut pan = [64.0f64, 64.0];
                    if vc.pan > 64 {
                        pan[0] = f64::from(128 - vc.pan);
                    }
                    if vc.pan < 64 {
                        pan[1] = f64::from(vc.pan);
                    }
                    let osc = Osc {
                        points: &vc.wave,
                        volume: vc.volume,
                        sample_num: 400,
                        reso: vc.wave_reso,
                    };
                    for s in 0..400 {
                        let o = if vc.kind == VoiceType::Overtone { osc.overtone(s) } else { osc.coordinate(s) };
                        for c in 0..2 {
                            let w = (o * pan[c] / 64.0).clamp(-1.0, 1.0);
                            vi.smp[s as usize * 2 + c] = (w * 32767.0) as i32 as i16;
                        }
                    }
                }
                VoiceType::Noise => {
                    let mut n = vc.ptn.clone()?;
                    vi.smp = bldr.build(&mut n);
                    vi.smp_body_w = n.smp_num_44k;
                }
            }
            // The envelope.
            vi.env.clear();
            vi.env_size = 0;
            if vc.env_head > 0 {
                let fps = f64::from(vc.env_fps);
                let size: i32 = vc.env.iter().take(vc.env_head as usize).fold(0i32, |s, p| s.wrapping_add(p.0));
                let mut env_size = (f64::from(size) * f64::from(RATE) / fps) as i32;
                if env_size == 0 {
                    env_size = 1;
                }
                if !(0..=1 << 26).contains(&env_size) {
                    return None;
                }
                vi.env_size = env_size;
                vi.env = vec![0u8; env_size as usize];
                // Kept points go to their own places, as pxtone puts them,
                // leaving zeros where one is passed over.
                let head = vc.env_head as usize;
                let mut pts = vec![(0i32, 0i32); head];
                let mut offset = 0i32;
                let mut num = 0usize;
                for e in 0..head {
                    let (x, y) = vc.env[e];
                    if e == 0 || x != 0 || y != 0 {
                        offset = offset.wrapping_add((f64::from(x) * f64::from(RATE) / fps) as i32);
                        pts[e] = (offset, y);
                        num += 1;
                    }
                }
                let (mut sx, mut sy) = (0i32, 0i32);
                let mut e = 0usize;
                for s in 0..env_size {
                    while e < num && s >= pts[e].0 {
                        (sx, sy) = pts[e];
                        e += 1;
                    }
                    vi.env[s as usize] = if e < num {
                        let d = pts[e].0 - sx;
                        if d == 0 {
                            sy as u8
                        } else {
                            (sy + (pts[e].1 - sy).wrapping_mul(s - sx) / d) as u8
                        }
                    } else {
                        sy as u8
                    };
                }
            }
            vi.env_release = if vc.env_tail != 0 {
                let x = vc.env.get(vc.env_head as usize).map_or(0, |p| p.0);
                (f64::from(x) * f64::from(RATE) / f64::from(vc.env_fps)) as i32
            } else {
                0
            };
        }
        Some(())
    }
}

/// pxtone's oscillator: a wave by overtones, or by points joined by lines.
struct Osc<'a> {
    points: &'a [(i32, i32)],
    volume: i32,
    sample_num: i32,
    reso: i32,
}

impl Osc<'_> {
    fn overtone(&self, index: i32) -> f64 {
        let pi = 3.1415926535897932f64;
        let mut w = 0.0f64;
        for &(x, y) in self.points {
            let sss = 2.0 * pi * f64::from(x) * f64::from(index) / f64::from(self.sample_num);
            w += sss.sin() * f64::from(y) / f64::from(x) / 128.0;
        }
        w * f64::from(self.volume) / 128.0
    }
    fn coordinate(&self, index: i32) -> f64 {
        let p = self.points;
        let n = p.len();
        if n == 0 || self.sample_num == 0 {
            return 0.0;
        }
        let mut i = (i64::from(self.reso) * i64::from(index) / i64::from(self.sample_num)) as i32;
        let mut c = 0;
        while c < n && p[c].0 <= i {
            c += 1;
        }
        let (x1, y1, x2, y2) = if c == n {
            (p[c - 1].0, p[c - 1].1, self.reso, p[0].1)
        } else if c > 0 {
            (p[c - 1].0, p[c - 1].1, p[c].0, p[c].1)
        } else {
            (p[0].0, p[0].1, p[0].0, p[0].1)
        };
        let w = x2 - x1;
        i -= x1;
        let h = y2 - y1;
        let work = if i != 0 { f64::from(y1) + f64::from(h) * f64::from(i) / f64::from(w) } else { f64::from(y1) };
        work * f64::from(self.volume) / 128.0 / 128.0
    }
}

// ---- The frequency table ----

#[derive(Clone)]
struct Freq {
    table: Vec<f32>,
}

impl Freq {
    fn new() -> Freq {
        fn divide_octave_rate(divi: i32) -> f64 {
            let mut parameter = 1.0f64;
            for i in 0..17 {
                let mut add = 1.0f64;
                for _ in 0..i {
                    add *= 0.1;
                }
                let mut j = 0;
                while j < 10 {
                    let work = parameter + add * f64::from(j);
                    let mut result = 1.0f64;
                    let mut k = 0;
                    while k < divi {
                        result *= work;
                        if result >= 2.0 {
                            break;
                        }
                        k += 1;
                    }
                    if k != divi {
                        break;
                    }
                    j += 1;
                }
                parameter += add * f64::from(j - 1);
            }
            parameter
        }
        let oct_x24 = divide_octave_rate(12 * 16);
        let table = (0..16 * 192)
            .map(|f| {
                let mut work = 2f64.powi(f / 192 - 8);
                for _ in 0..f % 192 {
                    work *= oct_x24;
                }
                work as f32
            })
            .collect();
        Freq { table }
    }
    fn get(&self, key: i32) -> f32 {
        let i = key.wrapping_add(0x6000).wrapping_mul(16) / 0x100;
        self.table[i.clamp(0, 3071) as usize]
    }
    fn get2(&self, key: i32) -> f32 {
        self.table[(key >> 4).clamp(0, 3071) as usize]
    }
}

// ---- The noise builder ----

const SMP_NUM: i32 = 441;
const SMP_NUM_RAND: i32 = 44100;
const SAMPLING_TOP: i32 = 32767;
const KEY_TOP: i32 = 0x3200;

#[derive(Clone)]
struct NoiseBuilder {
    tables: Vec<Vec<i16>>,
    freq: Freq,
}

#[derive(Clone, Copy, Default)]
struct NOsc {
    incr: f64,
    offset: f64,
    volume: f64,
    table: usize,
    rev: bool,
    ran: u8,
    rdm_start: i32,
    rdm_margin: i32,
    rdm_index: i32,
}

impl NoiseBuilder {
    fn new() -> NoiseBuilder {
        let n = SMP_NUM as usize;
        let mut t: Vec<Vec<i16>> = vec![vec![0; n]; WAVETYPE_NUM as usize];
        let clip = |w: f64| (w.clamp(-1.0, 1.0) * f64::from(SAMPLING_TOP)) as i32 as i16;
        let sine = [(1, 128)];
        let o = Osc {
            points: &sine,
            volume: 128,
            sample_num: SMP_NUM,
            reso: 0,
        };
        t[1] = (0..SMP_NUM).map(|s| clip(o.overtone(s))).collect();
        let work = f64::from(SAMPLING_TOP + SAMPLING_TOP);
        t[2] = (0..SMP_NUM).map(|s| (f64::from(SAMPLING_TOP) - work * f64::from(s) / f64::from(SMP_NUM)) as i32 as i16).collect();
        let steps = |cuts: &[(i32, i32)]| -> Vec<i16> {
            let mut v = Vec::with_capacity(n);
            let mut s = 0;
            for &(end, val) in cuts {
                while s < end {
                    v.push(val as i16);
                    s += 1;
                }
            }
            v
        };
        let top = SAMPLING_TOP;
        t[3] = steps(&[(SMP_NUM / 2, top), (SMP_NUM, -top)]);
        let mut buf = [0x4444i32, 0x8888i32];
        t[4] = (0..SMP_NUM_RAND)
            .map(|_| {
                let w1 = i32::from(buf[0] as i16) + buf[1];
                let lo = (w1 as u32) & 0xFFFF;
                let w2 = (((lo & 0xFF) << 8) | (lo >> 8)) as u16 as i16;
                buf[1] = i32::from(buf[0] as i16);
                buf[0] = i32::from(w2);
                w2
            })
            .collect();
        let saw2: Vec<(i32, i32)> = (1..=16).map(|x| (x, 128)).collect();
        let o = Osc {
            points: &saw2,
            volume: 128,
            sample_num: SMP_NUM,
            reso: 0,
        };
        t[5] = (0..SMP_NUM).map(|s| clip(o.overtone(s))).collect();
        let rect2: Vec<(i32, i32)> = (0..8).map(|k| (2 * k + 1, 128)).collect();
        let o = Osc {
            points: &rect2,
            volume: 128,
            sample_num: SMP_NUM,
            reso: 0,
        };
        t[6] = (0..SMP_NUM).map(|s| clip(o.overtone(s))).collect();
        let tri = [(0, 0), (SMP_NUM / 4, 128), (SMP_NUM * 3 / 4, -128), (SMP_NUM, 0)];
        let o = Osc {
            points: &tri,
            volume: 128,
            sample_num: SMP_NUM,
            reso: SMP_NUM,
        };
        t[7] = (0..SMP_NUM).map(|s| clip(o.coordinate(s))).collect();
        // Random2 has no table: its oscillators read the random table.
        t[8] = Vec::new();
        t[9] = steps(&[(SMP_NUM / 3, top), (SMP_NUM, -top)]);
        t[10] = steps(&[(SMP_NUM / 4, top), (SMP_NUM, -top)]);
        t[11] = steps(&[(SMP_NUM / 8, top), (SMP_NUM, -top)]);
        t[12] = steps(&[(SMP_NUM / 16, top), (SMP_NUM, -top)]);
        t[13] = steps(&[(SMP_NUM / 3, top), (SMP_NUM * 2 / 3, 0), (SMP_NUM, -top)]);
        t[14] = steps(&[(SMP_NUM / 4, top), (SMP_NUM * 2 / 4, top / 3), (SMP_NUM * 3 / 4, -top / 3), (SMP_NUM, -top)]);
        t[15] = steps(&[
            (SMP_NUM / 6, top),
            (SMP_NUM * 2 / 6, top - top * 2 / 5),
            (SMP_NUM * 3 / 6, top / 5),
            (SMP_NUM * 4 / 6, -top / 5),
            (SMP_NUM * 5 / 6, -top + top * 2 / 5),
            (SMP_NUM, -top),
        ]);
        t[16] = steps(&[
            (SMP_NUM / 8, top),
            (SMP_NUM * 2 / 8, top - top * 2 / 7),
            (SMP_NUM * 3 / 8, top - top * 4 / 7),
            (SMP_NUM * 4 / 8, top / 7),
            (SMP_NUM * 5 / 8, -top / 7),
            (SMP_NUM * 6 / 8, -top + top * 4 / 7),
            (SMP_NUM * 7 / 8, -top + top * 2 / 7),
            (SMP_NUM, -top),
        ]);
        NoiseBuilder { tables: t, freq: Freq::new() }
    }

    fn at(&self, table: usize, i: i32) -> i32 {
        self.tables.get(table).and_then(|t| t.get(usize::try_from(i).ok()?)).copied().map_or(0, i32::from)
    }

    fn set_osc(&self, o: &NoiseOsc, sps: i32) -> NOsc {
        let ran = match o.kind {
            4 => 1,
            8 => 2,
            _ => 0,
        };
        let rdm_index = (f64::from(SMP_NUM_RAND) * f64::from(o.offset / 100.0)) as i32;
        NOsc {
            incr: (44100.0 / f64::from(sps)) * (f64::from(o.freq) / 100.0),
            offset: if ran != 0 { 0.0 } else { f64::from(SMP_NUM) * f64::from(o.offset / 100.0) },
            volume: f64::from(o.volume / 100.0),
            table: o.kind.clamp(0, WAVETYPE_NUM - 1) as usize,
            rev: o.rev,
            ran,
            rdm_start: 0,
            rdm_margin: self.at(4, rdm_index),
            rdm_index,
        }
    }

    fn incr(&self, o: &mut NOsc, incr: f64) {
        o.offset += incr;
        if o.offset > f64::from(SMP_NUM) {
            o.offset -= f64::from(SMP_NUM);
            if o.offset >= f64::from(SMP_NUM) {
                o.offset = 0.0;
            }
            if o.ran != 0 {
                o.rdm_start = self.at(4, o.rdm_index);
                o.rdm_index += 1;
                if o.rdm_index >= SMP_NUM_RAND {
                    o.rdm_index = 0;
                }
                o.rdm_margin = self.at(4, o.rdm_index) - o.rdm_start;
            }
        }
    }

    /// A noise voice's 16-bit stereo points at 44.1 kHz.
    fn build(&self, noise: &mut Noise) -> Vec<i16> {
        noise.fix();
        let sps = RATE;
        struct U {
            enable: bool,
            pan: [f64; 2],
            enve_index: usize,
            enve_mag_start: f64,
            enve_mag_margin: f64,
            enve_count: i32,
            enves: Vec<(i32, f64)>,
            main: NOsc,
            freq: NOsc,
            volu: NOsc,
        }
        let mut units: Vec<U> = noise
            .units
            .iter()
            .map(|du| {
                let pan = if du.pan == 0 {
                    [1.0, 1.0]
                } else if du.pan < 0 {
                    [1.0, f64::from(100.0f32 + du.pan as f32) / 100.0]
                } else {
                    [f64::from(100.0f32 - du.pan as f32) / 100.0, 1.0]
                };
                let enves: Vec<(i32, f64)> = du.enves.iter().map(|&(x, y)| (sps.wrapping_mul(x) / 1000, f64::from(y) / 100.0)).collect();
                let mut u = U {
                    enable: du.enable,
                    pan,
                    enve_index: 0,
                    enve_mag_start: 0.0,
                    enve_mag_margin: 0.0,
                    enve_count: 0,
                    enves,
                    main: self.set_osc(&du.main, sps),
                    freq: self.set_osc(&du.freq, sps),
                    volu: self.set_osc(&du.volu, sps),
                };
                while u.enve_index < u.enves.len() {
                    u.enve_mag_margin = u.enves[u.enve_index].1 - u.enve_mag_start;
                    if u.enves[u.enve_index].0 != 0 {
                        break;
                    }
                    u.enve_mag_start = u.enves[u.enve_index].1;
                    u.enve_index += 1;
                }
                u
            })
            .collect();
        let smp_num = (f64::from(noise.smp_num_44k) / (44100.0 / f64::from(sps))) as i32;
        let mut out = Vec::with_capacity(smp_num.max(0) as usize * 2);
        let rand_val = |o: &NOsc| -> f64 {
            match o.ran {
                1 => f64::from(o.rdm_start + o.rdm_margin.wrapping_mul(o.offset as i32) / SMP_NUM),
                2 => f64::from(o.rdm_start),
                _ => 0.0,
            }
        };
        for _ in 0..smp_num.max(0) {
            for c in 0..2 {
                let mut store = 0.0f64;
                for u in units.iter().filter(|u| u.enable) {
                    let po = &u.main;
                    let mut work = match po.ran {
                        0 => {
                            let off = po.offset as i32;
                            if off >= 0 {
                                f64::from(self.at(po.table, off))
                            } else {
                                0.0
                            }
                        }
                        _ => {
                            if po.offset >= 0.0 {
                                rand_val(po)
                            } else {
                                0.0
                            }
                        }
                    };
                    if po.rev {
                        work *= -1.0;
                    }
                    work *= po.volume;
                    let po = &u.volu;
                    let mut vol = match po.ran {
                        0 => f64::from(self.at(po.table, po.offset as i32)),
                        _ => rand_val(po),
                    };
                    if po.rev {
                        vol *= -1.0;
                    }
                    vol *= po.volume;
                    work = work * (vol + f64::from(SAMPLING_TOP)) / f64::from(SAMPLING_TOP * 2);
                    work *= u.pan[c];
                    if u.enve_index < u.enves.len() {
                        work *= u.enve_mag_start + (u.enve_mag_margin * f64::from(u.enve_count) / f64::from(u.enves[u.enve_index].0));
                    } else {
                        work *= u.enve_mag_start;
                    }
                    store += work;
                }
                out.push((store as i32).clamp(-SAMPLING_TOP, SAMPLING_TOP) as i16);
            }
            for u in units.iter_mut().filter(|u| u.enable) {
                let po = &u.freq;
                let mut fre = match po.ran {
                    0 => f64::from(KEY_TOP * self.at(po.table, po.offset as i32) / SAMPLING_TOP),
                    _ => rand_val(po),
                };
                if po.rev {
                    fre *= -1.0;
                }
                fre *= po.volume;
                let mi = u.main.incr * f64::from(self.freq.get(fre as i32));
                self.incr(&mut u.main, mi);
                let fi = u.freq.incr;
                self.incr(&mut u.freq, fi);
                let vi = u.volu.incr;
                self.incr(&mut u.volu, vi);
                if u.enve_index < u.enves.len() {
                    u.enve_count += 1;
                    if u.enve_count >= u.enves[u.enve_index].0 {
                        u.enve_count = 0;
                        u.enve_mag_start = u.enves[u.enve_index].1;
                        u.enve_mag_margin = 0.0;
                        u.enve_index += 1;
                        while u.enve_index < u.enves.len() {
                            u.enve_mag_margin = u.enves[u.enve_index].1 - u.enve_mag_start;
                            if u.enves[u.enve_index].0 != 0 {
                                break;
                            }
                            u.enve_mag_start = u.enves[u.enve_index].1;
                            u.enve_index += 1;
                        }
                    }
                }
            }
        }
        out
    }
}

// ---- Effects ----

#[derive(Clone, Debug)]
struct Delay {
    unit: u16,
    group: usize,
    rate: f32,
    freq: f32,
    smp_num: usize,
    offset: usize,
    bufs: [Vec<i32>; 2],
    rate_s32: i32,
}

impl Delay {
    fn ready(&mut self, beat_num: i32, beat_tempo: f32, sps: i32) {
        self.bufs = [Vec::new(), Vec::new()];
        self.smp_num = 0;
        if self.freq != 0.0 && self.rate != 0.0 {
            self.offset = 0;
            self.rate_s32 = self.rate as i32;
            let n = match self.unit {
                0 => (sps.wrapping_mul(60) as f32 / beat_tempo / self.freq) as i32,
                1 => (sps.wrapping_mul(60).wrapping_mul(beat_num) as f32 / beat_tempo / self.freq) as i32,
                _ => (sps as f32 / self.freq) as i32,
            };
            self.smp_num = n.clamp(0, 1 << 24) as usize;
            self.bufs = [vec![0; self.smp_num], vec![0; self.smp_num]];
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OverDrive {
    group: usize,
    cut: f32,
    amp: f32,
    top: i32,
}

// ---- The song ----

#[derive(Clone, Debug, Default)]
pub struct UnitInfo {
    pub name: String,
}

#[derive(Clone)]
pub struct Song {
    pub master: Master,
    evels: Evelist,
    pub woices: Vec<Woice>,
    pub units: Vec<UnitInfo>,
    delays: Vec<Delay>,
    overdrives: Vec<OverDrive>,
    pub name: Vec<u8>,
    pub comment: Vec<u8>,
    pub project: bool,
    file: Arc<Vec<u8>>,
}

impl std::fmt::Debug for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pxtone::Song({} units, {} woices)", self.units.len(), self.woices.len())
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Fmt {
    X1x,
    X2x,
    X3x,
    X4x,
    V5,
}

fn version(c: &mut Cur) -> Option<Fmt> {
    let v = c.bytes(16)?;
    let (f, exe) = match v {
        b"PTCOLLAGE-050227" => (Fmt::X1x, false),
        b"PTCOLLAGE-050608" => (Fmt::X2x, false),
        b"PTCOLLAGE-060115" => (Fmt::X3x, true),
        b"PTCOLLAGE-060930" => (Fmt::X4x, true),
        b"PTCOLLAGE-071119" => (Fmt::V5, true),
        b"PTTUNE--20050608" => (Fmt::X2x, false),
        b"PTTUNE--20060115" => (Fmt::X3x, true),
        b"PTTUNE--20060930" => (Fmt::X4x, true),
        b"PTTUNE--20071119" => (Fmt::V5, true),
        _ => return None,
    };
    if exe {
        c.u16()?;
        c.u16()?;
    }
    Some(f)
}

pub fn probe(d: &[u8]) -> bool {
    version(&mut Cur { d, pos: 0 }).is_some()
}

fn skip_sized(c: &mut Cur) -> Option<()> {
    let size = c.i32()?;
    c.skip(i64::from(size))
}

/// How many events a file asks room for (pxtone sizes its list so).
fn count_events(d: &[u8]) -> Option<usize> {
    let mut c = Cur { d, pos: 0 };
    let f = version(&mut c)?;
    if f == Fmt::X1x {
        return Some(10_000);
    }
    let mut count: i64 = 0;
    loop {
        let code = c.bytes(8)?;
        match code {
            b"Event V5" => count += i64::from(Evelist::count_v5(&mut c)?),
            b"MasterV5" => {
                if c.u32()? != 15 {
                    return None;
                }
                c.bytes(15)?;
                count += 5;
            }
            b"evenMAST" => {
                c.i32()?;
                let data_num = c.u16()?;
                c.u16()?;
                let n = c.u32()?;
                if data_num != 3 {
                    return None;
                }
                for _ in 0..n {
                    c.v()?;
                    c.v()?;
                    c.v()?;
                }
                count += i64::from(n);
            }
            b"evenUNIT" => {
                c.i32()?;
                c.u16()?;
                c.u16()?;
                let data_num = c.u16()?;
                c.u16()?;
                let n = c.u32()?;
                if data_num != 2 {
                    return None;
                }
                for _ in 0..n {
                    c.v()?;
                    c.v()?;
                }
                count += i64::from(n);
            }
            b"pxtoneND" => break,
            b"antiOPER" | b"num UNIT" | b"pxtnUNIT" | b"matePCM " | b"matePTV " | b"matePTN " | b"mateOGGV" | b"effeDELA" | b"effeOVER" | b"textNAME" | b"textCOMM" | b"assiUNIT" | b"assiWOIC" => {
                skip_sized(&mut c)?
            }
            _ => return None,
        }
        if count > 2_000_000 {
            return None;
        }
    }
    if f <= Fmt::X3x {
        count += MAX_UNITS as i64 * 4;
    }
    Some(count as usize)
}

fn name16(b: &[u8]) -> String {
    let b = &b[..b.iter().position(|&x| x == 0).unwrap_or(b.len())];
    sjis_lossy(b)
}

/// Shift-JIS text as far as ASCII, kana and full-width letters and
/// digits go; other characters (kanji, symbols) as "?".
pub fn sjis_lossy(b: &[u8]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < b.len() {
        let x = b[i];
        match x {
            0x20..=0x7E => s.push(x as char),
            b'\r' | b'\n' | b'\t' => s.push(x as char),
            0xA1..=0xDF => s.push(char::from_u32(0xFF61 + u32::from(x - 0xA1)).unwrap_or('?')),
            0x81..=0x9F | 0xE0..=0xFC => {
                let y = u32::from(b.get(i + 1).copied().unwrap_or(0));
                let c = match (x, y) {
                    (0x81, 0x40) => Some(0x3000),
                    (0x82, 0x4F..=0x58) => Some(0xFF10 + y - 0x4F),
                    (0x82, 0x60..=0x79) => Some(0xFF21 + y - 0x60),
                    (0x82, 0x81..=0x9A) => Some(0xFF41 + y - 0x81),
                    (0x82, 0x9F..=0xF1) => Some(0x3041 + y - 0x9F),
                    (0x83, 0x40..=0x7E) => Some(0x30A1 + y - 0x40),
                    (0x83, 0x80..=0x96) => Some(0x30E0 + y - 0x80),
                    (0x81, 0x5B) => Some(0x30FC),
                    _ => None,
                };
                s.push(c.and_then(char::from_u32).unwrap_or('?'));
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    s
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        let n = count_events(d)?;
        let mut evels = Evelist::default();
        evels.allocate(n);
        let mut c = Cur { d, pos: 0 };
        let fmt = version(&mut c)?;
        let mut song = Song {
            master: Master::default(),
            evels,
            woices: Vec::new(),
            units: Vec::new(),
            delays: Vec::new(),
            overdrives: Vec::new(),
            name: Vec::new(),
            comment: Vec::new(),
            project: d.starts_with(b"PTCOLLAGE"),
            file: Arc::new(d.to_vec()),
        };
        if fmt >= Fmt::V5 {
            song.evels.clear();
            song.evels.linear = 0;
        } else {
            song.evels.clear();
            song.evels.linear = 0;
            song.evels.x4x = None;
        }
        song.read_items(&mut c)?;
        if fmt >= Fmt::V5 {
            song.evels.linear_end();
        }
        if fmt <= Fmt::X3x {
            // Old songs' keys and tunings were the voices'.
            if song.units.len() > song.woices.len() {
                return None;
            }
            for u in 0..song.units.len() {
                let change = song.woices[u].x3x_basic_key - DEFAULT_BASICKEY;
                if song.evels.count_unit_kind(u as u8, KIND_KEY) == 0 {
                    song.evels.add_i(0, u as u8, KIND_KEY, 0x6000);
                }
                song.evels.value_change(0, -1, u as u8, KIND_KEY, change);
            }
            for u in 0..song.units.len() {
                let t = song.woices[u].x3x_tuning;
                if t != 0.0 {
                    song.evels.add_i(0, u as u8, KIND_TUNING, t.to_bits() as i32);
                }
            }
            for (i, w) in song.woices.iter_mut().enumerate() {
                w.name = format!("voice_{i:02}");
            }
        }
        let clock1 = song.evels.max_clock();
        let clock2 = song.master.last_clock();
        song.master.adjust_meas_num(clock1.max(clock2));
        if song.master.beat_clock <= 0 || song.master.beat_num <= 0 || !(song.master.beat_tempo > 0.0) {
            return None;
        }
        Some(song)
    }

    fn read_items(&mut self, c: &mut Cur) -> Option<()> {
        loop {
            let code: [u8; 8] = c.bytes(8)?.try_into().ok()?;
            match &code {
                b"antiOPER" => return None,
                b"num UNIT" => {
                    if c.i32()? != 4 {
                        return None;
                    }
                    let num = c.u16()? as i16;
                    let rrr = c.u16()?;
                    if rrr != 0 || !(0..=MAX_UNITS as i16).contains(&num) {
                        return None;
                    }
                    self.units = vec![UnitInfo { name: "no name".into() }; num as usize];
                }
                b"MasterV5" => self.master.read_v5(c)?,
                b"Event V5" => self.evels.read_v5(c)?,
                b"matePCM " | b"matePTV " | b"matePTN " | b"mateOGGV" => {
                    if self.woices.len() >= MAX_WOICES {
                        return None;
                    }
                    let from = c.pos;
                    let mut w = match &code {
                        b"matePCM " => Woice::read_pcm(c)?,
                        b"matePTV " => Woice::read_ptv(c)?,
                        b"matePTN " => Woice::read_ptn(c)?,
                        _ => Woice::read_oggv(c)?,
                    };
                    w.raw = code.iter().chain(&c.d[from..c.pos]).copied().collect();
                    self.woices.push(w);
                }
                b"effeDELA" => {
                    if self.delays.len() >= MAX_DELAYS {
                        return None;
                    }
                    c.i32()?;
                    let unit = c.u16()?;
                    let group = c.u16()?;
                    let rate = c.f32()?;
                    let freq = c.f32()?;
                    if unit >= 3 {
                        return None;
                    }
                    let group = if usize::from(group) >= GROUPS { 0 } else { usize::from(group) };
                    self.delays.push(Delay {
                        unit,
                        group,
                        rate,
                        freq,
                        smp_num: 0,
                        offset: 0,
                        bufs: [Vec::new(), Vec::new()],
                        rate_s32: 100,
                    });
                }
                b"effeOVER" => {
                    if self.overdrives.len() >= MAX_OVERDRIVES {
                        return None;
                    }
                    c.i32()?;
                    let xxx = c.u16()?;
                    let group = c.u16()?;
                    let cut = c.f32()?;
                    let amp = c.f32()?;
                    let yyy = c.f32()?;
                    if xxx != 0 || yyy != 0.0 || !(50.0..=99.9).contains(&cut) || !(0.1..=8.0).contains(&amp) {
                        return None;
                    }
                    self.overdrives.push(OverDrive {
                        group: usize::from(group),
                        cut,
                        amp,
                        top: 0,
                    });
                }
                b"textNAME" | b"textCOMM" => {
                    let n = c.i32()?;
                    if n < 0 {
                        return None;
                    }
                    let t = c.bytes(n as usize)?.to_vec();
                    if &code == b"textNAME" {
                        self.name = t;
                    } else {
                        self.comment = t;
                    }
                }
                b"assiWOIC" | b"assiUNIT" => {
                    if c.i32()? != 20 {
                        return None;
                    }
                    let index = usize::from(c.u16()?);
                    let rrr = c.u16()?;
                    let name = name16(c.bytes(16)?);
                    if rrr != 0 {
                        return None;
                    }
                    if &code == b"assiWOIC" {
                        self.woices.get_mut(index)?.name = name;
                    } else {
                        self.units.get_mut(index)?.name = name;
                    }
                }
                b"pxtoneND" | b"END=====" => return Some(()),
                b"evenMAST" => self.master.read_x4x(c)?,
                b"evenUNIT" => self.evels.read_x4x_unit(c, false, true)?,
                b"pxtnUNIT" | b"UNIT====" => {
                    if self.units.len() >= MAX_UNITS {
                        return None;
                    }
                    let mut name = "no name".to_string();
                    let group = if &code == b"pxtnUNIT" {
                        c.i32()?;
                        let kind = c.u16()?;
                        let group = c.u16()?;
                        if !(1..=3).contains(&kind) {
                            return None;
                        }
                        group
                    } else {
                        c.i32()?;
                        name = name16(c.bytes(16)?);
                        let kind = c.u16()?;
                        let group = c.u16()?;
                        if kind != 1 {
                            return None;
                        }
                        group
                    };
                    let group = usize::from(group).min(GROUPS - 1);
                    let u = self.units.len() as u8;
                    self.evels.x4x_add(0, u, KIND_GROUPNO, group as i32);
                    self.evels.x4x = None;
                    self.evels.x4x_add(0, u, KIND_VOICENO, i32::from(u));
                    self.evels.x4x = None;
                    self.units.push(UnitInfo { name });
                }
                b"PROJECT=" => {
                    c.i32()?;
                    let name = c.bytes(16)?.to_vec();
                    let tempo = c.f32()?;
                    let clock = c.u16()?;
                    let num = c.u16()?;
                    c.bytes(12)?;
                    self.name = name[..name.iter().position(|&x| x == 0).unwrap_or(16)].to_vec();
                    self.master.beat_num = i32::from(num);
                    self.master.beat_tempo = tempo;
                    self.master.beat_clock = i32::from(clock);
                }
                b"matePCM=" => {
                    if self.woices.len() >= MAX_WOICES {
                        return None;
                    }
                    let from = c.pos;
                    let mut w = Woice::read_pcm(c)?;
                    w.raw = b"matePCM ".iter().chain(&c.d[from..c.pos]).copied().collect();
                    self.woices.push(w);
                }
                b"EVENT===" => self.evels.read_x4x_unit(c, true, false)?,
                _ => return None,
            }
        }
    }

    /// The song's events, in order: (clock, unit, kind, value).
    pub fn events(&self) -> Vec<(i32, u8, u8, i32)> {
        self.evels.iter().map(|r| (r.clock, r.unit, r.kind, r.value)).collect()
    }

    /// The samples it plays once through, at 44.1 kHz.
    pub fn end_sample(&self) -> i64 {
        let m = &self.master;
        let rate = clock_rate(m);
        (f64::from(m.play_meas()) * f64::from(m.beat_num) * f64::from(m.beat_clock) * f64::from(rate)) as i32 as i64
    }

    /// A woice's first voice's points, mono 16-bit at 44.1 kHz, for PCM voices.
    pub fn pcm_points(&self, w: usize) -> Option<Vec<i16>> {
        let wo = self.woices.get(w)?;
        if wo.kind != WoiceType::Pcm {
            return None;
        }
        let mut pcm = wo.voices.first()?.pcm.clone()?;
        if !pcm.convert(1, RATE, 16) {
            return None;
        }
        Some(pcm.smp.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect())
    }
}

fn clock_rate(m: &Master) -> f32 {
    (60.0f64 * f64::from(RATE) / (f64::from(m.beat_tempo) * f64::from(m.beat_clock))) as f32
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "ptcop"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        Some(self.end_sample() as f64 / f64::from(RATE))
    }

    /// Each woice (voice): its chunk as the file has it, but its unit's
    /// number. Its PCM, Ogg Vorbis, or PTV and PTN voice settings say it
    /// all.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, w) in self.woices.iter().enumerate() {
            if w.raw.len() < 14 {
                continue;
            }
            let mut raw = w.raw.clone();
            raw[12..14].fill(0);
            out.push(PatchBytes(raw).patch(k + 1, &w.name));
        }
        out
    }
}

// ---- Playing ----

#[derive(Clone, Copy, Default)]
struct Tone {
    smp_pos: f64,
    offset_freq: f32,
    env_volume: i32,
    life_count: i32,
    on_count: i32,
    env_start: i32,
    env_pos: i32,
    env_release_clock: i32,
}

#[derive(Clone)]
struct Unit {
    key_now: i32,
    key_start: i32,
    key_margin: i32,
    porta_pos: i32,
    porta_num: i32,
    pan_vols: [i32; 2],
    pan_times: [i32; 2],
    pan_bufs: [[i32; TIMEPAN]; 2],
    volume: i32,
    velocity: i32,
    group: i32,
    tuning: f32,
    woice: Option<usize>,
    vts: [Tone; UNIT_VOICES],
}

impl Unit {
    fn new() -> Unit {
        Unit {
            key_now: 0,
            key_start: 0,
            key_margin: 0,
            porta_pos: 0,
            porta_num: 0,
            pan_vols: [64, 64],
            pan_times: [0, 0],
            pan_bufs: [[0; TIMEPAN]; 2],
            volume: DEFAULT_VOLUME,
            velocity: DEFAULT_VELOCITY,
            group: 0,
            tuning: 1.0,
            woice: None,
            vts: [Tone::default(); UNIT_VOICES],
        }
    }
    fn init(&mut self) {
        self.group = 0;
        self.velocity = DEFAULT_VELOCITY;
        self.volume = DEFAULT_VOLUME;
        self.tuning = 1.0;
        self.porta_num = 0;
        self.porta_pos = 0;
        self.pan_vols = [64, 64];
        self.pan_times = [0, 0];
    }
}

/// The 16-bit samples pxtone makes, rendered whole (what its own player
/// writes), and each unit's part of them.
pub struct Moo {
    song: Arc<Song>,
    woices: Vec<Woice>,
    units: Vec<Unit>,
    delays: Vec<Delay>,
    overdrives: Vec<OverDrive>,
    freq: Freq,
    clock_rate: f32,
    smp_stride: f32,
    smp_count: i32,
    smp_end: i32,
    smp_repeat: i32,
    smp_smooth: i32,
    time_pan_index: usize,
    bt_tempo: f32,
    eve: Option<u32>,
    pub master_vol: f32,
    /// Whether it goes round at its end (else it stops), and whether it has.
    pub repeat: bool,
    pub looped: bool,
    /// Each unit's last sample (dry, then its share of its group's
    /// effects), left and right, before the master volume.
    pub unit_out: Vec<[f32; 2]>,
    energy: Vec<[f32; 2]>,
}

impl Moo {
    pub fn new(song: Arc<Song>) -> Moo {
        let bldr = NoiseBuilder::new();
        let mut woices = song.woices.clone();
        for w in &mut woices {
            if w.ready(&bldr).is_none() {
                // A voice that can't be made plays silence.
                for vi in &mut w.insts {
                    *vi = Instance::default();
                }
            }
        }
        let m = song.master;
        let mut delays = song.delays.clone();
        for d in &mut delays {
            d.ready(m.beat_num, m.beat_tempo, RATE);
        }
        let mut overdrives = song.overdrives.clone();
        for o in &mut overdrives {
            o.top = (32767.0f32 * (100.0 - o.cut) / 100.0) as i32;
        }
        let clock_rate = clock_rate(&m);
        let smp_end = (f64::from(m.play_meas()) * f64::from(m.beat_num) * f64::from(m.beat_clock) * f64::from(clock_rate)) as i32;
        let smp_repeat = (f64::from(m.repeat_meas) * f64::from(m.beat_num) * f64::from(m.beat_clock) * f64::from(clock_rate)) as i32;
        let n = song.units.len();
        let mut moo = Moo {
            woices,
            units: vec![Unit::new(); n],
            delays,
            overdrives,
            freq: Freq::new(),
            clock_rate,
            smp_stride: 44100.0f32 / RATE as f32,
            smp_count: 0,
            smp_end,
            smp_repeat,
            smp_smooth: RATE / 250,
            time_pan_index: 0,
            bt_tempo: m.beat_tempo,
            eve: song.evels.start,
            master_vol: 1.0,
            repeat: true,
            looped: false,
            unit_out: vec![[0.0; 2]; n],
            energy: vec![[0.0; 2]; n],
            song,
        };
        moo.init_unit_tone();
        moo
    }

    fn reset_voice_on(&mut self, u: usize, w: i32) {
        let Some(wo) = usize::try_from(w).ok().filter(|&w| w < self.woices.len()) else { return };
        let unit = &mut self.units[u];
        unit.woice = Some(wo);
        unit.key_now = DEFAULT_KEY;
        unit.key_margin = 0;
        unit.key_start = DEFAULT_KEY;
        let woice = &self.woices[wo];
        for (v, (vc, vi)) in woice.voices.iter().zip(&woice.insts).take(UNIT_VOICES).enumerate() {
            let ofs_freq = if vc.voice_flags & VOICEFLAG_BEATFIT != 0 {
                (vi.smp_body_w as f32 * self.bt_tempo) / ((44100 * 60) as f32 * vc.tuning)
            } else {
                self.freq.get(DEFAULT_BASICKEY - vc.basic_key) * vc.tuning
            };
            let t = &mut unit.vts[v];
            t.life_count = 0;
            t.on_count = 0;
            t.smp_pos = 0.0;
            t.env_release_clock = (vi.env_release as f32 / self.clock_rate) as i32;
            t.offset_freq = ofs_freq;
        }
    }

    fn init_unit_tone(&mut self) {
        for u in 0..self.units.len() {
            self.units[u].init();
            self.reset_voice_on(u, 0);
        }
    }

    fn voices(&self, u: usize) -> usize {
        self.units[u].woice.map_or(0, |w| self.woices[w].voices.len().min(UNIT_VOICES))
    }

    /// One stereo sample (pxtone's `_moo_PXTONE_SAMPLE`); false once a song
    /// that doesn't loop ends.
    pub fn sample(&mut self, out: &mut [i16; 2]) -> bool {
        // The envelopes.
        for u in 0..self.units.len() {
            let Some(w) = self.units[u].woice else { continue };
            let woice = &self.woices[w];
            for v in 0..woice.voices.len().min(UNIT_VOICES) {
                let vi = &woice.insts[v];
                let vt = &mut self.units[u].vts[v];
                if vt.life_count > 0 && vi.env_size != 0 {
                    if vt.on_count > 0 {
                        if vt.env_pos < vi.env_size {
                            vt.env_volume = i32::from(vi.env[vt.env_pos as usize]);
                            vt.env_pos += 1;
                        }
                    } else {
                        vt.env_volume = if vi.env_release != 0 {
                            vt.env_start.wrapping_add(vt.env_start.wrapping_neg().wrapping_mul(vt.env_pos) / vi.env_release)
                        } else {
                            0
                        };
                        vt.env_pos += 1;
                    }
                }
            }
        }
        let clock = (self.smp_count as f32 / self.clock_rate) as i32;
        // The events due.
        let song = self.song.clone();
        let ev = &song.evels;
        while let Some(i) = self.eve {
            let e = *ev.r(i);
            if e.clock > clock {
                break;
            }
            let u = usize::from(e.unit);
            if u < self.units.len() {
                self.event(u, &e, clock);
            }
            self.eve = e.next;
        }
        // Each unit's sample, into its time-pan buffer.
        for u in 0..self.units.len() {
            let Some(w) = self.units[u].woice else { continue };
            let woice = &self.woices[w];
            let unit = &mut self.units[u];
            for ch in 0..2 {
                let mut buf = 0i32;
                for v in 0..woice.voices.len().min(UNIT_VOICES) {
                    let vt = &unit.vts[v];
                    let vi = &woice.insts[v];
                    let mut work = 0i32;
                    if vt.life_count > 0 {
                        let pos = (vt.smp_pos as i32).wrapping_mul(2).wrapping_add(ch as i32);
                        work += usize::try_from(pos).ok().and_then(|p| vi.smp.get(p)).map_or(0, |&s| i32::from(s));
                        work = work.wrapping_mul(unit.velocity) / 128;
                        work = work.wrapping_mul(unit.volume) / 128;
                        work = work.wrapping_mul(unit.pan_vols[ch]) / 64;
                        if vi.env_size != 0 {
                            work = work.wrapping_mul(vt.env_volume) / 128;
                        }
                        if woice.voices[v].voice_flags & VOICEFLAG_SMOOTH != 0 && vt.life_count < self.smp_smooth {
                            work = work.wrapping_mul(vt.life_count) / self.smp_smooth;
                        }
                    }
                    buf = buf.wrapping_add(work);
                }
                unit.pan_bufs[ch][self.time_pan_index] = buf;
            }
        }
        // Mixed by group, through the overdrives and delays.
        for ch in 0..2 {
            let mut groups = [0i32; GROUPS];
            let mut dry = [0i32; MAX_UNITS];
            for (u, unit) in self.units.iter().enumerate() {
                let idx = (self.time_pan_index as i32 - unit.pan_times[ch]) & (TIMEPAN as i32 - 1);
                let x = unit.pan_bufs[ch][idx as usize];
                if let Some(g) = groups.get_mut(unit.group as usize) {
                    *g = g.wrapping_add(x);
                    dry[u] = x;
                }
            }
            let before = groups;
            for o in &self.overdrives {
                let Some(g) = groups.get_mut(o.group) else { continue };
                let work = if *g > o.top {
                    o.top
                } else if *g < -o.top {
                    -o.top
                } else {
                    *g
                };
                *g = (work as f32 * o.amp) as i32;
            }
            for d in &mut self.delays {
                if d.smp_num == 0 {
                    continue;
                }
                let a = d.bufs[ch][d.offset].wrapping_mul(d.rate_s32) / 100;
                groups[d.group] = groups[d.group].wrapping_add(a);
                d.bufs[ch][d.offset] = groups[d.group];
            }
            let work: i32 = groups.iter().fold(0i32, |s, &g| s.wrapping_add(g));
            let work = (work as f32 * self.master_vol) as i32;
            out[ch] = work.clamp(-0x7fff, 0x7fff) as i16;
            // Each unit's part: its own, and its group's effects shared out
            // by how much each has played lately.
            let mut total = [0.0f32; GROUPS];
            let mut count = [0usize; GROUPS];
            for (u, un) in self.units.iter().enumerate() {
                if let Some(g) = usize::try_from(un.group).ok().filter(|&g| g < GROUPS) {
                    total[g] += self.energy[u][ch];
                    count[g] += 1;
                }
            }
            for (u, un) in self.units.iter().enumerate() {
                self.unit_out[u][ch] = match usize::try_from(un.group).ok().filter(|&g| g < GROUPS) {
                    Some(g) => {
                        let share = if total[g] > 0.0 { self.energy[u][ch] / total[g] } else { 1.0 / count[g].max(1) as f32 };
                        dry[u] as f32 + groups[g].wrapping_sub(before[g]) as f32 * share
                    }
                    None => 0.0,
                };
                self.energy[u][ch] = self.energy[u][ch] * 0.9995 + (dry[u] as f32).abs();
            }
        }
        // On.
        self.smp_count += 1;
        self.time_pan_index = (self.time_pan_index + 1) & (TIMEPAN - 1);
        for u in 0..self.units.len() {
            let key = {
                let un = &mut self.units[u];
                if un.porta_num != 0 && un.key_margin != 0 {
                    if un.porta_pos < un.porta_num {
                        un.porta_pos += 1;
                        un.key_now = (f64::from(un.key_start) + f64::from(un.key_margin) * f64::from(un.porta_pos) / f64::from(un.porta_num)) as i32;
                    } else {
                        un.key_now = un.key_start.wrapping_add(un.key_margin);
                        un.key_start = un.key_now;
                        un.key_margin = 0;
                    }
                } else {
                    un.key_now = un.key_start.wrapping_add(un.key_margin);
                }
                un.key_now
            };
            let freq = self.freq.get2(key) * self.smp_stride;
            let n = self.voices(u);
            let Some(w) = self.units[u].woice else { continue };
            let woice = &self.woices[w];
            let un = &mut self.units[u];
            for v in 0..n {
                let vi = &woice.insts[v];
                let vt = &mut un.vts[v];
                if vt.life_count > 0 {
                    vt.life_count -= 1;
                }
                if vt.life_count > 0 {
                    vt.on_count -= 1;
                    vt.smp_pos += f64::from(vt.offset_freq * un.tuning * freq);
                    if vt.smp_pos >= f64::from(vi.smp_body_w) {
                        if woice.voices[v].voice_flags & VOICEFLAG_WAVELOOP != 0 {
                            if vt.smp_pos >= f64::from(vi.smp_body_w) {
                                vt.smp_pos -= f64::from(vi.smp_body_w);
                            }
                            if vt.smp_pos >= f64::from(vi.smp_body_w) {
                                vt.smp_pos = 0.0;
                            }
                        } else {
                            vt.life_count = 0;
                        }
                    }
                    if vt.on_count == 0 && vi.env_size != 0 {
                        vt.env_start = vt.env_volume;
                        vt.env_pos = 0;
                    }
                }
            }
        }
        for d in &mut self.delays {
            if d.smp_num == 0 {
                continue;
            }
            d.offset += 1;
            if d.offset >= d.smp_num {
                d.offset = 0;
            }
        }
        if self.smp_count >= self.smp_end {
            if !self.repeat {
                return false;
            }
            self.looped = true;
            self.smp_count = self.smp_repeat;
            self.eve = self.song.evels.start;
            self.init_unit_tone();
        }
        true
    }

    fn event(&mut self, u: usize, e: &Rec, clock: i32) {
        let rate = self.clock_rate;
        match e.kind {
            KIND_ON => {
                let on_count = (e.clock.wrapping_add(e.value).wrapping_sub(clock) as f32 * rate) as i32;
                if on_count <= 0 {
                    for t in &mut self.units[u].vts {
                        t.life_count = 0;
                    }
                    return;
                }
                let un = &mut self.units[u];
                un.key_now = un.key_start.wrapping_add(un.key_margin);
                un.key_start = un.key_now;
                un.key_margin = 0;
                let Some(w) = un.woice else { return };
                let n = self.voices(u);
                for v in 0..n {
                    let vi = &self.woices[w].insts[v];
                    let tone_rls = self.units[u].vts[v].env_release_clock;
                    let life = if vi.env_release != 0 {
                        let max1 = ((e.value.wrapping_sub(clock.wrapping_sub(e.clock))) as f32 * rate) as i32;
                        let max1 = max1.wrapping_add(vi.env_release);
                        let c = e.clock.wrapping_add(e.value).wrapping_add(tone_rls);
                        let ev = &self.song.evels;
                        let mut next = None;
                        let mut p = e.next;
                        while let Some(i) = p {
                            let r = ev.r(i);
                            if r.clock > c {
                                break;
                            }
                            if usize::from(r.unit) == u && r.kind == KIND_ON {
                                next = Some(*r);
                                break;
                            }
                            p = r.next;
                        }
                        let max2 = match next {
                            None => self.smp_end.wrapping_sub((clock as f32 * rate) as i32),
                            Some(n) => (n.clock.wrapping_sub(clock) as f32 * rate) as i32,
                        };
                        max1.min(max2)
                    } else {
                        (e.value.wrapping_sub(clock.wrapping_sub(e.clock)) as f32 * rate) as i32
                    };
                    let env_size = vi.env_size;
                    let t = &mut self.units[u].vts[v];
                    t.life_count = life;
                    if t.life_count > 0 {
                        t.on_count = on_count;
                        t.smp_pos = 0.0;
                        t.env_pos = 0;
                        let s = if env_size != 0 { 0 } else { 128 };
                        t.env_volume = s;
                        t.env_start = s;
                    }
                }
            }
            KIND_KEY => {
                let un = &mut self.units[u];
                un.key_start = un.key_now;
                un.key_margin = e.value.wrapping_sub(un.key_start);
                un.porta_pos = 0;
            }
            KIND_PAN_VOLUME => {
                let un = &mut self.units[u];
                un.pan_vols = [64, 64];
                if e.value >= 64 {
                    un.pan_vols[0] = 128i32.wrapping_sub(e.value);
                } else {
                    un.pan_vols[1] = e.value;
                }
            }
            KIND_PAN_TIME => {
                let un = &mut self.units[u];
                un.pan_times = [0, 0];
                if e.value >= 64 {
                    un.pan_times[0] = (e.value.wrapping_sub(64).min(63).wrapping_mul(44100)) / RATE;
                } else {
                    un.pan_times[1] = (64i32.wrapping_sub(e.value).min(63).wrapping_mul(44100)) / RATE;
                }
            }
            KIND_VELOCITY => self.units[u].velocity = e.value,
            KIND_VOLUME => self.units[u].volume = e.value,
            KIND_PORTAMENT => self.units[u].porta_num = (e.value as f32 * rate) as i32,
            KIND_VOICENO => self.reset_voice_on(u, e.value),
            KIND_GROUPNO => self.units[u].group = e.value,
            KIND_TUNING => self.units[u].tuning = f32::from_bits(e.value as u32),
            _ => {}
        }
    }

    /// The clock now (the song's ticks).
    pub fn clock(&self) -> i32 {
        (self.smp_count as f32 / self.clock_rate) as i32
    }
}

/// Samples a tick: 10 ms.
const TICK: usize = 441;
/// The Tracker's level for a unit's sample at full scale.
const LEVEL: f32 = 0.4 / 32768.0;

pub struct Player {
    moo: Moo,
    bufs: Vec<Vec<[f32; 2]>>,
    prev: Vec<[f32; 2]>,
    pos: f64,
    done: bool,
}

impl Player {
    fn new(song: Arc<Song>) -> Player {
        let n = song.units.len().max(1);
        let mut moo = Moo::new(song);
        moo.master_vol = 0.8;
        Player {
            moo,
            bufs: vec![vec![[0.0; 2]; TICK]; n],
            prev: vec![[0.0; 2]; n],
            pos: 0.0,
            done: false,
        }
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.bufs.len()
    }

    fn tick(&mut self) {
        let n = self.moo.units.len();
        for c in 0..self.bufs.len() {
            self.prev[c] = self.bufs[c][TICK - 1];
        }
        let mut s = [0i16; 2];
        for i in 0..TICK {
            self.moo.sample(&mut s);
            for u in 0..n {
                let o = self.moo.unit_out[u];
                self.bufs[u][i] = [o[0] * self.moo.master_vol, o[1] * self.moo.master_vol];
            }
        }
        if std::mem::take(&mut self.moo.looped) {
            self.done = true;
        }
        self.pos = (self.pos - TICK as f64).max(0.0);
    }

    fn tick_seconds(&self) -> f64 {
        TICK as f64 / f64::from(RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let step = f64::from(RATE) / f64::from(rate.max(1));
        for c in 0..self.bufs.len() {
            let (b, prev) = (&self.bufs[c], self.prev[c]);
            let at = |k: isize| if k < 0 { prev } else { b[(k as usize).min(TICK - 1)] };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (s0, s1) = (at(k as isize), at(k as isize + 1));
                l[i] = (s0[0] + (s1[0] - s0[0]) * f) * LEVEL;
                r[i] = (s0[1] + (s1[1] - s0[1]) * f) * LEVEL;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        let m = &self.moo.song.master;
        let meas = (m.beat_num * m.beat_clock).max(1);
        let clock = self.moo.clock().max(0);
        let rows = (m.beat_num * 4).max(1);
        ((clock / meas) as usize, ((clock % meas) * rows / meas) as usize)
    }

    fn speed(&self) -> (u32, u32) {
        (6, (self.moo.song.master.beat_tempo.round() as u32).clamp(1, 999))
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints() {
        let d = [0x05, 0x80, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
        let mut c = Cur { d: &d, pos: 0 };
        assert_eq!(c.v(), Some(5));
        assert_eq!(c.v(), Some(128));
        assert_eq!(c.v(), Some(-1));
    }

    #[test]
    fn frequency_table_doubles_an_octave() {
        let f = Freq::new();
        assert_eq!(f.get2(0x4500 + 12 * 256), f.get2(0x4500) * 2.0);
        assert_eq!(f.table[8 * 192], 1.0);
    }

    /// The samples pxtone's own player writes (ptwav, 44.1 kHz 16-bit
    /// stereo, master volume 0.80): PX_DIR the songs, PX_REF the renders
    /// (<name>.raw), the first PX_SECS seconds (10).
    #[test]
    #[ignore]
    fn the_same_as_the_original() {
        let (Ok(dir), Ok(rf)) = (std::env::var("PX_DIR"), std::env::var("PX_REF")) else { return };
        let only = std::env::var("PX_ONLY").ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        paths.sort();
        let (mut same, mut bad) = (0, 0);
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Ok(raw) = std::fs::read(format!("{rf}/{name}.raw")) else { continue };
            let Some(song) = Song::parse(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                bad += 1;
                continue;
            };
            let mut moo = Moo::new(Arc::new(song));
            moo.master_vol = 0.8;
            moo.repeat = std::env::var("PX_LOOP").is_ok();
            let theirs: Vec<i16> = raw.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            let mut s = [0i16; 2];
            let mut first = None;
            let mut diffs = 0usize;
            let mut ended = false;
            for k in 0..theirs.len() / 2 {
                // pxtone's player keeps no sample from the one that ends the song.
                if ended || !moo.sample(&mut s) {
                    ended = true;
                    s = [0, 0];
                }
                if s[0] != theirs[k * 2] || s[1] != theirs[k * 2 + 1] {
                    diffs += 1;
                    if first.is_none() {
                        first = Some((k, s, [theirs[k * 2], theirs[k * 2 + 1]]));
                    }
                }
            }
            match first {
                None => same += 1,
                Some((k, a, b)) => {
                    bad += 1;
                    eprintln!("{name}: {diffs} samples differ, first at {k} ({:.3} s): ours {a:?} theirs {b:?}", k as f64 / 44100.0);
                }
            }
        }
        eprintln!("{same} the same, {bad} not");
        assert_eq!(bad, 0);
    }
}
