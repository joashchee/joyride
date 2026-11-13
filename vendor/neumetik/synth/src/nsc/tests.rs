// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

use super::*;

/// The music data of nsc's `-A` output (the sequence segment's `.byte` and `.word`s), and its DPCM segment.
pub(super) fn asm_bytes(s: &str) -> (Vec<u8>, Vec<u8>) {
    let (mut seq, mut pcm) = (Vec::new(), Vec::new());
    let mut in_pcm = false;
    for line in s.lines() {
        let l = line.split(';').next().unwrap_or("").trim().to_string();
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        let l = l.as_str();
        // `.align $40` (before each DPCM sample) pads as the assembler would.
        if let Some(a) = l.strip_prefix(".align") {
            let n = usize::from_str_radix(a.trim().trim_start_matches('$'), 16).unwrap();
            let out = if in_pcm { &mut pcm } else { &mut seq };
            out.resize(out.len().div_ceil(n) * n, 0);
            continue;
        }
        if let Some(seg) = l.strip_prefix(".segment") {
            in_pcm = seg.trim().trim_matches('"') == "PCMDATA";
            continue;
        }
        let (word, rest) = if let Some(r) = l.strip_prefix(".byte") {
            (false, r)
        } else if let Some(r) = l.strip_prefix(".word") {
            (true, r)
        } else {
            continue;
        };
        let out = if in_pcm { &mut pcm } else { &mut seq };
        for v in rest.split(',') {
            let v = v.trim();
            let n = if let Some(h) = v.strip_prefix('$') {
                u32::from_str_radix(h, 16).unwrap()
            } else {
                v.parse::<u32>().unwrap()
            };
            out.push(n as u8);
            if word {
                out.push((n >> 8) as u8);
            }
        }
    }
    // The last sample's padding, which the binary layout has too.
    pcm.resize(pcm.len().div_ceil(64) * 64, 0);
    (seq, pcm)
}

#[test]
fn a_scale() {
    let o = compile(b"#Title \"t\"\r\nBGM(0) {\r\n  t120 o4 l8 cdefgab<c\r\n}\r\n").unwrap();
    // As nsc 1.31 writes it (its -A output for the same file).
    assert_eq!(o.music, [0x01, 0x00, 0x04, 0x00, 0x08, 0x78, 0x48, 0x80, 0x82, 0x84, 0x85, 0x87, 0x89, 0x8b, 0x28, 0x80, 0x00]);
    assert_eq!(o.title, b"t");
}

// Every X.mml in NSC_REF beside nsc's X.s (`synth/scripts/nsc-reference.sh`): the same music data, or both stop.
// NSC_REF=dir cargo test --release --lib nsc::tests::against_nsc -- --ignored --nocapture (NSC_ONLY=name)
#[test]
#[ignore]
fn against_nsc() {
    let dir = std::path::PathBuf::from(std::env::var("NSC_REF").expect("NSC_REF"));
    let only = std::env::var("NSC_ONLY").ok();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mml")))
        .collect();
    names.sort();
    let (mut same, mut differ) = (0, 0);
    for p in names {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        if only.as_ref().is_some_and(|o| *o != name) {
            continue;
        }
        let src = std::fs::read(&p).unwrap();
        let base = p.parent().unwrap().to_path_buf();
        let reader = move |f: &str| std::fs::read(base.join(f)).ok();
        let ours = compile_with(&src, &reader);
        let theirs = std::fs::read(dir.join(format!("{name}.s"))).ok().map(|b| asm_bytes(&String::from_utf8_lossy(&b)));
        let ok = match (&ours, &theirs) {
            (Ok(o), Some((seq, pcm))) => o.music == *seq && o.dpcm == *pcm,
            (Err(_), None) => true,
            _ => false,
        };
        if ok {
            same += 1;
            continue;
        }
        differ += 1;
        match (&ours, &theirs) {
            (Ok(o), Some((seq, pcm))) => {
                let i = (0..o.music.len().max(seq.len())).find(|&i| o.music.get(i) != seq.get(i));
                match i {
                    Some(i) => println!(
                        "{name}: first difference at {i} of {}/{}: nsc {:02x?} / ours {:02x?}",
                        seq.len(),
                        o.music.len(),
                        &seq[i.min(seq.len())..(i + 12).min(seq.len())],
                        &o.music[i.min(o.music.len())..(i + 12).min(o.music.len())]
                    ),
                    None => println!("{name}: DPCM differs ({} / {} bytes)", pcm.len(), o.dpcm.len()),
                }
            }
            (Ok(_), None) => println!("{name}: nsc stops, ours compiles"),
            (Err(e), Some(_)) => println!("{name}: nsc compiles, ours: {e}"),
            _ => {}
        }
    }
    println!("{same} the same, {differ} differ");
    assert_eq!(differ, 0);
}

