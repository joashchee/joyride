// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MGSC, Ain's MML compiler for MGSDRV (MSX, 1992-93), and its way back.
//!
//! `compile` turns MGSC's MML (`.mus`, Shift JIS text) into the `.mgs` that `mgs.rs` plays, as MGSC 1.11
//! writes it (format "313"), and stops where MGSC stops, with its message and line; `decompile` turns an
//! `.mgs` back into MML that compiles to the same bytes. Written clean from MGSC's manual (`MGSC111.TXT`,
//! "転載、配布は自由", gigamix.jp/mgsdrv/) and the format's published description (Mitsutaka Okazaki's
//! `mgs-format.md`, ISC); MGSC's code was not read. What the manual leaves open was found by running MGSC.COM
//! itself as a black box (Okazaki's `mgsc`, ISC, runs it in a Z80 emulator; `tests::against_mgsc`) and by
//! decompiling Modland's songs and compiling them back (`tests::real_songs_compile_back`). Among what it
//! does that the manual doesn't say:
//!
//! - The file: the tracks back to back, no title line without `#title`, a header tempo of 0 without
//!   `#tempo`, two zero bytes after the header, track 0 only when something is defined or `#alloc` gives
//!   it room, zeros to whole 128-byte records (CP/M's).
//! - The source: a line is read to its 255th byte (the rest is lost), `;` starts a comment even inside
//!   quotes, a track line may start with spaces and needs one after its tracks, a track named twice on a
//!   line gets the MML twice.
//! - Memory: room for 16342 bytes of tracks; a track not given room by `#alloc` gets 16256 / 18 (or / 16 in
//!   rhythm mode) when first used, and no track may outgrow its room; the title and the rooms together
//!   at most 16333 (so a long title can make "Object too big").
//! - Lengths: a note by number (`c5` is 192 / 5 steps, truncated) at most 14400 / tempo, a `%` length at
//!   least 192 / (14400 / tempo) and never below tempo 75, both by the tempo last set (`#tempo` or `t`) as
//!   the source is read; a note's length written only when it isn't `l`'s; a two-frame tempo step
//!   written 3.
//! - Notes: `+` raises a semitone but not `e` or `b`; `-` lowers one, `c-` to `b` in the same octave; one
//!   of them only. An `o` between `_` and its note is written one higher (MGSDRV takes it so).
//! - Each chip's commands: what doesn't apply is "Bad MML" (`m` `s` `n` `y` `/` on SCC, `m` `n` `p` `k`
//!   `@p` on FM, `@l` on the PSG...); `@e` on the PSG writes `@`'s byte; `\` and `h`'s fourth number go
//!   the other way on the PSG and SCC.
//! - Loops: `]`'s count only when `[` has none; only the last `|` of a loop is patched; a `[` left open
//!   keeps offset 0; 16 deep.
//! - Macros: a macro that calls another fails ("Bad MML") once the inner one ends; 8 deep.
//! - Whether a space may stand between a command and its number depends on the command.

use std::fmt;

/// Where MGSC stopped: why, in its words, and the line, counted from 0 as MGSC counts it (none for
/// "Object too big", found when saving).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: Option<usize>,
    pub what: &'static str,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "{} in {}", self.what, l),
            None => f.write_str(self.what),
        }
    }
}

impl std::error::Error for Error {}

type R<T> = Result<T, &'static str>;

// MGSC's messages.
const SYNTAX: &str = "Syntax error";
const BAD: &str = "Bad MML";
const MISSING: &str = "Missing parameter";
const INVALID: &str = "Invalid parameter";
const LENGTH: &str = "Invalid length";
const TRACK: &str = "Invalid track number";
const REPEAT: &str = "Repeat command error";
const UNDEFINED: &str = "Undefined Macro";
const REDEFINED: &str = "Can't defined Macro";
const NESTING: &str = "Macro nesting error";
const ALLOC: &str = "Can't allocate";
const FULL: &str = "Track buffer full";
const TITLE: &str = "Title string buffer over flow";
const BIG: &str = "Object too big";

// The room MGSC has for the tracks, and the most the title and the rooms may take.
const ROOM: usize = 16342;
const OBJECT: usize = 16333;
const LINE: usize = 255;
const TITLE_ROOM: usize = 1018;

/// The steps' frames of a tempo, 2 bits a step, as MGSC writes them after `t` (`41`): a whole note (192
/// steps) is 14400 / tempo whole frames, each step's end rounded to the nearest frame; a two-frame step
/// (below tempo 75) is written 3.
pub fn tempo_table(tempo: u16) -> [u8; 48] {
    let whole = 14400 / u32::from(tempo.max(1));
    let mut out = [0u8; 48];
    let mut prev = 0;
    for k in 0..192u32 {
        let c = (2 * (k + 1) * whole + 192) / 384;
        let f = match c - prev {
            0 => 0,
            1 => 1,
            _ => 3,
        };
        prev = c;
        out[(k / 4) as usize] |= f << (6 - 2 * (k % 4));
    }
    out
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Psg,
    Scc,
    Fm,
    Rhythm,
}

#[derive(Default)]
struct Track {
    /// Its room, once it has one (it's then in the file).
    room: Option<usize>,
    data: Vec<u8>,
    l: u16,
    /// Open loops: where the `[` is, whether it had a count, and its last `|`.
    loops: Vec<(usize, bool, Option<usize>)>,
    /// After `!`.
    off: bool,
    /// After `_`, until its note: an `o` is written one higher.
    porta: bool,
}

struct Compiler {
    mode: Option<u8>,
    machine: u8,
    lfo: u8,
    protect: bool,
    tempo: Option<u16>,
    /// The tempo last set as the source is read (for the length checks).
    now: u16,
    title: Option<Vec<u8>>,
    macro_offset: [u16; 26],
    macros: Vec<Option<Vec<u8>>>,
    tracks: Vec<Track>,
    rooms: usize,
    /// The tracks in the order they got their rooms (MGSC's buffers follow each other so).
    order: Vec<usize>,
    last_m_def: i32,
    last_m: i32,
}

// Shift JIS's first bytes of two.
fn is_lead(c: u8) -> bool {
    (0x81..=0x9f).contains(&c) || (0xe0..=0xfc).contains(&c)
}

fn track_of(c: u8, mode: u8) -> Option<usize> {
    let t = match c.to_ascii_lowercase() {
        c @ b'1'..=b'9' => usize::from(c - b'0'),
        c @ b'a'..=b'h' => usize::from(c - b'a') + 10,
        b'r' if mode == 1 => 15,
        _ => return None,
    };
    if mode == 1 && t > 15 {
        return None;
    }
    Some(t)
}

