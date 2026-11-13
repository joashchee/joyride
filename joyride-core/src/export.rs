//! Exporters (spec 5): the `.love` archive, desktop packages and the
//! PortMaster zip. Each takes a staged project (see `overlay::stage`) and the
//! templates for the active LÖVE version; none of them reads the original
//! source. Web, iOS and Android need their toolchain templates and are not
//! here yet (ROADMAP.md).

use crate::{err, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

/// A lowercase file-safe name: `My Game!` becomes `my-game`.
pub fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "game".into() } else { s }
}

fn opts(mode: u32) -> SimpleFileOptions {
    // A fixed timestamp, so the same project always zips to the same bytes.
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).unix_permissions(mode).last_modified_time(DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0).unwrap())
}

/// Zips a staged project into `out` as a `.love`: `main.lua` at the root,
/// files in name order, no timestamps.
pub fn love_archive(staged: &Path, out: &Path) -> Result<()> {
    if !staged.join("main.lua").is_file() {
        return err("a .love needs main.lua at its root");
    }
    if let Some(p) = out.parent() {
        fs::create_dir_all(p)?;
    }
    let mut z = ZipWriter::new(File::create(out)?);
    for e in WalkDir::new(staged).sort_by_file_name().into_iter().flatten() {
        let rel = e.path().strip_prefix(staged).unwrap();
        if rel.as_os_str().is_empty() || !e.file_type().is_file() {
            continue;
        }
        z.start_file(rel.to_string_lossy().replace('\\', "/"), opts(0o644))?;
        let mut f = File::open(e.path())?;
        std::io::copy(&mut f, &mut z)?;
    }
    z.finish()?;
    Ok(())
}

/// Windows: `love.exe` followed by `game.love` is the game's exe; the
/// runtime DLLs go beside it. (An `.ico` resource is not injected yet.)
pub fn windows(love_exe: &Path, game_love: &Path, out_dir: &Path, slug: &str) -> Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let exe = out_dir.join(format!("{slug}.exe"));
    fuse(love_exe, game_love, &exe)?;
    if let Some(dir) = love_exe.parent() {
        for e in fs::read_dir(dir)?.flatten() {
            if e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("dll")) {
                fs::copy(e.path(), out_dir.join(e.file_name()))?;
            }
        }
    }
    Ok(exe)
}

/// Linux: the LÖVE AppImage followed by `game.love`, marked executable.
pub fn linux(love_appimage: &Path, game_love: &Path, out_dir: &Path, slug: &str) -> Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let out = out_dir.join(format!("{slug}.AppImage"));
    fuse(love_appimage, game_love, &out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o755))?;
    }
    Ok(out)
}

fn fuse(runtime: &Path, game_love: &Path, out: &Path) -> Result<()> {
    if !runtime.is_file() {
        return err(format!("runtime {} not found", runtime.display()));
    }
    let mut o = File::create(out)?;
    std::io::copy(&mut File::open(runtime)?, &mut o)?;
    std::io::copy(&mut File::open(game_love)?, &mut o)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MacApp {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
}

/// macOS: copies `love.app` to `<name>.app`, puts `game.love` in
/// `Contents/Resources/`, and rewrites `Info.plist` (identifier, names,
/// version; the `.love` document and type declarations are removed so the
/// game does not claim every `.love` file). Signing is the caller's step:
/// ad hoc for local trials, Developer ID for release.
pub fn macos(love_app: &Path, game_love: &Path, out_dir: &Path, app: &MacApp) -> Result<PathBuf> {
    let plist_path = love_app.join("Contents/Info.plist");
    if !plist_path.is_file() {
        return err(format!("{} is not a love.app", love_app.display()));
    }
    fs::create_dir_all(out_dir)?;
    let dest = out_dir.join(format!("{}.app", app.name));
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    copy_tree(love_app, &dest)?;
    fs::copy(game_love, dest.join("Contents/Resources/game.love"))?;
    let p = dest.join("Contents/Info.plist");
    let mut v = plist::Value::from_file(&p).map_err(|e| crate::Error::Msg(format!("Info.plist: {e}")))?;
    let d = v.as_dictionary_mut().ok_or_else(|| crate::Error::Msg("Info.plist is not a dictionary".into()))?;
    d.insert("CFBundleIdentifier".into(), app.bundle_id.clone().into());
    d.insert("CFBundleName".into(), app.name.clone().into());
    d.insert("CFBundleDisplayName".into(), app.name.clone().into());
    d.insert("CFBundleShortVersionString".into(), app.version.clone().into());
    d.insert("CFBundleVersion".into(), app.version.clone().into());
    for k in ["UTExportedTypeDeclarations", "CFBundleDocumentTypes"] {
        d.remove(k);
    }
    v.to_file_xml(&p).map_err(|e| crate::Error::Msg(format!("Info.plist: {e}")))?;
    Ok(dest)
}

/// Recursive copy that keeps symlinks and permissions (an .app needs both).
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let ft = e.file_type()?;
        if ft.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(&src)?, &dst)?;
        } else if ft.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?; // keeps the permission bits
        }
    }
    Ok(())
}

