// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FM (psynths_fm.cpp): two sine operators a voice, the modulator (with
//! self-feedback, scaled down the keyboard) bending the carrier's phase,
//! each under an ADSR drawn into a table, at 44.1 kHz; float tables and
//! sinf (FM_HQ32, the full-quality build's).

use super::super::dsp::{renderbuf2output, RenderbufPars, SmootherCoefs};
use super::super::net::{self, Cmd, Event, Module, Net};

const MAX_CHANNELS: usize = 16;
const FM_SFREQ: i32 = 44100;
const ENV_STEP: i32 = 256;
const TABLE_SIZE: usize = 1550;
const TABLE_AMP: i32 = 1 << 15;
const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = PI * 2.0;

const C_CVOLUME: usize = 0;
const C_MVOLUME: usize = 1;
const C_PAN: usize = 2;
const C_CMUL: usize = 3;
const C_MMUL: usize = 4;
const C_MSELFMOD: usize = 5;
const C_CA: usize = 6;
const C_CD: usize = 7;
const C_CS: usize = 8;
const C_CR: usize = 9;
const C_MA: usize = 10;
const C_MD: usize = 11;
const C_MS: usize = 12;
const C_MR: usize = 13;
const C_MSCALING: usize = 14;
const C_CHANNELS: usize = 15;
const C_MODE: usize = 16;

const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    cptr: i32,
    mptr: i32,
    c_env_ptr: i32,
    m_env_ptr: i32,
    cdelta: u32,
    sustain: bool,
    note: i32,
    rb: RenderbufPars,
}

pub(crate) struct Fm {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    search_ptr: usize,
    cvolume_table: Vec<f32>,
    mvolume_table: Vec<f32>,
    cvolume_table_size: i32,
    mvolume_table_size: i32,
    tables_render_request: bool,
    coefs: SmootherCoefs,
    buf: Vec<f32>,
}

impl Default for Fm {
    fn default() -> Fm {
        Fm {
            channels: [Chan::default(); MAX_CHANNELS],
            no_active_channels: false,
            search_ptr: 0,
            cvolume_table: vec![0.0; TABLE_SIZE],
            mvolume_table: vec![0.0; TABLE_SIZE],
            cvolume_table_size: 0,
            mvolume_table_size: 0,
            tables_render_request: false,
            coefs: SmootherCoefs::default(),
            buf: Vec::new(),
        }
    }
}

/// render_level: a straight line into the table, x1 to x2 inclusive.
fn render_level(table: &mut [f32], x1: i32, y1: i32, x2: i32, y2: i32) {
    let mut size = x2 - x1;
    if size == 0 {
        size = 1;
    }
    let delta = (y2 - y1) as f32 / size as f32;
    let mut cy = y1 as f32;
    for cx in x1..=x2 {
        if let Some(t) = table.get_mut(cx as usize) {
            *t = cy / TABLE_AMP as f32;
        }
        cy += delta;
    }
}

