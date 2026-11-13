//! The offline API reference: `joyride_core::docs` built once from the LÖVE
//! types that ship inside the bundled language server.

use joyride_core::docs::{Docs, EntryKind};
use serde::Serialize;
use std::sync::OnceLock;
use tauri::AppHandle;

static DOCS: OnceLock<Option<Docs>> = OnceLock::new();

fn docs(app: &AppHandle) -> Option<&'static Docs> {
    DOCS.get_or_init(|| {
        let tc = crate::bundled_toolchains(app);
        let dir = joyride_core::lsp::find_love_types(tc.as_deref())?;
        let mut texts = vec![std::fs::read_to_string(dir.join("love.lua")).ok()?];
        for e in std::fs::read_dir(dir.join("love")).ok()?.flatten() {
            if e.path().extension().is_some_and(|x| x == "lua") {
                texts.push(std::fs::read_to_string(e.path()).ok()?);
            }
        }
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        Some(Docs::from_luacats("", &refs))
    })
    .as_ref()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    name: String,
    kind: &'static str,
    /// The first sentence-ish line of the description.
    summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    version: String,
    results: Vec<Found>,
}

fn kind(k: EntryKind) -> &'static str {
    match k {
        EntryKind::Function => "function",
        EntryKind::Callback => "callback",
        EntryKind::Method => "method",
        EntryKind::Type => "type",
        EntryKind::Enum => "enum",
    }
}

/// Searches the reference (built on first use, off the UI thread).
#[tauri::command]
pub async fn docs_search(app: AppHandle, query: String) -> Result<Results, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = docs(&app).ok_or("The LÖVE reference was not found. Run scripts/fetch-toolchains.py.")?;
        let results = d
            .search(&query, 40)
            .into_iter()
            .map(|h| {
                let e = &d.entries[h.index];
                Found { name: e.name.clone(), kind: kind(e.kind), summary: e.description.lines().next().unwrap_or("").to_string() }
            })
            .collect();
        Ok(Results { version: d.version.clone(), results })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct Detail {
    /// Markdown: signatures, then the description.
    text: String,
    /// A snippet to insert, for functions and callbacks.
    snippet: Option<String>,
}

#[tauri::command]
pub async fn docs_get(app: AppHandle, name: String) -> Result<Option<Detail>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(d) = docs(&app) else { return Ok(None) };
        Ok(d.hover(&name).map(|text| Detail { text, snippet: d.snippet(&name) }))
    })
    .await
    .map_err(|e| e.to_string())?
}
