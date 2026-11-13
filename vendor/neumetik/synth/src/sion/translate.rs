// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of parts of SiON's Translator (org.si.sion.utils.Translator,
// github.com/keim/SiON): the table numbers (`{...}`, `#TABLE`) and the
// voice parameters of `#@`, `#OPM@`, `#OPN@` and `#OPL@`.
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

use super::module::*;
use super::table::*;
use super::track::EnvTable;

/// `SiMMLTable.alg_opm`: the algorithm numbers of OPM and OPN by operator count.
const ALG_OPM: [[i32; 16]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 1, 1, 1, 0, 1, 1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 2, 3, 3, 4, 3, 5, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 2, 3, 4, 5, 6, 7, -1, -1, -1, -1, -1, -1, -1, -1],
];

/// `SiMMLTable.alg_opl`.
const ALG_OPL: [[i32; 16]; 4] = [
    [0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 3, 2, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 4, 8, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
];

/// Which parameter format a `#` command takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToneFormat {
    Param,
    Opm,
    Opn,
    Opl,
}

/// ActionScript's `int(string)` of a number as text.
fn int_of(s: &str) -> i32 {
    to_i32(s.trim().parse::<f64>().unwrap_or(0.0))
}

fn comments_out(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            if let Some(e) = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '*' && chars[j + 1] == '/') {
                i = e + 2;
                continue;
            }
        } else if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            if let Some(n) = (i + 2..chars.len()).find(|&j| matches!(chars[j], '\r' | '\n')) {
                let mut j = n;
                while j < chars.len() && matches!(chars[j], '\r' | '\n') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `_splitDataString`: the numbers of a voice, and how many operators they make.
fn split_data(data: &str, ch_params: usize, op_params: usize) -> Result<Option<(Vec<String>, i32)>, String> {
    if data.is_empty() {
        return Ok(None);
    }
    let clean = comments_out(data);
    let keep = |c: char| c.is_ascii_digit() || c == '-' || c == '.';
    let trimmed = clean.trim_matches(|c: char| !keep(c));
    let mut parts: Vec<String> = vec![];
    let mut cur = String::new();
    for c in trimmed.chars() {
        if keep(c) {
            cur.push(c);
        } else if !cur.is_empty() {
            parts.push(std::mem::take(&mut cur));
        }
    }
    parts.push(cur);
    for i in 1..5usize {
        if parts.len() == ch_params + op_params * i {
            return Ok(Some((parts, i as i32)));
        }
    }
    Err("the number of parameters of a tone isn't right".into())
}

/// `parseParam`, `parseOPMParam`, `parseOPNParam`, `parseOPLParam`: a voice from text.
pub fn parse_tone(format: ToneFormat, param: &mut ChannelParam, data: &str, table: &Table) -> Result<(), String> {
    let (ch, op) = match format {
        ToneFormat::Param => (3, 15),
        ToneFormat::Opm => (2, 11),
        ToneFormat::Opn => (2, 10),
        ToneFormat::Opl => (2, 11),
    };
    let Some((d, count)) = split_data(data, ch, op)? else {
        param.ope_count = 0;
        return Ok(());
    };
    param.ope_count = count;
    let n = |i: usize| -> i32 { int_of(&d[i]) };
    match format {
        ToneFormat::Param => {
            param.alg = n(0);
            param.fb = n(1);
            param.fbc = n(2);
            let mut k = 3;
            for o in 0..count as usize {
                let p = &mut param.operator_param[o];
                p.set_pg_type(table, n(k) & 511);
                p.ar = n(k + 1) & 63;
                p.dr = n(k + 2) & 63;
                p.sr = n(k + 3) & 63;
                p.rr = n(k + 4) & 63;
                p.sl = n(k + 5) & 15;
                p.tl = n(k + 6) & 127;
                p.ksr = n(k + 7) & 3;
                p.ksl = n(k + 8) & 3;
                let m: f64 = d[k + 9].parse().unwrap_or(f64::NAN);
                p.fmul = if m == 0.0 { 64 } else { to_i32(m * 128.0) };
                p.dt1 = n(k + 10) & 7;
                p.detune = n(k + 11);
                p.ams = n(k + 12) & 3;
                let i = n(k + 13);
                p.phase = if i == -1 { i } else { i & 255 };
                p.fixed_pitch = (n(k + 14) & 127) << 6;
                k += 15;
            }
        }
        ToneFormat::Opm | ToneFormat::Opn => {
            let alg = ALG_OPM[count as usize - 1][(n(0) & 15) as usize];
            if alg == -1 {
                return Err("that algorithm isn't valid".into());
            }
            param.alg = alg;
            param.fb = n(1);
            let mut k = 2;
            for o in 0..count as usize {
                let p = &mut param.operator_param[o];
                p.ar = (n(k) << 1) & 63;
                p.dr = (n(k + 1) << 1) & 63;
                p.sr = (n(k + 2) << 1) & 63;
                p.rr = ((n(k + 3) << 2) + 2) & 63;
                p.sl = n(k + 4) & 15;
                p.tl = n(k + 5) & 127;
                p.ksr = n(k + 6) & 3;
                p.set_mul(n(k + 7) & 15);
                p.dt1 = n(k + 8) & 7;
                if format == ToneFormat::Opm {
                    p.detune = table.dt2_table[(n(k + 9) & 3) as usize];
                    p.ams = n(k + 10) & 3;
                    k += 11;
                } else {
                    p.ams = n(k + 9) & 3;
                    k += 10;
                }
            }
        }
        ToneFormat::Opl => {
            let alg = ALG_OPL[count as usize - 1][(n(0) & 15) as usize];
            if alg == -1 {
                return Err("that algorithm isn't valid".into());
            }
            param.fratio = 133;
            param.alg = alg;
            param.fb = n(1);
            let mut k = 2;
            for o in 0..count as usize {
                let p = &mut param.operator_param[o];
                p.set_pg_type(table, PG_MA3_WAVE as i32 + (n(k) & 31));
                p.ar = (n(k + 1) << 2) & 63;
                p.dr = (n(k + 2) << 2) & 63;
                p.rr = (n(k + 3) << 2) & 63;
                p.sr = if n(k + 4) != 0 { 0 } else { p.rr };
                p.sl = n(k + 5) & 15;
                p.tl = n(k + 6) & 63;
                p.ksr = (n(k + 7) << 1) & 3;
                p.ksl = n(k + 8) & 3;
                let i = n(k + 9) & 15;
                p.set_mul(if i == 11 || i == 13 {
                    i - 1
                } else if i == 14 {
                    i + 1
                } else {
                    i
                });
                p.ams = n(k + 10) & 3;
                k += 11;
            }
        }
    }
    Ok(())
}

/// `parseTableNumbers`: the numbers of a table, with `(a,b),n` interpolations,
/// `[ ]n` loops and a `|` repeat point, the postfix `n*r+o` repeating, scaling and offsetting.
pub fn parse_table_numbers(numbers: &str, postfix: &str, max_index: usize) -> Result<EnvTable, String> {
    let t = parse_numbers(numbers, postfix, max_index)?;
    Ok(EnvTable::new(t.values, t.rep))
}

/// What `parseTableNumbers` found: the list, its repeat point, and how many numbers it counted.
pub struct TableNumbers {
    pub values: Vec<i32>,
    pub rep: Option<usize>,
    pub count: usize,
}

/// `parseWAV`: a wave of 2 to 1024 numbers in [-1, 1] from table numbers (`#WAV`).
pub fn parse_wav(numbers: &str, postfix: &str) -> Result<Vec<f64>, String> {
    let t = parse_numbers(numbers, postfix, 1024)?;
    let mut imax = 2;
    while imax < 1024 {
        if imax >= t.count {
            break;
        }
        imax <<= 1;
    }
    let mut wav = vec![0.0; imax];
    // The list ends, or goes round from its repeat point.
    let mut pos = Some(0usize);
    for slot in wav.iter_mut() {
        let Some(p) = pos else { break };
        let v = (f64::from(t.values[p]) + 0.5) * 0.0078125;
        *slot = v.clamp(-1.0, 1.0);
        pos = if p + 1 < t.values.len() { Some(p + 1) } else { t.rep.filter(|&r| r < t.values.len()) };
    }
    Ok(wav)
}

/// `parseWAVB`: a wave of signed bytes from hex digits (`#WAVB`).
pub fn parse_wavb(hex: &str) -> Vec<f64> {
    let digits: Vec<char> = hex.chars().filter(|c| !c.is_whitespace()).collect();
    (0..digits.len() / 2)
        .map(|i| {
            let ub = i32::from_str_radix(&digits[i * 2..i * 2 + 2].iter().collect::<String>(), 16).unwrap_or(0);
            if ub < 128 {
                f64::from(ub) * 0.0078125
            } else {
                f64::from(ub - 256) * 0.0078125
            }
        })
        .collect()
}

fn parse_numbers(numbers: &str, postfix: &str, max_index: usize) -> Result<TableNumbers, String> {
    let post: Vec<char> = postfix.chars().collect();
    // (\d+)?(\*(-?[\d.]+))?(([+-])([\d.]+))?
    let mut i = 0;
    let num = |i: &mut usize, neg: bool| -> Option<f64> {
        let s = *i;
        if neg && post.get(*i) == Some(&'-') {
            *i += 1;
        }
        while *i < post.len() && (post[*i].is_ascii_digit() || post[*i] == '.') {
            *i += 1;
        }
        let t: String = post[s..*i].iter().collect();
        if !t.chars().any(|c| c.is_ascii_digit()) {
            None
        } else {
            Some(t.parse::<f64>().unwrap_or(0.0))
        }
    };
    let mut jmax = 1i32;
    let ds = i;
    while i < post.len() && post[i].is_ascii_digit() {
        i += 1;
    }
    if i > ds {
        jmax = post[ds..i].iter().collect::<String>().parse().unwrap_or(1);
    }
    let mut r = 1.0;
    if post.get(i) == Some(&'*') {
        let mut k = i + 1;
        if let Some(v) = num(&mut k, true) {
            r = v;
            i = k;
        }
    }
    let mut o = 0.0;
    if matches!(post.get(i), Some('+' | '-')) {
        let minus = post[i] == '-';
        let mut k = i + 1;
        if let Some(v) = num(&mut k, false) {
            o = if minus { -v } else { v };
        }
    }

    let s: Vec<char> = numbers.chars().collect();
    let mut out: Vec<i32> = vec![];
    let mut rep: Option<usize> = None;
    let mut stack: Vec<usize> = vec![];
    let mut index = 0usize;
    let mut p = 0;
    while p < s.len() && index < max_index {
        let c = s[p];
        if c == '(' {
            // \(\s*([,\-\d\s]+)\)[,\s]*(\d+)
            let mut q = p + 1;
            while q < s.len() && (s[q] == ',' || s[q] == '-' || s[q].is_ascii_digit() || s[q].is_whitespace()) {
                q += 1;
            }
            if s.get(q) == Some(&')') {
                let inner: String = s[p + 1..q].iter().collect();
                let mut e = q + 1;
                while e < s.len() && (s[e] == ',' || s[e].is_whitespace()) {
                    e += 1;
                }
                let ds = e;
                while e < s.len() && s[e].is_ascii_digit() {
                    e += 1;
                }
                if e > ds {
                    let imax: i32 = s[ds..e].iter().collect::<String>().parse().unwrap_or(0);
                    let array: Vec<i32> = inner.split(|c: char| c == ',' || c.is_whitespace()).filter(|x| !x.is_empty()).map(int_of).collect();
                    if imax < 2 || array.is_empty() {
                        return Err("the table's interpolation isn't valid".into());
                    }
                    if array.len() > 1 {
                        let mut t = 0.0f64;
                        let step = (array.len() - 1) as f64 / f64::from(imax);
                        for _ in 0..imax {
                            if index >= max_index {
                                break;
                            }
                            let ti0 = t as usize;
                            let ti1 = (ti0 + 1).min(array.len() - 1);
                            let tr = t - ti0 as f64;
                            let v = to_i32(f64::from(array[ti0]) * (1.0 - tr) + f64::from(array[ti1]) * tr + 0.5);
                            let v = to_i32(f64::from(v) * r + o + 0.5);
                            for _ in 0..jmax {
                                out.push(v);
                                index += 1;
                            }
                            t += step;
                        }
                    } else {
                        let v = to_i32(f64::from(array[0]) * r + o + 0.5);
                        for _ in 0..imax {
                            if index >= max_index {
                                break;
                            }
                            for _ in 0..jmax {
                                out.push(v);
                                index += 1;
                            }
                        }
                    }
                    p = e;
                    continue;
                }
            }
            p += 1;
        } else if c.is_ascii_digit() || (c == '-' && s.get(p + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = p;
            p += 1;
            while p < s.len() && s[p].is_ascii_digit() {
                p += 1;
            }
            let n: i32 = s[st..p].iter().collect::<String>().parse().unwrap_or(0);
            let v = to_i32(f64::from(n) * r + o + 0.5);
            for _ in 0..jmax {
                out.push(v);
            }
            index += 1;
        } else if c == '|' {
            rep = Some(out.len());
            p += 1;
        } else if c == '[' {
            stack.push(out.len());
            p += 1;
        } else if c == ']' {
            p += 1;
            let ds = p;
            while p < s.len() && s[p].is_ascii_digit() {
                p += 1;
            }
            let n: usize = s[ds..p].iter().collect::<String>().parse().unwrap_or(0);
            let Some(start) = stack.pop() else { return Err("the table's loop has no [".into()) };
            let end = out.len();
            if start >= end {
                return Err("the table's loop is empty".into());
            }
            for _ in 0..(if n == 0 { 2 } else { n }) {
                for k in start..end {
                    let v = out[k];
                    out.push(v);
                }
            }
        } else {
            p += 1;
        }
    }
    if out.is_empty() {
        return Err("the table has no numbers".into());
    }
    Ok(TableNumbers { values: out, rep, count: index })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_numbers_loops_interpolation_and_postfix() {
        let t = parse_table_numbers("1,2,3", "", 1000).unwrap();
        assert_eq!(t.values, vec![1, 2, 3]);
        assert_eq!(t.loop_to, 2);
        // A repeat point.
        let t = parse_table_numbers("1,2 | 3,4", "", 1000).unwrap();
        assert_eq!((t.values.clone(), t.loop_to), (vec![1, 2, 3, 4], 2));
        // [ ]n: the loop body appears n more times.
        assert_eq!(parse_table_numbers("[1,2]2", "", 1000).unwrap().values, vec![1, 2, 1, 2, 1, 2]);
        // (a,b),n goes from a to b in n steps.
        assert_eq!(parse_table_numbers("(0,8),4", "", 1000).unwrap().values, vec![0, 2, 4, 6]);
        // The postfix: each number twice, times 2, plus 1.
        assert_eq!(parse_table_numbers("1,2", "2*2+1", 1000).unwrap().values, vec![3, 3, 5, 5]);
        assert!(parse_table_numbers("", "", 10).is_err());
    }

    #[test]
    fn waves_from_numbers_and_from_hex() {
        // 4 numbers: a wave of 4, each (n + 0.5) / 128.
        let w = parse_wav("0,64,-64,127", "").unwrap();
        assert_eq!(w.len(), 4);
        assert!((w[1] - 64.5 / 128.0).abs() < 1e-12 && (w[3] - 127.5 / 128.0).abs() < 1e-12 && w[2] < 0.0);
        // Three numbers round up to four, the last zero.
        assert_eq!(parse_wav("1,2,3", "").unwrap().len(), 4);
        assert_eq!(parse_wavb("00 40 80 ff"), vec![0.0, 0.5, -1.0, -0.0078125]);
    }

    #[test]
    fn voices_by_number_in_each_format() {
        let table = Table::new(44100);
        // #OPM@ with one operator: alg, fb, then 11.
        let mut p = ChannelParam::default();
        parse_tone(ToneFormat::Opm, &mut p, "0 0  31 0 0 15 0 20 0 1 0 0 0", &table).unwrap();
        assert_eq!(p.ope_count, 1);
        assert_eq!(p.operator_param[0].ar, 62);
        assert_eq!(p.operator_param[0].tl, 20);
        assert_eq!(p.operator_param[0].rr, (15 << 2) + 2 & 63);
        assert_eq!(p.operator_param[0].mul(), 1);
        // #@ with 2 operators: 3 + 15 * 2 numbers.
        let mut p = ChannelParam::default();
        let ops = "0 31 0 0 15 0 0 0 0 1 1 0 0 0 0";
        parse_tone(ToneFormat::Param, &mut p, &format!("2 3 0  {ops}  {ops}"), &table).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((p.ope_count, p.alg, p.fb), (2, 2, 3));
        // Not a count of an operator.
        assert!(parse_tone(ToneFormat::Opm, &mut ChannelParam::default(), "1 2 3", &table).is_err());
    }
}
