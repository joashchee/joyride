//! MML for MDSDRV (Sega Mega Drive): the dialect of ctrmml's `#platform mdsdrv`, compiled to a `.mds` file that
//! `mdsdrv::Song::parse` reads and `mdsdrv::Player` plays.
//!
//! Written from ctrmml's MML reference and the `.mds` format document (`mdsseq.md` of MDSDRV, Zlib), by
//! black box: ctrmml itself is GPL and was not read. Where the documents were silent the choices were made by
//! compiling test files with ctrmml's `mmlc` and reading only its output (`scripts/mdsmml-reference.sh`): note
//! numbers (`o1 c` is 0 on every channel, the default octave is 6), length bytes (ticks - 1), the tempo byte,
//! the PSG envelope bytes (attenuation), the drum slots, `lfo` and `fm3` bytes, pitch envelope deltas. The 15
//! `.mml` files of MDSDRV and the 5 in `testdata/mdsmml` give the same notes, steps and samples from both
//! compilers. The bytes differ where ours was free to choose (it always writes lengths, never shares data
//! blocks, numbers the table's slots in its own order).
//!
//! Supported: notes, rests, ties, slurs, `o < > l Q q C s R ~`, loops with `/` break, `L`, `*n` subroutines,
//! `{a/b/c}` blocks, key signatures, `v V ( ) p _ __ k K M P G D t T`, drum mode, the `'...'` commands of the
//! platform, the echo macro, FM / `2op` / PSG / PCM instruments, pitch envelopes (both formats), macro tracks.
//! Not supported (an error): `E` and pan envelopes.

use crate::mdsdrv::Error;
use std::collections::{BTreeMap, HashMap};

/// Note numbers are the same on every channel: `o1 c` is 0 (ctrmml's compiler, compared with it). A length byte holds ticks - 1.
const PSG_SHIFT: i32 = 0;
const LAST_NOTE: i32 = 93; // 0xdf - 0x82

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Class {
    Fm,
    Psg,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A track that plays.
    Track,
    /// A `*n` subroutine.
    Pat,
    /// A drum mode subroutine: it ends at its first note.
    Drum,
    /// A macro track (`P`).
    Macro,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Key {
    Inst(u32),
    Peg(u32),
    Pat(u32, Class),
    Drum(usize),
    Mtab(u32),
}

enum Inst {
    Fm { alg: i32, fb: i32, ops: [[i32; 10]; 4], trs: i32 },
    TwoOp { base: u32, ml: [i32; 4], trs: i32 },
    Psg(Vec<u8>),
    Pcm { path: String, rate: Option<u32>, offset: u32 },
}

struct Global {
    insts: BTreeMap<u32, Inst>,
    /// `@M` definitions: the words after the number.
    pegs: BTreeMap<u32, String>,
    tracks: BTreeMap<u32, String>,
    ext_peg: bool,
    slots: HashMap<Key, usize>,
    used: Vec<bool>,
    reserved: Vec<bool>,
    /// Drum mode: each `D<n>` has a block of consecutive slots (a to h), and a slot's drum is track n + k.
    drum_base: HashMap<usize, usize>,
    drum_src: HashMap<usize, usize>,
    /// What to build, in the order met.
    queue: Vec<(Key, Class)>,
    /// Code by slot (subroutines, drums, macro tables) and data blocks (glob id, bytes).
    code: BTreeMap<usize, Vec<u8>>,
    globs: Vec<(u32, Vec<u8>)>,
    /// PCM headers (id, position, start, size, rate) and the data.
    pcmh: Vec<(u32, u32, u32, u32, u32)>,
    pcmd: Vec<u8>,
    load: Box<dyn FnMut(&str) -> Option<Vec<u8>>>,
}

fn err<T>(m: impl Into<String>) -> Result<T, Error> {
    Err(Error(m.into()))
}

impl Global {
    fn slot(&mut self, k: Key, class: Class) -> Result<usize, Error> {
        if let Some(&s) = self.slots.get(&k) {
            return Ok(s);
        }
        let s = match &k {
            Key::Drum(i) => *i,
            _ => match (0..255).find(|&i| !self.used.get(i).copied().unwrap_or(false) && !self.reserved.get(i).copied().unwrap_or(false)) {
                Some(i) => i,
                None => return err("more than 255 data items (instruments, subroutines, envelopes) in one song"),
            },
        };
        if s >= 255 {
            return err("a drum's number is past the song data table");
        }
        if self.used.len() <= s {
            self.used.resize(s + 1, false);
        }
        if self.used[s] {
            return err(format!("drum {s} is also used by another item"));
        }
        self.used[s] = true;
        self.slots.insert(k.clone(), s);
        self.queue.push((k, class));
        Ok(s)
    }
}

// ---- text helpers ----

fn strip_comment(l: &str) -> &str {
    let mut q = false;
    for (i, c) in l.char_indices() {
        match c {
            '"' => q = !q,
            ';' if !q => return &l[..i],
            _ => {}
        }
    }
    l
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty())
}

fn num(s: &str) -> Result<i32, Error> {
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix('$')) {
        return i32::from_str_radix(h, 16).map_err(|_| Error(format!("'{s}' isn't a number")));
    }
    s.parse::<f64>().map(|v| v as i32).map_err(|_| Error(format!("'{s}' isn't a number")))
}

