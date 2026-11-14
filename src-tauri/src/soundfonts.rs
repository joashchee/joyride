//! Which sound MIDI has in JoyrIDE: the synthesizer's own instruments, or one of the General MIDI SoundFonts that ship with the app
//! (`joyride_core::soundfonts` has the list and the licences). The choice is kept in the app-data folder; the MIDI preview uses it.

use joyride_core::soundfonts::{self as list, BUILTIN};
use neumetik_midi::SoundBank;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

/// The bank last loaded, so a second MIDI file does not read the SoundFont again.
#[derive(Default)]
pub struct SoundfontCache(Mutex<Option<(String, Arc<SoundBank>)>>);

/// A bank chosen for MIDI, loaded.
pub struct Sound {
    pub name: String,
    pub bank: Arc<SoundBank>,
}

fn choice_file(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("soundfont.txt")).map_err(|e| e.to_string())
}

/// Where the shipped SoundFonts are: beside the app's other resources, or (a development build) in the source tree.
pub fn resources(app: &AppHandle) -> PathBuf {
    let installed = app.path().resource_dir().ok().map(|d| d.join("resources/soundfonts"));
    installed.filter(|p| p.is_dir()).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/soundfonts"))
}

/// The chosen bank, loaded (or the cached one). `None` for the built-in sound, or if the file cannot be read: the preview then plays
/// as it always did.
pub fn current(app: &AppHandle) -> Option<Sound> {
    let id = choice_file(app).map(|f| list::load_choice(&f)).unwrap_or_else(|_| list::DEFAULT.into());
    let b = list::get(&id)?;
    let cache = app.state::<SoundfontCache>();
    let mut g = cache.0.lock().ok()?;
    if let Some((cid, bank)) = g.as_ref() {
        if *cid == id {
            return Some(Sound { name: b.name.into(), bank: bank.clone() });
        }
    }
    let bytes = std::fs::read(list::path(&resources(app), &id)?).ok()?;
    let bank = Arc::new(SoundBank::from_bytes(&bytes).ok()?);
    *g = Some((id, bank.clone()));
    Some(Sound { name: b.name.into(), bank })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontView {
    id: String,
    name: String,
    licence: String,
    author: String,
    size: u64,
    summary: String,
    /// Its file is in this build.
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundView {
    chosen: String,
    fonts: Vec<FontView>,
}

fn view(app: &AppHandle) -> Result<SoundView, String> {
    let dir = resources(app);
    let mut fonts = vec![FontView {
        id: BUILTIN.into(),
        name: "Built-in synthesizer".into(),
        licence: "MIT".into(),
        author: "ansiapps (Neumetik)".into(),
        size: 0,
        summary: "Every instrument synthesized as it plays: no SoundFont, nothing to load. How MIDI sounded here before the SoundFonts.".into(),
        available: true,
    }];
    for b in list::BUNDLED {
        fonts.push(FontView { id: b.id.into(), name: b.name.into(), licence: b.licence.into(), author: b.author.into(), size: b.size, summary: b.summary.into(), available: list::path(&dir, b.id).is_some() });
    }
    Ok(SoundView { chosen: list::load_choice(&choice_file(app)?), fonts })
}

#[tauri::command]
pub fn soundfont_list(app: AppHandle) -> Result<SoundView, String> {
    view(&app)
}

/// Chooses the sound MIDI previews have; an id that is not in the list is refused.
#[tauri::command]
pub fn soundfont_choose(app: AppHandle, _cache: State<SoundfontCache>, id: String) -> Result<SoundView, String> {
    list::save_choice(&choice_file(&app)?, &id).map_err(|e| e.to_string())?;
    view(&app)
}

// ---- using a bank in the open project ------------------------------------------------------------------------------------------

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WarningView {
    level: &'static str,
    text: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    id: String,
    name: String,
    dest: String,
    licence_dest: String,
    warnings: Vec<WarningView>,
    blocked: bool,
    already_there: bool,
    snippet: String,
}

fn plan_view(app_dir: &std::path::Path, root: &std::path::Path, id: &str) -> Result<PlanView, String> {
    let b = list::get(id).ok_or_else(|| format!("`{id}` is not one of the SoundFonts that ship with JoyrIDE"))?;
    let file = list::path(app_dir, id);
    let p = list::plan(root, b, file.as_deref());
    Ok(PlanView {
        id: b.id.into(),
        name: b.name.into(),
        dest: p.dest,
        licence_dest: p.licence_dest,
        warnings: p.warnings.into_iter().map(|w| WarningView { level: w.level, text: w.text }).collect(),
        blocked: p.blocked,
        already_there: p.already_there,
        snippet: list::snippet(b),
    })
}

/// What adding the bank to the open project would do, and what to know first. Nothing is written.
#[tauri::command]
pub fn soundfont_plan(app: AppHandle, project: State<crate::project::Project>, id: String) -> Result<PlanView, String> {
    plan_view(&resources(&app), &project.root()?, &id)
}

/// Copies the bank into the open project (`assets/soundfonts/`), its licence beside it, and records it in the project's asset record with
/// the licence confirmed by JoyrIDE's maintainers. Refused when the plan has a blocker; nothing is replaced.
pub fn add_bank(resources: &std::path::Path, library: &std::path::Path, root: &std::path::Path, id: &str) -> Result<PlanView, String> {
    use asset_core::add::{add_to_project, AddRequest};
    use asset_core::licence::Licence;
    let view = plan_view(resources, root, id)?;
    if let Some(b) = view.warnings.iter().find(|w| w.level == "blocker") {
        return Err(b.text.clone());
    }
    let b = list::get(id).ok_or("unknown bank")?;
    let source = list::path(resources, id).ok_or("the bank is not in this build")?;
    if !view.already_there {
        std::fs::create_dir_all(library).map_err(|e| e.to_string())?;
        let mut cat = asset_core::catalog::Catalog::open(&library.join("catalog.sqlite")).map_err(|e| e.to_string())?;
        let name = root.canonicalize().map_err(|e| e.to_string())?.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
        let req = AddRequest {
            source: &source,
            dest: &view.dest,
            licence: Licence::from_spdx(b.licence),
            confirmed_by: "JoyrIDE maintainers",
            note: "Ships with JoyrIDE. The licence is the one in the file's own text, recorded with the file.",
            author: b.author,
            source_url: list::source_url(b),
            attribution_text: list::attribution(b),
            tags: vec!["soundfont".into(), "general-midi".into()],
        };
        add_to_project(&mut cat, &library.join("files"), &name, root, &req).map_err(|e| e.to_string())?;
    }
    // The licence travels with the bank (BSD asks for it to be kept); never over a file that is there.
    let licence_to = root.join(&view.licence_dest);
    if licence_to.symlink_metadata().is_err() {
        std::fs::copy(resources.join(b.licence_file), &licence_to).map_err(|e| format!("the licence file could not be copied: {e}"))?;
    }
    Ok(view)
}

/// Adds the bank to the open project (see [`add_bank`]).
#[tauri::command]
pub async fn soundfont_use(app: AppHandle, project: State<'_, crate::project::Project>, id: String) -> Result<PlanView, String> {
    let (res, lib, root) = (resources(&app), crate::assets::library_dir(&app)?, project.root()?);
    tauri::async_runtime::spawn_blocking(move || add_bank(&res, &lib, &root, &id)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn res() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/soundfonts")
    }

    #[test]
    fn adds_a_bank_with_its_licence_and_record_and_refuses_what_it_should() {
        let d = tempfile::tempdir().unwrap();
        let (root, lib) = (d.path().join("game"), d.path().join("lib"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("main.lua"), "").unwrap();

        let v = add_bank(&res(), &lib, &root, "p800").unwrap();
        assert_eq!(v.dest, "assets/soundfonts/sony-ericsson-p800-gm.sf2");
        assert_eq!(std::fs::read(root.join(&v.dest)).unwrap(), std::fs::read(res().join("sony-ericsson-p800-gm.sf2")).unwrap());
        assert!(std::fs::read_to_string(root.join(&v.licence_dest)).unwrap().contains("Beatnik, Inc"));
        // The project's asset record has it, the licence confirmed, and the credit line its BSD licence asks for.
        let record = std::fs::read_to_string(root.join(".joyride/assets.toml")).unwrap();
        assert!(record.contains("sony-ericsson-p800-gm.sf2") && record.contains("BSD-3-Clause") && record.contains("confirmed = true"), "{record}");
        let cat = asset_core::catalog::Catalog::open(&lib.join("catalog.sqlite")).unwrap();
        let credits = asset_core::project::credits(&cat, "game").unwrap();
        assert!(credits.contains("Beatnik"), "CREDITS.txt will carry it: {credits}");

        // Again: the same file is there, nothing more to add, and it is not an error.
        let again = add_bank(&res(), &lib, &root, "p800").unwrap();
        assert!(again.already_there);

        // A different file in the way is never replaced.
        std::fs::write(root.join("assets/soundfonts/protosquare.sf2"), "mine").unwrap();
        let e = add_bank(&res(), &lib, &root, "protosquare").unwrap_err();
        assert!(e.contains("different file"), "{e}");
        assert_eq!(std::fs::read(root.join("assets/soundfonts/protosquare.sf2")).unwrap(), b"mine");

        // Nothing but a listed id, and a CC0 bank asks for no credit.
        assert!(add_bank(&res(), &lib, &root, "../../etc/passwd").is_err());
        let v = add_bank(&res(), &lib, &root, "pocketsongs").unwrap();
        assert!(v.warnings.iter().any(|w| w.text.contains("111 samples")));
        let cat = asset_core::catalog::Catalog::open(&lib.join("catalog.sqlite")).unwrap();
        assert!(!asset_core::project::credits(&cat, "game").unwrap().contains("Vini"));
        // No project open: refused before anything is written.
        assert!(add_bank(&res(), &lib, &d.path().join("nowhere"), "p800").unwrap_err().contains("No project"));
    }
}
