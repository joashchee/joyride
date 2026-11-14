// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
//! NSF, the NES's music file: the game's own sound driver and its data, run on [`crate::cpu6502`] with the NSF
//! memory map (RAM, the $6000 work RAM, 4 KiB banks switched at $5FF8-$5FFF, the FDS's RAM and $5FF6-7), its
//! writes going to the 2A03 ([`crate::nes`]) and the expansion chips the header names: VRC6, MMC5, N163 and FDS
//! ([`crate::nes_ext`]), VRC7 ([`crate::opll`]) and the Sunsoft 5B ([`crate::ay`]). The init routine once, the
//! play routine at the header's rate, and between calls the CPU waits with its IRQ line live: the DMC's IRQ (a
//! sample's end, which NSD.Lib's banked-DPCM builds chain samples on) and the frame counter's, taken when the
//! program has written the IRQ vector at $FFFE (or NSF2 says it uses IRQs) and cleared the I flag. The 2A03's
//! writes, and the banks the DMC reads, take effect at the very cycle they're made; the expansion chips' at the
//! start of the frame they're made in.
//!
//! NSFe files (the chunked form: `INFO`, `DATA`, `BANK`, `RATE`, `auth`, `tlbl`, ...) and the metadata an NSF2 keeps
//! after its program are read too, from the NSFe specification (nesdev wiki): the songs' names, authors, lengths and
//! fades, the playlist, the text. A chunk whose name starts with a capital letter must be understood: one that isn't
//! known refuses the file, as the specification asks.

use std::sync::Arc;

use crate::ay::Ay;
use crate::cpu6502::{Bus, Cpu, RETURN};
use crate::nes::{Apu, Pending, DMC_NTSC, DMC_PAL, FRAME_NTSC, FRAME_PAL, LENGTHS, NTSC_HZ, PAL_HZ};
use crate::nes_ext::Expansion;
use crate::opll::Opll;
use crate::replay::{Bufs, Driver, Tune};

pub const VRC6: u8 = 0x01;
pub const VRC7: u8 = 0x02;
pub const FDS: u8 = 0x04;
pub const MMC5: u8 = 0x08;
pub const N163: u8 = 0x10;
pub const S5B: u8 = 0x20;

/// An NSF file read.
#[derive(Clone, Debug)]
pub struct Nsf {
    pub file: Vec<u8>,
    pub songs: usize,
    pub start: usize,
    pub load: u16,
    pub init: u16,
    pub play: u16,
    pub title: String,
    pub artist: String,
    pub copyright: String,
    /// The play routine's period, in seconds.
    pub period: f64,
    pub pal: bool,
    pub banks: [u8; 8],
    pub banked: bool,
    pub chips: u8,
    /// NSF2's flags byte (IRQ support, non-returning init, no play routine, metadata), 0 before version 2.
    pub flags: u8,
    /// The ripper's name (an NSFe's `auth`), and its `text`.
    pub ripper: String,
    pub text: String,
    /// Each song's name, author, length and fade, where the file gives them (`tlbl`, `taut`, `time`, `fade`).
    pub tracks: Vec<TrackInfo>,
    /// The order to play the songs in (`plst`), when given.
    pub playlist: Vec<usize>,
    /// `VRC7` says the chip is a YM2413 (its own patch set, not the VRC7's).
    pub ym2413: bool,
    /// The program, as the banks see it: padded at the front by the load address's place in its bank.
    image: Vec<u8>,
}

/// A song's metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackInfo {
    pub name: String,
    pub author: String,
    /// How long it plays and fades, in milliseconds.
    pub time: Option<u32>,
    pub fade: Option<u32>,
}

/// Text as UTF-8 (as NSFe asks), else as Shift JIS (as nsc writes what the MML had).
fn decode(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => crate::pxtone::sjis_lossy(b),
    }
}

fn text(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    decode(&b[..n]).trim().to_string()
}

/// Zero-terminated strings, one after another.
fn strings(b: &[u8]) -> Vec<String> {
    let b = b.strip_suffix(&[0]).unwrap_or(b);
    if b.is_empty() {
        return Vec::new();
    }
    b.split(|&c| c == 0).map(|s| decode(s).trim().to_string()).collect()
}

/// A chunk: its name, its bytes, and whether it came after `NEND`.
type Chunk<'a> = ([u8; 4], &'a [u8], bool);

/// An NSFe's chunks, or an NSF2's metadata: each chunk's name and bytes. Those after `NEND` are read too while they
/// hold together (nsc writes its `tlbl`, `taut`, `time` and `fade` there), and not held to the rules.
fn chunks(mut b: &[u8]) -> Result<Vec<Chunk<'_>>, String> {
    let mut out = Vec::new();
    let mut ended = false;
    while b.len() >= 8 {
        let len = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let id = [b[4], b[5], b[6], b[7]];
        let Some(data) = b.get(8..8usize.saturating_add(len)) else {
            if ended {
                break;
            }
            return Err(format!("the {} chunk is cut short", String::from_utf8_lossy(&id)));
        };
        if &id == b"NEND" {
            ended = true;
        } else if !(ended && !id.iter().all(|c| c.is_ascii_alphanumeric())) {
            out.push((id, data, ended));
        } else {
            break;
        }
        b = &b[8 + len..];
    }
    Ok(out)
}

impl Nsf {
    pub fn parse(file: &[u8]) -> Result<Nsf, String> {
        if file.starts_with(b"NSFE") {
            return Nsf::parse_nsfe(file);
        }
        if file.len() < 0x80 || &file[..5] != b"NESM\x1a" {
            return Err("not an NSF file".into());
        }
        let w = |o: usize| u16::from_le_bytes([file[o], file[o + 1]]);
        let mut banks = [0u8; 8];
        banks.copy_from_slice(&file[0x70..0x78]);
        let banked = banks.iter().any(|&b| b != 0);
        // NSF2's own length of the program (metadata may follow it).
        let len = usize::from(file[0x7d]) | usize::from(file[0x7e]) << 8 | usize::from(file[0x7f]) << 16;
        let data = if file[5] >= 2 && len > 0 { &file[0x80..(0x80 + len).min(file.len())] } else { &file[0x80..] };
        let load = w(8);
        if load < 0x6000 || (!banked && load < 0x8000 && file[0x7b] & FDS == 0) {
            return Err(format!("an NSF loaded at ${load:04X}"));
        }
        let pad = if banked { usize::from(load & 0x0fff) } else { 0 };
        let mut image = vec![0u8; pad];
        image.extend_from_slice(data);
        let video = file[0x7a];
        let pal = video & 3 == 1;
        let rate = if pal { w(0x78) } else { w(0x6e) };
        let period = if rate == 0 {
            if pal {
                1.0 / 50.0
            } else {
                1.0 / 60.0988
            }
        } else {
            f64::from(rate) / 1_000_000.0
        };
        let flags = if file[5] >= 2 { file[0x7c] } else { 0 };
        let mut n = Nsf {
            file: file.to_vec(),
            songs: usize::from(file[6]).max(1),
            start: usize::from(file[7]).max(1),
            load,
            init: w(0x0a),
            play: w(0x0c),
            title: text(&file[0x0e..0x2e]),
            artist: text(&file[0x2e..0x4e]),
            copyright: text(&file[0x4e..0x6e]),
            period,
            pal,
            banks,
            banked,
            chips: file[0x7b] & 0x3f,
            flags,
            ripper: String::new(),
            text: String::new(),
            tracks: Vec::new(),
            playlist: Vec::new(),
            ym2413: false,
            image,
        };
        // NSF2's metadata, after the program.
        if file[5] >= 2 && len > 0 && flags & 0x80 != 0 {
            if let Some(meta) = file.get(0x80 + len..) {
                for (id, d, late) in chunks(meta)? {
                    n.meta(&id, d, false, late)?;
                }
            }
        }
        Ok(n)
    }

