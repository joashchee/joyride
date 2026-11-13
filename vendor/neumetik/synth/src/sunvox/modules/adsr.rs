// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! ADSR (psynths_adsr.cpp): an envelope generator, its signal the output
//! or (modes 1 and 2) multiplying the input; started and stopped by
//! notes or its State controller. Its envelope (adsr_env) is FMX's too.

use super::super::dsp::{smoother_target, Smoother, SmootherCoefs};
use super::super::net::{self, Cmd, Event, Module, Net};
use super::generator::to_int16;

const OUTPUTS: usize = 2;
const MAX_CHANNELS: usize = 16;
pub(crate) const CURVE_TYPES: i32 = 12;
pub(crate) const SUSTAIN_MODES: i32 = 3;
pub(crate) const SMOOTH_MODES: i32 = 4;

const STATE_FLAG_D: u32 = 1 << 0;
const STATE_FLAG_R: u32 = 1 << 1;

const C_VOLUME: usize = 0;
const C_A: usize = 1;
const C_D: usize = 2;
const C_S: usize = 3;
const C_R: usize = 4;
const C_ACURVE: usize = 5;
const C_DCURVE: usize = 6;
const C_RCURVE: usize = 7;
const C_SUSTAIN: usize = 8;
const C_SUSTAIN_PEDAL: usize = 9;
const C_STATE: usize = 10;
const C_NOTEON: usize = 11;
const C_NOTEOFF: usize = 12;
const C_MODE: usize = 13;
const C_SMOOTH: usize = 14;

fn curve_val(x: i32, c: i8) -> i32 {
    let mut x = x;
    match c {
        1 => x = x.wrapping_mul(x) >> 15,
        2 => {
            x = x.wrapping_mul(x) >> 15;
            x = x.wrapping_mul(x) >> 15;
        }
        3 => {
            x -= 32768;
            x = 32768 - (x.wrapping_mul(x) >> 15);
        }
        4 => {
            x -= 32768;
            x = x.wrapping_mul(x) >> 15;
            x = 32768 - (x.wrapping_mul(x) >> 15);
        }
        5 => x = (((std::f32::consts::PI * x as f32 / 32768.0 - std::f32::consts::PI / 2.0).sin() + 1.0) * 32768.0 / 2.0) as i32,
        6 => {
            if x != 0 {
                x = 32768;
            }
        }
        7 => x = (x * 32).min(32768),
        8 => x = (x >> 14) << 14,
        9 => x = (x >> 13) << 13,
        10 => x = (x >> 12) << 12,
        11 => x = (x >> 11) << 11,
        _ => {}
    }
    x
}

fn curve_val_inv(y: i32, c: i8) -> i32 {
    let mut y = y;
    match c {
        1 => y = ((y << 15) as f32).sqrt() as i32,
        2 => y = (y as f64 * 32768.0 * 32768.0 * 32768.0).sqrt().sqrt() as i32,
        3 => y = (32768.0 - (((32768 - y) << 15) as f32).sqrt()) as i32,
        4 => y = (32768.0 - (32768.0 * 32768.0 * 32768.0 * (32768 - y) as f64).sqrt().sqrt()) as i32,
        5 => y = (((y as f32 / (32768.0 / 2.0) - 1.0).asin() + std::f32::consts::PI / 2.0) * 32768.0 / std::f32::consts::PI) as i32,
        6 => {
            if y != 0 {
                y = 1;
            }
        }
        7 => y /= 32,
        8 => y = (y >> 14) << 14,
        9 => y = (y >> 13) << 13,
        10 => y = (y >> 12) << 12,
        11 => y = (y >> 11) << 11,
        _ => {}
    }
    y
}

