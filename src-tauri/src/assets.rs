//! The Asset Library's commands over `asset_core`: browse and search the
//! library, import a project's assets (plan, then apply once a named person
//! confirmed each licence), confirm a licence, and use a confirmed asset in
//! the open project. The rules live in `asset_core` (nothing is usable or
//! imported until its licence is confirmed; nothing is replaced or deleted);
//! this file only converts to and from what the panel shows.

use crate::project::Project;
use asset_core::catalog::{Catalog, Filter};
use asset_core::import::{self, Decision, Options};
use asset_core::licence::{Licence, Status};
use asset_core::{Asset, Kind};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

const LIMIT: usize = 300;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetView {
    pub id: i64,
    pub name: String,
    pub kind: &'static str,
    pub size: u64,
    pub spdx: String,
    /// `confirmed`, `guessed` or `unknown`.
    pub status: &'static str,
    pub confirmed_by: String,
    pub commercial: &'static str,
    pub attribution_required: bool,
    pub share_alike: bool,
    pub author: String,
    pub source_url: String,
    pub tags: Vec<String>,
    /// A PNG data URL for images that have a stored thumbnail.
    pub thumb: Option<String>,
    /// Where it is used, as `project: path`.
    pub uses: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetList {
    pub assets: Vec<AssetView>,
    /// Matches before the list was cut to what the panel shows.
    pub matched: usize,
    pub total: usize,
}

pub(crate) fn library_dir(app: &AppHandle) -> Result<PathBuf, String> {
    crate::fonts::library_dir(app)
}

fn open(library: &Path) -> Result<Catalog, String> {
    std::fs::create_dir_all(library).map_err(|e| e.to_string())?;
    Catalog::open(&library.join("catalog.sqlite")).map_err(|e| e.to_string())
}

fn view(cat: &Catalog, a: &Asset) -> Result<AssetView, String> {
    let (status, by) = match &a.status {
        Status::Confirmed { by, .. } => ("confirmed", by.clone()),
        Status::Guessed => ("guessed", String::new()),
        Status::Unknown => ("unknown", String::new()),
    };
    let thumb = cat.preview(a.id).map_err(|e| e.to_string())?.and_then(|p| p.thumb_png).map(|png| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)));
    let uses = cat.uses(a.id).map_err(|e| e.to_string())?.into_iter().map(|(p, f)| format!("{p}: {f}")).collect();
    let l = a.licence.as_ref();
    Ok(AssetView {
        id: a.id,
        name: a.name.clone(),
        kind: a.kind.as_str(),
        size: a.size,
        spdx: l.map(|l| l.spdx.clone()).unwrap_or_default(),
        status,
        confirmed_by: by,
        commercial: l.map(|l| l.commercial_use.as_str()).unwrap_or("unknown"),
        attribution_required: l.is_some_and(|l| l.attribution_required),
        share_alike: l.is_some_and(|l| l.share_alike),
        author: a.author.clone(),
        source_url: a.source_url.clone(),
        tags: a.tags.clone(),
        thumb,
        uses,
    })
}

