// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's SiOPMModule, SiOPMStream, SiOPMOperatorParam and
// SiOPMChannelParam (org.si.sion.module, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! The module the channels write into: the pipes (rings of samples one
//! block long, five of them), the output stream and the effect sends'
//! streams, and the parameter sets a voice is made of.

use super::table::*;
use std::sync::Arc;

/// Streams a channel sends to (0 is the master).
pub const STREAM_SEND_SIZE: usize = 8;
/// Pipes the module has (4 is the standard output's).
pub const PIPE_SIZE: usize = 5;

/// `SiOPMOperatorParam`.
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorParam {
    pub pg_type: i32,
    pub pt_type: usize,
    pub ar: i32,
    pub dr: i32,
    pub sr: i32,
    pub rr: i32,
    pub sl: i32,
    pub tl: i32,
    pub ksr: i32,
    pub ksl: i32,
    pub fmul: i32,
    pub dt1: i32,
    pub detune: i32,
    pub ams: i32,
    pub phase: i32,
    pub fixed_pitch: i32,
    pub mute: bool,
    pub ssgec: i32,
    pub mod_level: i32,
    pub erst: bool,
}

impl Default for OperatorParam {
    fn default() -> OperatorParam {
        OperatorParam {
            pg_type: PG_SINE as i32,
            pt_type: PT_OPM,
            ar: 63,
            dr: 0,
            sr: 0,
            rr: 63,
            sl: 0,
            tl: 0,
            ksr: 1,
            ksl: 0,
            fmul: 128,
            dt1: 0,
            detune: 0,
            ams: 0,
            phase: 0,
            fixed_pitch: 0,
            mute: false,
            ssgec: 0,
            mod_level: 5,
            erst: false,
        }
    }
}

impl OperatorParam {
    /// `mul` as a setter: the fine multiple of a multiple.
    pub fn set_mul(&mut self, m: i32) {
        self.fmul = if m != 0 { m << 7 } else { 64 };
    }

    pub fn mul(&self) -> i32 {
        (self.fmul >> 7) & 15
    }

    /// `setPGType`: the pulse generator, and its pitch table.
    pub fn set_pg_type(&mut self, table: &Table, t: i32) {
        self.pg_type = t & 511;
        self.pt_type = table.get_wave_table(self.pg_type as usize).default_pt_type;
    }
}

/// `SiOPMChannelParam`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelParam {
    pub operator_param: [OperatorParam; 4],
    pub ope_count: i32,
    pub alg: i32,
    pub fb: i32,
    pub fbc: i32,
    pub fratio: i32,
    pub lfo_wave_shape: i32,
    pub lfo_freq_step: i32,
    pub amd: i32,
    pub pmd: i32,
    pub volumes: [f64; STREAM_SEND_SIZE],
    pub pan: i32,
    pub filter_type: i32,
    pub cutoff: i32,
    pub resonance: i32,
    pub far: i32,
    pub fdr1: i32,
    pub fdr2: i32,
    pub frr: i32,
    pub fdc1: i32,
    pub fdc2: i32,
    pub fsc: i32,
    pub frc: i32,
    /// The `initSequence` of an MML voice (`#@{...}` with a postfix).
    pub init_sequence: String,
}

impl Default for ChannelParam {
    fn default() -> ChannelParam {
        let mut volumes = [0.0; STREAM_SEND_SIZE];
        volumes[0] = 0.5;
        ChannelParam {
            operator_param: Default::default(),
            ope_count: 1,
            alg: 0,
            fb: 0,
            fbc: 0,
            fratio: 100,
            lfo_wave_shape: LFO_WAVE_TRIANGLE as i32,
            lfo_freq_step: 12126,
            amd: 0,
            pmd: 0,
            volumes,
            pan: 64,
            filter_type: 0,
            cutoff: 128,
            resonance: 0,
            far: 0,
            fdr1: 0,
            fdr2: 0,
            frr: 0,
            fdc1: 128,
            fdc2: 64,
            fsc: 32,
            frc: 128,
            init_sequence: String::new(),
        }
    }
}

/// The module: pipes, the output stream and the sends' streams.
pub struct Module {
    pub table: Arc<Table>,
    pub init_operator_param: OperatorParam,
    pub buffer_length: usize,
    /// The five pipes, each `buffer_length` long (rings).
    pub pipes: Vec<Vec<i32>>,
    /// Stream slots: 0 the output, then the effect sends'; interleaved
    /// stereo, `buffer_length * 2` each, or none.
    pub slots: Vec<Option<Vec<f64>>>,
    /// `SiOPMStream.channels` of each slot: 1 when its effect chain starts with a mono effector (the
    /// writes then add left and right together and ignore the pan), else 2.
    pub slot_channels: Vec<i32>,
    pub pcm_volume: f64,
    pub sampler_volume: f64,
    pub bit_rate: i32,
}

impl Module {
    /// A module for `buffer_length` samples a block (a power of two).
    pub fn new(table: Arc<Table>, buffer_length: usize) -> Module {
        assert!(buffer_length.is_power_of_two());
        let mut slots: Vec<Option<Vec<f64>>> = vec![None; STREAM_SEND_SIZE];
        slots[0] = Some(vec![0.0; buffer_length * 2]);
        Module {
            table,
            init_operator_param: OperatorParam::default(),
            buffer_length,
            pipes: vec![vec![0; buffer_length]; PIPE_SIZE],
            slots,
            slot_channels: vec![2; STREAM_SEND_SIZE],
            pcm_volume: 4.0,
            sampler_volume: 2.0,
            bit_rate: 0,
        }
    }

