//! Adding a font to the open project: look at the file first (what the font
//! says about itself, what licence it carries, whether LÖVE loads it), then
//! copy it in once a named person has confirmed the licence. The work is
//! `asset_core::font` and `asset_core::add`; nothing here writes outside the
//! project and the app's own asset library folder.

use crate::preview::Fact;
use crate::project::Project;
use asset_core::add::{add_to_project, AddRequest};
use asset_core::catalog::Catalog;
use asset_core::font::{self, Format, Love};
use asset_core::licence::{self, Licence};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

const MAX_FONT: u64 = 64 * 1024 * 1024;
const MAX_SAMPLE: u64 = 16 * 1024 * 1024;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicenceGuess {
    pub spdx: String,
    /// Where the guess came from, in words for the person.
    pub from: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FontPlan {
    pub source: String,
    pub file_name: String,
    pub facts: Vec<Fact>,
    /// For the sample; absent for a font too large to send.
    pub data_url: Option<String>,
    pub guess: Option<LicenceGuess>,
    /// What the font says about its own licence (name table), for the person to read.
    pub licence_text: String,
    pub licence_url: String,
    pub designer: String,
    pub suggested_dest: String,
    pub warnings: Vec<String>,
    /// The same bytes are already in the project (their path), so adding is pointless.
    pub already_in_project: Option<String>,
}

fn mime(f: Format) -> &'static str {
    match f {
        Format::TrueType | Format::Collection => "font/ttf",
        Format::OpenTypeCff => "font/otf",
        Format::Woff => "font/woff",
        Format::Woff2 => "font/woff2",
    }
}

fn short(s: &str, n: usize) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// The facts a font gives about itself, as lines to show.
pub fn facts_of(f: &font::FontFacts) -> Vec<Fact> {
    let mut facts = vec![];
    let mut fact = |l: &str, v: String| {
        if !v.trim().is_empty() {
            facts.push(Fact { label: l.into(), value: v });
        }
    };
    fact("Family", f.family.clone());
    fact("Style", f.style.clone());
    fact("Format", if f.fonts > 1 { format!("{} ({} fonts)", f.format.name(), f.fonts) } else { f.format.name().to_string() });
    fact("Version", f.version.clone());
    fact("Glyphs", f.glyphs.map(|g| g.to_string()).unwrap_or_default());
    fact("Designer", f.designer.clone());
    fact("Copyright", short(&f.copyright, 160));
    fact("Embedding", f.embedding.map(|e| e.describe().to_string()).unwrap_or_default());
    fact("Loads in LÖVE", match f.love {
        Love::Loads => "yes (checked on LÖVE 11.5 and 12.0)".into(),
        Love::Unknown => "not checked".into(),
    });
    facts
}

/// Reads a font file and says what it is. `root` is the open project, to spot a copy already in it.
pub fn inspect(source: &Path, root: Option<&Path>) -> Result<FontPlan, String> {
    let meta = std::fs::metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if !meta.is_file() {
        return Err("that is not a file".into());
    }
    if meta.len() > MAX_FONT {
        return Err("that file is too large to be a font".into());
    }
    let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let f = font::probe(&bytes).ok_or("this is not a font file this can read (TrueType, OpenType, TrueType Collection, WOFF or WOFF2)")?;
    let file_name = source.file_name().and_then(|n| n.to_str()).unwrap_or("font.ttf").to_string();

    let facts = facts_of(&f);

    let mut warnings = vec![];
    if f.embedding == Some(font::Embedding::Restricted) {
        warnings.push("The font's own flags say it must not be embedded, and a game ships its fonts. Check the licence before using it.".to_string());
    }
    if f.love == Love::Unknown {
        warnings.push("LÖVE's loading of this format has not been checked; try it in a game, or convert the font to TrueType or OpenType.".to_string());
    }
    if f.family.is_empty() {
        warnings.push("The file does not name its family, so its details may be incomplete.".to_string());
    }

    // A guess for the person to check: the font's own note, then a licence file beside it.
    let guess = f.licence_guess().map(|spdx| LicenceGuess { spdx, from: "the font's own licence note".into() }).or_else(|| {
        let dir = source.parent()?;
        licence::guess_for(source, dir).map(|g| LicenceGuess { spdx: g.spdx, from: format!("{} next to the font", g.source.file_name().and_then(|n| n.to_str()).unwrap_or("a licence file")) })
    });

    let already_in_project = root.and_then(|r| {
        let hash = asset_core::dedup::hash_file(source).ok()?;
        find_same(r, &hash, 0)
    });
    Ok(FontPlan {
        source: source.display().to_string(),
        suggested_dest: format!("fonts/{file_name}"),
        file_name,
        facts,
        data_url: (meta.len() <= MAX_SAMPLE).then(|| format!("data:{};base64,{}", mime(f.format), base64::engine::general_purpose::STANDARD.encode(&bytes))),
        guess,
        licence_text: short(&f.licence_text, 400),
        licence_url: f.licence_url.clone(),
        designer: f.designer,
        warnings,
        already_in_project,
    })
}