// 300 random sources (`synth/scripts/nsc-random.py`) run through nsc 1.31 itself (`reference/cases.tsv`: name, the
// source with `\n` for CR LF, and its music data in hex, or `ERR` where nsc stopped): ours the same.
#[test]
fn the_same_as_nsc() {
    let unescape = |s: &str| -> Vec<u8> {
        let mut o = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                o.push(c as u32 as u8);
                continue;
            }
            match it.next() {
                Some('n') => o.extend_from_slice(b"\r\n"),
                Some('N') => o.push(b'\n'),
                Some('t') => o.push(b'\t'),
                Some(x) => o.push(x as u32 as u8),
                None => {}
            }
        }
        o
    };
    let rows = include_str!("reference/cases.tsv");
    let mut bad = Vec::new();
    for row in rows.lines() {
        let mut f = row.split('\t');
        let (name, src, want) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
        let got = match compile(&unescape(src)) {
            Ok(o) => o.music.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            Err(_) => "ERR".to_string(),
        };
        if got != want {
            bad.push(name.to_string());
        }
    }
    assert!(rows.lines().count() >= 300);
    assert!(bad.is_empty(), "{} differ: {bad:?}", bad.len());
}

// Every X.mml in NSC_REF beside nsc's X.nsf and X.nsfe (`synth/scripts/nsd-reference.sh DIR`): our NSF and NSFe
// the same, byte for byte, metadata included.
// NSC_REF=dir cargo test --release --lib nsc::tests::links_like_nsc -- --ignored --nocapture
#[test]
#[ignore]
fn links_like_nsc() {
    let dir = std::path::PathBuf::from(std::env::var("NSC_REF").expect("NSC_REF"));
    let (mut same, mut differ) = (0, 0);
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mml")) || p.to_string_lossy().ends_with(".bank.mml") {
            continue;
        }
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read(&p).unwrap();
        let base = dir.clone();
        let reader = move |f: &str| std::fs::read(base.join(f)).ok();
        let out = compile_with(&src, &reader).map_err(|e| e.to_string());
        for (ext, link) in [("nsf", link::nsf as fn(&Output) -> Result<Vec<u8>, String>), ("nsfe", link::nsfe)] {
            let Ok(want) = std::fs::read(dir.join(format!("{name}.{ext}"))) else { continue };
            match out.clone().and_then(|o| link(&o)) {
                Ok(g) if g == want => same += 1,
                Ok(g) => {
                    differ += 1;
                    let i = (0..g.len().max(want.len())).find(|&i| g.get(i) != want.get(i)).unwrap();
                    println!("{name}.{ext}: differs at {i:#x} of {:#x}/{:#x}", want.len(), g.len());
                }
                Err(e) => {
                    differ += 1;
                    println!("{name}.{ext}: {e}");
                }
            }
        }
    }
    println!("{same} the same, {differ} differ");
    assert!(differ == 0 && same > 0);
}

#[test]
fn plays_through_the_driver() {
    let o = compile(b"#Title \"t\"\r\nBGM(0) {\r\n  TR1 t120 o4 l8 v15 @2 L cdefgab<c\r\n  TR3 o3 l4 c e g\r\n}\r\n").unwrap();
    let file = link::nsf(&o).unwrap();
    let song = crate::nsf::Song::parse(&file).unwrap();
    assert_eq!(song.nsf.title, "t");
    let mut p = crate::nsf::Player::new(song.nsf.clone(), 0, true);
    for _ in 0..240 {
        p.play();
    }
    let log = p.machine.log.as_ref().unwrap();
    let pulse: Vec<_> = log.iter().filter(|w| w.1 == 0x4002).map(|w| w.2).collect();
    let tri: Vec<_> = log.iter().filter(|w| w.1 == 0x400a).collect();
    // Pulse 1's period low byte changes with each note, the triangle plays, and the driver kept running.
    let mut distinct = pulse.clone();
    distinct.sort();
    distinct.dedup();
    assert!(distinct.len() >= 7, "{distinct:x?}");
    assert!(!tri.is_empty());
    assert!(log.iter().any(|w| w.0 > 200));
}

