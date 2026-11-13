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

    /// Builds the reference from LuaCATS definition files (`love.lua` and
    /// `love/*.lua` of the language server's bundled LÖVE types), so the docs
    /// and the completions come from the same data and no second copy of
    /// love-api is shipped. `version` is read from a `-- version: X` header
    /// when `version` is empty.
    pub fn from_luacats(version: &str, sources: &[&str]) -> Docs {
        let mut entries = Vec::new();
        let mut version = version.to_string();
        for src in sources {
            parse_luacats(src, &mut entries, &mut version);
        }
        Docs { version, entries }
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

const CALLBACKS: &[&str] = &[
    "load", "update", "draw", "keypressed", "keyreleased", "textinput", "textedited", "mousepressed", "mousereleased", "mousemoved", "wheelmoved", "touchpressed", "touchreleased", "touchmoved", "joystickpressed",
    "joystickreleased", "joystickaxis", "joystickhat", "joystickadded", "joystickremoved", "gamepadpressed", "gamepadreleased", "gamepadaxis", "focus", "mousefocus", "visible", "resize", "quit", "errorhandler",
    "run", "threaderror", "filedropped", "directorydropped", "lowmemory", "displayrotated",
];

/// Splits on commas that are not inside brackets, so `fun(a: T, b: U)` and
/// `table<string, number>` stay whole.
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out.into_iter().map(str::trim).filter(|p| !p.is_empty()).collect()
}

/// `name: type` or `name?: type` (an overload's argument).
fn arg_from_pair(p: &str) -> Option<Arg> {
    let (n, t) = p.split_once(':')?;
    let n = n.trim();
    Some(Arg { name: n.trim_end_matches('?').to_string(), ty: t.trim().to_string(), optional: n.ends_with('?') })
}

/// The arguments of `fun(a: T, b?: U):R`.
fn overload_args(rest: &str) -> Option<Vec<Arg>> {
    let inner = rest.trim().strip_prefix("fun(")?;
    let mut depth = 1;
    let mut end = None;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    Some(split_top(&inner[..end?]).into_iter().filter_map(arg_from_pair).collect())
}

