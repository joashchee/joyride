// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! psynth_sunvox: an engine run inside a module, a MetaModule's project
//! or a Sampler's effect (one copy of a module file a voice, each run by
//! hand on the voice's sound).

use super::engine::{self, Engine};
use super::file;
use super::net::{Cmd, Event, Net, Smp, FLAG_EXISTS, MAX_CHANNELS};

pub(crate) struct SubEngine {
    pub s: Box<Engine>,
    temp_bufs: [Vec<Smp>; MAX_CHANNELS],
}

/// The flags psynth_sunvox_new gives every engine it makes.
const FLAGS: u32 = engine::FLAG_PLAYER_ONLY
    | engine::FLAG_NO_GUI
    | engine::FLAG_NO_SCOPE
    | engine::FLAG_NO_MIDI
    | engine::FLAG_NO_GLOBAL_SYS_EVENTS
    | engine::FLAG_NO_KBD_EVENTS
    | engine::FLAG_DYNAMIC_PATTERN_STATE;

impl SubEngine {
    /// psynth_sunvox_new with one engine; `no_module_channels` for modules
    /// rendered by hand.
    pub fn new(net: &Net, _parent: usize, no_module_channels: bool) -> SubEngine {
        let flags = FLAGS | if no_module_channels { engine::FLAG_NO_MODULE_CHANNELS } else { 0 };
        let s = Box::new(Engine::new(flags, net.sampling_freq, net.globals.clone()));
        let temp_bufs = std::array::from_fn(|_| if no_module_channels { vec![0.0; s.net.max_buf_size as usize] } else { Vec::new() });
        SubEngine { s, temp_bufs }
    }

    /// sunvox_load_proj_from_fd (made static), the engine made again first.
    pub fn load_project(&mut self, file: &[u8]) -> bool {
        if !file::is_sunvox(file) {
            return false;
        }
        let flags = (self.s.flags & !(engine::FLAG_CREATE_PATTERN | engine::FLAG_CREATE_MODULES)) | engine::FLAG_DONT_START;
        let globals = self.s.net.globals.clone();
        let freq = self.s.freq;
        self.s = Box::new(Engine::new(flags, freq, globals));
        let ok = !matches!(file::load(file, &mut self.s), Err(file::LoadError::NotSunVox));
        self.s.flags &= !engine::FLAG_STATIC_TIMELINE;
        self.s.sort_patterns();
        self.s.flags |= engine::FLAG_STATIC_TIMELINE;
        ok
    }

    /// psynth_sunvox_set_module: the network cleared, then the module file
    /// loaded `count` times (modules 1, 2, 3...).
    pub fn set_module(&mut self, file: &[u8], count: usize) -> bool {
        for i in 1..self.s.net.mods_num() {
            self.s.net.remove_module(i);
        }
        for _ in 0..count {
            file::load_module(file, &mut self.s);
        }
        true
    }

    fn exists(&self, m: usize) -> bool {
        m < self.s.net.mods_num() && self.s.net.mods[m].flags & FLAG_EXISTS != 0
    }

    /// psynth_sunvox_handle_module_event, a command.
    pub fn module_command(&mut self, m: usize, cmd: Cmd) {
        if self.exists(m) {
            self.s.net.do_command(m, cmd);
        }
    }

    /// The controller's type, min and max (of psynth_sunvox_get_module's module 1).
    pub fn module_ctl(&self, m: usize, ctl: usize) -> Option<(u8, i32, i32)> {
        if !self.exists(m) {
            return None;
        }
        self.s.net.mods[m].ctls.get(ctl).map(|c| (c.typ, c.min, c.max))
    }

    /// psynth_sunvox_handle_module_event, a controller set.
    pub fn module_ctl_event(&mut self, m: usize, ctl: usize, val: i32) {
        if self.exists(m) {
            let mut e = Event::new(Cmd::SetGlobalController);
            e.set_ctl_num(ctl as u16);
            e.set_ctl_val(val as u16);
            self.s.net.handle_event(m, &mut e);
        }
    }

    /// psynth_sunvox_apply_module: module `m` run on `frames` frames of the
    /// buffers from `at`, its output put back.
    pub fn apply_module(&mut self, m: usize, bufs: &mut [Vec<f32>; 2], at: usize, frames: usize) {
        if !self.exists(m) {
            return;
        }
        let net = &mut self.s.net;
        net.buf_size = frames as i32;
        {
            let md = &mut net.mods[m];
            for ch in 0..MAX_CHANNELS {
                let src = ch.min(bufs.len() - 1);
                md.chin[ch] = bufs[src][at..at + frames].to_vec();
                md.in_empty[ch] = 0;
                md.chout[ch] = std::mem::take(&mut self.temp_bufs[ch]);
                md.out_empty[ch] = 0;
            }
        }
        let mut e = Event::new(Cmd::RenderReplace);
        net.handle_event(m, &mut e);
        let md = &mut net.mods[m];
        let mut prev = bufs.len() - 1;
        for ch in 0..bufs.len() {
            if (ch as i32) < md.output_channels {
                prev = ch;
            }
            bufs[ch][at..at + frames].copy_from_slice(&md.chout[prev][..frames]);
        }
        for ch in 0..MAX_CHANNELS {
            self.temp_bufs[ch] = std::mem::take(&mut md.chout[ch]);
            md.chin[ch] = Vec::new();
        }
    }
}