impl Fm {
    fn render_volume_tables(&mut self, net: &Net, n: usize) {
        let m = &net.mods[n];
        let c = |i| m.ctl(i);
        let x = c(C_CA);
        let y = TABLE_AMP;
        render_level(&mut self.cvolume_table, 0, 0, x, y);
        let x2 = x + c(C_CD);
        let y2 = c(C_CS) * (TABLE_AMP / 256);
        render_level(&mut self.cvolume_table, x, y, x2, y2);
        let x = x2 + c(C_CR);
        render_level(&mut self.cvolume_table, x2, y2, x, 0);
        self.cvolume_table[(x + 1) as usize] = 0.0;
        self.cvolume_table[(x + 2) as usize] = 0.0;
        self.cvolume_table_size = x;
        let x = c(C_MA);
        let y = c(C_MVOLUME) * (TABLE_AMP / 256);
        render_level(&mut self.mvolume_table, 0, 0, x, y);
        let x2 = x + c(C_MD);
        let y2 = ((c(C_MVOLUME) * c(C_MS)) / 256) * (TABLE_AMP / 256);
        render_level(&mut self.mvolume_table, x, y, x2, y2);
        let x = x2 + c(C_MR);
        render_level(&mut self.mvolume_table, x2, y2, x, 0);
        self.mvolume_table[(x + 1) as usize] = 0.0;
        self.mvolume_table[(x + 2) as usize] = 0.0;
        self.mvolume_table_size = x;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mode = net.mods[n].ctl(C_MODE);
        let want = if mode == MODE_HQ_MONO || mode == MODE_LQ_MONO { 1 } else { 2 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
        }
        if self.tables_render_request {
            self.render_volume_tables(net, n);
            self.tables_render_request = false;
        }
        let base = net.base_host_version;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.max(0) as usize;
        let c = |i| m.ctl(i);
        let (mselfmod, ctl_mscaling, ctl_channels, ctl_cmul, ctl_mmul, ctl_ca, ctl_cd, ctl_ma, ctl_md, ctl_cvolume, ctl_pan) =
            (c(C_MSELFMOD), c(C_MSCALING), c(C_CHANNELS), c(C_CMUL), c(C_MMUL), c(C_CA), c(C_CD), c(C_MA), c(C_MD), c(C_CVOLUME), c(C_PAN));
        let (csize, msize) = (self.cvolume_table_size, self.mvolume_table_size);
        let mut retval = 0i32;
        if self.buf.len() < frames {
            self.buf.resize(frames, 0.0);
        }
        self.no_active_channels = true;
        let mask: i32 = (1 << 22) - 1;
        for ci in 0..ctl_channels.max(0) as usize {
            let chan = &mut self.channels[ci];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            let mut cdelta = chan.cdelta;
            if ctl_cmul > 0 {
                cdelta = cdelta.wrapping_mul(ctl_cmul as u32);
            } else {
                cdelta /= 2;
            }
            let mdelta = if ctl_mmul > 0 { chan.cdelta.wrapping_mul(ctl_mmul as u32) } else { chan.cdelta / 2 };
            let c_sus = (ctl_ca + ctl_cd) * ENV_STEP;
            let m_sus = (ctl_ma + ctl_md) * ENV_STEP;
            let m_env_max = if base >= 0x01070401 { (msize + 1) * ENV_STEP } else { msize * ENV_STEP };
            let mut mscaling = 0;
            for i in 0..ctl_mscaling {
                if i == 0 {
                    mscaling = 128 - chan.note;
                } else {
                    mscaling = (mscaling * (128 - chan.note)) / 128;
                }
            }
            let rb = &mut self.buf;
            let mut cptr = chan.cptr;
            let mut mptr = chan.mptr;
            let mut c_env = chan.c_env_ptr;
            let mut m_env = chan.m_env_ptr;
            let mut i = 0usize;
            while i < frames {
                let mut mvol: f32 = if mode < MODE_LQ {
                    let coef = m_env & (ENV_STEP - 1);
                    let cf = coef as f32 / ENV_STEP as f32;
                    let k = (m_env / ENV_STEP) as usize;
                    cf * self.mvolume_table[k + 1] + (1.0 - cf) * self.mvolume_table[k]
                } else {
                    self.mvolume_table[(m_env / ENV_STEP) as usize]
                };
                if ctl_mscaling != 0 {
                    mvol = (mvol * mscaling as f32) / 128.0;
                }
                let mut cval: f32 = if mselfmod == 0 {
                    let mp = TWO_PI * ((mptr & mask) as f32 / (1 << 22) as f32);
                    let cp = TWO_PI * ((cptr & mask) as f32 / (1 << 22) as f32);
                    (mp.sin() * mvol * TWO_PI * 8.0 + cp).sin()
                } else {
                    let mptr_f = TWO_PI * ((mptr & mask) as f32 / (1 << 22) as f32);
                    let fb = ((mptr_f.sin() * mselfmod as f32) / 256.0) * TWO_PI * 8.0;
                    let mut s = (fb + mptr_f).sin();
                    s *= mvol;
                    s *= TWO_PI * 8.0;
                    s += TWO_PI * ((cptr & mask) as f32 / (1 << 22) as f32);
                    s.sin()
                };
                if mode < MODE_LQ {
                    let coef = c_env & (ENV_STEP - 1);
                    let cf = coef as f32 / ENV_STEP as f32;
                    let k = (c_env / ENV_STEP) as usize;
                    cval *= cf * self.cvolume_table[k + 1] + (1.0 - cf) * self.cvolume_table[k];
                } else {
                    cval *= self.cvolume_table[(c_env / ENV_STEP) as usize];
                }
                rb[i] = cval;
                cptr = cptr.wrapping_add(cdelta as i32);
                mptr = mptr.wrapping_add(mdelta as i32);
                c_env += 1;
                m_env += 1;
                if chan.sustain {
                    if c_env > c_sus {
                        c_env = c_sus;
                    }
                    if m_env > m_sus {
                        m_env = m_sus;
                    }
                } else {
                    if c_env >= csize * ENV_STEP {
                        chan.playing = false;
                        break;
                    }
                    if m_env >= m_env_max {
                        m_env = m_env_max;
                    }
                }
                i += 1;
            }
            chan.cptr = cptr;
            chan.mptr = mptr;
            chan.c_env_ptr = c_env;
            chan.m_env_ptr = m_env;
            if !chan.playing {
                chan.id = !0;
            }
            retval = renderbuf2output(retval, &mut m.chout, outputs_num, offset, frames, &rb[..frames], None, i, (ctl_cvolume * chan.vel) / 2, ctl_pan * 256, &mut chan.rb, &self.coefs, FM_SFREQ);
        }
        retval as isize
    }
}