fn parse_luacats(src: &str, out: &mut Vec<Entry>, version: &mut String) {
    let mut desc: Vec<String> = Vec::new();
    let mut params: Vec<Arg> = Vec::new();
    let mut overloads: Vec<Vec<Arg>> = Vec::new();
    let mut class: Option<String> = None;
    let mut alias: Option<(String, Vec<String>)> = None;
    let flush_alias = |alias: &mut Option<(String, Vec<String>)>, desc: &mut Vec<String>, out: &mut Vec<Entry>| {
        if let Some((name, values)) = alias.take() {
            let mut d = clean_desc(desc);
            if !values.is_empty() {
                d.push_str("\n\nValues: ");
                d.push_str(&values.join(", "));
            }
            out.push(Entry { name, kind: EntryKind::Enum, description: d, signatures: vec![], since: None, deprecated: None });
        }
    };
    for raw in src.lines() {
        let line = raw.trim_end();
        if let Some(v) = line.strip_prefix("-- version:") {
            if version.is_empty() {
                *version = v.trim().to_string();
            }
            continue;
        }
        if let Some(c) = line.strip_prefix("---") {
            if let Some(v) = c.strip_prefix("| ") {
                // An alias value: `'"fill"' # Draw filled shape.`
                if let Some((_, vals)) = alias.as_mut() {
                    vals.push(v.trim().replace("'\"", "\"").replace("\"'", "\"").to_string());
                }
                continue;
            }
            if let Some(t) = c.strip_prefix("@param ") {
                let mut it = t.splitn(3, ' ');
                let (n, ty) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                params.push(Arg { name: n.trim_end_matches('?').to_string(), ty: ty.to_string(), optional: n.ends_with('?') });
            } else if let Some(t) = c.strip_prefix("@overload ") {
                if let Some(a) = overload_args(t) {
                    overloads.push(a);
                }
            } else if let Some(t) = c.strip_prefix("@class ") {
                class = Some(t.split([':', ' ']).next().unwrap_or("").to_string());
            } else if let Some(t) = c.strip_prefix("@alias ") {
                flush_alias(&mut alias, &mut desc, out);
                let name = t.split_whitespace().next().unwrap_or("").to_string();
                // A callback is an alias of a function type: `---@alias love.load fun(arg: table)`.
                if let Some(args) = t[name.len()..].trim().strip_prefix("fun(").and(overload_args(t[name.len()..].trim())) {
                    let cb = name.strip_prefix("love.").unwrap_or("");
                    let kind = if CALLBACKS.contains(&cb) { EntryKind::Callback } else { EntryKind::Function };
                    out.push(Entry { name, kind, description: clean_desc(&desc), signatures: vec![args], since: None, deprecated: None });
                    continue;
                }
                // Keep the description for the alias; the values follow as `---|` lines.
                alias = Some((name, vec![]));
                continue;
            } else if c.starts_with('@') {
                // @return, @meta, @field...: not shown.
            } else {
                desc.push(c.strip_prefix(' ').unwrap_or(c).to_string());
            }
            continue;
        }
        // A non-comment line ends the block.
        let t = line.trim();
        if let Some(f) = t.strip_prefix("function ") {
            let name = f.split('(').next().unwrap_or("").trim();
            let args_src = f.split_once('(').and_then(|(_, r)| r.rsplit_once(')')).map(|(a, _)| a).unwrap_or("");
            // Parameter names from the signature order the @param docs.
            let mut sig = params.clone();
            if sig.is_empty() {
                sig = split_top(args_src).into_iter().map(|n| Arg { name: n.to_string(), ty: "any".into(), optional: false }).collect();
            }
            let mut signatures = vec![sig];
            signatures.extend(overloads.drain(..));
            let (full, kind) = if let Some((ty, m)) = name.split_once(':') {
                (format!("{}:{m}", ty.strip_prefix("love.").unwrap_or(ty)), EntryKind::Method)
            } else if CALLBACKS.contains(&name.strip_prefix("love.").unwrap_or("")) && name.matches('.').count() == 1 {
                (name.to_string(), EntryKind::Callback)
            } else {
                (name.to_string(), EntryKind::Function)
            };
            out.push(Entry { name: full, kind, description: clean_desc(&desc), signatures, since: None, deprecated: None });
        } else if let Some(name) = class.take() {
            // `love.graphics = {}` after `@class love.graphics` is a module (not listed);
            // a class with methods is a type.
            let plain = name.strip_prefix("love.").unwrap_or(&name).to_string();
            let is_module = t.ends_with("= {}") && t.starts_with(&name);
            if !is_module && !plain.is_empty() {
                out.push(Entry { name: plain, kind: EntryKind::Type, description: clean_desc(&desc), signatures: vec![], since: None, deprecated: None });
            }
        } else if alias.is_some() && !t.is_empty() {
            flush_alias(&mut alias, &mut desc, out);
        }
        if t.is_empty() {
            // A blank line ends the alias (its `---|` values come right after `---@alias`) and every block.
            flush_alias(&mut alias, &mut desc, out);
        }
        desc.clear();
        params.clear();
        overloads.clear();
    }
    flush_alias(&mut alias, &mut desc, out);
}

