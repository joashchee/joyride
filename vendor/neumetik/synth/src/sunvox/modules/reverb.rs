// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Reverb (psynths_reverb.cpp): Freeverb's eight damped combs and four
//! allpasses a side (or every other one in LQ), their lengths scaled by
//! the room size and shuffled by a seed, after a DC blocker; a freeze, a
//! width.

use super::super::net::{self, Cmd, Event, Module, Net};
use super::dc_blocker::DcFilter;

const OUTPUTS: usize = 2;
const MAX_COMBS: usize = 8;
const MAX_ALLPASSES: usize = 4;
const MAX_BUF_SCALE: i32 = 8;
const STEREO_SPREAD: i64 = 23;
const ALLPASS_FEEDBACK: f32 = 0.5;

const COMB_BASE: [i32; MAX_COMBS] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_BASE: [i32; MAX_ALLPASSES] = [556, 441, 341, 225];

const C_DRY: usize = 0;
const C_WET: usize = 1;
const C_FEEDBACK: usize = 2;
const C_DAMP: usize = 3;
const C_WIDTH: usize = 4;
const C_FREEZE: usize = 5;
const C_MODE: usize = 6;
const C_ALLPASS: usize = 7;
const C_ROOM_SIZE: usize = 8;
const C_SEED: usize = 9;

const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;

#[derive(Clone, Default)]
struct Comb {
    /// The allocation (smem_get_size of it): may be longer than buf_size.
    buf: Vec<f32>,
    buf_size: i32,
    buf_ptr: i32,
    feedback: f32,
    filterstore: f32,
    damp1: f32,
    damp2: f32,
}

#[derive(Clone, Default)]
struct Allpass {
    buf: Vec<f32>,
    buf_size: i32,
    buf_ptr: i32,
}

#[derive(Default)]
pub(crate) struct Reverb {
    combs: [Comb; MAX_COMBS * 2],
    biggest_comb: usize,
    allpasses: [Allpass; MAX_ALLPASSES * 2],
    filters_reinit_request: bool,
    filters_clean: bool,
    empty: bool,
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
    empty_check_buf_ptr: i32,
    dc: DcFilter,
}

impl Reverb {
    fn recalc_filter_buf_size(&mut self, net: &Net, n: usize) {
        let m = &net.mods[n];
        let scale = m.ctl(C_ROOM_SIZE).max(1) as i64;
        let delta: i64 = if net.sampling_freq != 44100 { ((net.sampling_freq << 11) / 44100) as i64 } else { 1 << 11 };
        let ctl_seed = m.ctl(C_SEED);
        let mut seed = ctl_seed as u32;
        let mut combs_changed = 0;
        for i in 0..MAX_COMBS * 2 {
            let mut bsize = COMB_BASE[i % MAX_COMBS];
            if ctl_seed > 0 {
                bsize = (bsize as u32 / 2 + (net::pseudo_random(&mut seed) % bsize as u32)) as i32;
            }
            let size = if i < MAX_COMBS { bsize as i64 * scale } else { (bsize as i64 + STEREO_SPREAD) * scale };
            let size = ((size * delta) >> 11) / 16;
            let f = &mut self.combs[i];
            if f.buf_size as i64 != size {
                f.buf_size = size as i32;
                combs_changed += 1;
            }
        }
        let mode = m.ctl(C_MODE);
        let mut biggest = 0;
        let mut max_size = self.combs[0].buf_size;
        let channels = if mode == MODE_HQ_MONO || mode == MODE_LQ_MONO { 1 } else { 2 };
        let filters_add = if mode < MODE_LQ { 1 } else { 2 };
        let mut i = 0;
        while i < MAX_COMBS * channels {
            if self.combs[i].buf_size > max_size {
                max_size = self.combs[i].buf_size;
                biggest = i;
            }
            i += filters_add;
        }
        if self.biggest_comb != biggest {
            self.biggest_comb = biggest;
            combs_changed += 1;
        }
        if combs_changed != 0 {
            self.empty_frames_counter = 0;
            self.empty_check_buf_ptr = 0;
        }
        for i in 0..MAX_ALLPASSES * 2 {
            let mut bsize = ALLPASS_BASE[i % MAX_ALLPASSES];
            if ctl_seed > 0 {
                bsize = (bsize as u32 / 2 + (net::pseudo_random(&mut seed) % bsize as u32)) as i32;
            }
            let size = if i < MAX_ALLPASSES { bsize as i64 * scale } else { (bsize as i64 + STEREO_SPREAD) * scale };
            self.allpasses[i].buf_size = (((size * delta) >> 11) / 16) as i32;
        }
    }

    fn realloc_filter_bufs(&mut self) {
        for f in self.combs.iter_mut() {
            if f.buf_size as usize > f.buf.len() {
                f.buf.resize(f.buf_size as usize + 128, 0.0);
            }
        }
        for f in self.allpasses.iter_mut() {
            if f.buf_size as usize > f.buf.len() {
                f.buf.resize(f.buf_size as usize + 128, 0.0);
            }
        }
    }

