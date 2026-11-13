//! iOS and iPadOS (spec 5.2): an Xcode project around the game.
//!
//! Written against the real `love` repository's `platform/xcode/love.xcodeproj`
//! (`11.x` and `main`, read 2026-11-13; both have the same target and ids): the
//! `love-ios` target, `ios/love-ios.plist`, `Images.xcassets` and a `Resources`
//! build phase the game is added to. The game becomes `platform/xcode/game.love`,
//! a file reference in the project and an entry in that phase; the app's identity
//! is the target's build settings in `project.pbxproj`; orientation and
//! full-screen are `ios/love-ios.plist`. The `.pbxproj` is edited as text, only
//! at those places: it is not parsed whole.
//!
//! The template also needs LÖVE's iOS dependencies (`love-apple-dependencies`:
//! `iOS/libraries` into `platform/xcode/ios`, `shared` into `platform/xcode`).
//! Building (`xcodebuild`) compiles LÖVE's C++ and needs Xcode and those
//! libraries, so it is the caller's step and is **not run here**; what is
//! checked is that Xcode itself reads the edited project.

use crate::toolchain::LoveVersion;
use crate::{err, Result};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Iphone,
    Ipad,
    Universal,
}

impl Family {
    fn setting(self) -> &'static str {
        match self {
            Family::Iphone => "1",
            Family::Ipad => "2",
            Family::Universal => "1,2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Any,
    Landscape,
    Portrait,
}

#[derive(Debug, Clone)]
pub struct IosApp {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    pub family: Family,
    pub orientation: Orientation,
    /// Apple Developer team id, for signing; empty leaves it for Xcode.
    pub team: Option<String>,
    pub love_version: LoveVersion,
}

const XCODE: &str = "platform/xcode";
const TARGET: &str = "love-ios";
const IOS_PLIST_SETTING: &str = "INFOPLIST_FILE = \"$(SRCROOT)/ios/love-ios.plist\";";

/// A pbxproj value: bare if simple, else quoted.
fn pbx(v: &str) -> String {
    if !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')) {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// A 24-hex id from a name, not already in the project.
fn unique_id(pbxproj: &str, name: &str) -> String {
    let mut n = 0u32;
    loop {
        let h = blake3::hash(format!("joyride:{name}:{n}").as_bytes()).to_hex().to_uppercase();
        let id = h[..24].to_string();
        if !pbxproj.contains(&id) {
            return id;
        }
        n += 1;
    }
}

/// The marketing version must be dotted numbers; anything else becomes 1.0.
fn marketing_version(v: &str) -> String {
    let ok = !v.is_empty() && v.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) && v.split('.').count() <= 3;
    if ok { v.to_string() } else { "1.0".into() }
}

/// Adds `game.love` to the project: a file reference next to `love-ios.plist`,
/// a build file, and an entry in the `love-ios` target's Resources phase.
pub fn add_game_to_pbxproj(text: &str) -> Result<String> {
    // The Resources phase the love-ios target lists, found through the target.
    let target_start = text.find(&format!("/* {TARGET} */ = {{\n\t\t\tisa = PBXNativeTarget;")).ok_or_else(|| crate::Error::Msg("no love-ios target in the project".into()))?;
    let target = &text[target_start..];
    let phase_id = target
        .lines()
        .skip_while(|l| !l.contains("buildPhases = ("))
        .skip(1)
        .take_while(|l| !l.contains(");"))
        .find(|l| l.contains("/* Resources */"))
        .and_then(|l| l.split_whitespace().next())
        .ok_or_else(|| crate::Error::Msg("the love-ios target has no Resources phase".into()))?
        .to_string();
    let file_id = unique_id(text, "game.love ref");
    let build_id = unique_id(text, "game.love build");

    let mut out = Vec::new();
    let (mut in_phase, mut added_ref, mut added_group, mut added_build, mut added_phase) = (false, false, false, false, false);
    for line in text.lines() {
        out.push(line.to_string());
        if line.starts_with("/* Begin PBXBuildFile section */") && !added_build {
            out.push(format!("\t\t{build_id} /* game.love in Resources */ = {{isa = PBXBuildFile; fileRef = {file_id} /* game.love */; }};"));
            added_build = true;
        }
        if line.starts_with("/* Begin PBXFileReference section */") && !added_ref {
            out.push(format!("\t\t{file_id} /* game.love */ = {{isa = PBXFileReference; lastKnownFileType = file; name = game.love; path = game.love; sourceTree = \"<group>\"; }};"));
            added_ref = true;
        }
        if line.contains("/* love-ios.plist */,") && !added_group {
            out.push(format!("\t\t\t\t{file_id} /* game.love */,"));
            added_group = true;
        }
        if line.trim_start().starts_with(&format!("{phase_id} /* Resources */ = {{")) {
            in_phase = true;
        }
        if in_phase && line.trim() == "files = (" && !added_phase {
            out.push(format!("\t\t\t\t{build_id} /* game.love in Resources */,"));
            added_phase = true;
            in_phase = false;
        }
    }
    if !(added_ref && added_group && added_build && added_phase) {
        return err("the project does not have the layout of LÖVE's Xcode project (build file, file reference, group or Resources phase not found)");
    }
    Ok(out.join("\n") + "\n")
}

/// Sets build settings on the `love-ios` target's configurations (the ones that
/// use `ios/love-ios.plist`), replacing or adding each. Other targets' settings
/// (the macOS app shares `PRODUCT_NAME` and the bundle id) are untouched.
pub fn set_target_settings(text: &str, edits: &[(&str, String)]) -> Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut edited = 0;
    while i < lines.len() {
        // A configuration block: `\t\tID /* Name */ = {` ... `\t\t};`
        if lines[i].starts_with("\t\t") && !lines[i].starts_with("\t\t\t") && lines[i].ends_with("= {") && lines.get(i + 1).is_some_and(|l| l.contains("isa = XCBuildConfiguration;")) {
            let end = (i..lines.len()).find(|&j| lines[j] == "\t\t};").unwrap_or(lines.len() - 1);
            let block = &lines[i..=end];
            if block.iter().any(|l| l.contains(IOS_PLIST_SETTING)) {
                let mut b: Vec<String> = block.iter().map(|s| s.to_string()).collect();
                for (key, value) in edits {
                    let new = format!("\t\t\t\t{key} = {};", pbx(value));
                    let pos = b.iter().position(|l| l.trim_start().starts_with(&format!("{key} = ")) && l.starts_with("\t\t\t\t") && !l.starts_with("\t\t\t\t\t"));
                    match pos {
                        Some(p) => b[p] = new,
                        None => {
                            // Before the closing of buildSettings (the `};` at four tabs).
                            let close = b.iter().rposition(|l| l == "\t\t\t};").unwrap_or(b.len() - 2);
                            b.insert(close, new);
                        }
                    }
                }
                out.extend(b);
                edited += 1;
            } else {
                out.extend(block.iter().map(|s| s.to_string()));
            }
            i = end + 1;
        } else {
            out.push(lines[i].to_string());
            i += 1;
        }
    }
    if edited < 2 {
        return err(format!("found {edited} love-ios build configuration(s) to edit, expected Debug and Release"));
    }
    Ok(out.join("\n") + "\n")
}

