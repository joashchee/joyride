//! The IO Checker sample project (`samples/io-checker`): it ships with JoyrIDE, detects the platform it runs on, checks that
//! platform's input, screen, sound and storage, and loads every asset type LÖVE supports (so it exercises the Asset Library).
//!
//! * Its assets match the library's record (`.joyride/assets.toml`): every file listed, every hash current, every licence confirmed.
//! * Run headless on the real LÖVE 11.5 and 12.0 (`love <folder> --report`) it loads every asset it should, refuses the 12.0-only
//!   WebP and QOI on 11.5 and loads them on 12.0, and has no FAIL.
//! * Built for each Joyride target by the real pipeline (shim and all), it recognises the target.
//! * `love <folder> --selftest` drives every screen against a mock of LÖVE's drawing (no GPU) and passes.
//!
//! Not tested here: drawing on a real GPU, sound out of a real speaker, a real controller, a phone or a browser. Those need a person.
//! LÖVE comes from the same places as in `e2e_love.rs`; a version that is not found is skipped.

use joyride_core::pipeline::{self, Build};
use joyride_core::scan::Target;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/io-checker")
}

fn love_binaries() -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    for (ver, var, app) in [("11.5", "JOYRIDE_LOVE_11_5", "love-11.5a.app"), ("12.0", "JOYRIDE_LOVE_12_0", "love-12.0.app")] {
        let sibling = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../r3i6nos/vendor/love-mac/{ver}/love.app/Contents/MacOS/love"));
        let candidates = [std::env::var(var).ok().map(PathBuf::from), Some(sibling), Some(PathBuf::from(format!("/Applications/{app}/Contents/MacOS/love")))];
        if let Some(p) = candidates.into_iter().flatten().find(|p| p.exists()) {
            out.push((ver, p));
        }
    }
    out
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type().unwrap().is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// Runs LÖVE on a folder or `.love` with arguments, headless, with a throwaway HOME. Returns (exit code, stdout, stderr).
fn run(love: &Path, game: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let home = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(love);
    cmd.arg(game).args(args).env("HOME", home.path()).env("SDL_VIDEODRIVER", "dummy").env("SDL_AUDIODRIVER", "dummy").env("ALSOFT_DRIVERS", "null");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output().expect("could not start love");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into())
}

/// FAIL lines of a report, with the section each is in.
fn fails(report: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for l in report.lines() {
        if let Some(s) = l.strip_prefix("== ").and_then(|s| s.strip_suffix(" ==")) {
            section = s.to_string();
        } else if l.starts_with("FAIL") {
            out.push((section.clone(), l.to_string()));
        }
    }
    out
}

/// Whether LÖVE can write its save folder here (a sandboxed build machine may forbid it; then the storage section cannot pass).
fn save_folder_writable(report: &str) -> bool {
    let Some(dir) = report.lines().find_map(|l| l.strip_prefix("INFO save folder: ")) else { return false };
    std::fs::create_dir_all(dir).is_ok() && std::fs::write(Path::new(dir).join(".probe"), "x").is_ok()
}

#[test]
fn every_asset_matches_the_library_record() {
    let root = sample();
    let toml = std::fs::read_to_string(root.join(".joyride/assets.toml")).expect("run asset-core's sample_assets_toml example");
    let mut listed = std::collections::BTreeSet::new();
    let (mut path, mut hash) = (String::new(), String::new());
    for l in toml.lines() {
        if let Some(v) = l.strip_prefix("path = ") {
            path = v.trim_matches('"').into();
        } else if let Some(v) = l.strip_prefix("hash = ") {
            hash = v.trim_matches('"').into();
        } else if let Some(v) = l.strip_prefix("confirmed = ") {
            assert_eq!(v, "true", "{path}: licence not confirmed");
            let now = asset_core::dedup::hash_file(&root.join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(now, hash, "{path} changed since .joyride/assets.toml was written (rerun tools/make-assets.mjs and sample_assets_toml)");
            listed.insert(path.clone());
        } else if l.starts_with("licence = ") {
            assert!(!l.contains("unknown"), "{path}: unknown licence");
        }
    }
    let mut on_disk = std::collections::BTreeSet::new();
    for e in walkdir::WalkDir::new(root.join("assets")).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        on_disk.insert(e.path().strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
    }
    assert_eq!(on_disk, listed, "files under assets/ and the library record disagree");
    assert!(listed.len() >= 47, "the sample should hold every asset type");
}

#[test]
fn the_project_is_lint_clean_on_both_loves() {
    use joyride_core::{lint, toolchain::LoveVersion};
    let root = sample();
    let mut n = 0;
    for e in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "lua") && !e.path().to_string_lossy().contains("/.joyride/")) {
        let text = std::fs::read_to_string(e.path()).unwrap();
        for v in ["11.5", "12.0"] {
            let d = lint::lint(&text, &e.path().display().to_string(), LoveVersion::parse(v).unwrap());
            assert!(d.is_empty(), "{} at {v}: {:?}", e.path().display(), d.iter().map(|x| format!("{}: {}", x.line, x.message)).collect::<Vec<_>>());
        }
        n += 1;
    }
    assert!(n >= 20, "only {n} Lua files");
}

