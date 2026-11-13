// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// A port of FlMML on HTML5's player (src/flmml, github.com/argentum384/flmml-on-html5),
// its FM voice played on our own YM2151 (ymfm.rs) in place of its fmgen port:
//
// Copyright (c) 2015, argentum384
// Copyright (c) 2007, Takeshi OKUBO
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of flmml-on-html5 nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! **FlMML**: songs in FlMML's MML played as FlMML plays them, by a port
//! of its compiler and synthesizer (FlMML on HTML5): sine, saw, triangle,
//! pulse (with PWM and noise mixed in), noise, the Famicom's pulse,
//! triangle, noises and DPCM, the Game Boy's wave and noises, wave
//! tables (`#WAV9`, `#WAV10`, `#WAV13`), two envelopes, low- and
//! high-pass filters, formants, LFOs on pitch, amplitude, cutoff, pulse
//! width, FM and pan, portamento, pipes, ring modulation and sync between
//! tracks, and polyphony (`#USING POLY`). Its FM tone (`@14`, `#OPM@`,
//! `#OPN@`) plays on Neumetik's own YM2151 rather than the fmgen port
//! FlMML uses, so it's close to FlMML's, not the same.
//!
//! FlMML renders at 44.1 kHz; [`render`] does too. FlMML's white noise
//! is made from `Math.random()`; here from a fixed seed, so a song sounds
//! the same each time.

// The port keeps FlMML's loops and arithmetic as they are written, so
// it stays sample for sample the same.
#![allow(clippy::needless_range_loop, clippy::neg_multiply, clippy::collapsible_match, clippy::while_let_loop)]

mod channel;
mod compile;
mod osc;

use channel::{Chan, Channel, Shared};
use compile::Ev;
use osc::{Fixed, PHASE_MSK};
use std::rc::Rc;

pub use compile::parse_int;

/// FlMML's sample rate.
pub const RATE: u32 = 44_100;

/// A DPCM wave (`#WAV9`): its bits, start level, looping, length in bits.
#[derive(Clone, Default)]
pub struct Dpcm {
    table: Vec<u32>,
    int_vol: i32,
    looped: bool,
    length: i32,
}

/// The tones a song defines: Game Boy waves, wave tables, DPCM, FM
/// voices, and the FM gain.
#[derive(Clone)]
pub struct Tables {
    gb_wave: Vec<Option<Vec<f64>>>,
    wave: Vec<Option<(Vec<f64>, f64)>>,
    dpcm: Vec<Dpcm>,
    dpcm_set: Vec<bool>,
    opm: Vec<Option<Vec<i32>>>,
    fm_gain: f64,
}