    /// An NSFe: `INFO` and `DATA` it must have; the rest as given.
    fn parse_nsfe(file: &[u8]) -> Result<Nsf, String> {
        let all = chunks(&file[4..])?;
        let get = |id: &[u8; 4]| all.iter().find(|(i, _, late)| i == id && !late).map(|(_, d, _)| *d);
        let info = get(b"INFO").ok_or("an NSFe without its INFO chunk")?;
        if info.len() < 9 {
            return Err("an NSFe whose INFO is too short".into());
        }
        let data = get(b"DATA").ok_or("an NSFe without its DATA chunk")?;
        let w = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let mut banks = [0u8; 8];
        if let Some(b) = get(b"BANK") {
            let n = b.len().min(8);
            banks[..n].copy_from_slice(&b[..n]);
        }
        let banked = banks.iter().any(|&b| b != 0);
        let chips = info[7] & 0x3f;
        let load = w(info, 0);
        if load < 0x6000 || (!banked && load < 0x8000 && chips & FDS == 0) {
            return Err(format!("an NSFe loaded at ${load:04X}"));
        }
        let pad = if banked { usize::from(load & 0x0fff) } else { 0 };
        let mut image = vec![0u8; pad];
        image.extend_from_slice(data);
        let pal = info[6] & 3 == 1;
        let mut period = if pal { 1.0 / 50.0 } else { 1.0 / 60.0988 };
        if let Some(r) = get(b"RATE") {
            let rate = match (pal, r.len()) {
                (false, 2..) => w(r, 0),
                (true, 4..) => w(r, 2),
                _ => 0,
            };
            if rate != 0 {
                period = f64::from(rate) / 1_000_000.0;
            }
        }
        let mut n = Nsf {
            file: file.to_vec(),
            songs: usize::from(info.get(8).copied().unwrap_or(1)).max(1),
            // INFO's starting song counts from 0.
            start: usize::from(info.get(9).copied().unwrap_or(0)) + 1,
            load,
            init: w(info, 2),
            play: w(info, 4),
            title: String::new(),
            artist: String::new(),
            copyright: String::new(),
            period,
            pal,
            banks,
            banked,
            chips,
            flags: 0,
            ripper: String::new(),
            text: String::new(),
            tracks: Vec::new(),
            playlist: Vec::new(),
            ym2413: false,
            image,
        };
        n.start = n.start.min(n.songs);
        for (id, d, late) in &all {
            n.meta(id, d, true, *late)?;
        }
        Ok(n)
    }

    /// A metadata chunk; one with a capital first letter that isn't known (or isn't allowed here) refuses the file,
    /// unless it came after `NEND` (`late`), where only the descriptive chunks are read.
    fn meta(&mut self, id: &[u8; 4], d: &[u8], nsfe: bool, late: bool) -> Result<(), String> {
        if late && !matches!(id, b"auth" | b"text" | b"tlbl" | b"taut" | b"time" | b"fade" | b"plst") {
            return Ok(());
        }
        let ints = |d: &[u8]| -> Vec<Option<u32>> { d.chunks_exact(4).map(|c| u32::try_from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])).ok()).collect() };
        let track = |t: &mut Vec<TrackInfo>, k: usize| -> usize {
            if t.len() <= k {
                t.resize(k + 1, TrackInfo::default());
            }
            k
        };
        match id {
            b"INFO" | b"DATA" | b"BANK" | b"RATE" if nsfe => {}
            b"NSF2" => {
                if let Some(&f) = d.first() {
                    self.flags = f;
                }
            }
            b"VRC7" => self.ym2413 = d.first() == Some(&1),
            b"auth" => {
                let s = strings(d);
                let mut it = s.into_iter();
                for f in [&mut self.title, &mut self.artist, &mut self.copyright, &mut self.ripper] {
                    match it.next() {
                        Some(v) if !v.is_empty() => *f = v,
                        _ => {}
                    }
                }
            }
            b"text" => self.text = text(d),
            b"tlbl" => {
                for (k, v) in strings(d).into_iter().enumerate() {
                    let k = track(&mut self.tracks, k);
                    self.tracks[k].name = v;
                }
            }
            b"taut" => {
                for (k, v) in strings(d).into_iter().enumerate() {
                    let k = track(&mut self.tracks, k);
                    self.tracks[k].author = v;
                }
            }
            b"time" => {
                for (k, v) in ints(d).into_iter().enumerate() {
                    let k = track(&mut self.tracks, k);
                    self.tracks[k].time = v;
                }
            }
            b"fade" => {
                for (k, v) in ints(d).into_iter().enumerate() {
                    let k = track(&mut self.tracks, k);
                    self.tracks[k].fade = v;
                }
            }
            b"plst" => self.playlist = d.iter().map(|&b| usize::from(b)).filter(|&b| b < self.songs).collect(),
            _ if id[0].is_ascii_uppercase() => {
                return Err(format!("a part this player doesn't know and must ({})", String::from_utf8_lossy(id)));
            }
            _ => {}
        }
        self.tracks.truncate(self.songs);
        Ok(())
    }

    /// The program as the banks see it (padded at the front by the load address's place in its bank).
    pub fn image(&self) -> &[u8] {
        &self.image
    }
}

/// A sound register written: the play call it was written in (0 for init), its address, its value.
pub type Write = (u32, u16, u8);

