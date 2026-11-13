//! The offline API reference (spec 3B): search over a LÖVE version's
//! `love-api` data and snippets for the editor. The data is the
//! love2d-community `love-api` JSON (modules, functions with variants,
//! callbacks, types, enums); a version's file is bundled (11.5) or fetched
//! into the toolchain cache (12.0 and later). Optional `since` and
//! `deprecated` fields on an entry become the "Added in" and "Deprecated in"
//! badges.

use crate::{Error, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Function,
    Callback,
    Method,
    Type,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Full dotted name: `love.graphics.draw`, `love.load`, `Image:getWidth`.
    pub name: String,
    pub kind: EntryKind,
    pub description: String,
    pub signatures: Vec<Vec<Arg>>,
    pub since: Option<String>,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: String,
    pub ty: String,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub index: usize,
    pub score: u32,
}

pub struct Docs {
    pub version: String,
    pub entries: Vec<Entry>,
}

#[derive(Deserialize, Default)]
struct RawApi {
    #[serde(default)]
    version: String,
    #[serde(default)]
    functions: Vec<RawFn>,
    #[serde(default)]
    modules: Vec<RawModule>,
    #[serde(default)]
    callbacks: Vec<RawFn>,
    #[serde(default)]
    types: Vec<RawType>,
    #[serde(default)]
    enums: Vec<RawEnum>,
}
#[derive(Deserialize, Default)]
struct RawModule {
    name: String,
    #[serde(default)]
    functions: Vec<RawFn>,
    #[serde(default)]
    types: Vec<RawType>,
    #[serde(default)]
    enums: Vec<RawEnum>,
}
#[derive(Deserialize, Default)]
struct RawType {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    functions: Vec<RawFn>,
    since: Option<String>,
    deprecated: Option<String>,
}
#[derive(Deserialize, Default)]
struct RawEnum {
    name: String,
    #[serde(default)]
    description: String,
    since: Option<String>,
}
#[derive(Deserialize, Default)]
struct RawFn {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    variants: Vec<RawVariant>,
    since: Option<String>,
    deprecated: Option<String>,
}
#[derive(Deserialize, Default)]
struct RawVariant {
    #[serde(default)]
    arguments: Vec<RawArg>,
}
#[derive(Deserialize, Default)]
struct RawArg {
    name: String,
    #[serde(default, rename = "type")]
    ty: String,
    default: Option<String>,
}

fn to_entry(prefix: &str, sep: &str, f: RawFn, kind: EntryKind) -> Entry {
    Entry {
        name: format!("{prefix}{sep}{}", f.name),
        kind,
        description: f.description,
        signatures: f.variants.into_iter().map(|v| v.arguments.into_iter().map(|a| Arg { name: a.name, ty: a.ty, optional: a.default.is_some() }).collect()).collect(),
        since: f.since,
        deprecated: f.deprecated,
    }
}

impl Docs {
    pub fn from_json(json: &str) -> Result<Docs> {
        let raw: RawApi = serde_json::from_str(json).map_err(|e| Error::Msg(format!("love-api: {e}")))?;
        let mut entries = Vec::new();
        for f in raw.functions {
            entries.push(to_entry("love", ".", f, EntryKind::Function));
        }
        for f in raw.callbacks {
            entries.push(to_entry("love", ".", f, EntryKind::Callback));
        }
        let mut types: Vec<(String, RawType)> = raw.types.into_iter().map(|t| ("love".to_string(), t)).collect();
        let mut enums: Vec<(String, RawEnum)> = raw.enums.into_iter().map(|e| ("love".to_string(), e)).collect();
        for m in raw.modules {
            let p = format!("love.{}", m.name);
            for f in m.functions {
                entries.push(to_entry(&p, ".", f, EntryKind::Function));
            }
            types.extend(m.types.into_iter().map(|t| (p.clone(), t)));
            enums.extend(m.enums.into_iter().map(|e| (p.clone(), e)));
        }
        for (_, t) in types {
            entries.push(Entry { name: t.name.clone(), kind: EntryKind::Type, description: t.description, signatures: vec![], since: t.since, deprecated: t.deprecated });
            for f in t.functions {
                entries.push(to_entry(&t.name, ":", f, EntryKind::Method));
            }
        }
        for (p, e) in enums {
            entries.push(Entry { name: format!("{p}.{}", e.name), kind: EntryKind::Enum, description: e.description, signatures: vec![], since: e.since, deprecated: None });
        }
        Ok(Docs { version: raw.version, entries })
    }

