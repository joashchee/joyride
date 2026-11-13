//! The Export Center's backend: from a project folder to a build, in one call.
//! Scan, make the overlay if the project has none, stage a copy, collect
//! credits, check the asset licences, zip. Every build is made from the
//! staged copy; the project is never modified except for writing a missing
//! overlay under `.joyride/`.

use crate::android::{self, AndroidApp};
use crate::apple_mobile::{self, Family, IosApp, Orientation};
use crate::compat;
use crate::export::{self, Port};
use crate::manifest::Manifest;
use crate::overlay;
use crate::web::{self, WebApp};
use crate::scan::{self, Target};
use crate::toolchain::LoveVersion;
use crate::{err, Result};
use asset_core::project::{self, ExportContext, Issue};
use asset_core::Catalog;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Check the project's library assets before building, and put a
/// `CREDITS.txt` in the build.
pub struct AssetGate<'a> {
    pub catalog: &'a Catalog,
    /// The project's name in the catalog (as imported).
    pub project: &'a str,
    pub commercial: bool,
    pub today: &'a str,
}

/// Build the 11.5 variant of a 12.0 project (see `compat`).
pub struct Compat<'a> {
    /// 12.0-only APIs with no honest emulation that the person accepts anyway
    /// (the game guards them, say). Anything else unsupported stops the build.
    pub allow: &'a [String],
}

pub struct Build<'a> {
    pub root: &'a Path,
    pub out_dir: &'a Path,
    pub gate: Option<AssetGate<'a>>,
    /// `Some` builds the LÖVE 11.5 variant of a 12.0 project, for any target.
    pub compat: Option<Compat<'a>>,
}

/// What the person is offered for a project, to show before building: `None`
/// unless it targets LÖVE 12.0 or later.
pub fn compat_offer(root: &Path) -> Result<Option<compat::Report>> {
    let m = manifest_for(root)?;
    let v = LoveVersion::parse(&m.project.love_version).ok_or_else(|| crate::Error::Msg("unsupported love_version".into()))?;
    Ok(compat::offered(v).then(|| compat::analyze_project(root)))
}

/// The LÖVE version a build runs on: the project's, or 11.5 for the variant.
fn effective_version(m: &Manifest, b: &Build) -> Result<LoveVersion> {
    let project = LoveVersion::parse(&m.project.love_version).ok_or_else(|| crate::Error::Msg("unsupported love_version".into()))?;
    if b.compat.is_some() {
        if !compat::offered(project) {
            return err(format!("this project already targets LÖVE {project}: there is no 11.5 variant to make"));
        }
        return Ok(crate::toolchain::BUNDLED);
    }
    Ok(project)
}

/// The project's manifest, or one made from its folder name when it has no
/// `love.toml` yet.
pub fn manifest_for(root: &Path) -> Result<Manifest> {
    match std::fs::read_to_string(root.join("love.toml")) {
        Ok(t) => {
            let m = Manifest::parse(&t)?;
            crate::manifest::require_valid(&m)?;
            Ok(m)
        }
        Err(_) => {
            let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("game");
            Ok(Manifest::new_project(name, &format!("dev.joyride.{}", export::slug(name).replace('-', ""))))
        }
    }
}

/// Everything a build refused for, as a readable list.
fn gate_message(issues: &[Issue]) -> String {
    let mut s = String::from("licence problems stop this build:");
    for i in issues {
        s.push_str("\n  ");
        s.push_str(&match i {
            Issue::Unconfirmed { path } => format!("{path}: licence not confirmed"),
            Issue::NotCommercial { path, spdx } => format!("{path}: {spdx} does not allow commercial use"),
            Issue::ShareAlike { path, spdx } => format!("{path}: {spdx} is share-alike; accept that or replace it"),
            Issue::Expired { path, expires } => format!("{path}: rights lapsed on {expires}"),
        });
    }
    s
}

