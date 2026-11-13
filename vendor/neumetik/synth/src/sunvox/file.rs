// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A SunVox project read into an engine (sunvox_load_proj_from_fd): its
//! blocks (a four-letter id, a 32-bit length, the data) in order, the
//! project's settings, then each pattern ended by PEND and each module
//! ended by SEND, the modules' links made once all are in.

use super::engine::{Engine, Note, Slot, PAT_INFO_FLAG_CLONE, PAT_INFO_FLAG_JUST_LOADED};
use super::modules;
use super::net::{self, Chunk, FLAG2_JUST_LOADED, FLAG2_LAST, FLAG_EXISTS};

/// Why a file isn't a SunVox project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadError {
    NotSunVox,
    Truncated,
}

/// The file's blocks.
pub(crate) struct Blocks<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Blocks<'a> {
    pub fn new(data: &'a [u8], at: usize) -> Blocks<'a> {
        Blocks { data, at }
    }
}

impl<'a> Iterator for Blocks<'a> {
    /// The id and data, or Err for a block cut off by the file's end.
    type Item = Result<([u8; 4], &'a [u8]), ()>;
    fn next(&mut self) -> Option<Self::Item> {
        let d = self.data;
        if self.at + 4 > d.len() {
            return None;
        }
        let id = [d[self.at], d[self.at + 1], d[self.at + 2], d[self.at + 3]];
        if self.at + 8 > d.len() {
            self.at = d.len();
            return Some(Err(()));
        }
        let len = u32::from_le_bytes([d[self.at + 4], d[self.at + 5], d[self.at + 6], d[self.at + 7]]) as usize;
        let start = self.at + 8;
        if len > d.len() - start {
            self.at = d.len();
            return Some(Err(()));
        }
        self.at = start + len;
        Some(Ok((id, &d[start..start + len])))
    }
}

/// A block's first four bytes as an int (block_data_int), 0 if shorter.
fn int(b: &[u8]) -> i32 {
    if b.len() >= 4 {
        i32::from_le_bytes([b[0], b[1], b[2], b[3]])
    } else {
        0
    }
}

fn ints(b: &[u8]) -> Vec<i32> {
    b.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// A block as a C string (to its first zero).
fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

pub fn is_sunvox(data: &[u8]) -> bool {
    data.len() >= 8 && &data[..8] == b"SVOX\0\0\0\0"
}

/// sunvox_load_proj_from_fd into `s`, a fresh engine (closed and made
/// again as the load does, base version 1.7 until the file says).
pub(crate) fn load(data: &[u8], s: &mut Engine) -> Result<(), LoadError> {
    if !is_sunvox(data) {
        return Err(LoadError::NotSunVox);
    }
    s.base_version = 0x0107_0000;
    s.net.base_host_version = s.base_version;
    for sn in 1..s.net.mods_num() {
        if s.net.mods[sn].flags & FLAG_EXISTS != 0 {
            s.net.remove_module(sn);
        }
    }
    s.flags &= !(super::engine::FLAG_SUPERTRACKS | super::engine::FLAG_NO_TONE_PORTA_ON_TICK0 | super::engine::FLAG_NO_VOL_SLIDE_ON_TICK0 | super::engine::FLAG_KBD_ROUNDROBIN);
    s.jump_request_mode = 0;
    s.supertrack_mute = Default::default();

    let mut pat = PatLoad::new();
    let mut m = ModLoad::new();
    let mut pat_cnt = 0i32;
    let mut pat_remap: Vec<(i32, i32)> = Vec::new();
    let mut s_cnt = 0i32;
    let mut s_remap: Vec<(i32, i32)> = Vec::new();
    let mut s_links2_list: Vec<Option<Vec<i32>>> = Vec::new();
    let mut s_links0: Option<Vec<i32>> = None;
    let mut prev_version = 0u32;
    let mut truncated = false;
    for block in Blocks::new(data, 8) {
        let Ok((id, b)) = block else {
            truncated = true;
            break;
        };
        let v = int(b);
        match &id {
            b"BVER" => {
                s.base_version = v as u32;
                s.net.base_host_version = s.base_version;
            }
            b"VERS" => prev_version = v as u32,
            b"FLGS" => {
                use super::engine::*;
                if v & 1 != 0 {
                    s.flags |= FLAG_SUPERTRACKS;
                }
                if v & 2 != 0 {
                    s.flags |= FLAG_NO_TONE_PORTA_ON_TICK0;
                }
                if v & 4 != 0 {
                    s.flags |= FLAG_NO_VOL_SLIDE_ON_TICK0;
                }
                if v & 8 != 0 {
                    s.flags |= FLAG_KBD_ROUNDROBIN;
                }
                s.net.midi_out_7bit = v & 16 != 0;
            }
            b"BPM " => {
                s.bpm = v as u16;
                if s.bpm == 0 {
                    s.bpm = 125;
                }
            }
            b"SPED" => {
                s.speed = v as u8;
                if s.speed == 0 {
                    s.speed = 6;
                }
            }
            b"TGRD" => s.tgrid = v as u8,
            b"TGD2" => s.tgrid2 = v as u8,
            b"NAME" => s.proj_name = cstr(b),
            b"GVOL" => s.net.global_volume = v,
            b"TIME" => s.line_counter = v,
            b"REPS" => s.restart_pos = v,
            b"STMT" => {
                for (i, w) in ints(b).into_iter().enumerate().take(s.supertrack_mute.len()) {
                    s.supertrack_mute[i] = w as u32;
                }
            }
            b"JAMD" => s.jump_request_mode = v as u8,
            b"PDTA" => pat.data = Some(b.to_vec()),
            b"PNME" => pat.name = Some(cstr(b)),
            b"PLIN" => pat.lines = v,
            b"PCHN" => pat.channels = v,
            b"PYSZ" => pat.ysize = v,
            b"PICO" => pat.icon = true,
            b"PPAR" => pat.parent = v,
            b"PPR#" => {
                pat.parent_id = v as u32;
                pat.parent_id_loaded = true;
            }
            b"PFLG" => pat.parent_flags = v as u32,
            b"PFFF" => pat.flags = v as u32,
            b"PXXX" => pat.x = v,
            b"PYYY" => pat.y = v,
            b"PEND" => {
                end_pattern(s, &mut pat, pat_cnt, prev_version, &mut pat_remap);
                // What C resets after a pattern; channels, height, flags,
                // place and (unless used) the pattern flags carry over.
                pat = PatLoad { flags: pat.flags, parent_flags: pat.parent_flags, x: pat.x, y: pat.y, ysize: pat.ysize, channels: pat.channels, ..PatLoad::new() };
                pat_cnt += 1;
            }
            b"SFFF" => m.flags = v as u32,
            b"SNAM" => m.name = cstr(&b[..b.len().min(32)]),
            b"STYP" => {
                m.kind = if m.flags & FLAG_EXISTS != 0 { Some(modules::kind_by_name(&cstr(b))) } else { None };
            }
            b"SFIN" => m.finetune = v,
            b"SREL" => m.relative_note = v,
            b"SXXX" => m.x = v,
            b"SYYY" => m.y = v,
            b"SZZZ" => m.z = v,
            b"SSCL" => m.scale = v,
            b"SCOL" => {
                if b.len() >= 3 {
                    m.color = [b[0], b[1], b[2]];
                }
            }
            b"SLNK" => m.links = Some(ints(b)),
            b"SLnK" => m.links2 = Some(ints(b)),
            b"CVAL" => m.ctls.push(v),
            b"CHNK" => m.chunks = vec![None; v.max(0) as usize],
            b"CHNM" => m.chunk_num = v,
            b"CHDT" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).data = b.to_vec();
                }
            }
            b"CHFF" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).flags = v as u32;
                }
            }
            b"CHFR" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).freq = v;
                }
            }
            b"SEND" => {
                end_module(s, &mut m, s_cnt, &mut s_remap, &mut s_links2_list, &mut s_links0);
                // What C resets after a module; its name and place carry over.
                m = ModLoad { name: std::mem::take(&mut m.name), x: m.x, y: m.y, z: m.z, chunk_num: m.chunk_num, ..ModLoad::new() };
                s_cnt += 1;
            }
            _ => {}
        }
    }
    // The patterns: clones tied to their parents, module numbers remapped.
    for i in 0..s.pats.len() {
        if s.pats[i] == Slot::Empty || s.pats_info[i].flags & PAT_INFO_FLAG_JUST_LOADED == 0 {
            continue;
        }
        s.pats_info[i].flags &= !PAT_INFO_FLAG_JUST_LOADED;
        if s.pats_info[i].flags & PAT_INFO_FLAG_CLONE != 0 {
            let mut parent = s.pats_info[i].parent_num;
            if !pat_remap.is_empty() {
                parent = remap(&pat_remap, parent);
                s.pats_info[i].parent_num = parent;
            }
            match usize::try_from(parent).ok().filter(|&p| p < s.pats.len()) {
                // A clone of a clone (or a missing parent) copies the slot as it is.
                Some(p) => s.pats[i] = s.pats[p],
                None => s.remove_pattern(i),
            }
        } else if let Slot::Data(d) = s.pats[i] {
            if !s_remap.is_empty() {
                for n in s.pat_data[d].data.iter_mut() {
                    if n.module > 1 {
                        n.module = (remap(&s_remap, n.module as i32 - 1) + 1) as u16;
                    }
                }
            }
        }
        if s.pats[i] == Slot::Placeholder {
            s.pats[i] = Slot::Empty;
        }
    }
    s.check_solo_mode();
    // The modules' links.
    for i in 1..s.net.mods_num() {
        {
            let mm = &mut s.net.mods[i];
            if mm.flags == FLAG_EXISTS && mm.flags2 == FLAG2_LAST {
                mm.flags = 0;
                mm.flags2 = 0;
            }
        }
        let mm = &s.net.mods[i];
        if mm.flags & FLAG_EXISTS == 0 || mm.flags2 & FLAG2_JUST_LOADED == 0 {
            continue;
        }
        s.net.mods[i].flags2 &= !FLAG2_JUST_LOADED;
        let links = s.net.mods[i].input_links.clone();
        for (l, &inp0) in links.iter().enumerate() {
            let mut inp = inp0;
            if !s_remap.is_empty() {
                inp = remap(&s_remap, inp);
                s.net.mods[i].input_links[l] = inp;
            }
            if (inp as u32) < s.net.mods_num() as u32 {
                if s.net.mods[inp as usize].flags & FLAG_EXISTS == 0 {
                    s.net.mods[i].input_links[l] = 0;
                } else {
                    let slot = s_links2_list.get(i).and_then(|o| o.as_ref()).and_then(|l2| l2.get(l)).copied().unwrap_or(-1);
                    s.net.add_link(false, inp, i as i32, slot);
                }
            }
        }
    }
    if let Some(l0) = s_links0 {
        for inp in l0 {
            let inp = if s_remap.is_empty() { inp } else { remap(&s_remap, inp) };
            s.net.make_link(0, inp);
        }
    }
    s.net.remove_empty_modules_at_the_end();
    if truncated {
        return Err(LoadError::Truncated);
    }
    Ok(())
}