/// The LÖVE runtimes PortMaster ships (`runtimes/love_<ver>`). Read from
/// PortMaster-New at commit 40186ea1 (2026-11-13): only 11.5; no port or script
/// there mentions `love_12`. `scripts/check-portmaster.py` compares this with
/// the live repository, so a 12.0 runtime appearing there fails that check.
pub const PORTMASTER_LOVE_VERSIONS: &[&str] = &["11.5"];

/// `port.json`'s schema version in PortMaster-New (the newest, 4, is the most
/// common among its LÖVE ports).
pub const PORT_JSON_VERSION: u32 = 4;

#[derive(Debug, Clone)]
pub struct Port {
    pub slug: String,
    pub title: String,
    pub description: String,
    /// For the PortMaster runtime `love_<ver>`; one of `PORTMASTER_LOVE_VERSIONS`.
    pub love_version: String,
}

/// The PortMaster zip: `<slug>/<Slug>.sh`, `port.json`, `game.gptk` and
/// `gamedata/game.love`. The launch script and metadata follow PortMaster's
/// conventions as best known; they have not been run on a handheld.
pub fn portmaster(game_love: &Path, gptk: &str, port: &Port, out_zip: &Path) -> Result<()> {
    if !PORTMASTER_LOVE_VERSIONS.contains(&port.love_version.as_str()) {
        return err(format!(
            "PortMaster has no LÖVE {} runtime (it ships {}); set love_version to one of those for this target",
            port.love_version,
            PORTMASTER_LOVE_VERSIONS.join(", ")
        ));
    }
    if let Some(p) = out_zip.parent() {
        fs::create_dir_all(p)?;
    }
    let script_name = format!("{}.sh", port_script_stem(&port.title));
    let mut z = ZipWriter::new(File::create(out_zip)?);
    let put = |z: &mut ZipWriter<File>, path: String, mode: u32, bytes: &[u8]| -> Result<()> {
        z.start_file(path, opts(mode))?;
        z.write_all(bytes)?;
        Ok(())
    };
    put(&mut z, script_name.clone(), 0o755, launch_script(port).as_bytes())?;
    put(&mut z, format!("{}/port.json", port.slug), 0o644, port_json(port, &script_name).as_bytes())?;
    put(&mut z, format!("{}/game.gptk", port.slug), 0o644, gptk.as_bytes())?;
    let mut love = Vec::new();
    File::open(game_love)?.read_to_end(&mut love)?;
    put(&mut z, format!("{}/gamedata/game.love", port.slug), 0o644, &love)?;
    z.finish()?;
    Ok(())
}

/// `My Game` becomes `My Game`-safe: letters, digits and spaces only.
fn port_script_stem(title: &str) -> String {
    let t: String = title.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect();
    let t = t.trim();
    if t.is_empty() { "Game".into() } else { t.into() }
}

pub fn port_json(p: &Port, script: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "version": PORT_JSON_VERSION,
        "name": format!("{}.zip", p.slug),
        "items": [script, p.slug],
        "items_opt": [],
        "attr": {
            "title": p.title,
            "porter": ["JoyrIDE"],
            "desc": p.description,
            "desc_md": "",
            "inst": "Ready to run: copy the zip's contents to your ports folder. Controls: see game.gptk.",
            "inst_md": "",
            "genres": [],
            "image": null,
            "rtr": true,
            "exp": false,
            "runtime": [],
            "store": [],
            "availability": "full",
            "reqs": [],
            "arch": [],
            "min_glibc": "",
        }
    }))
    .unwrap()
        + "\n"
}

