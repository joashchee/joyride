//! Builds data/libraries.toml from data/library-seeds.toml: clones each
//! library, pins a real commit (the newest version tag if there is one) and
//! reads its licence from the repository.
//!
//!   cargo run --example curate_libraries            read only (needs github.com)
//!   cargo run --example curate_libraries -- --probe also load each library
//!   cargo run --example curate_libraries -- --only a,b   redo just these, keep every other pin as it is
//!   cargo run --example curate_libraries -- --probe --pinned --only a,b
//!                                                       probe the commit already pinned (the audited one)
//!                                                       and only update the load results, never re-pin
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
            // "v1.2.3", "1.2.3" and release names like "1.92-0-1" (dots or dashes between numbers).
            let n: Vec<u64> = t.trim_start_matches('v').split(['.', '-']).map(|p| p.parse().ok()).collect::<Option<_>>()?;
            (!n.is_empty()).then(|| (n, t.to_string()))
        })
        .collect();
    v.sort();
    v.pop().map(|(_, t)| t)
}

/// Requires `module` from `repo` on this LÖVE, headless (runs the library's
/// code). Returns "loads", "needs-display" or "fails: <message>".
fn probe(love: &Path, repo: &Path, name: &str, module: &str, others: &[String]) -> String {
    let d = tempfile::tempdir().unwrap();
    // The repository appears as `<dir>/<name>`, as it does in a project's libs/ folder, so a library whose
    // init.lua sits at its root (`require "batteries"`) loads; the repo itself stays on the path for flat ones.
    let mount = d.path().join("mount");
    std::fs::create_dir_all(&mount).unwrap();
    std::os::unix::fs::symlink(repo, mount.join(name)).unwrap();
    std::fs::write(d.path().join("conf.lua"), "function love.conf(t) t.identity='curate' for _, m in ipairs({'window','graphics','video','joystick','touch','mouse','keyboard','audio','sound'}) do t.modules[m] = false end end").unwrap();
    std::fs::write(
        d.path().join("main.lua"),
        format!(
            "package.path = {mt:?} .. '/?.lua;' .. {mt:?} .. '/?/init.lua;' .. {p:?} .. '/?.lua;' .. {p:?} .. '/?/init.lua;' .. {p:?} .. '/lib/?.lua;' .. package.path\nfunction love.load()\n  local ok, err = pcall(require, {m:?})\n  print('CURATE', ok and 'loads' or ('fails: ' .. tostring(err):gsub('\\n.*', '')))\n  love.event.quit(0)\nend\n",
            p = repo.display().to_string(),
            mt = mount.display().to_string(),
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
        // It needs another curated library (lurker needs lume), which this probe did not mount: not a failure.
        Some(m) if others.iter().any(|o| o != name && m.contains(&format!("module '{o}' not found"))) => "unmeasured".into(),
        Some(m) => m.replace('"', "'"),
        None => format!("fails: no result ({})", out.lines().next().unwrap_or("")),
    }
}

fn main() {
    let seeds: toml::Value = toml::from_str(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/library-seeds.toml")).unwrap()).unwrap();
    let probing = std::env::args().any(|a| a == "--probe");
    let args: Vec<String> = std::env::args().collect();
    let only: Option<Vec<String>> = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)).map(|v| v.split(',').map(|n| n.to_lowercase()).collect());
    let pinned = args.iter().any(|a| a == "--pinned");
    if pinned && only.is_none() {
        eprintln!("--pinned needs --only name,name");
        std::process::exit(2);
    }
    let existing = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/libraries.toml")).unwrap_or_default();
    let all_names: Vec<String> = seeds["library"].as_array().unwrap().iter().filter_map(|l| l["name"].as_str().map(String::from)).collect();
    let mut fresh: Vec<(String, String)> = Vec::new();
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
        if only.as_ref().is_some_and(|o| !o.contains(&name.to_lowercase())) {
            continue;
        }
        let kind = lib.get("kind").and_then(|k| k.as_str()).unwrap_or("lua");
        let native = lib.get("native").and_then(|k| k.as_str());
        let caution = lib.get("caution").and_then(|k| k.as_str());
        let dir = work.path().join(name);
        eprint!("{name}: ");
        if let Err(e) = git(&["clone", "--quiet", url, dir.to_str().unwrap()], None) {
            eprintln!("clone failed: {e}");
            continue;
        }
        // `pin = "head"`: the project keeps fixing things without tagging, so a tag would be years old.
        // --pinned: the commit already in libraries.toml (audited), never a newer one.
        let pin_line = |key: &str| -> Option<String> {
            let blk = existing.split("\n[[library]]\n").find(|b| b.lines().any(|l| l == format!("name = {name:?}")))?;
            blk.lines().find_map(|l| l.strip_prefix(&format!("{key} = "))).map(|v| v.trim_matches('"').to_string())
        };
        let (tag, rev) = if pinned {
            let Some(rev) = pin_line("rev") else {
                eprintln!("not in libraries.toml yet: run without --pinned first");
                continue;
            };
            if let Err(e) = git(&["checkout", "--quiet", "--detach", &rev], Some(&dir)) {
                eprintln!("cannot check out the pinned commit: {e}");
                continue;
            }
            (pin_line("tag"), rev)
        } else {
            // `tag = "NAME"` in a seed names the release tag when its spelling is not a version number (luaunit's `LUAUNIT_V3_5`).
            let tag = if lib.get("pin").and_then(|p| p.as_str()) == Some("head") {
                None
            } else if let Some(t) = lib.get("tag").and_then(|t| t.as_str()) {
                Some(t.to_string())
            } else {
                newest_tag(&dir)
            };
            if let Some(t) = &tag {
                let _ = git(&["checkout", "--quiet", "--detach", t], Some(&dir));
            }
            (tag, git(&["rev-parse", "HEAD"], Some(&dir)).unwrap())
        };
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
        // Native libraries cannot load without their binary, so they are never probed.
        let (a, b) = if probing && kind == "lua" { (probe(l11.as_ref().unwrap(), &dir, name, module, &all_names), probe(l12.as_ref().unwrap(), &dir, name, module, &all_names)) } else { ("unmeasured".to_string(), "unmeasured".to_string()) };
        eprintln!("{} {} {} | 11.5 {a} | 12.0 {b}", tag.as_deref().unwrap_or("(no tag)"), &rev[..7], spdx);
        let mut block = format!("\n[[library]]\nname = {name:?}\ngit = {url:?}\nrev = {rev:?}\n");
        if let Some(t) = tag {
            block.push_str(&format!("tag = {t:?}\n"));
        }
        block.push_str(&format!("licence = {spdx:?}\nmodule = {module:?}\n"));
        if kind != "lua" {
            block.push_str(&format!("kind = {kind:?}\n"));
        }
        if let Some(n) = native {
            block.push_str(&format!("native = {n:?}\n"));
        }
        if let Some(c) = caution {
            block.push_str(&format!("caution = {c:?}\n"));
        }
        block.push_str(&format!("summary = {summary:?}\nloads_11_5 = {a:?}\nloads_12_0 = {b:?}\n"));
        fresh.push((name.to_string(), block.clone()));
        out.push_str(&block);
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/libraries.toml");
    if only.is_some() {
        // Keep every other entry exactly as it is: drop the redone ones, append the fresh blocks.
        let old = std::fs::read_to_string(&path).unwrap();
        let mut parts = old.split("\n[[library]]\n");
        let head = parts.next().unwrap().to_string();
        let mut kept = head;
        for p in parts {
            let n = p.lines().find_map(|l| l.strip_prefix("name = ")).map(|v| v.trim_matches('"').to_lowercase()).unwrap_or_default();
            if !fresh.iter().any(|(f, _)| f.to_lowercase() == n) {
                kept.push_str("\n[[library]]\n");
                kept.push_str(p);
            }
        }
        out = kept;
        for (_, b) in &fresh {
            out.push_str(b);
        }
    }
    std::fs::write(path, out).unwrap();
    eprintln!("wrote data/libraries.toml");
}
