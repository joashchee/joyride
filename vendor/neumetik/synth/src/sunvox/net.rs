// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! PSynth, SunVox's module network (psynth_net.cpp): the modules, their
//! controllers, links and data chunks, the events sent to them and the
//! order they render in, each into its own buffers, inputs summed from
//! the modules linked into it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::modules::{self, Kind};

/// PS_STYPE: a sample, 32-bit float.
pub(crate) type Smp = f32;

pub(crate) const MAX_CHANNELS: usize = 2;
pub(crate) const MAX_CTLS: usize = 127;

// Module flags.
pub(crate) const FLAG_EXISTS: u32 = 1 << 0;
pub(crate) const FLAG_OUTPUT: u32 = 1 << 1;
pub(crate) const FLAG_GENERATOR: u32 = 1 << 3;
pub(crate) const FLAG_EFFECT: u32 = 1 << 4;
pub(crate) const FLAG_INITIALIZED: u32 = 1 << 6;
pub(crate) const FLAG_MUTE: u32 = 1 << 7;
pub(crate) const FLAG_SOLO: u32 = 1 << 8;
pub(crate) const FLAG_GET_SPEED_CHANGES: u32 = 1 << 10;
pub(crate) const FLAG_DONT_FILL_INPUT: u32 = 1 << 13;
pub(crate) const FLAG_BYPASS: u32 = 1 << 14;
pub(crate) const FLAG_USE_MUTEX: u32 = 1 << 15;
pub(crate) const FLAG_IGNORE_MUTE: u32 = 1 << 16;
pub(crate) const FLAG_NO_SCOPE_BUF: u32 = 1 << 17;
pub(crate) const FLAG_OUTPUT_IS_EMPTY: u32 = 1 << 18;
pub(crate) const FLAG_GET_PLAY_COMMANDS: u32 = 1 << 20;
pub(crate) const FLAG_GET_RENDER_SETUP_COMMANDS: u32 = 1 << 21;
pub(crate) const FLAG_FEEDBACK: u32 = 1 << 22;
pub(crate) const FLAG_GET_STOP_COMMANDS: u32 = 1 << 23;
pub(crate) const FLAG_NO_RENDER: u32 = 1 << 24;

pub(crate) const FLAG2_GET_MUTED_COMMANDS: u32 = 1 << 0;
pub(crate) const FLAG2_NOTE_SENDER: u32 = 1 << 1;
pub(crate) const FLAG2_NOTE_RECEIVER: u32 = 1 << 2;
pub(crate) const FLAG2_JUST_LOADED: u32 = 1 << 29;
pub(crate) const FLAG2_LAST: u32 = 1 << 31;

// Realtime flags.
pub(crate) const RT_LOCKED: u8 = 1 << 0;
pub(crate) const RT_RENDERED: u8 = 1 << 1;
pub(crate) const RT_MUTE: u8 = 1 << 2;
pub(crate) const RT_SOLO: u8 = 1 << 3;
pub(crate) const RT_BYPASS: u8 = 1 << 4;
pub(crate) const RT_DONT_CLEAN_INPUT: u8 = 1 << 5;

// Net flags.
pub(crate) const NET_FLAG_MAIN: u32 = 1 << 0;
pub(crate) const NET_FLAG_NO_MODULE_CHANNELS: u32 = 1 << 4;

pub(crate) const EVENT_MUTE: u8 = 1 << 0;
pub(crate) const EVENT_SOLO: u8 = 1 << 1;
pub(crate) const EVENT_BYPASS: u8 = 1 << 2;

pub(crate) const NOTE0_PITCH: i32 = 30720;

/// The commands a module is sent (psynth_command), in their order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Cmd {
    #[default]
    Nop,
    GetDataSize,
    GetName,
    GetInfo,
    GetColor,
    GetInputsNum,
    GetOutputsNum,
    GetFlags,
    GetFlags2,
    Init,
    SetupFinished,
    BeforeSave,
    AfterSave,
    Clean,
    RenderSetup,
    RenderReplace,
    NoteOn,
    SetFreq,
    SetVelocity,
    SetSampleOffset,
    SetSampleOffset2,
    NoteOff,
    AllNotesOff,
    SetLocalController,
    SetGlobalController,
    ApplyControllers,
    SetMsb,
    ResetMsb,
    Muted,
    Finetune,
    SpeedChanged,
    Play,
    Stop,
    InputLinksChanged,
    OutputLinksChanged,
    Close,
    ReadCurve,
    WriteCurve,
    MidimsgSupport,
}

/// An event for a module (psynth_event). Its union is kept as its bytes,
/// so a field read from an event made as another kind reads what C reads.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Event {
    pub cmd: Cmd,
    /// 0xAAAABBBB: the pattern state (or a virtual pattern) and its track.
    pub id: u32,
    /// Frames into the buffer.
    pub offset: i32,
    pub u: [u8; 8],
}