/// adsr_env.
#[derive(Clone, Copy, Default)]
pub(crate) struct AdsrEnv {
    pub ctl_volume: i32,
    pub ctl_a: i32,
    pub ctl_d: i32,
    pub ctl_s: i32,
    pub ctl_r: i32,
    pub ctl_acurve: i8,
    pub ctl_dcurve: i8,
    pub ctl_rcurve: i8,
    pub ctl_sustain: i8,
    pub ctl_sustain_pedal: i8,
    pub ctl_smooth: i8,
    pub delta_recalc_request: bool,
    srate: i32,
    ad: i32,
    dd: i32,
    rd: i32,
    v: i32,
    v2: i32,
    v4: i32,
    v5: f32,
    coefs: SmootherCoefs,
    vol: Smoother,
    state_flags: u32,
    pub pressed: bool,
    pub playing: bool,
}

impl AdsrEnv {
    fn reset_volume_filter(&mut self) {
        self.vol.reset(self.ctl_volume as f32, 32768.0);
    }

    fn restart(&mut self) {
        self.v = 0;
        self.v4 = 0;
        if self.v5 != 0.0 {
            if self.ctl_smooth != 0 {
                let last = to_int16(self.v5) as i32;
                match self.ctl_smooth {
                    1 => self.v4 = last << 15,
                    2 => self.v = curve_val_inv(last, self.ctl_acurve).wrapping_shl(15),
                    _ => {}
                }
            }
        } else {
            self.reset_volume_filter();
        }
        self.v2 = 32768 << 15;
        self.state_flags = 0;
        if self.ctl_a == 0 {
            self.v = 32768 << 15;
            self.state_flags |= STATE_FLAG_D;
        }
        self.playing = true;
    }

    pub fn start(&mut self) {
        self.restart();
        self.pressed = true;
    }

    pub fn stop(&mut self) {
        self.pressed = false;
    }

    pub fn reset(&mut self) {
        self.state_flags = 0;
        self.playing = false;
        self.v4 = 0;
        self.v5 = 0.0;
        self.reset_volume_filter();
    }

    pub fn init(srate: i32) -> AdsrEnv {
        let mut a = AdsrEnv { ctl_volume: 32768, ctl_a: 100, ctl_d: 100, ctl_s: 32768 / 2, ctl_r: 100, ctl_sustain: 1, ctl_smooth: 2, srate, delta_recalc_request: true, ..AdsrEnv::default() };
        a.coefs = SmootherCoefs::new(100.0, srate as f32);
        a.reset_volume_filter();
        a
    }

    /// adsr_env_init with filled_with_zeros: what isn't set here is kept.
    pub fn init_filled(a: AdsrEnv, srate: i32) -> AdsrEnv {
        let mut a = AdsrEnv { ctl_volume: 32768, ctl_a: 100, ctl_d: 100, ctl_s: 32768 / 2, ctl_r: 100, ctl_sustain: 1, ctl_smooth: 2, srate, delta_recalc_request: true, ..a };
        a.coefs = SmootherCoefs::new(100.0, srate as f32);
        a.reset_volume_filter();
        a
    }

    pub fn change_srate(&mut self, srate: i32) {
        if self.srate != srate {
            self.srate = srate;
            self.delta_recalc_request = true;
            self.coefs = SmootherCoefs::new(100.0, srate as f32);
        }
    }

    pub fn change_adr(&mut self, alen: i32, dlen: i32, rlen: i32) {
        if self.ctl_a != alen || self.ctl_d != dlen || self.ctl_r != rlen {
            self.ctl_a = alen;
            self.ctl_d = dlen;
            self.ctl_r = rlen;
            self.delta_recalc_request = true;
        }
    }

