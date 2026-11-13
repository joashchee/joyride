// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's SiMMLTrack, SiMMLChannelSetting, SiMMLEnvelopTable and
// SiMMLVoice (org.si.sion.sequencer, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! A track plays one voice: the channel it controls, the module type and
//! tone it was set to, its gate (quantize) and key-on delay, the envelope
//! tables a note walks along (tone, amplitude, pitch, note and filter, on
//! key on and on key off), portamento and the modulation envelopes.

use super::fm::*;
use super::module::*;
use super::pcm::SampleBank;
use super::table::*;
use std::sync::Arc;

pub const SWEEP_FINESS: i32 = 128;
pub const FIXED_BITS_T: i32 = 16;
pub const SWEEP_MAX: i32 = 8192 << FIXED_BITS_T;

pub const MASK_VOLUME: i32 = 1;
pub const MASK_PAN: i32 = 2;
pub const MASK_QUANTIZE: i32 = 4;
pub const MASK_OPERATOR: i32 = 8;
pub const MASK_ENVELOP: i32 = 16;
pub const MASK_MODULATE: i32 = 32;
pub const MASK_SLUR: i32 = 64;

// Module types.
pub const MT_PSG: usize = 0;
pub const MT_APU: usize = 1;
pub const MT_NOISE: usize = 2;
pub const MT_MA3: usize = 3;
pub const MT_CUSTOM: usize = 4;
pub const MT_ALL: usize = 5;
pub const MT_FM: usize = 6;
pub const MT_PCM: usize = 7;
pub const MT_PULSE: usize = 8;
pub const MT_RAMP: usize = 9;
pub const MT_SAMPLE: usize = 10;
pub const MT_KS: usize = 11;
pub const MT_GB: usize = 12;
pub const MT_VRC6: usize = 13;
pub const MT_SID: usize = 14;
pub const MT_MAX: usize = 21;

const NORMAL: i32 = 0;
const ENVELOP: i32 = 2;

/// A table envelope: numbers a note walks along, then loops to a point
/// or stays at the last (`SiMMLEnvelopTable`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnvTable {
    pub values: Vec<i32>,
    pub loop_to: usize,
    pub is_zero: bool,
}

impl EnvTable {
    pub fn new(values: Vec<i32>, loop_point: Option<usize>) -> EnvTable {
        let last = values.len().saturating_sub(1);
        EnvTable { loop_to: loop_point.map_or(last, |l| l.min(last)), values, is_zero: false }
    }

    /// The table of a single zero, the default of the pitch and note envelopes.
    pub fn zero() -> EnvTable {
        EnvTable { values: vec![0], loop_to: 0, is_zero: true }
    }

    /// `toVector`: `length` numbers, 0 past the end, clamped.
    pub fn to_vector(&self, length: usize, min: i32, max: i32) -> Vec<i32> {
        let mut out = Vec::with_capacity(length);
        let mut pos = Some(0usize);
        for _ in 0..length {
            let n = match pos {
                Some(p) => {
                    let n = self.values[p];
                    pos = Some(self.next(p));
                    n
                }
                None => 0,
            };
            out.push(n.clamp(min, max));
        }
        out
    }

    /// The position after `pos`.
    pub fn next(&self, pos: usize) -> usize {
        if pos + 1 < self.values.len() {
            pos + 1
        } else {
            self.loop_to
        }
    }
}

/// A pointer into an envelope table.
#[derive(Debug, Clone)]
pub struct EnvPtr {
    pub table: Arc<EnvTable>,
    pub pos: usize,
}

impl EnvPtr {
    fn new(table: &Arc<EnvTable>) -> Option<EnvPtr> {
        if table.values.is_empty() {
            None
        } else {
            Some(EnvPtr { table: table.clone(), pos: 0 })
        }
    }

    fn value(&self) -> i32 {
        self.table.values[self.pos]
    }

    fn advance(&mut self) {
        self.pos = self.table.next(self.pos);
    }
}

/// `SiMMLVoice`: what `#@`, `#OPM@` and the like define.
#[derive(Debug, Clone)]
pub struct Voice {
    pub module_type: i32,
    pub channel_num: i32,
    pub tone_num: i32,
    pub channel_param: ChannelParam,
    pub update_track_parameters: bool,
}