/// FlMML's FM voice 0 when a song defines none (MOscOPM's defTimbre).
const DEF_TIMBRE: [i32; 55] = [
    4, 5, 31, 5, 0, 0, 0, 23, 1, 1, 3, 0, 0, 20, 10, 3, 7, 8, 0, 1, 1, 3, 0, 0, 31, 3, 0, 0, 0, 25, 1, 1, 7, 0, 0, 31, 12, 3, 7, 10, 2, 1, 1, 7, 0, 0, 15, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn nibble(c: char) -> i32 {
    match c {
        '0'..='9' => c as i32 - 48,
        'a'..='f' => c as i32 - 97 + 10,
        _ => 0,
    }
}

impl Tables {
    pub fn new() -> Tables {
        let mut t = Tables { gb_wave: vec![None; 32], wave: vec![None; 32], dpcm: vec![Dpcm::default(); 16], dpcm_set: vec![false; 16], opm: vec![None; 128], fm_gain: 14.25 };
        t.set_gb_wave(0, "0123456789abcdeffedcba9876543210");
        // (FlMML's own default, upper case, which its parser reads as zeros.)
        t.set_wave(0, "00112233445566778899AABBCCDDEEFFFFEEDDCCBBAA99887766554433221100");
        t.set_dpcm(0, 127, false, "");
        t.opm[0] = Some(DEF_TIMBRE.to_vec());
        t
    }
    pub fn set_gb_wave(&mut self, n: usize, w: &str) {
        let chars: Vec<char> = w.chars().collect();
        self.gb_wave[n] = Some((0..32).map(|i| (f64::from(nibble(chars.get(i).copied().unwrap_or('0'))) - 7.5) / 7.5).collect());
    }
    pub fn set_wave(&mut self, n: usize, w: &str) {
        let chars: Vec<char> = w.chars().collect();
        let mut table = vec![0.0; (chars.len() / 2).max(1)];
        let mut count = 0usize;
        let mut val = 0;
        for (j, &c) in chars.iter().enumerate().take(2048) {
            let code = nibble(c);
            if j & 1 == 1 {
                val += code;
                if count < table.len() {
                    table[count] = (f64::from(val) - 127.5) / 127.5;
                }
                count += 1;
            } else {
                val = code << 4;
            }
        }
        let count = count.max(1);
        self.wave[n] = Some((table, (PHASE_MSK + 1) as f64 / count as f64));
    }
    pub fn set_dpcm(&mut self, n: usize, int_vol: i32, looped: bool, wave: &str) {
        const MAX_LEN: i32 = 0xff1;
        let table_len = ((MAX_LEN >> 2) + 2) as usize;
        let mut table = vec![0u32; table_len];
        let (mut length, mut int_cnt, mut int_cn2, mut int_pos) = (0i32, 0u32, 0u32, 0usize);
        for c in wave.chars() {
            let code: u32 = match c {
                'A'..='Z' => c as u32 - 0x41,
                'a'..='z' => c as u32 - 0x61 + 26,
                '0'..='9' => c as u32 - 0x30 + 52,
                '+' => 62,
                '/' => 63,
                _ => 0,
            };
            for i in (0..6).rev() {
                let bit = (code >> i) & 1;
                table[int_pos] = table[int_pos].wrapping_add(bit.wrapping_shl(int_cnt * 8 + 7 - int_cn2));
                int_cn2 += 1;
                if int_cn2 >= 8 {
                    int_cn2 = 0;
                    int_cnt += 1;
                }
                length += 1;
                if int_cnt >= 4 {
                    int_cnt = 0;
                    int_pos = (int_pos + 1).min(table_len - 1);
                }
            }
        }
        length -= (length - 8) % 0x80;
        if length > MAX_LEN * 8 {
            length = MAX_LEN * 8;
        }
        if length == 0 {
            length = 8;
        }
        self.dpcm[n] = Dpcm { table, int_vol, looped, length };
        self.dpcm_set[n] = true;
    }
    /// An FM voice from `#OPM@n` or `#OPN@n` (OPN's lacking DT2).
    pub fn set_timbre(&mut self, n: usize, opn: bool, body: &str) {
        if n >= 128 {
            return;
        }
        let mut s = String::new();
        let mut sep = false;
        for c in body.chars() {
            if matches!(c, ',' | ';' | ' ' | '\t' | '\r' | '\n') || c.is_whitespace() {
                sep = true;
            } else {
                if sep && !s.is_empty() {
                    s.push(',');
                }
                sep = false;
                s.push(c);
            }
        }
        let a: Vec<i32> = s.split(',').map(|x| x.trim().parse::<f64>().map(osc::i32_of).unwrap_or(0)).collect();
        let mut zero = [0i32; 55];
        zero[46] = 15;
        if a.len() < if opn { 42 } else { 46 } || s.is_empty() {
            return;
        }
        let mut b = [0i32; 55];
        if !opn {
            for (i, v) in b.iter_mut().enumerate() {
                *v = if i < a.len() { a[i] } else { zero[i] };
            }
        } else {
            let mut j = 0;
            let mut i = 0;
            while i < 2 {
                b[i] = a[j];
                i += 1;
                j += 1;
            }
            while i < 46 {
                if (i - 2) % 11 == 9 {
                    b[i] = 0;
                } else {
                    b[i] = a.get(j).copied().unwrap_or(0);
                    j += 1;
                }
                i += 1;
            }
            let l = a.len().min(51);
            while j < l && i < 55 {
                b[i] = a[j];
                i += 1;
                j += 1;
            }
            while i < 55 {
                b[i] = zero[i];
                i += 1;
            }
        }
        self.opm[n] = Some(b.to_vec());
    }
}

impl Default for Tables {
    fn default() -> Tables {
        Tables::new()
    }
}

/// A track as it plays (MTrack's playing half).
struct Player {
    events: Vec<Ev>,
    pointer: usize,
    needle: f64,
    spt: f64,
    end: bool,
    lfo_width: f64,
    ch: Chan,
}

impl Player {
    fn exec(&mut self, sh: &Shared, e: Ev) {
        use compile::*;
        let (d0, d1) = (e.d0, e.d1);
        let (i0, i1) = (d0 as i64, d1 as i64);
        let ch = &mut self.ch;
        match e.status {
            NOTE_ON => ch.note_on(sh, i0, i1),
            NOTE_OFF => ch.note_off(i0),
            NOTE => ch.set_note_no(sh, i0),
            TEMPO => self.spt = spt(d0.floor() / 100.0),
            FORM => ch.set_form(i0, i1),
            ENV1_ATK => ch.apply(&mut |c| c.set_env1_atk(d0)),
            ENV1_ADD => ch.apply(&mut |c| c.set_env1_point(d0, d1)),
            ENV1_REL => ch.apply(&mut |c| c.set_env1_rel(d0)),
            ENV2_ATK => ch.apply(&mut |c| c.set_env2_atk(d0)),
            ENV2_ADD => ch.apply(&mut |c| c.set_env2_point(d0, d1)),
            ENV2_REL => ch.apply(&mut |c| c.set_env2_rel(d0)),
            NOISE_FREQ => ch.apply(&mut |c| c.set_noise_freq(d0)),
            PWM => ch.apply(&mut |c| c.set_pwm(d0)),
            PAN => ch.apply(&mut |c| c.set_pan(i0)),
            FORMANT => ch.apply(&mut |c| c.set_formant(i0)),
            DETUNE => ch.apply(&mut |c| c.set_detune(sh, i0)),
            LFO_FMSF => ch.apply(&mut |c| c.set_lfo_fmsf(i0, i1)),
            LFO_DPWD => {
                self.lfo_width = d1 * self.spt;
                let f = osc::SAMPLE_RATE / self.lfo_width;
                ch.apply(&mut |c| c.set_lfo_dpwd(d0, f));
            }
            LFO_DLTM => {
                let (dl, tm) = (d0 * self.spt, d1 * self.lfo_width);
                ch.apply(&mut |c| c.set_lfo_dltm(dl, tm));
            }
            LFO_TARGET => ch.apply(&mut |c| c.set_lfo_target(i0)),
            LPF_SWTAMT => ch.apply(&mut |c| c.set_lpf_swt_amt(i0, i1)),
            LPF_FRQRES => ch.apply(&mut |c| c.set_lpf_frq_res(i0, i1)),
            VOL_MODE => ch.set_vol_mode(i0),
            INPUT => ch.apply(&mut |c| c.set_input(i0, i1)),
            OUTPUT => ch.apply(&mut |c| c.set_output(i0, i1)),
            EXPRESSION => ch.apply(&mut |c| c.set_expression(sh, i0)),
            RING => ch.apply(&mut |c| c.set_ring(i0, i1)),
            SYNC => ch.apply(&mut |c| c.set_sync(i0, i1)),
            PORTAMENTO => {
                let (depth, len) = (d0 * 100.0, d1 * self.spt);
                ch.apply(&mut |c| c.set_portamento(depth, len));
            }
            MIDIPORT => ch.apply(&mut |c| c.set_midi_port(i0)),
            MIDIPORTRATE => {
                let r = (8.0 - (d0 * 7.99 / 128.0)) / d0;
                ch.apply(&mut |c| c.set_midi_port_rate(r));
            }
            BASENOTE => ch.apply(&mut |c| c.set_port_base(i0 * 100)),
            POLY => ch.set_voice_limit(i0),
            HW_LFO => ch.apply(&mut |c| c.set_hw_lfo(i0)),
            37 => ch.apply(&mut |c| c.sound_off()),
            38 => ch.reset(sh),
            CLOSE => ch.apply(&mut |c| c.close()),
            EOT => self.end = true,
            _ => {}
        }
    }

    /// Plays `[start, end)` of a block: its events in time, its samples
    /// between them (none for the tempo track).
    fn run(&mut self, sh: &mut Shared, mix: &mut [Vec<f32>; 2], start: usize, end: usize, tempo: bool) {
        if self.end {
            return;
        }
        let mut i = start;
        while i < end {
            loop {
                let Some(&e) = self.events.get(self.pointer) else { break };
                let delta = e.delta as f64 * self.spt;
                if self.needle >= delta {
                    self.exec(sh, e);
                    self.needle -= delta;
                    self.pointer += 1;
                } else {
                    break;
                }
            }
            let Some(e) = self.events.get(self.pointer) else { break };
            let delta = e.delta as f64 * self.spt;
            let mut di = (delta - self.needle).ceil() as usize;
            if i + di >= end {
                di = end - i;
            }
            self.needle += di as f64;
            if !tempo {
                self.ch.samples(sh, mix, end, i, di);
            }
            i += di;
        }
    }
}

/// How many samples a block is.
const BLOCK: usize = 8192;

/// A song rendered: left and right interleaved at 44.1 kHz (with
/// FlMML's three seconds after the music), the frame the music ends at,
/// and the song's title (`#TITLE`).
pub struct Rendered {
    pub samples: Vec<f32>,
    pub end: usize,
    pub title: String,
}

/// FlMML's MML rendered as FlMML plays it.
pub fn render_song(text: &str) -> Rendered {
    let song = compile::compile(text);
    let tables = Rc::new(song.tables.clone());
    let fixed = Rc::new(Fixed::new());
    let mut sh = Shared::new(BLOCK, song.pipes, song.syncs);
    let mut players: Vec<Player> = song
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let ch = match song.poly.get(i).copied().flatten() {
                Some(n) => Chan::poly(&sh, &fixed, &tables, n),
                None => Chan::Mono(Box::new(Channel::new(&sh, &fixed, &tables))),
            };
            Player { events: t.events.clone(), pointer: 0, needle: 0.0, spt: compile::spt(120.0), end: false, lfo_width: 0.0, ch }
        })
        .collect();
    let total = ((song.total_msec * f64::from(RATE) / 1000.0).ceil() as usize).min(f64::from(RATE) as usize * 3600);
    let mut out = Vec::with_capacity(total * 2);
    let mut done = 0;
    while done < total {
        let mut mix = [vec![0f32; BLOCK], vec![0f32; BLOCK]];
        if let Some((first, rest)) = players.split_first_mut() {
            first.run(&mut sh, &mut mix, 0, BLOCK, true);
            for p in rest {
                p.run(&mut sh, &mut mix, 0, BLOCK, false);
            }
        }
        let n = BLOCK.min(total - done);
        for k in 0..n {
            out.push(mix[0][k]);
            out.push(mix[1][k]);
        }
        done += n;
    }
    let end = ((song.total_msec - 3000.0).max(0.0) * f64::from(RATE) / 1000.0).round() as usize;
    Rendered { end: end.min(out.len() / 2), samples: out, title: song.title }
}