/// What the CPU can see of the 2A03's DMC and frame counter, kept cycle by cycle beside the APU (which runs later,
/// as it renders): whether a sample plays, and the two IRQs. The DMC's fetches as [`crate::nes`] makes them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Status {
    pal: bool,
    rate: u16,
    timer: u16,
    bits: u8,
    buffer: bool,
    address_len: u16,
    remaining: u16,
    looping: bool,
    irq_on: bool,
    dmc_irq: bool,
    frame_cycle: u32,
    /// $4017: the five-step sequence (no IRQ), and the IRQ inhibited.
    five: bool,
    inhibit: bool,
    frame_irq: bool,
    /// The length counters of pulse 1, pulse 2, the triangle and the noise (what $4015 bits 0 to 3 say), their halt flags and enables:
    /// sound engines that wait for a note to end read them.
    lengths: [u8; 4],
    halts: [bool; 4],
    enabled: [bool; 4],
}

impl Status {
    fn new(pal: bool) -> Status {
        Status {
            pal,
            rate: if pal { DMC_PAL[0] } else { DMC_NTSC[0] },
            bits: 8,
            ..Status::default()
        }
    }

    fn write(&mut self, a: u16, v: u8) {
        match a {
            0x4010 => {
                self.looping = v & 0x40 != 0;
                self.irq_on = v & 0x80 != 0;
                if !self.irq_on {
                    self.dmc_irq = false;
                }
                self.rate = if self.pal { DMC_PAL } else { DMC_NTSC }[usize::from(v & 15)];
            }
            0x4013 => self.address_len = (u16::from(v) << 4) | 1,
            0x4000 | 0x4004 | 0x400c => self.halts[usize::from((a - 0x4000) / 4).min(3)] = v & 0x20 != 0,
            0x4008 => self.halts[2] = v & 0x80 != 0,
            0x4003 | 0x4007 | 0x400b | 0x400f => {
                let i = usize::from((a - 0x4003) / 4);
                if self.enabled[i] {
                    self.lengths[i] = LENGTHS[usize::from(v >> 3)];
                }
            }
            0x4015 => {
                for i in 0..4 {
                    self.enabled[i] = v >> i & 1 != 0;
                    if !self.enabled[i] {
                        self.lengths[i] = 0;
                    }
                }
                self.dmc_irq = false;
                if v & 0x10 == 0 {
                    self.remaining = 0;
                } else if self.remaining == 0 {
                    self.remaining = self.address_len;
                }
            }
            0x4017 => {
                self.five = v & 0x80 != 0;
                self.inhibit = v & 0x40 != 0;
                if self.inhibit {
                    self.frame_irq = false;
                }
                self.frame_cycle = 0;
            }
            _ => {}
        }
    }

    /// $4015 read: the DMC playing (bit 4), its IRQ (bit 7) and the frame counter's (bit 6, cleared by the read).
    fn read(&mut self) -> u8 {
        let lengths = self.lengths.iter().enumerate().fold(0u8, |b, (i, &l)| b | u8::from(l > 0) << i);
        let v = lengths | u8::from(self.remaining > 0) << 4 | u8::from(self.frame_irq) << 6 | u8::from(self.dmc_irq) << 7;
        self.frame_irq = false;
        v
    }

    /// `cycles` go by: per cycle the frame counter steps, the sample buffer refills when empty, then the DMC's timer
    /// counts down (a bit out each time it runs out). Taken a timer period at a time, and in one step when the DMC
    /// has nothing to fetch.
    fn run(&mut self, cycles: u32) {
        let steps = if self.pal { &FRAME_PAL } else { &FRAME_NTSC };
        let (half, last) = (steps[1], steps[3]);
        // The frame counter: a half frame (the lengths count down) at its second step, and at its last the frame IRQ and a half frame again.
        let (mut c, mut left) = (self.frame_cycle, cycles);
        loop {
            let target = if c < half { half } else { last };
            if left < target - c {
                c += left;
                break;
            }
            left -= target - c;
            c = target;
            for (l, h) in self.lengths.iter_mut().zip(self.halts) {
                if !h && *l > 0 {
                    *l -= 1;
                }
            }
            if target == last {
                if !self.five && !self.inhibit {
                    self.frame_irq = true;
                }
                c = 0;
            }
        }
        self.frame_cycle = c;
        let mut n = cycles;
        while n > 0 {
            if !self.buffer && self.remaining > 0 {
                self.buffer = true;
                self.remaining -= 1;
                if self.remaining == 0 {
                    if self.looping {
                        self.remaining = self.address_len;
                    } else if self.irq_on {
                        self.dmc_irq = true;
                    }
                }
            }
            // Cycles to the timer's next bit, this one counted.
            let k = u32::from(self.timer) + 1;
            if n < k {
                self.timer -= n as u16;
                return;
            }
            n -= k;
            let rate = u32::from(self.rate.max(1));
            let bits = if !self.buffer && self.remaining == 0 {
                // Nothing to fetch: every bit to come in one step.
                let more = n / rate;
                self.timer = (rate - 1 - n % rate) as u16;
                n = 0;
                1 + more
            } else {
                self.timer = (rate - 1) as u16;
                1
            };
            let left = i64::from(self.bits) - 1 - i64::from(bits);
            if left < 0 {
                self.buffer = false;
            }
            self.bits = (left.rem_euclid(8) + 1) as u8;
        }
    }
}

/// The NES as an NSF sees it.
pub struct Machine {
    ram: [u8; 0x800],
    /// $6000 to $FFFF: the work RAM, then the banks (copied in when switched).
    prg: Vec<u8>,
    nsf: Arc<Nsf>,
    pub apu: Apu,
    pub ext: Expansion,
    opll: Option<Opll>,
    opll_addr: u8,
    pub opll_regs: [u8; 0x40],
    pub ay: Option<Ay>,
    ay_addr: u8,
    pub ay_regs: [u8; 14],
    status: Status,
    /// CPU cycles since the first play call (the APU's clock as it renders); while `direct` (the init routine),
    /// writes reach the APU at once.
    pub clock: u64,
    direct: bool,
    /// The IRQ vector the program wrote at $FFFE-$FFFF.
    irq_vector: [Option<u8>; 2],
    mul: [u8; 2],
    exram: [u8; 0x400],
    pub frame: u32,
    /// Every sound register written, when kept.
    pub log: Option<Vec<Write>>,
    /// The sound registers as last written: the 2A03's ($4000-$4017), VRC6's ($9000-$9002, $A000-, $B000-),
    /// MMC5's ($5000-$5015) and the FDS's ($4080-$408A).
    pub apu_regs: [u8; 0x18],
    pub vrc6_regs: [u8; 9],
    pub mmc5_regs: [u8; 0x16],
    pub fds_regs: [u8; 0x0b],
    /// Which channels were (re)started this frame, a bit each in [`Player::chans`]'s numbering of the 2A03's:
    /// a write to $4003, $4007, $400B, $400F, or the DMC started.
    pub hits: u32,
}