/// Stages and zips `<slug>-<target>.love` for a target; returns its path.
pub fn build_love(b: &Build, target: Target) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    let version = effective_version(&m, b)?;
    let polyfill = match &b.compat {
        Some(c) => {
            let report = compat::analyze_project(b.root);
            let blockers = report.blockers(c.allow);
            if !blockers.is_empty() {
                let mut msg = String::from("these LÖVE 12.0 functions have no honest equivalent in 11.5, so the 11.5 variant is not built:");
                for u in blockers {
                    let why = if let compat::Support::Unsupported(w) = u.support { w } else { "" };
                    msg.push_str(&format!("\n  {}:{}: {} ({why})", u.file, u.line, u.api));
                }
                msg.push_str("\nGuard them in the game, or accept them to build anyway.");
                return err(msg);
            }
            Some(compat::polyfill_lua(&report))
        }
        None => None,
    };
    if let Some(g) = &b.gate {
        let issues = project::export_issues(g.catalog, g.project, ExportContext { commercial: g.commercial, today: g.today }).map_err(|e| crate::Error::Msg(e.to_string()))?;
        if !issues.is_empty() {
            return err(gate_message(&issues));
        }
    }
    let name = overlay::target_dir_name(target);
    if !b.root.join(".joyride/patches").join(name).join("_jr_shim.lua").is_file() {
        let s = scan::scan_project(b.root);
        overlay::write(b.root, &overlay::generate(target, &s))?;
    }
    let tmp = tempdir_in(b.out_dir)?;
    let stage = tmp.join("stage");
    let mut conf = BTreeMap::new();
    conf.insert("version".to_string(), toml::Value::String(version.to_string()));
    overlay::stage_with(b.root, target, &stage, &conf, polyfill.as_deref())?;
    if let Some(g) = &b.gate {
        let text = project::credits(g.catalog, g.project).map_err(|e| crate::Error::Msg(e.to_string()))?;
        std::fs::write(stage.join("CREDITS.txt"), text)?;
    }
    let suffix = if b.compat.is_some() { "-11.5" } else { "" };
    let out = b.out_dir.join(format!("{}-{name}{suffix}.love", export::slug(&m.project.name)));
    let r = export::love_archive(&stage, &out);
    let _ = std::fs::remove_dir_all(&tmp);
    r?;
    Ok(out)
}

/// A plain `<slug>.love` of the project as it is (no target shim, for
/// desktop LÖVE): the project copied without `.git`, `build`, `.joyride` and
/// other `.love` files, the asset gate applied, `CREDITS.txt` added.
pub fn build_plain_love(b: &Build) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    if b.compat.is_some() {
        return err("the 11.5 variant needs a target shim; build it for PortMaster, web or mobile");
    }
    if !b.root.join("main.lua").is_file() {
        return err("the project has no main.lua");
    }
    if let Some(g) = &b.gate {
        let issues = project::export_issues(g.catalog, g.project, ExportContext { commercial: g.commercial, today: g.today }).map_err(|e| crate::Error::Msg(e.to_string()))?;
        if !issues.is_empty() {
            return err(gate_message(&issues));
        }
    }
    let tmp = tempdir_in(b.out_dir)?;
    let stage = tmp.join("stage");
    let copied = (|| -> Result<()> {
        for e in walkdir::WalkDir::new(b.root).sort_by_file_name().into_iter().filter_entry(|e| !overlay::skipped(e.path().strip_prefix(b.root).unwrap_or(e.path()))) {
            let e = e.map_err(|e| crate::Error::Msg(e.to_string()))?;
            let rel = e.path().strip_prefix(b.root).unwrap();
            if rel.as_os_str().is_empty() {
                continue;
            }
            // Links are neither followed nor copied: a project cannot pull files from outside itself.
            if e.file_type().is_dir() {
                std::fs::create_dir_all(stage.join(rel))?;
            } else if e.file_type().is_file() {
                if let Some(p) = stage.join(rel).parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(e.path(), stage.join(rel))?;
            }
        }
        if let Some(g) = &b.gate {
            let text = project::credits(g.catalog, g.project).map_err(|e| crate::Error::Msg(e.to_string()))?;
            std::fs::write(stage.join("CREDITS.txt"), text)?;
        }
        Ok(())
    })();
    let out = b.out_dir.join(format!("{}.love", export::slug(&m.project.name)));
    let r = copied.and_then(|_| export::love_archive(&stage, &out));
    let _ = std::fs::remove_dir_all(&tmp);
    r?;
    Ok(out)
}

