// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! NSD.Lib's MML compiler (`nsc` 1.31, S.W., 2012-2021; BSD-2, github.com/Shaw02/nsdlib): its MML in, the music
//! data the NSD.Lib driver plays out. A port of the C++ (the reader, the tracks, the driver's emulation that
//! counts ticks and drops what changes nothing, the layout), kept close to the original so that its output is
//! the same byte for byte: checked against `nsc` built from its source and run on the same files
//! (`synth/scripts/nsc-reference.sh`, `tests::against_nsc`).
//!
//! The NSF and NSFe files, with their metadata (`#Text`, `#Maker`, `#plst`, `#psfx`, `#mixe`, `#VRC7`, each
//! track's `#Label`, `#Author`, `#Time`, `#Fade`), are [`link`]'s, the same bytes as nsc's `-n` and `-x`. nsc's
//! messages are its own (Japanese) and are given here in English.

#![allow(non_upper_case_globals)]

pub mod link;
mod decompile;
pub use decompile::decompile;
mod reader;
mod tables;
mod track;

use std::collections::BTreeMap;
use std::fmt;

pub use reader::FileReader;
use reader::{Reader, R};

/// Where nsc stopped: the file and line, the command it was reading, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: usize,
    pub command: String,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (line {}): {}: {}", self.file, self.line, self.command, self.message)
    }
}

impl std::error::Error for Error {}

/// What nsc compiles: the music data (the driver's sequences, envelopes, voices, as its `-A` assembly lays them
/// out) and the DPCM samples, which go to their own segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub music: Vec<u8>,
    pub dpcm: Vec<u8>,
    /// Where each BGM, SE and the DPCM table start in `music`.
    pub bgm: Vec<usize>,
    pub se: Vec<usize>,
    pub dpcm_info: Option<usize>,
    pub title: Vec<u8>,
    pub composer: Vec<u8>,
    pub copyright: Vec<u8>,
    /// `#Maker` and `#Text` (each `#Text` after the first joined by CR LF), for the NSFe and NSF2 metadata.
    pub maker: Vec<u8>,
    pub text: Vec<u8>,
    /// `#VRC7`, `#plst`, `#psfx` and `#mixe` as the metadata chunks nsc makes of them, in the order given.
    pub chunks: Vec<([u8; 4], Vec<u8>)>,
    /// Each BGM's, then each SE's, `#Label`, `#Author`, `#Time` and `#Fade` (-1 when not given).
    pub tracks: Vec<TrackMeta>,
    pub warnings: Vec<String>,
    /// The driver ROM `#Code` names (nsc's default `nsd.bin`), `#External`'s chip flags, whether `#Bank` was
    /// given, and where the DPCM samples go (`#offsetPCM`; $C000 with `#Bank`).
    pub code: Vec<u8>,
    pub external: Option<u8>,
    pub bank: bool,
    pub offset_pcm: usize,
}

/// A BGM's or SE's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackMeta {
    pub label: Vec<u8>,
    pub author: Vec<u8>,
    pub time: i32,
    pub fade: i32,
}

// The driver's commands.
pub(crate) mod op {
    pub const END_OF_TRACK: u8 = 0x00;
    pub const JUMP: u8 = 0x01;
    pub const CALL: u8 = 0x02;
    pub const REPEAT_A_START: u8 = 0x03;
    pub const REPEAT_A_BRANCH: u8 = 0x04;
    pub const REPEAT_A_END: u8 = 0x05;
    pub const CALL_SE: u8 = 0x06;
    pub const POKE: u8 = 0x07;
    pub const TEMPO: u8 = 0x08;
    pub const LENGTH: u8 = 0x09;
    pub const GATE_Q: u8 = 0x0a;
    pub const GATE_U: u8 = 0x0b;
    pub const RELATIVE_TEMPO: u8 = 0x0c;
    pub const GATE_MODE_0: u8 = 0x0d;
    pub const GATE_MODE_1: u8 = 0x0e;
    pub const GATE_MODE_2: u8 = 0x0f;
    pub const ENV_VOICE: u8 = 0x10;
    pub const ENV_VOLUME: u8 = 0x11;
    pub const ENV_FREQUENCY: u8 = 0x12;
    pub const ENV_NOTE: u8 = 0x13;
    pub const DETUNE_CENT: u8 = 0x14;
    pub const DETUNE_REGISTER: u8 = 0x15;
    pub const SWEEP: u8 = 0x16;
    pub const PORTAMENTO: u8 = 0x17;
    pub const REPEAT_B_START: u8 = 0x18;
    pub const REPEAT_B_BRANCH: u8 = 0x19;
    pub const REPEAT_B_END: u8 = 0x1a;
    pub const VOICE: u8 = 0x1b;
    pub const VRC7: u8 = 0x1c;
    pub const VRC7_REG: u8 = 0x1d;
    pub const N163: u8 = 0x1e;
    pub const N163_CHANNEL: u8 = 0x1f;
    pub const VOLUME_DOWN: u8 = 0x20;
    pub const VOLUME_UP: u8 = 0x21;
    pub const FDS_CAREER: u8 = 0x22;
    pub const FDS_MODULATOR: u8 = 0x23;
    pub const FDS_FREQUENCY: u8 = 0x24;
    pub const FDS_VOLUME: u8 = 0x25;
    pub const FME7_FREQUENCY: u8 = 0x26;
    pub const OCTAVE_DOWN: u8 = 0x28;
    pub const OCTAVE_UP: u8 = 0x29;
    pub const TRANSPOSE: u8 = 0x2a;
    pub const RELATIVE_TRANSPOSE: u8 = 0x2b;
    pub const OCTAVE_DOWN_1: u8 = 0x2c;
    pub const OCTAVE_UP_1: u8 = 0x2d;
    pub const SUB_COMMAND: u8 = 0x2f;
    pub const RELEASE_VOICE: u8 = 0x30;
    pub const OCTAVE: u8 = 0x38;
    pub const LENGTH_96: u8 = 0x40;
    pub const GATE_BYTE: u8 = 0x50;
    pub const VOLUME: u8 = 0x60;
    pub const RELEASE_VOLUME: u8 = 0x70;
    pub const NOTE: u8 = 0x80;
    pub const NOTE_SLUR: u8 = 0x40;
    pub const NOTE_LENGTH: u8 = 0x20;
    pub const NOTE_GATE: u8 = 0x10;
    pub const SUB_JUMP_ON: u8 = 0x00;
    pub const SUB_JUMP_OFF: u8 = 0x01;
    pub const SUB_DETUNE_CENT: u8 = 0x02;
    pub const SUB_DETUNE_REGISTER: u8 = 0x03;
}

