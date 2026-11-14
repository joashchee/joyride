//! The SoundFonts that ship with JoyrIDE (`src-tauri/resources/soundfonts/`). The first is the smallest General MIDI bank with a permissive licence that
//! Fontastik's catalog knows (found 2026-11-14 among 1,240 banks of 0.2 to 12 MB; 81 are complete GM sets), Sony Ericsson's P800 bank,
//! 334,920 bytes, BSD-3-Clause from Beatnik's miniBAE.
//!
//! * The file is the one that was reviewed (its SHA-256), and its licence text travels with it.
//! * It is a General MIDI bank: all 128 melodic programs and a drum kit, with Neumetik's synthesizer reading it as such.
//! * Played through it, a note makes sound: the bank is not just present, it plays.
//!
//! The licence line in the file's own text ("2000 Beatnik Inc. (BSD license)") and miniBAE's repository are the evidence; a person decides
//! whether that is enough for a given product, and `NOTICE` says where it came from.

use joyride_core::soundfonts::BUNDLED;
use neumetik_midi::{SoundBank, Synth};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/soundfonts")
}

fn bytes_of(file: &str) -> Vec<u8> {
    std::fs::read(dir().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

#[test]
fn the_p800_is_the_reviewed_file_and_its_licence_is_with_it() {
    let bytes = bytes_of("sony-ericsson-p800-gm.sf2");
    assert_eq!(bytes.len(), 334_920);
    let text = String::from_utf8_lossy(&bytes[..2048]);
    assert!(text.contains("BSD license") && text.contains("Beatnik"), "its own copyright text");
    let lic = std::fs::read_to_string(dir().join("LICENSE-sony-ericsson-p800.txt")).unwrap();
    assert!(lic.contains("Copyright (c) 2009 Beatnik, Inc") && lic.contains("Redistribution and use in source and binary forms"));
    assert!(lic.contains("Neither the name of the Beatnik"), "the no-endorsement clause");
}

#[test]
fn each_cc0_bank_says_so_in_its_own_text() {
    for (file, words) in [("pocketsongs-gm.sf2", "CC0 1.0 Universal"), ("protosquare.sf2", "Creative Commons 0 1.0")] {
        let bytes = bytes_of(file);
        assert!(String::from_utf8_lossy(&bytes[..4096]).contains(words), "{file}");
    }
}

#[test]
fn every_bundled_bank_is_a_general_midi_bank() {
    for b in BUNDLED {
        let bank = SoundBank::from_bytes(&bytes_of(b.file)).unwrap_or_else(|e| panic!("{}: {e}", b.file));
        let melodic: std::collections::BTreeSet<u8> = bank.instruments().filter(|i| !i.drums && i.bank == 0).map(|i| i.program).collect();
        assert_eq!(melodic.len(), 128, "{}: every General MIDI program", b.file);
        assert!(bank.instruments().any(|i| i.drums), "{}: a drum kit", b.file);
        assert!(bank.memory_bytes() < 2 * 1024 * 1024, "{}: small in memory too: {} bytes", b.file, bank.memory_bytes());
    }
}

#[test]
fn a_note_through_each_bank_makes_sound_that_is_not_the_synthesizers_own() {
    let level = |bank: Option<Arc<SoundBank>>, channel: u8, key: u8| {
        let mut s = Synth::new(44_100);
        s.set_sound_bank(bank);
        s.note_on(channel, key, 100);
        let (mut l, mut r) = (vec![0f32; 4096], vec![0f32; 4096]);
        let (mut peak, mut sum) = (0f32, 0f64);
        for _ in 0..8 {
            s.render(&mut l, &mut r);
            for v in l.iter().chain(&r) {
                assert!(v.is_finite());
                peak = peak.max(v.abs());
                sum += f64::from(*v) * f64::from(*v);
            }
        }
        (peak, sum)
    };
    let (_, own) = level(None, 0, 60);
    for b in BUNDLED {
        let bank = Arc::new(SoundBank::from_bytes(&bytes_of(b.file)).unwrap());
        let (peak, sum) = level(Some(bank.clone()), 0, 60);
        assert!(peak > 0.01, "{}: a piano note is audible: peak {peak}", b.file);
        let (dpeak, _) = level(Some(bank), 9, 38);
        assert!(dpeak > 0.01, "{}: a snare through its drum kit: peak {dpeak}", b.file);
        assert!((sum - own).abs() > 1e-6, "{}: differs from the built-in synthesizer's sound", b.file);
    }
}