impl Machine {
    fn bank(&mut self, slot: usize, n: u8) {
        // Slot 0 to 7: $8000 + slot * $1000; 8 and 9 (the FDS's): $6000 and $7000.
        let at = if slot >= 8 { (slot - 8) * 0x1000 } else { 0x2000 + slot * 0x1000 };
        let from = usize::from(n) * 0x1000;
        for i in 0..0x1000 {
            self.prg[at + i] = self.nsf.image.get(from + i).copied().unwrap_or(0);
        }
        if slot < 8 {
            self.dmc_sees(slot * 0x1000, self.prg[at..at + 0x1000].to_vec());
        }
    }

    /// The DMC's memory ($8000-$FFFF) changed at `off`, from now.
    fn dmc_sees(&mut self, off: usize, b: Vec<u8>) {
        if self.direct {
            self.apu.memory[off..off + b.len()].copy_from_slice(&b);
        } else {
            self.apu.at(self.clock, Pending::Mem(off, b));
        }
    }

    fn irq_allowed(&self) -> bool {
        self.irq_vector.iter().all(Option::is_some) || self.nsf.flags & 0x10 != 0
    }

    fn sound(&mut self, addr: u16, v: u8) {
        match addr {
            0x4000..=0x4017 => {
                self.apu_regs[usize::from(addr - 0x4000)] = v;
                match addr {
                    0x4003 | 0x4007 | 0x400b | 0x400f => self.hits |= 1 << ((addr - 0x4003) / 4),
                    0x4015 if v & 0x10 != 0 => self.hits |= 1 << 4,
                    _ => {}
                }
            }
            0x9000..=0x9002 | 0xa000..=0xa002 | 0xb000..=0xb002 => self.vrc6_regs[usize::from((addr >> 12) - 9) * 3 + usize::from(addr & 3)] = v,
            0x5000..=0x5015 => self.mmc5_regs[usize::from(addr - 0x5000)] = v,
            0x4080..=0x408a => self.fds_regs[usize::from(addr - 0x4080)] = v,
            _ => {}
        }
        if let Some(l) = self.log.as_mut() {
            l.push((self.frame, addr, v));
        }
    }
}

impl Bus for Machine {
    fn irq(&mut self) -> bool {
        (self.status.dmc_irq || self.status.frame_irq) && self.irq_allowed()
    }

    fn tick(&mut self, cycles: u32) {
        self.clock += u64::from(cycles);
        self.status.run(cycles);
    }

    fn read(&mut self, a: u16) -> u8 {
        match a {
            0x0000..=0x1fff => self.ram[usize::from(a & 0x7ff)],
            0x4015 => self.status.read(),
            0xfffe | 0xffff if self.irq_vector[usize::from(a & 1)].is_some() => self.irq_vector[usize::from(a & 1)].unwrap_or(0),
            0x4040..=0x407f if self.ext.fds.is_some() => 0,
            0x4800 => match self.ext.n163.as_mut() {
                Some(n) => {
                    let (addr, inc) = (n.addr(), n.increments());
                    let v = n.ram[usize::from(addr)];
                    if inc {
                        n.set_address(((addr + 1) & 0x7f) | 0x80);
                    }
                    v
                }
                None => 0,
            },
            0x5205 if self.nsf.chips & MMC5 != 0 => (u16::from(self.mul[0]) * u16::from(self.mul[1])) as u8,
            0x5206 if self.nsf.chips & MMC5 != 0 => ((u16::from(self.mul[0]) * u16::from(self.mul[1])) >> 8) as u8,
            0x5c00..=0x5fef if self.nsf.chips & MMC5 != 0 => self.exram[usize::from(a - 0x5c00)],
            0x6000..=0xffff => self.prg[usize::from(a - 0x6000)],
            _ => 0,
        }
    }

    fn write(&mut self, a: u16, v: u8) {
        let chips = self.nsf.chips;
        match a {
            0x0000..=0x1fff => self.ram[usize::from(a & 0x7ff)] = v,
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                self.status.write(a, v);
                if self.direct {
                    self.apu.write(a - 0x4000, v);
                } else {
                    self.apu.at(self.clock, Pending::Reg(a - 0x4000, v));
                }
                self.sound(a, v);
            }
            0x4040..=0x408a if chips & FDS != 0 => {
                if let Some(f) = self.ext.fds.as_mut() {
                    f.write(a, v);
                }
                self.sound(a, v);
            }
            0x4800 if chips & N163 != 0 => {
                if let Some(n) = self.ext.n163.as_mut() {
                    n.write_data(v);
                }
                self.sound(a, v);
            }
            0x5000..=0x5015 if chips & MMC5 != 0 => {
                if let Some(m) = self.ext.mmc5.as_mut() {
                    m.write(a - 0x5000, v);
                }
                self.sound(a, v);
            }
            0x5205 | 0x5206 if chips & MMC5 != 0 => self.mul[usize::from(a - 0x5205)] = v,
            0x5c00..=0x5fef if chips & MMC5 != 0 => self.exram[usize::from(a - 0x5c00)] = v,
            0x5ff6 | 0x5ff7 if chips & FDS != 0 => self.bank(8 + usize::from(a - 0x5ff6), v),
            0x5ff8..=0x5fff => {
                if self.nsf.banked {
                    self.bank(usize::from(a - 0x5ff8), v);
                }
            }
            0x6000..=0x7fff => self.prg[usize::from(a - 0x6000)] = v,
            _ => {
                // $8000 and up: the FDS's RAM, and the mappers' sound ports.
                if chips & FDS != 0 && (0x8000..=0xdfff).contains(&a) {
                    self.prg[usize::from(a - 0x6000)] = v;
                    self.dmc_sees(usize::from(a - 0x8000), vec![v]);
                }
                if a >= 0xfffe {
                    self.irq_vector[usize::from(a & 1)] = Some(v);
                }
                if chips & VRC6 != 0 && matches!(a, 0x9000..=0x9003 | 0xa000..=0xa002 | 0xb000..=0xb002) {
                    if let Some(x) = self.ext.vrc6.as_mut() {
                        x.write(a, v);
                    }
                    self.sound(a, v);
                }
                if chips & VRC7 != 0 {
                    if a == 0x9010 {
                        self.opll_addr = v & 0x3f;
                    } else if a == 0x9030 {
                        let r = usize::from(self.opll_addr);
                        self.opll_regs[r] = v;
                        if let Some(o) = self.opll.as_mut() {
                            o.write(r, v);
                        }
                        self.sound(0x9030, v);
                    }
                }
                if chips & N163 != 0 && (0xf800..=0xffff).contains(&a) {
                    if let Some(n) = self.ext.n163.as_mut() {
                        n.set_address(v);
                    }
                    self.sound(0xf800, v);
                }
                if chips & S5B != 0 {
                    if (0xc000..=0xdfff).contains(&a) {
                        self.ay_addr = v & 0x0f;
                    } else if (0xe000..=0xffff).contains(&a) {
                        let r = usize::from(self.ay_addr);
                        if r < 14 {
                            self.ay_regs[r] = v;
                            let regs = self.ay_regs;
                            if let Some(y) = self.ay.as_mut() {
                                y.write_regs(&regs, r == 13);
                            }
                        }
                        self.sound(0xe000 | self.ay_addr as u16, v);
                    }
                }
            }
        }
    }
}