/// Replaces each `{a/b/c}` by its `index`th part (not `_{...}`, not inside quotes).
fn conditional(text: &str, index: usize) -> Result<String, Error> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut quote = false;
    while i < cs.len() {
        let c = cs[i];
        if c == '\'' {
            quote = !quote;
        }
        if c == '{' && !quote && !(i > 0 && cs[i - 1] == '_') {
            let mut depth = 0;
            let mut j = i + 1;
            let mut parts: Vec<String> = vec![String::new()];
            let mut bracket = 0;
            loop {
                let Some(&d) = cs.get(j) else { return err("a '{' without its '}'") };
                match d {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    '[' => bracket += 1,
                    ']' => bracket -= 1,
                    _ => {}
                }
                if d == '/' && depth == 0 && bracket == 0 {
                    parts.push(String::new());
                } else {
                    parts.last_mut().unwrap().push(d);
                }
                j += 1;
            }
            if let Some(p) = parts.get(index) {
                out.push_str(p);
            }
            i = j + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    Ok(out)
}

// ---- instruments ----

fn parse_fm(w: &[&str]) -> Result<Inst, Error> {
    let n: Vec<i32> = w.iter().map(|x| num(x)).collect::<Result<_, _>>()?;
    if n.len() < 42 {
        return err(format!("an FM instrument needs 2 + 4 x 10 numbers, found {}", n.len()));
    }
    let mut ops = [[0; 10]; 4];
    for (o, op) in ops.iter_mut().enumerate() {
        op.copy_from_slice(&n[2 + o * 10..12 + o * 10]);
    }
    Ok(Inst::Fm { alg: n[0], fb: n[1], ops, trs: n.get(42).copied().unwrap_or(0) })
}

fn fm_bytes(alg: i32, fb: i32, ops: &[[i32; 10]; 4], trs: i32) -> Vec<u8> {
    let mut b = vec![0u8; 30];
    // The register order of the operators: 1, 3, 2, 4.
    for (k, &o) in [0usize, 2, 1, 3].iter().enumerate() {
        let [ar, dr, sr, rr, sl, tl, ks, ml, dt, ssg] = ops[o];
        let (am, ssg) = if ssg >= 100 { (1, ssg - 100) } else { (0, ssg) };
        b[k] = ((dt & 7) << 4 | (ml & 15)) as u8;
        b[4 + k] = ((ks & 3) << 6 | (ar & 31)) as u8;
        b[8 + k] = (am << 7 | (dr & 31)) as u8;
        b[12 + k] = (sr & 31) as u8;
        b[16 + k] = ((sl & 15) << 4 | (rr & 15)) as u8;
        b[20 + k] = (ssg & 15) as u8;
        b[24 + k] = (tl & 127) as u8;
    }
    b[28] = ((fb & 7) << 3 | (alg & 7)) as u8;
    // The transpose is kept in the table's own steps (24 is "none"), two to the semitone.
    b[29] = (48 + 2 * trs).clamp(0, 255) as u8;
    b
}

/// A PSG envelope: `15 14 / 13>0:7`, `0>14:7 | 15 10 5 0 5 10`, `l:40 15 14 13`.
fn psg_env(text: &str) -> Result<Vec<u8>, Error> {
    let mut frames: Vec<(u8, u32)> = Vec::new();
    let mut default = 1u32;
    let mut b: Vec<u8> = Vec::new();
    let mut loop_at: Option<usize> = None;
    // The segments are written as they come, with the marks between them.
    let flush = |frames: &mut Vec<(u8, u32)>, b: &mut Vec<u8>| {
        let mut run: Vec<(u8, u32)> = Vec::new();
        for &(v, n) in frames.iter() {
            match run.last_mut() {
                Some(l) if l.0 == v => l.1 += n,
                _ => run.push((v, n)),
            }
        }
        for (v, mut n) in run {
            while n > 0 {
                let c = n.min(15);
                // The data holds attenuation: 0 is the loudest, 15 is off.
                b.push((c as u8) << 4 | (15 - v));
                n -= c;
            }
        }
        frames.clear();
    };
    for w in words(text) {
        if let Some(d) = w.strip_prefix("l:") {
            default = num(d)?.max(1) as u32;
        } else if w == "/" {
            flush(&mut frames, &mut b);
            b.push(0x01);
        } else if w == "|" {
            flush(&mut frames, &mut b);
            loop_at = Some(b.len());
        } else {
            let (body, len) = match w.split_once(':') {
                Some((x, l)) => (x, Some(num(l)?.max(1) as u32)),
                None => (w, None),
            };
            if let Some((a, z)) = body.split_once('>') {
                let (a, z) = (num(a)?.clamp(0, 15), num(z)?.clamp(0, 15));
                let steps = (a - z).unsigned_abs() + 1;
                let total = len.unwrap_or(steps * default);
                for f in 0..total {
                    let t = if total > 1 { f64::from(f) / f64::from(total - 1) } else { 0.0 };
                    let v = (f64::from(a) + f64::from(z - a) * t).round() as u8;
                    frames.push((v, 1));
                }
            } else {
                frames.push((num(body)?.clamp(0, 15) as u8, len.unwrap_or(default)));
            }
        }
    }
    flush(&mut frames, &mut b);
    match loop_at {
        Some(p) => b.extend([0x02, p as u8]),
        None => b.push(0x00),
    }
    if b.len() > 255 {
        return err("a PSG envelope longer than 255 bytes");
    }
    Ok(b)
}

/// A pitch envelope (`@M`): the nodes as (initial, delta, ticks) in 8.8 semitones, and the loop node.
fn peg_nodes(text: &str) -> Result<(Vec<(i32, i32, u32)>, Option<usize>), Error> {
    let mut nodes: Vec<(i32, i32, u32)> = Vec::new();
    let mut loop_at = None;
    let fx = |v: f64| (v * 256.0).round() as i32;
    let slide = |nodes: &mut Vec<(i32, i32, u32)>, a: f64, z: f64, len: u32| {
        let l = len.max(1);
        nodes.push((fx(a), (f64::from(fx(z - a)) / f64::from(l)) as i32, l));
    };
    let f = |s: &str| s.parse::<f64>().map_err(|_| Error(format!("'{s}' isn't a number in a pitch envelope")));
    for w in words(text) {
        if w == "|" {
            loop_at = Some(nodes.len());
        } else if let Some(v) = w.strip_prefix('V') {
            // Vibrato: bias, depth, quarter wave.
            let p: Vec<&str> = v.split(':').collect();
            if p.len() != 3 {
                return err("a vibrato in a pitch envelope is V<bias>:<depth>:<rate>");
            }
            let (bias, depth, rate) = (f(p[0])?, f(p[1])?, num(p[2])?.max(1) as u32);
            slide(&mut nodes, bias, bias + depth / 2.0, rate);
            slide(&mut nodes, bias + depth / 2.0, bias - depth / 2.0, rate * 2);
            slide(&mut nodes, bias - depth / 2.0, bias, rate);
        } else {
            let (body, len) = match w.split_once(':') {
                Some((x, l)) => (x, Some(num(l)?.max(1) as u32)),
                None => (w, None),
            };
            if let Some((a, z)) = body.split_once('>') {
                let (a, z) = (f(a)?, f(z)?);
                slide(&mut nodes, a, z, len.unwrap_or(1));
            } else {
                let a = f(body)?;
                let mut left = len.unwrap_or(1);
                while left > 0 {
                    let c = left.min(256);
                    nodes.push((fx(a), 0, c));
                    left -= c;
                }
            }
        }
    }
    if nodes.is_empty() {
        return err("an empty pitch envelope");
    }
    if nodes.len() > 256 {
        return err("a pitch envelope of more than 256 nodes");
    }
    if nodes.iter().any(|n| n.2 > 256) {
        return err("a pitch slide longer than 256 frames");
    }
    Ok((nodes, loop_at))
}

fn peg_bytes(nodes: &[(i32, i32, u32)], loop_at: Option<usize>, ext: bool) -> Vec<u8> {
    let last = nodes.len() - 1;
    let target = loop_at.unwrap_or(last).min(last);
    let mut b = Vec::new();
    // A last node that doesn't move and has nowhere to go stays (length 255), as ctrmml's does.
    let stay = loop_at.is_none() && nodes[last].1 == 0;
    for (i, &(init, delta, len)) in nodes.iter().enumerate() {
        let len = if stay && i == last { 256 } else { len };
        b.extend((init.clamp(-0x8000, 0x7eff) as i16).to_be_bytes());
        if ext {
            b.extend((delta.clamp(-0x8000, 0x7fff) as i16).to_be_bytes());
            b.push((len - 1) as u8);
            b.push(if i == last { target as u8 } else { (i + 1) as u8 });
        } else {
            b.push(delta.clamp(-128, 127) as i8 as u8);
            b.push((len - 1) as u8);
        }
    }
    if !ext && !stay {
        // The jump after the last node: to the loop, or the last node again.
        b.extend([0x7f, target as u8]);
    }
    b
}

// ---- the track compiler ----

enum Pend {
    None,
    Note { byte: u8, len: i64, slur: bool },
    Rest(i64),
}

struct Frame {
    /// Where the loop's body starts, and the `fd` of a break.
    start: usize,
    brk: Option<usize>,
}

struct MFrame {
    set: usize,
    start: usize,
    brk: Option<usize>,
}

struct Tc<'a> {
    g: &'a mut Global,
    s: Vec<char>,
    p: usize,
    name: String,
    kind: Kind,
    class: Class,
    base_class: Class,
    channel: usize,
    out: Vec<u8>,
    pend: Pend,
    oct: i32,
    len: i64,
    whole: i64,
    q8: i64,
    early: i64,
    shuf: i64,
    shuf_flip: bool,
    key: [i32; 7],
    drum: Option<usize>,
    /// The echo macro: how far back, the volume change, and the notes so far (None is a rest).
    echo: Option<(usize, i64)>,
    hist: Vec<Option<u8>>,
    loops: Vec<Frame>,
    segno: Option<usize>,
    done: bool,
    // Macro tracks.
    nodes: Vec<[u8; 2]>,
    mloops: Vec<MFrame>,
}

const SEMI: [i32; 7] = [9, 11, 0, 2, 4, 5, 7]; // a b c d e f g