    /// Fuzzy search over names, best first. Exact name, then prefix of the
    /// whole or last segment, then substring, then in-order letters.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<Hit> = self.entries.iter().enumerate().filter_map(|(i, e)| score(&e.name.to_lowercase(), &q).map(|s| Hit { index: i, score: s })).collect();
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(self.entries[a.index].name.len().cmp(&self.entries[b.index].name.len())).then(self.entries[a.index].name.cmp(&self.entries[b.index].name)));
        hits.truncate(limit);
        hits
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// A Monaco snippet for a function or callback, using its first variant:
    /// `love.graphics.draw(${1:drawable}, ${2:x}, ${3:y})`. Optional
    /// arguments are left out.
    pub fn snippet(&self, name: &str) -> Option<String> {
        let e = self.find(name)?;
        match e.kind {
            EntryKind::Callback => {
                let args: Vec<String> = e.signatures.first().map(|s| s.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
                Some(format!("function {}({})\n\t$0\nend", e.name, args.join(", ")))
            }
            EntryKind::Function | EntryKind::Method => {
                let req: Vec<&Arg> = e.signatures.first().map(|s| s.iter().filter(|a| !a.optional).collect()).unwrap_or_default();
                let list: Vec<String> = req.iter().enumerate().map(|(i, a)| format!("${{{}:{}}}", i + 1, a.name.replace(['\\', '}', '$'], ""))).collect();
                let sep = if e.kind == EntryKind::Method { ":" } else { "." };
                let call = match (e.kind, e.name.rsplit_once(sep)) {
                    (EntryKind::Method, Some((ty, m))) => format!("${{0:{}}}:{m}({})", lower_first(ty), list.join(", ")),
                    _ => format!("{}({})", e.name, list.join(", ")),
                };
                Some(call)
            }
            _ => None,
        }
    }

    /// Markdown for a hover: signature(s), description, version badges.
    pub fn hover(&self, name: &str) -> Option<String> {
        let e = self.find(name)?;
        let mut s = String::new();
        if let Some(v) = &e.since {
            s.push_str(&format!("`Added in {v}`  "));
        }
        if let Some(v) = &e.deprecated {
            s.push_str(&format!("`Deprecated in {v}`  "));
        }
        if !s.is_empty() {
            s.push_str("\n\n");
        }
        for sig in &e.signatures {
            let args: Vec<String> = sig.iter().map(|a| format!("{}{}: {}", a.name, if a.optional { "?" } else { "" }, a.ty)).collect();
            s.push_str(&format!("```lua\n{}({})\n```\n", e.name, args.join(", ")));
        }
        s.push_str(e.description.trim());
        Some(s)
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn score(name: &str, q: &str) -> Option<u32> {
    let last = name.rsplit(['.', ':']).next().unwrap_or(name);
    if name == q || last == q {
        return Some(1000);
    }
    if name.starts_with(q) {
        return Some(800);
    }
    if last.starts_with(q) {
        return Some(700);
    }
    if name.contains(q) {
        return Some(500);
    }
    // Letters in order, as in "gfxdraw" for love.graphics.draw: closer is better.
    let mut it = name.chars();
    let mut gaps = 0u32;
    for qc in q.chars().filter(|c| !c.is_whitespace()) {
        let mut skipped = 0;
        loop {
            match it.next() {
                Some(c) if c == qc => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(300u32.saturating_sub(gaps.min(250)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const API: &str = r#"{
      "version": "12.0",
      "functions": [{"name": "getVersion", "description": "Gets the version.", "variants": [{}]}],
      "callbacks": [{"name": "keypressed", "description": "A key was pressed.", "variants": [{"arguments": [
          {"name": "key", "type": "KeyConstant"}, {"name": "scancode", "type": "Scancode"}, {"name": "isrepeat", "type": "boolean"}]}]}],
      "modules": [
        {"name": "graphics", "description": "Drawing.",
         "functions": [
           {"name": "draw", "description": "Draws a drawable.", "variants": [{"arguments": [
              {"name": "drawable", "type": "Drawable"}, {"name": "x", "type": "number", "default": "0"}, {"name": "y", "type": "number", "default": "0"}]},
              {"arguments": [{"name": "drawable", "type": "Drawable"}, {"name": "quad", "type": "Quad"}]}]},
           {"name": "newTexture", "description": "Makes a texture.", "since": "12.0", "variants": [{"arguments": [{"name": "w", "type": "number"}, {"name": "h", "type": "number"}]}]},
           {"name": "setColor", "description": "Sets the colour.", "deprecated": "12.0", "variants": [{"arguments": [{"name": "r", "type": "number"}, {"name": "g", "type": "number"}, {"name": "b", "type": "number"}, {"name": "a", "type": "number", "default": "1"}]}]}
         ],
         "types": [{"name": "Image", "description": "An image.", "functions": [{"name": "getWidth", "description": "Width.", "variants": [{"arguments": []}]}]}],
         "enums": [{"name": "BlendMode", "description": "Blend modes."}]},
        {"name": "audio", "functions": [{"name": "newSource", "description": "A source.", "variants": [{"arguments": [{"name": "filename", "type": "string"}, {"name": "type", "type": "SourceType"}]}]}]}
      ]}"#;

    fn docs() -> Docs {
        Docs::from_json(API).unwrap()
    }

    fn names(d: &Docs, q: &str) -> Vec<String> {
        d.search(q, 10).into_iter().map(|h| d.entries[h.index].name.clone()).collect()
    }

    #[test]
    fn loads_every_kind_of_entry() {
        let d = docs();
        assert_eq!(d.version, "12.0");
        for n in ["love.getVersion", "love.keypressed", "love.graphics.draw", "Image", "Image:getWidth", "love.graphics.BlendMode", "love.audio.newSource"] {
            assert!(d.find(n).is_some(), "missing {n}");
        }
        assert_eq!(d.find("Image:getWidth").unwrap().kind, EntryKind::Method);
        assert!(Docs::from_json("not json").is_err());
    }

    #[test]
    fn search_ranks_exact_then_prefix_then_fuzzy() {
        let d = docs();
        assert_eq!(names(&d, "love.graphics.draw")[0], "love.graphics.draw");
        assert_eq!(names(&d, "draw")[0], "love.graphics.draw");
        assert_eq!(names(&d, "newt"), ["love.graphics.newTexture"]);
        assert!(names(&d, "grdrw").contains(&"love.graphics.draw".to_string()));
        assert!(names(&d, "zzzz").is_empty());
        assert!(names(&d, "  ").is_empty());
        assert_eq!(d.search("love", 3).len(), 3);
    }

    #[test]
    fn snippets_leave_out_optional_arguments() {
        let d = docs();
        assert_eq!(d.snippet("love.graphics.draw").unwrap(), "love.graphics.draw(${1:drawable})");
        assert_eq!(d.snippet("love.graphics.setColor").unwrap(), "love.graphics.setColor(${1:r}, ${2:g}, ${3:b})");
        assert_eq!(d.snippet("love.keypressed").unwrap(), "function love.keypressed(key, scancode, isrepeat)\n\t$0\nend");
        assert_eq!(d.snippet("Image:getWidth").unwrap(), "${0:image}:getWidth()");
        assert!(d.snippet("Image").is_none());
        assert!(d.snippet("nope").is_none());
    }

    #[test]
    fn hover_has_signatures_and_version_badges() {
        let d = docs();
        let h = d.hover("love.graphics.newTexture").unwrap();
        assert!(h.starts_with("`Added in 12.0`") && h.contains("newTexture(w: number, h: number)") && h.contains("Makes a texture."));
        assert!(d.hover("love.graphics.setColor").unwrap().contains("`Deprecated in 12.0`"));
        let draw = d.hover("love.graphics.draw").unwrap();
        assert!(draw.contains("x?: number") && draw.contains("quad: Quad") && !draw.contains("Added in"));
    }
}