impl Event {
    pub fn new(cmd: Cmd) -> Event {
        Event { cmd, ..Event::default() }
    }
    fn i16_at(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.u[at], self.u[at + 1]])
    }
    fn set_i16_at(&mut self, at: usize, v: i16) {
        self.u[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn i32_at(&self, at: usize) -> i32 {
        i32::from_le_bytes([self.u[at], self.u[at + 1], self.u[at + 2], self.u[at + 3]])
    }
    fn set_i32_at(&mut self, at: usize, v: i32) {
        self.u[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn root_note(&self) -> i16 {
        self.i16_at(0)
    }
    pub fn set_root_note(&mut self, v: i16) {
        self.set_i16_at(0, v)
    }
    pub fn velocity(&self) -> i16 {
        self.i16_at(2)
    }
    pub fn set_velocity(&mut self, v: i32) {
        self.set_i16_at(2, v as i16)
    }
    pub fn pitch(&self) -> i32 {
        self.i32_at(4)
    }
    pub fn set_pitch(&mut self, v: i32) {
        self.set_i32_at(4, v)
    }
    pub fn ctl_num(&self) -> u16 {
        self.i16_at(0) as u16
    }
    pub fn set_ctl_num(&mut self, v: u16) {
        self.set_i16_at(0, v as i16)
    }
    pub fn ctl_val(&self) -> u16 {
        self.i16_at(2) as u16
    }
    pub fn set_ctl_val(&mut self, v: u16) {
        self.set_i16_at(2, v as i16)
    }
    pub fn mute_flags(&self) -> u8 {
        self.u[0]
    }
    pub fn set_mute_flags(&mut self, v: u8) {
        self.u[0] = v
    }
    pub fn finetune(&self) -> i16 {
        self.i16_at(0)
    }
    pub fn relative_note(&self) -> i16 {
        self.i16_at(2)
    }
    pub fn set_finetune(&mut self, finetune: i32, relative: i32) {
        self.set_i16_at(0, finetune as i16);
        self.set_i16_at(2, relative as i16);
    }
    pub fn sample_offset(&self) -> u32 {
        self.i32_at(0) as u32
    }
    pub fn set_sample_offset(&mut self, v: u32) {
        self.set_i32_at(0, v as i32)
    }
    pub fn tick_size(&self) -> i32 {
        self.i32_at(0)
    }
    pub fn bpm(&self) -> u16 {
        self.i16_at(4) as u16
    }
    pub fn tpl(&self) -> u8 {
        self.u[6]
    }
    pub fn set_speed(&mut self, tick_size: i32, bpm: u16, tpl: u8) {
        self.set_i32_at(0, tick_size);
        self.set_i16_at(4, bpm as i16);
        self.u[6] = tpl;
    }
    pub fn midimsg(&self) -> (u8, u8) {
        (self.u[0], self.u[1])
    }
    pub fn set_midimsg(&mut self, msg: u8, ctl: u8) {
        self.u[0] = msg;
        self.u[1] = ctl;
    }
}

/// PSYNTH_EVT_ID_INC: a clone of an event's id made unique (to an extent) per module.
pub(crate) fn evt_id_inc(id: u32, mod_num: u32) -> u32 {
    let mut id = id.wrapping_add(mod_num << 5);
    id ^= (mod_num & 15) << 12;
    id ^ mod_num.wrapping_shl(16 + 10 + (mod_num & 3))
}

/// A controller (psynth_ctl); its value lives in its module's `vars`.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Ctl {
    pub min: i32,
    pub max: i32,
    pub def: i32,
    /// Where its value is in the module's vars; None for a slot made only
    /// to hold a value read from a file the module doesn't have.
    pub var: Option<usize>,
    pub show_offset: i32,
    pub normal_value: i32,
    pub flags: u32,
    /// 0: a level (events scale 0..32768 to its range); 1: a selector.
    pub typ: u8,
    pub group: u8,
}

/// A module's data chunk: samples, envelopes, waveforms...
#[derive(Clone, Default, Debug)]
pub(crate) struct Chunk {
    pub data: Vec<u8>,
    pub flags: u32,
    pub freq: i32,
}

pub(crate) const CHUNK_SMP_INT8: u32 = 1;
pub(crate) const CHUNK_SMP_INT16: u32 = 2;
pub(crate) const CHUNK_SMP_INT32: u32 = 3;
pub(crate) const CHUNK_SMP_FLOAT32: u32 = 4;
pub(crate) const CHUNK_SMP_TYPE_MASK: u32 = 7;
pub(crate) const CHUNK_SMP_STEREO: u32 = 1 << 3;

/// A module's place in the network (psynth_module), apart from its own data.
#[derive(Default)]
pub(crate) struct ModCommon {
    pub kind: Option<Kind>,
    pub flags: u32,
    pub flags2: u32,
    pub realtime_flags: u8,
    /// Its input buffers; empty for a NULL channel.
    pub chin: [Vec<Smp>; MAX_CHANNELS],
    pub chout: [Vec<Smp>; MAX_CHANNELS],
    /// Frames from which each channel is known to be silent.
    pub in_empty: [i32; MAX_CHANNELS],
    pub out_empty: [i32; MAX_CHANNELS],
    pub input_channels: i32,
    pub output_channels: i32,
    /// The stretch of the buffer a RENDER_REPLACE fills.
    pub frames: i32,
    pub offset: i32,
    /// Its events, by their place in the net's heap.
    pub events: Vec<usize>,
    pub finetune: i32,
    pub relative_note: i32,
    /// The modules linked into it (-1 for an empty slot), and out of it.
    pub input_links: Vec<i32>,
    pub output_links: Vec<i32>,
    pub ctls: Vec<Ctl>,
    /// Controllers registered past the count (psynth_change_ctls_num keeps
    /// them in the storage).
    pub ctls_store: Vec<Ctl>,
    pub vars: Vec<i32>,
    pub chunks: Vec<Option<Chunk>>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub scale: i32,
    pub color: [u8; 3],
    pub name: String,
    pub id: u32,
    pub midi_out_msg_map: [u8; 3],
}

impl ModCommon {
    /// A controller's value (0 for a slot without one).
    pub fn ctl(&self, i: usize) -> i32 {
        self.stored_ctl(i).and_then(|c| c.var).and_then(|v| self.vars.get(v).copied()).unwrap_or(0)
    }
    /// The controller in the storage, counted or not.
    pub fn stored_ctl(&self, i: usize) -> Option<&Ctl> {
        match self.ctls.get(i) {
            Some(c) => Some(c),
            None => self.ctls_store.get(i - self.ctls.len()),
        }
    }
    pub fn stored_ctl_mut(&mut self, i: usize) -> Option<&mut Ctl> {
        let n = self.ctls.len();
        if i < n {
            self.ctls.get_mut(i)
        } else {
            self.ctls_store.get_mut(i - n)
        }
    }
    pub fn chunk(&self, num: usize) -> Option<&Chunk> {
        self.chunks.get(num).and_then(|c| c.as_ref())
    }
    pub fn chunk_mut(&mut self, num: usize) -> Option<&mut Chunk> {
        self.chunks.get_mut(num).and_then(|c| c.as_mut())
    }
}

/// What every engine in the process shares: the module id counter and
/// the global pseudo-random generator (which modules seed their noise
/// from), so a song played here seeds as it does in the library.
#[derive(Debug)]
pub(crate) struct Globals {
    pub module_ids: AtomicU32,
    pub rand: AtomicU32,
}

impl Globals {
    /// As the library is when a song is loaded into a fresh slot: one
    /// engine made at sv_open_slot and closed by the load, then made again.
    pub fn new() -> Arc<Globals> {
        let g = Globals { module_ids: AtomicU32::new(0), rand: AtomicU32::new(1) };
        Arc::new(g)
    }
    pub fn next_id(&self) -> u32 {
        self.module_ids.fetch_add(1, Ordering::Relaxed)
    }
    /// pseudo_random(): 0..32767 from the shared state.
    pub fn pseudo_random(&self) -> u32 {
        let mut s = self.rand.load(Ordering::Relaxed);
        let r = pseudo_random(&mut s);
        self.rand.store(s, Ordering::Relaxed);
        r
    }
}

/// pseudo_random( seed ): 0..32767.
pub(crate) fn pseudo_random(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed / 65536) % 32768
}

/// psynth_rand: 0..32767.
pub(crate) fn psynth_rand(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed >> 16) & 32767
}

/// psynth_rand2: -32768..32767.
pub(crate) fn psynth_rand2(seed: &mut u32) -> i32 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed as i32) >> 16
}

/// A module's own state and its handler.
pub(crate) trait Module: Send {
    /// The module's handler for the commands sent to an instance (INIT on).
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize;
    /// A Feedback module's ring buffer (one `size` a channel) and the write
    /// position at the block's start, for the Feedback it sends to.
    fn feedback_buf(&self) -> Option<(&[Smp], u32, u32)> {
        None
    }
}

/// The engine a net belongs to, as far as modules ask after it.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Host {
    pub bpm: i32,
    pub speed: i32,
}

pub(crate) const DEFAULT_MODULE_EVENTS_NUM: usize = 128;

/// The network (psynth_net).
pub(crate) struct Net {
    pub flags: u32,
    pub mods: Vec<ModCommon>,
    pub handlers: Vec<Option<Box<dyn Module>>>,
    pub events_heap: Vec<Event>,
    pub events_num: usize,
    pub sampling_freq: i32,
    pub max_buf_size: i32,
    pub global_volume: i32,
    pub all_modules_muted: bool,
    pub buf_size: i32,
    pub base_host_version: u32,
    pub render_counter: u32,
    pub change_counter: i32,
    pub change_counter2: i32,
    pub prev_change_counter2: i32,
    pub midi_out_7bit: bool,
    /// The thread's buffers: PSYNTH_GET_RENDERBUF's, and the resamplers' input
    /// (two for mode 0 and 2, two for mode 1).
    pub temp_buf: [Vec<Smp>; MAX_CHANNELS],
    pub resamp_buf: [Vec<Smp>; MAX_CHANNELS * 2],
    pub host: Host,
    pub globals: Arc<Globals>,
}

impl Net {
    /// psynth_init: an empty network with its Output module.
    pub fn new(flags: u32, freq: i32, bpm: i32, tpl: i32, base_host_version: u32, globals: Arc<Globals>) -> Net {
        let mut net = Net {
            flags,
            mods: Vec::new(),
            handlers: Vec::new(),
            events_heap: vec![Event::default(); 256],
            events_num: 0,
            sampling_freq: freq,
            max_buf_size: (freq as f32 * 0.02f32) as i32,
            global_volume: 80,
            all_modules_muted: false,
            buf_size: 0,
            base_host_version,
            render_counter: 0,
            change_counter: 0,
            change_counter2: 0,
            prev_change_counter2: -1,
            midi_out_7bit: false,
            temp_buf: Default::default(),
            resamp_buf: Default::default(),
            host: Host { bpm, speed: tpl },
            globals,
        };
        net.grow();
        net.add_module(-1, None, Some("Output"), FLAG_OUTPUT, 512, 512, 0, bpm, tpl);
        net
    }

    fn grow(&mut self) {
        for _ in 0..4 {
            self.mods.push(ModCommon::default());
            self.handlers.push(None);
        }
    }

    pub fn mods_num(&self) -> usize {
        self.mods.len()
    }

