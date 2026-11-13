// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
// Ported from FlMML on HTML5 (BSD-3-Clause, notice in mod.rs): MEnvelope,
// MFilter, MFormant, MChannel and MPolyChannel.

//! A FlMML channel: an oscillator with its pitch LFO, two envelopes (the
//! amplitude's and the filter's), a formant and a filter, pipes and sync
//! between channels, portamento; and the polyphonic channel of several.

use super::osc::{i32_of, Fixed, Oscillator, FC_DPCM, FC_NOISE, FC_PULSE, GB_NOISE, GB_S_NOISE, MAX, NOISE, OPM, PHASE_LEN, PULSE, SAMPLE_RATE, SINE};
use super::Tables;
use std::rc::Rc;

pub const PITCH_RESOLUTION: i64 = 100;
const LFO_DELTA: usize = 245;

/// What channels share: the frequency and volume maps, the track buffer,
/// the pipes and the sync lines.
pub struct Shared {
    pub freq: Vec<f64>,
    pub volume: [[f64; 128]; 3],
    pub env_volume: [[f64; 256]; 3],
    pub samples: Vec<f32>,
    pub pipes: Vec<Vec<f32>>,
    pub sync: Vec<Vec<bool>>,
}

impl Shared {
    pub fn new(len: usize, pipes: usize, syncs: usize) -> Shared {
        let r = PITCH_RESOLUTION as f64;
        let freq = (0..128 * PITCH_RESOLUTION as usize).map(|i| 440.0 * 2f64.powf((i as f64 - 69.0 * r) / (12.0 * r))).collect();
        let mut volume = [[0.0; 128]; 3];
        for i in 1..128 {
            let x = i as f64;
            volume[0][i] = x / 127.0;
            volume[1][i] = 10f64.powf((x - 127.0) * (48.0 / (127.0 * 20.0)));
            volume[2][i] = 10f64.powf((x - 127.0) * (96.0 / (127.0 * 20.0)));
        }
        let mut env_volume = [[0.0; 256]; 3];
        for i in 1..256 {
            let x = i as f64;
            env_volume[0][i] = x / 255.0;
            env_volume[1][i] = 10f64.powf((x - 255.0) * (48.0 / (255.0 * 20.0)));
            env_volume[2][i] = 10f64.powf((x - 255.0) * (96.0 / (255.0 * 20.0)));
        }
        Shared { freq, volume, env_volume, samples: vec![0.0; len], pipes: vec![vec![0.0; len]; pipes], sync: vec![vec![false; len]; syncs] }
    }
    pub fn frequency(&self, freq_no: f64) -> f64 {
        let f = i64::from(i32_of(freq_no)).clamp(0, self.freq.len() as i64 - 1);
        self.freq[f as usize]
    }
}

// ---- MEnvelope.

#[derive(Clone)]
pub struct Envelope {
    /// Points: time (samples) and level; the first is (0, 0).
    points: Vec<(f64, f64)>,
    release_time: f64,
    current_val: f64,
    release_step: f64,
    releasing: bool,
    step: f64,
    playing: bool,
    counter: f64,
    time_in_samples: f64,
    current: usize,
}

