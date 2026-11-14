//! The Benchmark Libraries sample project (`samples/benchmark-libraries`): it loads every library JoyrIDE ships, compares the ones that do
//! the same job (features, speed, cost; rated), and writes a report for the platform it runs on.
//!
//! * Its `libs/` hold exactly the curated libraries (`data/libraries.toml`), each at its pinned commit, and `love.toml` and `love.lock`
//!   say the same: the sample never drifts from the list it benchmarks.
//! * Run headless on the real LÖVE 11.5 and 12.0 (`love <folder> --report`) every library that should load does, nothing FAILs, and the
//!   findings that are about the libraries themselves are in the report.
//! * Built for each JoyrIDE target by the real pipeline (shim and all), it recognises the target and skips what that target cannot run.
//! * `love <folder> --selftest` drives every screen against mocks (no GPU) and passes; `tests/unit.lua` passes under plain LuaJIT.
//!
//! Not tested here: drawing on a real GPU, the windowed comparison (the immediate-mode GUIs) and the specialists that need a window,
//! a controller or a phone. LÖVE comes from the same places as in `e2e_love.rs`; a version that is not found is skipped.

use joyride_core::library::Library;
use joyride_core::manifest::{Lock, Manifest};
use joyride_core::pipeline::{self, Build};
use joyride_core::scan::Target;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/benchmark-libraries")
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

fn lines_with<'a>(report: &'a str, status: &str) -> Vec<&'a str> {
    report.lines().filter(|l| l.starts_with(&format!("{status} "))).collect()
}

#[test]
fn libs_are_exactly_the_curated_list_at_its_pinned_commits() {
    let root = sample();
    let lib = Library::bundled();
    let manifest = Manifest::parse(&std::fs::read_to_string(root.join("love.toml")).unwrap()).unwrap();
    let lock = Lock::parse(&std::fs::read_to_string(root.join("love.lock")).unwrap()).unwrap();
    assert!(manifest.problems().is_empty(), "{:?}", manifest.problems());
    assert_eq!(manifest.project.bundle_id, "dev.joyride.benchmarklibraries");
    assert_eq!(manifest.dependencies.len(), lib.entries.len(), "love.toml and the curated list differ in size");
    assert_eq!(lock.packages.len(), lib.entries.len());
    for e in &lib.entries {
        let dep = manifest.dependencies.get(&e.name).unwrap_or_else(|| panic!("{} is not in love.toml", e.name));
        assert_eq!(dep, &e.dependency(), "{}: love.toml differs from the curated pin", e.name);
        let locked = lock.get(&e.name).unwrap_or_else(|| panic!("{} is not in love.lock", e.name));
        assert_eq!(locked.commit, e.rev, "{}: locked at another commit", e.name);
        let dir = root.join("libs").join(&e.name);
        assert!(dir.is_dir(), "libs/{} is missing (run tools/vendor-libs.py)", e.name);
        let mark = std::fs::read_to_string(dir.join(".joyride-commit")).unwrap_or_default();
        assert_eq!(mark.trim(), e.rev, "libs/{} is at another commit", e.name);
        // The licence travels with the code: a licence file, or (moonshine has none) the licence section of its README.
        let has_file = std::fs::read_dir(&dir).unwrap().flatten().any(|f| {
            let n = f.file_name().to_string_lossy().to_lowercase();
            n.contains("licen") || n.starts_with("copying")
        });
        let in_readme = std::fs::read_to_string(dir.join("README.md")).unwrap_or_default().contains("## License");
        // hump has neither: its licence text heads every source file.
        let in_source = walkdir::WalkDir::new(&dir).into_iter().flatten().any(|f| f.path().extension().is_some_and(|x| x == "lua") && std::fs::read_to_string(f.path()).unwrap_or_default().contains("Permission is hereby granted"));
        assert!(has_file || in_readme || in_source, "libs/{} carries no licence", e.name);
    }
    let folders: Vec<String> = std::fs::read_dir(root.join("libs")).unwrap().flatten().map(|f| f.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(folders.len(), lib.entries.len(), "libs/ holds folders that are not curated libraries: {folders:?}");
    // The catalogue the Lua side reads is the one the curated list generates from.
    let catalog = std::fs::read_to_string(root.join("lib/catalog.lua")).unwrap();
    for e in &lib.entries {
        assert!(catalog.contains(&format!("name = \"{}\"", e.name)) && catalog.contains(&e.rev), "lib/catalog.lua is out of date for {}", e.name);
    }
}

#[test]
fn nothing_compiled_and_no_sdk_is_vendored() {
    // luasteam's repository carries Valve's Steamworks SDK under Valve's own agreement: it must never be copied into a sample.
    let mut bad = Vec::new();
    for e in walkdir::WalkDir::new(sample().join("libs")).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let ext = e.path().extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ["dll", "so", "dylib", "a", "lib", "exe", "o", "framework"].contains(&ext.as_str()) || name.contains("steam_api") || e.path().to_string_lossy().contains("/sdk/") {
            bad.push(e.path().display().to_string());
        }
    }
    assert!(bad.is_empty(), "compiled or SDK files in libs/: {bad:?}");
}

