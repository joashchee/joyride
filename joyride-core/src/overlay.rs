//! Patch overlays (spec 4A): everything a target needs lives in
//! `.joyride/patches/<target>/`, never in the project's own source. At build
//! time the project is copied to a staging folder and the overlay merged
//! into the copy; the original is not touched.
//!
//! ```text
//! .joyride/patches/<target>/
//!   patch.toml     declarative rules: design size, [conf] overrides
//!   _jr_shim.lua   generated runtime hook
//!   game.gptk      PortMaster controller map (portmaster only)
//!   overrides/     files that replace or add to the project's own
//!   diffs/*.patch  unified diffs against project files
//! ```

use crate::scan::{Scan, Target};
use crate::shim::{self, Config};
use crate::{err, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn target_dir_name(t: Target) -> &'static str {
    match t {
        Target::PortMaster => "portmaster",
        Target::Mobile => "mobile",
        Target::Web => "web",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchToml {
    pub patch: PatchInfo,
    /// `love.conf` values to set, by path: `"window.width" = 640`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conf: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub input: Input,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchInfo {
    pub target: String,
    pub design_width: u32,
    pub design_height: u32,
}

/// What the scan detected, recorded so a person can see why the shim does
/// what it does.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Input {
    pub keys: Vec<String>,
    pub mouse: bool,
    pub touch: bool,
    pub gamepad: bool,
}

#[derive(Debug, Clone)]
pub struct Overlay {
    pub target: Target,
    pub patch: PatchToml,
    pub shim: String,
    pub gptk: Option<String>,
}

/// Makes a fresh overlay from a scan.
pub fn generate(target: Target, scan: &Scan) -> Overlay {
    let c = Config::from_scan(target, scan);
    Overlay {
        target,
        patch: PatchToml {
            patch: PatchInfo { target: target_dir_name(target).into(), design_width: c.design_w, design_height: c.design_h },
            conf: BTreeMap::new(),
            input: Input { keys: scan.keys.iter().cloned().collect(), mouse: scan.mouse, touch: scan.touch, gamepad: scan.gamepad },
        },
        shim: shim::generate(&c),
        gptk: (target == Target::PortMaster).then(|| shim::gptk(&scan.keys)),
    }
}

/// Writes the overlay under `<root>/.joyride/patches/<target>/`, replacing the
/// generated files (`patch.toml` is kept if it already exists, since a person
/// may have edited it) and leaving `overrides/` and `diffs/` alone.
pub fn write(root: &Path, o: &Overlay) -> Result<PathBuf> {
    let dir = root.join(".joyride/patches").join(target_dir_name(o.target));
    fs::create_dir_all(&dir)?;
    if !dir.join("patch.toml").exists() {
        fs::write(dir.join("patch.toml"), toml::to_string_pretty(&o.patch).expect("patch.toml serialises"))?;
    }
    fs::write(dir.join("_jr_shim.lua"), &o.shim)?;
    if let Some(g) = &o.gptk {
        // A tuned controller map is the person's; only write one if none exists.
        if !dir.join("game.gptk").exists() {
            fs::write(dir.join("game.gptk"), g)?;
        }
    }
    Ok(dir)
}

#[derive(Debug, Default)]
pub struct Staged {
    pub files: usize,
    pub overridden: Vec<String>,
    pub patched: Vec<String>,
    pub conf_overrides: usize,
}

/// Skipped when copying a project: version control, builds and the overlay itself.
fn skipped(rel: &Path) -> bool {
    let first = rel.components().next().and_then(|c| c.as_os_str().to_str()).unwrap_or("");
    matches!(first, ".git" | "build" | ".joyride" | ".DS_Store") || rel.extension().is_some_and(|e| e == "love") || rel.file_name().is_some_and(|n| n == ".DS_Store")
}

/// Copies the project to `staging` (which must be empty or absent) and merges
/// the target's overlay into the copy. `extra_conf` is merged beneath the
/// overlay's own `[conf]` (the IDE sets `version` here).
pub fn stage(root: &Path, target: Target, staging: &Path, extra_conf: &BTreeMap<String, toml::Value>) -> Result<Staged> {
    stage_with(root, target, staging, extra_conf, None)
}

/// [`stage`] plus an optional `_jr_compat.lua` (see `compat`), which the
/// wrapper `main.lua` requires before anything else.
pub fn stage_with(root: &Path, target: Target, staging: &Path, extra_conf: &BTreeMap<String, toml::Value>, compat: Option<&str>) -> Result<Staged> {
    if staging.exists() && fs::read_dir(staging)?.next().is_some() {
        return err(format!("staging folder {} is not empty", staging.display()));
    }
    if !root.join("main.lua").is_file() {
        return err("the project has no main.lua");
    }
    let overlay = root.join(".joyride/patches").join(target_dir_name(target));
    let shim_path = overlay.join("_jr_shim.lua");
    if !shim_path.is_file() {
        return err(format!("no overlay for {}: generate it first", target_dir_name(target)));
    }
    let patch: PatchToml = toml::from_str(&fs::read_to_string(overlay.join("patch.toml"))?).map_err(|e| crate::Error::Msg(format!("patch.toml: {e}")))?;

    let mut report = Staged::default();
    fs::create_dir_all(staging)?;
    for e in WalkDir::new(root).sort_by_file_name().into_iter().filter_entry(|e| !skipped(e.path().strip_prefix(root).unwrap_or(e.path()))) {
        let e = e.map_err(|e| crate::Error::Msg(e.to_string()))?;
        let rel = e.path().strip_prefix(root).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let to = staging.join(rel);
        if e.file_type().is_dir() {
            fs::create_dir_all(&to)?;
        } else if e.file_type().is_file() {
            fs::copy(e.path(), &to)?;
            report.files += 1;
        }
        // Symlinks are not followed or copied: a project cannot pull files from outside itself.
    }

    // Unified diffs, in name order.
    let mut diffs: Vec<PathBuf> = fs::read_dir(overlay.join("diffs")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "patch")).collect();
    diffs.sort();
    for d in diffs {
        for (file, text) in split_patch(&fs::read_to_string(&d)?)? {
            let target_path = safe_join(staging, &file)?;
            let original = fs::read_to_string(&target_path).map_err(|_| crate::Error::Msg(format!("{}: patches {file}, which the project does not have", d.display())))?;
            fs::write(&target_path, apply_unified(&original, &text).map_err(|e| crate::Error::Msg(format!("{}: {file}: {e}", d.display())))?)?;
            report.patched.push(file);
        }
    }

    // Drop-in replacements.
    let ov = overlay.join("overrides");
    if ov.is_dir() {
        for e in WalkDir::new(&ov).sort_by_file_name().into_iter().flatten() {
            if e.file_type().is_file() {
                let rel = e.path().strip_prefix(&ov).unwrap();
                let to = staging.join(rel);
                if let Some(p) = to.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::copy(e.path(), &to)?;
                report.overridden.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    // conf.lua, in the copy only.
    let mut conf = extra_conf.clone();
    conf.extend(patch.conf.clone());
    if !conf.is_empty() {
        let base = staging.join("conf.lua");
        let had = base.is_file();
        if had {
            fs::rename(&base, staging.join("_orig_conf.lua"))?;
        }
        fs::write(&base, conf_wrapper(had, &conf)?)?;
        report.conf_overrides = conf.len();
    }

    // main.lua: the shim first, the game's own main, then install.
    fs::rename(staging.join("main.lua"), staging.join("_orig_main.lua"))?;
    fs::write(staging.join("main.lua"), shim::main_wrapper_with(target_dir_name(target), compat.is_some()))?;
    if let Some(c) = compat {
        fs::write(staging.join("_jr_compat.lua"), c)?;
    }
    fs::copy(&shim_path, staging.join("_jr_shim.lua"))?;
    Ok(report)
}

fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::Prefix(_))) {
        return err(format!("`{rel}` leaves the project"));
    }
    Ok(base.join(p))
}

