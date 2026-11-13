// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.

//! The waves of the PCM module (`%7`) and the sampler (`%10`):
//! `SiOPMWavePCMData`, `SiOPMWavePCMTable`, `SiOPMWaveSamplerData` and
//! `SiOPMWaveSamplerTable`, the log transform of `SiONUtil`, and the
//! sound reference table the `#SAMPLER`, `#PCMWAVE` and `#PCMVOICE`
//! commands read their waves from.
//!
//! In SiON the waves are Flash `Sound` objects, which text can't carry:
//! `SiONDriver.setSoundReferenceTable` hands in a hash of them before the
//! MML is compiled. [`SoundReference`] is that hash: sounds by name, each
//! interleaved stereo at 44100 Hz, as Flash's `Sound.extract` gives them.
//! Without a reference the three commands do nothing, as in SiON.

use std::collections::HashMap;
use std::sync::Arc;

use super::table::{calc_log_table_index, to_i32, Table, ENV_TOP, LOG_TABLE_BOTTOM, LOG_TABLE_RESOLUTION, LOG_VOLUME_BITS, NOTE_TABLE_SIZE};

/// The longest PCM wave taken from a sound, in frames
/// (`SiOPMWavePCMData.maxSampleLengthFromSound`).
pub const MAX_PCM_FRAMES: usize = 1_048_576;
/// A sampler bank.
pub const SAMPLER_TABLE_MAX: usize = 4;
/// PCM waves.
pub const PCM_DATA_MAX: usize = 128;

/// A sound: stereo frames interleaved, 44100 Hz.
pub struct Sound {
    pub samples: Vec<f32>,
}

impl Sound {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
}

/// `soundReference`: the sounds the commands name.
#[derive(Default, Clone)]
pub struct SoundReference {
    sounds: HashMap<String, Arc<Sound>>,
}

impl SoundReference {
    pub fn new() -> SoundReference {
        SoundReference::default()
    }

    /// A sound of interleaved stereo frames (44100 Hz).
    pub fn insert_stereo(&mut self, name: &str, interleaved: Vec<f32>) {
        let mut s = interleaved;
        if s.len() % 2 == 1 {
            s.pop();
        }
        self.sounds.insert(name.to_string(), Arc::new(Sound { samples: s }));
    }

    /// A mono sound, on both sides.
    pub fn insert_mono(&mut self, name: &str, mono: &[f32]) {
        self.insert_stereo(name, mono.iter().flat_map(|&x| [x, x]).collect());
    }

    pub fn get(&self, name: &str) -> Option<&Arc<Sound>> {
        self.sounds.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }
}

/// `SiONUtil.logTransByteArray`: a sound as log table indices (`%7`'s
/// samples), amplified to the loudest peak when `maximize`.
pub fn log_trans(sound: &Sound, channels: i32, max_frames: usize, maximize: bool) -> Vec<i32> {
    let frames = sound.frames().min(max_frames);
    let mut logmax = LOG_TABLE_BOTTOM;
    let mut dst: Vec<i32>;
    if channels == 2 {
        dst = Vec::with_capacity(frames * 2);
        for i in 0..frames * 2 {
            let v = calc_log_table_index(f64::from(sound.samples[i]));
            logmax = logmax.min(v);
            dst.push(v);
        }
    } else {
        dst = Vec::with_capacity(frames);
        for i in 0..frames {
            let v = calc_log_table_index((f64::from(sound.samples[i * 2]) + f64::from(sound.samples[i * 2 + 1])) * 0.5);
            logmax = logmax.min(v);
            dst.push(v);
        }
    }
    if maximize && logmax > 1 {
        let gain = logmax & !1;
        for v in dst.iter_mut() {
            *v -= gain;
        }
    }
    dst
}

/// `SiONUtil.extract`: a sound as numbers, stereo as it is or mono by `(L+R)*0.6`.
pub fn extract(sound: &Sound, channels: i32, max_frames: usize) -> Vec<f64> {
    let frames = sound.frames().min(max_frames);
    if channels == 2 {
        sound.samples[..frames * 2].iter().map(|&x| f64::from(x)).collect()
    } else {
        (0..frames).map(|i| (f64::from(sound.samples[i * 2]) + f64::from(sound.samples[i * 2 + 1])) * 0.6).collect()
    }
}