fn remap(map: &[(i32, i32)], from: i32) -> i32 {
    map.iter().find(|(f, _)| *f == from).map_or(from, |(_, t)| *t)
}

#[derive(Default)]
struct PatLoad {
    data: Option<Vec<u8>>,
    name: Option<String>,
    channels: i32,
    lines: i32,
    ysize: i32,
    icon: bool,
    parent_id: u32,
    parent_id_loaded: bool,
    parent: i32,
    flags: u32,
    parent_flags: u32,
    x: i32,
    y: i32,
}

/// PEND: the pattern (or clone) made.
fn end_pattern(s: &mut Engine, p: &mut PatLoad, pat_cnt: i32, prev_version: u32, pat_remap: &mut Vec<(i32, i32)>) {
    let parent = p.parent;
    let pat_num = if parent >= 0 { s.new_pattern_empty_clone(parent, p.x, p.y) } else { s.new_pattern(p.lines, p.channels, p.x, p.y) };
    s.pats_info[pat_num].flags = p.flags | PAT_INFO_FLAG_JUST_LOADED;
    if parent >= 0 {
        s.pats_info[pat_num].flags |= PAT_INFO_FLAG_CLONE;
        if p.parent_id_loaded {
            s.pats_info[pat_num].parent_num = -1;
            for i in 0..s.pats.len() {
                if s.pats_info[i].flags & PAT_INFO_FLAG_CLONE != 0 {
                    continue;
                }
                if let Some(pp) = s.pat(i) {
                    if pp.id == p.parent_id {
                        s.pats_info[pat_num].parent_num = i as i32;
                        break;
                    }
                }
            }
            if s.pats_info[pat_num].parent_num == -1 {
                s.remove_pattern(pat_num);
            }
        }
    } else {
        s.pats_info[pat_num].flags &= !PAT_INFO_FLAG_CLONE;
        if !p.icon || p.lines <= 0 || p.channels <= 0 {
            s.remove_pattern(pat_num);
        } else {
            let Slot::Data(d) = s.pats[pat_num] else { return };
            let raw = p.data.take().unwrap_or_default();
            let cells = (p.channels * p.lines) as usize;
            let mut data: Vec<Note> = raw.chunks_exact(8).map(Note::from_bytes).collect();
            data.resize(cells.max(data.len()), Note::default());
            if prev_version < 0x0109_0500 {
                for n in data.iter_mut().take(cells) {
                    if n.module > 255 {
                        n.module &= 255;
                    }
                }
            }
            let pd = &mut s.pat_data[d];
            pd.data = data;
            pd.name = p.name.take();
            pd.flags = p.parent_flags;
            p.parent_flags = 0;
            pd.data_xsize = p.channels;
            pd.data_ysize = p.lines;
            pd.ysize = p.ysize;
            if pat_cnt != pat_num as i32 || !pat_remap.is_empty() {
                pat_remap.push((pat_cnt, pat_num as i32));
            }
        }
    }
}