/// A play call runs at most this many cycles (a frame's are about 30,000).
const CALL_CYCLES: u64 = 1_000_000;

/// An NSF's song playing.
pub struct Player {
    cpu: Cpu,
    pub machine: Machine,
    nsf: Arc<Nsf>,
    chans: Vec<Chan>,
    bufs: [Vec<f32>; 5],
    ext_bufs: [Vec<f32>; 14],
    dc: Vec<(f32, f32)>,
    opll_rate: u32,
    halted: bool,
    /// Each frame's state (RAM, work RAM, the registers), by its hash: the frame it was first seen.
    seen: std::collections::HashMap<u64, u32>,
    /// The frame the song came round to, once it has.
    pub looped: Option<u32>,
    done: bool,
    /// CPU cycles a play call's frame lasts, and the calls made.
    frame_cycles: f64,
    calls: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chan {
    /// The 2A03's pulse 1, pulse 2, triangle, noise, DMC: 0 to 4.
    Apu(usize),
    Vrc6(usize),
    Vrc7(usize),
    Fds,
    Mmc5(usize),
    N163(usize),
    S5b(usize),
}

impl Player {
    pub fn new(nsf: Arc<Nsf>, song: usize, log: bool) -> Player {
        let c = nsf.chips;
        let clock = if nsf.pal { PAL_HZ } else { NTSC_HZ };
        let ay = (c & S5B != 0).then(|| Ay::new(true, clock / 2.0, 44100));
        let opll = (c & VRC7 != 0).then(|| {
            let mut o = Opll::new(3_579_545, 44100);
            if !nsf.ym2413 {
                o.set_vrc7();
            }
            o
        });
        let mut m = Machine {
            ram: [0; 0x800],
            prg: vec![0; 0xa000],
            nsf: nsf.clone(),
            apu: Apu::new(nsf.pal),
            ext: Expansion::new(clock, c & VRC6 != 0, c & MMC5 != 0, c & N163 != 0, c & FDS != 0),
            opll,
            opll_addr: 0,
            opll_regs: [0; 0x40],
            ay,
            ay_addr: 0,
            ay_regs: [0, 0, 0, 0, 0, 0, 0, 0x3f, 0, 0, 0, 0, 0, 0],
            status: Status::new(nsf.pal),
            clock: 0,
            direct: true,
            irq_vector: [None; 2],
            mul: [0xff; 2],
            exram: [0; 0x400],
            frame: 0,
            log: log.then(Vec::new),
            apu_regs: [0; 0x18],
            vrc6_regs: [0; 9],
            mmc5_regs: [0; 0x16],
            fds_regs: [0; 0x0b],
            hits: 0,
        };
        if nsf.banked {
            for (i, &b) in nsf.banks.iter().enumerate() {
                m.bank(i, b);
            }
            if c & FDS != 0 {
                m.bank(8, nsf.banks[6]);
                m.bank(9, nsf.banks[7]);
            }
        } else {
            let at = usize::from(nsf.load) - 0x6000;
            let n = nsf.image.len().min(m.prg.len() - at);
            m.prg[at..at + n].copy_from_slice(&nsf.image[..n]);
            m.apu.memory.copy_from_slice(&m.prg[0x2000..0xa000]);
        }
        for a in 0x4000..=0x4013u16 {
            m.write(a, 0);
        }
        m.write(0x4015, 0);
        m.write(0x4015, 0x0f);
        m.write(0x4017, 0x40);
        if c & FDS != 0 {
            m.write(0x4089, 0x80);
            m.write(0x408a, 0xe8);
        }
        let mut chans: Vec<Chan> = (0..5).map(Chan::Apu).collect();
        if c & VRC6 != 0 {
            chans.extend((0..3).map(Chan::Vrc6));
        }
        if c & VRC7 != 0 {
            chans.extend((0..6).map(Chan::Vrc7));
        }
        if c & FDS != 0 {
            chans.push(Chan::Fds);
        }
        if c & MMC5 != 0 {
            chans.extend((0..2).map(Chan::Mmc5));
        }
        if c & N163 != 0 {
            chans.extend((0..8).map(Chan::N163));
        }
        if c & S5B != 0 {
            chans.extend((0..3).map(Chan::S5b));
        }
        let mut cpu = Cpu::default();
        cpu.a = (song.min(nsf.songs.max(1) - 1)) as u8;
        cpu.x = u8::from(nsf.pal);
        let halted = !cpu.call(&mut m, nsf.init, CALL_CYCLES * 4);
        m.frame = 1;
        // From here the APU's clock is the CPU's: the first play call at cycle 0.
        m.direct = false;
        m.clock = 0;
        let frame_cycles = nsf.period * if nsf.pal { PAL_HZ } else { NTSC_HZ };
        Player {
            cpu,
            machine: m,
            nsf,
            dc: vec![(0.0, 0.0); 19],
            chans,
            bufs: Default::default(),
            ext_bufs: Default::default(),
            opll_rate: 44100,
            halted,
            seen: std::collections::HashMap::new(),
            looped: None,
            done: false,
            frame_cycles,
            calls: 0,
        }
    }

    /// The cycle play call `k` starts at.
    fn frame_start(&self, k: u64) -> u64 {
        (k as f64 * self.frame_cycles).round() as u64
    }

