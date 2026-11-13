// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A small XML reader of its own (elements, attributes, text, CDATA and
//! the five entities and character references), for the formats kept as
//! XML (Arkos Tracker's, Renoise's), and base64 for what they embed.

/// Base64 decoded, anything not of its alphabet skipped.
pub(crate) fn base64(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// An XML element: its name, attributes, children and text.
#[derive(Debug, Default)]
pub(crate) struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The text of the element and everything in it.
    pub fn all_text(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.all_text());
        }
        s
    }

    pub fn string(&self, name: &str) -> String {
        self.child(name).map(|c| c.all_text()).unwrap_or_default()
    }

    /// A child's whole number (as JUCE's getIntValue reads it), or `fallback` when absent or empty.
    pub fn int_or(&self, name: &str, fallback: i32) -> i32 {
        match self.child(name).map(|c| c.all_text()) {
            Some(t) if !t.is_empty() => int_value(&t),
            _ => fallback,
        }
    }

    pub fn int(&self, name: &str) -> i32 {
        self.int_or(name, 0)
    }

    pub fn float_or(&self, name: &str, fallback: f64) -> f64 {
        match self.child(name).map(|c| c.all_text()) {
            Some(t) if !t.is_empty() => float_value(&t),
            _ => fallback,
        }
    }

    pub fn bool(&self, name: &str) -> bool {
        self.child(name).is_some_and(|c| c.all_text().trim().eq_ignore_ascii_case("true"))
    }
}

impl Node {
    /// The element written back as XML (attributes, children and text as
    /// read; whitespace between elements dropped), leaving out the
    /// children `skip` names, at any depth.
    pub fn write(&self, skip: &[&str], out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape(v, out);
            out.push('"');
        }
        out.push('>');
        escape(self.text.trim(), out);
        for c in self.children.iter().filter(|c| !skip.contains(&c.name.as_str())) {
            c.write(skip, out);
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// The leading whole number of a string (after spaces, with a sign), 0 if none.
pub(crate) fn int_value(s: &str) -> i32 {
    let s = s.trim_start();
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let mut v: i64 = 0;
    for c in digits.chars() {
        match c.to_digit(10) {
            Some(d) => v = (v * 10 + i64::from(d)).min(i64::from(i32::MAX)),
            None => break,
        }
    }
    (if neg { -v } else { v }) as i32
}

pub(crate) fn float_value(s: &str) -> f64 {
    let s = s.trim();
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0) || c == 'e' || c == 'E'))
        .map_or(s.len(), |(i, _)| i);
    s[..end].parse().unwrap_or(0.0)
}

pub(crate) fn hex_value(s: &str) -> Option<i32> {
    let s = s.strip_prefix('#')?;
    let mut v: u32 = 0;
    for c in s.chars() {
        match c.to_digit(16) {
            Some(d) => v = v.wrapping_mul(16).wrapping_add(d),
            None => break,
        }
    }
    Some(v as i32)
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e < 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parses an XML document to its root element.
pub(crate) fn parse_xml(d: &[u8]) -> Option<Node> {
    let text = String::from_utf8_lossy(d);
    let s = text.trim_start_matches('\u{feff}');
    let b = s.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    while i < b.len() {
        if b[i] == b'<' {
            if s[i..].starts_with("<!--") {
                i = s[i..].find("-->").map(|e| i + e + 3)?;
            } else if s[i..].starts_with("<![CDATA[") {
                let e = s[i..].find("]]>").map(|e| i + e)?;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&s[i + 9..e]);
                }
                i = e + 3;
            } else if s[i..].starts_with("<?") {
                i = s[i..].find("?>").map(|e| i + e + 2)?;
            } else if s[i..].starts_with("<!") {
                i = s[i..].find('>').map(|e| i + e + 1)?;
            } else if s[i..].starts_with("</") {
                let e = s[i..].find('>').map(|e| i + e)?;
                let node = stack.pop()?;
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                } else {
                    root = Some(node);
                    break;
                }
                i = e + 1;
            } else {
                let e = tag_end(b, i)?;
                let inner = &s[i + 1..e];
                let self_closing = inner.ends_with('/');
                let inner = inner.trim_end_matches('/');
                let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
                let mut node = Node {
                    name: inner[..name_end].to_string(),
                    ..Default::default()
                };
                node.attrs = attributes(&inner[name_end..]);
                if self_closing {
                    if let Some(top) = stack.last_mut() {
                        top.children.push(node);
                    } else {
                        root = Some(node);
                        break;
                    }
                } else {
                    stack.push(node);
                }
                i = e + 1;
            }
        } else {
            let e = s[i..].find('<').map_or(b.len(), |e| i + e);
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&unescape(&s[i..e]));
            }
            i = e;
        }
    }
    root
}

/// Where a start tag ends, past any quoted attribute.
fn tag_end(b: &[u8], from: usize) -> Option<usize> {
    let mut q = 0u8;
    for (k, &c) in b.iter().enumerate().skip(from) {
        if q != 0 {
            if c == q {
                q = 0;
            }
        } else if c == b'"' || c == b'\'' {
            q = c;
        } else if c == b'>' {
            return Some(k);
        }
    }
    None
}

fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim().to_string();
        let r = rest[eq + 1..].trim_start();
        let Some(q) = r.chars().next().filter(|&c| c == '"' || c == '\'') else { break };
        let Some(end) = r[1..].find(q) else { break };
        out.push((name, unescape(&r[1..1 + end])));
        rest = r[end + 2..].trim_start();
    }
    out
}
