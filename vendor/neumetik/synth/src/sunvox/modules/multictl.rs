// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MultiCtl (psynths_multictl.cpp): one value sent to up to 16
//! controllers of the modules it links to, each through a curve, gain,
//! quantization, its own range and an offset; eased at a rate of its own
//! when its response is under the maximum.

use super::super::net::{self, Chunk, Cmd, Event, Module, Net, FLAG_EXISTS, RT_MUTE};

const SLOTS: usize = 16;
const SLOT_SIZE: usize = 32;
const CURVE_POINTS: usize = 257;
const MAX_RESPONSE: i32 = 1000;

const C_VAL: usize = 0;
const C_GAIN: usize = 1;
const C_QUANT: usize = 2;
const C_OFFSET: usize = 3;
const C_RESPONSE: usize = 4;
const C_FREQ: usize = 5;

#[derive(Clone, Copy)]
struct Slot {
    min: i32,
    max: i32,
    ctl: i32,
    flags: u32,
}

fn rd(d: &[u8], at: usize) -> i32 {
    d.get(at..at + 4).map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn slots(m: &net::ModCommon) -> Vec<Slot> {
    let d = m.chunk(0).map(|c| c.data.as_slice()).unwrap_or(&[]);
    (0..SLOTS).map(|i| Slot { min: rd(d, i * SLOT_SIZE), max: rd(d, i * SLOT_SIZE + 4), ctl: rd(d, i * SLOT_SIZE + 8), flags: rd(d, i * SLOT_SIZE + 12) as u32 }).collect()
}

fn curve(m: &net::ModCommon) -> Option<Vec<i32>> {
    let c = m.chunk(1)?;
    Some((0..CURVE_POINTS).map(|i| c.data.get(i * 2..i * 2 + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32)).collect())
}

fn curve_val(input: i32, curve: &[i32]) -> i32 {
    let c1 = input & 127;
    let c2 = 128 - c1;
    if input < 32768 {
        let i = (input >> 7) as usize;
        (curve[i] * c2 + curve[i + 1] * c1) >> 7
    } else {
        curve[256]
    }
}

#[derive(Default)]
pub(crate) struct MultiCtl {
    floating_val: f32,
    tick_counter: i32,
    tick_size: i32,
    pars_recalc_request: i32,
}

impl MultiCtl {
    fn get_val(mut val: i32, curve: Option<&[i32]>, slot: &Slot, m: &net::ModCommon) -> i32 {
        if let Some(c) = curve {
            val = if slot.min < slot.max { curve_val(val, c) } else { 32768 - curve_val(32768 - val, c) };
        }
        val = (val * m.ctl(C_GAIN)) / 256;
        if val > 32768 {
            val = 32768;
        }
        let range = slot.max - slot.min;
        let ctl_quant = m.ctl(C_QUANT);
        if ctl_quant < 32768 {
            let quant = (ctl_quant - 1).max(1);
            let step_n = val * quant / 32768;
            let val2 = step_n * 32768 / quant;
            val = slot.min + (range * val2) / 32768;
        } else {
            val = slot.min + (range * val) / 32768;
        }
        val += m.ctl(C_OFFSET) - 16384;
        val.clamp(0, 32768)
    }

    fn calc_pars(&mut self, m: &net::ModCommon, srate: i32) {
        self.tick_size = 1;
        let f = m.ctl(C_FREQ);
        if f > 0 {
            self.tick_size = (srate * 256) / f;
        }
        if self.tick_size <= 0 {
            self.tick_size = 1;
        }
    }

    /// multictl_send: the value to each slot's controller (FOR_EACH_SLOT).
    fn send(n: usize, net: &mut Net, val: i32, offset: i32) {
        let m = &net.mods[n];
        let curve = curve(m);
        let slots = slots(m);
        let links = m.output_links.clone();
        let mut evs = Vec::new();
        let mut link_n = 0usize;
        for slot in &slots {
            if slot.flags & 1 != 0 && link_n != 0 {
                link_n -= 1;
            }
            if link_n >= links.len() {
                break;
            }
            let l = links[link_n];
            link_n += 1;
            let Some(l) = usize::try_from(l).ok().filter(|&l| l < net.mods_num() && net.mods[l].flags & FLAG_EXISTS != 0) else { continue };
            if slot.ctl != 0 {
                let mut e = Event::new(Cmd::SetGlobalController);
                e.offset = offset;
                e.set_ctl_num((slot.ctl - 1) as u16);
                e.set_ctl_val(Self::get_val(val, curve.as_deref(), slot, m) as u16);
                evs.push((l, e));
            }
        }
        for (l, e) in evs {
            net.add_event(l, &e);
        }
    }
}

impl Module for MultiCtl {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 32768, 0, 0, C_VAL, -1, 0);
                net.register_ctl(n, 0, 1024, 256, 0, C_GAIN, 256, 0);
                net.register_ctl(n, 0, 32768, 32768, 0, C_QUANT, -1, 0);
                net.register_ctl(n, 0, 32768, 16384, 0, C_OFFSET, 16384, 0);
                net.set_ctl_show_offset(n, 3, -16384);
                net.register_ctl(n, 0, MAX_RESPONSE, MAX_RESPONSE, 0, C_RESPONSE, -1, 1);
                net.register_ctl(n, 1, 32768, 150, 0, C_FREQ, 50, 1);
                net.set_ctl_flags(n, 5, 2);
                self.floating_val = net.mods[n].ctl(C_VAL) as f32;
                self.tick_counter = 0;
                self.pars_recalc_request = 0;
                self.calc_pars(&net.mods[n], net.sampling_freq);
                retval = 1;
            }
            Cmd::SetupFinished => {
                if net.mods[n].chunk(0).is_none() {
                    let mut d = vec![0u8; SLOTS * SLOT_SIZE];
                    for i in 0..SLOTS {
                        d[i * SLOT_SIZE + 4..i * SLOT_SIZE + 8].copy_from_slice(&32768i32.to_le_bytes());
                    }
                    net.new_chunk(n, 0, Chunk { data: d, flags: 0, freq: 0 });
                }
                if net.mods[n].chunk(1).is_none() {
                    let mut d = vec![0u8; CURVE_POINTS * 2];
                    for i in 0..CURVE_POINTS {
                        d[i * 2..i * 2 + 2].copy_from_slice(&((i * 128) as u16).to_le_bytes());
                    }
                    net.new_chunk(n, 1, Chunk { data: d, flags: 0, freq: 0 });
                }
                self.floating_val = net.mods[n].ctl(C_VAL) as f32;
                self.calc_pars(&net.mods[n], net.sampling_freq);
                retval = 1;
            }
            Cmd::Clean => {
                self.floating_val = net.mods[n].ctl(C_VAL) as f32;
                self.tick_counter = 0;
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if net.mods[n].realtime_flags & RT_MUTE != 0 {
                    return 0;
                }
                if e.ctl_num() < 4 {
                    retval = 1;
                    net::set_ctl(&mut net.mods[n], e);
                    if net.mods[n].ctl(C_RESPONSE) >= MAX_RESPONSE {
                        let v = net.mods[n].ctl(C_VAL);
                        self.floating_val = v as f32;
                        Self::send(n, net, v, e.offset);
                    }
                } else if e.ctl_num() == 5 {
                    self.pars_recalc_request += 1;
                }
            }
            Cmd::RenderReplace => {
                if self.pars_recalc_request != 0 {
                    self.pars_recalc_request = 0;
                    self.calc_pars(&net.mods[n], net.sampling_freq);
                }
                let m = &net.mods[n];
                if m.realtime_flags & RT_MUTE != 0 || m.ctl(C_RESPONSE) >= MAX_RESPONSE {
                    return 0;
                }
                let (offset, frames) = (m.offset, m.frames);
                let r = m.ctl(C_RESPONSE).max(1);
                let mut response = r as f32 / MAX_RESPONSE as f32;
                response *= response;
                let mut ptr = 0;
                loop {
                    let mut buf_size = frames - ptr;
                    if buf_size > (self.tick_size - self.tick_counter) / 256 {
                        buf_size = (self.tick_size - self.tick_counter) / 256;
                    }
                    if (self.tick_size - self.tick_counter) & 255 != 0 {
                        buf_size += 1;
                    }
                    if buf_size > frames - ptr {
                        buf_size = frames - ptr;
                    }
                    if buf_size < 0 {
                        buf_size = 0;
                    }
                    ptr += buf_size;
                    self.tick_counter += buf_size * 256;
                    if self.tick_counter >= self.tick_size {
                        self.tick_counter %= self.tick_size;
                        let prev_val = self.floating_val as i32;
                        let ctl_val = net.mods[n].ctl(C_VAL);
                        self.floating_val = (1.0 - response) * self.floating_val + response * ctl_val as f32;
                        if prev_val != self.floating_val as i32 {
                            let offset2 = if ptr > 0 { offset + ptr - 1 } else { offset };
                            Self::send(n, net, self.floating_val as i32, offset2);
                        }
                    }
                    if ptr >= frames {
                        break;
                    }
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