    /// Lets the CPU wait at [`RETURN`] until `until`, taking the IRQ when it comes (and running its handler).
    fn wait(&mut self, until: u64) {
        let m = &mut self.machine;
        while m.clock < until {
            let live = self.cpu.p & 0x04 == 0 && m.irq_allowed() && (m.status.irq_on || !(m.status.inhibit || m.status.five));
            if self.cpu.pc != RETURN || (live && m.irq()) {
                self.cpu.step(m);
                if self.cpu.jammed {
                    self.halted = true;
                    return;
                }
            } else {
                // Nothing to do: on to the end, or a few cycles at a time while an IRQ may come.
                let n = if live { (until - m.clock).min(8) } else { until - m.clock } as u32;
                m.tick(n);
                self.cpu.cycles += u64::from(n);
            }
        }
    }

    /// One call of the play routine, then the CPU waits for the next (taking IRQs).
    pub fn play(&mut self) {
        self.machine.hits = 0;
        if self.halted {
            return;
        }
        let start = self.frame_start(self.calls);
        self.calls += 1;
        // Changes the APU's rendering didn't reach (none render between ticks) are made now.
        self.machine.apu.flush_before(start);
        // An IRQ handler (or a long play routine) still running from the frame before finishes first. A play routine that never
        // returns (some rips loop `JSR engine / CLC / BCC` in it: libgme runs those too) simply keeps running, a frame at a time.
        let next = self.frame_start(self.calls);
        if self.cpu.pc != RETURN {
            while self.cpu.pc != RETURN && self.machine.clock < next && !self.cpu.jammed {
                self.cpu.step(&mut self.machine);
            }
            if self.cpu.jammed {
                self.halted = true;
                return;
            }
            if self.cpu.pc != RETURN {
                self.machine.frame += 1;
                return;
            }
        }
        self.wait(start);
        self.cpu.a = 0;
        self.cpu.call(&mut self.machine, self.nsf.play, next.saturating_sub(start).max(1));
        if self.cpu.jammed {
            self.halted = true;
        }
        if !self.halted {
            self.wait(next);
        }
        self.machine.frame += 1;
        if self.looped.is_none() {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            let m = &self.machine;
            (&m.ram[..], &m.prg[..0x2000], &m.apu_regs, &m.vrc6_regs, &m.mmc5_regs, &m.fds_regs, &m.opll_regs, &m.ay_regs).hash(&mut h);
            if let Some(n) = m.ext.n163.as_ref() {
                // Without the phases the chip keeps in its RAM (bytes 1, 3 and 5 of each channel's 8 at $40-$7F):
                // they move as it sounds.
                let mut ram = n.ram;
                for k in 0..8 {
                    for o in [1, 3, 5] {
                        ram[0x40 + 8 * k + o] = 0;
                    }
                }
                ram.hash(&mut h);
            }
            let k = h.finish();
            let f = self.machine.frame;
            match self.seen.get(&k) {
                Some(&at) => {
                    self.looped = Some(at);
                    self.done = true;
                    self.seen = std::collections::HashMap::new();
                }
                None => {
                    self.seen.insert(k, f);
                }
            }
        }
    }

    /// What each channel plays now: its MIDI note (fractional), when it sounds. Read from the registers.
    pub fn notes(&self) -> Vec<Option<f64>> {
        let m = &self.machine;
        let clock = if self.nsf.pal { PAL_HZ } else { NTSC_HZ };
        let r = &m.apu_regs;
        let on = r[0x15];
        let midi = |f: f64| if f > 8.0 && f < 20_000.0 { Some(69.0 + 12.0 * (f / 440.0).log2()) } else { None };
        let pulse = |regs: &[u8], enabled: bool| -> Option<f64> {
            let p = u32::from(regs[2]) | (u32::from(regs[3] & 7) << 8);
            let vol = regs[0] & 15;
            if !enabled || p < 8 || (vol == 0 && regs[0] & 0x10 != 0) {
                return None;
            }
            midi(clock / (16.0 * f64::from(p + 1)))
        };
        self.chans
            .iter()
            .map(|c| match *c {
                Chan::Apu(n @ 0..=1) => pulse(&r[n * 4..n * 4 + 4], on & (1 << n) != 0),
                Chan::Apu(2) => {
                    let p = u32::from(r[0x0a]) | (u32::from(r[0x0b] & 7) << 8);
                    if on & 4 == 0 || r[8] & 0x7f == 0 || p < 2 {
                        None
                    } else {
                        midi(clock / (32.0 * f64::from(p + 1)))
                    }
                }
                Chan::Apu(3) => {
                    if on & 8 == 0 || (r[0x0c] & 15 == 0 && r[0x0c] & 0x10 != 0) {
                        None
                    } else {
                        // The noise's 16 rates, shown as notes up from C-3.
                        Some(f64::from(48 + 15 - (r[0x0e] & 15)))
                    }
                }
                Chan::Apu(_) => (m.status.remaining > 0).then(|| f64::from(48 + (r[0x10] & 15))),
                Chan::Vrc6(n) => {
                    let v = &m.vrc6_regs[n * 3..n * 3 + 3];
                    let p = u32::from(v[1]) | (u32::from(v[2] & 15) << 8);
                    let vol = if n == 2 { v[0] & 0x3f } else { v[0] & 15 };
                    if v[2] & 0x80 == 0 || vol == 0 {
                        None
                    } else {
                        midi(clock / (if n == 2 { 14.0 } else { 16.0 } * f64::from(p + 1)))
                    }
                }
                Chan::Mmc5(n) => pulse(&m.mmc5_regs[n * 4..n * 4 + 4], m.mmc5_regs[0x15] & (1 << n) != 0),
                Chan::Fds => {
                    let f = &m.fds_regs;
                    let p = u32::from(f[2]) | (u32::from(f[3] & 15) << 8);
                    if f[3] & 0x80 != 0 || p == 0 || f[0] & 0x3f == 0 && f[0] & 0x80 != 0 {
                        None
                    } else {
                        midi(clock * f64::from(p) / 4_194_304.0)
                    }
                }
                Chan::N163(k) => {
                    let n = m.ext.n163.as_ref()?;
                    let nch = n.channels();
                    if k >= nch {
                        return None;
                    }
                    // Channel k is the chip's 7 - k: its registers at $78 - 8k.
                    let b = 0x78 - 8 * k;
                    let ram = &n.ram;
                    let f = u32::from(ram[b]) | u32::from(ram[b + 2]) << 8 | u32::from(ram[b + 4] & 3) << 16;
                    let len = 256 - u32::from(ram[b + 4] & 0xfc);
                    if ram[b + 7] & 15 == 0 || f == 0 {
                        None
                    } else {
                        midi(clock * f64::from(f) / (15.0 * 65536.0 * nch as f64 * f64::from(len)))
                    }
                }
                Chan::Vrc7(n) => {
                    let o = &m.opll_regs;
                    let fnum = u32::from(o[0x10 + n]) | u32::from(o[0x20 + n] & 1) << 8;
                    let block = (o[0x20 + n] >> 1) & 7;
                    if o[0x20 + n] & 0x10 == 0 || fnum == 0 {
                        None
                    } else {
                        midi(49_716.0 * f64::from(fnum) * f64::from(1u32 << block) / 524_288.0)
                    }
                }
                Chan::S5b(n) => {
                    let a = &m.ay_regs;
                    let p = u32::from(a[n * 2]) | u32::from(a[n * 2 + 1] & 15) << 8;
                    if a[7] & (1 << n) != 0 || a[8 + n] & 0x1f == 0 || p == 0 {
                        None
                    } else {
                        midi(clock / 2.0 / (16.0 * f64::from(p)))
                    }
                }
            })
            .collect()
    }

