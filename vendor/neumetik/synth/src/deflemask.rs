// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! DefleMask's modules (.dmf), unpacked: the format its author's
//! `DMF_SPECS.txt` describes (file version 24, DefleMask 1.0), and the
//! versions before and after it (16 to 27), whose differences were found
//! from the files themselves. A .dmf is zlib-packed; the host unpacks it
//! (many of Modland's have a wrong checksum, which DefleMask's own files
//! carry, so it isn't checked).

/// The sound hardware a module is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// YM2612 and SN76489; `ext3`: FM channel 3's operators as four channels.
    Genesis {
        ext3: bool,
    },
    /// SN76489.
    Sms,
    /// SN76489 and YM2413 (the FM Sound Unit).
    SmsOpll,
    GameBoy,
    /// HuC6280.
    PcEngine,
    /// 2A03.
    Nes,
    /// SID; `mos6581`: the older chip, else the 8580.
    C64 {
        mos6581: bool,
    },
    /// YM2151 and SegaPCM.
    Arcade,
    /// YM2610; `ext2`: FM channel 2's operators as four channels.
    NeoGeo {
        ext2: bool,
    },
}

impl System {
    pub fn from_byte(b: u8) -> Option<System> {
        Some(match b {
            0x02 => System::Genesis { ext3: false },
            0x42 => System::Genesis { ext3: true },
            0x03 => System::Sms,
            0x43 => System::SmsOpll,
            0x04 => System::GameBoy,
            0x05 => System::PcEngine,
            0x06 => System::Nes,
            0x07 => System::C64 { mos6581: false },
            0x47 => System::C64 { mos6581: true },
            0x08 => System::Arcade,
            0x09 => System::NeoGeo { ext2: false },
            0x49 => System::NeoGeo { ext2: true },
            _ => return None,
        })
    }

    pub fn channels(self) -> usize {
        match self {
            System::Genesis { ext3 } => 10 + 3 * usize::from(ext3),
            System::Sms => 4,
            System::SmsOpll => 13,
            System::GameBoy => 4,
            System::PcEngine => 6,
            System::Nes => 5,
            System::C64 { .. } => 3,
            System::Arcade => 13,
            System::NeoGeo { ext2 } => 13 + 3 * usize::from(ext2),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            System::Genesis { ext3: false } => "Genesis",
            System::Genesis { ext3: true } => "Genesis (ext. ch. 3)",
            System::Sms => "Master System",
            System::SmsOpll => "Master System + FM",
            System::GameBoy => "Game Boy",
            System::PcEngine => "PC Engine",
            System::Nes => "NES",
            System::C64 { mos6581: false } => "C64 (8580)",
            System::C64 { mos6581: true } => "C64 (6581)",
            System::Arcade => "Arcade",
            System::NeoGeo { ext2: false } => "Neo Geo",
            System::NeoGeo { ext2: true } => "Neo Geo (ext. ch. 2)",
        }
    }

    fn game_boy(self) -> bool {
        self == System::GameBoy
    }
}

/// A macro: its values and where it loops back to (None: it holds its last).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Macro {
    pub values: Vec<i32>,
    pub loop_at: Option<usize>,
}

