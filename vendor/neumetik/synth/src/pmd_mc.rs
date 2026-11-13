// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// A port of KAJA's MC (PMD's MML compiler, 4.8q, `mc.asm`, released 2019-12-25 "use it freely"; the source is
// github.com/takamichih/pmdsrc). Reference: the real MC.EXE built from that source and run under DOSBox-X
// (`synth/scripts/pmd-mc-reference.sh`); the port is checked against its `.M` files byte for byte.
//
// Stage 1 (this file so far): the pass-1 scan of the MML (`p1cloop`, mc.asm 440-570) and the `#` directives
// (`macro_set`, mc.asm 2074-2640). Not yet: `!` MML variables and `@` voice definitions (stage 2), the parts
// and their commands (stages 3 and 4), `lc.inc` and the `.M` writer (stage 5). `#Include` needs a reader
// (`compile_with`); `compile` refuses it with `Error::Unsupported`.

mod decompile;
pub use decompile::{decompile, to_mml, Fidelity};

/// MC's error: the character that names the command (`#` for a directive) and the message number, as MC
/// prints them (`error` in mc.asm: `dh`, `dl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Mc { ch: u8, code: u8 },
    Unsupported(&'static str),
    /// A command of MC's that this port does not have yet.
    UnsupportedCmd(u8),
}

const ERR_SYNTAX: u8 = 6; // "#" with no parameter, or no number where one is needed
const ERR_UNKNOWN: u8 = 7; // an unknown `#` directive or parameter

fn err(ch: u8, code: u8) -> Error {
    Error::Mc { ch, code }
}

/// What the directives set. The text ones keep the parameter as MC does, from where it starts to the end
/// of the line (`set_strings`: tabs and escapes allowed, anything below a space ends it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directives {
    pub title: Option<Vec<u8>>,
    pub composer: Option<Vec<u8>>,
    pub arranger: Option<Vec<u8>>,
    /// `#Memo` lines, in order.
    pub memo: Vec<Vec<u8>>,
    pub pcm_file: Option<Vec<u8>>,
    pub pps_file: Option<Vec<u8>>,
    pub ppz_file: Option<Vec<u8>>,
    pub ff_file: Option<Vec<u8>>,
    /// `#Filename`: the name, and whether it was only an extension (`.XYZ`).
    pub filename: Option<(Vec<u8>, bool)>,
    pub zenlen: u8,
    /// `deflng`: the default note length, a quarter of `zenlen`.
    pub deflng: u8,
    pub tempo: u8,
    pub timerb: u8,
    pub transpose: u8,
    pub bend: u8,
    pub loop_def: u8,
    pub jump_flag: u16,
    pub dt2_flg: bool,
    pub ext_detune: bool,
    pub ext_lfo: bool,
    pub ext_env: bool,
    pub pcm_vol_ext: bool,
    pub adpcm_flag: u8,
    /// `#Octave Rev`: `<` goes up and `>` down.
    pub octave_rev: bool,
    /// `#FM3Extend`'s part letters (`fm3_partchr1..3`, 0 = none).
    pub fm3_parts: [u8; 3],
    /// `#PCMExtend`'s part letters (`pcm_partchr1..8`).
    pub pcm_parts: [u8; 8],
    /// `#VolumeDown`, per FM, SSG, PCM, rhythm, PPZ8: the amount (a byte, two's complement) and whether it
    /// is absolute (1) or relative (0).
    pub voldown: [(u8, u8); 5],
    pub opts: Options,
}

/// `#Option` (and the command line): `/v`, `/p`, `/s`, `/m`, `/n`, `/t`, `/l`, `/o`, `/a`, `/c`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Options {
    pub prg_flg: u8,
    pub play_flg: bool,
    pub save_flg: bool,
    pub x68_flg: bool,
    pub towns_flg: bool,
    pub opl_flg: bool,
    pub memo_flg: bool,
    pub pcm_flg: bool,
    pub lc_flag: bool,
}

impl Default for Directives {
    fn default() -> Self {
        Directives {
            title: None,
            composer: None,
            arranger: None,
            memo: Vec::new(),
            pcm_file: None,
            pps_file: None,
            ppz_file: None,
            ff_file: None,
            filename: None,
            zenlen: 96,
            deflng: 24,
            tempo: 0,
            timerb: 0,
            transpose: 0,
            bend: 0,
            loop_def: 0,
            jump_flag: 0,
            dt2_flg: false,
            ext_detune: false,
            ext_lfo: false,
            ext_env: false,
            pcm_vol_ext: false,
            adpcm_flag: 0xff,
            octave_rev: false,
            fm3_parts: [0; 3],
            pcm_parts: [0; 8],
            voldown: [(0, 0); 5],
            opts: Options::default(),
        }
    }
}

/// What pass 1 leaves: the directives, the MML with every `#` line turned into a `;` comment (MC does that
/// in place, `macro_normal_ret`), and where the `!` and `@` definitions are (stage 2 reads them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass1 {
    pub directives: Directives,
    pub mml: Vec<u8>,
    pub variables: Variables,
    /// `voice_buf` after the `@` definitions (zeros where none).
    pub voices: Vec<u8>,
    /// The name MC last left in `prg_name`.
    pub prg_name: [u8; 8],
}

/// A Shift-JIS lead byte as MC's `sjis_check` takes it (`diskpmd.inc`): 0x80..0x9f and 0xe0..0xff.
fn is_sjis_lead(b: u8) -> bool {
    b >= 0x80 && (b.wrapping_sub(0x20) & 0x40) != 0
}

/// `partcheck`: a part letter is `L`..`~` but for `R`.
fn is_part(b: u8) -> bool {
    b >= b'L' && b != b'R' && b < 0x7f
}

struct Scan {
    b: Vec<u8>,
    si: usize,
    /// `skip_flag` bit 1: inside a `` ` `` block.
    skip: bool,
}

/// MC's `!` variables: 256 numbered (`hsbuf2`: where the definition's text starts, 0 = undefined) and 256
/// named (`hsbuf3`: that, and up to 30 bytes of name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variables {
    pub numbered: [u16; 256],
    pub named: Vec<(u16, [u8; 30])>,
}

impl Default for Variables {
    fn default() -> Self {
        Variables { numbered: [0; 256], named: vec![(0, [0; 30]); 256] }
    }
}

/// What `search_hs3` found in the named variables.
#[derive(Debug, PartialEq, Eq)]
pub enum Hs3 {
    /// A variable whose name is the MML's word (`cy` = 0): its index, and the offset after the word.
    Found(usize, usize),
    /// None; the longest variable whose name starts the word (index, if any) and the offset after that part.
    Partial(Option<usize>, usize),
}

/// The voices MC builds: `voice_buf`, 256 voices of 32 bytes (OPN/OPM: four slots in the order 1, 3, 2, 4 with
/// six bytes spread at a stride of four, then algorithm and feedback, then a 7-byte name), or of 16 bytes (OPL:
/// nine bytes and the name).
pub const VOICE_BYTES: usize = 32 * 256;

impl Scan {
    fn at(&self, i: usize) -> u8 {
        self.b.get(i).copied().unwrap_or(0x1a)
    }
    fn lodsb(&mut self) -> u8 {
        let c = self.at(self.si);
        self.si += 1;
        c
    }
    fn peek(&self) -> u8 {
        self.at(self.si)
    }
    /// `numget`: a decimal digit, or `None` and the byte stays.
    fn numget(&mut self) -> Option<u8> {
        let c = self.lodsb();
        if c.is_ascii_digit() {
            Some(c - b'0')
        } else {
            self.si -= 1;
            None
        }
    }
    /// `lngset`: spaces and tabs, an optional `%`, then `$` and up to two hex digits, or decimal digits (16 bits,
    /// wrapping). `Some(bx)` has a number (`al` is its low byte); `None` is MC's carry (no number; `al` = 1).
    fn lngset(&mut self) -> Option<u16> {
        let mut c = self.lodsb();
        while c == b' ' || c == 9 {
            c = self.lodsb();
        }
        if c == b'%' {
            c = self.lodsb();
        }
        if c == b'$' {
            let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
            let a = hex(self.lodsb())?;
            let c2 = self.lodsb();
            return Some(match hex(c2) {
                Some(d) => u16::from(a * 16 + d),
                None => {
                    self.si -= 1;
                    u16::from(a)
                }
            });
        }
        self.si -= 1;
        let mut bx: u16 = self.numget()? as u16;
        while let Some(d) = self.numget() {
            bx = bx.wrapping_mul(10).wrapping_add(u16::from(d));
        }
        Some(bx)
    }
    /// `getnum`: a signed `lngset`; no number reads as 1. Returns (`dl`, `bx`).
    fn getnum(&mut self) -> (u8, u16) {
        let mut c = self.lodsb();
        while c == b' ' || c == 9 {
            c = self.lodsb();
        }
        let neg = c == b'-';
        if c != b'+' && c != b'-' {
            self.si -= 1;
        }
        let bx = self.lngset().unwrap_or(1);
        let dl = bx as u8;
        if neg {
            (dl.wrapping_neg(), bx.wrapping_neg())
        } else {
            (dl, bx)
        }
    }
    /// `move_next_param`: past the directive's word, to the first byte of its parameter. `false` is MC's
    /// carry (the line ended).
    fn move_next_param(&mut self) -> bool {
        loop {
            let c = self.lodsb();
            if c == 9 || c == b' ' {
                break;
            }
            if c < b' ' {
                return false;
            }
        }
        loop {
            let c = self.lodsb();
            if c == 0x1b {
                break;
            }
            if c == 9 || c == b' ' {
                continue;
            }
            if c < b' ' {
                return false;
            }
            break;
        }
        self.si -= 1;
        true
    }
    /// `set_strings`: to the end of the line.
    fn string(&mut self) -> Vec<u8> {
        let mut v = Vec::new();
        loop {
            let c = self.lodsb();
            if c == 9 || c == 0x1b || c >= b' ' {
                v.push(c);
            } else {
                self.si -= 1;
                return v;
            }
        }
    }
    fn up(c: u8) -> u8 {
        c & 0xdf
    }
}

/// Pass 1 of MC: scans `mml` (the file's bytes), running every `#` directive, `!` variable definition and `@`
/// voice definition as it comes. A `` ` `` toggles a block that pass 1 skips; `;` or a control character ends a
/// line's scan.
pub fn pass1(mml: &[u8]) -> Result<Pass1, Error> {
    pass1_with(mml, None)
}

/// Reads an `#Include`d file by the name the MML gives (MC opens it in the current directory, then along its
/// environment's path): its bytes, or `None` when it can't be opened.
pub type IncludeReader<'a> = &'a dyn Fn(&[u8]) -> Option<Vec<u8>>;

/// MC's reading of the main file: trailing zero bytes dropped, a `1A` put at the end, and a CR LF before it if the
/// two bytes before the `1A` aren't one (lines 291-315 of `mc.asm`).
fn end_of_file(mml: &[u8]) -> Vec<u8> {
    let mut b = mml.to_vec();
    while b.last() == Some(&0) {
        b.pop();
    }
    if b.last() != Some(&0x1a) {
        b.push(0x1a);
    }
    let n = b.len();
    if n < 3 || b[n - 3] != b'\r' || b[n - 2] != b'\n' {
        b.truncate(n - 1);
        b.extend_from_slice(b"\r\n\x1a");
    }
    b
}

/// `pass1` with the files `#Include` names read by `inc` (`None`: `#Include` is `Error::Unsupported`).
pub fn pass1_with(mml: &[u8], inc: Option<IncludeReader>) -> Result<Pass1, Error> {
    let buf = end_of_file(mml);
    let mut d = Directives::default();
    let mut v = Variables::default();
    let mut voices = vec![0u8; VOICE_BYTES];
    let mut prg_name = [0u8; 8];
    let mut s = Scan { b: buf, si: 0, skip: false };
    'lines: loop {
        let linehead = s.si;
        loop {
            let c = s.lodsb();
            if is_sjis_lead(c) {
                s.si += 1;
                continue;
            }
            if c == 0x1a {
                break 'lines;
            }
            if c < b' ' + 1 || c == b';' {
                s.line_skip();
                continue 'lines;
            }
            if c == b'`' {
                s.skip = !s.skip;
                continue;
            }
            if s.skip {
                continue;
            }
            match c {
                b'!' => {
                    s.hsset(&mut v)?;
                    s.line_skip();
                    continue 'lines;
                }
                b'@' => {
                    s.nns(&d, &mut voices, &mut prg_name)?;
                    s.line_skip();
                    continue 'lines;
                }
                b'#' => {
                    macro_set(&mut s, &mut d, inc)?;
                    // `macro_normal_ret`: the line's first byte becomes `;`, and the line is skipped.
                    s.b[linehead] = b';';
                    s.line_skip();
                    continue 'lines;
                }
                _ => {}
            }
        }
    }
    Ok(Pass1 { directives: d, mml: s.b, variables: v, voices, prg_name })
}

impl Scan {
    /// `line_skip`: past the next line feed (it always moves at least one byte).
    fn line_skip(&mut self) {
        loop {
            let k = self.lodsb();
            if k == 0x0a || self.si > self.b.len() + 1 {
                break;
            }
        }
    }

    /// `search_hs3`: the named variable the MML's word at `si` names, as MC finds it.
    fn search_hs3(&self, v: &Variables) -> Hs3 {
        let (mut best, mut dx) = (0usize, None);
        'entries: for (idx, (ptr, name)) in v.named.iter().enumerate() {
            if *ptr == 0 {
                continue;
            }
            let mut matched = 0usize;
            for k in 0..30 {
                let m = self.at(self.si + k);
                if m < b'!' {
                    // the MML's word ended
                    if name[k] == 0 {
                        return Hs3::Found(idx, self.si + k);
                    }
                    continue 'entries;
                }
                if name[k] == 0 {
                    // the variable's name ended first: a partial match, the longest kept
                    if best < matched {
                        best = matched;
                        dx = Some(idx);
                    }
                    continue 'entries;
                }
                if m != name[k] {
                    continue 'entries;
                }
                matched += 1;
            }
            // thirty matched: skip the rest of the word
            let mut e = self.si + 30;
            while self.at(e) >= b'!' {
                e += 1;
            }
            return Hs3::Found(idx, e);
        }
        Hs3::Partial(dx, self.si + best)
    }

    /// `hsset`: `!n text` or `!name text`, after the `!`. Numbered variables are 0..255 (the low byte of the
    /// number); a name is kept up to 30 bytes in the first free named slot, or overwrites the one it names.
    fn hsset(&mut self, v: &mut Variables) -> Result<(), Error> {
        enum Slot {
            Num(usize),
            Named(usize),
        }
        let slot = match self.lngset() {
            Some(n) => Slot::Num(n as u8 as usize),
            None => match self.search_hs3(v) {
                Hs3::Found(i, _) => Slot::Named(i),
                Hs3::Partial(..) => {
                    let free = v.named.iter().position(|e| e.0 == 0).ok_or(err(b'!', 33))?;
                    let name = &mut v.named[free].1;
                    let mut n = 0;
                    while n < 30 {
                        let c = self.lodsb();
                        if c < b'!' {
                            break;
                        }
                        name[n] = c;
                        n += 1;
                    }
                    if n == 30 {
                        while self.lodsb() >= b'!' {}
                    }
                    self.si -= 1;
                    Slot::Named(free)
                }
            },
        };
        // The text starts after the first white byte (a line ending first leaves the slot undefined).
        let mut p = self.si;
        loop {
            let c = self.at(p);
            p += 1;
            if c == 0x0d {
                return Ok(());
            }
            if c <= b' ' {
                break;
            }
        }
        match slot {
            Slot::Num(i) => v.numbered[i] = p as u16,
            Slot::Named(i) => v.named[i].0 = p as u16,
        }
        Ok(())
    }

    /// `get_param`: the next number of a voice definition (a name `=text` may be among them; it goes to
    /// `prg_name`). Lines, spaces, tabs, commas and `;` comments between them are skipped.
    fn get_param(&mut self, prg_name: &mut [u8; 8]) -> Result<u8, Error> {
        loop {
            let c = self.lodsb();
            if is_sjis_lead(c) {
                self.si += 1;
                continue;
            }
            if c == 9 || c == b' ' || c == b',' {
                continue;
            }
            if c < b' ' || c == b';' {
                self.line_skip();
                if self.peek() == 0x1a {
                    return Err(err(b'@', 6));
                }
                continue;
            }
            if c == b'`' {
                self.skip = !self.skip;
                continue;
            }
            if self.skip {
                continue;
            }
            if c == b'=' {
                // `get_vname`
                self.si -= 1;
                loop {
                    self.si += 1;
                    if self.peek() != b' ' && self.peek() != 9 {
                        break;
                    }
                }
                let mut n = 0;
                while n < 7 {
                    let k = self.lodsb();
                    if k == 9 || k == 13 {
                        break;
                    }
                    prg_name[n] = k;
                    n += 1;
                }
                prg_name[n] = 0;
                self.line_skip();
                if self.peek() == 0x1a {
                    return Err(err(b'@', 6));
                }
                continue;
            }
            self.si -= 1;
            if c != b'+' && c != b'-' {
                if self.numget().is_none() {
                    return Err(err(b'@', 1));
                }
                self.si -= 1;
            }
            return Ok(self.getnum().0);
        }
    }

    /// One operator's numbers, `slot_get`: ml/dt, tl, ks/ar, ams/dr, dt2/sr, sl/rr.
    fn slot_get(&mut self, dt2_flg: bool, prg_name: &mut [u8; 8]) -> Result<[u8; 6], Error> {
        let mut m = [0u8; 6];
        m[2] = self.get_param(prg_name)? & 0x1f; // AR
        m[3] = self.get_param(prg_name)? & 0x1f; // DR
        m[4] = self.get_param(prg_name)? & 0x1f; // SR
        m[5] = self.get_param(prg_name)? & 0x0f; // RR
        m[5] |= (self.get_param(prg_name)? & 0x0f).rotate_right(4); // SL
        m[1] = self.get_param(prg_name)? & 0x7f; // TL
        m[2] |= (self.get_param(prg_name)? & 3).rotate_right(2); // KS
        m[0] = self.get_param(prg_name)? & 0x0f; // ML
        let mut dt = self.get_param(prg_name)?; // DT
        if dt & 0x80 != 0 {
            dt = (dt.wrapping_neg() & 3) | 4;
        }
        m[0] |= (dt & 7).rotate_right(4);
        if dt2_flg {
            m[4] |= (self.get_param(prg_name)? & 3).rotate_right(2); // DT2
        }
        m[3] |= (self.get_param(prg_name)? & 1).rotate_right(1); // AMS
        Ok(m)
    }

    /// `nns` / `opl_nns`: a voice definition after the `@`.
    fn nns(&mut self, d: &Directives, voices: &mut [u8], prg_name: &mut [u8; 8]) -> Result<(), Error> {
        if d.opts.opl_flg {
            const TABLE: [(u8, u8, u8); 24] = [
                (8, 1, 0), (8, 7, 1),
                (4, 15, 4), (4, 15, 0), (6, 15, 0), (6, 15, 4), (2, 63, 0), (2, 3, 6), (0, 15, 0), (0, 1, 4), (0, 1, 5), (0, 1, 6), (0, 1, 7),
                (5, 15, 4), (5, 15, 0), (7, 15, 0), (7, 15, 4), (3, 63, 0), (3, 3, 6), (1, 15, 0), (1, 1, 4), (1, 1, 5), (1, 1, 6), (1, 1, 7),
            ];
            let num = self.get_param(prg_name)? as usize;
            let mut buf = [0u8; 16];
            for (off, max, rot) in TABLE {
                let a = self.get_param(prg_name)? & max;
                buf[off as usize] |= a.rotate_left(u32::from(rot));
            }
            let at = num * 16;
            voices[at..at + 9].copy_from_slice(&buf[..9]);
            put_name(&mut voices[at + 9..at + 16], prg_name);
            return Ok(());
        }
        let num = self.get_param(prg_name)? as usize;
        let alg = self.get_param(prg_name)? & 7;
        let fb = self.get_param(prg_name)? & 7;
        let alg_fb = (fb << 3) | alg;
        prg_name[0] = 0;
        let mut slots = [[0u8; 6]; 4];
        for k in 0..4 {
            slots[k] = self.slot_get(d.dt2_flg, prg_name)?;
        }
        let at = num * 32;
        // slot order 1, 3, 2, 4 at a stride of four bytes
        for (k, slot) in [0usize, 2, 1, 3].into_iter().enumerate() {
            for i in 0..6 {
                voices[at + k + 4 * i] = slots[slot][i];
            }
        }
        voices[at + 24] = alg_fb;
        put_name(&mut voices[at + 25..at + 32], prg_name);
        Ok(())
    }
}

