// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! SunVox (Alexander Zolotov, WarmPlace, 2008-): a modular synthesizer
//! with a tracker. A project is a network of modules (synths, samplers,
//! effects, modulators, MetaModules holding projects of their own) linked
//! into its Output, and patterns placed on a timeline whose tracks send
//! notes, velocities, controller changes and effects to the modules.
//!
//! Ported from the SunVox Library's engine (lib_sunvox, lib_sundog's
//! parts it uses, lib_dsp), which its author released under the MIT
//! License: the timeline, the pattern effects and the module network as
//! it renders them, each module its own port, in 32-bit float with C's
//! promotions to double kept, at 44.1 kHz in blocks of 882 frames as the
//! library renders them (the full-quality build, CPUMARK 10, on a CPU
//! without x86's denormal noise). Its notice:
//!
//! > Copyright (c) 2007 - 2025, Alexander Zolotov <nightradio@gmail.com>
//! > WarmPlace.ru
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining
//! > a copy of this software and associated documentation files (the
//! > "Software"), to deal in the Software without restriction, including
//! > without limitation the rights to use, copy, modify, merge, publish,
//! > distribute, sublicense, and/or sell copies of the Software, and to
//! > permit persons to whom the Software is furnished to do so, subject to
//! > the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be
//! > included in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! > EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! > MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! > NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//! > LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
//! > OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
//! > WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

#![allow(dead_code)]

mod dsp;
mod engine;
mod file;
pub(crate) mod modules;
mod net;
mod subengine;

use std::sync::Arc;

use engine::Engine;
pub use file::is_sunvox;

/// The library's sample rate, and its blocks.
pub const RATE: u32 = 44100;

/// A SunVox project.
#[derive(Clone)]
pub struct Song {
    file: Arc<Vec<u8>>,
}

impl Song {
    /// The project read (and played to its first frame to be sure it loads).
    pub fn parse(file: &[u8]) -> Option<Song> {
        if !is_sunvox(file) {
            return None;
        }
        let song = Song { file: Arc::new(file.to_vec()) };
        song.engine(false).ok()?;
        Some(song)
    }

    /// An engine with the project loaded, as the library has it after
    /// sv_open_slot, sv_load and (when `play`) sv_play_from_beginning.
    pub(crate) fn engine(&self, play: bool) -> Result<Engine, file::LoadError> {
        let globals = net::Globals::new();
        let flags = engine::FLAG_CREATE_PATTERN | engine::FLAG_CREATE_MODULES | engine::FLAG_MAIN | engine::FLAG_ONE_THREAD;
        // sv_open_slot's engine, closed by the load.
        drop(Engine::new(flags, RATE as i32, globals.clone()));
        let flags = (flags & !(engine::FLAG_CREATE_PATTERN | engine::FLAG_CREATE_MODULES)) | engine::FLAG_DONT_START;
        let mut s = Engine::new(flags, RATE as i32, globals);
        match file::load(&self.file, &mut s) {
            Ok(()) => {}
            // A file cut off keeps what it has, as the library does.
            Err(file::LoadError::Truncated) => {}
            Err(e) => return Err(e),
        }
        if play {
            s.stop_at_the_end_of_proj = true;
            s.play(0, true);
        }
        Ok(s)
    }

    pub fn file(&self) -> &[u8] {
        &self.file
    }
}

/// Renders a project as the reference harness does: from the beginning,
/// 882 frames at a time, `secs` seconds or to its end; interleaved stereo.
pub fn render_stereo(file: &[u8], secs: f64) -> Option<Vec<f32>> {
    let song = Song { file: Arc::new(file.to_vec()) };
    let mut s = song.engine(true).ok()?;
    let total = (secs * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total * 2);
    let mut bufs = [vec![0.0f32; 882], vec![0.0f32; 882]];
    let mut done = 0;
    while done < total {
        let n = (total - done).min(882);
        s.render(n, &mut bufs);
        for i in 0..n {
            out.push(bufs[0][i]);
            out.push(bufs[1][i]);
        }
        done += n;
        if !s.playing {
            break;
        }
    }
    Some(out)
}

/// Frames a tick: the library's block (20 ms).
const BLOCK: usize = 882;
/// A full-scale channel's level here (the Tracker's: 0.4 at full volume).
const LEVEL: f32 = 0.4;
/// Lines a pattern in the tracker's view of the timeline.
pub const VIEW_LINES: i32 = 64;

