//! The synth and chip trackers, whose sounds are made as the song plays:
//! each read by neumetik-synth (`replay`), which plays it by its own
//! replayer; the module here shows it (its patterns, its instruments'
//! names as samples with no points) and keeps it in `Module::tune`.
//!
//! - AHX ("THX") and HivelyTracker ("HVL"): positions of a track a
//!   channel, each transposed; a pattern here a position.
//! - Future Composer 1.3 and 1.4: positions of a pattern a channel; its
//!   samples (not its waveforms) the module's, for the repository.
//! - SidMon 1: its rip's data found by its replayer's instructions; each
//!   channel's track rows laid out by their durations.
//! - SidMon 2: positions of a track a channel, its rows packed with runs
//!   of empty ones.
//! - BP SoundMon 1, 2.0 and 2.2: steps of a 16-row track a channel.
//! - Delta Music 2: each channel its own list of 16-row blocks, looping on
//!   its own; a pattern here the longest list's step, each channel's own
//!   step beside it.
//! - Delta Music 1: the same, each channel's list with jumps followed.
//! - Digital Mugician 1 to 3: sub-songs of 64-row tracks a channel; each
//!   sub-song its own patterns and order list (7 channels in 2 and 3).
//! - Sonic Arranger: positions, each channel a run of the song's rows;
//!   each sub-song its span of the positions.
//! - InStereo! 1.0 and 2.0: laid out as Sonic Arranger is; Synthesis as
//!   InStereo! 1.0 (its file laid out as 1.0's).
//! - Voodoo Supreme Synthesizer: each channel's stream of notes (its
//!   calls and loops followed, to its jump back) laid out by tick, a row
//!   for as many ticks as every note's hold is a multiple of.
//! - Music Assembler: a rip, its data found by its replayer's
//!   instructions; each sub-song a pattern for each of the first
//!   channel's tracks (repeats their own), each channel's rows laid out
//!   by how long each is held.
//! - Sawteeth: each channel its own list of parts (each part its own
//!   speed and length), looping on its own; a pattern here each of the
//!   first channel's steps, a row as many ticks as every channel's steps
//!   in it fall on.
//! - MONOTONE: 64-row patterns of one to four tracks (the PC speaker
//!   sounds the first four), an order list of them.
//! - Klystrack: each channel its own sequence of patterns (any length,
//!   transposed) placed on the song's rows; a pattern here a stretch of
//!   rows as long as its sequence editor's step.
//! - Pretracker: positions of a pattern a channel, each transposed; a
//!   pattern here a position. Its waves, drawn at load, stay the
//!   replayer's.
//! - Organya: eight melody and eight drum tracks of notes on a timeline
//!   of ticks; a pattern here a measure of the editor's grid, a row a
//!   tick.
//! - Future Composer BSI ("FUCO"): each channel its own track of shared
//!   blocks, stepping at its own pace; its notes laid out by step, a row
//!   for as many steps as every note's hold is a multiple of. Its digis
//!   the module's samples, for the repository.
//! - pxtone (Piston Collage): units driven by one list of events on a
//!   clock; a pattern here a measure, four rows a beat, a channel a unit.
//!   Its PCM voices the module's samples, for the repository.
//! - Pro Tracker 3 (and Vortex Tracker II's .pt3): positions of a pattern
//!   (each its own length) a channel, each channel's lines held their own
//!   count of rows; its samples as instruments (no points: they're AY
//!   settings).
//! - GoatTracker 1 ("GTS!") and 2 ("GTS5", and "GTS2" before its speed
//!   table): each channel its own order list of patterns
//!   (any length); followed without the SID, a pattern here each of the
//!   first channel's, the others' notes laid on its rows. A sequence a
//!   subtune.
//! - PMD and FMP (the PC-98's sound drivers): compiled MML, each part a
//!   stream of commands; its notes found by following the driver without
//!   its chips, a pattern a measure, a row a sixteenth. The PCM files the
//!   song asks for (beside it) are kept with it.
//! - MGSDRV (MSX): compiled MML, a track a channel (PSG, SCC, FM, and
//!   the drums in rhythm mode); its notes found by following the driver
//!   without its chips, a pattern a whole note, a row a sixteenth.
//! - Arkos Tracker 1 to 3 (.aks, XML, zipped or gzipped as the generic
//!   unpacking finds them) and STarKos (.sks): positions of a pattern (a
//!   track a channel, transposed, each position its own height); a
//!   pattern here a position, a sequence a subsong, its instruments as
//!   named samples (PSG instruments have no points).
//! - FamiTracker (.ftm, 0CC-FamiTracker's .0cc, Dn-FamiTracker's .dnm):
//!   frames of a pattern a channel (2A03 and expansion chips); a pattern
//!   here a frame, a sequence a song (FamiTracker's "tracks").
//! - DefleMask (.dmf, zlib-packed): a pattern matrix (a pattern a
//!   channel at each order); a pattern here an order, its instruments as
//!   named samples. Only the systems neumetik-synth plays are read.
//! - VIC-TRACKER (VIC-20, "T1" and version 1.0's "T0"): five pattern
//!   lists (three voices, the noise, and the volume and tempo) of 32-step
//!   patterns, each position its own length; a pattern here a position.
//!   Its songs (start, end and repeat positions) as sequences, its 16
//!   sounds as instruments.
//! - V2 (V2M): a MIDI sequence of 16 channels; a row here a sixteenth
//!   note, 64 to a pattern, a cell a row's first note on its channel.
//! - SunVox: a network of modules into its Output, and patterns placed
//!   on a timeline of lines, their tracks' notes each for a module; a
//!   channel here a module the Output takes in, a pattern 64 lines of the
//!   timeline, each note on the channel its module sounds through (the
//!   line's first there), its instrument the module.
//! - Renoise (.xrns, a zip of its Song.xml and sample files): tracks of
//!   note and effect columns, patterns in a pool, a sequence of them; a
//!   channel here a (sequencer) track, its line's first note there, a
//!   pattern Renoise's, the orders its sequence. Its instruments' samples
//!   (FLAC, WAV or Ogg Vorbis) are the module's, each instrument mapping
//!   its notes to them.
//! - Renoise 1 (.rns, Renoise 0.5 to 1.28): its own binary songs, read
//!   into the same model and shown the same way.
//! - StoneTracker (Amiga, "SPM." with its "SPS." sample bank beside it):
//!   songs of 4 to 8 tracks and an FX track, a pattern a track at each
//!   position; a pattern here a position of a song, a sequence a song.
//!   Its bank's samples (unpacked when Huffman-packed) are the module's.

use std::sync::Arc;

use neumetik_synth::{ahx, arkos, stonetracker, deflemask, deflemask_play, famitracker, famitracker_play, deltamusic1, deltamusic2, fc, fcbsi, goattracker, goattracker1, instereo, mdsdrv, mdsmml, mgs, mgsc, mmlgb, nsc, nsf, pc98, pt3, pxtone, klystrack, renoise, sunvox, mugician, monotone, musicassembler, organya, pretracker, sawteeth, sidmon1, sidmon2, sonic, soundmon, synthesis, v2, victracker, voodoo};

use crate::formats::Companion;
use crate::io::{Error, Result};
use crate::formats::common::amiga_pan as common_pan;
use crate::model::*;

/// The patterns' note for a format's 1-based note on `base` (the
/// tracker's note for its first), none when out of range.
fn note_at(note: i32, base: i32) -> u8 {
    let n = note - 1 + base;
    if (i32::from(NOTE_MIN)..=i32::from(NOTE_MAX)).contains(&n) {
        n as u8
    } else {
        NOTE_NONE
    }
}

/// A sample standing for an instrument: its name, no points.
fn named(name: &str) -> Sample {
    Sample { name: name.to_string(), ..Sample::empty() }
}

/// The order lists of a song whose positions are its patterns, one a
/// subsong from its first position (and back to the song's restart, when
/// that's in it).
fn subsong_orders(m: &mut Module, starts: &[usize], positions: usize, restart: usize) {
    let seqs: Vec<Sequence> = starts
        .iter()
        .enumerate()
        .map(|(k, &s)| Sequence { name: if k == 0 { "Song".into() } else { format!("Song {}", k + 1) }, orders: (s..positions).map(|p| p as u16).collect(), restart: restart.saturating_sub(s) as u16 })
        .collect();
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
}

pub fn probe_ahx(d: &[u8]) -> bool {
    ahx::probe(d)
}

/// AHX's effects the tracker has a name for, for the screen.
fn ahx_fx(fx: u8, p: u8) -> (Fx, u8) {
    let decimal = (p >> 4) * 10 + (p & 0xF);
    match fx {
        0x1 => (Fx::PortaUp, p),
        0x2 => (Fx::PortaDown, p),
        0x3 => (Fx::TonePorta, p),
        0x5 => (Fx::TonePortaVol, p),
        0x7 => (Fx::Panning, (p as i8 as i16 + 128).clamp(0, 255) as u8),
        0xA => (Fx::VolSlide, p),
        0xB => (Fx::PositionJump, decimal),
        0xC if p <= 0x40 => (Fx::Volume, p),
        0xD => (Fx::PatternBreak, decimal),
        0xE if matches!(p >> 4, 1 | 2 | 0xA | 0xB | 0xC | 0xD) => (Fx::ModEx, p),
        0xF => (Fx::Speed, p),
        _ => (Fx::None, 0),
    }
}