pub fn launch_script(p: &Port) -> String {
    // The shape of PortMaster-New's own LÖVE ports: the runtime's love.txt
    // provides LOVE_RUN, LOVE_BINARY and LOVE_GPTK; the controller helper runs
    // against LOVE_GPTK with the game's .gptk.
    let t = r#"#!/bin/bash
# PortMaster launch script for @TITLE@, generated by JoyrIDE.
XDG_DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}

if [ -d "/opt/system/Tools/PortMaster/" ]; then
  controlfolder="/opt/system/Tools/PortMaster"
elif [ -d "/opt/tools/PortMaster/" ]; then
  controlfolder="/opt/tools/PortMaster"
elif [ -d "$XDG_DATA_HOME/PortMaster/" ]; then
  controlfolder="$XDG_DATA_HOME/PortMaster"
else
  controlfolder="/roms/ports/PortMaster"
fi

source $controlfolder/control.txt
[ -f "${controlfolder}/mod_${CFW_NAME}.txt" ] && source "${controlfolder}/mod_${CFW_NAME}.txt"
get_controls

GAMEDIR="/$directory/ports/@SLUG@"
cd "$GAMEDIR" || exit 1

# Enable logging
: > "$GAMEDIR/log.txt" && exec > >(tee "$GAMEDIR/log.txt") 2>&1

# Saves and settings stay in the game folder.
export XDG_DATA_HOME="$GAMEDIR/conf"
export SDL_GAMECONTROLLERCONFIG="$sdl_controllerconfig"
mkdir -p "$XDG_DATA_HOME"

# Source the LÖVE runtime this game was made for.
source $controlfolder/runtimes/"love_@VER@"/love.txt

# Use the runtime with this game's controller map.
$GPTOKEYB "$LOVE_GPTK" -c "$GAMEDIR/game.gptk" &
pm_platform_helper "$LOVE_BINARY"
$LOVE_RUN "$GAMEDIR/gamedata/game.love"