fn lua_literal(v: &toml::Value) -> Result<String> {
    Ok(match v {
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::String(s) => {
            let mut o = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => o.push_str("\\\""),
                    '\\' => o.push_str("\\\\"),
                    '\n' => o.push_str("\\n"),
                    c if c.is_control() => return err("control characters in a conf string"),
                    c => o.push(c),
                }
            }
            o.push('"');
            o
        }
        _ => return err("conf values are numbers, booleans or strings"),
    })
}

fn conf_wrapper(had_original: bool, conf: &BTreeMap<String, toml::Value>) -> Result<String> {
    let mut s = String::from("-- conf.lua written by JoyrIDE; the project's own is _orig_conf.lua.\n");
    if had_original {
        s.push_str("require(\"_orig_conf\")\n");
    }
    s.push_str("local base = love.conf\nfunction love.conf(t)\n  if base then base(t) end\n");
    for (path, v) in conf {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.iter().any(|p| p.is_empty() || !p.chars().next().unwrap().is_ascii_alphabetic() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
            return err(format!("conf key `{path}` is not a plain dotted name"));
        }
        let mut prefix = String::from("t");
        for p in &parts[..parts.len() - 1] {
            prefix = format!("{prefix}.{p}");
            s.push_str(&format!("  {prefix} = {prefix} or {{}}\n"));
        }
        s.push_str(&format!("  t.{path} = {}\n", lua_literal(v)?));
    }
    s.push_str("end\n");
    Ok(s)
}

