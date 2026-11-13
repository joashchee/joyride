//! What sort of asset a file is, from its extension.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Image,
    Audio,
    Font,
    Shader,
    Data,
    Video,
    /// Lua source: a library or module, not a media asset.
    Script,
    /// A `.love` or `.zip` that LÖVE mounts as a folder.
    Archive,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Audio => "audio",
            Kind::Font => "font",
            Kind::Shader => "shader",
            Kind::Data => "data",
            Kind::Video => "video",
            Kind::Script => "script",
            Kind::Archive => "archive",
            Kind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Kind {
        [Kind::Image, Kind::Audio, Kind::Font, Kind::Shader, Kind::Data, Kind::Video, Kind::Script, Kind::Archive]
            .into_iter()
            .find(|k| k.as_str() == s)
            .unwrap_or(Kind::Other)
    }

    /// Media a game ships. Scripts and unknown files are not scanned by default.
    pub fn is_media(self) -> bool {
        !matches!(self, Kind::Script | Kind::Other)
    }
}

/// Extensions in LÖVE's tracker-music table (libmodplug), read from the real
/// 11.5 binary, and tracked as audio. LÖVE spells 669 as `699`; both are
/// listed. LÖVE picks an audio decoder by the file's content, not its name,
/// and on real sample files only MOD, XM, IT, S3M and MIDI loaded (see
/// [`love_support`]).
pub const MODPLUG: &[&str] = &["669", "699", "abc", "amf", "ams", "dbm", "dmf", "dsm", "far", "it", "j2b", "mdl", "med", "mid", "mod", "mt2", "mtm", "okt", "pat", "psm", "ptm", "s3m", "stm", "ult", "umx", "xm"];

pub fn classify(path: &Path) -> Kind {
    // A file known by its whole name.
    if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("gamecontrollerdb.txt")) {
        return Kind::Data;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        // Pictures LÖVE decodes, and the compressed-texture containers
        // `love.image.newCompressedData` reads (the GPU decides which formats inside run).
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tga" | "webp" | "hdr" | "exr" | "qoi" | "dds" | "ktx" | "pkm" | "pvr" | "astc" => Kind::Image,
        "wav" | "ogg" | "oga" | "mp3" | "flac" | "midi" => Kind::Audio,
        e if MODPLUG.contains(&e) => Kind::Audio,
        "ttf" | "otf" | "ttc" | "fnt" | "woff" | "woff2" | "bdf" | "pcf" | "pfb" | "pfa" => Kind::Font,
        "glsl" | "frag" | "vert" | "shader" | "fsh" | "vsh" => Kind::Shader,
        "json" | "toml" | "csv" | "tmx" | "tsx" | "tsv" | "xml" | "ldtk" => Kind::Data,
        "ogv" | "webm" | "mp4" => Kind::Video,
        "lua" | "luac" => Kind::Script,
        "love" | "zip" => Kind::Archive,
        _ => Kind::Other,
    }
}