/// An FM operator, in DefleMask's fields (DT stored +3, as the editor shows it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Op {
    pub am: u8,
    pub ar: u8,
    pub dr: u8,
    pub mult: u8,
    pub rr: u8,
    pub sl: u8,
    pub tl: u8,
    pub dt2: u8,
    pub rs: u8,
    pub dt: u8,
    pub d2r: u8,
    /// Bit 3 on, bits 0-2 the mode.
    pub ssg: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fm {
    pub alg: u8,
    pub fb: u8,
    /// FMS on the YM2612, PMS on the YM2151.
    pub lfo: u8,
    /// AMS.
    pub lfo2: u8,
    pub ops: [Op; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instrument {
    pub name: String,
    /// An FM instrument, else a standard one (its macros).
    pub fm: Option<Fm>,
    pub volume: Macro,
    /// Semitones from 12 (or, fixed, notes).
    pub arpeggio: Macro,
    pub arp_fixed: bool,
    pub duty: Macro,
    pub wave: Macro,
    /// The C64's settings, as the file has them (20 bytes).
    pub c64: Vec<u8>,
    /// The Game Boy's envelope: volume, direction, length, sound length.
    pub gb: [u8; 4],
}

/// A row of a channel: note (0 none, 1-12 C# to C, 100 off), octave,
/// volume (-1 none), effects (code, value; -1 none) and instrument (-1 none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub note: i16,
    pub octave: i16,
    pub volume: i16,
    pub fx: Vec<(i16, i16)>,
    pub instrument: i16,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub name: String,
    /// The rate (an index: 1 to 5 for 8, 11, 22, 32 and 44 kHz... as the
    /// system has them), pitch (5 as recorded), amp (50 as recorded) and bits.
    pub rate: u8,
    pub pitch: u8,
    pub amp: u8,
    pub bits: u8,
    pub data: Vec<i16>,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub version: u8,
    pub system: System,
    pub name: String,
    pub author: String,
    pub time_base: u8,
    pub tick1: u8,
    pub tick2: u8,
    /// NTSC (60 Hz), else PAL (50 Hz).
    pub ntsc: bool,
    /// A rate of its own, overriding NTSC or PAL.
    pub custom_hz: Option<u32>,
    pub rows: usize,
    /// Each channel's pattern at each order.
    pub matrix: Vec<Vec<u16>>,
    pub instruments: Vec<Instrument>,
    /// Wavetables (their values as the system's wave width has them).
    pub waves: Vec<Vec<u32>>,
    /// Each channel's effect columns.
    pub fx_columns: Vec<u8>,
    /// Each channel's rows, by order (the file keeps a pattern per order).
    pub patterns: Vec<Vec<Vec<Cell>>>,
    pub samples: Vec<Sample>,
}

pub const MAGIC: &[u8] = b".DelekDefleMask.";

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.d.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn i16(&mut self) -> Option<i16> {
        let b = self.take(2)?;
        Some(i16::from_le_bytes([b[0], b[1]]))
    }
    fn u16(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn text(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        let b = self.take(n)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        Some(b[..end].iter().map(|&c| c as char).collect::<String>().trim_end().to_string())
    }
    fn mac(&mut self) -> Option<Macro> {
        let n = usize::from(self.u8()?);
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(self.u32()? as i32);
        }
        let mut loop_at = None;
        if n > 0 {
            let l = self.u8()? as i8;
            if l >= 0 && (l as usize) < n {
                loop_at = Some(l as usize);
            }
        }
        Some(Macro { values, loop_at })
    }
}

/// Before version 20 the header has the arpeggio's tick speed.
const ARP_SPEED_UNTIL: u8 = 20;
/// Before version 18 the Game Boy's instruments have a volume macro and no envelope.
const GB_ENVELOPE_FROM: u8 = 18;
/// Before version 19 an FM instrument's fields are wider (see `old_fm`).
const FM_NARROW_FROM: u8 = 19;
const SAMPLE_BITS_FROM: u8 = 22;
const SAMPLE_NAME_FROM: u8 = 23;
/// From version 25 the matrix's entries are 16 bits.
const WIDE_MATRIX_FROM: u8 = 25;
const SAMPLE_LOOP_FROM: u8 = 27;

/// Patterns at most this long, matrices at most this many orders.
const MOST_ROWS: usize = 256;
/// A channel's effect columns: 1 to this many.
const MOST_FX: u8 = 4;

impl Module {
    pub fn probe(d: &[u8]) -> bool {
        d.len() > 18 && d.starts_with(MAGIC) && (16..=27).contains(&d[16]) && System::from_byte(d[17]).is_some()
    }

