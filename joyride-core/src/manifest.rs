//! `love.toml` (spec 3D) and `love.lock`.

use crate::{err, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_LOVE_VERSION: &str = "11.5";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub project: Project,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    #[serde(default = "default_version")]
    pub love_version: String,
}

fn default_version() -> String {
    DEFAULT_LOVE_VERSION.to_string()
}

/// A library from git, pinned by at most one of `tag`, `rev` or `branch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub git: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest> {
        let m: Manifest = toml::from_str(text).map_err(|e| crate::Error::Msg(format!("love.toml: {e}")))?;
        Ok(m)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("manifest serialises")
    }

    /// A new project's manifest, at the default LÖVE version.
    pub fn new_project(name: &str, bundle_id: &str) -> Manifest {
        Manifest {
            project: Project { name: name.into(), bundle_id: bundle_id.into(), version: "1.0.0".into(), love_version: default_version() },
            dependencies: BTreeMap::new(),
        }
    }

    /// Everything wrong with the manifest, in plain words; empty if fine.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.project.name.trim().is_empty() {
            p.push("project.name is empty".into());
        }
        if !valid_bundle_id(&self.project.bundle_id) {
            p.push(format!("project.bundle_id `{}` should be reverse-DNS, like dev.joyride.mygame", self.project.bundle_id));
        }
        if crate::toolchain::LoveVersion::parse(&self.project.love_version).is_none() {
            p.push(format!("project.love_version `{}` is not 11.5 or 12.0 and later", self.project.love_version));
        }
        for (name, d) in &self.dependencies {
            if !valid_dep_name(name) {
                p.push(format!("dependency name `{name}` must be letters, digits, `-`, `_` or `.`"));
            }
            if !valid_git_url(&d.git) {
                p.push(format!("dependency `{name}`: git URL `{}` is not allowed", d.git));
            }
            if [d.tag.is_some(), d.rev.is_some(), d.branch.is_some()].iter().filter(|x| **x).count() > 1 {
                p.push(format!("dependency `{name}`: give only one of tag, rev, branch"));
            }
            for r in [&d.tag, &d.rev, &d.branch].into_iter().flatten() {
                if !valid_ref(r) {
                    p.push(format!("dependency `{name}`: `{r}` is not a valid git ref"));
                }
            }
        }
        p
    }
}

pub fn valid_bundle_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !p.starts_with('-'))
}

/// A folder name under `libs/`: no separators, no dots-only names.
pub fn valid_dep_name(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// https, ssh, scp-style and file URLs. Never one starting with `-` (git
/// would read it as an option) or using a remote helper such as `ext::`.
pub fn valid_git_url(u: &str) -> bool {
    if u.is_empty() || u.starts_with('-') || u.contains("::") || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    u.starts_with("https://") || u.starts_with("ssh://") || u.starts_with("file://") || (u.starts_with("git@") && u.contains(':'))
}

pub fn valid_ref(r: &str) -> bool {
    !r.is_empty() && !r.starts_with('-') && r.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '+'))
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Lock {
    #[serde(default, rename = "package")]
    pub packages: Vec<Locked>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Locked {
    pub name: String,
    pub git: String,
    /// The pinned spec (`tag`, `rev` or `branch` and its value), so a change
    /// in `love.toml` is seen as a change.
    pub spec: String,
    /// The commit that was installed.
    pub commit: String,
}

impl Lock {
    pub fn parse(text: &str) -> Result<Lock> {
        toml::from_str(text).map_err(|e| crate::Error::Msg(format!("love.lock: {e}")))
    }
    pub fn to_toml(&self) -> String {
        format!("# Written by JoyrIDE. Commit it.\n{}", toml::to_string_pretty(self).expect("lock serialises"))
    }
    pub fn get(&self, name: &str) -> Option<&Locked> {
        self.packages.iter().find(|p| p.name == name)
    }
}

impl Dependency {
    /// `tag=v0.4`, `rev=964d299`, `branch=main` or `default`.
    pub fn spec(&self) -> String {
        match (&self.tag, &self.rev, &self.branch) {
            (Some(t), _, _) => format!("tag={t}"),
            (_, Some(r), _) => format!("rev={r}"),
            (_, _, Some(b)) => format!("branch={b}"),
            _ => "default".into(),
        }
    }
    pub fn checkout_ref(&self) -> Option<&str> {
        self.tag.as_deref().or(self.rev.as_deref()).or(self.branch.as_deref())
    }
}

pub fn require_valid(m: &Manifest) -> Result<()> {
    let p = m.problems();
    if p.is_empty() {
        Ok(())
    } else {
        err(p.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_EXAMPLE: &str = r#"
[project]
name = "my-game"
bundle_id = "dev.joyride.mygame"
version = "1.0.0"
love_version = "11.5" # Defaults to "11.5"; switchable to "12.0"

[dependencies]
hump = { git = "https://github.com/vrld/hump", tag = "v0.4" }
bump = { git = "https://github.com/kikito/bump.lua", rev = "964d299" }
"#;

    #[test]
    fn the_spec_example_parses_and_is_valid() {
        let m = Manifest::parse(SPEC_EXAMPLE).unwrap();
        assert!(m.problems().is_empty(), "{:?}", m.problems());
        assert_eq!(m.dependencies["hump"].spec(), "tag=v0.4");
        assert_eq!(Manifest::parse(&m.to_toml()).unwrap(), m);
    }

    #[test]
    fn love_version_defaults_to_11_5() {
        let m = Manifest::parse("[project]\nname=\"a\"\nbundle_id=\"dev.x.a\"\nversion=\"1\"").unwrap();
        assert_eq!(m.project.love_version, "11.5");
        assert_eq!(Manifest::new_project("a", "dev.x.a").project.love_version, "11.5");
    }

    #[test]
    fn problems_are_reported() {
        let mut m = Manifest::new_project("g", "nodots");
        m.project.love_version = "10.2".into();
        m.dependencies.insert("../evil".into(), Dependency { git: "https://x/y".into(), tag: None, rev: None, branch: None });
        m.dependencies.insert("ok".into(), Dependency { git: "--upload-pack=sh".into(), tag: Some("a".into()), rev: Some("b".into()), branch: None });
        m.dependencies.insert("ext".into(), Dependency { git: "ext::sh -c id".into(), tag: None, rev: None, branch: None });
        let p = m.problems().join("\n");
        for needle in ["bundle_id", "love_version", "../evil", "--upload-pack", "only one of", "ext::"] {
            assert!(p.contains(needle), "missing {needle} in {p}");
        }
        assert!(require_valid(&m).is_err());
    }

    #[test]
    fn lock_round_trips_and_is_found_by_name() {
        let l = Lock { packages: vec![Locked { name: "hump".into(), git: "https://x".into(), spec: "tag=v1".into(), commit: "abc".into() }] };
        let back = Lock::parse(&l.to_toml()).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.get("hump").unwrap().commit, "abc");
        assert!(back.get("nope").is_none());
    }

    #[test]
    fn git_urls() {
        assert!(valid_git_url("https://github.com/a/b"));
        assert!(valid_git_url("git@github.com:a/b.git"));
        assert!(!valid_git_url("-oProxyCommand=x"));
        assert!(!valid_git_url("http://insecure"));
        assert!(!valid_git_url("https://a b"));
    }
}
