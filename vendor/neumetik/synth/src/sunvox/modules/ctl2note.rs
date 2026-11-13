// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Ctl2Note (psynths_ctl2note.cpp): its Pitch controller turned into
//! notes (on, changes of pitch or velocity, off) to the modules it links to.

use super::super::net::{self, Cmd, Event, Module, Net, RT_MUTE};

const C_PITCH: usize = 0;
const C_NOTE: usize = 1;
const C_NOTES: usize = 2;
const C_TRANSPOSE: usize = 3;
const C_FINETUNE: usize = 4;
const C_VEL: usize = 5;
const C_STATE: usize = 6;
const C_NOTEON: usize = 7;
const C_NOTEOFF: usize = 8;
const C_REC: usize = 9;

/// SUNVOX_VIRTUAL_PATTERN_CTL2NOTE.
const VIRTUAL_PATTERN_CTL2NOTE: u32 = 0xFFFF - 2;

#[derive(Clone, Copy, PartialEq)]
enum Send {
    On,
    Off,
    Change,
    Vel,
}

#[derive(Default)]
pub(crate) struct Ctl2Note {
    /// out_evt: kept between sends (only what each send sets changes).
    out: Option<Event>,
}

fn set_var(net: &mut Net, n: usize, ctl: usize, v: i32) {
    let m = &mut net.mods[n];
    if let Some(var) = m.ctls.get(ctl).and_then(|c| c.var) {
        m.vars[var] = v;
    }
}

impl Ctl2Note {
    fn send(&mut self, n: usize, net: &mut Net, offset: i32, cmd: Send) {
        let mut e = self.out.unwrap_or_else(|| Event::new(Cmd::NoteOn));
        e.offset = offset;
        e.id = (VIRTUAL_PATTERN_CTL2NOTE << 16) | n as u32;
        e.set_velocity(net.mods[n].ctl(C_VEL));
        match cmd {
            Send::On => {
                set_var(net, n, C_STATE, 1);
                e.cmd = Cmd::NoteOn;
            }
            Send::Off => {
                set_var(net, n, C_STATE, 0);
                e.cmd = Cmd::NoteOff;
            }
            Send::Change => e.cmd = Cmd::SetFreq,
            Send::Vel => e.cmd = Cmd::SetVelocity,
        }
        let m = &net.mods[n];
        let final_pitch = m.ctl(C_PITCH) * m.ctl(C_NOTES) / (32768 / 256) + (m.ctl(C_NOTE) + (m.ctl(C_TRANSPOSE) - 128) + m.relative_note) * 256 + (m.ctl(C_FINETUNE) - 256) + m.finetune;
        let pitch = net::NOTE0_PITCH - final_pitch;
        if cmd != Send::Vel {
            net.multisend_pitch(n, &mut e, pitch);
        } else {
            net.multisend(n, &e);
        }
        self.out = Some(e);
    }
}

impl Module for Ctl2Note {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 32768, 0, 0, C_PITCH, -1, 0);
                net.register_ctl(n, 0, 120, 0, 1, C_NOTE, -1, 0);
                net.register_ctl(n, 0, 120, 120, 1, C_NOTES, -1, 0);
                net.register_ctl(n, 0, 256, 128, 1, C_TRANSPOSE, 128, 1);
                net.set_ctl_show_offset(n, 3, -128);
                net.register_ctl(n, 0, 512, 256, 0, C_FINETUNE, 256, 1);
                net.set_ctl_show_offset(n, 4, -256);
                net.register_ctl(n, 0, 256, 256, 0, C_VEL, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_STATE, -1, 2);
                net.register_ctl(n, 0, 1, 1, 1, C_NOTEON, -1, 3);
                net.register_ctl(n, 0, 2, 1, 1, C_NOTEOFF, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_REC, -1, 4);
                self.out = None;
                retval = 1;
            }
            Cmd::SetupFinished | Cmd::Clean => {
                set_var(net, n, C_STATE, 0);
                retval = 1;
            }
            Cmd::AllNotesOff | Cmd::Stop | Cmd::Muted => {
                if net.mods[n].ctl(C_STATE) != 0 {
                    self.send(n, net, e.offset, Send::Off);
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                let prev_state = net.mods[n].ctl(C_STATE);
                net::set_ctl(&mut net.mods[n], e);
                retval = 1;
                if net.mods[n].realtime_flags & RT_MUTE != 0 {
                    return retval;
                }
                let m = &net.mods[n];
                let mut cmd = None;
                match e.ctl_num() {
                    0..=4 => {
                        if m.ctl(C_NOTEON) == 1 {
                            cmd = Some(if m.ctl(C_STATE) != 0 { Send::Change } else { Send::On });
                        }
                        if m.ctl(C_NOTEOFF) == 1 && m.ctl(C_PITCH) == 0 {
                            cmd = Some(Send::Off);
                        }
                        if m.ctl(C_NOTEOFF) == 2 && m.ctl(C_PITCH) == 32768 {
                            cmd = Some(Send::Off);
                        }
                    }
                    5 => cmd = Some(Send::Vel),
                    6 => {
                        if prev_state != m.ctl(C_STATE) {
                            cmd = Some(if m.ctl(C_STATE) != 0 { Send::On } else { Send::Off });
                        }
                    }
                    _ => {}
                }
                if let Some(cmd) = cmd {
                    self.send(n, net, e.offset, cmd);
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
