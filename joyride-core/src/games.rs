//! The LÖVE games and apps catalogue (`docs/love-games-library.md`): what
//! exists, where it is downloaded from, under what licence, and which
//! version is the latest. Entries come from several places (GitHub, itch.io,
//! the LÖVE forums, PortMaster); the same game turns up in more than one, so
//! every entry added is **de-duplicated** against what is there and merged,
//! keeping the newest version and never losing a source or a download.
//!
//! This is the data model and its rules, with no network: `from_github`
//! turns GitHub's own JSON (fetched with `gh api`, no credentials of ours,
//! CLAUDE.md rule 5) into an entry, and `examples/curate_games.rs` does the
//! fetching. Nothing here downloads or runs a game.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

const BUNDLED: &str = include_str!("../data/games.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    #[default]
    Game,
    /// A tool or program made with LÖVE (an editor, a launcher, a utility).
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// A `.love` file for a LÖVE the person already has.
    Love,
    Windows,
    Mac,
    Linux,
    Android,
    Ios,
    Web,
    /// A PortMaster port (a zip for handhelds).
    Portmaster,
    /// The source (a repository archive).
    Source,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Github,
    Gitlab,
    Codeberg,
    Itch,
    Forum,
    Portmaster,
    Web,
}

/// A page the game is known from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    pub url: String,
}

/// One file to download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Download {
    pub url: String,
    pub platform: Platform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// BLAKE3 of the file, once someone has fetched it (the catalogue itself never downloads): the same bytes under two URLs are one download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licence {
    /// An SPDX id, `proprietary` (free to play, all rights reserved), or `unknown`.
    pub id: String,
    /// Where it was read from (a repository's LICENSE, the itch page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// A person looked at the licence text and agrees with `id`.
    #[serde(default)]
    pub confirmed: bool,
}

impl Default for Licence {
    fn default() -> Self {
        Licence { id: "unknown".into(), from: None, confirmed: false }
    }
}

impl Licence {
    pub fn is_unknown(&self) -> bool {
        self.id.eq_ignore_ascii_case("unknown") || self.id.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Latest {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Entry {
    /// A stable slug, from the name when first added.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub licence: Licence,
    /// The LÖVE version it targets ("11.5", "0.10"), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub love_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<Latest>,
    #[serde(default, rename = "source", skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    #[serde(default, rename = "download", skip_serializing_if = "Vec::is_empty")]
    pub downloads: Vec<Download>,
    /// The repository is archived or the page says it is abandoned.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    /// When this entry was last refreshed from its sources (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<String>,
}

// ---- keys for de-duplication -------------------------------------------------------------------

/// A URL reduced to what names the page: no scheme, `www.`, query, fragment,
/// trailing slash or `.git`, lower-case; a GitHub, GitLab or Codeberg URL
/// is cut to `host/owner/repo` (so `…/releases/tag/v1` is the repository).
pub fn canonical_url(url: &str) -> String {
    let u = url.trim().to_lowercase();
    let u = u.split_once("://").map_or(u.as_str(), |(_, r)| r);
    let u = u.split(['?', '#']).next().unwrap_or("");
    let u = u.strip_prefix("www.").unwrap_or(u).trim_end_matches('/');
    let u = u.strip_suffix(".git").unwrap_or(u);
    let mut parts = u.split('/');
    let host = parts.next().unwrap_or("");
    if matches!(host, "github.com" | "gitlab.com" | "codeberg.org") {
        let rest: Vec<&str> = parts.take(2).collect();
        return std::iter::once(host).chain(rest).collect::<Vec<_>>().join("/");
    }
    u.to_string()
}

/// `The Quick-Brown Fox!` → `the-quick-brown-fox`.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_string()
}

/// A name for comparing: lower-case letters and digits only, with a trailing
/// "(love)", "- LÖVE" or ".love" dropped.
fn name_key(name: &str) -> String {
    let n = name.to_lowercase();
    let n = n.trim_end_matches(".love");
    n.chars().filter(|c| c.is_alphanumeric()).collect::<String>().trim_end_matches("love").to_string()
}

fn same_author(a: &Entry, b: &Entry) -> bool {
    a.authors.iter().any(|x| b.authors.iter().any(|y| slug(x) == slug(y)))
}

/// Dotted-number versions compare by number (`v1.10` after `v1.9`); anything
/// else falls back to text. `None` when either has no number to go by.
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let nums = |s: &str| -> Option<Vec<u64>> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        let v: Vec<u64> = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
        (!v.is_empty() && s.starts_with(|c: char| c.is_ascii_digit())).then_some(v)
    };
    let (x, y) = (nums(a)?, nums(b)?);
    let n = x.len().max(y.len());
    for i in 0..n {
        match x.get(i).unwrap_or(&0).cmp(y.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            o => return Some(o),
        }
    }
    Some(Ordering::Equal)
}