    /// Reads an unpacked module. Samples the file breaks off in are left out.
    pub fn parse(d: &[u8]) -> Option<Module> {
        if !Module::probe(d) {
            return None;
        }
        let mut r = Reader { d, p: 16 };
        let version = r.u8()?;
        let system = System::from_byte(r.u8()?)?;
        let channels = system.channels();
        let name = r.text()?;
        let author = r.text()?;
        r.take(2)?; // the pattern highlights
        let time_base = r.u8()?;
        let tick1 = r.u8()?;
        let tick2 = r.u8()?;
        let ntsc = r.u8()? != 0;
        let custom = r.u8()? != 0;
        let hz = r.take(3)?;
        let digits: String = hz.iter().take_while(|c| c.is_ascii_digit()).map(|&c| c as char).collect();
        let custom_hz = if custom { digits.parse::<u32>().ok().filter(|&h| h > 0) } else { None };
        let rows = if version >= 24 { r.u32()? as usize } else { usize::from(r.u8()?) };
        if rows == 0 || rows > MOST_ROWS {
            return None;
        }
        let orders = usize::from(r.u8()?);
        if orders == 0 {
            return None;
        }
        if version < ARP_SPEED_UNTIL {
            r.u8()?;
        }
        let mut matrix = vec![Vec::with_capacity(orders); channels];
        for ch in matrix.iter_mut() {
            for _ in 0..orders {
                ch.push(if version >= WIDE_MATRIX_FROM { r.u16()? } else { u16::from(r.u8()?) });
            }
        }
        let count = usize::from(r.u8()?);
        let mut instruments = Vec::with_capacity(count);
        for _ in 0..count {
            let mut ins = Instrument {
                name: r.text()?,
                ..Default::default()
            };
            if r.u8()? == 1 {
                ins.fm = Some(if version < FM_NARROW_FROM { old_fm(&mut r)? } else { fm(&mut r)? });
            } else {
                if !system.game_boy() || version < GB_ENVELOPE_FROM {
                    ins.volume = r.mac()?;
                }
                ins.arpeggio = r.mac()?;
                ins.arp_fixed = r.u8()? != 0;
                ins.duty = r.mac()?;
                ins.wave = r.mac()?;
                match system {
                    System::C64 { .. } => ins.c64 = r.take(20)?.to_vec(),
                    System::GameBoy if version >= GB_ENVELOPE_FROM => ins.gb.copy_from_slice(r.take(4)?),
                    _ => {}
                }
            }
            instruments.push(ins);
        }
        let count = usize::from(r.u8()?);
        let mut waves = Vec::with_capacity(count);
        for _ in 0..count {
            let n = r.u32()? as usize;
            if n > 4096 {
                return None;
            }
            let mut w = Vec::with_capacity(n);
            for _ in 0..n {
                w.push(r.u32()?);
            }
            waves.push(w);
        }
        // A file that breaks off in its patterns (some on Modland do, their
        // deflate stream whole) keeps what's there: the rest is empty rows.
        // More effect columns than DefleMask has is a file damaged earlier.
        let mut broken = false;
        let mut fx_columns = Vec::with_capacity(channels);
        let mut patterns = Vec::with_capacity(channels);
        for _ in 0..channels {
            let fx = if broken { None } else { r.u8() };
            if fx.is_some_and(|n| !(1..=MOST_FX).contains(&n)) {
                return None;
            }
            broken |= fx.is_none();
            let fx = fx.unwrap_or(1);
            fx_columns.push(fx);
            let mut by_order = Vec::with_capacity(orders);
            for _ in 0..orders {
                let mut cells = Vec::with_capacity(rows);
                for _ in 0..rows {
                    let cell = if broken { None } else { read_cell(&mut r, fx) };
                    broken |= cell.is_none();
                    cells.push(cell.unwrap_or_else(|| Cell {
                        volume: -1,
                        fx: vec![(-1, -1); usize::from(fx)],
                        instrument: -1,
                        ..Default::default()
                    }));
                }
                by_order.push(cells);
            }
            patterns.push(by_order);
        }
        let samples = if broken { Vec::new() } else { read_samples(&mut r, version) };
        Some(Module {
            version,
            system,
            name,
            author,
            time_base,
            tick1,
            tick2,
            ntsc,
            custom_hz,
            rows,
            matrix,
            instruments,
            waves,
            fx_columns,
            patterns,
            samples,
        })
    }

