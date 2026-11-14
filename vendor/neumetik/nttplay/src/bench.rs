// Copyright 2026 ansiapps. Neumetik for the R36S: proprietary, see the LICENSE at the repository root.

//! The speed test: how many times faster than real time each engine plays,
//! and how long a Visual frame takes, on the machine it runs on. Only a
//! run on the R36S itself says whether the handheld keeps up (its
//! Cortex-A35 is far slower than any laptop); the host's device report
//! (`neumetik --report`, Start > Device report) puts these lines in
//! `/roms/Neumetik/r36s-report.txt`. Every line is in the report's
//! grammar (r3i6nos `docs/DEVICE-REPORTS.md`): `STATUS key: value`.

use std::path::Path;
use std::time::Instant;

use crate::{gen, Engine, Song, BLOCK};
#[cfg(feature = "visual")]
use crate::Vis;

/// The plan's bar: an engine must play at least this many times faster
/// than real time, leaving the rest of the CPU to drawing and the system.
pub const GOAL: f64 = 2.0;

/// A name as a report key: no colon (it ends the key), no line breaks.
fn key(s: &str) -> String {
    s.replace([':', '\n', '\r', '\t'], " ")
}

/// Seconds of `song` played as fast as it goes; the times real time.
fn time_song(song: Song, rate: u32, seconds: f64) -> (f64, f64) {
    let mut e = Engine::new(rate);
    e.set_volume(1.0);
    e.play(Some(Box::new(song)));
    let frames = (seconds * f64::from(rate)) as usize;
    let mut buf = vec![0f32; BLOCK * 2];
    let t = Instant::now();
    let mut done = 0;
    while done < frames {
        e.render(&mut buf);
        done += BLOCK;
    }
    let took = t.elapsed().as_secs_f64().max(1e-9);
    (took, seconds / took)
}

fn line_for(what: &str, name: &str, seconds: f64, took: f64, x: f64) -> String {
    let status = if x >= GOAL { "PASS" } else { "FAIL" };
    format!("{status} {what} {}: {x:.1}x real time (goal {GOAL}x; {seconds:.1} s in {took:.3} s)", key(name))
}

/// Visual's slowest kind must leave most of a 30 fps frame (33 ms) to the rest.
pub const VISUAL_GOAL_MS: f64 = 8.0;

/// Runs every test, a line at a time: the generated MIDI song and modules,
/// three BGN scenes, every Visual kind at the host's 64 x 20, then up to 20
/// of the user's files in `dir` (20 seconds each at most).
pub fn run(dir: Option<&Path>, rate: u32, seconds: f64, out: &mut dyn FnMut(&str)) {
    let seconds = seconds.clamp(1.0, 600.0);
    out(&format!("INFO speed test: neumetik-r36s {}, {rate} Hz stereo, {seconds:.0} s of each, goal {GOAL}x real time", env!("CARGO_PKG_VERSION")));
    let mut slow = 0;
    let mut tally = |l: String, x: f64, out: &mut dyn FnMut(&str)| {
        if x < GOAL {
            slow += 1;
        }
        out(&l);
    };

    let midi = gen::midi_song(seconds.ceil() as u32 + 2);
    match Song::midi(&midi, "generated", "MIDI", rate) {
        Ok(s) => {
            let (took, x) = time_song(s, rate, seconds);
            tally(line_for("midi", "generated GM song, 7 parts", seconds, took, x), x, out);
        }
        Err(e) => out(&format!("FAIL midi generated GM song: {}", key(&e))),
    }
    for ch in [4, 8] {
        match neumetik_tracker::load(&gen::module(ch, 64), "bench.mod") {
            Ok(m) => {
                let (took, x) = time_song(Song::module(m, "generated", rate), rate, seconds);
                tally(line_for("module", &format!("generated ProTracker, {ch} channels"), seconds, took, x), x, out);
            }
            Err(e) => out(&format!("FAIL module generated {ch} channels: {}", key(&e.to_string()))),
        }
    }
    #[cfg(feature = "bgn")]
    {
        let bank = neumetik_bgn::BANK;
        let mut scenes = vec![0usize];
        for word in ["rain", "crowd", "storm"] {
            if let Some(i) = bank.iter().position(|p| p.id.contains(word)) {
                if !scenes.contains(&i) {
                    scenes.push(i);
                }
            }
        }
        for i in scenes {
            if let Some(s) = Song::bgn(i, rate) {
                let (took, x) = time_song(s, rate, seconds);
                tally(line_for("bgn", bank[i].name, seconds, took, x), x, out);
            }
        }

    }

    #[cfg(feature = "visual")]
    {
        // Visual, drawing what the MIDI song plays: 60 frames of each kind at 30 a second.
        let mut e = Engine::new(rate);
        if let Ok(s) = Song::midi(&midi, "generated", "MIDI", rate) {
            e.play(Some(Box::new(s)));
        }
        let per_frame = (rate / 30) as usize;
        let mut buf = vec![0f32; per_frame * 2];
        let mut worst = (0.0f64, "");
        for (k, kind) in neumetik_visual::visual::KINDS.iter().enumerate() {
            let mut v = Vis::new(k);
            let mut took = 0.0;
            let frames = 60;
            for _ in 0..frames {
                e.render(&mut buf);
                v.capture(&e);
                let t = Instant::now();
                v.draw(1.0 / 30.0, 64, 20);
                took += t.elapsed().as_secs_f64();
            }
            let ms = took * 1000.0 / f64::from(frames);
            if ms > worst.0 {
                worst = (ms, kind.name);
            }
            out(&format!("INFO visual {}: {ms:.3} ms a frame (64 x 20)", key(kind.name)));
        }
        let status = if worst.0 <= VISUAL_GOAL_MS { "PASS" } else { "FAIL" };
        out(&format!("{status} visual slowest: {} at {:.3} ms a frame (goal {VISUAL_GOAL_MS} ms of a 33 ms frame at 30 fps)", key(worst.1), worst.0));

    }

    if let Some(dir) = dir {
        let mut files: Vec<_> = std::fs::read_dir(dir).map(|r| r.filter_map(Result::ok).map(|d| d.path()).filter(|p| p.is_file()).collect()).unwrap_or_default();
        files.sort();
        for path in files.into_iter().take(20) {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with('.') || name.ends_with(".txt") {
                continue;
            }
            match std::panic::catch_unwind(|| Song::open(&path, rate)) {
                Ok(Ok(s)) => {
                    let what = if s.kind() == crate::NM36_MODULE { "module" } else { "midi" };
                    let secs = if s.duration() > 0.0 { seconds.min(20.0).min(s.duration().max(1.0)) } else { seconds.min(20.0) };
                    let label = format!("{name} ({})", s.format());
                    let (took, x) = time_song(s, rate, secs);
                    tally(line_for(what, &label, secs, took, x), x, out);
                }
                Ok(Err(e)) => out(&format!("INFO file {}: cannot read it here ({})", key(&name), key(&e))),
                Err(_) => out(&format!("FAIL file {}: the reader failed (a panic)", key(&name))),
            }
        }
    }
    out(&format!("INFO speed summary: {slow} below the goal"));
}