impl PatLoad {
    fn new() -> PatLoad {
        PatLoad { parent: -1, ..PatLoad::default() }
    }
}

struct ModLoad {
    flags: u32,
    name: String,
    kind: Option<modules::Kind>,
    x: i32,
    y: i32,
    z: i32,
    scale: i32,
    color: [u8; 3],
    finetune: i32,
    relative_note: i32,
    links: Option<Vec<i32>>,
    links2: Option<Vec<i32>>,
    ctls: Vec<i32>,
    chunks: Vec<Option<Chunk>>,
    chunk_num: i32,
}

impl ModLoad {
    fn new() -> ModLoad {
        ModLoad {
            flags: 0,
            name: String::new(),
            kind: None,
            x: 0,
            y: 0,
            z: 0,
            scale: 256,
            color: [255; 3],
            finetune: 0,
            relative_note: 0,
            links: None,
            links2: None,
            ctls: Vec::new(),
            chunks: Vec::new(),
            chunk_num: 0,
        }
    }
}

/// SEND: the module made, its controllers and chunks set, then SETUP_FINISHED.
fn end_module(s: &mut Engine, m: &mut ModLoad, s_cnt: i32, s_remap: &mut Vec<(i32, i32)>, links2_list: &mut Vec<Option<Vec<i32>>>, links0: &mut Option<Vec<i32>>) {
    let bpm = s.bpm as i32;
    let speed = s.speed as i32;
    if m.flags & FLAG_EXISTS == 0 {
        // An empty slot keeps its number: a module made and removed there.
        let n = s.net.add_module(-1, None, Some(""), 0, 0, 0, 0, bpm, speed);
        if n >= 0 {
            s.net.remove_module(n as usize);
            s.net.mods[n as usize].flags2 = FLAG2_LAST;
            s.net.mods[n as usize].flags = FLAG_EXISTS;
        }
        return;
    }
    let n = if s_cnt == 0 {
        0
    } else {
        let n = s.net.add_module(-1, m.kind.or(Some(modules::Kind::Empty)), Some(&m.name), m.flags, m.x, m.y, m.z, bpm, speed);
        if n >= 0 {
            s.net.mods[n as usize].flags2 |= FLAG2_JUST_LOADED;
        }
        n
    };
    if n < 0 {
        return;
    }
    let nu = n as usize;
    if s_cnt != 0 {
        if let Some(l) = m.links.take() {
            s.net.mods[nu].input_links = l;
        }
        if let Some(l2) = m.links2.take() {
            if links2_list.len() <= nu {
                links2_list.resize(nu + 1, None);
            }
            links2_list[nu] = Some(l2);
        }
        if s_cnt != n || !s_remap.is_empty() {
            s_remap.push((s_cnt, n));
        }
    } else {
        *links0 = m.links.take();
        if let Some(l2) = m.links2.take() {
            if links2_list.is_empty() {
                links2_list.push(None);
            }
            links2_list[0] = Some(l2);
        }
    }
    let mm = &mut s.net.mods[nu];
    mm.finetune = m.finetune;
    mm.relative_note = m.relative_note;
    mm.x = m.x;
    mm.y = m.y;
    mm.z = m.z;
    mm.scale = m.scale as u16 as i32;
    mm.color = m.color;
    let c_num = m.ctls.len();
    if c_num > mm.ctls.len() {
        s.net.change_ctls_num(nu, c_num);
    }
    let mm = &mut s.net.mods[nu];
    for (cc, &v) in m.ctls.iter().enumerate() {
        if let Some(var) = mm.ctls.get(cc).and_then(|c| c.var) {
            mm.vars[var] = v;
        }
    }
    for (c, chunk) in std::mem::take(&mut m.chunks).into_iter().enumerate() {
        // A chunk counts once it has data (a CHDT block that isn't empty).
        if let Some(chunk) = chunk.filter(|c| !c.data.is_empty()) {
            s.net.new_chunk(nu, c, chunk);
        }
    }
    s.net.do_command(nu, net::Cmd::SetupFinished);
}