    /// adsr_env_run: `out` filled; the buffer's state (0 silence, 1 sound,
    /// 2 zeros) and the frames played before it stopped.
    pub fn run(&mut self, out: &mut [f32]) -> (i32, usize) {
        let frames = out.len();
        if !self.playing {
            return (0, 0);
        }
        let mut retval = 1;
        if self.delta_recalc_request {
            self.delta_recalc_request = false;
            let d = |ms: i32| -> i32 {
                let mut len = (ms as u64 * self.srate as u64 / 1000) as i32;
                if len == 0 {
                    len = 1;
                }
                let x = (32768 << 15) / len;
                if x == 0 {
                    1
                } else {
                    x
                }
            };
            self.ad = d(self.ctl_a);
            self.dd = d(self.ctl_d);
            self.rd = d(self.ctl_r);
        }
        let pressed = self.pressed;
        if !pressed && self.ctl_sustain_pedal == 0 {
            self.state_flags |= STATE_FLAG_R;
        }
        let mut i = 0;
        while i < frames {
            if self.state_flags & STATE_FLAG_R != 0 {
                self.v2 = self.v2.wrapping_sub(self.rd);
                if self.v2 < 0 {
                    self.v2 = 0;
                    self.v5 = 0.0;
                    self.state_flags = 0;
                    self.playing = false;
                    if self.ctl_sustain == 2 && (pressed || self.ctl_sustain_pedal != 0) {
                        self.restart();
                    } else {
                        self.reset_volume_filter();
                        break;
                    }
                }
            }
            if self.state_flags & STATE_FLAG_D != 0 {
                self.v = self.v.wrapping_sub(self.dd);
                if self.v < 0 {
                    self.v = 0;
                    if self.ctl_sustain != 1 {
                        self.state_flags |= STATE_FLAG_R;
                    }
                }
            } else {
                self.v = self.v.wrapping_add(self.ad);
                if self.v >= 32768 << 15 {
                    self.v = 32768 << 15;
                    self.state_flags |= STATE_FLAG_D;
                }
            }
            let c = if self.state_flags & STATE_FLAG_D != 0 { self.ctl_dcurve } else { self.ctl_acurve };
            let mut v = curve_val(self.v >> 15, c);
            let v2 = curve_val(self.v2 >> 15, self.ctl_rcurve);
            if self.state_flags & STATE_FLAG_D != 0 {
                v = ((v.wrapping_mul(32768 - self.ctl_s)) >> 15) + self.ctl_s;
            }
            out[i] = v.wrapping_mul(v2) as f32 / (1 << 30) as f32;
            i += 1;
        }
        if self.v4 != 0 {
            for o in out[..i].iter_mut() {
                self.v4 = self.v4.wrapping_sub(self.ad.wrapping_mul(2));
                if self.v4 < 0 {
                    self.v4 = 0;
                    break;
                }
                *o += (self.v4 >> 15) as f32 / 32768.0;
            }
        }
        self.v5 = 0.0;
        if self.playing && i == frames && frames > 0 {
            self.v5 = out[frames - 1];
        }
        let mut static_vol = true;
        let ctl_volume = self.ctl_volume;
        if self.ctl_smooth != 0 {
            let target = smoother_target(ctl_volume as f32, 32768.0);
            if self.vol.check(target) {
                static_vol = false;
                for o in out[..i].iter_mut() {
                    *o *= self.vol.val(&self.coefs, target);
                }
            }
        }
        if static_vol {
            if ctl_volume < 32768 {
                if ctl_volume == 0 {
                    retval = 2;
                    for o in out[..i].iter_mut() {
                        *o = 0.0;
                    }
                } else {
                    let v2 = ctl_volume as f32 / 32768.0;
                    for o in out[..i].iter_mut() {
                        *o *= v2;
                    }
                }
            }
            if ctl_volume > 32768 {
                let v2 = ctl_volume as f32 / 32768.0;
                for o in out[..i].iter_mut() {
                    *o *= v2;
                }
            }
        }
        for o in out[i..].iter_mut() {
            *o = 0.0;
        }
        (retval, i)
    }
}

#[derive(Default)]
pub(crate) struct Adsr {
    a: AdsrEnv,
    chans: [u32; MAX_CHANNELS],
    active_chans: i32,
    changed: bool,
}

