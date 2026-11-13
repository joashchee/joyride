// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Delay (psynths_delay.cpp): a delay line a channel (with feedback, or
//! as an allpass), its length in many units; the notes it receives are
//! delayed too, by the left channel's time, on to the modules it links to.

use super::super::net::{evt_id_inc, Cmd, Event, Module, Net, RT_BYPASS, RT_MUTE};

const OUTPUTS: usize = 2;
const DELAY_UNIT_MAX: i32 = 9;
const DELAY_MUL_MAX: i32 = 15;

const C_DRY: usize = 0;
const C_WET: usize = 1;
const C_DELAY_L: usize = 2;
const C_DELAY_R: usize = 3;
const C_VOLUME_L: usize = 4;
const C_VOLUME_R: usize = 5;
const C_MONO: usize = 6;
const C_INVERSE: usize = 7;
const C_DELAY_UNITS: usize = 8;
const C_DELAY_MUL: usize = 9;
const C_FEEDBACK: usize = 10;
const C_NEGATIVE_FEEDBACK: usize = 11;
const C_ALLPASS: usize = 12;

#[derive(Clone, Copy, Default)]
struct DelayEvt {
    evt: Event,
    vol: u16,
}

#[derive(Default)]
pub(crate) struct Delay {
    delay: [i32; OUTPUTS],
    buf_size: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: i32,
    buf_clean: bool,
    evtbuf: Vec<DelayEvt>,
    evtbuf_wp: usize,
    evtbuf_rp: usize,
    empty_frames_counter: i32,
    frame_counter: i32,
    tick_size: i32,
    ticks_per_line: u8,
    ctls_changed: u32,
}

impl Delay {
    /// add_event: an event queued at an absolute frame, the ring grown fourfold when full.
    fn add_event(&mut self, e: &Event, vol: i32, n: usize, offset: i32) {
        let size = self.evtbuf.len();
        let mask = size - 1;
        let wp = self.evtbuf_wp;
        self.evtbuf_wp = (wp + 1) & mask;
        let mut d = DelayEvt { evt: *e, vol: vol as u16 };
        d.evt.id = evt_id_inc(d.evt.id, n as u32);
        d.evt.offset = offset;
        self.evtbuf[wp] = d;
        let len = self.evtbuf_wp.wrapping_sub(self.evtbuf_rp) & mask;
        if len >= size - 1 {
            let new_size = size * 4;
            let mut nb = vec![DelayEvt::default(); new_size];
            let mut rp = self.evtbuf_rp;
            let mut p = 0;
            while rp != self.evtbuf_wp {
                nb[p] = self.evtbuf[rp];
                p += 1;
                rp = (rp + 1) & mask;
            }
            self.evtbuf_rp = 0;
            self.evtbuf_wp = p;
            self.evtbuf = nb;
        }
    }

