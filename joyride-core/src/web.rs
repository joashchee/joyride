//! Web (spec 5.4): a love.js page around the game.
//!
//! love.js (Davidobot's, LÖVE 11.5) packages a game with a few plain steps,
//! which are reproduced here in Rust (read from its `index.js`, 2026-11-13) so
//! no Node is needed: `game.data` is the `.love` bytes; `game.js` is the
//! template `src/game.js` with a file-list JSON filled in; `index.html` is
//! the template's with the title, the arguments (`["./game.love"]`) and the
//! memory size; `love.js`, `love.wasm`, `theme/` (and `love.worker.js` for the
//! standard version) are copied from the template. `template_src` is the
//! `src` folder of a love.js checkout.
//!
//! Two versions: the standard one uses threads and needs the page served with
//! `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy:
//! require-corp` (a `_headers` file for Netlify and Cloudflare is written); the
//! compatibility one works anywhere (itch.io) with rougher audio.
//!
//! Saved games persist: love.js's own startup code mounts IndexedDB at
//! `/home/web_user/love`, loads it before the game starts and syncs it back on
//! `beforeunload` (read from its `love.js`, 2026-11-13). It cannot sync at any
//! other time (the page has no `FS` object: only `FS_createPath` and friends are
//! exported), so a tab that dies without `beforeunload` (mobile browsers, a
//! crash) can lose the latest writes. The web shim marks writes (`jr.dirty`)
//! for a build that exports `FS`.
//!
//! Two template layouts are read: Davidobot's (`release/` and `compat/` each
//! with an `index.html` and `theme/`) and the `lua52` fork's (one shared
//! `html/index.html` and `html/theme/`, runtime files in `release/` and
//! `compat/`). The fork runs Lua 5.2 and Emscripten 6.x rather than LuaJIT
//! (`docs/lovejs-forks.md`); its engine differs, the packaging does not.

use crate::{err, Result};
use std::fs;
use std::path::Path;

/// The LÖVE versions the love.js template runs: 11.5 only (its readme, and the
/// `love.wasm` in `src/`). A 12.0 project needs its 11.5 variant for the web.
pub const WEB_LOVE_VERSIONS: &[&str] = &["11.5"];

#[derive(Debug, Clone)]
pub struct WebApp {
    pub title: String,
    /// The compatibility version: no threads, works without the two headers.
    pub compat: bool,
    /// `INITIAL_MEMORY`; `None` picks one that fits the game.
    pub memory: Option<u64>,
}

/// love.js's own default is 16 MiB and it refuses a game larger than the
/// memory. This is the game's size doubled, rounded up to a whole 16 MiB, never
/// below 16 MiB: room for the unpacked files and the engine.
pub fn memory_for(game_bytes: u64) -> u64 {
    const STEP: u64 = 16 * 1024 * 1024;
    (game_bytes * 2).div_ceil(STEP).max(1) * STEP
}

/// Mustache's HTML escaping, as `{{title}}` does it.
fn escape_html(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '/' => o.push_str("&#x2F;"),
            '`' => o.push_str("&#x60;"),
            '=' => o.push_str("&#x3D;"),
            c => o.push(c),
        }
    }
    o
}

/// Replaces every occurrence of a placeholder, failing if it is absent (the
/// template changed shape and the output would be silently wrong).
fn fill(template: &str, placeholder: &str, value: &str, what: &str) -> Result<String> {
    if !template.contains(placeholder) {
        return err(format!("the love.js {what} template has no {placeholder}: it is not the layout this exporter was written for"));
    }
    Ok(template.replace(placeholder, value))
}