    fn reinit_filter_parameters(&mut self, net: &Net, n: usize) {
        let m = &net.mods[n];
        let (feedback, damp) = if m.ctl(C_FREEZE) != 0 {
            (256, 256)
        } else {
            let fb = (m.ctl(C_FEEDBACK) * 71) / 256 + 179;
            let lowpass_freq = (256 - m.ctl(C_DAMP)) as f32 * 128.0 + 128.0;
            let lowpass_rc = (1.0 / ((2.0 * std::f64::consts::PI) * lowpass_freq as f64)) as f32;
            let dt = 1.0f32 / net.sampling_freq as f32;
            let alpha = dt / (lowpass_rc + dt);
            (fb, (alpha * 256.0) as i32)
        };
        for c in self.combs.iter_mut() {
            c.feedback = feedback as f32 / 256.0;
            c.damp1 = damp as f32 / 256.0;
            c.damp2 = 1.0 - c.damp1;
        }
    }

    fn clean_filters(&mut self) {
        if self.filters_clean {
            return;
        }
        for c in self.combs.iter_mut() {
            let s = (c.buf_size.max(0) as usize).min(c.buf.len());
            c.buf[..s].fill(0.0);
            c.filterstore = 0.0;
        }
        for a in self.allpasses.iter_mut() {
            let s = (a.buf_size.max(0) as usize).min(a.buf.len());
            a.buf[..s].fill(0.0);
        }
        self.filters_clean = true;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mode = net.mods[n].ctl(C_MODE);
        if mode == MODE_HQ_MONO || mode == MODE_LQ_MONO {
            if net.mods[n].output_channels != 1 {
                net.set_number_of_outputs(1, n);
                net.set_number_of_inputs(1, n);
            }
        } else if net.mods[n].output_channels != OUTPUTS as i32 {
            net.set_number_of_outputs(OUTPUTS as i32, n);
            net.set_number_of_inputs(2, n);
        }
        let m = &net.mods[n];
        let (frames, offset) = (m.frames as usize, m.offset as usize);
        let mut outputs_num = m.output_channels.max(0) as usize;
        let mut input_signal = false;
        for ch in 0..outputs_num {
            if m.in_empty[ch] < (offset + frames) as i32 {
                self.empty = false;
                self.empty_frames_counter = 0;
                self.empty_check_buf_ptr = 0;
                input_signal = true;
                break;
            }
        }
        if self.empty {
            return 0;
        }
        if !input_signal && m.ctl(C_FREEZE) == 0 {
            let mut reset_check = false;
            if self.empty_frames_counter >= self.empty_frames_counter_max {
                let f = &self.combs[self.biggest_comb];
                let check_offset = self.empty_check_buf_ptr;
                let mut check_size = f.buf_size - check_offset;
                let max_frames = ((frames / 2) as i32).max(1);
                if check_size > max_frames {
                    check_size = max_frames;
                }
                for i in check_offset..check_offset + check_size {
                    if net::is_not_min(f.buf[i as usize]) {
                        reset_check = true;
                        check_size = i - check_offset;
                        break;
                    }
                }
                self.empty_check_buf_ptr += check_size;
                if self.empty_check_buf_ptr >= f.buf_size {
                    self.empty = true;
                    return 0;
                }
            }
            if reset_check {
                self.empty_frames_counter = 0;
            }
            self.empty_frames_counter += frames as i32;
        }
        let stereo = outputs_num != 1;
        if stereo {
            outputs_num = 2;
        }
        let filters_add = if mode < MODE_LQ { 1 } else { 2 };
        if self.filters_reinit_request {
            self.reinit_filter_parameters(net, n);
            self.recalc_filter_buf_size(net, n);
            self.realloc_filter_bufs();
            self.filters_reinit_request = false;
        }
        let m = &mut net.mods[n];
        let mut wet = m.ctl(C_WET) as f32 / 256.0;
        if filters_add > 1 {
            wet *= 2.0;
        }
        let allpass = m.ctl(C_ALLPASS);
        let ctl_dry = m.ctl(C_DRY);
        let ctl_width = m.ctl(C_WIDTH);
        if allpass == 0 || allpass == 2 {
            wet *= 4.0;
        }
        for ch in 0..outputs_num {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let out = &mut chout[ch][offset..offset + frames];
            let inp = if input_signal { Some(&chin[ch][offset..offset + frames]) } else { None };
            if self.dc.run(ch, inp, out) {
                out.fill(0.0);
            }
        }
        self.filters_clean = false;
        let [ref mut out_l, ref mut out_r] = m.chout;
        let out_l = &mut out_l[offset..offset + frames];
        let mut out_r = if stereo { Some(&mut out_r[offset..offset + frames]) } else { None };
        for i in 0..frames {
            let mut o_l = 0.0f32;
            let mut o_r = 0.0f32;
            let mut input = out_l[i];
            if let Some(r) = out_r.as_deref() {
                input = (input + r[i]) / 2.0;
            }
            input *= 1.0;
            let mut a = 0;
            while a < MAX_COMBS * outputs_num {
                let f = &mut self.combs[a];
                let f_out = f.buf[f.buf_ptr as usize];
                f.filterstore = (f_out * f.damp1) + (f.filterstore * f.damp2);
                f.buf[f.buf_ptr as usize] = input + (f.filterstore * f.feedback);
                if a < MAX_COMBS {
                    o_l += f_out;
                } else {
                    o_r += f_out;
                }
                f.buf_ptr += 1;
                if f.buf_ptr >= f.buf_size {
                    f.buf_ptr = 0;
                }
                a += filters_add;
            }
            if allpass == 1 || allpass == 2 {
                let mut a = 0;
                while a < MAX_ALLPASSES * outputs_num {
                    let f = &mut self.allpasses[a];
                    let f_bufout = f.buf[f.buf_ptr as usize];
                    let f_in = if a < MAX_ALLPASSES { o_l } else { o_r };
                    let f_out;
                    if allpass == 1 {
                        f_out = -f_in + f_bufout;
                        f.buf[f.buf_ptr as usize] = f_in + (f_bufout * ALLPASS_FEEDBACK);
                    } else {
                        let w = f_in + (f_bufout * ALLPASS_FEEDBACK);
                        f.buf[f.buf_ptr as usize] = w;
                        f_out = -w * ALLPASS_FEEDBACK + f_bufout;
                    }
                    if a < MAX_ALLPASSES {
                        o_l = f_out;
                    } else {
                        o_r = f_out;
                    }
                    f.buf_ptr += 1;
                    if f.buf_ptr >= f.buf_size {
                        f.buf_ptr = 0;
                    }
                    a += filters_add;
                }
            }
            out_l[i] = (o_l / 16.0) * wet;
            if let Some(r) = out_r.as_deref_mut() {
                r[i] = (o_r / 16.0) * wet;
            }
        }
        let in_l = &m.chin[0][offset..offset + frames];
        let in_r = if stereo { Some(&m.chin[1][offset..offset + frames]) } else { None };
        if ctl_dry > 0 {
            if ctl_dry == 256 {
                for i in 0..frames {
                    out_l[i] += in_l[i];
                }
                if let (Some(r), Some(ir)) = (out_r.as_deref_mut(), in_r) {
                    for i in 0..frames {
                        r[i] += ir[i];
                    }
                }
            } else {
                let dry = ctl_dry as f32 / 256.0;
                for i in 0..frames {
                    out_l[i] += in_l[i] * dry;
                }
                if let (Some(r), Some(ir)) = (out_r.as_deref_mut(), in_r) {
                    for i in 0..frames {
                        r[i] += ir[i] * dry;
                    }
                }
            }
        }
        if outputs_num == 2 && ctl_width != 256 {
            let w = ctl_width / 2 + 128;
            let w2 = 256 - w;
            let fw = w as f32 / 256.0;
            let fw2 = w2 as f32 / 256.0;
            if let Some(r) = out_r.as_deref_mut() {
                for i in 0..frames {
                    out_l[i] = out_l[i] * fw + r[i] * fw2;
                    r[i] = r[i] * fw + out_l[i] * fw2;
                }
            }
        }
        1
    }
}

