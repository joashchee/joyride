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
            Kind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Kind {
        [Kind::Image, Kind::Audio, Kind::Font, Kind::Shader, Kind::Data, Kind::Video, Kind::Script]
            .into_iter()
            .find(|k| k.as_str() == s)
            .unwrap_or(Kind::Other)
    }

    /// Media a game ships. Scripts and unknown files are not scanned by default.
    pub fn is_media(self) -> bool {
        !matches!(self, Kind::Script | Kind::Other)
    }
}

pub fn classify(path: &Path) -> Kind {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tga" | "webp" | "dds" | "ktx" => Kind::Image,
        "wav" | "ogg" | "oga" | "mp3" | "flac" | "mod" | "xm" | "it" | "s3m" | "mid" | "midi" => Kind::Audio,
        "ttf" | "otf" | "fnt" | "woff" | "woff2" | "bdf" => Kind::Font,
        "glsl" | "frag" | "vert" | "shader" => Kind::Shader,
        "json" | "toml" | "csv" | "tmx" | "tsx" | "tsv" | "xml" | "ldtk" => Kind::Data,
        "ogv" | "webm" | "mp4" => Kind::Video,
        "lua" => Kind::Script,
        _ => Kind::Other,
    }
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
        for k in [Kind::Image, Kind::Audio, Kind::Font, Kind::Shader, Kind::Data, Kind::Video, Kind::Script] {
            assert_eq!(Kind::parse(k.as_str()), k);
        }
        assert_eq!(Kind::parse("nonsense"), Kind::Other);
    }
}