// Each X.nsf in NSC_REF played to its end or loop: frames, where it loops, notes. (A look, not a check.)
#[test]
#[ignore]
fn timelines() {
    let dir = std::path::PathBuf::from(std::env::var("NSC_REF").expect("NSC_REF"));
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "nsf"))
        .collect();
    names.sort();
    for p in names {
        let nsf = std::sync::Arc::new(crate::nsf::Nsf::parse(&std::fs::read(&p).unwrap()).unwrap());
        for song in 0..nsf.songs {
            let t = crate::nsf::timeline(&nsf, song, 60 * 60 * 10);
            let per: Vec<usize> = (0..t.chans.len()).map(|c| t.events.iter().filter(|e| e.1 == c && e.2.is_some()).count()).collect();
            println!(
                "{} {song}: {} frames ({:.1} s), restart {:?}, notes {per:?}",
                p.file_name().unwrap().to_string_lossy(),
                t.frames,
                f64::from(t.frames) * nsf.period,
                t.restart
            );
        }
    }
}

// Each expansion chip's own driver build: its first channels (tracks 6 on) play what they're given, through the
// chip's emulation, not silence.
#[test]
fn every_chip_sounds() {
    use crate::nsf::Chan;
    use crate::replay::Driver;
    let fds_wave: String = (0..64).map(|i| ((i * 63) / 63 % 64).to_string()).collect::<Vec<_>>().join(",");
    let cases: [(&str, &str, &[Chan]); 5] = [
        ("nsd_vrc6", "", &[Chan::Vrc6(0), Chan::Vrc6(1), Chan::Vrc6(2)]),
        ("nsd_mmc5", "", &[Chan::Mmc5(0), Chan::Mmc5(1)]),
        ("nsd_s5b", "", &[Chan::S5b(0), Chan::S5b(1), Chan::S5b(2)]),
        ("nsd_vrc7", "", &[Chan::Vrc7(0), Chan::Vrc7(1), Chan::Vrc7(2)]),
        ("nsd_fds", "FDSC(0){ WAVE }\r\n", &[Chan::Fds]),
    ];
    for (code, defs, chans) in cases {
        let tracks: String = (0..chans.len())
            .map(|k| {
                format!(
                    "  TR{} t150 v15 @{} {} o4 l4 c e g <c\r\n",
                    6 + k,
                    if code == "nsd_vrc7" { 3 } else { 1 },
                    if code == "nsd_fds" { "@FC0" } else { "" }
                )
            })
            .collect();
        let src = format!("#Code \"{code}.bin\"\r\n{}BGM(0) {{\r\n{tracks}}}\r\n", defs.replace("WAVE", &fds_wave));
        let o = compile(src.as_bytes()).unwrap_or_else(|e| panic!("{code}: {e}"));
        let file = link::nsf(&o).unwrap();
        let nsf = std::sync::Arc::new(crate::nsf::Nsf::parse(&file).unwrap());
        let mut p = crate::nsf::Player::new(nsf.clone(), 0, false);
        let n = p.chans().len();
        let mut peak = vec![0f32; n];
        let mut bufs = crate::replay::Bufs {
            left: vec![vec![0.0; 735]; n],
            right: vec![vec![0.0; 735]; n],
        };
        for _ in 0..90 {
            p.play();
            p.render(44100, 735, &mut bufs);
            for c in 0..n {
                peak[c] = bufs.left[c].iter().fold(peak[c], |a, v| a.max(v.abs()));
            }
        }
        for want in chans {
            let c = p.chans().iter().position(|x| x == want).unwrap_or_else(|| panic!("{code}: no {want:?}"));
            assert!(peak[c] > 0.01, "{code}: {want:?} is silent ({peaks:?})", peaks = peak);
        }
    }
}

