//! A small Lua lexer: enough to find calls, assignments and handlers without
//! being fooled by comments and strings. It does not parse; callers look at
//! token patterns. (The spec names `full-moon` for a full AST; it is not
//! available offline, and token patterns cover what the scanner needs. Swap
//! it in behind this module if a rule ever needs real structure.)

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Name(String),
    Number(f64),
    Str(String),
    /// Operators and punctuation, longest match (`==`, `..`, `...`, `::`).
    Punct(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    /// Byte range in the source.
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn is_name(&self, n: &str) -> bool {
        matches!(&self.tok, Tok::Name(s) if s == n)
    }
    pub fn is_punct(&self, p: &str) -> bool {
        matches!(&self.tok, Tok::Punct(s) if s == p)
    }
}

pub fn lex(src: &str) -> Vec<Token> {
    let b = src.as_bytes();
    let (mut i, mut line) = (0usize, 1u32);
    let mut out: Vec<Token> = Vec::new();
    while i < b.len() {
        let tok_start = i;
        let before = out.len();
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"--") {
            i += 2;
            if let Some(n) = long_bracket_level(b, i) {
                let (end, lines) = skip_long(b, i, n);
                line += lines;
                i = end;
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
        } else if c == b'"' || c == b'\'' {
            let (s, end, lines) = read_quoted(b, i);
            out.push(Token { tok: Tok::Str(s), line, start: 0, end: 0 });
            line += lines;
            i = end;
        } else if c == b'[' && long_bracket_level(b, i).is_some() {
            let n = long_bracket_level(b, i).unwrap();
            let start_line = line;
            let (end, lines) = skip_long(b, i, n);
            let open = i + n + 2;
            let close = end.saturating_sub(n + 2).max(open);
            let text = String::from_utf8_lossy(&b[open..close]).trim_start_matches('\n').to_string();
            out.push(Token { tok: Tok::Str(text), line: start_line, start: 0, end: 0 });
            line += lines;
            i = end;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Token { tok: Tok::Name(String::from_utf8_lossy(&b[s..i]).to_string()), line, start: 0, end: 0 });
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let s = i;
            if c == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) {
                i += 2;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let v = i64::from_str_radix(std::str::from_utf8(&b[s + 2..i]).unwrap_or("0"), 16).unwrap_or(0);
                out.push(Token { tok: Tok::Number(v as f64), line, start: 0, end: 0 });
            } else {
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b'e' || b[i] == b'E' || ((b[i] == b'-' || b[i] == b'+') && matches!(b[i - 1], b'e' | b'E'))) {
                    // `1..2` is a concatenation after a number, not a malformed number.
                    if b[i] == b'.' && b.get(i + 1) == Some(&b'.') {
                        break;
                    }
                    i += 1;
                }
                let v = std::str::from_utf8(&b[s..i]).unwrap_or("0").parse().unwrap_or(0.0);
                out.push(Token { tok: Tok::Number(v), line, start: 0, end: 0 });
            }
        } else {
            let three = &b[i..b.len().min(i + 3)];
            let two = &b[i..b.len().min(i + 2)];
            let p: &[u8] = if three == b"..." {
                three
            } else if [&b"=="[..], b"~=", b"<=", b">=", b"..", b"::"].contains(&two) {
                two
            } else {
                &b[i..i + 1]
            };
            out.push(Token { tok: Tok::Punct(String::from_utf8_lossy(p).to_string()), line, start: 0, end: 0 });
            i += p.len();
        }
        if out.len() > before {
            let t = out.last_mut().unwrap();
            t.start = tok_start;
            t.end = i;
        }
    }
    out
}

/// At `[`: the `=` count of a long bracket `[==[`, if it is one.
fn long_bracket_level(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let mut n = 0;
    while b.get(i + 1 + n) == Some(&b'=') {
        n += 1;
    }
    (b.get(i + 1 + n) == Some(&b'[')).then_some(n)
}

