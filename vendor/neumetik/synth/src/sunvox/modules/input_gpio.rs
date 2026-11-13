// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Input and GPIO (psynths_input.cpp, psynths_gpio.cpp): the audio
//! input and Linux's GPIO pins. A song played here has neither (as the
//! library rendering a song has no input buffer, and reads GPIO pins
//! only on Linux): they keep their controllers and make no sound.

use super::super::net::{Cmd, Event, Module, Net};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Which {
    Input,
    Gpio,
}

pub(crate) struct Silent(pub Which);

impl Module for Silent {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        match e.cmd {
            Cmd::Init => {
                match self.0 {
                    Which::Input => {
                        net.register_ctl(n, 0, 1024, 256, 0, 0, 256, 0);
                        net.register_ctl(n, 0, 1, 0, 1, 1, -1, 0);
                    }
                    Which::Gpio => {
                        net.register_ctl(n, 0, 1, 0, 1, 0, -1, 0);
                        net.register_ctl(n, 0, 256, 0, 1, 1, -1, 0);
                        net.register_ctl(n, 0, 100, 50, 0, 2, -1, 0);
                        net.register_ctl(n, 0, 1, 0, 1, 3, -1, 1);
                        net.register_ctl(n, 0, 256, 0, 1, 4, -1, 1);
                        net.register_ctl(n, 0, 128, 0, 1, 5, -1, 1);
                        net.register_ctl(n, 0, 100, 100, 0, 6, -1, 1);
                    }
                }
                1
            }
            Cmd::Clean | Cmd::Close => 1,
            _ => 0,
        }
    }
}
