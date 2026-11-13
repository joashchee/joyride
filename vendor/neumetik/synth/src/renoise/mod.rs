// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Renoise (Eduard Müller and Zvonko Tesic, 2002-): a tracker whose songs
//! (.xrns, a zip of the song as XML and its samples as FLAC) run each
//! track through a chain of effect devices into groups, send tracks and
//! the master.
//!
//! Written from Renoise's user manuals (the 1.5, 2.x and 3.4 editions:
//! its pattern commands, instruments, envelopes and devices) and from
//! what the songs' XML holds; no part of Renoise is used. Renoise's own
//! effects are closed, so each device is written here from what the
//! manual says it does, with the settings the song gives it: close to
//! Renoise, not the same. Plugin instruments and effects (VST, AU) are
//! silent.
//!
//! Renoise 1's own binary songs (.rns) are read by [`rns`] into the same
//! model.
//!
//! The host unzips a song into a bundle of its own (see [`bundle`]):
//! the song's XML and its sample files, which is what's kept as the
//! song's file.

pub(crate) mod audio;
mod dsp;
mod env;
mod filter;
mod params;
mod play;
pub mod rns;
pub mod song;
mod voice;

use std::sync::Arc;

pub use song::Song as Model;

/// The bundle's mark.
const MAGIC: &[u8; 8] = b"NMRNS01\0";

