// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The SunVox engine (sunvox_engine*.cpp): its patterns placed on the
//! timeline, the patterns playing at the line being played (each with its
//! own state for its tracks' effects), the commands their notes send to
//! the modules, and the render: ticks found inside each block of frames,
//! their events given the frame they fall on, then the network rendered
//! for the block. Its keyboard, MIDI and recording are left out.

use std::collections::VecDeque;
use std::sync::Arc;

use super::net::{self, Cmd, Event, Globals, Net, Smp};

pub(crate) const MAX_PLAYING_PATS: usize = 64;
pub(crate) const MAX_PATTERN_TRACKS: usize = 32;
const SUPERTRACK_WORDS: usize = MAX_PLAYING_PATS.div_ceil(32);

pub(crate) const NOTECMD_NOTE_OFF: u8 = 128;
pub(crate) const NOTECMD_ALL_NOTES_OFF: u8 = 129;
pub(crate) const NOTECMD_CLEAN_MODULES: u8 = 130;
pub(crate) const NOTECMD_STOP: u8 = 131;
pub(crate) const NOTECMD_PLAY: u8 = 132;
pub(crate) const NOTECMD_SET_PITCH: u8 = 133;
pub(crate) const NOTECMD_PREV_TRACK: u8 = 134;
pub(crate) const NOTECMD_PATPLAY_OFF: u8 = 135;
pub(crate) const NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP: u8 = 136;
pub(crate) const NOTECMD_JUMP: u8 = 137;
pub(crate) const NOTECMD_CLEAN_MODULE: u8 = 140;
pub(crate) const NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP2: u8 = 141;

pub(crate) const VIRTUAL_PATTERN: i32 = 0xFFFF;

pub(crate) const FLAG_CREATE_PATTERN: u32 = 1 << 0;
pub(crate) const FLAG_CREATE_MODULES: u32 = 1 << 1;
pub(crate) const FLAG_DONT_START: u32 = 1 << 2;
pub(crate) const FLAG_PLAYER_ONLY: u32 = 1 << 3;
pub(crate) const FLAG_NO_GUI: u32 = 1 << 4;
pub(crate) const FLAG_NO_SCOPE: u32 = 1 << 5;
pub(crate) const FLAG_NO_MIDI: u32 = 1 << 6;
pub(crate) const FLAG_NO_GLOBAL_SYS_EVENTS: u32 = 1 << 7;
pub(crate) const FLAG_NO_KBD_EVENTS: u32 = 1 << 8;
pub(crate) const FLAG_NO_MODULE_CHANNELS: u32 = 1 << 9;
pub(crate) const FLAG_STATIC_TIMELINE: u32 = 1 << 10;
pub(crate) const FLAG_DYNAMIC_PATTERN_STATE: u32 = 1 << 11;
pub(crate) const FLAG_ONE_THREAD: u32 = 1 << 12;
pub(crate) const FLAG_MAIN: u32 = 1 << 13;
pub(crate) const FLAG_IGNORE_SLOT_SYNC: u32 = 1 << 14;
pub(crate) const FLAG_SUPERTRACKS: u32 = 1 << 15;
pub(crate) const FLAG_NO_TONE_PORTA_ON_TICK0: u32 = 1 << 16;
pub(crate) const FLAG_NO_VOL_SLIDE_ON_TICK0: u32 = 1 << 17;
pub(crate) const FLAG_KBD_ROUNDROBIN: u32 = 1 << 18;
pub(crate) const FLAG_IGNORE_EFF31: u32 = 1 << 20;

pub(crate) const PAT_INFO_FLAG_CLONE: u32 = 1 << 0;
pub(crate) const PAT_INFO_FLAG_MUTE: u32 = 1 << 3;
pub(crate) const PAT_INFO_FLAG_SOLO: u32 = 1 << 4;
pub(crate) const PAT_INFO_FLAG_JUST_LOADED: u32 = 1 << 30;
pub(crate) const PAT_FLAG_NO_NOTES_OFF: u32 = 1 << 1;

const EFF_FLAG_TONE_PORTA: u16 = 1 << 0;
const EFF_FLAG_TONE_VIBRATO: u16 = 1 << 1;
const EFF_FLAG_ARPEGGIO_IN_PREVIOUS_TICK: u16 = 1 << 2;
const EFF_FLAG_TIMER_RETRIG: u16 = 1 << 3;
const EFF_FLAG_TIMER_CUT: u16 = 1 << 4;
const EFF_FLAG_TIMER_CUT2: u16 = 1 << 5;
const EFF_FLAG_TIMER_DELAY: u16 = 1 << 6;
const EFF_FLAG_TIMER_HANDLING: u16 = 1 << 7;
const EFF_FLAG_EVT_PITCH: u16 = 1 << 8;
const EFF_FLAG_PITCH_CHANGED: u16 = 1 << 9;
const EFF_FLAG_VEL_CHANGED: u16 = 1 << 10;

/// A cell of a pattern (sunvox_note).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    /// 0 nothing, 1..127 a note + 1, 128 note off, 129.. a NOTECMD.
    pub note: u8,
    /// 1..129 a velocity + 1, 0 the default.
    pub vel: u8,
    /// A module + 1, 0 none.
    pub module: u16,
    /// 0xCCEE: a controller + 1 (or a MIDI one + 128) and an effect.
    pub ctl: u16,
    pub ctl_val: u16,
}

impl Note {
    pub fn from_bytes(b: &[u8]) -> Note {
        Note { note: b[0], vel: b[1], module: u16::from_le_bytes([b[2], b[3]]), ctl: u16::from_le_bytes([b[4], b[5]]), ctl_val: u16::from_le_bytes([b[6], b[7]]) }
    }
}

/// A pattern's data (sunvox_pattern), shared by its clones.
#[derive(Clone, Default, Debug)]
pub(crate) struct Pattern {
    pub data: Vec<Note>,
    pub data_xsize: i32,
    pub data_ysize: i32,
    pub id: u32,
    pub channels: i32,
    pub lines: i32,
    pub ysize: i32,
    pub flags: u32,
    pub name: Option<String>,
}

/// Where a pattern sits (sunvox_pattern_info).
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct PatInfo {
    pub flags: u32,
    pub parent_num: i32,
    pub x: i32,
    pub y: i32,
    pub state_ptr: usize,
    pub track_status: u32,
}

/// A pattern slot: empty, a clone not yet tied to its parent, or a pattern's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Empty,
    Placeholder,
    Data(usize),
}

#[derive(Clone, Copy, Default, Debug)]
struct TrackEff {
    evt_pitch0: i32,
    evt_pitch: i32,
    cur_pitch: i32,
    target_pitch: i32,
    porta_speed: u16,
    flags: u16,
    cur_vel: i16,
    vel_speed: i16,
    arpeggio: u16,
    vib_amp: u8,
    vib_freq: u8,
    vib_phase: i32,
    timer: u8,
    timer_init: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PatState {
    effects: [TrackEff; MAX_PATTERN_TRACKS],
    track_status: u32,
    track_module: [i16; MAX_PATTERN_TRACKS],
    busy: bool,
    mutable_tracks: u8,
}

impl Default for PatState {
    fn default() -> PatState {
        PatState { effects: [TrackEff::default(); MAX_PATTERN_TRACKS], track_status: 0, track_module: [0; MAX_PATTERN_TRACKS], busy: false, mutable_tracks: 0 }
    }
}

/// clean_pattern_state.
fn clean_pattern_state(st: &mut PatState) {
    for e in st.effects.iter_mut() {
        e.flags = 0;
        e.vel_speed = 0;
        e.vib_amp = 0;
        e.porta_speed = 256 * 4;
    }
    st.track_status = 0;
    st.track_module = [-1; MAX_PATTERN_TRACKS];
}

/// sunvox_reset_track_effect.
fn reset_track_effect(e: &mut TrackEff) {
    e.flags &= EFF_FLAG_ARPEGGIO_IN_PREVIOUS_TICK;
    e.vel_speed = 0;
    e.arpeggio = 0;
}

/// A command from outside the patterns (sunvox_user_cmd).
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct UserCmd {
    pub n: Note,
    pub ch: u8,
}

/// sunvox_engine, as far as playing goes.
pub(crate) struct Engine {
    pub flags: u32,
    pub freq: i32,
    pub base_version: u32,
    pub playing: bool,
    pub single_pattern_play: i32,
    pub next_single_pattern_play: i32,
    pub restart_pos: i32,
    pub stop_at_the_end_of_proj: bool,
    pub tick_counter: u32,
    pub line_counter: i32,
    pub speed_counter: i32,
    pub jump_request: bool,
    pub jump_request_mode: u8,
    pub jump_request_line: i32,
    pub prev_bpm: u16,
    pub prev_speed: u8,
    pub bpm: u16,
    pub speed: u8,
    pub tgrid: u8,
    pub tgrid2: u8,
    pub proj_name: String,
    pub sorted_pats: Vec<usize>,
    pub cur_playing_pats: [i32; MAX_PLAYING_PATS],
    temp_pats: [i32; MAX_PLAYING_PATS],
    last_sort_pat: i32,
    pub proj_lines: i32,
    pub proj_len: u32,
    pat_state: Vec<PatState>,
    pub supertrack_mute: [u32; SUPERTRACK_WORDS],
    pub pats_solo_mode: bool,
    /// Pattern slots (pats_num of them) and the patterns' data.
    pub pats: Vec<Slot>,
    pub pats_info: Vec<PatInfo>,
    pub pat_data: Vec<Pattern>,
    pat_id_counter: u32,
    pub net: Net,
    virtual_pat_state: PatState,
    virtual_pat_info: PatInfo,
    virtual_pat_tracks: i32,
    pub user_commands: VecDeque<UserCmd>,
    /// Events from another network (a MetaModule's host) for this one's modules.
    pub psynth_events: Vec<(u16, Event)>,
    pub pitch_offset: i32,
    pub velocity: i32,
    rand_next: u32,
    level1_offset: i32,
    pub change_counter: i32,
}

impl Engine {
    /// sunvox_engine_init.
    pub fn new(flags: u32, freq: i32, globals: Arc<Globals>) -> Engine {
        let base_version = SUNVOX_ENGINE_VERSION;
        let mut net_flags = 0;
        if flags & FLAG_MAIN != 0 {
            net_flags |= net::NET_FLAG_MAIN;
        }
        if flags & FLAG_NO_MODULE_CHANNELS != 0 {
            net_flags |= net::NET_FLAG_NO_MODULE_CHANNELS;
        }
        // pat_id_counter = stime_ms() + ( pseudo_random() << 16 ), the clock at 0.
        let pat_id_counter = globals.pseudo_random() << 16;
        let net = Net::new(net_flags, freq, 125, 6, base_version, globals);
        let mut s = Engine {
            flags,
            freq,
            base_version,
            playing: false,
            single_pattern_play: -1,
            next_single_pattern_play: -1,
            restart_pos: 0,
            stop_at_the_end_of_proj: false,
            tick_counter: 0,
            line_counter: 0,
            speed_counter: 0,
            jump_request: false,
            jump_request_mode: 0,
            jump_request_line: 0,
            prev_bpm: 0,
            prev_speed: 0,
            bpm: 125,
            speed: 6,
            tgrid: 4,
            tgrid2: 4,
            proj_name: String::new(),
            sorted_pats: Vec::new(),
            cur_playing_pats: [-1; MAX_PLAYING_PATS],
            temp_pats: [0; MAX_PLAYING_PATS],
            last_sort_pat: 0,
            proj_lines: 0,
            proj_len: 0,
            pat_state: Vec::new(),
            supertrack_mute: [0; SUPERTRACK_WORDS],
            pats_solo_mode: false,
            pats: Vec::new(),
            pats_info: Vec::new(),
            pat_data: Vec::new(),
            pat_id_counter,
            net,
            virtual_pat_state: PatState::default(),
            virtual_pat_info: PatInfo::default(),
            virtual_pat_tracks: 0,
            user_commands: VecDeque::new(),
            psynth_events: Vec::new(),
            pitch_offset: 0,
            velocity: 256,
            // s->rand_next = stime_ms(): the clock at 0.
            rand_next: 0,
            level1_offset: 0,
            change_counter: 0,
        };
        clean_pattern_state(&mut s.virtual_pat_state);
        if flags & FLAG_DYNAMIC_PATTERN_STATE == 0 {
            s.pat_state = vec![PatState::default(); MAX_PLAYING_PATS];
            for st in s.pat_state.iter_mut() {
                clean_pattern_state(st);
            }
        }
        if flags & FLAG_CREATE_PATTERN != 0 {
            let lines = s.tgrid as i32 * s.tgrid2 as i32 * 2;
            s.new_pattern(lines, 4, 0, 0);
        }
        s.sort_patterns();
        s
    }

