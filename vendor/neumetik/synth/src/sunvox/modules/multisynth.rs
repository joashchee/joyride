// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MultiSynth (psynths_multisynth.cpp): the notes it receives passed on
//! to the modules it links to, transposed, detuned at random, their
//! velocity through two curves, their pitch through a third, to all its
//! outputs or one chosen by note, voice, turn or chance.

use super::super::net::{self, evt_id_inc, Cmd, Event, Module, Net, RT_MUTE};

const MAX_CHANNELS: usize = 64;
const HISTORY_SIZE: usize = 64;

const OPT_FLAG_GEN_MISSED_NOTEOFF: u32 = 1 << 0;
const OPT_FLAG_ROUND_PITCH: u32 = 1 << 1;
const OPT_FLAG_ROUND_PITCH2: u32 = 1 << 2;
const OPT_FLAG_REC_CURVE3: u32 = 1 << 3;
const OPT_FLAG_NOTE_DIFF: u32 = 1 << 5;
const OUTSLOT_OFF: u32 = 6;
const OUTSLOT_NOTE: u32 = 1 << OUTSLOT_OFF;
const OUTSLOT_POLYCHANNEL: u32 = 2 << OUTSLOT_OFF;
const OUTSLOT_ROUNDROBIN: u32 = 3 << OUTSLOT_OFF;
const OUTSLOT_RANDOM1: u32 = 4 << OUTSLOT_OFF;
const OUTSLOT_RANDOM2: u32 = 5 << OUTSLOT_OFF;
const OUTSLOT_RANDOM3: u32 = 6 << OUTSLOT_OFF;
const OUTSLOTS: u32 = 15 << OUTSLOT_OFF;

const CHAN_FLAG_FROM_HISTORY: u8 = 1 << 1;

const C_TRANSPOSE: usize = 0;
const C_RANDOM: usize = 1;
const C_VELOCITY: usize = 2;
const C_FINETUNE: usize = 3;
const C_RANDOM_PHASE: usize = 4;
const C_RANDOM_VELOCITY: usize = 5;
const C_PHASE: usize = 6;
const C_CURVE2_MIX: usize = 7;

const CURVE_CHUNK: [usize; 3] = [0, 2, 3];
const CURVE_ITEM: [usize; 3] = [1, 1, 2];
const CURVE_LEN: [usize; 3] = [128, 257, 128];

/// multisynth_options, kept in chunk 1 (8 bytes).
#[derive(Clone, Copy, Default)]
struct Opt {
    static_freq: u8,
    ignore_vel0: u8,
    curve_num: u8,
    trigger: u8,
    flags: u32,
}

#[derive(Clone, Copy)]
struct Chan {
    id: u32,
    pitch: i32,
    original_pitch: i32,
    out: u8,
    flags: u8,
}

impl Default for Chan {
    fn default() -> Chan {
        Chan { id: 0xFFFF_FFFF, pitch: 0, original_pitch: 0, out: 0, flags: 0 }
    }
}

#[derive(Clone, Copy)]
struct Hist {
    id: u32,
    pitch: i32,
    out: u8,
}

impl Default for Hist {
    fn default() -> Hist {
        Hist { id: 0xFFFF_FFFF, pitch: 0, out: 0 }
    }
}

pub(crate) struct MultiSynth {
    rand1: u32,
    rand2: u32,
    opt: Opt,
    channels: [Chan; MAX_CHANNELS + 1],
    active_chans: i32,
    outslot_cnt: i32,
    prev_random_outslot: i32,
    curve_changed: u32,
    rec_curve3_cnt: i32,
    hist: [Hist; HISTORY_SIZE],
    hist_wp: usize,
}

impl Default for MultiSynth {
    fn default() -> MultiSynth {
        MultiSynth {
            rand1: 0,
            rand2: 0,
            opt: Opt::default(),
            channels: [Chan::default(); MAX_CHANNELS + 1],
            active_chans: 0,
            outslot_cnt: 0,
            prev_random_outslot: 0,
            curve_changed: 0,
            rec_curve3_cnt: 0,
            hist: [Hist::default(); HISTORY_SIZE],
            hist_wp: 0,
        }
    }
}

