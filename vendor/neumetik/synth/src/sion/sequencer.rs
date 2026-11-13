// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's SiMMLSequencer, MMLSequencer and MMLExecutor
// (org.si.sion.sequencer, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! The sequencer: every track's executor walks its sequence, an event at
//! a time, in samples; the global sequence (tempo) runs beside them; and
//! one block of audio is made at a time.

use super::compile::*;
use super::effects::{Ctx, EffectModule};
use super::module::*;
use super::pcm::SoundReference;
use super::table::*;
use super::track::*;
use std::sync::Arc;

const FIXED_BITS: i32 = 8;
const FIXED_FILTER: i32 = (1 << FIXED_BITS) - 1;
const NONE_I: i32 = i32::MIN;

/// Beats a minute (`BeatPerMinutes`).
#[derive(Clone, Copy, Debug)]
struct Bpm {
    bpm: f64,
    sample_rate: i32,
    tick_per_sample: f64,
    sample_per_tick: i32,
}

impl Bpm {
    fn new(bpm: f64, sample_rate: i32) -> Bpm {
        let mut b = Bpm { bpm: 0.0, sample_rate: 0, tick_per_sample: 0.0, sample_per_tick: 0 };
        b.update(bpm, sample_rate);
        b
    }

    /// Whether it changed.
    fn update(&mut self, bpm: f64, sample_rate: i32) -> bool {
        let bpm = bpm.clamp(1.0, 511.0);
        if bpm != self.bpm || sample_rate != self.sample_rate {
            self.bpm = bpm;
            self.sample_rate = sample_rate;
            self.tick_per_sample = RES as f64 * bpm / (f64::from(sample_rate) * 240.0);
            self.sample_per_tick = to_i32((1.0 / self.tick_per_sample) * f64::from(1 << FIXED_BITS));
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ptr {
    Ev(usize),
    Process(usize),
    End,
}

/// `MMLExecutor`.
#[derive(Clone, Debug)]
struct Exec {
    ptr: Ptr,
    repeat_point: Option<usize>,
    counters: Vec<i64>,
    residue: i32,
    decimal: i32,
}

impl Exec {
    fn new() -> Exec {
        Exec { ptr: Ptr::Ev(0), repeat_point: None, counters: vec![], residue: 0, decimal: 0 }
    }

    fn on_tempo_changed(&mut self, ratio: f64) {
        let r = if self.residue < 0 { 1.0 / ratio } else { ratio };
        self.residue = to_i32(f64::from(self.residue) * r);
        self.decimal = to_i32(f64::from(self.decimal) * r);
    }
}

/// The parameters after an event: its own data, then the `,` that follow.
fn get_parameters(ev: &[E], i: usize, n: usize) -> (Vec<i32>, usize) {
    let mut p = Vec::with_capacity(n);
    let mut last = i;
    p.push(ev[i].data as i32);
    while p.len() < n && ev.get(last + 1).is_some_and(|e| e.k == K::Param) {
        last += 1;
        p.push(ev[last].data as i32);
    }
    while p.len() < n {
        p.push(NONE_I);
    }
    (p, last)
}

/// The sequencer.
pub struct Sequencer {
    song: Arc<Song>,
    shared: Shared,
    pub module: Module,
    effects: EffectModule,
    tracks: Vec<Track>,
    execs: Vec<Exec>,
    seq_of: Vec<usize>,
    global_exec: Exec,
    bpm: Bpm,
    sample_rate: i32,
    buffer_length: i32,
    process_sample_count: i32,
    global_buffer_sample_count: i32,
    global_execute_sample_count: i32,
    processed_sample_count: u64,
    pub is_sequence_finished: bool,
    steps: u64,
    /// The sample each track first came to its end at.
    end_at: Vec<Option<u64>>,
}

impl Sequencer {
    /// A sequencer for a compiled song, at 44100 or 22050 Hz, a block of `buffer_length` samples (a power of two).
    pub fn new(song: Song, sample_rate: i32, buffer_length: usize) -> Sequencer {
        let mut table = Table::new(sample_rate);
        for (i, w) in song.waves.iter().enumerate() {
            if w.is_some() {
                table.custom_wave_tables[i] = w.clone();
            }
        }
        let table = Arc::new(table);
        let mut shared = Shared::new(table.clone());
        shared.envelopes = song.envelopes.clone();
        shared.voices = song.voices.clone();
        shared.samples = song.samples.clone();
        let mut module = Module::new(table.clone(), buffer_length);
        // `SiMMLSequencer`'s constructor: the initial values of every operator.
        {
            let o = &mut module.init_operator_param;
            o.ar = 63;
            o.dr = 0;
            o.sr = 0;
            o.rr = 28;
            o.sl = 0;
            o.tl = 0;
            o.ksr = 0;
            o.ksl = 0;
            o.fmul = 128;
            o.dt1 = 0;
            o.detune = 0;
            o.ams = 1;
            o.phase = 0;
            o.fixed_pitch = 0;
            o.mod_level = 5;
            o.set_pg_type(&table, super::table::PG_SQUARE as i32);
        }
        let mut effects = EffectModule::new(Ctx { table: table.clone(), envelopes: Arc::new(song.envelopes.clone()) });
        for (slot, mml, post) in &song.effects {
            if (0..8).contains(slot) {
                effects.parse_mml(*slot as usize, mml, post);
            }
        }
        effects.prepare_process(&mut module);
        let song = Arc::new(song);
        let mut tracks = vec![];
        let mut execs = vec![];
        let mut seq_of = vec![];
        for (n, seq) in song.seqs.iter().enumerate() {
            // A sequence with nothing to play but rests has no track.
            if !seq.ev.iter().any(|e| e.len != 0 && e.k != K::Rest) {
                continue;
            }
            let mut t = Track::new(&module);
            t.set_default_fps(song.fps.clamp(1, 1000));
            t.reset(0, &shared, &module, song.setting.default_shift as i32, song.setting.velocity_mode, song.setting.expression_mode);
            // `_resetAllTracks`: the gate starts at the default quantize ratio (q6 of 8), not 1.
            t.quant_ratio = song.setting.default_quant_ratio as f64 / song.setting.max_quant_ratio as f64;
            tracks.push(t);
            execs.push(Exec::new());
            seq_of.push(n);
        }
        let seq_count = tracks.len();
        let bpm = Bpm::new(song.initial_bpm.unwrap_or(120.0), sample_rate);
        Sequencer {
            song,
            shared,
            module,
            effects,
            tracks,
            execs,
            seq_of,
            global_exec: Exec::new(),
            bpm,
            sample_rate,
            buffer_length: buffer_length as i32,
            process_sample_count: 0,
            global_buffer_sample_count: 0,
            global_execute_sample_count: 0,
            processed_sample_count: 0,
            is_sequence_finished: false,
            steps: 0,
            end_at: vec![None; seq_count],
        }
    }

    /// The sample at which every track has come to the end of its sequence once (a looping song goes on).
    pub fn song_end(&self) -> Option<u64> {
        if self.end_at.iter().all(|e| e.is_some()) {
            self.end_at.iter().flatten().copied().max()
        } else {
            None
        }
    }

    pub fn title(&self) -> &str {
        &self.song.title
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn bpm(&self) -> f64 {
        self.bpm.bpm
    }

    /// Samples made so far.
    pub fn processed_samples(&self) -> u64 {
        self.processed_sample_count
    }

    fn calc_sample_count(&self, len: i64) -> i32 {
        ((len * i64::from(self.bpm.sample_per_tick)) >> FIXED_BITS) as i32
    }

    fn set_bpm(&mut self, new_value: f64) {
        let old = self.bpm.bpm;
        let rate = self.sample_rate;
        if self.bpm.update(new_value, rate) {
            let ratio = old / self.bpm.bpm;
            for e in self.execs.iter_mut() {
                e.on_tempo_changed(ratio);
            }
        }
    }

    /// One block of audio: `buffer_length` samples of interleaved stereo in `module.output()`.
    pub fn process(&mut self) {
        self.module.begin_process();
        for t in self.tracks.iter_mut() {
            t.channel.reset_channel_buffer_status();
        }
        let mut finished = true;
        // start the global sequence
        self.global_buffer_sample_count = self.buffer_length;
        self.global_execute_sample_count = 0;
        loop {
            let buffering_length = self.execute_global_sequence();
            for i in 0..self.tracks.len() {
                let seg_start = self.buffer_length - self.global_buffer_sample_count - buffering_length;
                finished = self.process_executor(i, buffering_length, seg_start) && finished;
            }
            // isEndGlobalSequence
            if self.global_buffer_sample_count == 0 {
                break;
            }
        }
        self.processed_sample_count += self.buffer_length as u64;
        self.is_sequence_finished = finished;
        self.effects.end_process(&mut self.module);
        self.module.end_process();
    }

    fn execute_global_sequence(&mut self) -> i32 {
        self.global_execute_sample_count = 0;
        let song = self.song.clone();
        loop {
            let at = match self.global_exec.ptr {
                Ptr::Ev(i) if i < song.global.len() => Some(i),
                _ => None,
            };
            match at {
                None => {
                    self.global_execute_sample_count = self.global_buffer_sample_count;
                    self.global_buffer_sample_count = 0;
                }
                Some(i) => match song.global[i] {
                    Global::Tempo(p) => {
                        let bpm = song.bpm_of(p);
                        self.set_bpm(bpm);
                        self.global_exec.ptr = Ptr::Ev(i + 1);
                    }
                    Global::Wait(len) => {
                        let ex = &mut self.global_exec;
                        if ex.residue == 0 {
                            let fixed = (len as i64 * i64::from(self.bpm.sample_per_tick) + i64::from(ex.decimal)) as i32;
                            ex.residue = fixed >> FIXED_BITS;
                            ex.decimal = fixed & FIXED_FILTER;
                        }
                        if ex.residue <= self.global_buffer_sample_count {
                            self.global_execute_sample_count = ex.residue;
                            self.global_buffer_sample_count -= ex.residue;
                            ex.residue = 0;
                            ex.ptr = Ptr::Ev(i + 1);
                        } else {
                            self.global_execute_sample_count = self.global_buffer_sample_count;
                            ex.residue -= self.global_execute_sample_count;
                            self.global_buffer_sample_count = 0;
                        }
                    }
                },
            }
            if self.global_execute_sample_count != 0 {
                break;
            }
        }
        self.global_execute_sample_count
    }

    /// `processMMLExecutor`: whether the sequence is finished.
    fn process_executor(&mut self, ti: usize, count: i32, seg_start: i32) -> bool {
        let song = self.song.clone();
        let seq = &song.seqs[self.seq_of[ti]];
        let taken = &song.taken[self.seq_of[ti]];
        self.process_sample_count = count;
        while self.process_sample_count > 0 {
            self.steps += 1;
            match self.execs[ti].ptr {
                Ptr::End => {
                    // NOP: the rest of the block with nothing playing.
                    let n = self.process_sample_count;
                    self.tracks[ti].buffer(&self.shared, &mut self.module, n);
                    self.execs[ti].residue -= n;
                    return true;
                }
                Ptr::Process(i) => {
                    let len = seq.ev[i].len;
                    let ex = &mut self.execs[ti];
                    if ex.residue == 0 {
                        let fixed = (len * i64::from(self.bpm.sample_per_tick) + i64::from(ex.decimal)) as i32;
                        ex.residue = fixed >> FIXED_BITS;
                        ex.decimal = fixed & FIXED_FILTER;
                    }
                    let residue = ex.residue;
                    if residue <= self.process_sample_count {
                        self.tracks[ti].buffer(&self.shared, &mut self.module, residue);
                        self.process_sample_count -= residue;
                        self.execs[ti].residue = 0;
                        self.execs[ti].ptr = Ptr::Ev(i + 1);
                    } else {
                        let n = self.process_sample_count;
                        self.tracks[ti].buffer(&self.shared, &mut self.module, n);
                        self.execs[ti].residue -= n;
                        self.process_sample_count = 0;
                    }
                }
                Ptr::Ev(i) => {
                    if i >= seq.ev.len() {
                        // The sequence's tail: the repeat point, else the end.
                        if self.end_at[ti].is_none() {
                            let done = i64::from(seg_start) + i64::from(count - self.process_sample_count);
                            self.end_at[ti] = Some(self.processed_sample_count + done as u64);
                        }
                        self.execs[ti].ptr = match self.execs[ti].repeat_point {
                            Some(r) => Ptr::Ev(r),
                            None => Ptr::End,
                        };
                        // A repeat with nothing to wait for would never end.
                        if let Ptr::Ev(r) = self.execs[ti].ptr {
                            if !seq.ev[r..].iter().any(|e| e.len != 0) {
                                self.execs[ti].ptr = Ptr::End;
                            }
                        }
                        continue;
                    }
                    let next = self.event(ti, seq, taken, i);
                    self.execs[ti].ptr = next;
                }
            }
        }
        false
    }

    /// Publishes an event with a length as a process event.
    fn publish(&self, seq: &Seq, i: usize) -> Ptr {
        if seq.ev[i].len > 0 {
            Ptr::Process(i)
        } else {
            Ptr::Ev(i + 1)
        }
    }

    /// An event of a track: what it does, and where the executor goes next.
    fn event(&mut self, ti: usize, seq: &Seq, taken: &[bool], i: usize) -> Ptr {
        let e = &seq.ev[i];
        let ev = &seq.ev[..];
        let next = Ptr::Ev(i + 1);
        let mask = self.tracks[ti].event_mask;
        let table = self.shared.table.clone();
        match e.k {
            K::Rest => {
                self.tracks[ti].on_rest_event();
                self.publish(seq, i)
            }
            K::Note => {
                let n = self.calc_sample_count(e.len);
                self.tracks[ti].on_note_event(&self.shared, &mut self.module, e.data as i32, n);
                self.publish(seq, i)
            }
            K::Slur => {
                if mask & MASK_SLUR != 0 {
                    let n = self.calc_sample_count(e.len);
                    self.tracks[ti].change_note_length(n);
                } else {
                    self.tracks[ti].on_slur();
                }
                self.publish(seq, i)
            }
            K::SlurWeak => {
                if mask & MASK_SLUR != 0 {
                    let n = self.calc_sample_count(e.len);
                    self.tracks[ti].change_note_length(n);
                } else {
                    self.tracks[ti].on_slur_weak();
                }
                self.publish(seq, i)
            }
            K::Bend => {
                if mask & MASK_SLUR != 0 {
                    let n = self.calc_sample_count(e.len);
                    self.tracks[ti].change_note_length(n);
                } else {
                    let Some(nx) = ev.get(i + 1).filter(|n| n.k == K::Note) else { return next };
                    let term = self.calc_sample_count(e.len);
                    self.tracks[ti].on_pitch_bend(nx.data as i32, term);
                }
                self.publish(seq, i)
            }
            K::Quant => {
                if mask & MASK_QUANTIZE == 0 {
                    self.tracks[ti].quant_ratio = e.data as f64 / self.song.setting.max_quant_ratio as f64;
                }
                next
            }
            K::QuantCount => {
                let (p, last) = get_parameters(ev, i, 2);
                let res = RES as i32;
                let a = if p[0] == NONE_I { 0 } else { p[0] * res / 192 };
                let b = if p[1] == NONE_I { 0 } else { p[1] * res / 192 };
                if mask & MASK_QUANTIZE == 0 {
                    self.tracks[ti].quant_count = self.calc_sample_count(i64::from(a));
                    self.tracks[ti].key_on_delay = self.calc_sample_count(i64::from(b));
                }
                Ptr::Ev(last + 1)
            }
            K::Volume => {
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].mml_v_command(e.data as i32);
                }
                next
            }
            K::VolumeShift => {
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].mml_v_shift(e.data as i32);
                }
                next
            }
            K::FineVolume => {
                let (p, last) = get_parameters(ev, i, STREAM_SEND_SIZE);
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].channel.set_all_stream_send_levels(&p);
                }
                Ptr::Ev(last + 1)
            }
            K::ModType => {
                let (p, last) = get_parameters(ev, i, 2);
                let t = if p[0] < 0 || p[0] >= MT_MAX as i32 { MT_ALL } else { p[0] as usize };
                let (ch, tone) = (p[1], i32::MIN);
                self.tracks[ti].set_channel_module_type(&self.shared, &self.module, t, ch, tone);
                Ptr::Ev(last + 1)
            }
            K::ModParam => {
                let (p, last) = get_parameters(ev, i, 16);
                if mask & MASK_OPERATOR != 0 {
                    return Ptr::Ev(last + 1);
                }
                if let Some(text) = self.tracks[ti].set_channel_parameters(&self.shared, &self.module, &p) {
                    self.run_init_sequence(ti, &text);
                }
                Ptr::Ev(last + 1)
            }
            K::InPipe => {
                let (p, last) = get_parameters(ev, i, 2);
                let (a, b) = (if p[0] == NONE_I { 5 } else { p[0] }, if p[1] == NONE_I { 0 } else { p[1] });
                self.tracks[ti].channel.set_input(a, b);
                Ptr::Ev(last + 1)
            }
            K::OutPipe => {
                let (p, last) = get_parameters(ev, i, 2);
                let (a, b) = (if p[0] == NONE_I { 2 } else { p[0] }, if p[1] == NONE_I { 0 } else { p[1] });
                self.tracks[ti].channel.set_output(a, b);
                Ptr::Ev(last + 1)
            }
            K::RepAll => {
                self.execs[ti].repeat_point = Some(i + 1);
                next
            }
            K::RepBegin => {
                self.execs[ti].counters.push(e.data);
                next
            }
            K::RepBreak => {
                let ex = &mut self.execs[ti];
                if ex.counters.last() == Some(&1) {
                    ex.counters.pop();
                    Ptr::Ev(ev[e.jump].jump + 1)
                } else {
                    next
                }
            }
            K::RepEnd => {
                let ex = &mut self.execs[ti];
                match ex.counters.last_mut() {
                    Some(c) => {
                        *c -= 1;
                        if *c == 0 {
                            ex.counters.pop();
                            next
                        } else {
                            Ptr::Ev(e.jump + 1)
                        }
                    }
                    None => next,
                }
            }
            K::Tempo => {
                if !taken[i] {
                    let bpm = self.song.bpm_of(e.data);
                    self.set_bpm(bpm);
                }
                next
            }
            K::Param | K::System | K::Table => next,
            K::User(u) => self.user_event(ti, ev, i, u, mask, &table),
        }
    }

    /// The commands SiMMLSequencer registers by letter.
    fn user_event(&mut self, ti: usize, ev: &[E], i: usize, u: &str, mask: i32, table: &Table) -> Ptr {
        let e = &ev[i];
        let d = e.data as i32;
        let next = Ptr::Ev(i + 1);
        macro_rules! env_event {
            ($method:ident, $slot:expr $(, $extra:expr)?) => {{
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_ENVELOP == 0 {
                    let step = if p[1] == NONE_I { 1 } else { p[1] };
                    // SiON's own check is `p < 255`, which would turn away the 511, 510...
                    // that inline `{...}` tables are given: they are taken here.
                    let idx = if (0..512).contains(&p[0]) { p[0] } else { -1 };
                    let t = self.shared.envelope(idx);
                    self.tracks[ti].$method($slot, t, step $(, $extra)?);
                }
                return Ptr::Ev(last + 1);
            }};
        }
        match u {
            "k" => self.tracks[ti].pitch_shift = if d == NONE_I { 0 } else { d },
            "kt" => self.tracks[ti].note_shift = if d == NONE_I { 0 } else { d },
            "!@kr" => self.tracks[ti].pitch_shift += if d == NONE_I { 0 } else { d },
            "@mask" => self.tracks[ti].event_mask = if d != NONE_I { d } else { 0 },
            "p" => {
                if mask & MASK_PAN == 0 {
                    self.tracks[ti].channel.set_pan(if d == NONE_I { 0 } else { (d << 4) - 64 });
                }
            }
            "@p" => {
                if mask & MASK_PAN == 0 {
                    self.tracks[ti].channel.set_pan(if d == NONE_I { 0 } else { d });
                }
            }
            "x" => {
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].set_expression(if d == NONE_I { 128 } else { d });
                }
            }
            "%v" => {
                let (p, last) = get_parameters(ev, i, STREAM_SEND_SIZE);
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].vcommand_shift = if p[1] == NONE_I { 4 } else { p[1] };
                    self.tracks[ti].set_velocity_mode(if p[0] == NONE_I { 0 } else { p[0] });
                }
                return Ptr::Ev(last + 1);
            }
            "%x" => {
                if mask & MASK_VOLUME == 0 {
                    self.tracks[ti].set_expression_mode(if d == NONE_I { 0 } else { d });
                }
            }
            "@f" => {
                let (p, last) = get_parameters(ev, i, 10);
                let g = |k: usize, def: i32| if p[k] == NONE_I { def } else { p[k] };
                self.tracks[ti].channel.set_sv_filter(g(0, 128), g(1, 0), g(2, 0), g(3, 0), g(4, 0), g(5, 0), g(6, 128), g(7, 64), g(8, 32), g(9, 128));
                return Ptr::Ev(last + 1);
            }
            "%f" => self.tracks[ti].channel.set_filter_type(d),
            "@clock" => self.tracks[ti].channel.set_frequency_ratio(if d == NONE_I { 100 } else { d }),
            "@al" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    let cnt = if p[0] != NONE_I { p[0] } else { 0 };
                    let alg = if p[1] != NONE_I { p[1] } else { [0, 1, 5, 7][(cnt.clamp(0, 3)) as usize] };
                    self.tracks[ti].channel.set_algorism(cnt, alg);
                }
                return Ptr::Ev(last + 1);
            }
            "@fb" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_feedback(if p[0] != NONE_I { p[0] } else { 0 }, if p[1] != NONE_I { p[1] } else { 0 });
                }
                return Ptr::Ev(last + 1);
            }
            "@r" => {
                let (p, last) = get_parameters(ev, i, 2);
                let (a, b) = (if p[0] == NONE_I { 4 } else { p[0] }, if p[1] == NONE_I { 0 } else { p[1] });
                self.tracks[ti].channel.set_ring_modulation(a, b);
                return Ptr::Ev(last + 1);
            }
            "i" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_active_operator_index(if d == NONE_I { 4 } else { d });
                }
            }
            "@rr" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    if p[0] != NONE_I {
                        self.tracks[ti].channel.set_active_rr(p[0]);
                    }
                    self.tracks[ti].set_release_sweep(if p[1] == NONE_I { 0 } else { p[1] });
                }
                return Ptr::Ev(last + 1);
            }
            "@tl" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_active_tl(if d == NONE_I { 0 } else { d });
                }
            }
            "@ml" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    let (a, b) = (if p[0] == NONE_I { 0 } else { p[0] }, if p[1] == NONE_I { 0 } else { p[1] });
                    self.tracks[ti].channel.set_active_fmul((a << 7) + b);
                }
                return Ptr::Ev(last + 1);
            }
            "@dt" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_active_detune(if d == NONE_I { 0 } else { d });
                }
            }
            "@ph" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_active_phase(if d == NONE_I { 0 } else { d });
                }
            }
            "@fx" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    let (a, b) = (if p[0] == NONE_I { 0 } else { p[0] }, if p[1] == NONE_I { 0 } else { p[1] });
                    self.tracks[ti].channel.set_active_fixed_pitch((a << 6) + b);
                }
                return Ptr::Ev(last + 1);
            }
            "@se" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_active_ssgec(if d == NONE_I { 0 } else { d });
                }
            }
            "@er" => {
                if mask & MASK_OPERATOR == 0 {
                    self.tracks[ti].channel.set_erst(d == 1);
                }
            }
            "s" => {
                let (p, last) = get_parameters(ev, i, 2);
                if mask & MASK_OPERATOR == 0 {
                    if p[0] != NONE_I {
                        self.tracks[ti].channel.set_all_release_rate(p[0]);
                    }
                    self.tracks[ti].set_release_sweep(if p[1] == NONE_I { 0 } else { p[1] });
                }
                return Ptr::Ev(last + 1);
            }
            "@lfo" => {
                let (p, last) = get_parameters(ev, i, 2);
                if p[1] > 7 && p[1] < 255 {
                    match self.shared.envelope(p[1]) {
                        Some(env) => self.tracks[ti].channel.initialize_lfo(-1, Some(&env.to_vector(256, 0, 255))),
                        None => self.tracks[ti].channel.initialize_lfo(LFO_WAVE_TRIANGLE as i32, None),
                    }
                } else {
                    self.tracks[ti].channel.initialize_lfo(if p[1] == NONE_I { LFO_WAVE_TRIANGLE as i32 } else { p[1] }, None);
                }
                let ms = if p[0] == NONE_I { 333.0 } else { f64::from(p[0]) * 1000.0 / 60.0 };
                self.tracks[ti].channel.set_lfo_cycle_time(ms);
                return Ptr::Ev(last + 1);
            }
            "mp" | "ma" => {
                let (p, last) = get_parameters(ev, i, 4);
                if mask & MASK_MODULATE == 0 {
                    let g = |k: usize| if p[k] == NONE_I { 0 } else { p[k] };
                    self.tracks[ti].set_modulation_envelop(u == "mp", g(0), g(1), g(2), g(3));
                }
                return Ptr::Ev(last + 1);
            }
            "@fps" => {
                let frame = if d == NONE_I || d == 0 { 60 } else { d.min(1000) };
                self.tracks[ti].set_envelop_fps(frame);
            }
            "@@" => env_event!(set_tone_envelop, 1),
            "na" => env_event!(set_amplitude_envelop, 1, false),
            "!na" => env_event!(set_amplitude_envelop, 1, true),
            "np" => env_event!(set_pitch_envelop, 1),
            "nt" => env_event!(set_note_envelop, 1),
            "nf" => env_event!(set_filter_envelop, 1),
            "_@@" => env_event!(set_tone_envelop, 0),
            "_na" => env_event!(set_amplitude_envelop, 0, false),
            "_np" => env_event!(set_pitch_envelop, 0),
            "_nt" => env_event!(set_note_envelop, 0),
            "_nf" => env_event!(set_filter_envelop, 0),
            "po" => self.tracks[ti].set_portament(if d == NONE_I { 0 } else { d }),
            // %t, %e (event triggers) have nothing to trigger here.
            _ => {}
        }
        let _ = table;
        next
    }

    /// An FM voice's own MML (`#@3{...}cdef`), run as it's selected.
    fn run_init_sequence(&mut self, ti: usize, text: &str) {
        let Ok(events) = parse_fragment(text, &self.song.setting, self.shared.table.clone()) else { return };
        let taken = vec![false; events.len()];
        let seq = Seq { ev: events, globals: 0 };
        let mut i = 0;
        while i < seq.ev.len() {
            let e = &seq.ev[i];
            // It can't wait or choose a tone.
            if e.len != 0 || matches!(e.k, K::ModType | K::ModParam) {
                i += 1;
                continue;
            }
            match self.event(ti, &seq, &taken, i) {
                Ptr::Ev(n) => i = n.max(i + 1),
                _ => i += 1,
            }
        }
    }
}