/// Each module of a project as a module file (.sunsynth, what
/// [`load_module`] reads): its number, name, kind and the file. Only what
/// makes its sound goes in (its kind, tuning, controllers and chunks);
/// not its name, place, colour, links or the song's mute and solo, so the
/// same module in two songs is the same bytes.
pub(crate) fn module_files(data: &[u8]) -> Vec<(usize, String, modules::Kind, Vec<u8>)> {
    let mut out = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    let (mut name, mut kind, mut n) = (String::new(), None, 0usize);
    for block in Blocks::new(data, 8) {
        let Ok((id, b)) = block else { break };
        match &id {
            b"SNAM" => name = cstr(&b[..b.len().min(32)]),
            b"STYP" | b"SFIN" | b"SREL" | b"CVAL" | b"CHNK" | b"CHNM" | b"CHDT" | b"CHFF" | b"CHFR" => {
                if &id == b"STYP" {
                    kind = Some(modules::kind_by_name(&cstr(b)));
                }
                body.extend_from_slice(&id);
                body.extend_from_slice(&(b.len() as u32).to_le_bytes());
                body.extend_from_slice(b);
            }
            b"SEND" => {
                if let Some(k) = kind.take() {
                    let mut f = b"SSYN\0\0\0\0".to_vec();
                    f.extend_from_slice(b"SFFF");
                    f.extend_from_slice(&4u32.to_le_bytes());
                    f.extend_from_slice(&net::FLAG_EXISTS.to_le_bytes());
                    f.append(&mut body);
                    f.extend_from_slice(b"SEND");
                    f.extend_from_slice(&0u32.to_le_bytes());
                    out.push((n, std::mem::take(&mut name), k, f));
                }
                body.clear();
                n += 1;
            }
            _ => {}
        }
    }
    out
}