// 13 sources full of metadata (`#Text`, `#Maker`, long names, `#plst`, `#psfx`, `#mixe`, `#VRC7`, each track's
// `#Label`, `#Author`, `#Time`, `#Fade`, a banked build) linked by nsc 1.31 itself with `-n` and with `-x`
// (`reference/meta.tsv`: name, the source with `\n` for CR LF, the NSF's flags byte and everything after its program,
// and the NSFe with its DATA chunk's bytes left out): ours the same.
#[test]
fn links_metadata_as_nsc() {
    let unescape = |s: &str| -> Vec<u8> {
        let mut o = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                o.push(c as u32 as u8);
                continue;
            }
            match it.next() {
                Some('n') => o.extend_from_slice(b"\r\n"),
                Some('t') => o.push(b'\t'),
                Some(x) => o.push(x as u32 as u8),
                None => {}
            }
        }
        o
    };
    let hex = |b: &[u8]| b.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let rows = include_str!("reference/meta.tsv");
    for row in rows.lines() {
        let f: Vec<&str> = row.split('\t').collect();
        let o = compile(&unescape(f[1])).unwrap();
        let nsf = link::nsf(&o).unwrap();
        let size = usize::from(nsf[0x7d]) | usize::from(nsf[0x7e]) << 8 | usize::from(nsf[0x7f]) << 16;
        assert_eq!(format!("{:02x}{}", nsf[0x7c], hex(&nsf[0x80 + size..])), f[2], "{} (NSF)", f[0]);
        let e = link::nsfe(&o).unwrap();
        let mut cut = e[..4].to_vec();
        let mut i = 4;
        while i < e.len() {
            let len = u32::from_le_bytes(e[i..i + 4].try_into().unwrap()) as usize;
            cut.extend_from_slice(&e[i..i + 8]);
            if &e[i + 4..i + 8] != b"DATA" {
                cut.extend_from_slice(&e[i + 8..i + 8 + len]);
            }
            i += 8 + len;
        }
        assert_eq!(hex(&cut), f[3], "{} (NSFe)", f[0]);
        // Our reader takes back what was written.
        let back = crate::nsf::Nsf::parse(&e).unwrap();
        assert_eq!(back.image().len() - usize::from(back.load & 0x0fff) * usize::from(back.banked), size, "{}", f[0]);
        let meta = crate::nsf::Nsf::parse(&nsf).unwrap();
        assert_eq!((meta.title.as_str(), meta.songs), (back.title.as_str(), back.songs), "{}", f[0]);
    }
    assert_eq!(rows.lines().count(), 13);
}

// DPCM mode 2 (a `#Bank` build): a sample's end raises the DMC's IRQ, and NSD.Lib's handler starts the sample its
// note names next. a.dmc (all ones, the level climbs) chains to b.dmc (all zeros, it falls); TR5 plays a.dmc once.
// Without the IRQ only a.dmc plays.
#[test]
fn a_dpcm_sample_chains_to_the_next_by_irq() {
    use crate::replay::{Bufs, Driver};
    let src = b"#Bank\r\n#Code \"nsd_all.bin\"\r\nDPCM{\r\n c4, \"a.dmc\", 15, 2, 0, 38\r\n d4, \"b.dmc\", 15, 0\r\n}\r\nBGM(0) {\r\n TR5 o4 l1 c r r r\r\n}\r\n";
    let reader = |f: &str| match f {
        "a.dmc" => Some(vec![0xff; 0x41]),
        "b.dmc" => Some(vec![0x00; 0x41]),
        _ => None,
    };
    let o = compile_with(src, &reader).unwrap();
    let file = link::nsf(&o).unwrap();
    let nsf = std::sync::Arc::new(crate::nsf::Nsf::parse(&file).unwrap());
    let mut p = crate::nsf::Player::new(nsf, 0, true);
    let rate = 44_100;
    let mut dmc = Vec::new();
    let ch = p.channels();
    let mut bufs = Bufs { left: vec![vec![0.0; 2048]; ch], right: vec![vec![0.0; 2048]; ch] };
    for _ in 0..30 {
        p.tick();
        let n = (p.tick_seconds() * f64::from(rate)).round() as usize;
        p.render(rate, n, &mut bufs);
        dmc.extend_from_slice(&bufs.left[4][..n]);
    }
    let log = p.machine.log.as_ref().unwrap();
    // The sample's length written twice once playing: by the play routine, then by the IRQ handler.
    let starts: Vec<u32> = log.iter().filter(|w| w.1 == 0x4013 && w.0 > 0).map(|w| w.0).collect();
    assert_eq!(starts.len(), 2, "the IRQ started the next sample: {starts:?}");
    // The level climbs (to its top within a.dmc's first 8 bytes), then falls the moment a.dmc's 65 bytes end:
    // 65 * 8 bits of 54 cycles, 28,080 cycles, 692 samples at 44,100 Hz. (The DC filter's own decay is far slower
    // than a falling DMC's steps.)
    let d: Vec<f32> = dmc.windows(2).map(|w| w[1] - w[0]).collect();
    let rise = d.iter().position(|&x| x > 0.004).unwrap();
    let fall = d.iter().position(|&x| x < -0.004).expect("b.dmc falls");
    assert!((fall - rise).abs_diff(692) <= 12, "b.dmc follows a.dmc with no gap: rise at {rise}, fall at {fall}");
}