/// Sample rate of `render_song`.
pub const RATE: u32 = 44_100;

/// A song rendered: interleaved stereo, the frame the music ends at (a
/// looping song's loop length), and its title.
pub struct Rendered {
    pub samples: Vec<f32>,
    pub end: usize,
    pub title: String,
}

/// The longest song rendered, in seconds.
const LONGEST: u64 = 1200;

/// SiON's MML rendered as SiON plays it: to where every track has come to
/// its end, and three seconds more for the last sounds to ring out.
pub fn render_song(mml: &str) -> Result<Rendered, String> {
    render_song_with(mml, Arc::new(SoundReference::new()))
}

/// `render_song` with the sounds the MML's `#SAMPLER`, `#PCMWAVE` and `#PCMVOICE` name.
pub fn render_song_with(mml: &str, sounds: Arc<SoundReference>) -> Result<Rendered, String> {
    let table = Arc::new(Table::new(RATE as i32));
    let song = compile_with(mml, table, sounds)?;
    let mut seq = Sequencer::new(song, RATE as i32, 2048);
    if seq.track_count() == 0 {
        return Err("there are no notes in it".into());
    }
    let tail = u64::from(RATE) * 3;
    let longest = u64::from(RATE) * LONGEST;
    let mut samples: Vec<f32> = vec![];
    let mut stop = None;
    loop {
        seq.process();
        samples.extend(seq.module.output().iter().map(|&x| x as f32));
        let done = seq.processed_samples();
        if stop.is_none() {
            if let Some(end) = seq.song_end() {
                stop = Some(end + tail);
            }
        }
        if stop.is_some_and(|s| done >= s) || done >= longest {
            break;
        }
    }
    let end = seq.song_end().unwrap_or(seq.processed_samples()) as usize;
    let frames = stop.map_or(samples.len() / 2, |s| s as usize).min(samples.len() / 2);
    samples.truncate(frames * 2);
    Ok(Rendered { samples, end: end.min(frames), title: seq.title().to_string() })
}

