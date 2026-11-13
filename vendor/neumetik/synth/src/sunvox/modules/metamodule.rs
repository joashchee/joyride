// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MetaModule (psynths_metamodule.cpp): a whole project inside a module.
//! Its notes go to the project's input module (or start its patterns,
//! transposed as an arpeggiator), its input feeds that module, its user
//! controllers drive the project's, and the project's Output is its
//! output, the events reaching that Output passed on.

use super::super::engine::{self, Note, UserCmd};
use super::super::net::{self, Cmd, Event, Module, Net, FLAG_DONT_FILL_INPUT, FLAG_EFFECT, FLAG_EXISTS, FLAG_GENERATOR, FLAG_GET_SPEED_CHANGES, RT_DONT_CLEAN_INPUT, RT_MUTE};
use super::super::subengine::SubEngine;

const USER_CTLS_OFFSET: usize = 5;
const MAX_USER_CTLS: usize = 96;
const MAX_CHANNELS: usize = 16;

const OPT_FLAG_GEN: u32 = 1 << 0;
const OPT_FLAG_NOTGEN: u32 = 1 << 1;
const OPT_FLAG_AUTO_BPM_TPL: u32 = 1 << 2;
const OPT_FLAG_IGNORE_EFF31_AFTER_LAST_NOTEOFF: u32 = 1 << 3;
const OPT_FLAG_JMP_TO_RL_PAT_AFTER_LAST_NOTEOFF: u32 = 1 << 4;

const C_VOLUME: usize = 0;
const C_INPUT: usize = 1;
const C_PLAY: usize = 2;
const C_BPM: usize = 3;
const C_TPL: usize = 4;

/// metamodule_options (chunk 2, 8 bytes).
#[derive(Clone, Copy)]
struct Opt {
    user_ctls_num: u8,
    arpeggiator: bool,
    use_velocity: bool,
    no_evt_out: bool,
    flags: u32,
}

impl Default for Opt {
    fn default() -> Opt {
        Opt { user_ctls_num: 3, arpeggiator: false, use_velocity: false, no_evt_out: false, flags: 0 }
    }
}

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
}

#[derive(Default)]
pub(crate) struct MetaModule {
    channels: [Chan; MAX_CHANNELS],
    active_channels: i32,
    ps: Option<SubEngine>,
    release_pattern: i32,
    ctl_links: Vec<u32>,
    opt: Opt,
}

fn cmd(note: u8, ctl: u16, ctl_val: u16) -> UserCmd {
    UserCmd { n: Note { note, vel: 0, module: 0, ctl, ctl_val }, ch: 0 }
}

impl MetaModule {
    fn var(net: &mut Net, n: usize, ctl: usize) -> Option<&mut i32> {
        let m = &mut net.mods[n];
        let v = m.stored_ctl(ctl)?.var?;
        m.vars.get_mut(v)
    }

    fn set_var(net: &mut Net, n: usize, ctl: usize, val: i32) {
        if let Some(v) = Self::var(net, n, ctl) {
            *v = val;
        }
    }

    /// metamodule_unpack_user_ctls: each user controller takes on the
    /// range, type and value of the project's controller it is linked to.
    fn unpack_user_ctls(&self, n: usize, net: &mut Net) {
        let Some(ps) = self.ps.as_ref() else { return };
        let sn = &ps.s.net;
        for i in 0..MAX_USER_CTLS {
            let link = self.ctl_links.get(i).copied().unwrap_or(0);
            let (link_mod, link_ctl) = ((link & 0xFFFF) as usize, ((link >> 16) & 0xFFFF) as usize);
            if link_mod >= sn.mods_num() || sn.mods[link_mod].flags & FLAG_EXISTS == 0 {
                continue;
            }
            let lm = &sn.mods[link_mod];
            let Some(c2) = lm.ctls.get(link_ctl) else { continue };
            let val2 = lm.ctl(link_ctl);
            let m = &mut net.mods[n];
            let Some(c) = m.stored_ctl_mut(i + USER_CTLS_OFFSET) else { continue };
            c.min = c2.min;
            c.max = c2.max;
            c.def = c2.def;
            c.normal_value = c2.normal_value;
            c.show_offset = c2.show_offset;
            c.typ = c2.typ;
            if let Some(var) = c.var {
                m.vars[var] = val2;
            }
        }
    }