// ---------------------------------------------------------------- PCM

/// `SiOPMWavePCMData`: one wave of `%7`.
pub struct PcmData {
    /// Log table indices, mono or interleaved stereo.
    pub wavelet: Arc<Vec<i32>>,
    pub channel_count: i32,
    /// The pitch (note << 6) the wave was sampled at.
    pub sampling_pitch: i32,
    pub start_point: i32,
    pub end_point: i32,
    pub loop_point: i32,
}

impl PcmData {
    /// From a sound, sliced: `new SiOPMWavePCMData(sound, int(note*64), 2, channels)` and `slice`.
    pub fn from_sound(table: &Table, sound: &Sound, sampling_pitch: i32, channels: i32, start: i32, end: i32, loop_point: i32) -> PcmData {
        let ch = if channels == 1 { 1 } else { 2 };
        let wavelet = log_trans(sound, ch, MAX_PCM_FRAMES, true);
        let mut d = PcmDraft { wavelet, channel_count: ch, start: 0, end: 0, loop_point: -1 };
        d.end = d.sample_count() - 1;
        d.slice(table, start, end, loop_point);
        PcmData { wavelet: Arc::new(d.wavelet), channel_count: ch, sampling_pitch, start_point: d.start, end_point: d.end, loop_point: d.loop_point }
    }

    /// From log indices already made.
    pub fn from_wavelet(wavelet: Vec<i32>, channels: i32, sampling_pitch: i32) -> PcmData {
        let ch = if channels == 1 { 1 } else { 2 };
        let end = (wavelet.len() >> (ch - 1)) as i32 - 1;
        PcmData { wavelet: Arc::new(wavelet), channel_count: ch, sampling_pitch, start_point: 0, end_point: end, loop_point: -1 }
    }

    pub fn sample_count(&self) -> i32 {
        (self.wavelet.len() >> (self.channel_count - 1)) as i32
    }
}

/// The wave while it is sliced (it is changed by the loop's cross fade).
struct PcmDraft {
    wavelet: Vec<i32>,
    channel_count: i32,
    start: i32,
    end: i32,
    loop_point: i32,
}

impl PcmDraft {
    fn sample_count(&self) -> i32 {
        (self.wavelet.len() >> (self.channel_count - 1)) as i32
    }

    /// `slice`/`_slice`.
    fn slice(&mut self, table: &Table, start: i32, end: i32, loop_point: i32) {
        self.start = start;
        self.end = end;
        self.loop_point = loop_point;
        if self.start < 0 {
            self.start = self.seek_head_silence();
        }
        if self.loop_point < -1 {
            if self.end >= 0 {
                self.loop_tail_samples(table, -self.loop_point, 0, true);
                if self.start >= self.end {
                    self.end = self.start;
                }
            } else {
                self.loop_tail_samples(table, -self.loop_point, -self.end, true);
            }
        } else {
            let length = self.sample_count();
            if self.end < 0 {
                self.end += self.seek_end_gap();
            } else if self.end < self.start {
                self.end = self.start;
            } else if length < self.end {
                self.end = length - 1;
            }
            if self.loop_point != -1 && self.loop_point < self.start {
                self.loop_point = self.start;
            } else if self.end < self.loop_point {
                self.loop_point = -1;
            }
        }
    }

    fn seek_head_silence(&self) -> i32 {
        let threshold = LOG_TABLE_BOTTOM - LOG_TABLE_RESOLUTION * 14;
        let i = self.wavelet.iter().position(|&w| w < threshold).unwrap_or(self.wavelet.len());
        (i >> (self.channel_count - 1)) as i32
    }