    pub fn pat(&self, slot: usize) -> Option<&Pattern> {
        match self.pats.get(slot) {
            Some(Slot::Data(d)) => self.pat_data.get(*d),
            _ => None,
        }
    }

    fn pat_mut(&mut self, slot: usize) -> Option<&mut Pattern> {
        match self.pats.get(slot) {
            Some(Slot::Data(d)) => self.pat_data.get_mut(*d),
            _ => None,
        }
    }

    fn has_pat(&self, slot: usize) -> bool {
        self.pat(slot).is_some()
    }

    // Patterns.

    /// sunvox_get_free_pattern_num.
    pub fn get_free_pattern_num(&mut self) -> usize {
        if self.pats.is_empty() {
            self.pats = vec![Slot::Empty; 16];
            self.pats_info = vec![PatInfo::default(); 16];
        }
        match self.pats.iter().position(|p| *p == Slot::Empty) {
            Some(p) => p,
            None => {
                let p = self.pats.len();
                self.pats.resize(p + 16, Slot::Empty);
                self.pats_info.resize(p + 16, PatInfo::default());
                p
            }
        }
    }

    /// sunvox_new_pattern.
    pub fn new_pattern(&mut self, lines: i32, channels: i32, x: i32, y: i32) -> usize {
        let p = self.get_free_pattern_num();
        let id = self.pat_id_counter;
        self.pat_id_counter = self.pat_id_counter.wrapping_add(1);
        let pat = Pattern {
            data: vec![Note::default(); (channels.max(0) * lines.max(0)) as usize],
            data_xsize: channels,
            data_ysize: lines,
            id,
            channels,
            lines,
            ysize: 32,
            flags: 0,
            name: None,
        };
        self.pat_data.push(pat);
        self.pats[p] = Slot::Data(self.pat_data.len() - 1);
        self.pats_info[p] = PatInfo { x, y, flags: 0, state_ptr: 0, ..self.pats_info[p] };
        p
    }

    /// sunvox_new_pattern_empty_clone: a slot held for a clone of `parent`.
    pub fn new_pattern_empty_clone(&mut self, parent: i32, x: i32, y: i32) -> usize {
        let p = self.get_free_pattern_num();
        self.pats[p] = Slot::Placeholder;
        self.pats_info[p] = PatInfo { x, y, parent_num: parent, ..PatInfo::default() };
        p
    }

    /// sunvox_remove_pattern.
    pub fn remove_pattern(&mut self, p: usize) {
        let Some(slot) = self.pats.get(p).copied() else { return };
        match slot {
            Slot::Empty => {}
            Slot::Placeholder => self.pats[p] = Slot::Empty,
            Slot::Data(d) => {
                if self.pats_info[p].flags & PAT_INFO_FLAG_CLONE != 0 {
                    self.pats[p] = Slot::Empty;
                } else {
                    for s in self.pats.iter_mut() {
                        if *s == Slot::Data(d) {
                            *s = Slot::Empty;
                        }
                    }
                }
            }
        }
    }

    /// sunvox_check_solo_mode.
    pub fn check_solo_mode(&mut self) {
        self.pats_solo_mode = (0..self.pats.len()).any(|i| self.pats[i] != Slot::Empty && self.pats_info[i].flags & PAT_INFO_FLAG_SOLO != 0);
    }

    pub fn pattern_num_by_name(&self, name: &str) -> i32 {
        for i in 0..self.pats.len() {
            if let Some(p) = self.pat(i) {
                if p.name.as_deref() == Some(name) {
                    return i as i32;
                }
            }
        }
        -1
    }

    /// sunvox_pattern_shift: a track's lines rotated down by `cnt`.
    fn pattern_shift(&mut self, pat_num: i32, track: i32, line: i32, lines: i32, cnt: i32, polyrhythm_len: i32) -> i32 {
        let Some(pat) = usize::try_from(pat_num).ok().and_then(|p| self.pat_mut(p)) else { return -1 };
        if track as u32 >= pat.channels as u32 || cnt == 0 {
            return -1;
        }
        let mut lines = if lines == 0 { pat.lines } else { lines };
        let mut line = line;
        if line < 0 {
            lines -= -line;
            line = 0;
        }
        if line + lines > pat.lines {
            lines = pat.lines - line;
        }
        if lines <= 0 {
            return -1;
        }
        const TMP: i32 = 16;
        let mut tmp = [Note::default(); TMP as usize];
        let mut cnt = cnt;
        while cnt < 0 {
            cnt += lines;
        }
        let xs = pat.data_xsize;
        let at = |l: i32| (track + xs * l) as usize;
        while cnt > 0 {
            let cnt2 = cnt.min(TMP);
            let from = if polyrhythm_len != 0 { line + polyrhythm_len - cnt2 } else { line + lines - cnt2 };
            for l in 0..cnt2 {
                tmp[l as usize] = pat.data[at(from + l)];
            }
            let mut l = line + lines - 1;
            while l >= line + cnt2 {
                pat.data[at(l)] = pat.data[at(l - cnt2)];
                l -= 1;
            }
            for l in 0..cnt2 {
                pat.data[at(line + l)] = tmp[l as usize];
            }
            cnt -= cnt2;
        }
        0
    }

    /// sunvox_get_mpp: most patterns playing at once.
    fn get_mpp(&self) -> i32 {
        let mut pats = [-1i32; MAX_PLAYING_PATS];
        let mut pats_num = 0usize;
        let mut max = 0;
        let mut nn = 0;
        for &n in &self.sorted_pats {
            let info = &self.pats_info[n];
            for (i, p) in pats.iter_mut().enumerate() {
                if *p == -1 {
                    *p = n as i32;
                    if i >= pats_num {
                        pats_num = i + 1;
                    }
                    nn += 1;
                    break;
                }
            }
            for p in pats.iter_mut().take(pats_num) {
                if *p != -1 {
                    let n2 = *p as usize;
                    if self.pats_info[n2].x + self.pat(n2).map_or(0, |p| p.lines) <= info.x {
                        *p = -1;
                        nn -= 1;
                    }
                }
            }
            if nn > max {
                max = nn;
            }
            while pats_num > 0 && pats[pats_num - 1] == -1 {
                pats_num -= 1;
            }
        }
        max
    }