fn orientations(o: Orientation, ipad: bool) -> Vec<&'static str> {
    let land = ["UIInterfaceOrientationLandscapeLeft", "UIInterfaceOrientationLandscapeRight"];
    let port = ["UIInterfaceOrientationPortrait"];
    let port_ipad = ["UIInterfaceOrientationPortrait", "UIInterfaceOrientationPortraitUpsideDown"];
    match o {
        Orientation::Landscape => land.to_vec(),
        Orientation::Portrait => if ipad { port_ipad.to_vec() } else { port.to_vec() },
        Orientation::Any => if ipad { [&land[..], &port_ipad[..]].concat() } else { [&land[..], &port[..]].concat() },
    }
}

/// Rewrites `ios/love-ios.plist`: orientation, full-screen, no file sharing,
/// and no `.love` document or type declaration (the game must not claim `.love`
/// files).
pub fn patch_info_plist(path: &Path, app: &IosApp) -> Result<()> {
    let mut v = plist::Value::from_file(path).map_err(|e| crate::Error::Msg(format!("{}: {e}", path.display())))?;
    let d = v.as_dictionary_mut().ok_or_else(|| crate::Error::Msg("love-ios.plist is not a dictionary".into()))?;
    let arr = |o: Orientation, ipad| plist::Value::Array(orientations(o, ipad).into_iter().map(|s| s.into()).collect());
    d.insert("UISupportedInterfaceOrientations".into(), arr(app.orientation, false));
    d.insert("UISupportedInterfaceOrientations~ipad".into(), arr(app.orientation, true));
    d.insert("UIRequiresFullScreen".into(), true.into());
    d.insert("UIFileSharingEnabled".into(), false.into());
    for k in ["CFBundleDocumentTypes", "UTExportedTypeDeclarations"] {
        d.remove(k);
    }
    v.to_file_xml(path).map_err(|e| crate::Error::Msg(e.to_string()))
}