fn curve_u8(net: &Net, n: usize, chunk: usize, i: usize) -> i32 {
    net.mods[n].chunk(chunk).and_then(|c| c.data.get(i)).copied().unwrap_or(0) as i32
}

fn curve_u16(net: &Net, n: usize, i: usize) -> i32 {
    net.mods[n].chunk(3).and_then(|c| c.data.get(i * 2..i * 2 + 2)).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32)
}

fn set_curve_u16(net: &mut Net, n: usize, i: usize, v: i32) {
    if let Some(b) = net.mods[n].chunk_mut(3).and_then(|c| c.data.get_mut(i * 2..i * 2 + 2)) {
        b.copy_from_slice(&(v as u16).to_le_bytes());
    }
}

impl MultiSynth {
    fn store_opt(&self, net: &mut Net, n: usize) {
        if let Some(c) = net.mods[n].chunk_mut(1) {
            if c.data.len() >= 8 {
                let o = &self.opt;
                c.data[0] = o.static_freq;
                c.data[1] = o.ignore_vel0;
                c.data[2] = o.curve_num;
                c.data[3] = o.trigger;
                c.data[4..8].copy_from_slice(&o.flags.to_le_bytes());
            }
        }
    }

    fn reset_curve(&mut self, num: usize, net: &mut Net, n: usize) {
        let len = CURVE_LEN[num];
        let Some(c) = net.mods[n].chunk_mut(CURVE_CHUNK[num]) else { return };
        match num {
            0 => {
                for b in c.data.iter_mut().take(len) {
                    *b = 255;
                }
            }
            1 => {
                for (i, b) in c.data.iter_mut().take(len).enumerate() {
                    *b = if i < 256 { i as u8 } else { 255 };
                }
            }
            _ => {
                for i in 0..len {
                    if let Some(b) = c.data.get_mut(i * 2..i * 2 + 2) {
                        b.copy_from_slice(&((16384 + i * 256) as u16).to_le_bytes());
                    }
                }
            }
        }
        self.curve_changed &= !(1 << num);
        self.opt.flags &= !OPT_FLAG_REC_CURVE3;
        self.store_opt(net, n);
        net.change_counter += 1;
    }

    fn new_curve(&mut self, num: usize, net: &mut Net, n: usize) {
        if net.mods[n].chunk(CURVE_CHUNK[num]).is_some() {
            self.curve_changed |= 1 << num;
        } else {
            net.new_chunk_sized(n, CURVE_CHUNK[num], CURVE_LEN[num] * CURVE_ITEM[num], 0, 0);
            self.reset_curve(num, net, n);
        }
        net.change_counter += 1;
    }

    fn calc_active_chans(&mut self, last_disabled: i32) {
        let mut i = last_disabled;
        while i >= 0 {
            if self.channels[i as usize].id != 0xFFFF_FFFF {
                break;
            }
            if self.active_chans - 1 <= i {
                self.active_chans = i;
                i -= 1;
            } else {
                break;
            }
        }
    }

    fn release_chan(&mut self, cn: usize) {
        let chan = self.channels[cn];
        let wp = self.hist_wp.wrapping_sub(1) & (HISTORY_SIZE - 1);
        if self.hist[wp].id == chan.id {
            self.hist_wp = wp;
        }
        self.hist[self.hist_wp] = Hist { id: chan.id, pitch: chan.pitch, out: chan.out };
        self.hist_wp = (self.hist_wp + 1) & (HISTORY_SIZE - 1);
        self.channels[cn].id = 0xFFFF_FFFF;
        self.channels[cn].flags = 0;
    }

    /// convert_outslot: the n-th output link that's there, as a link index.
    fn convert_outslot(links: &[i32], n: i32) -> i32 {
        let mut i2 = 0;
        for (i, &l) in links.iter().enumerate() {
            if l >= 0 {
                if n == i2 {
                    return i as i32;
                }
                i2 += 1;
            }
        }
        -1
    }

