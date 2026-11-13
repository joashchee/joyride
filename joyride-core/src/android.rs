//! Android (spec 5.3): the `love-android` Gradle project with the game inside.
//!
//! Written against the real template's layout, read from its `11.x` and
//! `12.x` branches (2026-11-13): the game goes in `app/src/embed/assets/` as
//! `game.love`, the app's identity is `gradle.properties`
//! (`app.name`, `app.application_id`, `app.orientation`, `app.version_code`,
//! `app.version_name`), the icon is `app/src/main/res/drawable-*/love.png`, and
//! the `embed` flavor's release task makes the APK or bundle. This module
//! prepares that project and builds the commands; it never runs Gradle (that
//! downloads and executes the template's build scripts and needs the Android
//! SDK and NDK), so a build is the caller's step and is **not tested here**.
//!
//! The template needs its submodules (`love`, and `megasource` on 12.x)
//! checked out: `git submodule update --init --recursive`.

use crate::toolchain::LoveVersion;
use crate::{err, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct AndroidApp {
    pub name: String,
    /// The project's bundle id; made a valid Android application id.
    pub bundle_id: String,
    pub version_name: String,
    /// Landscape if the game's window is wider than tall.
    pub landscape: bool,
    pub love_version: LoveVersion,
}

/// Android application ids: dot-separated segments of letters, digits and
/// underscores, each starting with a letter. A hyphen becomes `_`; a segment
/// starting with a digit gets a `_` in front.
pub fn application_id(bundle_id: &str) -> String {
    let segs: Vec<String> = bundle_id
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            if s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { s } else { format!("_{s}") }
        })
        .collect();
    if segs.len() >= 2 { segs.join(".") } else { format!("dev.joyride.{}", segs.first().cloned().unwrap_or_else(|| "game".into())) }
}

/// `1.2.3` becomes 10203 (major*10000 + minor*100 + patch): increases with the
/// version, which is all Android asks of it. A version that doesn't parse is 1.
pub fn version_code(v: &str) -> u32 {
    let n: Vec<u32> = v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).take(3).filter_map(|p| p.parse().ok()).collect();
    match n.as_slice() {
        [] => 1,
        [a] => a * 10000,
        [a, b] => a * 10000 + b * 100,
        [a, b, c, ..] => a * 10000 + b * 100 + c,
    }
    .clamp(1, 2_100_000_000)
}

/// A `gradle.properties` value for the app name: ASCII as is; for 12.x
/// `\uXXXX` escapes (a Java properties file reads them).
fn escape_name(name: &str) -> String {
    let mut o = String::new();
    for c in name.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            c if c.is_ascii() && !c.is_control() => o.push(c),
            c => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    o.push_str(&format!("\\u{u:04X}"));
                }
            }
        }
    }
    o
}

/// Sets, replaces or removes `key=value` lines, keeping every other line.
pub fn set_properties(text: &str, edits: &[(&str, Option<String>)]) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    for (key, value) in edits {
        let is_key = |l: &str| {
            let t = l.trim_start();
            !t.starts_with('#') && t.split(['=', ':']).next().is_some_and(|k| k.trim() == *key)
        };
        match value {
            Some(v) => {
                let new = format!("{key}={v}");
                if let Some(i) = lines.iter().position(|l| is_key(l)) {
                    lines[i] = new;
                } else {
                    // Before the "no need to modify" footer if there is one, else at the end.
                    let at = lines.iter().position(|l| l.to_lowercase().contains("no need to modify")).unwrap_or(lines.len());
                    lines.insert(at, new);
                }
            }
            None => lines.retain(|l| !is_key(l)),
        }
    }
    lines.join("\n") + "\n"
}

fn uses_12_layout(template: &Path) -> bool {
    template.join("app/src/main/cpp").is_dir()
}

/// Checks the template is a `love-android` checkout with its sources.
fn check_template(t: &Path) -> Result<()> {
    for f in ["app/build.gradle", "app/src/main/AndroidManifest.xml", "gradle.properties", "gradlew"] {
        if !t.join(f).exists() {
            return err(format!("{} is not a love-android checkout: no {f}", t.display()));
        }
    }
    let native: Vec<PathBuf> = if uses_12_layout(t) {
        vec![t.join("app/src/main/cpp/love/src"), t.join("app/src/main/cpp/megasource")]
    } else {
        vec![t.join("love/src/jni/love/src")]
    };
    for n in native {
        if !n.is_dir() || fs::read_dir(&n)?.next().is_none() {
            return err(format!("the template's {} is empty: run `git submodule update --init --recursive` in it", n.strip_prefix(t).unwrap_or(&n).display()));
        }
    }
    Ok(())
}