impl Envelope {
    pub fn new(attack: f64, decay: f64, sustain: f64, release: f64) -> Envelope {
        let mut e = Envelope { points: vec![], release_time: 0.0, current_val: 0.0, release_step: 0.0, releasing: true, step: 0.0, playing: false, counter: 0.0, time_in_samples: 0.0, current: 0 };
        e.set_attack(attack);
        e.add_point(decay, sustain);
        e.set_release(release);
        e.playing = false;
        e.current_val = 0.0;
        e.releasing = true;
        e.release_step = 0.0;
        e
    }
    fn has_next(&self) -> bool {
        self.current + 1 < self.points.len()
    }
    pub fn set_attack(&mut self, attack: f64) {
        self.points = vec![(0.0, 0.0)];
        self.add_point(attack, 1.0);
    }
    pub fn add_point(&mut self, time: f64, level: f64) {
        self.points.push((time * SAMPLE_RATE, level));
    }
    pub fn set_release(&mut self, release: f64) {
        self.release_time = (if release > 0.0 { release } else { 1.0 / 127.0 }) * SAMPLE_RATE;
        if self.playing && !self.releasing {
            self.counter = self.time_in_samples;
            self.current = 0;
            while self.has_next() && self.counter >= self.points[self.current + 1].0 {
                self.current += 1;
                self.counter -= self.points[self.current].0;
            }
            if !self.has_next() {
                self.current_val = self.points[self.current].1;
            } else {
                let (p, n) = (self.points[self.current], self.points[self.current + 1]);
                self.step = (n.1 - p.1) / n.0;
                self.current_val = p.1 + self.step * self.counter;
            }
        }
    }
    pub fn trigger(&mut self, zero_start: bool) {
        self.playing = true;
        self.releasing = false;
        self.current = 0;
        let v = if zero_start { 0.0 } else { self.current_val };
        self.points[0].1 = v;
        self.current_val = v;
        self.step = (1.0 - self.current_val) / self.points[1].0;
        self.time_in_samples = 0.0;
        self.counter = 0.0;
    }
    pub fn release(&mut self) {
        self.releasing = true;
        self.release_step = self.current_val / self.release_time;
    }
    pub fn sound_off(&mut self) {
        self.release();
        self.playing = false;
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    /// One step of the attack, decays and sustain: (whether it moved on
    /// to a point, so the value was set).
    fn advance(&mut self) {
        let mut processed = false;
        while self.counter >= self.points[self.current + 1].0 {
            self.counter = 0.0;
            self.current += 1;
            if !self.has_next() {
                self.current_val = self.points[self.current].1;
                processed = true;
                break;
            }
            let (p, n) = (self.points[self.current], self.points[self.current + 1]);
            self.step = (n.1 - p.1) / n.0;
            self.current_val = p.1;
            processed = true;
        }
        if !processed {
            self.current_val += self.step;
        }
        self.counter += 1.0;
    }
    pub fn next_linear(&mut self) -> f64 {
        if !self.playing {
            return 0.0;
        }
        if !self.releasing {
            if !self.has_next() {
                self.current_val = self.points[self.current].1;
            } else {
                self.advance();
            }
        } else {
            self.current_val -= self.release_step;
        }
        if self.current_val <= 0.0 && self.releasing {
            self.playing = false;
            self.current_val = 0.0;
        }
        self.time_in_samples += 1.0;
        self.current_val
    }
    pub fn amp_linear(&mut self, s: &mut [f32], start: usize, end: usize, velocity: f64) {
        let mut amplitude = self.current_val * velocity;
        for v in &mut s[start..end] {
            if !self.playing {
                *v = 0.0;
                continue;
            }
            if !self.releasing {
                if self.has_next() {
                    self.advance();
                    amplitude = self.current_val * velocity;
                }
            } else {
                self.current_val -= self.release_step;
                amplitude = self.current_val * velocity;
            }
            if self.current_val <= 0.0 && self.releasing {
                self.playing = false;
                self.current_val = 0.0;
                amplitude = 0.0;
            }
            self.time_in_samples += 1.0;
            *v = (f64::from(*v) * amplitude) as f32;
        }
    }
    pub fn amp_nonlinear(&mut self, s: &mut [f32], start: usize, end: usize, velocity: f64, map: &[f64; 256]) {
        for v in &mut s[start..end] {
            if !self.playing {
                *v = 0.0;
                continue;
            }
            if !self.releasing {
                if !self.has_next() {
                    self.current_val = self.points[self.current].1;
                } else {
                    self.advance();
                }
            } else {
                self.current_val -= self.release_step;
            }
            if self.current_val <= 0.0 && self.releasing {
                self.playing = false;
                self.current_val = 0.0;
            }
            self.time_in_samples += 1.0;
            let mut cv = i32_of(self.current_val * 255.0);
            if cv > 255 {
                cv = 0;
            }
            *v = (f64::from(*v) * map[cv.clamp(0, 255) as usize] * velocity) as f32;
        }
    }
}

// ---- MFilter.

#[derive(Clone, Default)]
pub struct Filter {
    t1: f64,
    t2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    b3: f64,
    b4: f64,
    sw: i64,
}

fn small(x: f64) -> bool {
    (-0.000001..=0.000001).contains(&x)
}

impl Filter {
    pub fn reset(&mut self) {
        (self.t1, self.t2, self.b0, self.b1, self.b2, self.b3, self.b4) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    pub fn set_switch(&mut self, s: i64) {
        self.reset();
        self.sw = s;
    }
    pub fn check_to_silence(&self) -> bool {
        match self.sw {
            1 | -1 => small(self.b0) && small(self.b1),
            2 | -2 => [self.t1, self.t2, self.b0, self.b1, self.b2, self.b3, self.b4].iter().all(|&x| small(x)),
            _ => false,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn run(&mut self, sh: &Shared, s: &mut [f32], start: usize, end: usize, env: &mut Envelope, frq: f64, amt: f64, res: f64, key: f64) {
        let k = key * (2.0 * std::f64::consts::PI / (SAMPLE_RATE * 440.0));
        match self.sw {
            1 | -1 => {
                let hp = self.sw == -1;
                let (mut b0, mut b1) = (self.b0, self.b1);
                let cutf = |f: f64| {
                    let mut c = sh.frequency(f) * k;
                    if c < 1.0 / 127.0 {
                        c = 0.0;
                    }
                    if c > 1.0 - 0.0001 {
                        c = 1.0 - 0.0001;
                    }
                    c
                };
                let varying = amt > 0.0001 || amt < -0.0001;
                let mut cut = cutf(frq);
                let mut fb = res + res / (1.0 - cut);
                for v in &mut s[start..end] {
                    if varying {
                        cut = cutf(frq + amt * env.next_linear());
                        fb = res + res / (1.0 - cut);
                    }
                    let input = f64::from(*v);
                    b0 += cut * (input - b0 + fb * (b0 - b1));
                    b1 += cut * (b0 - b1);
                    *v = if hp { (input - b0) as f32 } else { b1 as f32 };
                }
                self.b0 = b0;
                self.b1 = b1;
            }
            2 | -2 => {
                let hp = self.sw == -2;
                let (mut t1, mut t2, mut b0, mut b1, mut b2, mut b3, mut b4) = (self.t1, self.t2, self.b0, self.b1, self.b2, self.b3, self.b4);
                for v in &mut s[start..end] {
                    let mut cut = sh.frequency(frq + amt * env.next_linear()) * k;
                    if cut < 1.0 / 127.0 {
                        cut = 0.0;
                    }
                    if cut > 1.0 {
                        cut = 1.0;
                    }
                    let mut q = 1.0 - cut;
                    let p = cut + 0.8 * cut * q;
                    let f = p + p - 1.0;
                    q = res * (1.0 + 0.5 * q * (1.0 - q + 5.6 * q * q));
                    let mut input = f64::from(*v);
                    input -= q * b4;
                    t1 = b1;
                    b1 = (input + b0) * p - b1 * f;
                    t2 = b2;
                    b2 = (b1 + t1) * p - b2 * f;
                    t1 = b3;
                    b3 = (b2 + t2) * p - b3 * f;
                    b4 = (b3 + t1) * p - b4 * f;
                    b4 = b4 - b4 * b4 * b4 * 0.166667;
                    b0 = input;
                    *v = if hp { (input - b4) as f32 } else { b4 as f32 };
                }
                (self.t1, self.t2, self.b0, self.b1, self.b2, self.b3, self.b4) = (t1, t2, b0, b1, b2, b3, b4);
            }
            _ => {}
        }
    }
}

// ---- MFormant.

const FORMANT: [[f64; 11]; 5] = [
    [0.00000811044, 8.943665402, -36.83889529, 92.01697887, -154.337906, 181.6233289, -151.8651235, 89.09614114, -35.10298511, 8.388101016, -0.923313471],
    [0.00000436215, 8.90438318, -36.55179099, 91.05750846, -152.422234, 179.1170248, -149.6496211, 87.78352223, -34.60687431, 8.282228154, -0.914150747],
    [0.00000333819, 8.893102966, -36.49532826, 90.96543286, -152.4545478, 179.4835618, -150.315433, 88.43409371, -34.98612086, 8.407803364, -0.932568035],
    [0.00000113572, 8.994734087, -37.2084849, 93.22900521, -156.6929844, 184.596544, -154.3755513, 90.49663749, -35.58964535, 8.478996281, -0.929252233],
    [4.09431e-7, 8.997322763, -37.20218544, 93.11385476, -156.2530937, 183.7080141, -153.2631681, 89.59539726, -35.12454591, 8.338655623, -0.910251753],
];

#[derive(Clone, Default)]
pub struct Formant {
    m: [f64; 10],
    vowel: usize,
    power: bool,
}

impl Formant {
    pub fn set_vowel(&mut self, v: i64) {
        self.power = true;
        self.vowel = v as usize;
    }
    pub fn disable(&mut self) {
        self.power = false;
        self.m = [0.0; 10];
    }
    pub fn check_to_silence(&self) -> bool {
        self.power && self.m.iter().all(|&x| small(x))
    }
    pub fn run(&mut self, s: &mut [f32], start: usize, end: usize) {
        if !self.power || self.vowel > 4 {
            return;
        }
        let c = &FORMANT[self.vowel];
        let m = &mut self.m;
        for v in &mut s[start..end] {
            let out = c[0] * f64::from(*v) + c[1] * m[0] + c[2] * m[1] + c[3] * m[2] + c[4] * m[3] + c[5] * m[4] + c[6] * m[5] + c[7] * m[6] + c[8] * m[7] + c[9] * m[8] + c[10] * m[9];
            *v = out as f32;
            for j in (1..10).rev() {
                m[j] = m[j - 1];
            }
            m[0] = f64::from(*v);
        }
    }
}

// ---- MChannel.

/// What the LFO moves.
const LFO_PITCH: i64 = 0;
const LFO_AMPLITUDE: i64 = 1;
const LFO_CUTOFF: i64 = 2;
const LFO_PWM: i64 = 3;
const LFO_FM: i64 = 4;
const LFO_PANPOT: i64 = 5;

pub struct Channel {
    note_no: i64,
    detune: i64,
    freq_no: i64,
    env1: Envelope,
    env2: Envelope,
    osc1: Oscillator,
    osc2: Oscillator,
    osc2_connect: i64,
    osc2_sign: f64,
    filter: Filter,
    filter_connect: i64,
    formant: Formant,
    expression: f64,
    velocity: f64,
    amp_level: f64,
    pan: f64,
    on_counter: i64,
    lfo_delay: f64,
    lfo_depth: f64,
    lfo_end: f64,
    lfo_target: i64,
    lpf_amt: f64,
    lpf_frq: f64,
    lpf_res: f64,
    pulse_width: f64,
    vol_mode: usize,
    in_sens: f64,
    in_pipe: usize,
    out_mode: i64,
    out_pipe: usize,
    ring_sens: f64,
    ring_pipe: usize,
    sync_mode: i64,
    sync_pipe: usize,
    port_depth: f64,
    port_depth_add: f64,
    portamento: i64,
    port_rate: f64,
    last_freq_no: i64,
    slave: bool,
    voice_id: i64,
}

impl Channel {
    pub fn new(sh: &Shared, fixed: &Rc<Fixed>, tables: &Rc<Tables>) -> Channel {
        let mut osc2 = Oscillator::new(fixed, tables);
        osc2.mods[NOISE].disable_reset_phase();
        osc2.set_form(SINE);
        let mut c = Channel {
            note_no: 0,
            detune: 0,
            freq_no: 0,
            env1: Envelope::new(0.0, 60.0 / 127.0, 30.0 / 127.0, 1.0 / 127.0),
            env2: Envelope::new(0.0, 30.0 / 127.0, 0.0, 1.0),
            osc1: Oscillator::new(fixed, tables),
            osc2,
            osc2_connect: 0,
            osc2_sign: 0.0,
            filter: Filter::default(),
            filter_connect: 0,
            formant: Formant::default(),
            expression: 0.0,
            velocity: 0.0,
            amp_level: 0.0,
            pan: 0.0,
            on_counter: 0,
            lfo_delay: 0.0,
            lfo_depth: 0.0,
            lfo_end: 0.0,
            lfo_target: 0,
            lpf_amt: 0.0,
            lpf_frq: 0.0,
            lpf_res: 0.0,
            pulse_width: 0.5,
            vol_mode: 0,
            in_sens: 0.0,
            in_pipe: 0,
            out_mode: 0,
            out_pipe: 0,
            ring_sens: 0.0,
            ring_pipe: 0,
            sync_mode: 0,
            sync_pipe: 0,
            port_depth: 0.0,
            port_depth_add: 0.0,
            portamento: 0,
            port_rate: 0.0,
            last_freq_no: 4800,
            slave: false,
            voice_id: 0,
        };
        c.set_expression(sh, 127);
        c.set_velocity(sh, 100);
        c.set_pan(64);
        c.set_input(0, 0);
        c.set_ring(0, 0);
        c
    }

    fn m1(&mut self) -> &mut super::osc::Osc {
        let i = self.osc1.current();
        &mut self.osc1.mods[i]
    }
    fn m2(&mut self) -> &mut super::osc::Osc {
        let i = self.osc2.current();
        &mut self.osc2.mods[i]
    }

    pub fn set_expression(&mut self, sh: &Shared, ex: i64) {
        self.expression = sh.volume[self.vol_mode][ex.clamp(0, 127) as usize];
        self.amp_level = self.velocity * self.expression;
        let e = self.expression;
        if let Some(o) = self.osc1.mods[OPM].opm() {
            o.set_expression(e);
        }
    }
    pub fn set_velocity(&mut self, sh: &Shared, v: i64) {
        self.velocity = sh.volume[self.vol_mode][v.clamp(0, 127) as usize];
        self.amp_level = self.velocity * self.expression;
        if let Some(o) = self.osc1.mods[OPM].opm() {
            o.set_velocity(v as i32);
        }
    }
    pub fn set_note_no(&mut self, sh: &Shared, note: i64, tie: bool) {
        self.note_no = note;
        self.freq_no = self.note_no * PITCH_RESOLUTION + self.detune;
        let f = sh.frequency(self.freq_no as f64);
        self.m1().set_frequency(f);
        if self.portamento == 1 {
            if !tie {
                self.port_depth = (self.last_freq_no - self.freq_no) as f64;
            } else {
                self.port_depth += (self.last_freq_no - self.freq_no) as f64;
            }
            self.port_depth_add = if self.port_depth < 0.0 { self.port_rate } else { self.port_rate * -1.0 };
        }
        self.last_freq_no = self.freq_no;
    }
    pub fn set_detune(&mut self, sh: &Shared, d: i64) {
        self.detune = d;
        self.freq_no = self.note_no * PITCH_RESOLUTION + self.detune;
        let f = sh.frequency(self.freq_no as f64);
        self.m1().set_frequency(f);
    }
    pub fn note_no(&self) -> i64 {
        self.note_no
    }
    pub fn is_playing(&self) -> bool {
        if self.osc1.form == OPM {
            self.osc1.mods[OPM].opm_ref().is_some_and(|o| o.playing())
        } else {
            self.env1.is_playing()
        }
    }
    pub fn id(&self) -> i64 {
        self.voice_id
    }
    pub fn note_on_with_id(&mut self, sh: &Shared, note: i64, vel: i64, id: i64) {
        self.voice_id = id;
        self.note_on(sh, note, vel);
    }
    pub fn note_on(&mut self, sh: &Shared, note: i64, vel: i64) {
        self.set_note_no(sh, note, false);
        self.env1.trigger(false);
        self.env2.trigger(true);
        self.m1().reset_phase();
        self.m2().reset_phase();
        self.filter.reset();
        self.set_velocity(sh, vel);
        self.on_counter = 0;
        let pw = self.pulse_width;
        self.osc1.mods[PULSE].set_pwm(pw);
        let n = self.note_no;
        for m in [FC_NOISE, GB_NOISE, GB_S_NOISE, FC_DPCM, OPM] {
            self.osc1.mods[m].set_note_no(n);
        }
    }
    pub fn note_off(&mut self, note: i64) {
        if note < 0 || note == self.note_no {
            self.env1.release();
            self.env2.release();
            if let Some(o) = self.osc1.mods[OPM].opm() {
                o.note_off();
            }
        }
    }
    pub fn sound_off(&mut self) {
        self.env1.sound_off();
        self.env2.sound_off();
    }
    pub fn close(&mut self) {
        let n = self.note_no;
        self.note_off(n);
        self.filter.set_switch(0);
    }
    pub fn set_noise_freq(&mut self, f: f64) {
        self.osc1.mods[NOISE].set_noise_freq_white(1.0 - f * (1.0 / 128.0));
    }
    pub fn set_form(&mut self, form: i64, sub: i64) {
        self.osc1.set_form(form.max(0) as usize);
        self.m1().set_wave_no(sub);
    }
    pub fn set_env1_atk(&mut self, a: f64) {
        self.env1.set_attack(a / 127.0);
    }
    pub fn set_env1_point(&mut self, t: f64, l: f64) {
        self.env1.add_point(t / 127.0, l / 127.0);
    }
    pub fn set_env1_rel(&mut self, r: f64) {
        self.env1.set_release(r / 127.0);
    }
    pub fn set_env2_atk(&mut self, a: f64) {
        self.env2.set_attack(a / 127.0);
    }
    pub fn set_env2_point(&mut self, t: f64, l: f64) {
        self.env2.add_point(t / 127.0, l / 127.0);
    }
    pub fn set_env2_rel(&mut self, r: f64) {
        self.env2.set_release(r / 127.0);
    }
    pub fn set_pwm(&mut self, pwm: f64) {
        if self.osc1.form != FC_PULSE {
            let mut pwm = pwm;
            self.osc1.mods[PULSE].set_mix(pwm < 0.0);
            if pwm < 0.0 {
                pwm *= -1.0;
            }
            self.pulse_width = pwm * 0.01;
            let pw = self.pulse_width;
            self.osc1.mods[PULSE].set_pwm(pw);
        } else {
            self.osc1.mods[FC_PULSE].set_pwm(0.125 * pwm.abs().floor());
        }
    }
    pub fn set_pan(&mut self, pan: i64) {
        self.pan = ((pan - 1) as f64 * (0.5 / 63.0)).max(0.0);
    }
    pub fn set_formant(&mut self, vowel: i64) {
        if vowel >= 0 {
            self.formant.set_vowel(vowel);
        } else {
            self.formant.disable();
        }
    }
    pub fn set_lfo_fmsf(&mut self, form: i64, sub: i64) {
        let f = if form >= 0 { form - 1 } else { -form - 1 };
        self.osc2.set_form(f.max(0) as usize);
        self.m2().set_wave_no(sub);
        self.osc2_sign = if form >= 0 { 1.0 } else { -1.0 };
        let form = form.abs() - 1;
        if form >= MAX as i64 {
            self.osc2_connect = 0;
        }
    }
    pub fn set_lfo_dpwd(&mut self, depth: f64, freq: f64) {
        self.lfo_depth = depth;
        self.osc2_connect = if depth == 0.0 { 0 } else { 1 };
        self.m2().set_frequency(freq);
        self.m2().reset_phase();
        self.osc2.mods[NOISE].set_noise_freq_white(freq / SAMPLE_RATE);
    }
    pub fn set_lfo_dltm(&mut self, delay: f64, time: f64) {
        self.lfo_delay = delay;
        self.lfo_end = if time > 0.0 { self.lfo_delay + time } else { 0.0 };
    }
    pub fn set_lfo_target(&mut self, t: i64) {
        self.lfo_target = t;
    }
    pub fn set_lpf_swt_amt(&mut self, swt: i64, amt: i64) {
        if -3 < swt && swt < 3 && swt != self.filter_connect {
            self.filter_connect = swt;
            self.filter.set_switch(swt);
        }
        self.lpf_amt = (amt.clamp(-127, 127) * PITCH_RESOLUTION) as f64;
    }
    pub fn set_lpf_frq_res(&mut self, frq: i64, res: i64) {
        self.lpf_frq = (frq.clamp(0, 127) * PITCH_RESOLUTION) as f64;
        self.lpf_res = (res as f64 * (1.0 / 127.0)).clamp(0.0, 1.0);
    }
    pub fn set_vol_mode(&mut self, m: i64) {
        if (0..=2).contains(&m) {
            self.vol_mode = m as usize;
        }
    }
    pub fn set_input(&mut self, i: i64, p: i64) {
        self.in_sens = shifted(i) * (1.0 / 8.0) * PHASE_LEN as f64;
        self.in_pipe = p.max(0) as usize;
    }
    pub fn set_output(&mut self, o: i64, p: i64) {
        self.out_mode = o;
        self.out_pipe = p.max(0) as usize;
    }
    pub fn set_ring(&mut self, s: i64, p: i64) {
        self.ring_sens = shifted(s) / 8.0;
        self.ring_pipe = p.max(0) as usize;
    }
    pub fn set_sync(&mut self, m: i64, p: i64) {
        self.sync_mode = m;
        self.sync_pipe = p.max(0) as usize;
    }
    pub fn set_portamento(&mut self, depth: f64, len: f64) {
        self.portamento = 0;
        self.port_depth = depth;
        self.port_depth_add = (self.port_depth.floor() / len) * -1.0;
    }
    pub fn set_midi_port(&mut self, mode: i64) {
        self.portamento = mode;
        self.port_depth = 0.0;
    }
    pub fn set_midi_port_rate(&mut self, rate: f64) {
        self.port_rate = rate;
    }
    pub fn set_port_base(&mut self, base: i64) {
        self.last_freq_no = base;
    }
    pub fn set_hw_lfo(&mut self, data: i64) {
        if let Some(fm) = self.osc1.mods[OPM].opm() {
            fm.set_wf(((data >> 27) & 3) as i32);
            fm.set_lfrq(((data >> 19) & 0xff) as i32);
            fm.set_pmd(((data >> 12) & 0x7f) as i32);
            fm.set_amd(((data >> 5) & 0x7f) as i32);
            fm.set_pms_ams(((data >> 2) & 7) as i32, (data & 3) as i32);
        }
    }
    pub fn reset(&mut self, sh: &Shared) {
        self.sound_off();
        self.pulse_width = 0.5;
        self.voice_id = 0;
        self.set_form(0, 0);
        self.set_detune(sh, 0);
        self.set_expression(sh, 127);
        self.set_velocity(sh, 100);
        self.set_pan(64);
        self.set_vol_mode(0);
        self.set_noise_freq(0.0);
        self.set_lfo_fmsf(0, 0);
        self.osc2_connect = 0;
        self.on_counter = 0;
        self.lfo_target = 0;
        self.lfo_delay = 0.0;
        self.lfo_depth = 0.0;
        self.lfo_end = 0.0;
        self.set_lpf_swt_amt(0, 0);
        self.set_lpf_frq_res(0, 0);
        self.set_formant(-1);
        self.set_input(0, 0);
        self.set_output(0, 0);
        self.set_ring(0, 0);
        self.set_sync(0, 0);
        self.port_depth = 0.0;
        self.port_depth_add = 0.0;
        self.last_freq_no = 4800;
        self.portamento = 0;
        self.port_rate = 0.0;
    }
    pub fn set_slave(&mut self, s: bool) {
        self.slave = s;
    }
    pub fn clear_out_pipe(&mut self, sh: &mut Shared, start: usize, delta: usize) {
        if self.out_mode == 1 {
            let p = &mut sh.pipes[self.out_pipe];
            let n = p.len();
            p[start..(start + delta).min(n)].fill(0.0);
        }
    }

    fn lfo_on(&self) -> bool {
        let c = self.on_counter as f64;
        c >= self.lfo_delay && (self.lfo_end == 0.0 || c < self.lfo_end)
    }

    /// The pulse's width from the LFO, clamped as FlMML clamps it.
    fn lfo_pwm(&mut self, depth: f64, block: usize) -> f64 {
        let mut pwm = self.pulse_width;
        if self.lfo_on() {
            pwm += self.m2().next(None, None) * depth;
            if block > 0 {
                self.m2().add_phase(block as i64 - 1);
            }
        }
        pwm.clamp(0.0, 100.0)
    }

    /// One step of portamento: by a block's samples (`block`), or one.
    fn port_step(&mut self, freq_no: &mut f64, by: f64) {
        if self.port_depth != 0.0 {
            *freq_no += f64::from(i32_of(self.port_depth));
            self.port_depth += self.port_depth_add * by;
            if self.port_depth * self.port_depth_add > 0.0 {
                self.port_depth = 0.0;
            }
        }
    }

    /// The oscillator's samples for `[s, e)`: plain, synced in or out.
    fn osc_block(&mut self, sh: &mut Shared, buf: Buf, s: usize, e: usize) {
        let cur = self.osc1.current();
        let (a, b) = self.osc1.mods.split_at_mut(NOISE);
        let (noise_osc, rest) = b.split_first_mut().expect("the noise oscillator");
        let (osc, noise) = if cur == NOISE {
            (noise_osc, None)
        } else if cur < NOISE {
            (&mut a[cur], if cur == PULSE { Some(noise_osc) } else { None })
        } else {
            (&mut rest[cur - NOISE - 1], None)
        };
        let (out, sync): (&mut [f32], Option<(&mut [bool], bool)>) = match (buf, self.sync_mode) {
            (Buf::Samples, 1) => (&mut sh.samples, Some((&mut sh.sync[self.sync_pipe], true))),
            (Buf::Samples, 2) => (&mut sh.samples, Some((&mut sh.sync[self.sync_pipe], false))),
            (Buf::Samples, _) => (&mut sh.samples, None),
            (Buf::Pipe(p), 1) => {
                let (pipes, sync) = (&mut sh.pipes, &mut sh.sync);
                (&mut pipes[p], Some((&mut sync[self.sync_pipe], true)))
            }
            (Buf::Pipe(p), 2) => {
                let (pipes, sync) = (&mut sh.pipes, &mut sh.sync);
                (&mut pipes[p], Some((&mut sync[self.sync_pipe], false)))
            }
            (Buf::Pipe(p), _) => (&mut sh.pipes[p], None),
        };
        osc.samples(out, s, e, noise, sync);
    }

    fn osc_next_ofs(&mut self, ofs: f64) -> f64 {
        let cur = self.osc1.current();
        let (a, b) = self.osc1.mods.split_at_mut(NOISE);
        let (noise_osc, rest) = b.split_first_mut().expect("the noise oscillator");
        let (osc, noise) = if cur == NOISE {
            (noise_osc, None)
        } else if cur < NOISE {
            (&mut a[cur], if cur == PULSE { Some(noise_osc) } else { None })
        } else {
            (&mut rest[cur - NOISE - 1], None)
        };
        osc.next(Some(ofs), noise)
    }

    /// Every one of FlMML's getSamples variants: pipe in (F), sync in (I)
    /// or out (O), or neither; the LFO on pitch (P), pulse width (W), FM
    /// depth (F) or none; portamento or not.
    fn wave(&mut self, sh: &mut Shared, buf: Buf, start: usize, end: usize) {
        let lfo = if self.osc2_connect == 0 { -1 } else { self.lfo_target };
        let port = self.port_depth != 0.0;
        if self.in_sens >= 0.000001 {
            // Per sample, through the input pipe.
            let sens0 = self.in_sens;
            let pipe_in = self.in_pipe;
            let depth = self.osc2_sign * self.lfo_depth;
            let set_once = !port && lfo != LFO_PITCH || (!port && lfo == -1);
            if set_once && (lfo == -1 || lfo == LFO_PWM || lfo == LFO_FM || lfo != LFO_PITCH) {
                let f = sh.frequency(self.freq_no as f64);
                self.m1().set_frequency(f);
            }
            let counts = lfo == LFO_PITCH || lfo == LFO_PWM || lfo == LFO_FM;
            for i in start..end {
                let mut freq_no = self.freq_no as f64;
                if port {
                    self.port_step(&mut freq_no, 1.0);
                }
                let mut sens = sens0;
                match lfo {
                    LFO_PITCH => {
                        if self.lfo_on() {
                            freq_no += f64::from(i32_of(self.m2().next(None, None) * depth));
                        }
                    }
                    LFO_PWM => {
                        let pwm = self.lfo_pwm(self.osc2_sign * self.lfo_depth * 0.01, 0);
                        if port {
                            let f = sh.frequency(freq_no);
                            self.m1().set_frequency(f);
                        }
                        self.osc1.mods[PULSE].set_pwm(pwm);
                    }
                    LFO_FM => {
                        if self.lfo_on() {
                            sens *= self.m2().next(None, None) * (self.osc2_sign * self.lfo_depth * (1.0 / 127.0));
                        }
                    }
                    _ => {}
                }
                if lfo == LFO_PITCH || port && lfo != LFO_PWM {
                    let f = sh.frequency(freq_no);
                    self.m1().set_frequency(f);
                }
                let input = f64::from(sh.pipes[pipe_in][i]) * sens;
                let v = self.osc_next_ofs(input);
                match buf {
                    Buf::Samples => sh.samples[i] = v as f32,
                    Buf::Pipe(p) => sh.pipes[p][i] = v as f32,
                }
                if counts {
                    self.on_counter += 1;
                }
            }
            return;
        }
        // Only pitch and pulse width are moved here; any other target plays
        // as if there were no LFO.
        let lfo = if lfo == LFO_PITCH || lfo == LFO_PWM { lfo } else { -1 };
        // No LFO and no portamento: the whole run at once, uncounted.
        if !port && lfo != LFO_PITCH && lfo != LFO_PWM {
            self.osc_block(sh, buf, start, end);
            return;
        }
        let synced = self.sync_mode == 1 || self.sync_mode == 2;
        let mut s = start;
        loop {
            let e = (s + LFO_DELTA).min(end);
            let mut freq_no = self.freq_no as f64;
            if port {
                self.port_step(&mut freq_no, (e - s - 1) as f64);
            }
            match lfo {
                LFO_PITCH => {
                    if self.lfo_on() {
                        let v = self.m2().next(None, None) * self.osc2_sign * self.lfo_depth;
                        // (FlMML's plain channel with portamento adds it unrounded.)
                        freq_no += if port && !synced { v } else { f64::from(i32_of(v)) };
                        self.m2().add_phase((e - s - 1) as i64);
                        if port && self.port_depth * self.port_depth_add > 0.0 {
                            self.port_depth = 0.0;
                        }
                    }
                    let f = sh.frequency(freq_no);
                    self.m1().set_frequency(f);
                }
                LFO_PWM => {
                    if port {
                        let f = sh.frequency(freq_no);
                        self.m1().set_frequency(f);
                    }
                    let pwm = self.lfo_pwm(self.osc2_sign * self.lfo_depth * 0.01, e - s);
                    self.osc1.mods[PULSE].set_pwm(pwm);
                }
                _ => {
                    let f = sh.frequency(freq_no);
                    self.m1().set_frequency(f);
                }
            }
            self.osc_block(sh, buf, s, e);
            // The plain channel with portamento alone doesn't count samples.
            if !(lfo == -1 && !synced) {
                self.on_counter += (e - s) as i64;
            }
            s = e;
            if s >= end {
                break;
            }
        }
        if port && self.port_depth == 0.0 && lfo != LFO_PITCH {
            let f = sh.frequency(self.freq_no as f64);
            self.m1().set_frequency(f);
        }
    }

    /// This channel's samples for `[start, start + delta)`, added to the
    /// stereo mix (or into its output pipe).
    pub fn samples(&mut self, sh: &mut Shared, mix: &mut [Vec<f32>; 2], max: usize, start: usize, delta: usize) {
        let mut end = start + delta;
        if end >= max {
            end = max;
        }
        let key = sh.frequency(self.freq_no as f64);
        let buf = if self.out_mode == 1 && !self.slave { Buf::Pipe(self.out_pipe) } else { Buf::Samples };
        let mut playing = self.is_playing();
        if playing {
            self.wave(sh, buf, start, end);
        }
        let amp = self.amp_level;
        let vm = self.vol_mode;
        let env_map = sh.env_volume[vm];
        {
            let tb: &mut [f32] = match buf {
                Buf::Samples => &mut sh.samples,
                Buf::Pipe(p) => &mut sh.pipes[p],
            };
            if self.osc1.form != OPM {
                if vm == 0 {
                    self.env1.amp_linear(tb, start, end, amp);
                } else {
                    self.env1.amp_nonlinear(tb, start, end, amp, &env_map);
                }
            }
        }
        if self.lfo_target == LFO_AMPLITUDE && self.osc2_connect != 0 {
            let depth = self.osc2_sign * self.lfo_depth / 127.0;
            for i in start..end {
                let mut vol = 1.0;
                if self.lfo_on() {
                    vol += self.m2().next(None, None) * depth;
                }
                if vol < 0.0 {
                    vol = 0.0;
                }
                let tb = buf.get(sh);
                tb[i] = (f64::from(tb[i]) * vol) as f32;
                self.on_counter += 1;
            }
        }
        if playing && self.ring_sens >= 0.000001 {
            let sens = self.ring_sens;
            for i in start..end {
                let r = f64::from(sh.pipes[self.ring_pipe][i]);
                let tb = buf.get(sh);
                tb[i] = (f64::from(tb[i]) * (r * sens)) as f32;
            }
        }
        let was = playing;
        playing = playing || self.formant.check_to_silence();
        if playing != was {
            let tb = buf.get(sh);
            let n = tb.len();
            tb[start..(start + delta).min(n)].fill(0.0);
        }
        if playing {
            self.formant.run(buf.get(sh), start, end);
        }
        let was = playing;
        playing = playing || self.filter.check_to_silence();
        if playing != was {
            let tb = buf.get(sh);
            let n = tb.len();
            tb[start..(start + delta).min(n)].fill(0.0);
        }
        if playing {
            if self.lfo_target == LFO_CUTOFF && self.osc2_connect != 0 {
                let depth = self.osc2_sign * self.lfo_depth;
                let mut s = start;
                loop {
                    let e = (s + LFO_DELTA).min(end);
                    let mut frq = self.lpf_frq;
                    if self.lfo_on() {
                        frq += f64::from(i32_of(self.m2().next(None, None) * depth));
                        self.m2().add_phase((e - s - 1) as i64);
                    }
                    let frq = frq.clamp(0.0, 127.0 * PITCH_RESOLUTION as f64);
                    // (FlMML filters the shared track buffer here, even
                    // when the channel writes a pipe.)
                    let mut tmp = std::mem::take(&mut sh.samples);
                    self.filter.run(sh, &mut tmp, s, e, &mut self.env2, frq, self.lpf_amt, self.lpf_res, key);
                    sh.samples = tmp;
                    self.on_counter += (e - s) as i64;
                    s = e;
                    if s >= end {
                        break;
                    }
                }
            } else {
                let mut tmp = match buf {
                    Buf::Samples => std::mem::take(&mut sh.samples),
                    Buf::Pipe(p) => std::mem::take(&mut sh.pipes[p]),
                };
                self.filter.run(sh, &mut tmp, start, end, &mut self.env2, self.lpf_frq, self.lpf_amt, self.lpf_res, key);
                match buf {
                    Buf::Samples => sh.samples = tmp,
                    Buf::Pipe(p) => sh.pipes[p] = tmp,
                }
            }
        }
        if playing {
            match self.out_mode {
                0 => {
                    if self.lfo_target == LFO_PANPOT && self.osc2_connect != 0 {
                        let depth = self.osc2_sign * self.lfo_depth * (1.0 / 127.0);
                        for i in start..end {
                            let pan = (self.pan + self.m2().next(None, None) * depth).clamp(0.0, 1.0);
                            let a = f64::from(buf.get(sh)[i]) * 0.5;
                            let r = a * pan;
                            mix[0][i] = (f64::from(mix[0][i]) + a - r) as f32;
                            mix[1][i] = (f64::from(mix[1][i]) + r) as f32;
                        }
                    } else {
                        for i in start..end {
                            let a = f64::from(buf.get(sh)[i]) * 0.5;
                            let r = a * self.pan;
                            mix[0][i] = (f64::from(mix[0][i]) + a - r) as f32;
                            mix[1][i] = (f64::from(mix[1][i]) + r) as f32;
                        }
                    }
                }
                1 => {
                    for i in start..end {
                        let v = buf.get(sh)[i];
                        let p = &mut sh.pipes[self.out_pipe];
                        if self.slave {
                            p[i] = (f64::from(p[i]) + f64::from(v)) as f32;
                        } else {
                            p[i] = v;
                        }
                    }
                }
                2 => {
                    for i in start..end {
                        let v = buf.get(sh)[i];
                        let p = &mut sh.pipes[self.out_pipe];
                        p[i] = (f64::from(p[i]) + f64::from(v)) as f32;
                    }
                }
                _ => {}
            }
        } else if self.out_mode == 1 && !self.slave {
            let p = &mut sh.pipes[self.out_pipe];
            let n = p.len();
            p[start..(start + delta).min(n)].fill(0.0);
        }
    }
}

/// `(1 << (n - 1))` as JavaScript computes it, for the pipe sensitivities.
fn shifted(n: i64) -> f64 {
    f64::from(1i32.wrapping_shl(((n - 1) as i32 as u32) & 31))
}

/// Where a channel's samples go before they're mixed.
#[derive(Clone, Copy)]
enum Buf {
    Samples,
    Pipe(usize),
}

impl Buf {
    fn get(self, sh: &mut Shared) -> &mut [f32] {
        match self {
            Buf::Samples => &mut sh.samples,
            Buf::Pipe(p) => &mut sh.pipes[p],
        }
    }
}

// ---- MPolyChannel.

pub struct Poly {
    form: i64,
    sub: i64,
    vol_mode: i64,
    voice_id: i64,
    last: Option<usize>,
    limit: usize,
    voices: Vec<Channel>,
}

/// A track's channel: one voice, or several (`#USING POLY`).
pub enum Chan {
    Mono(Box<Channel>),
    Poly(Poly),
}

macro_rules! each {
    ($self:ident, $c:ident => $e:expr) => {
        match $self {
            Chan::Mono($c) => {
                $e;
            }
            Chan::Poly(p) => {
                for $c in p.voices.iter_mut() {
                    $e;
                }
            }
        }
    };
}

impl Chan {
    pub fn poly(sh: &Shared, fixed: &Rc<Fixed>, tables: &Rc<Tables>, n: usize) -> Chan {
        Chan::Poly(Poly { form: FC_PULSE as i64, sub: 0, vol_mode: 0, voice_id: 0, last: None, limit: n, voices: (0..n).map(|_| Channel::new(sh, fixed, tables)).collect() })
    }
    pub fn note_on(&mut self, sh: &Shared, note: i64, vel: i64) {
        match self {
            Chan::Mono(c) => c.note_on(sh, note, vel),
            Chan::Poly(p) => {
                let count = p.voices.iter().filter(|v| v.is_playing()).count();
                let mut vo = None;
                if count <= p.limit {
                    vo = p.voices.iter().position(|v| !v.is_playing());
                }
                if vo.is_none() {
                    let mut min = i64::MAX;
                    for (i, v) in p.voices.iter().enumerate() {
                        if min > v.id() {
                            min = v.id();
                            vo = Some(i);
                        }
                    }
                }
                let i = vo.unwrap_or(0);
                let (form, sub, vm) = (p.form, p.sub, p.vol_mode);
                let id = p.voice_id;
                p.voice_id += 1;
                let v = &mut p.voices[i];
                v.set_form(form, sub);
                v.set_vol_mode(vm);
                v.note_on_with_id(sh, note, vel, id);
                p.last = Some(i);
            }
        }
    }
    pub fn note_off(&mut self, note: i64) {
        match self {
            Chan::Mono(c) => c.note_off(note),
            Chan::Poly(p) => {
                for v in p.voices.iter_mut().filter(|v| v.note_no() == note) {
                    v.note_off(note);
                }
            }
        }
    }
    pub fn set_note_no(&mut self, sh: &Shared, note: i64) {
        match self {
            Chan::Mono(c) => c.set_note_no(sh, note, true),
            Chan::Poly(p) => {
                if let Some(v) = p.last.and_then(|i| p.voices.get_mut(i)).filter(|v| v.is_playing()) {
                    v.set_note_no(sh, note, true);
                }
            }
        }
    }
    pub fn set_form(&mut self, form: i64, sub: i64) {
        match self {
            Chan::Mono(c) => c.set_form(form, sub),
            Chan::Poly(p) => {
                p.form = form;
                p.sub = sub;
            }
        }
    }
    pub fn set_vol_mode(&mut self, m: i64) {
        match self {
            Chan::Mono(c) => c.set_vol_mode(m),
            Chan::Poly(p) => p.vol_mode = m,
        }
    }
    pub fn set_voice_limit(&mut self, n: i64) {
        if let Chan::Poly(p) = self {
            p.limit = n.clamp(1, p.voices.len() as i64) as usize;
        }
    }
    pub fn reset(&mut self, sh: &Shared) {
        if let Chan::Poly(p) = self {
            p.form = 0;
            p.sub = 0;
            p.voice_id = 0;
            p.vol_mode = 0;
        }
        each!(self, c => c.reset(sh));
    }
    pub fn apply(&mut self, f: &mut dyn FnMut(&mut Channel)) {
        each!(self, c => f(c));
    }
    pub fn samples(&mut self, sh: &mut Shared, mix: &mut [Vec<f32>; 2], max: usize, start: usize, delta: usize) {
        match self {
            Chan::Mono(c) => c.samples(sh, mix, max, start, delta),
            Chan::Poly(p) => {
                let mut slave = false;
                for v in p.voices.iter_mut() {
                    if v.is_playing() {
                        v.set_slave(slave);
                        v.samples(sh, mix, max, start, delta);
                        slave = true;
                    }
                }
                if !slave {
                    p.voices[0].clear_out_pipe(sh, start, delta);
                }
            }
        }
    }
}