    /// psynth_get_module: the module if it exists.
    pub fn get(&self, n: i32) -> Option<&ModCommon> {
        self.mods.get(usize::try_from(n).ok()?).filter(|m| m.flags & FLAG_EXISTS != 0)
    }

    pub fn exists(&self, n: i32) -> bool {
        self.get(n).is_some()
    }

    /// Calls a module's handler, the module taken out of the net while it runs.
    pub fn call(&mut self, n: usize, e: &mut Event) -> isize {
        let Some(mut h) = self.handlers.get_mut(n).and_then(|h| h.take()) else { return 0 };
        let r = h.handle(n, e, self);
        if let Some(slot) = self.handlers.get_mut(n) {
            *slot = Some(h);
        }
        r
    }

    /// psynth_add_module.
    #[allow(clippy::too_many_arguments)]
    pub fn add_module(&mut self, n: i32, kind: Option<Kind>, name: Option<&str>, flags: u32, x: i32, y: i32, z: i32, bpm: i32, tpl: i32) -> i32 {
        self.change_counter += 1;
        self.change_counter2 += 1;
        let n = if n < 0 {
            match self.mods.iter().position(|m| m.flags & FLAG_EXISTS == 0) {
                Some(i) => i,
                None => {
                    let i = self.mods.len();
                    self.grow();
                    i
                }
            }
        } else {
            n as usize
        };
        let info = modules::info(kind);
        let mut m = ModCommon { kind, flags: FLAG_EXISTS | flags | info.flags, flags2: info.flags2, x, y, z, scale: 256, ..Default::default() };
        m.events = Vec::with_capacity(DEFAULT_MODULE_EVENTS_NUM);
        m.id = self.globals.next_id();
        match name {
            Some(name) => m.name = name.chars().take(32).collect(),
            None => {
                if flags & FLAG_OUTPUT == 0 {
                    m.name = self.unique_name(n, if info.name.is_empty() { "MODULE" } else { info.name });
                }
            }
        }
        self.mods[n] = m;
        self.handlers[n] = Some(modules::make(kind));
        let mut e = Event::new(Cmd::Init);
        self.call(n, &mut e);
        let max_buf = self.max_buf_size as usize;
        let no_channels = self.flags & NET_FLAG_NO_MODULE_CHANNELS != 0;
        let m = &mut self.mods[n];
        m.flags |= FLAG_INITIALIZED;
        m.input_channels = info.inputs;
        m.output_channels = info.outputs;
        let max_channels = info.outputs.max(info.inputs) as usize;
        if !no_channels {
            for ch in 0..max_channels.min(MAX_CHANNELS) {
                m.chin[ch] = vec![0.0; max_buf];
                m.in_empty[ch] = max_buf as i32;
                if m.flags & FLAG_OUTPUT == 0 {
                    m.chout[ch] = vec![0.0; max_buf];
                    m.out_empty[ch] = max_buf as i32;
                }
            }
        }
        if m.flags & FLAG_GET_SPEED_CHANGES != 0 {
            let mut e = Event::new(Cmd::SpeedChanged);
            e.set_speed(tick_size(self.sampling_freq, bpm) as i32, bpm as u16, tpl as u8);
            self.call(n, &mut e);
        }
        n as i32
    }

    /// The name psynth_add_module gives a module named after its kind: the
    /// kind's name, numbered after the highest number another such name has.
    fn unique_name(&self, n: usize, mod_name: &str) -> String {
        let mut max_num = 0;
        for (a, m) in self.mods.iter().enumerate() {
            if a == n || !m.name.starts_with(mod_name) {
                continue;
            }
            let rest = m.name[mod_name.len()..].trim_start_matches('_');
            let num = if rest.is_empty() {
                1
            } else if rest.bytes().all(|c| c.is_ascii_digit()) {
                string_to_int(rest)
            } else {
                0
            };
            max_num = max_num.max(num);
        }
        let mut name: String = mod_name.chars().take(32).collect();
        if max_num > 0 {
            if name.ends_with(|c: char| c.is_ascii_digit()) {
                name.push('_');
            }
            name.push_str(&(max_num + 1).to_string());
            name.truncate(32);
        }
        name
    }

    /// psynth_remove_module.
    pub fn remove_module(&mut self, n: usize) {
        if n >= self.mods.len() || self.mods[n].flags & FLAG_EXISTS == 0 {
            return;
        }
        self.change_counter += 1;
        self.change_counter2 += 1;
        let mut e = Event::new(Cmd::Close);
        self.call(n, &mut e);
        self.handlers[n] = None;
        let inputs = std::mem::take(&mut self.mods[n].input_links);
        if !inputs.is_empty() {
            for &i in &inputs {
                if let Some(im) = usize::try_from(i).ok().filter(|&i| i < self.mods.len()) {
                    let mut changed = false;
                    for l in self.mods[im].output_links.iter_mut() {
                        if *l == n as i32 {
                            *l = -1;
                            changed = true;
                        }
                    }
                    if changed {
                        self.do_command(im, Cmd::OutputLinksChanged);
                    }
                }
            }
            self.do_command(n, Cmd::InputLinksChanged);
        }
        let outputs = std::mem::take(&mut self.mods[n].output_links);
        if !outputs.is_empty() {
            for &o in &outputs {
                if let Some(om) = usize::try_from(o).ok().filter(|&o| o < self.mods.len()) {
                    let mut changed = false;
                    for l in self.mods[om].input_links.iter_mut() {
                        if *l == n as i32 {
                            *l = -1;
                            changed = true;
                        }
                    }
                    if changed {
                        self.do_command(om, Cmd::InputLinksChanged);
                    }
                }
            }
            self.do_command(n, Cmd::OutputLinksChanged);
        }
        let m = &mut self.mods[n];
        m.events = Vec::new();
        m.chunks = Vec::new();
        m.chin = Default::default();
        m.chout = Default::default();
        m.ctls = Vec::new();
        m.vars = Vec::new();
        m.flags = 0;
    }

    /// psynth_remove_empty_modules_at_the_end.
    pub fn remove_empty_modules_at_the_end(&mut self) {
        while self.mods.len() > 1 && self.mods.last().is_some_and(|m| m.flags & FLAG_EXISTS == 0) {
            self.mods.pop();
            self.handlers.pop();
        }
    }

    /// psynth_add_link: `link` into module `n`'s input (or output) links,
    /// at `slot` or the first free one.
    pub fn add_link(&mut self, input: bool, n: i32, link: i32, slot: i32) {
        let len = self.mods.len() as i32;
        if n < 0 || n >= len || link < 0 || link >= len {
            return;
        }
        self.change_counter += 1;
        self.change_counter2 += 1;
        let m = &mut self.mods[n as usize];
        if m.flags & FLAG_EXISTS == 0 {
            return;
        }
        let links = if input { &mut m.input_links } else { &mut m.output_links };
        if links.contains(&link) {
            return;
        }
        if links.is_empty() {
            let num = if slot < 0 { 2 } else { slot as usize + 1 };
            *links = vec![-1; num];
        }
        let i = if slot < 0 { links.iter().position(|&l| l < 0).unwrap_or(links.len()) } else { slot as usize };
        if i >= links.len() {
            links.resize(i + 2, -1);
        }
        links[i] = link;
        self.do_command(n as usize, if input { Cmd::InputLinksChanged } else { Cmd::OutputLinksChanged });
    }

    /// psynth_make_link: `in` into `out` (out.link = in).
    pub fn make_link(&mut self, out: i32, inp: i32) {
        if self.remove_link(out, inp) {
            return;
        }
        self.add_link(true, out, inp, -1);
        self.add_link(false, inp, out, -1);
    }