    pub fn chans(&self) -> &[Chan] {
        &self.chans
    }

    pub fn ram(&self) -> &[u8] {
        &self.machine.ram
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.chans.len()
    }

    fn tick(&mut self) {
        self.play();
    }

    fn tick_seconds(&self) -> f64 {
        self.nsf.period
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for b in self.bufs.iter_mut() {
            b.clear();
            b.resize(frames, 0.0);
        }
        {
            let [a, b, c, d, e] = &mut self.bufs;
            self.machine.apu.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..], &mut e[..]]);
        }
        self.machine.ext.render(rate, &mut self.ext_bufs, frames);
        for c in 0..self.chans.len() {
            out.left[c][..frames].fill(0.0);
            out.right[c][..frames].fill(0.0);
        }
        if self.machine.opll.is_some() && self.opll_rate != rate {
            let mut o = Opll::new(3_579_545, rate);
            if !self.nsf.ym2413 {
                o.set_vrc7();
            }
            for (r, &v) in self.machine.opll_regs.iter().enumerate() {
                o.write(r, v);
            }
            self.machine.opll = Some(o);
            self.opll_rate = rate;
        }
        if let Some(a) = self.machine.ay.as_mut() {
            a.set_rate(rate);
        }
        // The levels the FamiTracker player mixes the same chips at.
        let r = 1.0 - (2.0 * std::f64::consts::PI * 10.0 / f64::from(rate.max(1))) as f32;
        for (c, ch) in self.chans.iter().enumerate() {
            let (k, level) = match *ch {
                Chan::Apu(n) => (n, 1.7),
                Chan::Vrc6(n) => (5 + n, 0.19),
                Chan::Mmc5(n) => (8 + n, 0.19),
                Chan::N163(n) => (10 + n, 0.19),
                Chan::Fds => (18, 0.19),
                _ => continue,
            };
            let input: &[f32] = if k < 5 { &self.bufs[k] } else { &self.ext_bufs[k - 5] };
            let (mut x1, mut y1) = self.dc[k];
            for i in 0..frames {
                let x = input[i];
                let y = x - x1 + r * y1;
                x1 = x;
                y1 = y;
                out.left[c][i] = y * level;
                out.right[c][i] = y * level;
            }
            self.dc[k] = (x1, y1);
        }
        let vrc7: Vec<(usize, usize)> = self.chans.iter().enumerate().filter_map(|(c, ch)| if let Chan::Vrc7(n) = ch { Some((c, *n)) } else { None }).collect();
        let s5b: Vec<(usize, usize)> = self.chans.iter().enumerate().filter_map(|(c, ch)| if let Chan::S5b(n) = ch { Some((c, *n)) } else { None }).collect();
        if vrc7.is_empty() && s5b.is_empty() {
            return;
        }
        for i in 0..frames {
            if let Some(o) = self.machine.opll.as_mut() {
                let v = o.calc_channels();
                for &(c, n) in &vrc7 {
                    let x = f32::from(v[n]) * (0.15 / 2048.0);
                    out.left[c][i] = x;
                    out.right[c][i] = x;
                }
            }
            if let Some(a) = self.machine.ay.as_mut() {
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
        (0, 0)
    }

    fn speed(&self) -> (u32, u32) {
        (1, 150)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.halted
    }
}

/// A song's notes as its registers play them, frame by frame: each channel's note when it starts or changes
/// (its MIDI note, None for silence), until the song comes round or ends, or `most` frames.
pub struct Timeline {
    pub frames: u32,
    pub restart: Option<u32>,
    pub events: Vec<(u32, usize, Option<u8>)>,
    pub chans: Vec<Chan>,
}

pub fn timeline(nsf: &Arc<Nsf>, song: usize, most: u32) -> Timeline {
    let mut p = Player::new(nsf.clone(), song, false);
    let mut was: Vec<Option<u8>> = vec![None; p.chans.len()];
    let mut events = Vec::new();
    let mut frames = 0;
    while frames < most && p.playing() && p.looped.is_none() {
        p.play();
        frames += 1;
        let hits = p.machine.hits;
        for (c, n) in p.notes().into_iter().enumerate() {
            let n = n.map(|n| n.round().clamp(0.0, 127.0) as u8);
            let hit = c < 5 && hits & (1 << c) != 0 && n.is_some();
            if n != was[c] || hit {
                events.push((frames - 1, c, n));
                was[c] = n;
            }
        }
    }
    let mut restart = p.looped.map(|f| f.saturating_sub(1));
    // A state that comes round within a quarter second is the song's end (the driver idling), not a loop.
    if restart.is_some_and(|r| frames - r < 16) {
        frames = restart.unwrap_or(frames);
        restart = None;
    }
    Timeline {
        frames,
        restart,
        events,
        chans: p.chans.clone(),
    }
}

impl Chan {
    pub fn name(&self) -> String {
        match *self {
            Chan::Apu(n) => ["Pulse 1", "Pulse 2", "Triangle", "Noise", "DPCM"][n].into(),
            Chan::Vrc6(n) => ["VRC6 Pulse 1", "VRC6 Pulse 2", "VRC6 Saw"][n].into(),
            Chan::Vrc7(n) => format!("VRC7 FM {}", n + 1),
            Chan::Fds => "FDS".into(),
            Chan::Mmc5(n) => format!("MMC5 Pulse {}", n + 1),
            Chan::N163(n) => format!("N163 {}", n + 1),
            Chan::S5b(n) => format!("5B {}", ["A", "B", "C"][n]),
        }
    }
}

/// An NSF as a song.
pub struct Song {
    pub nsf: Arc<Nsf>,
    format: &'static str,
}

impl Song {
    pub fn parse(file: &[u8]) -> Result<Song, String> {
        Ok(Song {
            nsf: Arc::new(Nsf::parse(file)?),
            format: "nsf",
        })
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        self.format
    }