/// A note on the timeline, for the tracker's view.
#[derive(Clone, Copy, Debug)]
pub struct NoteAt {
    pub line: i32,
    /// The channel (see [`Summary::channels`]) its module sounds through.
    pub channel: usize,
    /// 1..=127 a note (C0 = 1), 128 note off.
    pub note: u8,
    /// 1..=129 a velocity + 1, 0 the module's own.
    pub vel: u8,
    /// Its module, 0 none.
    pub module: u16,
    pub ctl: u16,
    pub ctl_val: u16,
}

/// A project as the tracker shows it.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub name: String,
    pub bpm: u16,
    pub speed: u8,
    /// The timeline's lines, and where it loops back to.
    pub lines: i32,
    pub restart: i32,
    /// Its channels: the modules linked into the Output (the Output's own
    /// mix when none is).
    pub channels: Vec<String>,
    /// Every module's name, by its number (0 the Output; empty slots "").
    pub modules: Vec<String>,
    /// The patterns' notes, in timeline order.
    pub notes: Vec<NoteAt>,
}

/// The modules the Output takes in, in the order it adds them.
fn outputs(s: &Engine) -> Vec<usize> {
    let n = &s.net;
    n.mods[0].input_links.iter().filter_map(|&l| usize::try_from(l).ok()).filter(|&l| l < n.mods_num() && n.mods[l].flags & net::FLAG_EXISTS != 0).collect()
}

/// Which of the Output's inputs a module reaches first (by its links out).
fn channel_of(s: &Engine, outs: &[usize], m: usize) -> usize {
    let n = &s.net;
    let mut seen = vec![false; n.mods_num()];
    let mut queue = std::collections::VecDeque::from([m]);
    while let Some(x) = queue.pop_front() {
        if x >= seen.len() || seen[x] {
            continue;
        }
        seen[x] = true;
        if let Some(c) = outs.iter().position(|&o| o == x) {
            return c;
        }
        for &l in &n.mods[x].output_links {
            if let Ok(l) = usize::try_from(l) {
                queue.push_back(l);
            }
        }
    }
    0
}

impl Song {
    /// The project's settings, channels, modules and notes.
    pub fn summary(&self) -> Summary {
        let Ok(mut s) = self.engine(false) else { return Summary::default() };
        s.sort_patterns();
        let outs = outputs(&s);
        let names = |i: usize| s.net.mods.get(i).filter(|m| m.flags & net::FLAG_EXISTS != 0).map_or(String::new(), |m| m.name.clone());
        let channels = if outs.is_empty() { vec!["Output".to_string()] } else { outs.iter().map(|&o| names(o)).collect() };
        let modules = (0..s.net.mods_num()).map(names).collect();
        let mut notes = Vec::new();
        let mut map: std::collections::HashMap<usize, usize> = Default::default();
        for p in 0..s.pats.len() {
            let Some(pat) = s.pat(p) else { continue };
            let info = s.pats_info[p];
            if info.flags & engine::PAT_INFO_FLAG_MUTE != 0 {
                continue;
            }
            for l in 0..pat.lines.min(pat.data_ysize) {
                for t in 0..pat.channels.min(pat.data_xsize) {
                    let Some(n) = pat.data.get((l * pat.data_xsize + t) as usize).copied() else { continue };
                    if n.note == 0 && n.ctl == 0 {
                        continue;
                    }
                    let module = n.module as usize;
                    let channel = if module == 0 { 0 } else { *map.entry(module - 1).or_insert_with(|| channel_of(&s, &outs, module - 1)) };
                    notes.push(NoteAt { line: info.x + l, channel, note: n.note, vel: n.vel, module: n.module, ctl: n.ctl, ctl_val: n.ctl_val });
                }
            }
        }
        notes.sort_by_key(|n| n.line);
        Summary { name: s.proj_name.clone(), bpm: s.bpm, speed: s.speed, lines: s.proj_lines_now(), restart: s.restart_pos, channels, modules, notes }
    }
}