/// Whether LÖVE loads a file, from what was measured on the real LÖVE 11.5
/// and 12.0 (`scripts/love-asset-formats.py`) or found in their binaries; see
/// `docs/love-asset-types.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoveSupport {
    /// Loads on 11.5 and 12.0. `measured`: a sample file was loaded; otherwise
    /// the format's handler is in the binaries or LÖVE documents it.
    Loads { measured: bool },
    /// Loads on 12.0 only.
    Only12 { measured: bool },
    /// LÖVE does not load it (the reason says what to do).
    Refused(&'static str),
    /// Not confirmed: no sample was tried, or the one tried was refused.
    Unchecked(&'static str),
    /// LÖVE has no loader of its own: a library or the game reads it.
    NotNative(&'static str),
}

impl LoveSupport {
    /// One line for the person.
    pub fn describe(self) -> String {
        match self {
            LoveSupport::Loads { measured: true } => "yes, on LÖVE 11.5 and 12.0 (checked)".into(),
            LoveSupport::Loads { measured: false } => "yes, on LÖVE 11.5 and 12.0 (not tried with a file here)".into(),
            LoveSupport::Only12 { measured: true } => "LÖVE 12.0 only (checked); 11.5 refuses it".into(),
            LoveSupport::Only12 { measured: false } => "LÖVE 12.0 only (not tried with a file here)".into(),
            LoveSupport::Refused(why) => format!("no: {why}"),
            LoveSupport::Unchecked(why) => format!("not checked: {why}"),
            LoveSupport::NotNative(why) => format!("not by LÖVE itself: {why}"),
        }
    }

    /// A project on LÖVE 11.5 cannot use it as it is.
    pub fn blocks(self, version_major_minor: &str) -> bool {
        match self {
            LoveSupport::Refused(_) => true,
            LoveSupport::Only12 { .. } => !version_major_minor.starts_with("12"),
            _ => false,
        }
    }
}

pub fn love_support(path: &Path) -> LoveSupport {
    use LoveSupport::*;
    if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("gamecontrollerdb.txt")) {
        return Loads { measured: false };
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" => Loads { measured: false },
        "tga" | "hdr" => Loads { measured: true },
        "qoi" | "webp" => Only12 { measured: true },
        "exr" | "dds" | "ktx" | "pkm" | "pvr" | "astc" => Loads { measured: false },
        "gif" => Refused("LÖVE refuses GIF; convert it to PNG, or to a sprite sheet"),
        "psd" | "ppm" | "pgm" | "pbm" | "pnm" | "pcx" | "ico" | "tif" | "tiff" => Refused("LÖVE does not decode this picture format; convert it to PNG"),
        "wav" | "ogg" | "oga" | "mp3" | "flac" => Loads { measured: ext != "oga" },
        // Measured: MOD, XM, IT, S3M and MIDI load. LÖVE chooses by content, so `.midi` loads too.
        "mod" | "xm" | "it" | "s3m" | "mid" | "midi" => Loads { measured: true },
        "669" | "amf" | "dbm" | "far" | "mtm" | "med" | "okt" | "ptm" | "stm" | "ult" => Unchecked("LÖVE's decoder lists it, but the sample tried was refused by 11.5 and 12.0"),
        e if MODPLUG.contains(&e) => Unchecked("LÖVE's decoder lists it; no sample was tried"),
        "opus" | "aiff" | "aif" | "m4a" | "wma" | "wv" | "sid" | "nsf" => Refused("LÖVE decodes WAV, FLAC, MP3, Ogg Vorbis and tracker modules; convert it"),
        "ogv" => Loads { measured: false },
        "mp4" | "webm" => Refused("LÖVE plays Ogg Theora (.ogv) video only"),
        "ttf" | "otf" | "ttc" | "woff" => Loads { measured: true },
        "woff2" => Refused("LÖVE cannot load WOFF2; use TrueType, OpenType or WOFF"),
        "bdf" => Refused("LÖVE cannot load BDF; use a BMFont (.fnt) or an image font"),
        "fnt" => Loads { measured: false },
        "pcf" | "pfb" | "pfa" => Unchecked("FreeType may read it, but it was not tried"),
        "luac" => Unchecked("only LuaJIT bytecode loads (luajit -b output did; output of Lua 5.x luac was refused)"),
        "lua" | "love" | "zip" => Loads { measured: false },
        "glsl" | "frag" | "vert" | "shader" | "fsh" | "vsh" => Loads { measured: false },
        "json" | "toml" | "csv" | "tmx" | "tsx" | "tsv" | "xml" | "ldtk" => NotNative("LÖVE has no parser for it; a library or your own code reads it"),
        _ => NotNative("LÖVE reads it only as bytes (love.filesystem.read)"),
    }
}

/// A project file the project's LÖVE version cannot use as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unusable {
    /// Project-relative, with `/`.
    pub path: String,
    pub message: String,
}