// Every X.mml under the folders in NSC_DEC (`:`-separated) that nsc compiles and links: the NSF decompiled, and the
// MML compiled and linked back to the same NSF. A count, with the first reasons.
// NSC_DEC=dir:dir cargo test --release --lib nsc::tests::decompiles -- --ignored --nocapture
#[test]
#[ignore]
fn decompiles() {
    if let Ok(f) = std::env::var("NSC_DUMP") {
        let p = std::path::PathBuf::from(&f);
        let base = p.parent().unwrap().to_path_buf();
        let reader = move |f: &str| std::fs::read(base.join(f)).ok();
        let o = compile_with(&std::fs::read(&p).unwrap(), &reader).unwrap();
        let nsf = link::nsf(&o).unwrap();
        let dec = decompile::write(&nsf);
        let mml = dec.as_ref().map(|d| d.mml.clone());
        if std::env::var("FULL").is_ok() {
            println!("music {}", o.music.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "));
            println!("bgm {:x?} se {:x?}", o.bgm, o.se);
        }
        println!("{}", mml.as_ref().map_or("no MML".into(), |x| String::from_utf8_lossy(x).to_string()));
        let files = dec.map(|d| d.files).unwrap_or_default();
        let reader2 = move |f: &str| files.iter().find(|(n, _)| n == f).map(|(_, b)| b.clone());
        if let Some(Ok(o2)) = mml.map(|x| compile_with(&x, &reader2)) {
            let (a, b) = (&o.music, &o2.music);
            if std::env::var("FULL").is_ok() {
                let hex = |x: &[u8]| x.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ");
                println!("want {}\ngot  {}", hex(a), hex(b));
            }
            if let Some(k) = (0..a.len().max(b.len())).find(|&k| a.get(k) != b.get(k)) {
                let hex = |x: &[u8]| x.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ");
                let from = k.saturating_sub(16);
                println!("music at {k}: want {}\n                 got  {}", hex(&a[from..(k + 16).min(a.len())]), hex(&b[from..(k + 16).min(b.len())]));
            }
        }
        println!("{}", decompile::why(&nsf));
        return;
    }
    let (mut tried, mut same, mut bad) = (0, 0, Vec::new());
    for d in std::env::var("NSC_DEC").expect("NSC_DEC").split(':') {
        let dir = std::path::PathBuf::from(d);
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mml"))).collect();
        names.sort();
        for p in names {
            let src = std::fs::read(&p).unwrap();
            let base = dir.clone();
            let reader = move |f: &str| std::fs::read(base.join(f)).ok();
            let Ok(o) = compile_with(&src, &reader) else { continue };
            let Ok(nsf) = link::nsf(&o) else { continue };
            tried += 1;
            if decompile(&nsf).is_some() {
                same += 1;
            } else {
                bad.push(format!("{}: {}", p.display(), decompile::why(&nsf)));
            }
        }
    }
    for b in bad.iter().take(std::env::var("SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(20)) {
        println!("{b}");
    }
    println!("{same} of {tried} come back");
}

// The 300 random sources of `reference/cases.tsv` that nsc compiles: linked, decompiled, and compiled and linked
// back to the same NSF (all but a handful of shapes `decompile` leaves out).
#[test]
fn random_sources_decompile() {
    let unescape = |s: &str| -> Vec<u8> {
        let mut o = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                o.push(c as u32 as u8);
                continue;
            }
            match it.next() {
                Some('n') => o.extend_from_slice(b"\r\n"),
                Some('N') => o.push(b'\n'),
                Some('t') => o.push(b'\t'),
                Some(x) => o.push(x as u32 as u8),
                None => {}
            }
        }
        o
    };
    let (mut tried, mut back) = (0, 0);
    for row in include_str!("reference/cases.tsv").lines() {
        let src = unescape(row.split('\t').nth(1).unwrap());
        let Ok(o) = compile(&src) else { continue };
        let Ok(nsf) = link::nsf(&o) else { continue };
        tried += 1;
        if let Some(d) = decompile(&nsf) {
            back += 1;
            assert!(d.files.is_empty());
        }
    }
    assert!(tried >= 190, "{tried}");
    assert!(back * 100 >= tried * 98, "{back} of {tried}");
}