/// `nns_pname_set`: seven name bytes; once the name's zero is met, zeros.
fn put_name(out: &mut [u8], name: &[u8; 8]) {
    let mut p = 0;
    for o in out.iter_mut() {
        let c = name[p];
        if c != 0 {
            p += 1;
        }
        *o = c;
    }
}

fn macro_set(s: &mut Scan, d: &mut Directives, inc: Option<IncludeReader>) -> Result<(), Error> {
    let bx = s.si; // the directive's first letter
    let al = Scan::up(s.lodsb());
    let ah = Scan::up(s.peek());
    if !s.move_next_param() {
        return Err(err(b'#', ERR_SYNTAX));
    }
    let bad = || err(b'#', ERR_UNKNOWN);
    let syntax = || err(b'#', ERR_SYNTAX);
    let third = Scan::up(s.at(bx + 2));
    let fourth = Scan::up(s.at(bx + 3));
    match al {
        b'P' => match ah {
            // `#PPSFile`, `#PPC`, `#PPZ`
            b'P' => match third {
                b'C' => d.pcm_file = Some(s.string()),
                b'Z' => pcm_extend(s, d, fourth)?,
                b'S' => d.pps_file = Some(s.string()),
                _ => return Err(bad()),
            },
            b'C' => {
                if third != b'M' {
                    return Err(bad());
                }
                match fourth {
                    b'V' => match Scan::up(s.lodsb()) {
                        b'N' => d.pcm_vol_ext = false,
                        b'E' => d.pcm_vol_ext = true,
                        _ => return Err(bad()),
                    },
                    b'E' => pcm_extend(s, d, fourth)?,
                    b'F' => d.pcm_file = Some(s.string()),
                    _ => return Err(bad()),
                }
            }
            _ => return Err(bad()),
        },
        b'T' => {
            if ah == b'E' {
                d.tempo = s.lngset().ok_or_else(syntax)? as u8;
            } else if ah == b'R' {
                d.transpose = s.lngset().ok_or_else(bad)? as u8;
            } else if third == b'M' {
                d.timerb = s.lngset().ok_or_else(syntax)? as u8;
            } else {
                d.title = Some(s.string());
            }
        }
        b'C' => d.composer = Some(s.string()),
        b'A' => {
            if ah == b'D' {
                // `#ADPCM on/off` (MC checks the word and then sets the flag to 1 either way)
                if Scan::up(s.lodsb()) != b'O' {
                    return Err(bad());
                }
                let k = Scan::up(s.lodsb());
                if k != b'N' && k != b'F' {
                    return Err(bad());
                }
                d.adpcm_flag = 1;
            } else {
                d.arranger = Some(s.string());
            }
        }
        b'M' => d.memo.push(s.string()),
        b'Z' => {
            let z = s.lngset().ok_or_else(syntax)? as u8;
            d.zenlen = z;
            d.deflng = z >> 2;
        }
        b'L' => {
            if ah == b'O' {
                d.loop_def = s.lngset().ok_or_else(bad)? as u8;
            } else {
                match Scan::up(s.lodsb()) {
                    b'N' => d.ext_lfo = false,
                    b'E' => d.ext_lfo = true,
                    _ => return Err(bad()),
                }
            }
        }
        b'E' => match Scan::up(s.lodsb()) {
            b'N' => d.ext_env = false,
            b'E' => d.ext_env = true,
            _ => return Err(bad()),
        },
        b'V' => volume_down(s, d)?,
        b'J' => d.jump_flag = s.lngset().ok_or_else(syntax)?,
        b'F' => match ah {
            b'I' => {
                let c = s.lodsb();
                let ext = c == b'.';
                if !ext {
                    s.si -= 1;
                }
                let mut v = Vec::new();
                loop {
                    v.push(s.lodsb());
                    let n = s.peek();
                    if n == b';' || n < b'!' {
                        break;
                    }
                }
                d.filename = Some((v, ext));
            }
            b'F' => d.ff_file = Some(s.string()),
            _ => {
                if s.peek() < b' ' {
                    return Err(syntax());
                }
                let a = s.lodsb();
                if !is_part(a) {
                    return Err(bad());
                }
                d.fm3_parts[0] = a;
                for i in 1..3 {
                    let k = s.lodsb();
                    if !is_part(k) {
                        s.si -= 1;
                        break;
                    }
                    d.fm3_parts[i] = k;
                }
            }
        },
        b'D' => {
            if ah == b'E' {
                match Scan::up(s.lodsb()) {
                    b'N' => d.ext_detune = false,
                    b'E' => d.ext_detune = true,
                    _ => return Err(bad()),
                }
            } else {
                if Scan::up(s.lodsb()) != b'O' {
                    return Err(bad());
                }
                match Scan::up(s.lodsb()) {
                    b'N' => d.dt2_flg = true,
                    b'F' => d.dt2_flg = false,
                    _ => return Err(bad()),
                }
            }
        }
        b'O' => {
            if ah == b'P' {
                get_option(s, &mut d.opts, &mut d.dt2_flg)?;
            } else {
                match Scan::up(s.lodsb()) {
                    b'R' => d.octave_rev = true,
                    b'N' => d.octave_rev = false,
                    _ => return Err(bad()),
                }
            }
        }
        b'B' => d.bend = s.lngset().ok_or_else(syntax)? as u8,
        b'I' => include_set(s, inc)?,
        _ => return Err(bad()),
    }
    Ok(())
}

/// MC's buffer for the `.MML` and whatever `#Include` splices into it: `mml_buf` to `mmlbuf_end`, 61 KB - 1
/// bytes, and one more at `mmlbuf_end`.
const MML_BUF: usize = 61 * 1024 - 1;

/// `include_set`: `#Include name` splices the file into the MML buffer, in place of nothing: after the
/// directive's line come `01 name 00 0A` (a line `line_skip` passes over, the check code that tells `calc_line`
/// an include begins), the file (without its `1A`, with a CR LF added when it lacks one), `02 0A` (the include
/// ends) and the rest of the MML. `si` stays on the directive's CR. MC moves the rest of the MML to the
/// buffer's end first and reads the file into the gap, so "too large" (error 18) is judged from where that
/// rest starts: the file read must fit the gap (one byte less than it filled means it fit), and the file with
/// its CR LF and `02 0A` must too.
fn include_set(s: &mut Scan, inc: Option<IncludeReader>) -> Result<(), Error> {
    let read = inc.ok_or(Error::Unsupported("#Include"))?;
    let name = s.string(); // `si` is on the line's end now (MC's "CR position")
    let cr = s.si;
    let mut out = vec![1u8];
    out.extend_from_slice(&name);
    out.push(0);
    out.push(0x0a);
    let mut data = read(&name).ok_or_else(|| err(b'#', 3))?;
    // The MML as MC holds it: its `1A`, and the CR LF it adds when the last line lacks one (`crlf_set`).
    let mut end = s.b.len();
    while end > 0 && s.b[end - 1] == 0 {
        end -= 1;
    }
    let mut endadr = if end > 0 && s.b[end - 1] == 0x1a { end } else { end + 1 };
    let body = endadr - 1; // where the 1A is
    if !(body >= 2 && s.b[body - 2] == 13 && s.b[body - 1] == 10) {
        endadr += 2;
    }
    let head = cr + 2;
    let moved = endadr.saturating_sub(head);
    let bp = MML_BUF - moved + 1; // where the moved rest starts
    let dx = head + out.len(); // where the file is read to
    if bp < dx || data.len() > bp - dx {
        return Err(err(b'#', 18)); // "the MML is too large"
    }
    if data.last() == Some(&0x1a) {
        data.pop();
    }
    out.extend_from_slice(&data);
    let n = out.len();
    if !(n >= 2 && out[n - 2] == 13 && out[n - 1] == 10) {
        out.extend_from_slice(&[13, 10]);
    }
    out.extend_from_slice(&[2, 0x0a]);
    if head + out.len() > bp {
        return Err(err(b'#', 18));
    }
    let tail = s.b.split_off(head.min(s.b.len()));
    s.b.resize(head, 0x1a);
    s.b.extend_from_slice(&out);
    s.b.extend_from_slice(&tail);
    s.si = cr;
    Ok(())
}

/// `pcmextend_set`: `#PPZFile name`, or `#PCMExtend` and up to eight part letters.
fn pcm_extend(s: &mut Scan, d: &mut Directives, fourth: u8) -> Result<(), Error> {
    if fourth == b'F' {
        d.ppz_file = Some(s.string());
        return Ok(());
    }
    if s.peek() < b' ' {
        return Err(err(b'#', ERR_SYNTAX));
    }
    let a = s.lodsb();
    if !is_part(a) {
        return Err(err(b'#', ERR_UNKNOWN));
    }
    d.pcm_parts[0] = a;
    for i in 1..8 {
        let k = s.lodsb();
        if !is_part(k) {
            s.si -= 1;
            break;
        }
        d.pcm_parts[i] = k;
    }
    Ok(())
}

/// `VolDown_set`: `#VolumeDown F-4,S+2,...`: letters F S P R Z choose the sound sources, then a signed number
/// (without a sign it is absolute); a comma goes on with the next group.
fn volume_down(s: &mut Scan, d: &mut Directives) -> Result<(), Error> {
    loop {
        let (mut bits, mut absolute) = (0u8, 0u8);
        let c = loop {
            let c = s.lodsb();
            if c < b'9' + 1 {
                break c;
            }
            bits |= match Scan::up(c) {
                b'F' => 1,
                b'S' => 2,
                b'P' => 4,
                b'R' => 8,
                b'Z' => 16,
                _ => return Err(err(b'#', 1)),
            };
        };
        s.si -= 1;
        if c != b'+' && c != b'-' {
            absolute = 1;
        }
        if bits == 0 {
            return Err(err(b'#', 1));
        }
        let (dl, _) = s.getnum();
        for i in 0..5 {
            if bits & (1 << i) != 0 {
                d.voldown[i] = (dl, absolute);
            }
        }
        if s.peek() != b',' {
            return Ok(());
        }
        s.si += 1;
    }
}

/// `get_option` for `#Option` (the byte after the directive's word): `/x` or `-x` switches. In a file an
/// unknown switch is an error with code 0 (MC's `error` with `dx` cleared).
fn get_option(s: &mut Scan, o: &mut Options, dt2: &mut bool) -> Result<(), Error> {
    loop {
        let c = s.lodsb();
        if c == b' ' {
            continue;
        }
        if c < b' ' {
            s.si -= 1;
            return Ok(());
        }
        if c != b'/' && c != b'-' {
            return Err(err(0, 0));
        }
        match Scan::up(s.lodsb()) {
            b'V' => {
                if Scan::up(s.peek()) == b'W' {
                    s.si += 1;
                    o.prg_flg |= 2;
                } else {
                    o.prg_flg |= 1;
                }
            }
            b'P' => o.play_flg = true,
            b'S' => {
                o.save_flg = false;
                o.play_flg = true;
            }
            b'M' => {
                o.towns_flg = false;
                o.x68_flg = true;
                o.opl_flg = false;
                *dt2 = true;
            }
            b'N' => {
                o.towns_flg = false;
                o.x68_flg = false;
                o.opl_flg = false;
                *dt2 = false;
            }
            b'T' => {
                o.towns_flg = true;
                o.x68_flg = false;
                o.opl_flg = false;
                *dt2 = false;
            }
            b'L' => o.opl_flg = true,
            b'O' => o.memo_flg = false,
            b'A' => o.pcm_flg = false,
            b'C' => o.lc_flag = true,
            _ => return Err(err(0, 0)),
        }
    }
}


// ---------------------------------------------------------------------------------------------------------
// Stages 3 to 5, a thin slice: the part loop, the notes and the commands `l o < > ^ & t T q Q v V @`, `lc.inc`
// and the `.M` writer. Everything else answers `Error::Unsupported`.
// ---------------------------------------------------------------------------------------------------------

const MAX_PART: usize = 11;
const PCMPART: u8 = 10;
const RHYTHM2: u8 = 11;
const RHYTHM: u8 = 18;
/// `fnumdat`: the 12 notes of block 3 at 32 steps each (MC's pitch table for `I` on FM parts), in MC's own order.
const FNUMDAT: [u16; 384] = [
    0x26a, 0x26b, 0x26c, 0x26e, 0x26f, 0x270, 0x271, 0x272, 0x273, 0x274, 0x276, 0x277,
    0x278, 0x279, 0x27a, 0x27b, 0x27c, 0x27e, 0x27f, 0x280, 0x281, 0x282, 0x283, 0x284,
    0x286, 0x287, 0x288, 0x289, 0x28a, 0x28b, 0x28d, 0x28e, 0x28f, 0x290, 0x291, 0x293,
    0x294, 0x295, 0x296, 0x297, 0x299, 0x29a, 0x29b, 0x29c, 0x29d, 0x29f, 0x2a0, 0x2a1,
    0x2a2, 0x2a3, 0x2a5, 0x2a6, 0x2a7, 0x2a8, 0x2aa, 0x2ab, 0x2ac, 0x2ad, 0x2ae, 0x2b0,
    0x2b1, 0x2b2, 0x2b3, 0x2b5, 0x2b6, 0x2b7, 0x2b8, 0x2ba, 0x2bb, 0x2bc, 0x2bd, 0x2bf,
    0x2c0, 0x2c1, 0x2c3, 0x2c4, 0x2c5, 0x2c6, 0x2c8, 0x2c9, 0x2ca, 0x2cc, 0x2cd, 0x2ce,
    0x2cf, 0x2d1, 0x2d2, 0x2d3, 0x2d5, 0x2d6, 0x2d7, 0x2d9, 0x2da, 0x2db, 0x2dd, 0x2de,
    0x2df, 0x2e1, 0x2e2, 0x2e3, 0x2e5, 0x2e6, 0x2e7, 0x2e9, 0x2ea, 0x2eb, 0x2ed, 0x2ee,
    0x2ef, 0x2f1, 0x2f2, 0x2f3, 0x2f5, 0x2f6, 0x2f8, 0x2f9, 0x2fa, 0x2fc, 0x2fd, 0x2fe,
    0x300, 0x301, 0x303, 0x304, 0x305, 0x307, 0x308, 0x30a, 0x30b, 0x30c, 0x30e, 0x30f,
    0x311, 0x312, 0x313, 0x315, 0x316, 0x318, 0x319, 0x31b, 0x31c, 0x31d, 0x31f, 0x320,
    0x322, 0x323, 0x325, 0x326, 0x328, 0x329, 0x32a, 0x32c, 0x32d, 0x32f, 0x330, 0x332,
    0x333, 0x335, 0x336, 0x338, 0x339, 0x33b, 0x33c, 0x33e, 0x33f, 0x341, 0x342, 0x344,
    0x345, 0x347, 0x348, 0x34a, 0x34b, 0x34d, 0x34f, 0x350, 0x352, 0x353, 0x355, 0x356,
    0x358, 0x359, 0x35b, 0x35c, 0x35e, 0x35f, 0x361, 0x363, 0x364, 0x366, 0x367, 0x369,
    0x36a, 0x36c, 0x36d, 0x36f, 0x371, 0x372, 0x374, 0x375, 0x377, 0x379, 0x37a, 0x37c,
    0x37d, 0x37f, 0x381, 0x382, 0x384, 0x386, 0x387, 0x389, 0x38a, 0x38c, 0x38e, 0x38f,
    0x391, 0x393, 0x394, 0x396, 0x398, 0x399, 0x39b, 0x39d, 0x39e, 0x3a0, 0x3a2, 0x3a3,
    0x3a5, 0x3a7, 0x3a8, 0x3aa, 0x3ac, 0x3ad, 0x3af, 0x3b1, 0x3b3, 0x3b4, 0x3b6, 0x3b8,
    0x3b9, 0x3bb, 0x3bd, 0x3bf, 0x3c0, 0x3c2, 0x3c4, 0x3c6, 0x3c7, 0x3c9, 0x3cb, 0x3cd,
    0x3ce, 0x3d0, 0x3d2, 0x3d4, 0x3d5, 0x3d7, 0x3d9, 0x3db, 0x3dc, 0x3de, 0x3e0, 0x3e2,
    0x3e4, 0x3e5, 0x3e7, 0x3e9, 0x3eb, 0x3ed, 0x3ef, 0x3f0, 0x3f2, 0x3f4, 0x3f6, 0x3f8,
    0x3f9, 0x3fb, 0x3fd, 0x3ff, 0x401, 0x403, 0x405, 0x406, 0x408, 0x40a, 0x40c, 0x40e,
    0x410, 0x412, 0x414, 0x415, 0x417, 0x419, 0x41b, 0x41d, 0x41f, 0x421, 0x423, 0x425,
    0x427, 0x428, 0x42a, 0x42c, 0x42e, 0x430, 0x432, 0x434, 0x436, 0x438, 0x43a, 0x43c,
    0x43e, 0x440, 0x442, 0x444, 0x446, 0x448, 0x44a, 0x44c, 0x44e, 0x450, 0x452, 0x454,
    0x456, 0x458, 0x45a, 0x45c, 0x45e, 0x460, 0x462, 0x464, 0x466, 0x468, 0x46a, 0x46c,
    0x46e, 0x470, 0x472, 0x474, 0x476, 0x478, 0x47a, 0x47c, 0x47e, 0x480, 0x483, 0x485,
    0x487, 0x489, 0x48b, 0x48d, 0x48f, 0x491, 0x493, 0x495, 0x498, 0x49a, 0x49c, 0x49e,
    0x4a0, 0x4a2, 0x4a4, 0x4a6, 0x4a9, 0x4ab, 0x4ad, 0x4af, 0x4b1, 0x4b3, 0x4b6, 0x4b8,
    0x4ba, 0x4bc, 0x4be, 0x4c1, 0x4c3, 0x4c5, 0x4c7, 0x4c9, 0x4cc, 0x4ce, 0x4d0, 0x4d2,
];