#[test]
fn the_project_files_are_lint_clean_on_both_loves() {
    use joyride_core::{lint, toolchain::LoveVersion};
    let root = sample();
    let mut n = 0;
    for e in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()).filter(|e| {
        let p = e.path().to_string_lossy();
        e.path().extension().is_some_and(|x| x == "lua") && !p.contains("/libs/") && !p.contains("/.joyride/")
    }) {
        let text = std::fs::read_to_string(e.path()).unwrap();
        for v in ["11.5", "12.0"] {
            let d = lint::lint(&text, &e.path().display().to_string(), LoveVersion::parse(v).unwrap());
            assert!(d.is_empty(), "{} at {v}: {:?}", e.path().display(), d.iter().map(|x| format!("{}: {}", x.line, x.message)).collect::<Vec<_>>());
        }
        n += 1;
    }
    assert!(n >= 40, "only {n} Lua files");
}

#[test]
fn the_report_runs_on_each_real_love_with_no_fail() {
    let lib = Library::bundled();
    for (ver, love) in love_binaries() {
        let (code, out, err) = run(&love, &sample(), &["--report"], &[]);
        assert!(out.contains("# Benchmark Libraries report"), "LÖVE {ver}: no report\n{out}\n{err}");
        assert!(out.contains(&format!("# runtime: LÖVE {ver}")), "LÖVE {ver}: {out}");
        if cfg!(target_os = "macos") {
            assert!(out.contains("# platform: macOS (detected, certain)"), "LÖVE {ver}: platform");
        }
        assert!(lines_with(&out, "FAIL").is_empty(), "LÖVE {ver}: {:#?}", lines_with(&out, "FAIL"));
        assert_eq!(code, 0, "LÖVE {ver} exited {code}\n{err}");
        // Every curated library has a line in the libraries section, and the pure-Lua ones that need no window loaded.
        let section = out.split("== libraries ==").nth(1).and_then(|s| s.split("== compare").next()).expect("a libraries section");
        for e in &lib.entries {
            let line = section.lines().find(|l| l.split(' ').nth(1).map(|n| n.trim_end_matches(':')) == Some(e.name.as_str())).unwrap_or_else(|| panic!("LÖVE {ver}: no line for {}", e.name));
            let loaded = line.starts_with("PASS");
            let needs_window = ["g3d", "push", "talkies"].contains(&e.name.as_str());
            if e.kind == joyride_core::library::Kind::Lua && !needs_window && !(ver == "12.0" && e.name == "lurker") {
                assert!(loaded, "LÖVE {ver}: {} should load: {line}", e.name);
            }
        }
        // The compiled parts, when tools/build-binaries.sh has been run: nuklear and discord-rpc load, and discord-rpc starts and stops.
        if sample().join("bin/nuklear.so").exists() {
            assert!(out.lines().any(|l| l.starts_with("PASS love-nuklear:")), "LÖVE {ver}: nuklear should load\n{out}");
            assert!(out.lines().any(|l| l.starts_with("PASS lua-discordRPC:")), "LÖVE {ver}: lua-discordRPC should load");
            assert!(out.lines().any(|l| l.starts_with("PASS lua-discordRPC: Discord Rich Presence:")), "LÖVE {ver}: discord-rpc should start and stop");
        }
        // luasteam needs Valve's SDK, which is never built here.
        assert!(out.lines().any(|l| l.starts_with("SKIP luasteam:") && l.contains("Steamworks SDK")), "LÖVE {ver}: luasteam");
        // lurker really does not load on 12.0 (it calls a function LÖVE removed): a finding, shown as INFO and not as a FAIL.
        if ver == "12.0" {
            assert!(out.lines().any(|l| l.starts_with("INFO lurker:") && l.contains("known on this LÖVE")), "LÖVE 12.0: lurker's incompatibility should be reported as a finding");
        }
        // Every group has a section, a score line, and rated cells.
        for g in ["classes", "tweens", "timers", "events", "states", "vectors", "collections", "maths", "strings", "serialise", "printing", "collision", "pathfinding", "cameras", "testing", "profiling", "gui"] {
            assert!(out.contains(&format!("== compare {g} ==")), "LÖVE {ver}: no section for {g}");
            assert!(out.lines().any(|l| l.starts_with(&format!("INFO {g} overall "))), "LÖVE {ver}: no scores for {g}");
        }
        // Findings that are about the libraries, pinned: they would change if a library were updated.
        for (key, word) in [
            ("tweens/nested tween", "sibling"),
            ("serialise/cycle json", "circular"),
            ("collision/ray HC", "heading left or up"),
            ("classes/isa hump.class", "no such feature"),
            ("states/args batteries.state_machine", "no such feature"),
            ("printing/meta batteries.pretty", "metatable"),
        ] {
            let line = out.lines().find(|l| l.contains(&format!(" {key}:"))).unwrap_or_else(|| panic!("LÖVE {ver}: no line for {key}"));
            assert!(line.starts_with("INFO") && line.contains(word), "LÖVE {ver}: {line}");
        }
        // Speed rows carry a measurement.
        assert!(out.lines().any(|l| l.starts_with("PASS classes/new-speed classic:") && l.contains("objects/s")), "LÖVE {ver}: no measurement");
        // The comparisons that need a window are SKIP without one, and the GUI group is not given a best by its costs alone.
        assert!(out.lines().any(|l| l.starts_with("SKIP gui/frame suit:")), "LÖVE {ver}");
        assert!(out.lines().any(|l| l.starts_with("INFO gui overall suit: not rated here")), "LÖVE {ver}");
    }
}

