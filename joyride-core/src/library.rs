//! The curated libraries JoyrIDE offers by name (spec 3D): a bundled list,
//! each pinned to an exact commit, with its licence and what is known about
//! whether it loads on each LÖVE version. The list is generated
//! (`cargo run --example curate_libraries`, from `data/library-seeds.toml`)
//! and compiled in; installing one is adding a `love.toml` dependency pinned
//! to that commit.

use crate::manifest::{Dependency, Manifest};
use crate::toolchain::LoveVersion;
use crate::{err, Result};
use serde::Deserialize;

const BUNDLED: &str = include_str!("../data/libraries.toml");

/// What is known about a library on one LÖVE version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Badge {
    /// Required on the real LÖVE, headless, without error.
    Loads,
    /// Needs a window or graphics even to load, so a headless run cannot say.
    NeedsDisplay,
    /// Not run: loading a library executes it, so the test is opt-in.
    Unmeasured,
    /// Failed to load, with LÖVE's message.
    Fails(String),
}

impl Badge {
    fn parse(s: &str) -> Badge {
        match s {
            "loads" => Badge::Loads,
            "needs-display" => Badge::NeedsDisplay,
            "unmeasured" => Badge::Unmeasured,
            other => Badge::Fails(other.trim_start_matches("fails: ").to_string()),
        }
    }