# Clean up gptokeyb and any platform-specific changes.
pm_finish
"#;
    t.replace("@TITLE@", &p.title.replace('\n', " ")).replace("@SLUG@", &p.slug).replace("@VER@", &p.love_version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn staged(d: &Path) -> PathBuf {
        let s = d.join("stage");
        fs::create_dir_all(s.join("src")).unwrap();
        fs::write(s.join("main.lua"), "print('hi')").unwrap();
        fs::write(s.join("src/a.lua"), "return 1").unwrap();
        s
    }

    fn names(zip: &Path) -> Vec<String> {
        let mut a = zip::ZipArchive::new(File::open(zip).unwrap()).unwrap();
        (0..a.len()).map(|i| a.by_index(i).unwrap().name().to_string()).collect()
    }

    fn read(zip: &Path, name: &str) -> String {
        let mut a = zip::ZipArchive::new(File::open(zip).unwrap()).unwrap();
        let mut s = String::new();
        a.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("My Game!"), "my-game");
        assert_eq!(slug("???"), "game");
        assert_eq!(slug("Año 2"), "a-o-2");
    }

    #[test]
    fn love_archive_is_reproducible_and_needs_main() {
        let d = tempfile::tempdir().unwrap();
        let s = staged(d.path());
        let (a, b) = (d.path().join("a.love"), d.path().join("b.love"));
        love_archive(&s, &a).unwrap();
        love_archive(&s, &b).unwrap();
        assert_eq!(names(&a), ["main.lua", "src/a.lua"]);
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap(), "same input, same bytes");
        assert_eq!(read(&a, "src/a.lua"), "return 1");
        fs::remove_file(s.join("main.lua")).unwrap();
        assert!(love_archive(&s, &d.path().join("c.love")).is_err());
    }

    #[test]
    fn windows_and_linux_fuse_the_runtime_and_the_game() {
        let d = tempfile::tempdir().unwrap();
        let s = staged(d.path());
        let love = d.path().join("g.love");
        love_archive(&s, &love).unwrap();
        let rt = d.path().join("rt");
        fs::create_dir_all(&rt).unwrap();
        fs::write(rt.join("love.exe"), b"MZ-runtime").unwrap();
        fs::write(rt.join("SDL2.dll"), b"dll").unwrap();
        fs::write(rt.join("readme.txt"), b"x").unwrap();
        let exe = windows(&rt.join("love.exe"), &love, &d.path().join("win"), "my-game").unwrap();
        let bytes = fs::read(&exe).unwrap();
        assert!(bytes.starts_with(b"MZ-runtime") && bytes.ends_with(&fs::read(&love).unwrap()));
        assert!(d.path().join("win/SDL2.dll").exists() && !d.path().join("win/readme.txt").exists());
        // The fused file is still a readable zip (the game's), as LÖVE needs.
        fs::write(rt.join("l.AppImage"), b"ELF-runtime").unwrap();
        let ai = linux(&rt.join("l.AppImage"), &love, &d.path().join("lin"), "my-game").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&ai).unwrap().permissions().mode() & 0o111, 0o111);
        }
        assert!(zip::ZipArchive::new(File::open(&ai).unwrap()).is_ok());
        assert!(windows(&rt.join("missing.exe"), &love, &d.path().join("w2"), "x").is_err());
    }

    #[test]
    fn macos_app_gets_the_game_and_a_new_identity() {
        let d = tempfile::tempdir().unwrap();
        let s = staged(d.path());
        let love = d.path().join("g.love");
        love_archive(&s, &love).unwrap();
        let app = d.path().join("love.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        fs::write(app.join("Contents/MacOS/love"), b"bin").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(app.join("Contents/MacOS/love"), fs::Permissions::from_mode(0o755)).unwrap();
            std::os::unix::fs::symlink("MacOS/love", app.join("Contents/link")).unwrap();
        }
        let mut dict = plist::Dictionary::new();
        dict.insert("CFBundleIdentifier".into(), "org.love2d.love".into());
        dict.insert("CFBundleName".into(), "LÖVE".into());
        dict.insert("UTExportedTypeDeclarations".into(), plist::Value::Array(vec![]));
        dict.insert("CFBundleDocumentTypes".into(), plist::Value::Array(vec![]));
        plist::Value::Dictionary(dict).to_file_xml(app.join("Contents/Info.plist")).unwrap();
        let out = macos(&app, &love, &d.path().join("mac"), &MacApp { name: "My Game".into(), bundle_id: "dev.joyride.mygame".into(), version: "1.2.0".into() }).unwrap();
        assert!(out.join("Contents/Resources/game.love").is_file());
        let v = plist::Value::from_file(out.join("Contents/Info.plist")).unwrap();
        let d2 = v.as_dictionary().unwrap();
        assert_eq!(d2["CFBundleIdentifier"].as_string(), Some("dev.joyride.mygame"));
        assert_eq!(d2["CFBundleVersion"].as_string(), Some("1.2.0"));
        assert!(!d2.contains_key("UTExportedTypeDeclarations") && !d2.contains_key("CFBundleDocumentTypes"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(fs::symlink_metadata(out.join("Contents/link")).unwrap().file_type().is_symlink());
            assert_eq!(fs::metadata(out.join("Contents/MacOS/love")).unwrap().permissions().mode() & 0o111, 0o111);
        }
        assert!(macos(&d.path().join("nope.app"), &love, &d.path().join("m2"), &MacApp { name: "x".into(), bundle_id: "a.b".into(), version: "1".into() }).is_err());
    }

    /// Runs the launch script under bash against a fake PortMaster: a control
    /// folder whose `control.txt` and `runtimes/love_<ver>/love.txt` stub what
    /// the real ones provide. Proves the script's own logic (paths, the runtime
    /// it sources, what it hands the runner); what the real PortMaster defines
    /// is `scripts/check-portmaster.py`'s job, not this test's.
    #[test]
    #[cfg(unix)]
    fn the_launch_script_runs_against_a_stub_portmaster() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("pm");
        let ctl = root.join("PortMaster");
        let run_dir = root.join("ports/my-game");
        fs::create_dir_all(ctl.join("runtimes/love_11.5")).unwrap();
        fs::create_dir_all(run_dir.join("gamedata")).unwrap();
        let log = root.join("calls.log");
        fs::write(
            ctl.join("control.txt"),
            format!("directory=\"{}\"\nget_controls() {{ GPTOKEYB=\"{root}/fake-gptk\"; sdl_controllerconfig=x; }}\npm_platform_helper() {{ :; }}\npm_finish() {{ echo finish >> {log}; }}\n", root.strip_prefix("/").unwrap().parent().unwrap().display(), root = root.display(), log = log.display()),
        )
        .unwrap();
        fs::write(ctl.join("runtimes/love_11.5/love.txt"), format!("LOVE_BINARY=\"{r}/fake-love\"\nLOVE_RUN=\"{r}/fake-love\"\nLOVE_GPTK=\"love\"\n", r = root.display())).unwrap();
        for (name, body) in [("fake-love", "echo \"love $*\" >> LOG"), ("fake-gptk", "echo \"gptk $*\" >> LOG")] {
            let f = root.join(name);
            fs::write(&f, format!("#!/bin/sh\n{}\n", body.replace("LOG", &log.display().to_string()))).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&f, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let port = Port { slug: "my-game".into(), title: "My Game".into(), description: String::new(), love_version: "11.5".into() };
        // Point the script's last-resort control folder at the stub, and the games folder at it.
        let script = launch_script(&port).replace("/roms/ports/PortMaster", &ctl.display().to_string()).replace("GAMEDIR=\"/$directory/ports/my-game\"", &format!("GAMEDIR=\"{}\"", run_dir.display()));
        let sh = root.join("launch.sh");
        fs::write(&sh, &script).unwrap();
        let syntax = std::process::Command::new("bash").arg("-n").arg(&sh).status().unwrap();
        assert!(syntax.success(), "bash -n failed");
        if let Ok(sc) = std::process::Command::new("shellcheck").args(["-s", "bash", "-S", "warning", "-e", "SC1090,SC1091,SC2154,SC2034"]).arg(&sh).output() {
            assert!(sc.status.success(), "shellcheck:\n{}", String::from_utf8_lossy(&sc.stdout));
        }
        let out = std::process::Command::new("bash").arg(&sh).env("HOME", d.path()).output().unwrap();
        assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        std::thread::sleep(std::time::Duration::from_millis(300)); // the controller helper runs in the background
        let calls = fs::read_to_string(&log).unwrap();
        assert!(calls.contains(&format!("love {}/gamedata/game.love", run_dir.display())), "{calls}");
        assert!(calls.contains(&format!("gptk love -c {}/game.gptk", run_dir.display())), "{calls}");
        assert!(calls.contains("finish"), "{calls}");
    }

    #[test]
    fn portmaster_refuses_a_love_version_it_has_no_runtime_for() {
        let d = tempfile::tempdir().unwrap();
        let s = staged(d.path());
        let love = d.path().join("g.love");
        love_archive(&s, &love).unwrap();
        let port = Port { slug: "g".into(), title: "G".into(), description: String::new(), love_version: "12.0".into() };
        let e = portmaster(&love, "", &port, &d.path().join("o.zip")).unwrap_err().to_string();
        assert!(e.contains("no LÖVE 12.0 runtime") && e.contains("11.5"), "{e}");
        assert!(!d.path().join("o.zip").exists());
    }

    #[test]
    fn portmaster_zip_has_the_layout_and_the_runtime() {
        let d = tempfile::tempdir().unwrap();
        let s = staged(d.path());
        let love = d.path().join("g.love");
        love_archive(&s, &love).unwrap();
        let port = Port { slug: "my-game".into(), title: "My Game!".into(), description: "A game.".into(), love_version: "11.5".into() };
        let z = d.path().join("out/my-game-portmaster.zip");
        portmaster(&love, "a = z\n", &port, &z).unwrap();
        let n = names(&z);
        assert_eq!(n, ["My Game.sh", "my-game/port.json", "my-game/game.gptk", "my-game/gamedata/game.love"]);
        let sh = read(&z, "My Game.sh");
        assert!(sh.starts_with("#!/bin/bash") && sh.contains("runtimes/\"love_11.5\"/love.txt") && sh.contains("ports/my-game") && sh.contains("$GPTOKEYB \"$LOVE_GPTK\"") && sh.contains("pm_finish"));
        let pj: serde_json::Value = serde_json::from_str(&read(&z, "my-game/port.json")).unwrap();
        assert_eq!(pj["version"], PORT_JSON_VERSION);
        assert_eq!(pj["attr"]["runtime"], serde_json::json!([]), "LÖVE ports list no runtime attr: the love runtime is sourced by the script");
        assert_eq!(pj["attr"]["porter"], serde_json::json!(["JoyrIDE"]));
        assert_eq!(pj["items"][0], "My Game.sh");
        assert_eq!(read(&z, "my-game/game.gptk"), "a = z\n");
        let mut a = zip::ZipArchive::new(File::open(&z).unwrap()).unwrap();
        assert_eq!(a.by_name("My Game.sh").unwrap().unix_mode().unwrap() & 0o111, 0o111);
    }
}