const PSG: u8 = 2;
const PCM: u8 = 3;
const PCM_EX: u8 = 4;
const M_SIZE: usize = 63 * 1024 - 1;

/// `fmvol`: the FM carrier level for volumes 0 to 16.
const FMVOL: [u8; 17] = [127 - 0x2a, 127 - 0x28, 127 - 0x25, 127 - 0x22, 127 - 0x20, 127 - 0x1d, 127 - 0x1a, 127 - 0x18, 127 - 0x15, 127 - 0x12, 127 - 0x10, 127 - 0x0d, 127 - 0x0a, 127 - 0x08, 127 - 0x05, 127 - 0x02, 127];

/// What the command table (`comtbl`) holds, so a command MC has but this port doesn't is told from a typo.
const COMMANDS: &[u8] = b"cdefgabrxloC<>tTqQvVR@&D[]:L_)(M*EywP!BIp\\X^=HZ#SW~{}nNFsmOA0123456789%$.-+";

/// Where a command leaves MC's line loop.
enum Flow {
    /// `olc0`: the next command, with the "last was a length" flags cleared.
    Olc0,
    /// `olc02` and `olc03`: the next command.
    Next,
}

/// MC's compile state (`cm_init` and the bytes the commands keep).
struct Compiler {
    s: Scan,
    d: Directives,
    vars: Variables,
    voices: Vec<u8>,
    /// `prg_num`: the voices `@n` used, when `/v` is on.
    prg_num: [u8; 256],
    /// Where the `#FM3Extend` and `#PCMExtend` address words are (after 0xc6 and 0xb4).
    fm3_ofs: Option<usize>,
    pcm_ofs: Option<usize>,
    m: Vec<u8>,
    di: usize,
    part: u8,
    ongen: u8,
    octave: u8,
    deflng: u8,
    zenlen: u8,
    leng: u8,
    calflg: u8,
    prsok: u8,
    skip_flag: u8,
    tie_flag: u8,
    porta_flag: u8,
    length_check1: u8,
    length_check2: u8,
    volss: u8,
    volss2: u8,
    nowvol: u8,
    maxprg: u8,
    octss: u8,
    hsflag: u8,
    lopcnt: u8,
    allloop_flag: u8,
    /// `loptbl` and `lextbl`: where each open `[` and its `:` left their address slots.
    loptbl: [u16; 32],
    lextbl: [u16; 32],
    /// `dh` when an error is raised: the command's character.
    cmd: u8,
    /// `getnum` leaves its sign (0 or 1) in `dh`.
    gn_sign: u8,
    part_at: usize,
    detune: u16,
    master_detune: u16,
    pitch: u16,
    alldet: u16,
    /// `def_a` to `def_g`: the key signature set by `_{ }`.
    def_note: [u8; 7],
    acc_adr: usize,
    /// `lastprg`: the `@n` of an R part line (with bit 15 set), used by its notes.
    lastprg: u16,
    /// `kpart_maxprg` and `deflng_k`: the K part's pattern count and `l` value, for the R part.
    kpart_maxprg: u8,
    deflng_k: u8,
    /// `W` (pseudo echo) and `S` (grace notes): `ge_*`, `ss_*`.
    ge_delay: u8,
    ge_depth: u8,
    ge_depth2: u8,
    ge_flag1: u8,
    ge_flag2: u8,
    ge_tie: u8,
    ge_dep_flag: u8,
    ss_speed: u8,
    ss_depth: u8,
    ss_length: u8,
    ss_tie: u8,
}

fn mc(ch: u8, code: u8) -> Error {
    Error::Mc { ch, code }
}

impl Compiler {
    fn put(&mut self, b: u8) {
        self.m[self.di] = b;
        self.di += 1;
    }
    fn put2(&mut self, lo: u8, hi: u8) {
        self.put(lo);
        self.put(hi);
    }
    fn back(&self, k: usize) -> u8 {
        self.m[self.di - k]
    }
    fn err(&self, code: u8) -> Error {
        mc(self.cmd, code)
    }
    fn unsupported(&self) -> Error {
        Error::Unsupported("a command the port does not have yet")
    }

    /// `cm_init`.
    fn cm_init(&mut self) {
        self.maxprg = 0;
        self.volss = 0;
        self.volss2 = 0;
        self.octss = 0;
        self.skip_flag = 0;
        self.tie_flag = 0;
        self.porta_flag = 0;
        self.prsok = 0;
        self.ss_speed = 0;
        self.ss_depth = 0;
        self.ss_length = 0;
        self.ge_delay = 0;
        self.ge_depth = 0;
        self.ge_depth2 = 0;
        self.detune = 0;
        self.master_detune = 0;
        self.pitch = 0;
        self.alldet = 0x8000;
        self.def_note = [0; 7];
        self.octave = 3;
        self.deflng = self.zenlen >> 2;
    }

    /// `lngset2`: a length. Returns (carry, `bx`). With no number: `deflng`, `calflg` 1 and the carry.
    fn lngset2(&mut self) -> (bool, u16) {
        let mut c = self.s.lodsb();
        while c == b' ' || c == 9 {
            c = self.s.lodsb();
        }
        self.calflg = 0;
        if c == b'%' {
            self.calflg = 1;
            c = self.s.lodsb();
        }
        let none = |me: &mut Compiler| {
            me.calflg = 1;
            me.leng = me.deflng;
            (true, u16::from(me.deflng))
        };
        if c == b'$' {
            let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
            let Some(a) = hex(self.s.lodsb()) else { return none(self) };
            let c2 = self.s.lodsb();
            let v = match hex(c2) {
                Some(d) => a * 16 + d,
                None => {
                    self.s.si -= 1;
                    a
                }
            };
            self.leng = v;
            return (false, u16::from(v));
        }
        self.s.si -= 1;
        let Some(first) = self.s.numget() else { return none(self) };
        let mut bx = u16::from(first);
        while let Some(d) = self.s.numget() {
            bx = bx.wrapping_mul(10).wrapping_add(u16::from(d));
        }
        self.leng = bx as u8;
        (false, bx)
    }

    /// `lngset`: a number; with none, 1 (and the carry).
    fn lngset(&mut self) -> (bool, u16) {
        let (c, bx) = self.lngset2();
        if c {
            self.leng = 1;
            (true, 1)
        } else {
            (false, bx)
        }
    }

    /// `getnum`: signed; returns `dl`.
    fn getnum(&mut self) -> (u8, u16) {
        let mut c = self.s.lodsb();
        while c == b' ' || c == 9 {
            c = self.s.lodsb();
        }
        let neg = c == b'-';
        self.gn_sign = u8::from(neg);
        if c != b'+' && c != b'-' {
            self.s.si -= 1;
        }
        let (_, bx) = self.lngset();
        if neg {
            ((bx as u8).wrapping_neg(), bx.wrapping_neg())
        } else {
            (bx as u8, bx)
        }
    }

    /// `lngcal`: the length number as clocks.
    fn lngcal(&mut self) -> Result<(), Error> {
        if self.leng == 0 {
            return Err(mc(0, 21));
        }
        if self.calflg != 0 {
            return Ok(());
        }
        if self.zenlen % self.leng != 0 {
            return Err(mc(0, 21));
        }
        self.leng = self.zenlen / self.leng;
        Ok(())
    }

    /// `futen`: dots. Returns the carry (a length over 255 was split with ties). `al` is `leng`.
    fn futen(&mut self) -> Result<bool, Error> {
        let mut ax = u16::from(self.leng);
        let mut bx = ax;
        let mut carry = false;
        loop {
            while self.s.peek() == b'.' {
                if bx & 1 != 0 {
                    return Err(mc(b'.', 21));
                }
                bx >>= 1;
                ax += bx;
                self.s.si += 1;
            }
            if ax >> 8 == 0 {
                self.leng = ax as u8;
                return Ok(carry);
            }
            if self.d.opts.towns_flg && false {
                unreachable!();
            }
            // ft1: a length over 255
            if self.ge_delay != 0 || self.ss_length != 0 {
                return Err(mc(b'.', 20));
            }
            if self.part == RHYTHM {
                self.put2(255, 0x0f);
            } else {
                self.put2(255, 0xfb);
                let note = self.back(3);
                self.put(note);
            }
            ax -= 255;
            bx = ax;
            self.prsok |= 2;
            carry = true;
            if ax >> 8 == 0 {
                self.leng = ax as u8;
                return Ok(true);
            }
        }
    }

    /// `futen_skip`.
    fn futen_skip(&mut self) {
        while self.s.lodsb() == b'.' {}
        self.s.si -= 1;
    }

    /// `press`: joins a rest with the rest before it. `al` is `leng`.
    fn press(&mut self) {
        if self.prsok & 0x80 != 0 || self.prsok & 8 != 0 || self.skip_flag != 0 {
            return;
        }
        let mut al = self.leng;
        if self.prsok & 4 != 0 {
            if self.part == RHYTHM {
                self.prs3(&mut al);
                return;
            }
            let ah = self.back(1);
            if ah != self.back(4) {
                return;
            }
            self.di -= 3;
            self.prsok |= 2;
            let (sum, carry) = al.overflowing_add(self.m[self.di]);
            al = sum;
            if carry {
                self.m[self.di] = 255;
                al = al.wrapping_add(1);
                self.di += 3;
                if ah == 0x0f {
                    self.di -= 1;
                    self.m[self.di - 1] = ah;
                }
            }
            self.leng = al;
            return;
        }
        if self.back(1) != 0x0f {
            return;
        }
        if self.prsok & 1 != 0 {
            // restprs
            if self.part == RHYTHM {
                self.prs3(&mut al);
                return;
            }
            if self.back(3) != 0x0f {
                return;
            }
            self.prs3(&mut al);
        }
    }

    fn prs3(&mut self, al: &mut u8) {
        self.di -= 2;
        self.prsok |= 2;
        let (sum, carry) = al.overflowing_add(self.m[self.di]);
        *al = sum;
        if carry {
            self.m[self.di] = 255;
            *al = al.wrapping_add(1);
            self.di += 2;
        }
        self.leng = *al;
    }

