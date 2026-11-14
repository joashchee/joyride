//! A 68000 assembler for the dialect of `asm68k` that MDSDRV's source uses: written from the instruction set's encodings and from the source it has to
//! read (labels, `@` local labels scoped to the last global one, `equ`, `equr` register aliases, `rs`, `dc`, `dcb`, `macro` with `\1`, `\name` and `\@`,
//! `rept`, `if`/`else`/`endif`, `include`, `incbin`). Its aim is to run the original sound driver in `cpu.rs`, not to be asm68k: branches without a size are
//! always word branches and absolute addresses always long, so the code is a little longer than asm68k's and does the same.
//!
//! Two or more passes: the symbols of the pass before give the forward references, and the passes stop when the symbols stop changing.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Sz {
    B,
    W,
    L,
}

impl Sz {
    fn bits(self) -> u16 {
        match self {
            Sz::B => 0,
            Sz::W => 1,
            Sz::L => 2,
        }
    }
}

#[derive(Debug, Clone)]
enum Ea {
    Dn(u8),
    An(u8),
    Ind(u8),
    Post(u8),
    Pre(u8),
    Disp(i64, u8),
    /// (disp, base An, index register number, index is An, index is long)
    Idx(i64, u8, u8, bool, bool),
    AbsW(i64),
    AbsL(i64),
    PcD(i64),
    PcIdx(i64, u8, bool, bool),
    Imm(i64),
    Sr,
    Ccr,
}

struct Line {
    text: String,
    file: String,
    no: usize,
}

struct Macro {
    params: Vec<String>,
    body: Vec<String>,
}

/// The finished program.
pub struct Output {
    pub bytes: Vec<u8>,
    /// Every label and `equ` that isn't local.
    pub symbols: HashMap<String, i64>,
}

pub struct Assembler<'a> {
    files: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    syms: HashMap<String, i64>,
    defined: HashSet<String>,
    aliases: HashMap<String, String>,
    macros: HashMap<String, Macro>,
    scope: String,
    pc: i64,
    out: Vec<u8>,
    rs: i64,
    ifs: Vec<(bool, bool)>,
    uniq: u32,
    pass: u32,
    unresolved: bool,
}

type R<T> = Result<T, String>;

/// Assemble `main` (read through `files`, which also serves `include` and `incbin`), with `defs` predefined.
pub fn assemble(main: &str, defs: &[(&str, i64)], files: &dyn Fn(&str) -> Option<Vec<u8>>) -> R<Output> {
    let mut a = Assembler {
        files,
        syms: HashMap::new(),
        defined: HashSet::new(),
        aliases: HashMap::new(),
        macros: HashMap::new(),
        scope: String::new(),
        pc: 0,
        out: vec![],
        rs: 0,
        ifs: vec![],
        uniq: 0,
        pass: 0,
        unresolved: false,
    };
    let lines = a.load(main, 0)?;
    let mut last = None;
    for pass in 0..6 {
        a.pass = pass;
        a.defined.clear();
        a.aliases.clear();
        a.macros.clear();
        a.scope.clear();
        a.pc = 0;
        a.out.clear();
        a.rs = 0;
        a.ifs.clear();
        a.uniq = 0;
        for (n, v) in defs {
            a.syms.insert((*n).to_string(), *v);
            a.defined.insert((*n).to_string());
        }
        a.run(&lines)?;
        if !a.ifs.is_empty() {
            return Err("an if without its endif".into());
        }
        let now: Vec<(String, i64)> = {
            let mut v: Vec<_> = a.syms.iter().map(|(k, v)| (k.clone(), *v)).collect();
            v.sort();
            v
        };
        if pass >= 1 && last.as_ref() == Some(&now) {
            break;
        }
        last = Some(now);
    }
    let symbols = a.syms.iter().filter(|(k, _)| !k.contains('@')).map(|(k, v)| (k.clone(), *v)).collect();
    Ok(Output { bytes: a.out, symbols })
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '@' || c == '.'
}

fn strip_comment(s: &str) -> &str {
    let mut q = None;
    for (i, c) in s.char_indices() {
        match q {
            Some(qc) if c == qc => q = None,
            Some(_) => {}
            None => {
                if c == '"' || c == '\'' {
                    q = Some(c);
                } else if c == ';' {
                    return &s[..i];
                }
            }
        }
    }
    s
}

fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0;
    let mut q = None;
    let mut cur = String::new();
    for c in s.chars() {
        match q {
            Some(qc) => {
                if c == qc {
                    q = None;
                }
                cur.push(c);
            }
            None => match c {
                '"' | '\'' => {
                    q = Some(c);
                    cur.push(c);
                }
                '(' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' => {
                    depth -= 1;
                    cur.push(c);
                }
                c if c == sep && depth == 0 => out.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            },
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|x| x.trim().to_string()).collect()
}

impl<'a> Assembler<'a> {
    fn load(&self, name: &str, depth: u32) -> R<Vec<Line>> {
        if depth > 8 {
            return Err(format!("{name}: includes nest too deep"));
        }
        let raw = (self.files)(name).ok_or_else(|| format!("{name}: no such file"))?;
        let text = String::from_utf8_lossy(&raw).into_owned();
        let mut out = vec![];
        for (i, l) in text.lines().enumerate() {
            let t = l.trim_start();
            let low = t.to_ascii_lowercase();
            if low.starts_with("include") && t[7..].starts_with(|c: char| c.is_whitespace()) {
                let f = t[7..].trim().trim_matches('"').trim_matches('\'');
                out.extend(self.load(f, depth + 1)?);
                continue;
            }
            out.push(Line { text: l.to_string(), file: name.to_string(), no: i + 1 });
        }
        Ok(out)
    }

    fn key(&self, n: &str) -> String {
        if n.starts_with('@') {
            format!("{}{}", self.scope, n)
        } else {
            n.to_string()
        }
    }

