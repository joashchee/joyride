//! End to end with real LÖVE 11.5 and 12.0: scan, generate overlays, stage,
//! zip, and run the staged game headless. Headless means no window or graphics
//! module (as in r3i6nos's smoke test): LÖVE here has no GL under SDL's dummy
//! driver, so what this proves is the wrapper chain, the conf patch, the
//! web stubs and that each LÖVE version loads the `.love`. What the shim does
//! to drawing and input is tested in `shim_lua.rs` with a mocked `love`.
//!
//! LÖVE comes from `JOYRIDE_LOVE_11_5` / `JOYRIDE_LOVE_12_0`, or r3i6nos's
//! `vendor/love-mac/<version>/love.app` beside this repository, or
//! /Applications. A version that isn't found is skipped.

use joyride_core::{export, overlay, scan};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

/// Runs a `.love` headless, killed after 20 s. Returns (exit ok, stdout + stderr).
fn run(love: &Path, game: &Path) -> (bool, String) {
    let mut child = Command::new(love)
        .arg(game)
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_AUDIODRIVER", "dummy")
        .env("ALSOFT_DRIVERS", "null")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("could not start love");
    let start = Instant::now();
    loop {
        if let Some(st) = child.try_wait().unwrap() {
            let o = child.wait_with_output().unwrap();
            return (st.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)));
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let o = child.wait_with_output().unwrap();
            return (false, format!("TIMEOUT {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

const CONF: &str = r#"
function love.conf(t)
  t.identity = "from-the-game"
  t.window.width = 320
  t.window.height = 240
  for _, m in ipairs({ "window", "graphics", "audio", "joystick", "video", "touch", "mouse", "keyboard", "sound" }) do t.modules[m] = false end
end
"#;

const MAIN: &str = r#"
function love.load()
  local jr = require("_jr_shim")
  print("E2E", love.filesystem.getIdentity(), jr.target, tostring(jr.headless), tostring(love.filesystem.getInfo("_orig_main.lua") ~= nil), tostring(jr.dirty))
  love.event.quit(0)
end
"#;

fn staged_game(d: &Path, target: scan::Target, version: &str) -> PathBuf {
    let root = d.join("game");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.lua"), MAIN).unwrap();
    std::fs::write(root.join("conf.lua"), CONF).unwrap();
    let s = scan::scan_project(&root);
    assert_eq!((s.window.width, s.window.height), (Some(320.0), Some(240.0)));
    let o = overlay::generate(target, &s);
    let dir = overlay::write(&root, &o).unwrap();
    // A hand-written conf override, as a person would put in patch.toml.
    let mut pt: overlay::PatchToml = toml::from_str(&std::fs::read_to_string(dir.join("patch.toml")).unwrap()).unwrap();
    pt.conf.insert("identity".into(), toml::Value::String("patched".into()));
    std::fs::write(dir.join("patch.toml"), toml::to_string_pretty(&pt).unwrap()).unwrap();
    let mut conf = BTreeMap::new();
    conf.insert("version".to_string(), toml::Value::String(version.into()));
    let stage = d.join(format!("stage-{version}"));
    overlay::stage(&root, target, &stage, &conf).unwrap();
    let love = d.join(format!("game-{version}.love"));
    export::love_archive(&stage, &love).unwrap();
    love
}

#[test]
fn staged_builds_load_and_run_on_each_real_love() {
    for (ver, love) in love_binaries() {
        for target in [scan::Target::PortMaster, scan::Target::Mobile, scan::Target::Web] {
            let d = tempfile::tempdir().unwrap();
            let game = staged_game(d.path(), target, ver);
            let (ok, out) = run(&love, &game);
            assert!(ok, "LÖVE {ver}, {target:?}: failed\n{out}");
            let line = out.lines().find(|l| l.starts_with("E2E")).unwrap_or_else(|| panic!("LÖVE {ver}, {target:?}: no result line\n{out}"));
            let name = match target {
                scan::Target::PortMaster => "portmaster",
                scan::Target::Mobile => "mobile",
                scan::Target::Web => "web",
            };
            // identity patched in the copy; the shim loaded headless; the original main kept.
            let want_dirty = if target == scan::Target::Web { "false" } else { "nil" };
            assert_eq!(line, format!("E2E\tpatched\t{name}\t{}\ttrue\t{want_dirty}", if target == scan::Target::Web { "nil" } else { "true" }), "LÖVE {ver}, {target:?}");
            assert!(!out.contains("compatible"), "LÖVE {ver} warns the build is for another version:\n{out}");
        }
    }
}

#[test]
fn a_build_pinned_to_the_other_version_is_flagged_by_each_love() {
    // Measured on the real binaries (2026-11-13): either LÖVE prints "This
    // game indicates it was made for version 'X' ... may not be compatible"
    // when conf's t.version is the other one. The pipeline pins the
    // project's own version so a correct build never shows it.
    let bins = love_binaries();
    let Some((_, love)) = bins.iter().find(|(v, _)| *v == "11.5") else { return };
    let d = tempfile::tempdir().unwrap();
    let (ok, out) = run(love, &staged_game(d.path(), scan::Target::Web, "12.0"));
    assert!(ok && out.contains("may not be compatible"), "{out}");
    if let Some((_, love12)) = bins.iter().find(|(v, _)| *v == "12.0") {
        let d = tempfile::tempdir().unwrap();
        let (ok, out) = run(love12, &staged_game(d.path(), scan::Target::Web, "11.5"));
        assert!(ok && out.contains("may not be compatible"), "{out}");
    }
}

/// Which of `names` exist (as functions) in this LÖVE, headless: modules that
/// can't load here (graphics...) are reported as `None`.
fn present(love: &Path, names: &[&str]) -> BTreeMap<String, Option<bool>> {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("conf.lua"), "function love.conf(t) t.identity='probe' for _, m in ipairs({'window','graphics','video','joystick','touch','mouse','keyboard','audio'}) do t.modules[m] = false end end").unwrap();
    let list = names.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(",");
    std::fs::write(
        d.path().join("main.lua"),
        format!(
            "local names = {{{list}}}\nfunction love.load()\n  for _, n in ipairs(names) do\n    local mod = n:match('^love%.(%w+)%.')\n    local t = love\n    local loaded = (mod == nil) or love[mod] ~= nil\n    for part in n:gmatch('[^.]+') do if part ~= 'love' then t = type(t) == 'table' and t[part] or nil end end\n    print('PROBE', n, loaded and tostring(type(t) == 'function') or 'unloaded')\n  end\n  love.event.quit(0)\nend\n"
        ),
    )
    .unwrap();
    let (ok, out) = run(love, d.path());
    assert!(ok, "{out}");
    out.lines()
        .filter_map(|l| l.strip_prefix("PROBE\t"))
        .map(|l| {
            let (n, v) = l.split_once('\t').unwrap();
            (n.to_string(), match v { "true" => Some(true), "false" => Some(false), _ => None })
        })
        .collect()
}

#[test]
fn the_lint_tables_match_the_real_love_versions() {
    use joyride_core::lint::{ONLY_IN_12, REMOVED_IN_12};
    let bins = love_binaries();
    let (Some((_, l11)), Some((_, l12))) = (bins.iter().find(|(v, _)| *v == "11.5"), bins.iter().find(|(v, _)| *v == "12.0")) else { return };
    let all: Vec<&str> = REMOVED_IN_12.iter().chain(ONLY_IN_12.iter()).copied().collect();
    let (p11, p12) = (present(l11, &all), present(l12, &all));
    let mut checked = 0;
    for n in REMOVED_IN_12 {
        assert_eq!((p11[*n], p12[*n]), (Some(true), Some(false)), "{n} should be in 11.5 and gone in 12.0");
        checked += 1;
    }
    for n in ONLY_IN_12 {
        if p11[*n].is_none() || p12[*n].is_none() {
            continue; // its module needs a window: not measurable here (love.graphics.newTexture)
        }
        assert_eq!((p11[*n], p12[*n]), (Some(false), Some(true)), "{n} should be new in 12.0");
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} names were measurable");
}

/// The same 12.0 game, run natively on real LÖVE 12.0 and as its 11.5 variant
/// on real LÖVE 11.5, must behave the same for everything the variant claims
/// to support exactly or reduced.
const COMPAT_GAME: &str = r##"
local function show(...)
  local t = {}
  for i = 1, select("#", ...) do t[#t + 1] = tostring((select(i, ...))):gsub("0x%x+", "ADDR") end
  print("C", table.concat(t, "|"))
end
function love.load()
  local f, e = love.filesystem.openFile("conf.lua", "r")
  show("open r", type(f), e, f and f:isOpen(), f and f:getMode(), f and #f:read())
  if f then f:close() end
  show("open missing", love.filesystem.openFile("nope.txt", "r"))
  show("open nomode", pcall(love.filesystem.openFile, "conf.lua"))
  show("open badmode", pcall(love.filesystem.openFile, "conf.lua", "x"))
  love.event.clear(); love.event.restart(); love.event.pump()
  local got = {}
  for n, a in love.event.poll() do got[#got + 1] = tostring(n) .. ":" .. tostring(a) end
  show("restart", table.concat(got, ","))
  show("sensor", love.sensor.hasSensor("accelerometer"), love.sensor.isEnabled("gyroscope"))
  show("sensor setEnabled", pcall(love.sensor.setEnabled, "accelerometer", true))
  show("sensor getData", pcall(love.sensor.getData, "accelerometer"))
  show("sensor getName", pcall(love.sensor.getName, "gyroscope"))
  show("sensor bad", pcall(love.sensor.hasSensor, "nonsense"))
  show("deprecate ok", pcall(love.markDeprecated, 2, "x", "function", "renamed", "y"))
  show("deprecate bad type", pcall(love.markDeprecated, 2, "x", "gizmo", "renamed", "y"))
  show("deprecate bad kind", pcall(love.markDeprecated, 2, "x", "function", "old", "y"))
  love.event.quit(0)
end
"##;

#[test]
fn the_11_5_variant_behaves_like_the_12_0_game_on_the_real_binaries() {
    use joyride_core::pipeline::{build_love, Build, Compat};
    let bins = love_binaries();
    let (Some((_, l11)), Some((_, l12))) = (bins.iter().find(|(v, _)| *v == "11.5"), bins.iter().find(|(v, _)| *v == "12.0")) else { return };
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("game");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.lua"), COMPAT_GAME).unwrap();
    std::fs::write(root.join("conf.lua"), CONF).unwrap();
    std::fs::write(root.join("love.toml"), "[project]\nname='g'\nbundle_id='dev.x.g'\nversion='1'\nlove_version='12.0'\n").unwrap();
    let native = build_love(&Build { root: &root, out_dir: &d.path().join("n"), gate: None, compat: None }, scan::Target::Web).unwrap();
    let variant = build_love(&Build { root: &root, out_dir: &d.path().join("v"), gate: None, compat: Some(Compat { allow: &[] }) }, scan::Target::Web).unwrap();
    let (ok12, out12) = run(l12, &native);
    let (ok11, out11) = run(l11, &variant);
    assert!(ok12 && ok11, "12.0 native:\n{out12}\n11.5 variant:\n{out11}");
    let lines = |o: &str| o.lines().filter(|l| l.starts_with("C\t")).map(String::from).collect::<Vec<_>>();
    let (a, b) = (lines(&out12), lines(&out11));
    assert!(a.len() >= 13, "{out12}");
    // Per-line: identical text, except LÖVE's own error positions which include no file here.
    assert_eq!(a, b, "\n12.0 native:\n{out12}\n11.5 variant:\n{out11}");
    assert!(!out11.contains("compatible"), "the variant pins 11.5, so 11.5 must not complain:\n{out11}");
}
