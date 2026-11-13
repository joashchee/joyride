// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// The MML reader of NSD.Lib's compiler: a port of `MMLfile.cpp` and `FileInput.cpp` (nsc 1.31, S.W., BSD-2,
// github.com/Shaw02/nsdlib). Kept close to the original, down to its stream's behaviour at the end of a file (a
// read past the end returns the last byte again, as `ifstream::read` leaves its buffer) and its macro stack.

use std::collections::BTreeMap;

use super::{Error, Patch};

/// A file being read, as `FileInput` (an `ifstream`) reads it.
pub(crate) struct FileIn {
    pub name: String,
    data: Vec<u8>,
    pos: usize,
    pub line: usize,
    last: u8,
    /// Past the end: `eofbit` and `failbit`, which nothing clears.
    fail: bool,
}

impl FileIn {
    pub fn new(name: &str, data: Vec<u8>) -> FileIn {
        FileIn {
            name: name.to_string(),
            data,
            pos: 0,
            line: 1,
            last: 0,
            fail: false,
        }
    }
    fn read(&mut self) -> u8 {
        if !self.fail {
            if self.pos < self.data.len() {
                self.last = self.data[self.pos];
                self.pos += 1;
            } else {
                self.fail = true;
            }
        }
        if self.last == 0x0a {
            self.line += 1;
        }
        self.last
    }
    fn back(&mut self) {
        if self.fail || self.pos == 0 {
            if self.pos == 0 {
                self.fail = true;
            }
            if self.last == 0x0a {
                self.line = self.line.saturating_sub(1);
            }
            return;
        }
        self.pos -= 1;
        if self.last == 0x0a {
            self.line = self.line.saturating_sub(1);
        }
        self.last = self.data[self.pos];
    }
    fn eof(&self) -> bool {
        self.fail
    }
    fn tellg(&self) -> i64 {
        if self.fail {
            -1
        } else {
            self.pos as i64
        }
    }
    fn seek(&mut self, p: i64) {
        if self.fail {
            return;
        }
        if p < 0 || p as usize > self.data.len() {
            self.fail = true;
        } else {
            self.pos = p as usize;
        }
    }
}

#[derive(Clone, Default)]
struct MacroPos {
    name: Vec<u8>,
    line: usize,
}

/// A reader of more files (`#include` and DPCM samples): the name as written, its bytes if there are any.
pub type FileReader<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

pub(crate) type R<T> = Result<T, Error>;

pub(crate) struct Reader<'a> {
    files: Vec<FileIn>,
    macros: BTreeMap<Vec<u8>, Vec<u8>>,
    macro_lv: BTreeMap<Vec<u8>, i32>,
    s_macro: Vec<MacroPos>,
    now_macro: MacroPos,
    p_macro: usize,
    f_macro: bool,
    f_2to1: bool,
    now_command: Vec<u8>,
    pub inc: FileReader<'a>,
    pub patches: BTreeMap<usize, Patch>,
    pub offset_ei: i64,
    pub offset_ev: i64,
    pub offset_en: i64,
    pub offset_em: i64,
    pub release_volume: i32,
    pub repeat_mode: i32,
    pub tie_mode: i32,
    pub timebase: i32,
    pub octave_reverse: bool,
    pub q_reverse: bool,
    pub rest: i32,
    pub wait: i32,
    pub qmax: i32,
    pub priority: u8,
    pub warnings: Vec<String>,
    /// The id the last `get_command_id` found (`u32::MAX` for none).
    pub last_id: u32,
}