/// Skips a long bracket starting at `i`; returns the index after it and the
/// newlines inside.
fn skip_long(b: &[u8], i: usize, n: usize) -> (usize, u32) {
    let mut close = vec![b']'];
    close.extend(std::iter::repeat_n(b'=', n));
    close.push(b']');
    let mut j = i + n + 2;
    let mut lines = 0;
    while j < b.len() {
        if b[j..].starts_with(&close) {
            return (j + close.len(), lines);
        }
        if b[j] == b'\n' {
            lines += 1;
        }
        j += 1;
    }
    (b.len(), lines)
}

fn read_quoted(b: &[u8], i: usize) -> (String, usize, u32) {
    let q = b[i];
    let mut j = i + 1;
    let mut s = Vec::new();
    let mut lines = 0;
    while j < b.len() && b[j] != q && b[j] != b'\n' {
        if b[j] == b'\\' && j + 1 < b.len() {
            j += 1;
            match b[j] {
                b'n' => s.push(b'\n'),
                b't' => s.push(b'\t'),
                b'\n' => {
                    s.push(b'\n');
                    lines += 1;
                }
                c => s.push(c),
            }
        } else {
            s.push(b[j]);
        }
        j += 1;
    }
    (String::from_utf8_lossy(&s).to_string(), (j + 1).min(b.len().max(j)).min(j + 1), lines)
}

/// For each `function` token at index `i`, where its block ends (the index
/// of its matching `end`). Counts `function`/`if`/`do`/`repeat` openers
/// (`for` and `while` open through their `do`).
pub fn block_end(toks: &[Token], start: usize) -> usize {
    let mut depth = 0i32;
    for (k, t) in toks.iter().enumerate().skip(start) {
        if let Tok::Name(n) = &t.tok {
            match n.as_str() {
                "function" | "if" | "do" | "repeat" => depth += 1,
                "end" | "until" => {
                    depth -= 1;
                    if depth == 0 {
                        return k;
                    }
                }
                _ => {}
            }
        }
    }
    toks.len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(src: &str) -> Vec<String> {
        lex(src).into_iter().filter_map(|t| if let Tok::Name(n) = t.tok { Some(n) } else { None }).collect()
    }

    #[test]
    fn comments_and_strings_hide_code() {
        let src = "-- love.keyboard.isDown('x')\nlocal s = \"love.mouse.getPosition\" --[[ block\nrequire('ffi') ]] local ok = 1";
        assert_eq!(names(src), ["local", "s", "local", "ok"]);
    }

    #[test]
    fn long_strings_and_lines() {
        let t = lex("local a = [==[\nhi ]] there\n]==]\nfoo()");
        assert!(t.iter().any(|t| t.tok == Tok::Str("hi ]] there\n".into())));
        let foo = t.iter().find(|t| t.is_name("foo")).unwrap();
        assert_eq!(foo.line, 4);
    }

    #[test]
    fn numbers_strings_and_operators() {
        let t = lex("x = 0xFF .. 'a\\'b' == 1.5e1 ... 3..4");
        let toks: Vec<_> = t.into_iter().map(|t| t.tok).collect();
        assert!(toks.contains(&Tok::Number(255.0)));
        assert!(toks.contains(&Tok::Str("a'b".into())));
        assert!(toks.contains(&Tok::Number(15.0)));
        assert!(toks.contains(&Tok::Punct("...".into())));
        assert!(toks.contains(&Tok::Punct("==".into())));
        assert_eq!(toks.iter().filter(|t| **t == Tok::Punct("..".into())).count(), 2);
    }

    #[test]
    fn blocks_find_their_end() {
        let t = lex("function a() if x then for i=1,2 do y() end else z() end repeat q() until w end local after = 1");
        let end = block_end(&t, 0);
        assert!(t[end].is_name("end"));
        assert!(t[end + 1].is_name("local"));
    }
}