/// Replaces the app icon with one 1024 px image (Xcode 14 and later accept a
/// single size and make the rest).
fn write_icon(xcassets: &Path, icon: &Path) -> Result<()> {
    let set = xcassets.join("iOS AppIcon.appiconset");
    if !set.is_dir() {
        return err("the template has no iOS AppIcon set");
    }
    for e in fs::read_dir(&set)?.flatten() {
        fs::remove_file(e.path())?;
    }
    let img = image::open(icon).map_err(|e| crate::Error::Msg(format!("icon {}: {e}", icon.display())))?;
    img.resize_exact(1024, 1024, image::imageops::FilterType::Lanczos3).to_rgb8().save(set.join("icon-1024.png")).map_err(|e| crate::Error::Msg(e.to_string()))?;
    fs::write(set.join("Contents.json"), "{\n  \"images\" : [\n    {\n      \"filename\" : \"icon-1024.png\",\n      \"idiom\" : \"universal\",\n      \"platform\" : \"ios\",\n      \"size\" : \"1024x1024\"\n    }\n  ],\n  \"info\" : {\n    \"author\" : \"xcode\",\n    \"version\" : 1\n  }\n}\n")?;
    Ok(())
}

fn check_template(t: &Path, need_libraries: bool) -> Result<()> {
    for f in ["platform/xcode/love.xcodeproj/project.pbxproj", "platform/xcode/ios/love-ios.plist", "platform/xcode/Images.xcassets", "src"] {
        if !t.join(f).exists() {
            return err(format!("{} is not a love checkout for iOS: no {f}", t.display()));
        }
    }
    if need_libraries && !t.join("platform/xcode/ios/libraries").is_dir() {
        return err("the template lacks LÖVE's iOS libraries: download love-apple-dependencies for this LÖVE version and put `iOS/libraries` into platform/xcode/ios and `shared` into platform/xcode");
    }
    Ok(())
}