    /// `get_clock`: a number, or `l` and a length as a clock count.
    fn get_clock(&mut self) -> Result<u8, Error> {
        if self.s.peek() == b'l' {
            self.s.si += 1;
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(self.cmd, 8));
            }
            self.lngcal()?;
            let carry = self.futen()?;
            if carry {
                return Err(mc(self.cmd, 8));
            }
            Ok(self.leng)
        } else {
            Ok(self.lngset().1 as u8)
        }
    }

    // ----- the line loop (`one_line_compile`, `olc02`, `olc03`) ---------------------------------------------

    /// Compiles the rest of a line. `Ok(true)` is the `/` that ends the part.
    fn olc(&mut self) -> Result<bool, Error> {
        // A line starts at `olc02`: what the last length left in `prsok` carries over from the line before.
        let mut flow = Flow::Next;
        loop {
            if matches!(flow, Flow::Olc0) {
                self.prsok = 0;
            }
            if self.di > M_SIZE - 8 {
                return Err(mc(0, 19));
            }
            let c = self.s.lodsb();
            if is_sjis_lead(c) {
                self.s.si += 1;
                flow = Flow::Next;
                continue;
            }
            if c == 9 || c == b' ' {
                flow = Flow::Next;
                continue;
            }
            if c < b' ' && c != 13 {
                // `notend` with al < " ": handled below as a command byte error unless CR; LF etc. fall to the table
            }
            if c > b' ' && c == b';' {
                self.s.line_skip();
                return Ok(false);
            }
            if c == b'`' {
                self.skip_flag ^= 2;
                flow = Flow::Next;
                continue;
            }
            if self.skip_flag & 2 != 0 {
                // olc_skip2: to the end of the line or the closing backquote
                loop {
                    let k = self.s.lodsb();
                    if is_sjis_lead(k) {
                        self.s.si += 1;
                        continue;
                    }
                    if k == 0x0a {
                        return Ok(false);
                    }
                    if k == b'`' {
                        self.skip_flag &= 0xfd;
                        break;
                    }
                }
                flow = Flow::Next;
                continue;
            }
            if c == b'"' {
                self.skip_flag ^= 1;
                self.put2(0xc0, self.skip_flag & 1);
                flow = Flow::Olc0;
                continue;
            }
            if c == b'\'' {
                self.skip_flag &= 0xfe;
                self.put2(0xc0, 0);
                flow = Flow::Olc0;
                continue;
            }
            if c == b'|' {
                if self.skip_mml() {
                    return Ok(false);
                }
                flow = Flow::Next;
                continue;
            }
            if c == b'/' {
                return Ok(true);
            }
            if c == 13 {
                self.s.si += 1;
                return Ok(false);
            }
            self.cmd = c;
            flow = self.command(c)?;
        }
    }

    /// `|`: skips the rest of the line unless this part is named after it (`|AB cde`; `|!AB` names the parts
    /// that skip). Returns true when the line ends (`olc_fin`), false to go on with the line (`olc03`).
    fn skip_mml(&mut self) -> bool {
        let ah = self.part + b'A' - 1;
        loop {
            let c = self.s.peek();
            if c < b'!' {
                return false;
            }
            let mut found;
            if c == b'!' {
                self.s.si += 1;
                found = true; // reverse: a listed part is NOT compiled
                loop {
                    let k = self.s.lodsb();
                    if k == ah {
                        found = false;
                        break;
                    }
                    if k == 13 {
                        self.s.si += 1;
                        return true;
                    }
                    if k <= b' ' {
                        self.s.si -= 1;
                        break;
                    }
                }
            } else {
                found = false;
                loop {
                    let k = self.s.lodsb();
                    if k == ah {
                        found = true;
                        break;
                    }
                    if k == 13 {
                        self.s.si += 1;
                        return true;
                    }
                    if k <= b' ' {
                        break;
                    }
                }
            }
            if found {
                // part_found
                loop {
                    let k = self.s.lodsb();
                    if k == 13 {
                        self.s.si += 1;
                        return true;
                    }
                    if k <= b' ' {
                        return false;
                    }
                }
            }
            // part_not_found: to the next `|` or the line end
            loop {
                let k = self.s.lodsb();
                if k == 13 {
                    self.s.si += 1;
                    return true;
                }
                if k == b'|' {
                    break;
                }
            }
        }
    }

    fn command(&mut self, c: u8) -> Result<Flow, Error> {
        match c {
            b'c' => self.oto(0, self.def_note[2]),
            b'd' => self.oto(2, self.def_note[3]),
            b'e' => self.oto(4, self.def_note[4]),
            b'f' => self.oto(5, self.def_note[5]),
            b'g' => self.oto(7, self.def_note[6]),
            b'a' => self.oto(9, self.def_note[0]),
            b'b' => self.oto(0x0b, self.def_note[1]),
            b'x' => self.oto_x(0x0c),
            b'r' => self.rest(),
            b'l' => self.lengthset(),
            b'o' => self.octset(),
            b'>' => self.octmove(if self.d.octave_rev { -1 } else { 1 }),
            b'<' => self.octmove(if self.d.octave_rev { 1 } else { -1 }),
            b't' => self.tempoa(),
            b'T' => self.tempob(),
            b'q' => self.qset(),
            b'Q' => self.qset2(),
            b'v' => self.vseta(),
            b'V' => self.vsetb(),
            b'R' | b'@' => self.neirochg(),
            b'W' => self.giji_echo_set(),
            b'S' => self.sousyoku_onp_set(),
            b'\\' => self.rhycom(),
            b'!' => self.hscom(),
            b'A' => {
                self.put2(0xc0, 0xf7);
                let (dl, _) = self.getnum();
                self.put(dl);
                Ok(Flow::Olc0)
            }
            b'O' => self.tl_set(),
            b'E' => self.psgenvset(),
            b'y' => self.ycommand(),
            b'w' => self.psgnoise(),
            b'P' => self.psgpat(),
            b'p' => self.panset(),
            b'B' => {
                let (dl, _) = self.getnum();
                if dl >= 13 {
                    return Err(mc(b'B', 2));
                }
                self.d.bend = dl;
                if dl == 0 {
                    self.alldet = 0x8000;
                }
                Ok(Flow::Next)
            }
            b'I' => {
                let (_, bx) = self.getnum();
                self.pitch = bx;
                Ok(Flow::Next)
            }
            b'D' => self.detset(),
            b'_' => self.oshift(),
            b')' => self.vol_move(true),
            b'(' => self.vol_move(false),
            b'M' => self.lfoset(),
            b'*' => self.lfoswitch(),
            b'm' => self.partmask_set(),
            b's' => self.slotmask_set(),
            b'n' => self.efct_set(0xd4),
            b'N' => self.efct_set(0xd3),
            b'F' => self.fade_set(),
            b'~' => self.status_write(),
            b'Z' => self.syousetu_lng_set(),
            b'C' => self.zenlenset(),
            b'X' => {
                self.d.octave_rev = !self.d.octave_rev;
                Ok(Flow::Next)
            }
            b'H' => self.hardlfo_set(),
            b'#' => self.hardlfo_onoff(),
            b'{' => self.porta_start(),
            b'}' => self.porta_end(),
            b'[' => self.stloop(),
            b']' => self.edloop(),
            b':' => self.extloop(),
            b'L' => self.lopset(),
            b'&' => self.tieset(),
            b'^' => self.lngmul(),
            b'=' => self.lngrew(),
            b'0'..=b'9' | b'%' | b'$' | b'.' => {
                self.s.si -= 1;
                self.lngrew()
            }
            b'-' => self.lng_dec(),
            b'+' => self.tieset_2(),
            _ if COMMANDS.contains(&c) => Err(Error::UnsupportedCmd(c)),
            _ => Err(mc(0, 1)),
        }
    }

    // ----- notes ---------------------------------------------------------------------------------------------

    fn oto(&mut self, mut al: u8, ah: u8) -> Result<Flow, Error> {
        if self.part == RHYTHM2 && !self.d.opts.towns_flg {
            return Err(self.err(17));
        }
        if self.part == RHYTHM {
            return self.rhythm_note();
        }
        let mut bh = self.octave & 0x0f;
        let mut goto_001 = true;
        if self.s.peek() == b'=' {
            self.s.si += 1;
        } else if ah != 0 {
            al = al.wrapping_add(ah);
            goto_001 = false;
        }
        loop {
            if goto_001 {
                if self.s.peek() == b'+' {
                    al = al.wrapping_add(1);
                    self.s.si += 1;
                }
                if self.s.peek() == b'-' {
                    al = al.wrapping_sub(1);
                    self.s.si += 1;
                }
            }
            goto_001 = true;
            al &= 0x0f;
            let mut bl = al;
            if bl == 0x0f {
                bh = bh.wrapping_sub(1);
                if bh == 0xff {
                    return Err(self.err(26));
                }
                bl = 0x0b;
            }
            if bl == 0x0c {
                bh += 1;
                if bh == 8 {
                    return Err(self.err(26));
                }
                bl = 0;
            }
            al = bl;
            if self.s.peek() == b'+' || self.s.peek() == b'-' {
                continue;
            }
            bl |= bh << 4;
            if self.d.bend != 0 {
                self.pitch_set(bl)?;
            }
            return self.bp8(bl);
        }
    }

    /// A note in the R part is `@n c`: the last `@n` of the line is the sound.
    fn rhythm_note(&mut self) -> Result<Flow, Error> {
        if self.lastprg == 0 {
            return Err(self.err(30));
        }
        if self.skip_flag == 0 {
            self.put2((self.lastprg >> 8) as u8, self.lastprg as u8);
            self.length_check1 = 1;
            self.length_check2 = 1;
            self.prsok = 0;
        }
        self.bp9()
    }

    /// `ots002`: with `B` (bend range) on, the detune is written before a note when it changed.
    fn pitch_set(&mut self, note: u8) -> Result<(), Error> {
        let mut bx: u16 = 0;
        if self.pitch != 0 {
            if self.ongen < PSG {
                // fmpt: the pitch as the Fnum difference between the note and the note bent by `pitch`
                let off = i32::from(note & 0x0f) << 5;
                let q = (32 * i32::from(self.d.bend) * i32::from(self.pitch as i16)) / 8192;
                let mut idx = (off + 2 * q) as u16;
                let mut dx = FNUMDAT[(off / 2) as usize];
                loop {
                    if idx & 0x8000 != 0 {
                        idx = idx.wrapping_add(768);
                        dx = dx.wrapping_add(0x26a);
                    } else if idx >= 768 {
                        idx -= 768;
                        dx = dx.wrapping_sub(0x26a);
                    } else {
                        break;
                    }
                }
                bx = dx.wrapping_sub(FNUMDAT[usize::from(idx / 2)]).wrapping_neg();
            } else {
                let mut v = (self.pitch as i16) >> 7;
                if v < 0 {
                    v += 1;
                }
                bx = v as u16;
            }
        }
        bx = bx.wrapping_add(self.detune).wrapping_add(self.master_detune);
        if bx == self.alldet {
            return Ok(());
        }
        if self.porta_flag == 1 {
            if self.back(1) != 0xda {
                return Err(mc(0, 14));
            }
            self.di -= 1;
            self.put(0xfa);
            self.word_out(bx);
            self.put(0xda);
        } else {
            self.put(0xfa);
            self.word_out(bx);
        }
        self.alldet = bx;
        Ok(())
    }

    fn oto_x(&mut self, al: u8) -> Result<Flow, Error> {
        if self.part == RHYTHM2 && !self.d.opts.towns_flg {
            return Err(self.err(17));
        }
        if self.part == RHYTHM {
            return self.rhythm_note();
        }
        self.bp8(al)
    }

    fn rest(&mut self) -> Result<Flow, Error> {
        if self.part == RHYTHM2 && !self.d.opts.towns_flg {
            return Err(self.err(17));
        }
        self.bp8(0x0f)
    }

    /// `bp8`: the note byte `bl`, then its length.
    fn bp8(&mut self, bl: u8) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            if self.acc_adr == self.di {
                self.di -= 2;
                self.acc_adr = 0;
            }
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(0, 8));
            }
            self.futen_skip();
            self.tie_flag = 0;
            return Ok(Flow::Next);
        }
        self.length_check1 = 1;
        self.length_check2 = 1;
        self.put(bl);
        self.bp9()
    }

    fn bp9(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset2();
        if bx >> 8 != 0 {
            return Err(mc(0, 8));
        }
        self.lngcal()?;
        self.prsok &= 0xfd;
        self.futen()?;
        self.press();
        self.put(self.leng);
        self.prsok |= 1;
        self.prsok &= 0xf3;
        if self.back(2) & 0x0f != 0x0f && self.tie_flag == 0 && self.porta_flag == 0 {
            self.ge_flag1 = 0;
            self.ge_flag2 = 0;
            if self.ge_delay != 0 {
                self.ge_set()?;
            } else if self.ss_length != 0 {
                self.ss_set()?;
            }
        }
        self.tie_flag = 0;
        Ok(Flow::Next)
    }

    /// `W`: pseudo echo, `Wdelay[,+-depth][,tie]`.
    fn giji_echo_set(&mut self) -> Result<Flow, Error> {
        let al = self.get_clock()?;
        self.ge_delay = al;
        if al == 0 {
            self.ge_depth = 0;
            self.ge_depth2 = 0;
            if self.s.peek() == b',' {
                self.s.si += 1;
                self.getnum();
                if self.s.peek() == b',' {
                    self.s.si += 1;
                    self.getnum();
                }
            }
            return Ok(Flow::Next);
        }
        self.ge_tie = 0;
        self.ge_dep_flag = 0;
        self.ge_depth = 0xff;
        self.ge_depth2 = 0xff;
        if self.s.peek() != b',' {
            return Ok(Flow::Next);
        }
        self.s.si += 1;
        if self.s.peek() == b'%' {
            self.s.si += 1;
            self.ge_dep_flag = 1;
        }
        let (dl, _) = self.getnum();
        self.ge_depth = dl;
        self.ge_depth2 = dl;
        if self.s.peek() == b',' {
            self.s.si += 1;
            let (_, bx) = self.lngset();
            self.ge_tie = bx as u8;
        }
        Ok(Flow::Next)
    }

    /// `S`: grace notes, `Sspeed[,depth[,tie]]`.
    fn sousyoku_onp_set(&mut self) -> Result<Flow, Error> {
        let al = self.get_clock()?;
        self.ss_speed = al;
        if al == 0 {
            self.ss_depth = 0;
            self.ss_length = 0;
            if self.s.peek() == b',' {
                self.s.si += 1;
                self.lngset();
            }
            return Ok(Flow::Next);
        }
        self.ss_tie = 1;
        self.ss_depth = 0xff;
        if self.s.peek() == b',' {
            self.s.si += 1;
            let (dl, _) = self.getnum();
            self.ss_depth = dl;
            if self.s.peek() == b',' {
                self.s.si += 1;
                let (_, bx) = self.lngset();
                self.ss_tie = bx as u8;
            }
        }
        let mut ah = self.ss_depth;
        if ah & 0x80 != 0 {
            ah = ah.wrapping_neg();
        }
        let mut al = self.ss_speed;
        if ah != 1 {
            let p = u16::from(al) * u16::from(ah);
            if p >> 8 != 0 {
                return Err(mc(b'S', 2));
            }
            al = p as u8;
        }
        self.ss_length = al;
        Ok(Flow::Next)
    }

    /// `ge_set`: the pseudo echo notes after a note.
    fn ge_set(&mut self) -> Result<(), Error> {
        self.ge_depth = self.ge_depth2;
        loop {
            let (r, borrow) = self.back(1).overflowing_sub(self.ge_delay);
            if borrow || r == 0 {
                break;
            }
            let dh = r;
            let dl = self.back(2);
            let i = self.di - 1;
            self.m[i] = self.ge_delay;
            if self.ss_length != 0 && self.ss_length < self.back(1) {
                if self.ge_flag1 != 0 && self.ge_flag1 == self.back(4) && self.ge_flag2 == self.back(3) {
                    let (a, b) = (self.back(2), self.back(1));
                    let d = self.di;
                    self.m[d - 4] = a;
                    self.m[d - 3] = b;
                    self.di -= 2;
                }
                self.ss_set()?;
            }
            if self.ge_tie & 1 != 0 {
                self.put(0xfb);
            }
            self.ge_set_vol();
            self.put2(dl, dh);
            self.prsok |= 2;
            if self.ge_tie & 2 != 0 {
                break;
            }
        }
        if self.ss_length != 0 {
            self.ss_set()?;
        }
        Ok(())
    }

    fn ge_set_vol(&mut self) {
        let depth = self.ge_depth as i8;
        if depth == 0 {
            return;
        }
        let up = depth > 0;
        let mut al = if up { self.ge_depth } else { self.ge_depth.wrapping_neg() };
        if self.ge_dep_flag != 1 {
            al = self.ongen_sel_vol(al);
        }
        if al != 0 {
            let c = if up { 0xde } else { 0xdd };
            self.put2(c, al);
            self.ge_flag1 = c;
            self.ge_flag2 = al;
        }
        let al = self.ge_depth.wrapping_add(self.ge_depth2);
        self.ge_depth = if up {
            if self.ge_dep_flag == 1 {
                if al as i8 >= 0 { al } else { 127 }
            } else if al < 16 {
                al
            } else {
                15
            }
        } else if self.ge_dep_flag == 1 {
            if (al as i8) < 0 { al } else { 0x81 }
        } else if al >= 0xf1 {
            al
        } else {
            0xf1
        };
    }

    /// `ss_set`: the grace notes before a note.
    fn ss_set(&mut self) -> Result<(), Error> {
        if self.ss_length >= self.back(1) {
            return Ok(());
        }
        if self.back(2) == 0x0c {
            return Ok(());
        }
        self.di -= 2;
        let dh = self.m[self.di];
        let mut dl = self.m[self.di + 1];
        let mut bh = dh;
        let depth = self.ss_depth;
        let down_first = depth & 0x80 != 0;
        let count = if down_first { depth.wrapping_neg() } else { depth };
        for _ in 0..count {
            bh = if down_first { Self::one_down(bh)? } else { Self::one_up(bh)? };
        }
        loop {
            if self.ge_flag1 != 0 {
                self.put2(self.ge_flag1, self.ge_flag2);
            }
            self.put2(bh, self.ss_speed);
            if self.ss_tie != 0 {
                self.put(0xfb);
            }
            bh = if down_first { Self::one_up(bh)? } else { Self::one_down(bh)? };
            if bh == dh {
                break;
            }
        }
        if self.ge_flag1 != 0 {
            self.put2(self.ge_flag1, self.ge_flag2);
        }
        self.put(dh);
        dl = dl.wrapping_sub(self.ss_length);
        self.put(dl);
        self.prsok |= 2;
        Ok(())
    }

    fn one_down(mut bh: u8) -> Result<u8, Error> {
        bh = bh.wrapping_sub(1);
        if bh & 0x0f == 0x0f {
            bh = (bh & 0xf0) | 0x0b;
            if bh & 0x80 != 0 {
                return Err(mc(b'S', 26));
            }
        }
        Ok(bh)
    }

    fn one_up(mut bh: u8) -> Result<u8, Error> {
        bh = bh.wrapping_add(1);
        if bh & 0x0f == 0x0c {
            bh = (bh & 0xf0).wrapping_add(0x10);
            if bh & 0x80 != 0 {
                return Err(mc(b'S', 26));
            }
        }
        Ok(bh)
    }

    // ----- lengths -------------------------------------------------------------------------------------------

    fn lengthset(&mut self) -> Result<Flow, Error> {
        let c = self.s.lodsb();
        match c {
            b'=' => return self.lngrew(),
            b'-' => return self.lng_dec(),
            b'+' => return self.tieset_2(),
            b'^' => return self.lngmul(),
            _ => {}
        }
        self.s.si -= 1;
        let (carry, bx) = self.lngset2();
        if carry {
            return Err(mc(b'l', 6));
        }
        if bx >> 8 != 0 {
            return Err(mc(b'l', 8));
        }
        self.lngcal()?;
        if self.futen()? {
            return Err(mc(b'l', 8));
        }
        self.deflng = self.leng;
        Ok(Flow::Next)
    }

    fn lngrew(&mut self) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            return self.lng_skip_ret();
        }
        if self.prsok & 1 == 0 {
            return Err(mc(self.cmd, 16));
        }
        if self.prsok & 2 != 0 {
            return Err(mc(self.cmd, 31));
        }
        self.di -= 1;
        if self.s.peek() == b'.' {
            self.leng = self.m[self.di];
        } else {
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(b'=', 8));
            }
            self.lngcal()?;
        }
        let carry = self.futen()?;
        self.put(self.leng);
        if carry && self.prsok & 8 != 0 {
            return Err(mc(b'^', 8));
        }
        Ok(Flow::Next)
    }

    fn lng_dec(&mut self) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            return self.lng_skip_ret();
        }
        if self.prsok & 1 == 0 {
            return Err(mc(b'=', 16));
        }
        let (_, bx) = self.lngset2();
        if bx >> 8 != 0 {
            return Err(mc(b'-', 8));
        }
        self.lngcal()?;
        if self.futen()? {
            return Err(mc(b'-', 8));
        }
        let al = self.leng;
        if self.back(1) <= al {
            return Err(mc(b'-', 8));
        }
        let i = self.di - 1;
        self.m[i] -= al;
        Ok(Flow::Next)
    }

    fn lng_skip_ret(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset2();
        if bx >> 8 != 0 {
            return Err(mc(b'-', 8));
        }
        self.futen_skip();
        Ok(Flow::Next)
    }

    fn lngmul(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        let al = bx as u8;
        if al == 0 {
            return Err(mc(b'^', 2));
        }
        if self.skip_flag != 0 {
            return Ok(Flow::Next);
        }
        if self.prsok & 1 == 0 {
            return Err(mc(b'^', 16));
        }
        if self.prsok & 2 != 0 {
            return Err(mc(b'^', 31));
        }
        let times = al - 1;
        if times == 0 {
            return Ok(Flow::Next);
        }
        let ah = self.back(1);
        let mut a = ah;
        for _ in 0..times {
            let (sum, carry) = a.overflowing_add(ah);
            a = sum;
            if carry {
                // lm_over
                if self.prsok & 8 != 0 {
                    return Err(mc(b'^', 8));
                }
                a = a.wrapping_add(1);
                let i = self.di - 1;
                self.m[i] = 0xff;
                if self.part == RHYTHM {
                    self.m[self.di] = 0x0f;
                    self.m[self.di + 1] = 0;
                    self.di += 2;
                    self.prsok |= 2;
                    continue;
                }
                self.m[self.di] = 0xfb;
                let note = self.m[self.di - 2];
                self.m[self.di + 1] = note;
                self.m[self.di + 2] = 0;
                self.di += 3;
                self.prsok |= 2;
                // `jnc olc03` after the `mov`s keeps the carry of the last `add`, which was set: the loop goes on
                // (`lnml01`), then ends with the last length written over the final byte.
                continue;
            }
        }
        let i = self.di - 1;
        self.m[i] = a;
        Ok(Flow::Next)
    }

    // ----- ties ----------------------------------------------------------------------------------------------

    fn tieset(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'&' {
            return self.sular();
        }
        self.tieset_2()
    }

    /// `&&`: a slur.
    fn sular(&mut self) -> Result<Flow, Error> {
        if self.part == RHYTHM {
            return Err(mc(b'&', 32));
        }
        self.s.si += 1;
        if self.skip_flag != 0 {
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(b'&', 8));
            }
            self.futen_skip();
            return Ok(Flow::Next);
        }
        self.put(0xc1);
        let back = self.s.si;
        let (carry, bx) = self.lngset2();
        if bx >> 8 != 0 {
            return Err(mc(b'&', 8));
        }
        self.s.si = back;
        if carry {
            return Ok(Flow::Olc0);
        }
        if self.di - 3 < self.part_at {
            // no note before it in this part: MC reads whatever lies before the part (stale memory)
            return Err(Error::Unsupported("&& with no note before it"));
        }
        let bl = self.back(3);
        if self.d.bend != 0 {
            self.pitch_set(bl)?;
        }
        self.bp8(bl)
    }

    fn tieset_2(&mut self) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(b'&', 8));
            }
            self.futen_skip();
            return Ok(Flow::Next);
        }
        let (carry, bx) = self.lngset2();
        if carry {
            // tie_norm
            self.tie_flag = 1;
            if self.part == RHYTHM {
                return Err(mc(b'&', 32));
            }
            self.put(0xfb);
            if self.prsok & 1 == 0 {
                return Ok(Flow::Olc0);
            }
            self.prsok |= 4;
            self.prsok &= 0xfe;
            return Ok(Flow::Next);
        }
        if bx >> 8 != 0 {
            return Err(mc(b']', 8));
        }
        if self.prsok & 1 == 0 {
            return Err(mc(b']', 22));
        }
        self.di -= 1;
        self.lngcal()?;
        if self.futen()? {
            return Err(mc(b'&', 8));
        }
        let al = self.leng;
        let (sum, over) = self.m[self.di].overflowing_add(al);
        if !over {
            self.put(sum);
            self.prsok |= 2;
            return Ok(Flow::Next);
        }
        // tie_lng_over
        self.di += 1;
        if self.prsok & 8 != 0 {
            return Err(mc(b'^', 8));
        }
        if self.part == RHYTHM {
            self.put2(0x0f, al);
            self.prsok = 3;
            return Ok(Flow::Next);
        }
        let ah = self.m[self.di - 2];
        self.m[self.di] = 0xfb;
        self.di += 1;
        self.m[self.di] = ah;
        self.di += 1;
        self.put(al);
        self.prsok = 1;
        Ok(Flow::Next)
    }

    // ----- octave, tempo, gate, volume, voice ------------------------------------------------------------------

    fn octset(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'+' || self.s.peek() == b'-' {
            let (dl, _) = self.getnum();
            self.octss = dl;
            return self.octs0(self.octave.wrapping_add(dl));
        }
        let (carry, bx) = self.lngset();
        if carry {
            return Err(mc(b'o', 6));
        }
        let al = (bx as u8).wrapping_sub(1).wrapping_add(self.octss);
        self.octs0(al)
    }

    fn octmove(&mut self, by: i8) -> Result<Flow, Error> {
        let al = self.octave.wrapping_add(by as u8);
        self.octs0(al)
    }

    fn octs0(&mut self, al: u8) -> Result<Flow, Error> {
        if al >= 8 {
            return Err(self.err(26));
        }
        self.octave = al;
        Ok(Flow::Next)
    }

    fn tempoa(&mut self) -> Result<Flow, Error> {
        let c = self.s.peek();
        if c == b'+' || c == b'-' {
            return self.tempo_ss(0xfd);
        }
        let (_, bx) = self.lngset();
        if (bx as u8) < 18 {
            return Err(mc(b't', 2));
        }
        self.put(0xfc);
        self.put(0xff);
        self.put(bx as u8);
        Ok(Flow::Olc0)
    }

    fn tempob(&mut self) -> Result<Flow, Error> {
        let c = self.s.peek();
        if c == b'+' || c == b'-' {
            return self.tempo_ss(0xfe);
        }
        let (_, bx) = self.lngset();
        let al = bx as u8;
        if al >= 251 {
            return Err(mc(b't', 2));
        }
        self.put2(0xfc, al);
        Ok(Flow::Olc0)
    }

    fn tempo_ss(&mut self, ah: u8) -> Result<Flow, Error> {
        self.put2(0xfc, ah);
        let (dl, _) = self.getnum();
        if dl == 0 {
            self.di -= 2;
            return Ok(Flow::Next);
        }
        self.put(dl);
        Ok(Flow::Olc0)
    }

    fn qset(&mut self) -> Result<Flow, Error> {
        if self.s.peek() != b',' {
            let al = self.get_clock()?;
            self.put2(0xfe, al);
            if self.s.peek() == b'-' {
                self.s.si += 1;
                let b = self.get_clock()?;
                let (mut dh, borrow) = b.overflowing_sub(al);
                if borrow {
                    dh = dh.wrapping_neg() | 0x80;
                }
                if dh != 0 {
                    self.put2(0xb1, dh);
                }
            }
            if self.s.peek() != b',' {
                return Ok(Flow::Olc0);
            }
        }
        self.s.si += 1;
        let al = self.get_clock()?;
        self.put2(0xb3, al);
        Ok(Flow::Olc0)
    }

    fn qset2(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'%' {
            self.s.si += 1;
            let (_, bx) = self.lngset();
            self.put2(0xc4, !(bx as u8));
            return Ok(Flow::Olc0);
        }
        let (_, bx) = self.lngset();
        let mut al = bx as u8;
        if al >= 9 {
            return Err(mc(b'Q', 2));
        }
        al = al.wrapping_add(al);
        if al != 0 {
            al = al.wrapping_mul(16).wrapping_sub(1);
        }
        self.put2(0xc4, !al);
        Ok(Flow::Olc0)
    }

    fn vseta(&mut self) -> Result<Flow, Error> {
        let c = self.s.peek();
        if c == b'+' || c == b'-' {
            let (dl, _) = self.getnum();
            self.volss = dl;
            let dl = self.nowvol;
            if self.part == PCMPART || self.ongen == PCM_EX {
                return self.vsetm1(dl);
            }
            return self.vset2(dl);
        }
        if c == b')' || c == b'(' {
            self.s.si += 1;
            let (dl, _) = self.getnum();
            self.volss2 = if c == b'(' { dl.wrapping_neg() } else { dl };
            return Ok(Flow::Next);
        }
        let (_, bx) = self.lngset();
        let bl = (bx as u8).wrapping_add(self.volss2);
        if bl >= 17 {
            return Err(mc(b'v', 2));
        }
        if self.part == PCMPART || self.ongen == PCM_EX {
            return self.vsetm(bl);
        }
        if self.ongen >= PSG {
            return self.vset2(bl);
        }
        self.vset2(FMVOL[bl as usize])
    }

    fn vsetb(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        let bl = bx as u8;
        if self.part == PCMPART || self.ongen == PCM_EX {
            return self.vsetm1(bl);
        }
        self.vset2(bl)
    }

    fn vset2(&mut self, dl: u8) -> Result<Flow, Error> {
        let (mut al, carry) = self.volss.overflowing_add(dl);
        if carry || al >= 0x80 {
            if self.volss & 0x80 != 0 {
                al = 0;
            } else if self.ongen >= PSG {
                al = 15;
            } else {
                al = 0x7f;
            }
        }
        self.vset4(al)
    }

    fn vset4(&mut self, al: u8) -> Result<Flow, Error> {
        self.nowvol = al;
        self.put2(0xfd, al);
        Ok(Flow::Olc0)
    }

    fn vsetm(&mut self, mut bl: u8) -> Result<Flow, Error> {
        if self.d.pcm_vol_ext {
            let ax = u16::from(bl) * u16::from(bl);
            bl = if ax >= 256 { 255 } else { ax as u8 };
        } else {
            let mut carry = false;
            for _ in 0..4 {
                carry = bl & 0x80 != 0;
                bl <<= 1;
            }
            if carry {
                bl = 255;
            }
        }
        self.vsetm1(bl)
    }

    fn vsetm1(&mut self, bl: u8) -> Result<Flow, Error> {
        let al = self.volss;
        let mut dl = bl;
        if al & 0x80 != 0 {
            let (s, carry) = dl.overflowing_add(al);
            dl = if carry { s } else { 0 };
        } else {
            let (s, carry) = dl.overflowing_add(al);
            dl = if carry { 255 } else { s };
        }
        self.vset4(dl)
    }

    fn neirochg(&mut self) -> Result<Flow, Error> {
        let mut cx = 0u16;
        if self.s.peek() == b'@' {
            self.s.si += 1;
            cx = 128;
        }
        let (_, bx) = self.lngset();
        let bx = bx.wrapping_add(cx);
        if self.d.opts.prg_flg != 0 && self.ongen < PSG {
            // set_prg: the voice is wanted in the .M
            self.prg_num[usize::from(bx as u8)] = 1;
        }
        if self.part == RHYTHM {
            // rhyprg
            if bx >= 0x4000 {
                return Err(mc(b'@', 2));
            }
            self.lastprg = bx | 0x8000;
            return Ok(Flow::Next);
        }
        if self.ongen == PSG {
            // psgprg: the preset envelope `psgenvdat`, four bytes (MC reads past its ten presets, into its
            // own variables, for a number above 9: not ported)
            const PSGENV: [[u8; 4]; 10] = [[0, 0, 0, 0], [2, 255, 0, 1], [2, 254, 0, 1], [2, 254, 0, 8], [2, 255, 24, 1], [2, 254, 24, 1], [2, 254, 4, 1], [2, 1, 0, 1], [1, 2, 0, 1], [1, 2, 24, 1]];
            let idx = usize::from(bx);
            if bx > 9 {
                return Err(Error::Unsupported("@n above 9 in an SSG part (MC reads its own variables)"));
            }
            self.put(0xf0);
            for b in PSGENV[idx] {
                self.put(b);
            }
            return Ok(Flow::Olc0);
        }
        let bl = bx as u8;
        let nb = bl.wrapping_add(1);
        if self.maxprg < nb || (self.maxprg == 0 && nb == 0) {
            self.maxprg = nb;
        }
        if self.part == PCMPART || self.ongen == PCM_EX {
            if self.s.peek() == b',' {
                // repeat_check: `@n,repeat[,stop[,release]]` is 0xff n 0xce, then three words
                self.put2(0xff, bl);
                self.s.si += 1;
                let (_, bx) = self.getnum();
                self.put(0xce);
                self.put2(bx as u8, (bx >> 8) as u8);
                let mut words = [0u16, 0x8000];
                if self.s.peek() == b',' {
                    self.s.si += 1;
                    words[0] = self.getnum().1;
                    if self.s.peek() == b',' {
                        self.s.si += 1;
                        words[1] = self.getnum().1;
                    }
                }
                for w in words {
                    self.put2(w as u8, (w >> 8) as u8);
                }
                return Ok(Flow::Olc0);
            }
        } else if self.part == RHYTHM2 {
            if !self.d.opts.towns_flg {
                if self.skip_flag != 0 {
                    return Ok(Flow::Olc0);
                }
                self.put(bl);
                self.length_check1 = 1;
                self.length_check2 = 1;
                return Ok(Flow::Olc0);
            }
        }
        self.put2(0xff, bl);
        Ok(Flow::Olc0)
    }

    /// `\\`: the rhythm source's commands.
    fn rhycom(&mut self) -> Result<Flow, Error> {
        let al = self.s.lodsb();
        let e = |code: u8| mc(b'\\', code);
        match al {
            b'V' => {
                if self.s.peek() == b'+' || self.s.peek() == b'-' {
                    let (dl, _) = self.getnum();
                    self.put2(0xe6, dl);
                    return Ok(Flow::Olc0);
                }
                self.put(0xe8);
                let (carry, bx) = self.lngset();
                if carry {
                    return Err(e(6));
                }
                if bx as u8 >= 64 {
                    return Err(e(2));
                }
                self.put(bx as u8);
            }
            b'v' => {
                let al = self.rhysel()?;
                if self.s.peek() == b'+' || self.s.peek() == b'-' {
                    self.put2(0xe5, (al & 0xe0).rotate_left(3));
                    let (dl, _) = self.getnum();
                    self.put(dl);
                } else {
                    let (carry, bx) = self.lngset();
                    if carry {
                        return Err(e(6));
                    }
                    if bx as u8 >= 32 {
                        return Err(e(2));
                    }
                    self.put(0xea);
                    self.put((al & 0xe0) | (bx as u8 & 0x1f));
                }
            }
            b'l' | b'm' | b'r' => {
                let bl = match al {
                    b'l' => 2,
                    b'm' => 3,
                    _ => 1,
                };
                self.put(0xe9);
                let al = self.rhysel()?;
                self.put((al & 0xe0) | bl);
            }
            b'b' | b's' | b'c' | b'h' | b't' | b'i' => {
                let mut al = match al {
                    b'b' => 1,
                    b's' => 2,
                    b'c' => 4,
                    b'h' => 8,
                    b't' => 16,
                    _ => 32,
                };
                if self.skip_flag != 0 {
                    if self.s.peek() == b'p' {
                        self.s.si += 1;
                    }
                    return Ok(Flow::Next);
                }
                if self.s.peek() == b'p' {
                    self.s.si += 1;
                    al |= 0x80;
                } else if self.back(2) == 0xeb && self.back(1) & 0x80 == 0 && self.prsok == 0x80 {
                    // the same bar's earlier drums: one byte with all their bits
                    let i = self.di - 1;
                    self.m[i] |= al;
                    self.prsok = 0x80;
                    return Ok(Flow::Next);
                }
                self.put2(0xeb, al);
                if al & 0x80 == 0 {
                    self.prsok = 0x80;
                    return Ok(Flow::Next);
                }
            }
            _ => return Err(e(1)),
        }
        Ok(Flow::Olc0)
    }

    /// `rhysel`: the drum letter after `\\v`, `\\l`, `\\m`, `\\r`, as the top three bits.
    fn rhysel(&mut self) -> Result<u8, Error> {
        let c = self.s.lodsb();
        let n: u8 = match c {
            b'b' => 1,
            b's' => 2,
            b'c' => 3,
            b'h' => 4,
            b't' => 5,
            b'i' => 6,
            _ => return Err(mc(b'\\', 1)),
        };
        Ok(n << 5)
    }

    fn tl_set(&mut self) -> Result<Flow, Error> {
        if self.part == RHYTHM {
            return Err(mc(b'O', 17));
        }
        self.put(0xb8);
        let (_, bx) = self.getnum();
        if bx as u8 >= 16 {
            return Err(mc(b'O', 6));
        }
        self.put(bx as u8);
        if self.s.lodsb() != b',' {
            return Err(mc(b'O', 6));
        }
        if self.s.peek() == b'+' || self.s.peek() == b'-' {
            let at = self.di - 1;
            self.m[at] |= 0xf0;
        }
        let (_, bx) = self.getnum();
        self.put(bx as u8);
        Ok(Flow::Olc0)
    }

    /// `!`: the text of an MML variable is compiled where it is called.
    fn hscom(&mut self) -> Result<Flow, Error> {
        let (carry, bx) = self.lngset();
        let ptr;
        if !carry {
            if bx >> 8 != 0 {
                return Err(mc(b'!', 2));
            }
            ptr = self.vars.numbered[usize::from(bx as u8)];
        } else {
            if self.s.peek() < b'!' {
                return Err(mc(b'!', 6));
            }
            match self.s.search_hs3(&self.vars) {
                Hs3::Found(i, si) => {
                    self.s.si = si;
                    ptr = self.vars.named[i].0;
                }
                Hs3::Partial(Some(i), si) => {
                    self.s.si = si;
                    ptr = self.vars.named[i].0;
                }
                Hs3::Partial(None, _) => return Err(mc(b'!', 27)),
            }
        }
        if ptr == 0 {
            return Err(mc(b'!', 27));
        }
        if self.hsflag >= 40 {
            return Err(Error::Unsupported("variables nested more than 40 deep (MC would run out of stack)"));
        }
        let back = self.s.si;
        self.hsflag += 1;
        self.s.si = usize::from(ptr);
        let r = self.olc();
        self.hsflag -= 1;
        self.s.si = back;
        r?;
        Ok(Flow::Next)
    }

    // ----- SSG and register commands -------------------------------------------------------------------------

    fn psgenvset(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'X' {
            self.s.si += 1;
            self.put(0xc9);
            let (dl, _) = self.getnum();
            self.put(dl);
            return Ok(Flow::Olc0);
        }
        self.put(0xf0);
        for _ in 0..3 {
            let (dl, _) = self.getnum();
            self.put(dl);
            if self.s.lodsb() != b',' {
                return Err(mc(b'E', 6));
            }
        }
        let (dl, _) = self.getnum();
        self.put(dl);
        if self.s.peek() == b',' {
            self.m[self.di - 5] = 0xcd;
            self.s.si += 1;
            let (dl, _) = self.getnum();
            let at = self.di - 1;
            self.m[at] = (self.m[at] & 0x0f) | (dl.rotate_left(4) & 0xf0);
            let mut dl = 0;
            if self.s.peek() == b',' {
                self.s.si += 1;
                dl = self.getnum().0;
            }
            self.put(dl);
        }
        Ok(Flow::Olc0)
    }

    fn ycommand(&mut self) -> Result<Flow, Error> {
        self.put(0xef);
        let (_, bx) = self.lngset();
        self.put(bx as u8);
        if self.s.lodsb() != b',' {
            return Err(mc(b'y', 6));
        }
        let (_, bx) = self.lngset();
        self.put(bx as u8);
        Ok(Flow::Olc0)
    }

    fn psgnoise(&mut self) -> Result<Flow, Error> {
        let c = self.s.peek();
        if c == b'+' || c == b'-' {
            self.put(0xd0);
            let (dl, _) = self.getnum();
            self.put(dl);
        } else {
            self.put(0xee);
            let (_, bx) = self.lngset();
            self.put(bx as u8);
        }
        Ok(Flow::Olc0)
    }

    fn psgpat(&mut self) -> Result<Flow, Error> {
        self.put(0xed);
        let (carry, bx) = self.lngset();
        if carry {
            return Err(mc(b'P', 6));
        }
        let mut al = bx as u8;
        if al >= 4 {
            return Err(mc(b'P', 2));
        }
        let ah = (al & 2) << 2;
        al = (al & 1) | ah;
        al |= al.rotate_left(1);
        al |= al.rotate_left(1);
        self.put(al);
        Ok(Flow::Olc0)
    }

    fn panset(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'x' {
            self.s.si += 1;
            let (dl, _) = self.getnum();
            self.put(0xc3);
            self.put(dl);
            self.put(0);
            if self.s.peek() == b',' {
                self.s.si += 1;
                let (dl, _) = self.getnum();
                if dl != 0 {
                    let at = self.di - 1;
                    self.m[at] = 1;
                }
            }
            return Ok(Flow::Olc0);
        }
        let (_, bx) = self.getnum();
        if bx >= 4 {
            return Err(mc(b'p', 2));
        }
        self.put2(0xec, bx as u8);
        Ok(Flow::Olc0)
    }

    // ----- detune, transposition, volume steps, LFO -------------------------------------------------------------

    fn word_out(&mut self, bx: u16) {
        self.put2(bx as u8, (bx >> 8) as u8);
    }

    fn detset(&mut self) -> Result<Flow, Error> {
        let c = self.s.lodsb();
        match c {
            b'D' => {
                let (_, bx) = self.getnum();
                self.put(0xd5);
                self.word_out(bx);
                return Ok(Flow::Olc0);
            }
            b'X' => {
                self.put(0xcc);
                let (dl, _) = self.getnum();
                self.put(dl);
                return Ok(Flow::Olc0);
            }
            b'M' => {
                let (_, bx) = self.getnum();
                self.master_detune = bx;
                return self.detset_exit(bx.wrapping_add(self.detune));
            }
            b'F' | b'S' | b'P' | b'R' | b'Z' => {
                let mut al = match c {
                    b'F' => 0xfe,
                    b'S' => 0xfc,
                    b'P' => 0xfa,
                    b'R' => 0xf8,
                    _ => 0xf5,
                };
                if self.s.peek() != b'+' && self.s.peek() != b'-' {
                    al += 1;
                }
                self.put2(0xc0, al);
                let (dl, _) = self.getnum();
                self.put(dl);
                return Ok(Flow::Olc0);
            }
            _ => {}
        }
        self.s.si -= 1;
        let (_, bx) = self.getnum();
        self.detune = bx;
        self.detset_exit(bx.wrapping_add(self.master_detune))
    }

    fn detset_exit(&mut self, bx: u16) -> Result<Flow, Error> {
        if self.d.bend != 0 {
            return Ok(Flow::Next);
        }
        self.put(0xfa);
        self.word_out(bx);
        Ok(Flow::Olc0)
    }

    fn oshift(&mut self) -> Result<Flow, Error> {
        match self.s.peek() {
            b'M' => {
                self.s.si += 1;
                let (dl, _) = self.getnum();
                self.put2(0xb2, dl);
                return Ok(Flow::Olc0);
            }
            b'{' => {
                self.s.si += 1;
                let c = self.s.lodsb();
                let ah: u8 = match c {
                    b'=' => 0,
                    b'+' => 1,
                    b'-' => 0xff,
                    _ => return Err(mc(b'{', 2)),
                };
                loop {
                    let k = self.s.lodsb();
                    if k == b'}' {
                        return Ok(Flow::Next);
                    }
                    if k == b' ' || k == 9 || k == b',' {
                        continue;
                    }
                    let n = k.wrapping_sub(b'a');
                    if k < b'a' || n >= 7 {
                        if c == b'-' {
                            return Err(mc(b'{', 2));
                        }
                        return Err(Error::Unsupported("_{ with a bad letter (MC's error code is stale there)"));
                    }
                    self.def_note[usize::from(n)] = ah;
                }
            }
            b'_' => {
                self.s.si += 1;
                let (dl, _) = self.getnum();
                self.put2(0xe7, dl);
                return Ok(Flow::Olc0);
            }
            _ => {}
        }
        let (dl, _) = self.getnum();
        self.put2(0xf5, dl);
        Ok(Flow::Olc0)
    }

    /// `ongen_sel_vol`.
    fn ongen_sel_vol(&self, al: u8) -> u8 {
        if self.part == PCMPART || self.ongen == PCM_EX {
            return al.wrapping_shl(4);
        }
        if self.ongen == PSG {
            return al;
        }
        al.wrapping_shl(2)
    }

    /// `)` (up) and `(` (down).
    fn vol_move(&mut self, up: bool) -> Result<Flow, Error> {
        let (one, plain, rel) = if up { (0xf4, 0xe3, 0xde) } else { (0xf3, 0xe2, 0xdd) };
        match self.s.peek() {
            b'%' => {
                self.s.si += 1;
                let (_, bx) = self.lngset();
                self.put2(plain, bx as u8);
                Ok(Flow::Olc0)
            }
            b'^' => {
                self.s.si += 1;
                let al;
                if self.s.peek() == b'%' {
                    self.s.si += 1;
                    al = self.lngset().1 as u8;
                } else {
                    let v = self.lngset().1 as u8;
                    al = self.ongen_sel_vol(v);
                }
                if al == 0 {
                    return Ok(Flow::Next);
                }
                self.put2(rel, al);
                if self.skip_flag != 0 {
                    self.acc_adr = self.di;
                }
                Ok(Flow::Olc0)
            }
            _ => {
                let (_, bx) = self.lngset();
                if bx as u8 == 1 {
                    self.put(one);
                    return Ok(Flow::Olc0);
                }
                self.put(plain);
                let al = self.ongen_sel_vol(bx as u8);
                self.put(al);
                Ok(Flow::Olc0)
            }
        }
    }

    /// `M`: LFO.
    fn lfoset(&mut self) -> Result<Flow, Error> {
        if self.part == RHYTHM {
            return Err(mc(b'M', 17));
        }
        let c = self.s.lodsb();
        // the letter after the command: B (the second LFO), A (the first), or none
        let marker = |me: &mut Compiler, b_code: u8, a_code: u8| -> u8 {
            let k = me.s.lodsb();
            if k == b'B' {
                b_code
            } else if k == b'A' {
                a_code
            } else {
                me.s.si -= 1;
                a_code
            }
        };
        match c {
            b'X' => {
                let ah = marker(self, 0xbb, 0xca);
                self.put(ah);
                let (dl, _) = self.getnum();
                self.put(dl);
                return Ok(Flow::Olc0);
            }
            b'P' => return self.portaset(),
            b'D' => return self.depthset(),
            b'W' => {
                let ah = marker(self, 0xbc, 0xcb);
                let (dl, _) = self.getnum();
                self.put2(ah, dl);
                return Ok(Flow::Olc0);
            }
            b'M' => {
                let ah = marker(self, 0xba, 0xc5);
                let (dl, _) = self.getnum();
                self.put2(ah, dl);
                return Ok(Flow::Olc0);
            }
            _ => {}
        }
        let ah = match c {
            b'B' => 0xbf,
            b'A' => 0xf2,
            _ => {
                self.s.si -= 1;
                0xf2
            }
        };
        self.put(ah);
        self.cmd = b'M';
        let al = self.get_clock()?;
        self.put(al);
        if self.s.lodsb() != b',' {
            self.s.si -= 1;
            let at = self.di - 2;
            self.m[at] = if ah == 0xf2 { 0xc2 } else { 0xb9 };
            return Ok(Flow::Olc0);
        }
        for last in [false, false, true] {
            let (dl, _) = self.getnum();
            self.put(dl);
            if last {
                break;
            }
            if self.s.lodsb() != b',' {
                return Err(mc(b'M', 6));
            }
        }
        Ok(Flow::Olc0)
    }

    fn portaset(&mut self) -> Result<Flow, Error> {
        let k = self.s.lodsb();
        let ah = match k {
            b'B' => 0xbf,
            b'A' => 0xf2,
            _ => {
                self.s.si -= 1;
                0xf2
            }
        };
        let (mut bend2, mut bend3) = (0u8, 1u8);
        self.put(ah);
        let (dl, _) = self.getnum();
        let bend1 = dl;
        if self.s.peek() == b',' {
            self.s.si += 1;
            bend2 = self.get_clock()?;
            if self.s.peek() == b',' {
                self.s.si += 1;
                bend3 = self.getnum().0;
            }
        }
        self.put(bend2);
        self.put(bend3);
        self.put(bend1);
        self.put(255);
        self.put2(ah - 1, 1);
        Ok(Flow::Olc0)
    }

    fn depthset(&mut self) -> Result<Flow, Error> {
        let k = self.s.lodsb();
        let ah = match k {
            b'B' => 0xbd,
            b'A' => 0xd6,
            _ => {
                self.s.si -= 1;
                0xd6
            }
        };
        self.put(ah);
        let (dl, _) = self.getnum();
        self.put(dl);
        if dl == 0 {
            let mut dl = 0;
            if self.s.peek() == b',' {
                self.s.si += 1;
                dl = self.getnum().0;
            }
            self.put(dl);
            return Ok(Flow::Olc0);
        }
        if self.s.peek() != b',' {
            return Err(mc(b'M', 6));
        }
        self.s.si += 1;
        let (dl, _) = self.getnum();
        self.put(dl);
        if self.s.peek() != b',' {
            return Ok(Flow::Olc0);
        }
        self.s.si += 1;
        self.put(0xb7);
        let (_, bx) = self.getnum();
        let mut bl = bx as u8;
        if bl & 0x80 != 0 {
            return Err(mc(b'M', 2));
        }
        if ah != 0xd6 {
            bl |= 0x80;
        }
        self.put(bl);
        Ok(Flow::Olc0)
    }

    /// `*`: LFO switch, `*A1` or `*A1,B1` (the second letter's pair is written even when it repeats the first's command).
    fn lfoswitch(&mut self) -> Result<Flow, Error> {
        let k = self.s.lodsb();
        let mut ah = 0xf1;
        if k == b'B' {
            if self.part != RHYTHM {
                ah = 0xbe;
            }
        } else if k != b'A' {
            self.s.si -= 1;
        }
        let (carry, bx) = self.lngset();
        if carry {
            return Err(mc(b'*', 6));
        }
        self.put2(ah, bx as u8);
        if self.s.peek() != b',' {
            return Ok(Flow::Olc0);
        }
        self.s.si += 1;
        let k = self.s.lodsb();
        if self.part == RHYTHM {
            return Err(mc(b'*', 17));
        }
        let mut ah = 0xbe;
        if k == b'A' {
            ah = 0xf1;
        } else if k != b'B' {
            self.s.si -= 1;
        }
        let bh = ah;
        let (carry, bx) = self.lngset();
        if carry {
            return Err(mc(b'*', 6));
        }
        self.put2(bh, bx as u8);
        Ok(Flow::Olc0)
    }

    // ----- small commands ------------------------------------------------------------------------------------

    fn partmask_set(&mut self) -> Result<Flow, Error> {
        let (carry, bx) = self.lngset();
        if carry {
            return Err(mc(b'm', 6));
        }
        if bx as u8 >= 2 {
            return Err(mc(b'm', 2));
        }
        self.put2(0xc0, bx as u8);
        Ok(Flow::Olc0)
    }

    fn slotmask_set(&mut self) -> Result<Flow, Error> {
        match self.s.peek() {
            b'd' => {
                self.s.si += 1;
                let mut al = 0xc8;
                if self.s.peek() == b'd' {
                    al = 0xc7;
                    self.s.si += 1;
                }
                self.put(al);
                let (dl, _) = self.getnum();
                self.put(dl);
                if self.s.peek() != b',' {
                    return Err(mc(b's', 6));
                }
                self.s.si += 1;
                let (_, bx) = self.getnum();
                self.put2(bx as u8, (bx >> 8) as u8);
                return Ok(Flow::Olc0);
            }
            b'k' => {
                self.s.si += 1;
                self.put(0xb5);
                let (dl, _) = self.getnum();
                self.put(dl);
                if self.s.peek() != b',' {
                    if dl != 0 {
                        return Err(mc(b's', 6));
                    }
                    self.put(dl);
                    return Ok(Flow::Olc0);
                }
                self.s.si += 1;
                let al = self.get_clock()?;
                self.put(al);
                return Ok(Flow::Olc0);
            }
            _ => {}
        }
        let (_, bx) = self.lngset();
        self.put(0xcf);
        let mut ah = bx as u8;
        let mut al = 0u8;
        if self.s.peek() == b',' {
            self.s.si += 1;
            al = self.lngset().1 as u8;
        }
        ah = ah.rotate_left(4) & 0xf0;
        al = (al & 0x0f) | ah;
        self.put(al);
        Ok(Flow::Olc0)
    }

    fn efct_set(&mut self, code: u8) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        if self.skip_flag != 0 {
            return Ok(Flow::Next);
        }
        self.put2(code, bx as u8);
        Ok(Flow::Olc0)
    }

    fn fade_set(&mut self) -> Result<Flow, Error> {
        if self.s.peek() == b'B' {
            self.s.si += 1;
            self.put(0xb6);
            let c = self.s.peek();
            if c == b'+' || c == b'-' {
                let (dl, bx) = self.getnum();
                if dl.wrapping_add(7) >= 15 {
                    return Err(mc(b'F', 2));
                }
                self.put(bx as u8 | 0x80);
            } else {
                let (_, bx) = self.getnum();
                if bx as u8 >= 8 {
                    return Err(mc(b'F', 2));
                }
                self.put(bx as u8);
            }
            return Ok(Flow::Olc0);
        }
        let (_, bx) = self.lngset();
        self.put2(0xd2, bx as u8);
        Ok(Flow::Olc0)
    }

    fn status_write(&mut self) -> Result<Flow, Error> {
        let c = self.s.peek();
        if c == b'-' || c == b'+' {
            let (dl, _) = self.getnum();
            self.put2(0xdb, dl);
        } else {
            let (_, bx) = self.lngset();
            self.put2(0xdc, bx as u8);
        }
        Ok(Flow::Olc0)
    }

    fn syousetu_lng_set(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        self.put2(0xdf, bx as u8);
        Ok(Flow::Olc0)
    }

    fn zenlenset(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        let al = bx as u8;
        self.zenlen = al;
        self.deflng = al >> 2;
        self.put2(0xdf, al);
        Ok(Flow::Olc0)
    }

    fn hardlfo_set(&mut self) -> Result<Flow, Error> {
        let (_, bx) = self.lngset();
        let pms = bx as u8;
        if pms >= 8 {
            return Err(mc(b'H', 2));
        }
        let (bl, al);
        if self.s.peek() == b',' {
            self.s.si += 1;
            let (_, b2) = self.lngset();
            if b2 as u8 >= 4 {
                return Err(mc(b'H', 2));
            }
            bl = pms;
            al = b2 as u8;
        } else {
            bl = pms;
            al = 0;
        }
        let v = (al.rotate_left(4) | bl) & 0x37;
        self.put2(0xe1, v);
        if self.s.lodsb() == b',' {
            let al = self.get_clock()?;
            self.put2(0xe4, al);
        } else {
            self.s.si -= 1;
        }
        Ok(Flow::Olc0)
    }

    fn hardlfo_onoff(&mut self) -> Result<Flow, Error> {
        let c = self.s.lodsb();
        match c {
            b'D' => {
                let al = self.get_clock()?;
                self.put2(0xe4, al);
                return Ok(Flow::Olc0);
            }
            b'f' => {
                self.hf_set();
                return Ok(Flow::Olc0);
            }
            b'w' => {
                self.wf_set()?;
                return Ok(Flow::Olc0);
            }
            b'p' => {
                let _ = self.pmd_set();
                return Ok(Flow::Olc0);
            }
            b'a' => {
                self.amd_set();
                return Ok(Flow::Olc0);
            }
            b'#' => {
                self.hf_set();
                if self.s.lodsb() != b',' {
                    return Err(mc(b'#', 6));
                }
                self.wf_set()?;
                // (the two `jnz error` below reuse `dx` as the code before them left it)
                if self.s.lodsb() != b',' {
                    return Err(mc(b'#', 2));
                }
                let (dh, dl) = self.pmd_set();
                if self.s.lodsb() != b',' {
                    return Err(mc(dh, dl));
                }
                self.amd_set();
                return Ok(Flow::Olc0);
            }
            _ => {}
        }
        self.s.si -= 1;
        let (_, bx) = self.lngset();
        if bx as u8 == 0 {
            if self.s.peek() == b',' {
                self.s.si += 1;
                self.lngset();
            }
            self.put2(0xe0, 0);
            return Ok(Flow::Olc0);
        }
        if self.s.peek() != b',' {
            return Err(mc(b'#', 6));
        }
        self.s.si += 1;
        let (_, b2) = self.lngset();
        if b2 as u8 >= 8 {
            return Err(mc(b'#', 2));
        }
        self.put2(0xe0, b2 as u8 | 8);
        Ok(Flow::Olc0)
    }

    fn hf_set(&mut self) {
        let (dl, _) = self.getnum();
        self.put2(0xd7, dl);
    }

    fn wf_set(&mut self) -> Result<(), Error> {
        let (_, bx) = self.lngset();
        if bx as u8 >= 4 {
            return Err(mc(b'#', 2));
        }
        self.put2(0xd9, bx as u8);
        Ok(())
    }

    /// Returns what `getnum` left in `dx` (the sign in `dh`, the byte in `dl`), for the error after it.
    fn pmd_set(&mut self) -> (u8, u8) {
        let (dl, _) = self.getnum();
        self.put2(0xd8, dl | 0x80);
        (self.gn_sign, dl | 0x80)
    }

    fn amd_set(&mut self) {
        let (dl, _) = self.getnum();
        self.put2(0xd8, dl & 0x7f);
    }

    fn porta_start(&mut self) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            return Ok(Flow::Next);
        }
        if self.porta_flag != 0 {
            return Err(mc(b'{', 9));
        }
        self.put(0xda);
        self.porta_flag = 1;
        Ok(Flow::Olc0)
    }

    fn porta_end(&mut self) -> Result<Flow, Error> {
        if self.skip_flag != 0 {
            let (_, bx) = self.lngset2();
            if bx >> 8 != 0 {
                return Err(mc(b'}', 8));
            }
            self.futen_skip();
            if self.s.peek() != b',' {
                return Ok(Flow::Next);
            }
            self.s.si += 1;
            let (carry, _) = self.lngset2();
            if carry {
                return Err(mc(b'}', 6));
            }
            self.futen_skip();
            return Ok(Flow::Next);
        }
        if self.porta_flag != 1 {
            return Err(mc(b'}', 13));
        }
        if self.back(5) != 0xda {
            return Err(mc(b'}', 14));
        }
        if self.back(4) == 0x0f {
            return Err(mc(b'}', 15));
        }
        if self.back(2) == 0x0f {
            return Err(mc(b'}', 15));
        }
        let al = self.back(2);
        let n = self.di;
        self.m[n - 3] = al;
        self.di -= 2;
        self.porta_flag = 0;
        let (_, bx) = self.lngset2();
        if bx >> 8 != 0 {
            return Err(mc(b'}', 8));
        }
        self.lngcal()?;
        if self.futen()? {
            return Err(mc(b'}', 8));
        }
        self.put(self.leng);
        if self.s.peek() == b',' {
            self.s.si += 1;
            let (carry, _) = self.lngset2();
            if carry {
                return Err(mc(b'}', 6));
            }
            self.lngcal()?;
            self.futen()?;
            let al = self.leng;
            if al >= self.back(1) {
                // `lngcal` left 21 in `dx`, so `dh` is 0 here
                return Err(mc(0, 8));
            }
            let n = self.di;
            // dx = es:[di-2] to es:1[di]; dx = es:[di-4] (dh = the start note) to es:[di-1], then the rest
            let (a, b) = (self.m[n - 2], self.m[n - 1]);
            self.m[n + 1] = a;
            self.m[n + 2] = b;
            let (c4, c3) = (self.m[n - 4], self.m[n - 3]);
            self.m[n - 1] = c4;
            self.m[n] = c3;
            self.m[n - 4] = c3;
            self.m[n - 3] = al;
            self.m[n - 2] = 0xfb;
            self.m[n + 2] = self.m[n + 2].wrapping_sub(al);
            self.di += 3;
        }
        self.prsok = 9;
        Ok(Flow::Next)
    }

    // ----- loops ---------------------------------------------------------------------------------------------

    fn word_at(&mut self, at: usize, v: u16) {
        self.m[at] = v as u8;
        self.m[at + 1] = (v >> 8) as u8;
    }

    /// `[`
    fn stloop(&mut self) -> Result<Flow, Error> {
        if self.lopcnt >= 32 {
            return Err(Error::Unsupported("more than 32 nested loops"));
        }
        self.put(0xf9);
        let i = usize::from(self.lopcnt);
        self.lopcnt += 1;
        self.loptbl[i] = self.di as u16;
        self.lextbl[i] = 0;
        self.di += 2;
        self.length_check2 = 0;
        Ok(Flow::Olc0)
    }

    /// `]`
    fn edloop(&mut self) -> Result<Flow, Error> {
        self.put(0xf8);
        let (carry, bx) = self.lngset();
        let bl;
        if carry {
            bl = self.d.loop_def;
        } else {
            if bx >> 8 != 0 {
                return Err(mc(b']', 2));
            }
            bl = bx as u8;
        }
        self.edl_finish(bl)?;
        Ok(Flow::Olc0)
    }

    /// `edl00b` to the end of `]`: the repeat count `bl`, then the links between `[`, `:` and `]`.
    fn edl_finish(&mut self, bl: u8) -> Result<(), Error> {
        self.put(bl);
        if bl == 0 && self.length_check2 == 0 {
            return Err(mc(b'[', 24));
        }
        self.put(0);
        self.lopcnt = self.lopcnt.wrapping_sub(1);
        if self.lopcnt == 0xff {
            return Err(mc(b']', 23));
        }
        let i = usize::from(self.lopcnt);
        let start = self.loptbl[i];
        self.word_at(self.di, start);
        // the `[` gets the address of the repeat count, and so does a `:`
        let count_at = (self.di - 2) as u16;
        self.word_at(usize::from(start), count_at);
        let ext = self.lextbl[i];
        if ext != 0 {
            self.word_at(usize::from(ext), count_at);
            if self.d.bend != 0 {
                self.alldet = 0x8000;
            }
        }
        self.di += 2;
        Ok(())
    }

    /// `:`
    fn extloop(&mut self) -> Result<Flow, Error> {
        self.put(0xf7);
        let al = self.lopcnt.wrapping_sub(1);
        if al == 0xff {
            return Err(mc(b':', 23));
        }
        let i = usize::from(al);
        if self.lextbl[i] != 0 {
            return Err(mc(b':', 25));
        }
        self.lextbl[i] = self.di as u16;
        self.di += 2;
        Ok(Flow::Olc0)
    }

    /// `L`
    fn lopset(&mut self) -> Result<Flow, Error> {
        if self.part == RHYTHM {
            return Err(mc(b'L', 17));
        }
        self.put(0xf6);
        self.allloop_flag = 1;
        self.length_check1 = 0;
        Ok(Flow::Olc0)
    }

    // ----- the parts -----------------------------------------------------------------------------------------

    /// `cloop`: the lines of the file for one part. Returns at the part's end.
    fn cloop(&mut self) -> Result<(), Error> {
        self.s.si = 0;
        loop {
            let c = self.s.lodsb();
            if is_sjis_lead(c) {
                self.s.si += 1;
                continue;
            }
            if c == 0x1a {
                return Ok(());
            }
            if c < b' ' + 1 || c == b';' {
                self.s.line_skip();
                continue;
            }
            if c == b'`' {
                self.skip_flag ^= 2;
                continue;
            }
            if self.skip_flag & 2 == 0 {
                if c == b'!' {
                    // hsset again: the definitions are made once per part, in file order
                    let mut v = std::mem::take(&mut self.vars);
                    let r = self.s.hsset(&mut v);
                    self.vars = v;
                    r?;
                    self.s.line_skip();
                    continue;
                }
                if c == b'"' {
                    self.skip_flag ^= 1;
                    self.put2(0xc0, self.skip_flag & 1);
                    continue;
                }
                if c == b'\'' {
                    self.skip_flag &= 0xfe;
                    self.put2(0xc0, 0);
                    continue;
                }
            }
            if c != self.part + b'A' - 1 {
                continue;
            }
            if self.one_line()? {
                return Ok(());
            }
        }
    }

    /// `one_line_compile`: the rest of the token after the part letter (more letters may follow before the
    /// blank), then the line. `Ok(true)` is the `/` that ends the part.
    fn one_line(&mut self) -> Result<bool, Error> {
        self.lastprg = 0;
        self.s.skip = false;
        loop {
            let k = self.s.lodsb();
            if is_sjis_lead(k) {
                self.s.si += 1;
                continue;
            }
            if k == 13 {
                self.s.si += 1;
                return Ok(false);
            }
            if k == b';' {
                self.s.line_skip();
                return Ok(false);
            }
            if k == b'`' {
                self.skip_flag ^= 2;
                continue;
            }
            if self.skip_flag & 2 != 0 {
                // olc_skip2
                loop {
                    let j = self.s.lodsb();
                    if is_sjis_lead(j) {
                        self.s.si += 1;
                        continue;
                    }
                    if j == 0x0a {
                        break;
                    }
                    if j == b'`' {
                        self.skip_flag &= 0xfd;
                        break;
                    }
                }
                return Ok(false);
            }
            if k > b' ' {
                continue;
            }
            self.s.si -= 1;
            return self.olc();
        }
    }

    /// The R part: the `R` pattern lines, each compiled into the data area behind a table of their addresses.
    fn rpart(&mut self) -> Result<(), Error> {
        self.part = RHYTHM;
        self.ongen = PCM;
        let mut bx = self.di + 2 * usize::from(self.kpart_maxprg);
        self.s.si = 0;
        self.cm_init();
        self.deflng = self.deflng_k;
        'rt: loop {
            if self.kpart_maxprg != 0 {
                self.put2(bx as u8, (bx >> 8) as u8);
            }
            loop {
                if self.s.peek() == 0x1a {
                    break 'rt;
                }
                let found = loop {
                    let c = self.s.lodsb();
                    if is_sjis_lead(c) {
                        self.s.si += 1;
                        continue;
                    }
                    if c < b' ' + 1 || c == b';' {
                        self.s.line_skip();
                        break false;
                    }
                    if c == b'`' {
                        self.skip_flag ^= 2;
                        continue;
                    }
                    if self.skip_flag & 2 == 0 && c == b'!' {
                        let mut v = std::mem::take(&mut self.vars);
                        let r = self.s.hsset(&mut v);
                        self.vars = v;
                        r?;
                        self.s.line_skip();
                        break false;
                    }
                    if self.kpart_maxprg == 0 {
                        self.s.line_skip();
                        break false;
                    }
                    if c == b'R' {
                        break true;
                    }
                };
                if !found {
                    continue;
                }
                let table = self.di;
                self.di = bx;
                self.hsflag = 1;
                self.prsok = 0;
                self.one_line()?;
                self.put(0xff);
                bx = self.di;
                self.di = table;
                self.kpart_maxprg -= 1;
                continue 'rt;
            }
        }
        self.di = bx;
        if self.kpart_maxprg != 0 {
            return Err(mc(0, 29));
        }
        Ok(())
    }

    /// The commands every part starts with (`cmloop2`): `#Zenlength`, `#Tempo`, `#Timer`, `#VolumeDown`,
    /// the `#Detune`, `#EnvSpeed` and `#LFOSpeed` extensions, `#ADPCM` and `#Transpose`.
    fn part_start(&mut self) -> Result<(), Error> {
        let part = self.part;
        if part == 1 && self.d.fm3_parts[0] != 0 {
            // the extra FM3 parts' addresses, filled in as they are compiled
            self.put(0xc6);
            self.fm3_ofs = Some(self.di);
            self.di += 6;
        }
        if part == PCMPART && self.d.pcm_parts[0] != 0 {
            self.put(0xb4);
            self.pcm_ofs = Some(self.di);
            self.di += 16;
        }
        if part == 7 {
            if self.zenlen != 96 {
                self.put2(0xdf, self.zenlen);
            }
            if self.d.tempo != 0 {
                self.put2(0xfc, 0xff);
                self.put(self.d.tempo);
            }
            if self.d.timerb != 0 {
                self.put2(0xfc, self.d.timerb);
            }
            // FM (fec0), SSG (fcc0), PCM (fac0), PPZ8 (f5c0), rhythm (f8c0), in MC's order.
            for (idx, hi) in [(0usize, 0xfeu8), (1, 0xfc), (2, 0xfa), (4, 0xf5), (3, 0xf8)] {
                let (amount, abs) = self.d.voldown[idx];
                if amount != 0 && !self.d.opts.towns_flg {
                    self.put2(0xc0, hi + abs);
                    self.put(amount);
                }
            }
        }
        let o = &self.d.opts;
        let (opm, towns) = (o.opl_flg || o.x68_flg, o.towns_flg);
        if !opm && self.d.ext_detune && self.ongen == PSG {
            self.put2(0xcc, 0x01);
        }
        if !opm && self.d.ext_env && self.ongen >= PSG && part != RHYTHM2 {
            self.put2(0xc9, 0x01);
        }
        if self.d.ext_lfo && part != RHYTHM2 {
            self.put2(0xca, 0x01);
            if !towns {
                self.put2(0xbb, 0x01);
            }
        }
        if !towns && self.d.adpcm_flag != 0xff && part == PCMPART {
            self.put2(0xc0, 0xf7);
            self.put(self.d.adpcm_flag);
        }
        if self.d.transpose != 0 && part != RHYTHM2 {
            self.put2(0xb2, self.d.transpose);
        }
        Ok(())
    }
}