impl Tc<'_> {
    fn at(&self) -> String {
        format!("track {}", self.name)
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.p).copied()
    }

    fn ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace()) {
            self.p += 1;
        }
    }

    fn uint(&mut self) -> Option<i64> {
        let st = self.p;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.p += 1;
        }
        if st == self.p {
            return None;
        }
        self.s[st..self.p].iter().collect::<String>().parse().ok()
    }

    /// A number with an optional sign; the flag says there was a sign.
    fn int(&mut self) -> Option<(i64, bool)> {
        let st = self.p;
        let sign = match self.peek() {
            Some('+') => Some(1),
            Some('-') => Some(-1),
            _ => None,
        };
        if sign.is_some() {
            self.p += 1;
        }
        match self.uint() {
            Some(n) => Some((n * sign.unwrap_or(1), sign.is_some())),
            None => {
                self.p = st;
                None
            }
        }
    }

    fn need(&mut self, what: &str) -> Result<i64, Error> {
        match self.int() {
            Some((n, _)) => Ok(n),
            None => err(format!("{}: {what} needs a number", self.at())),
        }
    }

    /// A length: `4`, `4.`, `:24`, or nothing (the default), with dots.
    fn dur(&mut self, default: i64) -> Result<i64, Error> {
        let mut t = if self.peek() == Some(':') {
            self.p += 1;
            match self.uint() {
                Some(n) => n,
                None => return err(format!("{}: ':' needs a number of ticks", self.at())),
            }
        } else if let Some(n) = self.uint() {
            if n == 0 {
                return err(format!("{}: a length of 0", self.at()));
            }
            (self.whole as f64 / n as f64).round() as i64
        } else {
            default
        };
        if !(0..=100_000).contains(&t) {
            return err(format!("{}: a length of {t} ticks", self.at()));
        }
        let mut add = t / 2;
        while self.peek() == Some('.') {
            self.p += 1;
            t += add;
            add /= 2;
        }
        Ok(t)
    }

    fn raw(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }

    fn rest_bytes(&mut self, mut n: i64) {
        while n > 0 {
            let c = n.min(128);
            self.out.push((c - 1) as u8);
            n -= c;
        }
    }

    fn flush(&mut self) {
        match std::mem::replace(&mut self.pend, Pend::None) {
            Pend::None => {}
            Pend::Rest(n) => self.rest_bytes(n),
            Pend::Note { byte, len, slur } => {
                if len <= 0 {
                    return;
                }
                let sound = if slur {
                    len
                } else if self.early > 0 {
                    (len - self.early).max(1)
                } else {
                    (len * self.q8 / 8).max(1)
                }
                .min(len);
                let first = sound.min(128);
                self.out.extend([byte, (first - 1) as u8]);
                let mut left = sound - first;
                while left > 0 {
                    let c = left.min(128);
                    self.out.extend([0x81, (c - 1) as u8]);
                    left -= c;
                }
                self.rest_bytes(len - sound);
                if slur {
                    self.out.push(0xe0);
                }
            }
        }
    }

    /// The shuffle: ticks added to one event and taken from the next.
    fn shuffled(&mut self, t: i64) -> i64 {
        if self.shuf == 0 {
            return t;
        }
        self.shuf_flip = !self.shuf_flip;
        (if self.shuf_flip { t + self.shuf } else { t - self.shuf }).max(1)
    }

    fn note_value(&self, letter: usize, acc: i32) -> i32 {
        let mut n = (self.oct - 1) * 12 + SEMI[letter] + acc;
        if self.class == Class::Psg {
            n -= PSG_SHIFT;
        }
        n
    }

    /// A note letter's accidentals: explicit ones, or the key signature's.
    fn accidental(&mut self, letter: usize) -> i32 {
        let (mut acc, mut any) = (0, false);
        while let Some(c) = self.peek() {
            match c {
                '+' => acc += 1,
                '-' => acc -= 1,
                '=' => {}
                _ => break,
            }
            any = true;
            self.p += 1;
        }
        if any {
            acc
        } else {
            self.key[letter]
        }
    }

    fn run(&mut self) -> Result<(), Error> {
        while !self.done {
            self.ws();
            let Some(c) = self.peek() else { break };
            self.p += 1;
            if self.kind == Kind::Macro {
                self.macro_cmd(c)?;
            } else {
                self.cmd(c)?;
            }
        }
        Ok(())
    }

    fn letter(c: char) -> Option<usize> {
        match c {
            'a'..='g' => Some(c as usize - 'a' as usize),
            'h' => Some(1),
            _ => None,
        }
    }

    fn cmd(&mut self, c: char) -> Result<(), Error> {
        match c {
            '|' => {}
            'a'..='h' => {
                let l = Self::letter(c).unwrap();
                if let Some(d) = self.drum {
                    // Drum mode: a to h are 0 to 7 added to the drum's number; each has its slot.
                    self.flush();
                    let k = c as usize - 'a' as usize;
                    let Some(&base) = self.g.drum_base.get(&d) else { return err(format!("{}: drum {d} wasn't found", self.at())) };
                    let slot = base + k;
                    self.g.drum_src.insert(slot, d + k);
                    self.g.slot(Key::Drum(slot), self.class)?;
                    let len = self.dur(self.len)?;
                    self.pend = Pend::Note { byte: 0x82 + slot as u8, len: self.shuffled(len), slur: false };
                    return Ok(());
                }
                let acc = self.accidental(l);
                let n = self.note_value(l, acc);
                let len = self.dur(self.len)?;
                let len = self.shuffled(len);
                if self.kind == Kind::Drum {
                    if !(0..=255).contains(&n) {
                        return err(format!("{}: note out of range", self.at()));
                    }
                    self.raw(&[0xf7, n as u8]);
                    self.done = true;
                    return Ok(());
                }
                if !(0..=LAST_NOTE).contains(&n) {
                    return err(format!("{}: a note ({n}) outside the driver's range (0 to {LAST_NOTE})", self.at()));
                }
                self.flush();
                self.hist.push(Some(0x82 + n as u8));
                self.pend = Pend::Note { byte: 0x82 + n as u8, len, slur: false };
            }
            'r' => {
                let len = self.dur(self.len)?;
                self.hist.push(None);
                let len = self.shuffled(len);
                match &mut self.pend {
                    Pend::Rest(n) => *n += len,
                    _ => {
                        self.flush();
                        self.pend = Pend::Rest(len);
                    }
                }
            }
            '^' => {
                let len = self.dur(self.len)?;
                let len = self.shuffled(len);
                match &mut self.pend {
                    Pend::Note { len: n, .. } | Pend::Rest(n) => *n += len,
                    Pend::None => {
                        // After a loop's end or a command: a tie of its own.
                        let mut left = len;
                        while left > 0 {
                            let c = left.min(128);
                            self.out.extend([0x81, (c - 1) as u8]);
                            left -= c;
                        }
                    }
                }
            }
            '&' => {
                if let Pend::Note { slur, .. } = &mut self.pend {
                    *slur = true;
                }
                self.flush();
            }
            'R' => {
                let d = self.dur(self.len)?;
                match &mut self.pend {
                    Pend::Note { len: n, .. } | Pend::Rest(n) if *n > d => *n -= d,
                    _ => {} // nothing to take it from: ctrmml warns and goes on
                }
            }
            '~' => {
                // A grace note: its length is taken from the note before.
                let Some(lc) = self.peek().and_then(Self::letter) else { return err(format!("{}: '~' needs a note", self.at())) };
                self.p += 1;
                let acc = self.accidental(lc);
                let n = self.note_value(lc, acc);
                let d = self.dur(self.len)?;
                if !(0..=LAST_NOTE).contains(&n) {
                    return err(format!("{}: a note outside the driver's range", self.at()));
                }
                match &mut self.pend {
                    Pend::Note { len, .. } if *len > d => *len -= d,
                    _ => return err(format!("{}: a grace note needs a longer note before it", self.at())),
                }
                self.flush();
                self.pend = Pend::Note { byte: 0x82 + n as u8, len: d, slur: false };
            }
            'o' => self.oct = self.need("o")?.clamp(-20, 20) as i32,
            '<' => self.oct = (self.oct - 1).max(-20),
            '>' => self.oct = (self.oct + 1).min(20),
            'l' => self.len = self.dur(self.len)?,
            'Q' => {
                self.flush();
                self.q8 = self.need("Q")?.clamp(1, 8);
            }
            'q' => {
                self.flush();
                self.early = self.need("q")?.max(0);
            }
            'C' => self.whole = self.need("C")?.clamp(1, 10_000),
            's' => {
                self.shuf = self.need("s")?.clamp(-1000, 1000);
                self.shuf_flip = false;
            }
            '[' => {
                self.flush();
                self.out.push(0xfa);
                self.loops.push(Frame { start: self.out.len(), brk: None });
            }
            '/' => {
                self.flush();
                let Some(f) = self.loops.last_mut() else { return err(format!("{}: '/' outside a loop", self.at())) };
                f.brk = Some(self.out.len());
                self.out.extend([0xfd, 0, 0]);
            }
            ']' => {
                self.flush();
                let n = self.uint().unwrap_or(2);
                if n > 255 {
                    return err(format!("{}: a loop count over 255", self.at()));
                }
                let Some(f) = self.loops.pop() else { return err(format!("{}: ']' without '['", self.at())) };
                if n == 1 {
                    // One pass: no loop, and the break's tail is left out.
                    if let Some(b) = f.brk {
                        self.out.truncate(b);
                    }
                    self.out.remove(f.start - 1);
                    if let Some(s) = self.segno.as_mut() {
                        if *s >= f.start {
                            *s -= 1;
                        }
                    }
                    for o in &mut self.loops {
                        if let Some(b) = o.brk.as_mut() {
                            if *b >= f.start {
                                *b -= 1;
                            }
                        }
                    }
                } else {
                    self.out.extend([0xfb, n as u8]);
                    if let Some(b) = f.brk {
                        let w = (self.out.len() - (b + 3)) as u16;
                        self.out[b + 1..b + 3].copy_from_slice(&w.to_be_bytes());
                    }
                }
            }
            'L' => {
                self.flush();
                if !self.loops.is_empty() {
                    return err(format!("{}: 'L' inside a loop", self.at()));
                }
                self.segno = Some(self.out.len());
            }
            '*' => {
                let n = self.need("*")?;
                self.flush();
                if self.kind == Kind::Macro || !(0..=255).contains(&n) {
                    return err(format!("{}: bad subroutine '*{n}'", self.at()));
                }
                if !self.g.tracks.contains_key(&(n as u32)) {
                    return err(format!("{}: '*{n}' has no commands", self.at()));
                }
                let s = self.g.slot(Key::Pat(n as u32, self.class), self.class)?;
                self.raw(&[0xfe, s as u8]);
            }
            '@' => {
                let n = self.need("@")? as u32;
                self.flush();
                match self.g.insts.get(&n) {
                    None => return err(format!("{}: instrument @{n} isn't defined", self.at())),
                    Some(Inst::Pcm { .. }) => {
                        let s = self.g.slot(Key::Inst(n), self.class)?;
                        self.raw(&[0xf0, s as u8]);
                    }
                    Some(i) => {
                        let psg = matches!(i, Inst::Psg(_));
                        if psg != (self.class == Class::Psg) {
                            return err(format!("{}: @{n} is {} but this channel is {}", self.at(), if psg { "a PSG envelope" } else { "an FM voice" }, if psg { "FM" } else { "PSG" }));
                        }
                        let s = self.g.slot(Key::Inst(n), self.class)?;
                        self.raw(&[0xe1, s as u8]);
                    }
                }
            }
            'v' => {
                let n = self.need("v")?;
                self.flush();
                if !(0..=15).contains(&n) {
                    return err(format!("{}: v takes 0 to 15", self.at()));
                }
                self.raw(&[0xe2, 0x80 + n as u8]);
            }
            'V' => {
                self.flush();
                let Some((n, signed)) = self.int() else { return err(format!("{}: V needs a number", self.at())) };
                if signed {
                    self.raw(&[0xe3, n.clamp(-128, 127) as i8 as u8]);
                } else {
                    self.raw(&[0xe2, n.clamp(0, 127) as u8]);
                }
            }
            '(' | ')' => {
                self.flush();
                let n = self.uint().unwrap_or(1);
                let d = if c == ')' { n } else { -n };
                self.raw(&[0xe3, d.clamp(-128, 127) as i8 as u8]);
            }
            'p' => {
                let n = self.need("p")?;
                self.flush();
                if !(0..=3).contains(&n) {
                    return err(format!("{}: p takes 0 to 3", self.at()));
                }
                let v = if n & 2 != 0 { 0x80 } else { 0 } | if n & 1 != 0 { 0x40 } else { 0 };
                self.raw(&[0xe9, v]);
            }
            '_' => {
                self.flush();
                if self.peek() == Some('{') {
                    self.p += 1;
                    let st = self.p;
                    while self.peek().is_some_and(|c| c != '}') {
                        self.p += 1;
                    }
                    let body: String = self.s[st..self.p].iter().collect();
                    self.p += 1;
                    self.key_signature(&body)?;
                } else if self.peek() == Some('_') {
                    self.p += 1;
                    let n = self.need("__")?;
                    self.raw(&[0xe5, n.clamp(-128, 127) as i8 as u8]);
                } else {
                    let n = self.need("_")?;
                    self.raw(&[0xe4, n.clamp(-128, 127) as i8 as u8]);
                }
            }
            'k' => {
                let n = self.need("k")?;
                self.flush();
                self.raw(&[0xe4, n as u8]);
            }
            'K' => {
                let n = self.need("K")?;
                self.flush();
                self.raw(&[0xe6, n as u8]);
            }
            'G' => {
                let n = self.need("G")?;
                self.flush();
                self.raw(&[0xe7, n as u8]);
            }
            'M' => {
                let n = self.need("M")?;
                self.flush();
                if n == 0 {
                    self.raw(&[0xe8, 0]);
                } else {
                    if !self.g.pegs.contains_key(&(n as u32)) {
                        return err(format!("{}: pitch envelope @M{n} isn't defined", self.at()));
                    }
                    let s = self.g.slot(Key::Peg(n as u32), self.class)?;
                    self.raw(&[0xe8, s as u8 + 1]);
                }
            }
            'P' => {
                let n = self.need("P")?;
                self.flush();
                if n == 0 {
                    self.raw(&[0xeb, 0]);
                } else {
                    if !self.g.tracks.contains_key(&(n as u32)) {
                        return err(format!("{}: macro track *{n} has no commands", self.at()));
                    }
                    let s = self.g.slot(Key::Mtab(n as u32), self.class)?;
                    self.raw(&[0xeb, s as u8 + 1]);
                }
            }
            'D' => {
                let n = self.need("D")?;
                self.flush();
                if !(0..=LAST_NOTE as i64).contains(&n) {
                    return err(format!("{}: D takes 0 to {LAST_NOTE}", self.at()));
                }
                if n == 0 {
                    self.drum = None;
                    self.raw(&[0xec, 0x00]);
                } else {
                    self.drum = Some(n as usize);
                    self.raw(&[0xec, 0x08]);
                }
            }
            't' => {
                let n = self.need("t")?;
                self.flush();
                let dd = (((n as f64) * 256.0 / 300.0).round().clamp(1.0, 256.0) as u32 - 1) as u8;
                self.raw(&[0xf9, dd]);
            }
            'T' => {
                let n = self.need("T")?;
                self.flush();
                self.raw(&[0xf9, n.clamp(0, 255) as u8]);
            }
            'E' => return err(format!("{}: E (envelopes) isn't used by MDSDRV: PSG envelopes are instruments (@n)", self.at())),
            '\\' => {
                // The echo macro: `\=2,3` sets the delay and the volume change, `\8` plays the note that far back.
                if self.peek() == Some('=') {
                    self.p += 1;
                    let d = self.need("\\=")?;
                    if self.peek() == Some(',') {
                        self.p += 1;
                    }
                    let v = self.need("\\=")?;
                    if d >= 0 {
                        self.hist.clear();
                    }
                    self.echo = Some((d.unsigned_abs() as usize, v));
                } else {
                    let len = self.dur(self.len)?;
                    let len = self.shuffled(len);
                    let Some((delay, vol)) = self.echo else { return err(format!("{}: '\\' before '\\=' has set the echo", self.at())) };
                    self.flush();
                    let note = self.hist.len().checked_sub(delay).and_then(|i| self.hist.get(i)).copied().flatten();
                    if vol != 0 {
                        self.raw(&[0xe3, (-vol) as i8 as u8]);
                    }
                    self.pend = match note {
                        Some(byte) => Pend::Note { byte, len, slur: false },
                        None => Pend::Rest(len),
                    };
                    self.flush();
                    if vol != 0 {
                        self.raw(&[0xe3, vol as i8 as u8]);
                    }
                }
            }
            '\'' => {
                let st = self.p;
                while self.peek().is_some_and(|c| c != '\'') {
                    self.p += 1;
                }
                let body: String = self.s[st..self.p].iter().collect();
                self.p += 1;
                self.flush();
                self.platform(&body)?;
            }
            _ => return err(format!("{}: '{c}' isn't a command", self.at())),
        }
        Ok(())
    }

    fn key_signature(&mut self, body: &str) -> Result<(), Error> {
        let cs: Vec<char> = body.trim().chars().collect();
        if cs.is_empty() {
            return err(format!("{}: an empty key signature", self.at()));
        }
        // letter index in a..g order
        let li = |c: char| ('a'..='g').position(|x| x == c.to_ascii_lowercase());
        if matches!(cs[0], '+' | '-' | '=') {
            let mut mode = 0;
            for &c in &cs {
                match c {
                    '+' => mode = 1,
                    '-' => mode = -1,
                    '=' => mode = 0,
                    c if li(c).is_some() => self.key[li(c).unwrap()] = mode,
                    c if c.is_whitespace() => {}
                    c => return err(format!("{}: '{c}' in a key signature", self.at())),
                }
            }
            return Ok(());
        }
        let name: String = cs.iter().collect();
        let name = name.trim().to_string();
        const FIFTHS: [(&str, i32); 31] = [
            ("C", 0), ("G", 1), ("D", 2), ("A", 3), ("E", 4), ("B", 5), ("F+", 6), ("C+", 7), ("F", -1), ("B-", -2), ("E-", -3), ("A-", -4), ("D-", -5), ("G-", -6), ("C-", -7), ("a", 0), ("e", 1),
            ("b", 2), ("f+", 3), ("c+", 4), ("g+", 5), ("d+", 6), ("a+", 7), ("d", -1), ("g", -2), ("c", -3), ("f", -4), ("b-", -5), ("e-", -6), ("a-", -7), ("g-", -6),
        ];
        let Some(&(_, n)) = FIFTHS.iter().find(|(k, _)| *k == name) else { return err(format!("{}: '{name}' isn't a key", self.at())) };
        self.key = [0; 7];
        let sharps = ['f', 'c', 'g', 'd', 'a', 'e', 'b'];
        let flats = ['b', 'e', 'a', 'd', 'g', 'c', 'f'];
        if n > 0 {
            for &c in &sharps[..n as usize] {
                self.key[li(c).unwrap()] = 1;
            }
        } else {
            for &c in &flats[..(-n) as usize] {
                self.key[li(c).unwrap()] = -1;
            }
        }
        Ok(())
    }

    /// The `'...'` commands of the platform.
    fn platform(&mut self, body: &str) -> Result<(), Error> {
        let w: Vec<&str> = words(body).collect();
        let Some(&name) = w.first() else { return err(format!("{}: an empty '...' command", self.at())) };
        let arg = |i: usize| -> Result<i32, Error> { w.get(i).ok_or_else(|| Error(format!("{}: '{name}' needs a value", self.at()))).and_then(|s| num(s)) };
        match name {
            "fm3" => {
                let m = w.get(1).ok_or_else(|| Error(format!("{}: 'fm3' needs a mask", self.at())))?;
                let mask = u8::from_str_radix(m, 2).map_err(|_| Error(format!("{}: 'fm3' takes four 0/1 digits", self.at())))? & 15;
                // The driver's mask has a bit for each operator that is *not* this track's.
                self.raw(&[0xec, 0x80 | (!mask & 15)]);
                if mask == 15 {
                    self.class = self.base_class;
                } else {
                    // The track plays FM3's operators: its notes are FM's.
                    self.class = Class::Fm;
                }
            }
            "lfo" => {
                let (a, p) = (arg(1)?, arg(2)?);
                if self.class == Class::Fm {
                    self.raw(&[0xea, ((a & 3) << 4 | (p & 7)) as u8]);
                }
            }
            "lforate" => {
                let n = arg(1)?;
                self.raw(&[0xf6, 0x22, if n <= 0 { 0 } else { (8 + n - 1) as u8 }]);
            }
            "mode" => {
                let n = arg(1)?;
                self.raw(&[0xea, if n == 0 { 0x00 } else { 0xe7 }]);
            }
            "pcmmode" => {
                let n = arg(1)?;
                self.raw(&[0xf2, n.clamp(2, 3) as u8]);
            }
            "pcmrate" => {
                let n = arg(1)?;
                self.raw(&[0xf1, n.clamp(1, 8) as u8]);
            }
            "write" => {
                let r = self.reg(w.get(1).copied().unwrap_or(""))?;
                let d = arg(2)?;
                // The registers below 0x30 are the chip's own (not a channel's): written as they are.
                self.raw(&[if r < 0x30 { 0xf6 } else { 0xed }, r as u8, d as u8]);
            }
            n if n.len() == 3 && n.starts_with("tl") && n.as_bytes()[2].is_ascii_digit() => {
                let op = (n.as_bytes()[2] - b'0') as usize;
                if !(1..=4).contains(&op) {
                    return err(format!("{}: 'tl{op}': the operators are 1 to 4", self.at()));
                }
                let slot = [0u8, 2, 1, 3][op - 1];
                let v = w.get(1).ok_or_else(|| Error(format!("{}: '{n}' needs a value", self.at())))?;
                let signed = v.starts_with('+') || v.starts_with('-');
                let d = num(v)?;
                self.raw(&[if signed { 0xef } else { 0xee }, slot, d as i8 as u8]);
            }
            "carry" => return err(format!("{}: 'carry' belongs in a macro track", self.at())),
            _ => return err(format!("{}: the platform has no '{name}' command", self.at())),
        }
        Ok(())
    }

    /// An FM register: a number, or `dtml1`, `tl3`, `fbal`...
    fn reg(&self, s: &str) -> Result<i32, Error> {
        if let Ok(n) = s.strip_prefix("0x").map_or_else(|| s.parse::<i32>(), |h| i32::from_str_radix(h, 16)) {
            return Ok(n);
        }
        if s == "fbal" {
            return Ok(0xb0);
        }
        let (base, op) = if let Some(i) = s.find(|c: char| c.is_ascii_digit()) { (&s[..i], s[i..].parse::<usize>().unwrap_or(0)) } else { (s, 0) };
        let b = match base {
            "dtml" => 0x30,
            "tl" => 0x40,
            "ksar" => 0x50,
            "amdr" => 0x60,
            "sr" => 0x70,
            "slrr" => 0x80,
            "ssg" => 0x90,
            _ => return err(format!("{}: '{s}' isn't a register", self.at())),
        };
        if !(1..=4).contains(&op) {
            return err(format!("{}: '{s}': the operators are 1 to 4", self.at()));
        }
        Ok(b + [0, 8, 4, 12][op - 1])
    }

    // ---- macro tracks ----

    fn node(&mut self, c: u8, a: u8) -> Result<(), Error> {
        if self.nodes.len() >= 255 {
            return err(format!("{}: a macro track of more than 255 nodes", self.at()));
        }
        self.nodes.push([c, a]);
        Ok(())
    }

    fn wait(&mut self, first: u8, mut n: i64) -> Result<(), Error> {
        let mut cmd = first;
        while n > 0 {
            let c = n.min(256);
            self.node(cmd, (c - 1) as u8)?;
            cmd = 0x81;
            n -= c;
        }
        Ok(())
    }

    fn macro_cmd(&mut self, c: char) -> Result<(), Error> {
        match c {
            '|' => {}
            'a'..='h' => {
                // A note retriggers the playing one; its pitch isn't used.
                let l = Self::letter(c).unwrap();
                let _ = self.accidental(l);
                let d = self.dur(self.len)?;
                self.wait(0x82, d)?;
            }
            'r' => {
                let d = self.dur(self.len)?;
                self.wait(0x81, d)?;
            }
            'l' => self.len = self.dur(self.len)?,
            'o' | '<' | '>' | 'Q' | 'q' | 'C' | 's' => {
                if matches!(c, 'o' | 'Q' | 'q' | 'C' | 's') {
                    let _ = self.need("the command")?;
                }
            }
            'v' => {
                let n = self.need("v")?;
                self.node(0x18, 0x80 + n.clamp(0, 15) as u8)?;
            }
            'V' => {
                let Some((n, signed)) = self.int() else { return err(format!("{}: V needs a number", self.at())) };
                if signed {
                    self.node(0x58, n as i8 as u8)?;
                } else {
                    self.node(0x18, n.clamp(0, 127) as u8)?;
                }
            }
            '(' | ')' => {
                let n = self.uint().unwrap_or(1);
                self.node(0x58, (if c == ')' { n } else { -n }) as i8 as u8)?;
            }
            'K' => {
                let n = self.need("K")?;
                self.node(0x11, n as u8)?;
            }
            'k' | '_' => {
                let rel = c == '_' && self.peek() == Some('_');
                if rel {
                    self.p += 1;
                }
                let n = self.need("transpose")?;
                self.node(if rel { 0x56 } else { 0x16 }, n as i8 as u8)?;
            }
            'G' => {
                let n = self.need("G")?;
                self.node(0x17, n as u8)?;
            }
            'p' => {
                let n = self.need("p")?;
                let v = if n & 2 != 0 { 0x80 } else { 0 } | if n & 1 != 0 { 0x40 } else { 0 };
                self.node(0x87, v)?;
            }
            'L' => self.segno = Some(self.nodes.len()),
            '[' => {
                let set = self.nodes.len();
                self.node(0x84, 0)?;
                self.mloops.push(MFrame { set, start: self.nodes.len(), brk: None });
            }
            '/' => {
                let at = self.nodes.len();
                self.node(0x85, 0)?;
                match self.mloops.last_mut() {
                    Some(f) => f.brk = Some(at),
                    None => return err(format!("{}: '/' outside a loop", self.at())),
                }
            }
            ']' => {
                let n = self.uint().unwrap_or(2);
                let Some(f) = self.mloops.pop() else { return err(format!("{}: ']' without '['", self.at())) };
                if !(1..=256).contains(&n) {
                    return err(format!("{}: a macro loop count of 1 to 256", self.at()));
                }
                let at = self.nodes.len();
                let back = f.start as i64 - (at as i64 + 1);
                if back < -128 {
                    return err(format!("{}: a macro loop body that long", self.at()));
                }
                self.node(0x86, back as i8 as u8)?;
                self.nodes[f.set][1] = (n - 1) as u8;
                if let Some(b) = f.brk {
                    self.nodes[b][1] = (at - b) as u8;
                }
            }
            '\'' => {
                let st = self.p;
                while self.peek().is_some_and(|c| c != '\'') {
                    self.p += 1;
                }
                let body: String = self.s[st..self.p].iter().collect();
                self.p += 1;
                self.macro_platform(&body)?;
            }
            _ => return err(format!("{}: '{c}' can't be used in a macro track", self.at())),
        }
        Ok(())
    }

    fn macro_platform(&mut self, body: &str) -> Result<(), Error> {
        let w: Vec<&str> = words(body).collect();
        let Some(&name) = w.first() else { return err(format!("{}: an empty '...' command", self.at())) };
        let a = |i: usize| -> Result<i32, Error> { w.get(i).ok_or_else(|| Error(format!("{}: '{name}' needs a value", self.at()))).and_then(|s| num(s)) };
        match name {
            "carry" => self.node(0x83, 0)?,
            "lfo" => {
                let (x, p) = (a(1)?, a(2)?);
                self.node(0x88, ((x & 3) << 4 | (p & 7)) as u8)?;
            }
            "mode" => {
                let n = a(1)?;
                self.node(0x89, if n == 0 { 0x00 } else { 0xe7 })?;
            }
            "write" => {
                let r = self.reg(w.get(1).copied().unwrap_or(""))?;
                if r & 3 != 0 || !(0..=0xfc).contains(&r) {
                    return err(format!("{}: a macro track can write only registers that are multiples of 4", self.at()));
                }
                self.node(0xc0 | (r >> 2) as u8, a(2)? as u8)?;
            }
            n if n.len() == 3 && n.starts_with("tl") && (b'1'..=b'4').contains(&n.as_bytes()[2]) => {
                let op = (n.as_bytes()[2] - b'1') as usize;
                let slot = [0u8, 2, 1, 3][op];
                let v = w.get(1).ok_or_else(|| Error(format!("{}: '{n}' needs a value", self.at())))?;
                let signed = v.starts_with('+') || v.starts_with('-');
                self.node(if signed { 0x76 } else { 0x36 } + slot, num(v)? as i8 as u8)?;
            }
            _ => return err(format!("{}: '{name}' can't be used in a macro track", self.at())),
        }
        Ok(())
    }
}