impl<'a> Reader<'a> {
    pub fn new(name: &str, data: Vec<u8>, inc: FileReader<'a>) -> Reader<'a> {
        Reader {
            files: vec![FileIn::new(name, data)],
            macros: BTreeMap::new(),
            macro_lv: BTreeMap::new(),
            s_macro: Vec::new(),
            now_macro: MacroPos::default(),
            p_macro: 0,
            f_macro: false,
            f_2to1: false,
            now_command: Vec::new(),
            inc,
            patches: BTreeMap::new(),
            offset_ei: 0,
            offset_ev: 0,
            offset_en: 0,
            offset_em: 0,
            release_volume: 2,
            repeat_mode: 0,
            tie_mode: 0,
            timebase: 24,
            octave_reverse: false,
            q_reverse: false,
            rest: 2,
            wait: 0,
            qmax: 8,
            priority: 0,
            warnings: Vec::new(),
            last_id: u32::MAX,
        }
    }

    fn file(&mut self) -> &mut FileIn {
        self.files.last_mut().unwrap()
    }

    pub fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        let f = self.files.last().unwrap();
        Err(Error {
            file: f.name.clone(),
            line: f.line,
            command: String::from_utf8_lossy(&self.now_command).into_owned(),
            message: msg.into(),
        })
    }

    pub fn warning(&mut self, msg: impl Into<String>) {
        let f = self.files.last().unwrap();
        let m = format!("{} (line {}): {}", f.name, f.line, msg.into());
        self.warnings.push(m);
    }

    pub fn eof(&self) -> bool {
        self.files.last().unwrap().eof()
    }

    /// The end of the MML: the main file read to its end.
    pub fn eom(&self) -> bool {
        self.files.len() == 1 && self.eof()
    }

    pub fn line(&self) -> usize {
        self.files.last().unwrap().line
    }

    pub fn set_line(&mut self, l: usize) {
        self.file().line = l;
    }

    pub fn include(&mut self) -> R<()> {
        let mut name = Vec::new();
        self.get_string(&mut name, false)?;
        let name = String::from_utf8_lossy(&name).into_owned();
        if self.files.iter().any(|f| f.name == name) {
            return self.err(format!("{name}: the same include file is already open"));
        }
        match (self.inc)(&name) {
            Some(d) => {
                self.files.push(FileIn::new(&name, d));
                Ok(())
            }
            None => self.err(format!("{name}: can't open the file")),
        }
    }