    fn define(&mut self, name: &str, v: i64) -> R<()> {
        let k = self.key(name);
        if self.defined.contains(&k) && !self.macros.is_empty() && false {
            return Err(format!("{name} defined twice"));
        }
        self.defined.insert(k.clone());
        self.syms.insert(k, v);
        Ok(())
    }

    fn active(&self) -> bool {
        self.ifs.last().map_or(true, |x| x.0)
    }

    fn run(&mut self, lines: &[Line]) -> R<()> {
        let mut i = 0;
        while i < lines.len() {
            let l = &lines[i];
            i += 1;
            let text = strip_comment(&l.text).trim_end().to_string();
            if text.trim().is_empty() || text.trim_start().starts_with('*') && l.text.starts_with('*') {
                continue;
            }
            let at = |e: String| format!("{}:{}: {}", l.file, l.no, e);
            let col0 = !text.starts_with(|c: char| c.is_whitespace());
            let mut rest = text.trim_start();
            let mut label = None;
            if col0 {
                let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
                label = Some(rest[..end].trim_end_matches(':').to_string());
                rest = rest[end..].trim_start();
            }
            let (mn_full, args) = match rest.find(|c: char| c.is_whitespace()) {
                Some(p) => (&rest[..p], rest[p..].trim()),
                None => (rest, ""),
            };
            let mn_low = mn_full.to_ascii_lowercase();
            // Conditional assembly first: it works while inactive too.
            match mn_low.as_str() {
                "if" => {
                    let parent = self.active();
                    let v = if parent { self.eval(args).map_err(&at)? != 0 } else { false };
                    self.ifs.push((parent && v, parent));
                    continue;
                }
                "else" => {
                    let (a, p) = self.ifs.pop().ok_or_else(|| at("else without if".into()))?;
                    self.ifs.push((p && !a, p));
                    continue;
                }
                "endif" | "endc" => {
                    self.ifs.pop().ok_or_else(|| at("endif without if".into()))?;
                    continue;
                }
                _ => {}
            }
            if !self.active() {
                continue;
            }
            if mn_low == "macro" {
                let name = label.clone().ok_or_else(|| at("a macro with no name".into()))?;
                let params: Vec<String> = if args.is_empty() { vec![] } else { split_top(args, ',') };
                let mut body = vec![];
                while i < lines.len() {
                    let t = strip_comment(&lines[i].text).trim().to_ascii_lowercase();
                    i += 1;
                    if t == "endm" {
                        break;
                    }
                    body.push(lines[i - 1].text.clone());
                }
                self.macros.insert(name.to_ascii_lowercase(), Macro { params, body });
                continue;
            }
            if mn_low == "rept" {
                let n = self.eval(args).map_err(&at)?;
                let mut depth = 1;
                let start = i;
                while i < lines.len() {
                    let t = strip_comment(&lines[i].text).trim().to_ascii_lowercase();
                    i += 1;
                    if t.starts_with("rept ") {
                        depth += 1;
                    } else if t == "endr" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                let body: Vec<Line> = lines[start..i - 1].iter().map(|x| Line { text: x.text.clone(), file: x.file.clone(), no: x.no }).collect();
                for _ in 0..n {
                    self.run(&body)?;
                }
                continue;
            }
            // A label (not before equ and friends, which name themselves).
            let defines = matches!(mn_low.as_str(), "equ" | "equr" | "rs.b" | "rs.w" | "rs.l" | "set" | "=");
            if let Some(lb) = &label {
                if !defines {
                    if !lb.starts_with('@') {
                        self.scope = lb.clone();
                    }
                    let pc = self.pc;
                    self.define(lb, pc).map_err(&at)?;
                }
            }
            if mn_low.is_empty() {
                continue;
            }
            self.directive_or_instr(label.as_deref(), &mn_low, args).map_err(&at)?;
        }
        Ok(())
    }

    fn directive_or_instr(&mut self, label: Option<&str>, mn: &str, args: &str) -> R<()> {
        match mn {
            "equ" | "set" | "=" => {
                let v = self.eval(args)?;
                let n = label.ok_or("equ with no name")?;
                self.define(n, v)
            }
            "equr" => {
                let n = label.ok_or("equr with no name")?;
                let k = self.key(n);
                self.aliases.insert(k, args.trim().to_string());
                Ok(())
            }
            "rsreset" => {
                self.rs = 0;
                Ok(())
            }
            "rsset" => {
                self.rs = self.eval(args)?;
                Ok(())
            }
            "rs.b" | "rs.w" | "rs.l" => {
                let n = self.eval(args)?;
                if let Some(l) = label {
                    let v = self.rs;
                    self.define(l, v)?;
                }
                self.rs += n * match mn {
                    "rs.b" => 1,
                    "rs.w" => 2,
                    _ => 4,
                };
                Ok(())
            }
            "list" | "opt" | "inform" | "page" | "title" | "xdef" | "xref" | "section" | "nolist" | "end" => {
                if mn == "inform" {
                    let a = split_top(args, ',');
                    if a.first().map(|x| self.eval(x)) == Some(Ok(3)) {
                        return Err(format!("inform: {}", a.get(1).cloned().unwrap_or_default()));
                    }
                }
                Ok(())
            }
            "even" => {
                if self.pc & 1 != 0 {
                    self.emit(&[0]);
                }
                Ok(())
            }
            "dc.b" | "dc.w" | "dc.l" => {
                let sz = match mn {
                    "dc.b" => 1,
                    "dc.w" => 2,
                    _ => 4,
                };
                for item in split_top(args, ',') {
                    if (item.starts_with('"') && item.ends_with('"') || item.starts_with('\'') && item.ends_with('\'')) && item.len() >= 2 && sz == 1 {
                        let s = &item[1..item.len() - 1];
                        self.emit(s.as_bytes());
                    } else {
                        let v = self.eval(&item)?;
                        self.emit_n(v, sz);
                    }
                }
                Ok(())
            }
            "dcb.b" | "dcb.w" | "dcb.l" => {
                let a = split_top(args, ',');
                let n = self.eval(a.first().ok_or("dcb needs a count")?)?;
                let v = if a.len() > 1 { self.eval(&a[1])? } else { 0 };
                let sz = match mn {
                    "dcb.b" => 1,
                    "dcb.w" => 2,
                    _ => 4,
                };
                for _ in 0..n {
                    self.emit_n(v, sz);
                }
                Ok(())
            }
            "ds.b" | "ds.w" | "ds.l" => {
                let n = self.eval(args)?;
                let sz = match mn {
                    "ds.b" => 1,
                    "ds.w" => 2,
                    _ => 4,
                };
                for _ in 0..n * sz {
                    self.emit(&[0]);
                }
                Ok(())
            }
            "incbin" => {
                let f = args.trim().trim_matches('"').trim_matches('\'');
                let b = (self.files)(f).ok_or_else(|| format!("incbin {f}: no such file"))?;
                self.emit(&b);
                Ok(())
            }
            "org" => {
                let v = self.eval(args)?;
                while self.pc < v {
                    self.emit(&[0]);
                }
                Ok(())
            }
            _ => {
                if let Some(m) = self.macros.get(mn).map(|m| Macro { params: m.params.clone(), body: m.body.clone() }) {
                    return self.expand(&m, args);
                }
                let (name, sz) = match mn.rfind('.') {
                    Some(p) if matches!(&mn[p + 1..], "b" | "w" | "l" | "s") => (&mn[..p], Some(match &mn[p + 1..] {
                        "b" | "s" => (Sz::B, true),
                        "w" => (Sz::W, false),
                        _ => (Sz::L, false),
                    })),
                    _ => (mn, None),
                };
                let ops = if args.is_empty() { vec![] } else { split_top(args, ',') };
                self.instr(name, sz, &ops)
            }
        }
    }

    fn expand(&mut self, m: &Macro, args: &str) -> R<()> {
        let actual = if args.is_empty() { vec![] } else { split_top(args, ',') };
        self.uniq += 1;
        let id = self.uniq;
        let mut lines = vec![];
        for b in &m.body {
            let mut out = String::new();
            let cs: Vec<char> = b.chars().collect();
            let mut k = 0;
            while k < cs.len() {
                if cs[k] == '\\' && k + 1 < cs.len() {
                    if cs[k + 1] == '@' {
                        out.push_str(&format!("_{id}"));
                        k += 2;
                        continue;
                    }
                    let mut e = k + 1;
                    while e < cs.len() && (cs[e].is_ascii_alphanumeric() || cs[e] == '_') {
                        e += 1;
                    }
                    let name: String = cs[k + 1..e].iter().collect();
                    let by_name = m.params.iter().position(|p| *p == name);
                    let by_num = name.parse::<usize>().ok().filter(|n| *n >= 1).map(|n| n - 1);
                    if let Some(ix) = by_name.or(by_num) {
                        out.push_str(actual.get(ix).map_or("", |s| s.as_str()));
                        k = e;
                        continue;
                    }
                }
                out.push(cs[k]);
                k += 1;
            }
            lines.push(Line { text: out, file: "macro".into(), no: 0 });
        }
        self.run(&lines)
    }

    fn emit(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
        self.pc += b.len() as i64;
    }

    fn emit_n(&mut self, v: i64, size: usize) {
        let b = v.to_be_bytes();
        self.emit(&b[8 - size..]);
    }

    // ---- expressions ----

    fn eval(&mut self, s: &str) -> R<i64> {
        let toks = tokenize(s)?;
        let mut p = 0;
        let v = self.e_cmp(&toks, &mut p)?;
        if p != toks.len() {
            return Err(format!("can't read '{s}'"));
        }
        Ok(v)
    }

    fn e_cmp(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_or(t, p)?;
        loop {
            let op = match t.get(*p) {
                Some(Tok::Op(o)) if matches!(o.as_str(), "=" | "==" | "<>" | "!=" | "<" | ">" | "<=" | ">=") => o.clone(),
                _ => return Ok(v),
            };
            *p += 1;
            let r = self.e_or(t, p)?;
            v = i64::from(match op.as_str() {
                "=" | "==" => v == r,
                "<>" | "!=" => v != r,
                "<" => v < r,
                ">" => v > r,
                "<=" => v <= r,
                _ => v >= r,
            });
        }
    }

    fn e_or(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_xor(t, p)?;
        while matches!(t.get(*p), Some(Tok::Op(o)) if o == "|") {
            *p += 1;
            v |= self.e_xor(t, p)?;
        }
        Ok(v)
    }

    fn e_xor(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_and(t, p)?;
        while matches!(t.get(*p), Some(Tok::Op(o)) if o == "^") {
            *p += 1;
            v ^= self.e_and(t, p)?;
        }
        Ok(v)
    }

    fn e_and(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_shift(t, p)?;
        while matches!(t.get(*p), Some(Tok::Op(o)) if o == "&") {
            *p += 1;
            v &= self.e_shift(t, p)?;
        }
        Ok(v)
    }

    fn e_shift(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_add(t, p)?;
        loop {
            match t.get(*p) {
                Some(Tok::Op(o)) if o == "<<" => {
                    *p += 1;
                    let r = self.e_add(t, p)?;
                    v = v.wrapping_shl(r as u32);
                }
                Some(Tok::Op(o)) if o == ">>" => {
                    *p += 1;
                    let r = self.e_add(t, p)?;
                    v = v.wrapping_shr(r as u32);
                }
                _ => return Ok(v),
            }
        }
    }

    fn e_add(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_mul(t, p)?;
        loop {
            match t.get(*p) {
                Some(Tok::Op(o)) if o == "+" => {
                    *p += 1;
                    v = v.wrapping_add(self.e_mul(t, p)?);
                }
                Some(Tok::Op(o)) if o == "-" => {
                    *p += 1;
                    v = v.wrapping_sub(self.e_mul(t, p)?);
                }
                _ => return Ok(v),
            }
        }
    }

    fn e_mul(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        let mut v = self.e_un(t, p)?;
        loop {
            match t.get(*p) {
                Some(Tok::Op(o)) if o == "*" => {
                    *p += 1;
                    v = v.wrapping_mul(self.e_un(t, p)?);
                }
                Some(Tok::Op(o)) if o == "/" || o == "%" => {
                    let m = o == "%";
                    *p += 1;
                    let r = self.e_un(t, p)?;
                    if r == 0 {
                        if self.pass == 0 {
                            v = 0;
                            continue;
                        }
                        return Err("division by zero".into());
                    }
                    v = if m { v % r } else { v / r };
                }
                _ => return Ok(v),
            }
        }
    }

    fn e_un(&mut self, t: &[Tok], p: &mut usize) -> R<i64> {
        match t.get(*p) {
            Some(Tok::Op(o)) if o == "-" => {
                *p += 1;
                Ok(self.e_un(t, p)?.wrapping_neg())
            }
            Some(Tok::Op(o)) if o == "+" => {
                *p += 1;
                self.e_un(t, p)
            }
            Some(Tok::Op(o)) if o == "~" => {
                *p += 1;
                Ok(!self.e_un(t, p)?)
            }
            Some(Tok::Op(o)) if o == "(" => {
                *p += 1;
                let v = self.e_cmp(t, p)?;
                match t.get(*p) {
                    Some(Tok::Op(c)) if c == ")" => {
                        *p += 1;
                        Ok(v)
                    }
                    _ => Err("a ')' is missing".into()),
                }
            }
            Some(Tok::Num(n)) => {
                *p += 1;
                Ok(*n)
            }
            Some(Tok::Op(o)) if o == "*" => {
                *p += 1;
                Ok(self.pc)
            }
            Some(Tok::Id(name)) => {
                *p += 1;
                let name = name.clone();
                if name.eq_ignore_ascii_case("def") {
                    if let (Some(Tok::Op(a)), Some(Tok::Id(s)), Some(Tok::Op(b))) = (t.get(*p), t.get(*p + 1), t.get(*p + 2)) {
                        if a == "(" && b == ")" {
                            *p += 3;
                            let k = self.key(s);
                            return Ok(i64::from(self.defined.contains(&k)));
                        }
                    }
                    return Err("def(name)".into());
                }
                let k = self.key(&name);
                match self.syms.get(&k) {
                    Some(v) => Ok(*v),
                    None => {
                        self.unresolved = true;
                        if self.pass >= 1 {
                            Err(format!("'{name}' isn't defined"))
                        } else {
                            Ok(0)
                        }
                    }
                }
            }
            _ => Err("an expression is incomplete".into()),
        }
    }

    // ---- operands ----

    fn reg(&self, s: &str) -> Option<(bool, u8)> {
        let mut real = s.trim();
        for _ in 0..6 {
            match self.aliases.get(&self.key(real)) {
                Some(x) => real = x.trim(),
                None => break,
            }
        }
        let l = real.to_ascii_lowercase();
        if l == "sp" {
            return Some((true, 7));
        }
        let b = l.as_bytes();
        if b.len() == 2 && (b[0] == b'd' || b[0] == b'a') && (b'0'..=b'7').contains(&b[1]) {
            return Some((b[0] == b'a', b[1] - b'0'));
        }
        None
    }

    fn operand(&mut self, s: &str) -> R<Ea> {
        let s = s.trim();
        let l = s.to_ascii_lowercase();
        if l == "sr" {
            return Ok(Ea::Sr);
        }
        if l == "ccr" {
            return Ok(Ea::Ccr);
        }
        if let Some(r) = self.reg(s) {
            return Ok(if r.0 { Ea::An(r.1) } else { Ea::Dn(r.1) });
        }
        if let Some(e) = s.strip_prefix('#') {
            return Ok(Ea::Imm(self.eval(e)?));
        }
        if s.ends_with(')') {
            // find the matching '(' of the last ')'
            let cs: Vec<char> = s.chars().collect();
            let mut depth = 0;
            let mut open = None;
            for k in (0..cs.len()).rev() {
                match cs[k] {
                    ')' => depth += 1,
                    '(' => {
                        depth -= 1;
                        if depth == 0 {
                            open = Some(k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(o) = open {
                let prefix: String = cs[..o].iter().collect();
                let inner: String = cs[o + 1..cs.len() - 1].iter().collect();
                let parts = split_top(&inner, ',');
                let first = parts.first().map(|x| x.trim().to_ascii_lowercase()).unwrap_or_default();
                let base = self.reg(&first).filter(|r| r.0);
                let is_pc = first == "pc";
                if base.is_some() || is_pc {
                    let pre = prefix.trim();
                    if pre == "-" && parts.len() == 1 && base.is_some() {
                        return Ok(Ea::Pre(base.unwrap().1));
                    }
                    let disp = if pre.is_empty() { 0 } else { self.eval(pre)? };
                    if parts.len() == 1 {
                        return Ok(match base {
                            Some(b) => {
                                if pre.is_empty() {
                                    Ea::Ind(b.1)
                                } else {
                                    Ea::Disp(disp, b.1)
                                }
                            }
                            None => Ea::PcD(disp),
                        });
                    }
                    if parts.len() == 2 {
                        let (xr, long) = parse_index(&parts[1]);
                        let x = self.reg(&xr).ok_or_else(|| format!("'{}' isn't an index register", parts[1]))?;
                        return Ok(match base {
                            Some(b) => Ea::Idx(disp, b.1, x.1, x.0, long),
                            None => Ea::PcIdx(disp, x.1, x.0, long),
                        });
                    }
                    return Err(format!("can't read the operand '{s}'"));
                }
            }
        }
        if let Some(inner) = s.strip_suffix('+') {
            if let Some(i) = inner.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
                if let Some(r) = self.reg(i) {
                    if r.0 {
                        return Ok(Ea::Post(r.1));
                    }
                }
            }
        }
        // an absolute address, with an optional .w or .l
        let (e, wide) = if let Some(x) = s.strip_suffix(".w").or_else(|| s.strip_suffix(".W")) {
            (x, Some(false))
        } else if let Some(x) = s.strip_suffix(".l").or_else(|| s.strip_suffix(".L")) {
            (x, Some(true))
        } else {
            (s, None)
        };
        let v = self.eval(e)?;
        Ok(if wide == Some(false) { Ea::AbsW(v) } else { Ea::AbsL(v) })
    }

    /// The addressing mode's bits (mode << 3 | reg), with its extension words pushed to `w`.
    fn ea(&self, ea: &Ea, sz: Sz, w: &mut Vec<u16>, base: i64) -> R<u16> {
        Ok(match ea {
            Ea::Dn(n) => u16::from(*n),
            Ea::An(n) => 8 | u16::from(*n),
            Ea::Ind(n) => 0x10 | u16::from(*n),
            Ea::Post(n) => 0x18 | u16::from(*n),
            Ea::Pre(n) => 0x20 | u16::from(*n),
            Ea::Disp(d, n) => {
                w.push(fit16(*d)?);
                0x28 | u16::from(*n)
            }
            Ea::Idx(d, n, x, xa, xl) => {
                if !(-128..=127).contains(d) && self.pass >= 1 {
                    return Err(format!("an index displacement of {d} doesn't fit a byte"));
                }
                w.push((*d as u16 & 0xff) | u16::from(*x) << 12 | if *xa { 0x8000 } else { 0 } | if *xl { 0x800 } else { 0 });
                0x30 | u16::from(*n)
            }
            Ea::AbsW(a) => {
                w.push(*a as u16);
                0x38
            }
            Ea::AbsL(a) => {
                w.push((*a >> 16) as u16);
                w.push(*a as u16);
                0x39
            }
            Ea::PcD(t) => {
                let at = base + 2 * w.len() as i64;
                w.push(fit16(t - at)?);
                0x3a
            }
            Ea::PcIdx(t, x, xa, xl) => {
                let at = base + 2 * w.len() as i64;
                let d = t - at;
                if !(-128..=127).contains(&d) && self.pass >= 1 {
                    return Err(format!("a pc-relative index displacement of {d} doesn't fit a byte"));
                }
                w.push((d as u16 & 0xff) | u16::from(*x) << 12 | if *xa { 0x8000 } else { 0 } | if *xl { 0x800 } else { 0 });
                0x3b
            }
            Ea::Imm(v) => {
                match sz {
                    Sz::B => w.push(*v as u16 & 0xff),
                    Sz::W => w.push(*v as u16),
                    Sz::L => {
                        w.push((*v >> 16) as u16);
                        w.push(*v as u16);
                    }
                }
                0x3c
            }
            Ea::Sr | Ea::Ccr => return Err("sr or ccr can't be an address here".into()),
        })
    }

    fn put(&mut self, w: &[u16]) {
        for x in w {
            self.emit(&x.to_be_bytes());
        }
    }

    fn instr(&mut self, name: &str, sz: Option<(Sz, bool)>, ops: &[String]) -> R<()> {
        let pc = self.pc;
        let mut o: Vec<Ea> = vec![];
        let is_movem = name == "movem";
        for (i, s) in ops.iter().enumerate() {
            if is_movem {
                if let Some(m) = self.reglist(s) {
                    // a register list: the other operand decides the direction
                    o.push(Ea::Imm(i64::from(m)));
                    let _ = i;
                    continue;
                }
            }
            o.push(self.operand(s)?);
        }
        let size = sz.map(|x| x.0);
        let sw = |d: Sz| size.unwrap_or(d);
        let mut w: Vec<u16> = vec![0];
        let bad = || format!("can't assemble {name} with these operands");
        match name {
            "nop" => w[0] = 0x4e71,
            "rts" => w[0] = 0x4e75,
            "rte" => w[0] = 0x4e73,
            "rtr" => w[0] = 0x4e77,
            "reset" => w[0] = 0x4e70,
            "moveq" => match (&o[..], ) {
                ([Ea::Imm(v), Ea::Dn(d)],) => {
                    if !(-128..=255).contains(v) && self.pass >= 1 {
                        return Err(format!("moveq #{v} doesn't fit a byte"));
                    }
                    w[0] = 0x7000 | u16::from(*d) << 9 | (*v as u16 & 0xff);
                }
                _ => return Err(bad()),
            },
            "move" | "movea" => {
                let s = sw(Sz::W);
                if o.len() != 2 {
                    return Err(bad());
                }
                match (&o[0], &o[1]) {
                    (Ea::Sr, d) => {
                        let m = self.ea(d, Sz::W, &mut w, pc)?;
                        w[0] = 0x40c0 | m;
                    }
                    (src, Ea::Sr) => {
                        let m = self.ea(src, Sz::W, &mut w, pc)?;
                        w[0] = 0x46c0 | m;
                    }
                    (src, Ea::Ccr) => {
                        let m = self.ea(src, Sz::W, &mut w, pc)?;
                        w[0] = 0x44c0 | m;
                    }
                    (src, dst) => {
                        let sm = self.ea(src, s, &mut w, pc)?;
                        let dm = self.ea(dst, s, &mut w, pc)?;
                        let sb = match s {
                            Sz::B => 1,
                            Sz::W => 3,
                            Sz::L => 2,
                        };
                        if matches!(dst, Ea::An(_)) && s == Sz::B {
                            return Err("move.b to an address register".into());
                        }
                        w[0] = sb << 12 | (dm & 7) << 9 | (dm >> 3) << 6 | sm;
                    }
                }
            }
            "add" | "sub" | "and" | "or" | "eor" | "cmp" | "adda" | "suba" | "cmpa" | "addi" | "subi" | "andi" | "ori" | "eori" | "cmpi" => {
                if o.len() != 2 {
                    return Err(bad());
                }
                let s = sw(Sz::W);
                let imm_form = matches!(name, "addi" | "subi" | "andi" | "ori" | "eori" | "cmpi");
                let a_form = matches!(name, "adda" | "suba" | "cmpa") || (matches!(name, "add" | "sub" | "cmp") && matches!(o[1], Ea::An(_)));
                let base = match name {
                    "add" | "adda" | "addi" => 0xd000u16,
                    "sub" | "suba" | "subi" => 0x9000,
                    "and" | "andi" => 0xc000,
                    "or" | "ori" => 0x8000,
                    "eor" | "eori" => 0xb000,
                    _ => 0xb000,
                };
                let imm_op = match name {
                    "addi" | "add" | "adda" => 0x0600u16,
                    "subi" | "sub" | "suba" => 0x0400,
                    "andi" | "and" => 0x0200,
                    "ori" | "or" => 0x0000,
                    "eori" | "eor" => 0x0a00,
                    _ => 0x0c00,
                };
                let to_sr = match (&o[1], name) {
                    (Ea::Sr, "andi" | "ori" | "eori") => Some(0x7cu16),
                    (Ea::Ccr, "andi" | "ori" | "eori") => Some(0x3c),
                    _ => None,
                };
                if let Some(low) = to_sr {
                    let v = match o[0] {
                        Ea::Imm(v) => v,
                        _ => return Err(bad()),
                    };
                    w[0] = imm_op | low;
                    w.push(v as u16 & if low == 0x3c { 0xff } else { 0xffff });
                } else if a_form {
                    let an = match o[1] {
                        Ea::An(n) => n,
                        _ => return Err(bad()),
                    };
                    let m = self.ea(&o[0], if s == Sz::B { Sz::W } else { s }, &mut w, pc)?;
                    let opm = if s == Sz::L { 7 } else { 3 };
                    let b = match name {
                        "adda" | "add" => 0xd000,
                        "suba" | "sub" => 0x9000,
                        _ => 0xb000,
                    };
                    w[0] = b | u16::from(an) << 9 | opm << 6 | m;
                } else if imm_form || (matches!(o[0], Ea::Imm(_)) && !matches!(o[1], Ea::An(_))) {
                    let v = match o[0] {
                        Ea::Imm(v) => v,
                        _ => return Err(bad()),
                    };
                    w[0] = 0;
                    let mut ext = vec![];
                    match s {
                        Sz::B => ext.push(v as u16 & 0xff),
                        Sz::W => ext.push(v as u16),
                        Sz::L => {
                            ext.push((v >> 16) as u16);
                            ext.push(v as u16);
                        }
                    }
                    w.extend(ext);
                    let m = self.ea(&o[1], s, &mut w, pc)?;
                    w[0] = imm_op | s.bits() << 6 | m;
                } else {
                    // Dn <-> ea
                    let (dn, other, to_ea) = match (&o[0], &o[1]) {
                        (e, Ea::Dn(d)) if name != "eor" => (*d, e.clone(), false),
                        (Ea::Dn(d), e) => (*d, e.clone(), true),
                        _ => return Err(bad()),
                    };
                    let m = self.ea(&other, s, &mut w, pc)?;
                    let opm = if name == "cmp" {
                        s.bits()
                    } else if name == "eor" {
                        4 | s.bits()
                    } else if to_ea {
                        4 | s.bits()
                    } else {
                        s.bits()
                    };
                    if name == "cmp" && to_ea {
                        return Err(bad());
                    }
                    w[0] = base | u16::from(dn) << 9 | opm << 6 | m;
                }
            }
            "addq" | "subq" => {
                let s = sw(Sz::W);
                match (&o[0], &o[1]) {
                    (Ea::Imm(v), d) => {
                        if !(1..=8).contains(v) {
                            return Err(format!("{name} #{v}: 1 to 8"));
                        }
                        let m = self.ea(d, s, &mut w, pc)?;
                        w[0] = 0x5000 | (*v as u16 & 7) << 9 | if name == "subq" { 0x100 } else { 0 } | s.bits() << 6 | m;
                    }
                    _ => return Err(bad()),
                }
            }
            "addx" | "subx" => {
                let s = sw(Sz::W);
                let b = if name == "addx" { 0xd100u16 } else { 0x9100 };
                match (&o[0], &o[1]) {
                    (Ea::Dn(a), Ea::Dn(d)) => w[0] = b | u16::from(*d) << 9 | s.bits() << 6 | u16::from(*a),
                    (Ea::Pre(a), Ea::Pre(d)) => w[0] = b | u16::from(*d) << 9 | s.bits() << 6 | 8 | u16::from(*a),
                    _ => return Err(bad()),
                }
            }
            "neg" | "not" | "clr" | "tst" | "negx" => {
                let s = sw(Sz::W);
                let b = match name {
                    "neg" => 0x4400u16,
                    "not" => 0x4600,
                    "clr" => 0x4200,
                    "negx" => 0x4000,
                    _ => 0x4a00,
                };
                let m = self.ea(&o[0], s, &mut w, pc)?;
                w[0] = b | s.bits() << 6 | m;
            }
            "ext" => match o[0] {
                Ea::Dn(d) => w[0] = if sw(Sz::W) == Sz::L { 0x48c0 } else { 0x4880 } | u16::from(d),
                _ => return Err(bad()),
            },
            "swap" => match o[0] {
                Ea::Dn(d) => w[0] = 0x4840 | u16::from(d),
                _ => return Err(bad()),
            },
            "exg" => match (&o[0], &o[1]) {
                (Ea::Dn(a), Ea::Dn(b)) => w[0] = 0xc140 | u16::from(*a) << 9 | u16::from(*b),
                (Ea::An(a), Ea::An(b)) => w[0] = 0xc148 | u16::from(*a) << 9 | u16::from(*b),
                (Ea::Dn(a), Ea::An(b)) => w[0] = 0xc188 | u16::from(*a) << 9 | u16::from(*b),
                (Ea::An(b), Ea::Dn(a)) => w[0] = 0xc188 | u16::from(*a) << 9 | u16::from(*b),
                _ => return Err(bad()),
            },
            "lea" => match (&o[0], &o[1]) {
                (src, Ea::An(a)) => {
                    let m = self.ea(src, Sz::L, &mut w, pc)?;
                    w[0] = 0x41c0 | u16::from(*a) << 9 | m;
                }
                _ => return Err(bad()),
            },
            "pea" => {
                let m = self.ea(&o[0], Sz::L, &mut w, pc)?;
                w[0] = 0x4840 | m;
            }
            "jmp" | "jsr" => {
                let m = self.ea(&o[0], Sz::L, &mut w, pc)?;
                w[0] = if name == "jmp" { 0x4ec0 } else { 0x4e80 } | m;
            }
            "movem" => {
                let s = sw(Sz::W);
                let l = if s == Sz::L { 0x40 } else { 0 };
                match (&o[0], &o[1]) {
                    (Ea::Imm(mask), dst) if !matches!(dst, Ea::Imm(_)) => {
                        let mut mask = *mask as u16;
                        if matches!(dst, Ea::Pre(_)) {
                            mask = mask.reverse_bits();
                        }
                        w.push(mask);
                        let m = self.ea(dst, s, &mut w, pc)?;
                        w[0] = 0x4880 | l | m;
                    }
                    (src, Ea::Imm(mask)) => {
                        w.push(*mask as u16);
                        let m = self.ea(src, s, &mut w, pc)?;
                        w[0] = 0x4c80 | l | m;
                    }
                    _ => return Err(bad()),
                }
            }
            "mulu" | "muls" | "divu" | "divs" => match (&o[0], &o[1]) {
                (src, Ea::Dn(d)) => {
                    let m = self.ea(src, Sz::W, &mut w, pc)?;
                    let b = match name {
                        "mulu" => 0xc0c0u16,
                        "muls" => 0xc1c0,
                        "divu" => 0x80c0,
                        _ => 0x81c0,
                    };
                    w[0] = b | u16::from(*d) << 9 | m;
                }
                _ => return Err(bad()),
            },
            "btst" | "bchg" | "bclr" | "bset" => {
                let t = match name {
                    "btst" => 0u16,
                    "bchg" => 1,
                    "bclr" => 2,
                    _ => 3,
                };
                match (&o[0], &o[1]) {
                    (Ea::Dn(r), d) => {
                        let m = self.ea(d, Sz::B, &mut w, pc)?;
                        w[0] = 0x0100 | u16::from(*r) << 9 | t << 6 | m;
                    }
                    (Ea::Imm(v), d) => {
                        w.push(*v as u16 & 0xff);
                        let m = self.ea(d, Sz::B, &mut w, pc)?;
                        w[0] = 0x0800 | t << 6 | m;
                    }
                    _ => return Err(bad()),
                }
            }
            "asl" | "asr" | "lsl" | "lsr" | "rol" | "ror" | "roxl" | "roxr" => {
                let kind = match &name[..name.len() - 1] {
                    "as" => 0u16,
                    "ls" => 1,
                    "rox" => 2,
                    _ => 3,
                };
                let left = name.ends_with('l');
                let dir = if left { 0x100 } else { 0 };
                let s = sw(Sz::W);
                match o.as_slice() {
                    [Ea::Dn(c), Ea::Dn(d)] => w[0] = 0xe000 | u16::from(*c) << 9 | dir | s.bits() << 6 | 0x20 | kind << 3 | u16::from(*d),
                    [Ea::Imm(c), Ea::Dn(d)] => {
                        if !(1..=8).contains(c) {
                            return Err(format!("{name} #{c}: 1 to 8"));
                        }
                        w[0] = 0xe000 | (*c as u16 & 7) << 9 | dir | s.bits() << 6 | kind << 3 | u16::from(*d);
                    }
                    [Ea::Dn(d)] => w[0] = 0xe000 | 1 << 9 | dir | s.bits() << 6 | kind << 3 | u16::from(*d),
                    [mem] => {
                        let m = self.ea(mem, Sz::W, &mut w, pc)?;
                        w[0] = 0xe0c0 | kind << 9 | dir | m;
                    }
                    _ => return Err(bad()),
                }
            }
            _ => return self.branch_or_cc(name, sz, &o),
        }
        self.put(&w);
        Ok(())
    }

    fn branch_or_cc(&mut self, name: &str, sz: Option<(Sz, bool)>, o: &[Ea]) -> R<()> {
        let pc = self.pc;
        let target = |e: &Ea| -> R<i64> {
            match e {
                Ea::AbsL(v) | Ea::AbsW(v) => Ok(*v),
                _ => Err(format!("{name}: a branch goes to an address")),
            }
        };
        if let Some(c) = name.strip_prefix('b') {
            let cc = match c {
                "ra" => Some(0),
                "sr" => Some(1),
                x => cond(x).filter(|n| *n > 1),
            };
            if let Some(cc) = cc {
                let t = target(o.first().ok_or("a branch needs a target")?)?;
                let short = matches!(sz, Some((Sz::B, _)));
                if short {
                    let d = t - (pc + 2);
                    if (d == 0 || !(-128..=127).contains(&d)) && self.pass >= 1 {
                        return Err(format!("{name}.s to {t:#x} from {pc:#x} is out of reach"));
                    }
                    self.put(&[0x6000 | cc << 8 | (d as u16 & 0xff)]);
                } else {
                    let d = t - (pc + 2);
                    self.put(&[0x6000 | cc << 8, fit16(d)?]);
                }
                return Ok(());
            }
        }
        if let Some(c) = name.strip_prefix("db") {
            let cc = if c == "ra" { Some(1) } else { cond(c) };
            if let (Some(cc), [Ea::Dn(d), tgt]) = (cc, o) {
                let t = target(tgt)?;
                let d16 = fit16(t - (pc + 2))?;
                self.put(&[0x50c8 | cc << 8 | u16::from(*d), d16]);
                return Ok(());
            }
        }
        if let Some(c) = name.strip_prefix('s') {
            if let Some(cc) = cond(c) {
                let mut w = vec![0u16];
                let m = self.ea(&o[0], Sz::B, &mut w, pc)?;
                w[0] = 0x50c0 | cc << 8 | m;
                self.put(&w);
                return Ok(());
            }
        }
        Err(format!("I don't know the instruction '{name}'"))
    }

    fn reglist(&self, s: &str) -> Option<u16> {
        let mut mask = 0u16;
        let mut any = false;
        for part in s.split('/') {
            let (a, b) = match part.split_once('-') {
                Some((a, b)) => (a, b),
                None => (part, part),
            };
            let ra = self.reg(a)?;
            let rb = self.reg(b)?;
            if ra.0 != rb.0 || rb.1 < ra.1 {
                return None;
            }
            for n in ra.1..=rb.1 {
                mask |= 1 << (n + if ra.0 { 8 } else { 0 });
            }
            any = true;
        }
        any.then_some(mask)
    }
}

fn fit16(v: i64) -> R<u16> {
    if !(-32768..=65535).contains(&v) {
        return Err(format!("{v} doesn't fit a word"));
    }
    Ok(v as u16)
}

fn parse_index(s: &str) -> (String, bool) {
    let s = s.trim();
    if let Some(r) = s.strip_suffix(".l").or_else(|| s.strip_suffix(".L")) {
        (r.to_string(), true)
    } else if let Some(r) = s.strip_suffix(".w").or_else(|| s.strip_suffix(".W")) {
        (r.to_string(), false)
    } else {
        (s.to_string(), false)
    }
}

fn cond(s: &str) -> Option<u16> {
    Some(match s {
        "t" => 0,
        "f" => 1,
        "hi" => 2,
        "ls" => 3,
        "cc" | "hs" => 4,
        "cs" | "lo" => 5,
        "ne" => 6,
        "eq" => 7,
        "vc" => 8,
        "vs" => 9,
        "pl" => 10,
        "mi" => 11,
        "ge" => 12,
        "lt" => 13,
        "gt" => 14,
        "le" => 15,
        _ => return None,
    })
}

#[derive(Debug, Clone)]
enum Tok {
    Num(i64),
    Id(String),
    Op(String),
}

fn tokenize(s: &str) -> R<Vec<Tok>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '$' {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_ascii_hexdigit() {
                j += 1;
            }
            let t: String = cs[i + 1..j].iter().collect();
            out.push(Tok::Num(i64::from_str_radix(&t, 16).map_err(|_| format!("'${t}' isn't a number"))?));
            i = j;
        } else if c == '%' && i + 1 < cs.len() && (cs[i + 1] == '0' || cs[i + 1] == '1') && !matches!(out.last(), Some(Tok::Num(_)) | Some(Tok::Id(_))) {
            let mut j = i + 1;
            while j < cs.len() && (cs[j] == '0' || cs[j] == '1') {
                j += 1;
            }
            let t: String = cs[i + 1..j].iter().collect();
            out.push(Tok::Num(i64::from_str_radix(&t, 2).unwrap()));
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < cs.len() && cs[j].is_ascii_alphanumeric() {
                j += 1;
            }
            let t: String = cs[i..j].iter().collect();
            if let Some(h) = t.strip_prefix("0x") {
                out.push(Tok::Num(i64::from_str_radix(h, 16).map_err(|_| format!("'{t}' isn't a number"))?));
            } else {
                out.push(Tok::Num(t.parse().map_err(|_| format!("'{t}' isn't a number"))?));
            }
            i = j;
        } else if c == '\'' || c == '"' {
            let mut j = i + 1;
            let mut v = 0i64;
            while j < cs.len() && cs[j] != c {
                v = v << 8 | cs[j] as i64;
                j += 1;
            }
            out.push(Tok::Num(v));
            i = j + 1;
        } else if c.is_alphabetic() || c == '_' || c == '@' {
            let mut j = i;
            while j < cs.len() && is_ident(cs[j]) {
                j += 1;
            }
            out.push(Tok::Id(cs[i..j].iter().collect()));
            i = j;
        } else {
            let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            if matches!(two.as_str(), "<<" | ">>" | "<>" | "<=" | ">=" | "==" | "!=") {
                out.push(Tok::Op(two));
                i += 2;
            } else if "+-*/%&|^~()=<>".contains(c) {
                out.push(Tok::Op(c.to_string()));
                i += 1;
            } else {
                return Err(format!("unexpected '{c}' in an expression"));
            }
        }
    }
    Ok(out)
}