/// Every file in the project that LÖVE `version` ("11.5", "12.0") refuses or
/// cannot yet load (`LoveSupport::blocks`). Skips hidden folders, `build`,
/// `libs` and `node_modules`; stops at 500 files. A warning, never an error:
/// the game may not load the file at all.
pub fn unusable_files(root: &Path, version: &str) -> Vec<Unusable> {
    let mut out = Vec::new();
    let walk = walkdir::WalkDir::new(root).follow_links(false).sort_by_file_name().into_iter().filter_entry(|e| {
        e.depth() == 0 || !(e.file_type().is_dir() && matches!(e.file_name().to_str(), Some(n) if n.starts_with('.') || matches!(n, "build" | "libs" | "node_modules")))
    });
    for e in walk.flatten() {
        if !e.file_type().is_file() {
            continue;
        }
        let sup = love_support(e.path());
        if !sup.blocks(version) {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(root) else { continue };
        let message = match sup {
            LoveSupport::Only12 { .. } => format!("LÖVE {version} cannot load this; it needs LÖVE 12.0"),
            LoveSupport::Refused(why) => format!("LÖVE {version} cannot load this: {why}"),
            _ => continue,
        };
        out.push(Unusable { path: rel.to_string_lossy().replace('\\', "/"), message });
        if out.len() >= 500 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension_ignoring_case() {
        assert_eq!(classify(Path::new("a/Hero.PNG")), Kind::Image);
        assert_eq!(classify(Path::new("boom.ogg")), Kind::Audio);
        assert_eq!(classify(Path::new("main.lua")), Kind::Script);
        assert_eq!(classify(Path::new("README")), Kind::Other);
        assert!(!Kind::Script.is_media());
    }

    #[test]
    fn names_round_trip() {
        for k in [Kind::Image, Kind::Audio, Kind::Font, Kind::Shader, Kind::Data, Kind::Video, Kind::Script, Kind::Archive] {
            assert_eq!(Kind::parse(k.as_str()), k);
        }
        assert_eq!(Kind::parse("nonsense"), Kind::Other);
    }

    #[test]
    fn tracks_what_love_loads_natively() {
        for (f, k) in [
            ("a.hdr", Kind::Image), ("a.exr", Kind::Image), ("a.qoi", Kind::Image), ("a.pkm", Kind::Image), ("a.pvr", Kind::Image), ("a.astc", Kind::Image),
            ("a.med", Kind::Audio), ("a.umx", Kind::Audio), ("a.699", Kind::Audio), ("a.669", Kind::Audio), ("a.okt", Kind::Audio),
            ("a.ttc", Kind::Font), ("a.pfb", Kind::Font), ("a.pcf", Kind::Font),
            ("a.fsh", Kind::Shader), ("a.luac", Kind::Script), ("a.love", Kind::Archive), ("a.zip", Kind::Archive),
            ("assets/gamecontrollerdb.txt", Kind::Data), ("notes.txt", Kind::Other),
        ] {
            assert_eq!(classify(Path::new(f)), k, "{f}");
        }
        assert!(Kind::Archive.is_media());
    }

    #[test]
    fn says_what_love_does_with_each() {
        use LoveSupport::*;
        // Measured on the real LÖVE 11.5 and 12.0, with the r3i6nos corpus's files.
        for e in ["hdr", "tga", "mod", "xm", "it", "s3m", "mid", "midi", "ttf", "ttc", "woff"] {
            assert_eq!(love_support(Path::new(&format!("a.{e}"))), Loads { measured: true }, "{e}");
        }
        for e in ["qoi", "webp"] {
            assert_eq!(love_support(Path::new(&format!("a.{e}"))), Only12 { measured: true }, "{e}");
        }
        for f in ["a.gif", "clip.mp4", "clip.webm", "a.woff2", "a.bdf", "a.opus", "a.aiff", "a.pcx", "a.ico", "a.tiff", "a.psd"] {
            assert!(matches!(love_support(Path::new(f)), Refused(_)), "{f}");
        }
        for f in ["a.669", "a.med", "a.okt", "a.mtm", "a.ult", "a.abc", "a.pcf", "a.luac"] {
            assert!(matches!(love_support(Path::new(f)), Unchecked(_)), "{f}");
        }
        assert!(matches!(love_support(Path::new("level.tmx")), NotNative(_)));
        assert!(love_support(Path::new("a.qoi")).blocks("11.5") && !love_support(Path::new("a.qoi")).blocks("12.0"));
        assert!(love_support(Path::new("a.gif")).blocks("12.0") && !love_support(Path::new("a.png")).blocks("11.5"));
        assert_eq!(love_support(Path::new("data/gamecontrollerdb.txt")), Loads { measured: false });
        // Every tracked tracker extension has an honest answer, never a bare "loads".
        for e in MODPLUG {
            let s = love_support(Path::new(&format!("x.{e}")));
            assert!(matches!(s, Loads { measured: true } | Unchecked(_)), "{e}: {s:?}");
        }
    }

    #[test]
    fn finds_files_the_version_cannot_use() {
        let d = tempfile::tempdir().unwrap();
        for f in ["main.lua", "hero.png", "a.qoi", "gfx/anim.gif", "libs/x/doc.gif", "build/old.gif", ".git/y.gif", "music.mp4"] {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        let on_11: Vec<_> = unusable_files(d.path(), "11.5").into_iter().map(|u| u.path).collect();
        assert_eq!(on_11, ["a.qoi", "gfx/anim.gif", "music.mp4"]);
        let on_12: Vec<_> = unusable_files(d.path(), "12.0").into_iter().map(|u| u.path).collect();
        assert_eq!(on_12, ["gfx/anim.gif", "music.mp4"]);
        assert!(unusable_files(d.path(), "11.5")[0].message.contains("12.0"));
    }
}