    /// metamodule_get_flags: a generator or effect as the input module is.
    fn get_flags(&self, n: usize, net: &mut Net) {
        let Some(ps) = self.ps.as_ref() else { return };
        let ctl_input = net.mods[n].ctl(C_INPUT);
        let ctl_play = net.mods[n].ctl(C_PLAY);
        let sn = &ps.s.net;
        let mut flags = net.mods[n].flags & !(FLAG_GENERATOR | FLAG_EFFECT | FLAG_GET_SPEED_CHANGES);
        if let Some(im) = usize::try_from(ctl_input).ok().filter(|&i| i < sn.mods_num()).map(|i| &sn.mods[i]).filter(|m| m.flags & FLAG_EXISTS != 0) {
            flags |= im.flags & (FLAG_GENERATOR | FLAG_EFFECT);
        }
        if ctl_play != 0 {
            flags |= FLAG_GENERATOR;
        }
        if self.opt.flags & (OPT_FLAG_GEN | OPT_FLAG_NOTGEN) != 0 {
            flags &= !FLAG_GENERATOR;
            if self.opt.flags & OPT_FLAG_GEN != 0 {
                flags |= FLAG_GENERATOR;
            }
        }
        if self.opt.flags & OPT_FLAG_AUTO_BPM_TPL != 0 {
            flags |= FLAG_GET_SPEED_CHANGES;
        }
        net.mods[n].flags = flags;
    }

    fn check_auto_bpm_tpl(&mut self, n: usize, net: &mut Net, bpm: i32, tpl: i32) {
        if self.opt.flags & OPT_FLAG_AUTO_BPM_TPL == 0 {
            return;
        }
        let Some(ps) = self.ps.as_mut() else { return };
        let s = &mut ps.s;
        if bpm == 0 {
            s.bpm = net.host.bpm as u16;
            s.speed = net.host.speed as u8;
        } else {
            s.bpm = bpm as u16;
            s.speed = tpl as u8;
        }
        let (b, t) = (s.bpm as i32, s.speed as i32);
        Self::set_var(net, n, C_BPM, b);
        Self::set_var(net, n, C_TPL, t);
    }

    fn handle_ctl_play_simple(&mut self, ctl_play: i32) {
        if let Some(ps) = self.ps.as_mut() {
            ps.s.stop_at_the_end_of_proj = ctl_play == 2 || ctl_play == 4;
        }
    }

    fn handle_ctl_play(&mut self, ctl_play: i32, prev_ctl_play: i32) {
        let Some(ps) = self.ps.as_mut() else { return };
        let mut stop = prev_ctl_play > 0 && ctl_play == 0;
        if prev_ctl_play >= 3 && (ctl_play == 1 || ctl_play == 2) && self.active_channels == 0 {
            stop = true;
        }
        if stop {
            ps.s.user_commands.push_back(cmd(engine::NOTECMD_STOP, 0, 0));
            if self.active_channels > 0 {
                for c in self.channels.iter_mut() {
                    *c = Chan { playing: false, id: !0 };
                }
                self.active_channels = 0;
            }
        }
        self.handle_ctl_play_simple(ctl_play);
    }