impl Module for Reverb {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_DRY, -1, 0);
                net.register_ctl(n, 0, 256, 40, 0, C_WET, -1, 0);
                net.register_ctl(n, 0, 256, 256, 0, C_FEEDBACK, -1, 1);
                net.register_ctl(n, 0, 256, 128, 0, C_DAMP, -1, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_WIDTH, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_FREEZE, -1, 2);
                net.register_ctl(n, 0, 3, 0, 1, C_MODE, -1, 2);
                net.register_ctl(n, 0, 2, 1, 1, C_ALLPASS, -1, 2);
                net.register_ctl(n, 0, 16 * MAX_BUF_SCALE, 16, 0, C_ROOM_SIZE, -1, 2);
                net.register_ctl(n, 0, 32768, 0, 1, C_SEED, -1, 2);
                self.dc = DcFilter::new(net.sampling_freq);
                self.filters_reinit_request = true;
                self.filters_clean = true;
                self.biggest_comb = 0;
                self.empty = true;
                self.empty_frames_counter_max = net.sampling_freq * 5;
                self.empty_frames_counter = 0;
                self.empty_check_buf_ptr = 0;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.recalc_filter_buf_size(net, n);
                for f in self.combs.iter_mut() {
                    f.buf = vec![0.0; f.buf_size.max(0) as usize];
                    f.buf_ptr = 0;
                    f.filterstore = 0.0;
                }
                for f in self.allpasses.iter_mut() {
                    f.buf = vec![0.0; f.buf_size.max(0) as usize];
                    f.buf_ptr = 0;
                }
                self.clean_filters();
                retval = 1;
            }
            Cmd::Clean => {
                self.clean_filters();
                self.dc.stop();
                self.empty = true;
                self.empty_frames_counter = 0;
                self.empty_check_buf_ptr = 0;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => self.filters_reinit_request = true,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