impl Module for Fm {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 128, 0, C_CVOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 48, 0, C_MVOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 2, -128);
                net.register_ctl(n, 0, 16, 1, 0, C_CMUL, -1, 0);
                net.register_ctl(n, 0, 16, 1, 0, C_MMUL, -1, 0);
                net.register_ctl(n, 0, 256, 0, 0, C_MSELFMOD, -1, 0);
                net.register_ctl(n, 0, 512, 32, 0, C_CA, -1, 1);
                net.register_ctl(n, 0, 512, 32, 0, C_CD, -1, 1);
                net.register_ctl(n, 0, 256, 128, 0, C_CS, -1, 1);
                net.register_ctl(n, 0, 512, 64, 0, C_CR, -1, 1);
                net.register_ctl(n, 0, 512, 32, 0, C_MA, -1, 2);
                net.register_ctl(n, 0, 512, 32, 0, C_MD, -1, 2);
                net.register_ctl(n, 0, 256, 128, 0, C_MS, -1, 2);
                net.register_ctl(n, 0, 512, 64, 0, C_MR, -1, 2);
                net.register_ctl(n, 0, 4, 0, 0, C_MSCALING, -1, 2);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 4, 1, C_CHANNELS, -1, 3);
                net.register_ctl(n, 0, 3, MODE_HQ_MONO, 1, C_MODE, -1, 3);
                for c in self.channels.iter_mut() {
                    *c = Chan { id: !0, ..Chan::default() };
                }
                self.no_active_channels = true;
                self.search_ptr = 0;
                self.cvolume_table_size = 0;
                self.mvolume_table_size = 0;
                self.tables_render_request = true;
                self.coefs = SmootherCoefs::new(100.0, FM_SFREQ as f32);
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    c.playing = false;
                    c.id = !0;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                if !self.no_active_channels {
                    for c in self.channels.iter_mut() {
                        if c.id == e.id {
                            c.sustain = false;
                            c.id = !0;
                            break;
                        }
                    }
                    let mut c = 0;
                    while c < ctl_channels {
                        if !self.channels[self.search_ptr].playing {
                            break;
                        }
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        c += 1;
                    }
                    if c == ctl_channels {
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        if net.base_host_version >= 0x02010301 {
                            for _ in 0..ctl_channels {
                                if !self.channels[self.search_ptr].sustain {
                                    break;
                                }
                                self.search_ptr += 1;
                                if self.search_ptr >= ctl_channels {
                                    self.search_ptr = 0;
                                }
                            }
                        }
                    }
                } else {
                    self.search_ptr = 0;
                }
                let freq = net::get_freq(e.pitch() / 4);
                let (dh, dl) = net::get_delta(FM_SFREQ, freq);
                let ca = net.mods[n].ctl(C_CA);
                let chan = &mut self.channels[self.search_ptr];
                chan.rb.start = true;
                if chan.playing && ca != 0 {
                    chan.rb.anticlick = true;
                }
                chan.playing = true;
                chan.vel = e.velocity() as i32;
                chan.cdelta = dl | (dh << 16);
                chan.cptr = 0;
                chan.mptr = 0;
                chan.c_env_ptr = 0;
                chan.m_env_ptr = 0;
                chan.id = e.id;
                chan.note = if net.base_host_version == 0x01070000 { e.root_note() as i32 } else { ((net::NOTE0_PITCH - e.pitch()) / 256).clamp(0, 10 * 12 - 1) };
                chan.sustain = true;
                self.no_active_channels = false;
                retval = 1;
            }
            Cmd::SetFreq => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        let freq = net::get_freq(e.pitch() / 4);
                        let (dh, dl) = net::get_delta(FM_SFREQ, freq);
                        self.channels[c].cdelta = dl | (dh << 16);
                        self.channels[c].vel = e.velocity() as i32;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        self.channels[c].vel = e.velocity() as i32;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                for c in self.channels.iter_mut() {
                    if c.id == e.id {
                        c.sustain = false;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                for c in self.channels.iter_mut() {
                    c.sustain = false;
                    c.id = !0;
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => self.tables_render_request = true,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
