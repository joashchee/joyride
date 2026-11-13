//! Bringing a project's assets into the library.
//!
//! Two steps, so nothing is copied before a person has seen the licences:
//! [`plan`] only reads (it lists the project's media, groups identical
//! content, notes any licence text found beside each file, and says which
//! content the library already holds); [`apply`] takes the person's
//! decisions and imports only what they confirmed. Originals are never
//! changed or removed. Identical content is stored once; every path that
//! held it is recorded as a use.

use crate::audio::{self, AudioInfo};
use crate::catalog::{Catalog, NewAsset};
use crate::kind::{self, Kind};
use crate::licence::{self, Guess, Licence, Status};
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Folders that are never a project's own assets.
const SKIP_DIRS: [&str; 6] = [".git", ".svn", ".joyride", "build", "node_modules", "target"];

#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Also list Lua source (libraries and modules), not just media.
    pub include_code: bool,
}

/// One distinct piece of content found in the project.
#[derive(Debug, Clone)]
pub struct PlannedAsset {
    pub hash: String,
    pub size: u64,
    pub kind: Kind,
    /// The first path's file name.
    pub name: String,
    pub ext: String,
    /// Every project-relative path (with `/`) that holds this content.
    pub paths: Vec<String>,
    /// Licences guessed from text beside the files; for the person to check.
    pub guesses: Vec<Guess>,
    /// The library asset that already holds this content, if any.
    pub existing: Option<i64>,
    /// For audio files: what the header says.
    pub audio: Option<AudioInfo>,
    source: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub project: String,
    pub root: PathBuf,
    pub assets: Vec<PlannedAsset>,
    /// Files skipped because they could not be read.
    pub unreadable: Vec<String>,
}

impl Plan {
    /// Bytes that identical copies within the project would waste.
    pub fn duplicate_bytes(&self) -> u64 {
        self.assets.iter().map(|a| a.size * (a.paths.len() as u64 - 1)).sum()
    }
}

/// What the person decided for one piece of content.
#[derive(Debug, Clone)]
pub struct Decision {
    pub hash: String,
    pub licence: Licence,
    /// Who is confirming. Required: no name, not confirmed, not imported.
    pub confirmed_by: String,
    pub note: String,
    pub author: String,
    pub source_url: String,
    pub attribution_text: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Asset ids newly stored.
    pub imported: Vec<i64>,
    /// Asset ids that already existed; only their uses were added.
    pub linked: Vec<i64>,
    /// Content left alone for want of a confirmed decision.
    pub skipped: Vec<String>,
    /// The library holds this content under a different confirmed licence.
    pub conflicts: Vec<Conflict>,
    pub failed: Vec<(String, String)>,
    /// Bytes not copied because the content was already held.
    pub bytes_saved: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub hash: String,
    pub existing: String,
    pub proposed: String,
}

pub fn plan(cat: &mut Catalog, root: &Path, opts: &Options) -> Result<Plan> {
    let root = root.canonicalize()?;
    let project = root.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();

    let mut files: Vec<PathBuf> = Vec::new();
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !(e.depth() > 0 && e.file_type().is_dir() && SKIP_DIRS.iter().any(|s| e.file_name() == *s)));
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let k = kind::classify(entry.path());
        if !(k.is_media() || (opts.include_code && k == Kind::Script)) {
            continue;
        }
        if entry.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
            continue;
        }
        files.push(entry.into_path());
    }

    let hashes = cat.hash_many(&files)?;
    let mut assets: Vec<PlannedAsset> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut unreadable = Vec::new();
    for (path, hash) in files.iter().zip(hashes) {
        let rel = rel_path(&root, path);
        let Some(hash) = hash else {
            unreadable.push(rel);
            continue;
        };
        let guess = licence::guess_for(path, &root);
        let i = *index.entry(hash.clone()).or_insert_with(|| {
            assets.push(PlannedAsset {
                existing: None,
                audio: if kind::classify(path) == Kind::Audio { audio::probe(path).ok() } else { None },
                size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
                kind: kind::classify(path),
                name: path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
                ext: path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase(),
                paths: Vec::new(),
                guesses: Vec::new(),
                source: path.clone(),
                hash,
            });
            assets.len() - 1
        });
        let a = &mut assets[i];
        a.paths.push(rel);
        if let Some(g) = guess {
            if !a.guesses.iter().any(|x| x.spdx == g.spdx) {
                a.guesses.push(g);
            }
        }
    }
    for a in &mut assets {
        a.existing = cat.find_by_hash(&a.hash)?;
    }
    Ok(Plan { project, root, assets, unreadable })
}