/// A project file with this content (looked for in font-like files only, a few folders deep).
fn find_same(root: &Path, hash: &str, depth: u32) -> Option<String> {
    fn walk(base: &Path, dir: &Path, hash: &str, depth: u32) -> Option<String> {
        for e in std::fs::read_dir(dir).ok()?.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let ft = e.file_type().ok()?;
            if ft.is_dir() && depth < 4 && !matches!(name.as_str(), ".git" | ".joyride" | "build" | "node_modules") {
                if let Some(r) = walk(base, &p, hash, depth + 1) {
                    return Some(r);
                }
            } else if ft.is_file() && asset_core::kind::classify(&p) == asset_core::Kind::Font && asset_core::dedup::hash_file(&p).ok().as_deref() == Some(hash) {
                return p.strip_prefix(base).ok().map(|r| r.to_string_lossy().replace('\\', "/"));
            }
        }
        None
    }
    walk(root, root, hash, depth)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FontAdd {
    pub source: String,
    pub dest: String,
    /// An SPDX id from the offered list, `LicenseRef-Custom` or `unknown`.
    pub licence: String,
    pub confirmed_by: String,
    pub note: String,
    pub author: String,
    pub source_url: String,
    pub attribution: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FontAdded {
    pub dest: String,
    pub already_in_library: bool,
}

/// Copies the font into the project after its licence is confirmed. `library` is the app's asset-library folder.
pub fn add(library: &Path, root: &Path, r: &FontAdd) -> Result<FontAdded, String> {
    let source = PathBuf::from(&r.source);
    // Only fonts: the person picked this in the dialog, but a path is a path.
    let bytes_head = {
        use std::io::Read;
        let mut h = Vec::new();
        std::fs::File::open(&source).map_err(|e| e.to_string())?.take(1 << 20).read_to_end(&mut h).map_err(|e| e.to_string())?;
        h
    };
    let f = font::probe(&bytes_head).ok_or("this is not a font file")?;
    std::fs::create_dir_all(library).map_err(|e| e.to_string())?;
    let mut cat = Catalog::open(&library.join("catalog.sqlite")).map_err(|e| e.to_string())?;
    let name = root.canonicalize().map_err(|e| e.to_string())?.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
    let mut tags = vec![format!("font:{}", match f.format { Format::TrueType => "truetype", Format::OpenTypeCff => "opentype", Format::Collection => "collection", Format::Woff => "woff", Format::Woff2 => "woff2" })];
    if !f.family.is_empty() {
        let fam = asset_core::catalog::normalise_tag(&f.family);
        if !fam.is_empty() {
            tags.push(format!("family:{fam}"));
        }
    }
    let licence = if r.licence == "unknown" { Licence::unknown("unknown") } else { Licence::from_spdx(&r.licence) };
    let req = AddRequest { source: &source, dest: r.dest.trim(), licence, confirmed_by: &r.confirmed_by, note: &r.note, author: &r.author, source_url: &r.source_url, attribution_text: &r.attribution, tags };
    let a = add_to_project(&mut cat, &library.join("files"), &name, root, &req).map_err(|e| e.to_string())?;
    Ok(FontAdded { dest: a.dest, already_in_library: a.already_in_library })
}

fn library_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("asset-library")).map_err(|e| e.to_string())
}

