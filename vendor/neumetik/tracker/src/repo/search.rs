//! Finding samples in the database by what they're called, how they're
//! filed and what they're like.
//!
//! A search is words, each of which must match (in a name, a file name,
//! a tag, a category, the notes, or a module it came from), and terms:
//!
//! | Term | Matches |
//! | --- | --- |
//! | `tag:punchy` | tagged punchy |
//! | `cat:bass` | in a category starting "bass" |
//! | `fmt:xm` | found in an XM module |
//! | `from:axel` | found in a module whose file or title has "axel" |
//! | `bits:8`, `stereo:yes` | 8-bit; two channels |
//! | `loop:yes`, `loop:no`, `loop:pingpong` | looped or not |
//! | `len>2`, `len<0.1` | longer than 2 seconds; shorter than a tenth |
//! | `rating:3` | rated 3 or more |
//! | `note:A-5` | pitched nearest A-5 |
//! | `used:2` | found in 2 modules or more |
//! | `id:3a9e` | its ID starts so |
//! | `free:yes` | free to use (marked so, or its license says: `rights.rs`) |
//! | `license:cc` | its license (or a module's it came from) has "cc" |
//! | `genre:techno`, `era:1990s`, `platform:amiga`, `party:assembly95`, `inst:piano`, `use:game`, `length:short`, `year:1994`, `game:` | found in a module so tagged (`tagging.rs`: `genre:jungle` matches `genre:electronic/drum-and-bass/jungle`) |
//!
//! `license:free`, `license:restricted` and `license:free-with-credit`
//! are a module's tag; any other `license:` is the words of a license.
//!
//! `-` before a word or term turns it round; "quotes" keep words together.

use super::analysis::note_of;
use super::db::{Db, PatchRec, SampleRec};
use crate::model::LoopKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Name,
    Newest,
    Longest,
    Shortest,
    MostUsed,
    Category,
    Rating,
}

impl Sort {
    pub fn from_name(s: &str) -> Sort {
        match s {
            "newest" => Sort::Newest,
            "longest" => Sort::Longest,
            "shortest" => Sort::Shortest,
            "most-used" => Sort::MostUsed,
            "category" => Sort::Category,
            "rating" => Sort::Rating,
            _ => Sort::Name,
        }
    }
}

/// A search's words and terms, each maybe turned round.
fn terms(q: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut neg = false;
    let push = |cur: &mut String, neg: &mut bool, out: &mut Vec<(bool, String)>| {
        if !cur.is_empty() {
            out.push((*neg, cur.to_lowercase()));
        }
        cur.clear();
        *neg = false;
    };
    for c in q.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                if !quoted {
                    push(&mut cur, &mut neg, &mut out);
                }
            }
            c if c.is_whitespace() && !quoted => push(&mut cur, &mut neg, &mut out),
            '-' if cur.is_empty() && !quoted && !neg => neg = true,
            c => cur.push(c),
        }
    }
    push(&mut cur, &mut neg, &mut out);
    out
}

/// A sample's length in seconds at the first rate it was tuned to.
pub fn seconds(db: &Db, s: &SampleRec) -> f32 {
    let frames = db.waves.get(&s.wave).map_or(0, |w| w.frames);
    frames as f32 / s.rates.first().copied().unwrap_or(8363).max(1) as f32
}

/// Its pitch in hertz at its first rate, when it has one.
pub fn pitch(db: &Db, s: &SampleRec) -> Option<f32> {
    let w = db.waves.get(&s.wave)?;
    (w.period > 0.0).then(|| s.rates.first().copied().unwrap_or(8363) as f32 / w.period)
}