pub fn search(library: &Path, query: &str, kind: &str, confirmed_only: bool, commercial_only: bool) -> Result<AssetList, String> {
    let cat = open(library)?;
    let f = Filter { kind: (!kind.is_empty()).then(|| Kind::parse(kind)), confirmed_only, commercial_only, ..Default::default() };
    let all = cat.search(query, &f).map_err(|e| e.to_string())?;
    let total = cat.search("", &Filter::default()).map_err(|e| e.to_string())?.len();
    let matched = all.len();
    let assets = all.iter().take(LIMIT).map(|a| view(&cat, a)).collect::<Result<_, _>>()?;
    Ok(AssetList { assets, matched, total })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlannedView {
    pub hash: String,
    pub name: String,
    pub kind: &'static str,
    pub size: u64,
    pub paths: Vec<String>,
    pub guess: Option<String>,
    pub guess_from: Option<String>,
    /// The library already holds this content (its id).
    pub existing: Option<i64>,
    /// ... and under which confirmed licence, if confirmed.
    pub existing_licence: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanView {
    pub project: String,
    pub assets: Vec<PlannedView>,
    pub duplicate_bytes: u64,
    pub unreadable: Vec<String>,
}

fn plan_for(cat: &mut Catalog, root: &Path, include_code: bool) -> Result<import::Plan, String> {
    import::plan(cat, root, &Options { include_code }).map_err(|e| e.to_string())
}

pub fn import_plan(library: &Path, root: &Path, include_code: bool) -> Result<ImportPlanView, String> {
    let mut cat = open(library)?;
    let plan = plan_for(&mut cat, root, include_code)?;
    let mut assets = Vec::new();
    for a in &plan.assets {
        let held = match a.existing {
            Some(id) => cat.get(id).map_err(|e| e.to_string())?,
            None => None,
        };
        let existing_licence = held.as_ref().filter(|h| h.is_usable()).and_then(|h| h.licence.as_ref()).map(|l| l.spdx.clone());
        let g = a.guesses.first();
        assets.push(PlannedView {
            hash: a.hash.clone(),
            name: a.name.clone(),
            kind: a.kind.as_str(),
            size: a.size,
            paths: a.paths.clone(),
            guess: g.map(|g| g.spdx.clone()),
            guess_from: g.map(|g| g.source.file_name().and_then(|n| n.to_str()).unwrap_or("a licence file").to_string()),
            existing: a.existing,
            existing_licence,
        });
    }
    Ok(ImportPlanView { project: plan.project.clone(), duplicate_bytes: plan.duplicate_bytes(), unreadable: plan.unreadable.clone(), assets })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DecisionIn {
    pub hash: String,
    pub licence: String,
    pub confirmed_by: String,
    pub note: String,
    pub author: String,
    pub source_url: String,
    pub attribution: String,
}

fn licence_of(id: &str) -> Licence {
    if id == "unknown" {
        Licence::unknown("unknown")
    } else {
        Licence::from_spdx(id)
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub name: String,
    pub existing: String,
    pub proposed: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportReportView {
    pub imported: usize,
    pub linked: usize,
    pub skipped: usize,
    pub conflicts: Vec<ConflictView>,
    pub failed: Vec<String>,
    pub bytes_saved: u64,
}

/// Imports what the decisions confirm. A decision with an unknown licence needs a reason, as in Add Font.
pub fn import_apply(library: &Path, root: &Path, include_code: bool, decisions: &[DecisionIn]) -> Result<ImportReportView, String> {
    for d in decisions {
        let unknownish = d.licence == "unknown" || d.licence == "LicenseRef-Custom";
        if !d.confirmed_by.trim().is_empty() && unknownish && d.note.trim().is_empty() {
            return Err("a licence that is unknown or custom needs a note saying why it is used".into());
        }
        if !d.confirmed_by.trim().is_empty() && d.licence.trim().is_empty() {
            return Err("pick a licence for every asset you confirm".into());
        }
    }
    let mut cat = open(library)?;
    let plan = plan_for(&mut cat, root, include_code)?;
    let ds: Vec<Decision> = decisions
        .iter()
        .filter(|d| !d.confirmed_by.trim().is_empty())
        .map(|d| Decision {
            hash: d.hash.clone(),
            licence: licence_of(&d.licence),
            confirmed_by: d.confirmed_by.trim().into(),
            note: d.note.trim().into(),
            author: d.author.trim().into(),
            source_url: d.source_url.trim().into(),
            attribution_text: d.attribution.trim().into(),
            tags: vec![],
        })
        .collect();
    let r = import::apply(&mut cat, &library.join("files"), &plan, &ds).map_err(|e| e.to_string())?;
    asset_core::project::write_assets_toml(&cat, &plan.project, &plan.root).map_err(|e| e.to_string())?;
    let name_of = |hash: &str| plan.assets.iter().find(|a| a.hash == hash).map(|a| a.name.clone()).unwrap_or_else(|| hash.chars().take(8).collect());
    Ok(ImportReportView {
        imported: r.imported.len(),
        linked: r.linked.len(),
        skipped: r.skipped.len(),
        conflicts: r.conflicts.iter().map(|c| ConflictView { name: name_of(&c.hash), existing: c.existing.clone(), proposed: c.proposed.clone() }).collect(),
        failed: r.failed.iter().map(|(h, e)| format!("{}: {e}", name_of(h))).collect(),
        bytes_saved: r.bytes_saved,
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmIn {
    pub id: i64,
    pub licence: String,
    pub confirmed_by: String,
    pub note: String,
    pub author: String,
    pub source_url: String,
    pub attribution: String,
}

/// A person confirms (or corrects) a library asset's licence.
pub fn confirm(library: &Path, r: &ConfirmIn) -> Result<(), String> {
    if r.confirmed_by.trim().is_empty() {
        return Err("say who confirms the licence".into());
    }
    if r.licence.trim().is_empty() {
        return Err("pick a licence".into());
    }
    if (r.licence == "unknown" || r.licence == "LicenseRef-Custom") && r.note.trim().is_empty() {
        return Err("a licence that is unknown or custom needs a note saying why it is used".into());
    }
    let mut cat = open(library)?;
    cat.get(r.id).map_err(|e| e.to_string())?.ok_or("no such asset")?;
    cat.confirm_licence(r.id, &licence_of(&r.licence), r.confirmed_by.trim(), r.note.trim()).map_err(|e| e.to_string())?;
    cat.set_credit(r.id, r.author.trim(), r.source_url.trim(), r.attribution.trim()).map_err(|e| e.to_string())
}

pub fn use_asset(library: &Path, root: &Path, id: i64, dest: &str) -> Result<String, String> {
    let mut cat = open(library)?;
    let name = root.canonicalize().map_err(|e| e.to_string())?.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
    asset_core::add::use_in_project(&mut cat, &library.join("files"), &name, root, id, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn assets_search(app: AppHandle, query: String, kind: String, confirmed_only: bool, commercial_only: bool) -> Result<AssetList, String> {
    let lib = library_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || search(&lib, &query, &kind, confirmed_only, commercial_only)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn assets_import_plan(app: AppHandle, project: State<'_, Project>, include_code: bool) -> Result<ImportPlanView, String> {
    let (lib, root) = (library_dir(&app)?, project.root()?);
    tauri::async_runtime::spawn_blocking(move || import_plan(&lib, &root, include_code)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn assets_import_apply(app: AppHandle, project: State<'_, Project>, include_code: bool, decisions: Vec<DecisionIn>) -> Result<ImportReportView, String> {
    let (lib, root) = (library_dir(&app)?, project.root()?);
    tauri::async_runtime::spawn_blocking(move || import_apply(&lib, &root, include_code, &decisions)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn assets_confirm(app: AppHandle, request: ConfirmIn) -> Result<(), String> {
    let lib = library_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || confirm(&lib, &request)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn assets_use(app: AppHandle, project: State<'_, Project>, id: i64, dest: String) -> Result<String, String> {
    let (lib, root) = (library_dir(&app)?, project.root()?);
    tauri::async_runtime::spawn_blocking(move || use_asset(&lib, &root, id, &dest)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(path: &Path) {
        // A valid 2x2 PNG made by the image crate would need a dependency here; a tiny hand-made one is enough for hashing.
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, [0x89, b'P', b'N', b'G', 13, 10, 26, 10, 1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    }

    fn dec(hash: &str, by: &str, licence: &str, note: &str) -> DecisionIn {
        DecisionIn { hash: hash.into(), licence: licence.into(), confirmed_by: by.into(), note: note.into(), author: "A".into(), source_url: String::new(), attribution: String::new() }
    }

    #[test]
    fn plan_confirm_import_search_and_use() {
        let d = tempfile::tempdir().unwrap();
        let (lib, a, b) = (d.path().join("lib"), d.path().join("one"), d.path().join("two"));
        png(&a.join("art/hero.png"));
        png(&a.join("art/copy.png"));
        std::fs::write(a.join("LICENSE"), "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy").unwrap();
        std::fs::create_dir_all(&b).unwrap();

        let plan = import_plan(&lib, &a, false).unwrap();
        assert_eq!(plan.assets.len(), 1, "identical content is one asset");
        assert_eq!(plan.assets[0].paths.len(), 2);
        let hash = plan.assets[0].hash.clone();

        // Nothing is imported without a named confirmer.
        let r = import_apply(&lib, &a, false, &[dec(&hash, " ", "MIT", "")]).unwrap();
        assert_eq!((r.imported, r.skipped), (0, 1));
        assert_eq!(search(&lib, "", "", false, false).unwrap().total, 0);
        // An unknown licence needs a reason.
        assert!(import_apply(&lib, &a, false, &[dec(&hash, "Joash", "unknown", "")]).is_err());

        let r = import_apply(&lib, &a, false, &[dec(&hash, "Joash", "MIT", "")]).unwrap();
        assert_eq!(r.imported, 1);
        assert!(a.join(".joyride/assets.toml").is_file());
        let found = search(&lib, "copy", "image", true, true).unwrap();
        assert_eq!(found.assets.len(), 1);
        assert_eq!(found.assets[0].status, "confirmed");
        assert_eq!(found.assets[0].confirmed_by, "Joash");
        assert_eq!(found.assets[0].uses.len(), 2);
        assert_eq!(search(&lib, "", "audio", false, false).unwrap().matched, 0);

        // Using it in another project copies it and records the use.
        let id = found.assets[0].id;
        assert_eq!(use_asset(&lib, &b, id, "gfx/hero.png").unwrap(), "gfx/hero.png");
        assert!(b.join("gfx/hero.png").is_file() && use_asset(&lib, &b, id, "gfx/hero.png").is_err());
        // The plan for the second project now sees the library holds it, under MIT.
        let p2 = import_plan(&lib, &b, false).unwrap();
        assert_eq!(p2.assets[0].existing_licence.as_deref(), Some("MIT"));
    }

    #[test]
    fn confirming_needs_a_name_and_a_reason_for_unknown() {
        let d = tempfile::tempdir().unwrap();
        let (lib, a) = (d.path().join("lib"), d.path().join("one"));
        png(&a.join("x.png"));
        let hash = import_plan(&lib, &a, false).unwrap().assets[0].hash.clone();
        // Unconfirmed assets never reach the library, so make one with a guess through the core API.
        let mut cat = open(&lib).unwrap();
        let (id, _) = cat.add_asset(&asset_core::NewAsset { hash, size: 16, kind: Kind::Image, name: "x.png".into(), ext: "png".into(), stored: "x".into() }).unwrap();
        drop(cat);
        let c = |by: &str, l: &str, n: &str| ConfirmIn { id, licence: l.into(), confirmed_by: by.into(), note: n.into(), author: String::new(), source_url: String::new(), attribution: String::new() };
        assert!(confirm(&lib, &c("", "MIT", "")).is_err());
        assert!(confirm(&lib, &c("J", "unknown", "")).is_err());
        assert!(confirm(&lib, &c("J", "", "")).is_err());
        confirm(&lib, &c("J", "CC0-1.0", "")).unwrap();
        let l = search(&lib, "x", "", true, true).unwrap();
        assert_eq!(l.assets[0].commercial, "yes");
        assert!(confirm(&lib, &ConfirmIn { id: 999, ..c("J", "MIT", "") }).is_err());
    }
}