    /// sunvox_sort_patterns: by start, then by end; the project's length;
    /// each pattern's playing state.
    pub fn sort_patterns(&mut self) {
        if self.flags & FLAG_STATIC_TIMELINE != 0 {
            return;
        }
        self.proj_lines = 0;
        self.sorted_pats.clear();
        for i in 0..self.pats.len() {
            let Some(p) = self.pat(i) else { continue };
            let max_x = self.pats_info[i].x + p.lines;
            if max_x > self.proj_lines {
                self.proj_lines = max_x;
            }
            self.sorted_pats.push(i);
        }
        for i in 1..self.sorted_pats.len() {
            let mut j = i;
            let n = self.sorted_pats[i];
            let (x, end) = (self.pats_info[n].x, self.pats_info[n].x + self.pat(n).unwrap().lines);
            while j > 0 {
                let n2 = self.sorted_pats[j - 1];
                let (x2, end2) = (self.pats_info[n2].x, self.pats_info[n2].x + self.pat(n2).unwrap().lines);
                if !(x2 > x || (x2 == x && end2 > end)) {
                    break;
                }
                self.sorted_pats[j] = n2;
                j -= 1;
            }
            if j != i {
                self.sorted_pats[j] = n;
            }
        }
        if self.flags & FLAG_DYNAMIC_PATTERN_STATE != 0 {
            let mut pp = 0i32;
            if self.flags & FLAG_SUPERTRACKS != 0 {
                for i in 0..self.pats.len() {
                    if self.has_pat(i) {
                        let st = (self.pats_info[i].y + 16) / 32;
                        if st + 1 > pp {
                            pp = st + 1;
                        }
                    }
                }
            } else {
                pp = self.get_mpp();
            }
            let pp = (pp.max(0) as usize).min(MAX_PLAYING_PATS);
            if pp > self.pat_state.len() {
                let old = self.pat_state.len();
                self.pat_state.resize(pp, PatState::default());
                for st in self.pat_state[old..].iter_mut() {
                    clean_pattern_state(st);
                }
            }
        }
        let size = self.pat_state.len();
        if self.sorted_pats.is_empty() {
            return;
        }
        if self.flags & FLAG_SUPERTRACKS != 0 {
            for p in 0..self.sorted_pats.len() {
                let n = self.sorted_pats[p];
                let mut st = (self.pats_info[n].y + 16) / 32;
                if st < 0 {
                    st = 0;
                }
                if st as usize >= size {
                    st = size as i32 - 1;
                }
                self.pats_info[n].state_ptr = st.max(0) as usize;
                self.pats_info[n].track_status = 0;
            }
        } else {
            let mut pats = [-1i32; MAX_PLAYING_PATS];
            let mut pats_num = 0usize;
            for p in 0..self.sorted_pats.len() {
                let n = self.sorted_pats[p];
                let x = self.pats_info[n].x;
                let mut state_ptr = size.saturating_sub(1);
                let pats_num2 = (pats_num + 1).min(size);
                for (i, &n2) in pats.iter().enumerate().take(pats_num2) {
                    if n2 == -1 {
                        state_ptr = i;
                        break;
                    }
                    let n2 = n2 as usize;
                    if self.pats_info[n2].x + self.pat(n2).unwrap().lines <= x {
                        state_ptr = i;
                        break;
                    }
                }
                pats[state_ptr] = n as i32;
                self.pats_info[n].state_ptr = state_ptr;
                self.pats_info[n].track_status = 0;
                if state_ptr >= pats_num {
                    pats_num = state_ptr + 1;
                    if pats_num < size {
                        pats[pats_num] = -1;
                    }
                }
            }
        }
    }

    /// sunvox_select_current_playing_patterns.
    pub fn select_current_playing_patterns(&mut self, first: usize) {
        self.cur_playing_pats[0] = -1;
        self.last_sort_pat = -1;
        if self.sorted_pats.is_empty() {
            return;
        }
        let size = self.pat_state.len();
        let mut p = 0usize;
        for i in first..self.sorted_pats.len() {
            let n = self.sorted_pats[i];
            let info = self.pats_info[n];
            let lines = self.pat(n).unwrap().lines;
            if self.line_counter >= info.x && self.line_counter < info.x + lines {
                let st = &mut self.pat_state[info.state_ptr];
                if !st.busy {
                    clean_pattern_state(st);
                    st.busy = true;
                }
                self.cur_playing_pats[p] = i as i32;
                p += 1;
                if p >= size {
                    break;
                }
            }
            if info.x > self.line_counter {
                break;
            }
            self.last_sort_pat = i as i32;
        }
        if p < size {
            self.cur_playing_pats[p] = -1;
        }
    }

    /// sunvox_set_position.
    pub fn set_position(&mut self, pos: i32) {
        self.line_counter = pos;
        self.sort_patterns();
        self.select_current_playing_patterns(0);
    }

    /// sunvox_get_time_map's length: the frames from `start_line` for `len` lines.
    pub fn time_map_frames(&self, start_line: i32, len: i32) -> u32 {
        if len <= 0 {
            return 0;
        }
        let mut map = vec![(0u16, 0u8); len as usize];
        map[0] = (self.bpm, self.speed);
        for p in 0..self.pats.len() {
            let Some(pat) = self.pat(p) else { continue };
            let info = &self.pats_info[p];
            if info.flags & PAT_INFO_FLAG_MUTE != 0 || (self.pats_solo_mode && info.flags & PAT_INFO_FLAG_SOLO == 0) {
                continue;
            }
            let mut pat_x = info.x;
            let mut pat_lines = pat.lines;
            if pat_x < start_line {
                pat_lines -= start_line - pat_x;
                pat_x = start_line;
            }
            if pat_x + pat_lines > start_line + len {
                pat_lines = start_line + len - pat_x;
            }
            if pat_lines <= 0 {
                continue;
            }
            let xoffset = pat_x - info.x;
            let xx = pat_x - start_line;
            let mut lp = xoffset * pat.data_xsize;
            for ln in xx..xx + pat_lines {
                for cn in 0..pat.channels {
                    let n = &pat.data[(lp + cn) as usize];
                    let eff = n.ctl & 0xFF;
                    if eff == 0x0F || eff == 0x1F {
                        if eff == 0x0F && n.ctl_val < 32 {
                            map[ln as usize].1 = n.ctl_val.max(1) as u8;
                        } else {
                            map[ln as usize].0 = n.ctl_val.clamp(1, 16000);
                        }
                    }
                }
                lp += pat.data_xsize;
            }
        }
        let mut frames: u64 = 0;
        let mut add: u64 = 0;
        let mut v = map[0];
        for (i, m) in map.iter().enumerate() {
            if m.0 != 0 || m.1 != 0 || i == 0 {
                if m.0 != 0 {
                    v.0 = m.0;
                }
                if m.1 != 0 {
                    v.1 = m.1;
                }
                let one_tick = net::tick_size(self.net.sampling_freq, v.0 as i32);
                add = one_tick as u64 * v.1 as u64;
            }
            frames += add;
        }
        (frames >> 8) as u32
    }

    /// sunvox_get_proj_lines.
    pub fn proj_lines_now(&self) -> i32 {
        let mut n = 0;
        for p in 0..self.pats.len() {
            if let Some(pat) = self.pat(p) {
                let end = self.pats_info[p].x + pat.lines;
                if end > 0 && end > n {
                    n = end;
                }
            }
        }
        n
    }

    /// sunvox_get_proj_frames( 0, 0 ).
    pub fn proj_frames(&self) -> u32 {
        let lines = self.proj_lines_now();
        self.time_map_frames(0, lines)
    }

    // Playing.

    /// sunvox_play from a stop: the timeline set up, a PLAY command sent and
    /// handled by rendering one frame at once (one-thread mode).
    pub fn play(&mut self, pos: i32, jump_to_pos: bool) {
        if !self.playing {
            if jump_to_pos {
                self.line_counter = pos;
            }
            self.sort_patterns();
            self.select_current_playing_patterns(0);
            self.line_counter -= 1;
            self.proj_len = self.proj_frames();
            self.user_commands.push_back(UserCmd { n: Note { note: NOTECMD_PLAY, ..Note::default() }, ch: 0 });
            self.handle_all_commands();
        }
    }

    /// sunvox_handle_all_commands_UNSAFE: one frame rendered and dropped.
    pub fn handle_all_commands(&mut self) {
        self.flags |= FLAG_IGNORE_SLOT_SYNC;
        let mut out = [Vec::new(), Vec::new()];
        self.render(1, &mut out);
        self.flags &= !FLAG_IGNORE_SLOT_SYNC;
    }

    /// sunvox_check_speed: SPEED_CHANGED to the modules that ask when the
    /// tempo changed; the tick's length.
    fn check_speed(&mut self, offset: i32) -> u32 {
        let one_tick = net::tick_size(self.net.sampling_freq, self.bpm as i32);
        if self.prev_bpm != self.bpm || self.prev_speed != self.speed {
            let mut e = Event::new(Cmd::SpeedChanged);
            e.offset = offset;
            e.set_speed(one_tick as i32, self.bpm, self.speed);
            for m in 0..self.net.mods_num() {
                let f = self.net.mods[m].flags;
                if f & net::FLAG_EXISTS != 0 && f & net::FLAG_GET_SPEED_CHANGES != 0 {
                    self.net.add_event(m, &e);
                }
            }
            self.prev_bpm = self.bpm;
            self.prev_speed = self.speed;
        }
        self.net.host.bpm = self.bpm as i32;
        self.net.host.speed = self.speed as i32;
        one_tick
    }

    /// sunvox_reset_timeline_activity: every playing track's note off.
    fn reset_timeline_activity(&mut self, offset: i32) {
        for i in 0..self.pat_state.len() {
            let p = self.cur_playing_pats[i];
            if p == -1 {
                break;
            }
            if let Some(&n) = self.sorted_pats.get(p as usize) {
                if self.has_pat(n) {
                    self.pats_info[n].track_status = 0;
                }
            }
        }
        for i in 0..self.pat_state.len() {
            if !self.pat_state[i].busy {
                continue;
            }
            for a in 0..MAX_PATTERN_TRACKS {
                if self.pat_state[i].track_status & (1 << a) != 0 {
                    let m = self.pat_state[i].track_module[a] as i32;
                    if (m as u32) < self.net.mods_num() as u32 {
                        let mut e = Event::new(Cmd::NoteOff);
                        e.id = ((i as u32) << 16) | a as u32;
                        e.offset = offset;
                        e.set_velocity(256);
                        self.net.add_event(m as usize, &e);
                    }
                }
            }
            self.pat_state[i].busy = false;
            self.pat_state[i].track_status = 0;
        }
        self.jump_request = false;
    }