    /// Sends to the output a channel was given (or to all for 0).
    fn send_to_out(net: &mut Net, n: usize, out: u8, e: &Event) {
        if out == 0 {
            net.multisend(n, e);
        } else if ((out - 1) as usize) < net.mods[n].output_links.len() {
            let l = net.mods[n].output_links[(out - 1) as usize];
            net.add_event(l as u32 as usize, e);
        }
    }

    fn note(&mut self, n: usize, event: &Event, net: &mut Net) -> isize {
        if net.mods[n].realtime_flags & RT_MUTE != 0 {
            return 0;
        }
        let mut e = *event;
        e.id = evt_id_inc(e.id, n as u32);
        let opt = self.opt;
        let base = net.base_host_version;
        let m = &net.mods[n];
        let (ctl_transpose, ctl_random, ctl_velocity, ctl_finetune, ctl_random_phase, ctl_random_velocity, ctl_phase, ctl_curve2_mix) =
            (m.ctl(C_TRANSPOSE), m.ctl(C_RANDOM), m.ctl(C_VELOCITY), m.ctl(C_FINETUNE), m.ctl(C_RANDOM_PHASE), m.ctl(C_RANDOM_VELOCITY), m.ctl(C_PHASE), m.ctl(C_CURVE2_MIX));
        let mut chan_num = MAX_CHANNELS;
        for i in 0..self.active_chans.max(0) as usize {
            if self.channels[i].id == e.id {
                chan_num = i;
                if e.cmd == Cmd::NoteOn {
                    let outslot_mode = opt.flags & OUTSLOTS;
                    if opt.flags & OPT_FLAG_GEN_MISSED_NOTEOFF != 0 || (outslot_mode != 0 && outslot_mode != OUTSLOT_POLYCHANNEL) {
                        e.cmd = Cmd::NoteOff;
                        Self::send_to_out(net, n, self.channels[i].out, &e);
                        e.cmd = Cmd::NoteOn;
                    }
                }
                break;
            }
        }
        if chan_num == MAX_CHANNELS {
            for i in 0..MAX_CHANNELS {
                if self.channels[i].id == 0xFFFF_FFFF {
                    chan_num = i;
                    if e.cmd != Cmd::NoteOn {
                        let mut rp = self.hist_wp.wrapping_sub(1) & (HISTORY_SIZE - 1);
                        for _ in 0..HISTORY_SIZE {
                            let h = self.hist[rp];
                            if h.id == e.id {
                                self.channels[i].pitch = h.pitch;
                                self.channels[i].out = h.out;
                                self.channels[i].flags |= CHAN_FLAG_FROM_HISTORY;
                                break;
                            }
                            rp = rp.wrapping_sub(1) & (HISTORY_SIZE - 1);
                        }
                    }
                    break;
                }
            }
        }
        let mut pitch;
        if e.cmd == Cmd::SetVelocity {
            pitch = self.channels[chan_num].pitch;
        } else {
            pitch = if opt.static_freq != 0 && base < 0x01090000 { net::NOTE0_PITCH - 5 * 12 * 256 } else { e.pitch() };
            pitch = pitch - (ctl_finetune - 256) - (ctl_transpose - 128) * 256;
            if base == 0x01070000 {
                pitch -= net.mods[n].finetune;
            }
            if ctl_random != 0 {
                let mut r = net::pseudo_random(&mut self.rand1) as i32;
                r &= 8191;
                r -= 4096;
                r *= ctl_random;
                r /= 4096;
                pitch += r;
            }
        }
        let note = ((net::NOTE0_PITCH - pitch) / 256).clamp(0, 127);
        let mut vel = (e.velocity() as i32 * ctl_velocity) / 256;
        if ctl_random_velocity != 0 {
            let rnd = 32768 - ((net::pseudo_random(&mut self.rand1) as i32 * ctl_random_velocity) >> 15);
            vel = (vel * rnd) >> 15;
        }
        let vv = curve_u8(net, n, 0, note as usize);
        if vv != 255 {
            vel = (vel * vv) / 255;
        }
        if ctl_curve2_mix != 0 {
            vel = (vel * (256 - ctl_curve2_mix) + curve_u8(net, n, 2, vel as usize) * ctl_curve2_mix) >> 8;
            if vel >= 255 {
                vel = 256;
            }
        }
        e.set_velocity(vel);
        let mut note_closed = false;
        if opt.trigger != 0 && e.cmd == Cmd::NoteOn {
            for i in 0..self.active_chans.max(0) as usize {
                let ch2 = self.channels[i];
                if ch2.pitch == pitch && ch2.id != 0xFFFF_FFFF {
                    e.id = ch2.id;
                    e.cmd = Cmd::NoteOff;
                    self.release_chan(i);
                    self.calc_active_chans(i as i32);
                    note_closed = true;
                    chan_num = i;
                    break;
                }
            }
        }
        if !note_closed {
            let links = net.mods[n].output_links.clone();
            let chan = &mut self.channels[chan_num];
            chan.id = e.id;
            chan.pitch = pitch;
            if e.cmd != Cmd::SetVelocity {
                chan.original_pitch = e.pitch();
            }
            if e.cmd == Cmd::NoteOn {
                let outslot_mode = opt.flags & OUTSLOTS;
                if outslot_mode != 0 {
                    let outs = links.iter().filter(|&&l| l >= 0).count() as i32;
                    if outs != 0 {
                        let mut out;
                        match outslot_mode {
                            OUTSLOT_NOTE => {
                                out = ((net::NOTE0_PITCH - pitch) / 256) % outs;
                                self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                            }
                            OUTSLOT_POLYCHANNEL => {
                                out = chan_num as i32 % outs;
                                self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                            }
                            OUTSLOT_ROUNDROBIN => {
                                let mut out2 = 0;
                                for _ in 0..outs {
                                    if self.outslot_cnt >= outs {
                                        self.outslot_cnt = 0;
                                    }
                                    out = self.outslot_cnt;
                                    out2 = Self::convert_outslot(&links, out) + 1;
                                    for i in 0..self.active_chans.max(0) as usize {
                                        let c2 = self.channels[i];
                                        if c2.id == 0xFFFF_FFFF || c2.flags & CHAN_FLAG_FROM_HISTORY != 0 {
                                            continue;
                                        }
                                        if c2.out as i32 == out2 {
                                            out2 = 0;
                                            break;
                                        }
                                    }
                                    self.outslot_cnt += 1;
                                    if out2 != 0 {
                                        self.channels[chan_num].out = out2 as u8;
                                        break;
                                    }
                                }
                                if out2 == 0 {
                                    if self.outslot_cnt >= outs {
                                        self.outslot_cnt = 0;
                                    }
                                    out = self.outslot_cnt;
                                    self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                                    self.outslot_cnt += 1;
                                }
                            }
                            OUTSLOT_RANDOM1 => {
                                out = net::pseudo_random(&mut self.rand2) as i32 * outs / 32768;
                                self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                            }
                            OUTSLOT_RANDOM2 => {
                                out = net::pseudo_random(&mut self.rand2) as i32 % outs;
                                self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                            }
                            OUTSLOT_RANDOM3 => {
                                loop {
                                    out = net::pseudo_random(&mut self.rand2) as i32 * outs / 32768;
                                    if outs < 3 || out != self.prev_random_outslot {
                                        break;
                                    }
                                }
                                self.channels[chan_num].out = (Self::convert_outslot(&links, out) + 1) as u8;
                                self.prev_random_outslot = out;
                            }
                            _ => {}
                        }
                    }
                } else {
                    self.channels[chan_num].out = 0;
                }
            }
            if chan_num as i32 >= self.active_chans {
                self.active_chans = (chan_num as i32 + 1).min(MAX_CHANNELS as i32);
            }
        }
        if opt.static_freq != 0 && base >= 0x01090000 {
            pitch = net::NOTE0_PITCH - 5 * 12 * 256;
            pitch = pitch - (ctl_finetune - 256) - (ctl_transpose - 128) * 256;
            if ctl_random != 0 {
                let mut r = net::pseudo_random(&mut self.rand1) as i32;
                r &= 8191;
                r -= 4096;
                r *= ctl_random;
                r /= 4096;
                pitch += r;
            }
        }
        if self.curve_changed & (1 << 2) != 0 {
            let mut in_pitch = net::NOTE0_PITCH - pitch;
            if opt.flags & OPT_FLAG_ROUND_PITCH != 0 {
                in_pitch += 128;
            }
            let mut in_note = (in_pitch + 65536) / 256 - 256;
            let out_pitch;
            if opt.flags & OPT_FLAG_ROUND_PITCH2 != 0 {
                in_pitch += 16384;
                in_note = in_note.clamp(0, 127);
                let mut op = curve_u16(net, n, in_note as usize);
                let mut diff = (in_pitch - op).abs();
                for i in in_note + 1..128 {
                    let p = curve_u16(net, n, i as usize);
                    let d2 = (in_pitch - p).abs();
                    if d2 > diff {
                        break;
                    }
                    if d2 < diff {
                        diff = d2;
                        op = p;
                    }
                }
                let mut i = in_note - 1;
                while i >= 0 {
                    let p = curve_u16(net, n, i as usize);
                    let d2 = (in_pitch - p).abs();
                    if d2 > diff {
                        break;
                    }
                    if d2 < diff {
                        diff = d2;
                        op = p;
                    }
                    i -= 1;
                }
                out_pitch = op;
            } else if !(0..127).contains(&in_note) {
                out_pitch = curve_u16(net, n, if in_note < 0 { 0 } else { 127 });
            } else {
                let mut op = curve_u16(net, n, in_note as usize);
                if opt.flags & OPT_FLAG_ROUND_PITCH == 0 {
                    let c = in_pitch & 255;
                    op = (op * (256 - c) + curve_u16(net, n, in_note as usize + 1) * c) / 256;
                }
                out_pitch = op;
            }
            pitch = net::NOTE0_PITCH - (out_pitch - 16384);
        } else if opt.flags & (OPT_FLAG_ROUND_PITCH | OPT_FLAG_ROUND_PITCH2) != 0 {
            let in_pitch = net::NOTE0_PITCH - pitch + 128;
            let in_note = (in_pitch + 65536) / 256 - 256;
            pitch = net::NOTE0_PITCH - in_note * 256;
        }
        if opt.flags & OPT_FLAG_NOTE_DIFF != 0 {
            pitch = pitch - self.channels[chan_num].original_pitch + (net::NOTE0_PITCH - 5 * 12 * 256);
        }
        if !(opt.ignore_vel0 != 0 && e.velocity() == 0) {
            let links = net.mods[n].output_links.clone();
            let out = self.channels[chan_num].out;
            let (l0, l1) = if out != 0 { ((out - 1) as usize, out as usize) } else { (0, links.len()) };
            for &l in links.iter().take(l1).skip(l0) {
                if let Some(s) = net.get(l) {
                    let mut p = pitch - s.finetune - s.relative_note * 256;
                    if base < 0x01080000 && p >= 7680 * 4 {
                        p = 7680 * 4;
                    }
                    e.set_pitch(p);
                    net.add_event(l as usize, &e);
                }
            }
            if (ctl_random_phase != 0 || ctl_phase != 0) && e.cmd == Cmd::NoteOn {
                for &l in links.iter().take(l1).skip(l0) {
                    if net.exists(l) {
                        e.cmd = Cmd::SetSampleOffset2;
                        let so = ((net::pseudo_random(&mut self.rand1) as i32 * ctl_random_phase) >> 15) as u32;
                        e.set_sample_offset(so.wrapping_add(ctl_phase as u32));
                        net.add_event(l as usize, &e);
                    }
                }
            }
        }
        1
    }
}