    /// psynth_remove_link: the link between two modules, either way.
    pub fn remove_link(&mut self, out: i32, inp: i32) -> bool {
        self.change_counter += 1;
        self.change_counter2 += 1;
        let mut found = false;
        let mut cmds = Vec::new();
        for (a, b) in [(inp, out), (out, inp)] {
            if !self.exists(a) {
                continue;
            }
            let m = &mut self.mods[a as usize];
            let mut changed_in = false;
            let mut changed_out = false;
            for l in m.input_links.iter_mut() {
                if *l == b {
                    *l = -1;
                    changed_in = true;
                }
            }
            for l in m.output_links.iter_mut() {
                if *l == b {
                    *l = -1;
                    changed_out = true;
                }
            }
            found |= changed_in | changed_out;
            if changed_in {
                cmds.push((a as usize, Cmd::InputLinksChanged));
            }
            if changed_out {
                cmds.push((a as usize, Cmd::OutputLinksChanged));
            }
        }
        // The order psynth_remove_link sends them in: in's, then out's.
        for (n, c) in cmds {
            self.do_command(n, c);
        }
        found
    }

    /// psynth_reset_events.
    pub fn reset_events(&mut self) {
        if self.events_num == 0 {
            return;
        }
        self.events_num = 0;
        for m in self.mods.iter_mut() {
            if m.flags & FLAG_EXISTS != 0 {
                m.events.clear();
            }
        }
    }

    /// psynth_add_event.
    pub fn add_event(&mut self, n: usize, e: &Event) {
        if n >= self.mods.len() || self.mods[n].flags & FLAG_EXISTS == 0 {
            return;
        }
        let k = self.events_num;
        self.events_num += 1;
        if k >= self.events_heap.len() {
            self.events_heap.resize(k * 2, Event::default());
        }
        self.events_heap[k] = *e;
        self.mods[n].events.push(k);
    }

    /// psynth_multisend: the event to every module this one links out to.
    pub fn multisend(&mut self, n: usize, e: &Event) {
        let links = self.mods[n].output_links.clone();
        for l in links {
            if self.exists(l) {
                self.add_event(l as usize, e);
            }
        }
    }

    /// psynth_multisend_pitch: the same, the pitch made each module's own.
    pub fn multisend_pitch(&mut self, n: usize, e: &mut Event, pitch: i32) {
        let links = self.mods[n].output_links.clone();
        for l in links {
            if let Some(d) = self.get(l) {
                e.set_pitch(pitch - d.finetune - d.relative_note * 256);
                let e2 = *e;
                self.add_event(l as usize, &e2);
            }
        }
    }

    /// psynth_handle_event: an event handled at once, outside rendering.
    pub fn handle_event(&mut self, n: usize, e: &mut Event) -> isize {
        if !self.exists(n as i32) {
            return 0;
        }
        if e.cmd == Cmd::RenderReplace {
            let bs = self.buf_size;
            self.mods[n].frames = bs;
            self.mods[n].offset = 0;
        }
        let res = self.call(n, e);
        let m = &mut self.mods[n];
        match e.cmd {
            Cmd::RenderReplace => {
                let bs = self.buf_size;
                render_set_output_content(0, bs, res as i32, m);
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                self.change_counter += 1;
                if res == 0 {
                    set_ctl(m, e);
                }
            }
            Cmd::SetMsb => {
                self.change_counter += 1;
                self.change_counter2 += 1;
                if res == 0 {
                    set_msb(m, e);
                }
            }
            Cmd::ResetMsb => {
                self.change_counter += 1;
                self.change_counter2 += 1;
                if res == 0 {
                    let muted = self.all_modules_muted;
                    reset_msb(&mut self.mods[n], e, muted);
                }
            }
            Cmd::Finetune => {
                self.change_counter += 1;
                if res == 0 {
                    finetune(m, e);
                }
            }
            Cmd::MidimsgSupport => {
                self.change_counter += 1;
                if res == 0 {
                    midimsg_support(m, e);
                }
            }
            Cmd::WriteCurve => self.change_counter += 1,
            _ => {}
        }
        res
    }

    /// psynth_do_command.
    pub fn do_command(&mut self, n: usize, cmd: Cmd) -> isize {
        let mut e = Event::new(cmd);
        self.handle_event(n, &mut e)
    }

    /// psynth_handle_ctl_event.
    pub fn handle_ctl_event(&mut self, n: usize, ctl_num: i32, ctl_val: i32) -> isize {
        let mut e = Event::new(Cmd::SetGlobalController);
        e.set_ctl_num(ctl_num as u16);
        e.set_ctl_val(ctl_val as u16);
        self.handle_event(n, &mut e)
    }

    // Controllers.