    fn seek_end_gap(&self) -> i32 {
        let threshold = LOG_TABLE_BOTTOM - LOG_TABLE_RESOLUTION * 2;
        let mut i = self.wavelet.len() as i64 - 1;
        while i > 0 {
            if self.wavelet[i as usize] < threshold {
                break;
            }
            i -= 1;
        }
        ((i >> (self.channel_count - 1)) - 100) as i32
    }

    /// `loopTailSamples`: the last `sample_count` samples loop, cross faded into the ones before.
    fn loop_tail_samples(&mut self, table: &Table, sample_count: i32, tail_margin: i32, cross_fade: bool) {
        self.end = self.seek_end_gap() - tail_margin;
        if self.end < self.start + sample_count {
            if self.end < self.start {
                self.end = self.start;
            }
            self.loop_point = self.start;
            return;
        }
        self.loop_point = self.end - sample_count;
        if cross_fade && self.loop_point > self.start + sample_count {
            let envtop = (-ENV_TOP) << 3;
            let i2n = 1.0 / f64::from(1i32 << LOG_VOLUME_BITS);
            let shift = self.channel_count - 1;
            let offset = (self.loop_point << shift) as usize;
            let imax = (sample_count << shift) as usize;
            let dt = 1.5707963267948965 / imax as f64;
            let mut sin = Vec::with_capacity(imax);
            let mut t = 0.0;
            for _ in 0..imax {
                sin.push(f64::sin(t));
                t += dt;
            }
            let log = &table.log_table;
            let at = |i: i32| -> f64 { log.get(i.max(0) as usize).copied().map_or(0.0, f64::from) };
            for i in 0..imax {
                let idx0 = offset + i;
                if idx0 < imax || idx0 >= self.wavelet.len() {
                    continue;
                }
                let idx1 = idx0 - imax;
                let li0 = self.wavelet[idx0] + envtop;
                let li1 = self.wavelet[idx1] + envtop;
                let j = imax - 1 - i;
                self.wavelet[idx0] = calc_log_table_index((at(li0) * sin[j] + at(li1) * sin[i]) * i2n);
            }
        }
    }
}

/// `SiOPMWavePCMTable`: the waves of a `%7` tone by key, with a volume and a pan for each.
#[derive(Clone)]
pub struct PcmTable {
    pub table: Vec<Option<Arc<PcmData>>>,
    pub volume: Vec<f64>,
    pub pan: Vec<i32>,
}

impl Default for PcmTable {
    fn default() -> Self {
        PcmTable { table: vec![None; NOTE_TABLE_SIZE], volume: vec![1.0; NOTE_TABLE_SIZE], pan: vec![0; NOTE_TABLE_SIZE] }
    }
}

impl PcmTable {
    pub fn new() -> PcmTable {
        PcmTable::default()
    }

    /// `setSample`: a wave for a range of keys (`to` -1: the one key).
    pub fn set_sample(&mut self, data: Arc<PcmData>, from: i32, to: i32) -> bool {
        let from = from.max(0);
        let mut to = to.min(127);
        if to == -1 {
            to = from;
        }
        if from > 127 || to < 0 || to < from {
            return false;
        }
        for i in from..=to {
            self.table[i as usize] = Some(data.clone());
        }
        true
    }

    /// `setKeyScaleVolume`.
    pub fn set_key_scale_volume(&mut self, center: i32, key_range: f64, volume_range: f64) {
        let volume_range = volume_range * 0.0078125;
        let imin = to_i32(f64::from(center) - key_range * 0.5).max(0) as usize;
        let imax = (to_i32(f64::from(center) + key_range * 0.5).max(0) as usize).min(NOTE_TABLE_SIZE);
        let dv = if key_range == 0.0 { volume_range } else { volume_range / key_range };
        let n = NOTE_TABLE_SIZE;
        let mut i = 0;
        if volume_range > 0.0 {
            let mut v = 1.0 - volume_range;
            while i < imin.min(n) {
                self.volume[i] = v;
                i += 1;
            }
            while i < imax {
                self.volume[i] = v;
                v += dv;
                i += 1;
            }
            while i < n {
                self.volume[i] = 1.0;
                i += 1;
            }
        } else {
            let mut v = 1.0;
            while i < imin.min(n) {
                self.volume[i] = 1.0;
                i += 1;
            }
            while i < imax {
                self.volume[i] = v;
                v += dv;
                i += 1;
            }
            v = 1.0 + volume_range;
            while i < n {
                self.volume[i] = v;
                i += 1;
            }
        }
    }

