//! The original MDSDRV (`src/mdsdrv.68k` of superctr/MDSDRV, Zlib; its directory in `MDS_SRC`) assembled by `asm` and run by `cpu`, to be compared with
//! `mdsdrv::Driver`: the same chip writes, in the same order, frame by frame. The Z80 isn't run: its bus request reads as granted and its RAM is plain
//! memory, so the PCM channels do nothing here and only the YM2612 and PSG writes of the 68000 are compared.

use super::asm::assemble;
use super::cpu::{Bus, Cpu};
use std::collections::HashMap;

pub const ROM_AT: u32 = 0x10_0000;
pub const WORK: u32 = 0xff_0000;

pub struct Mem {
    pub code: Vec<u8>,
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub zram: Vec<u8>,
    pub log: Vec<(u8, u8, u8)>,
    ym_addr: [u8; 2],
}

impl Bus for Mem {
    fn read8(&mut self, a: u32) -> u8 {
        let a = a & 0xff_ffff;
        match a {
            0..=0x0f_ffff => self.code.get(a as usize).copied().unwrap_or(0),
            0x10_0000..=0x1f_ffff => self.rom.get((a - ROM_AT) as usize).copied().unwrap_or(0),
            0xa0_0000..=0xa0_1fff => self.zram[(a - 0xa0_0000) as usize],
            0xff_0000..=0xff_ffff => self.ram[(a - 0xff_0000) as usize],
            _ => 0,
        }
    }

    fn write8(&mut self, a: u32, v: u8) {
        let a = a & 0xff_ffff;
        match a {
            0xa0_0000..=0xa0_1fff => self.zram[(a - 0xa0_0000) as usize] = v,
            0xa0_4000 => self.ym_addr[0] = v,
            0xa0_4001 => self.log.push((0, self.ym_addr[0], v)),
            0xa0_4002 => self.ym_addr[1] = v,
            0xa0_4003 => self.log.push((1, self.ym_addr[1], v)),
            0xc0_0011 => self.log.push((2, 0, v)),
            0xff_0000..=0xff_ffff => self.ram[(a - 0xff_0000) as usize] = v,
            _ => {}
        }
    }
}

pub struct Original {
    pub cpu: Cpu,
    pub mem: Mem,
    pub sym: HashMap<String, i64>,
}