fn kind(t: usize, mode: u8) -> Kind {
    match t {
        1..=3 => Kind::Psg,
        4..=8 => Kind::Scc,
        15 if mode == 1 => Kind::Rhythm,
        _ => Kind::Fm,
    }
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

/// A cursor over MML or a definition's text.
struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(b: &'a [u8]) -> Cur<'a> {
        Cur { b, i: 0 }
    }
    fn skip(&mut self) {
        while self.i < self.b.len() && is_ws(self.b[self.i]) {
            self.i += 1;
        }
    }
    /// The next byte, lower case, spaces skipped first.
    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.raw()
    }
    /// The next byte, lower case, right here.
    fn raw(&self) -> Option<u8> {
        self.b.get(self.i).map(|c| c.to_ascii_lowercase())
    }
    fn next(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn eat_raw(&mut self, c: u8) -> bool {
        if self.raw() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn digit_raw(&self) -> bool {
        matches!(self.raw(), Some(b'0'..=b'9'))
    }
    /// Digits right here, if any.
    fn digits(&mut self) -> Option<i64> {
        if !self.digit_raw() {
            return None;
        }
        let mut n: i64 = 0;
        while self.digit_raw() {
            n = (n * 10 + i64::from(self.b[self.i] - b'0')).min(1 << 40);
            self.i += 1;
        }
        Some(n)
    }
    /// A number right here, in `lo..=hi`.
    fn num(&mut self, lo: i64, hi: i64) -> R<i32> {
        let n = self.digits().ok_or(MISSING)?;
        if n < lo || n > hi {
            return Err(INVALID);
        }
        Ok(n as i32)
    }
    /// A number after any spaces.
    fn num_sp(&mut self, lo: i64, hi: i64) -> R<i32> {
        self.skip();
        self.num(lo, hi)
    }
    /// A signed number right here.
    fn signed(&mut self, lo: i64, hi: i64) -> R<i32> {
        let neg = if self.eat_raw(b'-') {
            true
        } else {
            self.eat_raw(b'+');
            false
        };
        let n = self.digits().ok_or(MISSING)?;
        let n = if neg { -n } else { n };
        if n < lo || n > hi {
            return Err(INVALID);
        }
        Ok(n as i32)
    }
    fn signed_sp(&mut self, lo: i64, hi: i64) -> R<i32> {
        self.skip();
        self.signed(lo, hi)
    }
    fn hex(&mut self) -> Option<u8> {
        let v = (self.peek()? as char).to_digit(16)? as u8;
        self.i += 1;
        Some(v)
    }
    fn end(&mut self) -> bool {
        self.peek().is_none()
    }
}

impl Compiler {
    fn new() -> Compiler {
        Compiler {
            mode: None,
            machine: 0,
            lfo: 0,
            protect: false,
            tempo: None,
            now: 120,
            title: None,
            macro_offset: [0; 26],
            macros: vec![None; 256],
            tracks: (0..18).map(|_| Track { l: 48, ..Track::default() }).collect(),
            rooms: 0,
            order: Vec::new(),
            last_m_def: -1,
            last_m: -1,
        }
    }

    fn mode(&self) -> R<u8> {
        self.mode.ok_or(SYNTAX)
    }

    // A track's room when it is first used: 16256 bytes shared by all the tracks there can be.
    fn ensure_room(&mut self, t: usize) -> R<()> {
        if self.tracks[t].room.is_none() {
            let n = 16256 / if self.mode()? == 0 { 18 } else { 16 };
            if self.rooms + n > ROOM {
                return Err(ALLOC);
            }
            self.rooms += n;
            self.tracks[t].room = Some(n);
            self.order.push(t);
        }
        Ok(())
    }

    // A macro's number: digits, or a letter given an offset by #macro_offset and digits.
    fn macro_no(&self, c: &mut Cur) -> R<usize> {
        let mut base = 0;
        if let Some(l @ b'a'..=b'z') = c.peek() {
            c.i += 1;
            base = i32::from(self.macro_offset[usize::from(l - b'a')]);
        }
        let n = base + c.num_sp(0, 255)?;
        if n > 255 {
            return Err(MISSING);
        }
        Ok(n as usize)
    }

    // A `#` directive; true at `#end`. `lines` turns a place in the text into a line.
    fn directive(&mut self, body: &[u8], at: &dyn Fn(usize) -> usize) -> Result<bool, (usize, &'static str)> {
        let mut c = Cur::new(body);
        let here = |c: &Cur, e: &'static str| (at(c.i), e);
        let mut name = Vec::new();
        while let Some(&x) = c.b.get(c.i) {
            if x.is_ascii_alphanumeric() || x == b'_' {
                name.push(x.to_ascii_lowercase());
                c.i += 1;
            } else {
                break;
            }
        }
        if name != b"opll_mode" && self.mode.is_none() {
            return Err(here(&c, SYNTAX));
        }
        let syn = |c: &Cur| here(c, SYNTAX);
        // A directive's number: anything wrong is a syntax error.
        let dnum = |c: &mut Cur, lo: i64, hi: i64| c.num_sp(lo, hi).map_err(|_| (at(c.i), SYNTAX));
        match &name[..] {
            b"opll_mode" => self.mode = Some(u8::from(dnum(&mut c, 0, 255)? != 0)),
            b"machine_id" => self.machine = dnum(&mut c, 0, 7)? as u8,
            b"lfo_mode" => self.lfo = dnum(&mut c, 0, 1)? as u8,
            b"tempo" => {
                let t = dnum(&mut c, 57, 2047)? as u16;
                if self.tempo.is_some() {
                    return Err(syn(&c));
                }
                self.tempo = Some(t);
                self.now = t;
            }
            b"title" => {
                let mut t = Vec::new();
                let braced = c.eat(b'{');
                loop {
                    if braced && c.eat(b'}') {
                        break;
                    }
                    if !c.eat(b'"') {
                        return Err(syn(&c));
                    }
                    loop {
                        let x = *c.b.get(c.i).ok_or_else(|| syn(&c))?;
                        if x == b'\n' {
                            return Err(syn(&c));
                        }
                        c.i += 1;
                        if x == b'"' {
                            if c.b.get(c.i) == Some(&b'"') {
                                c.i += 1;
                                t.push(b'"');
                                continue;
                            }
                            break;
                        }
                        t.push(x);
                    }
                    t.extend_from_slice(b"\r\n");
                    if t.len() > TITLE_ROOM {
                        return Err(here(&c, TITLE));
                    }
                    if !braced {
                        break;
                    }
                }
                self.title = Some(t);
            }
            b"alloc" => {
                let braced = c.eat(b'{');
                loop {
                    let x = c.next().ok_or_else(|| syn(&c))?;
                    let t = if x == b'0' { 0 } else { track_of(x, self.mode.unwrap_or(0)).ok_or_else(|| syn(&c))? };
                    if !c.eat(b'=') {
                        return Err(syn(&c));
                    }
                    let n = c.num_sp(1, 65535).map_err(|_| here(&c, MISSING))? as usize;
                    if self.tracks[t].room.is_some() || self.rooms + n > ROOM {
                        return Err(here(&c, ALLOC));
                    }
                    self.rooms += n;
                    self.tracks[t].room = Some(n);
                    self.order.push(t);
                    if !braced || c.eat(b'}') {
                        break;
                    }
                    if !c.eat(b',') {
                        return Err(syn(&c));
                    }
                }
            }
            b"psg_tune" | b"opll_tune" => {
                if !c.eat(b'{') {
                    return Err(syn(&c));
                }
                let mut d = vec![if name == b"psg_tune" { 4 } else { 5 }];
                for k in 0..12 {
                    if k > 0 && !c.eat(b',') {
                        return Err(syn(&c));
                    }
                    let n = dnum(&mut c, 0, 65535)? as u16;
                    d.extend_from_slice(&n.to_le_bytes());
                }
                if !c.eat(b'}') {
                    return Err(syn(&c));
                }
                self.define(&d).map_err(|e| here(&c, e))?;
            }
            b"macro_offset" => {
                let braced = c.eat(b'{');
                loop {
                    let l = c.next().ok_or_else(|| syn(&c))?;
                    if !l.is_ascii_lowercase() || !c.eat(b'=') {
                        return Err(syn(&c));
                    }
                    self.macro_offset[usize::from(l - b'a')] = dnum(&mut c, 0, 255)? as u16;
                    if !braced || c.eat(b'}') {
                        break;
                    }
                    if !c.eat(b',') {
                        return Err(syn(&c));
                    }
                }
            }
            b"disenable_mgsrc" => self.protect = true,
            // Playing, saving and the track report are MGSC's own business, not the data's.
            b"play_start" | b"no_mgs" | b"track_status" => {}
            // Not checked by MGSC.
            b"play_track" => {}
            b"end" => return Ok(true),
            _ => return Err(syn(&c)),
        }
        Ok(false)
    }

    // Something for track 0.
    fn define(&mut self, d: &[u8]) -> R<()> {
        self.ensure_room(0)?;
        let t = &mut self.tracks[0];
        t.data.extend_from_slice(d);
        if t.data.len() > t.room.unwrap_or(0) {
            return Err(FULL);
        }
        Ok(())
    }

    // `@e`, `@r`, `@s`, `@v`, `@#` and `@m` at a line's start: written to track 0.
    fn definition(&mut self, c: &mut Cur) -> R<()> {
        self.mode()?;
        let what = c.next().ok_or(SYNTAX)?;
        let mut d = Vec::new();
        if what == b'm' {
            c.skip();
            let n = match c.digits() {
                Some(n) => n,
                None => i64::from(self.last_m_def) + 1,
            };
            if !(0..=127).contains(&n) {
                return Err(INVALID);
            }
            self.last_m_def = n as i32;
            if !c.eat(b'=') || !c.eat(b'{') {
                return Err(SYNTAX);
            }
            if !c.eat(b'"') {
                return Err(MISSING);
            }
            d.extend_from_slice(&[6, n as u8]);
            loop {
                let x = *c.b.get(c.i).ok_or(MISSING)?;
                c.i += 1;
                match x {
                    b'\n' => return Err(MISSING),
                    b'"' => break,
                    b'\\' => {
                        let e = *c.b.get(c.i).ok_or(MISSING)?;
                        c.i += 1;
                        match e {
                            b'\\' | b'"' => d.push(e),
                            b'x' | b'X' => {
                                let h = c.hex().ok_or(MISSING)?;
                                let l = c.hex().ok_or(MISSING)?;
                                d.push(h << 4 | l);
                            }
                            _ => {
                                let u = e.to_ascii_uppercase();
                                if !(0x41..0x60).contains(&u) {
                                    return Err(MISSING);
                                }
                                d.push(u - 0x40);
                            }
                        }
                    }
                    _ if is_lead(x) => {
                        d.push(x);
                        d.push(*c.b.get(c.i).ok_or(MISSING)?);
                        c.i += 1;
                    }
                    _ => d.push(x),
                }
            }
            d.push(0);
            if !c.eat(b'}') {
                return Err(SYNTAX);
            }
            return self.define(&d);
        }
        let n = c.num_sp(0, 31)? as u8;
        if !c.eat(b'=') {
            return Err(SYNTAX);
        }
        if what == b'#' {
            let m = c.num_sp(0, 15)? as u8;
            return self.define(&[1, n, m]);
        }
        if !c.eat(b'{') {
            return Err(SYNTAX);
        }
        match what {
            b'e' | b'r' => {
                // Mode and noise, either left empty for 1 and 0.
                let mode = if c.peek() == Some(b',') { 1 } else { c.num_sp(0, 3)? };
                if !c.eat(b',') {
                    return Err(MISSING);
                }
                let noise = if c.peek() == Some(b',') { 0 } else { c.num_sp(0, 31)? };
                if !c.eat(b',') {
                    return Err(MISSING);
                }
                let mn = (mode << 5 | noise) as u8;
                if what == b'r' {
                    d.extend_from_slice(&[2, n, 0x80 | mn]);
                    for k in 0..6 {
                        if k > 0 && !c.eat(b',') {
                            return Err(MISSING);
                        }
                        d.push(c.num_sp(0, 255)? as u8);
                    }
                } else {
                    let data = env_data(c)?;
                    if data.is_empty() {
                        return Err(SYNTAX);
                    }
                    if data.len() > 255 {
                        return Err(INVALID);
                    }
                    d.extend_from_slice(&[2, n, mn, data.len() as u8]);
                    d.extend_from_slice(&data);
                }
            }
            b's' => {
                d.extend_from_slice(&[3, n]);
                for _ in 0..32 {
                    let h = c.hex().ok_or(INVALID)?;
                    let l = c.hex().ok_or(INVALID)?;
                    d.push(h << 4 | l);
                }
                if c.peek() != Some(b'}') {
                    return Err(INVALID);
                }
            }
            b'v' => {
                // _VSETC's order: TL FB, then for each operator AR DR SL RR KL MT AM VB EG KR DT.
                let mut v = [0i32; 24];
                const HI: [i64; 11] = [15, 15, 15, 15, 3, 15, 1, 1, 1, 1, 1];
                for (k, x) in v.iter_mut().enumerate() {
                    if k > 0 && !c.eat(b',') {
                        return Err(MISSING);
                    }
                    let hi = match k {
                        0 => 63,
                        1 => 7,
                        _ => HI[(k - 2) % 11],
                    };
                    *x = c.num_sp(0, hi)?;
                }
                let op = |o: usize| &v[2 + o * 11..13 + o * 11];
                let (m, k) = (op(0), op(1));
                let b = |o: &[i32]| (o[6] << 7 | o[7] << 6 | o[8] << 5 | o[9] << 4 | o[5]) as u8;
                d.extend_from_slice(&[
                    0,
                    n,
                    b(m),
                    b(k),
                    (m[4] << 6 | v[0]) as u8,
                    (k[4] << 6 | k[10] << 4 | m[10] << 3 | v[1]) as u8,
                    (m[0] << 4 | m[1]) as u8,
                    (k[0] << 4 | k[1]) as u8,
                    (m[2] << 4 | m[3]) as u8,
                    (k[2] << 4 | k[3]) as u8,
                ]);
            }
            _ => return Err(SYNTAX),
        }
        if !c.eat(b'}') {
            return Err(SYNTAX);
        }
        self.define(&d)
    }

    fn mml(&mut self, t: usize, body: &[u8], depth: usize) -> R<()> {
        let k = kind(t, self.mode()?);
        let mut c = Cur::new(body);
        while !c.end() {
            if self.tracks[t].off {
                // A line of a track after `!` is skipped; but back in a macro's text after a macro that ran
                // `!`, MGSC still follows the macro calls (and nothing else).
                if depth == 0 {
                    return Ok(());
                }
                if c.next() == Some(b'*') {
                    self.call(t, &mut c, depth)?;
                }
                continue;
            }
            if c.peek() == Some(b'!') {
                // The rest of this text is skipped, and the track's MML from here on.
                self.tracks[t].off = true;
                return Ok(());
            }
            self.command(t, k, &mut c, depth)?;
        }
        Ok(())
    }

    fn call(&mut self, t: usize, c: &mut Cur, depth: usize) -> R<()> {
        if depth >= 8 {
            return Err(NESTING);
        }
        let n = self.macro_no(c)?;
        let body = match self.macros[n].clone() {
            Some(b) => b,
            // Followed after `!` (see `mml`), a macro not defined is passed over.
            None if self.tracks[t].off => return Ok(()),
            None => return Err(UNDEFINED),
        };
        self.mml(t, &body, depth + 1)?;
        // A macro called from a macro: MGSC loses its place when the inner one ends.
        if depth >= 1 && !self.tracks[t].off {
            return Err(BAD);
        }
        Ok(())
    }

    fn put(&mut self, t: usize, b: &[u8]) -> R<()> {
        let tr = &mut self.tracks[t];
        tr.data.extend_from_slice(b);
        if tr.data.len() > tr.room.unwrap_or(0) {
            return Err(FULL);
        }
        Ok(())
    }

    // One length: digits (1/n of 192 steps), `%` steps or nothing (`l`'s), then dots; checked against the
    // tempo last set. `need`: nothing isn't allowed.
    fn part(&self, t: usize, c: &mut Cur, need: bool) -> R<i32> {
        let whole = 14400 / i64::from(self.now);
        let mut n = if c.eat(b'%') {
            let n = c.digits().ok_or(MISSING)?;
            let least = 192 / whole;
            if !(1..=256).contains(&n) || least == 0 || n < least {
                return Err(LENGTH);
            }
            if c.peek() == Some(b'.') {
                return Err(BAD);
            }
            n
        } else if c.peek().is_some_and(|x| x.is_ascii_digit()) {
            let d = c.digits().unwrap_or(0);
            if d < 1 || d > 192 || d > whole {
                return Err(LENGTH);
            }
            192 / d
        } else if need {
            return Err(MISSING);
        } else {
            i64::from(self.tracks[t].l)
        };
        let mut add = n;
        while c.eat(b'.') {
            add /= 2;
            n += add;
        }
        Ok(n as i32)
    }

    // A note's length, with its `^` ties: 1 to 256 steps.
    fn length(&self, t: usize, c: &mut Cur) -> R<u16> {
        let mut n = self.part(t, c, false)?;
        while c.eat(b'^') {
            n += self.part(t, c, false)?;
        }
        if !(1..=256).contains(&n) {
            return Err(LENGTH);
        }
        Ok(n as u16)
    }

    // A note or rest: the short form at `l`'s length, else the length byte (0 for 256).
    fn sound(&mut self, t: usize, op: u8, len: u16) -> R<()> {
        if len == self.tracks[t].l {
            self.put(t, &[op | 0x10])
        } else {
            self.put(t, &[op, len as u8])
        }
    }

    fn command(&mut self, t: usize, k: Kind, c: &mut Cur, depth: usize) -> R<()> {
        let x = c.next().ok_or(BAD)?;
        let rhythm = k == Kind::Rhythm;
        let (psg, scc, fm) = (k == Kind::Psg, k == Kind::Scc, k == Kind::Fm);
        // Values the PSG and SCC take the other way round.
        let sign = if psg || scc { -1 } else { 1 };
        match x {
            b'b' | b's' | b'm' | b'c' | b'h' | b'r' if rhythm => {
                let bit = |x: u8| match x {
                    b'b' => 0x10,
                    b's' => 0x08,
                    b'm' => 0x04,
                    b'c' => 0x02,
                    b'h' => 0x01,
                    _ => 0,
                };
                let mut bits = bit(x);
                while let Some(y @ (b'b' | b's' | b'm' | b'c' | b'h' | b'r')) = c.peek() {
                    c.i += 1;
                    bits |= bit(y);
                }
                let len = match c.peek() {
                    Some(b':') => {
                        c.i += 1;
                        self.tracks[t].l
                    }
                    None => self.tracks[t].l,
                    Some(b'0'..=b'9' | b'%' | b'.' | b'^') => self.length(t, c)?,
                    _ => return Err(BAD),
                };
                if len == self.tracks[t].l {
                    self.put(t, &[0xa0 | bits])?;
                } else {
                    self.put(t, &[0x20 | bits, len as u8])?;
                }
            }
            b'r' => {
                let len = self.length(t, c)?;
                self.sound(t, 0x2c, len)?;
            }
            b'a'..=b'g' if !rhythm => {
                let base = [9u8, 11, 0, 2, 4, 5, 7][usize::from(x - b'a')];
                let mut n = base;
                match c.peek() {
                    Some(b'+') => {
                        c.i += 1;
                        if base != 4 && base != 11 {
                            n += 1;
                        }
                    }
                    Some(b'-') => {
                        c.i += 1;
                        n = (n + 11) % 12;
                    }
                    _ => {}
                }
                if matches!(c.peek(), Some(b'+' | b'-')) {
                    return Err(BAD);
                }
                let target = std::mem::take(&mut self.tracks[t].porta);
                if c.eat(b'_') {
                    if target {
                        return Err(BAD);
                    }
                    self.tracks[t].porta = true;
                    self.put(t, &[0x53, n])?;
                    // Only octaves may come before its note, on the same line; an `o` here is one higher.
                    loop {
                        match c.peek() {
                            Some(b'o') => {
                                c.i += 1;
                                let n = c.num(1, 8)? as u8;
                                self.put(t, &[0xd0 | n])?;
                            }
                            Some(b'>') => {
                                c.i += 1;
                                self.put(t, &[0x4e])?;
                            }
                            Some(b'<') => {
                                c.i += 1;
                                self.put(t, &[0x4f])?;
                            }
                            Some(b'a'..=b'g') => return Ok(()),
                            Some(_) => return Err(BAD),
                            None => return Err(MISSING),
                        }
                    }
                }
                let len = self.length(t, c)?;
                self.sound(t, 0x20 | n, len)?;
            }
            b'&' => self.put(t, &[0x40])?,
            b't' => {
                let n = c.digits().ok_or(MISSING)?;
                if n > 2047 {
                    return Err(INVALID);
                }
                if n < 57 {
                    return Err(LENGTH);
                }
                let n = n as u16;
                self.now = n;
                self.put(t, &[0x41])?;
                self.put(t, &n.to_le_bytes())?;
                self.put(t, &tempo_table(n))?;
            }
            b'l' => {
                let n = self.part(t, c, true)?;
                if !(1..=256).contains(&n) {
                    return Err(LENGTH);
                }
                self.put(t, &[0x42, n as u8])?;
                self.tracks[t].l = n as u16;
            }
            b'q' if !rhythm => {
                let n = c.num(0, 8)? as u8;
                self.put(t, &[0x44, n])?;
            }
            b'v' => {
                if rhythm {
                    if let Some(i) = c.raw().and_then(|y| b"bsmch".iter().position(|&z| z == y)) {
                        c.i += 1;
                        let i = (i as u8) << 5;
                        if matches!(c.raw(), Some(b'+' | b'-')) {
                            let n = c.signed(-15, 15)?;
                            return self.put(t, &[0x47, i | (n as u8 & 0x1f)]);
                        }
                        let n = c.num(0, 15)? as u8;
                        return self.put(t, &[0x45, i | n]);
                    }
                }
                if matches!(c.raw(), Some(b'+' | b'-')) {
                    let n = c.signed(-15, 15).map_err(|e| if e == MISSING { BAD } else { e })?;
                    self.put(t, &[0x46, n as u8])?;
                } else {
                    // No number is 8 (what follows is the next command).
                    c.skip();
                    let n = match c.digits() {
                        Some(n) if n > 15 => return Err(INVALID),
                        Some(n) => n as u8,
                        None => 8,
                    };
                    self.put(t, &[0xc0 | n])?;
                }
            }
            b')' | b'(' => {
                let n = match c.digits() {
                    Some(n) if n > 15 => return Err(INVALID),
                    Some(n) => n as i32,
                    None => 1,
                };
                let d = if x == b')' { n } else { -n };
                self.put(t, &[0x46, d as u8])?;
            }
            b'k' if rhythm => {
                if c.eat_raw(b'o') {
                    self.put(t, &[0x5a, 0])?;
                } else if c.eat_raw(b'f') {
                    self.put(t, &[0x5a, 1])?;
                } else {
                    return Err(if c.digit_raw() { INVALID } else { MISSING });
                }
            }
            b'k' if !fm => {
                let n = c.num(0, 255)? as u8;
                self.put(t, &[0x48, n])?;
            }
            b'@' => {
                let y = c.raw();
                match y {
                    Some(b'e' | b'r') if !rhythm => {
                        c.i += 1;
                        let n = c.num(0, 31)? as u8;
                        if psg {
                            self.put(t, &[0x80 | n])?;
                        } else {
                            self.put(t, &[0x49, n])?;
                        }
                    }
                    Some(b'\\') if !rhythm => {
                        c.i += 1;
                        if fm {
                            let n = c.num(0, 255)? as u8;
                            self.put(t, &[0x5f, n])?;
                        } else {
                            // No number is 0 here.
                            c.skip();
                            let n = if matches!(c.raw(), Some(b'0'..=b'9' | b'+' | b'-')) { c.signed(-32768, 32767)? as i16 } else { 0 };
                            self.put(t, &[0x51])?;
                            self.put(t, &n.to_le_bytes())?;
                        }
                    }
                    Some(b'p') if psg || scc => {
                        c.i += 1;
                        let n = c.signed_sp(-32768, 32767)? as i16;
                        self.put(t, &[0x55])?;
                        self.put(t, &n.to_le_bytes())?;
                    }
                    Some(b'l') if fm || rhythm => {
                        c.i += 1;
                        let n = c.num(0, 255)? as u8;
                        self.put(t, &[0x5c, 2, n])?;
                    }
                    Some(b'm') => {
                        c.i += 1;
                        let n = match c.digits() {
                            Some(n) => n,
                            None => i64::from(self.last_m) + 1,
                        };
                        if !(0..=127).contains(&n) {
                            return Err(INVALID);
                        }
                        self.last_m = n as i32;
                        self.put(t, &[0x60, n as u8])?;
                    }
                    Some(b'o') => {
                        c.i += 1;
                        let n = c.num(0, 255)? as u8;
                        self.put(t, &[0x61, n])?;
                    }
                    Some(b'f') => {
                        c.i += 1;
                        self.put(t, &[0x63])?;
                    }
                    _ if !rhythm && c.peek().is_some_and(|x| x.is_ascii_digit()) => {
                        let n = c.num(0, 31)? as u8;
                        self.put(t, &[0x80 | n])?;
                    }
                    _ => return Err(BAD),
                }
            }
            b'm' if psg => {
                let n = c.num(1, 65535)? as u16;
                self.put(t, &[0x4c])?;
                self.put(t, &n.to_le_bytes())?;
            }
            b's' if fm => {
                if c.eat_raw(b'o') {
                    self.put(t, &[0x5b, 1])?;
                } else if c.eat_raw(b'f') {
                    self.put(t, &[0x5b, 0])?;
                } else {
                    return Err(MISSING);
                }
            }
            b's' if psg => {
                let n = c.num(0, 15)? as u8;
                self.put(t, &[0x4b, n])?;
            }
            b'n' if psg => {
                let n = c.num(0, 31)? as u8;
                self.put(t, &[0x4d, n])?;
            }
            b'o' if !rhythm => {
                let n = c.num(1, 8)? as u8;
                self.put(t, &[0xd0 | (n - 1)])?;
            }
            b'>' if !rhythm => self.put(t, &[0x4e])?,
            b'<' if !rhythm => self.put(t, &[0x4f])?,
            b'\\' if !rhythm => {
                let n = c.signed_sp(-127, 127)?;
                self.put(t, &[0x50, (n * sign) as u8])?;
            }
            b'p' if psg || scc => {
                let n = c.num(0, 255)? as u8;
                self.put(t, &[0x52, n])?;
            }
            b'h' if !rhythm => {
                if c.eat_raw(b'o') {
                    self.put(t, &[0x5a, 1])?;
                } else if c.eat_raw(b'f') {
                    self.put(t, &[0x5a, 0])?;
                } else if c.eat_raw(b'i') {
                    self.put(t, &[0x64])?;
                } else {
                    let d = c.num_sp(0, 255).map_err(|_| INVALID)?;
                    let comma = |c: &mut Cur| if c.eat_raw(b',') { Ok(()) } else { Err(MISSING) };
                    comma(c)?;
                    let depth = c.num(0, 255)?;
                    comma(c)?;
                    let speed = c.num(0, 255)?;
                    comma(c)?;
                    let coarse = c.signed(-127, 127)?;
                    self.put(t, &[0x54, (d + 1) as u8, depth as u8, (speed + 1) as u8, (coarse * sign) as u8])?;
                }
            }
            b'[' => {
                // The count here, or after the `]`; 2 if neither, 0 for ever.
                let n = c.digits();
                if n.is_some_and(|n| n > 255) {
                    return Err(INVALID);
                }
                if self.tracks[t].loops.len() >= 16 {
                    return Err(REPEAT);
                }
                let at = self.tracks[t].data.len();
                self.tracks[t].loops.push((at, n.is_some(), None));
                self.put(t, &[0x57, n.unwrap_or(2) as u8, 0, 0])?;
            }
            b'|' => {
                let at = self.tracks[t].data.len();
                self.tracks[t].loops.last_mut().ok_or(REPEAT)?.2 = Some(at);
                self.put(t, &[0x58, 0, 0])?;
            }
            b']' => {
                let (open, counted, brk) = self.tracks[t].loops.pop().ok_or(REPEAT)?;
                // Its count, unless it's 2 (which is what none means), replaces the `[`'s.
                if let Some(n) = c.digits() {
                    if n > 255 {
                        return Err(INVALID);
                    }
                    if n != 2 {
                        self.tracks[t].data[open + 1] = n as u8;
                    }
                }
                let _ = counted;
                let q = self.tracks[t].data.len();
                let d = &mut self.tracks[t].data;
                d[open + 2..open + 4].copy_from_slice(&((q - open - 3) as u16).to_le_bytes());
                if let Some(b) = brk {
                    d[b + 1..b + 3].copy_from_slice(&((q - b - 2) as u16).to_le_bytes());
                }
                let back = (open as i32 + 1 - q as i32) as u16;
                self.put(t, &[0x59, 0])?;
                self.put(t, &back.to_le_bytes())?;
            }
            b'/' if psg => {
                let n = c.num(0, 3)? as u8;
                self.put(t, &[0xe0 | n])?;
            }
            b'/' if fm => {
                if c.digit_raw() {
                    return Err(BAD);
                }
                self.put(t, &[0x4a])?;
            }
            b'y' if !scc => {
                let r = c.num(0, if psg { 15 } else { 56 })? as u8;
                if !c.eat_raw(b',') {
                    return Err(MISSING);
                }
                let d = c.num(0, 255)? as u8;
                self.put(t, &[0x5c, r, d])?;
            }
            b'*' => self.call(t, c, depth)?,
            b'!' => self.tracks[t].off = true,
            b'$' => self.put(t, &[0x5d])?,
            _ => return Err(BAD),
        }
        Ok(())
    }
}

// An @e envelope's commands.
fn env_data(c: &mut Cur) -> R<Vec<u8>> {
    let mut d = Vec::new();
    // A count after `:` or `=`: a byte, but not 1 or 240.
    let count = |c: &mut Cur| -> R<u8> {
        let n = c.digits().ok_or(MISSING)?;
        if n == 1 || n == 240 || n > 255 {
            return Err(INVALID);
        }
        Ok(n as u8)
    };
    loop {
        let x = match c.peek() {
            None => return Err(MISSING),
            Some(b'}') => return Ok(d),
            Some(x) => x,
        };
        c.i += 1;
        match x {
            b'.' | b',' => {}
            b'0'..=b'9' | b'a'..=b'f' => {
                let v = (x as char).to_digit(16).unwrap() as u8;
                if c.eat_raw(b':') {
                    d.extend_from_slice(&[0xe0 | v, count(c)?]);
                } else if c.eat_raw(b'=') {
                    d.extend_from_slice(&[0x20 | v, count(c)?]);
                } else {
                    d.push(v);
                }
            }
            b'n' => d.push(0x80 | c.num(0, 31)? as u8),
            b'/' | b'*' => d.push(0xa0 | c.num(0, 3)? as u8),
            b'[' => d.push(0x40),
            b']' => d.push(0x60),
            b'@' => d.extend_from_slice(&[0x10, c.num(0, 31)? as u8]),
            b'y' => {
                let r = c.num(0, 255)? as u8;
                if !c.eat_raw(b',') {
                    return Err(MISSING);
                }
                d.extend_from_slice(&[0x11, r, c.num(0, 255)? as u8]);
            }
            b'\\' => d.extend_from_slice(&[0x12, c.signed(-127, 127)? as u8]),
            _ => return Err(INVALID),
        }
    }
}

/// MGSC: the MML source's bytes (Shift JIS) in, the `.mgs` file out.
pub fn compile(src: &[u8]) -> Result<Vec<u8>, Error> {
    // Each line as MGSC reads it: to its 255th byte, the comment off.
    let mut lines: Vec<&[u8]> = src
        .split(|&b| b == b'\n')
        .map(|l| {
            let l = l.strip_suffix(b"\r").unwrap_or(l);
            let l = &l[..l.len().min(LINE)];
            l.split(|&b| b == b';').next().unwrap_or(l)
        })
        .collect();
    if src.ends_with(b"\n") {
        lines.pop();
    }
    let mut cc = Compiler::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let start = i;
        let err = |what| Error { line: Some(start), what };
        let lead = line.iter().position(|&b| !is_ws(b)).unwrap_or(line.len());
        let line = &line[lead..];
        let first = line.first().copied();
        // A definition or directive with `{`: everything to its `}`, and a place in it back to its line.
        let block = |from: &[u8], i: &mut usize| -> (Vec<u8>, Vec<usize>) {
            let mut b = from.to_vec();
            let mut breaks = Vec::new();
            if from.contains(&b'{') && !brace_closed(from) {
                while *i + 1 < lines.len() {
                    *i += 1;
                    breaks.push(b.len());
                    b.push(b'\n');
                    b.extend_from_slice(lines[*i]);
                    if brace_closed(&b) {
                        break;
                    }
                }
            }
            (b, breaks)
        };
        match first {
            Some(b'#') => {
                let (body, breaks) = block(&line[1..], &mut i);
                let at = |p: usize| start + breaks.iter().filter(|&&b| b < p).count();
                match cc.directive(&body, &at) {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err((line, what)) => return Err(Error { line: Some(line), what }),
                }
            }
            Some(b'@') => {
                let (body, breaks) = block(&line[1..], &mut i);
                let mut c = Cur::new(&body);
                if let Err(what) = cc.definition(&mut c) {
                    // Run out of text with no `}`: MGSC has read to the end of the file.
                    // (An `@m` string ends with its line.)
                    let text = body.first().is_some_and(|b| b.eq_ignore_ascii_case(&b'm'));
                    if c.i >= body.len() && !brace_closed(&body) && !text {
                        return Err(Error {
                            line: Some(lines.len() + 1),
                            what: MISSING,
                        });
                    }
                    let line = start + breaks.iter().filter(|&&b| b < c.i).count();
                    return Err(Error { line: Some(line), what });
                }
            }
            Some(b'*') => {
                cc.mode().map_err(err)?;
                // A macro defined again is refused on its first line.
                let mut c = Cur::new(&line[1..]);
                if let Ok(n) = cc.macro_no(&mut c) {
                    if cc.macros[n].is_some() {
                        return Err(err(REDEFINED));
                    }
                }
                let (body, _) = block(&line[1..], &mut i);
                if !brace_closed(&body) {
                    return Err(Error {
                        line: Some(lines.len() + 1),
                        what: MISSING,
                    });
                }
                let mut c = Cur::new(&body);
                let n = cc.macro_no(&mut c).map_err(err)?;
                if !c.eat(b'=') || !c.eat(b'{') {
                    return Err(err(SYNTAX));
                }
                let end = body.iter().rposition(|&b| b == b'}').ok_or(err(SYNTAX))?;
                if cc.macros[n].is_some() {
                    return Err(Error { line: Some(i), what: REDEFINED });
                }
                cc.macros[n] = Some(body[c.i..end].to_vec());
            }
            None => {}
            Some(_) => {
                let mode = cc.mode().map_err(err)?;
                let n = line.iter().position(|&b| b == b' ' || b == b'\t').ok_or(err(SYNTAX))?;
                let ts: Vec<usize> = line[..n].iter().map(|&x| track_of(x, mode).ok_or(err(TRACK))).collect::<Result<_, _>>()?;
                for t in ts {
                    cc.ensure_room(t).map_err(err)?;
                    cc.mml(t, &line[n..], 0).map_err(err)?;
                }
            }
        }
        i += 1;
    }
    let mode = cc.mode().map_err(|what| Error {
        line: Some(lines.len().saturating_sub(1)),
        what,
    })?;
    let title = cc.title.clone().unwrap_or_default();
    if title.len() + cc.rooms > OBJECT {
        return Err(Error { line: None, what: BIG });
    }
    let mut out = b"MGS313\r\n".to_vec();
    // A line of 255 bytes or more: MGSC's line buffer ends on the file's first byte.
    if src.split(|&b| b == b'\n').any(|l| l.strip_suffix(b"\r").unwrap_or(l).len() >= LINE) {
        out[0] = 0;
    }
    out.extend_from_slice(&title);
    out.push(0x1a);
    let head = out.len();
    out.push(0);
    out.push(mode | cc.lfo << 1 | cc.machine << 2 | u8::from(cc.protect) << 6);
    // No #tempo: 0, which the driver plays at 120.
    out.extend_from_slice(&cc.tempo.unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&[0; 38]);
    // Each track ends with FF, but a full one with the byte after its room: the next room's first, or after
    // the last room the file's own first byte.
    let mut ends = [0xffu8; 18];
    for (k, &t) in cc.order.iter().enumerate().rev() {
        let tr = &cc.tracks[t];
        if tr.data.len() == tr.room.unwrap_or(0) {
            ends[t] = match cc.order.get(k + 1) {
                Some(&u) => cc.tracks[u].data.first().copied().unwrap_or(ends[u]),
                None => out[0],
            };
        }
    }
    for (t, tr) in cc.tracks.iter_mut().enumerate() {
        if tr.room.is_none() {
            continue;
        }
        tr.data.push(ends[t]);
        let off = (out.len() - head) as u16;
        out[head + 4 + t * 2..head + 6 + t * 2].copy_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&tr.data);
    }
    out.resize(out.len().div_ceil(128) * 128, 0);
    Ok(out)
}