/// The description lines without the "Open in Browser" link and stray blanks.
fn clean_desc(lines: &[String]) -> String {
    let kept: Vec<&str> = lines.iter().map(String::as_str).filter(|l| !l.trim_start().starts_with("[Open in Browser]")).collect();
    let mut s = kept.join("\n");
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim().to_string()
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

    const CATS: &str = r#"---@meta

-- version: 11.5
---
---Provides an interface to the user's clock.
---
---[Open in Browser](https://love2d.org/wiki/love.timer)
---
---@class love.timer
love.timer = {}

---
---Pauses the current thread for the specified amount of time.
---
---
---[Open in Browser](https://love2d.org/wiki/love.timer.sleep)
---
---@param s number # Seconds to sleep for.
function love.timer.sleep(s) end

---
---Draws a thing.
---
---@overload fun(mode: love.DrawMode, x: number, points?: table<string, number>)
---@param drawable love.Drawable # What to draw.
---@param x? number # X position.
function love.graphics.draw(drawable, x) end

---
---A picture.
---
---@class love.Image: love.Texture, love.Drawable, love.Object
local Image = {}

---
---Gets the width.
---
---@return number width # The width.
function Image:getWidth() end

---
---Called every frame.
---
---@param dt number # Seconds.
function love.update(dt) end

---
---How to draw.
---
---@alias love.DrawMode
---| '"fill"' # Filled.
---| '"line"' # Outline.

---
---Not a callback: love.getVersion.
---
function love.getVersion() end
"#;

    #[test]
    fn luacats_functions_types_callbacks_and_enums() {
        let d = Docs::from_luacats("", &[CATS]);
        assert_eq!(d.version, "11.5");
        let sleep = d.find("love.timer.sleep").unwrap();
        assert_eq!(sleep.kind, EntryKind::Function);
        assert_eq!(sleep.description, "Pauses the current thread for the specified amount of time.");
        assert_eq!(sleep.signatures, vec![vec![Arg { name: "s".into(), ty: "number".into(), optional: false }]]);
        let draw = d.find("love.graphics.draw").unwrap();
        assert_eq!(draw.signatures.len(), 2, "its own signature, then the overload");
        assert!(draw.signatures[0][1].optional && draw.signatures[0][1].name == "x");
        assert_eq!(draw.signatures[1].len(), 3);
        assert!(draw.signatures[1][2].optional && draw.signatures[1][2].ty == "table<string, number>", "{:?}", draw.signatures[1]);
        assert_eq!(d.find("Image").unwrap().kind, EntryKind::Type);
        assert_eq!(d.find("Image:getWidth").unwrap().kind, EntryKind::Method);
        assert_eq!(d.find("love.update").unwrap().kind, EntryKind::Callback);
        assert_eq!(d.find("love.getVersion").unwrap().kind, EntryKind::Function);
        assert!(d.find("love.timer").is_none(), "a module is not an entry");
        let e = d.find("love.DrawMode").unwrap();
        assert_eq!(e.kind, EntryKind::Enum);
        assert!(e.description.contains("How to draw.") && e.description.contains("\"fill\"") && e.description.contains("\"line\""), "{}", e.description);
        assert!(!d.entries.iter().any(|e| e.description.contains("Open in Browser")));
        // The same search, snippet and hover as the JSON source.
        assert_eq!(d.entries[d.search("sleep", 1)[0].index].name, "love.timer.sleep");
        assert_eq!(d.snippet("love.timer.sleep").unwrap(), "love.timer.sleep(${1:s})");
        assert_eq!(d.snippet("love.update").unwrap(), "function love.update(dt)\n\t$0\nend");
        assert!(d.hover("love.timer.sleep").unwrap().contains("```lua\nlove.timer.sleep(s: number)\n```"));
    }

    #[test]
    fn the_bundled_love_types_make_a_full_reference() {
        let tc = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/toolchains");
        let Some(dir) = crate::lsp::find_love_types(Some(&tc)) else { return };
        let mut texts = vec![std::fs::read_to_string(dir.join("love.lua")).unwrap()];
        for e in std::fs::read_dir(dir.join("love")).unwrap().flatten() {
            texts.push(std::fs::read_to_string(e.path()).unwrap());
        }
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let d = Docs::from_luacats("", &refs);
        assert_eq!(d.version, "11.5");
        for n in ["love.graphics.draw", "love.graphics.newImage", "love.keyboard.isDown", "love.load", "love.draw", "Image:isCompressed", "Texture:getWidth", "love.filesystem.read", "Image", "love.DrawMode"] {
            assert!(d.find(n).is_some(), "missing {n}");
        }
        assert!(d.entries.len() > 800, "only {} entries", d.entries.len());
        assert!(d.find("love.graphics.draw").unwrap().signatures.len() > 1);
        assert_eq!(d.find("love.load").unwrap().kind, EntryKind::Callback);
        assert!(d.entries.iter().all(|e| !e.name.is_empty() && !e.name.contains(' ')), "odd names: {:?}", d.entries.iter().find(|e| e.name.is_empty() || e.name.contains(' ')).map(|e| &e.name));
    }
}