    /// GET_PITCH.
    fn get_pitch(&self, nn: u8, xxyy: i32, add: i32) -> i32 {
        if nn == NOTECMD_SET_PITCH {
            xxyy + add
        } else {
            let mut p = 7680 * 4 - (nn as i32 - 1) * 256 + add;
            if self.base_version < 0x01080000 && p >= 7680 * 4 {
                p = 7680 * 4;
            }
            p
        }
    }

    fn state_index(&self, pat_num: i32) -> Option<usize> {
        if pat_num == VIRTUAL_PATTERN {
            None
        } else {
            Some(self.pats_info[pat_num as usize].state_ptr)
        }
    }

    fn state_mut(&mut self, si: Option<usize>) -> &mut PatState {
        match si {
            None => &mut self.virtual_pat_state,
            Some(i) => &mut self.pat_state[i],
        }
    }

    fn pat_info_mut(&mut self, pat_num: i32) -> &mut PatInfo {
        if pat_num == VIRTUAL_PATTERN {
            &mut self.virtual_pat_info
        } else {
            &mut self.pats_info[pat_num as usize]
        }
    }

    /// The note at a pattern's line and track (a pattern's own data).
    /// (C reads the pattern's memory: a cell past a row is the next row's.)
    fn note_at(&self, pat_num: i32, line: i32, track: i32) -> Note {
        let p = self.pat(pat_num as usize).unwrap();
        usize::try_from(line * p.data_xsize + track).ok().and_then(|i| p.data.get(i)).copied().unwrap_or_default()
    }

    fn set_note_at(&mut self, pat_num: i32, line: i32, track: i32, n: Note) {
        let p = self.pat_mut(pat_num as usize).unwrap();
        let xs = p.data_xsize;
        if let Some(c) = usize::try_from(line * xs + track).ok().and_then(|i| p.data.get_mut(i)) {
            *c = n;
        }
    }