#[test]
fn every_kind_of_asset_the_library_tracks_is_present() {
    // The point of the sample: the library classifies each file, and together they cover every kind LÖVE uses.
    let root = sample();
    let mut kinds = std::collections::BTreeSet::new();
    for e in walkdir::WalkDir::new(root.join("assets")).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        kinds.insert(asset_core::kind::classify(e.path()).as_str());
    }
    for want in ["image", "audio", "font", "shader", "data", "video", "script", "archive"] {
        assert!(kinds.contains(want), "no {want} asset in the sample (has {kinds:?})");
    }
}

#[test]
fn the_report_runs_on_each_real_love_and_loads_every_asset() {
    for (ver, love) in love_binaries() {
        let (code, out, err) = run(&love, &sample(), &["--report"], &[]);
        assert!(out.contains("# IO Checker device report"), "LÖVE {ver}: no report\n{out}\n{err}");
        assert!(out.contains(&format!("# runtime: LÖVE {ver}")), "LÖVE {ver}: {out}");
        if cfg!(target_os = "macos") {
            assert!(out.contains("# platform: macOS (detected, certain)"), "LÖVE {ver}: platform\n{out}");
        }
        let writable = save_folder_writable(&out);
        let bad: Vec<_> = fails(&out).into_iter().filter(|(s, _)| writable || s != "storage").collect();
        assert!(bad.is_empty(), "LÖVE {ver}: {bad:#?}");
        if writable {
            assert_eq!(code, 0, "LÖVE {ver} exited {code}\n{out}");
        }
        // 12.0-only formats: refused on 11.5 (and that is a PASS), loaded on 12.0.
        for f in ["hero.webp", "hero.qoi"] {
            let line = out.lines().find(|l| l.contains(f)).unwrap_or_else(|| panic!("no line for {f}"));
            assert!(line.starts_with("PASS"), "LÖVE {ver}: {line}");
            assert_eq!(line.contains("refused, as expected"), ver == "11.5", "LÖVE {ver}: {line}");
        }
        // Every type loaded; only the shader (needs a window) is skipped.
        let skipped: Vec<_> = out.lines().filter(|l| l.starts_with("SKIP assets/")).collect();
        assert_eq!(skipped.len(), 1, "LÖVE {ver}: {skipped:?}");
        assert!(skipped[0].contains("tint.glsl"));
        let loaded = out.lines().filter(|l| l.starts_with("PASS assets/")).count();
        assert!(loaded >= 44, "LÖVE {ver}: only {loaded} assets loaded");
    }
}

#[test]
fn every_screen_works_under_the_selftest() {
    for (ver, love) in love_binaries() {
        let (code, out, err) = run(&love, &sample(), &["--selftest"], &[]);
        assert!(out.contains("selftest:"), "LÖVE {ver}: no summary\n{out}\n{err}");
        let writable = {
            let (_, rep, _) = run(&love, &sample(), &["--report"], &[]);
            save_folder_writable(&rep)
        };
        // Without a writable save folder only the two storage-backed checks may fail.
        let failed: Vec<_> = out.lines().filter(|l| l.starts_with("FAIL")).collect();
        if writable {
            assert_eq!(code, 0, "LÖVE {ver}\n{out}");
        } else {
            assert!(failed.iter().all(|l| l.contains("storage") || l.contains("report was written")), "LÖVE {ver}: {failed:?}");
        }
    }
}

#[test]
fn the_plain_luajit_tests_pass() {
    let Some(luajit) = ["/opt/homebrew/bin/luajit", "/usr/local/bin/luajit", "/usr/bin/luajit"].iter().map(PathBuf::from).find(|p| p.exists()) else { return };
    let o = Command::new(luajit).arg("tests/unit.lua").current_dir(sample()).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success() && out.contains("failed"), "{out}{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains(", 0 failed"), "{out}");
}

#[test]
fn each_joyride_target_is_recognised_under_its_own_shim() {
    let Some((ver, love)) = love_binaries().into_iter().next() else { return };
    for (target, shim, env, want) in [
        (Target::PortMaster, "portmaster", vec![("CFW_NAME", "muOS"), ("DEVICE_NAME", "RG35XX")], "# platform: PortMaster (detected,"),
        (Target::Web, "web", vec![], "# platform: Web (love.js) (detected,"),
        // On a desktop the mobile build is a simulated phone (the window is not phone-sized when headless, so iPhone).
        (Target::Mobile, "mobile", vec![], "# platform: iOS (iPhone) (detected, simulated)"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let project = d.path().join("io-checker");
        copy_dir(&sample(), &project);
        let out_dir = d.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let love_file = pipeline::build_love(&Build { root: &project, out_dir: &out_dir, gate: None, compat: None }, target).unwrap_or_else(|e| panic!("{shim}: {e}"));
        let (_, out, err) = run(&love, &love_file, &["--report"], &env);
        assert!(out.contains(want), "LÖVE {ver}, {shim}: wanted `{want}`\n{out}\n{err}");
        if target == Target::Web {
            // The web shim removes io.popen and ffi; the web profile does not expect them, so those are PASS, not FAIL.
            assert!(out.contains("PASS Run a program (io.popen)") || out.contains("popen"), "{out}");
            assert!(!out.lines().any(|l| l.starts_with("FAIL ffi") || l.starts_with("FAIL Run a program")), "{out}");
        }
        // Everything still loads in the staged build (the shim wraps main.lua; the assets are the same).
        assert!(out.lines().filter(|l| l.starts_with("PASS assets/")).count() >= 44, "LÖVE {ver}, {shim}\n{out}");
    }
}
