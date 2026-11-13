//! Builds data/libraries.toml from data/library-seeds.toml: clones each
//! library, pins a real commit (the newest version tag if there is one) and
//! reads its licence from the repository.
//!
//!   cargo run --example curate_libraries            read only (needs github.com)
//!   cargo run --example curate_libraries -- --probe also load each library
//!
//! By default nothing from the cloned repositories is *run*: only their tags,
//! commit and licence files are read, and the load results are recorded as
//! `unmeasured`. Requiring a library executes its code, so the load test is
//! opt-in: `--probe` loads 17 third-party libraries into real LÖVE 11.5 and
//! 12.0 headless as your user. Run it deliberately (LÖVE comes from
//! JOYRIDE_LOVE_11_5 / JOYRIDE_LOVE_12_0, as in tests/e2e_love.rs).
//!
//! A library that needs a window or graphics to even load is recorded as
//! `needs-display`, not as failing: the headless run cannot say.

use asset_core::licence;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn love_binary(ver: &str) -> Option<PathBuf> {
    let var = format!("JOYRIDE_LOVE_{}", ver.replace('.', "_"));
    let sibling = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../r3i6nos/vendor/love-mac/{ver}/love.app/Contents/MacOS/love"));
    [std::env::var(&var).ok().map(PathBuf::from), Some(sibling), Some(PathBuf::from(format!("/Applications/love-{ver}a.app/Contents/MacOS/love"))), Some(PathBuf::from(format!("/Applications/love-{ver}.app/Contents/MacOS/love")))]
        .into_iter()
        .flatten()
        .find(|p| p.exists())
}

fn git(args: &[&str], dir: Option<&Path>) -> Result<String, String> {
    let mut c = Command::new("git");
    // No hooks, no remote helpers, no prompts: a clone must not run anything.
    c.args(["-c", "protocol.ext.allow=never", "-c", "core.hooksPath=/dev/null"]).args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(d) = dir {
        c.current_dir(d);
    }
    let o = c.output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
}

/// Newest tag that looks like a version, by version order.
fn newest_tag(repo: &Path) -> Option<String> {
    let tags = git(&["tag", "--list"], Some(repo)).ok()?;
    let mut v: Vec<(Vec<u64>, String)> = tags
        .lines()
        .filter_map(|t| {
            let n: Vec<u64> = t.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
            (!n.is_empty()).then(|| (n, t.to_string()))
        })
        .collect();
    v.sort();
    v.pop().map(|(_, t)| t)
}