/// sunvox_load_module_from_fd: a module file (.sunsynth) added to `s`'s
/// network at its first free slot; its number, or -1.
pub(crate) fn load_module(data: &[u8], s: &mut Engine) -> i32 {
    if data.len() < 8 || &data[..8] != b"SSYN\0\0\0\0" {
        return -1;
    }
    let mut m = ModLoad::new();
    let mut kind: Option<modules::Kind> = None;
    for block in Blocks::new(data, 8) {
        let Ok((id, b)) = block else { break };
        let v = int(b);
        match &id {
            b"SNAM" => m.name = cstr(&b[..b.len().min(32)]),
            b"SFFF" => m.flags = v as u32,
            b"SFIN" => m.finetune = v,
            b"SREL" => m.relative_note = v,
            b"SSCL" => m.scale = v,
            b"SCOL" => {
                if b.len() >= 3 {
                    m.color = [b[0], b[1], b[2]];
                }
            }
            b"STYP" => kind = Some(modules::kind_by_name(&cstr(b))),
            b"CVAL" => m.ctls.push(v),
            b"CHNK" => m.chunks = vec![None; v.max(0) as usize],
            b"CHNM" => m.chunk_num = v,
            b"CHDT" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).data = b.to_vec();
                }
            }
            b"CHFF" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).flags = v as u32;
                }
            }
            b"CHFR" => {
                if let Some(c) = m.chunks.get_mut(m.chunk_num as usize) {
                    c.get_or_insert_with(Chunk::default).freq = v;
                }
            }
            b"SEND" => {
                let (bpm, speed) = (s.bpm as i32, s.speed as i32);
                let kind = kind.unwrap_or(modules::Kind::Empty);
                if kind == modules::Kind::Empty {
                    return s.net.add_module(-1, Some(kind), Some("Unsupported"), m.flags, 0, 0, 0, bpm, speed);
                }
                let n = s.net.add_module(-1, Some(kind), Some(&m.name), m.flags, 0, 0, 0, bpm, speed);
                if n < 0 {
                    return n;
                }
                let nu = n as usize;
                let mm = &mut s.net.mods[nu];
                mm.finetune = m.finetune;
                mm.relative_note = m.relative_note;
                mm.scale = m.scale as u16 as i32;
                mm.color = m.color;
                if m.ctls.len() > mm.ctls.len() {
                    s.net.change_ctls_num(nu, m.ctls.len());
                }
                let mm = &mut s.net.mods[nu];
                for (cc, &v) in m.ctls.iter().enumerate() {
                    if let Some(var) = mm.ctls.get(cc).and_then(|c| c.var) {
                        mm.vars[var] = v;
                    }
                }
                for (c, chunk) in std::mem::take(&mut m.chunks).into_iter().enumerate() {
                    if let Some(chunk) = chunk.filter(|c| !c.data.is_empty()) {
                        s.net.new_chunk(nu, c, chunk);
                    }
                }
                s.net.do_command(nu, net::Cmd::SetupFinished);
                return n;
            }
            _ => {}
        }
    }
    -1
}