/// Copies the template to `out`, puts the game in, and sets the app's
/// identity. `icon` (a square PNG, ideally 512 or more) replaces LÖVE's.
pub fn stage_project(template: &Path, game_love: &Path, out: &Path, app: &AndroidApp, icon: Option<&Path>) -> Result<()> {
    check_template(template)?;
    if !game_love.is_file() {
        return err(format!("{} not found", game_love.display()));
    }
    if out.exists() && fs::read_dir(out)?.next().is_some() {
        return err(format!("{} is not empty", out.display()));
    }
    let skip = |rel: &Path| {
        let first = rel.components().next().and_then(|c| c.as_os_str().to_str()).unwrap_or("");
        matches!(first, ".git" | ".gradle" | "build" | ".idea") || rel == Path::new("local.properties") || rel == Path::new("app/build")
    };
    fs::create_dir_all(out)?;
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
            // Submodule checkouts and Gradle wrappers have none that matter; copy what they point at.
            if let Ok(meta) = fs::metadata(e.path()) {
                if meta.is_file() {
                    fs::copy(e.path(), &to)?;
                }
            }
        } else {
            fs::copy(e.path(), &to)?; // keeps the permission bits: gradlew stays executable
        }
    }
    // The game: a .love in the embed flavor's assets, and nothing else there.
    let assets = out.join("app/src/embed/assets");
    fs::create_dir_all(&assets)?;
    for e in fs::read_dir(&assets)?.flatten() {
        if e.file_type()?.is_dir() { fs::remove_dir_all(e.path())? } else { fs::remove_file(e.path())? }
    }
    fs::copy(game_love, assets.join("game.love"))?;

    // The app's identity.
    let props = out.join("gradle.properties");
    let name_edits: Vec<(&str, Option<String>)> = if app.love_version.is_12_or_later() || app.name.is_ascii() {
        vec![("app.name", Some(escape_name(&app.name))), ("app.name_byte_array", None)]
    } else {
        // 11.x reads non-ASCII names from a UTF-8 byte list.
        vec![("app.name", None), ("app.name_byte_array", Some(app.name.bytes().map(|b| b.to_string()).collect::<Vec<_>>().join(",")))]
    };
    let mut edits = name_edits;
    edits.extend([
        ("app.application_id", Some(application_id(&app.bundle_id))),
        ("app.orientation", Some(if app.landscape { "landscape" } else { "portrait" }.to_string())),
        ("app.version_code", Some(version_code(&app.version_name).to_string())),
        ("app.version_name", Some(escape_name(&app.version_name))),
    ]);
    fs::write(&props, set_properties(&fs::read_to_string(&props)?, &edits))?;

    if let Some(icon) = icon {
        write_icons(&out.join("app/src/main/res"), icon)?;
    }
    Ok(())
}

/// The launcher icon sizes the template uses (`drawable-*/love.png`).
const ICONS: [(&str, u32); 5] = [("drawable-mdpi", 48), ("drawable-hdpi", 72), ("drawable-xhdpi", 96), ("drawable-xxhdpi", 144), ("drawable-xxxhdpi", 192)];

fn write_icons(res: &Path, icon: &Path) -> Result<()> {
    let img = image::open(icon).map_err(|e| crate::Error::Msg(format!("icon {}: {e}", icon.display())))?;
    for (dir, px) in ICONS {
        let d = res.join(dir);
        if d.is_dir() {
            img.resize_exact(px, px, image::imageops::FilterType::Lanczos3).save(d.join("love.png")).map_err(|e| crate::Error::Msg(e.to_string()))?;
        }
    }
    Ok(())
}

/// `./gradlew` arguments: an APK, or an Android App Bundle for Google Play.
/// The embed flavor with no microphone permission, as the template documents.
pub fn gradle_args(bundle: bool, uses_microphone: bool) -> Vec<String> {
    let flavor = if uses_microphone { "EmbedRecord" } else { "EmbedNoRecord" };
    vec![format!("{}{flavor}Release", if bundle { "bundle" } else { "assemble" })]
}