/// The PortMaster zip for a project.
pub fn build_portmaster(b: &Build) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    let version = effective_version(&m, b)?.to_string();
    if !export::PORTMASTER_LOVE_VERSIONS.contains(&version.as_str()) {
        return err(format!("PortMaster has no LÖVE {version} runtime (it ships {}); build the 11.5 variant of this project instead", export::PORTMASTER_LOVE_VERSIONS.join(", ")));
    }
    let love = build_love(b, Target::PortMaster)?;
    let gptk = std::fs::read_to_string(b.root.join(".joyride/patches/portmaster/game.gptk")).unwrap_or_default();
    let slug = export::slug(&m.project.name);
    let port = Port { slug: slug.clone(), title: m.project.name.clone(), description: format!("{} for PortMaster handhelds.", m.project.name), love_version: version };
    let zip = b.out_dir.join(format!("{slug}-portmaster{}.zip", if b.compat.is_some() { "-11.5" } else { "" }));
    export::portmaster(&love, &gptk, &port, &zip)?;
    std::fs::remove_file(&love)?; // the zip carries it; the intermediate is not a deliverable
    Ok(zip)
}

/// The Android Gradle project for a game: the mobile `.love` (the 11.5
/// variant if `b.compat` asks) inside a `love-android` checkout of the matching
/// branch. Returns the project folder; building it (`android::gradle_args`) and
/// signing are the caller's steps.
pub fn build_android(b: &Build, template: &Path, icon: Option<&Path>) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    let version = effective_version(&m, b)?;
    let love = build_love(b, Target::Mobile)?;
    let scan = scan::scan_project(b.root);
    let landscape = scan.window.width.unwrap_or(800.0) >= scan.window.height.unwrap_or(600.0);
    let app = AndroidApp { name: m.project.name.clone(), bundle_id: m.project.bundle_id.clone(), version_name: m.project.version.clone(), landscape, love_version: version };
    let project = b.out_dir.join(format!("{}-android-project", export::slug(&m.project.name)));
    if project.exists() {
        std::fs::remove_dir_all(&project)?;
    }
    let r = android::stage_project(template, &love, &project, &app, icon);
    let _ = std::fs::remove_file(&love); // the project carries it
    r?;
    Ok(project)
}

/// The Xcode project for an iOS or iPadOS game: the mobile `.love` (or its
/// 11.5 variant) in a `love` checkout of the matching branch. Returns
/// `…/platform/xcode` of the staged copy; `apple_mobile::xcodebuild_args` and
/// signing are the caller's steps. `libraries` demands LÖVE's iOS dependencies
/// be in the template already.
pub fn build_ios(b: &Build, template: &Path, family: Family, team: Option<&str>, icon: Option<&Path>, libraries: bool) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    let version = effective_version(&m, b)?;
    let love = build_love(b, Target::Mobile)?;
    let scan = scan::scan_project(b.root);
    let (w, h) = (scan.window.width.unwrap_or(800.0), scan.window.height.unwrap_or(600.0));
    let orientation = if w > h { Orientation::Landscape } else if h > w { Orientation::Portrait } else { Orientation::Any };
    let app = IosApp { name: m.project.name.clone(), bundle_id: m.project.bundle_id.clone(), version: m.project.version.clone(), family, orientation, team: team.map(String::from), love_version: version };
    let project = b.out_dir.join(format!("{}-ios-project", export::slug(&m.project.name)));
    if project.exists() {
        std::fs::remove_dir_all(&project)?;
    }
    let r = apple_mobile::stage_project(template, &love, &project, &app, icon, libraries);
    let _ = std::fs::remove_file(&love);
    r
}

/// The love.js page for a game, in `<out_dir>/<slug>-web` (or `-web-compat`).
/// love.js runs LÖVE 11.5 only, so a 12.0 project needs `b.compat` (its 11.5
/// variant). `template_src` is the `src` folder of a love.js checkout.
pub fn build_web(b: &Build, template_src: &Path, compatibility_page: bool) -> Result<PathBuf> {
    let m = manifest_for(b.root)?;
    let version = effective_version(&m, b)?.to_string();
    if !web::WEB_LOVE_VERSIONS.contains(&version.as_str()) {
        return err(format!("love.js runs LÖVE {} only; build the 11.5 variant of this {version} project for the web", web::WEB_LOVE_VERSIONS.join(", ")));
    }
    let love = build_love(b, Target::Web)?;
    let out = b.out_dir.join(format!("{}-web{}", export::slug(&m.project.name), if compatibility_page { "-compat" } else { "" }));
    if out.exists() {
        std::fs::remove_dir_all(&out)?;
    }
    let r = web::package(template_src, &love, &out, &WebApp { title: m.project.name.clone(), compat: compatibility_page, memory: None });
    let _ = std::fs::remove_file(&love);
    r?;
    Ok(out)
}