    fn file(&self) -> &[u8] {
        &self.nsf.file
    }

    fn songs(&self) -> usize {
        self.nsf.songs
    }

    fn driver(&self, song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.nsf.clone(), song, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A DMC program: rates, lengths, loops, the IRQ, starts and stops, the frame counter's modes.
    fn program() -> Vec<(u32, u16, u8)> {
        vec![
            (0, 0x4017, 0x00),
            (10, 0x4010, 0x8f),
            (10, 0x4013, 0x02),
            (11, 0x4015, 0x1f),
            (5_000, 0x4010, 0x4f),
            (9_000, 0x4015, 0x0f),
            (9_100, 0x4010, 0x80),
            (9_100, 0x4013, 0x01),
            (9_200, 0x4015, 0x1f),
            (40_000, 0x4017, 0x40),
            (41_000, 0x4010, 0x0a),
            (41_000, 0x4015, 0x1f),
            (90_000, 0x4015, 0x00),
        ]
    }

    // Run in big steps, the CPU-side clock is the same as one cycle at a time.
    #[test]
    fn the_status_clock_skips_like_it_counts() {
        for pal in [false, true] {
            let (mut a, mut b) = (Status::new(pal), Status::new(pal));
            let mut prog = program().into_iter().peekable();
            let mut steps = [1u32, 7, 333, 29_000, 3].into_iter().cycle();
            let mut t = 0u32;
            while t < 120_000 {
                while let Some(&(at, r, v)) = prog.peek() {
                    if at > t {
                        break;
                    }
                    a.write(r, v);
                    b.write(r, v);
                    prog.next();
                }
                let next = prog.peek().map_or(120_000, |p| p.0);
                let n = steps.next().unwrap().min(next.max(t + 1) - t);
                for _ in 0..n {
                    a.run(1);
                }
                b.run(n);
                t += n;
                assert_eq!(a, b, "at {t}");
            }
        }
    }

    // The length counters the CPU reads in $4015 (some sound engines wait on them to time their notes): loaded by $4003 and its kin when the
    // channel is enabled, counted down at the frame counter's half frames (cycles 14913 and 29829 of 29830), held by the halt flag.
    #[test]
    fn the_status_length_counters_count_down_at_half_frames() {
        let mut st = Status::new(false);
        st.write(0x4015, 0x0f);
        st.write(0x4000, 0x00);
        st.write(0x4003, 0x18); // length index 3: 2 half frames
        st.write(0x4007, 0x08); // length index 1: 254
        assert_eq!(st.read() & 0x0f, 0b0011);
        st.run(14_912);
        assert_eq!(st.read() & 1, 1, "before the first half frame");
        st.run(1);
        assert_eq!(st.read() & 1, 1, "one half frame: one to go");
        st.run(14_916);
        assert_eq!(st.read() & 1, 0, "two half frames: ended");
        assert_eq!(st.read() & 2, 2, "the other is 254 long");
        // Halted: stays. Disabled: gone. A write to a disabled channel loads nothing.
        st.write(0x4004, 0x20);
        st.write(0x4003, 0x18);
        st.run(60_000);
        assert_eq!(st.read() & 0b11, 0b10);
        st.write(0x4015, 0x00);
        assert_eq!(st.read() & 0x0f, 0);
        st.write(0x4003, 0x18);
        assert_eq!(st.read() & 1, 0);
    }

    // The CPU-side clock says the DMC plays exactly when the APU's does.
    #[test]
    fn the_status_clock_follows_the_apu() {
        let mut apu = Apu::new(false);
        let mut st = Status::new(false);
        let rate = apu.clock_hz().round() as u32;
        let mut prog = program().into_iter().peekable();
        let mut out = [[0f32; 1]; 5];
        let mut done = 0u64;
        while apu.cycles < 100_000 {
            let t = apu.cycles as u32;
            while prog.peek().is_some_and(|p| p.0 <= t) {
                let (_, r, v) = prog.next().unwrap();
                apu.write(r - 0x4000, v);
                st.write(r, v);
            }
            let [a, b, c, d, e] = &mut out;
            apu.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..], &mut e[..]]);
            st.run((apu.cycles - done) as u32);
            done = apu.cycles;
            assert_eq!(apu.dmc_playing(), st.remaining > 0, "at {t}");
        }
    }

    fn nsfe(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut f = b"NSFE".to_vec();
        for (id, d) in chunks {
            f.extend_from_slice(&(d.len() as u32).to_le_bytes());
            f.extend_from_slice(*id);
            f.extend_from_slice(d);
        }
        f
    }

    #[test]
    fn nsfe_reads_and_keeps_its_rules() {
        // INFO: load $8000, init $8000, play $8001, NTSC, no chips, 2 songs, starting at the second (0-based 1).
        let info = vec![0x00, 0x80, 0x00, 0x80, 0x01, 0x80, 0, 0, 2, 1];
        let data = vec![0x60, 0x60];
        let f = nsfe(&[
            (b"INFO", info.clone()),
            (b"DATA", data.clone()),
            (b"RATE", vec![0x1a, 0x41]),
            (b"auth", b"Game\0Artist\0(c)\0Ripper\0".to_vec()),
            (b"tlbl", b"One\0Two\0".to_vec()),
            (b"time", [90_000i32, -1].iter().flat_map(|t| t.to_le_bytes()).collect()),
            (b"plst", vec![1, 0, 7]),
            (b"xtra", vec![1, 2, 3]),
            (b"NEND", vec![]),
        ]);
        let n = Nsf::parse(&f).unwrap();
        assert_eq!((n.songs, n.start, n.play, n.title.as_str(), n.artist.as_str(), n.ripper.as_str()), (2, 2, 0x8001, "Game", "Artist", "Ripper"));
        assert!((n.period - 0x411a as f64 / 1e6).abs() < 1e-12);
        assert_eq!(n.tracks.iter().map(|t| (t.name.as_str(), t.time)).collect::<Vec<_>>(), [("One", Some(90_000)), ("Two", None)]);
        assert_eq!(n.playlist, [1, 0]);
        // A capitalised chunk it doesn't know refuses the file; INFO and DATA must be there.
        assert!(Nsf::parse(&nsfe(&[(b"INFO", info.clone()), (b"DATA", data.clone()), (b"ZZZZ", vec![]), (b"NEND", vec![])])).is_err());
        assert!(Nsf::parse(&nsfe(&[(b"INFO", info), (b"NEND", vec![])])).is_err());
        assert!(Nsf::parse(&nsfe(&[(b"DATA", data), (b"NEND", vec![])])).is_err());
    }
}