    /// `setKeyScalePan`.
    pub fn set_key_scale_pan(&mut self, center: i32, key_range: f64, pan_width: f64) {
        let imin = (to_i32(f64::from(center) - key_range * 0.5).max(0) as usize).min(NOTE_TABLE_SIZE);
        let imax = (to_i32(f64::from(center) + key_range * 0.5).max(0) as usize).min(NOTE_TABLE_SIZE);
        let dp = if key_range == 0.0 { pan_width } else { pan_width / key_range };
        let mut p = -pan_width * 0.5;
        let mut i = 0;
        while i < imin {
            self.pan[i] = to_i32(p);
            i += 1;
        }
        while i < imax {
            self.pan[i] = to_i32(p);
            p += dp;
            i += 1;
        }
        p = pan_width * 0.5;
        while i < NOTE_TABLE_SIZE {
            self.pan[i] = to_i32(p);
            i += 1;
        }
    }
}

// ---------------------------------------------------------------- sampler

/// `SiOPMWaveSamplerData`: one sample of `%10`, played from its start point to its end.
pub struct SamplerData {
    /// Numbers in [-1, 1], mono or interleaved stereo.
    pub wave: Vec<f64>,
    pub channel_count: i32,
    pub pan: i32,
    pub start_point: i32,
    pub end_point: i32,
    pub loop_point: i32,
    pub ignore_note_off: bool,
}

impl SamplerData {
    /// `new SiOPMWaveSamplerData(sound, ignoreNoteOff, pan, 2, channels)` and `slice`.
    pub fn from_sound(sound: &Sound, ignore_note_off: bool, pan: i32, channels: i32, start: i32, end: i32, loop_point: i32) -> SamplerData {
        let ch = if channels == 1 { 1 } else { 2 };
        // A sound is extracted whole; SiON streams the longer ones from the Sound itself,
        // with the same samples.
        let wave = extract(sound, ch, usize::MAX);
        let mut d = SamplerData { wave, channel_count: ch, pan, start_point: start, end_point: end, loop_point, ignore_note_off: false };
        d.slice();
        // `ignoreNoteOff` is set after the loop point is: a looping sample ends on note off.
        d.ignore_note_off = d.loop_point == -1 && ignore_note_off;
        d
    }

    /// Numbers given as they are.
    pub fn from_wave(wave: Vec<f64>, channels: i32, ignore_note_off: bool, pan: i32) -> SamplerData {
        let ch = if channels == 1 { 1 } else { 2 };
        let mut d = SamplerData { wave, channel_count: ch, pan, start_point: 0, end_point: 0, loop_point: -1, ignore_note_off };
        d.end_point = d.length();
        d
    }

    pub fn length(&self) -> i32 {
        (self.wave.len() >> (self.channel_count - 1)) as i32
    }

    /// `getInitialSampleIndex`.
    pub fn initial_sample_index(&self, phase: f64) -> i32 {
        to_i32(f64::from(self.start_point) * (1.0 - phase) + f64::from(self.end_point) * phase)
    }

    /// `_slice`.
    fn slice(&mut self) {
        if self.start_point < 0 {
            self.start_point = self.seek_head_silence();
        }
        if self.loop_point < 0 {
            self.loop_point = -1;
        }
        if self.end_point < 0 {
            self.end_point = self.seek_end_gap();
        }
        if self.end_point < self.loop_point {
            self.loop_point = -1;
        }
        if self.end_point < self.start_point {
            self.end_point = self.length() - 1;
        }
    }

