//! Adding one file to a project from outside it (a font someone downloaded,
//! say): the licence is confirmed by a named person first, the file is copied
//! in without ever replacing anything, the library keeps a content-addressed
//! copy, the use is recorded and `.joyride/assets.toml` is rewritten.
//! `docs/asset-library.md`, rule 2: nothing is added with an unknown licence
//! unless the person overrides and says why.

use crate::catalog::{Catalog, NewAsset};
use crate::licence::{Licence, Status, Tri};
use crate::{dedup, kind, project, Error, Result};
use std::path::{Component, Path, PathBuf};

pub struct AddRequest<'a> {
    pub source: &'a Path,
    /// Where it goes, relative to the project (`fonts/Hack-Regular.ttf`).
    pub dest: &'a str,
    pub licence: Licence,
    /// Who confirms the licence. Required.
    pub confirmed_by: &'a str,
    /// A note; **required when the licence is unknown** (why it is used anyway).
    pub note: &'a str,
    pub author: &'a str,
    pub source_url: &'a str,
    pub attribution_text: &'a str,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct Added {
    pub id: i64,
    /// The project-relative path written.
    pub dest: String,
    /// The library already held this content; only the use was added.
    pub already_in_library: bool,
}

fn knows_nothing(l: &Licence) -> bool {
    l.commercial_use == Tri::Unknown && l.modification == Tri::Unknown && l.redistribution == Tri::Unknown
}

/// A project-relative path with only plain components.
fn plain_rel(rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    let ok = !rel.trim().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(Error::Other(format!("`{rel}` is not a path inside the project")));
    }
    Ok(p.to_path_buf())
}

pub fn add_to_project(cat: &mut Catalog, library_dir: &Path, project_name: &str, root: &Path, r: &AddRequest) -> Result<Added> {
    if r.confirmed_by.trim().is_empty() {
        return Err(Error::Other("say who confirms the licence".into()));
    }
    if knows_nothing(&r.licence) && r.note.trim().is_empty() {
        return Err(Error::Other("the licence is unknown: pick one, or say why the file is used anyway".into()));
    }
    let root = root.canonicalize()?;
    let rel = plain_rel(r.dest)?;
    let dest = root.join(&rel);
    // Nothing is replaced, and nothing leaves the project through a link.
    if dest.symlink_metadata().is_ok() {
        return Err(Error::Other(format!("`{}` already exists; choose another name", r.dest)));
    }
    let mut near = dest.parent().unwrap_or(&root);
    while !near.exists() {
        near = near.parent().ok_or_else(|| Error::Other("no such folder".into()))?;
    }
    if !near.canonicalize()?.starts_with(&root) {
        return Err(Error::Other(format!("`{}` leaves the project", r.dest)));
    }
    let meta = std::fs::metadata(r.source)?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(Error::Other(format!("{} is not a file with content", r.source.display())));
    }

    let hash = dedup::hash_file(r.source)?;
    let name = r.source.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let ext = r.source.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let kind = kind::classify(r.source);

    let existing = cat.find_by_hash(&hash)?;
    if let Some(id) = existing {
        let held = cat.get(id)?.ok_or_else(|| Error::Other("asset vanished".into()))?;
        if let (Status::Confirmed { .. }, Some(l)) = (&held.status, &held.licence) {
            if l.spdx != r.licence.spdx {
                return Err(Error::Other(format!("the library holds this file under {}, not {}", l.spdx, r.licence.spdx)));
            }
        }
    }

    // Copy into the project first: if it fails, the catalog is untouched.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("partial");
    std::fs::copy(r.source, &tmp)?;
    if dedup::hash_file(&tmp)? != hash {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Other(format!("{} changed while it was copied", r.source.display())));
    }
    std::fs::rename(&tmp, &dest)?;

    let id = match existing {
        Some(id) => {
            if !cat.get(id)?.is_some_and(|a| a.is_usable()) {
                cat.confirm_licence(id, &r.licence, r.confirmed_by, r.note)?;
            }
            id
        }
        None => {
            let stored = format!("{}/{}{}", &hash[..2], hash, if ext.is_empty() { String::new() } else { format!(".{ext}") });
            let lib = library_dir.join(&stored);
            if !lib.exists() {
                std::fs::create_dir_all(lib.parent().unwrap())?;
                let part = lib.with_extension("partial");
                std::fs::copy(&dest, &part)?;
                std::fs::rename(&part, &lib)?;
            }
            let (id, _) = cat.add_asset(&NewAsset { hash: hash.clone(), size: meta.len(), kind, name, ext, stored })?;
            cat.confirm_licence(id, &r.licence, r.confirmed_by, r.note)?;
            id
        }
    };
    if !(r.author.is_empty() && r.source_url.is_empty() && r.attribution_text.is_empty()) {
        cat.set_credit(id, r.author, r.source_url, r.attribution_text)?;
    }
    for t in &r.tags {
        cat.add_tag(id, t)?;
    }
    let rel_s = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
    cat.record_use(id, project_name, &rel_s)?;
    cat.record_origin(id, "file", &r.source.display().to_string())?;
    project::write_assets_toml(cat, project_name, &root)?;
    Ok(Added { id, dest: rel_s, already_in_library: existing.is_some() })
}