// Whether the text's first `{` (outside quotes) is closed: by the first `}` after it (MGSC doesn't count
// braces inside).
fn brace_closed(b: &[u8]) -> bool {
    let mut q = false;
    let mut seen = false;
    for &c in b {
        match c {
            b'"' => q = !q,
            b'{' if !q => seen = true,
            b'}' if !q && seen => return true,
            _ => {}
        }
    }
    !seen
}

// ---- The way back ----

const NOTES: [&str; 12] = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"];

// One length MGSC takes at tempo `now`: a note of 1/n or `%` steps.
fn len_part(v: u16, now: u16) -> Option<String> {
    let whole = 14400 / u32::from(now);
    let v32 = u32::from(v);
    let top = whole.min(192);
    if 192 % v32 == 0 && 192 / v32 <= top {
        return Some(format!("{}", 192 / v32));
    }
    let least = 192 / whole;
    if least > 0 && v32 >= least && v <= 256 {
        return Some(format!("%{v}"));
    }
    if let Some(d) = (1..=top).find(|&d| 192 / d == v32) {
        return Some(format!("{d}"));
    }
    // Dotted: each dot adds half the last added, rounded down.
    for d in 1..=top {
        let (mut n, mut add, mut dots) = (192 / d, 192 / d, String::new());
        while add > 1 && n < v32 {
            add /= 2;
            n += add;
            dots.push('.');
            if n == v32 {
                return Some(format!("{d}{dots}"));
            }
        }
    }
    None
}

