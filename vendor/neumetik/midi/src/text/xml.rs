// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! A small XML reader: elements, attributes, text, CDATA and the five
//! entities and character references; the prolog, DOCTYPE, comments and
//! processing instructions skipped. Enough for MusicXML and MapleStory 2.

use super::bad;
use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// Where it starts in the text.
    pub offset: usize,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|a| a.0 == name).map(|a| a.1.as_str())
    }
    /// Its child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.name == name)
    }
    /// All its text, trimmed.
    pub fn text(&self) -> String {
        let mut out = String::new();
        fn gather(e: &Element, out: &mut String) {
            for c in &e.children {
                match c {
                    Node::Text(t) => out.push_str(t),
                    Node::Element(e) => gather(e, out),
                }
            }
        }
        gather(self, &mut out);
        out.trim().to_string()
    }
    /// A child's text.
    pub fn get(&self, name: &str) -> Option<String> {
        self.child(name).map(|c| c.text())
    }
    /// A child's text as a number.
    pub fn num(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(|t| t.trim().parse().ok())
    }
    /// Every element in it, itself first, depth first.
    pub fn walk(&self, f: &mut impl FnMut(&Element)) {
        f(self);
        for e in self.elements() {
            e.walk(f);
        }
    }
}

/// Reads a document's root element.
pub fn parse(text: &str) -> Result<Element, Error> {
    let mut p = Parser { s: text, i: 0 };
    p.misc()?;
    if !p.s[p.i..].starts_with('<') {
        return Err(bad(text, p.i, "an XML document starts with an element"));
    }
    let root = p.element(0)?;
    Ok(root)
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn skip_space(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start().len();
    }
    /// Skips what can come before or between elements: space, `<?...?>`,
    /// `<!--...-->`, `<!DOCTYPE ...>`.
    fn misc(&mut self) -> Result<(), Error> {
        loop {
            self.skip_space();
            let r = self.rest();
            if r.starts_with("<?") {
                self.until("?>")?;
            } else if r.starts_with("<!--") {
                self.until("-->")?;
            } else if r.starts_with("<!DOCTYPE") || r.starts_with("<!doctype") {
                // An internal subset in [ ] may hold >.
                let mut depth = 0;
                let start = self.i;
                for (k, c) in r.char_indices() {
                    match c {
                        '[' => depth += 1,
                        ']' => depth -= 1,
                        '>' if depth <= 0 => {
                            self.i += k + 1;
                            break;
                        }
                        _ => {}
                    }
                }
                if self.i == start {
                    return Err(bad(self.s, start, "a DOCTYPE that doesn't end"));
                }
            } else {
                return Ok(());
            }
        }
    }
    fn until(&mut self, end: &str) -> Result<&'a str, Error> {
        let r = self.rest();
        let k = r.find(end).ok_or_else(|| bad(self.s, self.i, "XML that ends partway"))?;
        self.i += k + end.len();
        Ok(&r[..k])
    }
    fn name(&mut self) -> Result<String, Error> {
        let r = self.rest();
        let n = r.find(|c: char| c.is_whitespace() || matches!(c, '>' | '/' | '=')).unwrap_or(r.len());
        if n == 0 {
            return Err(bad(self.s, self.i, "an XML name is missing"));
        }
        self.i += n;
        Ok(r[..n].to_string())
    }
    fn element(&mut self, depth: usize) -> Result<Element, Error> {
        if depth > 256 {
            return Err(bad(self.s, self.i, "XML nested too deep"));
        }
        let offset = self.i;
        self.i += 1; // <
        let name = self.name()?;
        let mut e = Element { name, offset, ..Default::default() };
        loop {
            self.skip_space();
            let r = self.rest();
            if r.starts_with("/>") {
                self.i += 2;
                return Ok(e);
            }
            if r.starts_with('>') {
                self.i += 1;
                break;
            }
            if r.is_empty() {
                return Err(bad(self.s, offset, "an element that doesn't end"));
            }
            let at = self.i;
            let key = self.name()?;
            self.skip_space();
            if !self.rest().starts_with('=') {
                return Err(bad(self.s, at, "an attribute needs = and a value"));
            }
            self.i += 1;
            self.skip_space();
            let q = self.rest().chars().next().ok_or_else(|| bad(self.s, at, "an attribute's value is missing"))?;
            if q != '"' && q != '\'' {
                return Err(bad(self.s, at, "an attribute's value is in quotes"));
            }
            self.i += 1;
            let r = self.rest();
            let k = r.find(q).ok_or_else(|| bad(self.s, at, "an attribute's value doesn't end"))?;
            self.i += k + 1;
            e.attrs.push((key, unescape(&r[..k])));
        }
        loop {
            let r = self.rest();
            if r.is_empty() {
                return Err(bad(self.s, offset, "an element that isn't closed"));
            }
            if r.starts_with("</") {
                self.i += 2;
                let close = self.name()?;
                if close != e.name {
                    return Err(bad(self.s, self.i - close.len(), "a closing tag that doesn't match"));
                }
                self.until(">")?;
                return Ok(e);
            }
            if r.starts_with("<![CDATA[") {
                self.i += 9;
                let t = self.until("]]>")?;
                e.children.push(Node::Text(t.to_string()));
            } else if r.starts_with("<!--") {
                self.until("-->")?;
            } else if r.starts_with("<?") {
                self.until("?>")?;
            } else if r.starts_with('<') {
                let child = self.element(depth + 1)?;
                e.children.push(Node::Element(child));
            } else {
                let k = r.find('<').unwrap_or(r.len());
                let t = &r[..k];
                self.i += k;
                if !t.trim().is_empty() {
                    e.children.push(Node::Text(unescape(t)));
                }
            }
        }
    }
}

/// Entities and character references as their characters.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(end) = after.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = after;
            continue;
        };
        let ent = &after[..end];
        let c = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_document() {
        let doc = parse("<?xml version=\"1.0\"?>\n<!DOCTYPE x [ <!ENTITY y \"z\"> ]>\n<!-- hi --><a b='1' c=\"&lt;2&#x41;\"><x>one &amp; two</x><y/><![CDATA[<raw>]]></a>").unwrap();
        assert_eq!(doc.name, "a");
        assert_eq!(doc.attr("c"), Some("<2A"));
        assert_eq!(doc.get("x").as_deref(), Some("one & two"));
        assert!(doc.child("y").is_some());
        assert!(doc.text().ends_with("<raw>"));
        assert!(matches!(parse("<a><b></a>"), Err(Error::BadText { .. })));
    }
}