/// Splits a multi-file unified diff into (path, that file's diff text).
pub fn split_patch(text: &str) -> Result<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let p = rest.split('\t').next().unwrap_or(rest).trim();
            let p = p.strip_prefix("b/").unwrap_or(p);
            out.push((p.to_string(), String::new()));
        } else if line.starts_with("--- ") && out.last().is_none_or(|(_, t)| t.contains("@@")) {
            // start of the next file's header; its path comes from the +++ line
        } else if let Some((_, t)) = out.last_mut() {
            t.push_str(line);
            t.push('\n');
        }
    }
    if out.is_empty() {
        return err("patch names no file (no `+++` line)");
    }
    Ok(out)
}

/// Applies one file's unified-diff hunks. Every context and removed line
/// must match exactly where the hunk says: a patch that no longer fits the
/// source fails instead of guessing.
pub fn apply_unified(original: &str, diff: &str) -> Result<String> {
    let src: Vec<&str> = original.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut pos = 0usize; // next unread line of src
    let mut lines = diff.lines().peekable();
    while let Some(l) = lines.next() {
        if !l.starts_with("@@") {
            continue;
        }
        let start = l.split_whitespace().nth(1).and_then(|r| r.strip_prefix('-')).and_then(|r| r.split(',').next()).and_then(|n| n.parse::<usize>().ok()).ok_or_else(|| crate::Error::Msg(format!("bad hunk header `{l}`")))?;
        let start = start.saturating_sub(1);
        if start < pos || start > src.len() {
            return err("hunks overlap or point past the end of the file");
        }
        out.extend(src[pos..start].iter().map(|s| s.to_string()));
        pos = start;
        while let Some(h) = lines.peek() {
            if h.starts_with("@@") {
                break;
            }
            let h = lines.next().unwrap();
            match h.chars().next() {
                Some('+') => out.push(h[1..].to_string()),
                Some('-') | Some(' ') | None => {
                    let want = if h.is_empty() { "" } else { &h[1..] };
                    if src.get(pos) != Some(&want) {
                        return err(format!("line {} is `{}`, the patch expects `{want}`", pos + 1, src.get(pos).copied().unwrap_or("<end of file>")));
                    }
                    if h.starts_with(' ') || h.is_empty() {
                        out.push(want.to_string());
                    }
                    pos += 1;
                }
                Some('\\') => {}
                _ => return err(format!("unexpected line in hunk: `{h}`")),
            }
        }
    }
    out.extend(src[pos.min(src.len())..].iter().map(|s| s.to_string()));
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::scan_sources;

    fn project(root: &Path) -> Scan {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("build")).unwrap();
        fs::write(root.join("main.lua"), "function love.draw() love.graphics.print('hi') end\nfunction love.keypressed(k) if k == 'space' then end end\n").unwrap();
        fs::write(root.join("conf.lua"), "function love.conf(t)\n  t.window.width = 800\nend\n").unwrap();
        fs::write(root.join("src/a.lua"), "local x = 1\nlocal y = 2\nreturn x + y\n").unwrap();
        fs::write(root.join(".git/HEAD"), "ref").unwrap();
        fs::write(root.join("build/old.love"), "zip").unwrap();
        scan_sources(&[("main.lua".into(), fs::read_to_string(root.join("main.lua")).unwrap()), ("conf.lua".into(), fs::read_to_string(root.join("conf.lua")).unwrap())])
    }

    fn tree_hash(root: &Path) -> Vec<(String, Vec<u8>)> {
        WalkDir::new(root).sort_by_file_name().into_iter().flatten().filter(|e| e.file_type().is_file()).map(|e| (e.path().strip_prefix(root).unwrap().to_string_lossy().to_string(), fs::read(e.path()).unwrap())).collect()
    }

    #[test]
    fn staging_never_touches_the_source_and_wraps_main_and_conf() {
        let d = tempfile::tempdir().unwrap();
        let (root, st) = (d.path().join("game"), d.path().join("stage"));
        let scan = project(&root);
        write(&root, &generate(Target::PortMaster, &scan)).unwrap();
        // Hand-written conf override and a drop-in file.
        let mut pt: PatchToml = toml::from_str(&fs::read_to_string(root.join(".joyride/patches/portmaster/patch.toml")).unwrap()).unwrap();
        pt.conf.insert("window.width".into(), toml::Value::Integer(640));
        pt.conf.insert("window.title".into(), toml::Value::String("My \"Game\"".into()));
        fs::write(root.join(".joyride/patches/portmaster/patch.toml"), toml::to_string_pretty(&pt).unwrap()).unwrap();
        fs::create_dir_all(root.join(".joyride/patches/portmaster/overrides/shaders")).unwrap();
        fs::write(root.join(".joyride/patches/portmaster/overrides/shaders/lowres.glsl"), "x").unwrap();

        let before = tree_hash(&root);
        let mut extra = BTreeMap::new();
        extra.insert("version".to_string(), toml::Value::String("11.5".into()));
        let rep = stage(&root, Target::PortMaster, &st, &extra).unwrap();
        assert_eq!(before, tree_hash(&root), "the source tree changed");
        assert_eq!(rep.overridden, ["shaders/lowres.glsl"]);
        assert_eq!(rep.conf_overrides, 3);
        assert!(st.join("_orig_main.lua").is_file() && st.join("_jr_shim.lua").is_file() && st.join("_orig_conf.lua").is_file());
        assert!(fs::read_to_string(st.join("main.lua")).unwrap().contains("jr.install()"));
        let conf = fs::read_to_string(st.join("conf.lua")).unwrap();
        assert!(conf.contains("require(\"_orig_conf\")") && conf.contains("t.window.width = 640") && conf.contains("t.version = \"11.5\"") && conf.contains("t.window.title = \"My \\\"Game\\\"\""));
        assert!(!st.join(".git").exists() && !st.join("build").exists() && !st.join(".joyride").exists());
        assert!(st.join("src/a.lua").is_file());
        // The staged conf and main compile.
        if let Some(lj) = ["/opt/homebrew/bin/luajit"].iter().find(|p| Path::new(p).exists()) {
            for f in ["conf.lua", "main.lua", "_jr_shim.lua"] {
                let o = std::process::Command::new(lj).arg("-bl").arg(st.join(f)).output().unwrap();
                assert!(o.status.success(), "{f}: {}", String::from_utf8_lossy(&o.stderr));
            }
        }
    }

    #[test]
    fn a_regenerate_keeps_a_hand_edited_patch_toml_and_gptk() {
        let d = tempfile::tempdir().unwrap();
        let scan = project(d.path());
        let dir = write(d.path(), &generate(Target::PortMaster, &scan)).unwrap();
        fs::write(dir.join("game.gptk"), "a = q\n").unwrap();
        fs::write(dir.join("patch.toml"), "[patch]\ntarget = \"portmaster\"\ndesign_width = 1\ndesign_height = 1\n").unwrap();
        write(d.path(), &generate(Target::PortMaster, &scan)).unwrap();
        assert_eq!(fs::read_to_string(dir.join("game.gptk")).unwrap(), "a = q\n");
        assert!(fs::read_to_string(dir.join("patch.toml")).unwrap().contains("design_width = 1"));
    }

    #[test]
    fn unified_diffs_apply_exactly_or_fail() {
        let diff = "--- a/src/a.lua\n+++ b/src/a.lua\n@@ -1,3 +1,3 @@\n local x = 1\n-local y = 2\n+local y = 20\n return x + y\n";
        let d = tempfile::tempdir().unwrap();
        let (root, st) = (d.path().join("g"), d.path().join("s"));
        let scan = project(&root);
        let dir = write(&root, &generate(Target::Web, &scan)).unwrap();
        fs::create_dir_all(dir.join("diffs")).unwrap();
        fs::write(dir.join("diffs/01.patch"), diff).unwrap();
        let rep = stage(&root, Target::Web, &st, &BTreeMap::new()).unwrap();
        assert_eq!(rep.patched, ["src/a.lua"]);
        assert_eq!(fs::read_to_string(st.join("src/a.lua")).unwrap(), "local x = 1\nlocal y = 20\nreturn x + y\n");
        assert_eq!(fs::read_to_string(root.join("src/a.lua")).unwrap(), "local x = 1\nlocal y = 2\nreturn x + y\n");
        // A source that has moved on: refused, nothing guessed.
        fs::write(root.join("src/a.lua"), "local x = 9\nlocal y = 2\nreturn x + y\n").unwrap();
        let st2 = d.path().join("s2");
        let e = stage(&root, Target::Web, &st2, &BTreeMap::new()).unwrap_err().to_string();
        assert!(e.contains("expects"), "{e}");
    }

    #[test]
    fn diff_paths_cannot_escape_and_bad_conf_keys_are_refused() {
        assert!(safe_join(Path::new("/s"), "../x").is_err());
        assert!(safe_join(Path::new("/s"), "/etc/passwd").is_err());
        let mut bad = BTreeMap::new();
        bad.insert("window.width\nos.execute('x')".to_string(), toml::Value::Integer(1));
        assert!(conf_wrapper(false, &bad).is_err());
        let mut arr = BTreeMap::new();
        arr.insert("x".to_string(), toml::Value::Array(vec![]));
        assert!(conf_wrapper(false, &arr).is_err());
    }

    #[test]
    fn needs_main_lua_and_an_overlay() {
        let d = tempfile::tempdir().unwrap();
        assert!(stage(d.path(), Target::Web, &d.path().join("s"), &BTreeMap::new()).is_err());
        fs::write(d.path().join("main.lua"), "").unwrap();
        assert!(stage(d.path(), Target::Web, &d.path().join("s"), &BTreeMap::new()).unwrap_err().to_string().contains("no overlay"));
    }

    #[test]
    fn apply_unified_handles_insertions_and_multiple_hunks() {
        let orig = "a\nb\nc\nd\ne\nf\n";
        let diff = "@@ -1,2 +1,3 @@\n a\n+ins\n b\n@@ -5,2 +6,1 @@\n e\n-f\n";
        assert_eq!(apply_unified(orig, diff).unwrap(), "a\nins\nb\nc\nd\ne\n");
    }
}