#[test]
fn every_screen_works_under_the_selftest() {
    for (ver, love) in love_binaries() {
        let (code, out, err) = run(&love, &sample(), &["--selftest"], &[]);
        assert!(out.contains("selftest:"), "LÖVE {ver}: no summary\n{out}\n{err}");
        assert!(!out.lines().any(|l| l.starts_with("FAIL")), "LÖVE {ver}: {out}");
        assert_eq!(code, 0, "LÖVE {ver}\n{out}");
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
fn each_joyride_target_is_recognised_and_skips_what_it_cannot_run() {
    let Some((ver, love)) = love_binaries().into_iter().next() else { return };
    for (target, shim, env, want) in [
        (Target::PortMaster, "portmaster", vec![("CFW_NAME", "muOS"), ("DEVICE_NAME", "RG35XX")], "# platform: PortMaster (detected,"),
        (Target::Web, "web", vec![], "# platform: Web (love.js) (detected,"),
        (Target::Mobile, "mobile", vec![], "# platform: iOS (iPhone) (detected, simulated)"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let project = d.path().join("benchmark-libraries");
        copy_dir(&sample(), &project);
        let out_dir = d.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let love_file = pipeline::build_love(&Build { root: &project, out_dir: &out_dir, gate: None, compat: None }, target).unwrap_or_else(|e| panic!("{shim}: {e}"));
        let (_, out, err) = run(&love, &love_file, &["--report"], &env);
        assert!(out.contains(want), "LÖVE {ver}, {shim}: wanted `{want}`\n{out}\n{err}");
        assert!(lines_with(&out, "FAIL").is_empty(), "LÖVE {ver}, {shim}: {:#?}", lines_with(&out, "FAIL"));
        if target == Target::Web {
            // The web profile has no FFI: the libraries that use it are SKIP, not FAIL.
            for name in ["nativefs", "luajit-request"] {
                assert!(out.lines().any(|l| l.starts_with(&format!("SKIP {name}:")) && l.contains("no FFI")), "LÖVE {ver}, web: {name}\n{out}");
            }
        }
        // The libraries are in the staged build and still load (the shim wraps main.lua; libs/ is the same).
        assert!(lines_with(&out, "PASS").iter().filter(|l| l.contains(" lines; ")).count() >= 25, "LÖVE {ver}, {shim}\n{out}");
    }
}