    /// sunvox_add_psynth_event_UNSAFE to the input module, its pitch made
    /// the module's own (from 1.9).
    fn to_input(&mut self, net: &Net, ctl_input: i32, e: &mut Event, adjust_pitch: bool) {
        let Some(ps) = self.ps.as_mut() else { return };
        let sn = &ps.s.net;
        let Some(i) = usize::try_from(ctl_input).ok().filter(|&i| i < sn.mods_num()) else { return };
        if adjust_pitch && net.base_host_version >= 0x01090000 {
            let im = &sn.mods[i];
            e.set_pitch(e.pitch() - (im.finetune + im.relative_note * 256));
        }
        if sn.mods[i].flags & FLAG_EXISTS != 0 {
            ps.s.psynth_events.push((i as u16, *e));
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let Some(ps) = self.ps.as_mut() else { return 0 };
        let m = &net.mods[n];
        let (frames, offset) = (m.frames as usize, m.offset as usize);
        let outputs_num = m.output_channels.max(0) as usize;
        let mut input_signal = false;
        for ch in 0..outputs_num.min(2) {
            if m.in_empty[ch] < (offset + frames) as i32 {
                input_signal = true;
                break;
            }
        }
        let (ctl_volume, ctl_input) = (m.ctl(C_VOLUME), m.ctl(C_INPUT));
        let s = &mut ps.s;
        s.net.global_volume = ctl_volume;
        if m.ctl(C_BPM) != s.bpm as i32 || m.ctl(C_TPL) != s.speed as i32 {
            let (b, t) = (s.bpm as i32, s.speed as i32);
            Self::set_var(net, n, C_BPM, b);
            Self::set_var(net, n, C_TPL, t);
        }
        // The input module fed from this one's inputs.
        let mut input_mod = None;
        if let Some(i) = usize::try_from(ctl_input).ok().filter(|&i| i < s.net.mods_num()) {
            let im = &s.net.mods[i];
            let ok = im.flags & FLAG_EXISTS != 0 && im.flags & FLAG_EFFECT != 0 && im.flags & FLAG_DONT_FILL_INPUT == 0 && !im.input_links.iter().any(|&l| l >= 0) && input_signal;
            if ok {
                input_mod = Some(i);
            }
        }
        let mut saved: Option<([Vec<f32>; 2], [i32; 2])> = None;
        if let Some(i) = input_mod {
            let (outer, sn) = (&net.mods[n], &mut s.net);
            let im = &mut sn.mods[i];
            im.realtime_flags |= RT_DONT_CLEAN_INPUT;
            let mut chin: [Vec<f32>; 2] = [Vec::new(), Vec::new()];
            let in_empty = im.in_empty;
            for ch in 0..im.input_channels.clamp(0, 2) as usize {
                let src = ch.min(outputs_num.max(1) - 1);
                let mut b = vec![0.0f32; sn_max(frames, &im.chin[ch])];
                b[..frames].copy_from_slice(&outer.chin[src][offset..offset + frames]);
                chin[ch] = std::mem::replace(&mut im.chin[ch], b);
                im.in_empty[ch] = (outer.in_empty[src] - offset as i32).max(0);
            }
            saved = Some((chin, in_empty));
        }
        s.render(frames, &mut [Vec::new(), Vec::new()]);
        let mut retval = 0;
        {
            let om = &s.net.mods[0];
            let empty = (0..outputs_num.min(2)).all(|ch| om.in_empty[ch] >= frames as i32);
            if !empty {
                let mm = &mut net.mods[n];
                let mut data: Option<usize> = None;
                for ch in 0..outputs_num.min(2) {
                    if (ch as i32) < om.output_channels {
                        data = Some(ch).filter(|&c| !om.chin[c].is_empty());
                    }
                    let out = &mut mm.chout[ch][offset..offset + frames];
                    match data {
                        Some(c) => out.copy_from_slice(&om.chin[c][..frames]),
                        None => out.fill(0.0),
                    }
                }
                retval = 1;
            }
            if !self.opt.no_evt_out && net.mods[n].realtime_flags & RT_MUTE == 0 {
                let evs: Vec<Event> = om.events.iter().map(|&k| s.net.events_heap[k]).collect();
                let links = net.mods[n].output_links.clone();
                for mut e in evs {
                    e.id = net::evt_id_inc(e.id, n as u32);
                    e.id = e.id.wrapping_add(331 << 16);
                    e.offset += offset as i32;
                    let is_note = e.cmd == Cmd::NoteOn || e.cmd == Cmd::SetFreq;
                    let pitch = if is_note { e.pitch() } else { 0 };
                    for &l in &links {
                        let Some(l) = usize::try_from(l).ok().filter(|&l| l < net.mods_num()) else { continue };
                        if net.mods[l].flags & FLAG_EXISTS == 0 {
                            continue;
                        }
                        if is_note {
                            let t = &net.mods[l];
                            e.set_pitch(pitch - t.finetune - t.relative_note * 256);
                        }
                        net.add_event(l, &e);
                    }
                }
            }
        }
        if let (Some(i), Some((chin, in_empty))) = (input_mod, saved) {
            let im = &mut s.net.mods[i];
            for (ch, b) in chin.into_iter().enumerate() {
                if !b.is_empty() {
                    im.chin[ch] = b;
                }
            }
            im.in_empty = in_empty;
            im.realtime_flags &= !RT_DONT_CLEAN_INPUT;
        }
        retval
    }
}

/// A buffer for the input module's channel: as long as its own.
fn sn_max(frames: usize, own: &[f32]) -> usize {
    frames.max(own.len())
}

impl Module for MetaModule {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 1024, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 1, 256, 1, 1, C_INPUT, -1, 1);
                net.register_ctl(n, 0, 4, 0, 1, C_PLAY, -1, 1);
                net.register_ctl(n, 1, 1000, 125, 1, C_BPM, 125, 1);
                net.set_ctl_flags(n, 3, 1);
                net.register_ctl(n, 1, 31, 6, 1, C_TPL, 6, 1);
                for i in 0..MAX_USER_CTLS {
                    net.register_ctl(n, 0, 256, 0, 0, USER_CTLS_OFFSET + i, 256, 2);
                }
                net.change_ctls_num(n, USER_CTLS_OFFSET + 6);
                for c in self.channels.iter_mut() {
                    *c = Chan { playing: false, id: !0 };
                }
                self.active_channels = 0;
                self.ps = Some(SubEngine::new(net, n, false));
                self.release_pattern = -1;
                self.opt = Opt::default();
                retval = 1;
            }
            Cmd::SetupFinished => {
                if let Some(file) = net.mods[n].chunk(0).map(|c| c.data.clone()) {
                    if let Some(ps) = self.ps.as_mut() {
                        ps.load_project(&file);
                    }
                    net.remove_chunk(n, 0);
                }
                net.chunk_autocreate(n, 1, MAX_USER_CTLS * 4, false);
                self.ctl_links = net.mods[n].chunk(1).unwrap().data.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                let created = net.chunk_autocreate(n, 2, 8, true);
                if created {
                    let o = self.opt;
                    let d = &mut net.mods[n].chunk_mut(2).unwrap().data;
                    d[0] = o.user_ctls_num;
                    d[1] = o.arpeggiator as u8;
                    d[2] = o.use_velocity as u8;
                    d[3] = o.no_evt_out as u8;
                    d[4..8].copy_from_slice(&o.flags.to_le_bytes());
                }
                let d = &net.mods[n].chunk(2).unwrap().data;
                self.opt = Opt { user_ctls_num: d[0], arpeggiator: d[1] != 0, use_velocity: d[2] != 0, no_evt_out: d[3] != 0, flags: u32::from_le_bytes([d[4], d[5], d[6], d[7]]) };
                net.change_ctls_num(n, USER_CTLS_OFFSET + self.opt.user_ctls_num as usize);
                self.unpack_user_ctls(n, net);
                self.get_flags(n, net);
                self.check_auto_bpm_tpl(n, net, 0, 0);
                let play = net.mods[n].ctl(C_PLAY);
                self.handle_ctl_play(play, play);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let (ctl_play, ctl_input) = (net.mods[n].ctl(C_PLAY), net.mods[n].ctl(C_INPUT));
                let base = net.base_host_version;
                let opt = self.opt;
                if self.ps.is_some() {
                    if ctl_play != 0 {
                        let mut c = 0;
                        if self.active_channels > 0 {
                            c = self.channels.iter().position(|ch| ch.playing && ch.id == e.id).unwrap_or(MAX_CHANNELS);
                            if c >= MAX_CHANNELS {
                                if let Some(f) = self.channels.iter().position(|ch| !ch.playing) {
                                    c = f;
                                    self.channels[c] = Chan { playing: true, id: e.id };
                                    self.active_channels += 1;
                                }
                            }
                        } else {
                            self.channels[0] = Chan { playing: true, id: e.id };
                            self.active_channels += 1;
                            self.ps.as_mut().unwrap().s.flags &= !engine::FLAG_IGNORE_EFF31;
                        }
                        if c < MAX_CHANNELS {
                            self.ps.as_mut().unwrap().s.user_commands.push_back(cmd(engine::NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP, 0, 0));
                            self.handle_ctl_play_simple(ctl_play);
                            let ctl = if base >= 0x01090300 { 0x001D } else { 0 };
                            let s = &mut self.ps.as_mut().unwrap().s;
                            s.user_commands.push_back(cmd(engine::NOTECMD_PLAY, ctl, 0));
                            s.pitch_offset = if opt.arpeggiator { e.pitch() - (net::NOTE0_PITCH - 5 * 12 * 256) } else { 0 };
                            s.velocity = if opt.use_velocity { e.velocity() as i32 } else { 256 };
                        }
                    } else {
                        self.ps.as_mut().unwrap().s.velocity = 256;
                    }
                    self.to_input(net, ctl_input, e, true);
                }
                retval = 1;
            }
            Cmd::SetFreq => {
                let (ctl_play, ctl_input) = (net.mods[n].ctl(C_PLAY), net.mods[n].ctl(C_INPUT));
                if let Some(ps) = self.ps.as_mut() {
                    if ctl_play != 0 && self.active_channels > 0 {
                        ps.s.pitch_offset = if self.opt.arpeggiator { e.pitch() - (net::NOTE0_PITCH - 5 * 12 * 256) } else { 0 };
                    }
                    self.to_input(net, ctl_input, e, true);
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                let (ctl_play, ctl_input) = (net.mods[n].ctl(C_PLAY), net.mods[n].ctl(C_INPUT));
                if let Some(ps) = self.ps.as_mut() {
                    if ctl_play != 0 && self.active_channels > 0 {
                        ps.s.velocity = if self.opt.use_velocity { e.velocity() as i32 } else { 256 };
                    }
                    self.to_input(net, ctl_input, e, false);
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                let (ctl_play, ctl_input) = (net.mods[n].ctl(C_PLAY), net.mods[n].ctl(C_INPUT));
                if self.ps.is_some() {
                    if ctl_play != 0 && self.active_channels > 0 {
                        if let Some(c) = self.channels.iter().position(|ch| ch.id == e.id) {
                            self.channels[c] = Chan { playing: false, id: !0 };
                            self.active_channels -= 1;
                        }
                        if self.active_channels == 0 {
                            let s = &mut self.ps.as_mut().unwrap().s;
                            if ctl_play < 3 {
                                s.user_commands.push_back(cmd(engine::NOTECMD_STOP, 0, 0));
                            } else {
                                if self.opt.flags & OPT_FLAG_IGNORE_EFF31_AFTER_LAST_NOTEOFF != 0 {
                                    s.flags |= engine::FLAG_IGNORE_EFF31;
                                }
                                if self.opt.flags & OPT_FLAG_JMP_TO_RL_PAT_AFTER_LAST_NOTEOFF != 0 {
                                    if self.release_pattern < 0 {
                                        self.release_pattern = s.pattern_num_by_name("RL");
                                    }
                                    let rp = self.release_pattern;
                                    if let Some(x) = usize::try_from(rp).ok().filter(|&p| s.pat(p).is_some()).map(|p| s.pats_info[p].x) {
                                        if x >= 0 {
                                            s.user_commands.push_back(cmd(engine::NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP, 0, x as u16));
                                        } else {
                                            s.user_commands.push_back(cmd(engine::NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP2, 0, (-x) as u16));
                                        }
                                        s.user_commands.push_back(cmd(engine::NOTECMD_PLAY, 0x001D, 0));
                                    }
                                }
                            }
                        }
                    }
                    self.to_input(net, ctl_input, e, false);
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                let ctl_input = net.mods[n].ctl(C_INPUT);
                if let Some(ps) = self.ps.as_mut() {
                    ps.s.user_commands.push_back(cmd(engine::NOTECMD_STOP, 0, 0));
                    self.to_input(net, ctl_input, e, false);
                }
                for c in self.channels.iter_mut() {
                    *c = Chan { playing: false, id: !0 };
                }
                self.active_channels = 0;
                retval = 1;
            }
            Cmd::Clean => {
                if let Some(ps) = self.ps.as_mut() {
                    ps.s.user_commands.push_back(cmd(engine::NOTECMD_STOP, 0, 0));
                    ps.s.user_commands.push_back(cmd(engine::NOTECMD_CLEAN_MODULES, 0, 0));
                }
                for c in self.channels.iter_mut() {
                    c.playing = false;
                }
                self.active_channels = 0;
                retval = 1;
            }
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                let (ctl_play, ctl_input) = (net.mods[n].ctl(C_PLAY), net.mods[n].ctl(C_INPUT));
                let base = net.base_host_version;
                if let Some(ps) = self.ps.as_mut() {
                    if ctl_play != 0 {
                        let s = &mut ps.s;
                        let line = if e.cmd == Cmd::SetSampleOffset { e.sample_offset() / 256 } else { ((e.sample_offset() & 32767).wrapping_mul(s.proj_lines as u32)) >> 15 };
                        s.user_commands.push_back(cmd(engine::NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP, 0, line as u16));
                        let ctl = if base >= 0x01090300 { 0x001D } else { 0 };
                        s.user_commands.push_back(cmd(engine::NOTECMD_PLAY, ctl, 0));
                    }
                    self.to_input(net, ctl_input, e, false);
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                let v = e.ctl_val() as i32;
                let o = &mut self.opt;
                let mut flags_changed = false;
                retval = 1;
                match e.ctl_num() as i32 + 1 {
                    127 => o.arpeggiator = v != 0,
                    126 => o.use_velocity = v != 0,
                    125 => o.no_evt_out = v == 0,
                    124 => {
                        o.flags &= !(OPT_FLAG_GEN | OPT_FLAG_NOTGEN);
                        if v != 0 {
                            o.flags |= OPT_FLAG_GEN;
                        }
                        flags_changed = true;
                    }
                    123 => {
                        o.flags &= !(OPT_FLAG_GEN | OPT_FLAG_NOTGEN);
                        if v != 0 {
                            o.flags |= OPT_FLAG_NOTGEN;
                        }
                        flags_changed = true;
                    }
                    122 => {
                        o.flags &= !OPT_FLAG_AUTO_BPM_TPL;
                        if v != 0 {
                            o.flags |= OPT_FLAG_AUTO_BPM_TPL;
                        }
                        self.get_flags(n, net);
                        self.check_auto_bpm_tpl(n, net, 0, 0);
                    }
                    121 => {
                        o.flags &= !OPT_FLAG_IGNORE_EFF31_AFTER_LAST_NOTEOFF;
                        if v != 0 {
                            o.flags |= OPT_FLAG_IGNORE_EFF31_AFTER_LAST_NOTEOFF;
                        }
                    }
                    120 => {
                        o.flags &= !OPT_FLAG_JMP_TO_RL_PAT_AFTER_LAST_NOTEOFF;
                        if v != 0 {
                            o.flags |= OPT_FLAG_JMP_TO_RL_PAT_AFTER_LAST_NOTEOFF;
                        }
                    }
                    _ => retval = 0,
                }
                if flags_changed {
                    self.get_flags(n, net);
                }
                if retval != 0 {
                    return retval;
                }
                if self.ps.is_none() {
                    return 0;
                }
                let ctl_num = e.ctl_num() as usize;
                match ctl_num {
                    1 => {
                        Self::set_var(net, n, C_INPUT, v);
                        self.get_flags(n, net);
                    }
                    2 => {
                        let prev = net.mods[n].ctl(C_PLAY);
                        Self::set_var(net, n, C_PLAY, v);
                        self.get_flags(n, net);
                        if net.base_host_version >= 0x02010200 {
                            self.handle_ctl_play(v, prev);
                        }
                    }
                    3 => {
                        let val = v & 65535;
                        if val != 0 {
                            self.ps.as_mut().unwrap().s.bpm = val as u16;
                        }
                    }
                    4 => {
                        let val = v & 255;
                        if val != 0 {
                            self.ps.as_mut().unwrap().s.speed = val as u8;
                        }
                    }
                    _ => {}
                }
                if ctl_num >= USER_CTLS_OFFSET {
                    let i = ctl_num - USER_CTLS_OFFSET;
                    let link = self.ctl_links.get(i).copied().unwrap_or(0);
                    let link_mod = (link & 0xFFFF) as usize;
                    if link_mod != 0 {
                        let mut ev = *e;
                        ev.set_ctl_num(((link >> 16) & 0xFFFF) as u16);
                        let s = &mut self.ps.as_mut().unwrap().s;
                        if link_mod < s.net.mods_num() && s.net.mods[link_mod].flags & FLAG_EXISTS != 0 {
                            s.psynth_events.push((link_mod as u16, ev));
                        }
                    }
                }
            }
            Cmd::SpeedChanged => {
                self.check_auto_bpm_tpl(n, net, e.bpm() as i32, e.tpl() as i32);
                retval = 1;
            }
            Cmd::Close => {
                self.ps = None;
                retval = 1;
            }
            _ => {}
        }
        retval
    }
}
