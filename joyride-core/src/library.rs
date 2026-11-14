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
/// What the Benchmark Libraries sample measured, merged over the list (`scripts/ingest-benchmark.py` writes it from the sample's reports).
const BENCHMARKS: &str = include_str!("../data/library-benchmarks.json");
/// One record per audited pinned commit (`scripts/audit-libraries.py --stamp`).
#[cfg(test)]
const AUDITS: &str = include_str!("../data/library-audits.toml");
/// Libraries the maintainer excluded for good (`data/library-exclusions.toml`).
pub const EXCLUSIONS: &str = include_str!("../data/library-exclusions.toml");

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

/// How a library is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Lua only: installing it is all there is to do.
    Lua,
    /// Lua that loads a C shared library through LuaJIT's FFI.
    Ffi,
    /// A compiled C module loaded with `require` (found through `package.cpath`).
    Native,
}

impl Kind {
    fn parse(s: Option<&str>) -> Kind {
        match s {
            Some("ffi") => Kind::Ffi,
            Some("native") => Kind::Native,
            _ => Kind::Lua,
        }
    }

    /// The label the library manager shows.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Lua => "Lua",
            Kind::Ffi => "C library (FFI)",
            Kind::Native => "C module",
        }
    }

    /// Needs a binary that is not Lua: desktop only, never the web build (no FFI, no native modules).
    pub fn is_native(self) -> bool {
        self != Kind::Lua
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
    pub kind: Kind,
    /// For a C library: the binary that has to sit next to the game, and where it comes from.
    pub native: Option<String>,
    /// A warning to read before adding it (a development tool that opens a port, say).
    pub caution: Option<String>,
    /// One line of what the benchmark found: its scores against the libraries it was compared with, its cost, what it got wrong.
    pub benchmark: Option<String>,
    /// One of the person's own (added from a project's offer): not audited, not measured.
    pub custom: bool,
    /// For one of the person's own: the folder name of the project it was found in.
    pub found_in: Option<String>,
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
    kind: Option<String>,
    native: Option<String>,
    caution: Option<String>,
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
        // A curated entry always has its commit. One of the person's own may have only a tag, or only an address (its default branch).
        let rev = (!self.rev.is_empty()).then(|| self.rev.clone());
        let tag = if rev.is_none() { self.tag.clone() } else { None };
        Dependency { git: self.git.clone(), tag, rev, branch: None }
    }

    /// Whether adding it to a project has anything to fetch: a library the person's project had vendored with no address cannot be.
    pub fn installable(&self) -> bool {
        !self.git.is_empty()
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
        let mut lib = Library::parse(BUNDLED).expect("the bundled libraries.toml is valid (a test checks it)");
        lib.apply_benchmarks(BENCHMARKS).expect("the bundled library-benchmarks.json is valid (a test checks it)");
        lib
    }

    /// Merges benchmark results over the list: the measured way a library loads replaces "not tested", and the one-line summary is kept.
    pub fn apply_benchmarks(&mut self, json: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Raw {
            libraries: std::collections::BTreeMap<String, RawBench>,
        }
        #[derive(Deserialize)]
        struct RawBench {
            #[serde(default)]
            benchmark: String,
            loads_11_5: Option<String>,
            loads_12_0: Option<String>,
        }
        let raw: Raw = serde_json::from_str(json).map_err(|e| crate::Error::Msg(format!("library-benchmarks.json: {e}")))?;
        for e in &mut self.entries {
            if let Some(b) = raw.libraries.get(&e.name) {
                if !b.benchmark.is_empty() {
                    e.benchmark = Some(b.benchmark.clone());
                }
                // A compiled library's load is never claimed from a run (the panel says what binary it needs).
                if !e.kind.is_native() {
                    if let Some(l) = &b.loads_11_5 {
                        e.loads_11_5 = Badge::parse(l);
                    }
                    if let Some(l) = &b.loads_12_0 {
                        e.loads_12_0 = Badge::parse(l);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Library> {
        let raw: RawFile = toml::from_str(text).map_err(|e| crate::Error::Msg(format!("libraries.toml: {e}")))?;
        let entries = raw
            .library
            .into_iter()
            .map(|r| Entry { name: r.name, git: r.git, rev: r.rev, tag: r.tag, licence: r.licence, module: r.module, summary: r.summary, kind: Kind::parse(r.kind.as_deref()), native: r.native, caution: r.caution, benchmark: None, custom: false, found_in: None, loads_11_5: Badge::parse(&r.loads_11_5), loads_12_0: Badge::parse(&r.loads_12_0) })
            .collect();
        Ok(Library { entries })
    }

    /// The list with the person's own libraries after the curated ones. A name the curated list has is never replaced.
    pub fn with_custom(mut self, custom: &[crate::detect::Custom]) -> Library {
        for c in custom {
            if self.entries.iter().any(|e| e.name.eq_ignore_ascii_case(&c.name)) {
                continue;
            }
            self.entries.push(Entry {
                name: c.name.clone(),
                git: c.git.clone().unwrap_or_default(),
                rev: c.rev.clone().unwrap_or_default(),
                tag: c.tag.clone(),
                licence: c.licence.clone(),
                module: c.module.clone(),
                summary: c.summary.clone(),
                kind: Kind::Lua,
                native: None,
                caution: Some(format!("Added from {}. Not audited, not measured: it is not part of JoyrIDE's curated list. Read its code before you run it.", c.found_in)),
                benchmark: None,
                custom: true,
                found_in: Some(c.found_in.clone()),
                loads_11_5: Badge::Unmeasured,
                loads_12_0: Badge::Unmeasured,
            });
        }
        self
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
                let hay = format!("{} {} {} {} {}", e.name, e.summary, e.kind.label(), e.native.as_deref().unwrap_or(""), e.caution.as_deref().unwrap_or("")).to_lowercase();
                words.iter().all(|w| hay.contains(w)) && v.is_none_or(|v| !e.known_broken_on(v))
            })
            .collect()
    }

    /// Adds `name` to the manifest, pinned to the curated commit. A library
    /// already listed is left as the person has it.
    pub fn add_to(&self, m: &mut Manifest, name: &str) -> Result<()> {
        let e = self.get(name).ok_or_else(|| crate::Error::Msg(format!("`{name}` is not in the list")))?;
        if !e.installable() {
            return err(format!("{} has no source address to fetch from (it was found vendored in {}): add its git address to love.toml by hand", e.name, e.found_in.as_deref().unwrap_or("a project")));
        }
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
            // The library lands in libs/<name>/ and `require` reads dots as folder separators, so a dot in the name
            // (sock.lua) makes it impossible to require.
            assert!(!e.name.contains('.'), "{}: a dot in the name cannot be required", e.name);
            assert!(manifest::valid_dep_name(&e.name) && manifest::valid_git_url(&e.git), "{}", e.name);
            assert!(e.git.starts_with("https://github.com/") || e.git.starts_with("https://codeberg.org/"), "{}", e.name);
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

    /// scripts/audit-libraries.py treats a library's git submodules as not installed, because the installer
    /// only clones and checks out. If that ever changes, submodule code would be installed unaudited.
    #[test]
    fn the_installer_never_fetches_submodules() {
        let pkg = include_str!("pkg.rs");
        assert!(!pkg.contains("recurse") && !pkg.contains("submodule"), "pkg.rs now touches submodules: audit them first");
    }

    /// The rule: a library is audited (security and leaks, scripts/audit-libraries.py) and its findings reviewed
    /// before it is probed or committed, and again whenever its pinned commit changes.
    #[test]
    fn every_pinned_commit_has_a_reviewed_audit() {
        #[derive(Deserialize)]
        struct Audit {
            name: String,
            rev: String,
            reviewed: String,
        }
        #[derive(Deserialize)]
        struct Audits {
            #[serde(default)]
            audit: Vec<Audit>,
        }
        let audits: Audits = toml::from_str(AUDITS).expect("library-audits.toml is valid");
        for e in &Library::bundled().entries {
            let a = audits.audit.iter().find(|a| a.name == e.name && a.rev == e.rev);
            assert!(a.is_some(), "{} at {} has no audit: run `python3 -I scripts/audit-libraries.py --stamp \"<what you reviewed>\" {}`", e.name, e.rev, e.name);
            assert!(a.unwrap().reviewed.trim().len() >= 12, "{}: the audit says nothing about what was reviewed", e.name);
        }
    }

    /// An excluded library is never in the curated list, by name or by git address.
    #[test]
    fn excluded_libraries_are_not_in_the_list() {
        #[derive(Deserialize)]
        struct Ex {
            name: String,
            git: String,
            decided: String,
        }
        #[derive(Deserialize)]
        struct Exs {
            #[serde(default)]
            exclude: Vec<Ex>,
        }
        let ex: Exs = toml::from_str(EXCLUSIONS).expect("library-exclusions.toml is valid");
        assert!(!ex.exclude.is_empty());
        let norm = |g: &str| g.trim_end_matches('/').trim_end_matches(".git").to_lowercase();
        for x in &ex.exclude {
            assert!(!x.decided.is_empty());
            for e in &Library::bundled().entries {
                assert!(!e.name.eq_ignore_ascii_case(&x.name) && norm(&e.git) != norm(&x.git), "{} is excluded (library-exclusions.toml) and must not be curated", x.name);
            }
        }
    }

    #[test]
    fn c_libraries_say_what_binary_they_need() {
        let lib = Library::bundled();
        let native: Vec<_> = lib.entries.iter().filter(|e| e.kind.is_native()).collect();
        assert!(native.len() >= 6, "{} C libraries", native.len());
        for e in &native {
            assert!(e.native.as_deref().is_some_and(|n| n.len() > 20), "{} does not say what it needs", e.name);
        }
        // Pure Lua entries have no such note, and the old pins did not move.
        assert!(lib.entries.iter().filter(|e| !e.kind.is_native()).all(|e| e.native.is_none()));
        assert_eq!(lib.get("bump").unwrap().kind, Kind::Lua);
        assert_eq!(lib.get("nativefs").unwrap().kind, Kind::Ffi);
        assert_eq!(lib.get("luasteam").unwrap().kind, Kind::Native);
        // A Codeberg library installs like a GitHub one: its URL passes the manifest validator and pins an exact commit.
        let c = lib.get("cimgui-love").unwrap();
        assert!(c.git.starts_with("https://codeberg.org/") && manifest::valid_git_url(&c.git));
        assert_eq!(c.tag.as_deref(), Some("1.92-0-1"));
        // Found by what they are and what they need.
        assert!(lib.search("ffi", None).iter().any(|e| e.name == "nativefs"));
        assert!(lib.search("libcurl", None).iter().any(|e| e.name == "luajit-request"));
        assert!(lib.search("steam", None).iter().any(|e| e.name == "luasteam"));
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
    fn every_library_has_benchmark_results_and_they_change_what_the_panel_says() {
        let lib = Library::bundled();
        for e in &lib.entries {
            assert!(e.benchmark.is_some(), "{} has no benchmark line: run scripts/ingest-benchmark.py on a Benchmark Libraries report", e.name);
        }
        // Measured on a window: classic is the best of its four, and says so.
        assert!(lib.get("classic").unwrap().benchmark.as_ref().unwrap().contains("classes"), "classic's line names its comparison");
        // lurker does not load on LÖVE 12.0 (it calls a function LÖVE removed): the panel refuses it there and not on 11.5.
        let lurker = lib.get("lurker").unwrap();
        assert!(lurker.known_broken_on(v("12.0")) && !lurker.known_broken_on(v("11.5")));
        // A run is merged over the list, and a library the results do not name is left as it was.
        let mut l = Library::bundled();
        l.apply_benchmarks(r#"{"libraries": {"hump": {"benchmark": "x", "loads_12_0": "fails: nope"}}}"#).unwrap();
        assert_eq!(l.get("hump").unwrap().benchmark.as_deref(), Some("x"));
        assert!(l.get("hump").unwrap().known_broken_on(v("12.0")));
        assert_eq!(l.get("bump").unwrap().benchmark, lib.get("bump").unwrap().benchmark);
        assert!(l.apply_benchmarks("not json").is_err());
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