/// Puts an asset the library already holds into a project: the licence must
/// already be confirmed (an unconfirmed asset is refused), the bytes come from
/// the library's copy and are checked against the recorded hash, nothing is
/// replaced and nothing leaves the project. Records the use and rewrites
/// `.joyride/assets.toml`. Returns the project-relative path written.
pub fn use_in_project(cat: &mut Catalog, library_dir: &Path, project_name: &str, root: &Path, id: i64, dest: &str) -> Result<String> {
    let asset = cat.get(id)?.ok_or_else(|| Error::Other("no such asset in the library".into()))?;
    if !asset.is_usable() {
        return Err(Error::Other(format!("{}: confirm its licence before using it", asset.name)));
    }
    let root = root.canonicalize()?;
    let rel = plain_rel(dest)?;
    let target = root.join(&rel);
    if target.symlink_metadata().is_ok() {
        return Err(Error::Other(format!("`{dest}` already exists; choose another name")));
    }
    let mut near = target.parent().unwrap_or(&root);
    while !near.exists() {
        near = near.parent().ok_or_else(|| Error::Other("no such folder".into()))?;
    }
    if !near.canonicalize()?.starts_with(&root) {
        return Err(Error::Other(format!("`{dest}` leaves the project")));
    }
    let src = library_dir.join(&asset.stored);
    if !src.is_file() {
        return Err(Error::Other(format!("the library's copy of {} is missing", asset.name)));
    }
    if dedup::hash_file(&src)? != asset.hash {
        return Err(Error::Other(format!("the library's copy of {} no longer matches its record", asset.name)));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = target.with_extension("partial");
    std::fs::copy(&src, &tmp)?;
    if dedup::hash_file(&tmp)? != asset.hash {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Other("the copy did not match; nothing was added".into()));
    }
    std::fs::rename(&tmp, &target)?;
    let rel_s = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
    cat.record_use(id, project_name, &rel_s)?;
    project::write_assets_toml(cat, project_name, &root)?;
    Ok(rel_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Env {
        _d: tempfile::TempDir,
        cat: Catalog,
        lib: PathBuf,
        root: PathBuf,
        src: PathBuf,
    }

    fn env() -> Env {
        let d = tempfile::tempdir().unwrap();
        let (lib, root) = (d.path().join("lib"), d.path().join("game"));
        std::fs::create_dir_all(&root).unwrap();
        let src = d.path().join("Hack-Regular.ttf");
        std::fs::write(&src, b"pretend font bytes").unwrap();
        Env { cat: Catalog::in_memory().unwrap(), lib, root, src, _d: d }
    }

    fn go(e: &mut Env, dest: &str, l: Licence, by: &str, note: &str) -> Result<Added> {
        let r = AddRequest { source: &e.src, dest, licence: l, confirmed_by: by, note, author: "Source Foundry", source_url: "https://example.org/hack", attribution_text: "", tags: vec!["font:truetype".into()] };
        add_to_project(&mut e.cat, &e.lib, "game", &e.root, &r)
    }

    fn mit() -> Licence {
        Licence::from_spdx("MIT")
    }

    #[test]
    fn adds_a_font_with_a_confirmed_licence() {
        let mut e = env();
        let a = go(&mut e, "fonts/Hack-Regular.ttf", mit(), "Joash", "").unwrap();
        assert_eq!(a.dest, "fonts/Hack-Regular.ttf");
        assert_eq!(std::fs::read(e.root.join("fonts/Hack-Regular.ttf")).unwrap(), b"pretend font bytes");
        assert!(!e.root.join("fonts/Hack-Regular.partial").exists());
        let asset = e.cat.get(a.id).unwrap().unwrap();
        assert!(asset.is_usable() && asset.kind == crate::Kind::Font && asset.tags.contains(&"font:truetype".to_string()));
        assert_eq!(e.cat.uses(a.id).unwrap(), vec![("game".to_string(), "fonts/Hack-Regular.ttf".to_string())]);
        // The library keeps its own copy, and the project lists the use.
        assert!(e.lib.join(&asset.stored).is_file());
        let toml = std::fs::read_to_string(e.root.join(".joyride/assets.toml")).unwrap();
        assert!(toml.contains("fonts/Hack-Regular.ttf") && toml.contains("MIT") && toml.contains("confirmed = true"));
    }

    #[test]
    fn needs_a_named_confirmer_and_never_an_unknown_licence_without_a_reason() {
        let mut e = env();
        assert!(go(&mut e, "f.ttf", mit(), "  ", "").is_err());
        let unknown = Licence::unknown("unknown");
        assert!(go(&mut e, "f.ttf", unknown.clone(), "Joash", "").is_err());
        assert!(!e.root.join("f.ttf").exists(), "nothing is copied when refused");
        let ok = go(&mut e, "f.ttf", unknown, "Joash", "Client supplied it; licence asked for").unwrap();
        assert!(e.cat.get(ok.id).unwrap().unwrap().is_usable());
    }

    #[test]
    fn never_replaces_and_never_leaves_the_project() {
        let mut e = env();
        std::fs::create_dir_all(e.root.join("fonts")).unwrap();
        std::fs::write(e.root.join("fonts/a.ttf"), "mine").unwrap();
        assert!(go(&mut e, "fonts/a.ttf", mit(), "J", "").is_err());
        assert_eq!(std::fs::read_to_string(e.root.join("fonts/a.ttf")).unwrap(), "mine");
        for bad in ["../x.ttf", "/tmp/x.ttf", "", "a/../../x.ttf"] {
            assert!(go(&mut e, bad, mit(), "J", "").is_err(), "{bad}");
        }
        #[cfg(unix)]
        {
            let outside = e.root.parent().unwrap().join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, e.root.join("link")).unwrap();
            assert!(go(&mut e, "link/x.ttf", mit(), "J", "").is_err());
            assert!(!outside.join("x.ttf").exists());
        }
    }

    #[test]
    fn the_same_font_under_another_licence_is_a_conflict_and_the_same_one_is_reused() {
        let mut e = env();
        let a = go(&mut e, "fonts/a.ttf", mit(), "J", "").unwrap();
        let b = go(&mut e, "fonts/b.ttf", mit(), "J", "").unwrap();
        assert!(b.already_in_library && b.id == a.id);
        assert_eq!(e.cat.uses(a.id).unwrap().len(), 2);
        assert!(go(&mut e, "fonts/c.ttf", Licence::from_spdx("OFL-1.1"), "J", "").is_err());
        assert!(!e.root.join("fonts/c.ttf").exists());
    }

    #[test]
    fn uses_a_confirmed_library_asset_and_refuses_an_unconfirmed_one() {
        let mut e = env();
        let a = go(&mut e, "fonts/a.ttf", mit(), "J", "").unwrap();
        let w = use_in_project(&mut e.cat, &e.lib, "other", &e.root, a.id, "more/b.ttf").unwrap();
        assert_eq!(w, "more/b.ttf");
        assert_eq!(std::fs::read(e.root.join("more/b.ttf")).unwrap(), b"pretend font bytes");
        assert_eq!(e.cat.uses(a.id).unwrap().len(), 2);
        // Never replaces, never leaves the project.
        assert!(use_in_project(&mut e.cat, &e.lib, "other", &e.root, a.id, "more/b.ttf").is_err());
        assert!(use_in_project(&mut e.cat, &e.lib, "other", &e.root, a.id, "../x.ttf").is_err());
        // A damaged library copy is refused.
        let stored = e.lib.join(e.cat.get(a.id).unwrap().unwrap().stored);
        std::fs::write(&stored, b"tampered").unwrap();
        assert!(use_in_project(&mut e.cat, &e.lib, "other", &e.root, a.id, "more/c.ttf").is_err());
        assert!(!e.root.join("more/c.ttf").exists());
        // An unconfirmed asset is refused.
        let (id, _) = e.cat.add_asset(&NewAsset { hash: "ab".repeat(32), size: 3, kind: crate::Kind::Image, name: "x.png".into(), ext: "png".into(), stored: "ab/x.png".into() }).unwrap();
        assert!(use_in_project(&mut e.cat, &e.lib, "other", &e.root, id, "x.png").unwrap_err().to_string().contains("confirm its licence"));
    }
}
