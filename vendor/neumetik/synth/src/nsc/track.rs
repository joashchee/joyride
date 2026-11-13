// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// NSD.Lib compiler's blocks and tracks: a port of `TrackSet.cpp`, `MusicTrack.cpp` (with its tick-counting
// emulation of the driver, which marks the commands that change something; the rest are dropped) and their
// address fixing (nsc 1.31, S.W., BSD-2).

use std::collections::BTreeMap;

use super::op;
use super::reader::R;
use super::tables;
use super::{Kind, Nsc};

/// The driver's state as the compiler and the tick counter follow it (`NSD_WORK`).
#[derive(Clone, Copy)]
pub(crate) struct Work {
    pub gatemode: i32,
    pub length: i32,
    pub gate_q: i32,
    pub gate_u: i32,
    pub octave: i32,
    pub octave1: i32,
    pub detune_cent: i32,
    pub detune_reg: i32,
    pub trans: i32,
    pub voice: i32,
    pub voice_rel: i32,
    pub volume: i32,
    pub volume_rel: i32,
    pub env_volume: usize,
    pub env_voice: usize,
    pub env_frequency: usize,
    pub env_note: usize,
    pub sw_evoi: bool,
    pub sw_evol: bool,
    pub sw_em: bool,
    pub sw_en: bool,
    pub sweep: i32,
    pub fds_career: usize,
    pub fds_modulator: usize,
    pub sw_fds_career: bool,
    pub sw_fds_modulator: bool,
    pub fds_volume: i32,
    pub fds_frequency: i32,
    pub vrc7_voice: usize,
    pub sw_vrc7_voice: bool,
    pub n163_voice: usize,
    pub sw_n163_voice: bool,
    pub psg_frequency: i32,
}

impl Default for Work {
    fn default() -> Work {
        Work {
            gatemode: -1,
            length: 24,
            gate_q: 0,
            gate_u: 0,
            octave: 3,
            octave1: 0,
            detune_cent: 0,
            detune_reg: 0,
            trans: 0,
            voice: -1,
            voice_rel: -1,
            volume: 15,
            volume_rel: 2,
            env_volume: 0,
            env_voice: 0,
            env_frequency: 0,
            env_note: 0,
            sw_evoi: false,
            sw_evol: false,
            sw_em: false,
            sw_en: false,
            sweep: -1,
            fds_career: 0,
            fds_modulator: 0,
            sw_fds_career: false,
            sw_fds_modulator: false,
            fds_volume: -1,
            fds_frequency: -1,
            vrc7_voice: 0,
            sw_vrc7_voice: false,
            n163_voice: 0,
            sw_n163_voice: false,
            psg_frequency: -1,
        }
    }
}

/// A track being compiled (`MusicTrack`).
pub(crate) struct Track {
    pub item: usize,
    pub nsd: Work,
    offset_now: usize,
    compile_flag: bool,
    jump_flag: bool,
    opt_default_length: i32,
    qmax: i32,
    gatetime_q: i32,
    gatetime_big_q: i32,
    old_note: Option<usize>,
    key_sig: [i8; 8],
    now_key: i8,
    now_scale: i8,
    key_shift: i32,
    echo_already: bool,
    echo_vol_ret: bool,
    echo_flag: bool,
    echo_slur: bool,
    echo_length: i32,
    echo_volume: u8,
    echo_value: u8,
    old_notes: [i8; 256],
    pt_old: u8,
    f_patch: bool,
    i_patch: usize,
    is_loop: bool,
    is_rep_a_s: bool,
    is_rep_a_b: bool,
    is_rep_b_s: bool,
    is_rep_b_b: bool,
    count_repeat_a: i32,
    repeat_type: Vec<i32>,
    sp_repeat_c: usize,
    // Repeat (C)'s stack: its count, and its start, branch and end (an item of the track; None, the list's end).
    st_ct: Vec<i32>,
    st_s: Vec<Option<usize>>,
    st_b: Vec<Option<usize>>,
    st_e: Vec<Option<usize>>,
    vec_fdsc: Vec<usize>,
    vec_fdsm: Vec<usize>,
    vec_opll: Vec<usize>,
    vec_wave: Vec<usize>,
    vec_se: Vec<usize>,
    vec_sub: Vec<usize>,
    vec_env: Vec<usize>,
    vec_loop_end: Vec<usize>,
    vec_rep_a_end: Vec<usize>,
    vec_rep_a_branch: Vec<usize>,
    vec_rep_b_end: Vec<usize>,
    ptc_loop: BTreeMap<usize, usize>,
    ptc_rep_a: BTreeMap<usize, usize>,
    ptc_rep_a_e: BTreeMap<usize, usize>,
    ptc_rep_b: BTreeMap<usize, usize>,
    cnt_loop: usize,
    cnt_rep_a: usize,
    cnt_rep_b: usize,
    pub tick_total: i32,
    pub tick_loop: i32,
}

/// A BGM, SE or Sub block (`TrackSet`).
pub(crate) struct Set {
    pub item: usize,
    tracks: BTreeMap<usize, usize>,
    now: Option<usize>,
    i_track: usize,
    max_track: usize,
    sub: bool,
    se: bool,
    priority: u8,
    tempo: i32,
    f_jump: bool,
    track_pt: i64,
    track_line: usize,
    /// NSFe's track metadata: `#Label` (tlbl), `#Author` (taut), `#Time` and `#Fade` (-1 when not given).
    pub label: Vec<u8>,
    pub author: Vec<u8>,
    pub time: i32,
    pub fade: i32,
}

const SCALE: [[i8; 7]; 80] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, -1, 0, 0, 0, -1],
    [0, -1, -1, 0, 0, -1, -1],
    [0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, -1],
    [0, 0, -1, 0, 0, -1, -1],
    [0, -1, -1, 0, -1, -1, -1],
    [0, 1, 1, 1, 0, 1, 0],
    [0, 0, -1, -1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, -1, 0, 0, -1, 0],
    [0, -1, -1, 0, -1, 0, -1],
    [0, 0, 0, 0, 1, 0, 0],
    [0, 0, -1, 1, 0, 0, -1],
    [0, -1, 0, 0, 0, -1, -1],
    [0, 1, 0, 1, 0, 0, 0],
    [0, -1, -1, -1, -1, -1, -2],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, -1, 0, 0, 0, 0],
    [0, -1, -1, 0, 0, 0, -1],
    [0, 0, 0, 1, 1, 0, 0],
    [0, 0, 0, 1, 0, 0, -1],
    [0, 0, 0, 0, 0, -1, -1],
    [0, 0, -1, 0, -1, -1, -1],
    [0, -1, -1, -1, -1, -1, -1],
    [-1, -1, -1, 0, 0, -1, -1],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, -1, 0],
    [0, 0, -1, 0, -1, 0, -1],
    [0, -1, -1, -1, 0, -1, -1],
    [0, 0, -1, 1, 0, 0, 0],
    [0, -1, 0, 0, 0, 0, -1],
    [0, 1, 0, 1, 1, 0, 0],
    [0, -1, -1, 0, -1, -1, -2],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, -1, -1, 0, 0, 0, 0],
    [0, 0, 0, 1, 1, 1, 0],
    [0, 0, 0, 1, 1, 0, -1],
    [0, 0, 0, 1, 0, -1, -1],
    [0, 0, 0, 0, -1, -1, -1],
    [0, 0, -1, -1, -1, -1, -1],
    [0, -1, -2, -1, -1, -1, -1],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, -1, -1, 0, 0, -1, 0],
    [0, 0, 0, 1, 0, 1, 0],
    [0, 0, 0, 0, 1, 0, -1],
    [0, 0, -1, 1, 0, -1, -1],
    [0, -1, 0, 0, -1, -1, -1],
    [0, 1, 0, 0, 0, 0, 0],
    [0, -1, -2, -1, -1, -1, -2],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, -1, 0, 0, 0, -1, 0],
    [0, 1, 0, 1, 0, 1, 0],
    [0, -1, -1, -1, 0, -1, -2],
    [0, 0, -1, 1, 0, -1, 0],
    [0, -1, 0, 0, -1, 0, -1],
    [0, 1, 0, 0, 1, 0, 0],
    [0, -1, -2, 0, -1, -1, -2],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 1, 0, 0, -1],
    [0, -1, -1, -1, -1, -2, -2],
    [0, 0, -1, 0, -1, -1, 0],
    [0, -1, -1, -1, -1, 0, -1],
    [0, 0, -1, 0, 1, 0, 0],
    [0, -1, -1, 1, 0, 0, -1],
    [0, 0, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0],
];

const KEY: [[i8; 7]; 16] = [
    [-1, -1, -1, 0, -1, -1, -1],
    [0, -1, -1, 0, -1, -1, -1],
    [0, -1, -1, 0, 0, -1, -1],
    [0, 0, -1, 0, 0, -1, -1],
    [0, 0, -1, 0, 0, 0, -1],
    [0, 0, 0, 0, 0, 0, -1],
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1],
    [2, 1, 1, 2, 1, 1, 1],
];

const SHIFT: [usize; 16] = [3, 6, 2, 5, 1, 4, 0, 3, 6, 2, 5, 1, 4, 0, 3, 6];

impl Track {
    fn set_key(&mut self, key: i32, scale: i32) {
        self.now_key = key as i8;
        self.now_scale = 0;
        let k = (key + 6) as usize;
        let shift = SHIFT[k];
        let sc = &SCALE[scale.rem_euclid(80) as usize];
        for i in 0..7 {
            self.key_sig[i] = KEY[k][i].wrapping_add(sc[(shift + i) % 7]);
        }
    }
}

impl<'a> Nsc<'a> {
    fn ts(&self, s: usize) -> &Set {
        &self.sets[s]
    }

    fn now_track(&self, s: usize) -> usize {
        self.sets[s].now.expect("a track")
    }

    /// An event at the end of a track (`SetEvent`).
    fn set_event(&mut self, t: usize, it: usize) {
        self.tracks[t].offset_now += self.items[it].size;
        let ti = self.tracks[t].item;
        self.items[ti].kids.push(it);
    }

    fn ev(&mut self, t: usize, code: &[u8]) {
        let it = self.general(code);
        self.set_event(t, it);
    }