/// A bundle of a song's files (its Song.xml, its sample files), as the
/// host unzipped them: the mark, then each file's name and bytes, their
/// lengths first (u32, little-endian).
pub fn bundle(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for (name, data) in files {
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

/// A bundle's files.
pub fn unbundle(d: &[u8]) -> Option<Vec<(String, &[u8])>> {
    let mut at = MAGIC.len();
    if !d.starts_with(MAGIC) {
        return None;
    }
    let mut out = Vec::new();
    let u32_at = |i: usize| d.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    while at < d.len() {
        let n = u32_at(at)?;
        let name = String::from_utf8_lossy(d.get(at + 4..at + 4 + n)?).into_owned();
        at += 4 + n;
        let len = u32_at(at)?;
        out.push((name, d.get(at + 4..at + 4 + len)?));
        at += 4 + len;
    }
    Some(out)
}

/// Whether a zip's file names are a Renoise song's (its Song.xml at the top).
pub fn is_song_zip(names: &[String]) -> bool {
    names.iter().any(|n| n == "Song.xml")
}

/// The number after `word` in a path ("Instrument03 (Bass)" → 3).
fn number_after(path: &str, word: &str) -> Option<usize> {
    let at = path.find(word)? + word.len();
    let digits: String = path[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// A Renoise song, read and ready to play.
#[derive(Clone)]
pub struct Song {
    file: Arc<Vec<u8>>,
    /// A Renoise 1 song (.rns), read from the file itself.
    rns: bool,
    pub model: Arc<Model>,
}

impl Song {
    /// A song from its bundle.
    pub fn parse(file: &[u8]) -> Option<Song> {
        if rns::is_rns(file) {
            return Some(Song {
                file: Arc::new(file.to_vec()),
                rns: true,
                model: Arc::new(rns::read(file)?),
            });
        }
        let files = unbundle(file)?;
        let xml = files.iter().find(|(n, _)| n == "Song.xml")?.1;
        let mut waves: Vec<((usize, usize), &[u8])> = Vec::new();
        for (name, data) in &files {
            // Zips made on Windows may part their paths with backslashes.
            let name = name.replace('\\', "/");
            if let Some(rest) = name.strip_prefix("SampleData/") {
                if let (Some(i), Some(s)) = (number_after(rest, "Instrument"), rest.split('/').nth(1).and_then(|f| number_after(f, "Sample"))) {
                    waves.push(((i, s), data));
                }
            }
        }
        let decode = |i: usize, s: usize| waves.iter().find(|(k, _)| *k == (i, s)).and_then(|(_, d)| audio::decode(d)).map(Arc::new);
        let model = Model::read(xml, &decode)?;
        Some(Song {
            file: Arc::new(file.to_vec()),
            rns: false,
            model: Arc::new(model),
        })
    }

    pub fn file(&self) -> &[u8] {
        &self.file
    }
}

/// A channel at full volume (the Tracker's level: 0.4 at full scale).
const LEVEL: f32 = 0.4;

impl crate::replay::Tune for Song {
    fn format(&self) -> &'static str {
        if self.rns {
            "rns"
        } else {
            "xrns"
        }
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn crate::replay::Driver> {
        Box::new(Player::new(self.model.clone()))
    }

    /// Its sampled instruments: each its XML (its keyzones, envelopes or
    /// modulation, effect chains), the host's samples it plays beside it
    /// (the module has every instrument's samples, in order).
    fn patches(&self) -> Vec<crate::replay::Patch> {
        let mut out = Vec::new();
        let mut first = 0usize;
        for (k, ins) in self.model.instruments.iter().enumerate() {
            let n = ins.samples.len();
            if ins.samples.iter().any(|s| s.audio.frames() > 0) {
                let mut p = crate::replay::PatchBytes::default();
                p.bytes(ins.xml.as_bytes());
                let mut patch = p.patch(k + 1, &ins.name);
                patch.samples = (first..first + n).map(|i| i.min(usize::from(u16::MAX)) as u16).collect();
                out.push(patch);
            }
            first += n;
        }
        out
    }
}

/// The replayer as the host plays it: a tick's audio made when it's
/// heard, the master's inputs its channels.
pub struct Player {
    e: play::Engine,
    fresh: bool,
    n: usize,
    left: Vec<Vec<f32>>,
    right: Vec<Vec<f32>>,
    prev: Vec<(f32, f32)>,
    pos: f64,
    done: bool,
    started: bool,
}

impl Player {
    pub fn new(model: Arc<Model>) -> Player {
        let mut e = play::Engine::new(model);
        e.first();
        let ch = e.outputs.len().max(1);
        Player {
            n: e.frames,
            e,
            fresh: true,
            left: vec![Vec::new(); ch],
            right: vec![Vec::new(); ch],
            prev: vec![(0.0, 0.0); ch],
            pos: 0.0,
            done: false,
            started: false,
        }
    }

    fn make(&mut self) {
        self.e.make(true);
        let n = self.e.frames;
        for c in 0..self.left.len() {
            // A song with no sequencer tracks: its master on the one channel.
            let src = if self.e.out.is_empty() { Some(self.e.master_out()) } else { self.e.out.get(c) };
            self.left[c].clear();
            self.right[c].clear();
            match src {
                Some(o) => {
                    self.left[c].extend(o[0].iter().map(|x| x * LEVEL));
                    self.right[c].extend(o[1].iter().map(|x| x * LEVEL));
                }
                None => {
                    self.left[c].resize(n, 0.0);
                    self.right[c].resize(n, 0.0);
                }
            }
        }
    }
}

impl crate::replay::Driver for Player {
    fn channels(&self) -> usize {
        self.left.len()
    }

    fn tick(&mut self) {
        if self.started {
            if self.fresh {
                // A tick no one heard: its notes and voices move on silently.
                self.e.make(false);
                for p in self.prev.iter_mut() {
                    *p = (0.0, 0.0);
                }
            } else if self.n > 0 {
                for c in 0..self.prev.len() {
                    self.prev[c] = (self.left[c].get(self.n - 1).copied().unwrap_or(0.0), self.right[c].get(self.n - 1).copied().unwrap_or(0.0));
                }
            }
            self.pos = (self.pos - self.n as f64).max(0.0);
            self.e.advance();
            if std::mem::take(&mut self.e.looped) {
                self.done = true;
            }
        }
        self.started = true;
        self.n = self.e.frames;
        self.fresh = true;
    }

    fn tick_seconds(&self) -> f64 {
        self.e.frames as f64 / f64::from(filter::RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut crate::replay::Bufs) {
        if self.fresh {
            self.make();
            self.fresh = false;
        }
        let step = f64::from(filter::RATE) / f64::from(rate.max(1));
        let n = self.n;
        for c in 0..self.left.len() {
            let (bl, br, prev) = (&self.left[c], &self.right[c], self.prev[c]);
            let at = |k: isize| if k < 0 || n == 0 { prev } else { (bl[(k as usize).min(n - 1)], br[(k as usize).min(n - 1)]) };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (a, b) = (at(k as isize), at(k as isize + 1));
                l[i] = a.0 + (b.0 - a.0) * f;
                r[i] = a.1 + (b.1 - a.1) * f;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        (self.e.entry, self.e.line)
    }

    fn speed(&self) -> (u32, u32) {
        (self.e.tpl, self.e.bpm.round() as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.e.stopped
    }
}

/// Renders a song from its start, `secs` seconds (or to where it comes
/// round), as interleaved stereo at the internal rate, at Renoise's level.
pub fn render_stereo(model: Arc<Model>, secs: f64) -> Vec<f32> {
    let mut e = play::Engine::new(model);
    e.first();
    let total = (secs * f64::from(filter::RATE)) as usize;
    let mut out = Vec::with_capacity(total * 2);
    loop {
        e.make(true);
        let m = e.master_out();
        for i in 0..e.frames {
            out.push(m[0][i]);
            out.push(m[1][i]);
        }
        if out.len() >= total * 2 || e.stopped {
            break;
        }
        e.advance();
        if e.looped {
            break;
        }
    }
    out.truncate(total * 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A WAV of one channel at 44.1 kHz.
    fn wav(points: &[i16]) -> Vec<u8> {
        let mut w = b"RIFF".to_vec();
        w.extend(&(36 + points.len() as u32 * 2).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(&16u32.to_le_bytes());
        w.extend(&[1, 0, 1, 0]);
        w.extend(&44100u32.to_le_bytes());
        w.extend(&88200u32.to_le_bytes());
        w.extend(&[2, 0, 16, 0]);
        w.extend(b"data");
        w.extend(&(points.len() as u32 * 2).to_le_bytes());
        for p in points {
            w.extend(&p.to_le_bytes());
        }
        w
    }

    /// A song of one track: a looped 441 Hz sine (100 points) as its only
    /// sample, D-4 on line 1 at 120 BPM, 4 lines a beat.
    fn little_song() -> Vec<u8> {
        let sine: Vec<i16> = (0..100).map(|i| ((i as f32 / 100.0 * std::f32::consts::TAU).sin() * 16000.0) as i16).collect();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<RenoiseSong doc_version="37">
  <GlobalSongData><BeatsPerMin>120</BeatsPerMin><LinesPerBeat>4</LinesPerBeat><TicksPerLine>12</TicksPerLine><SongName>Little</SongName></GlobalSongData>
  <Instruments><Instrument><Name>sine</Name>
    <Samples><Sample><Name>sine</Name><Volume>1.0</Volume><Panning>0.5</Panning><BaseNote>48</BaseNote><NewNoteAction>Cut</NewNoteAction><LoopMode>Forward</LoopMode><LoopStart>0</LoopStart><LoopEnd>100</LoopEnd></Sample></Samples>
    <SampleSplitMap><NoteOnMappings><NoteOnMapping><SampleIndex>0</SampleIndex><BaseNote>48</BaseNote><NoteStart>0</NoteStart><NoteEnd>119</NoteEnd><VelocityStart>0</VelocityStart><VelocityEnd>127</VelocityEnd></NoteOnMapping></NoteOnMappings></SampleSplitMap>
  </Instrument></Instruments>
  <Tracks>
    <SequencerTrack type="SequencerTrack"><Name>One</Name><State>Active</State><NumberOfVisibleNoteColumns>1</NumberOfVisibleNoteColumns><FilterDevices><Devices><SequencerTrackDevice type="SequencerTrackDevice"><IsActive>true</IsActive><Panning><Value>0.5</Value></Panning><Volume><Value>1.0</Value></Volume><Surround><Value>0</Value></Surround><PostPanning><Value>0.5</Value></PostPanning><PostVolume><Value>1.0</Value></PostVolume></SequencerTrackDevice></Devices></FilterDevices></SequencerTrack>
    <SequencerMasterTrack type="SequencerMasterTrack"><Name>Mst</Name><State>Active</State><FilterDevices><Devices><SequencerMasterTrackDevice type="SequencerMasterTrackDevice"><IsActive>true</IsActive><Panning><Value>0.5</Value></Panning><Volume><Value>1.0</Value></Volume><Surround><Value>0</Value></Surround><PostPanning><Value>0.5</Value></PostPanning><PostVolume><Value>1.0</Value></PostVolume></SequencerMasterTrackDevice></Devices></FilterDevices></SequencerMasterTrack>
  </Tracks>
  <PatternPool><Patterns><Pattern><NumberOfLines>8</NumberOfLines><Tracks>
    <PatternTrack type="PatternTrack"><Lines><Line index="1"><NoteColumns><NoteColumn><Note>D-4</Note><Instrument>00</Instrument></NoteColumn></NoteColumns></Line></Lines></PatternTrack>
    <PatternMasterTrack type="PatternMasterTrack"/>
  </Tracks></Pattern></Patterns></PatternPool>
  <PatternSequence><SequenceEntries><SequenceEntry><Pattern>0</Pattern></SequenceEntry></SequenceEntries></PatternSequence>
</RenoiseSong>"#;
        bundle(&[("Song.xml".into(), xml.as_bytes().to_vec()), ("SampleData/Instrument00 (sine)/Sample00 (sine).wav".into(), wav(&sine))])
    }

    #[test]
    fn a_note_starts_on_its_line_at_its_pitch() {
        let s = Song::parse(&little_song()).expect("read");
        assert_eq!(s.model.name, "Little");
        let out = render_stereo(s.model.clone(), 0.5);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        // A line is 1/8 s: silence before it, sound from it.
        let first = left.iter().position(|x| x.abs() > 0.01).expect("sound");
        assert!((5500..5600).contains(&first), "starts at {first}");
        // D-4 over a C-4 sample: 441 Hz up two semitones, counted by its rising crossings.
        let rises = left[6000..6000 + 44100 / 4].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f32 * 4.0;
        let want = 441.0 * 2f32.powf(2.0 / 12.0);
        assert!((rises - want).abs() < 8.0, "{rises} Hz, want {want}");
    }

    /// Every bundle in RNS_DIR (made by the test tools from .xrns files) reads.
    #[test]
    #[ignore]
    fn read_all() {
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        let (mut ok, mut total) = (0, 0);
        for p in names {
            let Ok(d) = std::fs::read(&p) else { continue };
            total += 1;
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            match Song::parse(&d) {
                Some(s) => {
                    ok += 1;
                    let m = &s.model;
                    let samples: usize = m.instruments.iter().map(|i| i.samples.len()).sum();
                    let silent = m.instruments.iter().flat_map(|i| &i.samples).filter(|s| s.audio.frames() == 0).count();
                    println!(
                        "{name}: doc {} {} tracks {} patterns {} positions {} instruments {samples} samples ({silent} empty)",
                        m.doc,
                        m.tracks.len(),
                        m.patterns.len(),
                        m.sequence.len(),
                        m.instruments.len()
                    );
                }
                None => println!("{name}: not read"),
            }
        }
        println!("{ok} of {total} read");
        let _ = Path::new("");
    }

    /// Loops ending at a sample's last frame or past it: whether loop ends
    /// count inclusively.
    #[test]
    #[ignore]
    fn loop_ends() {
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let (mut at_len, mut at_last, mut inside, mut past) = (0, 0, 0, 0);
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let Some(s) = Song::parse(&std::fs::read(e.path()).unwrap()) else { continue };
            for i in &s.model.instruments {
                for x in i.samples.iter().filter(|x| x.loop_mode != song::Loop::Off && x.audio.frames() > 0) {
                    let n = x.audio.frames();
                    match x.loop_end {
                        e if e == n => at_len += 1,
                        e if e + 1 == n => at_last += 1,
                        e if e > n => past += 1,
                        _ => inside += 1,
                    }
                }
            }
        }
        println!("loop end = frames: {at_len}, = frames - 1: {at_last}, inside: {inside}, past: {past}");
    }

    /// Each track's peak over RNS_SECS of the song RNS_ONLY names.
    #[test]
    #[ignore]
    fn peaks() {
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let only = std::env::var("RNS_ONLY").expect("RNS_ONLY");
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let p = Path::new(&dir).join(format!("{only}.rnb"));
        let s = Song::parse(&std::fs::read(&p).unwrap()).unwrap();
        let mut e = play::Engine::new(s.model.clone());
        e.first();
        let mut peaks = vec![0.0f32; s.model.tracks.len()];
        let mut master = 0.0f32;
        let mut most_voices = 0;
        let mut t = 0.0;
        while t < secs {
            e.make(true);
            for (k, p) in e.track_peaks().into_iter().enumerate() {
                peaks[k] = peaks[k].max(p);
            }
            master = e.master_out().iter().flat_map(|b| b.iter()).fold(master, |a, &x| a.max(x.abs()));
            most_voices = most_voices.max(e.voice_count());
            t += e.frames as f64 / 44100.0;
            e.advance();
        }
        for (k, tr) in s.model.tracks.iter().enumerate() {
            println!(
                "{k:2} {:?} {:30} peak {:.3}  devices {:?}",
                tr.kind,
                tr.name,
                peaks[k],
                tr.devices.iter().map(|d| d.kind.as_str()).collect::<Vec<_>>()
            );
        }
        println!("master peak {master:.3}, at most {most_voices} voices");
    }

    /// Each track's channel to a WAV (RNS_OUT/<song>.<k>.wav), RNS_SECS of RNS_ONLY.
    #[test]
    #[ignore]
    fn channels() {
        use crate::replay::Tune;
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let only = std::env::var("RNS_ONLY").expect("RNS_ONLY");
        let out = std::env::var("RNS_OUT").expect("RNS_OUT");
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(5.0);
        let s = Song::parse(&std::fs::read(Path::new(&dir).join(format!("{only}.rnb"))).unwrap()).unwrap();
        let mut d = s.driver(0);
        let n = d.channels();
        let mut bufs = crate::replay::Bufs {
            left: vec![vec![0.0; 4096]; n],
            right: vec![vec![0.0; 4096]; n],
        };
        let mut data = vec![Vec::new(); n];
        let mut t = 0.0;
        while t < secs {
            d.tick();
            let frames = (d.tick_seconds() * 44100.0).round() as usize;
            d.render(44100, frames.min(4096), &mut bufs);
            for c in 0..n {
                for i in 0..frames.min(4096) {
                    data[c].push(bufs.left[c][i]);
                    data[c].push(bufs.right[c][i]);
                }
            }
            t += d.tick_seconds();
        }
        for (c, v) in data.iter().enumerate() {
            write_wav(&Path::new(&out).join(format!("{only}.{c}.wav")), v);
        }
    }

    /// Render time and voices every ten seconds of RNS_ONLY, RNS_SECS long.
    #[test]
    #[ignore]
    fn timing() {
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let only = std::env::var("RNS_ONLY").expect("RNS_ONLY");
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0);
        let s = Song::parse(&std::fs::read(Path::new(&dir).join(format!("{only}.rnb"))).unwrap()).unwrap();
        let mut e = play::Engine::new(s.model.clone());
        e.first();
        let (mut t, mut next) = (0.0, 10.0);
        let mut clock = std::time::Instant::now();
        let mut most = 0;
        while t < secs {
            let tick = std::time::Instant::now();
            e.make(true);
            if tick.elapsed().as_secs_f64() > 0.05 {
                println!(
                    "slow tick: {:.3}s, {} frames, entry {} line {} tick {}, bpm {} tpl {}",
                    tick.elapsed().as_secs_f64(),
                    e.frames,
                    e.entry,
                    e.line,
                    e.tick,
                    e.bpm,
                    e.tpl
                );
            }
            most = most.max(e.voice_count());
            t += e.frames as f64 / 44100.0;
            let a = std::time::Instant::now();
            e.advance();
            if a.elapsed().as_secs_f64() > 0.05 {
                println!("slow advance: {:.3}s at entry {} line {}", a.elapsed().as_secs_f64(), e.entry, e.line);
            }
            if t >= next {
                println!("{next:4.0}s: {:.2}s, at most {most} voices, entry {} line {}", clock.elapsed().as_secs_f64(), e.entry, e.line);
                clock = std::time::Instant::now();
                most = 0;
                next += 10.0;
            }
        }
    }

    fn write_wav(path: &Path, data: &[f32]) {
        let mut w = Vec::new();
        let n = data.len() * 2;
        w.extend(b"RIFF");
        w.extend(&((36 + n) as u32).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(&16u32.to_le_bytes());
        w.extend(&1u16.to_le_bytes());
        w.extend(&2u16.to_le_bytes());
        w.extend(&44100u32.to_le_bytes());
        w.extend(&(44100u32 * 4).to_le_bytes());
        w.extend(&4u16.to_le_bytes());
        w.extend(&16u16.to_le_bytes());
        w.extend(b"data");
        w.extend(&(n as u32).to_le_bytes());
        for &x in data {
            w.extend(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(path, w).unwrap();
    }

    /// Renders every bundle in RNS_DIR (RNS_ONLY a name part, RNS_SECS),
    /// writing WAVs to RNS_OUT when set, and prints each one's levels.
    #[test]
    #[ignore]
    fn render_all() {
        let dir = std::env::var("RNS_DIR").expect("RNS_DIR");
        let only = std::env::var("RNS_ONLY").ok();
        let out = std::env::var("RNS_OUT").ok();
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Some(s) = Song::parse(&std::fs::read(&p).unwrap()) else { continue };
            let t0 = std::time::Instant::now();
            let data = render_stereo(s.model.clone(), secs);
            let el = t0.elapsed().as_secs_f64();
            let peak = data.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
            let rms = (data.iter().map(|x| x * x).sum::<f32>() / data.len().max(1) as f32).sqrt();
            let nan = data.iter().filter(|x| !x.is_finite()).count();
            let silent = data.chunks(4410 * 2).filter(|c| c.iter().all(|x| x.abs() < 1e-4)).count();
            println!(
                "{name}: {:.1}s rendered in {el:.2}s, peak {peak:.3} rms {rms:.4} silent {silent}/{} nan {nan}",
                data.len() as f64 / 88200.0,
                data.len() / 8820 + 1
            );
            if let Some(o) = &out {
                write_wav(&Path::new(o).join(format!("{name}.wav")), &data);
            }
        }
    }
}