/// Compiles one track's text. Returns its code (or macro nodes as bytes).
fn compile_track(g: &mut Global, text: &str, kind: Kind, class: Class, channel: usize, name: &str) -> Result<Vec<u8>, Error> {
    let mut t = Tc {
        g,
        s: text.chars().collect(),
        p: 0,
        name: name.to_string(),
        kind,
        class,
        base_class: class,
        channel,
        out: Vec::new(),
        pend: Pend::None,
        oct: 6,
        len: 24,
        whole: 96,
        q8: 8,
        early: 0,
        shuf: 0,
        shuf_flip: false,
        key: [0; 7],
        drum: None,
        echo: None,
        hist: Vec::new(),
        loops: Vec::new(),
        segno: None,
        done: false,
        nodes: Vec::new(),
        mloops: Vec::new(),
    };
    let _ = t.channel;
    t.run()?;
    if kind == Kind::Macro {
        if !t.mloops.is_empty() {
            return err(format!("{}: a '[' without its ']'", t.at()));
        }
        let at = t.nodes.len();
        match t.segno {
            Some(s) => {
                let back = s as i64 - (at as i64 + 1);
                if !(-128..0).contains(&back) {
                    return err(format!("{}: the macro's loop is too long", t.at()));
                }
                t.node(0x80, back as i8 as u8)?;
            }
            _ => t.node(0x80, 0)?,
        }
        return Ok(t.nodes.iter().flatten().copied().collect());
    }
    if !t.loops.is_empty() {
        return err(format!("{}: a '[' without its ']'", t.at()));
    }
    t.flush();
    match kind {
        Kind::Drum => {
            if !t.done {
                return err(format!("{}: a drum subroutine needs a note", t.at()));
            }
        }
        Kind::Pat => t.out.push(0xff),
        _ => match t.segno {
            Some(s) => {
                let addr = t.out.len() as i64;
                let w = (s as i64 - (addr + 3)) as i16;
                t.out.push(0xf5);
                t.out.extend(w.to_be_bytes());
            }
            None => t.out.push(0xff),
        },
    }
    Ok(t.out)
}