/// `keytool` arguments to make a signing key. The password is not here: pass
/// it on the command's standard input or through the tool's prompt, never as an
/// argument (arguments show in process lists).
pub fn keytool_args(keystore: &Path, alias: &str, distinguished_name: &str) -> Vec<String> {
    ["-genkeypair", "-v", "-keystore", &keystore.to_string_lossy(), "-alias", alias, "-keyalg", "RSA", "-keysize", "4096", "-validity", "10000", "-dname", distinguished_name]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// `apksigner sign` arguments (Android build-tools). The keystore and key
/// passwords are read from environment variables, never put on the command line.
pub fn apksigner_args(keystore: &Path, alias: &str, pass_env: &str, key_pass_env: &str, apk: &Path) -> Vec<String> {
    ["sign", "--ks", &keystore.to_string_lossy(), "--ks-key-alias", alias, "--ks-pass", &format!("env:{pass_env}"), "--key-pass", &format!("env:{key_pass_env}"), &apk.to_string_lossy()]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Where Gradle writes the APK or bundle for the embed, no-record release.
pub fn output_path(project: &Path, bundle: bool) -> PathBuf {
    if bundle { project.join("app/build/outputs/bundle/embedNoRecordRelease") } else { project.join("app/build/outputs/apk/embedNoRecord/release") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_ids() {
        assert_eq!(application_id("dev.joyride.my-game"), "dev.joyride.my_game");
        assert_eq!(application_id("com.ansiapps.2048"), "com.ansiapps._2048");
        assert_eq!(application_id("game"), "dev.joyride.game");
        assert_eq!(application_id(""), "dev.joyride.game");
        assert_eq!(application_id("a.b.c"), "a.b.c");
    }

    #[test]
    fn version_codes_increase_with_the_version() {
        assert_eq!(version_code("1.2.3"), 10203);
        assert!(version_code("1.10.0") > version_code("1.9.9"));
        assert_eq!(version_code("2"), 20000);
        assert_eq!(version_code("nonsense"), 1);
        assert_eq!(version_code("1.0.0-beta"), 10000);
    }

    #[test]
    fn properties_are_edited_in_place_and_footers_stay_last() {
        let t = "app.name=Old\n# comment app.id=x\napp.application_id=org.love2d.android\n\n# No need to modify anything past this line!\nandroid.useAndroidX=true\n";
        let r = set_properties(t, &[("app.name", Some("New".into())), ("app.application_id", Some("a.b".into())), ("app.version_code", Some("5".into())), ("app.nothing", None)]);
        assert!(r.contains("app.name=New\n") && r.contains("app.application_id=a.b\n") && r.contains("# comment app.id=x"));
        let code = r.find("app.version_code=5").unwrap();
        assert!(code < r.find("No need to modify").unwrap(), "new keys go above the footer:\n{r}");
        assert!(r.ends_with("android.useAndroidX=true\n"));
        assert!(!set_properties("app.name_byte_array=1,2\n", &[("app.name_byte_array", None)]).contains("byte_array"));
    }

    #[test]
    fn names_are_escaped_for_properties() {
        assert_eq!(escape_name("My Game"), "My Game");
        assert_eq!(escape_name("LÖVE"), "L\\u00D6VE");
        assert_eq!(escape_name("a\\b"), "a\\\\b");
        assert_eq!(escape_name("😀"), "\\uD83D\\uDE00");
    }

    #[test]
    fn signing_commands_never_carry_a_password() {
        let k = keytool_args(Path::new("/k/a.jks"), "key", "CN=Me");
        assert!(k.contains(&"-genkeypair".into()) && k.iter().all(|a| !a.to_lowercase().contains("pass")));
        let s = apksigner_args(Path::new("/k/a.jks"), "key", "JR_KS_PASS", "JR_KEY_PASS", Path::new("a.apk"));
        assert!(s.contains(&"env:JR_KS_PASS".to_string()) && s.contains(&"env:JR_KEY_PASS".to_string()));
        assert_eq!(gradle_args(false, false), ["assembleEmbedNoRecordRelease"]);
        assert_eq!(gradle_args(true, true), ["bundleEmbedRecordRelease"]);
    }

    /// A minimal stand-in for each branch of the real template, shaped as
    /// the real ones are (the layout was read from both on 2026-11-13).
    fn fake_template(d: &Path, twelve: bool, submodules: bool) -> PathBuf {
        let t = d.join("love-android");
        fs::create_dir_all(t.join("app/src/main/res/drawable-mdpi")).unwrap();
        fs::create_dir_all(t.join("app/src/main/res/drawable-xxxhdpi")).unwrap();
        fs::create_dir_all(t.join("app/src/embed/assets")).unwrap();
        fs::write(t.join("app/build.gradle"), "android {}").unwrap();
        fs::write(t.join("app/src/main/AndroidManifest.xml"), "<manifest/>").unwrap();
        fs::write(t.join("app/src/embed/assets/leftover.txt"), "x").unwrap();
        fs::write(t.join("gradlew"), "#!/bin/sh\n").unwrap();
        fs::create_dir_all(t.join(".git")).unwrap();
        fs::write(t.join(".git/HEAD"), "ref").unwrap();
        fs::create_dir_all(t.join("app/build/x")).unwrap();
        fs::write(t.join("local.properties"), "sdk.dir=/Users/me/sdk").unwrap();
        fs::write(t.join("app/src/main/res/drawable-mdpi/love.png"), b"old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(t.join("gradlew"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (props, native): (&str, &[&str]) = if twelve {
            ("app.name=LÖVE for Android\napp.application_id=org.love2d.android\napp.orientation=landscape\napp.version_code=33\napp.version_name=12.0\n\n# No need to modify anything past this line!\nandroid.useAndroidX=true\n", &["app/src/main/cpp/love/src", "app/src/main/cpp/megasource"])
        } else {
            ("#app.name=LÖVE for Android\napp.name_byte_array=76,195,150,86,69\napp.application_id=org.love2d.android\napp.orientation=landscape\napp.version_code=32\napp.version_name=11.5a\n\n# No need to modify anything past this line!\nandroid.useAndroidX=true\n", &["love/src/jni/love/src"])
        };
        fs::write(t.join("gradle.properties"), props).unwrap();
        if twelve {
            fs::create_dir_all(t.join("app/src/main/cpp")).unwrap();
        }
        for n in native {
            fs::create_dir_all(t.join(n)).unwrap();
            if submodules {
                fs::write(t.join(n).join("source.cpp"), "x").unwrap();
            }
        }
        t
    }

    fn app(name: &str, v: &str) -> AndroidApp {
        AndroidApp { name: name.into(), bundle_id: "dev.joyride.my-game".into(), version_name: "1.2.3".into(), landscape: false, love_version: LoveVersion::parse(v).unwrap() }
    }

    #[test]
    fn stages_a_12x_project_with_the_games_identity() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path(), true, true);
        let love = d.path().join("g.love");
        fs::write(&love, b"PK-game").unwrap();
        let out = d.path().join("out");
        stage_project(&t, &love, &out, &app("Café Run", "12.0"), None).unwrap();
        assert_eq!(fs::read(out.join("app/src/embed/assets/game.love")).unwrap(), b"PK-game");
        assert!(!out.join("app/src/embed/assets/leftover.txt").exists(), "only the game is in the embed assets");
        assert!(!out.join(".git").exists() && !out.join("local.properties").exists() && !out.join("app/build").exists());
        let p = fs::read_to_string(out.join("gradle.properties")).unwrap();
        for line in ["app.name=Caf\\u00E9 Run", "app.application_id=dev.joyride.my_game", "app.orientation=portrait", "app.version_code=10203", "app.version_name=1.2.3", "android.useAndroidX=true"] {
            assert!(p.contains(line), "{line} in\n{p}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(out.join("gradlew")).unwrap().permissions().mode() & 0o111, 0o111, "gradlew stays executable");
        }
        // The template folder itself is untouched.
        assert!(t.join("app/src/embed/assets/leftover.txt").exists());
    }

    #[test]
    fn an_11x_project_gets_a_byte_list_name_for_non_ascii_and_a_plain_one_otherwise() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path(), false, true);
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        let o1 = d.path().join("o1");
        stage_project(&t, &love, &o1, &app("Café", "11.5"), None).unwrap();
        let p = fs::read_to_string(o1.join("gradle.properties")).unwrap();
        assert!(p.contains("app.name_byte_array=67,97,102,195,169"), "{p}");
        assert!(!p.lines().any(|l| l.starts_with("app.name=")), "only one of the two names is defined:\n{p}");
        let o2 = d.path().join("o2");
        stage_project(&t, &love, &o2, &app("Plain Game", "11.5"), None).unwrap();
        let p2 = fs::read_to_string(o2.join("gradle.properties")).unwrap();
        assert!(p2.contains("app.name=Plain Game") && !p2.contains("app.name_byte_array"), "{p2}");
    }

    #[test]
    fn missing_submodules_and_bad_templates_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        let t = fake_template(d.path(), true, false);
        let e = stage_project(&t, &love, &d.path().join("o"), &app("G", "12.0"), None).unwrap_err().to_string();
        assert!(e.contains("git submodule update --init --recursive"), "{e}");
        assert!(stage_project(d.path(), &love, &d.path().join("o2"), &app("G", "12.0"), None).unwrap_err().to_string().contains("not a love-android checkout"));
        let t2 = fake_template(&d.path().join("b"), false, true);
        assert!(stage_project(&t2, &d.path().join("nope.love"), &d.path().join("o3"), &app("G", "11.5"), None).is_err());
    }

    #[test]
    fn the_icon_is_resized_into_each_density_that_exists() {
        let d = tempfile::tempdir().unwrap();
        let t = fake_template(d.path(), true, true);
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        let icon = d.path().join("icon.png");
        image::RgbaImage::from_pixel(512, 512, image::Rgba([10, 200, 30, 255])).save(&icon).unwrap();
        let out = d.path().join("out");
        stage_project(&t, &love, &out, &app("G", "12.0"), Some(&icon)).unwrap();
        assert_eq!(image::image_dimensions(out.join("app/src/main/res/drawable-mdpi/love.png")).unwrap(), (48, 48));
        assert_eq!(image::image_dimensions(out.join("app/src/main/res/drawable-xxxhdpi/love.png")).unwrap(), (192, 192));
        assert!(!out.join("app/src/main/res/drawable-hdpi").exists(), "a density the template lacks is not invented");
    }

    /// With the real `love-android` checkout (`JOYRIDE_LOVE_ANDROID`, branch
    /// 12.x or 11.x), whose submodules are not fetched: staging must say so.
    #[test]
    fn the_real_template_layout_is_recognised() {
        let Ok(t) = std::env::var("JOYRIDE_LOVE_ANDROID") else { return };
        let d = tempfile::tempdir().unwrap();
        let love = d.path().join("g.love");
        fs::write(&love, b"x").unwrap();
        let e = stage_project(Path::new(&t), &love, &d.path().join("o"), &app("G", "12.0"), None).unwrap_err().to_string();
        assert!(e.contains("submodule"), "a plain clone has empty submodules, so the layout check must reach them: {e}");
        // With the submodule folders filled (as `git submodule update` would),
        // the real gradle.properties takes the game's identity.
        let copy = d.path().join("t");
        assert!(std::process::Command::new("cp").args(["-R", &t]).arg(&copy).status().unwrap().success());
        for n in ["app/src/main/cpp/love/src", "app/src/main/cpp/megasource", "love/src/jni/love/src"] {
            if copy.join(n).parent().is_some_and(|p| p.exists()) || n.starts_with("app/src/main/cpp") == copy.join("app/src/main/cpp").exists() {
                fs::create_dir_all(copy.join(n)).unwrap();
                fs::write(copy.join(n).join("x.cpp"), "x").unwrap();
            }
        }
        let twelve = copy.join("app/src/main/cpp").is_dir();
        let a = AndroidApp { name: "Café Run".into(), bundle_id: "dev.joyride.my-game".into(), version_name: "1.2.3".into(), landscape: true, love_version: LoveVersion::parse(if twelve { "12.0" } else { "11.5" }).unwrap() };
        let out = d.path().join("out");
        stage_project(&copy, &love, &out, &a, None).unwrap();
        let p = fs::read_to_string(out.join("gradle.properties")).unwrap();
        assert!(p.contains("app.application_id=dev.joyride.my_game") && p.contains("app.version_name=1.2.3") && p.contains("app.version_code=10203") && p.contains("app.orientation=landscape"), "{p}");
        assert_eq!(p.lines().filter(|l| l.starts_with("app.name=") || l.starts_with("app.name_byte_array=")).count(), 1, "exactly one name key:\n{p}");
        assert!(p.contains("android.useAndroidX=true"), "the template's own settings are kept");
        assert!(out.join("app/src/embed/assets/game.love").is_file());
    }
}