    /// sunvox_handle_command: a pattern cell (or a user command) acted on.
    /// `line` is the cell's line in its pattern (for the effects that look
    /// at its neighbours); `snote` the cell as it is now.
    #[allow(clippy::cognitive_complexity)]
    fn handle_command(&mut self, offset: i32, snote: Note, line: i32, pat_num: i32, track_num: i32) {
        let mut e = Event { offset, ..Event::default() };
        // C reaches a row's other cells from the cell itself (snote - track_num + n),
        // so after a "previous track" cell moved track_num they are this far off.
        let orig_track = track_num;
        let mut track_num = track_num;
        let si;
        if pat_num == VIRTUAL_PATTERN {
            si = None;
            if track_num >= self.virtual_pat_tracks {
                self.virtual_pat_tracks = track_num + 1;
            }
            e.id = ((pat_num as u32) << 16) | track_num as u32;
        } else {
            // A "previous track" cell acts on the first track before it that
            // isn't one; what it sends is still its own.
            if snote.note == NOTECMD_PREV_TRACK {
                loop {
                    track_num -= 1;
                    if track_num < 0 {
                        break;
                    }
                    if self.note_at(pat_num, line, track_num).note != NOTECMD_PREV_TRACK {
                        break;
                    }
                }
                if track_num < 0 {
                    return;
                }
            }
            si = self.state_index(pat_num);
            e.id = ((si.unwrap() as u32) << 16) | track_num as u32;
        }
        let tn = track_num as usize;
        let mut vel = snote.vel as i32;
        let mut ctl = snote.ctl;
        let mut ctl_val = snote.ctl_val as i32;
        let mut delay_len = 0;
        let mut note = snote.note as i32;
        let ctl_eff = (ctl & 0xFF) as u8;
        let mut mod_num = snote.module as i32 - 1;
        let mods_num = self.net.mods_num() as i32;
        let mut has_mod = (mod_num as u32) < mods_num as u32;
        let mut note_num = (note > 0 && note < 128) || note == NOTECMD_SET_PITCH as i32;
        let track_bit = 1u32 << track_num;
        let mut vel_handled = false;
        let timer_handling = self.state_mut(si).effects[tn].flags & EFF_FLAG_TIMER_HANDLING != 0;
        let mut goto_delay = false;
        if timer_handling {
            let eff = &mut self.state_mut(si).effects[tn];
            if eff.flags & EFF_FLAG_TIMER_RETRIG != 0 {
                eff.timer = eff.timer_init;
            } else {
                if eff.flags & EFF_FLAG_TIMER_CUT != 0 {
                    note = NOTECMD_NOTE_OFF as i32;
                    note_num = false;
                }
                if eff.flags & EFF_FLAG_TIMER_CUT2 != 0 {
                    note = 0;
                    note_num = false;
                    vel = 1;
                }
            }
        } else if ctl_eff != 0 {
            if ctl_eff == 0x1D && ctl_val != 0 {
                goto_delay = true;
            } else if ctl_eff > 0x40 && ctl_eff < 0x60 {
                delay_len = ctl_eff as i32;
                goto_delay = true;
            } else {
                match ctl_eff {
                    0x20 => {
                        let r = (net::pseudo_random(&mut self.rand_next) & 0x7FFF) as i32;
                        if r >= ctl_val {
                            note_num = false;
                            note = 0;
                            vel = 0;
                        }
                    }
                    0x21 => {
                        let r = (net::pseudo_random(&mut self.rand_next) & 0x7FFF) as i32;
                        if r >= ctl_val {
                            note_num = false;
                            note = 0;
                            vel = 0;
                        } else if vel != 0 {
                            vel = (((vel - 1) * r) / 0x8000) + 1;
                        } else {
                            vel = (r >> 8) + 1;
                        }
                    }
                    0x22 => {
                        if ctl_val > 32768 {
                            ctl_val = 32768;
                        }
                        let r = net::pseudo_random(&mut self.rand_next) & 0x7FFF;
                        let r = (ctl_val as u32).wrapping_mul(r);
                        ctl_val = (r / 32768) as i32;
                        if (r & 32767) > 16384 {
                            ctl_val += 1;
                        }
                    }
                    0x23 => {
                        let mut r = net::pseudo_random(&mut self.rand_next) & 0x7FFF;
                        let mut ctl_type = 0u8;
                        let mut range_ctl = None;
                        if has_mod {
                            let cn = (((ctl >> 8) & 0xFF) as u32).wrapping_sub(1);
                            if let Some(c) = self.net.mods[mod_num as usize].ctls.get(cn as usize) {
                                ctl_type = c.typ;
                                range_ctl = Some((c.min, c.max));
                            }
                        }
                        let mut v1 = ((ctl_val >> 8) & 255) as u32;
                        let mut v2 = (ctl_val & 255) as u32;
                        if v1 > v2 {
                            std::mem::swap(&mut v1, &mut v2);
                        }
                        if ctl_type == 0 {
                            let range = v2 - v1;
                            v1 = (v1 * 32768) / 255;
                            v2 = (range * 32768) / 255;
                            r = v2.wrapping_mul(r);
                            v2 = r / 32768;
                            if (r & 32767) > 16384 {
                                v2 += 1;
                            }
                        } else {
                            let (min, max) = range_ctl.unwrap_or((0, 0));
                            if v1 < min as u32 {
                                v1 = min as u32;
                            } else if v1 > max as u32 {
                                v1 = max as u32;
                            }
                            if v2 < min as u32 {
                                v2 = min as u32;
                            } else if v2 > max as u32 {
                                v2 = max as u32;
                            }
                            let range = v2.wrapping_sub(v1);
                            r = range.wrapping_mul(r);
                            v2 = r / 32768;
                            if (r & 32767) > 16384 {
                                v2 += 1;
                            }
                        }
                        ctl_val = v1.wrapping_add(v2) as i32;
                    }
                    0x24..=0x29 => {
                        if pat_num != VIRTUAL_PATTERN {
                            let pos = if ctl_eff & 1 != 0 {
                                let r = (net::pseudo_random(&mut self.rand_next) & 32767) as i32;
                                let mut v1 = (ctl_val >> 8) & 255;
                                let mut v2 = ctl_val & 255;
                                if v1 > v2 {
                                    std::mem::swap(&mut v1, &mut v2);
                                }
                                let range = v2 - v1;
                                (((range + 1) * r) >> 15) + v1
                            } else {
                                ctl_val
                            };
                            let pat = self.pat(pat_num as usize).unwrap();
                            let mut snote2 = None;
                            if (ctl_eff >> 1) & 1 != 0 {
                                if pos < pat.channels {
                                    snote2 = pat.data.get((line * pat.data_xsize + orig_track - track_num + pos) as usize).copied();
                                }
                            } else if pos < pat.lines {
                                snote2 = Some(if ctl_eff < 0x28 { pat.data[(track_num + pos * pat.data_xsize) as usize] } else { pat.data[(pos * pat.data_xsize) as usize] });
                            }
                            let mut no_note = true;
                            if let Some(s2) = snote2 {
                                if (s2.note > 0 && s2.note < 128) || s2.note == NOTECMD_SET_PITCH {
                                    no_note = false;
                                    let mut p = self.get_pitch(s2.note, s2.ctl_val as i32, 0);
                                    if note_num {
                                        p -= (note - 1 - 12 * 5) * 256;
                                    }
                                    if !has_mod {
                                        mod_num = s2.module as i32 - 1;
                                        has_mod = (mod_num as u32) < mods_num as u32;
                                    }
                                    note_num = true;
                                    note = NOTECMD_SET_PITCH as i32;
                                    ctl = 0;
                                    ctl_val = p;
                                } else if s2.note == NOTECMD_NOTE_OFF {
                                    no_note = false;
                                    note = NOTECMD_NOTE_OFF as i32;
                                    ctl = 0;
                                    ctl_val = 0;
                                }
                            }
                            if no_note {
                                note_num = false;
                                note = 0;
                                vel = 0;
                            }
                        }
                    }
                    0x30 => {
                        note_num = false;
                        note = NOTECMD_STOP as i32;
                    }
                    _ => {}
                }
            }
        }
        if !goto_delay {
            if note_num && has_mod {
                let (ft, rel) = (self.net.mods[mod_num as usize].finetune, self.net.mods[mod_num as usize].relative_note);
                let add = -ft - rel * 256 + self.pitch_offset;
                let p0 = self.get_pitch(note as u8, ctl_val, add);
                let mut evt_pitch = p0;
                let mut not_allow = false;
                match ctl_eff {
                    0x3 => not_allow = true,
                    0x5 => evt_pitch -= ctl_val,
                    0x6 => evt_pitch += ctl_val,
                    _ => {}
                }
                if pat_num != VIRTUAL_PATTERN {
                    let pat = self.pat(pat_num as usize).unwrap();
                    for i in track_num + 1..pat.channels {
                        let s2 = pat.data[(line * pat.data_xsize + i) as usize];
                        if s2.note != NOTECMD_PREV_TRACK {
                            break;
                        }
                        match s2.ctl & 0xFF {
                            0x3 => not_allow = true,
                            0x5 => evt_pitch -= s2.ctl_val as i32,
                            0x6 => evt_pitch += s2.ctl_val as i32,
                            _ => {}
                        }
                    }
                }
                {
                    let eff = &mut self.state_mut(si).effects[tn];
                    eff.evt_pitch0 = p0;
                    eff.evt_pitch = evt_pitch;
                }
                if !not_allow {
                    let mut v = if vel != 0 { (vel - 1) * 2 } else { 256 };
                    if self.velocity != 256 {
                        v = (v as i16 as i32 * self.velocity) / 256;
                    }
                    e.set_velocity(v);
                    vel_handled = true;
                    let rn = if note == NOTECMD_SET_PITCH as i32 { (net::NOTE0_PITCH - evt_pitch) / 256 } else { note - 1 };
                    e.set_root_note(rn as i16);
                    let st = self.state_mut(si);
                    st.effects[tn].cur_vel = e.velocity();
                    st.effects[tn].cur_pitch = evt_pitch;
                    let prev_mod = st.track_module[tn] as i32;
                    let busy = st.track_status & track_bit != 0;
                    if busy && prev_mod != mod_num {
                        e.cmd = Cmd::NoteOff;
                        self.net.add_event(prev_mod as u32 as usize, &e);
                    }
                    e.cmd = Cmd::NoteOn;
                    e.set_pitch(evt_pitch);
                    self.net.add_event(mod_num as usize, &e);
                    let st = self.state_mut(si);
                    st.track_module[tn] = mod_num as i16;
                    st.track_status |= track_bit;
                    st.effects[tn].vib_phase = 0;
                    self.pat_info_mut(pat_num).track_status |= track_bit;
                }
            }
            match note as u8 {
                NOTECMD_PLAY => {
                    let one_tick = net::tick_size(self.net.sampling_freq, self.bpm as i32);
                    self.playing = true;
                    self.tick_counter = one_tick;
                    if pat_num == VIRTUAL_PATTERN && ctl_eff == 0x1D {
                        let mut t2 = one_tick as i32 - 2 * 256;
                        while t2 < 0 {
                            t2 += one_tick as i32;
                        }
                        self.tick_counter = t2 as u32;
                    }
                    self.speed_counter = self.speed as i32 - 1;
                    self.single_pattern_play = ctl_val - 1;
                    self.next_single_pattern_play = -1;
                    for i in 0..self.net.mods_num() {
                        let f = self.net.mods[i].flags;
                        if f & (net::FLAG_EXISTS | net::FLAG_GET_PLAY_COMMANDS) == (net::FLAG_EXISTS | net::FLAG_GET_PLAY_COMMANDS) {
                            e.cmd = Cmd::Play;
                            self.net.add_event(i, &e);
                        }
                    }
                }
                NOTECMD_STOP => {
                    self.reset_timeline_activity(offset);
                    self.playing = false;
                    self.single_pattern_play = -1;
                    self.next_single_pattern_play = -1;
                    for i in 0..self.net.mods_num() {
                        let f = self.net.mods[i].flags;
                        if f & (net::FLAG_EXISTS | net::FLAG_GET_STOP_COMMANDS) == (net::FLAG_EXISTS | net::FLAG_GET_STOP_COMMANDS) {
                            e.cmd = Cmd::Stop;
                            self.net.add_event(i, &e);
                        }
                    }
                }
                NOTECMD_ALL_NOTES_OFF => {
                    for i in 0..self.net.mods_num() {
                        if self.net.mods[i].flags & net::FLAG_EXISTS != 0 {
                            e.cmd = Cmd::AllNotesOff;
                            self.net.add_event(i, &e);
                        }
                    }
                }
                NOTECMD_NOTE_OFF => {
                    let st = self.state_mut(si);
                    let m2 = st.track_module[tn] as i32;
                    if (m2 as u32) < mods_num as u32 && st.track_status & track_bit != 0 {
                        e.cmd = Cmd::NoteOff;
                        let mut v = if vel != 0 { (vel - 1) * 2 } else { 256 };
                        if self.velocity != 256 {
                            v = (v as i16 as i32 * self.velocity) / 256;
                        }
                        e.set_velocity(v);
                        self.net.add_event(m2 as usize, &e);
                        self.state_mut(si).track_status &= !track_bit;
                        self.pat_info_mut(pat_num).track_status &= !track_bit;
                    }
                }
                NOTECMD_CLEAN_MODULES => {
                    for i in 0..self.net.mods_num() {
                        if self.net.mods[i].flags & net::FLAG_EXISTS != 0 {
                            e.cmd = Cmd::Clean;
                            self.net.add_event(i, &e);
                        }
                    }
                    clean_pattern_state(&mut self.virtual_pat_state);
                    self.virtual_pat_tracks = 0;
                }
                NOTECMD_CLEAN_MODULE => {
                    let mut m2 = mod_num;
                    if m2 < 0 {
                        m2 = self.state_mut(si).track_module[tn] as i32;
                    }
                    e.cmd = Cmd::Clean;
                    self.net.add_event(m2 as u32 as usize, &e);
                }
                NOTECMD_PATPLAY_OFF => {
                    self.single_pattern_play = -1;
                    self.next_single_pattern_play = -1;
                }
                NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP | NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP2 => {
                    if self.playing {
                        let stop = Note { note: NOTECMD_STOP, ..Note::default() };
                        self.handle_command(offset, stop, line, pat_num, track_num);
                    }
                    let one_tick = net::tick_size(self.net.sampling_freq, self.bpm as i32);
                    self.tick_counter = one_tick;
                    self.set_position(if note as u8 == NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP { ctl_val } else { -ctl_val });
                    self.line_counter -= 1;
                    self.speed_counter = self.speed as i32 - 1;
                    self.jump_request = false;
                }
                NOTECMD_JUMP => {
                    if self.playing {
                        let mut s2 = Note { note: NOTECMD_STOP, ..Note::default() };
                        self.handle_command(offset, s2, line, pat_num, track_num);
                        s2.note = NOTECMD_PREPARE_FOR_IMMEDIATE_JUMP;
                        s2.ctl_val = ctl_val as u16;
                        self.handle_command(offset, s2, line, pat_num, track_num);
                        s2.note = NOTECMD_PLAY;
                        s2.ctl_val = 0;
                        self.handle_command(offset, s2, line, pat_num, track_num);
                    } else {
                        self.set_position(ctl_val);
                    }
                }
                _ => {}
            }
            if vel != 0 && !vel_handled {
                let m2 = self.state_mut(si).track_module[tn] as i32;
                if (m2 as u32) < mods_num as u32 {
                    let mut v = (vel - 1) * 2;
                    if self.velocity != 256 {
                        v = (v as i16 as i32 * self.velocity) / 256;
                    }
                    e.set_velocity(v);
                    self.state_mut(si).effects[tn].cur_vel = e.velocity();
                    e.cmd = Cmd::SetVelocity;
                    self.net.add_event(m2 as usize, &e);
                }
            }
            if ctl & 0xFF00 != 0 {
                let mut m2 = mod_num;
                e.cmd = Cmd::SetLocalController;
                if m2 >= 0 {
                    if note == 0 {
                        e.cmd = Cmd::SetGlobalController;
                    }
                } else {
                    m2 = self.state_mut(si).track_module[tn] as i32;
                }
                if (m2 as u32) < mods_num as u32 {
                    e.set_ctl_num((((ctl >> 8) & 0xFF) as u16).wrapping_sub(1));
                    e.set_ctl_val(ctl_val as u16);
                    self.net.add_event(m2 as usize, &e);
                }
            }
        }
        // delay:
        if self.state_mut(si).effects[tn].flags & EFF_FLAG_TIMER_HANDLING != 0 {
            return;
        }
        if note_num {
            self.state_mut(si).effects[tn].flags |= EFF_FLAG_EVT_PITCH;
        }
        if ctl_eff == 0 {
            return;
        }
        let mut m2 = mod_num;
        if m2 < 0 {
            m2 = self.state_mut(si).track_module[tn] as i32;
        }
        if delay_len > 0 {
            let eff = &mut self.state_mut(si).effects[tn];
            eff.timer = delay_len as u8;
            eff.flags |= EFF_FLAG_TIMER_DELAY;
            return;
        }
        let base_version = self.base_version;
        let speed = self.speed as i32;
        match ctl_eff {
            0x1 => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.flags |= EFF_FLAG_TONE_PORTA;
                eff.target_pitch = -7680 * 4;
                if ctl_val != 0 {
                    eff.porta_speed = (ctl_val * 4) as u16;
                }
            }
            0x2 => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.flags |= EFF_FLAG_TONE_PORTA;
                eff.target_pitch = if base_version < 0x01080000 { 7680 * 4 } else { 7680 * 4 * 2 };
                if ctl_val != 0 {
                    eff.porta_speed = (ctl_val * 4) as u16;
                }
            }
            0x3 => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.flags |= EFF_FLAG_TONE_PORTA;
                if ctl_val != 0 {
                    eff.porta_speed = (ctl_val * 4) as u16;
                }
                if eff.flags & EFF_FLAG_EVT_PITCH != 0 {
                    eff.target_pitch = eff.evt_pitch;
                    if note == NOTECMD_SET_PITCH as i32 {
                        let mut delta = (eff.target_pitch - eff.cur_pitch).abs();
                        if base_version < 0x01090600 {
                            delta /= 6;
                        } else {
                            delta /= speed;
                        }
                        if delta == 0 {
                            delta = 1;
                        }
                        eff.porta_speed = delta as u16;
                    }
                }
            }
            0x4 => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.flags |= EFF_FLAG_TONE_VIBRATO;
                if !(ctl_val == 0 && base_version != 0x01070000) {
                    eff.vib_freq = ((ctl_val >> 8) & 255) as u8;
                    eff.vib_amp = (ctl_val & 255) as u8;
                }
            }
            0x5 | 0x6 => {
                let eff = &mut self.state_mut(si).effects[tn];
                let mut new_pitch = eff.evt_pitch0;
                if ctl_eff == 0x5 {
                    new_pitch -= ctl_val;
                } else {
                    new_pitch += ctl_val;
                }
                if new_pitch != eff.evt_pitch {
                    let diff = new_pitch - eff.evt_pitch;
                    eff.evt_pitch += diff;
                    eff.cur_pitch += diff;
                    eff.target_pitch += diff;
                    eff.flags |= EFF_FLAG_PITCH_CHANGED;
                }
            }
            0x8 => self.state_mut(si).effects[tn].arpeggio = ctl_val as u16,
            0x9 => {
                e.set_sample_offset((ctl_val as u32).wrapping_mul(256));
                e.cmd = Cmd::SetSampleOffset;
                self.net.add_event(m2 as u32 as usize, &e);
            }
            0x7 => {
                e.set_sample_offset(ctl_val as u32);
                e.cmd = Cmd::SetSampleOffset2;
                self.net.add_event(m2 as u32 as usize, &e);
            }
            0xA => {
                let eff = &mut self.state_mut(si).effects[tn];
                if ctl_val & 0xFF00 != 0 {
                    eff.vel_speed = (ctl_val >> 8) as i16;
                }
                if ctl_val & 0xFF != 0 {
                    eff.vel_speed = -((ctl_val & 0xFF) as i16);
                }
            }
            0xF => {
                if ctl_val < 32 {
                    let prev = self.speed;
                    self.speed = ctl_val as u8;
                    if self.speed == 0 {
                        self.speed = 1;
                    }
                    if prev != self.speed {
                        self.change_counter += 1;
                    }
                } else if (0xF000..=0xF1FF).contains(&ctl_val) {
                    if ctl_val < 0xF100 {
                        self.tgrid = (ctl_val & 255).max(2) as u8;
                    } else {
                        self.tgrid2 = (ctl_val & 255).max(2) as u8;
                    }
                    self.change_counter += 1;
                } else {
                    let prev = self.bpm;
                    self.bpm = (ctl_val as u16).clamp(1, 16000);
                    if prev != self.bpm {
                        self.change_counter += 1;
                    }
                }
            }
            0x11 | 0x12 => {
                let eff = &mut self.state_mut(si).effects[tn];
                if ctl_val != 0 {
                    eff.porta_speed = (ctl_val * 4) as u16;
                }
                if ctl_eff == 0x11 {
                    eff.cur_pitch -= eff.porta_speed as i32;
                    if eff.cur_pitch < -7680 * 4 {
                        eff.cur_pitch = -7680 * 4;
                    }
                } else {
                    eff.cur_pitch += eff.porta_speed as i32;
                    if eff.cur_pitch > 7680 * 4 {
                        eff.cur_pitch = 7680 * 4;
                    }
                }
                eff.flags |= EFF_FLAG_PITCH_CHANGED;
            }
            0x13 | 0x14 => {
                let mut f = 0u8;
                if ctl_val & 0x001 != 0 {
                    f |= net::EVENT_MUTE;
                }
                if ctl_val & 0x010 != 0 {
                    f |= net::EVENT_SOLO;
                }
                if ctl_val & 0x100 != 0 {
                    f |= net::EVENT_BYPASS;
                }
                e.set_mute_flags(f);
                e.cmd = if ctl_eff == 0x13 { Cmd::SetMsb } else { Cmd::ResetMsb };
                self.net.add_event(m2 as u32 as usize, &e);
            }
            0x15 => {
                if ctl_val != 0 {
                    let mut ft = -512;
                    let mut rel = -512;
                    let v = ctl_val & 255;
                    if v != 0 {
                        ft = (v - 128) * 2;
                    }
                    let v = (ctl_val >> 8) & 255;
                    if v != 0 {
                        rel = v - 128;
                    }
                    e.set_finetune(ft, rel);
                    e.cmd = Cmd::Finetune;
                    self.net.add_event(m2 as u32 as usize, &e);
                }
            }
            0x19 => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.timer = (ctl_val & 31) as u8;
                eff.timer_init = eff.timer;
                if eff.timer > 0 {
                    eff.timer += 1;
                    eff.flags |= EFF_FLAG_TIMER_RETRIG;
                }
            }
            0x1A => {
                let eff = &mut self.state_mut(si).effects[tn];
                let prev = eff.cur_vel;
                if ctl_val & 0xFF00 != 0 {
                    eff.cur_vel = eff.cur_vel.wrapping_add((ctl_val >> 8) as i16);
                    if eff.cur_vel > 256 {
                        eff.cur_vel = 256;
                    }
                }
                if ctl_val & 0xFF != 0 {
                    eff.cur_vel = eff.cur_vel.wrapping_sub((ctl_val & 0xFF) as i16);
                    if eff.cur_vel < 0 {
                        eff.cur_vel = 0;
                    }
                }
                if prev != eff.cur_vel {
                    eff.flags |= EFF_FLAG_VEL_CHANGED;
                }
            }
            0x1C => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.timer = ((ctl_val & 31) + 1) as u8;
                if (ctl_val & 0xF000) == 0x1000 {
                    eff.flags |= EFF_FLAG_TIMER_CUT2;
                } else {
                    eff.flags |= EFF_FLAG_TIMER_CUT;
                }
            }
            0x1D => {
                let eff = &mut self.state_mut(si).effects[tn];
                eff.timer = (ctl_val & 31) as u8;
                if eff.timer > 0 {
                    eff.timer += 1;
                    eff.flags |= EFF_FLAG_TIMER_DELAY;
                }
            }
            0x1F => {
                let prev = self.bpm;
                self.bpm = (ctl_val as u16).clamp(1, 16000);
                if prev != self.bpm {
                    self.change_counter += 1;
                }
            }
            0x31 => {
                if self.flags & FLAG_IGNORE_EFF31 == 0 {
                    let x = if pat_num == VIRTUAL_PATTERN { self.virtual_pat_info.x } else { self.pats_info[pat_num as usize].x };
                    let v = match self.jump_request_mode {
                        1 => x + ctl_val,
                        2 => x - ctl_val,
                        3 => self.line_counter + 1 + ctl_val,
                        4 => self.line_counter + 1 - ctl_val,
                        _ => ctl_val,
                    };
                    self.jump_request_line = v;
                    self.jump_request = true;
                }
            }
            0x32 => self.jump_request_mode = ctl_val as u8,
            0x33 => {}
            0x34 => {
                let set = (ctl_val >> 8) & 0xFF;
                let reset = ctl_val & 0xFF;
                if set & 1 != 0 {
                    self.flags |= FLAG_NO_TONE_PORTA_ON_TICK0;
                }
                if set & 2 != 0 {
                    self.flags |= FLAG_NO_VOL_SLIDE_ON_TICK0;
                }
                if set & 4 != 0 {
                    self.flags |= FLAG_KBD_ROUNDROBIN;
                }
                if set & 8 != 0 {
                    self.net.midi_out_7bit = true;
                }
                if reset & 1 != 0 {
                    self.flags &= !FLAG_NO_TONE_PORTA_ON_TICK0;
                }
                if reset & 2 != 0 {
                    self.flags &= !FLAG_NO_VOL_SLIDE_ON_TICK0;
                }
                if reset & 4 != 0 {
                    self.flags &= !FLAG_KBD_ROUNDROBIN;
                }
                if reset & 8 != 0 {
                    self.net.midi_out_7bit = false;
                }
            }
            0x35 => {
                e.cmd = Cmd::MidimsgSupport;
                e.set_midimsg(((ctl_val >> 8) & 255) as u8, (ctl_val & 255) as u8);
                self.net.add_event(m2 as u32 as usize, &e);
            }
            0x38 => {
                if pat_num != VIRTUAL_PATTERN {
                    let r = (net::pseudo_random(&mut self.rand_next) & 0x7FFF) as i32;
                    if (r >> 7) <= (ctl_val & 255) {
                        let t = (ctl_val >> 8) & 255;
                        if t < self.pat(pat_num as usize).unwrap().channels {
                            self.set_note_at(pat_num, line, orig_track - track_num + t, Note::default());
                            self.change_counter += 1;
                        }
                    }
                }
            }
            0x39 => {
                self.pattern_shift(pat_num, (ctl_val >> 8) & 255, 0, 0, ctl_val & 255, 0);
                self.change_counter += 1;
            }
            0x3A => {
                if pat_num != VIRTUAL_PATTERN {
                    let t = (ctl_val >> 8) & 255;
                    let len = ctl_val & 255;
                    let lines = self.pat(pat_num as usize).unwrap().lines;
                    if len != 0 && len < lines {
                        let d = lines % len;
                        if d != 0 {
                            self.pattern_shift(pat_num, t, 0, 0, len - d, len);
                            self.change_counter += 1;
                        }
                    }
                }
            }
            0x3B | 0x3C => {
                if pat_num != VIRTUAL_PATTERN {
                    let t = (ctl_val >> 8) & 255;
                    let name = format!("{:X}{:X}", (ctl_val >> 4) & 15, ctl_val & 15);
                    let other = self.pattern_num_by_name(&name);
                    let (src, dest) = if ctl_eff == 0x3B { (pat_num, other) } else { (other, pat_num) };
                    if let (Some(s), Some(d)) = (usize::try_from(src).ok().and_then(|p| self.pat(p)).cloned(), usize::try_from(dest).ok().and_then(|p| self.pat(p).map(|_| p))) {
                        let dp = self.pat(d).unwrap();
                        if t < s.channels && t < dp.channels {
                            let ysize = s.lines.min(dp.lines);
                            let dxs = dp.data_xsize;
                            let dpm = self.pat_mut(d).unwrap();
                            for l in 0..ysize {
                                dpm.data[(t + l * dxs) as usize] = s.data[(t + l * s.data_xsize) as usize];
                            }
                        }
                    }
                    self.change_counter += 1;
                }
            }
            0x3D => {
                if pat_num != VIRTUAL_PATTERN {
                    let pat = self.pat(pat_num as usize).unwrap();
                    let t = ctl_val & 255;
                    let pars_line = (ctl_val >> 8) & 255;
                    if pars_line + 3 <= pat.lines && t < pat.channels {
                        let xs = pat.data_xsize;
                        let mut min = pat.data[(pars_line * xs) as usize].ctl_val as i32;
                        let mut max = pat.data[((pars_line + 1) * xs) as usize].ctl_val as i32;
                        let col = pat.data[((pars_line + 2) * xs) as usize].ctl_val;
                        let r = net::pseudo_random(&mut self.rand_next) as i32;
                        if min > max {
                            std::mem::swap(&mut min, &mut max);
                        }
                        let mut v = ((max - min + 1) * r / 32768 + min) as u16;
                        let t = orig_track - track_num + t;
                        let mut n2 = self.note_at(pat_num, line, t);
                        match col {
                            0 => {
                                if v > 127 {
                                    v = 127;
                                }
                                n2.note = v as u8;
                            }
                            1 => {
                                if v > 128 {
                                    v = 128;
                                }
                                n2.vel = (v + 1) as u8;
                            }
                            2 => n2.module = v.wrapping_add(1),
                            3 => n2.ctl = (n2.ctl & 0x00FF) | ((v & 255) << 8),
                            4 => n2.ctl = (n2.ctl & 0xFF00) | (v & 255),
                            5 => n2.ctl_val = (n2.ctl_val & 0x00FF) | ((v & 255) << 8),
                            6 => n2.ctl_val = (n2.ctl_val & 0xFF00) | (v & 255),
                            7 => n2.ctl_val = v,
                            _ => {}
                        }
                        self.set_note_at(pat_num, line, t, n2);
                        self.change_counter += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// sunvox_handle_track_effects: a tick of a track's timer, portamento,
    /// vibrato, arpeggio and volume slide.
    fn handle_track_effects(&mut self, offset: i32, pat_num: i32, track_num: i32) {
        let tn = track_num as usize;
        let si = self.state_index(pat_num);
        let evt_id = track_num as u32 | if pat_num == VIRTUAL_PATTERN { (pat_num as u32) << 16 } else { (si.unwrap() as u32) << 16 };
        if pat_num != VIRTUAL_PATTERN {
            let speed = self.speed as i32;
            let eff = &mut self.state_mut(si).effects[tn];
            if eff.timer != 0 {
                if eff.timer > 0x40 {
                    let new_timer = (eff.timer as i32 - 0x40) * speed / 32;
                    eff.timer = (new_timer + 1) as u8;
                }
                eff.timer = eff.timer.wrapping_sub(1);
                if eff.timer == 0 && eff.flags & (EFF_FLAG_TIMER_RETRIG | EFF_FLAG_TIMER_CUT | EFF_FLAG_TIMER_CUT2 | EFF_FLAG_TIMER_DELAY) != 0 {
                    eff.flags |= EFF_FLAG_TIMER_HANDLING;
                    let info = self.pats_info[pat_num as usize];
                    let pat = self.pat(pat_num as usize).unwrap();
                    let line_num = self.line_counter - info.x;
                    if line_num >= 0 && line_num < pat.lines && track_num < pat.channels {
                        let n = pat.data[(line_num * pat.data_xsize + track_num) as usize];
                        self.handle_command(offset, n, line_num, pat_num, track_num);
                    }
                    self.state_mut(si).effects[tn].flags &= !EFF_FLAG_TIMER_HANDLING;
                }
            }
        }
        let mod_num = self.state_mut(si).track_module[tn] as i32;
        if (mod_num as u32) >= self.net.mods_num() as u32 {
            return;
        }
        let flags = self.flags;
        let speed_counter = self.speed_counter;
        let speed = self.speed as i32;
        let base_version = self.base_version;
        let eff = &mut self.state_mut(si).effects[tn];
        let mut pitch = eff.cur_pitch;
        if eff.flags & EFF_FLAG_TONE_PORTA != 0 && eff.cur_pitch != eff.target_pitch && !(flags & FLAG_NO_TONE_PORTA_ON_TICK0 != 0 && speed_counter == 0) {
            if eff.cur_pitch < eff.target_pitch {
                eff.cur_pitch += eff.porta_speed as i32;
                if eff.cur_pitch > eff.target_pitch {
                    eff.cur_pitch = eff.target_pitch;
                }
            } else {
                eff.cur_pitch -= eff.porta_speed as i32;
                if eff.cur_pitch < eff.target_pitch {
                    eff.cur_pitch = eff.target_pitch;
                }
            }
            pitch = eff.cur_pitch;
            eff.flags |= EFF_FLAG_PITCH_CHANGED;
        }
        if eff.flags & EFF_FLAG_TONE_VIBRATO != 0 && eff.vib_amp != 0 {
            let phase = eff.vib_phase / 256;
            let mut v = super::dsp::HSIN_TAB[(phase & 255) as usize] as i32;
            if phase & 256 != 0 {
                v = -v;
            }
            pitch += (v * eff.vib_amp as i32) / 64;
            eff.vib_phase += ((eff.vib_freq as i32 * (512 / 32)) * 256) / speed;
            eff.flags |= EFF_FLAG_PITCH_CHANGED;
        }
        if eff.arpeggio != 0 {
            match speed_counter % 3 {
                1 => pitch -= ((eff.arpeggio >> 8) & 255) as i32 * 256,
                2 => pitch -= (eff.arpeggio & 255) as i32 * 256,
                _ => {}
            }
            eff.flags |= EFF_FLAG_ARPEGGIO_IN_PREVIOUS_TICK | EFF_FLAG_PITCH_CHANGED;
        } else if eff.flags & EFF_FLAG_ARPEGGIO_IN_PREVIOUS_TICK != 0 {
            eff.flags &= !EFF_FLAG_ARPEGGIO_IN_PREVIOUS_TICK;
            eff.flags |= EFF_FLAG_PITCH_CHANGED;
        }
        let mut evs: [Option<Event>; 2] = [None, None];
        if eff.flags & EFF_FLAG_PITCH_CHANGED != 0 {
            eff.flags &= !EFF_FLAG_PITCH_CHANGED;
            if base_version < 0x01080000 && pitch > 7680 * 4 {
                pitch = 7680 * 4;
            }
            let mut e = Event::new(Cmd::SetFreq);
            e.id = evt_id;
            e.offset = offset;
            e.set_pitch(pitch);
            e.set_velocity(eff.cur_vel as i32);
            evs[0] = Some(e);
        }
        if eff.vel_speed != 0 && !(flags & FLAG_NO_VOL_SLIDE_ON_TICK0 != 0 && speed_counter == 0) {
            let prev = eff.cur_vel;
            eff.cur_vel = eff.cur_vel.wrapping_add(eff.vel_speed);
            if eff.cur_vel > 256 {
                eff.cur_vel = 256;
            }
            if eff.cur_vel < 0 {
                eff.cur_vel = 0;
            }
            if eff.cur_vel != prev {
                eff.flags |= EFF_FLAG_VEL_CHANGED;
            }
        }
        if eff.flags & EFF_FLAG_VEL_CHANGED != 0 {
            eff.flags &= !EFF_FLAG_VEL_CHANGED;
            let mut e = Event::new(Cmd::SetVelocity);
            e.id = evt_id;
            e.offset = offset;
            e.set_velocity(eff.cur_vel as i32);
            evs[1] = Some(e);
        }
        for e in evs.iter().flatten() {
            self.net.add_event(mod_num as usize, e);
        }
    }

    /// sunvox_render_piece_of_sound: `frames` frames into `out` (Output's
    /// channels, or nothing when `out` is empty), a block at a time.
    pub fn render(&mut self, frames: usize, out: &mut [Vec<Smp>; 2]) {
        let mut ptr = 0usize;
        let max = self.net.max_buf_size as usize;
        while ptr < frames {
            let size = (frames - ptr).min(max);
            self.level1_offset = ptr as i32;
            self.render_level2(size, out, ptr);
            ptr += size;
        }
    }

    fn render_level2(&mut self, frames: usize, out: &mut [Vec<Smp>; 2], out_at: usize) {
        let frames_i = frames as i32;
        let mut ptr: i32 = 0;
        self.net.reset_events();
        let mut one_tick = self.check_speed(ptr);
        loop {
            let mut size = frames_i - ptr;
            let left = one_tick.wrapping_sub(self.tick_counter);
            if size > (left / 256) as i32 {
                size = (left / 256) as i32;
            }
            if left & 255 != 0 {
                size += 1;
            }
            if size > frames_i - ptr {
                size = frames_i - ptr;
            }
            if size < 0 {
                size = 0;
            }
            let mut jump_to_start = false;
            while let Some(cmd) = self.user_commands.pop_front() {
                if (cmd.ch as usize) < MAX_PATTERN_TRACKS {
                    reset_track_effect(&mut self.virtual_pat_state.effects[cmd.ch as usize]);
                    self.handle_command(ptr, cmd.n, 0, VIRTUAL_PATTERN, cmd.ch as i32);
                }
                if cmd.n.note == NOTECMD_PLAY {
                    jump_to_start = true;
                    break;
                }
            }
            if jump_to_start {
                continue;
            }
            if !self.psynth_events.is_empty() {
                let evs = std::mem::take(&mut self.psynth_events);
                for (m, mut e) in evs {
                    e.offset = 0;
                    self.net.add_event(m as usize, &e);
                }
            }
            ptr += size;
            let mut handle_tick = 0;
            self.tick_counter = self.tick_counter.wrapping_add(256 * size as u32);
            if self.tick_counter >= one_tick {
                self.tick_counter %= one_tick;
                handle_tick = 1;
            }
            while handle_tick > 0 {
                handle_tick -= 1;
                if self.playing {
                    self.speed_counter += 1;
                    if self.speed_counter >= self.speed as i32 {
                        self.speed_counter = 0;
                        // The song's end with autostop: out of the tick loop.
                        if !self.next_line(ptr) {
                            break;
                        }
                        one_tick = self.check_speed(ptr);
                    }
                    let mut handled = [0u32; SUPERTRACK_WORDS];
                    for i in 0..self.pat_state.len() {
                        let sp = self.cur_playing_pats[i];
                        if sp == -1 {
                            break;
                        }
                        let pat_num = self.sorted_pats[sp as usize];
                        let st = self.pats_info[pat_num].state_ptr;
                        let bit = 1u32 << (st & 31);
                        if handled[st / 32] & bit == 0 {
                            let mt = self.pat_state[st].mutable_tracks as i32;
                            for track in 0..mt {
                                self.handle_track_effects(ptr, pat_num as i32, track);
                            }
                            handled[st / 32] |= bit;
                        }
                    }
                }
                for ch in 0..self.virtual_pat_tracks {
                    self.handle_track_effects(ptr, VIRTUAL_PATTERN, ch);
                }
            }
            if ptr >= frames_i {
                break;
            }
        }
        self.net.render_setup(frames_i);
        self.net.render_all();
        let m0 = &self.net.mods[0];
        if !out[0].is_empty() {
            for ch in 0..2 {
                let src = if (ch as i32) < m0.input_channels && !m0.chin[ch].is_empty() { ch } else { 0 };
                out[ch][out_at..out_at + frames].copy_from_slice(&m0.chin[src][..frames]);
            }
        }
    }

    /// A new line: the line counter moved on (jumps, the end, a pattern
    /// played alone), the patterns that end let go, those that start taken
    /// up, and the line's cells acted on. False if the song stopped.
    fn next_line(&mut self, ptr: i32) -> bool {
        let mut new_line = self.line_counter + 1;
        if self.jump_request {
            new_line = self.jump_request_line;
            self.jump_request = false;
        }
        let pnum = self.single_pattern_play;
        if pnum >= 0 {
            if let Some(p) = self.pat(pnum as usize) {
                if new_line >= self.pats_info[pnum as usize].x + p.lines {
                    if self.next_single_pattern_play >= 0 && self.next_single_pattern_play != pnum {
                        self.single_pattern_play = self.next_single_pattern_play;
                    }
                    new_line = self.pats_info[self.single_pattern_play as usize].x;
                }
            }
        }
        if new_line >= self.proj_lines {
            if self.stop_at_the_end_of_proj {
                let stop = Note { note: NOTECMD_STOP, ..Note::default() };
                self.handle_command(ptr, stop, 0, VIRTUAL_PATTERN, 0);
                return false;
            }
            new_line = self.restart_pos;
        }
        self.line_counter += 1;
        if new_line != self.line_counter {
            self.reset_timeline_activity(ptr);
            self.line_counter = new_line;
            self.select_current_playing_patterns(0);
        }
        let size = self.pat_state.len();
        let mut p = 0usize;
        self.temp_pats[0] = -1;
        for i in 0..size {
            let cp = self.cur_playing_pats[i];
            if cp == -1 {
                break;
            }
            if (cp as usize) < self.sorted_pats.len() {
                let pat_num = self.sorted_pats[cp as usize];
                if let Some(pat) = self.pat(pat_num) {
                    let info = self.pats_info[pat_num];
                    if self.line_counter >= info.x && self.line_counter < info.x + pat.lines {
                        self.temp_pats[p] = cp;
                        p += 1;
                    } else {
                        let end_flags = pat.flags;
                        let st = info.state_ptr;
                        if self.flags & FLAG_SUPERTRACKS != 0 {
                            if end_flags & PAT_FLAG_NO_NOTES_OFF == 0 && info.track_status != 0 {
                                for a in 0..MAX_PATTERN_TRACKS {
                                    if info.track_status & (1 << a) != 0 && self.pat_state[st].track_status & (1 << a) != 0 {
                                        self.note_off_track(st, a, ptr);
                                    }
                                }
                            }
                        } else {
                            for a in 0..MAX_PATTERN_TRACKS {
                                if self.pat_state[st].track_status & (1 << a) != 0 {
                                    self.note_off_track(st, a, ptr);
                                }
                            }
                            self.pat_state[st].busy = false;
                        }
                        self.pats_info[pat_num].track_status = 0;
                    }
                }
            }
            self.last_sort_pat = cp;
        }
        let mut search_again = true;
        while search_again && p < size {
            search_again = false;
            let n = self.sorted_pats.len() as i32;
            if n != 0 && self.last_sort_pat < n - 1 {
                let mut next = (self.last_sort_pat + 1) as usize;
                if self.has_pat(self.sorted_pats[next]) {
                    let next_time = self.pats_info[self.sorted_pats[next]].x;
                    if next_time == self.line_counter {
                        while next < self.sorted_pats.len() {
                            let info = self.pats_info[self.sorted_pats[next]];
                            if info.x != next_time {
                                break;
                            }
                            self.temp_pats[p] = next as i32;
                            let st = &mut self.pat_state[info.state_ptr];
                            if !st.busy {
                                clean_pattern_state(st);
                                st.busy = true;
                            }
                            p += 1;
                            if p >= size {
                                break;
                            }
                            next += 1;
                        }
                    } else if next_time < self.line_counter {
                        search_again = true;
                        self.last_sort_pat += 1;
                    }
                }
            }
        }
        let mut handled = [0u32; SUPERTRACK_WORDS];
        for i in 0..p {
            self.cur_playing_pats[i] = self.temp_pats[i];
            let pat_num = self.sorted_pats[self.temp_pats[i] as usize];
            let channels = self.pat(pat_num).unwrap().channels;
            let st = self.pats_info[pat_num].state_ptr;
            let bit = 1u32 << (st & 31);
            let state = &mut self.pat_state[st];
            let prev;
            if handled[st / 32] & bit == 0 {
                prev = 0;
                state.mutable_tracks = channels as u8;
                handled[st / 32] |= bit;
            } else {
                prev = state.mutable_tracks as i32;
                if channels > state.mutable_tracks as i32 {
                    state.mutable_tracks = channels as u8;
                }
            }
            for t in prev..state.mutable_tracks as i32 {
                reset_track_effect(&mut state.effects[t as usize]);
            }
        }
        if p < size {
            self.cur_playing_pats[p] = -1;
        }
        for i in 0..p {
            let pat_num = self.sorted_pats[self.temp_pats[i] as usize];
            let info = self.pats_info[pat_num];
            let st = info.state_ptr;
            let mut mute = info.flags & PAT_INFO_FLAG_MUTE != 0 || (self.pats_solo_mode && info.flags & PAT_INFO_FLAG_SOLO == 0);
            if !mute && self.flags & FLAG_SUPERTRACKS != 0 {
                let stk = (info.y + 16) / 32;
                if self.supertrack_mute[(stk / 32) as usize] & (1 << (stk & 31)) != 0 {
                    mute = true;
                }
            }
            if mute {
                if info.track_status != 0 {
                    for a in 0..MAX_PATTERN_TRACKS {
                        if info.track_status & (1 << a) != 0 && self.pat_state[st].track_status & (1 << a) != 0 {
                            self.note_off_track(st, a, ptr);
                        }
                    }
                }
                self.pats_info[pat_num].track_status = 0;
                continue;
            }
            let line = self.line_counter - info.x;
            let channels = self.pat(pat_num).unwrap().channels;
            for track in 0..channels {
                let n = self.note_at(pat_num as i32, line, track);
                self.handle_command(ptr, n, line, pat_num as i32, track);
            }
            if !self.playing {
                break;
            }
        }
        if self.flags & FLAG_SUPERTRACKS != 0 {
            for w in 0..SUPERTRACK_WORDS {
                let mute = self.supertrack_mute[w];
                if mute == 0 {
                    continue;
                }
                for b in 0..32 {
                    if mute & (1 << b) == 0 {
                        continue;
                    }
                    let st = w * 32 + b;
                    if st >= self.pat_state.len() || self.pat_state[st].track_status == 0 {
                        continue;
                    }
                    for a in 0..MAX_PATTERN_TRACKS {
                        let m = self.pat_state[st].track_module[a] as i32;
                        if (m as u32) < self.net.mods_num() as u32 {
                            self.note_off_track(st, a, ptr);
                        }
                    }
                }
            }
        }
        true
    }

    /// A track's note off, its bit in the state cleared.
    fn note_off_track(&mut self, st: usize, a: usize, ptr: i32) {
        let m = self.pat_state[st].track_module[a] as i32;
        if (m as u32) < self.net.mods_num() as u32 {
            let mut e = Event::new(Cmd::NoteOff);
            e.id = ((st as u32) << 16) | a as u32;
            e.offset = ptr;
            e.set_velocity(256);
            self.net.add_event(m as usize, &e);
            self.pat_state[st].track_status &= !(1 << a);
        }
    }
}

/// SUNVOX_ENGINE_VERSION: 2.1.4d.
pub(crate) const SUNVOX_ENGINE_VERSION: u32 = (2 << 24) | (1 << 16) | (4 << 8) | 3;