impl Default for Voice {
    fn default() -> Voice {
        Voice { module_type: 6, channel_num: 0, tone_num: -1, channel_param: ChannelParam::default(), update_track_parameters: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectTone {
    Nop,
    Normal,
    Fm,
}

/// `SiMMLChannelSetting`: what a module type is.
#[derive(Debug, Clone)]
pub struct ChannelSetting {
    pub type_: usize,
    pub select_tone_type: SelectTone,
    pub pg_type_list: Vec<usize>,
    pub pt_type_list: Vec<usize>,
    pub init_voice_index: usize,
    pub voice_index_table: Vec<i32>,
    pub default_ope_count: i32,
}

impl ChannelSetting {
    fn new(table: &Table, type_: usize, offset: usize, length: usize, step: usize, channel_count: usize) -> ChannelSetting {
        let mut pg = Vec::with_capacity(length);
        let mut pt = Vec::with_capacity(length);
        let mut idx = offset;
        for _ in 0..length {
            pg.push(idx);
            pt.push(table.get_wave_table(idx).default_pt_type);
            idx += step;
        }
        ChannelSetting { type_, select_tone_type: SelectTone::Normal, pg_type_list: pg, pt_type_list: pt, init_voice_index: 0, voice_index_table: (0..channel_count as i32).collect(), default_ope_count: 1 }
    }
}

/// The module types' settings (`SiMMLTable`).
pub fn channel_settings(table: &Table) -> Vec<ChannelSetting> {
    let mut s: Vec<ChannelSetting> = vec![];
    let mk = |t: usize, off: usize, len: usize, step: usize, ch: usize| ChannelSetting::new(table, t, off, len, step, ch);
    s.push(mk(MT_PSG, PG_SQUARE, 3, 1, 4));
    s.push(mk(MT_APU, PG_PULSE, 11, 2, 4));
    s.push(mk(MT_NOISE, PG_NOISE_WHITE, 16, 1, 16));
    s.push(mk(MT_MA3, PG_MA3_WAVE, 32, 1, 32));
    s.push(mk(MT_CUSTOM, PG_CUSTOM, 256, 1, 256));
    s.push(mk(MT_ALL, PG_SINE, 512, 1, 512));
    s.push(mk(MT_FM, PG_SINE, 1, 1, 1));
    s.push(mk(MT_PCM, PG_PCM, 128, 1, 128));
    s.push(mk(MT_PULSE, PG_PULSE, 32, 1, 32));
    s.push(mk(MT_RAMP, PG_RAMP, 128, 1, 128));
    s.push(mk(MT_SAMPLE, 0, 4, 1, 4));
    s.push(mk(MT_KS, 0, 3, 1, 3));
    s.push(mk(MT_GB, PG_PULSE, 11, 2, 4));
    s.push(mk(MT_VRC6, PG_PULSE, 9, 1, 3));
    s.push(mk(MT_SID, PG_PULSE, 12, 1, 3));
    // PSG
    {
        let ms = &mut s[MT_PSG];
        ms.pg_type_list[0] = PG_SQUARE;
        ms.pg_type_list[1] = PG_NOISE_PULSE;
        ms.pg_type_list[2] = PG_PC_NZ_16BIT;
        ms.pt_type_list[0] = PT_PSG;
        ms.pt_type_list[1] = PT_PSG_NOISE;
        ms.pt_type_list[2] = PT_PSG;
        ms.voice_index_table = vec![0, 0, 0, 1];
    }
    // APU
    {
        let ms = &mut s[MT_APU];
        ms.pg_type_list[8] = PG_TRIANGLE_FC;
        ms.pg_type_list[9] = PG_NOISE_PULSE;
        ms.pg_type_list[10] = PG_NOISE_SHORT;
        for i in 0..9 {
            ms.pt_type_list[i] = PT_PSG;
        }
        for i in 9..11 {
            ms.pt_type_list[i] = PT_APU_NOISE;
        }
        ms.init_voice_index = 1;
        ms.voice_index_table = vec![4, 4, 8, 9];
    }
    // GB
    {
        let ms = &mut s[MT_GB];
        ms.pg_type_list[8] = PG_CUSTOM;
        ms.pg_type_list[9] = PG_NOISE_PULSE;
        ms.pg_type_list[10] = PG_NOISE_GB_SHORT;
        for i in 0..9 {
            ms.pt_type_list[i] = PT_PSG;
        }
        for i in 9..11 {
            ms.pt_type_list[i] = PT_GB_NOISE;
        }
        ms.init_voice_index = 1;
        ms.voice_index_table = vec![4, 4, 8, 9];
    }
    // VRC6
    {
        let ms = &mut s[MT_VRC6];
        ms.pg_type_list[8] = PG_SAW_VC6;
        ms.pt_type_list[8] = PT_PSG;
        ms.init_voice_index = 1;
        ms.voice_index_table = vec![7, 7, 8];
    }
    // SID
    {
        let ms = &mut s[MT_SID];
        ms.pg_type_list[8] = PG_TRIANGLE;
        ms.pg_type_list[9] = PG_SAW_UP;
        ms.pg_type_list[10] = PG_SAW_VC6;
        ms.pg_type_list[11] = PG_NOISE_PULSE;
        for i in 0..11 {
            ms.pt_type_list[i] = PT_PSG;
        }
        ms.pt_type_list[11] = PT_OPM_NOISE;
        ms.init_voice_index = 1;
        ms.voice_index_table = vec![7, 7, 7];
    }
    s[MT_FM].select_tone_type = SelectTone::Fm;
    s[MT_SAMPLE].select_tone_type = SelectTone::Normal;
    // The chip-specific FM types (15 to 20) have no setting of their own
    // in SiON's table: they play as FM.
    while s.len() < MT_MAX {
        let mut fm = s[MT_FM].clone();
        fm.type_ = s.len();
        s.push(fm);
    }
    s
}

/// What the tracks share: the tables a song defines and the module
/// types' settings.
pub struct Shared {
    pub table: Arc<Table>,
    pub settings: Vec<ChannelSetting>,
    pub envelopes: Vec<Option<Arc<EnvTable>>>,
    pub voices: Vec<Option<Arc<Voice>>>,
    /// The waves of the PCM module and the sampler.
    pub samples: Arc<SampleBank>,
}

impl Shared {
    pub fn new(table: Arc<Table>) -> Shared {
        let settings = channel_settings(&table);
        Shared { table, settings, envelopes: vec![None; 512], voices: vec![None; 256], samples: Arc::new(SampleBank::new()) }
    }

    pub fn envelope(&self, index: i32) -> Option<Arc<EnvTable>> {
        if !(0..512).contains(&index) {
            return None;
        }
        self.envelopes[index as usize].clone()
    }

    pub fn voice(&self, index: i32) -> Option<Arc<Voice>> {
        if !(0..256).contains(&index) {
            return None;
        }
        self.voices[index as usize].clone()
    }
}

/// `SiMMLTrack`.
pub struct Track {
    pub channel: FmChannel,
    pub note_shift: i32,
    pub pitch_shift: i32,
    pub key_on_delay: i32,
    pub quant_ratio: f64,
    pub quant_count: i32,
    pub event_mask: i32,
    key_on_counter: i32,
    key_on_length: i32,
    flag_no_key_on: bool,
    process_mode: i32,
    setting: usize,
    velocity_mode: usize,
    expression_mode: usize,
    velocity: i32,
    expression: i32,
    pitch_index: i32,
    pitch_bend: i32,
    voice_index: i32,
    note: i32,
    default_fps: i32,
    pub channel_number: i32,
    pub vcommand_shift: i32,
    set_process_mode: [i32; 2],
    set_env_exp: [Option<Arc<EnvTable>>; 2],
    set_env_voice: [Option<Arc<EnvTable>>; 2],
    set_env_note: [Arc<EnvTable>; 2],
    set_env_pitch: [Arc<EnvTable>; 2],
    set_env_filter: [Option<Arc<EnvTable>>; 2],
    set_exp_offset: [bool; 2],
    pns_or: [bool; 2],
    set_cnt_exp: [i32; 2],
    set_cnt_voice: [i32; 2],
    set_cnt_note: [i32; 2],
    set_cnt_pitch: [i32; 2],
    set_cnt_filter: [i32; 2],
    table_env_ma: [Option<Arc<Vec<i32>>>; 2],
    table_env_mp: [Option<Arc<Vec<i32>>>; 2],
    set_sweep_step: [i32; 2],
    set_sweep_end: [i32; 2],
    env_interval: i32,
    zero: Arc<EnvTable>,
    env_exp: Option<EnvPtr>,
    env_voice: Option<EnvPtr>,
    env_note: EnvPtr,
    env_pitch: EnvPtr,
    env_filter: Option<EnvPtr>,
    cnt_exp: i32,
    max_cnt_exp: i32,
    cnt_voice: i32,
    max_cnt_voice: i32,
    cnt_note: i32,
    max_cnt_note: i32,
    cnt_pitch: i32,
    max_cnt_pitch: i32,
    cnt_filter: i32,
    max_cnt_filter: i32,
    env_mp: Option<(Arc<Vec<i32>>, usize)>,
    env_ma: Option<(Arc<Vec<i32>>, usize)>,
    sweep_step: i32,
    sweep_end: i32,
    sweep_pitch: i32,
    env_exp_offset: i32,
    env_pitch_active: bool,
    residue: i32,
}

impl Track {
    pub fn new(module: &Module) -> Track {
        let zero = Arc::new(EnvTable::zero());
        let z = |n: &Arc<EnvTable>| -> EnvPtr { EnvPtr { table: n.clone(), pos: 0 } };
        let mut t = Track {
            channel: FmChannel::new(module),
            note_shift: 0,
            pitch_shift: 0,
            key_on_delay: 0,
            quant_ratio: 0.0,
            quant_count: 0,
            event_mask: 0,
            key_on_counter: 0,
            key_on_length: 0,
            flag_no_key_on: false,
            process_mode: NORMAL,
            setting: MT_PSG,
            velocity_mode: VM_LINEAR,
            expression_mode: VM_LINEAR,
            velocity: 256,
            expression: 128,
            pitch_index: 0,
            pitch_bend: 0,
            voice_index: 0,
            note: -1,
            default_fps: 60,
            channel_number: 0,
            vcommand_shift: 4,
            set_process_mode: [NORMAL; 2],
            set_env_exp: [None, None],
            set_env_voice: [None, None],
            set_env_note: [zero.clone(), zero.clone()],
            set_env_pitch: [zero.clone(), zero.clone()],
            set_env_filter: [None, None],
            set_exp_offset: [false; 2],
            pns_or: [false; 2],
            set_cnt_exp: [1; 2],
            set_cnt_voice: [1; 2],
            set_cnt_note: [1; 2],
            set_cnt_pitch: [1; 2],
            set_cnt_filter: [1; 2],
            table_env_ma: [None, None],
            table_env_mp: [None, None],
            set_sweep_step: [0; 2],
            set_sweep_end: [0; 2],
            env_interval: 0,
            env_exp: None,
            env_voice: None,
            env_note: z(&zero),
            env_pitch: z(&zero),
            env_filter: None,
            cnt_exp: 0,
            max_cnt_exp: 0,
            cnt_voice: 0,
            max_cnt_voice: 0,
            cnt_note: 0,
            max_cnt_note: 0,
            cnt_pitch: 0,
            max_cnt_pitch: 0,
            cnt_filter: 0,
            max_cnt_filter: 0,
            env_mp: None,
            env_ma: None,
            sweep_step: 0,
            sweep_end: 0,
            sweep_pitch: 0,
            env_exp_offset: 0,
            env_pitch_active: false,
            residue: 0,
            zero,
        };
        t.env_interval = module.table.rate / 60;
        t
    }

    pub fn velocity(&self) -> i32 {
        self.velocity
    }

    pub fn expression(&self) -> i32 {
        self.expression
    }

    pub fn note(&self) -> i32 {
        self.note
    }

    pub fn voice_index(&self) -> i32 {
        self.voice_index
    }

    pub fn setting_type(&self) -> usize {
        self.setting
    }

    pub fn set_velocity(&mut self, v: i32) {
        self.velocity = v.clamp(0, 512);
        self.channel.offset_volume(self.expression, self.velocity);
    }

    pub fn set_expression(&mut self, x: i32) {
        self.expression = x.clamp(0, 128);
        self.channel.offset_volume(self.expression, self.velocity);
    }

    pub fn set_velocity_mode(&mut self, mode: i32) {
        self.velocity_mode = if (0..VM_MAX as i32).contains(&mode) { mode as usize } else { VM_LINEAR };
        self.channel.set_volume_tables(self.velocity_mode, self.expression_mode);
    }

    pub fn set_expression_mode(&mut self, mode: i32) {
        self.expression_mode = if (0..VM_MAX as i32).contains(&mode) { mode as usize } else { VM_LINEAR };
        self.channel.set_volume_tables(self.velocity_mode, self.expression_mode);
    }

    /// `_mmlVCommand`.
    pub fn mml_v_command(&mut self, v: i32) {
        self.set_velocity(v << self.vcommand_shift);
    }

    /// `_mmlVShift`.
    pub fn mml_v_shift(&mut self, v: i32) {
        self.set_velocity(self.velocity + (v << self.vcommand_shift));
    }

    /// `_reset`: back to the start of a song.
    pub fn reset(&mut self, buffer_index: usize, shared: &Shared, module: &Module, vcommand_shift: i32, velocity_mode: i32, expression_mode: i32) {
        self.setting = MT_PSG;
        self.channel_number = 0;
        self.vcommand_shift = vcommand_shift;
        self.velocity_mode = if (0..VM_MAX as i32).contains(&velocity_mode) { velocity_mode as usize } else { VM_LINEAR };
        self.expression_mode = if (0..VM_MAX as i32).contains(&expression_mode) { expression_mode as usize } else { VM_LINEAR };
        self.velocity = 256;
        self.expression = 128;
        self.pitch_bend = 0;
        self.note = -1;
        self.voice_index = self.initialize_tone(shared, module, None, buffer_index);
        self.channel.set_volume_tables(self.velocity_mode, self.expression_mode);
        self.note_shift = 0;
        self.pitch_shift = 0;
        self.key_on_counter = 0;
        self.key_on_length = 0;
        self.flag_no_key_on = false;
        self.process_mode = NORMAL;
        self.key_on_delay = 0;
        self.quant_ratio = 1.0;
        self.quant_count = 0;
        self.event_mask = 0;
        self.env_pitch_active = false;
        self.pitch_index = 0;
        self.sweep_pitch = 0;
        self.env_exp_offset = 0;
        self.set_envelop_fps(self.default_fps);
        self.residue = 0;
        self.env_exp = None;
        self.env_voice = None;
        self.env_note = EnvPtr { table: self.zero.clone(), pos: 0 };
        self.env_pitch = EnvPtr { table: self.zero.clone(), pos: 0 };
        self.env_filter = None;
        self.env_ma = None;
        self.env_mp = None;
        for i in 0..2 {
            self.set_process_mode[i] = NORMAL;
            self.set_env_exp[i] = None;
            self.set_env_voice[i] = None;
            self.set_env_note[i] = self.zero.clone();
            self.set_env_pitch[i] = self.zero.clone();
            self.set_env_filter[i] = None;
            self.pns_or[i] = false;
            self.set_exp_offset[i] = false;
            self.set_cnt_exp[i] = 1;
            self.set_cnt_voice[i] = 1;
            self.set_cnt_note[i] = 1;
            self.set_cnt_pitch[i] = 1;
            self.set_cnt_filter[i] = 1;
            self.set_sweep_step[i] = 0;
            self.set_sweep_end[i] = 0;
            self.table_env_ma[i] = None;
            self.table_env_mp[i] = None;
        }
    }

    pub fn set_default_fps(&mut self, fps: i32) {
        self.default_fps = fps;
    }

    pub fn reset_volume_offset(&mut self) {
        self.channel.offset_volume(self.expression, self.velocity);
    }

    // module type and tone

    /// `SiMMLChannelSetting.initializeTone`: the channel back to its start
    /// for a module type; the voice index it picks (-1 when no channel
    /// number was asked for).
    fn initialize_tone(&mut self, shared: &Shared, module: &Module, ch_num: Option<i32>, buffer_index: usize) -> i32 {
        let s = &shared.settings[self.setting];
        self.channel.set_ks_mode(self.setting == MT_KS);
        self.channel.set_kind(match self.setting {
            MT_PCM => Kind::Pcm,
            MT_SAMPLE => Kind::Sampler,
            _ => Kind::Fm,
        });
        self.channel.reinitialize(buffer_index, module);
        self.reset_volume_offset();
        let ch = ch_num.unwrap_or(i32::MIN);
        let mut voice_index = s.init_voice_index as i32;
        let mut restrict = ch;
        if ch >= 0 && (ch as usize) < s.voice_index_table.len() {
            voice_index = s.voice_index_table[ch as usize];
        } else {
            restrict = 0;
        }
        self.channel_number = if ch < 0 { -1 } else { ch };
        self.channel.set_channel_number(restrict);
        self.channel.set_algorism(s.default_ope_count, 0);
        self.select_tone(shared, module, voice_index);
        if ch == -1 {
            -1
        } else {
            voice_index
        }
    }

    /// `selectTone`: a tone of the module type; the init sequence of an FM
    /// voice, if it has one.
    pub fn select_tone(&mut self, shared: &Shared, _module: &Module, voice_index: i32) -> Option<String> {
        if voice_index == -1 {
            return None;
        }
        let s = &shared.settings[self.setting];
        match s.select_tone_type {
            SelectTone::Normal => {
                let mut v = voice_index;
                if v < 0 || v as usize >= s.pg_type_list.len() {
                    v = s.init_voice_index as i32;
                }
                self.channel.set_type_with(s.pg_type_list[v as usize] as i32, s.pt_type_list[v as usize], &shared.samples);
                None
            }
            SelectTone::Fm => {
                let mut v = voice_index;
                if !(0..256).contains(&v) {
                    v = 0;
                }
                if let Some(voice) = shared.voice(v) {
                    self.channel.set_channel_param(&voice.channel_param, false, false);
                    self.reset_volume_offset();
                    if voice.channel_param.init_sequence.is_empty() {
                        return None;
                    }
                    return Some(voice.channel_param.init_sequence.clone());
                }
                None
            }
            SelectTone::Nop => None,
        }
    }

    /// `onUpdateRegister`'s default: a register write to the channel.
    pub fn update_register(&mut self, addr: i32, data: i32) {
        self.channel.set_register(addr, data);
    }

    /// `setChannelModuleType` (`%`).
    pub fn set_channel_module_type(&mut self, shared: &Shared, module: &Module, type_: usize, channel_num: i32, tone_num: i32) {
        self.setting = type_.min(MT_MAX - 1);
        self.voice_index = self.initialize_tone(shared, module, Some(channel_num), self.channel.buffer_index());
        if tone_num >= 0 {
            self.voice_index = tone_num;
            self.select_tone(shared, module, tone_num);
        }
    }

    /// `_setChannelParameters` (`@`): the tone, then the operator parameters.
    pub fn set_channel_parameters(&mut self, shared: &Shared, module: &Module, param: &[i32]) -> Option<String> {
        let mut seq = None;
        if param[0] != i32::MIN {
            seq = self.select_tone(shared, module, param[0]);
            self.voice_index = param[0];
        }
        self.channel.set_parameters(param);
        // A Karplus-Strong seed that is an FM voice.
        let (seed, index) = self.channel.ks_seed();
        if self.channel.is_ks() && seed == KS_SEED_FM && (0..256).contains(&index) {
            if let Some(voice) = shared.voice(index) {
                self.channel.set_channel_param(&voice.channel_param, false, true);
            }
        }
        seq
    }

    // gate, slur, portamento

    pub fn set_envelop_fps(&mut self, fps: i32) {
        self.env_interval = self.channel_rate() / fps;
    }

    fn channel_rate(&self) -> i32 {
        // SiOPMTable.instance.rate
        self.channel.rate()
    }

    pub fn set_portament(&mut self, frame: i32) {
        self.set_sweep_step[1] = frame;
        if frame != 0 {
            self.pns_or[1] = true;
            self.envelop_on(1);
        } else {
            self.envelop_off(1);
        }
    }

    pub fn set_release_sweep(&mut self, sweep: i32) {
        self.set_sweep_step[0] = sweep << FIXED_BITS_T;
        self.set_sweep_end[0] = if sweep < 0 { 0 } else { SWEEP_MAX };
        if sweep != 0 {
            self.pns_or[0] = true;
            self.envelop_on(0);
        } else {
            self.envelop_off(0);
        }
    }

    pub fn set_modulation_envelop(&mut self, is_pitch_mod: bool, depth: i32, end_depth: i32, delay: i32, term: i32) {
        let slot = if is_pitch_mod { &mut self.table_env_mp } else { &mut self.table_env_ma };
        slot[1] = None;
        if (0 <= depth && depth < end_depth) || (depth < 0 && depth > end_depth) {
            let table = make_modulation_table(depth, end_depth, delay, term);
            slot[1] = Some(Arc::new(table));
            self.envelop_on(1);
        } else {
            if is_pitch_mod {
                self.channel.set_pitch_modulation(depth);
            } else {
                self.channel.set_amplitude_modulation(depth);
            }
            self.envelop_off(1);
        }
    }

    pub fn set_tone_envelop(&mut self, note_on: usize, table: Option<Arc<EnvTable>>, step: i32) {
        match table {
            Some(t) if step != 0 => {
                self.set_env_voice[note_on] = Some(t);
                self.set_cnt_voice[note_on] = step;
                self.envelop_on(note_on);
            }
            _ => {
                self.set_env_voice[note_on] = None;
                self.envelop_off(note_on);
            }
        }
    }

    pub fn set_amplitude_envelop(&mut self, note_on: usize, table: Option<Arc<EnvTable>>, step: i32, offset: bool) {
        match table {
            Some(t) if step != 0 => {
                self.set_env_exp[note_on] = Some(t);
                self.set_cnt_exp[note_on] = step;
                self.set_exp_offset[note_on] = offset;
                self.envelop_on(note_on);
            }
            _ => {
                self.set_env_exp[note_on] = None;
                self.envelop_off(note_on);
            }
        }
    }

    pub fn set_filter_envelop(&mut self, note_on: usize, table: Option<Arc<EnvTable>>, step: i32) {
        match table {
            Some(t) if step != 0 => {
                self.set_env_filter[note_on] = Some(t);
                self.set_cnt_filter[note_on] = step;
                self.envelop_on(note_on);
            }
            _ => {
                self.set_env_filter[note_on] = None;
                self.envelop_off(note_on);
            }
        }
    }

    pub fn set_pitch_envelop(&mut self, note_on: usize, table: Option<Arc<EnvTable>>, step: i32) {
        match table {
            Some(t) if step != 0 => {
                self.set_env_pitch[note_on] = t;
                self.set_cnt_pitch[note_on] = step;
                self.pns_or[note_on] = true;
                self.envelop_on(note_on);
            }
            _ => {
                self.set_env_pitch[note_on] = self.zero.clone();
                self.envelop_off(note_on);
            }
        }
    }

    pub fn set_note_envelop(&mut self, note_on: usize, table: Option<Arc<EnvTable>>, step: i32) {
        match table {
            Some(t) if step != 0 => {
                self.set_env_note[note_on] = t;
                self.set_cnt_note[note_on] = step;
                self.pns_or[note_on] = true;
                self.envelop_on(note_on);
            }
            _ => {
                self.set_env_note[note_on] = self.zero.clone();
                self.envelop_off(note_on);
            }
        }
    }

    fn envelop_off(&mut self, note_on: usize) {
        if self.set_sweep_step[note_on] == 0 && self.set_env_pitch[note_on].is_zero && self.set_env_note[note_on].is_zero {
            self.pns_or[note_on] = false;
        }
        if !self.pns_or[note_on]
            && self.table_env_ma[note_on].is_none()
            && self.table_env_mp[note_on].is_none()
            && self.set_env_exp[note_on].is_none()
            && self.set_env_filter[note_on].is_none()
            && self.set_env_voice[note_on].is_none()
        {
            self.set_process_mode[note_on] = NORMAL;
        }
    }

    fn envelop_on(&mut self, note_on: usize) {
        self.set_process_mode[note_on] = ENVELOP;
    }

    // events

    /// `_onRestEvent`.
    pub fn on_rest_event(&mut self) {
        self.flag_no_key_on = false;
    }

    /// `_onNoteEvent`: a note of `length` samples.
    pub fn on_note_event(&mut self, shared: &Shared, module: &mut Module, note: i32, length: i32) {
        self.key_on_length = ((f64::from(length) * self.quant_ratio) as i32).wrapping_sub(self.quant_count).wrapping_sub(self.key_on_delay);
        if self.key_on_length < 1 {
            self.key_on_length = 1;
        }
        self.mml_key_on(shared, module, note);
    }

    pub fn on_slur(&mut self) {
        self.flag_no_key_on = true;
        self.key_on_counter = 0;
    }

    pub fn on_slur_weak(&mut self) {
        self.key_on_counter = 0;
    }

    /// `_onPitchBend`: a slur, with a sweep to the next note.
    pub fn on_pitch_bend(&mut self, next_note: i32, term: i32) {
        let start_pitch = self.channel.pitch();
        let end_pitch = (((next_note + self.note_shift) << 6) | (start_pitch & 63)) + self.pitch_shift;
        self.on_slur();
        if start_pitch == end_pitch {
            return;
        }
        let term = term.max(1);
        self.sweep_step = (((end_pitch - start_pitch) as i64 * 65536) * i64::from(self.env_interval) / i64::from(term)) as i32;
        self.sweep_end = end_pitch << FIXED_BITS_T;
        self.sweep_pitch = start_pitch << FIXED_BITS_T;
        self.env_pitch_active = true;
        self.env_note = EnvPtr { table: self.set_env_note[1].clone(), pos: 0 };
        self.env_pitch = EnvPtr { table: self.set_env_pitch[1].clone(), pos: 0 };
        self.process_mode = ENVELOP;
    }

    /// `_changeNoteLength` (a masked slur).
    pub fn change_note_length(&mut self, length: i32) {
        self.key_on_counter = ((f64::from(length) * self.quant_ratio) as i32).wrapping_sub(self.quant_count).wrapping_sub(self.key_on_delay);
        if self.key_on_counter < 1 {
            self.key_on_counter = 1;
        }
    }

    fn mml_key_on(&mut self, shared: &Shared, module: &mut Module, note: i32) {
        self.note = note;
        if self.key_on_delay != 0 {
            self.key_off();
            self.key_on_counter = self.key_on_delay;
        } else {
            self.key_on(shared, module);
        }
    }

    // buffering

    /// `_buffer`: `length` samples.
    pub fn buffer(&mut self, shared: &Shared, module: &mut Module, mut length: i32) {
        if self.key_on_counter == 0 {
            self.process(shared, module, length);
        } else if self.key_on_counter > length {
            self.process(shared, module, length);
            self.key_on_counter -= length;
        } else {
            length -= self.key_on_counter;
            let c = self.key_on_counter;
            self.process(shared, module, c);
            self.toggle_key(shared, module);
            if length > 0 {
                self.process(shared, module, length);
            }
        }
    }

    fn process(&mut self, shared: &Shared, module: &mut Module, len: i32) {
        match self.process_mode {
            ENVELOP => self.residue = self.buffer_envelop(shared, module, len, self.residue),
            _ => self.channel.buffer(len as usize, module),
        }
    }

    fn buffer_envelop(&mut self, shared: &Shared, module: &mut Module, mut length: i32, mut step: i32) -> i32 {
        while length >= step {
            if step > 0 {
                self.channel.buffer(step as usize, module);
            }
            if self.env_exp.is_some() {
                self.cnt_exp -= 1;
                if self.cnt_exp == 0 {
                    let p = self.env_exp.as_mut().unwrap();
                    let x = (self.env_exp_offset + p.value()).clamp(0, 128);
                    self.channel.offset_volume(x, self.velocity);
                    p.advance();
                    self.cnt_exp = self.max_cnt_exp;
                }
            }
            if self.env_pitch_active {
                let pitch = self.env_pitch.value() + (self.env_note.value() << 6) + (self.sweep_pitch >> FIXED_BITS_T);
                self.channel.set_pitch(pitch);
                self.cnt_pitch -= 1;
                if self.cnt_pitch == 0 {
                    self.env_pitch.advance();
                    self.cnt_pitch = self.max_cnt_pitch;
                }
                self.cnt_note -= 1;
                if self.cnt_note == 0 {
                    self.env_note.advance();
                    self.cnt_note = self.max_cnt_note;
                }
                self.sweep_pitch = self.sweep_pitch.wrapping_add(self.sweep_step);
                if self.sweep_step > 0 {
                    if self.sweep_pitch > self.sweep_end {
                        self.sweep_pitch = self.sweep_end;
                        self.sweep_step = 0;
                    }
                } else if self.sweep_pitch < self.sweep_end {
                    self.sweep_pitch = self.sweep_end;
                    self.sweep_step = 0;
                }
            }
            if self.env_filter.is_some() {
                self.cnt_filter -= 1;
                if self.cnt_filter == 0 {
                    let p = self.env_filter.as_mut().unwrap();
                    self.channel.offset_filter(p.value());
                    p.advance();
                    self.cnt_filter = self.max_cnt_filter;
                }
            }
            if self.env_voice.is_some() {
                self.cnt_voice -= 1;
                if self.cnt_voice == 0 {
                    let v = self.env_voice.as_ref().unwrap().value();
                    self.select_tone(shared, module, v);
                    self.env_voice.as_mut().unwrap().advance();
                    self.cnt_voice = self.max_cnt_voice;
                }
            }
            if let Some((t, pos)) = self.env_ma.take() {
                self.channel.set_amplitude_modulation(t[pos]);
                self.env_ma = if pos + 1 < t.len() { Some((t, pos + 1)) } else { None };
            }
            if let Some((t, pos)) = self.env_mp.take() {
                self.channel.set_pitch_modulation(t[pos]);
                self.env_mp = if pos + 1 < t.len() { Some((t, pos + 1)) } else { None };
            }
            length -= step;
            step = self.env_interval;
        }
        if length > 0 {
            self.channel.buffer(length as usize, module);
        }
        self.env_interval - length
    }

    fn toggle_key(&mut self, shared: &Shared, module: &mut Module) {
        if self.channel.is_note_on {
            self.key_off();
        } else {
            self.key_on(shared, module);
        }
    }

    fn key_on(&mut self, shared: &Shared, module: &mut Module) {
        let old_pitch = self.channel.pitch();
        self.pitch_index = ((self.note + self.note_shift) << 6) + self.pitch_shift;
        self.channel.set_pitch(self.pitch_index + self.pitch_bend);
        if !self.flag_no_key_on {
            if self.process_mode == ENVELOP {
                self.channel.offset_volume(self.expression, self.velocity);
                let v = self.voice_index;
                self.select_tone(shared, module, v);
                self.channel.offset_filter(128);
            }
            if self.channel.is_note_on {
                self.channel.note_off();
            }
            self.update_process(1);
            self.channel.note_on();
        } else {
            if self.set_sweep_step[1] > 0 {
                self.channel.set_pitch(old_pitch);
                self.sweep_step = (((self.pitch_index - old_pitch) as i64 * 65536) / i64::from(self.set_sweep_step[1])) as i32;
                self.sweep_end = self.pitch_index << FIXED_BITS_T;
                self.sweep_pitch = old_pitch << FIXED_BITS_T;
            } else {
                self.sweep_pitch = self.channel.pitch() << FIXED_BITS_T;
            }
            self.envelop_off(1);
        }
        self.flag_no_key_on = false;
        self.key_on_counter = self.key_on_length;
    }

    fn key_off(&mut self) {
        self.channel.note_off();
        self.key_on_counter = 0;
        self.update_process(0);
    }

    fn update_process(&mut self, key_on: usize) {
        self.process_mode = self.set_process_mode[key_on];
        if self.process_mode == ENVELOP {
            self.env_exp = self.set_env_exp[key_on].as_ref().and_then(EnvPtr::new);
            self.env_voice = self.set_env_voice[key_on].as_ref().and_then(EnvPtr::new);
            self.env_note = EnvPtr { table: self.set_env_note[key_on].clone(), pos: 0 };
            self.env_pitch = EnvPtr { table: self.set_env_pitch[key_on].clone(), pos: 0 };
            self.env_filter = self.set_env_filter[key_on].as_ref().and_then(EnvPtr::new);
            self.max_cnt_exp = self.set_cnt_exp[key_on];
            self.max_cnt_voice = self.set_cnt_voice[key_on];
            self.max_cnt_note = self.set_cnt_note[key_on];
            self.max_cnt_pitch = self.set_cnt_pitch[key_on];
            self.max_cnt_filter = self.set_cnt_filter[key_on];
            self.cnt_exp = 1;
            self.cnt_voice = 1;
            self.cnt_note = 1;
            self.cnt_pitch = 1;
            self.cnt_filter = 1;
            self.env_ma = self.table_env_ma[key_on].clone().map(|t| (t, 0));
            self.env_mp = self.table_env_mp[key_on].clone().map(|t| (t, 0));
            self.sweep_step = if key_on != 0 { 0 } else { self.set_sweep_step[key_on] };
            self.sweep_end = if key_on != 0 { 0 } else { self.set_sweep_end[key_on] };
            self.sweep_pitch = self.channel.pitch() << FIXED_BITS_T;
            self.env_exp_offset = if self.set_exp_offset[key_on] { self.expression } else { 0 };
            self.env_pitch_active = self.pns_or[key_on];
            if !self.channel.is_filter_active() {
                let on = self.env_filter.is_some();
                self.channel.activate_filter(on);
            }
            self.residue = 0;
        }
    }

    /// Settings a sequence of the track reads back.
    pub fn eg_tl_table_ok(&self) -> bool {
        true
    }
}

/// `_makeModulationTable`: a delay at `depth`, then `term` steps to `end_depth`.
fn make_modulation_table(depth: i32, end_depth: i32, delay: i32, term: i32) -> Vec<i32> {
    let mut list = Vec::with_capacity((delay + term + 1).max(1) as usize);
    for _ in 0..delay.max(0) {
        list.push(depth);
    }
    if term > 0 {
        let mut d = depth << FIXED_BITS_T;
        let step = ((end_depth << FIXED_BITS_T) - d) / term;
        for _ in 0..term {
            list.push(d >> FIXED_BITS_T);
            d += step;
        }
    }
    list.push(end_depth);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_envelope_table_walks_loops_and_holds() {
        let held = EnvTable::new(vec![1, 2, 3], None);
        assert_eq!(held.to_vector(6, -100, 100), vec![1, 2, 3, 3, 3, 3]);
        let looped = EnvTable::new(vec![1, 2, 3, 4], Some(1));
        assert_eq!(looped.to_vector(7, -100, 100), vec![1, 2, 3, 4, 2, 3, 4]);
        assert_eq!(held.to_vector(2, 2, 2), vec![2, 2]);
    }

    #[test]
    fn module_types_have_tones_as_siopm_lists_them() {
        let t = Table::new(44100);
        let s = channel_settings(&t);
        assert_eq!(s.len(), MT_MAX);
        // PSG: square, noise, periodic noise.
        assert_eq!(s[MT_PSG].pg_type_list, vec![PG_SQUARE, PG_NOISE_PULSE, PG_PC_NZ_16BIT]);
        assert_eq!(s[MT_PSG].pt_type_list[1], PT_PSG_NOISE);
        // The sine type has all 512 pulse generators.
        assert_eq!(s[MT_ALL].pg_type_list.len(), 512);
        assert_eq!(s[MT_ALL].pg_type_list[5], PG_SQUARE);
        // APU: pulses at every other duty, then triangle and the noises.
        assert_eq!(s[MT_APU].pg_type_list[2], PG_PULSE + 4);
        assert_eq!(s[MT_APU].pg_type_list[8], PG_TRIANGLE_FC);
    }

    #[test]
    fn a_modulation_table_delays_then_ramps() {
        assert_eq!(make_modulation_table(0, 8, 2, 4), vec![0, 0, 0, 2, 4, 6, 8]);
    }
}