/// Copies the template to `out`, adds the game and sets the app's identity.
/// Returns the Xcode project folder (`…/platform/xcode`). `libraries` demands
/// the iOS dependencies be in place (leave false to prepare a project to which
/// they are added later).
pub fn stage_project(template: &Path, game_love: &Path, out: &Path, app: &IosApp, icon: Option<&Path>, libraries: bool) -> Result<PathBuf> {
    check_template(template, libraries)?;
    if !game_love.is_file() {
        return err(format!("{} not found", game_love.display()));
    }
    if out.exists() && fs::read_dir(out)?.next().is_some() {
        return err(format!("{} is not empty", out.display()));
    }
    fs::create_dir_all(out)?;
    let skip = |rel: &Path| matches!(rel.components().next().and_then(|c| c.as_os_str().to_str()), Some(".git" | ".github" | "build" | "DerivedData"));
    for e in WalkDir::new(template).sort_by_file_name().into_iter().filter_entry(|e| !skip(e.path().strip_prefix(template).unwrap_or(e.path()))) {
        let e = e.map_err(|e| crate::Error::Msg(e.to_string()))?;
        let rel = e.path().strip_prefix(template).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let to = out.join(rel);
        let ft = e.file_type();
        if ft.is_dir() {
            fs::create_dir_all(&to)?;
        } else if ft.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(e.path())?, &to)?;
        } else {
            fs::copy(e.path(), &to)?;
        }
    }
    let xcode = out.join(XCODE);
    fs::copy(game_love, xcode.join("game.love"))?;
    let pbxproj = xcode.join("love.xcodeproj/project.pbxproj");
    let mut text = add_game_to_pbxproj(&fs::read_to_string(&pbxproj)?)?;
    let mut edits = vec![
        ("PRODUCT_BUNDLE_IDENTIFIER", app.bundle_id.clone()),
        ("PRODUCT_NAME", app.name.clone()),
        ("MARKETING_VERSION", marketing_version(&app.version)),
        ("TARGETED_DEVICE_FAMILY", app.family.setting().to_string()),
    ];
    if let Some(t) = app.team.as_ref().filter(|t| !t.is_empty()) {
        edits.push(("DEVELOPMENT_TEAM", t.clone()));
    }
    text = set_target_settings(&text, &edits)?;
    fs::write(&pbxproj, text)?;
    patch_info_plist(&xcode.join("ios/love-ios.plist"), app)?;
    if let Some(icon) = icon {
        write_icon(&xcode.join("Images.xcassets"), icon)?;
    }
    Ok(xcode)
}

/// `xcodebuild` arguments for the simulator (`device = false`) or an unsigned
/// device build (the `.app` of which [`make_unsigned_ipa`] packs for
/// sideloading), from the staged project.
pub fn xcodebuild_args(xcode_dir: &Path, device: bool) -> Vec<String> {
    ["-project", &xcode_dir.join("love.xcodeproj").to_string_lossy(), "-target", TARGET, "-configuration", "Release", "-sdk", if device { "iphoneos" } else { "iphonesimulator" }, "CODE_SIGNING_ALLOWED=NO", "build"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Where `xcodebuild -target` leaves the app.
pub fn built_app_path(xcode_dir: &Path, device: bool) -> PathBuf {
    xcode_dir.join("build").join(if device { "Release-iphoneos" } else { "Release-iphonesimulator" }).join("love.app")
}

/// An unsigned `.ipa` (for AltStore or SideStore, which sign on the device):
/// `Payload/<name>.app/…`, executable bits kept.
pub fn make_unsigned_ipa(app_dir: &Path, name: &str, out: &Path) -> Result<()> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    if !app_dir.is_dir() {
        return err(format!("{} is not a built app", app_dir.display()));
    }
    let mut z = zip::ZipWriter::new(File::create(out)?);
    let base = format!("Payload/{name}.app");
    for e in WalkDir::new(app_dir).sort_by_file_name().into_iter().flatten() {
        let rel = e.path().strip_prefix(app_dir).unwrap();
        let path = if rel.as_os_str().is_empty() { base.clone() } else { format!("{base}/{}", rel.to_string_lossy().replace('\\', "/")) };
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            e.metadata().map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644)
        };
        #[cfg(not(unix))]
        let mode = 0o644;
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(mode);
        if e.file_type().is_dir() {
            z.add_directory(format!("{path}/"), opts)?;
        } else if e.file_type().is_file() {
            z.start_file(path, opts)?;
            z.write_all(&fs::read(e.path())?)?;
        }
    }
    z.finish()?;
    Ok(())
}

