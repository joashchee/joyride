// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's MML compiler (org.si.sion.sequencer.base.MMLParser and
// MMLSequencer, SiMMLSequencer's onBeforeCompile and system commands,
// github.com/keim/SiON), as neumetik-midi's `text/sion.rs` has it, but
// keeping everything the synthesizer plays: the tone definitions, the
// envelope tables and the commands.
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! The compiler: MML text to sequences of events, a global tempo
//! sequence, and the tables and voices the song defines.

use super::module::ChannelParam;
use super::table::Table;
use super::track::{EnvTable, Voice};
use super::pcm::{parse_pcm_voice, parse_pcm_wave, parse_sampler_wave, SampleBank, SoundReference, SAMPLER_TABLE_MAX};
use super::table::{calc_log_table_index, WaveTable, NOTE_BITS, NOTE_TABLE_SIZE};
use super::translate::{parse_table_numbers, parse_tone, parse_wav, parse_wavb, ToneFormat};
use std::sync::Arc;

/// Ticks a whole note (`MMLParserSetting.resolution`).
pub const RES: i64 = 1920;

/// SiON's `int.MIN_VALUE`, for a parameter left out.
pub const NONE: i64 = i32::MIN as i64;

/// The most steps one sequence takes, past any song, short of a trap.
const MOST_STEPS: usize = 1 << 23;

/// The most text macros and repeats expand to.
const MOST_TEXT: usize = 8 << 20;

/// What an event is (`MMLEvent`'s ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K {
    Note,
    Rest,
    Quant,
    QuantCount,
    Volume,
    VolumeShift,
    FineVolume,
    Slur,
    SlurWeak,
    Bend,
    RepBegin,
    RepBreak,
    RepEnd,
    RepAll,
    ModType,
    ModParam,
    InPipe,
    OutPipe,
    Param,
    Tempo,
    System,
    Table,
    /// A command SiMMLSequencer registers by its letters.
    User(&'static str),
}

#[derive(Debug, Clone)]
pub struct E {
    pub k: K,
    pub data: i64,
    pub len: i64,
    /// A repeat's other end (begin: its end; end and break: their begin).
    pub jump: usize,
}

/// One `;`-ended sequence, as the parser chains it.
#[derive(Debug, Clone, Default)]
pub struct Seq {
    pub ev: Vec<E>,
    /// Tempo events in it (`SEQUENCE_HEAD.data`).
    pub globals: usize,
}

/// SiMMLSequencer's command letters (`newMMLEventListener`).
const USER: &[&str] = &[
    "k", "kt", "!@kr", "@mask", "p", "@p", "@f", "x", "%v", "%x", "%f", "@clock", "@al", "@fb", "@r", "%t", "%e", "i", "@rr", "@tl", "@ml", "@dt", "@ph", "@fx", "@se", "@er", "s", "@lfo", "mp", "ma", "@fps", "@@", "na",
    "np", "nt", "nf", "_@@", "_na", "_np", "_nt", "_nf", "!na", "po",
];

/// `MMLParserSetting` as SiMMLSequencer sets it, and what `#` commands change.
#[derive(Debug, Clone)]
pub struct Setting {
    pub max_quant_ratio: i64,
    pub default_quant_ratio: i64,
    pub octave_polarization: i64,
    pub volume_polarization: i64,
    pub key_sign: [i64; 7],
    /// `t`: 0 BPM, 1 TIMERB, 2 frame.
    pub tmode: u8,
    pub tres: f64,
    pub default_shift: i64,
    pub velocity_mode: i32,
    pub expression_mode: i32,
}

impl Setting {
    pub fn new() -> Setting {
        Setting { max_quant_ratio: 8, default_quant_ratio: 6, octave_polarization: 1, volume_polarization: 1, key_sign: [0; 7], tmode: 0, tres: 1.0, default_shift: 4, velocity_mode: 0, expression_mode: 0 }
    }
}

const KEY_SIGNS: [[i64; 7]; 15] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, -1],
    [0, 0, -1, 0, 0, 0, -1],
    [0, 0, -1, 0, 0, -1, -1],
    [0, -1, -1, 0, 0, -1, -1],
    [0, -1, -1, 0, -1, -1, -1],
    [-1, -1, -1, 0, -1, -1, -1],
    [-1, -1, -1, -1, -1, -1, -1],
];

/// `MMLParser.keySign`: the signature a letter names.
fn key_sign(sign: &str) -> Option<[i64; 7]> {
    let n = match sign {
        "" | "C" | "Am" => 0,
        "G" | "Em" => 1,
        "D" | "Bm" => 2,
        "A" | "F+m" | "F#m" => 3,
        "E" | "C+m" | "C#m" => 4,
        "B" | "G+m" | "G#m" => 5,
        "F+" | "F#" | "D+m" | "D#m" => 6,
        "C+" | "C#" | "A+m" | "A#m" => 7,
        "F" | "Dm" => 8,
        "B-" | "Bb" | "Gm" => 9,
        "E-" | "Eb" | "Cm" => 10,
        "A-" | "Ab" | "Fm" => 11,
        "D-" | "Db" | "B-m" | "Bbm" => 12,
        "G-" | "Gb" | "E-m" | "Ebm" => 13,
        "C-" | "Cb" | "A-m" | "Abm" => 14,
        _ => return None,
    };
    Some(KEY_SIGNS[n])
}

