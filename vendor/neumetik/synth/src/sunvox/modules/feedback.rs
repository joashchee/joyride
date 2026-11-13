// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Feedback (psynths_feedback.cpp): a pair that lets a loop be made. The
//! first records its input into a ring buffer; the second (linked from
//! it) plays it back a block (20 ms) later.

use super::super::net::{Cmd, Event, Module, Net, Smp};
use super::Kind;

const OUTPUTS: usize = 2;

const C_VOLUME: usize = 0;
const C_MONO: usize = 1;

#[derive(Default)]
pub(crate) struct Feedback {
    buf: Vec<Smp>,
    buf_size: u32,
    buf_wp: u32,
    buf_wp_start: u32,
    buf_clean: bool,
}

impl Feedback {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mut receiver = None;
        for &l in &net.mods[n].input_links {
            if (l as u32) < net.mods_num() as u32 {
                if net.mods[l as usize].kind == Some(Kind::Feedback) {
                    receiver = Some(l as usize);
                    break;
                }
            }
        }
        let mut mono = net.mods[n].ctl(C_MONO) != 0;
        let mut vol = net.mods[n].ctl(C_VOLUME);
        if let Some(r) = receiver {
            if net.mods[r].ctl(C_MONO) != 0 {
                mono = true;
            }
            vol = (vol * net.mods[r].ctl(C_VOLUME)) / 10000;
        }
        let want = if mono { 1 } else { OUTPUTS as i32 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
            net.set_number_of_inputs(want, n);
        }
        let max_buf = net.max_buf_size as u32;
        let (offset, frames) = (net.mods[n].offset as usize, net.mods[n].frames as usize);
        let mut retval = 0;
        if let Some(r) = receiver {
            let outputs_num = net.mods[r].output_channels.clamp(0, 2) as usize;
            let Some(h) = net.handlers.get(r).and_then(|h| h.as_ref()) else { return 0 };
            let Some((buf, size, wp_start)) = h.feedback_buf() else { return 0 };
            let mut outs: Vec<Vec<Smp>> = Vec::new();
            for ch in 0..outputs_num {
                let b = &buf[(size * ch as u32) as usize..];
                let mut rp = wp_start.wrapping_add(offset as u32).wrapping_sub(max_buf) & (size - 1);
                let mut out = vec![0.0; frames];
                if vol != 0 {
                    if vol == 10000 {
                        for o in out.iter_mut() {
                            *o = b[rp as usize];
                            rp = (rp + 1) & (size - 1);
                        }
                    } else {
                        let fvol = vol as f32 / 10000.0;
                        for o in out.iter_mut() {
                            *o = b[rp as usize] * fvol;
                            rp = (rp + 1) & (size - 1);
                        }
                    }
                    retval = 3;
                }
                outs.push(out);
            }
            if vol != 0 {
                let m = &mut net.mods[n];
                for (ch, o) in outs.iter().enumerate() {
                    if let Some(dst) = m.chout.get_mut(ch).filter(|d| !d.is_empty()) {
                        dst[offset..offset + frames].copy_from_slice(o);
                    }
                }
            }
        } else {
            let m = &net.mods[n];
            let inputs_num = m.input_channels.clamp(0, 2) as usize;
            let mut wp = self.buf_wp;
            for ch in 0..inputs_num {
                let input_signal = m.in_empty[ch] < (offset + frames) as i32;
                if input_signal {
                    self.buf_clean = false;
                }
                wp = self.buf_wp;
                if !input_signal && self.buf_clean {
                    wp = (wp + frames as u32) & (self.buf_size - 1);
                } else {
                    let base = (self.buf_size * ch as u32) as usize;
                    for &x in &m.chin[ch][offset..offset + frames] {
                        self.buf[base + wp as usize] = x;
                        wp = (wp + 1) & (self.buf_size - 1);
                    }
                }
            }
            self.buf_wp = wp;
        }
        retval
    }
}

impl Module for Feedback {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 10000, 1000, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 0);
                self.buf_size = ((net.max_buf_size * 2) as u32).next_power_of_two();
                self.buf = vec![0.0; (self.buf_size as usize) * OUTPUTS];
                self.buf_wp = 0;
                self.buf_wp_start = 0;
                self.buf_clean = true;
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_clean {
                    self.buf.fill(0.0);
                    self.buf_clean = true;
                }
                retval = 1;
            }
            Cmd::RenderSetup => {
                self.buf_wp_start = self.buf_wp;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }

    fn feedback_buf(&self) -> Option<(&[Smp], u32, u32)> {
        Some((&self.buf, self.buf_size, self.buf_wp_start))
    }
}