    /// `_seekHeadSilence`: where a 0.5 ms window first holds sound. SiON's
    /// result can be negative (22 before a loud first sample); it is kept
    /// from going below the start of the wave here.
    fn seek_head_silence(&self) -> i32 {
        let mut ring = [0.0f64; 22];
        let mut at = 0usize;
        let mut ms = 0.0;
        let w = &self.wave;
        let mut i = 0usize;
        if self.channel_count == 1 {
            while i < w.len() {
                ms -= ring[at];
                at = (at + 1) % 22;
                ring[at] = w[i] * w[i];
                ms += ring[at];
                if ms > 0.0011 {
                    break;
                }
                i += 1;
            }
        } else {
            while i < w.len() {
                ms -= ring[at];
                at = (at + 1) % 22;
                let l = w[i] * w[i];
                let r = w.get(i + 1).map_or(0.0, |x| x * x);
                ring[at] = l + r;
                i += 2;
                ms += ring[at];
                if ms > 0.0022 {
                    break;
                }
            }
            i >>= 1;
        }
        (i as i32 - 22).max(0)
    }

    /// `_seekEndGap`.
    fn seek_end_gap(&self) -> i32 {
        let w = &self.wave;
        let length = self.length();
        let mut i: i64;
        if self.channel_count == 1 {
            i = w.len() as i64 - 1;
            while i >= 0 {
                if w[i as usize] * w[i as usize] > 0.0001 {
                    break;
                }
                i -= 1;
            }
        } else {
            i = w.len() as i64 - 1;
            while i >= 0 {
                let r = w[i as usize];
                i -= 1;
                let l = if i >= 0 { w[i as usize] } else { 0.0 };
                i -= 1;
                if r * r + l * l > 0.0002 {
                    break;
                }
            }
            i >>= 1;
        }
        if i > i64::from(length) - 1152 {
            i as i32
        } else {
            length - 1152
        }
    }
}

/// `SiOPMWaveSamplerTable`: the samples of a bank by key.
#[derive(Clone)]
pub struct SamplerTable {
    pub table: Vec<Option<Arc<SamplerData>>>,
}

impl Default for SamplerTable {
    fn default() -> Self {
        SamplerTable { table: vec![None; NOTE_TABLE_SIZE] }
    }
}

impl SamplerTable {
    pub fn new() -> SamplerTable {
        SamplerTable::default()
    }

    /// `setSample`: for the keys `from` to `to` (-1: the one key).
    pub fn set_sample(&mut self, data: Arc<SamplerData>, from: i32, to: i32) -> bool {
        let from = from.max(0);
        let mut to = to.min(127);
        if to == -1 {
            to = from;
        }
        if from > 127 || to < 0 || to < from {
            return false;
        }
        for i in from..=to {
            self.table[i as usize] = Some(data.clone());
        }
        true
    }

    pub fn get_sample(&self, n: i32) -> Option<Arc<SamplerData>> {
        self.table.get(n as usize).and_then(|s| s.clone())
    }
}

/// What a song's `%7` and `%10` play: the PCM tables by number and the sampler banks.
#[derive(Default)]
pub struct SampleBank {
    pub pcm: Vec<Option<Arc<PcmTable>>>,
    pub sampler: Vec<Arc<SamplerTable>>,
}

impl SampleBank {
    pub fn new() -> SampleBank {
        SampleBank { pcm: vec![None; PCM_DATA_MAX], sampler: (0..SAMPLER_TABLE_MAX).map(|_| Arc::new(SamplerTable::new())).collect() }
    }

    /// `getPCMData`.
    pub fn pcm_table(&self, index: i32) -> Option<Arc<PcmTable>> {
        self.pcm.get((index & (PCM_DATA_MAX as i32 - 1)) as usize).and_then(|t| t.clone())
    }

    pub fn sampler_table(&self, bank: usize) -> Option<Arc<SamplerTable>> {
        self.sampler.get(bank).cloned()
    }

    /// The PCM table of a number, made if it isn't (`_getPCMVoice`).
    pub fn pcm_table_mut(&mut self, index: i32) -> &mut PcmTable {
        let i = (index & (PCM_DATA_MAX as i32 - 1)) as usize;
        let t = self.pcm[i].get_or_insert_with(|| Arc::new(PcmTable::new()));
        Arc::make_mut(t)
    }