fn class_of(track: u32) -> Class {
    if (6..=9).contains(&track) {
        Class::Psg
    } else {
        Class::Fm
    }
}

fn track_name(n: u32) -> String {
    if n < 26 {
        ((b'A' + n as u8) as char).to_string()
    } else {
        format!("*{n}")
    }
}

// ---- the file ----

fn parse_wav(d: &[u8]) -> Option<(Vec<u8>, u32)> {
    if d.get(..4)? != b"RIFF" || d.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut at, mut fmt, mut data) = (12, None, None);
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let size = u32::from_le_bytes(d[at + 4..at + 8].try_into().ok()?) as usize;
        let body = d.get(at + 8..(at + 8 + size).min(d.len()))?;
        match id {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    let f = fmt.filter(|f| f.len() >= 16)?;
    let ch = u16::from_le_bytes([f[2], f[3]]) as usize;
    let rate = u32::from_le_bytes(f[4..8].try_into().ok()?);
    let bits = u16::from_le_bytes([f[14], f[15]]) as usize;
    let data = data?;
    let step = ch.max(1) * bits / 8;
    let mut out = Vec::new();
    for fr in data.chunks_exact(step.max(1)) {
        out.push(match bits {
            8 => fr[0],
            16 => ((i16::from_le_bytes([fr[0], fr[1]]) >> 8) + 128) as u8,
            24 => ((i16::from_le_bytes([fr[1], fr[2]]) >> 8) + 128) as u8,
            _ => return None,
        });
    }
    Some((out, rate))
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    if body.len() & 1 == 1 {
        v.push(0);
    }
    v
}

/// Compiles MDSDRV MML to a `.mds` file. A `@n pcm "file"` instrument is an error: use `compile_with`.
pub fn compile(src: &str) -> Result<Vec<u8>, Error> {
    compile_with(src, |_| None)
}