pub fn load_ahx(d: &[u8]) -> Result<Module> {
    let song = ahx::Song::parse(d).ok_or_else(|| Error::Bad("its header or tracks are damaged".into()))?;
    let hvl = song.kind == ahx::Kind::Hvl;
    let nch = song.channels;
    let mut m = Module {
        title: song.title.clone(),
        format: if hvl { "hvl" } else { "ahx" }.into(),
        format_name: if hvl { "HivelyTracker" } else { "AHX" }.into(),
        tracker: match song.kind {
            ahx::Kind::Ahx1 => "AHX 1",
            ahx::Kind::Ahx2 => "AHX 2",
            ahx::Kind::Hvl => "HivelyTracker",
        }
        .into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: (125 * song.speed_multiplier.max(1)) as u16,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    const LEFT: [u8; 5] = [128, 96, 64, 32, 0];
    const RIGHT: [u8; 5] = [128, 160, 193, 225, 255];
    m.channels = (0..nch).map(|c| Channel { pan: if matches!(c % 4, 0 | 3) { LEFT[song.stereo] } else { RIGHT[song.stereo] }, ..Default::default() }).collect();
    for p in 0..song.positions.len() {
        let mut pat = Pattern::new(song.track_length, nch);
        for c in 0..nch {
            let tr = i32::from(song.positions[p].transpose[c]);
            for row in 0..song.track_length {
                let s = song.step(p, c, row);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                // AHX's C-1 is the tracker's C-2 (ProTracker's C-1 is AHX's C-3).
                if s.note != 0 {
                    cell.note = note_at(i32::from(s.note) + tr, 25);
                }
                cell.instr = s.instrument;
                let (fx, param) = ahx_fx(s.fx, s.param);
                let (fx, param) = if fx == Fx::None { ahx_fx(s.fx2, s.param2) } else { (fx, param) };
                (cell.fx, cell.param) = (fx, param);
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    subsong_orders(&mut m, &song.subsongs, song.positions.len(), song.restart);
    m.samples = song.instruments.iter().skip(1).map(|i| named(&i.name)).collect();
    m.message = song.instruments.iter().skip(1).map(|i| i.name.clone()).collect::<Vec<_>>().join("\n").trim_end().to_string();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// The patterns' note for an Amiga period (ProTracker's C-1, 856, is 49).
fn note_of_period(p: u16) -> u8 {
    if p == 0 {
        return NOTE_NONE;
    }
    let n = 49.0 + 12.0 * (856.0 / f64::from(p)).log2();
    (n.round() as i32).clamp(i32::from(NOTE_MIN), i32::from(NOTE_MAX)) as u8
}

/// A sample of 8-bit points from a replayer's memory.
fn amiga_points(mem: &[i8], at: usize, len: usize, loop_start: usize, loop_len: usize, name: String) -> Sample {
    let pcm: Vec<i16> = mem.get(at..at + len).unwrap_or_default().iter().map(|&x| i16::from(x) << 8).collect();
    let looped = if loop_len > 2 && loop_start < len { Loop::forward(loop_start as u32, (loop_start + loop_len).min(len) as u32) } else { Loop::default() };
    Sample { name, wave: Arc::new(Wave::new(8, 1, pcm)), c5: 8287, volume: 64, looped, ..Sample::empty() }
}

pub fn probe_fc(d: &[u8]) -> bool {
    fc::probe(d)
}

pub fn load_fc(d: &[u8]) -> Result<Module> {
    let song = fc::Song::parse(d).ok_or_else(|| Error::Bad("its header or sequences are damaged".into()))?;
    let mut m = Module {
        format: "fc".into(),
        format_name: "Future Composer".into(),
        tracker: if song.v13 { "Future Composer 1.3" } else { "Future Composer 1.4" }.into(),
        kind: Kind::Mod,
        speed: song.positions[0].speed.max(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    // The samples: each of the ten, or each of a set's twenty.
    for i in 0..10 {
        if let Some(Some(set)) = song.multi.get(i) {
            for (k, s) in set.iter().enumerate().filter(|(_, s)| s.present()) {
                m.samples.push(amiga_points(&song.mem, s.at, s.len, s.loop_start, s.loop_len, format!("Sample {} ({})", i + 1, k + 1)));
            }
        } else if song.sounds[i].present() {
            let s = song.sounds[i];
            m.samples.push(amiga_points(&song.mem, s.at, s.len, s.loop_start, s.loop_len, format!("Sample {}", i + 1)));
        }
    }
    for (p, pos) in song.positions.iter().enumerate() {
        // A pattern ends at its END mark (each channel's on its own; the first's shown).
        let rows = (0..32).find(|&r| song.row(p, 0, r).0 == 0x49).unwrap_or(32).max(1);
        let mut pat = Pattern::new(rows, 4);
        for c in 0..4 {
            let mut ended = false;
            for row in 0..rows {
                let (note, info) = song.row(p, c, row);
                if note == 0x49 {
                    ended = true;
                }
                if ended {
                    continue;
                }
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                if note & 0x7F != 0 {
                    let n = (i32::from(note & 0x7F) + i32::from(pos.transpose[c])) as u8 & 0x7F;
                    cell.note = note_of_period(fc::period(usize::from(n), song.v13).clamp(113, 3424));
                    cell.instr = ((info & 0x3F) as i8).wrapping_add(pos.sound_transpose[c]) as u8 & 0x3F;
                    cell.instr = cell.instr.saturating_add(1);
                }
                if info & 0x80 != 0 && row + 1 < 32 {
                    let v = song.row(p, c, row + 1).1;
                    (cell.fx, cell.param) = if v <= 31 { (Fx::PortaUp, v) } else { (Fx::PortaDown, v & 0x1F) };
                }
            }
        }
        if pos.speed != 0 {
            let cell = pat.cell_mut(0, 0).expect("in the pattern");
            if cell.fx == Fx::None {
                (cell.fx, cell.param) = (Fx::Speed, pos.speed);
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..song.positions.len() as u16).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_sidmon1(d: &[u8]) -> bool {
    sidmon1::probe(d)
}

pub fn load_sidmon1(d: &[u8]) -> Result<Module> {
    let song = sidmon1::Song::parse(d).ok_or_else(|| Error::Bad("its replayer or data can't be found".into()))?;
    let mut m = Module {
        format: "sid1".into(),
        format_name: "SidMon 1".into(),
        tracker: "SidMon 1".into(),
        kind: Kind::Mod,
        speed: (song.speed + 1).min(255) as u8,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let rows = (song.rows as usize).clamp(1, 256);
    for p in 0..song.positions() {
        let mut pat = Pattern::new(rows, 4);
        for c in 0..4 {
            let tr = song.sequences[c][p].transpose;
            for (at, r) in song.rows_at(p, c) {
                let Some(cell) = pat.cell_mut(at, c) else { continue };
                if r.note == -1 {
                    cell.note = NOTE_OFF;
                } else if r.note != 0 {
                    let n = i32::from(r.note.wrapping_add(tr));
                    let fine = song.instruments.get(usize::from(r.instrument).wrapping_sub(1)).map_or(0, |i| usize::from(i.finetune));
                    let per = song.periods.get((n.max(0) as usize) + fine * song.finetune_step).copied().unwrap_or(0);
                    cell.note = note_of_period(per);
                }
                cell.instr = r.instrument;
                (cell.fx, cell.param) = match r.effect {
                    0 => (Fx::None, 0),
                    2 => (Fx::Speed, r.param.saturating_add(1)),
                    3 => (Fx::None, 0),
                    _ if r.note > 0 => (Fx::TonePorta, r.param),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..song.positions() as u16).collect();
    m.samples = song.samples.iter().filter(|s| s.len > 0).map(|s| amiga_points(&song.mem, s.at, s.len, s.loop_start.unwrap_or(0), s.loop_start.map_or(0, |l| s.len - l), if s.name.is_empty() { "Sample".into() } else { s.name.clone() })).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_sidmon2(d: &[u8]) -> bool {
    sidmon2::probe(d)
}

/// SidMon 2's effects the tracker has a name for, for the screen.
fn sidmon2_fx(e: u8, a: u8) -> (Fx, u8) {
    if e < 0x70 {
        return if e != 0 && a != 0 { (Fx::TonePorta, a) } else { (Fx::None, 0) };
    }
    match e & 0x0F {
        0x0 => (Fx::Arpeggio, a),
        0x1 => (Fx::PortaUp, a),
        0x2 => (Fx::PortaDown, a),
        0x3 => (Fx::VolSlide, (a.min(15)) << 4),
        0x4 => (Fx::VolSlide, a.min(15)),
        0xC => (Fx::Volume, a.min(64)),
        0xF => (Fx::Speed, a & 0x0F),
        _ => (Fx::None, 0),
    }
}

pub fn load_sidmon2(d: &[u8]) -> Result<Module> {
    let song = sidmon2::Song::parse(d).ok_or_else(|| Error::Bad("its blocks are damaged".into()))?;
    let mut m = Module {
        format: "sid2".into(),
        format_name: "SidMon 2".into(),
        tracker: "SidMon 2".into(),
        kind: Kind::Mod,
        speed: song.speed.max(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    for p in 0..song.positions {
        let mut pat = Pattern::new(64, 4);
        for c in 0..4 {
            let step = song.sequences[c][p];
            for (note, ins, e, a, row) in song.events(step.track) {
                let Some(cell) = pat.cell_mut(row, c) else { continue };
                if note != 0 {
                    cell.note = note_of_period(sidmon2::period((i32::from(note) + i32::from(step.note_transpose)).clamp(0, 72) as usize));
                }
                if ins != 0 {
                    cell.instr = (i32::from(ins) + i32::from(step.instrument_transpose)).clamp(1, 255) as u8;
                }
                (cell.fx, cell.param) = sidmon2_fx(e, a);
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..song.positions as u16).collect();
    m.samples = song.samples.iter().map(|s| amiga_points(&song.mem, s.at, s.len, s.loop_start, s.loop_len, if s.name.is_empty() { "Sample".into() } else { s.name.clone() })).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_soundmon(d: &[u8]) -> bool {
    soundmon::probe(d)
}

pub fn load_soundmon(d: &[u8]) -> Result<Module> {
    let song = soundmon::Song::parse(d).ok_or_else(|| Error::Bad("its steps or tracks are damaged".into()))?;
    let mut m = Module {
        title: song.title.clone(),
        format: "bp".into(),
        format_name: "BP SoundMon".into(),
        tracker: if song.v22 { "BP SoundMon 2.2" } else if song.v1 { "BP SoundMon 1" } else { "BP SoundMon 2.0" }.into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    for step in 0..song.steps[0].len() {
        let mut pat = Pattern::new(16, 4);
        for c in 0..4 {
            let st = song.steps[c][step];
            for row in 0..16 {
                let r = song.row(step, c, row);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                if r.note != 0 {
                    let n = i32::from(r.note) + if r.optional != 0xA || r.data & 0xF0 == 0 { i32::from(st.transpose) } else { 0 };
                    cell.note = note_of_period(soundmon::period((n + 35).clamp(0, 83) as usize));
                    if r.instrument != 0 {
                        let k = i32::from(r.instrument) + if r.optional != 0xA || r.data & 0x0F == 0 { i32::from(st.sound_transpose) } else { 0 };
                        cell.instr = if (1..=15).contains(&k) { k as u8 } else { r.instrument };
                    }
                }
                (cell.fx, cell.param) = match r.optional {
                    0x0 if r.data != 0 => (Fx::Arpeggio, r.data),
                    0x1 => (Fx::Volume, r.data.min(64)),
                    0x2 => (Fx::Speed, r.data),
                    0x4 => (Fx::FinePortaUp, r.data),
                    0x5 => (Fx::FinePortaDown, r.data),
                    0x6 if song.v22 => (Fx::Vibrato, r.data),
                    0x7 if song.v22 => (Fx::PositionJump, r.data),
                    0x9 | 0xD | 0xE | 0xF if r.data != 0 => (Fx::Arpeggio, r.data),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..song.steps[0].len() as u16).collect();
    for ins in &song.instruments {
        m.samples.push(match ins {
            soundmon::Instrument::Sample { name, at, len, loop_start, loop_len, volume } => Sample { volume: (*volume).min(64) as u8, ..amiga_points(&song.mem, *at, *len, *loop_start, *loop_len, name.clone()) },
            soundmon::Instrument::Synth(_) => named("Synth"),
        });
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_deltamusic2(d: &[u8]) -> bool {
    deltamusic2::probe(d)
}

pub fn load_deltamusic2(d: &[u8]) -> Result<Module> {
    let song = deltamusic2::Song::parse(d).ok_or_else(|| Error::Bad("its blocks or instruments are damaged".into()))?;
    let mut m = Module {
        format: "dm2".into(),
        format_name: "Delta Music 2".into(),
        tracker: "Delta Music 2".into(),
        kind: Kind::Mod,
        speed: (song.speed.max(0) as u8).saturating_add(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    for p in 0..song.positions() {
        let mut pat = Pattern::new(16, 4);
        for c in 0..4 {
            let Some((b, tr)) = song.step_at(c, p) else { continue };
            for (row, l) in song.blocks[b].iter().enumerate() {
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                if l.note != 0 {
                    cell.note = note_of_period(deltamusic2::period((i32::from(l.note) + i32::from(tr)).clamp(0, 84) as usize));
                    cell.instr = l.instrument.saturating_add(1);
                }
                (cell.fx, cell.param) = match l.effect {
                    1 => (Fx::Speed, (l.arg & 0x0F) + 1),
                    3 => (Fx::PortaUp, l.arg),
                    4 => (Fx::PortaDown, l.arg),
                    5 => (Fx::TonePorta, l.arg),
                    6 => (Fx::Volume, l.arg & 0x3F),
                    7 => (Fx::GlobalVolume, (l.arg & 0x3F) * 2),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..song.positions() as u16).collect();
    for ins in song.instruments.iter() {
        m.samples.push(match ins {
            Some(i) if i.is_sample => amiga_points(&song.mem, i.at, i.length, i.repeat_start, i.repeat_length, format!("Sample {}", i.number + 1)),
            Some(_) => named("Synth"),
            None => named(""),
        });
    }
    while m.samples.last().is_some_and(|s| s.name.is_empty()) {
        m.samples.pop();
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_deltamusic1(d: &[u8]) -> bool {
    deltamusic1::probe(d)
}

pub fn load_deltamusic1(d: &[u8]) -> Result<Module> {
    let song = deltamusic1::Song::parse(d).ok_or_else(|| Error::Bad("its blocks or instruments are damaged".into()))?;
    let mut m = Module {
        format: "dm1".into(),
        format_name: "Delta Music 1".into(),
        tracker: "Delta Music 1".into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let walks: Vec<Vec<(usize, i8)>> = (0..4).map(|c| song.walk(c, 512)).collect();
    let n = walks.iter().map(Vec::len).max().unwrap_or(0).max(1);
    for p in 0..n {
        let mut pat = Pattern::new(16, 4);
        for c in 0..4 {
            let w = &walks[c];
            if w.is_empty() {
                continue;
            }
            let (b, tr) = w[p % w.len()];
            for (row, l) in song.blocks[b].iter().enumerate() {
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                if l.note != 0 {
                    cell.note = note_of_period(deltamusic1::period((i32::from(l.note) + i32::from(tr)).clamp(0, 83) as usize));
                    cell.instr = l.instrument.saturating_add(1);
                }
                (cell.fx, cell.param) = match l.effect {
                    0x01 if l.arg != 0 => (Fx::Speed, l.arg),
                    0x02 => (Fx::PortaUp, l.arg),
                    0x03 => (Fx::PortaDown, l.arg),
                    0x09 => (Fx::TonePorta, l.arg),
                    0x0A => (Fx::Volume, l.arg.min(64)),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..n as u16).collect();
    for ins in song.instruments.iter() {
        m.samples.push(match ins {
            Some(i) if i.is_sample => amiga_points(&song.mem, i.at, i.length.min(i.size), i.repeat_start, i.repeat_length, format!("Sample {}", i.number + 1)),
            Some(_) => named("Synth"),
            None => named(""),
        });
    }
    while m.samples.last().is_some_and(|s| s.name.is_empty()) {
        m.samples.pop();
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_mugician(d: &[u8]) -> bool {
    mugician::probe(d)
}

pub fn load_mugician(d: &[u8]) -> Result<Module> {
    let song = mugician::Song::parse(d).ok_or_else(|| Error::Bad("its songs or tracks are damaged".into()))?;
    let nch = song.channels();
    let mut m = Module {
        format: "dmu".into(),
        format_name: "Digital Mugician".into(),
        tracker: if song.v3 { "Digital Mugician 3" } else if song.v2 { "Digital Mugician 2" } else { "Digital Mugician" }.into(),
        kind: Kind::Mod,
        speed: (song.subsongs[song.playable[0]].speed & 0x0F).max(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..nch).map(|c| Channel { pan: if c >= 3 && nch == 7 { 0 } else { common_pan(c) }, ..Default::default() }).collect();
    let mut seqs = Vec::new();
    for (k, &sub) in song.playable.iter().enumerate() {
        let first = m.patterns.len() as u16;
        let len = song.subsongs[sub].length;
        for p in 0..len {
            let mut pat = Pattern::new(64, nch);
            for c in 0..nch {
                let (t, tr) = song.step(k, p, c);
                for (row, r) in song.tracks[usize::from(t)].iter().enumerate() {
                    let cell = pat.cell_mut(row, c).expect("in the pattern");
                    if r.note != 0 {
                        let fine = song.instruments.get(usize::from(r.instrument).wrapping_sub(1)).map_or(0, |i| usize::from(i.finetune));
                        let idx = (7 + i32::from(r.note) + i32::from(tr)).max(0) as usize + fine * 64;
                        cell.note = note_of_period(mugician::period(idx.min(1023)));
                        cell.instr = r.instrument;
                    }
                    (cell.fx, cell.param) = match r.effect {
                        1..=63 if r.param != 0 => (Fx::TonePorta, r.param),
                        67 if r.param != 0 && r.param <= 64 => (Fx::PatternBreak, 0),
                        68 => (Fx::Speed, r.param & 0x0F),
                        73 => (Fx::Arpeggio, r.param),
                        _ => (Fx::None, 0),
                    };
                }
            }
            m.patterns.push(Arc::new(pat));
        }
        let ss = song.subsongs[sub];
        seqs.push(Sequence { name: if song.names[sub].is_empty() { format!("Song {}", k + 1) } else { song.names[sub].clone() }, orders: (first..first + len as u16).collect(), restart: if ss.looped { ss.loop_position.min(len.saturating_sub(1)) as u16 } else { 0 } });
    }
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    m.samples = song.samples.iter().enumerate().map(|(i, s)| amiga_points(&song.mem, s.at, s.len, s.loop_start.unwrap_or(0), s.loop_start.map_or(0, |l| s.len - l), format!("Sample {}", i + 1))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_sonic(d: &[u8]) -> bool {
    sonic::probe(d)
}

pub fn load_sonic(d: &[u8]) -> Result<Module> {
    let song = sonic::Song::parse(d).ok_or_else(|| Error::Bad("its blocks are damaged".into()))?;
    let first = song.subsongs[0];
    let mut m = Module {
        format: "sa".into(),
        format_name: "Sonic Arranger".into(),
        tracker: "Sonic Arranger".into(),
        kind: Kind::Mod,
        speed: first.speed.clamp(1, 255) as u8,
        tempo: (first.tempo * 5 / 2).clamp(32, 999),
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let rows = usize::from(first.rows).clamp(1, 256);
    for p in 0..song.positions.len() {
        let mut pat = Pattern::new(rows, 4);
        for c in 0..4 {
            let (_, st, nt) = song.positions[p][c];
            for row in 0..rows {
                let l = song.line(p, c, row);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                match l.note {
                    0 => {}
                    0x7F => cell.note = NOTE_CUT,
                    0x80 => {}
                    n => {
                        let n = if l.no_note_transpose { i32::from(n) } else { i32::from(n) + i32::from(nt) };
                        cell.note = note_of_period(sonic::period(n.clamp(0, 108) as usize));
                    }
                }
                if l.instrument != 0 {
                    let k = if l.no_sound_transpose { i32::from(l.instrument) } else { i32::from(l.instrument) + i32::from(st) };
                    cell.instr = k.clamp(0, 255) as u8;
                }
                (cell.fx, cell.param) = match l.effect {
                    0x0 if l.arg != 0 => (Fx::Arpeggio, l.arg),
                    0x4 => (Fx::Vibrato, l.arg),
                    0x7 => (Fx::TonePorta, l.arg),
                    0xA => (Fx::VolSlide, if (l.arg as i8) < 0 { (l.arg as i8).unsigned_abs().min(15) } else { l.arg.min(15) << 4 }),
                    0xB => (Fx::PositionJump, l.arg),
                    0xC => (Fx::Volume, l.arg.min(64)),
                    0xD => (Fx::PatternBreak, 0),
                    0xF => (Fx::Speed, l.arg),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    let n = song.positions.len() as u16;
    let seqs: Vec<Sequence> = song
        .subsongs
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let (a, b) = (s.first.min(n - 1), s.last.min(n - 1).max(s.first.min(n - 1)));
            Sequence { name: format!("Song {}", k + 1), orders: (a..=b).collect(), restart: s.restart.saturating_sub(a).min(b - a) }
        })
        .collect();
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    let mut samples: Vec<Sample> = song.samples.iter().enumerate().map(|(i, &(at, len))| amiga_points(&song.mem, at, len, 0, 0, format!("Sample {}", i + 1))).collect();
    // Each sample's loop, as the first instrument playing it has it.
    for ins in song.instruments.iter().filter(|i| !i.synth) {
        if let Some(s) = samples.get_mut(usize::from(ins.waveform)) {
            if !s.looped.on() && ins.repeat > 1 {
                let (ls, ll) = (u32::from(ins.length) * 2, u32::from(ins.repeat) * 2);
                s.looped = Loop::forward(ls, ls + ll).clamped(s.wave.frames() as u32);
            }
            if s.name.starts_with("Sample") && !ins.name.is_empty() {
                s.name = ins.name.clone();
            }
        }
    }
    m.samples = samples;
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_instereo(d: &[u8]) -> bool {
    instereo::probe(d)
}

pub fn load_instereo(d: &[u8]) -> Result<Module> {
    let song = instereo::Song::parse(d).ok_or_else(|| Error::Bad("its blocks are damaged".into()))?;
    let mut m = instereo_layout(&song, false);
    (m.format, m.format_name, m.tracker) = ("is".into(), "InStereo!".into(), if song.v2() { "InStereo! 2.0" } else { "InStereo! 1.0" }.into());
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_synthesis(d: &[u8]) -> bool {
    synthesis::probe(d)
}

pub fn load_synthesis(d: &[u8]) -> Result<Module> {
    let song = synthesis::Song::parse(d).ok_or_else(|| Error::Bad("its blocks are damaged".into()))?;
    let mut m = instereo_layout(&song.song, true);
    (m.format, m.format_name, m.tracker) = ("syn".into(), "Synthesis".into(), if d.starts_with(b"Synth4.0") { "Synthesis 4.0" } else { "Synthesis 4.2" }.into());
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// InStereo!'s module and Synthesis's (laid out as InStereo! 1.0's).
fn instereo_layout(song: &instereo::Song, synthesis: bool) -> Module {
    let v2 = song.v2();
    let first = song.subsongs[0];
    let mut m = Module {
        title: song.title.clone(),
        kind: Kind::Mod,
        speed: first.speed.max(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let rows = usize::from(first.rows).clamp(1, 256);
    for p in 0..song.positions.len() {
        let mut pat = Pattern::new(rows, 4);
        for c in 0..4 {
            let (_, st, nt) = song.positions[p][c];
            for row in 0..rows {
                let l = song.line(p, c, row);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                match l.note {
                    0 | 0x80 => {}
                    0x7F => cell.note = NOTE_CUT,
                    n => {
                        let n = if l.no_note_transpose { i32::from(n) } else { i32::from(n) + i32::from(nt) };
                        cell.note = note_of_period(sonic::period(n.clamp(0, 108) as usize));
                    }
                }
                if l.instrument != 0 {
                    let k = if l.no_sound_transpose { i32::from(l.instrument) } else { i32::from(l.instrument) + i32::from(st) };
                    cell.instr = (k & 0x3F).max(1) as u8;
                }
                (cell.fx, cell.param) = match (v2, l.effect) {
                    _ if synthesis => match l.effect {
                        0x8 => (Fx::Speed, l.arg),
                        0xF => (Fx::Volume, l.arg.min(64)),
                        _ => (Fx::None, 0),
                    },
                    (true, 0x0) if l.arg != 0 => (Fx::Arpeggio, l.arg),
                    (true, 0x7) => (Fx::TonePorta, l.arg),
                    (true, 0xB) => (Fx::PositionJump, l.arg),
                    (true, 0xC) => (Fx::Volume, l.arg.min(64)),
                    (true, 0xD) => (Fx::PatternBreak, 0),
                    (false, 0x7) => (Fx::Volume, l.arg & 0x3F),
                    (_, 0xF) => (Fx::Speed, l.arg),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    let n = song.positions.len() as u16;
    let seqs: Vec<Sequence> = song
        .subsongs
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let (a, b) = (s.first.min(n - 1), s.last.min(n - 1).max(s.first.min(n - 1)));
            Sequence { name: format!("Song {}", k + 1), orders: (a..=b).collect(), restart: s.restart.saturating_sub(a).min(b - a) }
        })
        .collect();
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    let spans: Vec<(usize, usize)> = match &song.kind {
        instereo::Kind::V1 { samples, .. } => samples.clone(),
        instereo::Kind::V2 { data, .. } => data.clone(),
    };
    m.samples = spans.iter().enumerate().map(|(i, &(at, len))| amiga_points(&song.mem, at, len, 0, 0, song.names.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("Sample {}", i + 1)))).collect();
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// r3i6nos's corpus: this song as a file (only with NEUMETIK_CORPUS_DIR set).
    #[test]
    fn corpus_file_tiny_ahx() {
        crate::corpus::write("tiny.ahx", &tiny_ahx());
    }

    /// r3i6nos's corpus: this song as a file (only with NEUMETIK_CORPUS_DIR set).
    #[test]
    fn corpus_file_tiny_sawteeth() {
        crate::corpus::write("tiny.st", &tiny_sawteeth());
    }

    /// r3i6nos's corpus: this song as a file (only with NEUMETIK_CORPUS_DIR set).
    #[test]
    fn corpus_file_tiny_klystrack() {
        crate::corpus::write("tiny.kt", &tiny_klystrack());
    }

    /// r3i6nos's corpus: this song as a file (only with NEUMETIK_CORPUS_DIR set).
    #[test]
    fn corpus_file_tiny_v2m() {
        crate::corpus::write("tiny.v2m", &tiny_v2m());
    }

    /// A small AHX 2 song: one position, a track of two notes, one instrument.
    fn tiny_ahx() -> Vec<u8> {
        let mut d = b"THX\x01".to_vec();
        d.extend_from_slice(&[0, 0]); // the names' offset, set below
        d.extend_from_slice(&[0x00, 1, 0, 0, 4, 1, 1, 0]); // flag, positions, restart, track length, tracks, instruments, subsongs
        d.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]); // position 0: track 1 on channel 0, none elsewhere
        d.extend_from_slice(&[0; 12]); // track 0
        // Track 1: C-3 instrument 1, then nothing, E-3, nothing.
        let step = |note: u8, ins: u8| [note << 2 | ins >> 4, (ins & 0xF) << 4, 0];
        for (n, i) in [(25, 1), (0, 0), (29, 1), (0, 0)] {
            d.extend_from_slice(&step(n, i));
        }
        // Instrument 1: volume 64, a 32-point wave, a quick envelope, a play list of a triangle.
        d.extend_from_slice(&[64, 3, 1, 64, 1, 64, 10, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1]);
        d.extend_from_slice(&[0x00, 0x80, 0, 0]); // waveform 1 (triangle), no note
        let names = d.len() as u16;
        d[4..6].copy_from_slice(&names.to_be_bytes());
        d.extend_from_slice(b"tiny\0lead\0");
        d
    }

    #[test]
    fn ahx_reads_and_plays() {
        let d = tiny_ahx();
        assert!(probe_ahx(&d));
        let m = load_ahx(&d).unwrap();
        assert_eq!(m.title, "tiny");
        assert_eq!(m.samples[0].name, "lead");
        assert_eq!(m.patterns.len(), 1);
        assert_eq!(m.patterns[0].cell(0, 0).note, 49);
        assert_eq!(m.patterns[0].cell(2, 0).note, 53);
        let audio = crate::player::render(Arc::new(m.clone()), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.02, "it plays: {rms}");
        // Through a UMF: the song kept, played the same.
        let refs: Vec<_> = m.samples.iter().map(|_| None).collect();
        let bytes = crate::umf::write(&m, &refs, &[], &Default::default(), true);
        let back = crate::umf::read(&bytes, &mut |_| None).expect("reads back").module;
        assert_eq!(back.tune, m.tune);
        assert_eq!(crate::player::render(Arc::new(back), 22_050, 1.0, 0.0), audio);
    }

    /// A small Sawteeth song: one channel playing a part of a C-5 on a
    /// sawtooth, three empty steps and a G-5 that slides up.
    fn tiny_sawteeth() -> Vec<u8> {
        let mut d = b"SWTD".to_vec();
        d.extend_from_slice(&[0x04, 0xB0, 0x03, 0x72]); // version 1.2, 882 samples a tick
        d.extend_from_slice(&[1, 200, 200, 0, 1, 0, 0, 0, 0, 0, 0, 0]); // a channel: centred, one step, its loop; part 0
        d.extend_from_slice(&[1, 6, 5]); // a part: 6 ticks a step, 5 steps
        d.extend_from_slice(&[1, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x13, 67]);
        d.extend_from_slice(&[1, 1, 0, 255, 1, 0, 255, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0x81, 12]); // a sawtooth an octave up, at full level
        d.extend_from_slice(b"\0tiny\0me\0part\0saw\0");
        d
    }

    #[test]
    fn sawteeth_reads_and_plays() {
        let d = tiny_sawteeth();
        assert!(probe_sawteeth(&d));
        let m = load_sawteeth(&d).unwrap();
        assert_eq!((m.title.as_str(), m.artist.as_str()), ("tiny", "me"));
        assert_eq!(m.samples[0].name, "saw");
        assert_eq!((m.patterns.len(), m.patterns[0].rows, m.speed, m.tempo), (1, 5, 6, 125));
        assert_eq!(m.patterns[0].cell(0, 0).note, NOTE_MIDDLE_C);
        assert_eq!((m.patterns[0].cell(4, 0).note, m.patterns[0].cell(4, 0).fx), (NOTE_MIDDLE_C + 7, Fx::PortaUp));
        let audio = crate::player::render(Arc::new(m.clone()), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.02, "it plays: {rms}");
        let refs: Vec<_> = m.samples.iter().map(|_| None).collect();
        let bytes = crate::umf::write(&m, &refs, &[], &Default::default(), true);
        let back = crate::umf::read(&bytes, &mut |_| None).expect("reads back").module;
        assert_eq!(back.tune, m.tune);
        assert_eq!(crate::player::render(Arc::new(back), 22_050, 1.0, 0.0), audio);
    }

    /// A small Klystrack song: one channel, a saw instrument, a C-4 at the
    /// start of a 4-step pattern on a 16-row song.
    fn tiny_klystrack() -> Vec<u8> {
        let mut d = b"cyd!song".to_vec();
        d.extend_from_slice(&[27, 1, 4, 4, 16, 0, 1, 1, 0, 1, 0]); // version, channels, time signature, step, instruments, patterns, sequences
        d.extend_from_slice(&[16, 0, 0, 0, 128, 6, 6, 50, 0, 0, 0, 0, 3, 0]); // length, loop, volume, speeds, rate, flags, multiplex, pitch
        d.extend_from_slice(b"\x04tiny");
        d.extend_from_slice(&[0, 128, 0]); // no effects, the channel's volume and pan
        d.extend_from_slice(&[48, 0, 0, 0, 8, 0, 0, 0, 0, 0, 31, 4, 0xff, 0xff, 0, 4, 128, 0, 1, 0, 0, 0, 0, 0x80, 48, 0]);
        d.extend_from_slice(b"\x03saw");
        d.extend_from_slice(&[0xff, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        d.extend_from_slice(&[0; 13]); // no FM
        d.extend_from_slice(&[0, 0, 0, 0, 0]); // the pattern placed at row 0
        d.extend_from_slice(&[4, 0, 0, 0x30, 0x00, 48, 0, 0]); // 4 steps: C-4, instrument 0; no waves
        d
    }

    /// A small V2M: channel 1 plays a C-4 for a beat on a patch made from
    /// V2's default (a saw), the newest synth version.
    fn tiny_v2m() -> Vec<u8> {
        let mut d = Vec::new();
        for v in [96u32, 384, 0] {
            d.extend(v.to_le_bytes()); // ticks a beat, length, no tempo changes
        }
        d.extend(2u32.to_le_bytes()); // two notes: on at tick 1, off at 96
        d.extend([1, 95, 0, 0, 0, 0, 60, 0, 127, 129]);
        d.extend(1u32.to_le_bytes()); // a program change at 0: patch 0
        d.extend([0, 0, 0, 0]);
        d.extend([0u8; 4 * 8]); // no pitch bends or controllers
        d.extend([0u8; 4 * 15]); // the other channels empty
        d.extend(23u32.to_le_bytes());
        d.extend([64, 64, 32, 127, 100, 80, 64, 64, 0, 0, 64, 0, 127, 0, 0, 1, 2, 90, 32, 20, 64, 64, 0]);
        let mut patch = vec![
            64, 64, 1, 0, 64, 64, 0, 127, 0, 0, 64, 64, 32, 127, 0, 0, 64, 64, 32, 127, 1, 127, 0, 0, 64, 0, 0, 64, 0, 32, 0, 64, 0, 64, 127, 64, 80, 0, 0, 64, 127, 64, 80, 64, 1, 1, 0, 64, 2, 0, 0, 1, 1, 0, 64,
            2, 0, 127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 100, 64, 64, 64, 32, 32, 0, 0, 64, 0, 0, 1, 2, 90, 32, 20, 64, 64, 1,
        ];
        patch[37] = 127; // the amp envelope's level
        patch[59] = 100; // the channel's volume
        patch.push(0); // no modulations
        d.extend((4 + patch.len() as u32).to_le_bytes());
        d.extend(4u32.to_le_bytes());
        d.extend(patch);
        d
    }

    #[test]
    fn v2m_reads_and_plays() {
        let d = tiny_v2m();
        assert!(probe_v2m(&d));
        let m = load_v2m(&d).unwrap();
        assert_eq!((m.channels.len(), m.patterns.len(), m.patterns[0].rows, m.tempo), (16, 1, 64, 120));
        assert_eq!((m.patterns[0].cell(0, 0).note, m.patterns[0].cell(0, 0).instr), (NOTE_MIDDLE_C, 1));
        assert_eq!(m.patterns[0].cell(4, 0).note, NOTE_OFF);
        assert_eq!(m.samples.len(), 1);
        let audio = crate::player::render(Arc::new(m.clone()), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.02, "it plays: {rms}");
        let refs: Vec<_> = m.samples.iter().map(|_| None).collect();
        let bytes = crate::umf::write(&m, &refs, &[], &Default::default(), true);
        let back = crate::umf::read(&bytes, &mut |_| None).expect("reads back").module;
        assert_eq!(back.tune, m.tune);
        assert_eq!(crate::player::render(Arc::new(back), 22_050, 1.0, 0.0), audio);
    }

    #[test]
    fn mmlgb_mml_compiles_and_plays() {
        let src = b"; a scale\nA t120 o4 l8 c d e f\nB o3 c1\n";
        assert!(probe_mmlgb(src));
        assert!(!probe_mmlgb(b"A song for you\n"));
        assert!(!probe_mmlgb(b"#title \"x\"\r\n#opll_mode 0\r\n"));
        let m = load_mmlgb(src).unwrap();
        assert_eq!((m.format.as_str(), m.channels.len()), ("mmlgb", 4));
        assert!(load_mmlgb(b"A c5\n").is_err());
    }

    #[test]
    fn mds_sound_data_loads() {
        let Ok(dir) = std::env::var("MDS_DIR") else { return };
        let d = std::fs::read(format!("{dir}/idk.mds")).unwrap();
        assert!(probe_mds(&d));
        let m = load_mds(&d).unwrap();
        assert_eq!((m.format.as_str(), m.channels.len()), ("mds", 10));
        assert!(m.patterns.len() > 10);
    }

    #[test]
    fn mdsdrv_mml_compiles_and_plays() {
        let src = b"#platform mdsdrv\n@1 fm 4 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n@2 psg 15 10 / 5>0:5\nA @1 t120 v15 o4 l8 cdefgab>c4\nG @2 v15 o4 l8 cdefgab>c4\n";
        assert!(probe_mdsmml(src));
        assert!(!probe_mdsmml(b"A cdefgab\n"));
        let m = crate::formats::load(src, "scale.mml").unwrap();
        assert_eq!((m.format.as_str(), m.channels.len()), ("mds", 10));
        let audio = crate::player::render(Arc::new(m), 22_050, 2.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.01, "it plays: {rms}");
        let e = crate::formats::load(b"#platform mdsdrv\nA c d\nB E1\n", "bad.mml");
        assert!(e.is_err());
    }

    #[test]
    fn mgsc_mml_compiles_and_plays() {
        let src = b"; a scale\r\n#opll_mode 0\r\n#title \"Scale\"\r\n#tempo 150\r\n1 v13o4l8 cdefgab>c4\r\n9 v13@0o4l8 cdefgab>c4\r\n";
        assert!(probe_mgsc(src));
        assert!(!probe_mgsc(b"#title \"x\"\r\n#opll_mode 0\r\n"));
        let m = crate::formats::load(src, "scale.mus").unwrap();
        assert_eq!((m.format.as_str(), m.title.as_str()), ("mgsc", "Scale"));
        assert!(m.tune.is_some());
        let audio = crate::player::render(Arc::new(m), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.01, "it plays: {rms}");
        // MGSC's error, with its line.
        let e = crate::formats::load(b"#opll_mode 0\r\n1 c128\r\n", "bad.mus").unwrap_err();
        assert!(e.to_string().contains("line 2: Invalid length"), "{e}");
    }

    #[test]
    fn nsd_mml_and_its_nsfe_play_with_their_song_names() {
        let src = b"#Title \"Two\"\r\n#Text \"notes\"\r\n#BGM 2\r\nBGM(0) {\r\n #Label \"Opening\"\r\n TR1 t120 o4 l8 v15 cdefg\r\n}\r\nBGM(1) {\r\n TR1 t120 o4 l8 v15 gfedc\r\n}\r\n";
        let m = crate::formats::load(src, "two.mml").unwrap();
        assert_eq!((m.format.as_str(), m.title.as_str()), ("nsd", "Two"));
        assert_eq!(m.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Opening", "Song 2"]);
        assert!(m.message.contains("notes"), "{}", m.message);
        let o = neumetik_synth::nsc::compile(src).unwrap();
        let e = neumetik_synth::nsc::link::nsfe(&o).unwrap();
        assert!(probe_nsf(&e) && !probe_nsd(&e));
        let m = crate::formats::load(&e, "two.nsfe").unwrap();
        assert_eq!((m.format_name.as_str(), m.title.as_str(), m.sequences[0].name.as_str()), ("NSFe", "Two", "Opening"));
        let audio = crate::player::render(Arc::new(m), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.01, "it plays: {rms}");
    }

    #[test]
    fn klystrack_reads_and_plays() {
        let d = tiny_klystrack();
        assert!(probe_klystrack(&d));
        let m = load_klystrack(&d).unwrap();
        assert_eq!(m.title, "tiny");
        assert_eq!(m.samples[0].name, "saw");
        assert_eq!((m.patterns.len(), m.patterns[0].rows, m.speed, m.tempo), (1, 16, 6, 125));
        assert_eq!((m.patterns[0].cell(0, 0).note, m.patterns[0].cell(0, 0).instr), (NOTE_MIDDLE_C - 12, 1));
        assert_eq!(m.patterns[0].cell(4, 0).note, NOTE_NONE);
        let audio = crate::player::render(Arc::new(m.clone()), 22_050, 1.0, 0.0);
        let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
        assert!(rms > 0.02, "it plays: {rms}");
        let refs: Vec<_> = m.samples.iter().map(|_| None).collect();
        let bytes = crate::umf::write(&m, &refs, &[], &Default::default(), true);
        let back = crate::umf::read(&bytes, &mut |_| None).expect("reads back").module;
        assert_eq!(back.tune, m.tune);
        assert_eq!(crate::player::render(Arc::new(back), 22_050, 1.0, 0.0), audio);
    }
}

pub fn probe_musicassembler(d: &[u8]) -> bool {
    musicassembler::probe(d)
}

pub fn load_musicassembler(d: &[u8]) -> Result<Module> {
    let song = musicassembler::Song::parse(d).ok_or_else(|| Error::Bad("its replayer or data can't be found".into()))?;
    let mut m = Module {
        format: "ma".into(),
        format_name: "Music Assembler".into(),
        tracker: "Music Assembler".into(),
        kind: Kind::Mod,
        speed: song.subsongs[0].speed.max(1),
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let mut seqs = Vec::new();
    for sub in 0..song.subsongs.len() {
        let first = m.patterns.len() as u16;
        let steps: Vec<Vec<(usize, u8)>> = (0..4).map(|c| song.steps(sub, c)).collect();
        let n = steps[0].len().max(1);
        let mut instrument = [0usize; 4];
        for p in 0..n {
            let rows: Vec<Vec<musicassembler::Row>> = steps.iter().map(|s| if s.is_empty() { Vec::new() } else { song.rows(s[p % s.len()].0) }).collect();
            let len = rows.iter().map(|r| r.iter().map(|x| usize::from(x.delay) + 1).sum::<usize>()).max().unwrap_or(1).clamp(1, 256);
            let mut pat = Pattern::new(len, 4);
            for c in 0..4 {
                let Some(&(_, tr)) = steps[c].get(p % steps[c].len().max(1)) else { continue };
                let mut at = 0;
                for r in &rows[c] {
                    if let Some(k) = r.instrument {
                        instrument[c] = usize::from(k).min(song.instruments.len() - 1);
                    }
                    if let Some(cell) = pat.cell_mut(at, c) {
                        let ins = &song.instruments[instrument[c]];
                        let smp = usize::from(ins.sample).min(song.samples.len() - 1);
                        if r.release {
                            cell.note = NOTE_OFF;
                        } else if let Some(nt) = r.note {
                            let key = if song.samples[smp].synth() { 0 } else { ins.key_wave_rate };
                            let i = usize::from(nt.wrapping_add(tr).wrapping_add(key)).min(47);
                            cell.note = note_of_period(musicassembler::PERIODS[i]);
                            cell.instr = (smp + 1) as u8;
                            (cell.fx, cell.param) = match r.portamento {
                                0 => (Fx::None, 0),
                                x if x & 0x80 != 0 => (Fx::PortaDown, x & 0x7F),
                                x => (Fx::PortaUp, x),
                            };
                        }
                    }
                    at += usize::from(r.delay) + 1;
                }
            }
            m.patterns.push(Arc::new(pat));
        }
        seqs.push(Sequence { name: if sub == 0 { "Song".into() } else { format!("Song {}", sub + 1) }, orders: (first..first + n as u16).collect(), restart: 0 });
    }
    m.orders = seqs[0].orders.clone();
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    m.samples = song
        .samples
        .iter()
        .map(|s| {
            let name = if s.name.chars().all(|c| c == '-') { String::new() } else { s.name.clone() };
            if s.synth() || s.len <= 1 {
                named(&name)
            } else {
                let lp = if s.loop_len != 0 { (s.len - s.loop_len.min(s.len)) * 2 } else { 0 };
                amiga_points(&song.mem, s.at, s.len * 2, lp, if s.loop_len != 0 { s.loop_len.min(s.len) * 2 } else { 0 }, name)
            }
        })
        .collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_voodoo(d: &[u8]) -> bool {
    voodoo::probe(d)
}

pub fn load_voodoo(d: &[u8]) -> Result<Module> {
    let song = voodoo::Song::parse(d).ok_or_else(|| Error::Bad("its tracks or tables are damaged".into()))?;
    let (grain, _) = song.rows(0);
    let mut m = Module {
        format: "vss".into(),
        format_name: "Voodoo Supreme Synthesizer".into(),
        tracker: "Voodoo Supreme Synthesizer".into(),
        kind: Kind::Mod,
        speed: grain as u8,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let mut seqs = Vec::new();
    for sub in 0..song.subsongs.len() {
        let (grain, rows) = song.rows(sub);
        let n = rows.div_ceil(64).min(256);
        let first = m.patterns.len();
        let mut pats: Vec<Pattern> = (0..n).map(|_| Pattern::new(64, 4)).collect();
        for c in 0..4 {
            for (t, note, _) in song.timeline(sub, c, 100_000) {
                let r = t / grain;
                let Some(cell) = pats.get_mut(r / 64).and_then(|p| p.cell_mut(r % 64, c)) else { continue };
                cell.note = if note == 0xFF { NOTE_CUT } else { note_of_period(voodoo::period(usize::from(note))) };
            }
        }
        m.patterns.extend(pats.into_iter().map(Arc::new));
        seqs.push(Sequence { name: format!("Song {}", sub + 1), orders: (first as u16..(first + n) as u16).collect(), restart: 0 });
    }
    m.orders = seqs[0].orders.clone();
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    // Its samples, each once, in the file's order.
    let mut seen: Vec<(usize, usize)> = song.subsongs.iter().flat_map(|s| s.items.iter()).filter_map(|i| if let voodoo::Item::Sample(at, len) = i { Some((*at, *len)) } else { None }).collect();
    seen.sort_unstable();
    seen.dedup();
    m.samples = seen.iter().enumerate().map(|(k, &(at, len))| amiga_points(&song.mem, at, len, 0, 0, format!("Sample {}", k + 1))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_sawteeth(d: &[u8]) -> bool {
    sawteeth::probe(d)
}

/// Sawteeth's effects the tracker has a name for, for the screen.
fn sawteeth_fx(e: u8) -> (Fx, u8) {
    let x = e & 0xF;
    match e >> 4 {
        0x1 => (Fx::PortaUp, x + 1),
        0x2 => (Fx::PortaDown, x + 1),
        0x3 => (Fx::TonePorta, x + 1),
        0xA => (Fx::VolSlide, x + 1),
        0xB => (Fx::VolSlide, (x + 1) << 4),
        0xC => (Fx::Volume, (u16::from(x) * 64 / 15) as u8),
        _ => (Fx::None, 0),
    }
}

pub fn load_sawteeth(d: &[u8]) -> Result<Module> {
    let song = sawteeth::Song::parse(d).ok_or_else(|| Error::Bad("its channels, parts or instruments are damaged".into()))?;
    let nch = song.channels.len();
    let bpm = (110_250.0 / f64::from(song.spspal)).round() as u16;
    let mut m = Module {
        title: song.title.clone(),
        artist: song.author.clone(),
        format: "st".into(),
        format_name: "Sawteeth".into(),
        tracker: format!("Sawteeth {}.{}", song.version / 1000, song.version % 1000 / 100),
        kind: Kind::Mod,
        speed: song.windows[0].grain.min(255) as u8,
        tempo: bpm,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = song
        .channels
        .iter()
        .map(|c| {
            let (l, r) = (u32::from(c.left), u32::from(c.right));
            Channel { pan: (r * 255).checked_div(l + r).unwrap_or(128) as u8, ..Default::default() }
        })
        .collect();
    let starts = song.starts();
    let shown = usize::from(song.channels[0].rloop) + 1;
    let end = starts[shown - 1] + song.windows[shown - 1].ticks;
    let mut pats: Vec<Pattern> = song.windows[..shown].iter().map(|w| Pattern::new(w.rows(), nch)).collect();
    for c in 0..nch {
        for e in song.timeline(c, end) {
            // The first channel's step it falls in.
            let p = starts[..shown].partition_point(|&s| s <= e.tick) - 1;
            let row = ((e.tick - starts[p]) / song.windows[p].grain) as usize;
            let Some(cell) = pats[p].cell_mut(row, c) else { continue };
            if !cell.is_empty() {
                continue;
            }
            if e.step.note != 0 {
                cell.note = note_at(sawteeth::Song::note(e.step.note, e.transpose), 1);
            }
            cell.instr = e.step.ins;
            (cell.fx, cell.param) = sawteeth_fx(e.step.eff);
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = song.channels[0].lloop.min(shown as u16 - 1);
    m.samples = song.instruments[1..].iter().map(|i| named(&i.name)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_monotone(d: &[u8]) -> bool {
    monotone::probe(d)
}

pub fn load_monotone(d: &[u8]) -> Result<Module> {
    let song = monotone::Song::parse(d).ok_or_else(|| Error::Bad("its header or order list is damaged".into()))?;
    let nch = song.tracks;
    let mut m = Module {
        title: song.title.clone(),
        message: song.comment.clone(),
        format: "mon".into(),
        format_name: "MONOTONE".into(),
        tracker: "MONOTONE".into(),
        kind: Kind::Mod,
        speed: song.start_speed(),
        // Its ticks are 60 a second.
        tempo: 150,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..nch).map(|_| Channel { pan: 128, ..Default::default() }).collect();
    for p in 0..song.patterns.len() {
        let mut pat = Pattern::new(64, nch);
        for row in 0..64 {
            for c in 0..nch {
                let s = song.cell(p, row, c);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                // Its A0 (note 1) is the tracker's A-0, its C-4 the tracker's C-4.
                cell.note = match s.note {
                    0 => NOTE_NONE,
                    127 => NOTE_OFF,
                    n => note_at(i32::from(n), 10),
                };
                (cell.fx, cell.param) = match s.effect {
                    0x0 if s.raw => (Fx::Arpeggio, s.param),
                    0x1 => (Fx::PortaUp, s.param),
                    0x2 => (Fx::PortaDown, s.param),
                    0x3 => (Fx::TonePorta, s.param),
                    0x4 => (Fx::Vibrato, s.param),
                    0xB => (Fx::PositionJump, s.param),
                    0xD => (Fx::PatternBreak, s.param),
                    0xF => (Fx::Speed, s.param.max(1)),
                    _ => (Fx::None, 0),
                };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = song.orders.iter().map(|&o| u16::from(o)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_klystrack(d: &[u8]) -> bool {
    klystrack::probe(d)
}

/// Klystrack's commands the tracker has a name for, for the screen.
fn klystrack_fx(c: u16) -> (Fx, u8) {
    let p = (c & 0xff) as u8;
    match c >> 8 {
        0x00 if p != 0 => (Fx::Arpeggio, p),
        0x01 => (Fx::PortaUp, p),
        0x02 => (Fx::PortaDown, p),
        0x03 => (Fx::TonePorta, p),
        0x04 => (Fx::Vibrato, p),
        0x0A => (Fx::VolSlide, p),
        0x0C => (Fx::Volume, (u16::from(p.min(0x80)) / 2) as u8),
        0x0E => (Fx::ModEx, p),
        0x0F => (Fx::Speed, (p & 0xf).max(1)),
        0x18 => (Fx::Panning, (u16::from(p.min(0x80)) * 255 / 128) as u8),
        0x1D => (Fx::GlobalVolume, p.min(0x80)),
        _ => (Fx::None, 0),
    }
}

pub fn load_klystrack(d: &[u8]) -> Result<Module> {
    let song = klystrack::Song::parse(d).ok_or_else(|| Error::Bad("its header is damaged or it has more than 32 channels".into()))?;
    let nch = song.channels();
    let (speed, _, rate) = song.speeds();
    let (length, restart) = song.length();
    let rows = klystrack::rows_per_pattern(&song);
    let ts = song.time_signature();
    let (beat, measure) = (ts >> 8, ts & 0xff);
    let mut m = Module {
        title: song.title().to_string(),
        format: "kt".into(),
        format_name: "Klystrack".into(),
        tracker: "Klystrack".into(),
        kind: Kind::Mod,
        speed: speed.max(1),
        // Its rate is ticks a second; 50 is 125 beats a minute.
        tempo: (u16::from(rate.max(1)) * 5 / 2).max(1),
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    if beat > 0 && measure > 0 {
        m.time = TimeSig { rows_per_beat: beat, rows_per_measure: beat * measure };
    }
    m.channels = (0..nch).map(|c| Channel { pan: ((i32::from(song.panning(c)) + 64).clamp(0, 128) * 255 / 128) as u8, ..Default::default() }).collect();
    let len = usize::from(length).max(1);
    let shown = len.div_ceil(rows);
    let mut pats: Vec<Pattern> = (0..shown).map(|p| Pattern::new(rows.min(len - p * rows), nch)).collect();
    for (c, seq) in song.sequences().iter().enumerate().take(nch) {
        // Each row plays the last pattern placed at or before it, from where it was placed.
        let mut at = 0;
        for row in 0..len {
            while at < seq.len() && usize::from(seq[at].position) <= row {
                at += 1;
            }
            let Some(placed) = at.checked_sub(1).map(|i| seq[i]) else { continue };
            let Some(pat) = song.patterns().get(usize::from(placed.pattern)) else { continue };
            let Some(st) = pat.steps.get(row - usize::from(placed.position)) else { continue };
            let Some(cell) = pats[row / rows].cell_mut(row % rows, c) else { continue };
            cell.note = match st.note {
                klystrack::NOTE_NONE => NOTE_NONE,
                klystrack::NOTE_RELEASE => NOTE_OFF,
                n if n < 0xf0 => note_at(i32::from(n) + i32::from(placed.note_offset) + 1, 1),
                _ => NOTE_NONE,
            };
            if st.instrument != 0xff {
                cell.instr = st.instrument.wrapping_add(1);
            }
            match st.volume {
                v @ 0..=0x80 => (cell.vol, cell.volp) = (Vol::Volume, v / 2),
                v if v & 0xf0 == 0xa0 => (cell.vol, cell.volp) = (Vol::Panning, (v & 0xf) * 64 / 15),
                _ => {}
            }
            (cell.fx, cell.param) = klystrack_fx(st.command);
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = (usize::from(restart) / rows).min(shown - 1) as u16;
    m.samples = song.instruments().iter().map(|i| named(&i.name)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_v2m(d: &[u8]) -> bool {
    v2::probe(d)
}

pub fn load_v2m(d: &[u8]) -> Result<Module> {
    let song = v2::Song::parse(d).ok_or_else(|| Error::Bad("its sequence or patches are damaged".into()))?;
    let (timediv, maxtime) = song.timing();
    let td = u64::from(timediv.max(1));
    let tempo = song.tempos().first().filter(|t| t.0 == 0).map_or(500_000, |t| t.1.max(1));
    let events: Vec<Vec<(u32, v2::Event)>> = (0..16).map(|c| song.events(c)).collect();
    let last = events.iter().flatten().map(|e| e.0).max().unwrap_or(0).max(maxtime);
    let rows = ((u64::from(last) * 4).div_ceil(td) as usize).max(1);
    let shown = rows.div_ceil(v2::ROWS);
    let lyrics = song.lyrics();
    let mut m = Module {
        format: "v2m".into(),
        format_name: "V2".into(),
        tracker: "V2 (farbrausch)".into(),
        message: if lyrics.is_empty() { String::new() } else { format!("Lyrics:\n{}", lyrics.join("\n")) },
        kind: Kind::Mod,
        speed: 6,
        // A row a sixteenth note: at speed 6 the tempo is the beats a minute.
        tempo: (60_000_000 / tempo).clamp(32, 999) as u16,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..16).map(|_| Channel { pan: 128, ..Default::default() }).collect();
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(v2::ROWS, 16)).collect();
    for (c, evs) in events.iter().enumerate() {
        let mut program = 0u8;
        for &(t, e) in evs {
            let row = (u64::from(t) * 4 / td) as usize;
            if row >= rows {
                continue;
            }
            match e {
                v2::Event::Program(p) => program = p,
                v2::Event::Note(n, vel) => {
                    let Some(cell) = pats[row / v2::ROWS].cell_mut(row % v2::ROWS, c) else { continue };
                    if vel > 0 {
                        if cell.note != NOTE_NONE && cell.note != NOTE_OFF {
                            continue;
                        }
                        // MIDI's middle C (60) is the tracker's.
                        cell.note = note_at(i32::from(n) + 1, 1);
                        cell.instr = (program & 0x7f) + 1;
                        (cell.vol, cell.volp) = (Vol::Volume, vel.min(127) / 2);
                    } else if cell.note == NOTE_NONE {
                        cell.note = NOTE_OFF;
                    }
                }
            }
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.samples = (0..song.patches()).map(|p| named(&format!("Patch {}", p + 1))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_sunvox(d: &[u8]) -> bool {
    sunvox::is_sunvox(d)
}

pub fn load_sunvox(d: &[u8]) -> Result<Module> {
    let song = sunvox::Song::parse(d).ok_or_else(|| Error::Bad("its patterns or modules are damaged".into()))?;
    let s = song.summary();
    let view = sunvox::VIEW_LINES;
    let lines = s.lines.max(1);
    let shown = (((lines + view - 1) / view) as usize).min(1000);
    let nch = s.channels.len().max(1);
    let mut m = Module {
        title: s.name.clone(),
        format: "sunvox".into(),
        format_name: "SunVox".into(),
        tracker: "SunVox".into(),
        kind: Kind::Mod,
        speed: s.speed.max(1),
        tempo: s.bpm.clamp(32, 999),
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = s.channels.iter().map(|name| Channel { pan: 128, name: name.clone(), ..Default::default() }).collect();
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(view as usize, nch)).collect();
    for n in &s.notes {
        if n.line < 0 || n.line >= lines || n.note == 0 {
            continue;
        }
        let (p, r) = ((n.line / view) as usize, (n.line % view) as usize);
        let Some(cell) = pats.get_mut(p).and_then(|pat| pat.cell_mut(r, n.channel)) else { continue };
        if cell.note != NOTE_NONE {
            continue;
        }
        match n.note {
            // Its C0 (note 1) is 16.35 Hz: MIDI's 12.
            1..=127 => {
                cell.note = note_at(i32::from(n.note) + 12, 1);
                cell.instr = n.module.saturating_sub(1).min(255) as u8;
                if n.vel > 0 {
                    (cell.vol, cell.volp) = (Vol::Volume, ((n.vel - 1) / 2).min(64));
                }
            }
            128 => cell.note = NOTE_OFF,
            _ => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = ((s.restart.max(0) / view) as u16).min(shown as u16 - 1);
    m.samples = s.modules.iter().skip(1).map(|name| named(name)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// Whether a zip's files are a Renoise song's.
pub fn is_xrns(files: &[(String, Vec<u8>)]) -> bool {
    files.iter().any(|(n, _)| n == "Song.xml")
}

/// A Renoise song from its zip's files.
pub fn load_xrns(files: &[(String, Vec<u8>)]) -> Result<Module> {
    let kept: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter(|(n, _)| {
            let n = n.replace('\\', "/");
            n == "Song.xml" || (n.starts_with("SampleData/") && !n.ends_with('/'))
        })
        .cloned()
        .collect();
    let song = renoise::Song::parse(&renoise::bundle(&kept)).ok_or_else(|| Error::Bad("its Song.xml is damaged".into()))?;
    let tracker = format!("Renoise (song version {})", song.model.doc);
    Ok(renoise_module(song, "xrns", "Renoise", tracker))
}

pub fn probe_rns(d: &[u8]) -> bool {
    renoise::rns::is_rns(d)
}

/// A Renoise 1 song (.rns, Renoise 0.5 to 1.28).
pub fn load_rns(d: &[u8]) -> Result<Module> {
    let song = renoise::Song::parse(d).ok_or_else(|| Error::Bad("it's a Renoise 1 song this version of its format isn't read in, or it's damaged".into()))?;
    let version: String = d[3..d.len().min(7)].iter().take_while(|c| c.is_ascii_digit()).map(|&c| c as char).collect();
    let version = version.trim_start_matches('0');
    Ok(renoise_module(song, "rns", "Renoise 1", format!("Renoise 1 (file version {})", if version.is_empty() { "0" } else { version })))
}

/// The tracker's view of a Renoise song: its tracks as channels, its
/// samples and instruments, its patterns as far as a cell shows them.
fn renoise_module(song: renoise::Song, format: &str, format_name: &str, tracker: String) -> Module {
    let r = song.model.clone();
    let tracks: Vec<usize> = (0..r.tracks.len()).filter(|&t| r.tracks[t].kind == renoise::song::TrackKind::Normal).collect();
    let nch = tracks.len().max(1);
    let mut m = Module {
        title: r.name.clone(),
        artist: r.artist.clone(),
        message: r.comments.join("\n").trim().to_string(),
        format: format.into(),
        format_name: format_name.into(),
        tracker,
        kind: Kind::It,
        speed: r.tpl.clamp(1, 31) as u8,
        tempo: (r.bpm.round() as u16).clamp(32, 999),
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.time.rows_per_beat = r.lpb.clamp(1, 64) as u16;
    m.time.rows_per_measure = m.time.rows_per_beat * 4;
    m.channels = tracks.iter().map(|&t| Channel { pan: 128, name: r.tracks[t].name.clone(), ..Default::default() }).collect();
    // Every sample, with its audio; each instrument maps its notes to them.
    for ins in &r.instruments {
        let first = m.samples.len();
        for s in &ins.samples {
            let a = &s.audio;
            let frames = a.frames();
            let mut pcm = Vec::with_capacity(frames * a.channels.len().min(2));
            for i in 0..frames {
                for c in a.channels.iter().take(2) {
                    pcm.push((c[i].clamp(-1.0, 1.0) * 32767.0) as i16);
                }
            }
            let looped = match s.loop_mode {
                renoise::song::Loop::Forward | renoise::song::Loop::Backward => Loop::forward(s.loop_start as u32, s.loop_end as u32),
                renoise::song::Loop::PingPong => Loop { kind: LoopKind::PingPong, start: s.loop_start as u32, end: s.loop_end as u32 },
                renoise::song::Loop::Off => Loop::default(),
            };
            m.samples.push(Sample {
                name: s.name.clone(),
                wave: Arc::new(Wave::new(16, a.channels.len().clamp(1, 2) as u8, pcm)),
                c5: a.rate.max(1),
                volume: (s.volume * 64.0).round().clamp(0.0, 64.0) as u8,
                pan: Some((s.pan * 255.0).round().clamp(0.0, 255.0) as u8),
                looped,
                slices: s.slices.iter().map(|&x| x as u32).collect(),
                ..Sample::empty()
            });
        }
        let mut inst = Instrument { name: ins.name.clone(), keymap: vec![(0, 0); 120], global_volume: 128, ..Default::default() };
        for (n, slot) in inst.keymap.iter_mut().enumerate() {
            *slot = (n as u8, 0);
        }
        for z in ins.zones.iter().filter(|z| !z.off_layer) {
            for n in z.notes.0..=z.notes.1.min(119) {
                // The sample at its own rate on its base note: C-5 here.
                let played = if z.key_to_pitch { i32::from(n) } else { z.base_note } + 60 - z.base_note;
                inst.keymap[usize::from(n)] = (played.clamp(0, 119) as u8, (first + z.sample + 1).min(usize::from(u16::MAX)) as u16);
            }
        }
        m.instruments.push(inst);
    }
    for p in 0..r.patterns.len() {
        let lines = r.patterns[p].lines;
        let mut pat = Pattern::new(lines, nch);
        for (k, &t) in tracks.iter().enumerate() {
            let Some(pt) = r.pattern_track(p, t) else { continue };
            for (at, line) in &pt.lines {
                let Some(cell) = pat.cell_mut(*at, k) else { continue };
                // The line's first note there (or its first instrument or volume).
                let pick = line.notes.iter().find(|c| c.note != renoise::song::NONE).or_else(|| line.notes.iter().find(|c| c.instr != renoise::song::NONE));
                if let Some(c) = pick {
                    cell.note = match c.note {
                        renoise::song::OFF => NOTE_OFF,
                        renoise::song::NONE => NOTE_NONE,
                        n => n + 1,
                    };
                    if c.instr != renoise::song::NONE {
                        cell.instr = c.instr.saturating_add(1);
                    }
                    if let renoise::song::Col::Set(v) = c.vol {
                        (cell.vol, cell.volp) = (Vol::Volume, (v / 2).min(64));
                    }
                }
                // Tempo and speed changes, shown.
                for fx in &line.fx {
                    let (fxk, param) = match (r.doc >= 37, fx.num) {
                        (true, [b'Z', b'T']) | (false, [b'F', b'0']) => (Fx::Tempo, fx.val),
                        (true, [b'Z', b'K']) | (false, [b'F', b'2']) if r.doc > 10 || fx.num[0] == b'Z' => (Fx::Speed, fx.val),
                        (false, [b'F', b'1']) if r.doc <= 10 => (Fx::Speed, fx.val),
                        (true, [b'Z', b'B']) | (false, [b'F', b'B']) => (Fx::PatternBreak, fx.val),
                        _ => continue,
                    };
                    (cell.fx, cell.param) = (fxk, param);
                    break;
                }
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = r.sequence.iter().map(|e| e.pattern.min(usize::from(ORDER_SKIP) - 1) as u16).collect();
    for (o, e) in r.sequence.iter().enumerate() {
        for &t in &e.muted {
            if let Some(k) = tracks.iter().position(|&x| x == t) {
                m.slot_mutes.push((o as u16, k as u16));
            }
        }
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    m
}

pub fn probe_pretracker(d: &[u8]) -> bool {
    pretracker::probe(d)
}

/// Pretracker's commands the tracker has a name for, for the screen.
fn pretracker_fx(cmd: u8, p: u8) -> (Fx, u8) {
    match cmd {
        0x1 => (Fx::PortaUp, p),
        0x2 => (Fx::PortaDown, p),
        0x3 => (Fx::TonePorta, p),
        0x4 => (Fx::Vibrato, p),
        0xA => (Fx::VolSlide, p),
        0xB => (Fx::PositionJump, p),
        0xC => (Fx::Volume, p.min(64)),
        0xD => (Fx::PatternBreak, p),
        0xE if p >> 4 == 0xD => (Fx::ModEx, p),
        // Above $2F a shuffle: even rows' ticks, then odd rows'.
        0xF if p <= 0x2f => (Fx::Speed, p),
        0xF => (Fx::Speed, p >> 4),
        _ => (Fx::None, 0),
    }
}

pub fn load_pretracker(d: &[u8]) -> Result<Module> {
    let song = pretracker::Song::parse(d).ok_or_else(|| Error::Bad("its header, instruments or waves are damaged".into()))?;
    let steps = usize::from(song.steps);
    let mut m = Module {
        title: song.title.clone(),
        artist: song.author.clone(),
        format: "prt".into(),
        format_name: "Pretracker".into(),
        tracker: if song.version == 0x1e { "Pretracker 1.5" } else { "Pretracker" }.into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    for p in 0..usize::from(song.length) {
        let mut pat = Pattern::new(steps, 4);
        for c in 0..4 {
            let (n, transpose) = song.position(p, c);
            if n == 0 {
                continue;
            }
            for row in 0..steps {
                let [b0, b1, b2] = song.step(n, row);
                let cell = pat.cell_mut(row, c).expect("in the pattern");
                // Its pitch 1 (period 848) is ProTracker's C-1.
                cell.note = match b0 & 0x3f {
                    0 => NOTE_NONE,
                    0x3d => NOTE_OFF,
                    x => note_at(i32::from(x) + i32::from(transpose), 49),
                };
                cell.instr = (b1 >> 4) | ((b0 & 0x80) >> 3);
                (cell.fx, cell.param) = if b0 & 0x40 != 0 { (Fx::Arpeggio, b2) } else { pretracker_fx(b1 & 15, b2) };
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..u16::from(song.length)).collect();
    m.restart = u16::from(song.restart).min(u16::from(song.length) - 1);
    m.samples = song.instrument_names.iter().map(|n| named(n)).collect();
    let waves: Vec<&str> = song.wave_names.iter().map(String::as_str).filter(|n| !n.is_empty()).collect();
    if !waves.is_empty() {
        m.message = format!("Waves:\n{}", waves.join("\n"));
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_organya(d: &[u8]) -> bool {
    organya::probe(d)
}

pub fn load_organya(d: &[u8]) -> Result<Module> {
    let song = organya::Song::parse(d).ok_or_else(|| Error::Bad("its notes run past the end of the file".into()))?;
    let measure = song.measure();
    // Its end, at least a tick; past 65536 ticks (55 minutes at its fastest
    // usual pace) it plays on unshown.
    let end = (song.end.max(1) as usize).min(65_536);
    let wait = f64::from(song.wait.max(1));
    let speed = (32.0 * wait / 2500.0).ceil().clamp(1.0, 31.0);
    let mut m = Module {
        format: "org".into(),
        format_name: "Organya".into(),
        tracker: match song.version {
            1 => "Organya",
            2 => "Organya 2",
            _ => "Organya 2 (new drums)",
        }
        .into(),
        kind: Kind::Mod,
        // A row is a tick, `wait` ms: as ticks of 2.5/tempo s, as few as keep the tempo 32 or more.
        speed: speed as u8,
        tempo: (2500.0 * speed / wait).round().min(999.0) as u16,
        // The editor's grid: `dot` ticks a beat, `line` beats a measure.
        time: TimeSig { rows_per_beat: u16::from(song.dot.max(1)).min(measure as u16), rows_per_measure: measure as u16 },
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    const NAMES: [&str; organya::TRACKS] = ["1", "2", "3", "4", "5", "6", "7", "8", "Q", "W", "E", "R", "T", "Y", "U", "I"];
    m.channels = NAMES.iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    let mut patterns: Vec<Pattern> = (0..end.div_ceil(measure)).map(|p| Pattern::new(measure.min(end - p * measure), organya::TRACKS)).collect();
    for (t, track) in song.tracks.iter().enumerate() {
        for (i, n) in track.notes.iter().enumerate() {
            let Some(x) = usize::try_from(n.x).ok().filter(|&x| x < end) else { continue };
            let Some(cell) = patterns[x / measure].cell_mut(x % measure, t) else { continue };
            if n.key != organya::DUMMY {
                // Its key 0 is C1 (32.7 Hz), the tracker's C-2.
                cell.note = note_at(i32::from(n.key) + 1, 25);
                cell.instr = t as u8 + 1;
            }
            if n.volume != organya::DUMMY {
                // DirectSound's decibels, shown as the amplitude they give.
                cell.vol = Vol::Volume;
                cell.volp = (64.0 * 10f64.powf(f64::from((i32::from(n.volume) - 255) * 8) / 2000.0)).round() as u8;
            }
            if n.pan != organya::DUMMY {
                (cell.fx, cell.param) = (Fx::Panning, (i32::from(organya::PANS[usize::from(n.pan.min(12))]) * 255 / 512) as u8);
            }
            // A held melody note let go before the next shows where.
            if t < organya::MELODY && n.key != organya::DUMMY && !track.pizzicato {
                let off = x + usize::from(n.length);
                let next = track.notes[i + 1..].iter().find(|n| n.key != organya::DUMMY).map_or(usize::MAX, |n| usize::try_from(n.x).unwrap_or(0));
                if off < next.min(end) {
                    if let Some(c) = patterns[off / measure].cell_mut(off % measure, t) {
                        if c.note == NOTE_NONE {
                            c.note = NOTE_OFF;
                        }
                    }
                }
            }
        }
    }
    m.patterns = patterns.into_iter().map(Arc::new).collect();
    m.orders = (0..m.patterns.len() as u16).collect();
    m.restart = ((song.repeat.max(0) as usize).min(end - 1) / measure) as u16;
    m.samples = song
        .tracks
        .iter()
        .enumerate()
        .map(|(t, tr)| {
            if t < organya::MELODY {
                named(&format!("Wave {:02}{}", tr.wave, if tr.pizzicato { " (pizzicato)" } else { "" }))
            } else {
                named(song.drum_name(t).unwrap_or("(no drum)"))
            }
        })
        .collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_fcbsi(d: &[u8]) -> bool {
    fcbsi::probe(d)
}

pub fn load_fcbsi(d: &[u8]) -> Result<Module> {
    let song = fcbsi::Song::parse(d).ok_or_else(|| Error::Bad("its tracks or blocks are cut short".into()))?;
    let (grain, rows) = song.rows();
    let mut m = Module {
        format: "bsi".into(),
        format_name: "Future Composer BSI".into(),
        tracker: "Future Composer BSI".into(),
        kind: Kind::Mod,
        // Two vertical blanks a step to start with (its tempo 2 is a step every third).
        speed: (3 * grain).min(31) as u8,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..4).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let n = rows.div_ceil(64).min(256);
    let mut pats: Vec<Pattern> = (0..n).map(|p| Pattern::new(64.min(rows - p * 64), 4)).collect();
    for (c, line) in song.timeline(100_000).iter().enumerate() {
        for &(t, note, digi, _) in line {
            let r = t / grain;
            let Some(cell) = pats.get_mut(r / 64).and_then(|p| p.cell_mut(r % 64, c)) else { continue };
            cell.note = note_of_period(fcbsi::period(usize::from(note)));
            cell.instr = digi + 1;
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..n as u16).collect();
    m.samples = song
        .digis
        .iter()
        .map(|g| {
            let name: String = g.name.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { ' ' }).collect::<String>().trim().to_string();
            if g.len == 0 {
                return named(&name);
            }
            let (ls, ll) = if g.loop_len != 0 { (g.loop_start as usize, g.loop_len as usize) } else { (0, 0) };
            amiga_points(&song.mem, g.at as usize, (g.len as usize).min(song.mem.len().saturating_sub(g.at as usize)), ls, ll, name)
        })
        .collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_pxtone(d: &[u8]) -> bool {
    pxtone::probe(d)
}

pub fn load_pxtone(d: &[u8]) -> Result<Module> {
    let song = pxtone::Song::parse(d).ok_or_else(|| Error::Bad("its events, voices or effects are damaged".into()))?;
    let m0 = song.master;
    let beat_clock = m0.beat_clock.max(1);
    let rows_per_beat = 4;
    let rows_per_meas = (m0.beat_num.max(1) * rows_per_beat) as usize;
    let meas_clock = i64::from(beat_clock) * i64::from(m0.beat_num.max(1));
    let measures = (m0.play_meas().max(1) as usize).min(1000);
    let units = song.units.len().max(1);
    let mut m = Module {
        title: pxtone::sjis_lossy(&song.name),
        message: pxtone::sjis_lossy(&song.comment),
        format: "ptcop".into(),
        format_name: "pxtone (Piston Collage)".into(),
        tracker: if song.project { "Piston Collage" } else { "Piston Collage (tune)" }.into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: (m0.beat_tempo.round() as i64).clamp(32, 999) as u16,
        time: TimeSig { rows_per_beat: rows_per_beat as u16, rows_per_measure: rows_per_meas as u16 },
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..units).map(|u| Channel { pan: 128, name: song.units.get(u).map_or(String::new(), |x| x.name.clone()), ..Default::default() }).collect();
    let mut pats: Vec<Pattern> = (0..measures).map(|_| Pattern::new(rows_per_meas, units)).collect();
    let row_of = |clock: i32| -> Option<(usize, usize)> {
        let r = (i64::from(clock.max(0)) * i64::from(rows_per_beat) / i64::from(beat_clock)) as usize;
        let p = r / rows_per_meas;
        (p < measures && i64::from(clock) < measures as i64 * meas_clock).then_some((p, r % rows_per_meas))
    };
    let mut key = vec![0x6000i32; units];
    let mut voice = vec![0u8; units];
    let mut velocity = vec![104i32; units];
    let events = song.events();
    for (i, &(clock, unit, kind, value)) in events.iter().enumerate() {
        let u = usize::from(unit);
        if u >= units {
            continue;
        }
        match kind {
            2 => key[u] = value,
            12 => voice[u] = value.clamp(0, 254) as u8,
            4 => velocity[u] = value,
            1 => {
                // Keys and voices set at the same clock come before the note.
                for &(c2, u2, k2, v2) in &events[i + 1..] {
                    if c2 != clock {
                        break;
                    }
                    if usize::from(u2) == u {
                        match k2 {
                            2 => key[u] = v2,
                            12 => voice[u] = v2.clamp(0, 254) as u8,
                            4 => velocity[u] = v2,
                            _ => {}
                        }
                    }
                }
                let Some((p, r)) = row_of(clock) else { continue };
                if let Some(cell) = pats[p].cell_mut(r, u) {
                    // Its key in 256ths of a semitone, 0x4500 the A of 440 Hz: MIDI's numbers.
                    cell.note = note_at(key[u] / 256 + 1, 1);
                    cell.instr = voice[u] + 1;
                    (cell.vol, cell.volp) = (Vol::Volume, (velocity[u].clamp(0, 128) / 2) as u8);
                }
                // Let go before the next note shows where.
                let off = clock.saturating_add(value);
                let next = events[i + 1..].iter().find(|e| usize::from(e.1) == u && e.2 == 1).map_or(i32::MAX, |e| e.0);
                if off < next {
                    if let Some((p, r)) = row_of(off) {
                        if let Some(c) = pats[p].cell_mut(r, u) {
                            if c.note == NOTE_NONE {
                                c.note = NOTE_OFF;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..measures as u16).collect();
    m.restart = (m0.repeat_meas.max(0) as u16).min(measures as u16 - 1);
    m.samples = song
        .woices
        .iter()
        .enumerate()
        .map(|(w, wo)| {
            let name = if wo.name.is_empty() { format!("Voice {}", w + 1) } else { wo.name.clone() };
            match song.pcm_points(w) {
                Some(pcm) if !pcm.is_empty() => Sample { name, wave: Arc::new(Wave::new(16, 1, pcm)), c5: 44100, volume: 64, ..Sample::empty() },
                _ => named(&name),
            }
        })
        .collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_pt3(d: &[u8]) -> bool {
    pt3::probe(d)
}

pub fn load_pt3(d: &[u8]) -> Result<Module> {
    let song = pt3::Song::parse(d).ok_or_else(|| Error::Bad("its positions or patterns are damaged".into()))?;
    let mut m = Module {
        title: song.name.clone(),
        artist: song.author.clone(),
        format: "pt3".into(),
        format_name: "Pro Tracker 3".into(),
        tracker: if d.starts_with(b"Vortex") { "Vortex Tracker II".to_string() } else { format!("Pro Tracker 3.{}", song.version) },
        kind: Kind::Mod,
        speed: song.tempo.clamp(1, 31),
        // A frame 1/50 s, as at speed 6 tempo 125.
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = ["A", "B", "C"].iter().enumerate().map(|(c, n)| Channel { pan: [64, 128, 192][c], name: n.to_string(), ..Default::default() }).collect();
    let used = song.positions.iter().copied().max().unwrap_or(0) + 1;
    for p in 0..used {
        let rows = song.pattern(p);
        let mut pat = Pattern::new(rows.len().max(1), 3);
        for (r, row) in rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let Some(x) = pat.cell_mut(r, c) else { continue };
                match cell.note {
                    Some(0xFF) => x.note = NOTE_OFF,
                    // Its first note is C-1 (32.7 Hz on a 1.75 MHz AY), the tracker's C-2.
                    Some(n) => x.note = note_at(i32::from(n) + 1, 25),
                    None => {}
                }
                if let Some(s) = cell.sample {
                    x.instr = s;
                }
                if let Some(v) = cell.volume {
                    (x.vol, x.volp) = (Vol::Volume, v * 64 / 15);
                }
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = song.positions.iter().map(|&p| p as u16).collect();
    m.restart = song.loop_pos as u16;
    m.samples = (1..32).map(|s| named(&format!("Sample {s}"))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

fn file_ext(name: &str) -> String {
    let n = name.rsplit(['/', '\\']).next().unwrap_or(name);
    n.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

pub fn probe_pmd(d: &[u8], name: &str) -> bool {
    matches!(file_ext(name).as_str(), "m" | "m2" | "mz") && pc98::probe_pmd(d)
}

pub fn probe_fmp(d: &[u8], name: &str) -> bool {
    matches!(file_ext(name).as_str(), "opi" | "ovi" | "ozi") && pc98::probe_fmp(d)
}

pub fn load_pmd(d: &[u8], companion: Companion) -> Result<Module> {
    load_pc98(d, pc98::Which::Pmd, companion)
}

pub fn load_fmp(d: &[u8], companion: Companion) -> Result<Module> {
    load_pc98(d, pc98::Which::Fmp, companion)
}

fn load_pc98(d: &[u8], which: pc98::Which, companion: Companion) -> Result<Module> {
    // The PCM files it asks for, by the name it gives them, when they're beside it.
    let mut pcm = Vec::new();
    for (kind, name, exts) in pc98::Song::wanted(which, d) {
        let found = exts.iter().find_map(|ext| {
            let n = format!("{name}.{ext}");
            [n.clone(), n.to_ascii_uppercase(), n.to_ascii_lowercase()].iter().find_map(|n| companion(n))
        });
        if let Some(b) = found {
            pcm.push((kind, b));
        }
    }
    let song = pc98::Song::new(which, d, pcm).ok_or_else(|| Error::Bad("its header is damaged".into()))?;
    let tl = song.timeline();
    let (format, format_name, tracker) = match which {
        pc98::Which::Pmd => ("pmd", "PMD", "PMD (Professional Music Driver)"),
        pc98::Which::Fmp => ("fmp", "FMP", "FMP (PC-98 FM driver)"),
    };
    let mut m = Module { format: format.into(), format_name: format_name.into(), tracker: tracker.into(), kind: Kind::Mod, speed: 6, tempo: 125, global_volume: 128, mix_volume: 32, separation: 128, ..Default::default() };
    // A line of its text: without the escape sequences that colour it on the PC-98's screen.
    let line = |b: &[u8]| {
        let mut plain = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == 0x1b {
                i += 1;
                if b.get(i) == Some(&b'[') {
                    while i < b.len() && !b[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                }
            } else {
                plain.push(b[i]);
            }
            i += 1;
        }
        let s: String = pxtone::sjis_lossy(&plain).chars().filter(|c| !c.is_control()).collect();
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    match which {
        pc98::Which::Pmd => {
            if let Some(p) = neumetik_synth::pmd::Pmd::load(d) {
                m.title = p.memo(1).map(line).unwrap_or_default();
                m.artist = p.memo(2).map(line).unwrap_or_default();
                let memo: Vec<String> = (3..64).map_while(|i| p.memo(i).map(line)).filter(|l| !l.is_empty()).collect();
                m.message = memo.join("\n");
            }
        }
        pc98::Which::Fmp => {
            if let Some(mut f) = neumetik_synth::fmp::Fmp::load(d) {
                let mut w = neumetik_synth::fmwork::Work::dummy();
                f.init(&mut w);
                let lines: Vec<String> = f.comment.iter().map(|l| line(l)).filter(|l| !l.is_empty()).collect();
                m.title = lines.first().cloned().unwrap_or_default();
                m.message = lines.join("\n");
            }
        }
    }
    let channels = song.channels();
    m.channels = pc98::CHANNEL_NAMES[..channels].iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    // A pattern a measure (at most 999 of them shown).
    let shown = tl.measures.min(999);
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(16, channels)).collect();
    for e in &tl.events {
        let Some(pat) = pats.get_mut(e.measure) else { continue };
        let Some(cell) = pat.cell_mut(e.row, e.channel) else { continue };
        match e.note {
            Some(n) => {
                // MIDI's middle C (60) is the tracker's.
                cell.note = note_at(i32::from(n) + 1, 1);
            }
            None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
            None => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = tl.restart.filter(|&r| r < shown).unwrap_or(0) as u16;
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_stone(d: &[u8]) -> bool {
    stonetracker::probe(d)
}

/// StoneTracker's effects the tracker has a name for, for the screen
/// (its 10 to 1E are ProTracker's E0 to EE).
fn stone_fx(e: u8, p: u8) -> (Fx, u8) {
    match e {
        0x00 if p != 0 => (Fx::Arpeggio, p),
        0x01 => (Fx::PortaUp, p),
        0x02 => (Fx::PortaDown, p),
        0x03 => (Fx::TonePorta, p),
        0x04 => (Fx::Vibrato, p),
        0x05 => (Fx::TonePortaVol, p),
        0x06 => (Fx::VibratoVol, p),
        0x07 => (Fx::Tremolo, p),
        0x09 => (Fx::Offset, p),
        0x0A => (Fx::VolSlide, p),
        0x0B => (Fx::PositionJump, p),
        0x0C => (Fx::Volume, p.min(64)),
        0x0D => (Fx::PatternBreak, p),
        0x0F if (1..32).contains(&p) => (Fx::Speed, p),
        0x0F if p >= 32 => (Fx::Tempo, p),
        0x10..=0x1E if p < 16 => (Fx::ModEx, (e - 0x10) << 4 | p),
        _ => (Fx::None, 0),
    }
}

/// A StoneTracker module, its sample bank beside it ("SPS." for
/// "SPM.", ".sps" for ".spm").
pub fn load_stone(d: &[u8], name: &str, companion: Companion) -> Result<Module> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut wanted = Vec::new();
    if base.len() > 4 && base[..4].eq_ignore_ascii_case("spm.") {
        let rest = &base[4..];
        wanted.extend([format!("SPS.{rest}"), format!("sps.{rest}")]);
    }
    if let Some(stem) = base.strip_suffix(".spm").or_else(|| base.strip_suffix(".SPM")) {
        wanted.extend([format!("{stem}.sps"), format!("{stem}.SPS")]);
    }
    let bank = wanted.iter().find_map(|n| companion(n)).ok_or_else(|| Error::Bad("its sample bank (the SPS. file beside it) isn't there".into()))?;
    let song = stonetracker::Song::from_files(d, &bank).ok_or_else(|| Error::Bad("its songs, patterns or sample bank are damaged".into()))?;
    let channels = song.songs.iter().map(|s| s.voices).max().unwrap_or(4);
    let mut m = Module {
        title: song.name.clone(),
        format: "stone".into(),
        format_name: "StoneTracker".into(),
        tracker: "StoneTracker".into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: u16::from(song.songs.first().map_or(125, |s| s.bpm)).max(32),
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..channels).map(|c| Channel { pan: common_pan(c), ..Default::default() }).collect();
    let mut sequences = Vec::new();
    for (k, s) in song.songs.iter().enumerate() {
        let first = m.patterns.len() as u16;
        for p in 0..s.positions() {
            let mut pat = Pattern::new(song.lines, channels);
            let fx_line = |line: usize| -> Option<&stonetracker::Line> {
                let n = match &s.fx {
                    Some(l) => *l.get(p)?,
                    None => 0,
                };
                song.fx_patterns.get(usize::from(n))?.get(line)
            };
            for t in 0..s.voices {
                let Some(lines) = s.tracks[t].get(p).and_then(|&n| song.pattern(n)) else { continue };
                for (r, l) in lines.iter().enumerate() {
                    let Some(cell) = pat.cell_mut(r, t) else { continue };
                    if l.note != 0 {
                        cell.note = note_at(i32::from(l.note), 49);
                    }
                    cell.instr = l.inst;
                    // A volume in the volume column, the first other effect the tracker names beside it.
                    let mut fx = l.fx.iter().copied().chain(fx_line(r).into_iter().flat_map(|f| f.fx.iter().copied()));
                    if let Some(&(_, v)) = l.fx.iter().find(|f| f.0 == 0x0C) {
                        (cell.vol, cell.volp) = (Vol::Volume, v.min(64));
                    }
                    if let Some((f, v)) = fx.find_map(|(e, p)| (e != 0x0C || cell.vol == Vol::None).then(|| stone_fx(e, p)).filter(|x| x.0 != Fx::None)) {
                        (cell.fx, cell.param) = (f, v);
                    }
                }
            }
            m.patterns.push(Arc::new(pat));
        }
        let orders: Vec<u16> = (first..m.patterns.len() as u16).collect();
        let name = if s.name.is_empty() { format!("Song {}", k + 1) } else { s.name.clone() };
        sequences.push(Sequence { name, orders, restart: 0 });
    }
    m.orders = sequences.first().map(|s| s.orders.clone()).unwrap_or_default();
    if sequences.len() > 1 {
        m.sequences = sequences;
    }
    m.samples = song
        .samples
        .iter()
        .map(|s| {
            let pts: Vec<i16> = song.data.get(s.start..(s.start + s.len).min(song.data.len())).unwrap_or(&[]).iter().map(|&x| i16::from(x) << 8).collect();
            let frames = pts.len() as u32;
            let ls = s.loop_start.saturating_sub(s.start) as u32;
            let looped = if s.loop_len > 2 && s.loop_start >= s.start { Loop::forward(ls, ls + s.loop_len as u32).clamped(frames) } else { Loop::default() };
            Sample { name: s.name.clone(), wave: Arc::new(Wave::new(8, 1, pts)), c5: (8363.0 * 2f64.powf(f64::from(s.finetune) / 96.0)).round() as u32, volume: s.volume, looped, ..Sample::empty() }
        })
        .collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_goattracker(d: &[u8]) -> bool {
    goattracker::probe(d)
}

pub fn load_goattracker(d: &[u8]) -> Result<Module> {
    let song = goattracker::Song::parse(d).ok_or_else(|| Error::Bad("its order lists or patterns are damaged".into()))?;
    let mut m = gt_module(&song.name, &song.author, &song.copyright, "gt2", "GoatTracker 2");
    gt_layout(&mut m, song.subtunes(), |sub| song.timeline(sub, 50 * 60 * 15));
    m.samples = song.ins_names.iter().map(|n| named(n)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_goattracker1(d: &[u8]) -> bool {
    goattracker1::probe(d)
}

pub fn load_goattracker1(d: &[u8]) -> Result<Module> {
    let song = goattracker1::Song::parse(d).ok_or_else(|| Error::Bad("its order lists or patterns are damaged".into()))?;
    let mut m = gt_module(&song.name, &song.author, &song.copyright, "gt1", "GoatTracker 1");
    gt_layout(&mut m, song.subtunes(), |sub| song.timeline(sub, 50 * 60 * 15));
    m.samples = song.ins_names.iter().map(|n| named(n)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

fn gt_module(title: &str, author: &str, copyright: &str, format: &str, name: &str) -> Module {
    let mut m = Module {
        title: title.to_string(),
        artist: author.to_string(),
        message: copyright.to_string(),
        format: format.into(),
        format_name: name.into(),
        tracker: name.into(),
        kind: Kind::Mod,
        speed: 6,
        // A frame 1/50 s, as at speed 6 tempo 125.
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (1..=3).map(|c| Channel { pan: 128, name: format!("Voice {c}"), ..Default::default() }).collect();
    m
}

// Each subtune followed without the SID: a pattern each of the first
// channel's, the others' notes laid on its rows; a sequence a subtune.
fn gt_layout(m: &mut Module, subtunes: usize, timeline: impl Fn(usize) -> goattracker::Timeline) {
    let mut seqs = Vec::new();
    for sub in 0..subtunes {
        let tl = timeline(sub);
        let first = m.patterns.len();
        let mut starts = tl.pattern_starts();
        let restart = tl.restart_row.map_or(0, |row| starts.iter().rposition(|&s| s <= row).unwrap_or(0));
        starts.push(tl.rows.len());
        let mut row_of = vec![(0usize, 0usize); tl.rows.len()];
        let mut pats = Vec::new();
        for w in starts.windows(2) {
            let k = first + pats.len();
            for i in w[0]..w[1] {
                row_of[i] = (k, i - w[0]);
            }
            pats.push(Pattern::new((w[1] - w[0]).max(1), 3));
        }
        for &(row, c, note) in &tl.notes {
            let (k, r) = row_of[row];
            let Some(cell) = pats[k - first].cell_mut(r, c) else { continue };
            match note {
                // Its C-0 is MIDI's 12; MIDI's middle C (60) is the tracker's.
                Some(n) => cell.note = note_at(i32::from(n) + 13, 1),
                None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
                None => {}
            }
        }
        m.patterns.extend(pats.into_iter().map(Arc::new));
        let orders: Vec<u16> = (first..m.patterns.len()).map(|p| p as u16).collect();
        seqs.push(Sequence { name: if sub == 0 { "Song".into() } else { format!("Song {}", sub + 1) }, orders, restart: restart as u16 });
        if m.patterns.len() > 4000 {
            break;
        }
    }
    m.orders = seqs.first().map(|s| s.orders.clone()).unwrap_or_default();
    m.restart = seqs.first().map_or(0, |s| s.restart);
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
}

pub fn probe_mgs(d: &[u8]) -> bool {
    d.starts_with(b"MGS") && mgs::Song::parse(d).is_some()
}

/// MGSC's MML source (`.mus`): text whose first line that isn't blank or a
/// comment is `#opll_mode`, which MGSC wants before anything else.
pub fn probe_mgsc(d: &[u8]) -> bool {
    d.split(|&b| b == b'\n')
        .map(|l| {
            let l = l.split(|&b| b == b';').next().unwrap_or(l);
            let s = l.iter().position(|b| !b.is_ascii_whitespace()).map_or(&l[l.len()..], |i| &l[i..]);
            s
        })
        .find(|l| !l.is_empty())
        .is_some_and(|l| l.len() >= 10 && l[..10].eq_ignore_ascii_case(b"#opll_mode"))
}

/// MGSC's MML, compiled as MGSC does and played as the `.mgs` it makes.
pub fn load_mgsc(d: &[u8]) -> Result<Module> {
    let mgs = mgsc::compile(d).map_err(|e| {
        Error::Bad(match e.line {
            Some(l) => format!("MGSC stops at line {}: {}", l + 1, e.what),
            None => format!("MGSC stops: {}", e.what),
        })
    })?;
    let mut m = load_mgs(&mgs)?;
    m.format = "mgsc".into();
    m.format_name = "MGSC MML".into();
    m.tracker = "MGSC (MGSDRV 3.13)".into();
    Ok(m)
}

pub fn load_mgs(d: &[u8]) -> Result<Module> {
    let song = mgs::Song::parse(d).ok_or_else(|| Error::Bad("its tracks are damaged".into()))?;
    let tl = song.timeline();
    // The title as MGSC wrote it: Shift JIS, to the end of its line.
    let head = &d[8.min(d.len())..];
    let end = head.iter().position(|&b| b == 0x1a || b == 0x0d || b == 0x0a).unwrap_or(head.len());
    let title: String = pxtone::sjis_lossy(&head[..end]).chars().filter(|c| !c.is_control()).collect();
    let mut m = Module {
        title: title.split_whitespace().collect::<Vec<_>>().join(" "),
        format: "mgs".into(),
        format_name: "MGSDRV".into(),
        tracker: format!("MGSDRV (MGSC {}.{:02})", song.version / 100, song.version % 100),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let channels = mgs::CHANNEL_NAMES.len();
    m.channels = mgs::CHANNEL_NAMES.iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    // A pattern a whole note (192 steps), a row a sixteenth; at most 999 shown.
    let shown = (tl.steps.div_ceil(192) as usize).clamp(1, 999);
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(16, channels)).collect();
    for &(step, c, note) in &tl.events {
        let s = step.saturating_sub(1);
        let Some(pat) = pats.get_mut((s / 192) as usize) else { continue };
        let Some(cell) = pat.cell_mut(((s % 192) / 12) as usize, c) else { continue };
        match note {
            // MIDI's middle C (60) is the tracker's.
            Some(n) => cell.note = note_at(i32::from(n) + 1, 1),
            None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
            None => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = tl.restart.map(|s| (s / 192) as usize).filter(|&r| r < shown).unwrap_or(0) as u16;
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// MDSDRV's MML (the `#platform mdsdrv` dialect of ctrmml): text that names the platform or defines an
/// `fm`/`2op`/`psg`/`pcm` instrument, and compiles.
pub fn probe_mdsmml(d: &[u8]) -> bool {
    if d.len() > 1 << 20 || d.iter().take(4096).any(|&b| b == 0) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(d) else { return false };
    mdsmml::looks_like(text) && mdsmml::compile(text).is_ok()
}

/// MDSDRV's MML, compiled to the `.mds` data the driver plays (that is what the song keeps).
pub fn load_mdsmml(d: &[u8]) -> Result<Module> {
    let text = String::from_utf8_lossy(d);
    let mds = mdsmml::compile(&text).map_err(|e| Error::Bad(format!("MDSDRV MML: {e}")))?;
    load_mds(&mds)
}

/// mmlgb's MML (Game Boy): text whose lines are channel letters (`A` to `D`) and commands, or `@wave` and `@@`
/// definitions, and that mmlgb's parser accepts with something to play.
pub fn probe_mmlgb(d: &[u8]) -> bool {
    if d.len() > 1 << 20 || d.iter().take(4096).any(|&b| b == 0) {
        return false;
    }
    // The first thing that isn't blank or a comment.
    let first = d.split(|&b| b == b'\n').map(|l| l.split(|&b| b == b';').next().unwrap_or(l)).find_map(|l| l.iter().find(|b| !b.is_ascii_whitespace()).copied());
    if !matches!(first, Some(b'A'..=b'D' | b'@')) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(d) else { return false };
    match mmlgb::Song::from_mml(text) {
        // Something besides the four end marks.
        Ok(song) => song.data().len() > 18,
        Err(_) => false,
    }
}

/// mmlgb's MML, compiled as mmlgb does and played by its driver on the Game Boy's sound chip.
pub fn load_mmlgb(d: &[u8]) -> Result<Module> {
    let text = String::from_utf8_lossy(d);
    let song = mmlgb::Song::from_mml(&text).map_err(|e| Error::Bad(format!("mmlgb stops at {e}")))?;
    let tl = song.timeline();
    let mut m = Module {
        format: "mmlgb".into(),
        format_name: "mmlgb MML".into(),
        tracker: "mmlgb (Game Boy)".into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let channels = mmlgb::CHANNEL_NAMES.len();
    m.channels = mmlgb::CHANNEL_NAMES.iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    // A pattern a bar (192 steps), a row a sixteenth; at most 999 shown.
    let shown = (tl.steps.div_ceil(192) as usize).clamp(1, 999);
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(16, channels)).collect();
    for &(step, c, note) in &tl.events {
        let s = step.saturating_sub(1);
        let Some(pat) = pats.get_mut((s / 192) as usize) else { continue };
        let Some(cell) = pat.cell_mut(((s % 192) / 12) as usize, c) else { continue };
        match note {
            // MIDI's middle C (60) is the tracker's.
            Some(n) => cell.note = note_at(i32::from(n) + 1, 1),
            None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
            None => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = tl.restart.map(|s| (s / 192) as usize).filter(|&r| r < shown).unwrap_or(0) as u16;
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_mds(d: &[u8]) -> bool {
    d.starts_with(b"RIFF") && d.get(8..12) == Some(b"MDS0")
}

/// MDSDRV sound data (`.mds`, Mega Drive): the driver's own sequence run on a YM2612 and an SN76489 (its PCM channels on the DAC), the notes shown as a pattern a bar (96 steps), a row a sixteenth.
pub fn load_mds(d: &[u8]) -> Result<Module> {
    let song = mdsdrv::Song::parse(d).map_err(|e| Error::Bad(format!("MDSDRV: {e}")))?;
    let tl = song.timeline();
    let mut m = Module {
        format: "mds".into(),
        format_name: "MDSDRV".into(),
        tracker: "MDSDRV (Mega Drive)".into(),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let channels = mdsdrv::CHANNEL_NAMES.len();
    m.channels = mdsdrv::CHANNEL_NAMES.iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    let shown = (tl.steps.div_ceil(96) as usize).clamp(1, 999);
    let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(16, channels)).collect();
    for &(step, c, note) in &tl.events {
        let s = step.saturating_sub(1);
        let Some(pat) = pats.get_mut((s / 96) as usize) else { continue };
        let Some(cell) = pat.cell_mut(((s % 96) / 6) as usize, c) else { continue };
        match note {
            Some(n) => cell.note = note_at(i32::from(n) + 1, 1),
            None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
            None => {}
        }
    }
    m.patterns = pats.into_iter().map(Arc::new).collect();
    m.orders = (0..shown as u16).collect();
    m.restart = tl.restart.map(|s| (s / 96) as usize).filter(|&r| r < shown).unwrap_or(0) as u16;
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_nsf(d: &[u8]) -> bool {
    d.starts_with(b"NESM\x1a") || (d.starts_with(b"NSFE") && d.get(8..12) == Some(b"INFO"))
}

/// Frames of each NSF song played to find its notes, at most (ten minutes at 60 a second).
const NSF_FRAMES: u32 = 36_000;

/// An NSF or NSFe: its own driver run, a song a sequence, its notes read from the sound registers (a row each 6
/// frames).
pub fn load_nsf(d: &[u8]) -> Result<Module> {
    let song = nsf::Song::parse(d).map_err(Error::Bad)?;
    let mut m = nsf_module(song)?;
    m.format = "nsf".into();
    m.format_name = if d.starts_with(b"NSFE") { "NSFe" } else { "NSF" }.into();
    m.tracker = "NSF (the game's own driver)".into();
    Ok(m)
}

fn nsf_module(song: nsf::Song) -> Result<Module> {
    let n = song.nsf.clone();
    let mut message = n.copyright.clone();
    for (what, s) in [("Ripped by", &n.ripper), ("", &n.text)] {
        if !s.is_empty() {
            message = format!("{message}{}{}{s}", if message.is_empty() { "" } else { "\n\n" }, if what.is_empty() { String::new() } else { format!("{what} ") });
        }
    }
    let mut m = Module {
        title: n.title.clone(),
        artist: n.artist.clone(),
        message,
        kind: Kind::Mod,
        speed: 6,
        tempo: 150,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let mut seqs = Vec::new();
    for s in 0..n.songs {
        let t = nsf::timeline(&n, s, NSF_FRAMES);
        if m.channels.is_empty() {
            m.channels = t.chans.iter().map(|c| Channel { pan: 128, name: c.name(), ..Default::default() }).collect();
        }
        let rows = (t.frames.div_ceil(6) as usize).max(1);
        let first = m.patterns.len();
        let shown = rows.div_ceil(64).min(999);
        let mut pats: Vec<Pattern> = (0..shown).map(|_| Pattern::new(64, t.chans.len())).collect();
        for &(frame, c, note) in &t.events {
            let row = (frame / 6) as usize;
            let Some(pat) = pats.get_mut(row / 64) else { continue };
            let Some(cell) = pat.cell_mut(row % 64, c) else { continue };
            match note {
                // MIDI's middle C (60) is the tracker's.
                Some(k) => cell.note = note_at(i32::from(k) + 1, 1),
                None if cell.note == NOTE_NONE => cell.note = NOTE_OFF,
                None => {}
            }
        }
        m.patterns.extend(pats.into_iter().map(Arc::new));
        let orders: Vec<u16> = (first..first + shown).map(|p| p as u16).collect();
        let restart = t.restart.map_or(0, |f| (f / 6 / 64) as usize).min(shown - 1);
        let name = match n.tracks.get(s).map(|t| t.name.as_str()) {
            Some(l) if !l.is_empty() => l.to_string(),
            _ if s == 0 => "Song".into(),
            _ => format!("Song {}", s + 1),
        };
        seqs.push(Sequence { name, orders, restart: restart as u16 });
    }
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// NSD.Lib's MML: a BGM block (`BGM(n) {`, or its Japanese `音楽（`), and nsc compiles it.
pub fn probe_nsd(d: &[u8]) -> bool {
    let has = |pat: &[u8]| d.windows(pat.len()).any(|w| w.eq_ignore_ascii_case(pat));
    let bgm = d.windows(4).enumerate().any(|(i, w)| w.eq_ignore_ascii_case(b"BGM(") || (w.eq_ignore_ascii_case(b"BGM ") && d[i + 4..].iter().find(|b| **b != b' ') == Some(&b'(')));
    // 音楽（ in Shift JIS.
    (bgm || has(b"\x89\xb9\x8a\x79\x81\x69")) && !d.starts_with(b"NESM") && !d.starts_with(b"NSFE")
}

/// NSD.Lib's MML, compiled as nsc does, linked with NSD.Lib's driver, and played as the NSF that makes.
/// Its DPCM samples and `#include`s are read from beside it (nsc's paths, `\` or `/`, relative to the MML).
pub fn load_nsd(d: &[u8], companion: Companion) -> Result<Module> {
    let reader = |f: &str| companion(&f.replace('\\', "/"));
    let o = nsc::compile_with(d, &reader).map_err(|e| Error::Bad(format!("nsc stops: {e}")))?;
    let file = nsc::link::nsf(&o).map_err(|e| Error::Bad(format!("nsc can't link it: {e}")))?;
    let song = nsf::Song::parse(&file).map_err(Error::Bad)?;
    let mut m = nsf_module(song)?;
    m.format = "nsd".into();
    m.format_name = "NSD.Lib MML".into();
    m.tracker = "NSD.Lib (nsc 1.31)".into();
    if !o.warnings.is_empty() {
        m.message = format!("{}{}nsc's warnings:\n{}", m.message, if m.message.is_empty() { "" } else { "\n\n" }, o.warnings.join("\n"));
    }
    Ok(m)
}

pub fn probe_victracker(d: &[u8]) -> bool {
    victracker::probe(d)
}

pub fn load_victracker(d: &[u8]) -> Result<Module> {
    let song = victracker::Song::parse(d).ok_or_else(|| Error::Bad("its header is damaged".into()))?;
    let mut m = Module {
        title: song.title.clone(),
        artist: song.author.clone(),
        message: song.year.clone(),
        format: "vt".into(),
        format_name: "VIC-TRACKER".into(),
        tracker: format!("VIC-TRACKER {}.{}", song.version.0, song.version.1),
        kind: Kind::Mod,
        speed: 6,
        tempo: 125,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = victracker::VOICE_NAMES.iter().map(|n| Channel { pan: 128, name: n.to_string(), ..Default::default() }).collect();
    // A pattern each position, built once whichever songs play it.
    let mut pattern_of: Vec<Option<u16>> = vec![None; 256];
    let mut seqs = Vec::new();
    for n in 0..song.song_count() {
        let (start, end, repeat, speed) = song.song(n);
        let mut orders = Vec::new();
        let mut pos = start;
        loop {
            let k = match pattern_of[usize::from(pos)] {
                Some(k) => k,
                None => {
                    let k = m.patterns.len() as u16;
                    m.patterns.push(Arc::new(vt_pattern(&song, pos)));
                    pattern_of[usize::from(pos)] = Some(k);
                    k
                }
            };
            orders.push(k);
            if pos == end || orders.len() >= 256 {
                break;
            }
            pos = pos.wrapping_add(1);
        }
        // The song comes back to its start, or with its repeat flags to its repeat position.
        let back = if speed & 0xC0 == 0 { start } else { repeat };
        let restart = usize::from(back.wrapping_sub(start)).min(orders.len() - 1);
        if n == 0 {
            m.speed = (speed & 0x3F) + 1;
        }
        seqs.push(Sequence { name: if n == 0 { "Song".into() } else { format!("Song {}", n + 1) }, orders, restart: restart as u16 });
    }
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    m.samples = (0..16).map(|s| named(&format!("Sound {s:X}"))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// Position `pos`'s steps, each voice's from its own pattern.
fn vt_pattern(song: &victracker::Song, pos: u8) -> Pattern {
    let rows = song.length(pos);
    let mut pat = Pattern::new(rows, victracker::VOICES);
    // The tracker's note (MIDI's plus one) of a register value on a voice.
    let note_of_reg = |v: usize, reg: u8| {
        let hz = song.hz(v, reg);
        let midi = 69.0 + 12.0 * (hz / 440.0).log2();
        note_at(midi.round() as i32 + 1, 1)
    };
    for r in 0..rows {
        for v in 0..victracker::VOICES {
            let st = song.step(v, pos, r);
            let Some(x) = pat.cell_mut(r, v) else { continue };
            match v {
                0..=2 => {
                    if let Some(n) = victracker::Song::note_of(v, st.note) {
                        x.note = note_of_reg(v, victracker::Song::freq_tab()[usize::from(n)]);
                    }
                }
                3 if st.note != 0 && st.note != 0x80 => x.note = note_of_reg(3, st.note),
                4 if st.note != 0 && st.note != 0x80 => {
                    (x.vol, x.volp) = (Vol::Volume, (st.note & 0x0F) * 64 / 15);
                    if st.note & 0xF0 != 0 {
                        (x.fx, x.param) = (Fx::Speed, (st.note >> 4) + 1);
                    }
                }
                _ => {}
            }
            let p = st.param;
            match p & 0xF0 {
                0x10 => (x.fx, x.param) = (Fx::PortaUp, p & 0x0F),
                0x20 => (x.fx, x.param) = (Fx::PortaDown, p & 0x0F),
                0x80 if v < 4 => x.instr = (p & 0x0F) + 1,
                0xC0 if v < 4 => (x.fx, x.param) = (Fx::ModEx, p),
                0xD0 => (x.fx, x.param) = (Fx::ModEx, p),
                _ => {}
            }
        }
    }
    pat
}

pub fn probe_arkos(d: &[u8]) -> bool {
    arkos::probe_sks(d) || neumetik_synth::arkos_xml::probe(d)
}

pub fn load_arkos(d: &[u8]) -> Result<Module> {
    let song = arkos::Song::parse(d).ok_or_else(|| Error::Bad("its song data is damaged".into()))?;
    let (format_name, tracker) = match song.kind {
        "sks" => ("STarKos", "STarKos"),
        "aks1" => ("Arkos Tracker 1", "Arkos Tracker 1"),
        "aks2" => ("Arkos Tracker 2", "Arkos Tracker 2"),
        _ => ("Arkos Tracker 3", "Arkos Tracker 3"),
    };
    let first = &song.subsongs[0];
    let channels = first.channels();
    let mut m = Module {
        title: if song.title == "Untitled" { String::new() } else { song.title.clone() },
        artist: if song.author.is_empty() { song.composer.clone() } else { song.author.clone() },
        message: song.comments.clone(),
        format: "arkos".into(),
        format_name: format_name.into(),
        tracker: tracker.into(),
        kind: Kind::Mod,
        speed: first.initial_speed.clamp(1, 255) as u8,
        // A tick at the replay rate: 50 Hz is tempo 125.
        tempo: (first.replay_hz * 2.5).round().clamp(32.0, 255.0) as u16,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    m.channels = (0..channels)
        .map(|c| {
            let psg = first.psgs.get(c / 3).copied().unwrap_or(arkos::Psg::new(arkos::CPC_HZ, 440.0));
            let name = if channels > 3 { format!("PSG {} {}", c / 3 + 1, ["A", "B", "C"][c % 3]) } else { ["A", "B", "C"][c % 3].to_string() };
            Channel { pan: (psg.pan[c % 3] * 255.0).round() as u8, name, ..Default::default() }
        })
        .collect();
    let mut seqs = Vec::new();
    for (k, sub) in song.subsongs.iter().enumerate() {
        let firstp = m.patterns.len();
        for p in 0..=sub.end.min(sub.positions.len().saturating_sub(1)) {
            let rows = sub.position_height(p);
            let mut pat = Pattern::new(rows, channels);
            for r in 0..rows {
                if let Some(s) = sub.special(true, p, r) {
                    if let Some(x) = pat.cell_mut(r, 0) {
                        (x.fx, x.param) = (Fx::Speed, s.clamp(1, 255) as u8);
                    }
                }
                for c in 0..channels.min(sub.channels()) {
                    let Some(cell) = sub.cell(p, r, c) else { continue };
                    let Some(x) = pat.cell_mut(r, c) else { continue };
                    let trans = sub.positions[p].transpositions.get(c).copied().unwrap_or(0);
                    if let Some(n) = cell.note {
                        // A reset (instrument 0) stops the channel.
                        x.note = if cell.instrument == Some(0) { NOTE_OFF } else { note_at(i32::from(n) + trans + 25, 1) };
                    }
                    if let Some(i) = cell.instrument.filter(|&i| i > 0) {
                        x.instr = i;
                    }
                    for (e, v) in cell.effects.iter().flatten().copied() {
                        match e {
                            arkos::Effect::Volume => (x.vol, x.volp) = (Vol::Volume, (v * 64 / 15) as u8),
                            arkos::Effect::Arpeggio3Notes if x.fx == Fx::None => (x.fx, x.param) = (Fx::Arpeggio, v as u8),
                            arkos::Effect::PitchUp | arkos::Effect::FastPitchUp if x.fx == Fx::None => (x.fx, x.param) = (Fx::PortaUp, (v >> 4).clamp(0, 255) as u8),
                            arkos::Effect::PitchDown | arkos::Effect::FastPitchDown if x.fx == Fx::None => (x.fx, x.param) = (Fx::PortaDown, (v >> 4).clamp(0, 255) as u8),
                            arkos::Effect::PitchGlide if x.fx == Fx::None => (x.fx, x.param) = (Fx::TonePorta, (v >> 4).clamp(0, 255) as u8),
                            _ => {}
                        }
                    }
                }
            }
            m.patterns.push(Arc::new(pat));
        }
        let orders: Vec<u16> = (firstp..m.patterns.len()).map(|p| p as u16).collect();
        seqs.push(Sequence { name: if sub.name.trim().is_empty() || song.subsongs.len() == 1 { if k == 0 { "Song".into() } else { format!("Song {}", k + 1) } } else { sub.name.clone() }, orders, restart: sub.loop_start as u16 });
        if m.patterns.len() > 4000 {
            break;
        }
    }
    m.orders = seqs[0].orders.clone();
    m.restart = seqs[0].restart;
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    m.samples = song.instruments.iter().skip(1).map(|i| named(&i.name)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

/// A DefleMask module unpacked, if it's one (its zlib checksum isn't
/// checked: many of DefleMask's own files have a wrong one).
fn deflemask_unpacked(d: &[u8]) -> Option<Vec<u8>> {
    let head = crate::unpack::zlib_head(d, 32)?;
    if !deflemask::Module::probe(&head) {
        return None;
    }
    crate::unpack::zlib(d, 256 << 20)
}

/// DefleMask's YMU759 (a mobile phone's sound chip; system 1): its songs
/// are told apart only to say they aren't played.
fn deflemask_ymu759(head: &[u8]) -> bool {
    head.len() > 18 && head.starts_with(deflemask::MAGIC) && (16..=27).contains(&head[16]) && head[17] == 0x01
}

pub fn probe_deflemask(d: &[u8]) -> bool {
    crate::unpack::zlib_head(d, 32).is_some_and(|h| deflemask::Module::probe(&h) || deflemask_ymu759(&h))
}

pub fn load_deflemask(d: &[u8]) -> Result<Module> {
    if crate::unpack::zlib_head(d, 32).is_some_and(|h| deflemask_ymu759(&h)) {
        return Err(Error::Unsupported("DefleMask's YMU759 songs aren't played".into()));
    }
    let raw = deflemask_unpacked(d).ok_or_else(|| Error::Bad("its packing is damaged".into()))?;
    let dm = deflemask::Module::parse(&raw).ok_or_else(|| Error::Bad("its patterns are damaged".into()))?;
    if !deflemask_play::supported(dm.system) {
        return Err(Error::Unsupported(format!("DefleMask's {} songs aren't played yet", dm.system.name())));
    }
    let song = deflemask_play::Song::parse(&raw).ok_or_else(|| Error::Bad("its patterns are damaged".into()))?;
    let dm = song.module.clone();
    let speed = u32::from(dm.tick1.max(1)) * (u32::from(dm.time_base) + 1);
    let mut m = Module {
        title: dm.name.clone(),
        artist: dm.author.clone(),
        format: "deflemask".into(),
        format_name: "DefleMask".into(),
        tracker: format!("DefleMask ({})", dm.system.name()),
        kind: Kind::Mod,
        speed: speed.clamp(1, 31) as u8,
        // A tick at the module's rate: 50 Hz is tempo 125.
        tempo: (dm.hz() * 2.5).round().clamp(32.0, 255.0) as u16,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let nch = dm.system.channels();
    m.channels = (0..nch).map(|c| Channel { pan: 128, name: deflemask_play::channel_name(dm.system, c).into(), ..Default::default() }).collect();
    for o in 0..dm.orders() {
        let mut pat = Pattern::new(dm.rows, nch);
        for c in 0..nch {
            for (r, cell) in dm.patterns[c][o].iter().enumerate() {
                let Some(x) = pat.cell_mut(r, c) else { continue };
                x.note = match cell.note {
                    100 => NOTE_CUT,
                    // (Some modules write a C as note 0 of its octave.)
                    0..=12 if cell.note > 0 || cell.octave > 0 => note_at(i32::from(cell.octave) * 12 + i32::from(cell.note) + deflemask_play::midi_offset(dm.system, c) + 1, 1),
                    _ => NOTE_NONE,
                };
                if cell.instrument >= 0 {
                    x.instr = (cell.instrument as u16 + 1).min(255) as u8;
                }
                if cell.volume >= 0 {
                    let most = deflemask_play::max_volume(dm.system, c).max(1);
                    (x.vol, x.volp) = (Vol::Volume, (i32::from(cell.volume).min(most) * 64 / most) as u8);
                }
                for &(e, v) in &cell.fx {
                    if x.fx != Fx::None || e < 0 {
                        continue;
                    }
                    let p = v.max(0) as u8;
                    (x.fx, x.param) = match e {
                        0x00 if p != 0 => (Fx::Arpeggio, p),
                        0x01 => (Fx::PortaUp, p),
                        0x02 => (Fx::PortaDown, p),
                        0x03 => (Fx::TonePorta, p),
                        0x04 => (Fx::Vibrato, p),
                        0x07 => (Fx::Tremolo, p),
                        0x0A => (Fx::VolSlide, p),
                        0x0B => (Fx::PositionJump, p),
                        0x0D => (Fx::PatternBreak, p),
                        0x09 | 0x0F => (Fx::Speed, p.max(1)),
                        0xEC => (Fx::ModEx, 0xC0 | (p & 15)),
                        0xED => (Fx::ModEx, 0xD0 | (p & 15)),
                        _ => (Fx::None, 0),
                    };
                }
            }
        }
        m.patterns.push(Arc::new(pat));
    }
    m.orders = (0..m.patterns.len() as u16).collect();
    m.samples = dm.instruments.iter().map(|i| named(&i.name)).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}

pub fn probe_famitracker(d: &[u8]) -> bool {
    famitracker::probe(d)
}

pub fn load_famitracker(d: &[u8]) -> Result<Module> {
    let song = famitracker_play::Song::parse(d).ok_or_else(|| Error::Bad("its blocks are damaged".into()))?;
    let fm = song.module.clone();
    let chips: Vec<&str> = [(1u8, "VRC6"), (2, "VRC7"), (4, "FDS"), (8, "MMC5"), (16, "N163"), (32, "5B")].iter().filter(|c| fm.expansion & c.0 != 0).map(|c| c.1).collect();
    let tracker = if d.starts_with(b"Dn-") { "Dn-FamiTracker" } else { "FamiTracker" };
    let first = &fm.tracks[0];
    let hz = if fm.engine_speed > 0 { fm.engine_speed as f64 } else if fm.pal { 50.0 } else { 60.0 };
    let mut m = Module {
        title: fm.title.clone(),
        artist: fm.artist.clone(),
        message: if fm.comment.is_empty() { fm.copyright.clone() } else { fm.comment.clone() },
        format: "ftm".into(),
        format_name: "FamiTracker".into(),
        tracker: if chips.is_empty() { tracker.to_string() } else { format!("{tracker} ({})", chips.join(", ")) },
        kind: Kind::Mod,
        speed: (first.speed as u8).clamp(1, 31),
        // A tick at the engine's rate: 50 Hz is tempo 125.
        tempo: (hz * 2.5).round().clamp(32.0, 255.0) as u16,
        global_volume: 128,
        mix_volume: 32,
        separation: 128,
        ..Default::default()
    };
    let nch = fm.channels.len();
    m.channels = fm.channels.iter().map(|c| Channel { pan: 128, name: c.name(), ..Default::default() }).collect();
    let mut seqs = Vec::new();
    for (k, t) in fm.tracks.iter().enumerate() {
        let firstp = m.patterns.len();
        for f in 0..t.frames.len() {
            let mut pat = Pattern::new(t.rows.max(1), nch);
            for r in 0..t.rows {
                for c in 0..nch {
                    let row = t.row(f, c, r);
                    let Some(x) = pat.cell_mut(r, c) else { continue };
                    x.note = match row.note {
                        1..=12 if fm.channels[c] == famitracker::Chan::Noise => note_at(i32::from(row.octave) * 12 + i32::from(row.note) + 48, 1),
                        // A-3 sounds at 440 Hz (MIDI's 69).
                        1..=12 => note_at(i32::from(row.octave) * 12 + i32::from(row.note) + 24, 1),
                        famitracker::NOTE_RELEASE => NOTE_FADE,
                        famitracker::NOTE_HALT => NOTE_CUT,
                        _ => NOTE_NONE,
                    };
                    if row.instrument < famitracker::NO_INSTRUMENT {
                        x.instr = row.instrument + 1;
                    }
                    if row.volume < famitracker::NO_VOLUME {
                        (x.vol, x.volp) = (Vol::Volume, row.volume * 64 / 15);
                    }
                    for &(e, p) in &row.fx {
                        if x.fx != Fx::None {
                            break;
                        }
                        (x.fx, x.param) = match e {
                            famitracker::EF_SPEED if u32::from(p) < fm.speed_split => (Fx::Speed, p.max(1)),
                            famitracker::EF_SPEED => (Fx::Tempo, p),
                            famitracker::EF_JUMP => (Fx::PositionJump, p),
                            famitracker::EF_SKIP => (Fx::PatternBreak, p),
                            famitracker::EF_ARPEGGIO => (Fx::Arpeggio, p),
                            famitracker::EF_PORTA_UP => (Fx::PortaUp, p),
                            famitracker::EF_PORTA_DOWN => (Fx::PortaDown, p),
                            famitracker::EF_PORTAMENTO => (Fx::TonePorta, p),
                            famitracker::EF_VIBRATO => (Fx::Vibrato, p),
                            famitracker::EF_TREMOLO => (Fx::Tremolo, p),
                            famitracker::EF_VOLUME_SLIDE => (Fx::VolSlide, p),
                            famitracker::EF_NOTE_CUT => (Fx::ModEx, 0xC0 | (p & 15)),
                            famitracker::EF_DELAY => (Fx::ModEx, 0xD0 | (p & 15)),
                            _ => (Fx::None, 0),
                        };
                    }
                }
            }
            m.patterns.push(Arc::new(pat));
        }
        let orders: Vec<u16> = (firstp..m.patterns.len()).map(|p| p as u16).collect();
        seqs.push(Sequence { name: if t.name.trim().is_empty() { if k == 0 { "Song".into() } else { format!("Song {}", k + 1) } } else { t.name.clone() }, orders, restart: 0 });
        if m.patterns.len() > 4000 {
            break;
        }
    }
    m.orders = seqs[0].orders.clone();
    if seqs.len() > 1 {
        m.sequences = seqs;
    }
    let top = fm.instruments.keys().copied().max().map_or(0, |k| usize::from(k) + 1);
    m.samples = (0..top).map(|i| named(fm.instruments.get(&(i as u8)).map_or("", |x| x.name.as_str()))).collect();
    m.tune = Some(Tune(Arc::new(song)));
    m.tidy();
    Ok(m)
}