    pub fn set_macro(&mut self, lv: i32) -> R<()> {
        let mut name = Vec::new();
        loop {
            let c = self.c_read();
            if (c as i8) <= 0x20 {
                break;
            }
            if c == b'{' {
                self.back();
                break;
            }
            name.push(c);
        }
        if self.macros.contains_key(&name) {
            return self.err("that macro name already exists");
        }
        self.chk_left_curly_brace()?;
        let mut body = Vec::new();
        let mut depth = 0i32;
        loop {
            let (c, more) = self.get_char_chk_right_curly()?;
            if !(more || depth != 0) {
                break;
            }
            if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
            }
            body.push(c);
        }
        body.push(b' ');
        self.macros.insert(name.clone(), body);
        self.macro_lv.insert(name, lv);
        Ok(())
    }

    pub fn delete_macro(&mut self, lv: i32) {
        let names: Vec<Vec<u8>> = self.macro_lv.iter().filter(|(_, &l)| l == lv).map(|(n, _)| n.clone()).collect();
        for n in names {
            self.macros.remove(&n);
            self.macro_lv.remove(&n);
        }
    }

    pub fn call_macro(&mut self) -> R<()> {
        let mut cands: Vec<Vec<u8>> = self.macros.keys().cloned().collect();
        let mut name = Vec::new();
        let mut i = 0;
        loop {
            let c = self.c_read();
            name.push(c);
            let mut n = 0;
            if (c as i8) > 0x20 {
                cands.retain(|m| m.get(i).copied().unwrap_or(0) == c);
                n = cands.len();
            }
            i += 1;
            if n == 0 {
                break;
            }
        }
        self.back();
        name.pop();
        if !self.macros.contains_key(&name) {
            return self.err("that macro doesn't exist");
        }
        if self.s_macro[..self.p_macro].iter().any(|m| m.name == name) {
            return self.err("a macro calls itself");
        }
        if self.p_macro > 0 {
            self.s_macro[self.p_macro - 1] = self.now_macro.clone();
        }
        self.now_macro = MacroPos { name, line: 0 };
        self.s_macro.push(self.now_macro.clone());
        self.p_macro += 1;
        Ok(())
    }

    pub fn macro_nest(&self) -> usize {
        self.p_macro
    }

    pub fn tellg(&self) -> i64 {
        if self.p_macro > 0 {
            self.now_macro.line as i64
        } else {
            self.files.last().unwrap().tellg()
        }
    }

    pub fn seek(&mut self, p: i64) {
        if self.p_macro > 0 {
            self.now_macro.line = p as usize;
        } else {
            self.file().seek(p);
        }
    }

    fn back_one(&mut self) {
        if self.f_macro {
            self.f_macro = false;
            if self.p_macro > 0 {
                self.s_macro[self.p_macro - 1] = self.now_macro.clone();
            }
            self.p_macro += 1;
            self.now_macro = self.s_macro[self.p_macro - 1].clone();
            self.now_macro.line = self.now_macro.line.wrapping_sub(1);
        } else if self.p_macro > 0 {
            self.now_macro.line = self.now_macro.line.wrapping_sub(1);
        } else {
            self.file().back();
        }
    }

    pub fn back(&mut self) {
        if self.f_2to1 {
            self.back_one();
            self.back_one();
        } else {
            self.back_one();
        }
    }

    fn read_char(&mut self) -> u8 {
        if self.f_macro {
            self.f_macro = false;
            self.s_macro.pop();
        }
        if self.p_macro > 0 {
            let body = self.macros.get(&self.now_macro.name).map(|b| b.as_slice()).unwrap_or(&[]);
            let c = body.get(self.now_macro.line).copied().unwrap_or(0);
            let len = body.len();
            self.now_macro.line = self.now_macro.line.wrapping_add(1);
            if len == self.now_macro.line {
                self.s_macro[self.p_macro - 1] = self.now_macro.clone();
                self.p_macro -= 1;
                self.f_macro = true;
                if self.p_macro > 0 {
                    self.now_macro = self.s_macro[self.p_macro - 1].clone();
                }
            }
            c
        } else {
            self.file().read()
        }
    }

    /// A byte, with Shift JIS's full-width symbols and digits read as their ASCII.
    pub fn c_read(&mut self) -> u8 {
        let c = self.read_char();
        let to = |lead: u8, t: u8| -> Option<u8> {
            match (lead, t) {
                (0x81, 0x67) | (0x81, 0x68) => Some(b'"'),
                (0x81, 0x69) => Some(b'('),
                (0x81, 0x6a) => Some(b')'),
                (0x81, 0x6f) => Some(b'{'),
                (0x81, 0x70) => Some(b'}'),
                (0x81, 0x7b) => Some(b'+'),
                (0x81, 0x7c) => Some(b'-'),
                (0x81, 0x94) => Some(b'#'),
                (0x81, 0xf3) => Some(b'-'),
                (0x82, 0x4f..=0x58) => Some(b'0' + (t - 0x4f)),
                _ => None,
            }
        };
        match c {
            0x81 | 0x82 => {
                let t = self.read_char();
                self.f_2to1 = true;
                match to(c, t) {
                    Some(x) => x,
                    None => {
                        self.f_2to1 = false;
                        self.back();
                        c
                    }
                }
            }
            _ => {
                self.f_2to1 = false;
                c
            }
        }
    }

    pub fn get_char_chk_right_curly(&mut self) -> R<(u8, bool)> {
        let c = self.get_char()?;
        self.now_command = b"{".to_vec();
        self.chk_eof()?;
        Ok((c, c != b'}'))
    }

    /// A byte past spaces, controls and comments; 0 at the end of the MML.
    pub fn get_char(&mut self) -> R<u8> {
        loop {
            let c = self.c_read();
            if self.eom() {
                return Ok(0);
            } else if self.eof() {
                self.files.pop();
            } else if c <= 0x20 {
            } else if c == b';' {
                loop {
                    if self.eof() {
                        break;
                    }
                    if self.c_read() == 0x0a {
                        break;
                    }
                }
            } else if c == b'/' {
                let c2 = self.c_read();
                match c2 {
                    b'/' => loop {
                        if self.eof() {
                            break;
                        }
                        if self.c_read() == 0x0a {
                            break;
                        }
                    },
                    b'*' => loop {
                        loop {
                            let c3 = self.c_read();
                            self.chk_eof()?;
                            if c3 == b'*' {
                                break;
                            }
                        }
                        let c3 = self.c_read();
                        self.back();
                        if c3 == b'/' {
                            self.c_read();
                            break;
                        }
                    },
                    _ => {
                        self.back();
                        return self.err_unknown_cmd();
                    }
                }
            } else {
                return Ok(c);
            }
        }
    }

    pub fn chk_comma(&mut self) -> R<()> {
        let c = self.get_char()?;
        if c != b',' {
            self.back();
            return self.err("a parameter is missing");
        }
        Ok(())
    }

    pub fn chk_left_curly_brace(&mut self) -> R<()> {
        loop {
            let c = self.get_char()?;
            if c == b'{' {
                return Ok(());
            } else if c != b'=' {
                return self.err("'{' is missing");
            }
        }
    }

    pub fn chk_eof(&self) -> R<()> {
        if self.eof() {
            return self.err("the file ends unexpectedly");
        }
        Ok(())
    }

    pub fn get_string(&mut self, s: &mut Vec<u8>, esc: bool) -> R<()> {
        let c = self.get_char()?;
        if c != b'"' {
            return self.err("'\"' is missing at the string's start");
        }
        loop {
            let mut c = self.c_read();
            if c == b'"' {
                return Ok(());
            }
            self.chk_eof()?;
            if esc && c == b'\\' {
                c = self.get_char()?;
                self.chk_eof()?;
                c = match c {
                    b'a' => 7,
                    b'b' => 8,
                    b'f' => 12,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'v' => 11,
                    b'\'' | b'"' | b'\\' | b'?' => c,
                    b'x' => self.get_hex()? as u8,
                    b'0'..=b'7' => {
                        self.back();
                        self.get_oct()? as u8
                    }
                    _ => {
                        s.push(b'\\');
                        c
                    }
                };
            }
            s.push(c);
        }
    }

    /// A number in `( )`.
    pub fn get_num(&mut self) -> R<i32> {
        if self.get_char()? != b'(' {
            return self.err("'(' is missing before the number");
        }
        let n = self.get_int()?;
        if self.get_char()? != b')' {
            return self.err("')' is missing after the number");
        }
        Ok(n)
    }

    pub fn chk_range(&self, what: &str, min: i32, max: i32, v: i32) -> R<()> {
        if v < min || v > max {
            return self.err(format!("{what} must be {min} to {max}"));
        }
        Ok(())
    }

    pub fn get_int_range(&mut self, what: &str, min: i32, max: i32) -> R<i32> {
        let n = self.get_int()?;
        self.chk_range(what, min, max, n)?;
        Ok(n)
    }

    pub fn get_int(&mut self) -> R<i32> {
        let c = self.get_char()?;
        let (neg, n) = if c == b'$' {
            let neg = self.chk_sign()?;
            (neg, self.get_hex()?)
        } else if c == b'%' {
            let neg = self.chk_sign()?;
            (neg, self.get_bin()?)
        } else if c.is_ascii_digit() || c == b'-' || c == b'+' {
            self.back();
            let neg = self.chk_sign()?;
            (neg, self.get_dec()?)
        } else {
            self.back();
            return self.err("a number was expected");
        };
        Ok(if neg { n.wrapping_neg() } else { n })
    }

    fn chk_sign(&mut self) -> R<bool> {
        Ok(match self.get_char()? {
            b'-' => true,
            b'+' => false,
            _ => {
                self.back();
                false
            }
        })
    }

    // The digit loops stop at the end of a file, where nsc would read the last byte for ever.
    fn digits(&mut self, base: i32, ok: fn(u8) -> Option<i32>) -> R<i32> {
        let mut n: i32 = 0;
        let mut c = self.get_char()?;
        while let Some(d) = ok(c) {
            n = n.wrapping_mul(base).wrapping_add(d);
            c = self.c_read();
            if self.eof() && self.p_macro == 0 {
                break;
            }
        }
        self.back();
        Ok(n)
    }

    pub fn get_hex(&mut self) -> R<i32> {
        self.digits(16, |c| (c as char).to_digit(16).map(|d| d as i32))
    }
    pub fn get_dec(&mut self) -> R<i32> {
        self.digits(10, |c| c.is_ascii_digit().then(|| i32::from(c - b'0')))
    }
    pub fn get_oct(&mut self) -> R<i32> {
        self.digits(8, |c| (b'0'..=b'7').contains(&c).then(|| i32::from(c - b'0')))
    }
    pub fn get_bin(&mut self) -> R<i32> {
        self.digits(2, |c| (b'0'..=b'1').contains(&c).then(|| i32::from(c - b'0')))
    }

    pub fn read_length(&mut self, default: i32) -> R<i32> {
        let c = self.get_char()?;
        self.back();
        let mut len;
        if c.is_ascii_digit() || c == b'.' {
            if c.is_ascii_digit() {
                let i = self.get_int()?;
                if i == 0 {
                    return self.err("a length can't be 0");
                }
                len = (self.timebase * 4) / i;
                if (self.timebase * 4) % i != 0 {
                    self.warning("the length isn't a whole number of ticks; rounded down");
                }
            } else if default == -1 {
                return self.err("a length is needed here");
            } else {
                len = default;
            }
            let mut dot = len;
            loop {
                let c = self.c_read();
                if c != b'.' {
                    break;
                }
                if len == -1 {
                    return self.err("a dot with no length needs an `l` first");
                }
                let odd = dot & 1;
                dot >>= 1;
                if odd != 0 {
                    self.warning("the dotted length isn't a whole number of ticks; rounded down");
                }
                len += dot;
            }
            self.back();
        } else if c == b'%' {
            self.get_char()?;
            len = self.get_int()?;
        } else if default == -1 {
            return self.err("a length is needed here");
        } else {
            len = -1;
        }
        Ok(len)
    }

    pub fn get_length(&mut self, default: i32) -> R<i32> {
        let mut len = self.read_length(default)?;
        let mut c = self.c_read();
        while c == b'-' || c == b'~' || c == b'+' || (self.tie_mode == 1 && c == b'^') {
            if len == -1 {
                len = default;
            }
            let add = c == b'+' || c == b'^';
            let mut d = self.read_length(default)?;
            if d == -1 {
                d = default;
            }
            if add {
                len += d;
            } else {
                len -= d;
            }
            c = self.c_read();
        }
        self.back();
        if len == -1 {
            return Ok(len);
        }
        if !(1..=255).contains(&len) {
            return self.err("the length comes to more than 255 ticks (use a tie `&` past 255)");
        }
        Ok(len)
    }

    /// The command at the reader, the longest of `table`'s that matches; None if none does.
    pub fn get_command_id(&mut self, table: &[(&[u8], u32)]) -> Option<u32> {
        let mut end = self.tellg();
        let c = self.c_read();
        self.now_command = vec![c];
        self.back();
        let mut cands: Vec<(&[u8], u32)> = table.to_vec();
        let mut result = None;
        let mut i = 0;
        loop {
            let c = self.c_read();
            let mut n = 0;
            let mut k = 0;
            while k < cands.len() {
                let ch = cands[k].0.get(i).copied().unwrap_or(0);
                if ch == c {
                    n += 1;
                    k += 1;
                } else if ch == 0 {
                    self.back();
                    end = self.tellg();
                    self.c_read();
                    self.now_command = cands[k].0.to_vec();
                    result = Some(cands[k].1);
                    cands.remove(k);
                } else {
                    cands.remove(k);
                }
            }
            i += 1;
            if !(n > 0 && !self.f_macro) {
                break;
            }
        }
        if self.f_macro {
            self.back();
        }
        self.seek(end);
        self.last_id = result.unwrap_or(u32::MAX);
        result
    }

    pub fn err_unknown_cmd<T>(&mut self) -> R<T> {
        let c = self.c_read();
        self.now_command = vec![c];
        self.err("unknown command")
    }
}