    pub fn sampler_table_mut(&mut self, bank: usize) -> &mut SamplerTable {
        let b = bank & (SAMPLER_TABLE_MAX - 1);
        Arc::make_mut(&mut self.sampler[b])
    }
}

// ---------------------------------------------------------------- commands

fn args_of(dat: &str) -> Vec<String> {
    // mml.split(/\s*,\s*/g)
    dat.split(',').map(|s| s.trim().to_string()).collect()
}

fn arg_int(args: &[String], i: usize, default: i32) -> i32 {
    match args.get(i) {
        Some(s) if !s.is_empty() => {
            let t = s.trim();
            let (neg, d) = match t.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, t.strip_prefix('+').unwrap_or(t)),
            };
            let end = d.find(|c: char| !c.is_ascii_digit()).unwrap_or(d.len());
            let n = d[..end].parse::<i64>().unwrap_or(0).min(i64::from(i32::MAX)) as i32;
            if neg {
                -n
            } else {
                n
            }
        }
        _ => default,
    }
}

fn arg_num(args: &[String], i: usize, default: f64) -> f64 {
    match args.get(i) {
        Some(s) if !s.is_empty() => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => default,
    }
}

/// `Translator.parseSamplerWave`: `#SAMPLER n{id, ignoreNoteOff, pan, channels, start, end, loop}`.
/// False when the sound isn't in the reference.
pub fn parse_sampler_wave(bank: &mut SamplerTable, note: i32, mml: &str, refs: &SoundReference, _table: &Table) -> bool {
    let args = args_of(mml);
    let id = args.first().cloned().unwrap_or_default();
    // Boolean(string) is true for any string that isn't empty.
    let ignore = args.get(1).is_some_and(|s| !s.is_empty());
    let pan = arg_int(&args, 2, 0);
    let channels = arg_int(&args, 3, 2);
    let (start, end, lp) = (arg_int(&args, 4, -1), arg_int(&args, 5, -1), arg_int(&args, 6, -1));
    let Some(sound) = refs.get(&id) else { return false };
    let d = SamplerData::from_sound(sound, ignore, pan, channels, start, end, lp);
    bank.set_sample(Arc::new(d), note, -1);
    true
}

/// `Translator.parsePCMWave`: `#PCMWAVE n{id, samplingNote, keyFrom, keyTo, channels, start, end, loop}`.
pub fn parse_pcm_wave(pcm: &mut PcmTable, mml: &str, refs: &SoundReference, table: &Table) -> bool {
    let args = args_of(mml);
    let id = args.first().cloned().unwrap_or_default();
    let sampling_note = arg_int(&args, 1, 69);
    let (from, to) = (arg_int(&args, 2, 0), arg_int(&args, 3, 127));
    let channels = arg_int(&args, 4, 2);
    let (start, end, lp) = (arg_int(&args, 5, -1), arg_int(&args, 6, -1), arg_int(&args, 7, -1));
    let Some(sound) = refs.get(&id) else { return false };
    let d = PcmData::from_sound(table, sound, sampling_note * 64, channels, start, end, lp);
    pcm.set_sample(Arc::new(d), from, to);
    true
}

