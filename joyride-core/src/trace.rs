//! Linking a game's error output to source (spec 3C): recognises
//! `Error: file.lua:12: message` and `[string "chunk"]:12:` and the
//! `file.lua:12:` frames of a traceback.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub file: String,
    pub line: u32,
    pub message: String,
    /// Byte range of the `file:line` text in the input, to make clickable.
    pub span: (usize, usize),
}

fn patterns() -> &'static [Regex; 3] {
    static P: OnceLock<[Regex; 3]> = OnceLock::new();
    P.get_or_init(|| {
        [
            Regex::new(r"Error: ([^:\n]+):(\d+): (.*)").unwrap(),
            Regex::new(r#"\[string "([^"]+)"\]:(\d+):\s*(.*)"#).unwrap(),
            // A traceback frame: `\tsrc/enemy.lua:44: in function 'hit'`
            Regex::new(r"^\s+([^\s:\[\]][^:\n]*\.lua):(\d+):\s*(.*)").unwrap(),
        ]
    })
}

/// The link on one line of output, if it has one.
pub fn link(line: &str) -> Option<Link> {
    for re in patterns() {
        if let Some(c) = re.captures(line) {
            let file = c.get(1)?;
            let ln = c.get(2)?;
            return Some(Link {
                file: file.as_str().to_string(),
                line: ln.as_str().parse().ok()?,
                message: c.get(3).map(|m| m.as_str().to_string()).unwrap_or_default(),
                span: (file.start(), ln.end()),
            });
        }
    }
    None
}

/// Every link in a block of output, in order.
pub fn links(text: &str) -> Vec<Link> {
    text.lines().filter_map(link).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn love_error_line() {
        let l = link("Error: src/player.lua:42: attempt to index a nil value (local 'p')").unwrap();
        assert_eq!((l.file.as_str(), l.line), ("src/player.lua", 42));
        assert!(l.message.starts_with("attempt to index"));
        assert_eq!(&"Error: src/player.lua:42: x"[l.span.0..l.span.1], "src/player.lua:42");
    }

    #[test]
    fn string_chunks_and_traceback_frames() {
        assert_eq!(link(r#"[string "return x +"]:1: unexpected symbol"#).unwrap().file, "return x +");
        let tb = "Error: main.lua:3: boom\nstack traceback:\n\t[C]: in function 'error'\n\tmain.lua:3: in function 'f'\n\tsrc/a.lua:10: in main chunk";
        let ls = links(tb);
        assert_eq!(ls.iter().map(|l| (l.file.as_str(), l.line)).collect::<Vec<_>>(), [("main.lua", 3), ("main.lua", 3), ("src/a.lua", 10)]);
    }

    #[test]
    fn plain_output_has_no_link() {
        assert!(link("loaded 12 sprites").is_none());
        assert!(link("Error: no colon number").is_none());
    }
}