/// As `compile`, with `load` giving the bytes of a WAV file by the path the MML names.
pub fn compile_with(src: &str, load: impl FnMut(&str) -> Option<Vec<u8>> + 'static) -> Result<Vec<u8>, Error> {
    let mut g = Global {
        insts: BTreeMap::new(),
        pegs: BTreeMap::new(),
        tracks: BTreeMap::new(),
        ext_peg: true,
        slots: HashMap::new(),
        used: Vec::new(),
        reserved: Vec::new(),
        drum_base: HashMap::new(),
        drum_src: HashMap::new(),
        queue: Vec::new(),
        code: BTreeMap::new(),
        globs: Vec::new(),
        pcmh: Vec::new(),
        pcmd: Vec::new(),
        load: Box::new(load),
    };
    let lines: Vec<&str> = src.lines().map(strip_comment).collect();
    let mut cur: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let at = i + 1;
        i += 1;
        if l.trim().is_empty() {
            continue;
        }
        let first = l.chars().next().unwrap();
        let with_line = |e: Error| Error(format!("line {at}: {}", e.0));
        if first == '#' {
            let mut w = words(&l[1..]);
            match w.next() {
                Some("option") => {
                    if w.any(|o| o == "noextpitch") {
                        g.ext_peg = false;
                    }
                }
                Some("platform") => {
                    let p = w.next().unwrap_or("");
                    if p != "mdsdrv" && p != "megadrive" {
                        return err(format!("line {at}: #platform {p}: only mdsdrv and megadrive are supported"));
                    }
                }
                _ => {}
            }
        } else if first == '@' {
            // A definition: the rest of its line, and the indented lines after it.
            let mut body = l[1..].to_string();
            let fm = body.split_whitespace().nth(1).is_some_and(|k| k == "fm");
            while i < lines.len() {
                let n = lines[i];
                let t = n.trim();
                let numeric = t.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '+');
                let count = words(&body).skip(2).count();
                let cont = if fm {
                    // 2 + 40 numbers, then perhaps the transpose.
                    (count < 42 && (t.is_empty() || numeric)) || (count == 42 && numeric)
                } else {
                    !t.is_empty() && n.starts_with(char::is_whitespace)
                };
                if !cont {
                    break;
                }
                body.push(' ');
                body.push_str(n);
                i += 1;
            }
            let mut w = words(&body);
            let head = w.next().unwrap_or("");
            let rest: Vec<&str> = w.collect();
            if let Some(n) = head.strip_prefix('M') {
                let n = num(n).map_err(with_line)? as u32;
                g.pegs.insert(n, rest.join(" "));
            } else if head.starts_with('E') || head.starts_with('P') {
                // Envelopes and pan envelopes aren't used by this platform.
            } else {
                let n = num(head).map_err(with_line)? as u32;
                let Some(&kind) = rest.first() else { return err(format!("line {at}: @{n} needs a type (fm, 2op, psg or pcm)")) };
                let inst = match kind {
                    "fm" => parse_fm(&rest[1..]).map_err(with_line)?,
                    "2op" => {
                        let v: Vec<i32> = rest[1..].iter().map(|x| num(x)).collect::<Result<_, _>>().map_err(with_line)?;
                        if v.len() < 5 {
                            return err(format!("line {at}: 2op takes an instrument, four multiples and a transpose"));
                        }
                        Inst::TwoOp { base: v[0] as u32, ml: [v[1], v[2], v[3], v[4]], trs: v.get(5).copied().unwrap_or(0) }
                    }
                    "psg" => Inst::Psg(psg_env(&rest[1..].join(" ")).map_err(with_line)?),
                    "pcm" => {
                        let text = body.split_once("pcm").map(|x| x.1).unwrap_or("");
                        let path = text.split('"').nth(1).map(str::to_string).ok_or_else(|| Error(format!("line {at}: pcm needs a file name in quotes")))?;
                        let after = text.rsplit('"').next().unwrap_or("");
                        let get = |k: &str| after.split_whitespace().find_map(|w| w.strip_prefix(k)).and_then(|v| v.parse::<u32>().ok());
                        Inst::Pcm { path, rate: get("rate="), offset: get("offset=").unwrap_or(0) }
                    }
                    k => return err(format!("line {at}: instrument type '{k}' isn't one of fm, 2op, psg, pcm")),
                };
                g.insts.insert(n, inst);
            }
        } else {
            // Tracks, then commands; or an indented continuation.
            let (list, cmds) = if first.is_whitespace() { (None, l.trim_start()) } else { l.split_once(char::is_whitespace).map(|(a, b)| (Some(a), b)).unwrap_or((Some(l), "")) };
            if let Some(list) = list {
                cur.clear();
                let cs: Vec<char> = list.chars().collect();
                let mut p = 0;
                while p < cs.len() {
                    match cs[p] {
                        '*' => {
                            let st = p + 1;
                            p += 1;
                            while p < cs.len() && cs[p].is_ascii_digit() {
                                p += 1;
                            }
                            let n: String = cs[st..p].iter().collect();
                            cur.push(n.parse().map_err(|_| Error(format!("line {at}: '*' needs a track number")))?);
                        }
                        c @ 'A'..='Z' => {
                            cur.push(c as u32 - 'A' as u32);
                            p += 1;
                        }
                        c => return err(format!("line {at}: '{c}' where a track was expected (A to Z, or *n)")),
                    }
                }
            }
            if cur.is_empty() {
                return err(format!("line {at}: commands with no track selected"));
            }
            for (k, &t) in cur.iter().enumerate() {
                let text = conditional(cmds, k).map_err(with_line)?;
                let e = g.tracks.entry(t).or_default();
                e.push(' ');
                e.push_str(&text);
            }
        }
    }
    // Drum mode: each D<n> gets a block of consecutive slots, as many as its highest letter needs.
    g.reserved = vec![false; 256];
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    for t in g.tracks.values() {
        let cs: Vec<char> = t.chars().collect();
        let (mut cur, mut quote, mut i) = (None::<usize>, false, 0);
        while i < cs.len() {
            let c = cs[i];
            if c == '\'' {
                quote = !quote;
            } else if !quote && c == 'D' && cs.get(i + 1).is_some_and(char::is_ascii_digit) {
                let n: String = cs[i + 1..].iter().take_while(|c| c.is_ascii_digit()).collect();
                i += n.len();
                cur = n.parse::<usize>().ok().filter(|&n| n > 0 && n <= LAST_NOTE as usize);
            } else if !quote && ('a'..='h').contains(&c) {
                if let Some(d) = cur {
                    let k = c as usize - 'a' as usize + 1;
                    match blocks.iter_mut().find(|b| b.0 == d) {
                        Some(b) => b.1 = b.1.max(k),
                        None => blocks.push((d, k)),
                    }
                }
            }
            i += 1;
        }
    }
    let mut next = 0;
    for (d, size) in blocks {
        if next + size > 255 {
            return err("the drums need more than 255 slots in the song data table");
        }
        g.drum_base.insert(d, next);
        for k in next..next + size {
            g.reserved[k] = true;
        }
        next += size;
    }
    // The tracks that play.
    let players: Vec<u32> = g.tracks.iter().filter(|(&n, t)| n < 16 && !t.trim().is_empty()).map(|(&n, _)| n).collect();
    if players.is_empty() {
        return err("no track plays: tracks A to P hold no commands");
    }
    let mut track_code: Vec<(u32, Vec<u8>)> = Vec::new();
    for &n in &players {
        let text = g.tracks[&n].clone();
        let code = compile_track(&mut g, &text, Kind::Track, class_of(n), n as usize, &track_name(n))?;
        track_code.push((n, code));
    }
    // What they use: subroutines, drums, macro tables, instruments, envelopes.
    while let Some((k, class)) = g.queue.pop() {
        let slot = g.slots[&k];
        match k {
            Key::Pat(n, _) => {
                let text = g.tracks[&n].clone();
                let c = compile_track(&mut g, &text, Kind::Pat, class, 99, &format!("*{n}"))?;
                g.code.insert(slot, c);
            }
            Key::Drum(_) => {
                let idx = g.drum_src[&slot];
                let Some(text) = g.tracks.get(&(idx as u32)).cloned() else { return err(format!("drum {idx} is used but track *{idx} has no commands")) };
                let c = compile_track(&mut g, &text, Kind::Drum, class, 99, &format!("*{idx}"))?;
                g.code.insert(slot, c);
            }
            Key::Mtab(n) => {
                let text = g.tracks[&n].clone();
                let c = compile_track(&mut g, &text, Kind::Macro, class, 99, &format!("*{n}"))?;
                g.code.insert(slot, c);
            }
            Key::Peg(n) => {
                let (nodes, lp) = peg_nodes(&g.pegs[&n]).map_err(|e| Error(format!("@M{n}: {}", e.0)))?;
                // The short format when every node fits it (as ctrmml does), the extended one when one doesn't.
                let fits = nodes.iter().all(|n| (-128..=127).contains(&n.1) && (-0x8000..=0x7eff).contains(&n.0));
                if !fits && !g.ext_peg {
                    return err(format!("@M{n}: a node needs the extended pitch envelope (#option noextpitch is set)"));
                }
                let ext = !fits;
                g.globs.push((slot as u32 | if ext { 0x8000_0000 } else { 0 }, peg_bytes(&nodes, lp, ext)));
            }
            Key::Inst(n) => {
                let bytes = match &g.insts[&n] {
                    Inst::Fm { alg, fb, ops, trs } => fm_bytes(*alg, *fb, ops, *trs),
                    Inst::TwoOp { base, ml, trs } => {
                        let Some(Inst::Fm { alg, fb, ops, .. }) = g.insts.get(base) else { return err(format!("@{n}: 2op needs an FM instrument @{base}")) };
                        let mut ops = *ops;
                        for (o, m) in ops.iter_mut().zip(ml) {
                            o[7] = *m;
                        }
                        // The carriers that were switched off are switched on.
                        let carriers: &[usize] = match alg & 7 {
                            0..=3 => &[3],
                            4 => &[1, 3],
                            5 | 6 => &[1, 2, 3],
                            _ => &[0, 1, 2, 3],
                        };
                        for &c in carriers {
                            if ops[c][5] >= 127 {
                                ops[c][5] = 0;
                            }
                        }
                        fm_bytes(*alg, *fb, &ops, *trs)
                    }
                    Inst::Psg(b) => b.clone(),
                    Inst::Pcm { path, rate, offset } => {
                        let Some(file) = (g.load)(path) else { return err(format!("@{n}: the sample '{path}' isn't available")) };
                        let Some((data, wav_rate)) = parse_wav(&file) else { return err(format!("@{n}: '{path}' isn't a WAV file of 8, 16 or 24 bits")) };
                        let (path, offset) = (path.clone(), *offset);
                        let rate = rate.unwrap_or(wav_rate);
                        let _ = path;
                        if offset as usize >= data.len() {
                            return err(format!("@{n}: the offset is past the end of the sample"));
                        }
                        let pos = g.pcmd.len() as u32;
                        g.pcmd.extend(&data);
                        g.pcmh.push((slot as u32, pos, offset, data.len() as u32 - offset, rate));
                        continue;
                    }
                };
                g.globs.push((slot as u32, bytes));
            }
        }
    }
    // The sequence: header, track table, song data table, the code.
    let mut maxslot = 0;
    for (i, &u) in g.used.iter().enumerate() {
        if u {
            maxslot = i + 1;
        }
    }
    let tbase = 4 + 4 * track_code.len();
    let mut table = vec![0u8; maxslot * 2];
    let mut body: Vec<u8> = Vec::new();
    let region = tbase + table.len();
    let mut positions = Vec::new();
    for (_, code) in &track_code {
        positions.push(region + body.len() - tbase);
        body.extend(code);
    }
    for (&slot, code) in &g.code {
        let off = region + body.len() - tbase;
        if off > 0x7fff {
            return err("the sequence is too long for the driver's 16-bit offsets");
        }
        table[slot * 2..slot * 2 + 2].copy_from_slice(&(off as u16).to_be_bytes());
        body.extend(code);
    }
    // Sub-tables can start at an odd byte: pad nothing, the driver reads bytes.
    let mut seq = Vec::new();
    seq.extend((tbase as i16).to_be_bytes());
    seq.push(0); // song volume
    seq.push(track_code.len() as u8);
    for ((n, _), p) in track_code.iter().zip(&positions) {
        seq.extend([*n as u8, 0]);
        seq.extend((*p as u16).to_be_bytes());
    }
    seq.extend(&table);
    seq.extend(&body);
    // The file.
    let mut list = b"dblk".to_vec();
    for (id, data) in &g.globs {
        let mut b = id.to_le_bytes().to_vec();
        b.extend(data);
        list.extend(chunk(b"glob", &b));
    }
    for &(id, pos, start, size, rate) in &g.pcmh {
        let mut b = Vec::new();
        for v in [id, pos, start, size, 0, 0, rate, 0, 0] {
            b.extend(v.to_le_bytes());
        }
        list.extend(chunk(b"pcmh", &b));
    }
    let mut body = b"MDS0".to_vec();
    body.extend(chunk(b"ver ", &[0, 4]));
    body.extend(chunk(b"seq ", &seq));
    body.extend(chunk(b"LIST", &list));
    if !g.pcmd.is_empty() {
        body.extend(chunk(b"pcmd", &g.pcmd));
    }
    let mut file = b"RIFF".to_vec();
    file.extend((body.len() as u32).to_le_bytes());
    file.extend(body);
    Ok(file)
}