    fn make_track(&mut self, s: usize, n: usize) -> usize {
        let it = self.new_item(Vec::new(), Kind::Plain, Some(n));
        let t = self.tracks.len();
        self.items[it].kind = Kind::Track(t);
        self.items[it].used = true;
        let qmax = self.mml.qmax;
        let mut tr = Track {
            item: it,
            nsd: Work { length: -1, ..Work::default() },
            offset_now: 0,
            compile_flag: false,
            jump_flag: false,
            opt_default_length: -1,
            qmax,
            gatetime_q: 0,
            gatetime_big_q: qmax,
            old_note: None,
            key_sig: [0; 8],
            now_key: 0,
            now_scale: 0,
            key_shift: 0,
            echo_already: false,
            echo_vol_ret: false,
            echo_flag: false,
            echo_slur: false,
            echo_length: -1,
            echo_volume: 0,
            echo_value: 0,
            old_notes: [-1; 256],
            pt_old: 0,
            f_patch: false,
            i_patch: 0,
            is_loop: false,
            is_rep_a_s: false,
            is_rep_a_b: false,
            is_rep_b_s: false,
            is_rep_b_b: false,
            count_repeat_a: 0,
            repeat_type: vec![0],
            sp_repeat_c: 0,
            st_ct: vec![0],
            st_s: vec![None],
            st_b: vec![None],
            st_e: vec![None],
            vec_fdsc: Vec::new(),
            vec_fdsm: Vec::new(),
            vec_opll: Vec::new(),
            vec_wave: Vec::new(),
            vec_se: Vec::new(),
            vec_sub: Vec::new(),
            vec_env: Vec::new(),
            vec_loop_end: Vec::new(),
            vec_rep_a_end: Vec::new(),
            vec_rep_a_branch: Vec::new(),
            vec_rep_b_end: Vec::new(),
            ptc_loop: BTreeMap::new(),
            ptc_rep_a: BTreeMap::new(),
            ptc_rep_a_e: BTreeMap::new(),
            ptc_rep_b: BTreeMap::new(),
            cnt_loop: 0,
            cnt_rep_a: 0,
            cnt_rep_b: 0,
            tick_total: 0,
            tick_loop: 0,
        };
        tr.set_key(0, 0);
        self.tracks.push(tr);
        let null = self.new_item(Vec::new(), Kind::Plain, None);
        self.set_event(t, null);
        let set_item = self.sets[s].item;
        self.items[set_item].kids.push(it);
        self.sets[s].tracks.insert(n, t);
        if self.mml.release_volume != 2 {
            let rv = op::RELEASE_VOLUME + self.mml.release_volume as u8;
            self.ev(t, &[rv]);
        }
        t
    }

    fn get_track(&mut self, s: usize, n: usize) -> usize {
        let mut i = self.sets[s].max_track;
        while i < n {
            i += 1;
            if !self.sets[s].tracks.contains_key(&i) {
                self.make_track(s, i);
            }
        }
        self.sets[s].max_track = i;
        self.sets[s].tracks[&n]
    }