/// Is `a` a newer release than `b`? Versions decide when both have numbers; else dates (ISO, so text order is date order).
fn newer(a: &Option<Latest>, b: &Option<Latest>) -> bool {
    match (a, b) {
        (Some(_), None) => true,
        (None, _) => false,
        (Some(x), Some(y)) => match compare_versions(&x.version, &y.version) {
            Some(Ordering::Greater) => true,
            Some(Ordering::Less) => false,
            _ => matches!((&x.date, &y.date), (Some(dx), Some(dy)) if dx > dy),
        },
    }
}

fn same_game(o: &Entry, e: &Entry) -> bool {
    let urls: Vec<String> = e.sources.iter().map(|s| canonical_url(&s.url)).collect();
    o.sources.iter().any(|s| urls.contains(&canonical_url(&s.url)))
        || o.downloads.iter().any(|d| e.downloads.iter().any(|x| canonical_url(&x.url) == canonical_url(&d.url) || (d.blake3.is_some() && d.blake3 == x.blake3)))
        || (!name_key(&e.name).is_empty() && name_key(&o.name) == name_key(&e.name) && same_author(o, e))
}

// ---- the catalogue -----------------------------------------------------------------------------

/// What `Catalogue::add` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    New(String),
    /// Merged into the entry with this id (a source, a download or an author in common).
    Merged(String),
}

/// A possible duplicate that was not merged: the same name by different (or unknown) authors. A person decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookalike {
    pub id: String,
    pub other: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalogue {
    /// `YYYY-MM-DD` the file was generated.
    #[serde(default)]
    pub generated: String,
    #[serde(default, rename = "entry")]
    pub entries: Vec<Entry>,
    /// Same-name pairs left apart, for a person to look at.
    #[serde(skip)]
    pub lookalikes: Vec<Lookalike>,
}

impl Catalogue {
    pub fn bundled() -> Catalogue {
        Catalogue::parse(BUNDLED).expect("the bundled games.toml is valid (a test checks it)")
    }