    /// psynth_register_ctl: the next controller, its value kept in var `var`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_ctl(&mut self, n: usize, min: i32, max: i32, def: i32, typ: u8, var: usize, normal: i32, group: u8) -> i32 {
        let m = &mut self.mods[n];
        let normal_value = if normal == -1 { max } else { normal };
        if m.vars.len() <= var {
            m.vars.resize(var + 1, 0);
        }
        m.vars[var] = def;
        let c = Ctl { min, max, def, var: Some(var), show_offset: 0, normal_value, flags: 0, typ, group };
        let rv = m.ctls.len() as i32;
        // Past the limit C writes it beyond the count, where nothing reads it.
        if m.ctls.len() < MAX_CTLS {
            m.ctls.push(c);
        }
        rv
    }

    /// psynth_change_ctls_num.
    pub fn change_ctls_num(&mut self, n: usize, num: usize) {
        let m = &mut self.mods[n];
        let num = num.min(MAX_CTLS);
        if num < m.ctls.len() {
            let mut tail = m.ctls.split_off(num);
            tail.append(&mut m.ctls_store);
            m.ctls_store = tail;
        } else {
            let take = (num - m.ctls.len()).min(m.ctls_store.len());
            let back: Vec<Ctl> = m.ctls_store.drain(..take).collect();
            m.ctls.extend(back);
            m.ctls.resize(num, Ctl::default());
        }
    }

    /// psynth_change_ctl: a controller's settings changed (negative = kept).
    #[allow(clippy::too_many_arguments)]
    pub fn change_ctl(&mut self, n: usize, ctl: usize, min: i32, max: i32, def: i32, typ: i32, var: Option<usize>, normal: i32, group: i32) {
        let m = &mut self.mods[n];
        let Some(c) = m.ctls.get_mut(ctl) else { return };
        if min >= 0 {
            c.min = min;
        }
        if max >= 0 {
            c.max = max;
        }
        if def >= 0 {
            c.def = def;
        }
        if var.is_some() {
            c.var = var;
        }
        if normal >= 0 {
            c.normal_value = normal;
        }
        if typ >= 0 {
            c.typ = typ as u8;
        }
        if group >= 0 {
            c.group = group as u8;
        }
        if let Some(v) = var {
            let def = c.def;
            if m.vars.len() <= v {
                m.vars.resize(v + 1, 0);
            }
            m.vars[v] = def;
        }
    }

    /// psynth_change_ctl_limits.
    pub fn change_ctl_limits(&mut self, n: usize, ctl: usize, min: i32, max: i32, normal: i32) {
        let m = &mut self.mods[n];
        let Some(c) = m.ctls.get_mut(ctl) else { return };
        if min >= 0 {
            c.min = min;
        }
        if max >= 0 {
            c.max = max;
        }
        if normal >= 0 {
            c.normal_value = normal;
        }
        if let Some(v) = c.var {
            let x = &mut m.vars[v];
            if *x < min {
                *x = min;
            }
            if *x > max {
                *x = max;
            }
        }
    }

    pub fn set_ctl_show_offset(&mut self, n: usize, ctl: usize, offset: i32) {
        if let Some(c) = self.mods[n].ctls.get_mut(ctl) {
            c.show_offset = offset;
        }
    }

    pub fn set_ctl_flags(&mut self, n: usize, ctl: usize, flags: u32) {
        if let Some(c) = self.mods[n].ctls.get_mut(ctl) {
            c.flags = flags;
        }
    }

    /// psynth_get_scaled_ctl_value: a controller's decimal value as a
    /// pattern's 0..32768.
    pub fn get_scaled_ctl_value(&self, n: usize, ctl_num: usize, val: i32, positive_val_without_offset: bool) -> i32 {
        let Some(m) = self.get(n as i32) else { return -1 };
        let mut v: u32;
        let mut val = val;
        match m.ctls.get(ctl_num) {
            Some(c) => {
                if !positive_val_without_offset {
                    val -= c.show_offset;
                    val -= c.min;
                }
                if c.typ == 0 {
                    let range = c.max - c.min;
                    v = if range != 0 { ((val << 15) as u32) / range as u32 } else { (val << 15) as u32 };
                    let v2 = v.wrapping_mul(range as u32) / 32768;
                    if (v2 as i32) < val {
                        v += 1;
                    }
                } else {
                    v = (val + c.min) as u32;
                }
            }
            None => {
                let midi_ctl = ctl_num as i32 + 1 - 0x80;
                v = if midi_ctl >= 0 {
                    if midi_ctl < 32 {
                        (val << 1) as u32
                    } else {
                        (val << 8) as u32
                    }
                } else {
                    val as u32
                };
            }
        }
        v.min(0x8000) as i32
    }

    // Chunks.

    /// psynth_new_chunk.
    pub fn new_chunk(&mut self, n: usize, num: usize, c: Chunk) {
        if n >= self.mods.len() {
            return;
        }
        let m = &mut self.mods[n];
        if m.chunks.is_empty() {
            m.chunks = vec![None; 4.max(num + 1)];
        }
        if m.chunks.len() <= num {
            m.chunks.resize(num + 1, None);
        }
        m.chunks[num] = Some(c);
    }

    /// psynth_new_chunk with a zeroed block of `size` bytes.
    pub fn new_chunk_sized(&mut self, n: usize, num: usize, size: usize, flags: u32, freq: i32) {
        self.new_chunk(n, num, Chunk { data: vec![0; size], flags, freq });
    }

    /// psynth_get_chunk_data_autocreate: the chunk, made (zeroed) if missing
    /// and grown to `size`; whether it was made.
    pub fn chunk_autocreate(&mut self, n: usize, num: usize, size: usize, cut_unused: bool) -> bool {
        let exists = self.mods[n].chunk(num).is_some();
        if !exists {
            self.new_chunk_sized(n, num, size, 0, 0);
            return true;
        }
        let c = self.mods[n].chunk_mut(num).unwrap();
        let old = c.data.len();
        if old < size || (cut_unused && old > size) {
            c.data.resize(size, 0);
        }
        false
    }

    pub fn remove_chunk(&mut self, n: usize, num: usize) {
        if let Some(c) = self.mods.get_mut(n).and_then(|m| m.chunks.get_mut(num)) {
            *c = None;
        }
    }

    // Channels.

    /// psynth_set_number_of_outputs.
    pub fn set_number_of_outputs(&mut self, num: i32, n: usize) {
        let max = self.max_buf_size;
        let no_channels = self.flags & NET_FLAG_NO_MODULE_CHANNELS != 0;
        let m = &mut self.mods[n];
        if m.output_channels != num {
            m.output_channels = num;
            if !no_channels {
                for c in num.max(0) as usize..MAX_CHANNELS {
                    if m.chout[c].is_empty() {
                        continue;
                    }
                    let from = m.out_empty[c].max(0) as usize;
                    for x in m.chout[c][from.min(max as usize)..max as usize].iter_mut() {
                        *x = 0.0;
                    }
                    m.out_empty[c] = max;
                }
            }
        }
    }

    /// psynth_set_number_of_inputs.
    pub fn set_number_of_inputs(&mut self, num: i32, n: usize) {
        let max = self.max_buf_size;
        let no_channels = self.flags & NET_FLAG_NO_MODULE_CHANNELS != 0;
        let m = &mut self.mods[n];
        if m.input_channels != num {
            m.input_channels = num;
            if !no_channels {
                for c in num.max(0) as usize..MAX_CHANNELS {
                    if m.chin[c].is_empty() {
                        continue;
                    }
                    let from = m.in_empty[c].max(0) as usize;
                    for x in m.chin[c][from.min(max as usize)..max as usize].iter_mut() {
                        *x = 0.0;
                    }
                    m.in_empty[c] = max;
                }
            }
        }
    }

    /// psynth_get_temp_buf: the thread's temporary buffer (made on first use).
    pub fn temp_buf(&mut self, i: usize) -> &mut Vec<Smp> {
        let max = self.max_buf_size as usize;
        let b = &mut self.temp_buf[i];
        if b.is_empty() {
            *b = vec![0.0; max];
        }
        b
    }

    // Rendering.

    pub fn render_setup(&mut self, buf_size: i32) {
        self.buf_size = buf_size;
        self.render_counter = self.render_counter.wrapping_add(1);
    }

    /// psynth_render_all: every module's buffer for this block, Output's inputs first.
    pub fn render_all(&mut self) {
        self.all_modules_muted = false;
        for i in 0..self.mods.len() {
            let m = &mut self.mods[i];
            if m.flags & FLAG_EXISTS != 0 {
                m.realtime_flags &= !RT_RENDERED;
                if m.flags & FLAG_SOLO != 0 {
                    self.all_modules_muted = true;
                }
                if m.flags & FLAG_GET_RENDER_SETUP_COMMANDS != 0 {
                    self.do_command(i, Cmd::RenderSetup);
                }
            }
        }
        if self.prev_change_counter2 != self.change_counter2 {
            self.prev_change_counter2 = self.change_counter2;
            for m in self.mods.iter_mut() {
                if m.flags & FLAG_EXISTS != 0 {
                    m.realtime_flags &= !(RT_MUTE | RT_SOLO | RT_BYPASS);
                }
            }
        }
        self.render_module0();
        for i in 1..self.mods.len() {
            self.render(i);
        }
    }

    fn render_module0(&mut self) {
        let m0 = &self.mods[0];
        if m0.realtime_flags & RT_RENDERED != 0 || m0.realtime_flags & RT_LOCKED != 0 {
            return;
        }
        let bs = self.buf_size;
        self.mods[0].realtime_flags |= RT_LOCKED;
        let mut main_rendered = [false; MAX_CHANNELS];
        let links = self.mods[0].input_links.clone();
        for &l in &links {
            let Some(inp) = usize::try_from(l).ok().filter(|&l| l < self.mods.len()) else { continue };
            if self.mods[inp].realtime_flags & RT_RENDERED == 0 {
                self.render(inp);
            }
            if self.mods[inp].realtime_flags & RT_RENDERED == 0 {
                continue;
            }
            let (m0, im) = two(&mut self.mods, 0, inp);
            let mut prev: Option<usize> = None;
            for ch in 0..m0.input_channels.max(0) as usize {
                let src = if ch as i32 >= im.output_channels { prev } else { Some(ch) };
                prev = src;
                let Some(sc) = src.filter(|&c| !im.chout[c].is_empty()) else { continue };
                if m0.chin[ch].is_empty() {
                    continue;
                }
                let oe = im.out_empty[sc];
                if main_rendered[ch] {
                    for i in oe.max(0)..bs {
                        m0.chin[ch][i as usize] += im.chout[sc][i as usize];
                    }
                    if oe < m0.in_empty[ch] {
                        m0.in_empty[ch] = oe;
                    }
                } else {
                    if oe < bs {
                        clean(&mut m0.chin[ch], m0.in_empty[ch], oe);
                        copy(&mut m0.chin[ch], &im.chout[sc], oe, bs);
                        m0.in_empty[ch] = oe;
                    } else {
                        clean(&mut m0.chin[ch], m0.in_empty[ch], bs);
                        m0.in_empty[ch] = bs;
                    }
                    if m0.in_empty[ch] < bs {
                        main_rendered[ch] = true;
                    }
                }
            }
        }
        let gv = self.global_volume;
        let m0 = &mut self.mods[0];
        for ch in 0..m0.input_channels.max(0) as usize {
            if m0.chin[ch].is_empty() {
                continue;
            }
            if main_rendered[ch] {
                if gv != 256 {
                    for i in m0.in_empty[ch].max(0)..bs {
                        let v = m0.chin[ch][i as usize] * gv as f32 / 256.0;
                        m0.chin[ch][i as usize] = v;
                    }
                }
            } else if bs > m0.in_empty[ch] {
                clean(&mut m0.chin[ch], m0.in_empty[ch], bs);
                m0.in_empty[ch] = bs;
            }
        }
        m0.realtime_flags &= !RT_LOCKED;
        m0.realtime_flags |= RT_RENDERED;
    }

    /// psynth_render: one module (its inputs first); 0 when rendered.
    fn render(&mut self, n: usize) -> i32 {
        let m = &self.mods[n];
        if m.flags & FLAG_EXISTS == 0 {
            return -1;
        }
        if m.realtime_flags & RT_RENDERED != 0 {
            return 0;
        }
        if m.realtime_flags & RT_LOCKED != 0 {
            return -1;
        }
        let bs = self.buf_size;
        {
            let m = &mut self.mods[n];
            m.realtime_flags |= RT_LOCKED;
            m.realtime_flags &= !(RT_MUTE | RT_SOLO | RT_BYPASS);
            if m.flags & FLAG_IGNORE_MUTE == 0 {
                if m.flags & FLAG_MUTE != 0 {
                    m.realtime_flags |= RT_MUTE;
                }
                if m.flags & FLAG_SOLO != 0 {
                    m.realtime_flags |= RT_SOLO;
                }
                if m.flags & FLAG_BYPASS != 0 {
                    m.realtime_flags |= RT_BYPASS;
                }
            }
        }
        let mut dont_fill_input = self.mods[n].flags & FLAG_DONT_FILL_INPUT != 0;
        if self.mods[n].realtime_flags & RT_BYPASS != 0 {
            dont_fill_input = false;
        }
        let mut input_rendered = false;
        let links = self.mods[n].input_links.clone();
        for &l in &links {
            let Some(inp) = usize::try_from(l).ok().filter(|&l| l < self.mods.len()) else { continue };
            if self.mods[n].flags & FLAG_FEEDBACK != 0 && self.mods[inp].flags & FLAG_FEEDBACK != 0 {
                if self.mods[inp].realtime_flags & RT_SOLO != 0 {
                    self.mods[n].realtime_flags |= RT_SOLO;
                }
                continue;
            }
            if self.mods[inp].realtime_flags & RT_RENDERED == 0 && self.render(inp) != 0 {
                continue;
            }
            if self.mods[inp].realtime_flags & RT_SOLO != 0 {
                self.mods[n].realtime_flags |= RT_SOLO;
            }
            if self.mods[inp].realtime_flags & RT_RENDERED == 0 {
                continue;
            }
            let (m, im) = two(&mut self.mods, n, inp);
            let mut prev: Option<usize> = None;
            if !input_rendered {
                for ch in 0..m.input_channels.max(0) as usize {
                    let src = if ch as i32 >= im.output_channels { prev } else { Some(ch) };
                    prev = src;
                    let Some(sc) = src.filter(|&c| !im.chout[c].is_empty()) else { continue };
                    if m.chin[ch].is_empty() {
                        continue;
                    }
                    let oe = im.out_empty[sc];
                    if oe < bs {
                        if !dont_fill_input {
                            clean(&mut m.chin[ch], m.in_empty[ch], oe);
                            copy(&mut m.chin[ch], &im.chout[sc], oe, bs);
                        }
                        m.in_empty[ch] = oe;
                    } else {
                        if !dont_fill_input {
                            clean(&mut m.chin[ch], m.in_empty[ch], bs);
                        }
                        m.in_empty[ch] = bs;
                    }
                    if m.in_empty[ch] < bs {
                        input_rendered = true;
                    }
                }
            } else {
                for ch in 0..m.input_channels.max(0) as usize {
                    let mut src = if ch as i32 >= im.output_channels || im.chout[ch].is_empty() { None } else { Some(ch) };
                    if src.is_none() {
                        src = prev;
                    }
                    prev = src;
                    let Some(sc) = src.filter(|&c| !im.chout[c].is_empty()) else { continue };
                    if m.chin[ch].is_empty() {
                        continue;
                    }
                    let oe = im.out_empty[sc];
                    if !dont_fill_input {
                        for i in oe.max(0)..bs {
                            m.chin[ch][i as usize] += im.chout[sc][i as usize];
                        }
                    }
                    if oe < m.in_empty[ch] {
                        m.in_empty[ch] = oe;
                    }
                }
            }
        }
        {
            let m = &mut self.mods[n];
            if !input_rendered && m.flags & FLAG_EFFECT != 0 && m.realtime_flags & RT_DONT_CLEAN_INPUT == 0 {
                for c in 0..m.input_channels.max(0) as usize {
                    if !m.chin[c].is_empty() && bs > m.in_empty[c] {
                        clean(&mut m.chin[c], m.in_empty[c], bs);
                        m.in_empty[c] = bs;
                    }
                }
            }
        }
        if self.mods[n].flags & FLAG_INITIALIZED != 0 {
            if self.mods[n].flags & FLAG_IGNORE_MUTE == 0 {
                let muted_all = self.all_modules_muted;
                let m = &mut self.mods[n];
                if muted_all && !(m.realtime_flags & RT_SOLO != 0 && m.realtime_flags & RT_MUTE == 0) {
                    m.realtime_flags |= RT_MUTE;
                }
                if m.realtime_flags & RT_MUTE != 0 && m.flags2 & FLAG2_GET_MUTED_COMMANDS != 0 {
                    let mut e = Event::new(Cmd::Muted);
                    self.call(n, &mut e);
                }
            }
            let mut bypass = self.mods[n].realtime_flags & RT_BYPASS != 0;
            let no_render = self.mods[n].flags & FLAG_NO_RENDER != 0;
            if self.mods[n].events.is_empty() {
                if !no_render {
                    self.mods[n].frames = bs;
                    self.mods[n].offset = 0;
                    let res = self.render_replace(n, bypass);
                    let m = &mut self.mods[n];
                    render_set_output_content(0, bs, res as i32, m);
                }
            } else {
                // The events in time order (an insertion sort, stable).
                let heap = &self.events_heap;
                let evs = &mut self.mods[n].events;
                for i in 1..evs.len() {
                    let key_evt = evs[i];
                    let key = heap[key_evt].offset;
                    let mut j = i;
                    while j > 0 && heap[evs[j - 1]].offset > key {
                        evs[j] = evs[j - 1];
                        j -= 1;
                    }
                    if j != i {
                        evs[j] = key_evt;
                    }
                }
                self.mods[n].offset = 0;
                let mut i = 0;
                while i < self.mods[n].events.len() {
                    let k = self.mods[n].events[i];
                    i += 1;
                    let frames = self.events_heap[k].offset - self.mods[n].offset;
                    self.mods[n].frames = frames;
                    if frames > 0 && !no_render {
                        let res = self.render_replace(n, bypass);
                        let m = &mut self.mods[n];
                        let (off, fr) = (m.offset, m.frames);
                        render_set_output_content(off, fr, res as i32, m);
                    }
                    let m = &mut self.mods[n];
                    m.offset += m.frames;
                    let mut e = self.events_heap[k];
                    let handled = self.call(n, &mut e);
                    self.events_heap[k] = e;
                    let muted_all = self.all_modules_muted;
                    let m = &mut self.mods[n];
                    match e.cmd {
                        Cmd::Clean => {
                            let mut empty_input = true;
                            'outer: for c in 0..m.input_channels.max(0) as usize {
                                if m.chin[c].is_empty() || m.in_empty[c] >= bs {
                                    continue;
                                }
                                let start = m.in_empty[c].max(m.offset);
                                for a in start..bs {
                                    if m.chin[c][a as usize] != 0.0 {
                                        empty_input = false;
                                        break 'outer;
                                    }
                                }
                            }
                            if empty_input {
                                set_input_content(0, bs, false, m);
                            }
                        }
                        Cmd::SetLocalController | Cmd::SetGlobalController => {
                            self.change_counter += 1;
                            if handled == 0 {
                                set_ctl(m, &e);
                            }
                        }
                        Cmd::SetMsb => {
                            self.change_counter += 1;
                            self.change_counter2 += 1;
                            if handled == 0 {
                                set_msb(m, &e);
                            }
                            bypass = m.realtime_flags & RT_BYPASS != 0;
                        }
                        Cmd::ResetMsb => {
                            self.change_counter += 1;
                            self.change_counter2 += 1;
                            if handled == 0 {
                                reset_msb(m, &e, muted_all);
                            }
                            bypass = m.realtime_flags & RT_BYPASS != 0;
                        }
                        Cmd::Finetune => {
                            self.change_counter += 1;
                            if handled == 0 {
                                finetune(m, &e);
                            }
                        }
                        Cmd::MidimsgSupport => {
                            self.change_counter += 1;
                            if handled == 0 {
                                midimsg_support(m, &e);
                            }
                        }
                        _ => {}
                    }
                }
                if self.mods[n].offset < bs && !no_render {
                    let off = self.mods[n].offset;
                    self.mods[n].frames = bs - off;
                    let res = self.render_replace(n, bypass);
                    let m = &mut self.mods[n];
                    let (off, fr) = (m.offset, m.frames);
                    render_set_output_content(off, fr, res as i32, m);
                }
            }
        }
        let m = &mut self.mods[n];
        m.realtime_flags &= !RT_LOCKED;
        m.realtime_flags |= RT_RENDERED;
        0
    }

    /// RENDER_REPLACE through the module's handler, or psynth_bypass's.
    fn render_replace(&mut self, n: usize, bypass: bool) -> isize {
        if bypass {
            bypass_render(n, self)
        } else {
            let mut e = Event::new(Cmd::RenderReplace);
            self.call(n, &mut e)
        }
    }
}