    /// A BGM, SE or Sub block: its tracks compiled, its header made.
    pub fn track_set(&mut self, id: usize, sub: bool, se: bool) -> R<usize> {
        use tables::ts::*;
        let it = self.new_item(Vec::new(), Kind::Plain, Some(id));
        let s = self.sets.len();
        self.items[it].kind = Kind::Set(s);
        let priority = if se { self.mml.priority } else { 0 };
        self.sets.push(Set {
            item: it,
            tracks: BTreeMap::new(),
            now: None,
            i_track: 0,
            max_track: 0,
            sub,
            se,
            priority,
            tempo: 120,
            f_jump: false,
            track_pt: 0,
            track_line: 0,
            label: Vec::new(),
            author: Vec::new(),
            time: -1,
            fade: -1,
        });
        let t0 = self.make_track(s, 0);
        self.sets[s].now = Some(t0);
        self.tracks[t0].compile_flag = true;
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if more {
                self.mml.back();
            } else {
                self.track_chk(s)?;
                if self.sets[s].now.is_none() {
                    break;
                }
            }
            let cmd = self.mml.get_command_id(TABLE);
            let t = self.now_track(s);
            match cmd {
                Some(mml_label) | Some(mml_author) => {
                    let mut x = Vec::new();
                    self.mml.get_string(&mut x, true)?;
                    if cmd == Some(mml_label) {
                        self.sets[s].label = x;
                    } else {
                        self.sets[s].author = x;
                    }
                }
                Some(mml_time) => self.sets[s].time = self.mml.get_int()?,
                Some(mml_fade) => self.sets[s].fade = self.mml.get_int()?,
                Some(mml_Track) => {
                    if self.ts(s).sub {
                        self.mml.warning("TR in a Sub block is ignored");
                    } else {
                        self.track_proc(s)?;
                    }
                }
                Some(mml_Scale) => {
                    let sc = self.mml.get_num()? as i8;
                    let tr = &mut self.tracks[t];
                    tr.now_scale = sc;
                    let k = tr.now_key;
                    tr.set_key(i32::from(k), i32::from(sc));
                }
                Some(mml_Major) => {
                    let k = self.tracks[t].now_key;
                    self.tracks[t].set_key(i32::from(k), 0);
                }
                Some(mml_Minor) => {
                    let k = self.tracks[t].now_key;
                    self.tracks[t].set_key(i32::from(k), 5);
                }
                Some(mml_HMinor) | Some(mml_MMinor) => {
                    let mut sc: i8 = if self.mml.last_id == mml_HMinor { 10 } else { 20 };
                    let c = self.mml.get_char()?;
                    self.mml.back();
                    if c == b'(' {
                        sc = sc.wrapping_add(self.mml.get_num()? as i8);
                    }
                    let k = self.tracks[t].now_key;
                    self.tracks[t].set_key(i32::from(k), i32::from(sc));
                }
                Some(mml_KeySignature) => self.key_signature(t)?,
                Some(mml_Macro) => self.mml.call_macro()?,
                Some(mml_MacroSet) => self.mml.set_macro(1)?,
                Some(mml_CallSE) => {
                    let n = self.mml.get_int()? as usize;
                    if !self.tracks[t].jump_flag {
                        let e = self.address(Some(n), op::CALL_SE, None);
                        self.set_event(t, e);
                    }
                }
                Some(mml_Subroutine) => {
                    let n = self.mml.get_int()? as usize;
                    if !self.tracks[t].jump_flag {
                        let e = self.address(Some(n), op::CALL, None);
                        self.items[e].kind = Kind::CallSub { patch: false, no_opt: false };
                        self.set_event(t, e);
                    }
                    self.tracks[t].opt_default_length = -1;
                }
                Some(mml_Loop) => {
                    if self.ts(s).sub {
                        self.mml.warning("L in a Sub block is ignored");
                    } else {
                        self.set_loop(t)?;
                    }
                }
                Some(mml_Repeat_B_Start) => {
                    self.repeat_b_start(t);
                    self.tracks[t].is_rep_b_s = true;
                    self.tracks[t].is_rep_b_b = false;
                }
                Some(mml_Repeat_B_Branch) => {
                    self.echo_vol_ret(t);
                    if !self.tracks[t].is_rep_b_s {
                        return self.mml.err("no |: before \\");
                    }
                    if self.tracks[t].is_rep_b_b {
                        return self.mml.err("\\ twice in repeat (B)");
                    }
                    self.tracks[t].is_rep_b_b = true;
                    self.ev(t, &[op::REPEAT_B_BRANCH]);
                }
                Some(mml_Repeat_B_End) => {
                    self.echo_vol_ret(t);
                    if !self.tracks[t].is_rep_b_s {
                        return self.mml.err("no |: before :|");
                    }
                    if !self.tracks[t].is_rep_b_b {
                        return self.mml.err("repeat (B) needs \\");
                    }
                    self.repeat_b_end(t);
                }
                Some(mml_Repeat_A_Branch) => self.repeat_branch(t)?,
                Some(mml_Repeat_A_Start) => {
                    let (mode, sub) = (self.mml.repeat_mode, self.ts(s).sub);
                    if mode == 0 && !sub {
                        if !self.tracks[t].is_rep_a_s {
                            self.repeat_a_start(t)?;
                        } else {
                            self.repeat_c_start(t)?;
                        }
                    } else if mode == 1 {
                        self.repeat_a_start(t)?;
                    } else {
                        self.repeat_c_start(t)?;
                    }
                }
                Some(mml_Repeat_A_End) => {
                    let (mode, sub) = (self.mml.repeat_mode, self.ts(s).sub);
                    if mode == 0 && !sub {
                        match self.tracks[t].repeat_type.last().copied() {
                            Some(1) => self.repeat_a_end(t)?,
                            Some(3) => self.repeat_c_end(t)?,
                            _ => return self.mml.err("no repeat start"),
                        }
                    } else if mode == 1 {
                        self.repeat_a_end(t)?;
                    } else {
                        self.repeat_c_end(t)?;
                    }
                }
                Some(mml_Repeat_C_Start) => {
                    let (mode, sub) = (self.mml.repeat_mode, self.ts(s).sub);
                    if mode == 2 || (mode == 0 && sub) {
                        self.repeat_a_start(t)?;
                    } else {
                        self.repeat_c_start(t)?;
                    }
                }
                Some(mml_Repeat_C_End) => {
                    let (mode, sub) = (self.mml.repeat_mode, self.ts(s).sub);
                    if mode == 2 || (mode == 0 && sub) {
                        self.repeat_a_end(t)?;
                    } else {
                        self.repeat_c_end(t)?;
                    }
                }
                Some(mml_Tempo) => {
                    let v = self.mml.get_int()?;
                    let tempo = (v * self.mml.timebase) / 24;
                    self.sets[s].tempo = tempo;
                    self.mml.chk_range("the tempo (with the timebase)", 0, 255, tempo)?;
                    self.ev(t, &[op::TEMPO, tempo as u8]);
                }
                Some(mml_Tempo_Relative) => {
                    let v = self.mml.get_int()?;
                    self.sets[s].tempo += v;
                    self.ev(t, &[op::RELATIVE_TEMPO, v as u8]);
                }
                Some(mml_Tempo_Up) => {
                    self.sets[s].tempo += 4;
                    self.ev(t, &[op::RELATIVE_TEMPO, 4]);
                }
                Some(mml_Tempo_Down) => {
                    self.sets[s].tempo -= 4;
                    self.ev(t, &[op::RELATIVE_TEMPO, 0xfc]);
                }
                Some(k @ (mml_La | mml_Si | mml_Do | mml_Re | mml_Mi | mml_Fa | mml_Sol)) => {
                    let note = match k {
                        mml_La => 5,
                        mml_Si => 6,
                        mml_Do => 0,
                        mml_Re => 1,
                        mml_Mi => 2,
                        mml_Fa => 3,
                        _ => 4,
                    };
                    self.set_note(t, note)?;
                }
                Some(mml_Rest) => {
                    let m = self.mml.rest;
                    self.set_rest(t, m)?;
                }
                Some(mml_Wait) => {
                    let m = self.mml.wait | 0x80;
                    self.set_rest(t, m)?;
                }
                Some(mml_Tai) => {
                    if !self.tracks[t].jump_flag {
                        match self.tracks[t].old_note {
                            None => return self.mml.err("the note to tie isn't known"),
                            Some(n) => self.items[n].code[0] |= op::NOTE_SLUR,
                        }
                    }
                    self.set_rest(t, 2)?;
                }
                Some(mml_Length) => self.set_length(t)?,
                Some(mml_Gate_Q) | Some(mml_Gate_q) => {
                    let big = (self.mml.last_id == mml_Gate_Q) != self.mml.q_reverse;
                    if big {
                        let qmax = self.tracks[t].qmax;
                        let i = self.mml.get_int_range("the gate time (Q)", 1, qmax)?;
                        self.tracks[t].gatetime_big_q = i;
                        if i == qmax {
                            self.tracks[t].gatetime_q = 0;
                            self.set_q(t, 0);
                        }
                    } else {
                        let i = self.mml.get_int_range("the gate time (q)", 0, 255)?;
                        self.tracks[t].gatetime_q = i;
                        self.tracks[t].gatetime_big_q = self.tracks[t].qmax;
                        self.set_q(t, i);
                    }
                }
                Some(mml_Gate_u) => {
                    let i = if self.mml.get_char()? == b'0' {
                        0
                    } else {
                        self.mml.back();
                        self.mml.get_length(-1)?
                    };
                    self.set_u(t, i);
                }
                Some(k @ (mml_Echo_C | mml_Echo_D | mml_Echo_E | mml_Echo_F | mml_Echo_G | mml_Echo_A | mml_Echo_B | mml_Echo_R)) => {
                    let note = match k {
                        mml_Echo_C => 0,
                        mml_Echo_D => 1,
                        mml_Echo_E => 2,
                        mml_Echo_F => 3,
                        mml_Echo_G => 4,
                        mml_Echo_A => 5,
                        mml_Echo_B => 6,
                        _ => -1,
                    };
                    self.echo_buffer(t, note)?;
                }
                Some(mml_Echo_Reset) => {
                    self.tracks[t].old_notes = [-1; 256];
                    self.tracks[t].pt_old = 0;
                }
                Some(mml_Echo_Off) => {
                    self.echo_vol_ret(t);
                    self.tracks[t].echo_flag = false;
                }
                Some(mml_Echo) => self.set_echo(t)?,
                Some(mml_Jump) => {
                    let v = self.mml.get_int_range("J", 0, 1)?;
                    self.tracks[t].jump_flag = v == 1;
                }
                Some(mml_Jump_drv) => {
                    let c = self.mml.get_char()?;
                    self.mml.back();
                    let v = if c.is_ascii_digit() { self.mml.get_int_range("j", 0, 1)? } else { 0 };
                    if v == 0 {
                        self.ev(t, &[op::SUB_COMMAND, op::SUB_JUMP_OFF]);
                        if !self.sets[s].f_jump {
                            let e = self.general(&[op::SUB_COMMAND, op::SUB_JUMP_ON]);
                            let t0 = self.sets[s].tracks[&0];
                            self.tracks[t0].offset_now += 2;
                            let ti = self.tracks[t0].item;
                            self.items[ti].kids.insert(0, e);
                        }
                    } else {
                        self.ev(t, &[op::SUB_COMMAND, op::SUB_JUMP_ON]);
                    }
                    self.sets[s].f_jump = true;
                }
                Some(mml_Envelop_Voice) => {
                    let n = (i64::from(self.mml.get_int()?) + self.mml.offset_ei) as usize;
                    self.env_evoi(t, n);
                }
                Some(mml_Envelop_Volume) => {
                    let n = (i64::from(self.mml.get_int()?) + self.mml.offset_ev) as usize;
                    self.env_evol(t, Some(n));
                }
                Some(mml_Envelop_Frequency) => {
                    let n = (i64::from(self.mml.get_int()?) + self.mml.offset_em) as usize;
                    self.env_em(t, Some(n));
                }
                Some(mml_Envelop_Note) => {
                    let n = (i64::from(self.mml.get_int()?) + self.mml.offset_en) as usize;
                    self.env_en(t, Some(n));
                }
                Some(mml_Envelop_Off_Voice) => return self.mml.err("E@* doesn't exist; use @"),
                Some(mml_Envelop_Off_Volume) => self.env_evol(t, None),
                Some(mml_Envelop_Off_Frequency) => self.env_em(t, None),
                Some(mml_Envelop_Off_Note) => self.env_en(t, None),
                Some(mml_Patch) => {
                    let i = self.mml.get_int()? as usize;
                    self.tracks[t].f_patch = true;
                    self.tracks[t].i_patch = i;
                    if !self.mml.patches.contains_key(&i) {
                        self.tracks[t].f_patch = false;
                        return self.mml.err(format!("Patch({i}) isn't defined"));
                    }
                }
                Some(mml_Patch_Off) => self.tracks[t].f_patch = false,
                Some(mml_Release_mdoe) => {
                    let i = self.mml.get_int_range("the release mode", 0, 2)?;
                    self.ev(t, &[op::GATE_MODE_0 + i as u8]);
                }
                Some(mml_Release_Voice) => {
                    let i = self.mml.get_int_range("the release voice", 0, 7)?;
                    self.ev(t, &[op::RELEASE_VOICE + i as u8]);
                }
                Some(mml_Release_Volume) => {
                    let i = self.mml.get_int_range("the volume", 0, 15)?;
                    self.ev(t, &[op::RELEASE_VOLUME + i as u8]);
                }
                Some(mml_Voice) => {
                    let n = self.mml.get_int()?;
                    self.set_voice(t, n);
                }
                Some(mml_FDSC) | Some(mml_FDSM) | Some(mml_VRC7) => {
                    let code = match self.mml.last_id {
                        mml_FDSC => op::FDS_CAREER,
                        mml_FDSM => op::FDS_MODULATOR,
                        _ => op::VRC7,
                    };
                    let n = self.mml.get_int()? as usize;
                    let e = self.address(Some(n), code, None);
                    self.set_event(t, e);
                }
                Some(mml_FDSF) => {
                    let i = self.mml.get_int_range("the FDS career frequency", 0, 0x0fff)?;
                    self.ev(t, &[op::FDS_FREQUENCY, i as u8, (i >> 8) as u8]);
                }
                Some(mml_FDSV) => {
                    let i = self.mml.get_int_range("the FDS master volume", 0, 3)?;
                    self.ev(t, &[op::FDS_VOLUME, i as u8]);
                }
                Some(mml_N163) => {
                    let num = self.mml.get_int_range("the n16x wave's address", 0, 63)? as u8;
                    self.mml.chk_comma()?;
                    let n = self.mml.get_int()? as usize;
                    let e = self.address(Some(n), op::N163, Some(num));
                    self.set_event(t, e);
                }
                Some(mml_N163_Load) => {
                    let i = self.mml.get_int_range("the n16x wave's address", 0, 252)?;
                    if i % 4 != 0 {
                        return self.mml.err("the n16x wave's address must be a multiple of 4");
                    }
                    self.mml.chk_comma()?;
                    let n = self.mml.get_int()? as usize;
                    let e = self.address(Some(n), op::N163, Some((i / 4) as u8));
                    self.set_event(t, e);
                }
                Some(mml_N163_Set) => {
                    let i = self.mml.get_int_range("the n16x wave's address", 0, 252)?;
                    if i % 4 != 0 {
                        return self.mml.err("the n16x wave's address must be a multiple of 4");
                    }
                    self.set_voice(t, i / 4);
                    if self.mml.get_char()? == b',' {
                        let i = self.mml.get_int_range("the n16x sample length", 4, 256)?;
                        if i % 4 != 0 {
                            return self.mml.err("the n16x sample length must be a multiple of 4");
                        }
                        self.ev(t, &[op::SWEEP, (64 - i / 4) as u8]);
                    } else {
                        self.mml.back();
                    }
                }
                Some(mml_N163_Channel) => {
                    let i = self.mml.get_int_range("n163's channels", 1, 8)?;
                    self.ev(t, &[op::N163_CHANNEL, (i - 1) as u8]);
                }
                Some(mml_FME7_frequency) => {
                    let i = self.mml.get_int_range("the 5B's envelope frequency", 0, 0xffff)?;
                    self.ev(t, &[op::FME7_FREQUENCY, i as u8, (i >> 8) as u8]);
                }
                Some(mml_Octave) => {
                    let o = self.mml.get_int_range("the octave", 1, 8)? - 1;
                    self.tracks[t].nsd.octave = o;
                    self.ev(t, &[op::OCTAVE + o as u8]);
                }
                Some(mml_Octave_Up) | Some(mml_Octave_Down) => {
                    let up = (self.mml.last_id == mml_Octave_Up) != self.mml.octave_reverse;
                    if up {
                        self.octave_inc(t);
                    } else {
                        self.octave_dec(t);
                    }
                }
                Some(mml_Octave_Up1) => {
                    if !self.tracks[t].jump_flag {
                        self.ev(t, &[op::OCTAVE_UP_1]);
                        self.tracks[t].nsd.octave1 += 1;
                    }
                }
                Some(mml_Octave_Down1) => {
                    if !self.tracks[t].jump_flag {
                        self.ev(t, &[op::OCTAVE_DOWN_1]);
                        self.tracks[t].nsd.octave1 -= 1;
                    }
                }
                Some(mml_Detune_Cent) | Some(mml_Detune_Register) => {
                    let code = if self.mml.last_id == mml_Detune_Cent { op::DETUNE_CENT } else { op::DETUNE_REGISTER };
                    let v = self.mml.get_int_range("a parameter", -128, 255)?;
                    self.ev(t, &[code, v as u8]);
                }
                Some(mml_Detune_Cent_Relative) | Some(mml_Detune_Register_Relative) => {
                    let sub = if self.mml.last_id == mml_Detune_Cent_Relative {
                        op::SUB_DETUNE_CENT
                    } else {
                        op::SUB_DETUNE_REGISTER
                    };
                    let v = self.mml.get_int()?;
                    self.ev(t, &[op::SUB_COMMAND, sub, v as u8]);
                }
                Some(mml_Transpose) => {
                    let v = self.mml.get_int_range("the transpose", -128, 127)?;
                    self.set_transpose(t, v);
                }
                Some(mml_Transpose_Relative) => {
                    let v = self.mml.get_int_range("the relative transpose", -128, 127)?;
                    self.tracks[t].nsd.trans += v;
                    self.ev(t, &[op::RELATIVE_TRANSPOSE, v as u8]);
                }
                Some(mml_KeyShift) => self.tracks[t].key_shift = self.mml.get_int_range("the key shift", -128, 127)?,
                Some(mml_KeyShift_Relative) => {
                    let v = self.mml.get_int()?;
                    self.tracks[t].key_shift += v;
                    let k = self.tracks[t].key_shift;
                    self.mml.chk_range("the key shift's range", -128, 127, k)?;
                }
                Some(mml_Protament) => {
                    let decay = self.mml.get_int_range("the portamento's 1st parameter", 0, 255)? + 1;
                    self.mml.chk_comma()?;
                    let rate = self.mml.get_int_range("the portamento's 2nd parameter", 1, 256)?;
                    self.mml.chk_comma()?;
                    let depth = self.mml.get_int_range("the portamento's 3rd parameter", -128, 127)?;
                    self.mml.chk_comma()?;
                    let target = self.mml.get_int_range("the portamento's 4th parameter", -128, 127)?;
                    if !self.tracks[t].jump_flag {
                        self.ev(t, &[op::PORTAMENTO, decay as u8, rate as u8, depth as u8, target as u8]);
                    }
                }
                Some(mml_Protament2) => {
                    let tempo = self.sets[s].tempo as u8;
                    self.portamento2(t, tempo)?;
                }
                Some(mml_Sweep) => {
                    let speed = self.mml.get_int()?;
                    let c = if self.mml.get_char()? != b',' {
                        self.mml.chk_range("s", 0, 255, speed)?;
                        self.mml.back();
                        speed as u8
                    } else {
                        self.mml.chk_range("s's 1st parameter", 0, 15, speed)?;
                        let depth = self.mml.get_int()?;
                        self.mml.chk_range("s's 2nd parameter", 0, 15, depth)?;
                        (((speed & 0x0f) << 4) | (depth & 0x0f)) as u8
                    };
                    self.ev(t, &[op::SWEEP, c]);
                }
                Some(mml_Volume) => {
                    let v = self.mml.get_int_range("the volume", 0, 15)?;
                    self.tracks[t].nsd.volume = v;
                    self.tracks[t].echo_vol_ret = false;
                    self.ev(t, &[op::VOLUME + v as u8]);
                }
                Some(mml_Volume_Up) | Some(mml_Volume_Down) => {
                    let up = self.mml.last_id == mml_Volume_Up;
                    let c = self.mml.get_char()?;
                    self.mml.back();
                    let mut v = if c.is_ascii_digit() { self.mml.get_int_range("the relative volume", 0, 15)? } else { 1 };
                    self.echo_vol_ret(t);
                    while v > 0 {
                        self.ev(t, &[if up { op::VOLUME_UP } else { op::VOLUME_DOWN }]);
                        self.tracks[t].nsd.volume += if up { 1 } else { -1 };
                        v -= 1;
                    }
                    let n = &mut self.tracks[t].nsd;
                    n.volume = n.volume.clamp(if up { i32::MIN } else { 0 }, if up { 15 } else { i32::MAX });
                }
                Some(mml_VRC7_Write) => {
                    let r = self.mml.get_int_range("VRC7's register", 0, 0x40)?;
                    self.mml.chk_comma()?;
                    let d = self.mml.get_int_range("VRC7's data", 0, 0xff)?;
                    self.ev(t, &[op::VRC7_REG, r as u8, d as u8]);
                }
                Some(mml_Memory_Write) => {
                    let a = self.mml.get_int_range("y's address", 0, 0xffff)?;
                    self.mml.chk_comma()?;
                    let d = self.mml.get_int_range("y's data", 0, 255)?;
                    self.ev(t, &[op::POKE, a as u8, (a >> 8) as u8, d as u8]);
                }
                Some(mml_Priority) => {
                    let i = self.mml.get_int_range("the priority", 0, 3)?;
                    if !self.sets[s].se {
                        self.mml.warning("a priority outside an SE block is ignored");
                    } else {
                        self.sets[s].priority = i as u8;
                    }
                }
                Some(mml_Bar) => {}
                _ => return self.mml.err_unknown_cmd(),
            }
        }
        self.mml.delete_macro(1);
        if self.sets[s].sub {
            self.items[it].code.clear();
            let t0 = self.sets[s].tracks[&0];
            self.set_end(t0)?;
        } else {
            let max = self.sets[s].max_track;
            let size = 2 + (max + 1) * 2;
            let pr = self.sets[s].priority;
            let mut code = vec![0u8; size];
            code[0] = (max + 1) as u8;
            code[1] = pr;
            self.items[it].code = code;
            self.items[it].size = size;
            for n in 0..=max {
                let t = self.sets[s].tracks[&n];
                self.set_end(t)?;
            }
        }
        Ok(s)
    }

    fn track_chk(&mut self, s: usize) -> R<()> {
        if self.mml.macro_nest() > 0 {
            return self.mml.err("tracks can't be given in a macro");
        }
        let mut i = self.sets[s].i_track + 1;
        self.sets[s].i_track = 0;
        while i <= self.sets[s].max_track {
            let t = self.sets[s].tracks[&i];
            if self.tracks[t].compile_flag {
                self.sets[s].i_track = i;
                break;
            }
            i += 1;
        }
        if self.sets[s].i_track != 0 {
            let (pt, line) = (self.sets[s].track_pt, self.sets[s].track_line);
            self.mml.seek(pt);
            self.mml.set_line(line);
            let n = self.sets[s].i_track;
            let t = self.get_track(s, n);
            self.sets[s].now = Some(t);
        } else {
            self.sets[s].now = None;
        }
        Ok(())
    }

    fn track_proc(&mut self, s: usize) -> R<()> {
        self.track_chk(s)?;
        if self.sets[s].now.is_none() {
            for n in 0..=self.sets[s].max_track {
                let t = self.sets[s].tracks[&n];
                self.tracks[t].compile_flag = false;
            }
            loop {
                let i = self.mml.get_int()? - 1;
                if i <= -1 {
                    return self.mml.err("a track number out of range");
                }
                self.sets[s].i_track = i as usize;
                let t = self.get_track(s, i as usize);
                self.sets[s].now = Some(t);
                self.tracks[t].compile_flag = true;
                if self.mml.get_char()? != b',' {
                    break;
                }
            }
            self.mml.back();
            self.sets[s].track_pt = self.mml.tellg();
            self.sets[s].track_line = self.mml.line();
            for n in 0..=self.sets[s].max_track {
                let t = self.sets[s].tracks[&n];
                if self.tracks[t].compile_flag {
                    self.sets[s].i_track = n;
                    let t = self.get_track(s, n);
                    self.sets[s].now = Some(t);
                    break;
                }
            }
        }
        Ok(())
    }

    fn set_end(&mut self, t: usize) -> R<()> {
        if self.tracks[t].is_rep_a_s {
            return self.mml.err("a repeat (A) isn't closed");
        }
        if self.tracks[t].sp_repeat_c != 0 {
            return self.mml.err("a repeat (C) isn't closed");
        }
        if self.tracks[t].offset_now != 0 {
            if !self.tracks[t].is_loop {
                self.ev(t, &[op::END_OF_TRACK]);
            } else {
                self.echo_vol_ret(t);
                let n = self.tracks[t].cnt_loop - 1;
                let e = self.address(Some(n), op::JUMP, None);
                self.set_event(t, e);
            }
        }
        let ti = self.tracks[t].item;
        self.items[ti].size = self.tracks[t].offset_now;
        Ok(())
    }

    fn set_loop(&mut self, t: usize) -> R<()> {
        if self.tracks[t].is_loop {
            return self.mml.err("L twice in a track");
        }
        self.tracks[t].is_loop = true;
        let n = self.tracks[t].cnt_loop;
        let e = self.new_item(Vec::new(), Kind::Plain, Some(n));
        self.set_event(t, e);
        self.tracks[t].ptc_loop.insert(n, e);
        self.tracks[t].cnt_loop += 1;
        Ok(())
    }

    fn repeat_b_start(&mut self, t: usize) {
        let n = self.tracks[t].cnt_rep_b;
        let e = self.new_item(vec![op::REPEAT_B_START], Kind::Repeat, Some(n));
        self.set_event(t, e);
        self.tracks[t].ptc_rep_b.insert(n, e);
        self.tracks[t].cnt_rep_b += 1;
    }

    fn repeat_b_end(&mut self, t: usize) {
        let n = self.tracks[t].cnt_rep_b.wrapping_sub(1);
        let e = self.address(Some(n), op::REPEAT_B_END, None);
        self.set_event(t, e);
        self.tracks[t].opt_default_length = -1;
    }

    fn repeat_count(&mut self) -> R<i32> {
        let c = self.mml.get_char()?;
        self.mml.back();
        if !c.is_ascii_digit() {
            Ok(-1)
        } else {
            self.mml.get_int_range("the repeat count", 2, 255)
        }
    }

    fn repeat_a_start(&mut self, t: usize) -> R<()> {
        let n = self.repeat_count()?;
        self.tracks[t].count_repeat_a = n;
        if self.tracks[t].is_rep_a_s {
            return self.mml.err("repeat (A) can't nest");
        }
        self.tracks[t].is_rep_a_s = true;
        self.tracks[t].is_rep_a_b = false;
        self.event_repeat_a_start(t, n as u8);
        self.tracks[t].repeat_type.push(1);
        Ok(())
    }

    fn event_repeat_a_start(&mut self, t: usize, count: u8) {
        let n = self.tracks[t].cnt_rep_a;
        let e = self.new_item(vec![op::REPEAT_A_START, count], Kind::Repeat, Some(n));
        self.set_event(t, e);
        self.tracks[t].ptc_rep_a.insert(n, e);
        self.tracks[t].cnt_rep_a += 1;
    }

    fn event_repeat_a_branch(&mut self, t: usize) {
        let n = self.tracks[t].cnt_rep_a.wrapping_sub(1);
        let e = self.address(Some(n), op::REPEAT_A_BRANCH, None);
        self.set_event(t, e);
    }

    fn event_repeat_a_end(&mut self, t: usize) {
        let n = self.tracks[t].cnt_rep_a.wrapping_sub(1);
        let e = self.address(Some(n), op::REPEAT_A_END, None);
        self.set_event(t, e);
        self.tracks[t].ptc_rep_a_e.insert(n, e);
        self.tracks[t].opt_default_length = -1;
    }

    fn last_event(&self, t: usize) -> Option<usize> {
        self.items[self.tracks[t].item].kids.last().copied()
    }

    fn repeat_c_start(&mut self, t: usize) -> R<()> {
        let n = self.repeat_count()?;
        let last = self.last_event(t);
        let tr = &mut self.tracks[t];
        tr.st_ct.push(n);
        tr.st_s.push(last);
        tr.st_b.push(last);
        tr.st_e.push(last);
        tr.sp_repeat_c += 1;
        tr.repeat_type.push(3);
        Ok(())
    }

    fn repeat_branch(&mut self, t: usize) -> R<()> {
        self.echo_vol_ret(t);
        match self.tracks[t].repeat_type.last().copied() {
            Some(1) => {
                if !self.tracks[t].is_rep_a_s {
                    return self.mml.err("no [ before :");
                }
                if self.tracks[t].is_rep_a_b {
                    return self.mml.err(": twice in repeat (A)");
                }
                self.tracks[t].is_rep_a_b = true;
                self.event_repeat_a_branch(t);
            }
            Some(3) => {
                if self.tracks[t].sp_repeat_c == 0 {
                    return self.mml.err("no [: before :");
                }
                let last = self.last_event(t);
                let tr = &mut self.tracks[t];
                if tr.st_b.last() == tr.st_s.last() {
                    *tr.st_b.last_mut().unwrap() = last;
                } else {
                    return self.mml.err(": twice in repeat (C)");
                }
            }
            _ => return self.mml.err("no repeat start"),
        }
        Ok(())
    }

    fn repeat_a_end(&mut self, t: usize) -> R<()> {
        self.echo_vol_ret(t);
        if !self.tracks[t].is_rep_a_s || self.tracks[t].repeat_type.last() != Some(&1) {
            return self.mml.err("no [ before ]");
        }
        let c = self.mml.get_char()?;
        self.mml.back();
        if !c.is_ascii_digit() {
            if self.tracks[t].count_repeat_a == -1 {
                return self.mml.err("the repeat count is missing");
            }
        } else {
            if self.tracks[t].count_repeat_a != -1 {
                return self.mml.err("the repeat count is given twice");
            }
            self.tracks[t].count_repeat_a = self.mml.get_int_range("the repeat count", 2, 255)?;
        }
        let n = self.tracks[t].cnt_rep_a - 1;
        let start = self.tracks[t].ptc_rep_a[&n];
        self.items[start].code[1] = self.tracks[t].count_repeat_a as u8;
        self.event_repeat_a_end(t);
        self.tracks[t].is_rep_a_s = false;
        self.tracks[t].repeat_type.pop();
        Ok(())
    }

    // The item after `x` in the track (`x` None: the list's end).
    fn next_event(&self, t: usize, x: Option<usize>) -> Option<usize> {
        let kids = &self.items[self.tracks[t].item].kids;
        match x {
            None => kids.first().copied(),
            Some(x) => {
                let i = kids.iter().position(|&k| k == x)?;
                kids.get(i + 1).copied()
            }
        }
    }

    fn repeat_c_end(&mut self, t: usize) -> R<()> {
        self.echo_vol_ret(t);
        if self.tracks[t].sp_repeat_c == 0 || self.tracks[t].repeat_type.last() != Some(&3) {
            return self.mml.err("no [: before :]");
        }
        let c = self.mml.get_char()?;
        let mut count = *self.tracks[t].st_ct.last().unwrap();
        self.mml.back();
        if !c.is_ascii_digit() {
            if count == -1 {
                return self.mml.err("the repeat count is missing");
            }
        } else {
            if count != -1 {
                return self.mml.err("the repeat count is given twice");
            }
            count = self.mml.get_int_range("the repeat count", 2, 255)?;
            *self.tracks[t].st_ct.last_mut().unwrap() = count;
        }
        let last = self.last_event(t);
        *self.tracks[t].st_e.last_mut().unwrap() = last;
        let (s, b, e) = (*self.tracks[t].st_s.last().unwrap(), *self.tracks[t].st_b.last().unwrap(), *self.tracks[t].st_e.last().unwrap());
        let mut left = (count - 1) as u32;
        while left > 0 {
            let mut pt = s;
            if pt != e {
                loop {
                    pt = self.next_event(t, pt);
                    let Some(x) = pt else { break };
                    let opc = self.items[x].get_code(0);
                    match opc {
                        op::REPEAT_A_START => {
                            let cnt = self.items[x].code[1];
                            self.event_repeat_a_start(t, cnt);
                        }
                        op::REPEAT_A_BRANCH => self.event_repeat_a_branch(t),
                        op::REPEAT_A_END => self.event_repeat_a_end(t),
                        op::REPEAT_B_START => self.repeat_b_start(t),
                        op::REPEAT_B_BRANCH => self.ev(t, &[op::REPEAT_B_BRANCH]),
                        op::REPEAT_B_END => self.repeat_b_end(t),
                        _ => {
                            // A copy (`mml_CallSub`'s, or a `MusicEvent`'s).
                            let copy = self.items[x].clone();
                            self.items.push(copy);
                            let ni = self.items.len() - 1;
                            self.set_event(t, ni);
                        }
                    }
                    if left == 1 && pt == b {
                        pt = e;
                    }
                    if pt == e {
                        break;
                    }
                }
            }
            left -= 1;
        }
        let tr = &mut self.tracks[t];
        tr.st_ct.pop();
        tr.st_s.pop();
        tr.st_b.pop();
        tr.st_e.pop();
        tr.sp_repeat_c -= 1;
        tr.opt_default_length = -1;
        tr.repeat_type.pop();
        Ok(())
    }

    fn call_patch(&mut self, t: usize, note: i8) {
        if !self.tracks[t].f_patch {
            return;
        }
        let ip = self.tracks[t].i_patch;
        let Some(p) = self.mml.patches.get_mut(&ip) else { return };
        p.set_note(i32::from(note));
        let sc = p.now();
        if sc.f_sub && !self.tracks[t].jump_flag {
            let e = self.address(Some(sc.sub), op::CALL, None);
            self.items[e].kind = Kind::CallSub { patch: true, no_opt: sc.f_sub_opt };
            self.set_event(t, e);
        }
        if sc.f_gate_q {
            self.set_q(t, sc.gate_q as i32);
        }
        if sc.f_gate_u {
            self.set_u(t, sc.gate_u as i32);
        }
        if sc.f_sweep {
            self.ev(t, &[op::SWEEP, sc.sweep]);
        }
        if sc.f_key {
            self.set_transpose(t, sc.key as i32);
        }
        if sc.f_voi {
            self.set_voice(t, sc.voi as i32);
        }
        if sc.f_evoi && sc.sw_evoi {
            let n = (sc.evoi as i64 + self.mml.offset_ei) as usize;
            self.env_evoi(t, n);
        }
        if sc.f_evol {
            let n = sc.sw_evol.then(|| (sc.evol as i64 + self.mml.offset_ev) as usize);
            self.env_evol(t, n);
        }
        if sc.f_em {
            let n = sc.sw_em.then(|| (sc.em as i64 + self.mml.offset_em) as usize);
            self.env_em(t, n);
        }
        if sc.f_en {
            let n = sc.sw_en.then(|| (sc.en as i64 + self.mml.offset_en) as usize);
            self.env_en(t, n);
        }
    }

    // nsc keeps the voice envelope's number in `env_volume` and the volume's in `env_voice` (swapped, as here).
    fn env_evoi(&mut self, t: usize, n: usize) {
        self.tracks[t].nsd.env_volume = n;
        self.tracks[t].nsd.sw_evoi = true;
        let e = self.address(Some(n), op::ENV_VOICE, None);
        self.set_event(t, e);
    }

    fn env_evol(&mut self, t: usize, n: Option<usize>) {
        if let Some(n) = n {
            self.tracks[t].nsd.env_voice = n;
        }
        self.tracks[t].nsd.sw_evol = n.is_some();
        let e = self.address(n, op::ENV_VOLUME, None);
        self.set_event(t, e);
    }

    fn env_em(&mut self, t: usize, n: Option<usize>) {
        if let Some(n) = n {
            self.tracks[t].nsd.env_frequency = n;
        }
        self.tracks[t].nsd.sw_em = n.is_some();
        let e = self.address(n, op::ENV_FREQUENCY, None);
        self.set_event(t, e);
    }

    fn env_en(&mut self, t: usize, n: Option<usize>) {
        if let Some(n) = n {
            self.tracks[t].nsd.env_note = n;
        }
        self.tracks[t].nsd.sw_en = n.is_some();
        let e = self.address(n, op::ENV_NOTE, None);
        self.set_event(t, e);
    }

    fn set_voice(&mut self, t: usize, n: i32) {
        self.tracks[t].nsd.voice = n;
        self.tracks[t].nsd.sw_evoi = false;
        self.ev(t, &[op::VOICE, n as u8]);
    }

    fn set_transpose(&mut self, t: usize, n: i32) {
        self.tracks[t].nsd.trans = n;
        self.ev(t, &[op::TRANSPOSE, n as u8]);
    }

    fn set_q(&mut self, t: usize, i: i32) {
        self.tracks[t].nsd.gate_q = i;
        if (0..=15).contains(&i) {
            self.ev(t, &[op::GATE_BYTE + i as u8]);
        } else {
            self.ev(t, &[op::GATE_Q, i as u8]);
        }
    }

    fn set_u(&mut self, t: usize, i: i32) {
        self.tracks[t].nsd.gate_u = i;
        self.ev(t, &[op::GATE_U, i as u8]);
    }

    fn octave_inc(&mut self, t: usize) {
        self.ev(t, &[op::OCTAVE_UP]);
        if self.tracks[t].nsd.octave < 10 {
            self.tracks[t].nsd.octave += 1;
        }
    }

    fn octave_dec(&mut self, t: usize) {
        self.ev(t, &[op::OCTAVE_DOWN]);
        if self.tracks[t].nsd.octave > 0 {
            self.tracks[t].nsd.octave -= 1;
        }
    }

    fn key_signature(&mut self, t: usize) -> R<()> {
        use tables::ks::*;
        let mut sign: i8 = 0;
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                return Ok(());
            }
            self.mml.back();
            let tr_key = |me: &mut Self, key: i32, scale: i32| me.tracks[t].set_key(key, scale);
            match self.mml.get_command_id(TABLE) {
                Some(k @ ks_c..=ks_r) => self.tracks[t].key_sig[(k - ks_c) as usize] = sign,
                Some(ks_Natural) => sign = 0,
                Some(ks_Sharp) => sign = 1,
                Some(ks_Flat) => sign = -1,
                Some(ks_0) => tr_key(self, 0, 0),
                Some(k @ ks_s1..=ks_s7) => tr_key(self, (k - ks_s1 + 1) as i32, 0),
                Some(k @ ks_f1..=ks_f7) => tr_key(self, -((k - ks_f1 + 1) as i32), 0),
                Some(ks_m0) => tr_key(self, 0, 5),
                Some(k @ ks_ms1..=ks_ms9) => tr_key(self, (k - ks_ms1 + 1) as i32, 5),
                Some(k @ ks_mf1..=ks_mf6) => tr_key(self, -((k - ks_mf1 + 1) as i32), 5),
                _ => return self.mml.err("an unknown symbol in K{}"),
            }
        }
    }

    fn set_length(&mut self, t: usize) -> R<()> {
        let l = self.mml.get_length(self.tracks[t].nsd.length)?;
        self.tracks[t].nsd.length = l;
        self.tracks[t].opt_default_length = l;
        const TBL: [i32; 16] = [96, 72, 48, 36, 32, 24, 18, 16, 12, 9, 8, 6, 4, 3, 2, 1];
        match TBL.iter().position(|&x| x == l) {
            Some(i) => self.ev(t, &[op::LENGTH_96 + i as u8]),
            None => self.ev(t, &[op::LENGTH, l as u8]),
        }
        Ok(())
    }

    fn calc_note(&mut self, t: usize, note: usize) -> R<i8> {
        const CODE: [i8; 7] = [0, 2, 4, 5, 7, 9, 11];
        let mut c = self.mml.get_char()?;
        let mut key = CODE[note];
        if c != b'=' && c != b'*' {
            key = key.wrapping_add(self.tracks[t].key_sig[note]);
            while c == b'#' || c == b'+' || c == b'-' {
                if c == b'-' {
                    key = key.wrapping_sub(1);
                } else {
                    key = key.wrapping_add(1);
                }
                c = self.mml.get_char()?;
            }
            self.mml.back();
        }
        Ok(key)
    }

    fn calc_length(&mut self, t: usize) -> R<i32> {
        let c = self.mml.get_char()?;
        self.mml.back();
        if c.is_ascii_digit() || c == b'%' || c == b'.' || c == b'~' || (self.mml.tie_mode == 1 && c == b'^') {
            self.mml.get_length(self.tracks[t].nsd.length)
        } else {
            Ok(-1)
        }
    }

    fn calc_gate(&mut self, t: usize) -> R<i32> {
        if self.mml.get_char()? == b',' {
            if self.mml.get_char()? == b'0' {
                Ok(0)
            } else {
                self.mml.back();
                self.mml.get_length(self.tracks[t].nsd.length)
            }
        } else {
            self.mml.back();
            Ok(-1)
        }
    }

    fn calc_slur(&mut self) -> R<bool> {
        if self.mml.get_char()? == b'&' {
            Ok(true)
        } else {
            self.mml.back();
            Ok(false)
        }
    }

    fn set_echo(&mut self, t: usize) -> R<()> {
        self.echo_vol_ret(t);
        let value = self.mml.get_int_range("EC's 1st parameter", 0, 255)?;
        self.mml.chk_comma()?;
        let volume = self.mml.get_int_range("EC's 2nd parameter", -1, 15)?;
        let tr = &mut self.tracks[t];
        tr.echo_flag = true;
        tr.echo_value = value as u8;
        if volume == -1 {
            tr.echo_slur = true;
        } else {
            tr.echo_slur = false;
            tr.echo_volume = volume as u8;
        }
        if self.mml.get_char()? != b',' {
            self.mml.back();
            self.tracks[t].echo_length = -1;
        } else {
            let l = self.mml.get_length(self.tracks[t].nsd.length)?;
            self.tracks[t].echo_length = l;
        }
        Ok(())
    }

    fn echo_buffer(&mut self, t: usize, note: i32) -> R<()> {
        let n: i32 = if note == -1 {
            -1
        } else {
            let mut k = i32::from(self.calc_note(t, note as usize)?);
            let c = self.mml.get_char()?;
            self.mml.back();
            let o = if c.is_ascii_digit() { self.mml.get_int()? - 1 } else { self.tracks[t].nsd.octave };
            k += self.tracks[t].key_shift;
            k += o * 12;
            self.mml.chk_range("the note number", 0, 255, k)?;
            k
        };
        let tr = &mut self.tracks[t];
        tr.pt_old = tr.pt_old.wrapping_add(1);
        tr.old_notes[usize::from(tr.pt_old)] = n as i8;
        Ok(())
    }

    fn echo_vol_ret(&mut self, t: usize) {
        if self.tracks[t].echo_vol_ret {
            let v = self.tracks[t].nsd.volume as u8;
            self.ev(t, &[op::VOLUME.wrapping_add(v)]);
            self.tracks[t].echo_vol_ret = false;
        }
    }

    fn note_event(&mut self, t: usize, code: i32, length: i32, gate: i32, slur: bool) -> R<usize> {
        if !(0..=15).contains(&code) {
            return self.mml.err("a note out of range (mml_note::mml_note())");
        }
        let mut opc = op::NOTE | code as u8;
        let mut c = vec![0u8];
        if slur {
            opc |= op::NOTE_SLUR;
        }
        if length != -1 {
            opc |= op::NOTE_LENGTH;
            c.push(length as u8);
        }
        if gate != -1 {
            opc |= op::NOTE_GATE;
            c.push(gate as u8);
        }
        c[0] = opc;
        let e = self.new_item(c, Kind::Note, None);
        self.items[e].used = true;
        self.set_event(t, e);
        self.tracks[t].old_note = Some(e);
        Ok(e)
    }

    fn generate_echo(&mut self, t: usize, mut length: i32, gate: i32, slur: bool) -> R<()> {
        let tr = &self.tracks[t];
        let old_note = tr.old_notes[usize::from(tr.pt_old.wrapping_sub(tr.echo_value))];
        let now_octave = tr.nsd.octave as i8;
        let mut old_octave = old_note / 12;
        self.call_patch(t, old_note);
        if !self.tracks[t].echo_slur {
            let v = self.tracks[t].echo_volume;
            self.ev(t, &[op::VOLUME.wrapping_add(v)]);
            self.tracks[t].echo_vol_ret = true;
        } else if !self.tracks[t].echo_already {
            if let Some(n) = self.tracks[t].old_note {
                self.items[n].code[0] |= op::NOTE_SLUR;
            }
        }
        while old_octave < now_octave {
            old_octave += 1;
            self.ev(t, &[op::OCTAVE_DOWN_1]);
        }
        while old_octave > now_octave {
            old_octave -= 1;
            self.ev(t, &[op::OCTAVE_UP_1]);
        }
        if length == self.tracks[t].opt_default_length {
            length = -1;
        }
        self.note_event(t, i32::from(old_note % 12), length, gate, slur)?;
        self.tracks[t].echo_already = true;
        Ok(())
    }

    fn set_note_with(&mut self, t: usize, mut key: i32, length: i32, gate: i32, slur: bool) -> R<()> {
        let mut length0 = length;
        if length0 == -1 {
            length0 = self.tracks[t].nsd.length;
        }
        let (bq, qmax) = (self.tracks[t].gatetime_big_q, self.tracks[t].qmax);
        if bq != qmax {
            self.set_q(t, (length0 * (qmax - bq)) / qmax);
        }
        key += self.tracks[t].key_shift;
        let tr = &mut self.tracks[t];
        let note_no = ((key + (tr.nsd.octave + tr.nsd.octave1) * 12) & 0xff) as u8 as i8;
        tr.pt_old = tr.pt_old.wrapping_add(1);
        tr.old_notes[usize::from(tr.pt_old)] = note_no;
        tr.nsd.octave1 = 0;
        tr.echo_already = false;
        if tr.echo_flag {
            self.echo_vol_ret(t);
        }
        while key < 0 {
            key += 12;
            self.ev(t, &[op::OCTAVE_DOWN_1]);
        }
        while key >= 12 {
            key -= 12;
            self.ev(t, &[op::OCTAVE_UP_1]);
        }
        self.call_patch(t, note_no);
        let tr = &self.tracks[t];
        let echo_src = tr.old_notes[usize::from(tr.pt_old.wrapping_sub(tr.echo_value))];
        if tr.echo_flag && echo_src != -1 && tr.echo_length != -1 && length0 > tr.echo_length && !slur {
            length0 -= tr.echo_length;
            if length0 == tr.opt_default_length {
                length0 = -1;
            }
            let el = tr.echo_length;
            self.note_event(t, key, length0, gate, slur)?;
            self.generate_echo(t, el, gate, slur)?;
        } else {
            let mut length = length;
            if length == tr.opt_default_length {
                length = -1;
            }
            self.note_event(t, key, length, gate, slur)?;
        }
        Ok(())
    }

    fn set_note(&mut self, t: usize, note: usize) -> R<()> {
        let key = i32::from(self.calc_note(t, note)?);
        let length = self.calc_length(t)?;
        let gate = self.calc_gate(t)?;
        let slur = self.calc_slur()?;
        if !self.tracks[t].jump_flag {
            self.set_note_with(t, key, length, gate, slur)?;
        }
        Ok(())
    }

    fn set_rest(&mut self, t: usize, mode: i32) -> R<()> {
        let tr = &self.tracks[t];
        let old_note = tr.old_notes[usize::from(tr.pt_old.wrapping_sub(tr.echo_value))];
        let code: i32 = match self.mml.get_char()? {
            b'-' => 0x0d,
            b'#' | b'+' => 0x0e,
            b'=' => 0x0f,
            b'*' => 0x0c,
            _ => {
                self.mml.back();
                if mode & 0x80 != 0 {
                    match mode {
                        0x80 => 0x0d,
                        0x81 => 0x0e,
                        0x82 => 0x0f,
                        _ => 0x0c,
                    }
                } else {
                    match self.tracks[t].key_sig[7] {
                        -1 => 0x0d,
                        1 => 0x0e,
                        _ => match mode {
                            0 => 0x0d,
                            1 => 0x0e,
                            2 => 0x0f,
                            _ => 0x0c,
                        },
                    }
                }
            }
        };
        let mut length = self.calc_length(t)?;
        let gate = self.calc_gate(t)?;
        let slur = self.calc_slur()?;
        if !self.tracks[t].jump_flag {
            let (bq, qmax) = (self.tracks[t].gatetime_big_q, self.tracks[t].qmax);
            if bq != qmax {
                self.set_q(t, (length * (qmax - bq)) / qmax);
            }
            let tr = &self.tracks[t];
            if !tr.echo_flag || code != 0x0f || old_note == -1 || tr.echo_length != -1 {
                if length == tr.opt_default_length {
                    length = -1;
                }
                self.note_event(t, code, length, gate, slur)?;
            } else {
                self.generate_echo(t, length, gate, slur)?;
            }
        }
        Ok(())
    }

    fn note_letter(&mut self) -> R<usize> {
        Ok(match self.mml.get_char()? {
            b'c' => 0,
            b'd' => 1,
            b'e' => 2,
            b'f' => 3,
            b'g' => 4,
            b'a' => 5,
            b'b' => 6,
            _ => return self.mml.err("a note a, b, c to g is needed"),
        })
    }

    // `{c e}`: a portamento from one note to the other over the note's length.
    fn portamento2(&mut self, t: usize, tempo: u8) -> R<()> {
        let note = self.note_letter()?;
        let key = self.calc_note(t, note)?;
        let mut length = self.calc_length(t)?;
        let mut gate = self.calc_gate(t)?;
        let mut slur = self.calc_slur()?;
        let n = self.tracks[t].nsd;
        let i_note = (i32::from(key) + (n.octave + n.octave1) * 12) & 0xff;
        let (mut o_rel, mut o_rel2) = (0, 0);
        loop {
            match self.mml.get_char()? {
                b'<' => o_rel -= 1,
                b'>' => o_rel += 1,
                b'`' => o_rel2 += 1,
                b'"' => o_rel2 -= 1,
                _ => break,
            }
        }
        self.mml.back();
        let note2 = self.note_letter()?;
        let key2 = self.calc_note(t, note2)?;
        let i_note2 = (i32::from(key2) + (n.octave + n.octave1 + o_rel + o_rel2) * 12) & 0xff;
        if self.mml.get_char()? != b'}' {
            return self.mml.err("the portamento must end with }");
        }
        let length2 = self.calc_length(t)?;
        if length2 != -1 {
            if length != -1 {
                return self.mml.err("the length is given twice in a portamento");
            }
            length = length2;
        }
        let gate2 = self.calc_gate(t)?;
        if gate2 != -1 {
            if gate != -1 {
                return self.mml.err("the gate time is given twice in a portamento");
            }
            gate = gate2;
        }
        if self.calc_slur()? {
            slur = true;
        }
        let mut length0 = length;
        if length0 == -1 {
            length0 = self.tracks[t].nsd.length;
        }
        let pol_length = (length0 * 150) / i32::from(tempo);
        let target = i_note2 - i_note;
        // nsc divides by these unchecked: a crash on x86, 0 on ARM (where the reference runs); 0 here.
        let div = |a: i32, b: i32| a.checked_div(b).unwrap_or(0);
        let (rate, depth);
        if target.abs() * 16 >= pol_length {
            rate = 1;
            let mut d = div(target * 16, pol_length);
            if target >= 0 {
                d += 1;
            } else {
                d -= 1;
            }
            depth = d;
        } else if target < 0 {
            rate = div(pol_length, -target * 16);
            depth = -1;
        } else {
            rate = div(pol_length, target * 16);
            depth = 1;
        }
        if !self.tracks[t].jump_flag {
            self.ev(t, &[op::PORTAMENTO, 1, rate as u8, depth as u8, target as u8]);
            self.set_note_with(t, i32::from(key), length, gate, slur)?;
        }
        while o_rel > 0 {
            if self.mml.octave_reverse {
                self.octave_dec(t);
            } else {
                self.octave_inc(t);
            }
            o_rel -= 1;
        }
        while o_rel < 0 {
            if self.mml.octave_reverse {
                self.octave_inc(t);
            } else {
                self.octave_dec(t);
            }
            o_rel += 1;
        }
        Ok(())
    }

    // ---- Tick counting: the driver's run through a block, marking the commands that change something ----

    pub fn set_tick_count(&mut self, s: usize) -> R<()> {
        for n in 0..=self.sets[s].max_track {
            let t = self.sets[s].tracks[&n];
            self.tracks[t].nsd = Work::default();
            self.track_tick_count(t, 0)?;
        }
        Ok(())
    }

    fn use_item(&mut self, i: usize) {
        self.items[i].used = true;
    }

    fn track_tick_count(&mut self, t: usize, depth: usize) -> R<i32> {
        const TBL: [i32; 16] = [96, 72, 48, 36, 32, 24, 18, 16, 12, 9, 8, 6, 4, 3, 2, 1];
        if depth > 64 {
            return self.mml.err("subroutines call each other without end");
        }
        let kids = self.items[self.tracks[t].item].kids.clone();
        let mut loop_start = 0usize;
        let mut loop_count = 0;
        let (mut ra_start, mut ra_end, mut ra_count) = (0usize, 0usize, 0i32);
        let (mut rb_start, mut rb_end, mut rb_count) = (0usize, 0usize, 0i32);
        let (mut no_sub, mut sw_sub) = (0usize, false);
        let mut ticks = 0i32;
        self.tracks[t].tick_total = 0;
        self.tracks[t].tick_loop = 0;
        let mut i = 0usize;
        while i < kids.len() {
            let x = kids[i];
            let tr = &self.tracks[t];
            if tr.is_loop && Some(&x) == tr.ptc_loop.get(&0) {
                loop_start = i;
                if loop_count == 0 {
                    self.tracks[t].tick_loop = ticks;
                }
                self.use_item(x);
                i += 1;
                continue;
            }
            if self.items[x].size == 0 {
                self.use_item(x);
                i += 1;
                continue;
            }
            let code = self.items[x].get_code(0);
            let c1 = self.items[x].get_code(1);
            let c2 = self.items[x].get_code(2);
            let id = self.items[x].id;
            let f_id = self.items[x].f_id;
            let used = self.items[x].used;
            macro_rules! changes {
                ($field:ident, $v:expr) => {{
                    let v = $v;
                    if self.tracks[t].nsd.$field != v {
                        self.tracks[t].nsd.$field = v;
                        self.use_item(x);
                    }
                }};
            }
            if code < 0x30 {
                match code {
                    op::END_OF_TRACK => {
                        self.tracks[t].tick_total = ticks;
                        self.use_item(x);
                    }
                    op::JUMP => {
                        if !used {
                            if !self.tracks[t].ptc_loop.contains_key(&id) {
                                return self.mml.err(format!("L({id}) isn't there"));
                            }
                            self.use_item(x);
                            self.tracks[t].vec_loop_end.push(x);
                        }
                        if loop_count == 0 && self.tracks[t].is_loop {
                            self.tracks[t].tick_total = ticks;
                            i = loop_start;
                            loop_count += 1;
                        }
                    }
                    op::CALL => {
                        let (patch, no_opt) = match self.items[x].kind {
                            Kind::CallSub { patch, no_opt } => (patch, no_opt),
                            _ => (false, false),
                        };
                        if no_sub != id || !sw_sub || !self.opt_seq || no_opt || !patch {
                            if patch {
                                no_sub = id;
                                sw_sub = true;
                            }
                            let Some(&sset) = self.sub.get(&id) else {
                                return self.mml.err(format!("Sub({id}) isn't there"));
                            };
                            let st = self.sets[sset].tracks[&0];
                            let work = self.tracks[t].nsd;
                            self.tracks[st].nsd = work;
                            ticks += self.track_tick_count(st, depth + 1)?;
                            self.tracks[t].nsd = self.tracks[st].nsd;
                            let si = self.sets[sset].item;
                            self.use_item(si);
                            if !self.items[x].used {
                                self.use_item(x);
                                self.tracks[t].vec_sub.push(x);
                            }
                        }
                    }
                    op::CALL_SE => {
                        if !used {
                            if !self.se.contains_key(&id) {
                                return self.mml.err(format!("SE({id}) isn't there"));
                            }
                            self.use_item(x);
                            self.tracks[t].vec_se.push(x);
                        }
                    }
                    op::REPEAT_A_START => {
                        ra_count = i32::from(c1);
                        ra_start = i;
                        self.use_item(x);
                    }
                    op::REPEAT_A_BRANCH => {
                        if !used {
                            if !self.tracks[t].ptc_rep_a_e.contains_key(&id) {
                                return self.mml.err(format!("](id {id}) isn't there"));
                            }
                            self.use_item(x);
                            self.tracks[t].vec_rep_a_branch.push(x);
                        }
                        if ra_count == 1 {
                            i = ra_end;
                        }
                    }
                    op::REPEAT_A_END => {
                        if !used {
                            if !self.tracks[t].ptc_rep_a.contains_key(&id) {
                                return self.mml.err(format!("[(id {id}) isn't there"));
                            }
                            self.use_item(x);
                            self.tracks[t].vec_rep_a_end.push(x);
                        }
                        ra_end = i;
                        ra_count -= 1;
                        if ra_count != 0 {
                            i = ra_start;
                        }
                    }
                    op::REPEAT_B_START => {
                        rb_start = i;
                        rb_count = 0;
                        self.use_item(x);
                    }
                    op::REPEAT_B_BRANCH => {
                        if rb_count != 0 {
                            i = rb_end;
                        } else {
                            self.use_item(x);
                        }
                    }
                    op::REPEAT_B_END => {
                        if !used {
                            if !self.tracks[t].ptc_rep_b.contains_key(&id) {
                                return self.mml.err(format!("|:(id {id}) isn't there"));
                            }
                            self.use_item(x);
                            self.tracks[t].vec_rep_b_end.push(x);
                        }
                        rb_end = i;
                        i = rb_start;
                        rb_count += 1;
                    }
                    op::LENGTH => changes!(length, i32::from(c1)),
                    op::GATE_Q => changes!(gate_q, i32::from(c1)),
                    op::GATE_U => changes!(gate_u, i32::from(c1)),
                    op::GATE_MODE_0 => changes!(gatemode, 0),
                    op::GATE_MODE_1 => changes!(gatemode, 1),
                    op::GATE_MODE_2 => changes!(gatemode, 2),
                    op::VOICE => {
                        let v = i32::from(c1);
                        let n = &mut self.tracks[t].nsd;
                        if n.voice != v || n.sw_evoi {
                            self.items[x].used = true;
                        }
                        let n = &mut self.tracks[t].nsd;
                        n.sw_evoi = false;
                        n.voice = v;
                    }
                    op::ENV_VOICE | op::ENV_VOLUME | op::ENV_FREQUENCY | op::ENV_NOTE => {
                        let n = self.tracks[t].nsd;
                        let (cur, sw) = match code {
                            op::ENV_VOICE => (n.env_voice, n.sw_evoi),
                            op::ENV_VOLUME => (n.env_volume, n.sw_evol),
                            op::ENV_FREQUENCY => (n.env_frequency, n.sw_em),
                            _ => (n.env_note, n.sw_en),
                        };
                        if f_id {
                            if cur != id || !sw || !self.opt_seq {
                                let n = &mut self.tracks[t].nsd;
                                match code {
                                    op::ENV_VOICE => n.env_voice = id,
                                    op::ENV_VOLUME => n.env_volume = id,
                                    op::ENV_FREQUENCY => n.env_frequency = id,
                                    _ => n.env_note = id,
                                }
                                self.tick_envelope(t, x, id)?;
                            }
                        } else if sw {
                            self.use_item(x);
                        }
                        let n = &mut self.tracks[t].nsd;
                        match code {
                            op::ENV_VOICE => n.sw_evoi = f_id,
                            op::ENV_VOLUME => n.sw_evol = f_id,
                            op::ENV_FREQUENCY => n.sw_em = f_id,
                            _ => n.sw_en = f_id,
                        }
                    }
                    op::SWEEP => changes!(sweep, i32::from(c1 as i8)),
                    op::VRC7 | op::N163 | op::FDS_CAREER | op::FDS_MODULATOR => {
                        let n = self.tracks[t].nsd;
                        let (cur, sw) = match code {
                            op::VRC7 => (n.vrc7_voice, n.sw_vrc7_voice),
                            op::N163 => (n.n163_voice, n.sw_n163_voice),
                            op::FDS_CAREER => (n.fds_career, n.sw_fds_career),
                            _ => (n.fds_modulator, n.sw_fds_modulator),
                        };
                        if cur != id || !sw || !self.opt_seq {
                            let n = &mut self.tracks[t].nsd;
                            match code {
                                op::VRC7 => {
                                    n.vrc7_voice = id;
                                    n.sw_vrc7_voice = true;
                                }
                                op::N163 => {
                                    n.n163_voice = id;
                                    n.sw_n163_voice = true;
                                }
                                op::FDS_CAREER => {
                                    n.fds_career = id;
                                    n.sw_fds_career = true;
                                }
                                _ => {
                                    n.fds_modulator = id;
                                    n.sw_fds_modulator = true;
                                }
                            }
                            if !self.items[x].used {
                                let (map, name) = match code {
                                    op::VRC7 => (&self.vrc7, "VRC7"),
                                    op::N163 => (&self.n163, "N163"),
                                    op::FDS_CAREER => (&self.fdsc, "FDSC"),
                                    _ => (&self.fdsm, "FDSM"),
                                };
                                let Some(&d) = map.get(&id) else {
                                    return self.mml.err(format!("{name}({id}) isn't there"));
                                };
                                self.use_item(d);
                                self.use_item(x);
                                let tr = &mut self.tracks[t];
                                match code {
                                    op::VRC7 => tr.vec_opll.push(x),
                                    op::N163 => tr.vec_wave.push(x),
                                    op::FDS_CAREER => tr.vec_fdsc.push(x),
                                    _ => tr.vec_fdsm.push(x),
                                }
                            }
                        }
                    }
                    op::VOLUME_DOWN => {
                        let n = &mut self.tracks[t].nsd;
                        n.volume = (n.volume - 1).max(0);
                        self.use_item(x);
                    }
                    op::VOLUME_UP => {
                        let n = &mut self.tracks[t].nsd;
                        n.volume = (n.volume + 1).min(15);
                        self.use_item(x);
                    }
                    op::FDS_FREQUENCY => changes!(fds_frequency, i32::from(c1) + (i32::from(c2) << 8)),
                    op::FDS_VOLUME => changes!(fds_volume, i32::from(c1)),
                    op::FME7_FREQUENCY => changes!(psg_frequency, i32::from(c1) + (i32::from(c2) << 8)),
                    op::OCTAVE_DOWN => {
                        let n = &mut self.tracks[t].nsd;
                        n.octave = (n.octave - 1).max(0);
                        self.use_item(x);
                    }
                    op::OCTAVE_UP => {
                        let n = &mut self.tracks[t].nsd;
                        n.octave += 1;
                        if n.octave * 12 >= 128 {
                            n.octave -= 1;
                        }
                        self.use_item(x);
                    }
                    op::TRANSPOSE => changes!(trans, i32::from(c1 as i8)),
                    op::RELATIVE_TRANSPOSE => {
                        self.tracks[t].nsd.trans += i32::from(c1 as i8);
                        self.use_item(x);
                    }
                    op::DETUNE_CENT => changes!(detune_cent, i32::from(c1 as i8)),
                    op::DETUNE_REGISTER => changes!(detune_reg, i32::from(c1 as i8)),
                    op::SUB_COMMAND => {
                        match c1 {
                            op::SUB_DETUNE_CENT => self.tracks[t].nsd.detune_cent += i32::from(c2 as i8),
                            op::SUB_DETUNE_REGISTER => self.tracks[t].nsd.detune_reg += i32::from(c2 as i8),
                            _ => {}
                        }
                        self.use_item(x);
                    }
                    _ => self.use_item(x),
                }
            } else if code < 0x38 {
                changes!(voice_rel, i32::from(code & 7));
            } else if code < 0x40 {
                changes!(octave, i32::from(code & 7));
            } else if code < 0x50 {
                changes!(length, TBL[usize::from(code & 0x0f)]);
            } else if code < 0x60 {
                changes!(gate_q, i32::from(code & 0x0f));
            } else if code < 0x70 {
                changes!(volume, i32::from(code & 0x0f));
            } else if code < 0x80 {
                changes!(volume_rel, i32::from(code & 0x0f));
            } else {
                if code & 0x7f & op::NOTE_LENGTH != 0 {
                    ticks += i32::from(c1);
                } else {
                    ticks += self.tracks[t].nsd.length;
                }
                self.use_item(x);
            }
            i += 1;
        }
        if !self.tracks[t].is_loop {
            self.tracks[t].tick_loop = -1;
        }
        Ok(self.tracks[t].tick_total)
    }

    fn tick_envelope(&mut self, t: usize, x: usize, n: usize) -> R<()> {
        if !self.items[x].used {
            let Some(&e) = self.env.get(&n) else {
                return self.mml.err(format!("Envelope({n}) isn't there"));
            };
            self.use_item(e);
            self.use_item(x);
            self.tracks[t].vec_env.push(x);
        }
        Ok(())
    }

    // ---- Addresses ----

    fn set_address(&mut self, x: usize, addr: usize) {
        let it = &mut self.items[x];
        match it.size {
            3 => {
                it.code[1] = addr as u8;
                it.code[2] = (addr >> 8) as u8;
            }
            4 => {
                it.code[2] = addr as u8;
                it.code[3] = (addr >> 8) as u8;
            }
            _ => {}
        }
    }

    fn track_fix_address(&mut self, t: usize) -> R<()> {
        let rel = |me: &Self, x: usize, target: usize, back: usize| target.wrapping_sub(me.items[x].offset).wrapping_sub(back);
        let after = |me: &Self, i: usize| me.items[i].offset + me.items[i].size;
        for x in self.tracks[t].vec_loop_end.clone() {
            let id = self.items[x].id;
            let Some(&l) = self.tracks[t].ptc_loop.get(&id) else { return self.mml.err("L isn't there") };
            let a = rel(self, x, after(self, l), 1);
            self.set_address(x, a);
        }
        for x in self.tracks[t].vec_rep_a_end.clone() {
            let id = self.items[x].id;
            let Some(&l) = self.tracks[t].ptc_rep_a.get(&id) else { return self.mml.err("[ isn't there") };
            let a = rel(self, x, after(self, l), 1);
            self.set_address(x, a);
        }
        for x in self.tracks[t].vec_rep_a_branch.clone() {
            let id = self.items[x].id;
            let Some(&l) = self.tracks[t].ptc_rep_a_e.get(&id) else { return self.mml.err("] isn't there") };
            let a = rel(self, x, after(self, l), 1);
            self.set_address(x, a);
        }
        for x in self.tracks[t].vec_rep_b_end.clone() {
            let id = self.items[x].id;
            let Some(&l) = self.tracks[t].ptc_rep_b.get(&id) else { return self.mml.err("|: isn't there") };
            let a = rel(self, x, after(self, l), 1);
            self.set_address(x, a);
        }
        let lists: [(Vec<usize>, u8); 7] = [
            (self.tracks[t].vec_se.clone(), 0),
            (self.tracks[t].vec_sub.clone(), 1),
            (self.tracks[t].vec_env.clone(), 2),
            (self.tracks[t].vec_fdsc.clone(), 3),
            (self.tracks[t].vec_fdsm.clone(), 4),
            (self.tracks[t].vec_opll.clone(), 5),
            (self.tracks[t].vec_wave.clone(), 6),
        ];
        for (list, what) in lists {
            for x in list {
                let id = self.items[x].id;
                let target = match what {
                    0 => self.se.get(&id).map(|&s| self.sets[s].item),
                    1 => self.sub.get(&id).map(|&s| self.sets[s].item),
                    2 => self.env.get(&id).copied(),
                    3 => self.fdsc.get(&id).copied(),
                    4 => self.fdsm.get(&id).copied(),
                    5 => self.vrc7.get(&id).copied(),
                    _ => self.n163.get(&id).copied(),
                };
                let Some(target) = target else { return self.mml.err(format!("({id}) isn't there")) };
                let a = rel(self, x, self.items[target].offset, if what == 6 { 2 } else { 1 });
                self.set_address(x, a);
            }
        }
        Ok(())
    }

    pub fn set_fix_address(&mut self, s: usize) -> R<()> {
        let it = self.sets[s].item;
        if self.sets[s].sub {
            if self.items[it].size > 0 {
                let t = self.sets[s].tracks[&0];
                self.track_fix_address(t)?;
            }
            return Ok(());
        }
        let max = self.sets[s].max_track;
        for n in 0..=max {
            let t = self.sets[s].tracks[&n];
            self.track_fix_address(t)?;
        }
        let mut i = self.items[it].code.len();
        for n in 0..=max {
            let t = self.sets[s].tracks[&n];
            let size = self.items[self.tracks[t].item].size;
            let code = &mut self.items[it].code;
            if size == 0 {
                code[n * 2 + 2] = 0;
                code[n * 2 + 3] = 0;
            } else {
                code[n * 2 + 2] = i as u8;
                code[n * 2 + 3] = (i >> 8) as u8;
            }
            i += size;
        }
        Ok(())
    }
}