/// What a chosen font file is, for the Add Font panel.
#[tauri::command]
pub async fn font_inspect(project: State<'_, Project>, source: String) -> Result<FontPlan, String> {
    let root = project.root().ok();
    tauri::async_runtime::spawn_blocking(move || inspect(Path::new(&source), root.as_deref())).await.map_err(|e| e.to_string())?
}

/// Adds the font to the open project (licence confirmed, nothing replaced).
#[tauri::command]
pub async fn font_add(app: AppHandle, project: State<'_, Project>, request: FontAdd) -> Result<FontAdded, String> {
    let root = project.root()?;
    let lib = library_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || add(&lib, &root, &request)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const VGA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../public/fonts/ansiapps/WebPlus_IBM_VGA_8x16.woff");

    fn req(source: &str, dest: &str, licence: &str, by: &str, note: &str) -> FontAdd {
        FontAdd { source: source.into(), dest: dest.into(), licence: licence.into(), confirmed_by: by.into(), note: note.into(), author: "VileR".into(), source_url: "https://int10h.org/oldschool-pc-fonts/".into(), attribution: String::new() }
    }

    #[test]
    fn inspects_a_font_and_finds_the_licence_beside_it() {
        let p = inspect(Path::new(VGA), None).unwrap();
        assert!(p.facts.iter().any(|f| f.label == "Format" && f.value == "WOFF"));
        assert!(p.facts.iter().any(|f| f.label == "Loads in LÖVE" && f.value.starts_with("yes")));
        let g = p.guess.expect("the LICENSE file next to it names the licence");
        assert!(g.spdx.starts_with("CC-BY-SA"), "{}", g.spdx);
        assert!(g.from.contains("next to the font"));
        assert!(p.data_url.unwrap().starts_with("data:font/woff;base64,"));
        assert_eq!(p.suggested_dest, "fonts/WebPlus_IBM_VGA_8x16.woff");
    }

    #[test]
    fn refuses_what_is_not_a_font() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.ttf"), "this is text").unwrap();
        assert!(inspect(&d.path().join("a.ttf"), None).unwrap_err().contains("not a font"));
        assert!(add(&d.path().join("lib"), d.path(), &req(d.path().join("a.ttf").to_str().unwrap(), "fonts/a.ttf", "MIT", "J", "")).is_err());
        assert!(inspect(d.path(), None).is_err());
    }

    #[test]
    fn adds_it_and_then_knows_it_is_there() {
        let d = tempfile::tempdir().unwrap();
        let (root, lib) = (d.path().join("game"), d.path().join("lib"));
        std::fs::create_dir_all(&root).unwrap();
        // The licence must be confirmed by name first.
        assert!(add(&lib, &root, &req(VGA, "fonts/vga.woff", "CC-BY-SA-4.0", "", "")).is_err());
        assert!(!root.join("fonts").exists());
        let a = add(&lib, &root, &req(VGA, "fonts/vga.woff", "CC-BY-SA-4.0", "Joash", "")).unwrap();
        assert_eq!(a.dest, "fonts/vga.woff");
        assert!(root.join("fonts/vga.woff").is_file());
        let toml = std::fs::read_to_string(root.join(".joyride/assets.toml")).unwrap();
        assert!(toml.contains("fonts/vga.woff") && toml.contains("CC-BY-SA-4.0"));
        assert_eq!(inspect(Path::new(VGA), Some(&root)).unwrap().already_in_project.as_deref(), Some("fonts/vga.woff"));
        // A second copy under another name is refused when the licence differs.
        assert!(add(&lib, &root, &req(VGA, "fonts/other.woff", "MIT", "Joash", "")).is_err());
        // And an existing file is never replaced.
        assert!(add(&lib, &root, &req(VGA, "fonts/vga.woff", "CC-BY-SA-4.0", "Joash", "")).is_err());
    }

    #[test]
    fn an_unknown_licence_needs_a_reason() {
        let d = tempfile::tempdir().unwrap();
        let (root, lib) = (d.path().join("game"), d.path().join("lib"));
        std::fs::create_dir_all(&root).unwrap();
        assert!(add(&lib, &root, &req(VGA, "fonts/v.woff", "unknown", "Joash", "")).is_err());
        assert!(add(&lib, &root, &req(VGA, "fonts/v.woff", "unknown", "Joash", "Supplied by the client; licence requested")).is_ok());
    }
}