/// Two distinct modules borrowed at once.
pub(crate) fn two(mods: &mut [ModCommon], a: usize, b: usize) -> (&mut ModCommon, &mut ModCommon) {
    assert!(a != b);
    if a < b {
        let (x, y) = mods.split_at_mut(b);
        (&mut x[a], &mut y[0])
    } else {
        let (x, y) = mods.split_at_mut(a);
        (&mut y[0], &mut x[b])
    }
}

/// PS_STYPE_BUF_CLEAN.
pub(crate) fn clean(buf: &mut [Smp], begin: i32, end: i32) {
    if end > begin {
        let end = (end as usize).min(buf.len());
        let begin = (begin.max(0) as usize).min(end);
        buf[begin..end].fill(0.0);
    }
}

/// PS_STYPE_BUF_COPY.
pub(crate) fn copy(dest: &mut [Smp], src: &[Smp], begin: i32, end: i32) {
    if end > begin {
        let end = (end as usize).min(dest.len()).min(src.len());
        let begin = (begin.max(0) as usize).min(end);
        dest[begin..end].copy_from_slice(&src[begin..end]);
    }
}

/// PS_STYPE_IS_NOT_MIN, for CPUMARK 10.
pub(crate) fn is_not_min(v: Smp) -> bool {
    v.abs() > 1.0 / 16777216.0
}