/// `Translator.parsePCMVoice`: `#PCMVOICE n{volNote, volKeyRange, volRange, panNote, panKeyRange, panWidth, ...}`.
/// The envelope arguments and the postfix belong to the voice, which an MML
/// song never plays (its `%7` takes the table only), so only the key scales are kept.
pub fn parse_pcm_voice(pcm: &mut PcmTable, mml: &str) {
    let args = args_of(mml);
    pcm.set_key_scale_volume(arg_num(&args, 0, 64.0) as i32, arg_num(&args, 1, 0.0), arg_num(&args, 2, 0.0));
    pcm.set_key_scale_pan(arg_num(&args, 3, 64.0) as i32, arg_num(&args, 4, 0.0), arg_num(&args, 5, 0.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table::new(44100)
    }

    fn sine(n: usize, cycle: f64, amp: f32) -> Vec<f32> {
        (0..n).map(|i| (i as f64 / cycle * std::f64::consts::TAU).sin() as f32 * amp).collect()
    }

    #[test]
    fn a_log_transform_is_amplified_to_the_peak() {
        let mut s = SoundReference::new();
        s.insert_mono("quiet", &sine(400, 40.0, 0.1));
        s.insert_mono("loud", &sine(400, 40.0, 0.9));
        let a = log_trans(s.get("quiet").unwrap(), 1, 1000, true);
        let b = log_trans(s.get("loud").unwrap(), 1, 1000, true);
        // The same shape, brought to the same peak.
        let min = |v: &[i32]| v.iter().copied().min().unwrap();
        assert_eq!(min(&a) >> 1, min(&b) >> 1);
        assert!(min(&a) <= 3);
        // Without it the quiet one stays quiet.
        let c = log_trans(s.get("quiet").unwrap(), 1, 1000, false);
        assert!(min(&c) > 400);
    }

    #[test]
    fn a_pcm_wave_is_sliced_to_where_the_sound_is() {
        let mut v = vec![0.0f32; 300];
        v.extend(sine(2000, 50.0, 0.8));
        v.extend(vec![0.0; 800]);
        let mut r = SoundReference::new();
        r.insert_mono("w", &v);
        let d = PcmData::from_sound(&table(), r.get("w").unwrap(), 69 * 64, 1, -1, -1, -1);
        assert!((250..=320).contains(&d.start_point), "{}", d.start_point);
        assert!(d.end_point > 1500 && d.end_point < 2300, "{}", d.end_point);
        assert_eq!(d.loop_point, -1);
        // A negative loop point loops the tail, cross faded.
        let l = PcmData::from_sound(&table(), r.get("w").unwrap(), 69 * 64, 1, -1, -1, -300);
        assert!(l.loop_point > l.start_point && l.loop_point < l.end_point);
    }

    #[test]
    fn a_sampler_sample_is_sliced_and_keeps_its_note_off_unless_it_loops() {
        let mut v = vec![0.0f32; 3000];
        v.extend(sine(4000, 60.0, 0.7));
        v.extend(vec![0.0; 5000]);
        let mut r = SoundReference::new();
        r.insert_mono("s", &v);
        let d = SamplerData::from_sound(r.get("s").unwrap(), true, 0, 2, -1, -1, -1);
        assert!(d.start_point > 2500 && d.start_point <= 3000, "{}", d.start_point);
        assert!(d.end_point > 6500 && d.end_point < 12000, "{}", d.end_point);
        assert!(d.ignore_note_off);
        let l = SamplerData::from_sound(r.get("s").unwrap(), true, 0, 2, 3000, 7000, 4000);
        assert!(!l.ignore_note_off);
    }

    #[test]
    fn the_commands_need_their_sounds_in_the_reference() {
        let t = table();
        let mut refs = SoundReference::new();
        refs.insert_mono("a", &sine(1000, 50.0, 0.5));
        let mut bank = SamplerTable::new();
        assert!(parse_sampler_wave(&mut bank, 60, "a,,10,1", &refs, &t));
        assert!(bank.get_sample(60).is_some_and(|s| s.pan == 10 && s.channel_count == 1));
        assert!(bank.get_sample(61).is_none());
        assert!(!parse_sampler_wave(&mut bank, 61, "nothing", &refs, &t));
        let mut p = PcmTable::new();
        assert!(parse_pcm_wave(&mut p, "a,69,0,127,1", &refs, &t));
        assert!(p.table[0].is_some() && p.table[127].is_some());
        assert_eq!(p.table[60].as_ref().unwrap().sampling_pitch, 69 * 64);
        parse_pcm_voice(&mut p, "64,0,64,64,0,40");
        assert!(p.volume[0] < 1.0 && p.volume[100] == 1.0);
        assert!(p.pan[0] < 0 && p.pan[127] > 0);
    }
}