impl Module for MultiSynth {
    fn handle(&mut self, n: usize, event: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match event.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 128, 1, C_TRANSPOSE, 128, 0);
                net.set_ctl_show_offset(n, 0, -128);
                net.register_ctl(n, 0, 4096, 0, 0, C_RANDOM, -1, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_VELOCITY, -1, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_FINETUNE, 256, 0);
                net.set_ctl_show_offset(n, 3, -256);
                net.register_ctl(n, 0, 0x8000, 0, 0, C_RANDOM_PHASE, -1, 1);
                net.register_ctl(n, 0, 0x8000, 0, 0, C_RANDOM_VELOCITY, -1, 1);
                net.register_ctl(n, 0, 0x8000, 0, 0, C_PHASE, -1, 0);
                net.register_ctl(n, 0, 256, 256, 0, C_CURVE2_MIX, -1, 0);
                // stime_ms() + mod_num * 33, the clock at 0.
                self.rand1 = (n as u32).wrapping_mul(33);
                self.rand2 = self.rand1.wrapping_add((n as u32).wrapping_mul(39));
                self.channels = [Chan::default(); MAX_CHANNELS + 1];
                self.active_chans = 0;
                self.hist = [Hist::default(); HISTORY_SIZE];
                retval = 1;
            }
            Cmd::SetupFinished => {
                net.chunk_autocreate(n, 1, 8, true);
                let d = &net.mods[n].chunk(1).unwrap().data;
                self.opt = Opt { static_freq: d[0], ignore_vel0: d[1], curve_num: d[2], trigger: d[3], flags: u32::from_le_bytes([d[4], d[5], d[6], d[7]]) };
                for i in 0..3 {
                    self.new_curve(i, net, n);
                }
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    c.id = 0xFFFF_FFFF;
                    c.pitch = 0;
                    c.flags = 0;
                }
                self.active_chans = 0;
                for h in self.hist.iter_mut() {
                    h.id = 0xFFFF_FFFF;
                }
                retval = 1;
            }
            Cmd::NoteOn if self.opt.flags & OPT_FLAG_REC_CURVE3 != 0 => {
                let in_pitch = net::NOTE0_PITCH - event.pitch();
                if in_pitch >= 0 {
                    let in_pitch = in_pitch % (256 * 12);
                    let in_note = in_pitch / 256;
                    if self.rec_curve3_cnt == 0 {
                        for oct in 0..11 {
                            let from = oct * 12;
                            let to = (from + 12).min(128);
                            for i in from..to {
                                set_curve_u16(net, n, i as usize, 16384 + in_pitch + oct * 256 * 12);
                            }
                        }
                    } else {
                        for oct in 0..11 {
                            let mut i = oct * 12 + in_note;
                            if i < 128 {
                                let p0 = curve_u16(net, n, i as usize);
                                let p = 16384 + in_pitch + oct * 256 * 12;
                                if p > p0 {
                                    let oct_end = (oct * 12 + 12).min(128);
                                    while i < oct_end && p > curve_u16(net, n, i as usize) {
                                        set_curve_u16(net, n, i as usize, p);
                                        i += 1;
                                    }
                                }
                                if p < p0 {
                                    let oct_begin = oct * 12;
                                    while i >= oct_begin && p < curve_u16(net, n, i as usize) {
                                        set_curve_u16(net, n, i as usize, p);
                                        i -= 1;
                                    }
                                }
                            }
                        }
                    }
                    self.rec_curve3_cnt += 1;
                    self.curve_changed |= 1 << 2;
                }
                retval = 1;
            }
            Cmd::NoteOn | Cmd::SetFreq | Cmd::SetVelocity => retval = self.note(n, event, net),
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if net.mods[n].realtime_flags & RT_MUTE != 0 {
                    return 0;
                }
                let mut e = *event;
                e.id = evt_id_inc(e.id, n as u32);
                if e.cmd == Cmd::SetSampleOffset2 {
                    let v = e.sample_offset().wrapping_add(net.mods[n].ctl(C_PHASE) as u32);
                    e.set_sample_offset(v.min(32768));
                }
                if self.opt.flags & OUTSLOTS != 0 {
                    for i in 0..self.active_chans.max(0) as usize {
                        if self.channels[i].id == e.id {
                            Self::send_to_out(net, n, self.channels[i].out, &e);
                            break;
                        }
                    }
                } else {
                    net.multisend(n, &e);
                }
                retval = 1;
            }
            Cmd::Muted => {
                for i in 0..self.active_chans.max(0) as usize {
                    if self.channels[i].id != 0xFFFF_FFFF {
                        let mut e = *event;
                        e.cmd = Cmd::NoteOff;
                        e.set_velocity(256);
                        e.set_pitch(0);
                        e.id = self.channels[i].id;
                        net.multisend(n, &e);
                    }
                    self.release_chan(i);
                }
                self.active_chans = 0;
            }
            Cmd::AllNotesOff => {
                if net.mods[n].realtime_flags & RT_MUTE != 0 {
                    return 0;
                }
                for i in 0..self.active_chans.max(0) as usize {
                    self.release_chan(i);
                }
                self.active_chans = 0;
                retval = 1;
            }
            Cmd::NoteOff => {
                if net.mods[n].realtime_flags & RT_MUTE != 0 {
                    return 0;
                }
                let mut e = *event;
                e.id = evt_id_inc(e.id, n as u32);
                if self.opt.trigger != 0 {
                    return 1;
                }
                let mut out = 0;
                for i in 0..self.active_chans.max(0) as usize {
                    if self.channels[i].id == e.id {
                        out = self.channels[i].out;
                        self.release_chan(i);
                        self.calc_active_chans(i as i32);
                        break;
                    }
                }
                Self::send_to_out(net, n, out, &e);
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                let v = event.ctl_val() as i32;
                let o = &mut self.opt;
                retval = 1;
                match event.ctl_num() as i32 + 1 {
                    127 => o.static_freq = (v != 0) as u8,
                    126 => o.ignore_vel0 = (v != 0) as u8,
                    125 => o.curve_num = (v % 3) as u8,
                    124 => o.trigger = (v != 0) as u8,
                    123 => {
                        o.flags &= !OPT_FLAG_GEN_MISSED_NOTEOFF;
                        if v != 0 {
                            o.flags |= OPT_FLAG_GEN_MISSED_NOTEOFF;
                        }
                    }
                    122 | 121 => {
                        o.flags &= !(OPT_FLAG_ROUND_PITCH | OPT_FLAG_ROUND_PITCH2);
                        if v != 0 {
                            o.flags |= if event.ctl_num() as i32 + 1 == 122 { OPT_FLAG_ROUND_PITCH } else { OPT_FLAG_ROUND_PITCH2 };
                        }
                    }
                    120 => {
                        o.flags &= !OPT_FLAG_REC_CURVE3;
                        if v != 0 {
                            o.flags |= OPT_FLAG_REC_CURVE3;
                        }
                        self.rec_curve3_cnt = 0;
                    }
                    118 => {
                        o.flags &= !OPT_FLAG_NOTE_DIFF;
                        if v != 0 {
                            o.flags |= OPT_FLAG_NOTE_DIFF;
                        }
                    }
                    k @ 112..=117 => {
                        o.flags &= !OUTSLOTS;
                        if v != 0 {
                            o.flags |= match k {
                                117 => OUTSLOT_NOTE,
                                116 => OUTSLOT_POLYCHANNEL,
                                115 => OUTSLOT_ROUNDROBIN,
                                114 => OUTSLOT_RANDOM1,
                                113 => OUTSLOT_RANDOM2,
                                _ => OUTSLOT_RANDOM3,
                            };
                        }
                    }
                    _ => retval = 0,
                }
                if retval != 0 {
                    self.store_opt(net, n);
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