impl crate::replay::Tune for Song {
    fn format(&self) -> &'static str {
        "sunvox"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn crate::replay::Driver> {
        Box::new(Player::new(self))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        let mut s = self.engine(false).ok()?;
        s.sort_patterns();
        Some(s.proj_frames() as f64 / f64::from(RATE))
    }
    /// Its instruments: the modules that make sound from notes (synths,
    /// samplers, MetaModules, the Vorbis player), each as a module file.
    /// Effects and controllers are the song's wiring, not instruments.
    fn patches(&self) -> Vec<crate::replay::Patch> {
        use modules::Kind;
        file::module_files(&self.file)
            .into_iter()
            .filter(|(_, _, k, _)| matches!(k, Kind::Generator | Kind::Kicker | Kind::Fm | Kind::Fmx | Kind::DrumSynth | Kind::Sampler | Kind::SpectraVoice | Kind::AnalogGenerator | Kind::MetaModule | Kind::VorbisPlayer))
            .map(|(n, name, _, data)| crate::replay::Patch { slot: n.min(usize::from(u16::MAX)) as u16, name, data, samples: Vec::new() })
            .collect()
    }
}

/// The replayer: a block of the library's at a tick, each of the
/// Output's inputs a channel (at the project's volume).
pub struct Player {
    s: Option<Box<Engine>>,
    outs: Vec<usize>,
    left: Vec<Vec<f32>>,
    right: Vec<Vec<f32>>,
    prev: Vec<(f32, f32)>,
    /// Where in the block the next frame is read, from its first sample.
    pos: f64,
    done: bool,
    last_line: i32,
}

impl Player {
    fn new(song: &Song) -> Player {
        let s = song.engine(true).ok().map(Box::new);
        let outs = s.as_deref().map(outputs).unwrap_or_default();
        let channels = outs.len().max(1);
        let mut p = Player { s, outs, left: vec![vec![0.0; BLOCK]; channels], right: vec![vec![0.0; BLOCK]; channels], prev: vec![(0.0, 0.0); channels], pos: 0.0, done: false, last_line: 0 };
        if let Some(s) = p.s.as_mut() {
            p.last_line = s.line_counter;
            s.stop_at_the_end_of_proj = false;
        }
        p
    }