/// FlMML's MML rendered as FlMML plays it: left and right interleaved at
/// 44.1 kHz, and the song's title.
pub fn render(text: &str) -> (Vec<f32>, String) {
    let r = render_song(text);
    (r.samples, r.title)
}

/// Whether a text looks like FlMML's MML (as neumetik-midi tells).
pub fn looks_like(text: &str) -> bool {
    let directives = ["#TITLE", "#ARTIST", "#COMMENT", "#CODING", "#OPM@", "#OPN@", "#WAV9", "#WAV10", "#WAV13", "#USING POLY", "#OCTAVE REVERSE", "#VELOCITY REVERSE", "#FMGAIN"];
    let lower = text.to_ascii_lowercase();
    directives.iter().any(|d| text.lines().any(|l| l.trim_start().starts_with(d)))
        || (lower.contains("/:") && lower.contains(":/"))
        || lower.contains("@e1,")
        || text.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with('$') && l.contains('=') && l.trim_end().ends_with(';')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0f32, |a, &b| a.max(b.abs()))
    }

    #[test]
    fn a_pulse_plays_at_its_pitch() {
        // o5a: 440 Hz, a second of it.
        let (s, _) = render("t60 q16 l1 o5 a");
        let left: Vec<f32> = s.iter().step_by(2).copied().collect();
        assert!(peak(&left) > 0.1);
        // Rising zero crossings over the note's middle.
        let mid = &left[10_000..30_000];
        let ups = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!((ups as f64 - 440.0 * 20_000.0 / 44_100.0).abs() < 3.0, "{ups}");
    }

    #[test]
    fn every_tone_sounds() {
        for t in 0..=14 {
            let (s, _) = render(&format!("t120 @{t} l4 o4 cdef"));
            assert!(peak(&s) > 0.01, "@{t} is silent");
            assert!(s.iter().all(|v| v.is_finite()), "@{t} isn't finite");
        }
    }

    #[test]
    fn effects_and_the_song_end() {
        let song = "#TITLE Test\n#WAV10 1,0123456789abcdeffedcba9876543210\n#OPM@1 { 4 7\n31 0 0 15 0 20 0 1 3 0 0\n31 0 0 15 0 20 0 1 3 0 0\n31 0 0 15 0 20 0 1 3 0 0\n31 0 0 15 0 20 0 1 3 0 0\n15 0 0 0 0 0 0 0 0 }\nt150 @10-1 @e1,0,20,10,5 @f2,64,40,80 @l20,30,1,0,0 [ceg]4 @14-1 c4;\n@3 @w-30 @'a' @l10,10,4,0 *c c4 @o1,1 d4;\n@i4,1 @0 e2;";
        let (s, title) = render(song);
        assert_eq!(title, "Test");
        assert!(peak(&s) > 0.01);
        assert!(s.iter().all(|v| v.is_finite()));
        // Three beats at 150 (the stray * reads as a quarter-note c, as
        // FlMML reads it), then FlMML's three seconds after.
        let secs = s.len() as f64 / 2.0 / 44_100.0;
        assert!((secs - 4.2).abs() < 0.01, "{secs}");
    }

    /// Every `<name>.mml` in FLMML_DIR against FlMML's own render of it,
    /// `<name>.ref` (float32 pairs at 44.1 kHz, from FlMML on HTML5 run
    /// headless with the same seeded Math.random): sample for sample. FM
    /// songs (`fm*.mml`) are left out: they play on a different chip.
    #[test]
    #[ignore]
    fn against_flmml() {
        let dir = std::env::var("FLMML_DIR").expect("FLMML_DIR");
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "mml")).collect();
        names.sort();
        let mut bad = vec![];
        for p in &names {
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            if stem.starts_with("fm") {
                continue;
            }
            let Ok(r) = std::fs::read(p.with_extension("ref")) else { continue };
            let reference: Vec<f32> = r.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            let (ours, _) = render(&std::fs::read_to_string(p).unwrap());
            let same = ours.len() == reference.len() && ours.iter().zip(&reference).all(|(a, b)| a.to_bits() == b.to_bits());
            println!("{stem}: {}", if same { "the same" } else { "DIFFERENT" });
            if !same {
                bad.push(stem);
            }
        }
        assert!(bad.is_empty(), "{bad:?}");
    }
}