    fn change_ctl_limits(&self, n: usize, net: &mut Net) {
        if net.base_host_version < 0x01090202 {
            return;
        }
        let max = match net.mods[n].ctl(C_DELAY_UNITS) {
            1 => 4000,
            2 => 8192,
            7..=9 => 32768,
            _ => 256,
        };
        for cnum in 2..=3 {
            if net.mods[n].ctls.get(cnum).is_some_and(|c| c.max != max) {
                net.change_ctl_limits(n, cnum, -1, max, max);
            }
        }
    }

    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.ctls_changed & 1 != 0 {
            let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { 2 };
            if net.mods[n].output_channels != want {
                net.set_number_of_outputs(want, n);
                net.set_number_of_inputs(want, n);
            }
        }
        if self.ctls_changed & 2 != 0 {
            let m = &net.mods[n];
            let sr = net.sampling_freq;
            let (dl, dr) = (m.ctl(C_DELAY_L), m.ctl(C_DELAY_R));
            let units = m.ctl(C_DELAY_UNITS);
            let ts = self.tick_size as u32 as u64;
            let tpl = self.ticks_per_line as u64;
            self.delay = match units {
                0 => [((sr / 64) * dl) >> 8, ((sr / 64) * dr) >> 8],
                1 => [(sr * dl) / 1000, (sr * dr) / 1000],
                2 => [if dl == 0 { sr } else { sr / dl }, if dr == 0 { sr } else { sr / dr }],
                3 => [((ts * dl as u32 as u64) / 256) as i32, ((ts * dr as u32 as u64) / 256) as i32],
                4..=6 => [((ts * tpl * dl as u32 as u64) / 256) as i32 / (units - 3), ((ts * tpl * dr as u32 as u64) / 256) as i32 / (units - 3)],
                7 => [(dl as i64 * sr as i64 / 44100) as i32, (dr as i64 * sr as i64 / 44100) as i32],
                8 => [(dl as i64 * sr as i64 / 48000) as i32, (dr as i64 * sr as i64 / 48000) as i32],
                9 => [dl, dr],
                _ => [0, 0],
            };
            let mut buf_size = self.buf_size;
            let max_buf_size = if net.base_host_version >= 0x02000000 { sr * 60 } else { sr * 4 };
            let mul = m.ctl(C_DELAY_MUL);
            for d in self.delay.iter_mut() {
                *d = d.wrapping_mul(mul);
                if *d == 0 {
                    *d = 1;
                }
                if *d > max_buf_size {
                    *d = max_buf_size;
                }
                if *d > buf_size {
                    buf_size = *d;
                }
            }
            if buf_size > self.buf_size {
                for b in self.buf.iter_mut() {
                    b.resize(buf_size as usize, 0.0);
                }
                self.buf_size = buf_size;
            }
            if self.buf_ptr >= self.buf_size {
                self.buf_ptr = 0;
            }
        }
        self.ctls_changed = 0;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.ctls_changed != 0 {
            self.handle_changes(n, net);
        }
        let (frames, offset) = (net.mods[n].frames, net.mods[n].offset);
        self.frame_counter = self.frame_counter.wrapping_add(frames);
        if net.mods[n].realtime_flags & RT_MUTE != 0 {
            self.evtbuf_wp = 0;
            self.evtbuf_rp = 0;
        }
        let base = net.base_host_version;
        let (vol_l, wet_c, feedback_c) = (net.mods[n].ctl(C_VOLUME_L), net.mods[n].ctl(C_WET), net.mods[n].ctl(C_FEEDBACK));
        while self.evtbuf_rp != self.evtbuf_wp {
            let rp = self.evtbuf_rp;
            let e0 = self.evtbuf[rp];
            if self.frame_counter.wrapping_sub(e0.evt.offset.wrapping_add(self.delay[0])) <= 0 {
                break;
            }
            let mut evt = e0.evt;
            evt.offset = evt.offset + self.delay[0] - (self.frame_counter - frames) + offset;
            if evt.offset < 0 {
                evt.offset = 0;
            }
            let pitch = evt.pitch();
            let vel = evt.velocity() as i32;
            let links = net.mods[n].output_links.clone();
            for l in links {
                let Some(dest) = net.get(l) else { continue };
                let (ft, rel) = (dest.finetune, dest.relative_note);
                match evt.cmd {
                    Cmd::NoteOn | Cmd::SetFreq | Cmd::SetVelocity | Cmd::NoteOff => {
                        if matches!(evt.cmd, Cmd::NoteOn | Cmd::SetFreq) {
                            evt.set_pitch(pitch - ft - rel * 256);
                        }
                        if base >= 0x02000000 {
                            let w = vol_l * wet_c / 256;
                            let v = (vel * e0.vol as i32 * w / (256 * 256)).min(256);
                            evt.set_velocity(v);
                        } else {
                            evt.set_velocity(vel * e0.vol as i32 / 256);
                        }
                    }
                    _ => {}
                }
                net.add_event(l as usize, &evt);
            }
            self.evtbuf[rp].evt = evt;
            if feedback_c != 0 && e0.vol != 0 {
                if evt.cmd != Cmd::SetSampleOffset && evt.cmd != Cmd::SetSampleOffset2 {
                    evt.set_velocity(vel);
                    evt.set_pitch(pitch);
                }
                let vol = e0.vol as i32 * feedback_c / 32768;
                self.evtbuf[rp].vol = vol as u16;
                let at = self.frame_counter - frames + evt.offset;
                self.add_event(&evt, vol, n, at);
            }
            self.evtbuf_rp = (self.evtbuf_rp + 1) & (self.evtbuf.len() - 1);
        }
        let m = &mut net.mods[n];
        let outputs_num = m.output_channels.max(0) as usize;
        let (frames, offset) = (frames as usize, offset as usize);
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        if input_signal {
            self.empty_frames_counter = 0;
        } else {
            if self.empty_frames_counter >= self.buf_size && feedback_c == 0 {
                return 0;
            }
            self.empty_frames_counter += frames as i32;
        }
        self.buf_clean = false;
        let dry_c = m.ctl(C_DRY);
        let dry = dry_c as f32 / 256.0;
        let mut fb = feedback_c as f32 / 32768.0;
        if m.ctl(C_NEGATIVE_FEEDBACK) != 0 {
            fb = -fb;
        }
        let inverse = m.ctl(C_INVERSE) != 0;
        let allpass = m.ctl(C_ALLPASS) != 0;
        let vol_r = m.ctl(C_VOLUME_R);
        let bs = self.buf_size;
        let mut buf_ptr = 0;
        for ch in 0..outputs_num {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let inp = &chin[ch][offset..offset + frames];
            let out = &mut chout[ch][offset..offset + frames];
            let cbuf = &mut self.buf[ch];
            buf_ptr = self.buf_ptr;
            let wet = if ch == 0 { (wet_c * vol_l) / 256 } else { (wet_c * vol_r) / 256 };
            let mut ctl_wet = wet as f32 / 256.0;
            let delay = self.delay[ch];
            if fb != 0.0 {
                let inv = if inverse { -1.0f32 } else { 1.0 };
                for i in 0..frames {
                    let mut p = buf_ptr - delay;
                    if p < 0 {
                        p += bs;
                    }
                    let mut out_val = cbuf[p as usize] * inv;
                    if allpass {
                        let w = inp[i] + out_val * fb;
                        out_val += w * -fb;
                        out[i] = out_val * ctl_wet;
                        cbuf[buf_ptr as usize] = w;
                    } else {
                        out[i] = out_val * ctl_wet;
                        cbuf[buf_ptr as usize] = inp[i] + out_val * fb;
                    }
                    buf_ptr += 1;
                    if buf_ptr >= bs {
                        buf_ptr = 0;
                    }
                }
            } else if ctl_wet == 0.0 {
                for i in 0..frames {
                    cbuf[buf_ptr as usize] = inp[i];
                    out[i] = 0.0;
                    buf_ptr += 1;
                    if buf_ptr >= bs {
                        buf_ptr = 0;
                    }
                }
            } else {
                if inverse {
                    ctl_wet = -ctl_wet;
                }
                for i in 0..frames {
                    let mut p = buf_ptr - delay;
                    if p < 0 {
                        p += bs;
                    }
                    out[i] = cbuf[p as usize] * ctl_wet;
                    cbuf[buf_ptr as usize] = inp[i];
                    buf_ptr += 1;
                    if buf_ptr >= bs {
                        buf_ptr = 0;
                    }
                }
            }
            if dry_c > 0 {
                if dry_c != 256 {
                    for i in 0..frames {
                        out[i] += inp[i] * dry;
                    }
                } else {
                    for i in 0..frames {
                        out[i] += inp[i];
                    }
                }
            }
        }
        self.buf_ptr = buf_ptr;
        1
    }
}