// A note's length, tied from parts if one won't do.
fn len_text(v: u16, now: u16) -> Option<String> {
    if let Some(s) = len_part(v, now) {
        return Some(s);
    }
    // The fewest parts.
    let n = usize::from(v);
    let mut best: Vec<Option<(usize, u16)>> = vec![None; n + 1];
    best[0] = Some((0, 0));
    for x in 1..=n {
        for p in 1..=x as u16 {
            if let (Some((k, _)), Some(_)) = (best[x - usize::from(p)], len_part(p, now)) {
                if best[x].is_none_or(|(b, _)| k + 1 < b) {
                    best[x] = Some((k + 1, p));
                }
            }
        }
    }
    best[n]?;
    let mut parts = Vec::new();
    let mut x = n;
    while x > 0 {
        let (_, p) = best[x]?;
        parts.push(len_part(p, now)?);
        x -= usize::from(p);
    }
    Some(parts.join("^"))
}

// A track's commands as MML; None for anything MGSC 1.11 can't have written. `now` is the tempo MGSC
// would have last been given, as it reads the MML written so far.
fn track_mml(d: &[u8], k: Kind, now: &mut u16) -> Option<(Vec<String>, usize)> {
    let mut out: Vec<String> = Vec::new();
    let mut l: u16 = 48;
    let mut i = 0;
    let (psg, scc, fm, rhythm) = (k == Kind::Psg, k == Kind::Scc, k == Kind::Fm, k == Kind::Rhythm);
    let sign = if psg || scc { -1 } else { 1 };
    let len = |b: u8| if b == 0 { 256 } else { u16::from(b) };
    let mut porta = false;
    loop {
        let c = *d.get(i)?;
        // `_`, its octaves and its note stay on one line.
        let join = porta;
        if porta && !matches!(c, 0xd1..=0xd8 | 0x4e | 0x4f | 0x20..=0x2b | 0x30..=0x3b) {
            return None;
        }
        let a = |n: usize| d.get(i + n).copied();
        let w = |n: usize| Some(i16::from_le_bytes([a(n)?, a(n + 1)?]));
        let (s, n) = match c {
            0xff => break,
            0x20..=0x3f | 0xa0..=0xbf if rhythm => {
                let bits = c & 0x1f;
                let s: String = if bits == 0 {
                    "r".into()
                } else {
                    b"bsmch".iter().enumerate().filter(|&(j, _)| bits & (0x10 >> j) != 0).map(|(_, &x)| x as char).collect()
                };
                if c & 0x80 != 0 {
                    (s + ":", 1)
                } else {
                    let n = len(a(1)?);
                    if n == l {
                        return None;
                    }
                    (s + &len_text(n, *now)?, 2)
                }
            }
            0x20..=0x2c => {
                let n = len(a(1)?);
                if n == l {
                    return None;
                }
                if c != 0x2c {
                    porta = false;
                }
                let name = if c == 0x2c { "r" } else { NOTES[usize::from(c & 15)] };
                (format!("{name}{}", len_text(n, *now)?), 2)
            }
            0x30..=0x3c => {
                if c != 0x3c {
                    porta = false;
                }
                ((if c == 0x3c { "r" } else { NOTES[usize::from(c & 15)] }).to_string(), 1)
            }
            0x40 => ("&".into(), 1),
            0x41 => {
                let t = w(1)? as u16;
                if !(57..=2047).contains(&t) || d.get(i + 3..i + 51)? != tempo_table(t) {
                    return None;
                }
                *now = t;
                (format!("t{t}"), 51)
            }
            0x42 => {
                l = len(a(1)?);
                (format!("l{}", len_part(l, *now)?), 2)
            }
            0x44 if !rhythm && a(1)? <= 8 => (format!("q{}", a(1)?), 2),
            0x45 if rhythm => {
                let x = a(1)?;
                (format!("v{}{}", *b"bsmch".get(usize::from(x >> 5))? as char, x & 15), 2)
            }
            0x47 if rhythm => {
                let x = a(1)?;
                let v = i32::from(((x & 0x1f) << 3) as i8 >> 3);
                if v.abs() > 15 {
                    return None;
                }
                (format!("v{}{v:+}", *b"bsmch".get(usize::from(x >> 5))? as char), 2)
            }
            0x46 => {
                let v = i32::from(a(1)? as i8);
                if v.abs() > 15 {
                    return None;
                }
                (format!("v{v:+}"), 2)
            }
            0x48 if psg || scc => (format!("k{}", a(1)?), 2),
            0x49 if (scc || fm) && a(1)? < 32 => (format!("@e{}", a(1)?), 2),
            0x4a if fm => ("/".into(), 1),
            0x4b if psg && a(1)? < 16 => (format!("s{}", a(1)?), 2),
            0x4c if psg && w(1)? != 0 => (format!("m{}", w(1)? as u16), 3),
            0x4d if psg && a(1)? < 32 => (format!("n{}", a(1)?), 2),
            0x4e if !rhythm => (">".into(), 1),
            0x4f if !rhythm => ("<".into(), 1),
            0x50 if !rhythm => {
                let v = i32::from(a(1)? as i8) * sign;
                if v.abs() > 127 {
                    return None;
                }
                (format!("\\{v}"), 2)
            }
            0x51 if psg || scc => (format!("@\\{}", w(1)?), 3),
            0x52 if psg || scc => (format!("p{}", a(1)?), 2),
            0x53 if !rhythm && a(1)? < 12 && !porta => {
                porta = true;
                (format!("{}_", NOTES[usize::from(a(1)?)]), 2)
            }
            0x54 if !rhythm => {
                let (dl, dp, sp, co) = (a(1)?, a(2)?, a(3)?, i32::from(a(4)? as i8) * sign);
                // Delay and speed are written plus 1, so 255 is 0.
                if co.abs() > 127 {
                    return None;
                }
                (format!("h{},{dp},{},{co}", dl.wrapping_sub(1), sp.wrapping_sub(1)), 5)
            }
            0x55 if psg || scc => (format!("@p{}", w(1)?), 3),
            0x57 => (format!("[{}", a(1)?), 4),
            0x58 => ("|".into(), 3),
            0x59 => ("]".into(), 4),
            0x5a => {
                let on = a(1)?;
                let s = match (rhythm, on) {
                    (true, 0) => "ko",
                    (true, 1) => "kf",
                    (false, 0) => "hf",
                    (false, 1) => "ho",
                    _ => return None,
                };
                (s.into(), 2)
            }
            0x5b if fm && a(1)? <= 1 => ((if a(1)? == 1 { "so" } else { "sf" }).into(), 2),
            0x5c if !scc && a(1)? <= if psg { 15 } else { 56 } => (format!("y{},{}", a(1)?, a(2)?), 3),
            0x5d => ("$".into(), 1),
            0x5f if fm => (format!("@\\{}", a(1)?), 2),
            0x60 if a(1)? < 128 => (format!("@m{}", a(1)?), 2),
            0x61 => (format!("@o{}", a(1)?), 2),
            0x63 => ("@f".into(), 1),
            0x64 if !rhythm => ("hi".into(), 1),
            0x80..=0x9f if !rhythm => (format!("@{}", c & 0x1f), 1),
            0xc0..=0xcf => (format!("v{}", c & 15), 1),
            // Between `_` and its note MGSC writes `o` one higher.
            0xd1..=0xd8 if porta => (format!("o{}", c & 15), 1),
            0xd0..=0xd7 if !rhythm && !porta => (format!("o{}", (c & 7) + 1), 1),
            0xe0..=0xe3 if psg => (format!("/{}", c & 3), 1),
            _ => return None,
        };
        match out.last_mut() {
            Some(last) if join => last.push_str(&s),
            _ => out.push(s),
        }
        i += n;
    }
    Some((out, i))
}

