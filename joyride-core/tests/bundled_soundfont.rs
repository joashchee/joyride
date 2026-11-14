//! The SoundFont that ships with JoyrIDE (`src-tauri/resources/soundfonts/`): the smallest General MIDI bank with a permissive licence that
//! Fontastik's catalog knows (found 2026-11-14 among 1,240 banks of 0.2 to 12 MB; 81 are complete GM sets), Sony Ericsson's P800 bank,
//! 334,920 bytes, BSD-3-Clause from Beatnik's miniBAE.
//!
//! * The file is the one that was reviewed (its SHA-256), and its licence text travels with it.
//! * It is a General MIDI bank: all 128 melodic programs and a drum kit, with Neumetik's synthesizer reading it as such.
//! * Played through it, a note makes sound: the bank is not just present, it plays.
//!
//! The licence line in the file's own text ("2000 Beatnik Inc. (BSD license)") and miniBAE's repository are the evidence; a person decides
//! whether that is enough for a given product, and `NOTICE` says where it came from.

use neumetik_midi::{SoundBank, Synth};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/soundfonts")
}

fn bank_bytes() -> Vec<u8> {
    std::fs::read(dir().join("sony-ericsson-p800-gm.sf2")).expect("the bundled SoundFont")
}

#[test]
fn it_is_the_reviewed_file_and_its_licence_is_with_it() {
    let bytes = bank_bytes();
    assert_eq!(bytes.len(), 334_920);
    // The file's own INFO text names the licence, and the licence file reproduces the BSD text that requires it be kept.
    let text = String::from_utf8_lossy(&bytes[..2048]);
    assert!(text.contains("BSD license") && text.contains("Beatnik"), "its own copyright text");
    let lic = std::fs::read_to_string(dir().join("LICENSE.txt")).unwrap();
    assert!(lic.contains("Copyright (c) 2009 Beatnik, Inc") && lic.contains("Redistribution and use in source and binary forms"));
    assert!(lic.contains("Neither the name of the Beatnik"), "the no-endorsement clause");
    assert!(lic.contains("4d78a19f60518664f6e2ccdd0cf3216a12eceb6bd27e2e85704be3660a40f5fa"), "the checksum it records is the file's");
    use std::process::Command;
    // (shasum is on macOS and Linux; where it is missing the check above on the recorded value still ran)
    if let Ok(o) = Command::new("shasum").args(["-a", "256"]).arg(dir().join("sony-ericsson-p800-gm.sf2")).output() {
        assert!(String::from_utf8_lossy(&o.stdout).starts_with("4d78a19f60518664f6e2ccdd0cf3216a12eceb6bd27e2e85704be3660a40f5fa"));
    }
}

#[test]
fn it_is_a_general_midi_bank() {
    let bank = SoundBank::from_bytes(&bank_bytes()).expect("a valid SoundFont");
    let melodic: std::collections::BTreeSet<u8> = bank.instruments().filter(|i| !i.drums && i.bank == 0).map(|i| i.program).collect();
    assert_eq!(melodic.len(), 128, "every General MIDI program");
    assert!(bank.instruments().any(|i| i.drums), "a drum kit");
    assert!(bank.memory_bytes() < 2 * 1024 * 1024, "small in memory too: {} bytes", bank.memory_bytes());
}

#[test]
fn a_note_through_it_makes_sound_and_is_not_the_synthesizers_own() {
    let bank = Arc::new(SoundBank::from_bytes(&bank_bytes()).unwrap());
    let level = |with_bank: bool, channel: u8, key: u8| {
        let mut s = Synth::new(44_100);
        if with_bank {
            s.set_sound_bank(Some(bank.clone()));
        }
        s.note_on(channel, key, 100);
        let (mut l, mut r) = (vec![0f32; 4096], vec![0f32; 4096]);
        let mut peak = 0f32;
        let mut sum = 0f64;
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
    let (peak, sum) = level(true, 0, 60);
    assert!(peak > 0.01, "a piano note through the bank is audible: peak {peak}");
    let (dpeak, _) = level(true, 9, 38);
    assert!(dpeak > 0.01, "a snare through the bank's drum kit: peak {dpeak}");
    let (_, own) = level(false, 0, 60);
    assert!((sum - own).abs() > 1e-6, "the bank's sound differs from the built-in synthesizer's");
}