impl Module for Delay {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_DRY, 256, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_WET, 256, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_DELAY_L, -1, 1);
                net.register_ctl(n, 0, 256, 160, 0, C_DELAY_R, -1, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME_L, -1, 2);
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME_R, -1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_INVERSE, -1, 3);
                net.register_ctl(n, 0, DELAY_UNIT_MAX, 0, 1, C_DELAY_UNITS, -1, 1);
                net.register_ctl(n, 1, DELAY_MUL_MAX, 1, 1, C_DELAY_MUL, 1, 1);
                net.register_ctl(n, 0, 32768, 0, 0, C_FEEDBACK, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_NEGATIVE_FEEDBACK, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_ALLPASS, -1, 3);
                self.buf_size = net.sampling_freq / 64;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; self.buf_size as usize];
                }
                self.buf_ptr = 0;
                self.buf_clean = true;
                self.empty_frames_counter = self.buf_size;
                self.evtbuf = vec![DelayEvt::default(); 16];
                self.ctls_changed = 0xFFFF_FFFF;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                self.change_ctl_limits(n, net);
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_clean {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_clean = true;
                }
                self.buf_ptr = 0;
                self.evtbuf_wp = 0;
                self.evtbuf_rp = 0;
                self.empty_frames_counter = self.buf_size;
                self.frame_counter = 0;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                2 | 3 => self.ctls_changed |= 2,
                6 => self.ctls_changed |= 1,
                8 | 9 => {
                    let (var, max) = if e.ctl_num() == 8 { (C_DELAY_UNITS, DELAY_UNIT_MAX) } else { (C_DELAY_MUL, DELAY_MUL_MAX) };
                    let v = (e.ctl_val() as i32).min(max);
                    if let Some(vi) = net.mods[n].ctls.get(var).and_then(|c| c.var) {
                        net.mods[n].vars[vi] = v;
                    }
                    if e.ctl_num() == 8 {
                        self.change_ctl_limits(n, net);
                    }
                    self.ctls_changed |= 2;
                    retval = 1;
                }
                _ => {}
            },
            Cmd::NoteOn | Cmd::NoteOff | Cmd::AllNotesOff | Cmd::SetFreq | Cmd::SetVelocity | Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if net.mods[n].realtime_flags & RT_BYPASS != 0 {
                    let mut e2 = *e;
                    e2.id = evt_id_inc(e2.id, n as u32);
                    net.multisend(n, &e2);
                } else {
                    let fc = self.frame_counter;
                    self.add_event(e, 256, n, fc);
                }
                retval = 1;
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                let u = net.mods[n].ctl(C_DELAY_UNITS);
                if (3..=6).contains(&u) {
                    self.ctls_changed |= 2;
                }
                retval = 1;
            }
            Cmd::ResetMsb => self.ctls_changed |= 1,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