/// Requires `module` from `repo` on this LÖVE, headless (runs the library's
/// code). Returns "loads", "needs-display" or "fails: <message>".
fn probe(love: &Path, repo: &Path, module: &str) -> String {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("conf.lua"), "function love.conf(t) t.identity='curate' for _, m in ipairs({'window','graphics','video','joystick','touch','mouse','keyboard','audio','sound'}) do t.modules[m] = false end end").unwrap();
    std::fs::write(
        d.path().join("main.lua"),
        format!(
            "package.path = {p:?} .. '/?.lua;' .. {p:?} .. '/?/init.lua;' .. {p:?} .. '/lib/?.lua;' .. package.path\nfunction love.load()\n  local ok, err = pcall(require, {m:?})\n  print('CURATE', ok and 'loads' or ('fails: ' .. tostring(err):gsub('\\n.*', '')))\n  love.event.quit(0)\nend\n",
            p = repo.display().to_string(),
            m = module
        ),
    )
    .unwrap();
    let mut child = Command::new(love).arg(d.path()).env("SDL_VIDEODRIVER", "dummy").env("SDL_AUDIODRIVER", "dummy").env("ALSOFT_DRIVERS", "null").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let start = Instant::now();
    let out = loop {
        if child.try_wait().unwrap().is_some() {
            let o = child.wait_with_output().unwrap();
            break format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            break String::from("TIMEOUT");
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    match out.lines().find_map(|l| l.strip_prefix("CURATE\t")) {
        Some("loads") => "loads".into(),
        Some(m) if m.contains("graphics") || m.contains("love.window") || m.contains("'window'") || m.contains("love.mouse") || m.contains("love.keyboard") || m.contains("love.joystick") => "needs-display".into(),
        Some(m) => m.replace('"', "'"),
        None => format!("fails: no result ({})", out.lines().next().unwrap_or("")),
    }
}

fn main() {
    let seeds: toml::Value = toml::from_str(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/library-seeds.toml")).unwrap()).unwrap();
    let probing = std::env::args().any(|a| a == "--probe");
    let (l11, l12) = (love_binary("11.5"), love_binary("12.0"));
    if probing && (l11.is_none() || l12.is_none()) {
        eprintln!("--probe needs real LÖVE 11.5 and 12.0 (JOYRIDE_LOVE_11_5 / JOYRIDE_LOVE_12_0)");
        std::process::exit(2);
    }
    let work = tempfile::tempdir().unwrap();
    let date = Command::new("date").arg("+%Y-%m-%d").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut out = format!("# Generated by `cargo run --example curate_libraries` from library-seeds.toml. Do not edit.\n# `rev` is the exact commit to install; `tag` is the version it was taken from, if any.\n# loads_*: `loads` (required on the real LÖVE, headless), `needs-display` (cannot load without a window: unmeasured),\n# `unmeasured` (not run: pass --probe), or `fails: ...`.\ngenerated = \"{date}\"\n");
    for lib in seeds["library"].as_array().unwrap() {
        let (name, url, module, summary) = (lib["name"].as_str().unwrap(), lib["git"].as_str().unwrap(), lib["module"].as_str().unwrap(), lib["summary"].as_str().unwrap());
        let dir = work.path().join(name);
        eprint!("{name}: ");
        if let Err(e) = git(&["clone", "--quiet", url, dir.to_str().unwrap()], None) {
            eprintln!("clone failed: {e}");
            continue;
        }
        let tag = newest_tag(&dir);
        if let Some(t) = &tag {
            let _ = git(&["checkout", "--quiet", "--detach", t], Some(&dir));
        }
        let rev = git(&["rev-parse", "HEAD"], Some(&dir)).unwrap();
        let spdx = ["LICENSE", "LICENSE.md", "LICENSE.txt", "license.txt", "COPYING", "README.md", "readme.md"]
            .iter()
            .find_map(|f| licence::guess_for(&dir.join(f), &dir).map(|g| g.spdx))
            .or_else(|| licence::guess_for(&dir.join("init.lua"), &dir).map(|g| g.spdx))
            // Some libraries keep the licence in docs/ or in a source header.
            .or_else(|| {
                let m = module.replace('.', "/");
                ["docs/license.rst", "docs/LICENSE", "init.lua", &format!("{m}.lua"), &format!("{m}/init.lua")].iter().find_map(|f| {
                    let text = std::fs::read_to_string(dir.join(f)).ok()?;
                    licence::guess_from_text(&text.chars().take(8192).collect::<String>())
                })
            })
            .unwrap_or_else(|| "unknown".into());
        let (a, b) = if probing { (probe(l11.as_ref().unwrap(), &dir, module), probe(l12.as_ref().unwrap(), &dir, module)) } else { ("unmeasured".to_string(), "unmeasured".to_string()) };
        eprintln!("{} {} {} | 11.5 {a} | 12.0 {b}", tag.as_deref().unwrap_or("(no tag)"), &rev[..7], spdx);
        out.push_str(&format!("\n[[library]]\nname = {name:?}\ngit = {url:?}\nrev = {rev:?}\n"));
        if let Some(t) = tag {
            out.push_str(&format!("tag = {t:?}\n"));
        }
        out.push_str(&format!("licence = {spdx:?}\nmodule = {module:?}\nsummary = {summary:?}\nloads_11_5 = {a:?}\nloads_12_0 = {b:?}\n"));
    }
    std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/libraries.toml"), out).unwrap();
    eprintln!("wrote data/libraries.toml");
}