/// `lc.inc`'s `jumptable`: the bytes after each command byte from 0xff down to 0xb1; 100 and up are the commands
/// with a handler of their own.
const LC_JUMP: [u8; 79] = [
    1, 1, 1, 100, 0, 2, 101, 102, 103, 104, 1, 0, 0, 4, 1, 4, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 105, 1, 1, 1, 2, 2, 1, 1, 1, 1, 1, 1, 6, 5,
    1, 1, 1, 1, 3, 3, 6, 1, 1, 2, 1, 0, 106, 4, 1, 2, 1, 1, 1, 1, 2, 1, 1, 2, 16, 1, 1, 1,
];

impl Compiler {
    /// `lc`: the longest part's length and the longest loop, as the 8 bytes MC writes after the parts.
    fn lc(&mut self) -> Result<[u8; 8], Error> {
        // `lc` works on the real buffer: its loop counters stay in the `.M` as the run left them.
        let mut m = std::mem::take(&mut self.m);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Self::lc_run(&mut m)))
            .unwrap_or(Err(Error::Unsupported("lc.inc ran off the buffer (MC would read stale memory)")));
        self.m = m;
        r
    }

    /// `command_exec` of lc.inc: one command byte `al` (0xb1 and up), `si` after it.
    fn lc_exec(m: &mut [u8], al: u8, si: &mut usize, all: &mut u32, loop_len: &mut u32, loop_flag: &mut bool) -> Result<(), Error> {
        let word = |m: &[u8], at: usize| usize::from(m[at]) | usize::from(m[at + 1]) << 8;
        if al < 0xb1 {
            return Err(Error::Unsupported("a byte lc.inc has no command for"));
        }
        match LC_JUMP[usize::from(!al)] {
            n @ 0..=16 => *si += usize::from(n),
            100 => {
                let b = m[*si];
                *si += 1;
                if b >= 251 {
                    *si += 1;
                }
            }
            101 => {
                let ax = word(m, *si);
                *si += 2;
                m[ax + 1] = 0;
            }
            102 => {
                let count = m[*si];
                *si += 1;
                if count == 0 {
                    *loop_flag = true;
                } else {
                    m[*si] = m[*si].wrapping_add(1);
                    let now = m[*si];
                    *si += 1;
                    if count != now {
                        *si = word(m, *si) + 2;
                    } else {
                        *si += 2;
                    }
                }
            }
            103 => {
                let bx = word(m, *si);
                *si += 2;
                let dl = m[bx].wrapping_sub(1);
                if dl == m[bx + 1] {
                    *si = bx + 1 + 3;
                }
            }
            104 => *loop_len = *all,
            105 => {
                *si += 2;
                *all += u32::from(m[*si]);
                *si += 1;
            }
            _ => {
                let x = m[*si];
                *si += 1;
                if x >= 2 {
                    if x < 0xf5 {
                        return Err(Error::Unsupported("a 0xc0 sub-command lc.inc has no entry for"));
                    }
                    *si += 1;
                }
            }
        }
        Ok(())
    }

    /// One part's walk in `lc.inc` from `si`: (length, loop start, loop flag); `k` is the K part.
    fn lc_scan(m: &mut [u8], mut si: usize, k: bool) -> Result<(u32, u32, bool), Error> {
        let word = |m: &[u8], at: usize| usize::from(m[at]) | usize::from(m[at + 1]) << 8;
        let (mut all, mut loop_len, mut loop_flag) = (0u32, u32::MAX, false);
        if k {
            // the K part: a byte below 0x80 is a pattern of the R part (`kcom_loop`, `rcom_loop`)
            let table = word(m, 2 * 11);
            loop {
                let al = m[si];
                si += 1;
                if al == 0x80 {
                    break;
                }
                if al < 0x80 {
                    let mut rsi = word(m, table + 2 * usize::from(al));
                    loop {
                        let b = m[rsi];
                        rsi += 1;
                        if b == 0xff {
                            break;
                        }
                        if b < 0xc0 {
                            if b & 0x80 != 0 {
                                rsi += 1;
                            }
                            all += u32::from(m[rsi]);
                            rsi += 1;
                            continue;
                        }
                        Self::lc_exec(m, b, &mut rsi, &mut all, &mut loop_len, &mut loop_flag)?;
                        if loop_flag {
                            break;
                        }
                    }
                } else {
                    Self::lc_exec(m, al, &mut si, &mut all, &mut loop_len, &mut loop_flag)?;
                }
                if loop_flag {
                    break;
                }
            }
        } else {
            loop {
                let al = m[si];
                si += 1;
                if al == 0x80 {
                    break;
                }
                if al < 0x80 {
                    all += u32::from(m[si]);
                    si += 1;
                    continue;
                }
                Self::lc_exec(m, al, &mut si, &mut all, &mut loop_len, &mut loop_flag)?;
                if loop_flag {
                    break;
                }
            }
        }
        Ok((all, loop_len, loop_flag))
    }

    fn lc_run(m: &mut Vec<u8>) -> Result<[u8; 8], Error> {
        let word = |m: &[u8], at: usize| usize::from(m[at]) | usize::from(m[at + 1]) << 8;
        let m = &mut *m;
        let (mut max_all, mut max_loop) = (0u32, 0u32);
        let mut fold = |all: u32, loop_len: u32, loop_flag: bool| {
            if all != 0 {
                if all > max_all {
                    max_all = all;
                }
                if !loop_flag && loop_len != u32::MAX {
                    let l = all.wrapping_sub(loop_len);
                    if l > max_loop {
                        max_loop = l;
                    }
                }
            }
        };
        // parts A-J, then the extension parts (`#FM3Extend`, `#PCMExtend`: the words after 0xc6 and 0xb4), then K
        let mut ext: Vec<usize> = Vec::new();
        for idx in 0..10 {
            let mut si = word(m, idx * 2);
            let words = if idx == 0 && m[si] == 0xc6 {
                3
            } else if idx == 9 && m[si] == 0xb4 {
                8
            } else {
                0
            };
            if words != 0 {
                si += 1;
                for k in 0..words {
                    ext.push(word(m, si + 2 * k));
                }
                si += 2 * words;
            }
            let (all, loop_len, loop_flag) = Self::lc_scan(m, si, false)?;
            fold(all, loop_len, loop_flag);
        }
        for si in ext {
            if si != 0 {
                let (all, loop_len, loop_flag) = Self::lc_scan(m, si, false)?;
                fold(all, loop_len, loop_flag);
            }
        }
        let ksi = word(m, 20);
        let (all, loop_len, loop_flag) = Self::lc_scan(m, ksi, true)?;
        fold(all, loop_len, loop_flag);
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&max_all.to_le_bytes());
        out[4..].copy_from_slice(&max_loop.to_le_bytes());
        Ok(out)
    }
}