    /// The label the library manager shows.
    pub fn label(&self) -> String {
        match self {
            Badge::Loads => "Loads".into(),
            Badge::NeedsDisplay => "Needs a display to test".into(),
            Badge::Unmeasured => "Not tested".into(),
            Badge::Fails(m) => format!("Does not load: {m}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub git: String,
    /// The exact commit installed.
    pub rev: String,
    /// The version it was taken from, if the repository has version tags.
    pub tag: Option<String>,
    pub licence: String,
    pub module: String,
    pub summary: String,
    loads_11_5: Badge,
    loads_12_0: Badge,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    library: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    name: String,
    git: String,
    rev: String,
    tag: Option<String>,
    licence: String,
    module: String,
    summary: String,
    loads_11_5: String,
    loads_12_0: String,
}

impl Entry {
    /// What is known about this library on `v`.
    pub fn badge(&self, v: LoveVersion) -> &Badge {
        if v.is_12_or_later() { &self.loads_12_0 } else { &self.loads_11_5 }
    }

    /// The `love.toml` dependency that installs exactly this commit.
    pub fn dependency(&self) -> Dependency {
        Dependency { git: self.git.clone(), tag: None, rev: Some(self.rev.clone()), branch: None }
    }

    /// Fails to load on `v`: should not be offered as working there.
    pub fn known_broken_on(&self, v: LoveVersion) -> bool {
        matches!(self.badge(v), Badge::Fails(_))
    }
}

pub struct Library {
    pub entries: Vec<Entry>,
}

impl Library {
    pub fn bundled() -> Library {
        Library::parse(BUNDLED).expect("the bundled libraries.toml is valid (a test checks it)")
    }

    pub fn parse(text: &str) -> Result<Library> {
        let raw: RawFile = toml::from_str(text).map_err(|e| crate::Error::Msg(format!("libraries.toml: {e}")))?;
        let entries = raw
            .library
            .into_iter()
            .map(|r| Entry { name: r.name, git: r.git, rev: r.rev, tag: r.tag, licence: r.licence, module: r.module, summary: r.summary, loads_11_5: Badge::parse(&r.loads_11_5), loads_12_0: Badge::parse(&r.loads_12_0) })
            .collect();
        Ok(Library { entries })
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Entries whose name or summary contain every word of `query`, for the
    /// search box; an empty query lists all. Libraries known not to load on
    /// `v` are left out when `v` is given.
    pub fn search(&self, query: &str, v: Option<LoveVersion>) -> Vec<&Entry> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        self.entries
            .iter()
            .filter(|e| {
                let hay = format!("{} {}", e.name, e.summary).to_lowercase();
                words.iter().all(|w| hay.contains(w)) && v.is_none_or(|v| !e.known_broken_on(v))
            })
            .collect()
    }

    /// Adds `name` to the manifest, pinned to the curated commit. A library
    /// already listed is left as the person has it.
    pub fn add_to(&self, m: &mut Manifest, name: &str) -> Result<()> {
        let e = self.get(name).ok_or_else(|| crate::Error::Msg(format!("`{name}` is not in the curated list")))?;
        let v = LoveVersion::parse(&m.project.love_version);
        if let Some(v) = v {
            if e.known_broken_on(v) {
                return err(format!("{} does not load on LÖVE {v}: {}", e.name, e.badge(v).label()));
            }
        }
        m.dependencies.entry(e.name.clone()).or_insert_with(|| e.dependency());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;

    fn v(s: &str) -> LoveVersion {
        LoveVersion::parse(s).unwrap()
    }

    #[test]
    fn the_bundled_list_is_valid_and_every_entry_installs() {
        let lib = Library::bundled();
        assert!(lib.entries.len() >= 15, "{} entries", lib.entries.len());
        let mut names = std::collections::BTreeSet::new();
        for e in &lib.entries {
            assert!(names.insert(e.name.to_lowercase()), "duplicate {}", e.name);
            assert!(manifest::valid_dep_name(&e.name) && manifest::valid_git_url(&e.git), "{}", e.name);
            assert!(e.git.starts_with("https://github.com/"), "{}", e.name);
            assert!(e.rev.len() == 40 && e.rev.chars().all(|c| c.is_ascii_hexdigit()), "{} has no exact commit", e.name);
            assert!(!e.module.is_empty() && !e.summary.is_empty());
            assert_ne!(e.licence, "unknown", "{} has no recorded licence", e.name);
            assert!(asset_core::Licence::from_spdx(&e.licence).allows_commercial_use(), "{}: {}", e.name, e.licence);
            // The dependency is exactly what the manifest validator accepts.
            let mut m = Manifest::new_project("g", "dev.x.g");
            m.dependencies.insert(e.name.clone(), e.dependency());
            assert!(m.problems().is_empty(), "{}: {:?}", e.name, m.problems());
        }
    }

    #[test]
    fn search_matches_words_in_name_and_summary() {
        let lib = Library::bundled();
        assert_eq!(lib.search("bump", None)[0].name, "bump");
        assert!(lib.search("tween", None).iter().any(|e| e.name == "flux"));
        assert!(lib.search("collision detection", None).len() >= 2);
        assert!(lib.search("zzzz-nothing", None).is_empty());
        assert_eq!(lib.search("", None).len(), lib.entries.len());
        assert!(lib.get("LUME").is_some());
    }

    #[test]
    fn badges_follow_the_version_and_parse_every_form() {
        let text = "[[library]]\nname='a'\ngit='https://github.com/x/a'\nrev='0000000000000000000000000000000000000000'\nlicence='MIT'\nmodule='a'\nsummary='s'\nloads_11_5='loads'\nloads_12_0=\"fails: boom\"\n\n[[library]]\nname='b'\ngit='https://github.com/x/b'\nrev='1111111111111111111111111111111111111111'\ntag='v1'\nlicence='MIT'\nmodule='b'\nsummary='t'\nloads_11_5='needs-display'\nloads_12_0='unmeasured'\n";
        let lib = Library::parse(text).unwrap();
        let a = lib.get("a").unwrap();
        assert_eq!(a.badge(v("11.5")), &Badge::Loads);
        assert_eq!(a.badge(v("12.0")), &Badge::Fails("boom".into()));
        assert_eq!(a.badge(v("12.0")).label(), "Does not load: boom");
        let b = lib.get("b").unwrap();
        assert_eq!((b.badge(v("11.5")).label(), b.badge(v("12.0")).label()), ("Needs a display to test".into(), "Not tested".into()));
        assert_eq!(b.tag.as_deref(), Some("v1"));
        // Known-broken libraries are hidden for that version and refused on add.
        assert_eq!(lib.search("", Some(v("12.0"))).len(), 1);
        let mut m = Manifest::new_project("g", "dev.x.g");
        m.project.love_version = "12.0".into();
        assert!(lib.add_to(&mut m, "a").unwrap_err().to_string().contains("does not load on LÖVE 12.0"));
        assert!(lib.add_to(&mut m, "nope").is_err());
    }

    #[test]
    fn adding_pins_the_exact_commit_and_keeps_a_users_own_entry() {
        let lib = Library::bundled();
        let mut m = Manifest::new_project("g", "dev.x.g");
        lib.add_to(&mut m, "bump").unwrap();
        let d = &m.dependencies["bump"];
        assert_eq!((d.rev.as_deref(), d.tag.as_ref(), d.branch.as_ref()), (Some(lib.get("bump").unwrap().rev.as_str()), None, None));
        assert!(m.problems().is_empty());
        // A hand-edited entry is not overwritten.
        m.dependencies.insert("lume".into(), Dependency { git: "https://github.com/me/lume-fork".into(), tag: Some("mine".into()), rev: None, branch: None });
        lib.add_to(&mut m, "lume").unwrap();
        assert_eq!(m.dependencies["lume"].git, "https://github.com/me/lume-fork");
    }
}