/// A patch (`Patch(n){...}`): per note from, what a note sets on the way.
#[derive(Clone, Copy, Default)]
pub(crate) struct Scrap {
    pub voi: u32,
    pub evoi: usize,
    pub evol: usize,
    pub em: usize,
    pub en: usize,
    pub key: u32,
    pub sweep: u8,
    pub sub: usize,
    pub gate_q: u32,
    pub gate_u: u32,
    pub f_voi: bool,
    pub f_evoi: bool,
    pub f_evol: bool,
    pub f_em: bool,
    pub f_en: bool,
    pub f_key: bool,
    pub f_sweep: bool,
    pub f_sub: bool,
    pub f_sub_opt: bool,
    pub f_gate_q: bool,
    pub f_gate_u: bool,
    pub sw_evoi: bool,
    pub sw_evol: bool,
    pub sw_em: bool,
    pub sw_en: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Patch {
    m: BTreeMap<usize, Scrap>,
    now: usize,
}

impl Patch {
    /// The scrap in force for a note: the nearest at or below it.
    pub fn set_note(&mut self, note: i32) {
        // A note above 127 comes as a negative `char` in nsc, its search then from far above every key.
        let mut k = if note < 0 { usize::MAX } else { note as usize };
        k = self.m.range(..=k).next_back().map_or(0, |(&n, _)| n);
        self.now = k;
    }
    pub fn now(&self) -> Scrap {
        self.m.get(&self.now).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// A plain item (a track's leading "Null", a loop point).
    Plain,
    /// A command of fixed bytes.
    Event,
    Note,
    /// A command with an address to fix.
    Addr,
    CallSub {
        patch: bool,
        no_opt: bool,
    },
    Repeat,
    Track(usize),
    Set(usize),
    /// A definition: envelope, FDS or VRC7 voice, N163 wave.
    Def,
    DpcmInfo,
}

#[derive(Clone)]
pub(crate) struct Item {
    pub code: Vec<u8>,
    pub kids: Vec<usize>,
    pub size: usize,
    pub offset: usize,
    pub id: usize,
    pub f_id: bool,
    pub used: bool,
    pub kind: Kind,
}

impl Item {
    pub fn get_code(&self, n: usize) -> u8 {
        if n >= self.size {
            0xff
        } else {
            self.code.get(n).copied().unwrap_or(0)
        }
    }
}

#[derive(Clone, Default)]
struct DpcmNote {
    file: Vec<u8>,
    ctrl: u8,
    da: u8,
    next: u8,
    offset: u8,
    size: u8,
}

struct Dpcm {
    data: Vec<u8>,
    size_field: u8,
    offset: usize,
}

struct DpcmInfo {
    item: usize,
    bank: bool,
    max_number: u8,
    notes: Vec<DpcmNote>,
    files: BTreeMap<Vec<u8>, Dpcm>,
    count: usize,
}

pub(crate) struct Nsc<'a> {
    pub mml: Reader<'a>,
    pub items: Vec<Item>,
    pub tracks: Vec<track::Track>,
    pub sets: Vec<track::Set>,
    top: Vec<usize>,
    pub env: BTreeMap<usize, usize>,
    pub fdsc: BTreeMap<usize, usize>,
    pub fdsm: BTreeMap<usize, usize>,
    pub vrc7: BTreeMap<usize, usize>,
    pub n163: BTreeMap<usize, usize>,
    pub sub: BTreeMap<usize, usize>,
    pub bgm: BTreeMap<usize, usize>,
    pub se: BTreeMap<usize, usize>,
    dpcm: Option<DpcmInfo>,
    pub ibgm: usize,
    pub ise: usize,
    offset_pcm: usize,
    bank: bool,
    code: Vec<u8>,
    external: Option<u8>,
    title: Vec<u8>,
    composer: Vec<u8>,
    copyright: Vec<u8>,
    maker: Vec<u8>,
    text: Vec<u8>,
    chunks: Vec<([u8; 4], Vec<u8>)>,
    /// Where `#mixe`'s chunk is in `chunks` (one for all `#mixe`s, as in nsc).
    mixe_at: Option<usize>,
    /// nsc's `-Os` and `-Oo` (both on unless turned off).
    pub opt_seq: bool,
    pub opt_obj: bool,
}

/// nsc: the MML's bytes (Shift JIS) in, the music data out.
pub fn compile(mml: &[u8]) -> Result<Output, Error> {
    compile_with(mml, &|_| None)
}

/// `compile`, with `#include` files and DPCM samples read by `files` (by the name the MML gives).
pub fn compile_with(mml: &[u8], files: FileReader) -> Result<Output, Error> {
    let mut c = Nsc {
        mml: Reader::new("main.mml", mml.to_vec(), files),
        items: Vec::new(),
        tracks: Vec::new(),
        sets: Vec::new(),
        top: Vec::new(),
        env: BTreeMap::new(),
        fdsc: BTreeMap::new(),
        fdsm: BTreeMap::new(),
        vrc7: BTreeMap::new(),
        n163: BTreeMap::new(),
        sub: BTreeMap::new(),
        bgm: BTreeMap::new(),
        se: BTreeMap::new(),
        dpcm: None,
        ibgm: 1,
        ise: 0,
        offset_pcm: 0x10000,
        bank: false,
        code: b"nsd.bin".to_vec(),
        external: None,
        title: Vec::new(),
        composer: Vec::new(),
        copyright: Vec::new(),
        maker: Vec::new(),
        text: Vec::new(),
        chunks: Vec::new(),
        mixe_at: None,
        opt_seq: true,
        opt_obj: true,
    };
    c.music_file()?;
    c.tick_count()?;
    let size = c.top.clone().into_iter().fold(0, |off, i| c.set_offset(i, off));
    c.set_dpcm_offset(size);
    c.fix_address()?;
    let mut music = Vec::new();
    for i in c.top.clone() {
        c.get_code(i, &mut music);
    }
    let mut dpcm = Vec::new();
    if let Some(d) = &c.dpcm {
        for f in d.files.values() {
            dpcm.extend_from_slice(&f.data);
            let padded = (f.data.len() & 0xffc0) + 0x40;
            dpcm.resize(dpcm.len() + padded - f.data.len(), 0);
        }
    }
    Ok(Output {
        music,
        dpcm,
        bgm: (0..c.ibgm).map(|n| c.items[c.sets[c.bgm[&n]].item].offset).collect(),
        se: (0..c.ise).map(|n| c.items[c.sets[c.se[&n]].item].offset).collect(),
        dpcm_info: c.dpcm.as_ref().map(|d| c.items[d.item].offset),
        tracks: c
            .bgm
            .values()
            .chain(c.se.values())
            .map(|&s| {
                let t = &c.sets[s];
                TrackMeta { label: t.label.clone(), author: t.author.clone(), time: t.time, fade: t.fade }
            })
            .collect(),
        title: c.title,
        composer: c.composer,
        copyright: c.copyright,
        maker: c.maker,
        text: c.text,
        chunks: c.chunks,
        warnings: c.mml.warnings,
        code: c.code,
        external: c.external,
        bank: c.bank,
        offset_pcm: c.offset_pcm,
    })
}

impl<'a> Nsc<'a> {
    pub fn new_item(&mut self, code: Vec<u8>, kind: Kind, id: Option<usize>) -> usize {
        let size = code.len();
        self.items.push(Item {
            code,
            kids: Vec::new(),
            size,
            offset: 0,
            id: id.unwrap_or(0),
            f_id: id.is_some(),
            used: false,
            kind,
        });
        self.items.len() - 1
    }