// An @e envelope's commands as text.
fn env_text(d: &[u8]) -> Option<String> {
    let mut s = String::new();
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        let a = |n: usize| d.get(i + n).copied();
        let n = match c {
            0x00..=0x0f => {
                s += &format!("{c:x}");
                1
            }
            0x10 if a(1)? < 32 => {
                s += &format!("@{}.", a(1)?);
                2
            }
            0x11 => {
                s += &format!("y{},{}.", a(1)?, a(2)?);
                3
            }
            0x12 if a(1)? != 0x80 => {
                s += &format!("\\{}.", a(1)? as i8);
                2
            }
            0x20..=0x2f | 0xe0..=0xef if !matches!(a(1)?, 1 | 240) => {
                s += &format!("{:x}{}{}.", c & 15, if c < 0x80 { '=' } else { ':' }, a(1)?);
                2
            }
            0x40 => {
                s.push('[');
                1
            }
            0x60 => {
                s.push(']');
                1
            }
            0x80..=0x9f => {
                s += &format!("n{}.", c & 0x1f);
                1
            }
            0xa0..=0xa3 => {
                s += &format!("/{}.", c & 3);
                1
            }
            _ => return None,
        };
        i += n;
    }
    Some(s)
}

fn quoted(b: &[u8], m: bool) -> Vec<u8> {
    let mut o = vec![b'"'];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if is_lead(c) && i + 1 < b.len() {
            o.extend_from_slice(&b[i..i + 2]);
            i += 2;
            continue;
        }
        match c {
            b'"' if m => o.extend_from_slice(b"\\\""),
            b'"' => o.extend_from_slice(b"\"\""),
            b'\\' if m => o.extend_from_slice(b"\\\\"),
            0..=0x1f | b';' if m => o.extend_from_slice(format!("\\x{c:02x}").as_bytes()),
            _ => o.push(c),
        }
        i += 1;
    }
    o.push(b'"');
    o
}