// Every kind of command, block and definition, DPCM samples (shared by notes) and metadata: back the same.
#[test]
fn every_command_decompiles() {
    let src = "#Title \"A song with a long name of more than thirty-two bytes\"\r\n#Composer \"me\"\r\n#Maker \"m\"\r\n#Text \"one\"\r\n\
#BGM 2\r\n#SE 1\r\n#OffsetPCM $C000\r\n#plst {1,0}\r\n\
E(0){ 1 2 3 | 4 5 R 6 | 7 }\r\nE(1){ 15 D20 3 3 (15,0,10) }\r\nE(2){ | 9 R R 8 }\r\n\
FDSC(0){ 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63 }\r\n\
FDSM(0){ 0,1,2,3,4,5,6,7,0,1,2,3,4,5,6,7,0,1,2,3,4,5,6,7,0,1,2,3,4,5,6,7 }\r\n\
VRC7(0){ @R 1,2,3,4,5,6,7,8 }\r\nN163(0){ 0,1,2,3,4,5,6,7 }\r\n\
DPCM{ c4, \"a.dmc\", 15, 0, 10\r\n d4, \"a.dmc\", 14, 0\r\n e4, \"b.dmc\", 13, 0, -1 }\r\n\
Sub(0){ c d e }\r\n\
BGM(0){\r\n #Title \"first\"\r\n #Time 3000\r\n\
 TR1 t150 l8 o4 v12 q3 u%4 Rm1 R@2 Rv5 c d%7 e,2 f,0 g& a r r- r# r* L c > d < e ` f \" g\r\n\
 TR2 [2 c : d ] |: e \\ f :| S0 SE0 E@0 Ev1 Em2 En0 Ev* Em* En* D5 D%3 D_2 D%_1 s3 P1,2,3,4 _2 __1 t_3 ( ) j1 j0\r\n\
 TR3 @3 @V0 yV1,2 y4096,7 @FC0 @FM0 @FF100 @FV2 F300 @N1,0 NC4\r\n\
 TR6 o3 c d e\r\n}\r\n\
BGM(1){ TR1 c }\r\nSE(0){ #Priority 2\r\n TR1 v15 c }\r\n";
    let files = |f: &str| match f {
        "a.dmc" => Some((0..33).map(|i| (i * 7) as u8).collect()),
        "b.dmc" => Some((0..65).map(|i| (i * 3) as u8).collect()),
        _ => None,
    };
    let o = compile_with(src.as_bytes(), &files).unwrap();
    let nsf = link::nsf(&o).unwrap();
    let d = decompile(&nsf).unwrap_or_else(|| panic!("{}", decompile::why(&nsf)));
    assert_eq!(d.files.len(), 2);
    assert!(String::from_utf8_lossy(&d.mml).contains("BGM(1)"));
}