/// A UUID-shaped id from the game's bytes, so the same game always gives the same page.
fn package_uuid(bytes: &[u8]) -> String {
    let h = blake3::hash(bytes).to_hex();
    format!("{}-{}-4{}-a{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
}

/// Writes the web page for `game_love` into `out`.
pub fn package(template_src: &Path, game_love: &Path, out: &Path, app: &WebApp) -> Result<()> {
    let flavour = if app.compat { "compat" } else { "release" };
    let dir = template_src.join(flavour);
    // The page and theme live beside the runtime, or (lua52 fork) shared in html/.
    let page = if dir.join("index.html").exists() { dir.clone() } else { template_src.join("html") };
    let mut need = vec![page.join("index.html"), dir.join("love.js"), dir.join("love.wasm"), page.join("theme"), template_src.join("game.js")];
    if !app.compat {
        need.push(dir.join("love.worker.js"));
    }
    for n in &need {
        if !n.exists() {
            return err(format!("{} is not a love.js `src` folder: no {}", template_src.display(), n.strip_prefix(template_src).unwrap_or(n).display()));
        }
    }
    let game = fs::read(game_love)?;
    let memory = app.memory.unwrap_or_else(|| memory_for(game.len() as u64));
    if memory < game.len() as u64 {
        return err(format!("memory ({memory} bytes) must be at least the game's size ({} bytes)", game.len()));
    }
    if out.exists() && fs::read_dir(out)?.next().is_some() {
        return err(format!("{} is not empty", out.display()));
    }
    fs::create_dir_all(out)?;

    let metadata = serde_json::json!({
        "package_uuid": package_uuid(&game),
        "remote_package_size": game.len(),
        "files": [{ "filename": "/game.love", "crunched": 0, "start": 0, "end": game.len(), "audio": false }],
    });
    let game_js = fill(&fs::read_to_string(template_src.join("game.js"))?, "{{{create_file_paths}}}", "", "game.js")?;
    let game_js = fill(&game_js, "{{{metadata}}}", &metadata.to_string(), "game.js")?;
    let mut html = fs::read_to_string(page.join("index.html"))?;
    html = fill(&html, "{{title}}", &escape_html(&app.title), "index.html")?;
    html = fill(&html, "{{{arguments}}}", "[\"./game.love\"]", "index.html")?;
    html = fill(&html, "{{memory}}", &memory.to_string(), "index.html")?;

    fs::write(out.join("index.html"), html)?;
    fs::write(out.join("game.js"), game_js)?;
    fs::write(out.join("game.data"), &game)?;
    for f in ["love.js", "love.wasm"] {
        fs::copy(dir.join(f), out.join(f))?;
    }
    if !app.compat {
        fs::copy(dir.join("love.worker.js"), out.join("love.worker.js"))?;
        fs::write(out.join("_headers"), HEADERS)?;
    }
    copy_dir(&page.join("theme"), &out.join("theme"))?;
    Ok(())
}

/// The two response headers the standard (threaded) version needs, in the
/// format Netlify and Cloudflare Pages read.
const HEADERS: &str = "/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n";

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in shaped like love.js's `src`, with the placeholders the real
    /// templates have (checked against the real ones in the env-gated test).
    fn fake_template(d: &Path) -> std::path::PathBuf {
        let s = d.join("src");
        for f in ["release", "compat"] {
            fs::create_dir_all(s.join(f).join("theme")).unwrap();
            fs::write(s.join(f).join("index.html"), "<title>{{title}}</title><h1>{{title}}</h1><script>var Module={arguments: {{{arguments}}}, INITIAL_MEMORY: {{memory}}}</script>").unwrap();
            fs::write(s.join(f).join("love.js"), format!("js-{f}")).unwrap();
            fs::write(s.join(f).join("love.wasm"), format!("wasm-{f}")).unwrap();
            fs::write(s.join(f).join("theme/love.css"), "css").unwrap();
        }
        fs::write(s.join("release/love.worker.js"), "worker").unwrap();
        fs::write(s.join("game.js"), "function f(){\n      {{{create_file_paths}}}\n}\nloadPackage({{{metadata}}});\n").unwrap();
        s
    }

    fn app(compat: bool) -> WebApp {
        WebApp { title: "Tom & <Jerry>'s \"Game\"".into(), compat, memory: None }
    }

    #[test]
    fn memory_covers_the_game_in_whole_16_mib_steps() {
        const M: u64 = 16 * 1024 * 1024;
        assert_eq!(memory_for(0), M);
        assert_eq!(memory_for(1_000_000), M);
        assert_eq!(memory_for(8 * 1024 * 1024), M);
        assert_eq!(memory_for(8 * 1024 * 1024 + 1), 2 * M);
        assert_eq!(memory_for(40 * 1024 * 1024), 5 * M);
    }

    #[test]
    fn the_standard_page_has_the_game_the_headers_and_an_escaped_title() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path());
        let love = d.path().join("g.love");
        fs::write(&love, vec![7u8; 1000]).unwrap();
        let out = d.path().join("out");
        package(&t, &love, &out, &app(false)).unwrap();
        assert_eq!(fs::read(out.join("game.data")).unwrap(), vec![7u8; 1000]);
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("<title>Tom &amp; &lt;Jerry&gt;&#39;s &quot;Game&quot;</title>"), "{html}");
        assert!(html.contains("arguments: [\"./game.love\"]") && html.contains(&format!("INITIAL_MEMORY: {}", 16 * 1024 * 1024)));
        let js = fs::read_to_string(out.join("game.js")).unwrap();
        let meta: serde_json::Value = serde_json::from_str(js.split("loadPackage(").nth(1).unwrap().trim_end().trim_end_matches(';').trim_end_matches(')')).unwrap();
        assert_eq!((meta["remote_package_size"].as_u64(), meta["files"][0]["filename"].as_str(), meta["files"][0]["end"].as_u64()), (Some(1000), Some("/game.love"), Some(1000)));
        assert!(!js.contains("{{"), "every placeholder is filled");
        assert_eq!(fs::read_to_string(out.join("love.js")).unwrap(), "js-release");
        assert!(out.join("love.worker.js").is_file() && out.join("theme/love.css").is_file());
        let h = fs::read_to_string(out.join("_headers")).unwrap();
        assert!(h.contains("Cross-Origin-Opener-Policy: same-origin") && h.contains("Cross-Origin-Embedder-Policy: require-corp"));
    }

    #[test]
    fn the_compat_page_needs_no_headers_and_no_worker() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path());
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        let out = d.path().join("out");
        package(&t, &love, &out, &app(true)).unwrap();
        assert_eq!(fs::read_to_string(out.join("love.js")).unwrap(), "js-compat");
        assert!(!out.join("_headers").exists() && !out.join("love.worker.js").exists());
    }

    #[test]
    fn the_same_game_gives_the_same_page_and_a_changed_game_a_new_id() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path());
        let love = d.path().join("g.love");
        fs::write(&love, b"one").unwrap();
        package(&t, &love, &d.path().join("a"), &app(true)).unwrap();
        package(&t, &love, &d.path().join("b"), &app(true)).unwrap();
        assert_eq!(fs::read(d.path().join("a/game.js")).unwrap(), fs::read(d.path().join("b/game.js")).unwrap());
        fs::write(&love, b"two").unwrap();
        package(&t, &love, &d.path().join("c"), &app(true)).unwrap();
        assert_ne!(fs::read(d.path().join("a/game.js")).unwrap(), fs::read(d.path().join("c/game.js")).unwrap());
        let id = package_uuid(b"one");
        assert_eq!(id.len(), 36);
        assert!(id.split('-').map(str::len).eq([8, 4, 4, 4, 12]));
    }

    #[test]
    fn a_wrong_template_a_too_small_memory_and_a_full_folder_are_refused() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path());
        let love = d.path().join("g.love");
        fs::write(&love, vec![0u8; 2000]).unwrap();
        assert!(package(d.path(), &love, &d.path().join("o1"), &app(true)).unwrap_err().to_string().contains("not a love.js `src` folder"));
        let small = WebApp { memory: Some(1000), ..app(true) };
        assert!(package(&t, &love, &d.path().join("o2"), &small).unwrap_err().to_string().contains("at least the game's size"));
        fs::write(t.join("compat/index.html"), "<title>no placeholders</title>").unwrap();
        assert!(package(&t, &love, &d.path().join("o3"), &app(true)).unwrap_err().to_string().contains("{{title}}"));
        let busy = d.path().join("busy");
        fs::create_dir_all(&busy).unwrap();
        fs::write(busy.join("x"), "x").unwrap();
        assert!(package(&fake_template(&d.path().join("t2")), &love, &busy, &app(false)).unwrap_err().to_string().contains("not empty"));
    }

    #[test]
    fn the_shared_html_layout_of_the_lua52_fork_is_read_too() {
        let d = tempfile::tempdir().unwrap();
        let s = d.path().join("src");
        for f in ["release", "compat", "html/theme"] {
            fs::create_dir_all(s.join(f)).unwrap();
        }
        for f in ["release", "compat"] {
            fs::write(s.join(f).join("love.js"), format!("js-{f}")).unwrap();
            fs::write(s.join(f).join("love.wasm"), "w").unwrap();
        }
        fs::write(s.join("release/love.worker.js"), "w").unwrap();
        fs::write(s.join("html/index.html"), "{{title}} {{{arguments}}} {{memory}}").unwrap();
        fs::write(s.join("html/theme/love.css"), "css").unwrap();
        fs::write(s.join("game.js"), "{{{create_file_paths}}}{{{metadata}}}").unwrap();
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        package(&s, &love, &d.path().join("o"), &WebApp { title: "T".into(), compat: false, memory: None }).unwrap();
        assert!(fs::read_to_string(d.path().join("o/index.html")).unwrap().starts_with("T [\"./game.love\"]"));
        assert!(d.path().join("o/theme/love.css").is_file() && d.path().join("o/love.worker.js").is_file());
    }

    /// With the real love.js repository (`JOYRIDE_LOVEJS`): both versions
    /// package, every placeholder is filled, and the pieces are the real ones.
    #[test]
    fn the_real_template_packages_both_versions() {
        let Ok(t) = std::env::var("JOYRIDE_LOVEJS") else { return };
        let src = Path::new(&t).join("src");
        let d = tempfile::tempdir().unwrap();
        let love = d.path().join("g.love");
        fs::write(&love, vec![1u8; 5000]).unwrap();
        for compat in [false, true] {
            let out = d.path().join(if compat { "c" } else { "r" });
            package(&src, &love, &out, &WebApp { title: "Real".into(), compat, memory: None }).unwrap();
            for f in ["index.html", "game.js"] {
                assert!(!fs::read_to_string(out.join(f)).unwrap().contains("{{"), "{f} has an unfilled placeholder");
            }
            assert_eq!(fs::metadata(out.join("love.wasm")).unwrap().len(), fs::metadata(src.join(if compat { "compat" } else { "release" }).join("love.wasm")).unwrap().len());
            assert_eq!(out.join("love.worker.js").exists(), !compat);
        }
    }
}