    /// An `mml_general`: a command of fixed bytes.
    pub fn general(&mut self, code: &[u8]) -> usize {
        self.new_item(code.to_vec(), Kind::Event, None)
    }

    /// An `mml_Address`: a command whose address is fixed once the data is laid out.
    pub fn address(&mut self, id: Option<usize>, code: u8, data: Option<u8>) -> usize {
        let code = match data {
            Some(d) => vec![code, d, 0, 0],
            None => vec![code, 0, 0],
        };
        self.new_item(code, Kind::Addr, id)
    }

    fn music_file(&mut self) -> R<()> {
        use tables::mf::*;
        loop {
            self.mml.get_char()?;
            if self.mml.eom() {
                break;
            }
            self.mml.back();
            match self.mml.get_command_id(TABLE) {
                Some(id_Code) | Some(id_Composer) | Some(id_Copyright) | Some(id_Maker) | Some(id_Title) | Some(id_SegmentSEQ) | Some(id_SegmentPCM) | Some(id_Label) => {
                    let id = self.last_id();
                    let mut s = Vec::new();
                    let esc = matches!(id, id_Title | id_Composer | id_Copyright | id_Maker);
                    self.mml.get_string(&mut s, esc)?;
                    match id {
                        id_Title => self.title = s,
                        id_Composer => self.composer = s,
                        id_Copyright => self.copyright = s,
                        id_Maker => self.maker = s,
                        id_Code => self.code = s,
                        _ => {}
                    }
                }
                Some(id_Text) => {
                    // Each `#Text` after the first goes on a new line (CR LF).
                    if !self.text.is_empty() {
                        self.text.extend_from_slice(b"\r\n");
                    }
                    let mut s = std::mem::take(&mut self.text);
                    self.mml.get_string(&mut s, true)?;
                    self.text = s;
                }
                Some(id_plst) => {
                    self.meta_once(*b"plst", "#plst");
                    self.number_list(tables::plst::TABLE, tables::plst::plst_Num, "#plst", self.ibgm as i32 - 1, 0)?
                }
                Some(id_psfx) => {
                    self.meta_once(*b"psfx", "#psfx");
                    let first = self.ibgm as i32;
                    self.number_list(tables::psfx::TABLE, tables::psfx::psfx_Num, "#psfx", self.ise as i32 - 1, first)?
                }
                Some(id_mixe) => self.mixe()?,
                Some(id_vrc7_chg) => {
                    let v = self.mml.get_int_range("#VRC7", 0, 1)?;
                    self.meta_once(*b"VRC7", "#VRC7");
                    self.chunks.last_mut().unwrap().1.push(v as u8);
                }
                Some(id_OffsetPCM) => {
                    let mut o = self.mml.get_int_range("#offsetPCM", 0xc000, 0x10000)?;
                    if !self.bank && o & 0x3f != 0 {
                        self.mml.warning("the DPCM's address is aligned to 64 bytes");
                        o = (o & 0xffc0) + 0x40;
                    }
                    self.offset_pcm = o as usize;
                }
                Some(id_External) => {
                    self.external = Some(self.mml.get_int_range("#External", 0, 0x3f)? as u8);
                }
                Some(id_Bank) => {
                    self.bank = true;
                    self.offset_pcm = 0xc000;
                }
                Some(id_include) => self.mml.include()?,
                Some(id_bgm_num) => self.ibgm = self.mml.get_int_range("#BGM", 0, 255)? as usize,
                Some(id_se_num) => self.ise = self.mml.get_int_range("#SE", 0, 255)? as usize,
                Some(id_timebase) => self.mml.timebase = self.mml.get_int()?,
                Some(id_OctaveReverse) => self.mml.octave_reverse = true,
                Some(id_QReverse) => self.mml.q_reverse = true,
                Some(id_releaseVolume) => self.mml.release_volume = self.mml.get_int_range("#ReleaseVolume", 0, 15)?,
                Some(id_repeatMode) => self.mml.repeat_mode = self.mml.get_int_range("#RepeatMode", 0, 2)?,
                Some(id_TieMode) => self.mml.tie_mode = self.mml.get_int_range("#TieMode", 0, 1)?,
                Some(id_offset_Ei) => self.mml.offset_ei = i64::from(self.mml.get_int()?),
                Some(id_offset_Ev) => self.mml.offset_ev = i64::from(self.mml.get_int()?),
                Some(id_offset_En) => self.mml.offset_en = i64::from(self.mml.get_int()?),
                Some(id_offset_Em) => self.mml.offset_em = i64::from(self.mml.get_int()?),
                Some(id_Priority) => self.mml.priority = self.mml.get_int_range("#priority", 0, 3)? as u8,
                Some(id_QMax) => self.mml.qmax = self.mml.get_int()?,
                Some(id_rest) => self.mml.rest = self.mml.get_int_range("#Rest", 0, 3)?,
                Some(id_wait) => self.mml.wait = self.mml.get_int_range("#Wait", 0, 3)?,
                Some(id_Macro) => self.mml.set_macro(0)?,
                Some(id_Patch) => self.patch()?,
                Some(id_DPCM) => {
                    if self.dpcm.is_some() {
                        return self.mml.err("DPCM is already defined");
                    }
                    self.dpcm_info()?;
                }
                Some(id_FDSC) => self.def_block(1)?,
                Some(id_FDSM) => self.def_block(2)?,
                Some(id_VRC7) => self.def_block(3)?,
                Some(id_N163) => self.def_block(4)?,
                Some(id_Envelop) => self.def_block(5)?,
                Some(id_Vibrato) => self.def_block(6)?,
                Some(id_Sub) => {
                    let n = self.mml.get_num()? as usize;
                    if self.sub.contains_key(&n) {
                        return self.mml.err(format!("Sub({n}) is already defined"));
                    }
                    let s = self.track_set(n, true, false)?;
                    self.top.push(self.sets[s].item);
                    self.sub.insert(n, s);
                }
                Some(id_BGM) | Some(id_SE) => {
                    let se = self.last_id() == id_SE;
                    let n = self.mml.get_num()?;
                    let count = if se { self.ise } else { self.ibgm };
                    self.mml.chk_range(if se { "SE()" } else { "BGM()" }, 0, count as i32 - 1, n)?;
                    let n = n as usize;
                    if (if se { &self.se } else { &self.bgm }).contains_key(&n) {
                        return self.mml.err(format!("{}({n}) is already defined", if se { "SE" } else { "BGM" }));
                    }
                    let s = self.track_set(n, false, se)?;
                    let it = self.sets[s].item;
                    self.items[it].used = true;
                    self.top.push(it);
                    if se {
                        self.se.insert(n, s);
                    } else {
                        self.bgm.insert(n, s);
                    }
                }
                Some(id_Null) => {}
                _ => return self.mml.err_unknown_cmd(),
            }
            if self.mml.eom() {
                break;
            }
        }
        if self.ibgm + self.ise > 255 {
            return self.mml.err("more than 255 BGM and SE together");
        }
        for n in 0..self.ibgm {
            if !self.bgm.contains_key(&n) {
                return self.mml.err(format!("BGM({n}) isn't there"));
            }
        }
        for n in 0..self.ise {
            if !self.se.contains_key(&n) {
                return self.mml.err(format!("SE({n}) isn't there"));
            }
        }
        Ok(())
    }