fn matches(db: &Db, s: &SampleRec, term: &str) -> bool {
    let yes = |v: &str| matches!(v, "yes" | "y" | "1" | "true" | "on");
    let num = |v: &str| v.trim().parse::<f32>().ok();
    if let Some(v) = term.strip_prefix("len>") {
        return num(v).is_some_and(|n| seconds(db, s) > n);
    }
    if let Some(v) = term.strip_prefix("len<") {
        return num(v).is_some_and(|n| seconds(db, s) < n);
    }
    if let Some((k, v)) = term.split_once(':') {
        let has = |list: &[String]| list.iter().any(|t| t.to_lowercase() == v);
        let w = db.waves.get(&s.wave);
        match k {
            "tag" => return has(&s.catalog.tags),
            "cat" | "category" => return s.categories().iter().any(|c| c.to_lowercase().starts_with(v)),
            "fmt" | "format" => return db.sources_of(s).iter().any(|(m, _)| m.format == v),
            "from" => return db.sources_of(s).iter().any(|(m, _)| m.files.iter().any(|f| f.to_lowercase().contains(v)) || m.title.to_lowercase().contains(v)),
            "bits" => return w.is_some_and(|w| w.bits.to_string() == v),
            "stereo" => return w.is_some_and(|w| (w.channels == 2) == yes(v)),
            "loop" => {
                return match v {
                    "pingpong" | "ping-pong" => s.looped.kind == LoopKind::PingPong,
                    v => s.looped.on() == yes(v),
                }
            }
            "rating" => return num(v).is_some_and(|n| f32::from(s.catalog.rating) >= n),
            "used" => return num(v).is_some_and(|n| s.sources.len() as f32 >= n),
            "id" => return s.id.to_string().starts_with(v),
            "note" => return pitch(db, s).and_then(note_of).is_some_and(|(n, _)| n.to_lowercase() == v),
            "free" => return super::rights::freedom(db, s).free == yes(v),
            "license" | "licence" if !matches!(v, "free" | "free-with-credit" | "restricted") => return super::rights::freedom(db, s).license.to_lowercase().contains(v),
            k if super::tagging::NAMESPACES.contains(&(if k == "licence" { "license" } else { k })) => {
                let k = if k == "licence" { "license" } else { k };
                return db.sources_of(s).iter().any(|(m, _)| super::tagging::term_matches(&m.guessed, k, v));
            }
            _ => {}
        }
    }
    let t = term;
    s.names.iter().any(|n| n.to_lowercase().contains(t))
        || s.filenames.iter().any(|n| n.to_lowercase().contains(t))
        || s.catalog.tags.iter().any(|n| n.to_lowercase().contains(t))
        || s.categories().iter().any(|n| n.to_lowercase().contains(t))
        || s.catalog.notes.to_lowercase().contains(t)
        || db.sources_of(s).iter().any(|(m, _)| m.title.to_lowercase().contains(t) || m.files.iter().any(|f| f.to_lowercase().contains(t)))
}

/// The samples a search finds, sorted.
pub fn search<'a>(db: &'a Db, query: &str, sort: Sort) -> Vec<&'a SampleRec> {
    let ts = terms(query);
    let mut hits: Vec<&SampleRec> = db.samples.values().filter(|s| ts.iter().all(|(neg, t)| matches(db, s, t) != *neg)).collect();
    let name = |s: &SampleRec| s.name().to_lowercase();
    match sort {
        Sort::Name => hits.sort_by_cached_key(|s| (name(s).is_empty(), name(s))),
        Sort::Newest => hits.sort_by(|a, b| b.catalog.added.cmp(&a.catalog.added).then(name(a).cmp(&name(b)))),
        Sort::Longest => hits.sort_by(|a, b| seconds(db, b).total_cmp(&seconds(db, a))),
        Sort::Shortest => hits.sort_by(|a, b| seconds(db, a).total_cmp(&seconds(db, b))),
        Sort::MostUsed => hits.sort_by(|a, b| b.sources.len().cmp(&a.sources.len()).then(name(a).cmp(&name(b)))),
        Sort::Category => hits.sort_by_cached_key(|s| (s.categories().first().map(|c| c.to_string()).unwrap_or_default(), name(s))),
        Sort::Rating => hits.sort_by(|a, b| b.catalog.rating.cmp(&a.catalog.rating).then(name(a).cmp(&name(b)))),
    }
    hits
}