/// Read the driver's sources from `dir` and assemble them.
pub fn build(dir: &std::path::Path) -> Result<(Vec<u8>, HashMap<String, i64>), String> {
    let src = dir.join("src");
    let files = |name: &str| -> Option<Vec<u8>> {
        if name.contains("out/") {
            // the Z80 program: a single byte stands in (the Z80 isn't run)
            return Some(vec![0]);
        }
        let b = std::fs::read(src.join(name)).ok()?;
        if name == "mdsdrv.inc" {
            // Not packed: the Z80 program isn't there to unpack.
            let t = String::from_utf8_lossy(&b).lines().map(|l| if l.starts_with("PACK_Z80") { "PACK_Z80 equ 0".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("\n");
            return Some(t.into_bytes());
        }
        Some(b)
    };
    // The driver is assembled after the definitions the demo's main file brings in.
    let main = "all.68k";
    let wrap = |name: &str| -> Option<Vec<u8>> {
        if name == main {
            return Some(b"\tinclude \"mddef.inc\"\n\tinclude \"mdsdrv.inc\"\n\tinclude \"mdsseq.inc\"\n\tinclude \"mdssub.inc\"\n\tinclude \"mdsdrv.68k\"\n".to_vec());
        }
        files(name)
    };
    let o = assemble(main, &[], &wrap)?;
    Ok((o.bytes, o.symbols))
}

impl Original {
    pub fn new(dir: &std::path::Path, rom: &[u8]) -> Result<Original, String> {
        let (code, sym) = build(dir)?;
        let mut o = Original {
            cpu: Cpu::default(),
            mem: Mem { code, rom: rom.to_vec(), ram: vec![0; 0x1_0000], zram: vec![0; 0x2000], log: vec![], ym_addr: [0; 2] },
            sym,
        };
        o.cpu.a[7] = 0x0100_0000;
        o.cpu.a[0] = WORK;
        o.cpu.a[1] = ROM_AT;
        o.cpu.a[2] = ROM_AT + 0x8_0000;
        let init = o.addr("mds_init")?;
        o.cpu.call(&mut o.mem, init, 2_000_000)?;
        if o.cpu.d[0] & 0xffff != 0 {
            return Err("mds_init refused the data".into());
        }
        Ok(o)
    }

    pub fn addr(&self, name: &str) -> Result<u32, String> {
        self.sym.get(name).map(|v| *v as u32).ok_or_else(|| format!("no symbol {name}"))
    }

    /// `mds_request`.
    pub fn request(&mut self, id: u16, priority: u16) -> Result<(), String> {
        self.cpu.a[0] = WORK;
        self.cpu.d[0] = u32::from(id);
        self.cpu.d[1] = u32::from(priority);
        let a = self.addr("mds_request")?;
        self.cpu.call(&mut self.mem, a, 1000)
    }

    /// `mds_update`: one frame.
    pub fn update(&mut self) -> Result<(), String> {
        self.cpu.a[0] = WORK;
        let a = self.addr("mds_update")?;
        self.cpu.call(&mut self.mem, a, 2_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> Option<std::path::PathBuf> {
        std::env::var("MDS_SRC").ok().map(Into::into).filter(|p: &std::path::PathBuf| p.join("src/mdsdrv.68k").exists())
    }

    #[test]
    fn the_original_assembles() {
        let Some(dir) = src() else { return };
        let (code, sym) = build(&dir).unwrap_or_else(|e| panic!("{e}"));
        eprintln!("{} bytes, mds_init at {:#x}", code.len(), sym["mds_init"]);
        assert!(code.len() > 4000);
        if let Ok(at) = std::env::var("MDS_DUMP") {
            let at = usize::from_str_radix(at.trim_start_matches("0x"), 16).unwrap();
            eprintln!("{:04x?}", code[at..at + 40].chunks(2).map(|c| u16::from(c[0]) << 8 | u16::from(c[1])).collect::<Vec<_>>());
        }
        if let Ok(at) = std::env::var("MDS_WHERE") {
            let at = i64::from_str_radix(at.trim_start_matches("0x"), 16).unwrap();
            let mut near: Vec<_> = sym.iter().filter(|(k, v)| **v <= at && !k.contains('@')).collect();
            near.sort_by_key(|x| -*x.1);
            eprintln!("{:?}", &near[..3.min(near.len())]);
        }
    }

    /// Play `frames` frames of music (song 1 at priority 3) on the original and on the port; the first difference in the chip writes is an error.
    fn compare(dir: &std::path::Path, f: &[u8], frames: usize) -> Result<usize, String> {
        use crate::mdsdrv::{Driver, Song};
        let song = Song::parse(f).map_err(|e| e.to_string())?;
        let mut o = Original::new(dir, song.rom())?;
        o.mem.log.clear();
        o.request(1, 3)?;
        let mut d = Driver::new(&song);
        d.chip_log = Some(vec![]);
        let mut total = 0;
        for fr in 0..frames {
            o.update()?;
            d.update();
            let a: Vec<_> = std::mem::take(&mut o.mem.log);
            let b: Vec<_> = d.chip_log.as_mut().map(std::mem::take).unwrap_or_default();
            total += a.len();
            let tbase = o.sym["w_track"] as usize;
            let tsize = o.sym["TSIZE"] as usize;
            for (tn, t) in d.tracks().iter().enumerate() {
                let orig = &o.mem.ram[tbase + tn * tsize..tbase + (tn + 1) * tsize];
                let addr_of = |x: &[u8]| u32::from_be_bytes([0, x[3], x[4], x[5]]);
                if orig[2] == t[2] && addr_of(orig).wrapping_sub(ROM_AT) != addr_of(t) && orig[0] & 0x80 != 0 {
                    return Err(format!("frame {fr}: track {tn} base address {:#x}, the port's {:#x}", addr_of(orig), addr_of(t)));
                }
                if let Some(k) = (0..tsize).find(|k| (!(3..6).contains(k)) && orig[*k] != t[*k]) {
                    let mut names: Vec<_> = o.sym.iter().filter(|(n, v)| n.starts_with("t_") && **v as usize <= k).collect();
                    names.sort_by_key(|x| -*x.1);
                    return Err(format!("frame {fr}: track {tn} differs at byte {k} ({}+{}): original {:02x?} port {:02x?}\n chip writes {}\n original track {:02x?}\n port     track {:02x?}\n rom at peg {:02x?}", names[0].0, k as i64 - names[0].1, &orig[k.saturating_sub(2)..(k + 3).min(tsize)], &t[k.saturating_sub(2)..(k + 3).min(tsize)], if a == b { "equal".to_string() } else { format!("differ: orig {:?} port {:?}", a, b) }, orig, t, { let pg = u16::from_be_bytes([orig[30], orig[31]]) as usize & 0x7fff; &song.rom()[8 + pg.saturating_sub(4)..8 + pg + 16] }));
                }
            }
            if a != b {
                let i = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                return Err(format!("frame {fr}: write {i} differs\n original {:?}\n port     {:?}", &a[i.saturating_sub(14)..(i + 4).min(a.len())], &b[i.saturating_sub(14)..(i + 4).min(b.len())]));
            }
        }
        Ok(total)
    }

    /// Something asked of the driver at a frame: a request or a command of `mds_command`.
    #[derive(Clone, Copy, Debug)]
    enum Ask {
        Request(u16, u16),
        Command(u8, u32, u32),
    }

    impl Original {
        fn ask(&mut self, a: Ask) -> Result<u32, String> {
            match a {
                Ask::Request(id, p) => self.request(id, p).map(|_| 0),
                Ask::Command(c, d1, d2) => {
                    self.cpu.a[0] = WORK;
                    self.cpu.d[0] = u32::from(c);
                    self.cpu.d[1] = d1;
                    self.cpu.d[2] = d2;
                    let at = self.addr("mds_command")?;
                    self.cpu.call(&mut self.mem, at, 100_000)?;
                    Ok(self.cpu.d[0] & 0xffff)
                }
            }
        }
    }

    /// Like `compare`, with things asked at given frames, and the work area before the tracks compared too.
    fn compare_asking(dir: &std::path::Path, f: &[u8], frames: usize, asks: &[(usize, Ask)]) -> Result<usize, String> {
        use crate::mdsdrv::{Driver, Song};
        let song = Song::parse(f).map_err(|e| e.to_string())?;
        let mut o = Original::new(dir, song.rom())?;
        o.mem.log.clear();
        let mut d = Driver::new(&song);
        d.chip_log = Some(vec![]);
        // The port starts song 1 at once; the original is asked the same, through whatever the first ask says.
        let first = asks.iter().find(|a| a.0 == 0).map(|a| a.1);
        match first {
            Some(Ask::Request(id, p)) => {
                o.request(id, p)?;
                d.request_song(id, usize::from(p));
            }
            _ => {
                o.request(1, 3)?;
            }
        }
        let w_req = o.sym["w_request"] as usize;
        let img_len = d.work_image().len();
        let mut total = 0;
        for fr in 0..frames {
            for (at, a) in asks {
                if *at == fr && !(fr == 0 && matches!(a, Ask::Request(..))) {
                    let r = o.ask(*a)?;
                    let q = match *a {
                        Ask::Request(id, p) => {
                            d.request_song(id, usize::from(p));
                            0
                        }
                        Ask::Command(c, d1, d2) => d.command(c, d1, d2),
                    };
                    let comparable = matches!(*a, Ask::Command(c, ..) if matches!(c, 0x00 | 0x01 | 0x02 | 0x04 | 0x06 | 0x0c | 0x0e | 0x10));
                    if comparable && r != q & 0xffff {
                        return Err(format!("frame {fr}: {a:?} gives {r:#x} in the original and {q:#x} in the port"));
                    }
                }
            }
            o.update()?;
            d.update();
            let a: Vec<_> = std::mem::take(&mut o.mem.log);
            let b: Vec<_> = d.chip_log.as_mut().map(std::mem::take).unwrap_or_default();
            total += a.len();
            let img = d.work_image();
            let orig = &o.mem.ram[w_req..w_req + img_len];
            if let Some(k) = (0..img_len).find(|k| orig[*k] != img[*k]) {
                return Err(format!("frame {fr}: work area byte {k} differs after {asks:?}: original {:02x?} port {:02x?}", &orig[..], &img[..]));
            }
            if a != b {
                let i = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                return Err(format!("frame {fr}: chip write {i} differs\n original {:?}\n port     {:?}", &a[i.saturating_sub(6)..(i + 4).min(a.len())], &b[i.saturating_sub(6)..(i + 4).min(b.len())]));
            }
        }
        Ok(total)
    }

    #[test]
    fn requests_and_commands_do_what_the_original_does() {
        let (Some(dir), Ok(songs)) = (src(), std::env::var("MDS_DIR")) else { return };
        let scenarios: Vec<(&str, Vec<(usize, Ask)>)> = vec![
            ("volume", vec![(30, Ask::Command(0x0d, 3, 20)), (200, Ask::Command(0x0d, 3, 0))]),
            ("fade out and stop", vec![(60, Ask::Command(0x0a, 0x0500 | 0x80 | 60, 0))]),
            ("fade to a level", vec![(60, Ask::Command(0x0a, 0x0300 | 40, 0)), (400, Ask::Command(0x0a, 0x0700 | 5, 0))]),
            ("pause and resume", vec![(40, Ask::Command(0x0b, 3, 1)), (200, Ask::Command(0x0b, 3, 0))]),
            ("tempo", vec![(30, Ask::Command(0x0f, 3, 90)), (300, Ask::Command(0x05, 100, 0))]),
            ("fade in", vec![(0, Ask::Request(1 | 0x2000 | (6 << 10), 3))]),
            ("slow fade in", vec![(0, Ask::Request(1 | 0x2000 | (1 << 10), 3))]),
            ("volume settings", vec![(0, Ask::Command(0x07, 0x0a03, 0)), (1, Ask::Request(1, 3))]),
            ("the song over itself as an effect", vec![(80, Ask::Request(1, 0)), (400, Ask::Request(0, 0))]),
            ("restarted", vec![(100, Ask::Request(1, 3))]),
            ("asking", vec![(10, Ask::Command(0x00, 0, 0)), (11, Ask::Command(0x01, 0, 0)), (12, Ask::Command(0x02, 3, 0)), (13, Ask::Command(0x04, 0, 0)), (14, Ask::Command(0x0c, 3, 0)), (15, Ask::Command(0x0e, 3, 0)), (16, Ask::Command(0x06, 0, 0)), (17, Ask::Command(0x10, 0, 0))]),
        ];
        let mut failed = vec![];
        for e in std::fs::read_dir(songs).unwrap().flatten() {
            let p = e.path();
            if !p.extension().is_some_and(|x| x == "mds") {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&p).unwrap();
            for (what, asks) in &scenarios {
                match compare_asking(&dir, &bytes, 900, asks) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{name} / {what}: {e}");
                        failed.push(format!("{name} / {what}"));
                    }
                }
            }
        }
        assert!(failed.is_empty(), "{} failed: {failed:?}", failed.len());
    }

    #[test]
    fn the_port_writes_what_the_original_writes() {
        let (Some(dir), Ok(songs)) = (src(), std::env::var("MDS_DIR")) else { return };
        let frames: usize = std::env::var("MDS_FRAMES").ok().and_then(|x| x.parse().ok()).unwrap_or(600);
        let mut failed = vec![];
        for e in std::fs::read_dir(songs).unwrap().flatten() {
            let p = e.path();
            if !p.extension().is_some_and(|x| x == "mds") {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            match compare(&dir, &std::fs::read(&p).unwrap(), frames) {
                Ok(n) => eprintln!("{name}: {n} writes, all the same"),
                Err(e) => {
                    eprintln!("{name}: {e}");
                    failed.push(name);
                }
            }
        }
        assert!(failed.is_empty(), "{failed:?}");
    }
}