/// psynth_set_output_content: what a render filled (0 silence, 1 sound,
/// 2 silence written, 3 to be looked at) and where silence starts.
pub(crate) fn set_output_content(offset: i32, size: i32, channels_filled: i32, m: &mut ModCommon) {
    let end = offset + size;
    for c in 0..m.output_channels.max(0) as usize {
        if c >= MAX_CHANNELS || m.chout[c].is_empty() {
            continue;
        }
        let mut filled = channels_filled;
        if filled == 3 {
            filled = 2;
            for i in offset.max(0)..end {
                if is_not_min(m.chout[c][i as usize]) {
                    filled = 1;
                    break;
                }
            }
        }
        if filled == 1 {
            if m.out_empty[c] > offset {
                m.out_empty[c] = offset;
            }
        } else {
            let mut offset2 = offset;
            if offset2 <= m.out_empty[c] {
                offset2 = m.out_empty[c];
                if end > m.out_empty[c] {
                    m.out_empty[c] = end;
                }
            }
            if filled == 0 {
                clean(&mut m.chout[c], offset2, end);
            }
        }
    }
}

/// RENDER_SET_OUTPUT_CONTENT: and silence for a muted module.
fn render_set_output_content(offset: i32, size: i32, res: i32, m: &mut ModCommon) {
    set_output_content(offset, size, res, m);
    if m.realtime_flags & RT_MUTE != 0 {
        set_output_content(offset, size, 0, m);
    }
}

/// psynth_set_input_content.
pub(crate) fn set_input_content(offset: i32, size: i32, filled: bool, m: &mut ModCommon) {
    let end = offset + size;
    for c in 0..m.input_channels.max(0) as usize {
        if c >= MAX_CHANNELS || m.chin[c].is_empty() {
            continue;
        }
        if !filled {
            let mut offset2 = offset;
            if offset2 <= m.in_empty[c] {
                offset2 = m.in_empty[c];
                if end > m.in_empty[c] {
                    m.in_empty[c] = end;
                }
            }
            clean(&mut m.chin[c], offset2, end);
        } else if m.in_empty[c] > offset {
            m.in_empty[c] = offset;
        }
    }
}

/// psynth_set_ctl: a controller event's 0..32768 (or selector value) set.
pub(crate) fn set_ctl(m: &mut ModCommon, e: &Event) {
    let ctl_num = e.ctl_num() as usize;
    let Some(c) = m.ctls.get(ctl_num) else { return };
    let Some(var) = c.var else { return };
    let ctl_val = (e.ctl_val() as u32).min(0x8000);
    let v = if c.typ == 0 {
        let val = ((c.max - c.min) as u32).wrapping_mul(ctl_val) >> 15;
        (val as i32).wrapping_add(c.min)
    } else {
        (ctl_val as i32).clamp_c(c.min, c.max)
    };
    m.vars[var] = v;
}

trait ClampC {
    fn clamp_c(self, min: i32, max: i32) -> i32;
}

impl ClampC for i32 {
    /// C's `if( v < min ) v = min; if( v > max ) v = max;`.
    fn clamp_c(self, min: i32, max: i32) -> i32 {
        let mut v = self;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v
    }
}

fn set_msb(m: &mut ModCommon, e: &Event) {
    let f = e.mute_flags();
    if f & EVENT_MUTE != 0 {
        m.flags |= FLAG_MUTE;
        if m.flags & FLAG_IGNORE_MUTE == 0 {
            m.realtime_flags |= RT_MUTE;
        }
    }
    if f & EVENT_SOLO != 0 {
        m.flags |= FLAG_SOLO;
    }
    if f & EVENT_BYPASS != 0 {
        m.flags |= FLAG_BYPASS;
        m.realtime_flags |= RT_BYPASS;
    }
}

fn reset_msb(m: &mut ModCommon, e: &Event, all_muted: bool) {
    let f = e.mute_flags();
    if f & EVENT_MUTE != 0 {
        m.flags &= !FLAG_MUTE;
        if m.flags & FLAG_IGNORE_MUTE == 0 && !all_muted {
            m.realtime_flags &= !RT_MUTE;
        }
    }
    if f & EVENT_SOLO != 0 {
        m.flags &= !FLAG_SOLO;
    }
    if f & EVENT_BYPASS != 0 {
        m.flags &= !FLAG_BYPASS;
        m.realtime_flags &= !RT_BYPASS;
    }
}

fn finetune(m: &mut ModCommon, e: &Event) {
    if e.finetune() >= -256 {
        m.finetune = e.finetune() as i32;
    }
    if e.relative_note() >= -256 {
        m.relative_note = e.relative_note() as i32;
    }
}

fn midimsg_support(m: &mut ModCommon, e: &Event) {
    let (msg, ctl) = e.midimsg();
    let msg = msg as usize;
    if msg < 3 {
        for c in m.midi_out_msg_map.iter_mut() {
            if *c == ctl {
                *c = 0;
            }
        }
        m.midi_out_msg_map[msg] = ctl;
    }
}