    // The id the last get_command_id found (kept by the reader as its command name).
    fn last_id(&self) -> u32 {
        self.mml.last_id
    }

    // A new metadata chunk (`#plst`, `#psfx`, `#VRC7`): nsc warns when there was one, and keeps both.
    fn meta_once(&mut self, id: [u8; 4], what: &str) {
        if self.chunks.iter().any(|(i, _)| *i == id) {
            self.mml.warning(format!("{what} is given twice"));
        }
        self.chunks.push((id, Vec::new()));
    }

    // `#plst`, `#psfx`: a list of numbers in braces, each (plus `add`) a byte of the last chunk.
    fn number_list(&mut self, table: &[(&[u8], u32)], num: u32, what: &str, max: i32, add: i32) -> R<()> {
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                return Ok(());
            }
            self.mml.back();
            match self.mml.get_command_id(table) {
                Some(id) if id == num => {
                    self.mml.back();
                    let v = self.mml.get_int_range(what, 0, max)?;
                    self.chunks.last_mut().unwrap().1.push((v + add) as u8);
                }
                Some(_) => {}
                None => return self.mml.err_unknown_cmd(),
            }
        }
    }

    fn mixe(&mut self) -> R<()> {
        use tables::mixe::*;
        let at = match self.mixe_at {
            Some(at) => {
                self.mml.warning("#mixe is given twice");
                at
            }
            None => {
                self.chunks.push((*b"mixe", Vec::new()));
                self.chunks.len() - 1
            }
        };
        self.mixe_at = Some(at);
        let mut id = 0u8;
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                return Ok(());
            }
            self.mml.back();
            match self.mml.get_command_id(TABLE) {
                Some(mixe_Num) => {
                    self.mml.back();
                    let v = self.mml.get_int_range("#mixe", -32768, 32767)?;
                    let data = &mut self.chunks[at].1;
                    if data.chunks(3).any(|c| c[0] == id) {
                        self.mml.warning("#mixe names a chip twice");
                    } else {
                        data.push(id);
                        data.extend_from_slice(&(v as i16).to_le_bytes());
                    }
                }
                Some(mixe_Commma) => {}
                Some(chip) => id = chip as u8,
                None => return self.mml.err_unknown_cmd(),
            }
        }
    }

    // FDSC, FDSM, VRC7, N163, Envelope and the vibrato envelope (`ビブラート`, numbered from 1000000).
    fn def_block(&mut self, what: u8) -> R<()> {
        let (name, n) = match what {
            1 => ("FDSC", self.mml.get_num()? as usize),
            2 => ("FDSM", self.mml.get_num()? as usize),
            3 => ("VRC7", self.mml.get_num()? as usize),
            4 => ("N163", self.mml.get_num()? as usize),
            5 => ("Envelope", self.mml.get_num()? as usize),
            _ => {
                self.mml.offset_em = 1_000_000;
                ("Vibrato", (i64::from(self.mml.get_num()?) + self.mml.offset_em) as usize)
            }
        };
        let map = match what {
            1 => &self.fdsc,
            2 => &self.fdsm,
            3 => &self.vrc7,
            4 => &self.n163,
            _ => &self.env,
        };
        if map.contains_key(&n) {
            return self.mml.err(format!("{name}({n}) is already defined"));
        }
        let code = match what {
            1 => self.wave(tables::fdsc::TABLE, tables::fdsc::FDSC_Num, "FDSC", 63, Some(64))?,
            2 => self.wave(tables::fdsm::TABLE, tables::fdsm::FDSM_Num, "FDSM", 7, Some(32))?,
            3 => self.vrc7_voice()?,
            4 => {
                let w = self.wave(tables::n163::TABLE, tables::n163::N163_Num, "N163", 15, None)?;
                if w.len() > 256 {
                    return self.mml.err("a wave of more than 256 samples");
                }
                if w.len() & 3 != 0 {
                    return self.mml.err("a wave's samples must be a multiple of 4");
                }
                let mut c = vec![(w.len() >> 1) as u8];
                c.extend(w.chunks(2).map(|p| p[0] | (p[1] << 4)));
                c
            }
            _ => self.envelope()?,
        };
        let it = self.new_item(code, Kind::Def, Some(n));
        self.top.push(it);
        match what {
            1 => self.fdsc.insert(n, it),
            2 => self.fdsm.insert(n, it),
            3 => self.vrc7.insert(n, it),
            4 => self.n163.insert(n, it),
            _ => self.env.insert(n, it),
        };
        Ok(())
    }

    fn wave(&mut self, table: &[(&[u8], u32)], num: u32, what: &str, max: i32, count: Option<usize>) -> R<Vec<u8>> {
        let mut w = Vec::new();
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                break;
            }
            self.mml.back();
            match self.mml.get_command_id(table) {
                Some(id) if id == num => {
                    self.mml.back();
                    let i = self.mml.get_int_range(what, 0, max)?;
                    w.push(i as u8 & max as u8);
                }
                Some(_) => {}
                None => return self.mml.err_unknown_cmd(),
            }
        }
        if let Some(n) = count {
            if w.len() != n {
                return self.mml.err(format!("{what} needs {n} values"));
            }
        }
        Ok(w)
    }

    fn vrc7_voice(&mut self) -> R<Vec<u8>> {
        use tables::vrc7::*;
        let mut code = vec![0u8; 8];
        let mut opll = [0u8; 24];
        let (mut mset, mut mode, mut pt) = (false, 0, 0usize);
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                break;
            }
            self.mml.back();
            match self.mml.get_command_id(TABLE) {
                Some(VRC7_Normal) | Some(VRC7_Resister) => {
                    if mset {
                        return self.mml.err("@ or @R only once");
                    }
                    mset = true;
                    mode = if self.last_id() == VRC7_Normal { 0 } else { 1 };
                }
                Some(VRC7_Num) => {
                    if !mset {
                        return self.mml.err("@ or @R first");
                    }
                    self.mml.back();
                    let i = self.mml.get_int_range("a parameter", 0, 255)? as u8;
                    if mode == 0 {
                        if pt >= 24 {
                            return self.mml.err("more than 24 parameters");
                        }
                        opll[pt] = i;
                    } else {
                        if pt >= 8 {
                            return self.mml.err("more than 8 registers");
                        }
                        code[pt] = i;
                    }
                    pt += 1;
                }
                Some(_) => {}
                None => return self.mml.err_unknown_cmd(),
            }
        }
        if mode == 0 {
            if pt < 24 {
                return self.mml.err("fewer than 24 parameters");
            }
            let o = &opll;
            code[0] = ((o[8] & 1) << 7) | ((o[9] & 1) << 6) | ((o[10] & 1) << 5) | ((o[11] & 1) << 4) | (o[7] & 0x0f);
            code[1] = ((o[19] & 1) << 7) | ((o[20] & 1) << 6) | ((o[21] & 1) << 5) | ((o[22] & 1) << 4) | (o[18] & 0x0f);
            code[2] = ((o[6] & 3) << 6) | (o[0] & 0x3f);
            code[3] = ((o[17] & 3) << 6) | ((o[23] & 1) << 4) | ((o[12] & 1) << 3) | (o[1] & 7);
            code[4] = ((o[2] & 0x0f) << 4) | (o[3] & 0x0f);
            code[5] = ((o[13] & 0x0f) << 4) | (o[14] & 0x0f);
            code[6] = ((o[4] & 0x0f) << 4) | (o[5] & 0x0f);
            code[7] = ((o[15] & 0x0f) << 4) | (o[16] & 0x0f);
        } else if pt < 8 {
            return self.mml.err("fewer than 8 registers");
        }
        Ok(code)
    }

    fn envelope(&mut self) -> R<Vec<u8>> {
        use tables::env::*;
        let mut e = Env {
            code: vec![0],
            pt: 1,
            release: false,
            loop_normal: -1,
        };
        let mut loop_release = -1;
        let mut value = 255;
        let mut run = 0;
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                break;
            }
            self.mml.back();
            match self.mml.get_command_id(TABLE) {
                Some(Env_Num) => {
                    self.mml.back();
                    let i = self.mml.get_int_range("an envelope's value", -64, 127)?;
                    if value == i {
                        run += 1;
                    } else {
                        if run > 0 {
                            e.hold(run - 1);
                            run = 0;
                        }
                        value = i;
                        e.code.push(i as u8 & 0x7f);
                        e.pt += 1;
                    }
                }
                Some(Env_Sweep) => {
                    if run > 0 {
                        e.hold(run - 1);
                        run = 0;
                    }
                    value = self.env_sweep(&mut e)?;
                }
                Some(Env_Hold) => run += self.mml.get_int_range("an envelope's hold", 0, 255)? + 1,
                Some(Env_Loop) => {
                    value = 255;
                    if run > 0 {
                        e.hold(run - 1);
                        run = 0;
                    }
                    if e.pt > 0x3f {
                        self.mml.warning("a loop point past 64 bytes");
                    }
                    if !e.release {
                        e.loop_normal = e.pt;
                    } else {
                        loop_release = e.pt;
                    }
                }
                Some(Env_Release) => {
                    value = 255;
                    if run > 0 {
                        e.hold(run - 1);
                        run = 0;
                    }
                    let lp = e.loop_normal;
                    self.env_loop(&mut e, lp)?;
                    e.code[0] = e.pt as u8;
                    e.release = true;
                }
                Some(Env_Commma) => {}
                _ => return self.mml.err_unknown_cmd(),
            }
        }
        let _ = value;
        if run > 0 {
            e.hold(run - 1);
        }
        let lp = if e.release { loop_release } else { e.loop_normal };
        self.env_loop(&mut e, lp)?;
        if e.code.len() > 256 {
            self.mml.warning("an envelope of more than 256 bytes");
        }
        Ok(e.code)
    }

    fn env_loop(&mut self, e: &mut Env, lp: i32) -> R<()> {
        if lp == -1 {
            self.mml.warning("no loop point; the last value is held");
            e.code.push(((e.pt - 1) | 0xc0) as u8);
        } else {
            if e.pt == e.loop_normal {
                return self.mml.err("an envelope can't end on its L");
            }
            e.code.push((lp | 0xc0) as u8);
        }
        e.pt += 1;
        Ok(())
    }

    fn env_sweep(&mut self, e: &mut Env) -> R<i32> {
        let start = self.mml.get_int_range("the start", -64, 127)?;
        self.mml.chk_comma()?;
        let end = self.mml.get_int_range("the end", -64, 127)?;
        self.mml.chk_comma()?;
        let len = self.mml.get_int_range("the length", 1, 255)?;
        let c = self.mml.get_char()?;
        if c != b')' && c != b'}' {
            return self.mml.err("')' is missing");
        }
        let delta = end - start;
        let (mut now, mut cnt) = (0, 0);
        for i in 0..len {
            let t = start + (delta * i) / len;
            if i == 0 {
                now = t;
                cnt = 0;
            } else if t != now {
                e.code.push(now as u8 & 0x7f);
                e.pt += 1;
                if cnt >= 1 {
                    e.hold(cnt - 1);
                }
                now = t;
                cnt = 0;
            } else {
                cnt += 1;
            }
        }
        e.code.push(now as u8 & 0x7f);
        e.pt += 1;
        if cnt >= 1 {
            e.hold(cnt - 1);
        }
        Ok(now)
    }

    fn patch(&mut self) -> R<()> {
        use tables::patch::*;
        let id = self.mml.get_num()? as usize;
        if self.mml.patches.contains_key(&id) {
            return self.mml.err(format!("Patch({id}) is already defined"));
        }
        let mut p = Patch::default();
        p.m.insert(0, Scrap::default());
        p.now = 0;
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                break;
            }
            self.mml.back();
            let cmd = self.mml.get_command_id(TABLE);
            let set_n = |p: &mut Patch, mml: &Reader, n: i32| -> R<()> {
                if !(0..=255).contains(&n) {
                    return mml.err("the note is out of range");
                }
                p.now = n as usize;
                p.m.entry(n as usize).or_default();
                Ok(())
            };
            macro_rules! s {
                () => {
                    p.m.get_mut(&p.now).unwrap()
                };
            }
            match cmd {
                Some(Patch_Macro) => self.mml.call_macro()?,
                Some(Patch_MacroSet) => self.mml.set_macro(1)?,
                Some(k @ Patch_C..=Patch_B) => {
                    let o = self.mml.get_int()?;
                    set_n(&mut p, &self.mml, (o - 1) * 12 + (k - Patch_C) as i32)?;
                }
                Some(Patch_Note) => {
                    let n = self.mml.get_int()?;
                    set_n(&mut p, &self.mml, n)?;
                }
                Some(Patch_Voice) => {
                    if s!().f_evoi || s!().f_voi {
                        return self.mml.err("@ and E@ clash");
                    }
                    let i = self.mml.get_int_range("@", 0, 255)?;
                    let sc = s!();
                    sc.voi = i as u32;
                    sc.f_voi = true;
                    sc.sw_evoi = false;
                }
                Some(Patch_Off_Evoi) => return self.mml.err("E@* doesn't exist; use @"),
                Some(Patch_Off_Evol) | Some(Patch_Off_Em) | Some(Patch_Off_En) => {
                    let k = self.last_id();
                    let sc = s!();
                    let (f, sw) = match k {
                        Patch_Off_Evol => (&mut sc.f_evol, &mut sc.sw_evol),
                        Patch_Off_Em => (&mut sc.f_em, &mut sc.sw_em),
                        _ => (&mut sc.f_en, &mut sc.sw_en),
                    };
                    if *f {
                        return self.mml.err("defined twice");
                    }
                    *f = true;
                    *sw = false;
                }
                Some(Patch_Evoi) => {
                    if s!().f_evoi || s!().f_voi {
                        return self.mml.err("@ and E@ clash");
                    }
                    let i = self.mml.get_int()?;
                    let sc = s!();
                    sc.evoi = i as usize;
                    sc.f_evoi = true;
                    sc.sw_evoi = true;
                }
                Some(Patch_Evol) | Some(Patch_Em) | Some(Patch_En) => {
                    let k = self.last_id();
                    let already = match k {
                        Patch_Evol => s!().f_evol,
                        Patch_Em => s!().f_em,
                        _ => s!().f_en,
                    };
                    if already {
                        return self.mml.err("defined twice");
                    }
                    let i = self.mml.get_int()? as usize;
                    let sc = s!();
                    match k {
                        Patch_Evol => {
                            sc.evol = i;
                            sc.f_evol = true;
                            sc.sw_evol = true;
                        }
                        Patch_Em => {
                            sc.em = i;
                            sc.f_em = true;
                            sc.sw_em = true;
                        }
                        _ => {
                            sc.en = i;
                            sc.f_en = true;
                            sc.sw_en = true;
                        }
                    }
                }
                Some(Patch_Gate_q) => {
                    if s!().f_gate_q {
                        return self.mml.err("q defined twice");
                    }
                    let i = self.mml.get_int_range("q", 0, 255)?;
                    s!().gate_q = i as u32;
                    s!().f_gate_q = true;
                }
                Some(Patch_Gate_u) => {
                    if s!().f_gate_u {
                        return self.mml.err("u defined twice");
                    }
                    let i = if self.mml.get_char()? == b'0' {
                        0
                    } else {
                        self.mml.back();
                        self.mml.get_length(-1)?
                    };
                    s!().gate_u = i as u32;
                    s!().f_gate_u = true;
                }
                Some(Patch_KeyShift) => {
                    if s!().f_key {
                        return self.mml.err("_ defined twice");
                    }
                    let i = self.mml.get_int_range("_", -128, 127)?;
                    s!().key = i as u32;
                    s!().f_key = true;
                }
                Some(Patch_Sweep) => {
                    if s!().f_sweep {
                        return self.mml.err("s defined twice");
                    }
                    let speed = self.mml.get_int()?;
                    let c = if self.mml.get_char()? != b',' {
                        self.mml.chk_range("s", 0, 255, speed)?;
                        self.mml.back();
                        speed as u8
                    } else {
                        self.mml.chk_range("s", 0, 15, speed)?;
                        let depth = self.mml.get_int()?;
                        self.mml.chk_range("s", 0, 15, depth)?;
                        (((speed & 0x0f) << 4) | (depth & 0x0f)) as u8
                    };
                    s!().sweep = c;
                    s!().f_sweep = true;
                }
                Some(Patch_n163set) => {
                    if s!().f_voi || s!().f_evoi {
                        return self.mml.err("@ or E@ is already given");
                    }
                    let i = self.mml.get_int_range("@NS", 0, 252)?;
                    if i % 4 != 0 {
                        return self.mml.err("@NS's address must be a multiple of 4");
                    }
                    let sc = s!();
                    sc.voi = (i / 4) as u32;
                    sc.f_voi = true;
                    sc.sw_evoi = false;
                    if self.mml.get_char()? == b',' {
                        if s!().f_sweep {
                            return self.mml.err("s is already given");
                        }
                        let i = self.mml.get_int_range("@NS's length", 4, 256)?;
                        if i % 4 != 0 {
                            return self.mml.err("@NS's length must be a multiple of 4");
                        }
                        s!().sweep = (64 - i / 4) as u8;
                        s!().f_sweep = true;
                    } else {
                        self.mml.back();
                    }
                }
                Some(Patch_Sub) => {
                    if s!().f_sub {
                        return self.mml.err("S defined twice");
                    }
                    let i = self.mml.get_int()?;
                    s!().sub = i as usize;
                    s!().f_sub = true;
                    if self.mml.get_char()? == b',' {
                        let o = self.mml.get_int_range("S's optimisation flag", 0, 1)?;
                        s!().f_sub_opt = o == 1;
                    } else {
                        self.mml.back();
                    }
                }
                _ => return self.mml.err_unknown_cmd(),
            }
        }
        self.mml.delete_macro(1);
        self.mml.patches.insert(id, p);
        Ok(())
    }

    fn dpcm_info(&mut self) -> R<()> {
        use tables::dpcm::*;
        let it = self.new_item(Vec::new(), Kind::DpcmInfo, None);
        let mut d = DpcmInfo {
            item: it,
            bank: self.bank,
            max_number: 0,
            notes: vec![DpcmNote::default(); 256],
            files: BTreeMap::new(),
            count: 0,
        };
        self.mml.chk_left_curly_brace()?;
        loop {
            let (_, more) = self.mml.get_char_chk_right_curly()?;
            if !more {
                break;
            }
            self.mml.back();
            let note = match self.mml.get_command_id(TABLE) {
                Some(k @ DPCM_C..=DPCM_B) => {
                    let o = self.mml.get_int_range("an octave", 1, 12)?;
                    (o - 1) * 12 + (k - DPCM_C) as i32
                }
                Some(DPCM_Note) => self.mml.get_int_range("a note number", 0, 255)?,
                _ => return self.mml.err_unknown_cmd(),
            };
            self.dpcm_note(&mut d, note as usize)?;
        }
        let n = (usize::from(d.max_number) + 1) * if d.bank { 6 } else { 4 };
        self.items[it].code = vec![0; n];
        self.items[it].size = n;
        self.top.push(it);
        self.dpcm = Some(d);
        Ok(())
    }

    fn dpcm_note(&mut self, d: &mut DpcmInfo, note: usize) -> R<()> {
        if usize::from(d.max_number) < note {
            d.max_number = note as u8;
        }
        self.mml.chk_comma()?;
        let mut file = Vec::new();
        self.mml.get_string(&mut file, false)?;
        if !d.files.contains_key(&file) {
            let name = String::from_utf8_lossy(&file).into_owned();
            let Some(mut data) = (self.mml.inc)(&name) else {
                return self.mml.err(format!("{name}: can't open the file"));
            };
            let mut size = data.len();
            if size > 0x0ff1 {
                self.mml.warning("a DPCM sample over 4081 bytes; cut");
                size = 0x0ff1;
            } else if size & 0x0f != 1 {
                size = (size & 0x0ff0) + 0x11;
            }
            data.resize(size, 0xaa);
            d.files.insert(
                file.clone(),
                Dpcm {
                    size_field: (size >> 4) as u8,
                    data,
                    offset: 0,
                },
            );
            d.count += 1;
        }
        d.notes[note].file = file;
        self.mml.chk_comma()?;
        let freq = self.mml.get_int_range("the DPCM's frequency", 0, 15)?;
        self.mml.chk_comma()?;
        let mode = self.mml.get_int_range("the DPCM's mode", 0, 2)?;
        if mode == 2 && !d.bank {
            return self.mml.err("DPCM mode 2 (IRQ) needs #Bank");
        }
        d.notes[note].ctrl = ((mode << 6) as u8).wrapping_add(freq as u8);
        if self.mml.get_char()? == b',' {
            d.notes[note].da = self.mml.get_int_range("the DPCM's start value", -1, 127)? as u8;
        } else {
            self.mml.back();
            d.notes[note].da = 0;
        }
        if mode == 2 {
            self.mml.chk_comma()?;
            d.notes[note].next = self.mml.get_int_range("the DPCM's next note", -1, 255)? as u8;
        } else {
            d.notes[note].next = 0;
        }
        if self.mml.get_char()? == b',' {
            d.notes[note].offset = self.mml.get_int_range("the DPCM's start offset", 0, 255)? as u8;
        } else {
            self.mml.back();
            d.notes[note].offset = 0;
        }
        if self.mml.get_char()? == b',' {
            d.notes[note].size = self.mml.get_int_range("the DPCM's size", 0, 255)? as u8;
        } else {
            self.mml.back();
            d.notes[note].size = 0;
        }
        Ok(())
    }

    fn set_dpcm_offset(&mut self, mus_size: usize) {
        let mut mus_bank = (mus_size >> 12) as u8;
        if mus_size & 0x0fff != 0 {
            mus_bank = mus_bank.wrapping_add(1);
        }
        let Some(d) = self.dpcm.as_mut() else { return };
        let mut off = self.offset_pcm;
        if d.count > 0 {
            for f in d.files.values_mut() {
                f.offset = off;
                off += (f.data.len() & 0xffc0) + 0x40;
            }
        }
        let bank_no = mus_bank.wrapping_add(3);
        let w = if d.bank { 6 } else { 4 };
        let code = &mut self.items[d.item].code;
        for i in 0..=usize::from(d.max_number) {
            let n = &d.notes[i];
            let c = &mut code[i * w..i * w + w];
            match d.files.get(&n.file).filter(|_| !n.file.is_empty()) {
                None => c.fill(0),
                Some(f) => {
                    c[0] = n.ctrl;
                    c[1] = n.da;
                    c[2] = if d.bank { ((f.offset & 0x0fff) >> 6) as u8 } else { (f.offset.wrapping_sub(0xc000) >> 6) as u8 }.wrapping_add(n.offset);
                    c[3] = if n.size == 0 {
                        if usize::from(f.size_field) >= usize::from(n.offset) * 4 {
                            f.size_field.wrapping_sub(n.offset.wrapping_mul(4))
                        } else {
                            0
                        }
                    } else {
                        n.size
                    };
                    if d.bank {
                        c[4] = ((f.offset.wrapping_sub(0xc000) / 0x1000) as u8).wrapping_add(bank_no);
                        c[5] = n.next;
                    }
                }
            }
        }
    }

    pub fn set_offset(&mut self, i: usize, off: usize) -> usize {
        self.items[i].offset = off;
        let mut o = off + self.items[i].code.len();
        for k in self.items[i].kids.clone() {
            o = self.set_offset(k, o);
        }
        self.items[i].size = o - off;
        o
    }

    fn get_code(&self, i: usize, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.items[i].code);
        for &k in &self.items[i].kids {
            self.get_code(k, out);
        }
    }

    /// `clear_Optimize`: an item nothing uses goes, with what it holds.
    pub fn optimize(&mut self, i: usize) {
        if !self.items[i].used {
            let it = &mut self.items[i];
            it.code.clear();
            it.kids.clear();
            it.size = 0;
        } else {
            for k in self.items[i].kids.clone() {
                self.optimize(k);
            }
        }
    }

    fn tick_count(&mut self) -> R<()> {
        for n in 0..self.ibgm {
            let s = self.bgm[&n];
            self.set_tick_count(s)?;
        }
        for n in 0..self.ise {
            let s = self.se[&n];
            self.set_tick_count(s)?;
        }
        if self.opt_seq {
            let sets: Vec<usize> = self.bgm.values().chain(self.se.values()).chain(self.sub.values()).copied().collect();
            for s in sets {
                let it = self.sets[s].item;
                self.optimize(it);
            }
        }
        if self.opt_obj {
            let defs: Vec<usize> = self
                .env
                .values()
                .chain(self.fdsc.values())
                .chain(self.fdsm.values())
                .chain(self.vrc7.values())
                .chain(self.n163.values())
                .copied()
                .collect();
            for d in defs {
                self.optimize(d);
            }
        }
        Ok(())
    }

    fn fix_address(&mut self) -> R<()> {
        let sets: Vec<usize> = (0..self.ibgm)
            .map(|n| self.bgm[&n])
            .chain((0..self.ise).map(|n| self.se[&n]))
            .chain(self.sub.values().copied())
            .collect();
        for s in sets {
            self.set_fix_address(s)?;
        }
        Ok(())
    }
}

struct Env {
    code: Vec<u8>,
    pt: i32,
    release: bool,
    loop_normal: i32,
}

impl Env {
    fn hold(&mut self, mut len: i32) {
        while len > 15 {
            self.code.push(0x8f);
            self.pt += 1;
            len -= 16;
        }
        self.code.push(((len & 0x0f) | 0x80) as u8);
        self.pt += 1;
    }
}

#[cfg(test)]
mod tests;
