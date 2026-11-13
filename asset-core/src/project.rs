//! A project's view of the library: the `.joyride/assets.toml` it carries
//! (so a project shared in Git says what it uses and under what licences),
//! the `CREDITS.txt` its builds need, and the gate that stops an export with
//! an asset whose licence nobody has confirmed.

use crate::catalog::{Asset, Catalog};
use crate::licence::Tri;
use crate::{Error, Result};
use std::fmt::Write as _;
use std::path::Path;

/// Why an export should not go ahead, or should be looked at first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// Blocks: the licence is unknown, only guessed, or never confirmed.
    Unconfirmed { path: String },
    /// Blocks: the licence forbids commercial use and the project is commercial.
    NotCommercial { path: String, spdx: String },
    /// Blocks: the licence is share-alike, which the person must accept.
    ShareAlike { path: String, spdx: String },
    /// Blocks: rights lapse on or before the given date.
    Expired { path: String, expires: String },
}

impl Issue {
    pub fn path(&self) -> &str {
        match self {
            Issue::Unconfirmed { path } | Issue::NotCommercial { path, .. } | Issue::ShareAlike { path, .. } | Issue::Expired { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExportContext<'a> {
    /// Selling the game: non-commercial licences are issues.
    pub commercial: bool,
    /// Today as `YYYY-MM-DD`, for expiry.
    pub today: &'a str,
}

/// What stops a project's export. Empty means clear.
pub fn export_issues(cat: &Catalog, project: &str, ctx: ExportContext) -> Result<Vec<Issue>> {
    let mut issues = Vec::new();
    for (path, a) in cat.project_assets(project)? {
        if !a.is_usable() {
            issues.push(Issue::Unconfirmed { path });
            continue;
        }
        let l = a.licence.as_ref().expect("a confirmed asset has a licence");
        if ctx.commercial && l.commercial_use != Tri::Yes {
            issues.push(Issue::NotCommercial { path: path.clone(), spdx: l.spdx.clone() });
        }
        if l.share_alike {
            issues.push(Issue::ShareAlike { path: path.clone(), spdx: l.spdx.clone() });
        }
        let r = cat.rights(a.id)?;
        if !r.expires.is_empty() && r.expires.as_str() <= ctx.today {
            issues.push(Issue::Expired { path, expires: r.expires });
        }
    }
    Ok(issues)
}

/// The text of `CREDITS.txt`: every asset whose licence asks for attribution,
/// one entry each (an asset used at several paths is listed once).
pub fn credits(cat: &Catalog, project: &str) -> Result<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut entries: Vec<(String, String)> = Vec::new();
    for (_, a) in cat.project_assets(project)? {
        let Some(l) = &a.licence else { continue };
        if !l.attribution_required || !a.is_usable() || !seen.insert(a.id) {
            continue;
        }
        entries.push((a.name.to_lowercase(), credit_line(&a)));
    }
    entries.sort();
    let mut s = format!("Credits for {project}\n\n");
    if entries.is_empty() {
        s.push_str("No asset in this project asks for credit.\n");
    }
    for (_, line) in entries {
        s.push_str(&line);
        s.push('\n');
    }
    Ok(s)
}

fn credit_line(a: &Asset) -> String {
    let l = a.licence.as_ref().unwrap();
    if !a.attribution_text.trim().is_empty() {
        return format!("- {} ({})", a.attribution_text.trim(), l.spdx);
    }
    let mut s = format!("- {}", a.name);
    if !a.author.is_empty() {
        let _ = write!(s, " by {}", a.author);
    }
    let _ = write!(s, " ({})", l.spdx);
    if !a.source_url.is_empty() {
        let _ = write!(s, ", {}", a.source_url);
    }
    s
}

/// The contents of `.joyride/assets.toml`: what the project uses, by path,
/// with the content hash and licence, so the file can live in Git and a
/// checkout can tell when a library asset has changed.
pub fn assets_toml(cat: &Catalog, project: &str) -> Result<String> {
    let mut s = String::from("# Written by JoyrIDE: the library assets this project uses and their licences.\n");
    for (path, a) in cat.project_assets(project)? {
        let l = a.licence.as_ref();
        let _ = write!(s, "\n[[asset]]\npath = {}\nhash = {}\nlicence = {}\nconfirmed = {}\n", q(&path), q(&a.hash), q(l.map(|l| l.spdx.as_str()).unwrap_or("unknown")), a.is_usable());
        if !a.author.is_empty() {
            let _ = writeln!(s, "author = {}", q(&a.author));
        }
        if !a.source_url.is_empty() {
            let _ = writeln!(s, "source = {}", q(&a.source_url));
        }
    }
    Ok(s)
}

fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Writes `.joyride/assets.toml` into a project.
pub fn write_assets_toml(cat: &Catalog, project: &str, root: &Path) -> Result<()> {
    let dir = root.join(".joyride");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("assets.toml"), assets_toml(cat, project)?).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::NewAsset;
    use crate::licence::Licence;
    use crate::Kind;

    fn add(c: &mut Catalog, hash: &str, name: &str, spdx: Option<(&str, bool)>) -> i64 {
        let (id, _) = c.add_asset(&NewAsset { hash: hash.into(), size: 1, kind: Kind::Image, name: name.into(), ext: "png".into(), stored: "x".into() }).unwrap();
        match spdx {
            Some((s, true)) => c.confirm_licence(id, &Licence::from_spdx(s), "me", "").unwrap(),
            Some((s, false)) => c.set_licence(id, &Licence::from_spdx(s), true).unwrap(),
            None => {}
        }
        c.record_use(id, "game", &format!("img/{name}")).unwrap();
        id
    }

    fn library() -> Catalog {
        let mut c = Catalog::in_memory().unwrap();
        let a = add(&mut c, "h1", "hero.png", Some(("CC-BY-4.0", true)));
        c.set_credit(a, "Ada", "https://ada.example/hero", "").unwrap();
        let b = add(&mut c, "h2", "tile.png", Some(("CC0-1.0", true)));
        let _ = b;
        add(&mut c, "h3", "mystery.png", None);
        add(&mut c, "h4", "guess.png", Some(("MIT", false)));
        let n = add(&mut c, "h5", "nc.png", Some(("CC-BY-NC-4.0", true)));
        c.set_credit(n, "Bo", "", "Music box art by Bo").unwrap();
        let s = add(&mut c, "h6", "sa.png", Some(("CC-BY-SA-4.0", true)));
        let mut r = c.rights(s).unwrap();
        r.expires = "2026-12-01".into();
        c.set_rights(s, &r).unwrap();
        // Same asset used at two paths.
        c.record_use(a, "game", "ui/hero2.png").unwrap();
        c
    }

    #[test]
    fn the_gate_names_every_problem() {
        let c = library();
        let issues = export_issues(&c, "game", ExportContext { commercial: true, today: "2026-11-13" }).unwrap();
        let has = |f: fn(&Issue) -> bool, p: &str| issues.iter().any(|i| f(i) && i.path() == p);
        assert!(has(|i| matches!(i, Issue::Unconfirmed { .. }), "img/mystery.png"));
        assert!(has(|i| matches!(i, Issue::Unconfirmed { .. }), "img/guess.png"), "a guess is not confirmed");
        assert!(has(|i| matches!(i, Issue::NotCommercial { .. }), "img/nc.png"));
        assert!(has(|i| matches!(i, Issue::ShareAlike { .. }), "img/sa.png"));
        assert!(!issues.iter().any(|i| i.path() == "img/tile.png" || i.path() == "img/hero.png"));
        // Later, the share-alike asset's rights have lapsed.
        let later = export_issues(&c, "game", ExportContext { commercial: true, today: "2026-12-02" }).unwrap();
        assert!(later.iter().any(|i| matches!(i, Issue::Expired { expires, .. } if expires == "2026-12-01")));
        // A free, non-commercial project is fine with the NC asset.
        let free = export_issues(&c, "game", ExportContext { commercial: false, today: "2026-11-13" }).unwrap();
        assert!(!free.iter().any(|i| matches!(i, Issue::NotCommercial { .. })));
    }

    #[test]
    fn credits_list_each_asset_once_in_order() {
        let c = library();
        let t = credits(&c, "game").unwrap();
        assert!(t.starts_with("Credits for game\n"));
        assert!(t.contains("- hero.png by Ada (CC-BY-4.0), https://ada.example/hero"));
        assert_eq!(t.matches("hero.png").count(), 1, "{t}");
        assert!(t.contains("- Music box art by Bo (CC-BY-NC-4.0)"));
        assert!(!t.contains("tile.png"), "CC0 asks for no credit");
        assert!(!t.contains("mystery") && !t.contains("guess.png"), "unconfirmed assets are not credited");
        assert!(credits(&c, "empty").unwrap().contains("No asset in this project asks for credit."));
    }

    #[test]
    fn assets_toml_is_valid_toml_with_each_use() {
        let c = library();
        let text = assets_toml(&c, "game").unwrap();
        let v: toml_check::Value = toml_check::parse(&text);
        assert_eq!(v.count, 7, "{text}");
        assert!(text.contains("path = \"img/hero.png\"") && text.contains("path = \"ui/hero2.png\""));
        assert!(text.contains("confirmed = false"));
        let d = tempfile::tempdir().unwrap();
        write_assets_toml(&c, "game", d.path()).unwrap();
        assert!(d.path().join(".joyride/assets.toml").is_file());
    }

    /// A tiny structural check, so the test needs no TOML parser: every
    /// `[[asset]]` table has a path, a hash and a licence line.
    mod toml_check {
        pub struct Value {
            pub count: usize,
        }
        pub fn parse(t: &str) -> Value {
            let tables: Vec<&str> = t.split("[[asset]]").skip(1).collect();
            for b in &tables {
                for key in ["path = \"", "hash = \"", "licence = \"", "confirmed = "] {
                    assert!(b.contains(key), "table lacks {key}: {b}");
                }
            }
            Value { count: tables.len() }
        }
    }
}