/// psynth_bypass: the input copied to the output.
fn bypass_render(n: usize, net: &mut Net) -> isize {
    let m = &mut net.mods[n];
    if m.chin[0].is_empty() || m.chout[0].is_empty() || m.flags & FLAG_OUTPUT_IS_EMPTY != 0 {
        return 0;
    }
    let max_in = m.chin.iter().take_while(|c| !c.is_empty()).count() as i32;
    let max_out = m.chout.iter().take_while(|c| !c.is_empty()).count() as i32;
    let (prev_out, prev_in) = (m.output_channels, m.input_channels);
    if prev_out != max_out {
        net.set_number_of_outputs(max_out, n);
    }
    if prev_in != max_in {
        net.set_number_of_inputs(max_in, n);
    }
    let m = &mut net.mods[n];
    let (offset, frames) = (m.offset, m.frames);
    let mut no_input = true;
    for ch in 0..max_in as usize {
        if m.in_empty[ch] < offset + frames {
            no_input = false;
            break;
        }
    }
    if no_input {
        return 0;
    }
    let mut inc: Option<usize> = None;
    let mut outc: Option<usize> = None;
    for ch in 0..MAX_CHANNELS {
        if !m.chin[ch].is_empty() {
            inc = Some(ch);
        }
        let Some(i) = inc else { break };
        if !m.chout[ch].is_empty() {
            outc = Some(ch);
        }
        let Some(o) = outc else { break };
        for k in 0..frames as usize {
            let v = m.chin[i][offset as usize + k];
            m.chout[o][offset as usize + k] = v;
        }
    }
    1
}

/// PSYNTH_TICK_SIZE: a tick in frames, 256ths.
pub(crate) fn tick_size(sfreq: i32, bpm: i32) -> u32 {
    ((((sfreq as u64) * 60) << 8) / bpm as u64 / 24) as u32
}

/// string_to_int: a decimal number, sign allowed.
pub(crate) fn string_to_int(s: &str) -> i32 {
    let mut v: i32 = 0;
    let mut neg = false;
    for c in s.bytes() {
        match c {
            b'-' => neg = true,
            b'0'..=b'9' => v = v.wrapping_mul(10).wrapping_add((c - b'0') as i32),
            _ => {}
        }
    }
    if neg {
        -v
    } else {
        v
    }
}

/// The noise every module shares (psynth_get_noise_table): 32768 signed bytes.
pub(crate) fn noise_table() -> &'static [i8] {
    static T: std::sync::OnceLock<Vec<i8>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut r: u32 = 12345;
        (0..32768).map(|_| (pseudo_random(&mut r) as i32 - 32768 / 2) as i8).collect()
    })
}

/// psynth_get_sine_table: `1 << length_bits` points of a sine at `amp`
/// (offset to be unsigned when not `sign`), each an i16.
pub(crate) fn sine_table(sign: bool, length_bits: u32, amp: i32) -> Vec<i16> {
    let len = 1usize << length_bits;
    let mut add = 0.0f32;
    let mut amp2 = amp as f32;
    if !sign {
        amp2 /= 2.0;
        add = amp2;
    }
    (0..len)
        .map(|i| {
            let v = (i as f32 / len as f32 * std::f32::consts::PI * 2.0).sin() * amp2 + add;
            v as i32 as i16
        })
        .collect()
}

pub(crate) const BASE_WAVE_SIZE: usize = 256;
pub(crate) const BASE_WAVES: usize = 9;

/// psynth_get_base_wavetable: triangle, triangle³, saw, saw³, square,
/// sine, half sine, absolute sine, sine³; 256 points each.
pub(crate) fn base_wavetable() -> &'static [Smp] {
    static T: std::sync::OnceLock<Vec<Smp>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let n = BASE_WAVE_SIZE as i32;
        let mut t = vec![0.0f32; BASE_WAVES * BASE_WAVE_SIZE];
        for i in 0..n {
            let p = ((i + n / 4) & (n - 1)) * 4;
            let mut v = -n + p;
            if p >= n * 2 {
                v = n - (p - n * 2);
            }
            t[i as usize] = v as f32 * 1.0 / n as f32;
        }
        for i in 0..BASE_WAVE_SIZE {
            let v = t[i];
            t[BASE_WAVE_SIZE + i] = (v * v / 1.0) * v / 1.0;
        }
        for i in 0..n {
            let p = ((i + n / 2) & (n - 1)) * 2;
            let v = -n + p;
            t[2 * BASE_WAVE_SIZE + i as usize] = v as f32 * 1.0 / n as f32;
        }
        for i in 0..BASE_WAVE_SIZE {
            let v = t[2 * BASE_WAVE_SIZE + i];
            t[3 * BASE_WAVE_SIZE + i] = (v * v / 1.0) * v / 1.0;
        }
        for i in 0..BASE_WAVE_SIZE {
            t[4 * BASE_WAVE_SIZE + i] = if i < BASE_WAVE_SIZE / 2 { 1.0 } else { -1.0 };
        }
        let s = 5 * BASE_WAVE_SIZE;
        for i in 0..BASE_WAVE_SIZE / 2 {
            let v = ((i as f32 / BASE_WAVE_SIZE as f32) as f64 * 2.0 * std::f64::consts::PI).sin() * 1.0;
            t[s + i] = v as f32;
        }
        for i in BASE_WAVE_SIZE / 2..BASE_WAVE_SIZE {
            t[s + i] = -t[s + i - BASE_WAVE_SIZE / 2];
        }
        for i in 0..BASE_WAVE_SIZE / 2 {
            t[6 * BASE_WAVE_SIZE + i] = t[s + i];
        }
        for i in 0..BASE_WAVE_SIZE {
            t[7 * BASE_WAVE_SIZE + i] = t[s + i].abs();
        }
        let s3 = 8 * BASE_WAVE_SIZE;
        for i in 0..BASE_WAVE_SIZE / 2 {
            let v1 = ((i as f32 / BASE_WAVE_SIZE as f32) as f64 * 2.0 * std::f64::consts::PI).sin() as f32;
            let v2 = v1 * v1 * v1;
            t[s3 + i] = v2 * 1.0;
        }
        for i in BASE_WAVE_SIZE / 2..BASE_WAVE_SIZE {
            t[s3 + i] = -t[s3 + i - BASE_WAVE_SIZE / 2];
        }
        t
    })
}

/// g_linear_freq_tab: pow( 2, 1 - i / 768 ) * 267616, truncated.
pub(crate) fn linear_freq_tab() -> &'static [u32; 768] {
    static T: std::sync::OnceLock<[u32; 768]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 768];
        for (i, v) in t.iter_mut().enumerate() {
            *v = ((2.0f64).powf(1.0 - i as f64 / 768.0) * 267616.0) as u32;
        }
        t
    })
}

/// PSYNTH_GET_FREQ: a frequency (in Hz, 4 or 5 octaves above what the
/// generators sound) from a pitch in 64ths of a semitone.
pub(crate) fn get_freq(pitch64: i32) -> i32 {
    let tab = linear_freq_tab();
    if pitch64 >= 0 {
        (tab[(pitch64 % 768) as usize] >> (pitch64 / 768)) as i32
    } else if pitch64 < -7680 {
        let p = 7680 * 4 - 7680;
        (tab[(p % 768) as usize] << -((p / 768) - (7680 * 4) / 768)) as i32
    } else {
        let p = 7680 * 4 + pitch64;
        (tab[(p % 768) as usize] << -((p / 768) - (7680 * 4) / 768)) as i32
    }
}

/// PSYNTH_GET_DELTA64: a 48.16 step, its lowest two bits cleared.
pub(crate) fn get_delta64(srate: i32, freq: i32) -> u64 {
    (((freq as u32 as u64) << 16) / srate as u32 as u64) & 0xFFFF_FFFF_FFFF_FFFC
}

/// PSYNTH_GET_DELTA64_HQ.
pub(crate) fn get_delta64_hq(srate: i32, freq: i32) -> u64 {
    ((freq as u32 as u64) << 16) / srate as u32 as u64
}

/// PSYNTH_GET_DELTA: a 32.16 step split, the low part's lowest two bits cleared.
pub(crate) fn get_delta(srate: i32, freq: i32) -> (u32, u32) {
    let d = ((freq as u32 as u64) << 16) / srate as u32 as u64;
    ((d >> 16) as u32, (d as u32) & (65535 ^ 3))
}

/// PSYNTH_GET_DELTA_HQ.
pub(crate) fn get_delta_hq(srate: i32, freq: i32) -> (u32, u32) {
    let d = ((freq as u32 as u64) << 16) / srate as u32 as u64;
    ((d >> 16) as u32, (d as u32) & 65535)
}