    /// The engine's rate: ticks a second.
    pub fn hz(&self) -> f64 {
        match self.custom_hz {
            Some(h) => f64::from(h),
            None if self.ntsc => 60.0,
            None => 50.0,
        }
    }

    pub fn orders(&self) -> usize {
        self.matrix.first().map_or(0, Vec::len)
    }
}

fn op(b: &[u8], at: [usize; 12]) -> Op {
    Op {
        am: b[at[0]],
        ar: b[at[1]],
        dr: b[at[2]],
        mult: b[at[3]],
        rr: b[at[4]],
        sl: b[at[5]],
        tl: b[at[6]],
        dt2: b[at[7]],
        rs: b[at[8]],
        dt: b[at[9]],
        d2r: b[at[10]],
        ssg: b[at[11]],
    }
}

fn fm(r: &mut Reader) -> Option<Fm> {
    let h = r.take(4)?;
    let mut f = Fm {
        alg: h[0],
        fb: h[1],
        lfo: h[2],
        lfo2: h[3],
        ..Default::default()
    };
    for o in f.ops.iter_mut() {
        *o = op(r.take(12)?, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    }
    Some(f)
}

/// Before version 19: ALG, FB, LFO and LFO2 a byte each with a byte
/// after, and each operator 18 bytes, its fields at these places (found
/// from what DefleMask's files play: the rest are 0).
fn old_fm(r: &mut Reader) -> Option<Fm> {
    let h = r.take(8)?;
    let mut f = Fm {
        alg: h[0],
        fb: h[2],
        lfo: h[4],
        lfo2: h[6],
        ..Default::default()
    };
    for o in f.ops.iter_mut() {
        *o = op(r.take(18)?, [0, 1, 3, 7, 8, 9, 11, 12, 14, 15, 16, 17]);
    }
    Some(f)
}

fn read_cell(r: &mut Reader, fx: u8) -> Option<Cell> {
    let note = r.i16()?;
    let octave = r.i16()?;
    let volume = r.i16()?;
    let mut e = Vec::with_capacity(usize::from(fx));
    for _ in 0..fx {
        e.push((r.i16()?, r.i16()?));
    }
    let instrument = r.i16()?;
    Some(Cell {
        note,
        octave,
        volume,
        fx: e,
        instrument,
    })
}

fn read_samples(r: &mut Reader, version: u8) -> Vec<Sample> {
    let mut out = Vec::new();
    let Some(count) = r.u8() else { return out };
    for _ in 0..count {
        let Some(s) = read_sample(r, version) else { break };
        out.push(s);
    }
    out
}

fn read_sample(r: &mut Reader, version: u8) -> Option<Sample> {
    let n = r.u32()? as usize;
    let name = if version >= SAMPLE_NAME_FROM { r.text()? } else { String::new() };
    let rate = r.u8()?;
    let pitch = r.u8()?;
    let amp = r.u8()?;
    let bits = if version >= SAMPLE_BITS_FROM { r.u8()? } else { 16 };
    if version >= SAMPLE_LOOP_FROM {
        r.take(8)?;
    }
    let b = r.take(n.checked_mul(2)?)?;
    let data = b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    Some(Sample { name, rate, pitch, amp, bits, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads every unpacked module in DMF_DIR (`*.raw`, written by the
    /// tracker's loader test) to its samples.
    #[test]
    #[ignore]
    fn reads_all() {
        let dir = std::env::var("DMF_DIR").expect("DMF_DIR");
        let (mut ok, mut bad) = (0, 0);
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_none_or(|x| x != "raw") {
                continue;
            }
            let d = std::fs::read(&p).unwrap();
            match Module::parse(&d) {
                Some(_) => ok += 1,
                None => {
                    bad += 1;
                    println!("not read: {}", p.display());
                }
            }
        }
        println!("{ok} read, {bad} not");
    }
}