impl Adsr {
    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.changed {
            let want = if net.mods[n].ctl(C_MODE) < 2 { 1 } else { OUTPUTS as i32 };
            if net.mods[n].output_channels != want {
                net.set_number_of_inputs(want, n);
                net.set_number_of_outputs(want, n);
            }
        }
        self.changed = false;
    }

    fn set_state(&mut self, n: usize, net: &mut Net) {
        let v = self.a.pressed as i32;
        let m = &mut net.mods[n];
        if let Some(var) = m.ctls.get(C_STATE).and_then(|c| c.var) {
            m.vars[var] = v;
        }
    }
}

impl Module for Adsr {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                self.a = AdsrEnv::init(net.sampling_freq);
                let a = self.a;
                net.register_ctl(n, 0, 32768, a.ctl_volume, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 10000, a.ctl_a, 0, C_A, -1, 1);
                net.register_ctl(n, 0, 10000, a.ctl_d, 0, C_D, -1, 1);
                net.register_ctl(n, 0, 32768, a.ctl_s, 0, C_S, -1, 1);
                net.register_ctl(n, 0, 10000, a.ctl_r, 0, C_R, -1, 1);
                net.set_ctl_flags(n, 1, 2);
                net.set_ctl_flags(n, 2, 2);
                net.set_ctl_flags(n, 4, 2);
                net.register_ctl(n, 0, CURVE_TYPES - 1, a.ctl_acurve as i32, 1, C_ACURVE, -1, 2);
                net.register_ctl(n, 0, CURVE_TYPES - 1, a.ctl_dcurve as i32, 1, C_DCURVE, -1, 2);
                net.register_ctl(n, 0, CURVE_TYPES - 1, a.ctl_rcurve as i32, 1, C_RCURVE, -1, 2);
                net.register_ctl(n, 0, SUSTAIN_MODES - 1, a.ctl_sustain as i32, 1, C_SUSTAIN, -1, 3);
                net.register_ctl(n, 0, 1, a.ctl_sustain_pedal as i32, 1, C_SUSTAIN_PEDAL, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_STATE, -1, 4);
                net.register_ctl(n, 0, 2, 1, 1, C_NOTEON, -1, 5);
                net.register_ctl(n, 0, 2, 1, 1, C_NOTEOFF, -1, 5);
                net.register_ctl(n, 0, 2, 0, 1, C_MODE, -1, 5);
                net.register_ctl(n, 0, SMOOTH_MODES - 1, a.ctl_smooth as i32, 1, C_SMOOTH, -1, 5);
                self.chans = [!0; MAX_CHANNELS];
                self.changed = true;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                let m = &net.mods[n];
                let a = &mut self.a;
                a.ctl_volume = m.ctl(C_VOLUME);
                a.ctl_a = m.ctl(C_A);
                a.ctl_d = m.ctl(C_D);
                a.ctl_s = m.ctl(C_S);
                a.ctl_r = m.ctl(C_R);
                a.ctl_acurve = m.ctl(C_ACURVE) as i8;
                a.ctl_dcurve = m.ctl(C_DCURVE) as i8;
                a.ctl_rcurve = m.ctl(C_RCURVE) as i8;
                a.ctl_sustain = m.ctl(C_SUSTAIN) as i8;
                a.ctl_sustain_pedal = m.ctl(C_SUSTAIN_PEDAL) as i8;
                a.pressed = m.ctl(C_STATE) != 0;
                a.ctl_smooth = m.ctl(C_SMOOTH) as i8;
                a.reset();
                if m.ctl(C_STATE) != 0 {
                    a.start();
                }
                retval = 1;
            }
            Cmd::Clean => {
                self.a.reset();
                self.chans = [!0; MAX_CHANNELS];
                self.active_chans = 0;
                retval = 1;
            }
            Cmd::RenderReplace => {
                if !self.a.playing {
                    return 0;
                }
                if self.changed {
                    self.handle_changes(n, net);
                }
                let m = &mut net.mods[n];
                let (offset, frames) = (m.offset as usize, m.frames as usize);
                let outputs_num = m.output_channels.clamp(0, 2) as usize;
                let (r, _) = self.a.run(&mut m.chout[0][offset..offset + frames]);
                retval = r as isize;
                if m.ctl(C_MODE) != 0 && retval != 0 {
                    let env: Vec<f32> = m.chout[0][offset..offset + frames].to_vec();
                    for ch in (0..outputs_num).rev() {
                        let (ci, co) = (&m.chin, &mut m.chout);
                        for i in 0..frames {
                            co[ch][offset + i] = ci[ch][offset + i] * env[i] / 1.0;
                        }
                    }
                }
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                net::set_ctl(&mut net.mods[n], e);
                retval = 1;
                let m = &net.mods[n];
                let a = &mut self.a;
                match e.ctl_num() as usize {
                    C_VOLUME => a.ctl_volume = m.ctl(C_VOLUME),
                    C_A => {
                        a.ctl_a = m.ctl(C_A);
                        a.delta_recalc_request = true;
                    }
                    C_D => {
                        a.ctl_d = m.ctl(C_D);
                        a.delta_recalc_request = true;
                    }
                    C_S => a.ctl_s = m.ctl(C_S),
                    C_R => {
                        a.ctl_r = m.ctl(C_R);
                        a.delta_recalc_request = true;
                    }
                    C_ACURVE => a.ctl_acurve = m.ctl(C_ACURVE) as i8,
                    C_DCURVE => a.ctl_dcurve = m.ctl(C_DCURVE) as i8,
                    C_RCURVE => a.ctl_rcurve = m.ctl(C_RCURVE) as i8,
                    C_SUSTAIN => a.ctl_sustain = m.ctl(C_SUSTAIN) as i8,
                    C_SUSTAIN_PEDAL => a.ctl_sustain_pedal = m.ctl(C_SUSTAIN_PEDAL) as i8,
                    C_STATE => {
                        let state = m.ctl(C_STATE) != 0;
                        if a.pressed != state || (!a.playing && state) {
                            a.pressed = state;
                            if a.pressed {
                                a.start();
                            }
                        }
                    }
                    C_MODE => self.changed = true,
                    C_SMOOTH => a.ctl_smooth = m.ctl(C_SMOOTH) as i8,
                    _ => {}
                }
            }
            Cmd::NoteOn => {
                let mut c = 0;
                if self.active_chans > 0 {
                    c = self.chans.iter().position(|&id| id == e.id).unwrap_or(MAX_CHANNELS);
                    if c >= MAX_CHANNELS {
                        c = MAX_CHANNELS;
                        if let Some(k) = self.chans.iter().position(|&id| id == !0) {
                            self.chans[k] = e.id;
                            self.active_chans += 1;
                            c = k;
                        }
                    }
                } else {
                    self.chans[0] = e.id;
                    self.active_chans += 1;
                }
                let noteon = net.mods[n].ctl(C_NOTEON);
                if noteon != 0 {
                    let start = noteon == 2 || (noteon == 1 && c < MAX_CHANNELS && self.active_chans == 1);
                    if start {
                        self.a.start();
                        self.set_state(n, net);
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.active_chans > 0 {
                    if let Some(c) = self.chans.iter().position(|&id| id == e.id) {
                        self.chans[c] = !0;
                        self.active_chans -= 1;
                    }
                }
                let noteoff = net.mods[n].ctl(C_NOTEOFF);
                if noteoff != 0 {
                    let stop = noteoff == 2 || (noteoff == 1 && self.active_chans == 0);
                    if stop {
                        self.a.stop();
                        self.set_state(n, net);
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                if net.mods[n].ctl(C_NOTEOFF) != 0 {
                    self.a.stop();
                    self.set_state(n, net);
                    self.chans = [!0; MAX_CHANNELS];
                    self.active_chans = 0;
                }
                retval = 1;
            }
            Cmd::ResetMsb => self.changed = true,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