/// Whether a synth patch matches a word or term: words its names, kind,
/// format, tags, categories, notes and the modules it came from; terms
/// `kind:` (or `fmt:`), `playable:yes`, `tag:`, `cat:`, `rating:`,
/// `used:`, `id:`, `from:`, and the modules' tags as for samples.
fn patch_matches(db: &Db, p: &PatchRec, term: &str) -> bool {
    let yes = |v: &str| matches!(v, "yes" | "y" | "1" | "true" | "on");
    let num = |v: &str| v.trim().parse::<f32>().ok();
    let sources = || p.sources.iter().filter_map(|s| db.modules.get(&s.module));
    if let Some((k, v)) = term.split_once(':') {
        let has = |list: &[String]| list.iter().any(|t| t.to_lowercase() == v);
        match k {
            "kind" | "fmt" | "format" => return p.kind == v || p.format_name.to_lowercase().contains(v),
            "playable" => return super::patch::playable(&p.kind) == yes(v),
            "tag" => return has(&p.catalog.tags),
            "cat" | "category" => return p.catalog.categories.iter().any(|c| c.to_lowercase().starts_with(v)),
            "rating" => return num(v).is_some_and(|n| f32::from(p.catalog.rating) >= n),
            "used" => return num(v).is_some_and(|n| p.sources.len() as f32 >= n),
            "id" => return p.id.to_string().starts_with(v),
            "from" => return sources().any(|m| m.files.iter().any(|f| f.to_lowercase().contains(v)) || m.title.to_lowercase().contains(v)),
            k if super::tagging::NAMESPACES.contains(&(if k == "licence" { "license" } else { k })) => {
                let k = if k == "licence" { "license" } else { k };
                return sources().any(|m| super::tagging::term_matches(&m.guessed, k, v));
            }
            _ => {}
        }
    }
    let t = term;
    p.names.iter().any(|n| n.to_lowercase().contains(t))
        || p.kind == t
        || p.format_name.to_lowercase().contains(t)
        || p.catalog.tags.iter().any(|n| n.to_lowercase().contains(t))
        || p.catalog.categories.iter().any(|n| n.to_lowercase().contains(t))
        || p.catalog.notes.to_lowercase().contains(t)
        || sources().any(|m| m.title.to_lowercase().contains(t) || m.artist.to_lowercase().contains(t) || m.files.iter().any(|f| f.to_lowercase().contains(t)))
}

/// The synth patches a search finds, sorted (by name, newest, most used,
/// rating, or `Category`: by kind).
pub fn search_patches<'a>(db: &'a Db, query: &str, sort: Sort) -> Vec<&'a PatchRec> {
    let ts = terms(query);
    let mut hits: Vec<&PatchRec> = db.patches.values().filter(|p| ts.iter().all(|(neg, t)| patch_matches(db, p, t) != *neg)).collect();
    let name = |p: &PatchRec| p.name().to_lowercase();
    match sort {
        Sort::Newest => hits.sort_by(|a, b| b.catalog.added.cmp(&a.catalog.added).then(name(a).cmp(&name(b)))),
        Sort::MostUsed => hits.sort_by(|a, b| b.sources.len().cmp(&a.sources.len()).then(name(a).cmp(&name(b)))),
        Sort::Rating => hits.sort_by(|a, b| b.catalog.rating.cmp(&a.catalog.rating).then(name(a).cmp(&name(b)))),
        Sort::Category => hits.sort_by_cached_key(|p| (p.format_name.clone(), name(p).is_empty(), name(p))),
        _ => hits.sort_by_cached_key(|p| (name(p).is_empty(), name(p), p.format_name.clone())),
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_quotes_and_negation() {
        assert_eq!(terms(r#"kick -tag:old "hi hat" len>2"#), vec![(false, "kick".into()), (true, "tag:old".into()), (false, "hi hat".into()), (false, "len>2".into())]);
    }
}