/// Imports what the decisions confirm into `library_dir` (content-addressed:
/// `ab/abcdef….ext`) and the catalog. Anything without a decision, or whose
/// decision names no confirmer, is skipped and not copied.
pub fn apply(cat: &mut Catalog, library_dir: &Path, plan: &Plan, decisions: &[Decision]) -> Result<Report> {
    let by_hash: HashMap<&str, &Decision> = decisions.iter().map(|d| (d.hash.as_str(), d)).collect();
    let mut report = Report::default();
    for a in &plan.assets {
        let Some(d) = by_hash.get(a.hash.as_str()).filter(|d| !d.confirmed_by.trim().is_empty()) else {
            report.skipped.push(a.hash.clone());
            continue;
        };
        match apply_one(cat, library_dir, plan, a, d, &mut report) {
            Ok(()) => {}
            Err(e) => report.failed.push((a.hash.clone(), e.to_string())),
        }
    }
    Ok(report)
}

fn apply_one(cat: &mut Catalog, library_dir: &Path, plan: &Plan, a: &PlannedAsset, d: &Decision, report: &mut Report) -> Result<()> {
    let existing = cat.find_by_hash(&a.hash)?;
    let id = match existing {
        Some(id) => {
            let held = cat.get(id)?.ok_or_else(|| Error::Other("asset vanished".into()))?;
            if let (Status::Confirmed { .. }, Some(l)) = (&held.status, &held.licence) {
                if l.spdx != d.licence.spdx {
                    report.conflicts.push(Conflict { hash: a.hash.clone(), existing: l.spdx.clone(), proposed: d.licence.spdx.clone() });
                    return Ok(());
                }
            }
            if !held.is_usable() {
                cat.confirm_licence(id, &d.licence, &d.confirmed_by, &d.note)?;
            }
            report.linked.push(id);
            report.bytes_saved += a.size * a.paths.len() as u64;
            id
        }
        None => {
            let stored = store(library_dir, a)?;
            let (id, _) = cat.add_asset(&NewAsset {
                hash: a.hash.clone(),
                size: a.size,
                kind: a.kind,
                name: a.name.clone(),
                ext: a.ext.clone(),
                stored,
            })?;
            cat.confirm_licence(id, &d.licence, &d.confirmed_by, &d.note)?;
            report.imported.push(id);
            report.bytes_saved += a.size * (a.paths.len() as u64 - 1);
            id
        }
    };
    if !(d.author.is_empty() && d.source_url.is_empty() && d.attribution_text.is_empty()) {
        cat.set_credit(id, &d.author, &d.source_url, &d.attribution_text)?;
    }
    if a.kind == Kind::Audio {
        if let Some(info) = &a.audio {
            cat.set_audio(id, info, audio::levels(&a.source).ok().flatten())?;
        }
        let stem = Path::new(&a.name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(g) = audio::parse_game_name(stem) {
            for t in audio::tags_from_name(&g) {
                cat.add_tag(id, &t)?;
            }
        }
    }
    for t in &d.tags {
        cat.add_tag(id, t)?;
    }
    for p in &a.paths {
        cat.record_use(id, &plan.project, p)?;
    }
    cat.record_origin(id, "project", &format!("{}:{}", plan.project, a.paths[0]))?;
    Ok(())
}

/// Copies the file into the library under its hash, checking the copy.
/// Returns the library-relative path.
fn store(library_dir: &Path, a: &PlannedAsset) -> Result<String> {
    let rel = format!("{}/{}{}", &a.hash[..2], a.hash, if a.ext.is_empty() { String::new() } else { format!(".{}", a.ext) });
    let dest = library_dir.join(&rel);
    if dest.exists() {
        return Ok(rel);
    }
    std::fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("partial");
    std::fs::copy(&a.source, &tmp)?;
    let got = crate::dedup::hash_file(&tmp)?;
    if got != a.hash {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Other(format!("{} changed while importing", a.source.display())));
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(rel)
}

fn rel_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}