    /// The block just rendered, each channel's.
    fn take(&mut self) {
        let Some(s) = self.s.as_deref() else { return };
        let n = &s.net;
        let gv = n.global_volume as f32 / 256.0;
        if self.outs.is_empty() {
            let m0 = &n.mods[0];
            for ch in 0..2 {
                let src = if !m0.chin[ch].is_empty() { ch } else { 0 };
                let out = if ch == 0 { &mut self.left[0] } else { &mut self.right[0] };
                for (i, o) in out.iter_mut().enumerate() {
                    *o = m0.chin[src].get(i).copied().unwrap_or(0.0) * LEVEL;
                }
            }
            return;
        }
        for (c, &o) in self.outs.iter().enumerate() {
            let m = &n.mods[o];
            for ch in 0..2 {
                let src = if (ch as i32) < m.output_channels && !m.chout[ch].is_empty() { ch } else { 0 };
                let out = if ch == 0 { &mut self.left[c] } else { &mut self.right[c] };
                let empty = m.out_empty[src].max(0) as usize;
                for (i, v) in out.iter_mut().enumerate() {
                    *v = if i < empty { 0.0 } else { m.chout[src].get(i).copied().unwrap_or(0.0) * gv * LEVEL };
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
        for c in 0..self.left.len() {
            self.prev[c] = (self.left[c][BLOCK - 1], self.right[c][BLOCK - 1]);
        }
        self.pos = (self.pos - BLOCK as f64).max(0.0);
        let Some(s) = self.s.as_mut() else { return };
        if !s.playing {
            for c in 0..self.left.len() {
                self.left[c].fill(0.0);
                self.right[c].fill(0.0);
            }
            return;
        }
        s.render(BLOCK, &mut [Vec::new(), Vec::new()]);
        let line = s.line_counter;
        if !s.playing || line < self.last_line {
            self.done = true;
        }
        self.last_line = line;
        self.take();
    }

    fn tick_seconds(&self) -> f64 {
        BLOCK as f64 / f64::from(RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut crate::replay::Bufs) {
        let step = f64::from(RATE) / f64::from(rate.max(1));
        for c in 0..self.left.len() {
            let (bl, br, prev) = (&self.left[c], &self.right[c], self.prev[c]);
            let at = |k: isize| if k < 0 { prev } else { (bl[(k as usize).min(BLOCK - 1)], br[(k as usize).min(BLOCK - 1)]) };
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
        let line = self.s.as_ref().map_or(0, |s| s.line_counter.max(0)) as usize;
        (line / VIEW_LINES as usize, line % VIEW_LINES as usize)
    }

    fn speed(&self) -> (u32, u32) {
        self.s.as_ref().map_or((6, 125), |s| (u32::from(s.speed), u32::from(s.bpm)))
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        self.s.as_ref().is_some_and(|s| s.playing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn read_wav_f32(p: &Path) -> Option<Vec<f32>> {
        let d = std::fs::read(p).ok()?;
        Some(d[44..].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }

    /// Ours against the reference library's renders: SV_DIR (projects),
    /// SV_REF (sv2wav's WAVs, same names), SV_SECS, SV_ONLY.
    #[test]
    #[ignore]
    fn against() {
        let dir = std::env::var("SV_DIR").expect("SV_DIR");
        let refd = std::env::var("SV_REF").expect("SV_REF");
        let secs: f64 = std::env::var("SV_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(20.0);
        let only = std::env::var("SV_ONLY").ok();
        let mut files: Vec<_> = walk(Path::new(&dir));
        files.sort();
        let (mut same, mut total) = (0, 0);
        for f in files {
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Some(r) = read_wav_f32(&Path::new(&refd).join(format!("{name}.wav"))) else { continue };
            let data = std::fs::read(&f).unwrap();
            let kinds = kinds_missing(&data);
            let Some(ours) = render_stereo(&data, secs) else {
                println!("{name}: not loaded");
                continue;
            };
            total += 1;
            let n = ours.len().min(r.len());
            let mut first = None;
            let mut maxd = 0.0f32;
            for i in 0..n {
                let d = (ours[i] - r[i]).abs();
                if d > maxd {
                    maxd = d;
                }
                if d != 0.0 && first.is_none() {
                    first = Some(i / 2);
                }
            }
            if first.is_none() && ours.len() == r.len() {
                same += 1;
                println!("{name}: same ({} frames)", n / 2);
            } else {
                println!("{name}: first difference at frame {:?} of {} (lengths {} {}), max {maxd:.6}{}", first, n / 2, ours.len() / 2, r.len() / 2, kinds);
            }
        }
        println!("{same} of {total} the same");
    }

    /// Each module's output per block, as sv2wav's SV_DUMP writes it, then
    /// compared with the reference's: SV_FILE, SV_SECS, SV_REFDUMP.
    #[test]
    #[ignore]
    fn dump() {
        let file = std::env::var("SV_FILE").expect("SV_FILE");
        let refd = std::env::var("SV_REFDUMP").expect("SV_REFDUMP");
        let secs: f64 = std::env::var("SV_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(8.0);
        let data = std::fs::read(&file).unwrap();
        let song = Song { file: Arc::new(data) };
        let mut s = song.engine(true).unwrap();
        let total = (secs * RATE as f64) as usize;
        let mut bufs = [vec![0.0f32; 882], vec![0.0f32; 882]];
        let mut outs: std::collections::BTreeMap<(usize, usize), Vec<f32>> = Default::default();
        let mut done = 0;
        while done < total {
            let n = (total - done).min(882);
            s.render(n, &mut bufs);
            for (mi, m) in s.net.mods.iter().enumerate() {
                if m.flags & net::FLAG_EXISTS == 0 {
                    continue;
                }
                for ch in 0..2 {
                    let (c, empty) = if m.flags & net::FLAG_OUTPUT != 0 { (&m.chin[ch], m.in_empty[ch]) } else { (&m.chout[ch], m.out_empty[ch]) };
                    if c.is_empty() {
                        continue;
                    }
                    let v = outs.entry((mi, ch)).or_default();
                    for i in 0..n {
                        v.push(if (i as i32) < empty { 0.0 } else { c[i] });
                    }
                }
            }
            done += n;
            if !s.playing {
                break;
            }
        }
        let mut firsts = Vec::new();
        for ((mi, ch), v) in &outs {
            let p = Path::new(&refd).join(format!("m{mi}_c{ch}.f32"));
            let Some(r) = std::fs::read(&p).ok() else {
                println!("m{mi} c{ch}: no reference");
                continue;
            };
            let r: Vec<f32> = r.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            let first = v.iter().zip(&r).position(|(a, b)| a.to_bits() != b.to_bits());
            let kind = s.net.mods[*mi].kind;
            if let Some(f) = first {
                firsts.push((f, *mi, *ch));
                println!("m{mi} c{ch} {kind:?}: first difference at {f} (ours {} ref {})", v[f], r[f]);
            }
        }
        firsts.sort();
        println!("earliest: {:?}", firsts.first());
    }

    /// Every instrument of every project in SV_DIR loads back as a module
    /// file, as the kind it was, with its controllers.
    #[test]
    #[ignore]
    fn patches_load() {
        use crate::replay::Tune;
        let dir = std::env::var("SV_DIR").expect("SV_DIR");
        let (mut all, mut unique) = (0, std::collections::HashSet::new());
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            let Ok(data) = std::fs::read(&path) else { continue };
            let Some(song) = Song::parse(&data) else { continue };
            let s = song.engine(false).unwrap();
            for p in song.patches() {
                let mut e = Engine::new(engine::FLAG_CREATE_MODULES | engine::FLAG_ONE_THREAD, RATE as i32, net::Globals::new());
                let n = file::load_module(&p.data, &mut e);
                assert!(n > 0, "{:?} module {} didn't load", path, p.slot);
                let (a, b) = (&s.net.mods[usize::from(p.slot)], &e.net.mods[n as usize]);
                assert_eq!(a.kind, b.kind, "{:?} module {}", path, p.slot);
                assert_eq!(a.vars, b.vars, "{:?} module {} controllers", path, p.slot);
                all += 1;
                unique.insert(p.data);
            }
        }
        println!("{all} instruments, {} different", unique.len());
    }

    /// The driver's channels added up against the library's mix (at
    /// LEVEL), SV_FILE, SV_SECS: the largest difference.
    #[test]
    #[ignore]
    fn driver_sum() {
        use crate::replay::Tune;
        let file = std::env::var("SV_FILE").expect("SV_FILE");
        let secs: f64 = std::env::var("SV_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
        let data = std::fs::read(&file).unwrap();
        let mix = render_stereo(&data, secs).unwrap();
        let song = Song::parse(&data).unwrap();
        let mut d = song.driver(0);
        let ch = d.channels();
        let mut bufs = crate::replay::Bufs { left: vec![vec![0.0; BLOCK]; ch], right: vec![vec![0.0; BLOCK]; ch] };
        let mut maxd = 0.0f32;
        let mut at = 0;
        while at + BLOCK <= mix.len() / 2 {
            d.tick();
            // A sample behind, as rendered at the library's rate.
            d.render(RATE, BLOCK, &mut bufs);
            for i in 1..BLOCK {
                let (mut l, mut r) = (0.0f32, 0.0f32);
                for c in 0..ch {
                    l += bufs.left[c][i];
                    r += bufs.right[c][i];
                }
                let k = at + i - 1;
                maxd = maxd.max((l - mix[k * 2] * LEVEL).abs()).max((r - mix[k * 2 + 1] * LEVEL).abs());
            }
            at += BLOCK;
        }
        println!("{ch} channels, largest difference {maxd:e}");
    }

    /// The module types used (MetaModules' projects included) not ported.
    fn kinds_missing(data: &[u8]) -> String {
        let mut missing = Vec::new();
        let mut i = 0;
        while i + 8 <= data.len() {
            if &data[i..i + 4] == b"STYP" {
                let len = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
                let b = &data[i + 8..(i + 8 + len).min(data.len())];
                let name = String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned();
                let k = modules::kind_by_name(&name);
                if !modules::implemented(k) && !missing.contains(&name) {
                    missing.push(name);
                }
                i += 8;
            } else {
                i += 1;
            }
        }
        if missing.is_empty() {
            String::new()
        } else {
            format!(" (not ported: {})", missing.join(", "))
        }
    }

    fn walk(d: &Path) -> Vec<std::path::PathBuf> {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    v.extend(walk(&p));
                } else if p.extension().is_some_and(|x| x == "sunvox") {
                    v.push(p);
                }
            }
        }
        v
    }
}
