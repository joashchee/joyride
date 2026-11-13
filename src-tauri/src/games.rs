//! The games and apps library's commands over `joyride_core::games` (the
//! bundled catalogue). Reading only: nothing is downloaded here; "Open" hands
//! a catalogue page or file to the browser, and only a URL the catalogue
//! itself lists.

use joyride_core::games::{Catalogue, Entry, Filter, Kind, Platform};
use serde::Serialize;

#[derive(Serialize)]
pub struct Link {
    kind: String,
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadView {
    url: String,
    platform: String,
    version: Option<String>,
    date: Option<String>,
    size: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    id: String,
    name: String,
    kind: &'static str,
    summary: String,
    authors: Vec<String>,
    tags: Vec<String>,
    licence: String,
    licence_from: Option<String>,
    licence_confirmed: bool,
    open_source: bool,
    latest: Option<String>,
    love_version: Option<String>,
    archived: bool,
    checked: Option<String>,
    sources: Vec<Link>,
    downloads: Vec<DownloadView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesResult {
    games: Vec<GameView>,
    /// Entries in the whole catalogue, whatever the filters.
    total: usize,
    generated: String,
}

fn name_of<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|x| x.as_str().map(str::to_string)).unwrap_or_default()
}

fn view(e: &Entry) -> GameView {
    GameView {
        id: e.id.clone(),
        name: e.name.clone(),
        kind: if e.kind == Kind::App { "app" } else { "game" },
        summary: e.summary.clone(),
        authors: e.authors.clone(),
        tags: e.tags.clone(),
        licence: e.licence.id.clone(),
        licence_from: e.licence.from.clone(),
        licence_confirmed: e.licence.confirmed,
        open_source: joyride_core::games::is_open(&e.licence),
        latest: e.latest.as_ref().map(|l| match &l.date {
            Some(d) => format!("{} ({d})", l.version),
            None => l.version.clone(),
        }),
        love_version: e.love_version.clone(),
        archived: e.archived,
        checked: e.checked.clone(),
        sources: e.sources.iter().map(|s| Link { kind: name_of(&s.kind), url: s.url.clone() }).collect(),
        downloads: e.downloads.iter().map(|d| DownloadView { url: d.url.clone(), platform: name_of(&d.platform), version: d.version.clone(), date: d.date.clone(), size: d.size }).collect(),
    }
}

fn parse_platform(s: &str) -> Option<Platform> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

pub fn search(c: &Catalogue, query: &str, kind: &str, platform: &str, open_source: bool, include_archived: bool) -> GamesResult {
    let f = Filter {
        kind: match kind {
            "game" => Some(Kind::Game),
            "app" => Some(Kind::App),
            _ => None,
        },
        platform: parse_platform(platform),
        known_licence: false,
        open_source,
        include_archived,
    };
    GamesResult { games: c.search(query, &f).into_iter().map(view).collect(), total: c.entries.len(), generated: c.generated.clone() }
}

/// Only an `https://` URL the catalogue lists (a source page or a download).
pub fn listed_url(c: &Catalogue, url: &str) -> bool {
    url.starts_with("https://") && !url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) && c.entries.iter().any(|e| e.sources.iter().any(|s| s.url == url) || e.downloads.iter().any(|d| d.url == url))
}

#[tauri::command]
pub async fn games_search(query: String, kind: String, platform: String, open_source: bool, include_archived: bool) -> Result<GamesResult, String> {
    tauri::async_runtime::spawn_blocking(move || search(&Catalogue::bundled(), &query, &kind, &platform, open_source, include_archived)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn games_open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !listed_url(&Catalogue::bundled(), &url) {
        return Err("only a page or file the catalogue lists can be opened from here".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyride_core::games::{Download, Licence, Source, SourceKind};

    fn cat() -> Catalogue {
        let mut c = Catalogue::default();
        c.add(Entry {
            name: "Sky Race".into(),
            summary: "Fly".into(),
            licence: Licence { id: "MIT".into(), from: None, confirmed: true },
            sources: vec![Source { kind: SourceKind::Github, url: "https://github.com/a/sky".into() }],
            downloads: vec![Download { url: "https://github.com/a/sky/releases/download/v1/sky.apk".into(), platform: Platform::Android, version: Some("v1".into()), date: None, size: Some(5), blake3: None }],
            ..Default::default()
        });
        c.add(Entry { name: "Old".into(), archived: true, sources: vec![Source { kind: SourceKind::Web, url: "https://old.example/".into() }], ..Default::default() });
        c
    }

    #[test]
    fn search_filters_and_reports_the_whole_size() {
        let c = cat();
        let r = search(&c, "", "", "", false, false);
        assert_eq!((r.games.len(), r.total), (1, 2));
        assert_eq!(search(&c, "", "", "android", false, false).games.len(), 1);
        assert_eq!(search(&c, "", "", "windows", false, false).games.len(), 0);
        assert_eq!(search(&c, "", "app", "", false, false).games.len(), 0);
        assert_eq!(search(&c, "", "", "", true, true).games.len(), 1, "only the open-source one");
        let g = &search(&c, "sky", "", "", false, false).games[0];
        assert_eq!((g.platform_of_first(), g.open_source), ("android".to_string(), true));
    }

    impl GameView {
        fn platform_of_first(&self) -> String {
            self.downloads[0].platform.clone()
        }
    }

    #[test]
    fn only_listed_https_urls_open() {
        let c = cat();
        assert!(listed_url(&c, "https://github.com/a/sky"));
        assert!(listed_url(&c, "https://github.com/a/sky/releases/download/v1/sky.apk"));
        for bad in ["http://github.com/a/sky", "https://github.com/a/other", "file:///etc/passwd", "https://github.com/a/sky ", "javascript:alert(1)"] {
            assert!(!listed_url(&c, bad), "{bad}");
        }
    }
}