/// Renders a song to interleaved stereo at 44100 Hz: `seconds` long.
pub fn render_mml(mml: &str, seconds: f64) -> Result<Vec<f32>, String> {
    render_mml_with(mml, seconds, Arc::new(SoundReference::new()))
}

/// `render_mml` with a sound reference.
pub fn render_mml_with(mml: &str, seconds: f64, sounds: Arc<SoundReference>) -> Result<Vec<f32>, String> {
    let table = Arc::new(Table::new(44100));
    let song = compile_with(mml, table, sounds)?;
    let mut seq = Sequencer::new(song, 44100, 2048);
    let total = (seconds * 44100.0) as usize;
    let mut out: Vec<f32> = Vec::with_capacity(total * 2);
    while out.len() < total * 2 {
        seq.process();
        out.extend(seq.module.output().iter().map(|&x| x as f32));
        if seq.is_sequence_finished && out.len() > 44100 * 2 {
            // The tail rings out for a block more.
            break;
        }
    }
    out.truncate(total * 2);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(mml: &str) -> Sequencer {
        Sequencer::new(compile(mml, Arc::new(Table::new(44100))).unwrap(), 44100, 2048)
    }

    fn render(seq: &mut Sequencer, blocks: usize) -> Vec<f64> {
        let mut out = vec![];
        for _ in 0..blocks {
            seq.process();
            out.extend_from_slice(seq.module.output());
        }
        out
    }

    fn peak(v: &[f64]) -> f64 {
        v.iter().fold(0.0, |m: f64, x| m.max(x.abs()))
    }

    #[test]
    fn a_note_sounds_for_its_length_and_the_song_ends() {
        // t120: a quarter is half a second, 22050 samples; l4 is the default.
        let mut s = song("t120 o5 c");
        let out = render(&mut s, 20);
        let at = |from: usize, to: usize| peak(&out[from * 2..to * 2]);
        assert!(at(2000, 20000) > 0.05, "{}", at(2000, 20000));
        // The gate (q6 of 8) ends it at 16537; the release (rr 28) then dies away. Checked against Flash's SiON.
        assert!(at(25000, 40000) < at(2000, 20000) * 0.5, "{}", at(25000, 40000));
        assert!(s.is_sequence_finished);
    }

    #[test]
    fn notes_come_at_their_times_and_pitches() {
        let mut s = song("t120 l8 o5 c r c");
        let out = render(&mut s, 16);
        // Mono: left channel. Eighth = 11025 samples: sound, gap, sound.
        let l: Vec<f64> = out.iter().step_by(2).copied().collect();
        let seg = |a: usize, b: usize| peak(&l[a..b]);
        assert!(seg(1000, 9000) > 0.05);
        // The gate ends the note at 8268 and the release (rr 28) dies away slowly.
        assert!(seg(13000, 21000) < seg(1000, 9000) * 0.5, "{}", seg(13000, 21000));
        assert!(seg(23000, 31000) > 0.05);
        // The first note's pitch: middle C, 261.6 Hz.
        let crossings = l[2000..11000].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        let hz = crossings as f64 * 44100.0 / 9000.0;
        assert!((hz - 261.6).abs() < 12.0, "{hz}");
    }

    #[test]
    fn a_tempo_change_moves_the_later_notes() {
        let at_second_note = |mml: &str| {
            let mut s = song(mml);
            let out = render(&mut s, 24);
            let l: Vec<f64> = out.iter().step_by(2).copied().collect();
            // When the second note begins: the level (per 100 samples) jumps up after the release.
            let chunks: Vec<f64> = l.chunks(100).map(peak).collect();
            for i in 20..chunks.len() {
                if chunks[i] > chunks[i - 1] * 1.5 && chunks[i] > 0.05 {
                    return i * 100;
                }
            }
            usize::MAX
        };
        let slow = at_second_note("t120 q4 o5 c4 r4 c4");
        let fast = at_second_note("t240 q4 o5 c4 r4 c4");
        assert!(slow != usize::MAX && fast != usize::MAX);
        assert!((slow as f64 / fast as f64 - 2.0).abs() < 0.1, "{slow} {fast}");
    }

    /// SiON itself (its ActionScript compiled with Apache Royale, run in Ruffle: roadmap 5e) rendered
    /// this scale; `reference/scale.f32` is every 64th sample of its left channel, 110592 of them in all.
    /// It found two bugs the property tests could not: the default gate (q6 of 8) and the operators'
    /// initial values (rr 28, not 63), both set by SiON's sequencer.
    #[test]
    fn the_scale_matches_siON_run_in_flash() {
        let reference: Vec<f32> = include_bytes!("reference/scale.f32").chunks(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let r = render_song("t120 l8 o4 cdefgab<c").unwrap();
        assert_eq!(reference.len() * 64, 110592);
        for (i, want) in reference.iter().enumerate() {
            let got = r.samples[i * 64 * 2];
            assert!((got - want).abs() < 1e-5, "sample {}: {got} against {want}", i * 64);
        }
    }

    /// Compares our render with SiON run in Flash (`scripts/sion-ref/Batch.as`): the reference holds every 64th
    /// frame's left sample, for as long as SiON's own render lasted.
    fn matches_flash(mml: &str, reference: &[u8]) {
        let want: Vec<f32> = reference.chunks(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let r = render_song(mml).unwrap();
        assert!(want.iter().any(|x| x.abs() > 0.01));
        // SiON stops when the sound has died away; ours after three seconds past the end, so a note that never
        // dies (slow sustain) is compared for as long as ours lasts.
        for (i, w) in want.iter().enumerate().take(r.samples.len() / 128) {
            let got = r.samples[i * 64 * 2];
            assert!((got - w).abs() < 1e-5, "sample {}: {got} against {w}", i * 64);
        }
    }

    #[test]
    fn the_noise_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2 o5 cdefgab<c", include_bytes!("reference/noise.f32"));
    }

    #[test]
    fn the_fm6_matches_siON_run_in_flash() {
        matches_flash("#OPM@1 { 4 0  31 0 0 15 0 20 0 1 0 0 0 };\nt120 l8 %6@1 o5 cdefgab<c", include_bytes!("reference/fm6.f32"));
    }

    #[test]
    fn the_pulse_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@0 o4 cdefgab<c", include_bytes!("reference/pulse.f32"));
    }

    #[test]
    fn the_saw_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@2 o4 cdefgab<c", include_bytes!("reference/saw.f32"));
    }

    #[test]
    fn the_wavb_matches_siON_run_in_flash() {
        matches_flash("#WAVB1{ 40 40 40 40 c0 c0 c0 c0 };\nt120 l8 %4@1 o4 cdefgab<c", include_bytes!("reference/wavb.f32"));
    }

    #[test]
    fn the_chords_matches_siON_run_in_flash() {
        matches_flash("t120 l4 o4 c;t120 l4 o4 e;t120 l4 o4 g", include_bytes!("reference/chords.f32"));
    }

    #[test]
    fn the_volpan_matches_siON_run_in_flash() {
        matches_flash("t120 l8 o4 v8 p0 c p4 d p8 e v15 p4 f", include_bytes!("reference/volpan.f32"));
    }

    #[test]
    fn the_gate_matches_siON_run_in_flash() {
        matches_flash("t120 l8 o4 q1 c q4 d q8 e r", include_bytes!("reference/gate.f32"));
    }

    #[test]
    fn the_lowpass_matches_siON_run_in_flash() {
        matches_flash("#EFFECT0{lf300};\nt120 l8 o6 %5@4 cdefgab<c", include_bytes!("reference/lowpass.f32"));
    }

    #[test]
    fn the_delay_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{delay150,0,0,100};\nt120 @v0,128 o5 l4 c r r r", include_bytes!("reference/delay.f32"));
    }

    #[test]
    fn the_n_white_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@0 o5 cdefg", include_bytes!("reference/n_white.f32"));
    }

    #[test]
    fn the_n_pulse_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@1 o5 cdefg", include_bytes!("reference/n_pulse.f32"));
    }

    #[test]
    fn the_n_93_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@2 o5 cdefg", include_bytes!("reference/n_93.f32"));
    }

    #[test]
    fn the_n_hp_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@3 o5 cdefg", include_bytes!("reference/n_hp.f32"));
    }

    #[test]
    fn the_n_pink_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@4 o5 cdefg", include_bytes!("reference/n_pink.f32"));
    }

    #[test]
    fn the_n_128_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@5 o5 cdefg", include_bytes!("reference/n_128.f32"));
    }

    #[test]
    fn the_n_periodic_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@8 o5 cdefg", include_bytes!("reference/n_periodic.f32"));
    }

    #[test]
    fn the_n_pitched_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %2@9 o5 cdefg", include_bytes!("reference/n_pitched.f32"));
    }

    #[test]
    fn the_psg0_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %0@0 o5 cdefg", include_bytes!("reference/psg0.f32"));
    }

    #[test]
    fn the_psg1_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,16)16};t120 %0@1 nt0,20 q8 o0 c1", include_bytes!("reference/psg1.f32"));
    }

    #[test]
    fn the_psg2_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %0@2 o4 cdefg", include_bytes!("reference/psg2.f32"));
    }

    #[test]
    fn the_nes3_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %1@3 o5 cdefg", include_bytes!("reference/nes3.f32"));
    }

    #[test]
    fn the_nes8_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %1@8 o4 cdefg", include_bytes!("reference/nes8.f32"));
    }

    #[test]
    fn the_nes9_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,16)16};t120 %1@9 nt0,20 q8 o0 c1", include_bytes!("reference/nes9.f32"));
    }

    #[test]
    fn the_ma3_0_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %3@0 o4 cdefg", include_bytes!("reference/ma3_0.f32"));
    }

    #[test]
    fn the_ma3_5_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %3@5 o4 cdefg", include_bytes!("reference/ma3_5.f32"));
    }

    #[test]
    fn the_ma3_tbl_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,6)6}+8;t120 %3@@0,20 q8 c1", include_bytes!("reference/ma3_tbl.f32"));
    }

    #[test]
    fn the_all_tri_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 o4 cdefg", include_bytes!("reference/all_tri.f32"));
    }

    #[test]
    fn the_all_ramp_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@140 o4 cdefg", include_bytes!("reference/all_ramp.f32"));
    }

    #[test]
    fn the_all_pulse_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@66 o4 cdefg", include_bytes!("reference/all_pulse.f32"));
    }

    #[test]
    fn the_pulse8_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %8@3 o4 cdefg", include_bytes!("reference/pulse8.f32"));
    }

    #[test]
    fn the_ramp9_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %9@20 o4 cdefg", include_bytes!("reference/ramp9.f32"));
    }

    #[test]
    fn the_ramp9b_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %9@100 o4 cdefg", include_bytes!("reference/ramp9b.f32"));
    }

    #[test]
    fn the_na_table_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(64,0)5,(32,0)5,(16,0)5};t120 l8 q8 na0 cde", include_bytes!("reference/na_table.f32"));
    }

    #[test]
    fn the_na_slur_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,128)90};t120 l8 q8 na0 c&d&e", include_bytes!("reference/na_slur.f32"));
    }

    #[test]
    fn the_nt_arp_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{|0,4,7};t120 l8 nt0 cde", include_bytes!("reference/nt_arp.f32"));
    }

    #[test]
    fn the_np_bend_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,128,-64,0)60};t120 q8 np0 c1", include_bytes!("reference/np_bend.f32"));
    }

    #[test]
    fn the_at_tone_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,32)32};t120 q8 %3 @@0,5 c1", include_bytes!("reference/at_tone.f32"));
    }

    #[test]
    fn the_lfo_mp_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 @lfo30 mp32 c1", include_bytes!("reference/lfo_mp.f32"));
    }

    #[test]
    fn the_lfo_ma_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 @lfo20,1 ma16 c1", include_bytes!("reference/lfo_ma.f32"));
    }

    #[test]
    fn the_lfo_mp2_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 mp8,32,60,40 c1^1", include_bytes!("reference/lfo_mp2.f32"));
    }

    #[test]
    fn the_porta_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 po6 l8 c&g&f&b&&ag&&fe&d&c4", include_bytes!("reference/porta.f32"));
    }

    #[test]
    fn the_detune_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 k-2 cde k2 cde", include_bytes!("reference/detune.f32"));
    }

    #[test]
    fn the_kt_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 kt3 cde kt-3 cde", include_bytes!("reference/kt.f32"));
    }

    #[test]
    fn the_pan_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 @p-64 c @p-32 c @p0 c @p32 c @p64 c", include_bytes!("reference/pan.f32"));
    }

    #[test]
    fn the_xvol_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 v8 x32 c x64 c x96 c x128 c", include_bytes!("reference/xvol.f32"));
    }

    #[test]
    fn the_vlog_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 %v3,1 v128 c v96 c v64 c v32 c", include_bytes!("reference/vlog.f32"));
    }

    #[test]
    fn the_xlog_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 %x1 x128 c x120 c x112 c x104 c", include_bytes!("reference/xlog.f32"));
    }

    #[test]
    fn the_fm_al_matches_siON_run_in_flash() {
        matches_flash("#OPM@2 { 4 0  31 0 0 15 0 20 0 1 0 0 0  31 0 0 15 0 30 0 2 0 0 0 };t120 l8 %6@2 o5 cdefg", include_bytes!("reference/fm_al.f32"));
    }

    #[test]
    fn the_fm_fb_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %6@0 @fb5,1 @al2,0 o4 cdefg", include_bytes!("reference/fm_fb.f32"));
    }

    #[test]
    fn the_fm_ops_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 @al3 @fb4 o4 cdefg", include_bytes!("reference/fm_ops.f32"));
    }

    #[test]
    fn the_fm_s_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 s8 l4 o4 cde", include_bytes!("reference/fm_s.f32"));
    }

    #[test]
    fn the_ssg_matches_siON_run_in_flash() {
        matches_flash("t120 %5@4 @se8 l2 o4 c", include_bytes!("reference/ssg.f32"));
    }

    #[test]
    fn the_ar_dr_matches_siON_run_in_flash() {
        matches_flash("t120 l4 %5@4 @ 4,20,10,5,12,3,10 o4 cde", include_bytes!("reference/ar_dr.f32"));
    }

    #[test]
    fn the_fx_eq_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{eq150,80,120,800,5000};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_eq.f32"));
    }

    #[test]
    fn the_fx_dist_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{dist50,100};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_dist.f32"));
    }

    #[test]
    fn the_fx_rev_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{reverb70,40,80,100};t120 l8 @v64,64 %5@4 o5 cr r r", include_bytes!("reference/fx_rev.f32"));
    }

    #[test]
    fn the_fx_chorus_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{chorus20,50,200,100};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_chorus.f32"));
    }

    #[test]
    fn the_fx_comp_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{comp70,50,20,20,6,50};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_comp.f32"));
    }

    #[test]
    fn the_fx_autopan_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{autopan1,100};t120 l8 @v64,64 %5@4 o4 c1", include_bytes!("reference/fx_autopan.f32"));
    }

    #[test]
    fn the_fx_stereo_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{stereo140,0,0};t120 l8 @v64,64 %5@4 o4 cdefg", include_bytes!("reference/fx_stereo.f32"));
    }

    #[test]
    fn the_fx_ds_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{ds1,8,2};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_ds.f32"));
    }

    #[test]
    fn the_fx_chain_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{chorus}#EFFECT2{delay};t120 l8 @v64,32,32 %5@4 o4 cdefg", include_bytes!("reference/fx_chain.f32"));
    }

    #[test]
    fn the_song1_matches_siON_run_in_flash() {
        matches_flash("t140 %5@2 l8 o3 [4 c c>c<c] ; t140 %5@4 o5 l8 [2 cdefgfed]; t140 %2@0 l16 v8 [8 c r c r]", include_bytes!("reference/song1.f32"));
    }

    #[test]
    fn the_song2_matches_siON_run_in_flash() {
        matches_flash("t100 %6@0 @fb4 o4 l8 [2 cegecege]; t100 %5@4 o3 l4 [4 c g]", include_bytes!("reference/song2.f32"));
    }

    #[test]
    fn the_song3_matches_siON_run_in_flash() {
        matches_flash("#TABLE0{(0,8)8};t150 %1@@0,3 o5 l16 [4 cdefgabc]; t150 %0@1 nt0,20 o2 l8 q8 [8 c]", include_bytes!("reference/song3.f32"));
    }

    #[test]
    fn repeats_and_two_tracks_sound_together() {
        let mut one = song("t240 [2 c4]");
        let mut two = song("t240 [2 c4]; o4 [2 c4]");
        let a = peak(&render(&mut one, 12));
        let b = peak(&render(&mut two, 12));
        assert!(b > a * 1.2, "{a} {b}");
        assert_eq!(two.track_count(), 2);
    }

    #[test]
    fn modules_voices_and_envelope_tables() {
        // A noise channel (%2) makes sound; so does a #@ voice on %6, and a volume table.
        let mut noise = song("t240 %2 c");
        assert!(peak(&render(&mut noise, 4)) > 0.01);
        let mut fm = song("#OPM@1 { 4 0  31 0 0 15 0 20 0 1 0 0 0 };\nt240 %6@1 o5 c");
        assert!(peak(&render(&mut fm, 4)) > 0.01);
        // na{...}: an amplitude table that is silent starts silent.
        let mut quiet = song("t240 na{0} c");
        let mut loud = song("t240 c");
        assert!(peak(&render(&mut quiet, 4)) < peak(&render(&mut loud, 4)) * 0.5);
    }

    #[test]
    fn volume_pan_and_gates_change_what_is_heard() {
        let level = |mml: &str| peak(&render(&mut song(mml), 6));
        assert!(level("t240 v4 c") < level("t240 v12 c") * 0.8);
        // q: a short gate ends the note early.
        let mut s = song("t120 q1 c");
        let out = render(&mut s, 10);
        let l: Vec<f64> = out.iter().step_by(2).copied().collect();
        let mut full = song("t120 q8 c");
        let fl: Vec<f64> = render(&mut full, 10).iter().step_by(2).copied().collect();
        assert!(peak(&l[8000..20000]) < peak(&fl[8000..20000]) * 0.6, "{} {}", peak(&l[8000..20000]), peak(&fl[8000..20000]));
        // p0 is left only.
        let out = render(&mut song("t240 p0 c"), 4);
        let (l, r): (f64, f64) = (out.iter().step_by(2).fold(0.0, |m, x| m.max(x.abs())), out.iter().skip(1).step_by(2).fold(0.0, |m, x| m.max(x.abs())));
        assert!(l > 0.01 && r < 1e-6, "{l} {r}");
    }

    #[test]
    fn it_renders_to_samples() {
        let samples = render_mml("t120 o5 cdefgab<c", 1.0).unwrap();
        assert_eq!(samples.len(), 44100 * 2);
        assert!(samples.iter().any(|x| x.abs() > 0.05));
        assert!(render_mml("c ]", 1.0).is_err());
    }

    /// The MML strings in SiON's own tutorials, all of which play.
    #[test]
    fn songs_from_sions_samples_play_without_trouble() {
        let songs = [
            "t132; %6@0o3l8$c2cc.c.; %6@1o3$rcrc; %6@2v8l16$[crccrrcc]; %6@3v8o3$[rc8r8];",
            "#EFFECT0{ws95lf4000}; %6@0o3l8c2cc.c.; %6@1o3rcrc; %6@2v8l16[crccrrcc]; %6@3v8o3[rc8r8]",
            "#EFFECT0{ws90};%5v24@0s63o6g16*o3c192",
            "%6@0l16o3$aa<c8>a<d8>a<d+8d+edc>ag; %10q8o5$c1",
            "%t0,1,1 t100 l8 [ccggaag4 ffeeddc4 | [ggffeed4]2 ]2",
            "t100 l8 [ccggaag4 ffeeddc4 | [ggffeed4]2 ]2",
            "l16v8o6cg<c>g1",
            "o6q1l16$c<c>g<g>;o4l4s28,-32q0$c",
            "t100 l8 nt{|0,4,7} o5 cdefgab<c",
        ];
        for mml in songs {
            let out = render_mml(mml, 2.0).unwrap_or_else(|e| panic!("{mml}: {e}"));
            assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{mml}");
            // Most of them make sound at once; the sampler one (%10) has no sample
            // to play without a sound reference.
            if !mml.contains("%10") && !mml.contains("%6@") {
                assert!(out.iter().any(|x| x.abs() > 0.001), "{mml}");
            }
        }
    }

    #[test]
    fn a_wav_table_is_a_waveform_a_track_can_play() {
        // A square of eight numbers on %4 (custom tables), and the same on a plain sine: they differ.
        let wave = "#WAV0{100,100,100,100,-100,-100,-100,-100};\nt240 %4@0 o5 c";
        let mut a = song(wave);
        let mut b = song("t240 %5@0 o5 c");
        let (oa, ob) = (render(&mut a, 4), render(&mut b, 4));
        assert!(peak(&oa) > 0.01);
        let diff: f64 = oa.iter().zip(&ob).map(|(x, y)| (x - y).abs()).sum();
        assert!(diff > 5.0, "{diff}");
        // Hex bytes too.
        let mut c = song("#WAVB1{ 40 40 40 40 c0 c0 c0 c0 };\nt240 %4@1 o5 c");
        assert!(peak(&render(&mut c, 4)) > 0.01);
    }

    #[test]
    fn a_plucked_string_rings_and_dies_away() {
        // %11 is Karplus-Strong. A long note: loud at the start, decaying.
        let mut s = song("t120 %11 o4 c1");
        let out = render(&mut s, 40);
        let l: Vec<f64> = out.iter().step_by(2).copied().collect();
        let early = peak(&l[1000..8000]);
        let late = peak(&l[60000..70000]);
        assert!(early > 0.02, "{early}");
        assert!(late < early * 0.7, "{early} {late}");
        // Its pitch: o4 c is 261.6 Hz, a period of 168.6 samples (the best lag of the self-similarity).
        let t = &l[10000..16000];
        let score = |lag: usize| -> f64 { (0..4000).map(|i| t[i] * t[i + lag]).sum() };
        let best = (100..400).max_by(|&a, &b| score(a).total_cmp(&score(b))).unwrap();
        assert!((best as f64 - 168.6).abs() < 3.0 || (best as f64 - 337.2).abs() < 4.0, "{best}");
    }

    #[test]
    fn vmode_changes_how_loud_a_volume_step_is() {
        // A linear v8 is half the amplitude; on n88's 32 dB scale half the step is about 16 dB down.
        let level = |mml: &str| peak(&render(&mut song(mml), 6));
        let linear = level("t240 v8 c") / level("t240 v16 c");
        let n88 = level("#VMODE{n88};\nt240 v8 c") / level("#VMODE{n88};\nt240 v16 c");
        assert!(linear < 0.7, "{linear}");
        assert!(n88 < linear * 0.6 && n88 > 0.05, "{n88} {linear}");
    }

    #[test]
    fn effects_are_applied_to_the_master_and_to_the_sends() {
        // A master low-pass takes the highs off a bright note.
        let dull = peak(&render(&mut song("#EFFECT0{lf300};\nt240 o7 %5@4 c"), 6));
        let bright = peak(&render(&mut song("t240 o7 %5@4 c"), 6));
        assert!(dull < bright * 0.5, "{dull} {bright}");
        // A send: the dry track is quiet when it sends everything to slot 1 and the slot has an echo.
        let mut with = song("#EFFECT1{delay150,0,0,100};\nt240 @v0,128 o5 c4 r4 r4");
        let out = render(&mut with, 14);
        let l: Vec<f64> = out.iter().step_by(2).copied().collect();
        // Only the echo is heard: nothing at the very start, the note after the delay.
        assert!(peak(&l[0..2000]) < 0.001, "{}", peak(&l[0..2000]));
        assert!(peak(&l[7500..11000]) > 0.01, "{}", peak(&l[7500..11000]));
    }

    #[test]
    fn a_song_renders_to_where_it_ends_and_a_loop_gives_its_length() {
        // t120 l4: two quarters are one second; the song ends at 44100 frames.
        let r = render_song("t120 o5 cd").unwrap();
        assert!((r.end as i64 - 44100).abs() < 4, "{}", r.end);
        assert_eq!(r.samples.len() / 2, r.end + 3 * 44100);
        // A track that repeats forever still has an end: where it first comes round.
        let r = render_song("t120 $cdef").unwrap();
        assert!((r.end as i64 - 88200).abs() < 4, "{}", r.end);
        // Two tracks: the longer one ends the song.
        let r = render_song("t120 c; t120 cccc").unwrap();
        assert!((r.end as i64 - 4 * 22050).abs() < 4, "{}", r.end);
        let r = render_song("#TITLE Hi;\nt120 c").unwrap();
        assert_eq!(r.title, "Hi");
        assert!(render_song("r").is_err());
    }

    // %7 and %10 with the sounds of a reference table

    fn sine_sound(hz: f64, seconds: f64, amp: f32) -> Vec<f32> {
        (0..(44100.0 * seconds) as usize).map(|i| (i as f64 / 44100.0 * hz * std::f64::consts::TAU).sin() as f32 * amp).collect()
    }

    fn refs(items: &[(&str, Vec<f32>)]) -> Arc<SoundReference> {
        let mut r = SoundReference::new();
        for (n, v) in items {
            r.insert_mono(n, v);
        }
        Arc::new(r)
    }

    /// Rising zero crossings a second of the left side between two times.
    fn hz_of(out: &[f32], from: f64, to: f64) -> f64 {
        let l: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let (a, b) = ((from * 44100.0) as usize, (to * 44100.0) as usize);
        let n = l[a..b].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        n as f64 / (to - from)
    }

    fn energy(out: &[f32], from: f64, to: f64) -> f64 {
        let (a, b) = ((from * 88200.0) as usize, ((to * 88200.0) as usize).min(out.len()));
        out[a..b].iter().map(|x| f64::from(x.abs())).sum::<f64>() / (b - a).max(1) as f64
    }

    #[test]
    fn the_pcm_module_plays_its_wave_at_the_pitch_of_the_key() {
        let r = refs(&[("w", sine_sound(440.0, 1.5, 0.8))]);
        let mml = |note: &str| format!("#PCMWAVE0{{w,69,0,127,1}};\nt120 l1 %7@0 o5 {note}");
        let a = render_mml_with(&mml("a"), 1.2, r.clone()).unwrap();
        let hz = hz_of(&a, 0.05, 0.3);
        assert!(energy(&a, 0.05, 0.3) > 0.01, "silent");
        assert!((hz - 440.0).abs() < 8.0, "{hz}");
        // An octave up is twice as fast; a fifth below, 2/3.
        let b = render_mml_with(&mml("<a"), 1.2, r.clone()).unwrap();
        assert!((hz_of(&b, 0.05, 0.3) - 880.0).abs() < 15.0, "{}", hz_of(&b, 0.05, 0.3));
        let c = render_mml_with(&mml("d"), 1.2, r).unwrap();
        assert!((hz_of(&c, 0.05, 0.3) - 440.0 * 2f64.powf(-7.0 / 12.0)).abs() < 8.0, "{}", hz_of(&c, 0.05, 0.3));
    }

    #[test]
    fn the_pcm_module_is_silent_without_a_wave_and_stops_at_the_end_of_one() {
        // No reference: the command does nothing, the channel has no wave.
        let a = render_mml("#PCMWAVE0{w,69};\nt120 l1 %7@0 o5 a", 1.0).unwrap();
        assert!(energy(&a, 0.0, 1.0) < 1e-6);
        // A short wave plays once.
        let r = refs(&[("w", sine_sound(440.0, 0.2, 0.8))]);
        let b = render_mml_with("#PCMWAVE0{w,69,0,127,1};\nt120 l1 %7@0 o5 a", 1.5, r).unwrap();
        assert!(energy(&b, 0.02, 0.15) > 0.01);
        assert!(energy(&b, 0.5, 1.2) < 1e-4, "{}", energy(&b, 0.5, 1.2));
    }

    #[test]
    fn a_pcm_wave_with_a_loop_goes_on() {
        let r = refs(&[("w", sine_sound(440.0, 0.3, 0.8))]);
        // start 0, end 8820, loop from sample 4410
        let a = render_mml_with("#PCMWAVE0{w,69,0,127,1,0,8820,4410};\nt120 l1 %7@0 o5 a", 1.5, r).unwrap();
        assert!(energy(&a, 0.8, 1.2) > 0.01);
    }

    #[test]
    fn pcm_key_scales_make_a_voice_softer_and_panned() {
        let r = refs(&[("w", sine_sound(440.0, 1.0, 0.8))]);
        let plain = render_mml_with("#PCMWAVE0{w,69,0,127,1};\nt120 l1 %7@0 o5 a", 1.0, r.clone()).unwrap();
        let soft = render_mml_with("#PCMWAVE0{w,69,0,127,1};\n#PCMVOICE0{80,0,100};\nt120 l1 %7@0 o5 a", 1.0, r.clone()).unwrap();
        assert!(energy(&soft, 0.1, 0.8) < energy(&plain, 0.1, 0.8) * 0.5, "{} {}", energy(&soft, 0.1, 0.8), energy(&plain, 0.1, 0.8));
        let panned = render_mml_with("#PCMWAVE0{w,69,0,127,1};\n#PCMVOICE0{,,,64,0,128};\nt120 l1 %7@0 o5 a", 1.0, r).unwrap();
        // Key 69 is above the pan centre: it goes to the right.
        let (mut l, mut rr) = (0.0, 0.0);
        for f in panned[8820..30000].chunks(2) {
            l += f64::from(f[0].abs());
            rr += f64::from(f[1].abs());
        }
        assert!(rr > l * 1.5, "{l} {rr}");
    }

    #[test]
    fn the_quant_matches_siON_run_in_flash() {
        matches_flash("#QUANT8;t120 l8 %5@4 o4 cdefg", include_bytes!("reference/quant.f32"));
    }

    #[test]
    fn the_quant4_matches_siON_run_in_flash() {
        matches_flash("#QUANT4;t120 l8 q4 %5@4 o4 cdefg", include_bytes!("reference/quant4.f32"));
    }

    #[test]
    fn the_rev_matches_siON_run_in_flash() {
        matches_flash("#REV octave;t120 l8 %5@4 o4 c<c>c<c>", include_bytes!("reference/rev.f32"));
    }

    #[test]
    fn the_fx_slot2_matches_siON_run_in_flash() {
        matches_flash("#EFFECT2{delay};t120 l8 @v64,0,64 %5@4 o4 cdefg", include_bytes!("reference/fx_slot2.f32"));
    }

    #[test]
    fn the_fx_slot3_matches_siON_run_in_flash() {
        matches_flash("#EFFECT3{chorus};t120 l8 @v64,0,0,64 %5@2 o4 cdefg", include_bytes!("reference/fx_slot3.f32"));
    }

    #[test]
    fn the_fx_slot_mix_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{reverb};#EFFECT2{delay};t120 l8 @v64,32,100 %5@4 o4 cdefg", include_bytes!("reference/fx_slot_mix.f32"));
    }

    #[test]
    fn the_fx_slot7_matches_siON_run_in_flash() {
        matches_flash("#EFFECT7{dist};t120 l8 @v64,0,0,0,0,0,0,64 %5@2 o4 cdefg", include_bytes!("reference/fx_slot7.f32"));
    }

    #[test]
    fn the_fx_vowel_matches_siON_run_in_flash() {
        matches_flash("#EFFECT1{vowel};t120 l8 @v64,64 %5@2 o4 cdefg", include_bytes!("reference/fx_vowel.f32"));
    }

    #[test]
    fn the_pipe_o_matches_siON_run_in_flash() {
        matches_flash("t120 l8 @o1 %5@4 o4 cdefg", include_bytes!("reference/pipe_o.f32"));
    }

    #[test]
    fn the_pipe_io_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %5@4 @o1 o4 cde; t120 l8 %6@0 @i1 o4 cde", include_bytes!("reference/pipe_io.f32"));
    }

    #[test]
    fn the_pt_e_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %e1 %5@4 o4 cdefg", include_bytes!("reference/pt_e.f32"));
    }

    #[test]
    fn the_pt_t_matches_siON_run_in_flash() {
        matches_flash("t120 l8 %t1 %5@4 o4 cdefg", include_bytes!("reference/pt_t.f32"));
    }

    #[test]
    fn the_fm_conn_matches_siON_run_in_flash() {
        matches_flash("#FM{4(1>2>3>4)};t120 l8 %6@0 o4 cdefg", include_bytes!("reference/fm_conn.f32"));
    }

    #[test]
    fn the_macro_s_matches_siON_run_in_flash() {
        matches_flash("#MACRO dynamic;#A=cde;t120 l8 %5@4 o4 A fg", include_bytes!("reference/macro_s.f32"));
    }

    #[test]
    fn the_sampler_plays_a_sample_on_its_key_and_stops_on_note_off() {
        let r = refs(&[("s", sine_sound(300.0, 1.0, 0.8))]);
        let mml = "#SAMPLER60{s,,0,1};\nt120 %10 o5 c8 r2.";
        let a = render_mml_with(mml, 1.5, r.clone()).unwrap();
        assert!(energy(&a, 0.05, 0.2) > 0.01, "silent");
        // The note is an eighth long: the sample stops with it.
        assert!(energy(&a, 0.4, 1.2) < 1e-4, "{}", energy(&a, 0.4, 1.2));
        // A key with no sample is silent.
        let b = render_mml_with("#SAMPLER60{s,,0,1};\nt120 %10 o5 d8 r2.", 1.0, r.clone()).unwrap();
        assert!(energy(&b, 0.0, 1.0) < 1e-6);
        // Ignoring note off, it plays to its end.
        let c = render_mml_with("#SAMPLER60{s,1,0,1};\nt120 %10 o5 c8 r2.", 1.5, r).unwrap();
        assert!(energy(&c, 0.4, 0.9) > 0.01);
        assert!(energy(&c, 1.2, 1.5) < 1e-4);
    }

    #[test]
    fn a_looping_sample_goes_on_until_note_off() {
        let r = refs(&[("s", sine_sound(300.0, 0.4, 0.8))]);
        // start 0, end 17640, loop point 8820; a note of 2 s
        let a = render_mml_with("#SAMPLER60{s,,0,1,0,17640,8820};\nt120 %10 o5 l1 c", 2.0, r).unwrap();
        assert!(energy(&a, 1.0, 1.8) > 0.01);
    }
}
