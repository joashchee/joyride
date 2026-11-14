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