impl Compiler {
    /// One part, from `cm_init` to its end mark (`cmloop2` to `part_end`).
    fn compile_part(&mut self) -> Result<(), Error> {
        self.part_at = self.di;
        self.cm_init();
        self.part_start()?;
        self.cloop()?;
        // part_end: the checks
        // check_lopcnt: MC warns and closes each open loop with `]1` (its `bh` is taken to be 0 here)
        while self.lopcnt != 0 {
            self.put(0xf8);
            self.edl_finish(1)?;
        }
        if self.porta_flag != 0 {
            return Err(mc(b'{', 9));
        }
        if self.allloop_flag != 0 && self.length_check1 == 0 {
            return Err(mc(b'L', 10));
        }
        self.put(0x80);
        Ok(())
    }
}

/// MC: the `.MML` file's bytes in, the `.M` file out.
pub fn compile(mml: &[u8]) -> Result<Vec<u8>, Error> {
    compile_with(mml, None)
}

/// `compile`, with the files `#Include` names read by `inc`.
pub fn compile_with(mml: &[u8], inc: Option<IncludeReader>) -> Result<Vec<u8>, Error> {
    let p1 = pass1_with(mml, inc)?;
    let d = p1.directives;
    let zenlen = d.zenlen;
    let prg_flg = d.opts.prg_flg;
    let (opl, towns) = (d.opts.opl_flg, d.opts.towns_flg);
    let d_x68 = d.opts.x68_flg;
    let opm = opl || d.opts.x68_flg; // OPM and OPL: FM everywhere but the PCM part
    let mut c = Compiler {
        s: Scan { b: p1.mml, si: 0, skip: false },
        d,
        vars: p1.variables,
        voices: p1.voices,
        prg_num: [0; 256],
        fm3_ofs: None,
        pcm_ofs: None,
        m: vec![0; M_SIZE + 16],
        di: 2 * (MAX_PART + 1) + if prg_flg & 1 != 0 { 2 } else { 0 },
        part: 0,
        ongen: 0,
        octave: 3,
        deflng: zenlen >> 2,
        zenlen,
        leng: 0,
        calflg: 0,
        prsok: 0,
        skip_flag: 0,
        tie_flag: 0,
        porta_flag: 0,
        length_check1: 0,
        length_check2: 0,
        volss: 0,
        volss2: 0,
        nowvol: 0,
        maxprg: 0,
        octss: 0,
        hsflag: 0,
        lopcnt: 0,
        allloop_flag: 0,
        loptbl: [0; 32],
        lextbl: [0; 32],
        cmd: 0,
        gn_sign: 0,
        part_at: 0,
        detune: 0,
        master_detune: 0,
        pitch: 0,
        alldet: 0x8000,
        def_note: [0; 7],
        acc_adr: 0,
        lastprg: 0,
        kpart_maxprg: 0,
        deflng_k: 0,
        ge_delay: 0,
        ge_depth: 0,
        ge_depth2: 0,
        ge_flag1: 0,
        ge_flag2: 0,
        ge_tie: 0,
        ge_dep_flag: 0,
        ss_speed: 0,
        ss_depth: 0,
        ss_length: 0,
        ss_tie: 0,
    };
    for part in 1..=MAX_PART as u8 {
        c.part = part;
        c.ongen = if opm { if part == PCMPART { PCM } else { 0 } } else { (part - 1) / 3 };
        let at = c.di as u16;
        let slot = 2 * (usize::from(part) - 1);
        c.m[slot] = at as u8;
        c.m[slot + 1] = (at >> 8) as u8;
        c.compile_part()?;
    }
    // K's `maxprg` and `l` are taken before the extension parts (`fm3_check` and `pcm_check` come after)
    c.kpart_maxprg = if towns { 0 } else { c.maxprg }; // Towns has no R part
    c.deflng_k = c.deflng;
    // the extension parts: `#FM3Extend` (FM) and `#PCMExtend` (the extra PCM), each at the next address word
    for (kind, ofs, letters) in [(0u8, c.fm3_ofs, c.d.fm3_parts.to_vec()), (PCM_EX, c.pcm_ofs, c.d.pcm_parts.to_vec())] {
        let Some(ofs) = ofs else { continue };
        for (i, letter) in letters.into_iter().filter(|&l| l != 0).enumerate() {
            c.part = letter - b'A' + 1;
            c.ongen = kind;
            let at = c.di as u16;
            c.m[ofs + 2 * i] = at as u8;
            c.m[ofs + 2 * i + 1] = (at >> 8) as u8;
            c.compile_part()?;
        }
    }
    c.ongen = PCM;
    // the R part: MC compiles `R` pattern definitions only when the K part uses patterns
    let r_at = c.di as u16;
    c.m[2 * MAX_PART] = r_at as u8;
    c.m[2 * MAX_PART + 1] = (r_at >> 8) as u8;
    c.rpart()?;
    // rem_set
    if !towns {
        let lc = c.lc()?;
        for b in lc {
            c.put(b);
        }
    }
    let bp = c.di;
    c.di += 2;
    c.put(if towns { 0x46 } else { 0x48 });
    c.put(0xfe);
    // vdat_set: the voices used, number then 25 bytes each, ended by 00 ff
    if prg_flg & 1 != 0 {
        let at = 2 * (MAX_PART + 1);
        c.m[at] = c.di as u8;
        c.m[at + 1] = (c.di >> 8) as u8;
        for i in 0..256 {
            if c.prg_num[i] != 0 {
                c.put(i as u8);
                if opl {
                    for k in 0..9 {
                        c.put(c.voices[16 * i + k]);
                    }
                } else {
                    for k in 0..25 {
                        c.put(c.voices[32 * i + k]);
                    }
                }
            }
        }
        c.put2(0, 0xff);
    }
    // memo_write
    let mut ptrs: Vec<u16> = Vec::new();
    let mut strings: Vec<(Option<Vec<u8>>, bool)> = Vec::new();
    if !towns {
        strings.push((c.d.ppz_file.clone(), true)); // Towns has no #PPZFile
    }
    strings.extend([
        (c.d.pps_file.clone(), true),
        (c.d.pcm_file.clone(), true),
        (c.d.title.clone(), false),
        (c.d.composer.clone(), false),
        (c.d.arranger.clone(), false),
    ]);
    for m in c.d.memo.clone() {
        strings.push((Some(m), false));
    }
    for (text, upper) in strings {
        ptrs.push(c.di as u16);
        match text {
            None => c.put(0),
            Some(t) => {
                for b in t {
                    c.put(if upper && b.is_ascii_lowercase() { b - 32 } else { b });
                }
                c.put(0);
            }
        }
    }
    c.m[bp] = c.di as u8;
    c.m[bp + 1] = (c.di >> 8) as u8;
    for p in ptrs {
        c.put2(p as u8, (p >> 8) as u8);
    }
    c.put2(0, 0);
    if c.di > M_SIZE {
        return Err(mc(0, 19));
    }
    let mut out = vec![u8::from(opl) * 2 | u8::from(d_x68)]; // m_start: OPL and X68 flags
    out.extend_from_slice(&c.m[..c.di]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every `.M` (any case, `.M2` and `.MZ` too) under the folders in PMD_M (separated by `:`): decompiled, and
    // compiled back to the same bytes. A count, with the first reasons.
    // PMD_M=dir:dir cargo test --release --lib pmd_mc::tests::decompiles -- --ignored --nocapture
    #[test]
    #[ignore]
    fn decompiles() {
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| matches!(x.to_ascii_lowercase().to_str(), Some("m" | "m2" | "mz"))) {
                    out.push(p);
                }
            }
        }
        if let Ok(f) = std::env::var("DUMP") {
            let m = std::fs::read(&f).unwrap();
            let mml = decompile::write(&m, true).or_else(|| decompile::write(&m, false));
            println!("{}", mml.as_ref().map_or("no MML".into(), |x| String::from_utf8_lossy(x).to_string()));
            if let Some(Ok(got)) = mml.map(|x| compile(&x)) {
                let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
                println!("want {}\n got {}", hex(&m[..m.len().min(160)]), hex(&got[..got.len().min(160)]));
                let w = |b: &[u8], i: usize| usize::from(b[1 + 2 * i]) | usize::from(b[2 + 2 * i]) << 8;
                for i in 0..11 {
                    let (a0, a1, b0, b1) = (w(&m, i), w(&m, i + 1), w(&got, i), w(&got, i + 1));
                    let (x, y) = (&m[1 + a0..1 + a1.max(a0)], &got[1 + b0..1 + b1.max(b0)]);
                    if x != y {
                        let k = (0..x.len().max(y.len())).find(|&k| x.get(k) != y.get(k)).unwrap();
                        let from = k.saturating_sub(12);
                        println!("part {} at {k}: want {}\n               got  {}", (b'A' + i as u8) as char, hex(&x[from..(k + 16).min(x.len())]), hex(&y[from..(k + 16).min(y.len())]));
                        break;
                    }
                }
            }
            println!("{}", decompile::why(&m));
            if let Some(Ok(got)) = decompile::write(&m, true).or_else(|| decompile::write(&m, false)).map(|x| compile(&x)) {
                println!("music: {}", decompile::music_diff(&m, &got));
            }
            return;
        }
        let mut files = Vec::new();
        for d in std::env::var("PMD_M").expect("PMD_M").split(':') {
            walk(std::path::Path::new(d), &mut files);
        }
        files.sort();
        let mut music = 0;
        let (mut same, mut bad) = (0, Vec::new());
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        for p in &files {
            let m = std::fs::read(p).unwrap();
            if decompile(&m).is_some() {
                same += 1;
            } else if to_mml(&m).is_some() {
                music += 1;
            } else {
                let w = decompile::why(&m);
                *reasons.entry(w.split(" at ").next().unwrap().split(" [").next().unwrap().to_string()).or_default() += 1;
                bad.push(format!("{}: {w}", p.display()));
            }
        }
        for b in bad.iter().take(std::env::var("SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(20)) {
            println!("{b}");
        }
        let mut r: Vec<_> = reasons.into_iter().collect();
        r.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in r {
            println!("{n:5} {k}");
        }
        println!("{same} of {} come back, {music} more as the same music", files.len());
    }

    fn p(s: &str) -> Pass1 {
        pass1(s.as_bytes()).unwrap()
    }

    #[test]
    fn text_directives_keep_their_lines() {
        let r = p("#Title   My song\r\n#Composer Me\r\n#Memo one\r\n#Memo two\r\nA cde\r\n");
        assert_eq!(r.directives.title.as_deref(), Some(&b"My song"[..]));
        assert_eq!(r.directives.composer.as_deref(), Some(&b"Me"[..]));
        assert_eq!(r.directives.memo, vec![b"one".to_vec(), b"two".to_vec()]);
        // The `#` lines become comments.
        assert!(r.mml.starts_with(b";Title"));
        assert!(r.mml.windows(5).any(|w| w == b"A cde"));
    }

    #[test]
    fn numbers_and_switches() {
        let r = p("#Tempo 120\n#Zenlength 192\n#Timer $f0\n#Jump 40000\n#Octave Rev\n#DT2flag on\n#LFOSpeed Extend\n#Bendrange 12\n");
        let d = r.directives;
        assert_eq!((d.tempo, d.zenlen, d.deflng, d.timerb, d.jump_flag, d.bend), (120, 192, 48, 0xf0, 40000, 12));
        assert!(d.octave_rev && d.dt2_flg && d.ext_lfo);
    }

    #[test]
    fn volume_down_groups() {
        let d = p("#VolumeDown F-4,S+2,P8\n").directives;
        assert_eq!(d.voldown[0], (252, 0)); // -4, relative
        assert_eq!(d.voldown[1], (2, 0));
        assert_eq!(d.voldown[2], (8, 1)); // absolute
    }

    #[test]
    fn part_letters_and_options() {
        let d = p("#FM3Extend XYZ\n#PCMExtend LMN\n#Option /v /p -n\n").directives;
        assert_eq!(d.fm3_parts, [b'X', b'Y', b'Z']);
        assert_eq!(&d.pcm_parts[..4], b"LMN\0");
        assert_eq!((d.opts.play_flg, d.opts.prg_flg, d.dt2_flg), (true, 1, false));
    }

    #[test]
    fn definitions_are_noted_and_skipped_blocks_are_not_read() {
        let r = p("!0 cde\r\n`\r\n#Tempo 99\r\n`\r\n#Tempo 100\r\n");
        assert_eq!(r.variables.numbered[0], 3);
        assert_eq!(r.directives.tempo, 100);
    }

    #[test]
    fn named_variables_and_voices() {
        let r = p("!tune cde\r\n!tune2 fga\r\n!tune gg\r\n");
        // `tune` is a whole word, so `!tune` the second time overwrites it; `tune2` is its own.
        assert_eq!(r.variables.named.iter().filter(|e| e.0 != 0).count(), 2);
        assert_eq!(&r.variables.named[0].1[..5], b"tune\0");
        // @ num, alg, fb, then four operators of ar dr sr rr sl tl ks ml dt ams; a name with =.
        let r = p("@3 4,5\r\n  31,1,2,3,4,50,1,2,-1,1\r\n  31 1 2 3 4 50 1 2 5 0\r\n  0,0,0,0,0,0,0,0,0,0\r\n  =Piano\r\n  1,2,3,4,5,6,7,8,9,10\r\n");
        let v = &r.voices[3 * 32..4 * 32];
        assert_eq!(v[24], 5 << 3 | 4);
        // slot 1 is the first group: ml/dt at 0, tl at 4; dt -1 is stored as 5
        assert_eq!(v[0], 2 | 5 << 4);
        assert_eq!(v[4], 50);
        assert_eq!(v[8], 31 | 1 << 6);
        assert_eq!(&v[25..31], b"Piano\0");
        // the second group (slot 2) is at +2: dt 5 -> 5<<4 | ml 2
        assert_eq!(v[2], 2 | 5 << 4);
    }

    /// Files MC.EXE itself compiled (`synth/scripts/pmd-mc-reference.sh`): the `.M` must be the same bytes.
    #[test]
    fn compiles_the_same_bytes_as_mc() {
        let files: &[(&str, &[u8], &[u8])] = &[
            ("a1", &include_bytes!("pmd_mc_ref/a1.mml")[..], &include_bytes!("pmd_mc_ref/a1.m")[..]),
            ("a2", &include_bytes!("pmd_mc_ref/a2.mml")[..], &include_bytes!("pmd_mc_ref/a2.m")[..]),
            ("a3", &include_bytes!("pmd_mc_ref/a3.mml")[..], &include_bytes!("pmd_mc_ref/a3.m")[..]),
            ("a4", &include_bytes!("pmd_mc_ref/a4.mml")[..], &include_bytes!("pmd_mc_ref/a4.m")[..]),
            ("a5", &include_bytes!("pmd_mc_ref/a5.mml")[..], &include_bytes!("pmd_mc_ref/a5.m")[..]),
            ("a7", &include_bytes!("pmd_mc_ref/a7.mml")[..], &include_bytes!("pmd_mc_ref/a7.m")[..]),
            ("a8", &include_bytes!("pmd_mc_ref/a8.mml")[..], &include_bytes!("pmd_mc_ref/a8.m")[..]),
            ("a9", &include_bytes!("pmd_mc_ref/a9.mml")[..], &include_bytes!("pmd_mc_ref/a9.m")[..]),
            ("pcmrep1", &include_bytes!("pmd_mc_ref/pcmrep1.mml")[..], &include_bytes!("pmd_mc_ref/pcmrep1.m")[..]),
            ("pcmrep2", &include_bytes!("pmd_mc_ref/pcmrep2.mml")[..], &include_bytes!("pmd_mc_ref/pcmrep2.m")[..]),
            ("lfo2a", &include_bytes!("pmd_mc_ref/lfo2a.mml")[..], &include_bytes!("pmd_mc_ref/lfo2a.m")[..]),
            ("lfo2b", &include_bytes!("pmd_mc_ref/lfo2b.mml")[..], &include_bytes!("pmd_mc_ref/lfo2b.m")[..]),
            ("vopt1", &include_bytes!("pmd_mc_ref/vopt1.mml")[..], &include_bytes!("pmd_mc_ref/vopt1.m")[..]),
            ("vopt2", &include_bytes!("pmd_mc_ref/vopt2.mml")[..], &include_bytes!("pmd_mc_ref/vopt2.m")[..]),
            ("ext1", &include_bytes!("pmd_mc_ref/ext1.mml")[..], &include_bytes!("pmd_mc_ref/ext1.m")[..]),
            ("ext2", &include_bytes!("pmd_mc_ref/ext2.mml")[..], &include_bytes!("pmd_mc_ref/ext2.m")[..]),
            ("ext3", &include_bytes!("pmd_mc_ref/ext3.mml")[..], &include_bytes!("pmd_mc_ref/ext3.m")[..]),
            ("fmpitch1", &include_bytes!("pmd_mc_ref/fmpitch1.mml")[..], &include_bytes!("pmd_mc_ref/fmpitch1.m")[..]),
            ("fmpitch2", &include_bytes!("pmd_mc_ref/fmpitch2.mml")[..], &include_bytes!("pmd_mc_ref/fmpitch2.m")[..]),
            ("modem", &include_bytes!("pmd_mc_ref/modem.mml")[..], &include_bytes!("pmd_mc_ref/modem.m")[..]),
            ("model_v", &include_bytes!("pmd_mc_ref/model_v.mml")[..], &include_bytes!("pmd_mc_ref/model_v.m")[..]),
            ("modet", &include_bytes!("pmd_mc_ref/modet.mml")[..], &include_bytes!("pmd_mc_ref/modet.m")[..]),
            ("modet_v", &include_bytes!("pmd_mc_ref/modet_v.mml")[..], &include_bytes!("pmd_mc_ref/modet_v.m")[..]),
            ("noeol", &include_bytes!("pmd_mc_ref/noeol.mml")[..], &include_bytes!("pmd_mc_ref/noeol.m")[..]),
            ("noeol2", &include_bytes!("pmd_mc_ref/noeol2.mml")[..], &include_bytes!("pmd_mc_ref/noeol2.m")[..]),
            ("l000", &include_bytes!("pmd_mc_ref/l000.mml")[..], &include_bytes!("pmd_mc_ref/l000.m")[..]),
            ("l003", &include_bytes!("pmd_mc_ref/l003.mml")[..], &include_bytes!("pmd_mc_ref/l003.m")[..]),
            ("rh1", &include_bytes!("pmd_mc_ref/rh1.mml")[..], &include_bytes!("pmd_mc_ref/rh1.m")[..]),
            ("rh2", &include_bytes!("pmd_mc_ref/rh2.mml")[..], &include_bytes!("pmd_mc_ref/rh2.m")[..]),
            ("rh3", &include_bytes!("pmd_mc_ref/rh3.mml")[..], &include_bytes!("pmd_mc_ref/rh3.m")[..]),
            ("ws1", &include_bytes!("pmd_mc_ref/ws1.mml")[..], &include_bytes!("pmd_mc_ref/ws1.m")[..]),
            ("ws2", &include_bytes!("pmd_mc_ref/ws2.mml")[..], &include_bytes!("pmd_mc_ref/ws2.m")[..]),
            ("rh1", &include_bytes!("pmd_mc_ref/rh1.mml")[..], &include_bytes!("pmd_mc_ref/rh1.m")[..]),
            ("rh2", &include_bytes!("pmd_mc_ref/rh2.mml")[..], &include_bytes!("pmd_mc_ref/rh2.m")[..]),
            ("rh3", &include_bytes!("pmd_mc_ref/rh3.mml")[..], &include_bytes!("pmd_mc_ref/rh3.m")[..]),
            ("ws1", &include_bytes!("pmd_mc_ref/ws1.mml")[..], &include_bytes!("pmd_mc_ref/ws1.m")[..]),
            ("ws2", &include_bytes!("pmd_mc_ref/ws2.mml")[..], &include_bytes!("pmd_mc_ref/ws2.m")[..]),
            ("l087", &include_bytes!("pmd_mc_ref/l087.mml")[..], &include_bytes!("pmd_mc_ref/l087.m")[..]),
            ("s004", &include_bytes!("pmd_mc_ref/s004.mml")[..], &include_bytes!("pmd_mc_ref/s004.m")[..]),
            ("s007", &include_bytes!("pmd_mc_ref/s007.mml")[..], &include_bytes!("pmd_mc_ref/s007.m")[..]),
            ("s008", &include_bytes!("pmd_mc_ref/s008.mml")[..], &include_bytes!("pmd_mc_ref/s008.m")[..]),
            ("t003", &include_bytes!("pmd_mc_ref/t003.mml")[..], &include_bytes!("pmd_mc_ref/t003.m")[..]),
            ("t005", &include_bytes!("pmd_mc_ref/t005.mml")[..], &include_bytes!("pmd_mc_ref/t005.m")[..]),
            ("v001", &include_bytes!("pmd_mc_ref/v001.mml")[..], &include_bytes!("pmd_mc_ref/v001.m")[..]),
            ("v075", &include_bytes!("pmd_mc_ref/v075.mml")[..], &include_bytes!("pmd_mc_ref/v075.m")[..]),
            ("w003", &include_bytes!("pmd_mc_ref/w003.mml")[..], &include_bytes!("pmd_mc_ref/w003.m")[..]),
        ];
        for (name, mml, want) in files {
            assert_eq!(compile(mml).as_deref(), Ok(*want), "{name}");
        }
    }

    #[test]
    fn errors_are_mcs() {
        assert_eq!(pass1(b"#Tempo\n").unwrap_err(), Error::Mc { ch: b'#', code: 6 });
        assert_eq!(pass1(b"#Nonsense 1\n").unwrap_err(), Error::Mc { ch: b'#', code: 7 });
        assert_eq!(pass1(b"#Tempo abc\n").unwrap_err(), Error::Mc { ch: b'#', code: 6 });
    }

    /// `#Include` splices the file in (checked against MC.EXE: 60 files, 0 differences): the same `.M` as the
    /// MML with the file's lines where the directive was.
    #[test]
    fn include_is_the_file_in_place() {
        let files = |n: &[u8]| match n {
            b"a.inc" => Some(b"A o5 l8 cdef\r\nB r4 r4".to_vec()), // no final line break
            b"b.inc" => Some(b"#Include c.inc\r\nB d\x1a".to_vec()), // nested, with a 1A
            b"c.inc" => Some(b"C g a\r\n".to_vec()),
            _ => None,
        };
        let inc = |m: &[u8]| compile_with(m, Some(&files));
        assert_eq!(inc(b"#Include a.inc\r\nA c d e\r\n"), compile(b"A o5 l8 cdef\r\nB r4 r4\r\nA c d e\r\n"));
        assert_eq!(inc(b"#Include b.inc\r\nA c\r\n"), compile(b"C g a\r\nB d\r\nA c\r\n"));
        assert_eq!(inc(b"#Include missing.inc\r\nA c\r\n"), Err(Error::Mc { ch: b'#', code: 3 }));
        assert_eq!(compile(b"#Include a.inc\r\n"), Err(Error::Unsupported("#Include")));
    }
}