    /// `_beginProcess`: the streams cleared.
    pub fn begin_process(&mut self) {
        for s in self.slots.iter_mut().flatten() {
            s.fill(0.0);
        }
    }

    /// `_endProcess`: the output limited to [-1, 1] (and quantized).
    pub fn end_process(&mut self) {
        let bit_rate = self.bit_rate;
        if let Some(out) = self.slots[0].as_mut() {
            for n in out.iter_mut() {
                *n = n.clamp(-1.0, 1.0);
            }
            if bit_rate != 0 {
                let r = f64::from(1i32 << bit_rate);
                let ir = 2.0 / r;
                for n in out.iter_mut() {
                    *n = f64::from(to_i32(*n * r) >> 1) * ir;
                }
            }
        }
    }

    /// The output as interleaved stereo.
    pub fn output(&self) -> &[f64] {
        self.slots[0].as_deref().unwrap_or(&[])
    }

    /// `SiOPMStream.write`: `len` samples from a pipe (from `start`, a ring)
    /// added to a slot's stream at the same place, by volume and pan.
    pub fn write_stream(&mut self, slot: usize, pipe: usize, start: usize, len: usize, vol: f64, pan: i32) {
        let mask = self.buffer_length - 1;
        let Some(buf) = self.slots.get_mut(slot).and_then(|s| s.as_mut()) else { return };
        let vol = vol * self.table.i2n;
        let vl = self.table.pan_table[(128 - pan).clamp(0, 128) as usize] * vol;
        let vr = self.table.pan_table[pan.clamp(0, 128) as usize] * vol;
        let src = &self.pipes[pipe];
        let mono = self.slot_channels[slot] == 1;
        for k in 0..len {
            let n = f64::from(src[(start + k) & mask]);
            let i = ((start + k) & mask) << 1;
            if mono {
                buf[i] += n * vol;
                buf[i + 1] += n * vol;
            } else {
                buf[i] += n * vl;
                buf[i + 1] += n * vr;
            }
        }
    }
}

impl Module {
    /// `SiOPMStream.writeStereo`: two pipes, left and right, added to a slot's stream.
    pub fn write_stereo(&mut self, slot: usize, pipe_l: usize, pipe_r: usize, start: usize, len: usize, vol: f64, pan: i32) {
        let mask = self.buffer_length - 1;
        let Some(buf) = self.slots.get_mut(slot).and_then(|s| s.as_mut()) else { return };
        let vol = vol * self.table.i2n;
        let vl = self.table.pan_table[(128 - pan).clamp(0, 128) as usize] * vol;
        let vr = self.table.pan_table[pan.clamp(0, 128) as usize] * vol;
        let mono = self.slot_channels[slot] == 1;
        for k in 0..len {
            let i = ((start + k) & mask) << 1;
            let (l, r) = (f64::from(self.pipes[pipe_l][(start + k) & mask]), f64::from(self.pipes[pipe_r][(start + k) & mask]));
            if mono {
                let n = (l + r) * vol * 0.5;
                buf[i] += n;
                buf[i + 1] += n;
            } else {
                buf[i] += l * vl;
                buf[i + 1] += r * vr;
            }
        }
    }

    /// `SiOPMStream.writeVectorNumber`: `len` samples of a wave (mono or
    /// interleaved stereo) from `start_pointer`, added to a slot's stream
    /// from `start_buffer`.
    pub fn write_vector(&mut self, slot: usize, wave: &[f64], start_pointer: usize, start_buffer: usize, len: usize, vol: f64, pan: i32, sample_channels: i32) {
        let Some(buf) = self.slots.get_mut(slot).and_then(|s| s.as_mut()) else { return };
        let vl = self.table.pan_table[(128 - pan).clamp(0, 128) as usize];
        let vr = self.table.pan_table[pan.clamp(0, 128) as usize];
        let at = |j: usize| wave.get(j).copied().unwrap_or(0.0);
        let mono = self.slot_channels[slot] == 1;
        let mut i = start_buffer << 1;
        for k in 0..len {
            if i + 1 >= buf.len() {
                break;
            }
            if mono {
                let n = if sample_channels == 2 {
                    let j = (start_pointer + k) << 1;
                    (at(j) + at(j + 1)) * vol * 0.5
                } else {
                    at(start_pointer + k) * vol
                };
                buf[i] += n;
                buf[i + 1] += n;
            } else if sample_channels == 2 {
                let j = (start_pointer + k) << 1;
                buf[i] += at(j) * vl * vol;
                buf[i + 1] += at(j + 1) * vr * vol;
            } else {
                let n = at(start_pointer + k);
                buf[i] += n * vl * vol * 0.707;
                buf[i + 1] += n * vr * vol * 0.707;
            }
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pipe_is_written_to_the_output_by_volume_and_pan() {
        let mut m = Module::new(Arc::new(Table::new(44100)), 64);
        m.pipes[4][0] = 8192;
        m.write_stream(0, 4, 0, 1, 0.5, 64);
        let (l, r) = (m.output()[0], m.output()[1]);
        // Centre pan: both sides sin(pi/4), 8192/2^13*0.5*0.5.
        assert!((l - 0.25 * 0.7071).abs() < 1e-3, "{l}");
        assert!((l - r).abs() < 1e-9);
        // Hard right: the left is silent, the right full.
        m.begin_process();
        m.write_stream(0, 4, 0, 1, 0.5, 128);
        assert!(m.output()[0].abs() < 1e-9 && (m.output()[1] - 0.25).abs() < 1e-3);
    }
}