/// `ExportOptions.plist` for `xcodebuild -exportArchive`: `method` is
/// `app-store-connect`, `ad-hoc` or `development`.
pub fn export_options_plist(method: &str, team: &str) -> Result<String> {
    if !["app-store-connect", "ad-hoc", "development"].contains(&method) {
        return err(format!("unknown export method `{method}`"));
    }
    if team.is_empty() || !team.chars().all(|c| c.is_ascii_alphanumeric()) {
        return err("a signed export needs the Apple Developer team id (letters and digits)");
    }
    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>method</key>\n\t<string>{method}</string>\n\t<key>teamID</key>\n\t<string>{team}</string>\n</dict>\n</plist>\n"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const MINI_PBX: &str = "// !$*UTF8*$!\n{\n\tobjects = {\n\n/* Begin PBXBuildFile section */\n\t\tAAAAAAAAAAAAAAAAAAAAAAA1 /* x in Resources */ = {isa = PBXBuildFile; };\n/* End PBXBuildFile section */\n\n/* Begin PBXFileReference section */\n\t\tFA5D24971A96CE1E00C6FC8F /* love-ios.plist */ = {isa = PBXFileReference; path = \"ios/love-ios.plist\"; sourceTree = \"<group>\"; };\n/* End PBXFileReference section */\n\n/* Begin PBXGroup section */\n\t\tFA5D24A01A96CF7200C6FC8F /* Supporting Files */ = {\n\t\t\tisa = PBXGroup;\n\t\t\tchildren = (\n\t\t\t\tFA5D24971A96CE1E00C6FC8F /* love-ios.plist */,\n\t\t\t);\n\t\t};\n/* End PBXGroup section */\n\n/* Begin PBXNativeTarget section */\n\t\tFA0B7F051A95AAF3000E1D17 /* love-ios */ = {\n\t\t\tisa = PBXNativeTarget;\n\t\t\tbuildPhases = (\n\t\t\t\tFA0B7F021A95AAF3000E1D17 /* Sources */,\n\t\t\t\tFA0B7F041A95AAF3000E1D17 /* Resources */,\n\t\t\t);\n\t\t\tname = \"love-ios\";\n\t\t};\n/* End PBXNativeTarget section */\n\n/* Begin PBXResourcesBuildPhase section */\n\t\t8D1107290486CEB800E47090 /* Resources */ = {\n\t\t\tisa = PBXResourcesBuildPhase;\n\t\t\tfiles = (\n\t\t\t\tMACOSONLY /* license.txt in Resources */,\n\t\t\t);\n\t\t};\n\t\tFA0B7F041A95AAF3000E1D17 /* Resources */ = {\n\t\t\tisa = PBXResourcesBuildPhase;\n\t\t\tfiles = (\n\t\t\t\tIOSONLY /* Images.xcassets in Resources */,\n\t\t\t);\n\t\t};\n/* End PBXResourcesBuildPhase section */\n\n/* Begin XCBuildConfiguration section */\n\t\tMAC00000000000000000000A /* Debug */ = {\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbuildSettings = {\n\t\t\t\tINFOPLIST_FILE = \"macosx/love-macosx.plist\";\n\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = org.love2d.love;\n\t\t\t\tPRODUCT_NAME = love;\n\t\t\t};\n\t\t\tname = Debug;\n\t\t};\n\t\tIOS0000000000000000000A1 /* Debug */ = {\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbuildSettings = {\n\t\t\t\tINFOPLIST_FILE = \"$(SRCROOT)/ios/love-ios.plist\";\n\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = org.love2d.love;\n\t\t\t\tPRODUCT_NAME = love;\n\t\t\t\tTARGETED_DEVICE_FAMILY = \"1,2\";\n\t\t\t};\n\t\t\tname = Debug;\n\t\t};\n\t\tIOS0000000000000000000R1 /* Release */ = {\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbuildSettings = {\n\t\t\t\tINFOPLIST_FILE = \"$(SRCROOT)/ios/love-ios.plist\";\n\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = org.love2d.love;\n\t\t\t\tPRODUCT_NAME = love;\n\t\t\t};\n\t\t\tname = Release;\n\t\t};\n/* End XCBuildConfiguration section */\n\t};\n}\n";

    #[test]
    fn the_game_is_added_in_all_four_places_and_only_to_the_ios_phase() {
        let t = add_game_to_pbxproj(MINI_PBX).unwrap();
        let lines = |needle: &str| t.lines().filter(|l| l.contains(needle)).count();
        assert_eq!((lines("game.love in Resources */ = {isa = PBXBuildFile"), lines("game.love */ = {isa = PBXFileReference"), lines("game.love in Resources */,"), lines("/* game.love */,")), (1, 1, 1, 1), "one line each:\n{t}");
        let build_id = t.lines().find(|l| l.contains("game.love in Resources */ = {isa = PBXBuildFile")).unwrap().split_whitespace().next().unwrap().to_string();
        // It is in the iOS Resources phase, not the macOS one.
        let mac = &t[t.find("8D1107290486CEB800E47090 /* Resources */").unwrap()..t.find("FA0B7F041A95AAF3000E1D17 /* Resources */ = {").unwrap()];
        let ios = &t[t.find("FA0B7F041A95AAF3000E1D17 /* Resources */ = {").unwrap()..];
        assert!(!mac.contains(&build_id) && ios.contains(&build_id));
        assert!(t.contains("path = game.love; sourceTree = \"<group>\""));
        assert!(add_game_to_pbxproj("{ not a love project }").unwrap_err().to_string().contains("no love-ios target"));
    }

    #[test]
    fn settings_change_only_the_ios_configurations() {
        let t = set_target_settings(MINI_PBX, &[("PRODUCT_BUNDLE_IDENTIFIER", "dev.me.game".into()), ("PRODUCT_NAME", "My Game".into()), ("DEVELOPMENT_TEAM", "ABCDE12345".into()), ("TARGETED_DEVICE_FAMILY", "2".into())]).unwrap();
        // macOS config untouched.
        let mac = &t[t.find("MAC00000000000000000000A").unwrap()..t.find("IOS0000000000000000000A1").unwrap()];
        assert!(mac.contains("PRODUCT_BUNDLE_IDENTIFIER = org.love2d.love;") && mac.contains("PRODUCT_NAME = love;") && !mac.contains("DEVELOPMENT_TEAM"));
        for cfg in ["IOS0000000000000000000A1", "IOS0000000000000000000R1"] {
            let b = &t[t.find(cfg).unwrap()..];
            let b = &b[..b.find("\t\t};").unwrap()];
            for line in ["PRODUCT_BUNDLE_IDENTIFIER = dev.me.game;", "PRODUCT_NAME = \"My Game\";", "DEVELOPMENT_TEAM = ABCDE12345;", "TARGETED_DEVICE_FAMILY = 2;"] {
                assert!(b.contains(line), "{line} in {cfg}:\n{b}");
            }
            assert_eq!(b.matches("TARGETED_DEVICE_FAMILY").count(), 1, "replaced, not duplicated");
        }
        assert!(set_target_settings("no configs here", &[("A", "b".into())]).is_err());
    }

    #[test]
    fn values_are_quoted_when_they_need_to_be() {
        assert_eq!(pbx("dev.me.game"), "dev.me.game");
        assert_eq!(pbx("My Game"), "\"My Game\"");
        assert_eq!(pbx("a\"b"), "\"a\\\"b\"");
        assert_eq!(pbx(""), "\"\"");
        assert_eq!(marketing_version("1.2.3"), "1.2.3");
        assert_eq!(marketing_version("1.0-beta"), "1.0");
        assert_eq!(marketing_version("1.2.3.4"), "1.0");
    }

    #[test]
    fn export_options_check_their_inputs() {
        let p = export_options_plist("ad-hoc", "ABCDE12345").unwrap();
        assert!(p.contains("<string>ad-hoc</string>") && p.contains("<string>ABCDE12345</string>"));
        assert!(export_options_plist("anything", "ABC").is_err());
        assert!(export_options_plist("ad-hoc", "").is_err());
        assert!(export_options_plist("ad-hoc", "A<B").is_err());
    }

    #[test]
    fn an_unsigned_ipa_is_payload_slash_app_with_executable_bits() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("love.app");
        fs::create_dir_all(app.join("Frameworks")).unwrap();
        fs::write(app.join("love"), b"bin").unwrap();
        fs::write(app.join("Info.plist"), b"p").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(app.join("love"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let ipa = d.path().join("g.ipa");
        make_unsigned_ipa(&app, "My Game", &ipa).unwrap();
        let mut z = zip::ZipArchive::new(File::open(&ipa).unwrap()).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"Payload/My Game.app/love".to_string()) && names.contains(&"Payload/My Game.app/Info.plist".to_string()), "{names:?}");
        #[cfg(unix)]
        assert_eq!(z.by_name("Payload/My Game.app/love").unwrap().unix_mode().unwrap() & 0o111, 0o111);
        assert!(make_unsigned_ipa(&d.path().join("nope.app"), "x", &d.path().join("x.ipa")).is_err());
    }

    #[test]
    fn xcodebuild_args_target_the_ios_target_unsigned() {
        let a = xcodebuild_args(Path::new("/p/platform/xcode"), true);
        assert!(a.windows(2).any(|w| w == ["-target", "love-ios"]) && a.windows(2).any(|w| w == ["-sdk", "iphoneos"]) && a.contains(&"CODE_SIGNING_ALLOWED=NO".to_string()));
        assert!(xcodebuild_args(Path::new("/p"), false).contains(&"iphonesimulator".to_string()));
        assert!(built_app_path(Path::new("/p"), false).ends_with("Release-iphonesimulator/love.app"));
    }

    /// With a real `love` checkout (`JOYRIDE_LOVE_SRC`, branch main or 11.x):
    /// the game is added, the identity set, and Xcode itself reads the result.
    #[test]
    fn the_real_template_is_edited_and_xcode_reads_it() {
        let Ok(t) = std::env::var("JOYRIDE_LOVE_SRC") else { return };
        let d = tempfile::tempdir().unwrap();
        let love = d.path().join("g.love");
        fs::write(&love, b"PK-game").unwrap();
        let app = IosApp { name: "Café Run".into(), bundle_id: "dev.joyride.cafe".into(), version: "1.2.3".into(), family: Family::Universal, orientation: Orientation::Landscape, team: Some("ABCDE12345".into()), love_version: LoveVersion::parse("12.0").unwrap() };
        let icon = d.path().join("icon.png");
        image::RgbaImage::from_pixel(512, 512, image::Rgba([10, 200, 30, 255])).save(&icon).unwrap();
        let xcode = stage_project(Path::new(&t), &love, &d.path().join("out"), &app, Some(&icon), false).unwrap();
        assert_eq!(fs::read(xcode.join("game.love")).unwrap(), b"PK-game");
        let p = fs::read_to_string(xcode.join("love.xcodeproj/project.pbxproj")).unwrap();
        assert!(p.contains("PRODUCT_BUNDLE_IDENTIFIER = dev.joyride.cafe;") && p.contains("MARKETING_VERSION = 1.2.3;"));
        // Six in the template: the iOS target's Debug, Release and Distribution are ours now; the rest are the macOS target's and stay.
        assert_eq!(p.matches("PRODUCT_BUNDLE_IDENTIFIER = dev.joyride.cafe;").count(), 3);
        assert_eq!(p.matches("PRODUCT_BUNDLE_IDENTIFIER = org.love2d.love;").count(), 3, "the macOS target keeps LÖVE's identity");
        let plist = plist::Value::from_file(xcode.join("ios/love-ios.plist")).unwrap();
        let o = plist.as_dictionary().unwrap();
        assert!(!o.contains_key("CFBundleDocumentTypes") && o["UIRequiresFullScreen"].as_boolean() == Some(true));
        assert_eq!(o["UISupportedInterfaceOrientations"].as_array().unwrap().len(), 2);
        assert_eq!(image::image_dimensions(xcode.join("Images.xcassets/iOS AppIcon.appiconset/icon-1024.png")).unwrap(), (1024, 1024));
        // Xcode's own parser accepts the edited project (it lists the targets); skipped without Xcode.
        if Path::new("/usr/bin/xcodebuild").exists() {
            let o = std::process::Command::new("xcodebuild").arg("-list").arg("-project").arg(xcode.join("love.xcodeproj")).output().unwrap();
            let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
            if !text.contains("xcode-select") && !text.contains("license") {
                assert!(o.status.success() && text.contains("love-ios"), "xcodebuild -list:\n{text}");
            }
        }
    }
}