/// Whether text looks like this dialect: a `#platform` of ours, or an instrument defined as `fm`, `2op`, `psg` or `pcm`.
pub fn looks_like(text: &str) -> bool {
    for l in text.lines().map(strip_comment) {
        let l = l.trim();
        if let Some(p) = l.strip_prefix("#platform") {
            return matches!(p.trim(), "mdsdrv" | "megadrive");
        }
        if let Some(r) = l.strip_prefix('@') {
            let mut w = r.split_whitespace();
            if w.next().is_some_and(|n| n.trim_start_matches('M').chars().all(|c| c.is_ascii_digit())) && matches!(w.next(), Some("fm" | "2op" | "psg" | "pcm")) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdsdrv::Song;

    fn seq_of(file: &[u8]) -> Vec<u8> {
        let mut at = 12;
        while at + 8 <= file.len() {
            let size = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
            if &file[at..at + 4] == b"seq " {
                return file[at + 8..at + 8 + size].to_vec();
            }
            at += 8 + size + (size & 1);
        }
        panic!("no seq")
    }

    /// The first track's code.
    fn code(src: &str) -> Vec<u8> {
        let f = compile(src).unwrap();
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        let pos = u16::from_be_bytes([s[6], s[7]]) as usize;
        s[tbase + pos..].to_vec()
    }

    const FM: &str = "#platform mdsdrv\n@1 fm 4 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n";

    #[test]
    fn notes_and_lengths() {
        // o4 c is note 36 (o1 c is 0); l4 is 24 ticks, written as 23; an end.
        assert_eq!(code("A o4 c d4 e8.\n"), vec![0x82 + 36, 23, 0x82 + 38, 23, 0x82 + 40, 17, 0xff]);
        // Accidentals, octaves and ties; a tie after a note joins it.
        assert_eq!(code("A o4 o3 c+ >c<< c-4^4\n"), vec![0x82 + 25, 23, 0x82 + 36, 23, 0x82 + 11, 47, 0xff]);
        // Rests, and direct ticks.
        assert_eq!(code("A o4 r2 c:5\n"), vec![47, 0x82 + 36, 4, 0xff]);
        // A long note is cut into ties of at most 127.
        assert_eq!(code("A o4 c:200\n"), vec![0x82 + 36, 127, 0x81, 71, 0xff]);
    }

    #[test]
    fn quantize_and_slur() {
        // Q4: half the length, then a rest for the other half.
        assert_eq!(code("A o4 Q4 c\n"), vec![0x82 + 36, 11, 11, 0xff]);
        // q: early release.
        assert_eq!(code("A o4 q6 c\n"), vec![0x82 + 36, 17, 5, 0xff]);
        // A slur: the full length, then the slur command.
        assert_eq!(code("A o4 Q4 c&d\n"), vec![0x82 + 36, 23, 0xe0, 0x82 + 38, 11, 11, 0xff]);
    }

    #[test]
    fn loops() {
        assert_eq!(code("A o4 [c]3\n"), vec![0xfa, 0x82 + 36, 23, 0xfb, 3, 0xff]);
        // A break skips the tail on the last pass: fd with the distance to the end.
        let c = code("A o4 [c/d]2\n");
        assert_eq!(c, vec![0xfa, 0x82 + 36, 23, 0xfd, 0, 4, 0x82 + 38, 23, 0xfb, 2, 0xff]);
        // One pass has no loop at all.
        assert_eq!(code("A o4 [c/d]1\n"), vec![0x82 + 36, 23, 0xff]);
        // L: the end jumps back, relative to the end of the jump command.
        let c = code("A o4 c L d\n");
        assert_eq!(c, vec![0x82 + 36, 23, 0x82 + 38, 23, 0xf5, 0xff, 0xfb]);
    }

    #[test]
    fn commands() {
        let c = code(&format!("{FM}A @1 v10 V20 V+5 p1 t120 K-3 _2 __-1 M0 G4\n"));
        assert_eq!(c, vec![0xe1, 0, 0xe2, 0x8a, 0xe2, 20, 0xe3, 5, 0xe9, 0x40, 0xf9, 101, 0xe6, 0xfd, 0xe4, 2, 0xe5, 0xff, 0xe8, 0, 0xe7, 4, 0xff]);
        // Pan is written on any channel; note numbers are the same on every channel.
        let c = code("G p1 o2 c\n");
        assert_eq!(c, vec![0xe9, 0x40, 0x82 + 12, 23, 0xff]);
    }

    #[test]
    fn conditional_blocks_and_key_signatures() {
        // Three tracks, one command line: each takes its own part.
        let f = compile("ABC o4 {c/d/e}\n").unwrap();
        assert!(Song::parse(&f).is_ok());
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        for (k, note) in [36u8, 38, 40].iter().enumerate() {
            let pos = u16::from_be_bytes([s[6 + 4 * k], s[7 + 4 * k]]) as usize;
            assert_eq!(s[tbase + pos], 0x82 + note);
        }
        // D major: f and c are sharp; an explicit natural overrides.
        assert_eq!(code("A o4 _{D} f c f=\n"), vec![0x82 + 42, 23, 0x82 + 37, 23, 0x82 + 41, 23, 0xff]);
        assert_eq!(code("A o4 _{+b} _{=b} b\n")[0], 0x82 + 47);
    }

    #[test]
    fn subroutines_and_drums() {
        // A subroutine is called by its slot and ends with ff.
        let f = compile("A o4 *40 *40\n*40 o4 cd\n").unwrap();
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        let pos = u16::from_be_bytes([s[6], s[7]]) as usize;
        assert_eq!(&s[tbase + pos..tbase + pos + 5], &[0xfe, 0, 0xfe, 0, 0xff]);
        let sub = u16::from_be_bytes([s[tbase], s[tbase + 1]]) as usize;
        assert_eq!(&s[tbase + sub..], &[0x82 + 36, 23, 0x82 + 38, 23, 0xff]);
        // Drum mode: a, b... take consecutive slots (0 and 1 here); the subroutine ends at its first note with f7.
        let f = compile("A D30 a b\n*30 o1c\n*31 o2c\n").unwrap();
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        let pos = u16::from_be_bytes([s[6], s[7]]) as usize;
        assert_eq!(&s[tbase + pos..tbase + pos + 7], &[0xec, 0x08, 0x82, 23, 0x82 + 1, 23, 0xff]);
        let d0 = u16::from_be_bytes([s[tbase], s[tbase + 1]]) as usize;
        assert_eq!(&s[tbase + d0..tbase + d0 + 2], &[0xf7, 0]);
        // A drum's note is the plain note number, whatever the channel.
        let f = compile("J D40 a\n*40 o9b\n").unwrap();
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        let d = u16::from_be_bytes([s[tbase], s[tbase + 1]]) as usize;
        assert_eq!(&s[tbase + d..tbase + d + 2], &[0xf7, (8 * 12 + 11) as u8]);
    }

    #[test]
    fn instruments_and_envelopes() {
        let f = compile(&format!("{FM}@5 psg 15 10 / 5>0:5\nG @5 c\nA @1 c\n")).unwrap();
        let song = Song::parse(&f).unwrap();
        assert!(song.file_bytes().len() > 40);
        // An FM voice: operators in register order, the transpose at 48 for none.
        let b = fm_bytes(4, 3, &[[31, 1, 2, 3, 4, 5, 1, 6, 2, 0], [0; 10], [0; 10], [0; 10]], 0);
        assert_eq!((b[0], b[4], b[8], b[12], b[16], b[20], b[24], b[28], b[29]), (0x26, 0x5f, 1, 2, 0x43, 0, 5, 0x1c, 48));
        // PSG envelopes: a run of 15 for 1 frame, sustain, a slide, an end.
        assert_eq!(psg_env("15 10 / 5>0:5").unwrap(), vec![0x10, 0x15, 0x01, 0x1a, 0x1b, 0x1c, 0x1e, 0x1f, 0x00]);
        assert_eq!(psg_env("15:20 | 7").unwrap(), vec![0xf0, 0x50, 0x18, 0x02, 0x02]);
        // Pitch envelopes: extended nodes of six bytes; the last node stays.
        let (n, lp) = peg_nodes("0 3 5").unwrap();
        assert_eq!(n.len(), 3);
        assert_eq!(lp, None);
        let b = peg_bytes(&n, None, true);
        assert_eq!(b.len(), 18);
        assert_eq!(&b[6..12], &[0x03, 0x00, 0, 0, 0, 2]);
        assert_eq!(b[17], 2);
        // A slide: the delta spreads the distance over its length.
        let (n, _) = peg_nodes("-2>0:20 0").unwrap();
        assert_eq!(n[0], (-512, 25, 20));
    }

    #[test]
    fn macro_tracks() {
        let f = compile(&format!("{FM}A @1 P100 c\n*100 V0 [V+5 r32]15 L r\n")).unwrap();
        let s = seq_of(&f);
        let tbase = i16::from_be_bytes([s[0], s[1]]) as usize;
        // The macro's nodes: set volume, loop count, add, wait 32, loop, then L's wait and the jump back.
        let m = u16::from_be_bytes([s[tbase + 2], s[tbase + 3]]) as usize;
        assert_eq!(&s[tbase + m..tbase + m + 14], &[0x18, 0, 0x84, 14, 0x58, 5, 0x81, 2, 0x86, 0xfd, 0x81, 23, 0x80, 0xfe]);
    }

    #[test]
    fn errors() {
        assert!(compile("A c\nB E3\n").is_err());
        assert!(compile("A @9 c\n").is_err());
        assert!(compile("A o9 c\n").is_err());
        assert!(compile("#platform nes\nA c\n").is_err());
        assert!(compile("A [c\n").is_err());
        assert!(compile("; nothing\n").is_err());
    }

    /// The pitch of a channel's output, by its zero crossings.
    fn pitch_of(src: &str, channel: usize) -> f64 {
        use crate::mdsdrv::Player;
        use crate::replay::{Bufs, Driver};
        let f = compile(src).unwrap();
        let song = Song::parse(&f).unwrap();
        let mut pl = Player::new(&song);
        let rate = 44_100u32;
        let mut v: Vec<f32> = Vec::new();
        for i in 0..40 {
            pl.tick();
            let frames = (crate::mdsdrv::FRAME * f64::from(rate)).round() as usize;
            let mut b = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
            pl.render(rate, frames, &mut b);
            if i >= 10 {
                v.extend(&b.left[channel]);
            }
        }
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let crossings = v.windows(2).filter(|w| (w[0] - mean) < 0.0 && (w[1] - mean) >= 0.0).count();
        crossings as f64 / (v.len() as f64 / f64::from(rate))
    }

    #[test]
    fn pitches_are_the_notes_written() {
        let fm = "@1 fm 7 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n 31 0 0 0 0 0 0 1 0 0\n";
        for (note, hz) in [("o4 a", 220.0), ("o3 c", 65.4), ("o5 e", 329.6)] {
            let p = pitch_of(&format!("{fm}A @1 v15 {note}:200\n"), 0);
            assert!((p / hz - 1.0).abs() < 0.03, "FM {note}: {p} Hz");
        }
        // The PSG's note numbers start two octaves higher in pitch than the FM's: the same letters sound an octave up from o1... here o3 a.
        let p = pitch_of("@1 psg 15 /\nG @1 v15 o3 a:200\n", 6);
        assert!((p / 440.0 - 1.0).abs() < 0.03, "PSG: {p} Hz");
        // The transpose and detune commands are the driver's: _12 is an octave.
        let p = pitch_of(&format!("{fm}A @1 v15 _12 o4 a:200\n"), 0);
        assert!((p / 440.0 - 1.0).abs() < 0.03, "_12: {p} Hz");
    }

    // Against ctrmml's own compiler: MDS_REF holds its .mds of each .mml (mmlc -f mds), MDS_DIR is data/bgm.
    #[test]
    fn against_ctrmml() {
        let (Ok(dir), Ok(refd)) = (std::env::var("MDS_DIR"), std::env::var("MDS_REF")) else { return };
        let data = std::path::Path::new(&dir).parent().unwrap().to_path_buf();
        let mut bad = 0;
        let extra = std::env::var("MDS_EXTRA").ok().map(std::path::PathBuf::from);
        let mut dirs = vec![data.join("bgm"), data.join("se")];
        dirs.extend(extra);
        for d in dirs {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if !p.extension().is_some_and(|x| x == "mml") {
                    continue;
                }
                let name = p.file_stem().unwrap().to_string_lossy().to_string();
                let base = p.parent().unwrap().to_path_buf();
                let src = std::fs::read_to_string(&p).unwrap();
                let mine = compile_with(&src, move |x| std::fs::read(base.join(x)).ok()).unwrap();
                let theirs = std::fs::read(format!("{refd}/{name}.mds")).unwrap();
                let (a, b) = (Song::parse(&mine).unwrap().timeline(), Song::parse(&theirs).unwrap().timeline());
                let (mut ea, mut eb) = (a.events.clone(), b.events.clone());
                // Rest events differ with how a rest is cut up; the notes and when they start are what is heard.
                ea.retain(|e| e.2.is_some());
                eb.retain(|e| e.2.is_some());
                ea.sort();
                eb.sort();
                // The sound: the same samples from both, channel by channel.
                {
                    use crate::mdsdrv::Player;
                    use crate::replay::{Bufs, Driver};
                    let (sa, sb) = (Song::parse(&mine).unwrap(), Song::parse(&theirs).unwrap());
                    let (mut pa, mut pb) = (Player::new(&sa), Player::new(&sb));
                    let mut worst = [0f32; 10];
                    for _ in 0..60 * 10 {
                        pa.tick();
                        pb.tick();
                        let frames = (crate::mdsdrv::FRAME * 44_100.0).round() as usize;
                        let mut x = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
                        let mut y = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
                        pa.render(44_100, frames, &mut x);
                        pb.render(44_100, frames, &mut y);
                        for c in 0..10 {
                            for i in 0..frames {
                                worst[c] = worst[c].max((x.left[c][i] - y.left[c][i]).abs()).max((x.right[c][i] - y.right[c][i]).abs());
                            }
                        }
                    }
                    if worst.iter().any(|&w| w > 1e-6) {
                        bad += 1;
                        eprintln!("{name}: SOUND differs, largest sample difference per channel {worst:?}");
                    }
                }
                let same = ea == eb && a.steps == b.steps;
                if !same {
                    bad += 1;
                    let first = ea.iter().zip(&eb).position(|(x, y)| x != y);
                    for c in 0..10 {
                        let x: Vec<_> = ea.iter().filter(|e| e.1 == c).collect();
                        let y: Vec<_> = eb.iter().filter(|e| e.1 == c).collect();
                        if x != y {
                            let i = x.iter().zip(&y).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
                            eprintln!("  channel {c}: {} vs {} events; first difference at index {i}: {:?} vs {:?}", x.len(), y.len(), x.get(i), y.get(i));
                        }
                    }
                    eprintln!("{name}: MISMATCH steps {} vs {}, events {} vs {}, first difference at {first:?}: {:?} vs {:?}", a.steps, b.steps, ea.len(), eb.len(), first.map(|i| ea[i]), first.map(|i| eb[i]));
                } else {
                    eprintln!("{name}: same ({} events, {} steps)", ea.len(), a.steps);
                }
            }
        }
        assert_eq!(bad, 0);
    }

    #[test]
    fn garbage_never_panics() {
        let alphabet: Vec<char> = "ABGJ*@#'{}[]/|<>()^&~_\\\n \tabcdefghrlovVqQtTkKGMPDRCsLp+-=.:0123456789;\"fm2op psg pcm write tl1 lfo mode carry".chars().collect();
        let mut x = 0x1234_5678_9abc_def0u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let seeds = ["A c d e\nB [c/d]3 L e\n", "#platform mdsdrv\n@1 psg 15 10 / 5>0:5\n@M1 | 0 3 5 V0:1:5\nG @1 M1 c\n", "ABC {c/d/e} *40\n*40 [V+1 r8]4 'carry'\nA P40 D30 a\n*30 o1c\n"];
        for n in 0..6000 {
            let mut text: Vec<char> = if n % 2 == 0 { seeds[n % 3].chars().collect() } else { Vec::new() };
            for _ in 0..1 + next() % 40 {
                let c = alphabet[(next() % alphabet.len() as u64) as usize];
                let at = if text.is_empty() { 0 } else { (next() % (text.len() as u64 + 1)) as usize };
                text.insert(at, c);
            }
            let s: String = text.into_iter().collect();
            if let Ok(f) = compile(&s) {
                let _ = Song::parse(&f);
            }
        }
    }

    #[test]
    fn detection() {
        assert!(looks_like("#platform mdsdrv\nA c"));
        assert!(looks_like("@1 psg 15\nA c"));
        assert!(!looks_like("A cdef"));
        assert!(!looks_like("#platform nsd\nA c"));
    }

    // The repository's own .mml files (the song and the sound effects), when MDS_DIR is set (data/bgm).
    #[test]
    fn real_mml_files_sound() {
        use crate::mdsdrv::Player;
        use crate::replay::{Bufs, Driver};
        let Ok(dir) = std::env::var("MDS_DIR") else { return };
        let data = std::path::Path::new(&dir).parent().unwrap().to_path_buf();
        let mut n = 0;
        for sub in ["bgm", "se"] {
            for e in std::fs::read_dir(data.join(sub)).unwrap().flatten() {
                let p = e.path();
                if !p.extension().is_some_and(|x| x == "mml") {
                    continue;
                }
                let base = p.parent().unwrap().to_path_buf();
                let src = std::fs::read_to_string(&p).unwrap();
                let f = compile_with(&src, move |path| std::fs::read(base.join(path)).ok()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                let song = Song::parse(&f).unwrap();
                let mut pl = Player::new(&song);
                let rate = 44_100u32;
                let mut energy = [0f64; 10];
                for _ in 0..60 * 12 {
                    pl.tick();
                    let frames = (crate::mdsdrv::FRAME * f64::from(rate)).round() as usize;
                    let mut b = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
                    pl.render(rate, frames, &mut b);
                    for c in 0..10 {
                        for v in &b.left[c] {
                            assert!(v.is_finite() && v.abs() < 4.0);
                            energy[c] += f64::from(v * v);
                        }
                    }
                }
                eprintln!("{}: {:?}", p.file_name().unwrap().to_string_lossy(), energy.iter().map(|e| (e.sqrt() * 10.0).round() / 10.0).collect::<Vec<_>>());
                assert!(energy.iter().sum::<f64>() > 0.0, "{} is silent", p.display());
                n += 1;
            }
        }
        assert_eq!(n, 15);
    }

}