fn tempdir_in(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join(format!(".joyride-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asset_core::catalog::NewAsset;
    use asset_core::{Kind, Licence};
    use std::io::Read;

    fn project(d: &Path) -> PathBuf {
        let root = d.join("My Game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("main.lua"), "function love.keypressed(k) if k == 'space' then end end\nfunction love.draw() end").unwrap();
        std::fs::write(root.join("conf.lua"), "function love.conf(t) t.window.width = 640 t.window.height = 480 end").unwrap();
        root
    }

    fn gate(c: &Catalog) -> Option<AssetGate<'_>> {
        Some(AssetGate { catalog: c, project: "My Game", commercial: true, today: "2026-11-13" })
    }

    fn zip_names(p: &Path) -> Vec<String> {
        let mut a = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
        (0..a.len()).map(|i| a.by_index(i).unwrap().name().to_string()).collect()
    }

    #[test]
    fn a_plain_love_is_the_project_as_it_is() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.lua"), "return 1").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "x").unwrap();
        std::fs::create_dir_all(root.join(".joyride/patches")).unwrap();
        std::fs::write(root.join("old.love"), "x").unwrap();
        let out = root.join("build");
        let love = build_plain_love(&Build { root: &root, out_dir: &out, gate: None, compat: None }).unwrap();
        assert!(love.ends_with("build/my-game.love"));
        assert_eq!(zip_names(&love), ["conf.lua", "main.lua", "src/a.lua"], "no .git, .joyride, other .love, and no shim");
        assert!(!out.join(format!(".joyride-build-{}", std::process::id())).exists());
        // Building again (now with build/ present) does not pack the build folder.
        let again = build_plain_love(&Build { root: &root, out_dir: &out, gate: None, compat: None }).unwrap();
        assert_eq!(std::fs::read(&love).unwrap(), std::fs::read(&again).unwrap());
        assert!(build_plain_love(&Build { root: &root, out_dir: &out, gate: None, compat: Some(Compat { allow: &[] }) }).is_err());
        std::fs::remove_file(root.join("main.lua")).unwrap();
        assert!(build_plain_love(&Build { root: &root, out_dir: &out, gate: None, compat: None }).is_err());
    }

    #[test]
    fn one_call_builds_a_portmaster_zip_and_makes_the_overlay() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        let out = d.path().join("out");
        let zip = build_portmaster(&Build { root: &root, out_dir: &out, gate: None, compat: None }).unwrap();
        assert!(zip.ends_with("my-game-portmaster.zip"));
        assert_eq!(zip_names(&zip), ["My Game.sh", "my-game/port.json", "my-game/game.gptk", "my-game/gamedata/game.love"]);
        assert!(root.join(".joyride/patches/portmaster/_jr_shim.lua").is_file(), "the overlay was generated for the user to see and edit");
        assert!(!out.join("my-game-portmaster.love").exists() && !out.join(format!(".joyride-build-{}", std::process::id())).exists(), "no leftovers");
        // The game.love inside is the staged one: wrapped main, original kept, version pinned in conf.
        let mut a = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
        let mut inner = Vec::new();
        a.by_name("my-game/gamedata/game.love").unwrap().read_to_end(&mut inner).unwrap();
        let mut l = zip::ZipArchive::new(std::io::Cursor::new(inner)).unwrap();
        let names: Vec<String> = (0..l.len()).map(|i| l.by_index(i).unwrap().name().to_string()).collect();
        for n in ["main.lua", "_orig_main.lua", "_jr_shim.lua", "conf.lua", "_orig_conf.lua"] {
            assert!(names.contains(&n.to_string()), "{n} missing from {names:?}");
        }
        let mut conf = String::new();
        l.by_name("conf.lua").unwrap().read_to_string(&mut conf).unwrap();
        assert!(conf.contains("t.version = \"11.5\""));
    }

    #[test]
    fn the_gate_stops_a_build_with_an_unconfirmed_asset_and_credits_ship_when_clear() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        let mut cat = Catalog::in_memory().unwrap();
        let (id, _) = cat.add_asset(&NewAsset { hash: "h1".into(), size: 1, kind: Kind::Image, name: "hero.png".into(), ext: "png".into(), stored: "x".into() }).unwrap();
        cat.record_use(id, "My Game", "img/hero.png").unwrap();
        let out = d.path().join("out");
        let e = build_love(&Build { root: &root, out_dir: &out, gate: gate(&cat), compat: None }, Target::Mobile).unwrap_err().to_string();
        assert!(e.contains("img/hero.png: licence not confirmed"), "{e}");
        assert!(!out.join("my-game-mobile.love").exists());
        cat.confirm_licence(id, &Licence::from_spdx("CC-BY-4.0"), "me", "").unwrap();
        cat.set_credit(id, "Ada", "", "").unwrap();
        let love = build_love(&Build { root: &root, out_dir: &out, gate: gate(&cat), compat: None }, Target::Mobile).unwrap();
        let mut a = zip::ZipArchive::new(std::fs::File::open(&love).unwrap()).unwrap();
        let mut credits = String::new();
        a.by_name("CREDITS.txt").unwrap().read_to_string(&mut credits).unwrap();
        assert!(credits.contains("hero.png by Ada (CC-BY-4.0)"));
    }

    #[test]
    fn manifest_defaults_and_validation() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        let m = manifest_for(&root).unwrap();
        assert_eq!((m.project.name.as_str(), m.project.bundle_id.as_str(), m.project.love_version.as_str()), ("My Game", "dev.joyride.mygame", "11.5"));
        std::fs::write(root.join("love.toml"), "[project]\nname='x'\nbundle_id='bad'\nversion='1'\n").unwrap();
        assert!(manifest_for(&root).is_err());
    }

    #[test]
    fn a_12_0_project_cannot_target_portmaster_yet() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        std::fs::write(root.join("love.toml"), "[project]\nname='g'\nbundle_id='dev.x.g'\nversion='1'\nlove_version='12.0'\n").unwrap();
        let out = d.path().join("o");
        let e = build_portmaster(&Build { root: &root, out_dir: &out, gate: None, compat: None }).unwrap_err().to_string();
        assert!(e.contains("no LÖVE 12.0 runtime"), "{e}");
        assert!(!out.exists(), "refused before building anything");
    }

    fn project12(d: &Path, main: &str) -> PathBuf {
        let root = project(d);
        std::fs::write(root.join("main.lua"), main).unwrap();
        std::fs::write(root.join("love.toml"), "[project]\nname='My Game'\nbundle_id='dev.x.mygame'\nversion='1'\nlove_version='12.0'\n").unwrap();
        root
    }

    fn inner_love(zip: &Path) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut a = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
        let mut bytes = Vec::new();
        a.by_name("my-game/gamedata/game.love").unwrap().read_to_end(&mut bytes).unwrap();
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap()
    }

    #[test]
    fn a_12_project_gets_an_11_5_variant_for_portmaster() {
        let d = tempfile::tempdir().unwrap();
        let root = project12(d.path(), "local f = love.filesystem.openFile('a', 'r')\nfunction love.keypressed(k) if k == 'r' then love.event.restart() end end\nlocal s = love.sensor.hasSensor('gyroscope')");
        let out = d.path().join("out");
        let zip = build_portmaster(&Build { root: &root, out_dir: &out, gate: None, compat: Some(Compat { allow: &[] }) }).unwrap();
        assert!(zip.ends_with("my-game-portmaster-11.5.zip"));
        let mut l = inner_love(&zip);
        let mut conf = String::new();
        l.by_name("conf.lua").unwrap().read_to_string(&mut conf).unwrap();
        assert!(conf.contains("t.version = \"11.5\""), "{conf}");
        let (mut compat_lua, mut main) = (String::new(), String::new());
        l.by_name("_jr_compat.lua").unwrap().read_to_string(&mut compat_lua).unwrap();
        l.by_name("main.lua").unwrap().read_to_string(&mut main).unwrap();
        assert!(compat_lua.contains("openFile") && compat_lua.contains("restart") && compat_lua.contains("love.sensor"));
        assert!(main.find("_jr_compat").unwrap() < main.find("_jr_shim").unwrap(), "compat is defined first");
        // The project itself is unchanged: still 12.0 on disk, no variant files.
        assert!(!root.join("_jr_compat.lua").exists() && std::fs::read_to_string(root.join("love.toml")).unwrap().contains("12.0"));
        // Without the variant, the same project cannot go to PortMaster.
        assert!(build_portmaster(&Build { root: &root, out_dir: &out, gate: None, compat: None }).is_err());
    }

    #[test]
    fn unsupported_12_apis_stop_the_variant_unless_accepted() {
        let d = tempfile::tempdir().unwrap();
        let root = project12(d.path(), "local m = 1\nif love.system.getMemorySize then m = love.system.getMemorySize() end");
        let out = d.path().join("out");
        let e = build_love(&Build { root: &root, out_dir: &out, gate: None, compat: Some(Compat { allow: &[] }) }, Target::Mobile).unwrap_err().to_string();
        assert!(e.contains("main.lua:2: love.system.getMemorySize") && e.contains("Guard them"), "{e}");
        assert!(!out.join("my-game-mobile-11.5.love").exists());
        let allow = vec!["love.system.getMemorySize".to_string()];
        let love = build_love(&Build { root: &root, out_dir: &out, gate: None, compat: Some(Compat { allow: &allow }) }, Target::Mobile).unwrap();
        assert!(love.ends_with("my-game-mobile-11.5.love"));
        let mut a = zip::ZipArchive::new(std::fs::File::open(&love).unwrap()).unwrap();
        let mut c = String::new();
        a.by_name("_jr_compat.lua").unwrap().read_to_string(&mut c).unwrap();
        assert!(!c.contains("getMemorySize"), "an accepted API is still not faked");
    }

    #[test]
    fn the_variant_is_only_for_12_projects_and_is_offered_with_its_report() {
        let d = tempfile::tempdir().unwrap();
        let p11 = project(d.path());
        assert!(compat_offer(&p11).unwrap().is_none());
        let e = build_love(&Build { root: &p11, out_dir: &d.path().join("o"), gate: None, compat: Some(Compat { allow: &[] }) }, Target::Web).unwrap_err().to_string();
        assert!(e.contains("already targets LÖVE 11.5"), "{e}");
        let d2 = tempfile::tempdir().unwrap();
        let p12 = project12(d2.path(), "love.event.restart()\nlove.markDeprecated(1)");
        let r = compat_offer(&p12).unwrap().expect("a 12.0 project is offered the variant");
        assert_eq!((r.uses.len(), r.reduced().len(), r.blockers(&[]).len()), (2, 1, 0));
    }

    #[test]
    fn an_android_project_carries_the_staged_game_and_the_games_orientation() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path()); // conf: 640x480, so landscape
        let t = d.path().join("tpl");
        for dir in ["app/src/main/cpp/love/src", "app/src/main/cpp/megasource", "app/src/embed/assets"] {
            std::fs::create_dir_all(t.join(dir)).unwrap();
        }
        std::fs::write(t.join("app/src/main/cpp/love/src/a.cpp"), "x").unwrap();
        std::fs::write(t.join("app/src/main/cpp/megasource/b.cpp"), "x").unwrap();
        std::fs::write(t.join("app/build.gradle"), "").unwrap();
        std::fs::write(t.join("app/src/main/AndroidManifest.xml"), "").unwrap();
        std::fs::write(t.join("gradlew"), "").unwrap();
        std::fs::write(t.join("gradle.properties"), "app.name=x\napp.orientation=portrait\n").unwrap();
        let out = d.path().join("out");
        let p = build_android(&Build { root: &root, out_dir: &out, gate: None, compat: None }, &t, None).unwrap();
        assert!(p.ends_with("my-game-android-project"));
        let props = std::fs::read_to_string(p.join("gradle.properties")).unwrap();
        assert!(props.contains("app.orientation=landscape") && props.contains("app.name=My Game"), "{props}");
        let mut z = zip::ZipArchive::new(std::fs::File::open(p.join("app/src/embed/assets/game.love")).unwrap()).unwrap();
        assert!(z.by_name("_jr_shim.lua").is_ok(), "the mobile build, shim and all");
        assert!(!out.join("my-game-mobile.love").exists());
    }

    #[test]
    fn an_ios_project_carries_the_game_and_the_games_orientation() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path()); // 640x480: landscape
        let t = d.path().join("love");
        for dir in ["platform/xcode/love.xcodeproj", "platform/xcode/ios", "platform/xcode/Images.xcassets", "src"] {
            std::fs::create_dir_all(t.join(dir)).unwrap();
        }
        std::fs::write(t.join("platform/xcode/love.xcodeproj/project.pbxproj"), crate::apple_mobile::tests::MINI_PBX).unwrap();
        plist::Value::Dictionary(plist::Dictionary::new()).to_file_xml(t.join("platform/xcode/ios/love-ios.plist")).unwrap();
        let out = d.path().join("out");
        let xcode = build_ios(&Build { root: &root, out_dir: &out, gate: None, compat: None }, &t, Family::Iphone, Some("ABCDE12345"), None, false).unwrap();
        assert!(xcode.ends_with("my-game-ios-project/platform/xcode"));
        assert!(xcode.join("game.love").is_file());
        let p = std::fs::read_to_string(xcode.join("love.xcodeproj/project.pbxproj")).unwrap();
        assert!(p.contains("PRODUCT_BUNDLE_IDENTIFIER = dev.joyride.mygame;") && p.contains("DEVELOPMENT_TEAM = ABCDE12345;") && p.contains("TARGETED_DEVICE_FAMILY = 1;"), "{p}");
        let plist = plist::Value::from_file(xcode.join("ios/love-ios.plist")).unwrap();
        assert_eq!(plist.as_dictionary().unwrap()["UISupportedInterfaceOrientations"].as_array().unwrap().len(), 2, "landscape: left and right");
        // Needing the iOS libraries is an explicit choice, with an instruction when they are missing.
        let e = build_ios(&Build { root: &root, out_dir: &d.path().join("o2"), gate: None, compat: None }, &t, Family::Universal, None, None, true).unwrap_err().to_string();
        assert!(e.contains("love-apple-dependencies"), "{e}");
    }

    fn web_template(d: &Path) -> PathBuf {
        let s = d.join("lovejs");
        for f in ["release", "compat"] {
            std::fs::create_dir_all(s.join(f).join("theme")).unwrap();
            std::fs::write(s.join(f).join("index.html"), "{{title}} {{{arguments}}} {{memory}}").unwrap();
            std::fs::write(s.join(f).join("love.js"), "js").unwrap();
            std::fs::write(s.join(f).join("love.wasm"), "wasm").unwrap();
        }
        std::fs::write(s.join("release/love.worker.js"), "w").unwrap();
        std::fs::write(s.join("game.js"), "{{{create_file_paths}}} loadPackage({{{metadata}}});").unwrap();
        s
    }

    #[test]
    fn a_web_page_carries_the_web_build_and_a_12_project_needs_its_variant() {
        let d = tempfile::tempdir().unwrap();
        let root = project(d.path());
        let t = web_template(d.path());
        let out = d.path().join("out");
        let page = build_web(&Build { root: &root, out_dir: &out, gate: None, compat: None }, &t, true).unwrap();
        assert!(page.ends_with("my-game-web-compat") && page.join("game.data").is_file() && page.join("index.html").is_file());
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(std::fs::read(page.join("game.data")).unwrap())).unwrap();
        let mut shim = String::new();
        z.by_name("_jr_shim.lua").unwrap().read_to_string(&mut shim).unwrap();
        assert!(shim.contains("io.popen") && shim.contains("web"), "the web shim is in the game");
        assert!(!out.join("my-game-web.love").exists());
        // A 12.0 project is refused for the web until it builds its 11.5 variant.
        let d2 = tempfile::tempdir().unwrap();
        let p12 = project12(d2.path(), "love.event.restart()");
        let t2 = web_template(d2.path());
        let e = build_web(&Build { root: &p12, out_dir: &d2.path().join("o"), gate: None, compat: None }, &t2, false).unwrap_err().to_string();
        assert!(e.contains("11.5 variant"), "{e}");
        let ok = build_web(&Build { root: &p12, out_dir: &d2.path().join("o"), gate: None, compat: Some(Compat { allow: &[] }) }, &t2, false).unwrap();
        assert!(ok.join("_headers").is_file() && ok.join("love.worker.js").is_file());
    }

    #[test]
    fn a_missing_main_lua_is_reported() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("g");
        std::fs::create_dir_all(&root).unwrap();
        assert!(build_portmaster(&Build { root: &root, out_dir: &d.path().join("o"), gate: None, compat: None }).is_err());
    }
}