/// `.mgs` (format "304" and later, MGSARC's packed ones too) back to MGSC's MML, Shift JIS text that
/// `compile` turns into the same data. None for what MGSC 1.11 can't write (older formats, values out of
/// its ranges, a track 0 it doesn't know).
pub fn decompile(mgs: &[u8]) -> Option<Vec<u8>> {
    if !mgs.starts_with(b"MGS") {
        return None;
    }
    let eof = mgs.iter().position(|&b| b == 0x1a)?;
    if mgs.get(eof + 2)? & 0x80 != 0 {
        return decompile(&crate::mgs::uncompress(mgs)?);
    }
    let version: u16 = std::str::from_utf8(mgs.get(3..6)?).ok()?.parse().ok()?;
    if version < 304 {
        return None;
    }
    let bin = &mgs[eof + 1..];
    if bin.len() < 42 || bin[0] != 0 {
        return None;
    }
    let flags = bin[1];
    let mode = flags & 1;
    let tempo = u16::from_le_bytes([bin[2], bin[3]]);
    let offs: Vec<usize> = (0..18).map(|t| usize::from(u16::from_le_bytes([bin[4 + t * 2], bin[5 + t * 2]]))).collect();
    let mut o: Vec<u8> = Vec::new();
    let line = |o: &mut Vec<u8>, s: &[u8]| {
        o.extend_from_slice(s);
        o.extend_from_slice(b"\r\n");
    };
    line(&mut o, format!("#opll_mode {mode}").as_bytes());
    line(&mut o, format!("#machine_id {}", flags >> 2 & 7).as_bytes());
    line(&mut o, format!("#lfo_mode {}", flags >> 1 & 1).as_bytes());
    if flags & 0x40 != 0 {
        line(&mut o, b"#disenable_mgsrc");
    }
    let title = &mgs[8..eof];
    if !title.is_empty() {
        let mut t = b"#title {".to_vec();
        let body = title.strip_suffix(b"\r\n")?;
        let mut from = 0;
        loop {
            let to = body[from..].windows(2).position(|w| w == b"\r\n").map(|k| from + k);
            let l = &body[from..to.unwrap_or(body.len())];
            // A comment can't be in it, and its line must fit MGSC's 255 bytes.
            if l.contains(&b'\n') || l.contains(&b'\r') || l.contains(&b';') || l.len() + l.iter().filter(|&&b| b == b'"').count() > 240 {
                return None;
            }
            t.extend_from_slice(b"\r\n  ");
            t.extend_from_slice(&quoted(l, false));
            match to {
                Some(k) => from = k + 2,
                None => break,
            }
        }
        t.extend_from_slice(b" }");
        line(&mut o, &t);
    }
    if title.len() > TITLE_ROOM {
        return None;
    }
    // The tempo MGSC checks lengths against as it reads.
    let mut now = 120;
    if (57..=2047).contains(&tempo) {
        line(&mut o, format!("#tempo {tempo}").as_bytes());
        now = tempo;
    } else if tempo != 0 {
        return None;
    }
    // Each track's room, just what it holds (MGSC would otherwise give each 16256 / 18): written here
    // once the tracks are read.
    let alloc_at = o.len();
    let mut rooms: Vec<(usize, usize)> = Vec::new();
    // Track 0, in its order.
    if offs[0] != 0 {
        let d = bin.get(offs[0]..)?;
        let mut i = 0;
        loop {
            let a = |n: usize| d.get(i + n).copied();
            match *d.get(i)? {
                0xff => {
                    rooms.push((0, i + 1));
                    break;
                }
                0 => {
                    let v = d.get(i + 2..i + 10)?;
                    if v[3] & 0x20 != 0 || a(1)? > 31 {
                        return None;
                    }
                    let op = |x: u8, ksl: u8, ardr: u8, slrr: u8, dt: u8| {
                        format!(
                            "{},{},{},{},{},{},{},{},{},{},{}",
                            ardr >> 4,
                            ardr & 15,
                            slrr >> 4,
                            slrr & 15,
                            ksl,
                            x & 15,
                            x >> 7,
                            x >> 6 & 1,
                            x >> 5 & 1,
                            x >> 4 & 1,
                            dt
                        )
                    };
                    let s = format!(
                        "@v{} = {{ {},{},\r\n  {},\r\n  {} }}",
                        a(1)?,
                        v[2] & 63,
                        v[3] & 7,
                        op(v[0], v[2] >> 6, v[4], v[6], v[3] >> 3 & 1),
                        op(v[1], v[3] >> 6, v[5], v[7], v[3] >> 4 & 1)
                    );
                    line(&mut o, s.as_bytes());
                    i += 10;
                }
                1 if a(1)? < 32 && a(2)? < 16 => {
                    line(&mut o, format!("@#{} = {}", a(1)?, a(2)?).as_bytes());
                    i += 3;
                }
                2 if a(1)? < 32 => {
                    let mn = a(2)?;
                    let (mode, noise) = (mn >> 5 & 3, mn & 31);
                    if mn & 0x80 != 0 {
                        let p = d.get(i + 3..i + 9)?;
                        line(&mut o, format!("@r{} = {{ {mode},{noise},{},{},{},{},{},{} }}", a(1)?, p[0], p[1], p[2], p[3], p[4], p[5]).as_bytes());
                        i += 9;
                    } else {
                        let n = usize::from(a(3)?);
                        let e = env_text(d.get(i + 4..i + 4 + n)?)?;
                        line(&mut o, format!("@e{} = {{ {mode},{noise},{e} }}", a(1)?).as_bytes());
                        i += 4 + n;
                    }
                }
                3 if a(1)? < 32 => {
                    let w = d.get(i + 2..i + 34)?;
                    let hex: String = w.iter().map(|b| format!("{b:02x}")).collect();
                    line(&mut o, format!("@s{} = {{ {} {} }}", a(1)?, &hex[..32], &hex[32..]).as_bytes());
                    i += 34;
                }
                x @ (4 | 5) => {
                    let v: Vec<String> = (0..12).map(|k| Some(u16::from_le_bytes([a(1 + 2 * k)?, a(2 + 2 * k)?]).to_string())).collect::<Option<_>>()?;
                    line(&mut o, format!("#{}_tune {{ {} }}", if x == 4 { "psg" } else { "opll" }, v.join(",")).as_bytes());
                    i += 25;
                }
                6 if a(1)? < 128 => {
                    let n = d[i + 2..].iter().position(|&b| b == 0)?;
                    let mut s = format!("@m{} = {{ ", a(1)?).into_bytes();
                    s.extend_from_slice(&quoted(&d[i + 2..i + 2 + n], true));
                    s.extend_from_slice(b" }");
                    line(&mut o, &s);
                    i += 3 + n;
                }
                _ => return None,
            }
        }
    }
    for t in 1..18 {
        if offs[t] == 0 {
            continue;
        }
        if mode == 1 && t > 15 {
            return None;
        }
        let name = b"0123456789abcdefgh"[t];
        let (cmds, used) = track_mml(bin.get(offs[t]..)?, kind(t, mode), &mut now)?;
        rooms.push((t, used + 1));
        let mut l = vec![name, b' '];
        for c in cmds {
            if l.len() > 100 {
                line(&mut o, &l);
                l = vec![name, b' '];
            }
            l.extend_from_slice(c.as_bytes());
        }
        line(&mut o, &l);
    }
    let total: usize = rooms.iter().map(|&(_, r)| r).sum();
    if total > ROOM || total + title.len() > OBJECT {
        return None;
    }
    let mut alloc = b"#alloc {".to_vec();
    for (k, &(t, r)) in rooms.iter().enumerate() {
        alloc.extend_from_slice(format!("{}\r\n  {}={}", if k > 0 { "," } else { "" }, b"0123456789abcdefgh"[t] as char, r).as_bytes());
    }
    alloc.extend_from_slice(b" }\r\n");
    o.splice(alloc_at..alloc_at, alloc);
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The binary part's tracks, from a compiled song.
    fn tracks(m: &[u8]) -> Vec<Option<Vec<u8>>> {
        let eof = m.iter().position(|&b| b == 0x1a).unwrap();
        let bin = &m[eof + 1..];
        let offs: Vec<usize> = (0..18).map(|t| usize::from(u16::from_le_bytes([bin[4 + t * 2], bin[5 + t * 2]]))).collect();
        (0..18)
            .map(|t| {
                (offs[t] != 0).then(|| {
                    let d = &bin[offs[t]..];
                    let next = offs.iter().copied().filter(|&x| x > offs[t]).min();
                    match next {
                        Some(n) => d[..n - offs[t]].to_vec(),
                        None => d[..d.iter().rposition(|&b| b != 0).unwrap() + 1].to_vec(),
                    }
                })
            })
            .collect()
    }

    #[test]
    fn a_scale_on_fm() {
        let m = compile(b"#opll_mode 0\r\n#tempo 150\r\n#title \"Scale\"\r\n9 v13@0o4cdefgab>c4.r8\r\n").unwrap();
        assert!(m.starts_with(b"MGS313\r\nScale\r\n\x1a\x00\x00\x96\x00"));
        assert!(compile(b"#opll_mode 0\r\n9 c\r\n").unwrap().starts_with(b"MGS313\r\n\x1a\x00\x00\x00\x00"));
        assert_eq!(m.len() % 128, 0);
        let t = tracks(&m);
        assert!(t[0].is_none());
        assert_eq!(t[9].as_deref(), Some(&[0xcd, 0x80, 0xd3, 0x30, 0x32, 0x34, 0x35, 0x37, 0x39, 0x3b, 0x4e, 0x20, 72, 0x2c, 24, 0xff][..]));
        assert!(t[1].is_none());
        // And it plays.
        let song = crate::mgs::Song::parse(&m).unwrap();
        assert_eq!(song.tempo, 150);
    }

    #[test]
    fn lengths_ties_and_l() {
        let m = compile(b"#opll_mode 1\r\n1 l8c c8 c4 c^ c4^8 c%36 c%256 l16. c16. r8^16\r\n").unwrap();
        assert_eq!(
            tracks(&m)[1].as_deref(),
            Some(&[0x42, 24, 0x30, 0x30, 0x20, 48, 0x20, 48, 0x20, 72, 0x20, 36, 0x20, 0, 0x42, 18, 0x30, 0x2c, 36, 0xff][..])
        );
        assert_eq!(compile(b"#opll_mode 1\r\n1 c1^2\r\n").unwrap_err().what, LENGTH);
        assert_eq!(compile(b"#opll_mode 1\r\n1 c128\r\n").unwrap_err(), Error { line: Some(1), what: LENGTH });
    }

    #[test]
    fn loops_point_where_the_driver_reads() {
        // The bytes of a real song (MGSDRV/Konami, "Usas - Miss"): `[3 f8 g g ]`.
        let m = compile(b"#opll_mode 1\r\n3 l16 [3 f8 g g ]\r\n").unwrap();
        assert_eq!(tracks(&m)[3].as_deref(), Some(&[0x42, 12, 0x57, 3, 5, 0, 0x25, 24, 0x37, 0x37, 0x59, 0, 0xf9, 0xff, 0xff][..]));
        let m = compile(b"#opll_mode 1\r\n3 [c|d]0\r\n").unwrap();
        assert_eq!(tracks(&m)[3].as_deref(), Some(&[0x57, 0, 6, 0, 0x30, 0x58, 2, 0, 0x32, 0x59, 0, 0xf8, 0xff, 0xff][..]));
        // Left open, as real songs have it.
        let m = compile(b"#opll_mode 1\r\n3 [c\r\n").unwrap();
        assert_eq!(tracks(&m)[3].as_deref(), Some(&[0x57, 2, 0, 0, 0x30, 0xff][..]));
    }

    #[test]
    fn track_kinds_choose_the_bytes() {
        let m = compile(b"#opll_mode 1\r\n1 \\5 h1,2,3,-4 /2 @\\-300\r\n4 \\5\r\n9 \\5 h1,2,3,-4 / @\\200 so\r\nr v12 vb10 vh-2 bh8 s: r: ko\r\n").unwrap();
        let t = tracks(&m);
        assert_eq!(t[1].as_deref(), Some(&[0x50, 0xfb, 0x54, 2, 2, 4, 4, 0xe2, 0x51, 0xd4, 0xfe, 0xff][..]));
        assert_eq!(t[4].as_deref(), Some(&[0x50, 0xfb, 0xff][..]));
        assert_eq!(t[9].as_deref(), Some(&[0x50, 5, 0x54, 2, 2, 4, 0xfc, 0x4a, 0x5f, 200, 0x5b, 1, 0xff][..]));
        assert_eq!(t[15].as_deref(), Some(&[0xcc, 0x45, 10, 0x47, 0x9e, 0x31, 24, 0xa8, 0xa0, 0x5a, 0, 0xff][..]));
    }

    #[test]
    fn definitions_in_track_0() {
        let src = b"#opll_mode 0\r\n@v15 = {\r\n 25, 7,\r\n 8, 8, 1, 2, 0, 0, 0, 1, 0, 0, 0,\r\n 8, 2, 1, 1, 1, 1, 0, 1, 0, 0, 0 }\r\n\
@#3 = 0\r\n@e1 = { 1,0,fedc:4.8=10.n3./2[@2y7,56\\-3] }\r\n@r2 = { 2,5,255,255,255,190,0,20 }\r\n\
@s0 = { 7f7f7f7f 7f7f7f7f 7f7f7f7f 7f7f7f7f\r\n 80808080 80808080 80808080 80808080 }\r\n@m = { \"Hi\\C\\x41\\\\\" }\r\n";
        let m = compile(src).unwrap();
        let t0 = tracks(&m)[0].clone().unwrap();
        let mut want = vec![0, 15, 0x40, 0x41, 25, 0x47, 0x88, 0x82, 0x12, 0x11];
        want.extend_from_slice(&[1, 3, 0]);
        want.extend_from_slice(&[2, 1, 0x20, 18, 0x0f, 0x0e, 0x0d, 0xec, 4, 0x28, 10, 0x83, 0xa2, 0x40, 0x10, 2, 0x11, 7, 56, 0x12, 0xfd, 0x60]);
        want.extend_from_slice(&[2, 2, 0x80 | 0x45, 255, 255, 255, 190, 0, 20]);
        want.extend_from_slice(&[3, 0]);
        want.extend_from_slice(&[0x7f; 16]);
        want.extend_from_slice(&[0x80; 16]);
        want.extend_from_slice(&[6, 0, b'H', b'i', 3, 0x41, b'\\', 0, 0xff]);
        assert_eq!(t0, want);
        // And back.
        let back = decompile(&m).unwrap();
        assert_eq!(compile(&back).unwrap(), m);
    }

    #[test]
    fn macros_and_offsets() {
        let m = compile(b"#opll_mode 0\r\n#macro_offset { a = 32, c = 16 }\r\n*20 = { t150 }\r\n*40 = { v15 }\r\n*42 = { l16 }\r\n1 *c4*a8*a10 c ! d\r\n1 e\r\n").unwrap();
        let mut want = vec![0x41, 150, 0];
        want.extend_from_slice(&tempo_table(150));
        want.extend_from_slice(&[0xcf, 0x42, 12, 0x30, 0xff]);
        assert_eq!(tracks(&m)[1].as_deref(), Some(&want[..]));
        assert_eq!(compile(b"#tempo 120\r\n").unwrap_err().what, SYNTAX);
    }

    #[test]
    fn tempo_tables() {
        // As real songs have them: t60 (two-frame steps, written 3) and t150.
        assert_eq!(tempo_table(60), [0x75; 48]);
        assert_eq!(tempo_table(75), [0x55; 48]);
    }

    // 1082 sources run through MGSC.COM itself (`mgsc_ref/cases.tsv`: name, the source with `\n` for CR LF,
    // and the file it wrote up to its data's end, in hex, or `ERR:` and what it said): ours the same.
    #[test]
    fn the_same_as_mgsc_com() {
        let unescape = |s: &str| -> Vec<u8> {
            let mut o = Vec::new();
            let mut it = s.bytes();
            while let Some(c) = it.next() {
                if c != b'\\' {
                    o.push(c);
                    continue;
                }
                match it.next() {
                    Some(b'n') => o.extend_from_slice(b"\r\n"),
                    Some(b'N') => o.push(b'\n'),
                    Some(b'r') => o.push(b'\r'),
                    Some(b't') => o.push(b'\t'),
                    Some(x) => o.push(x),
                    None => {}
                }
            }
            o
        };
        let mut bad = Vec::new();
        let rows = include_str!("mgsc_ref/cases.tsv");
        for row in rows.lines() {
            let mut f = row.split('\t');
            let (name, src, want) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
            // Sources were written as Latin-1 text: back to their bytes.
            let src: Vec<u8> = String::from_utf8(unescape(src)).unwrap().chars().map(|c| c as u32 as u8).collect();
            let got = match compile(&src) {
                Ok(m) => {
                    let end = m.iter().rposition(|&b| b != 0).map_or(0, |e| e + 1);
                    m[..end].iter().map(|b| format!("{b:02x}")).collect::<String>()
                }
                Err(e) => format!("ERR:{e}"),
            };
            if got != want {
                bad.push(format!("{name}: MGSC {}, ours {}", &want[..want.len().min(60)], &got[..got.len().min(60)]));
            }
        }
        assert!(rows.lines().count() > 1000);
        assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
    }

    // Against MGSC.COM itself (Okazaki's mgsc, ISC, running the original in a Z80 emulator): every X.mus in
    // MGSC_REF beside the X.MGS it wrote (empty when it stopped) and X.out, what it printed ("<why> in <line>").
    // Ours must be the same file, or stop on the same line for the same reason. MGSC_REF=dir cargo test
    // --release --lib mgsc::tests::against_mgsc -- --ignored --nocapture (MGSC_ONLY=name).
    #[test]
    #[ignore]
    fn against_mgsc() {
        let dir = std::path::PathBuf::from(std::env::var("MGSC_REF").expect("MGSC_REF"));
        let only = std::env::var("MGSC_ONLY").ok();
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "mus"))
            .collect();
        names.sort();
        let (mut same, mut differ) = (0, 0);
        for p in names {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| *o != name) {
                continue;
            }
            let src = std::fs::read(&p).unwrap();
            let theirs = std::fs::read(dir.join(format!("{name}.MGS"))).unwrap_or_default();
            let out = String::from_utf8_lossy(&std::fs::read(dir.join(format!("{name}.out"))).unwrap_or_default()).to_string();
            let said = out.lines().find(|l| (l.contains(" in ") && !l.starts_with(">>")) || l.trim() == BIG).map(|l| l.trim().to_string());
            let ours = compile(&src);
            let ok = match (&ours, theirs.is_empty()) {
                (Ok(m), false) => *m == theirs,
                (Err(e), true) => said.as_deref() == Some(&e.to_string()[..]),
                _ => false,
            };
            if ok {
                same += 1;
                continue;
            }
            differ += 1;
            match ours {
                Ok(m) if !theirs.is_empty() => {
                    let i = (0..m.len().max(theirs.len())).find(|&i| m.get(i) != theirs.get(i)).unwrap();
                    println!(
                        "{name}: first difference at {i}: {:02x?} / ours {:02x?}",
                        &theirs[i.min(theirs.len())..(i + 10).min(theirs.len())],
                        &m[i.min(m.len())..(i + 10).min(m.len())]
                    );
                }
                Ok(_) => println!("{name}: MGSC: {}, ours compiles", said.unwrap_or_default()),
                Err(e) if theirs.is_empty() => println!("{name}: MGSC: {} / ours: {e}", said.unwrap_or_default()),
                Err(e) => println!("{name}: MGSC compiles, ours: {e}"),
            }
        }
        println!("{same} the same, {differ} differ");
        assert_eq!(differ, 0);
    }

    // Every song in MGSC_DIR that MGSC 1.11 wrote ("313") decompiled and compiled back: the same bytes,
    // but for the zeros MGSC pads the last record with. MGSC_DIR=$TMPDIR/syn/mgs cargo test --release
    // --lib mgsc::tests::real_songs_compile_back -- --ignored --nocapture (MGSC_SHOW=name prints the MML).
    #[test]
    #[ignore]
    fn real_songs_compile_back() {
        let dir = std::env::var("MGSC_DIR").expect("MGSC_DIR");
        let show = std::env::var("MGSC_SHOW").ok();
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        names.sort();
        let (mut same, mut differ, mut skipped) = (0, 0, 0);
        for p in names {
            let mut d = std::fs::read(&p).unwrap();
            // MGSARC's packed songs (some say "313"): compared unpacked.
            if let Some(eof) = d.iter().position(|&b| b == 0x1a) {
                if d.get(eof + 2).is_some_and(|f| f & 0x80 != 0) {
                    match crate::mgs::uncompress(&d) {
                        Some(u) => d = u,
                        None => continue,
                    }
                }
            }
            // MGSC_OLD=1: the older formats too ("304" to "311"), but for their version.
            let old = std::env::var("MGSC_OLD").is_ok() && d.starts_with(b"MGS3") && d.get(3..6).is_some_and(|v| v >= &b"304"[..]);
            if !d.starts_with(b"MGS313") && !old {
                continue;
            }
            d[3..6].copy_from_slice(b"313");
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let Some(mml) = decompile(&d) else {
                println!("{name}: not decompiled");
                skipped += 1;
                continue;
            };
            // MGSC_DUMP=dir: the MML and the song as it was, to run MGSC itself on (X.mus, X.orig).
            if let Ok(dump) = std::env::var("MGSC_DUMP") {
                let stem = name.trim_end_matches(".MGS").trim_end_matches(".mgs").replace(['\'', '(', ')', '&', ','], "_");
                std::fs::write(format!("{dump}/{stem}.mus"), &mml).unwrap();
                std::fs::write(format!("{dump}/{stem}.orig"), &d).unwrap();
            }
            if show.as_deref() == Some(&name[..]) {
                println!("{}", String::from_utf8_lossy(&mml));
            }
            match compile(&mml) {
                Err(e) => {
                    println!("{name}: {e}");
                    differ += 1;
                }
                Ok(m) => {
                    // Up to the data's end (its last FF): what follows is the last record's padding.
                    let m = &m[..m.iter().rposition(|&b| b != 0).unwrap() + 1];
                    let d = &d[..d.len().min(m.len().max(d.len().min(m.len())))];
                    let n = d.len().min(m.len());
                    let first = (0..n).find(|&i| d[i] != m[i]);
                    let tail_ok = |a: &[u8]| a.iter().all(|&b| b == 0);
                    let real_end = d.len().max(m.len());
                    let ok = first.is_none() && (tail_ok(&m[n..]) || d.len() > m.len());
                    if ok {
                        same += 1;
                    } else if let Some(i) = first {
                        println!(
                            "{name}: first difference at {i} of {real_end}: {:02x?} / ours {:02x?}",
                            &d[i..(i + 8).min(d.len())],
                            &m[i..(i + 8).min(m.len())]
                        );
                        differ += 1;
                    } else {
                        println!("{name}: lengths {} / ours {}", d.len(), m.len());
                        differ += 1;
                    }
                }
            }
        }
        println!("{same} the same, {differ} differ, {skipped} not decompiled");
    }
}