    pub fn parse(text: &str) -> Result<Catalogue> {
        toml::from_str(text).map_err(|e| crate::Error::Msg(format!("games.toml: {e}")))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("a catalogue serialises")
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The entry the new one is the same as: a shared source page, a shared download file (by hash, then by URL), or the same name with an author in common.
    fn find_same(&self, e: &Entry) -> Option<usize> {
        self.entries.iter().position(|o| same_game(o, e))
    }

    /// Adds `e`, merging it into an existing entry when it is the same game.
    /// The newest version wins; sources, downloads, tags and authors are the
    /// union; a known licence beats an unknown one and a confirmed one beats
    /// a guess; the existing entry's name and id stay.
    pub fn add(&mut self, mut e: Entry) -> Added {
        if e.id.is_empty() {
            e.id = slug(&e.name);
        }
        match self.find_same(&e) {
            Some(i) => {
                let id = self.entries[i].id.clone();
                merge(&mut self.entries[i], e);
                Added::Merged(id)
            }
            None => {
                // Ids are unique: a second "Pong" becomes "pong-2".
                let base = e.id.clone();
                let mut n = 2;
                while self.entries.iter().any(|o| o.id == e.id) {
                    e.id = format!("{base}-{n}");
                    n += 1;
                }
                // The same name by someone else is a different game until a person says otherwise.
                let key = name_key(&e.name);
                for o in &self.entries {
                    if name_key(&o.name) == key && !key.is_empty() {
                        self.lookalikes.push(Lookalike { id: o.id.clone(), other: e.id.clone() });
                    }
                }
                let id = e.id.clone();
                self.entries.push(e);
                Added::New(id)
            }
        }
    }

    /// Merges entries that turn out to be the same (after sources were added to one of them): run after a batch of `add`s. The earlier entry stays.
    pub fn dedupe(&mut self) -> usize {
        let mut merged = 0;
        let mut i = 0;
        while i < self.entries.len() {
            let mut j = i + 1;
            while j < self.entries.len() {
                if same_game(&self.entries[i], &self.entries[j]) {
                    let other = self.entries.remove(j);
                    merge(&mut self.entries[i], other);
                    merged += 1;
                    j = i + 1; // the merged entry may now match one it did not before
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        merged
    }

    /// Entries matching every word of `query` in name, summary, authors or tags, filtered.
    pub fn search(&self, query: &str, f: &Filter) -> Vec<&Entry> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        self.entries
            .iter()
            .filter(|e| {
                let hay = format!("{} {} {} {}", e.name, e.summary, e.authors.join(" "), e.tags.join(" ")).to_lowercase();
                words.iter().all(|w| hay.contains(w))
                    && f.kind.is_none_or(|k| e.kind == k)
                    && f.platform.is_none_or(|p| e.downloads.iter().any(|d| d.platform == p))
                    && (!f.known_licence || !e.licence.is_unknown())
                    && (!f.open_source || is_open(&e.licence))
                    && (f.include_archived || !e.archived)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub kind: Option<Kind>,
    pub platform: Option<Platform>,
    pub known_licence: bool,
    /// Only entries whose licence is an open-source SPDX id.
    pub open_source: bool,
    pub include_archived: bool,
}

/// An SPDX id that is not `proprietary` or unknown (a free-to-play game with all rights reserved is not open source).
pub fn is_open(l: &Licence) -> bool {
    !l.is_unknown() && !l.id.eq_ignore_ascii_case("proprietary") && !l.id.eq_ignore_ascii_case("noassertion") && !l.id.eq_ignore_ascii_case("other")
}

fn merge(into: &mut Entry, from: Entry) {
    let take_latest = newer(&from.latest, &into.latest);
    for s in from.sources {
        if !into.sources.iter().any(|x| canonical_url(&x.url) == canonical_url(&s.url)) {
            into.sources.push(s);
        }
    }
    for a in from.authors {
        if !into.authors.iter().any(|x| slug(x) == slug(&a)) {
            into.authors.push(a);
        }
    }
    for t in from.tags {
        if !into.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            into.tags.push(t);
        }
    }
    // A download known by URL is updated in place (it may have gained a hash or size); a new one is added.
    for d in from.downloads {
        match into.downloads.iter_mut().find(|x| canonical_url(&x.url) == canonical_url(&d.url)) {
            Some(x) => {
                x.blake3 = x.blake3.take().or(d.blake3);
                x.size = x.size.or(d.size);
                x.version = x.version.take().or(d.version);
                x.date = x.date.take().or(d.date);
            }
            None => into.downloads.push(d),
        }
    }
    if take_latest {
        into.latest = from.latest;
    }
    if into.summary.is_empty() {
        into.summary = from.summary;
    }
    if into.love_version.is_none() {
        into.love_version = from.love_version;
    }
    if (into.licence.is_unknown() && !from.licence.is_unknown()) || (!into.licence.confirmed && from.licence.confirmed) {
        into.licence = from.licence;
    }
    into.archived = into.archived && from.archived;
    into.checked = into.checked.clone().max(from.checked);
    // The newest downloads first: by version, then by date.
    into.downloads.sort_by(|a, b| {
        let by_version = match (&a.version, &b.version) {
            (Some(x), Some(y)) => compare_versions(y, x).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        };
        by_version.then_with(|| b.date.cmp(&a.date))
    });
}

// ---- GitHub -------------------------------------------------------------------------------------

fn platform_of(name: &str) -> Platform {
    let n = name.to_lowercase();
    if n.ends_with(".love") {
        Platform::Love
    } else if n.ends_with(".apk") || n.ends_with(".aab") {
        Platform::Android
    } else if n.ends_with(".ipa") {
        Platform::Ios
    } else if n.ends_with(".dmg") || n.contains("macos") || n.contains("osx") || n.contains("mac.") || n.contains("-mac") || n.contains("_mac") {
        Platform::Mac
    } else if n.ends_with(".appimage") || n.ends_with(".deb") || n.ends_with(".rpm") || n.contains("linux") {
        Platform::Linux
    } else if n.ends_with(".exe") || n.ends_with(".msi") || n.contains("win") {
        Platform::Windows
    } else if n.contains("portmaster") {
        Platform::Portmaster
    } else if n.contains("web") || n.contains("html5") {
        Platform::Web
    } else {
        Platform::Other
    }
}

fn day(s: Option<&str>) -> Option<String> {
    s.map(|d| d.chars().take(10).collect())
}

/// An entry from GitHub's JSON for a repository (`gh api repos/OWNER/REPO`)
/// and, if there is one, its latest release
/// (`gh api repos/OWNER/REPO/releases/latest`). `kind` is the catalogue's
/// call (a repository cannot say game or app); `today` stamps `checked`.
pub fn from_github(repo: &serde_json::Value, release: Option<&serde_json::Value>, kind: Kind, today: &str) -> Option<Entry> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let name = s(repo, "name")?;
    let url = s(repo, "html_url")?;
    // GitHub says `NOASSERTION` for a licence it could not match and `null` for none at all.
    let licence = match repo.pointer("/license/spdx_id").and_then(|x| x.as_str()) {
        Some(id) if id != "NOASSERTION" => Licence { id: id.to_string(), from: Some(format!("{url} (GitHub's licence detection)")), confirmed: false },
        _ => Licence::default(),
    };
    let mut downloads = Vec::new();
    let mut latest = None;
    if let Some(r) = release {
        let tag = s(r, "tag_name").unwrap_or_default();
        let date = day(r.get("published_at").and_then(|x| x.as_str()));
        if !tag.is_empty() && r.get("draft").and_then(|x| x.as_bool()) != Some(true) {
            latest = Some(Latest { version: tag.clone(), date: date.clone() });
            for a in r.get("assets").and_then(|a| a.as_array()).into_iter().flatten() {
                let (Some(n), Some(u)) = (s(a, "name"), s(a, "browser_download_url")) else { continue };
                downloads.push(Download { url: u, platform: platform_of(&n), version: Some(tag.clone()), date: date.clone(), size: a.get("size").and_then(|x| x.as_u64()), blake3: None });
            }
        }
    }
    Some(Entry {
        id: slug(&name),
        name,
        kind,
        summary: s(repo, "description").unwrap_or_default(),
        authors: repo.pointer("/owner/login").and_then(|x| x.as_str()).map(|o| vec![o.to_string()]).unwrap_or_default(),
        tags: repo.get("topics").and_then(|t| t.as_array()).map(|t| t.iter().filter_map(|x| x.as_str().map(str::to_string)).filter(|t| !matches!(t.as_str(), "love2d" | "love" | "lua" | "game" | "games")).collect()).unwrap_or_default(),
        licence,
        love_version: None,
        latest,
        sources: vec![Source { kind: SourceKind::Github, url }],
        downloads,
        archived: repo.get("archived").and_then(|x| x.as_bool()).unwrap_or(false),
        checked: Some(today.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn game(name: &str, src: &str, ver: Option<&str>) -> Entry {
        Entry { id: slug(name), name: name.into(), sources: vec![Source { kind: SourceKind::Github, url: src.into() }], latest: ver.map(|v| Latest { version: v.into(), date: None }), ..Default::default() }
    }

    #[test]
    fn urls_reduce_to_the_page_they_name() {
        for u in ["https://github.com/Foo/Bar", "http://www.github.com/foo/bar/", "https://github.com/foo/bar.git", "https://github.com/foo/bar/releases/tag/v1?x=1#y"] {
            assert_eq!(canonical_url(u), "github.com/foo/bar", "{u}");
        }
        assert_eq!(canonical_url("https://someone.itch.io/pong/"), "someone.itch.io/pong");
        assert_eq!(slug("The Quick-Brown Fox!"), "the-quick-brown-fox");
    }

    #[test]
    fn versions_compare_by_number_not_text() {
        assert_eq!(compare_versions("v1.10", "v1.9"), Some(Ordering::Greater));
        assert_eq!(compare_versions("1.0", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("2021-05-01", "2020-01-01"), Some(Ordering::Greater));
        assert_eq!(compare_versions("final", "beta"), None);
    }

    #[test]
    fn the_same_game_from_two_places_is_one_entry_with_the_newest_version() {
        let mut c = Catalogue::default();
        let mut a = game("Mari0", "https://github.com/x/mari0", Some("v1.0"));
        a.downloads.push(Download { url: "https://github.com/x/mari0/releases/download/v1.0/m.love".into(), platform: Platform::Love, version: Some("v1.0".into()), date: None, size: None, blake3: None });
        assert_eq!(c.add(a), Added::New("mari0".into()));
        // An itch page for it, found later, carrying the same download and a newer version.
        let mut b = game("Mari0 (itch)", "https://x.itch.io/mari0", Some("v1.2"));
        b.sources.push(Source { kind: SourceKind::Github, url: "https://www.github.com/X/Mari0.git".into() });
        b.licence = Licence { id: "MIT".into(), from: Some("itch page".into()), confirmed: false };
        b.downloads.push(Download { url: "https://x.itch.io/mari0/download/1".into(), platform: Platform::Windows, version: Some("v1.2".into()), date: None, size: Some(9), blake3: None });
        assert_eq!(c.add(b), Added::Merged("mari0".into()));
        assert_eq!(c.entries.len(), 1);
        let e = &c.entries[0];
        assert_eq!((e.name.as_str(), e.latest.as_ref().unwrap().version.as_str(), e.licence.id.as_str()), ("Mari0", "v1.2", "MIT"));
        assert_eq!(e.sources.len(), 2);
        assert_eq!(e.downloads[0].version.as_deref(), Some("v1.2"), "newest download first");
        // An older version arriving later does not roll the latest back.
        c.add(game("Mari0", "https://github.com/x/mari0", Some("v0.9")));
        assert_eq!(c.entries[0].latest.as_ref().unwrap().version, "v1.2");
    }

    #[test]
    fn the_same_file_under_two_urls_is_one_game() {
        let mut c = Catalogue::default();
        let dl = |u: &str| Download { url: u.into(), platform: Platform::Love, version: None, date: None, size: None, blake3: Some("abc123".into()) };
        let mut a = game("Alpha", "https://a.example/alpha", None);
        a.downloads.push(dl("https://a.example/alpha.love"));
        let mut b = game("Completely Different Name", "https://b.example/zzz", None);
        b.downloads.push(dl("https://mirror.example/alpha.love"));
        c.add(a);
        assert!(matches!(c.add(b), Added::Merged(_)));
        assert_eq!(c.entries.len(), 1);
        assert_eq!(c.entries[0].downloads.len(), 2, "both URLs kept");
    }

    #[test]
    fn two_games_with_one_name_stay_apart_and_are_flagged() {
        let mut c = Catalogue::default();
        let mut a = game("Pong", "https://github.com/a/pong", None);
        a.authors = vec!["alice".into()];
        let mut b = game("Pong", "https://github.com/b/pong", None);
        b.authors = vec!["bob".into()];
        c.add(a);
        assert_eq!(c.add(b), Added::New("pong-2".into()));
        assert_eq!(c.lookalikes, vec![Lookalike { id: "pong".into(), other: "pong-2".into() }]);
        // The same name and the same author is the same game, even from a new place.
        let mut again = game("Pong", "https://alice.itch.io/pong", None);
        again.authors = vec!["Alice".into()];
        assert_eq!(c.add(again), Added::Merged("pong".into()));
    }

    #[test]
    fn dedupe_merges_what_became_the_same_after_adding_sources() {
        let mut c = Catalogue::default();
        c.entries.push(game("One", "https://github.com/a/one", Some("1.0")));
        c.entries.push(game("Uno", "https://x.example/uno", Some("2.0")));
        c.entries.push(game("Other", "https://github.com/a/other", None));
        c.entries[1].sources.push(Source { kind: SourceKind::Github, url: "https://github.com/a/one".into() });
        assert_eq!(c.dedupe(), 1);
        assert_eq!(c.entries.len(), 2);
        let one = c.entries.iter().find(|e| e.name == "One").unwrap();
        assert_eq!((one.sources.len(), one.latest.as_ref().unwrap().version.as_str()), (2, "2.0"));
    }

    #[test]
    fn a_confirmed_licence_beats_a_guess_and_a_known_one_beats_unknown() {
        let mut c = Catalogue::default();
        let mut a = game("G", "https://github.com/a/g", None);
        a.licence = Licence { id: "GPL-3.0".into(), from: None, confirmed: false };
        c.add(a);
        let mut b = game("G", "https://github.com/a/g", None);
        b.licence = Licence { id: "MIT".into(), from: None, confirmed: true };
        c.add(b);
        assert_eq!(c.entries[0].licence.id, "MIT");
        c.add(game("G", "https://github.com/a/g", None));
        assert_eq!(c.entries[0].licence.id, "MIT", "unknown never replaces");
    }

    #[test]
    fn github_json_becomes_an_entry() {
        let repo = json!({"name":"mari0","html_url":"https://github.com/Stabyourself/mari0","description":"Mario with portals","owner":{"login":"Stabyourself"},
            "license":{"spdx_id":"NOASSERTION"},"topics":["love2d","platformer","portals"],"archived":false});
        let rel = json!({"tag_name":"v1.6","published_at":"2024-03-02T10:00:00Z","draft":false,"assets":[
            {"name":"mari0.love","browser_download_url":"https://github.com/Stabyourself/mari0/releases/download/v1.6/mari0.love","size":1000},
            {"name":"mari0-win64.zip","browser_download_url":"https://example/w.zip","size":5},
            {"name":"mari0.apk","browser_download_url":"https://example/a.apk","size":6}]});
        let e = from_github(&repo, Some(&rel), Kind::Game, "2026-11-14").unwrap();
        assert!(e.licence.is_unknown(), "NOASSERTION is not a licence");
        assert_eq!(e.tags, ["platformer", "portals"]);
        assert_eq!(e.latest, Some(Latest { version: "v1.6".into(), date: Some("2024-03-02".into()) }));
        let p: Vec<Platform> = e.downloads.iter().map(|d| d.platform).collect();
        assert_eq!(p, [Platform::Love, Platform::Windows, Platform::Android]);
        let mut c = Catalogue::default();
        c.add(e);
        assert!(c.entries[0].downloads.iter().all(|d| d.version.as_deref() == Some("v1.6")));
        // A draft release is not a release.
        let draft = json!({"tag_name":"v9","draft":true,"assets":[]});
        assert!(from_github(&repo, Some(&draft), Kind::Game, "d").unwrap().latest.is_none());
    }

    #[test]
    fn search_filters_by_kind_platform_and_licence() {
        let mut c = Catalogue::default();
        let mut a = game("Sky Race", "https://github.com/a/sky", None);
        a.licence = Licence { id: "MIT".into(), from: None, confirmed: true };
        a.downloads.push(Download { url: "https://x/a.apk".into(), platform: Platform::Android, version: None, date: None, size: None, blake3: None });
        let mut b = game("Level Editor", "https://github.com/a/ed", None);
        b.kind = Kind::App;
        b.licence = Licence { id: "proprietary".into(), from: None, confirmed: false };
        let mut old = game("Old Game", "https://github.com/a/old", None);
        old.archived = true;
        for e in [a, b, old] {
            c.add(e);
        }
        let f = Filter::default();
        assert_eq!(c.search("", &f).len(), 2, "archived are left out");
        assert_eq!(c.search("sky", &f)[0].name, "Sky Race");
        assert_eq!(c.search("", &Filter { kind: Some(Kind::App), ..Default::default() })[0].name, "Level Editor");
        assert_eq!(c.search("", &Filter { platform: Some(Platform::Android), ..Default::default() }).len(), 1);
        assert_eq!(c.search("", &Filter { open_source: true, ..Default::default() }).len(), 1);
        assert_eq!(c.search("", &Filter { include_archived: true, ..Default::default() }).len(), 3);
    }

    #[test]
    fn toml_round_trips_and_the_bundled_file_is_valid() {
        let mut c = Catalogue { generated: "2026-11-14".into(), ..Default::default() };
        let mut e = game("G", "https://github.com/a/g", Some("v1"));
        e.downloads.push(Download { url: "https://x/g.love".into(), platform: Platform::Love, version: None, date: None, size: Some(3), blake3: None });
        c.add(e);
        let back = Catalogue::parse(&c.to_toml()).unwrap();
        assert_eq!(back.entries, c.entries);
        let b = Catalogue::bundled();
        let mut ids: Vec<&str> = b.entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), b.entries.len(), "ids are unique");
    }
}