/// ActionScript's `int(string)`: digits as far as they go, else 0.
fn as_int(s: &str) -> i64 {
    let s = s.trim();
    let (neg, digits) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    let n = digits[..end].parse::<i64>().unwrap_or(0).min(i64::from(i32::MAX));
    if neg {
        -n
    } else {
        n
    }
}

fn starts(s: &[char], at: usize, pat: &str) -> bool {
    let mut i = at;
    for c in pat.chars() {
        if s.get(i) != Some(&c) {
            return false;
        }
        i += 1;
    }
    true
}

fn find(s: &[char], from: usize, pat: &str) -> Option<usize> {
    (from..s.len()).find(|&i| starts(s, i, pat))
}

fn find_char(s: &[char], from: usize, c: char) -> Option<usize> {
    (from..s.len()).find(|&i| s[i] == c)
}

fn text_of(s: &[char]) -> String {
    s.iter().collect()
}

/// Removes `/* */` and `// ... newline` as `onBeforeCompile` does.
fn strip_comments(s: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if starts(s, i, "/*") {
            if let Some(e) = find(s, i + 2, "*/") {
                i = e + 2;
                continue;
            }
        } else if starts(s, i, "//") {
            if let Some(n) = (i + 2..s.len()).find(|&j| matches!(s[j], '\r' | '\n')) {
                let mut j = n;
                while j < s.len() && matches!(s[j], '\r' | '\n') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// Macros and the commands before parsing.
pub struct Pre {
    macros: [String; 26],
    dynamic: bool,
    pub setting: Setting,
    pub title: String,
    pub envelopes: Vec<Option<Arc<EnvTable>>>,
    pub voices: Vec<Option<Arc<Voice>>>,
    pub waves: Vec<Option<Arc<WaveTable>>>,
    pub effects: Vec<(i64, String, String)>,
    pub fps: i32,
    next_table: i64,
    table: Arc<Table>,
    sounds: Arc<SoundReference>,
    pub samples: SampleBank,
}

impl Pre {
    /// `_expandMacro`: every capital letter a macro, `A(n)` transposed by n.
    fn expand(&self, m: &[char], depth: usize) -> Result<Vec<char>, String> {
        if depth > 64 {
            return Err("a macro uses itself".into());
        }
        let mut out = Vec::with_capacity(m.len());
        let mut i = 0;
        while i < m.len() {
            let c = m[i];
            if !c.is_ascii_uppercase() {
                out.push(c);
                i += 1;
                continue;
            }
            let id = (c as u8 - b'A') as usize;
            i += 1;
            // ( digits and minus signs )
            let mut arg = None;
            if m.get(i) == Some(&'(') {
                let mut j = i + 1;
                while j < m.len() && (m[j] == '-' || m[j].is_ascii_digit()) {
                    j += 1;
                }
                if j > i + 1 && m.get(j) == Some(&')') {
                    arg = Some(as_int(&text_of(&m[i + 1..j])));
                    i = j + 1;
                }
            }
            let body = &self.macros[id];
            if body.is_empty() {
                continue;
            }
            let body: Vec<char> = if self.dynamic { self.expand(&body.chars().collect::<Vec<_>>(), depth + 1)? } else { body.chars().collect() };
            if let Some(t) = arg {
                out.extend(format!("!@ns{t}").chars());
                out.extend(body);
                out.extend(format!("!@ns{}", -t).chars());
            } else {
                out.extend(body);
            }
            if out.len() > MOST_TEXT {
                return Err("the macros are too long".into());
            }
        }
        Ok(out)
    }

    /// `_parseSystemCommandBefore`: whether it was done now.
    fn system(&mut self, cmd: &str, prm: &[char]) -> Result<bool, String> {
        // \s*(\d*)\s*(\{(.*?)\})?(.*)
        let mut i = 0;
        while i < prm.len() && prm[i].is_whitespace() {
            i += 1;
        }
        let d0 = i;
        while i < prm.len() && prm[i].is_ascii_digit() {
            i += 1;
        }
        let num = as_int(&text_of(&prm[d0..i]));
        while i < prm.len() && prm[i].is_whitespace() {
            i += 1;
        }
        let (no_data, dat, pfx) = if prm.get(i) == Some(&'{') {
            match find_char(prm, i + 1, '}') {
                Some(e) => (false, text_of(&prm[i + 1..e]), text_of(&prm[e + 1..])),
                None => (true, String::new(), text_of(&prm[i..])),
            }
        } else {
            (true, String::new(), text_of(&prm[i..]))
        };
        match cmd {
            "#TITLE" => self.title = if no_data { pfx } else { dat },
            "#FPS" => {
                self.fps = if num > 0 {
                    num as i32
                } else if no_data {
                    60
                } else {
                    as_int(&dat) as i32
                }
            }
            "#TABLE" => {
                if !(0..=254).contains(&num) {
                    return Err("#TABLE is 0 to 254".into());
                }
                let t = parse_table_numbers(&dat, &pfx, 65536)?;
                self.envelopes[num as usize] = Some(Arc::new(t));
            }
            "#EFFECT" => self.effects.push((num, dat.clone(), pfx.clone())),
            "#WAV" | "#WAVB" => {
                if !(0..=255).contains(&num) {
                    return Err("#WAV is 0 to 255".into());
                }
                let wave = if cmd == "#WAV" { parse_wav(&dat, &pfx)? } else { parse_wavb(if no_data { &pfx } else { &dat }) };
                let table: Vec<i32> = wave.iter().map(|&w| calc_log_table_index(w)).collect();
                self.waves[(num & 127) as usize] = Some(Arc::new(WaveTable::new(table, 0)));
            }
            "#SAMPLER" => {
                if !(0..=255).contains(&num) {
                    return Err("#SAMPLE is 0 to 255".into());
                }
                // Without a sound reference the command is left alone, as in SiON.
                if !self.sounds.is_empty() {
                    let bank = ((num >> NOTE_BITS) & (SAMPLER_TABLE_MAX as i64 - 1)) as usize;
                    let note = (num & (NOTE_TABLE_SIZE as i64 - 1)) as i32;
                    let refs = self.sounds.clone();
                    let t = self.table.clone();
                    parse_sampler_wave(self.samples.sampler_table_mut(bank), note, &dat, &refs, &t);
                }
            }
            "#PCMWAVE" => {
                if !(0..=255).contains(&num) {
                    return Err("#PCMWAVE is 0 to 255".into());
                }
                if !self.sounds.is_empty() {
                    let refs = self.sounds.clone();
                    let t = self.table.clone();
                    parse_pcm_wave(self.samples.pcm_table_mut(num as i32), &dat, &refs, &t);
                }
            }
            "#PCMVOICE" => {
                if !(0..=255).contains(&num) {
                    return Err("#PCMVOICE is 0 to 255".into());
                }
                if !self.sounds.is_empty() {
                    parse_pcm_voice(self.samples.pcm_table_mut(num as i32), &dat);
                }
            }
            "#WAVEXP" | "#PCMB" | "#PCMC" => {}
            "#OPX@" | "#MA@" | "#AL@" => {}
            "#@" | "#OPM@" | "#OPN@" | "#OPL@" => {
                let format = match cmd {
                    "#OPM@" => ToneFormat::Opm,
                    "#OPN@" => ToneFormat::Opn,
                    "#OPL@" => ToneFormat::Opl,
                    _ => ToneFormat::Param,
                };
                let mut v = Voice::default();
                parse_tone(format, &mut v.channel_param, &dat, &self.table)?;
                if !pfx.is_empty() {
                    v.channel_param.init_sequence = pfx.trim().to_string();
                }
                self.voices[(num & 255) as usize] = Some(Arc::new(v));
            }
            "#SIGN" => {
                let s = if no_data { pfx.trim().to_string() } else { dat };
                self.setting.key_sign = key_sign(&s).ok_or("it isn't a key signature")?;
            }
            "#MACRO" => {
                let d = if no_data { pfx.trim().to_string() } else { dat };
                match d.as_str() {
                    "dynamic" => self.dynamic = true,
                    "static" => self.dynamic = false,
                    _ => return Err("#MACRO is dynamic or static".into()),
                }
            }
            "#QUANT" => {
                if num > 0 {
                    self.setting.max_quant_ratio = num;
                    self.setting.default_quant_ratio = (num as f64 * 0.75) as i64;
                }
            }
            "#TMODE" => {
                // (unit|timerb|fps)=?([\d.]*)
                let d: Vec<char> = dat.chars().collect();
                for (name, mode) in [("unit", 0u8), ("timerb", 1), ("fps", 2)] {
                    if let Some(at) = find(&d, 0, name) {
                        let mut j = at + name.len();
                        if d.get(j) == Some(&'=') {
                            j += 1;
                        }
                        let k = (j..d.len()).find(|&k| !(d[k].is_ascii_digit() || d[k] == '.')).unwrap_or(d.len());
                        let n: f64 = text_of(&d[j..k]).parse().unwrap_or(0.0);
                        match mode {
                            0 => self.setting.tres = if n > 0.0 { 1.0 / n } else { 1.0 },
                            1 => self.setting.tres = (if n > 0.0 { n } else { 4000.0 }) * 1.220703125,
                            _ => self.setting.tres = if n > 0.0 { n * 60.0 } else { 3600.0 },
                        }
                        self.setting.tmode = mode;
                        break;
                    }
                }
            }
            "#VMODE" => {
                // (n88|mdx|psg|mck|tss|%[xv])(\d*)(\s*,?\s*(\d?)), again and again.
                let d: Vec<char> = dat.chars().collect();
                let mut i = 0;
                while i < d.len() {
                    let word = ["n88", "mdx", "psg", "mck", "tss", "%x", "%v"].into_iter().find(|w| starts(&d, i, w));
                    let Some(w) = word else {
                        i += 1;
                        continue;
                    };
                    let mut j = i + w.len();
                    let ds = j;
                    while j < d.len() && d[j].is_ascii_digit() {
                        j += 1;
                    }
                    let n = as_int(&text_of(&d[ds..j]));
                    // \s*,?\s*(\d?)
                    while j < d.len() && d[j].is_whitespace() {
                        j += 1;
                    }
                    if d.get(j) == Some(&',') {
                        j += 1;
                    }
                    while j < d.len() && d[j].is_whitespace() {
                        j += 1;
                    }
                    let digit = d.get(j).filter(|c| c.is_ascii_digit()).map(|c| i64::from(*c as u8 - b'0'));
                    if digit.is_some() {
                        j += 1;
                    }
                    let st = &mut self.setting;
                    match w {
                        "%v" => {
                            st.velocity_mode = if (0..5).contains(&n) { n as i32 } else { 0 };
                            let sh = digit.unwrap_or(4);
                            st.default_shift = if (0..8).contains(&sh) { sh } else { 0 };
                        }
                        "%x" => st.expression_mode = if (0..5).contains(&n) { n as i32 } else { 0 },
                        "n88" | "mdx" => {
                            st.velocity_mode = 4;
                            st.expression_mode = 3;
                        }
                        "psg" => {
                            st.velocity_mode = 3;
                            st.expression_mode = 3;
                        }
                        _ => {
                            st.velocity_mode = 0;
                            st.expression_mode = 0;
                        }
                    }
                    i = j;
                }
            }
            "#REV" => {
                let d = if no_data { pfx.trim().to_string() } else { dat };
                match d.as_str() {
                    "" => {
                        self.setting.octave_polarization = -1;
                        self.setting.volume_polarization = -1;
                    }
                    "octave" => self.setting.octave_polarization = -1,
                    "volume" => self.setting.volume_polarization = -1,
                    _ => return Err("#REV is octave or volume".into()),
                }
            }
            // After parsing: the FM connections, which aren't made here.
            "#FM" => return Ok(false),
            _ => {}
        }
        Ok(true)
    }
}

/// `onBeforeCompile`: comments, macros and the `#` commands out, the
/// sequences left, with `![ !]` repeats written out.
pub fn precompile(text: &str, table: Arc<Table>) -> Result<(Vec<char>, Pre), String> {
    precompile_with(text, table, Arc::new(SoundReference::new()))
}

/// `precompile` with the sounds `#SAMPLER`, `#PCMWAVE` and `#PCMVOICE` name.
pub fn precompile_with(text: &str, table: Arc<Table>, sounds: Arc<SoundReference>) -> Result<(Vec<char>, Pre), String> {
    let mut pre = Pre { macros: Default::default(), dynamic: false, setting: Setting::new(), title: String::new(), envelopes: vec![None; 512], voices: vec![None; 256], waves: vec![None; 128], effects: vec![], fps: 60, next_table: 511, table, sounds, samples: SampleBank::new() };
    let mut s: Vec<char> = text.chars().collect();
    s.push('\n');
    let mut s = strip_comments(&s);
    while s.last().is_some_and(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        s.pop();
    }
    if s.is_empty() {
        return Err("there is no MML in it".into());
    }
    if s.last() != Some(&';') {
        s.push(';');
    }
    let mut out: Vec<char> = vec![];
    let mut i = 0;
    while i < s.len() {
        while i < s.len() && matches!(s[i], ' ' | '\t' | '\r' | '\n') {
            i += 1;
        }
        if i >= s.len() {
            break;
        }
        // (#([A-Z@\-]+)(\+=|=)?)?
        let mut name: Option<String> = None;
        let mut op = 0u8;
        if s[i] == '#' {
            let mut j = i + 1;
            while j < s.len() && (s[j].is_ascii_uppercase() || s[j] == '@' || s[j] == '-') {
                j += 1;
            }
            if j > i + 1 {
                name = Some(text_of(&s[i + 1..j]));
                i = j;
                if starts(&s, i, "+=") {
                    op = 2;
                    i += 2;
                } else if s.get(i) == Some(&'=') {
                    op = 1;
                    i += 1;
                }
            }
        }
        // ([^;{]*({.*?})?[^;]*);
        let mut p = i;
        while p < s.len() && s[p] != ';' && s[p] != '{' {
            p += 1;
        }
        if s.get(p) == Some(&'{') {
            let mut c = p + 1;
            while let Some(q) = find_char(&s, c, '}') {
                if find_char(&s, q + 1, ';').is_some() {
                    p = q + 1;
                    break;
                }
                c = q + 1;
            }
        }
        let Some(end) = find_char(&s, p, ';') else { break };
        let body = &s[i..end];
        i = end + 1;
        match (name, op) {
            (None, _) => {
                out.extend(pre.expand(body, 0)?);
                out.push(';');
            }
            (Some(n), 0) => {
                if n == "END" {
                    break;
                }
                let cmd = format!("#{n}");
                if !pre.system(&cmd, body)? {
                    out.extend(format!("{cmd}").chars());
                    out.extend(body);
                    out.push(';');
                }
            }
            (Some(n), op) => {
                // Macro ids: ([A-Z])?(-([A-Z])?)? again and again.
                let ids: Vec<char> = n.chars().collect();
                let value = if pre.dynamic { body.to_vec() } else { pre.expand(body, 0)? };
                let mut k = 0;
                loop {
                    let from = k;
                    let first = ids.get(k).copied().filter(|c| c.is_ascii_uppercase());
                    if first.is_some() {
                        k += 1;
                    }
                    let mut dash = false;
                    let mut last = None;
                    if ids.get(k) == Some(&'-') {
                        dash = true;
                        k += 1;
                        if let Some(c) = ids.get(k).copied().filter(|c| c.is_ascii_uppercase()) {
                            last = Some(c);
                            k += 1;
                        }
                    }
                    if k == from {
                        break;
                    }
                    let start = first.map_or(0, |c| (c as u8 - b'A') as usize);
                    let stop = if dash { last.map_or(25, |c| (c as u8 - b'A') as usize) } else { start };
                    for m in start..=stop.min(25) {
                        let v = text_of(&value);
                        if op == 2 {
                            pre.macros[m].push_str(&v);
                        } else {
                            pre.macros[m] = v;
                        }
                    }
                }
            }
        }
        if out.len() > MOST_TEXT {
            return Err("the macros are too long".into());
        }
    }
    Ok((expand_repeats(&out)?, pre))
}

/// `![n ... !| ... !]n`, written out n times.
fn expand_repeats(s: &[char]) -> Result<Vec<char>, String> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < s.len() {
        if starts(s, i, "![") {
            let mut j = i + 2;
            let d1 = j;
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            let digits1 = text_of(&s[d1..j]);
            // (.*?) lazily, then (!\|(.*?))?!\]
            let mut k = j;
            while k <= s.len() {
                let mut hit: Option<(usize, Option<(usize, usize)>)> = None;
                if starts(s, k, "!|") {
                    if let Some(e) = find(s, k + 2, "!]") {
                        hit = Some((e, Some((k + 2, e))));
                    }
                }
                if hit.is_none() && starts(s, k, "!]") {
                    hit = Some((k, None));
                }
                if let Some((e, tail)) = hit {
                    let body = &s[j..k];
                    let mut after = e + 2;
                    let d5 = after;
                    while after < s.len() && s[after].is_ascii_digit() {
                        after += 1;
                    }
                    let digits5 = text_of(&s[d5..after]);
                    let n = if !digits1.is_empty() {
                        as_int(&digits1) - 1
                    } else if !digits5.is_empty() {
                        as_int(&digits5) - 1
                    } else {
                        1
                    }
                    .min(256);
                    let mut unit = body.to_vec();
                    if let Some((a, b)) = tail {
                        unit.extend_from_slice(&s[a..b]);
                    }
                    for _ in 0..n.max(0) {
                        out.extend_from_slice(&unit);
                        if out.len() > MOST_TEXT {
                            return Err("the repeats are too long".into());
                        }
                    }
                    out.extend_from_slice(body);
                    i = after;
                    continue 'outer;
                }
                k += 1;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    Ok(out)
}

/// What the regular expression of `MMLParser` makes of one place.
enum Tok {
    Space,
    System(String),
    Note(char, i64),
    User(&'static str),
    Event(&'static str),
    Table(String),
}

const DEFAULT_EVENTS: &[&str] = &["@q", "@v", "@i", "@o", "&&", "!@ns", "@", "r", "l", "q", "o", "v", "t", "^", "<", ">", "(", ")", "[", "]", "/", "|", "$", "%", "&", "*", ",", ";"];

/// The token at `i` (if any) and where it ends: the alternatives in
/// the order the regular expression tries them.
fn token(s: &[char], i: usize) -> Option<(Tok, usize)> {
    let c = s[i];
    if c.is_whitespace() {
        let mut j = i;
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        return Some((Tok::Space, j));
    }
    if c == '#' {
        let j = find_char(s, i, ';').unwrap_or(s.len());
        return Some((Tok::System(text_of(&s[i..j])), j));
    }
    if ('a'..='g').contains(&c) {
        let mut shift = 0;
        let mut j = i + 1;
        match s.get(j) {
            Some('+' | '#') => {
                shift = 1;
                j += 1;
            }
            Some('-') => {
                shift = -1;
                j += 1;
            }
            _ => {}
        }
        return Some((Tok::Note(c, shift), j));
    }
    // User events, the longest of them (the expression lists them backwards).
    if let Some(u) = USER.iter().filter(|u| starts(s, i, u)).max_by_key(|u| u.len()) {
        return Some((Tok::User(u), i + u.chars().count()));
    }
    // The defaults: @q @v @i @o, &&, !@ns, @, then the single letters.
    if let Some(d) = DEFAULT_EVENTS.iter().find(|d| starts(s, i, d)) {
        return Some((Tok::Event(d), i + d.chars().count()));
    }
    if c == '{' {
        let e = find_char(s, i, '}')?;
        let mut j = e + 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if s.get(j) == Some(&'*') {
            j += 1;
        }
        while j < s.len() && (s[j].is_ascii_digit() || matches!(s[j], '-' | '.')) {
            j += 1;
        }
        if s.get(j) == Some(&'+') {
            j += 1;
        }
        while j < s.len() && (s[j].is_ascii_digit() || matches!(s[j], '-' | '.')) {
            j += 1;
        }
        return Some((Tok::Table(text_of(&s[i..j])), j));
    }
    None
}

/// `MMLParser.parse` over the expanded text.
pub fn parse(text: &str, s: &[char], pre: &mut Pre) -> Result<Vec<Seq>, String> {
    let _ = text;
    let setting = pre.setting.clone();
    let setting = &setting;
    let err = |why: &'static str| why.to_string();
    let mml2nn = 60 - 5 * 12;
    let mut seqs: Vec<Seq> = vec![Seq::default()];
    let mut length = RES / 4;
    let mut octave = 5i64;
    let mut note_shift = 0i64;
    let mut last_is_length = false;
    let mut stack: Vec<usize> = vec![];
    const KEY_SCALE: [i64; 7] = [0, 2, 4, 5, 7, 9, 11];
    let mut i = 0;
    let mut steps = 0usize;
    while i < s.len() {
        steps += 1;
        if steps > MOST_STEPS {
            return Err(err("the MML is too long"));
        }
        let Some((tok, end)) = token(s, i) else {
            i += 1;
            continue;
        };
        // \s*(-?[0-9]*)\s*(\.*)
        let mut j = end;
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        let p0 = j;
        if s.get(j) == Some(&'-') {
            j += 1;
        }
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        let param: Option<i64> = if j > p0 { Some(as_int(&text_of(&s[p0..j]))) } else { None };
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        let mut periods = 0;
        while s.get(j) == Some(&'.') {
            periods += 1;
            j += 1;
        }
        // A system command takes the rest up to the `;`, so no tail.
        i = if matches!(tok, Tok::System(_)) { end } else { j };
        let seq = seqs.last_mut().unwrap();
        // `__calcLength()` at the parse: the length in ticks, or none.
        let len_param = || -> Result<Option<i64>, String> {
            match param {
                None => Ok(None),
                Some(0) => Ok(Some(0)),
                Some(n) => {
                    let l = RES / n;
                    if !(1..=RES).contains(&l) {
                        return Err(err("a length must be 1 to 1920"));
                    }
                    Ok(Some(l))
                }
            }
        };
        // `__calcLength(iLength, period)`.
        let calc = |l: Option<i64>, length: i64| -> i64 {
            let base = l.unwrap_or(length);
            let mut total = base;
            let mut p = periods;
            while p > 0 {
                total += base >> p.min(62);
                p -= 1;
            }
            total
        };
        let push = |seq: &mut Seq, k: K, data: i64, len: i64| {
            if k == K::Rest && seq.ev.last().is_some_and(|e| e.k == K::Rest) {
                seq.ev.last_mut().unwrap().len += len;
            } else {
                seq.ev.push(E { k, data, len, jump: 0 });
                if k == K::Tempo {
                    seq.globals += 1;
                }
            }
        };
        // A note option (`&`, `&&`, `*`): after a note, taking its length.
        let option = |seq: &mut Seq, k: K| -> Result<(), String> {
            match seq.ev.last_mut() {
                Some(e) if e.k == K::Note => {
                    let len = e.len;
                    e.len = 0;
                    seq.ev.push(E { k, data: 0, len, jump: 0 });
                    Ok(())
                }
                _ => Err(err("a * or & needs a note before it")),
            }
        };
        let mut was_length = false;
        match tok {
            Tok::Space => {}
            Tok::Note(c, shift) => {
                let n = c as i64 - 'c' as i64;
                let n = if n < 0 { n + 7 } else { n } as usize;
                let shift = setting.key_sign[n] + shift;
                let len = calc(len_param()?, length);
                let key = (KEY_SCALE[n] + shift + mml2nn + octave * 12 + note_shift).clamp(0, 127);
                push(seq, K::Note, key, len);
            }
            Tok::User(u) => push(seq, K::User(u), param.unwrap_or(NONE), 0),
            Tok::Event(e) => match e {
                "r" => {
                    let len = calc(len_param()?, length);
                    push(seq, K::Rest, 0, len);
                }
                "l" => {
                    length = calc(len_param()?, length);
                    was_length = true;
                }
                "^" => {
                    let len = calc(len_param()?, length);
                    if last_is_length {
                        length += len;
                        was_length = true;
                    } else if let Some(l) = seq.ev.last_mut().filter(|l| matches!(l.k, K::Rest | K::Note)) {
                        l.len += len;
                    } else {
                        return Err(err("a tie needs a note or rest before it"));
                    }
                }
                "o" => {
                    let o = param.unwrap_or(5);
                    if !(0..=9).contains(&o) {
                        return Err(err("o is 0 to 9"));
                    }
                    octave = o;
                }
                "q" => {
                    let q = param.unwrap_or(setting.default_quant_ratio);
                    if !(0..=setting.max_quant_ratio).contains(&q) {
                        return Err(err("q is out of range"));
                    }
                    push(seq, K::Quant, q, 0);
                }
                "@q" => {
                    let q = param.unwrap_or(0);
                    if !(-192..=192).contains(&q) {
                        return Err(err("@q is -192 to 192"));
                    }
                    push(seq, K::QuantCount, q, 0);
                }
                "v" => {
                    let v = param.unwrap_or(256);
                    if !(0..=512).contains(&v) {
                        return Err(err("v is 0 to 512"));
                    }
                    push(seq, K::Volume, v, 0);
                }
                "@v" => {
                    let v = param.unwrap_or(64);
                    if !(0..=128).contains(&v) {
                        return Err(err("@v is 0 to 128"));
                    }
                    push(seq, K::FineVolume, v, 0);
                }
                "%" => push(seq, K::ModType, param.unwrap_or(NONE), 0),
                "@" => push(seq, K::ModParam, param.unwrap_or(NONE), 0),
                "@i" => push(seq, K::InPipe, param.unwrap_or(0), 0),
                "@o" => push(seq, K::OutPipe, param.unwrap_or(0), 0),
                "(" | ")" => {
                    let p = param.unwrap_or(1) * if e == "(" { 1 } else { -1 } * setting.volume_polarization;
                    match seq.ev.last_mut() {
                        Some(l) if matches!(l.k, K::VolumeShift | K::Volume) => l.data += p,
                        _ => push(seq, K::VolumeShift, p, 0),
                    }
                }
                "<" => octave += param.unwrap_or(1) * setting.octave_polarization,
                ">" => octave += -param.unwrap_or(1) * setting.octave_polarization,
                "&" => option(seq, K::Slur)?,
                "&&" => option(seq, K::SlurWeak)?,
                "*" => option(seq, K::Bend)?,
                "," => push(seq, K::Param, param.unwrap_or(NONE), 0),
                ";" => {
                    if !seq.ev.is_empty() {
                        seqs.push(Seq::default());
                        length = RES / 4;
                        octave = 5;
                        note_shift = 0;
                        stack.clear();
                    }
                }
                "$" => push(seq, K::RepAll, 0, 0),
                "[" => {
                    let n = param.unwrap_or(2);
                    if !(1..=65535).contains(&n) {
                        return Err(err("[ is 1 to 65535 times"));
                    }
                    push(seq, K::RepBegin, n, 0);
                    stack.push(seq.ev.len() - 1);
                }
                "|" => {
                    let Some(&b) = stack.last() else { return Err(err("a | is outside a repeat")) };
                    push(seq, K::RepBreak, 0, 0);
                    seq.ev.last_mut().unwrap().jump = b;
                }
                "]" => {
                    let Some(b) = stack.pop() else { return Err(err("a ] has no [")) };
                    push(seq, K::RepEnd, 0, 0);
                    let e = seq.ev.len() - 1;
                    seq.ev[e].jump = b;
                    seq.ev[b].jump = e;
                    if let Some(n) = param {
                        if !(1..=65535).contains(&n) {
                            return Err(err("] is 1 to 65535 times"));
                        }
                        seq.ev[b].data = n;
                    }
                }
                "!@ns" => note_shift += param.unwrap_or(0),
                "t" => push(seq, K::Tempo, param.unwrap_or(120), 0),
                // `/` is a letter of the expression, but not a command.
                _ => return Err(err("it isn't a command")),
            },
            Tok::System(t) => {
                if !seq.ev.is_empty() {
                    return Err(err("a # command belongs at the start of a sequence"));
                }
                let _ = t;
                seq.ev.push(E { k: K::System, data: 0, len: 0, jump: 0 });
            }
            Tok::Table(t) => {
                const ENV_LETTERS: [&str; 11] = ["@@", "na", "np", "nt", "nf", "_@@", "_na", "_np", "_nt", "_nf", "!na"];
                match seq.ev.last_mut() {
                    Some(prev) if matches!(prev.k, K::User(u) if ENV_LETTERS.contains(&u)) => {
                        // \{([^}]*)\}(.*)
                        let chars: Vec<char> = t.chars().collect();
                        let close = find_char(&chars, 1, '}').unwrap_or(chars.len());
                        let dat = text_of(&chars[1..close]);
                        let pfx = text_of(&chars[(close + 1).min(chars.len())..]);
                        let table = parse_table_numbers(&dat, &pfx, 65536).map_err(|e| e)?;
                        prev.data = pre.next_table;
                        pre.envelopes[pre.next_table as usize] = Some(Arc::new(table));
                        pre.next_table -= 1;
                    }
                    _ => return Err(err("a { } table belongs after an envelope command")),
                }
            }
        }
        last_is_length = was_length;
    }
    if !stack.is_empty() {
        return Err(err("a [ has no ]"));
    }
    seqs.retain(|s| !s.ev.is_empty() && s.ev[0].k != K::System);
    Ok(seqs)
}

/// The order a sequence's events run in, repeats and all: `MMLExecutor`.
pub struct Walk {
    pub pos: usize,
    counters: Vec<i64>,
}

impl Walk {
    pub fn new() -> Walk {
        Walk { pos: 0, counters: vec![] }
    }

    /// The next event to do, or none at the end.
    pub fn step(&mut self, ev: &[E]) -> Option<usize> {
        loop {
            let i = self.pos;
            let e = ev.get(i)?;
            self.pos = i + 1;
            match e.k {
                K::RepBegin => self.counters.push(e.data),
                K::RepBreak => {
                    if self.counters.last() == Some(&1) {
                        self.counters.pop();
                        self.pos = ev[e.jump].jump + 1;
                    }
                }
                K::RepEnd => {
                    let Some(c) = self.counters.last_mut() else { continue };
                    *c -= 1;
                    if *c == 0 {
                        self.counters.pop();
                    } else {
                        self.pos = e.jump + 1;
                    }
                }
                _ => return Some(i),
            }
        }
    }
}

/// `_abstructGlobalSequence`: each tempo and the tick it comes at, and
/// which of a sequence's tempo events it took out.
pub fn tempos(seqs: &[Seq]) -> (Vec<(i64, i64)>, Vec<Vec<bool>>) {
    let mut found: Vec<(i64, i64)> = vec![];
    let mut taken = vec![];
    for seq in seqs {
        let mut gone = vec![false; seq.ev.len()];
        let mut count = seq.globals;
        if count > 0 {
            let mut walk = Walk::new();
            let mut pos = 0i64;
            let mut only_rests = true;
            let mut steps = 0usize;
            while count > 0 || only_rests {
                steps += 1;
                if steps > MOST_STEPS {
                    break;
                }
                let Some(i) = walk.step(&seq.ev) else { break };
                let e = &seq.ev[i];
                match e.k {
                    K::Tempo if !gone[i] => {
                        gone[i] = true;
                        found.push((pos, e.data));
                        count = count.saturating_sub(1);
                    }
                    _ if e.len != 0 => {
                        pos += e.len;
                        if e.k != K::Rest {
                            only_rests = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        taken.push(gone);
    }
    (found, taken)
}


/// A compiled song.
pub struct Song {
    pub seqs: Vec<Seq>,
    /// Which tempo events the global sequence took out of each sequence.
    pub taken: Vec<Vec<bool>>,
    /// The global sequence: waits (ticks) and tempo commands, in order.
    pub global: Vec<Global>,
    pub initial_bpm: Option<f64>,
    pub setting: Setting,
    pub title: String,
    pub fps: i32,
    pub envelopes: Vec<Option<Arc<EnvTable>>>,
    pub voices: Vec<Option<Arc<Voice>>>,
    pub waves: Vec<Option<Arc<WaveTable>>>,
    pub effects: Vec<(i64, String, String)>,
    /// The waves of `%7` and `%10`.
    pub samples: Arc<SampleBank>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Global {
    Wait(i64),
    Tempo(i64),
}

impl Song {
    /// `t`'s parameter as beats a minute (`_calcBPMfromTcommand`).
    pub fn bpm_of(&self, p: i64) -> f64 {
        let s = &self.setting;
        match s.tmode {
            0 => p as f64 * s.tres,
            2 => {
                if p != 0 {
                    s.tres / p as f64
                } else {
                    120.0
                }
            }
            _ => {
                if (0..256).contains(&p) {
                    s.tres / (256 - p) as f64
                } else {
                    120.0
                }
            }
        }
    }
}

/// Compiles MML text.
pub fn compile(text: &str, table: Arc<Table>) -> Result<Song, String> {
    compile_with(text, table, Arc::new(SoundReference::new()))
}

/// Compiles MML text with the sounds its `#SAMPLER`, `#PCMWAVE` and `#PCMVOICE` commands name
/// (`SiONDriver.setSoundReferenceTable`).
pub fn compile_with(text: &str, table: Arc<Table>, sounds: Arc<SoundReference>) -> Result<Song, String> {
    let (expanded, mut pre) = precompile_with(text, table, sounds)?;
    let seqs = parse(text, &expanded, &mut pre)?;
    let (mut found, taken) = tempos(&seqs);
    // Sorted by position; those at the start are the initial tempo.
    found.sort_by_key(|f| f.0);
    let mut global = vec![];
    let mut pos = 0;
    let mut initial = None;
    let mut song = Song { seqs: vec![], taken: vec![], global: vec![], initial_bpm: None, setting: pre.setting.clone(), title: String::new(), fps: pre.fps, envelopes: vec![], voices: vec![], waves: vec![], effects: vec![], samples: Arc::new(SampleBank::new()) };
    for &(at, p) in &found {
        if at == 0 {
            initial = Some(song.bpm_of(p));
        } else {
            if at - pos > 0 {
                global.push(Global::Wait(at - pos));
            }
            pos = at;
            global.push(Global::Tempo(p));
        }
    }
    song.seqs = seqs;
    song.taken = taken;
    song.global = global;
    song.initial_bpm = initial;
    song.title = pre.title.lines().next().unwrap_or("").trim().to_string();
    song.envelopes = pre.envelopes;
    song.voices = pre.voices;
    song.waves = pre.waves;
    song.effects = pre.effects;
    song.samples = Arc::new(pre.samples);
    Ok(song)
}

/// MML text parsed on its own (a voice's init sequence): its events.
pub fn parse_fragment(text: &str, setting: &Setting, table: Arc<Table>) -> Result<Vec<E>, String> {
    let mut pre = Pre { macros: Default::default(), dynamic: false, setting: setting.clone(), title: String::new(), envelopes: vec![None; 512], voices: vec![None; 256], waves: vec![None; 128], effects: vec![], fps: 60, next_table: 511, table, sounds: Arc::new(SoundReference::new()), samples: SampleBank::new() };
    let chars: Vec<char> = text.chars().collect();
    let seqs = parse(text, &chars, &mut pre)?;
    Ok(seqs.into_iter().next().map(|s| s.ev).unwrap_or_default())
}
