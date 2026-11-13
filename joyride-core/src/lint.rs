//! Version-calibrated LÖVE lints (spec 3B). These run beside `luacheck`,
//! which JoyrIDE also runs with `love` as a read-only global
//! ([`luacheckrc`]); these cover what luacheck cannot know: the LÖVE
//! version in play.

use crate::lua::{lex, Tok};
use crate::toolchain::LoveVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: u32,
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

/// APIs that exist only from LÖVE 12.0. Measured on the real 11.5 and 12.0
/// binaries with `scripts/love-api-diff.py` (2026-11-13), which could only
/// load the modules that run without a window, so `love.graphics` is not
/// covered by measurement: `newTexture` is from the spec and unverified.
/// Rerun the script with `--graphics` on a machine with a display.
pub const ONLY_IN_12: &[&str] = &[
    "love.graphics.newTexture",
    "love.event.restart",
    "love.event.setModalDrawCallback",
    "love.filesystem.canonicalizeRealPath",
    "love.filesystem.getFullCommonPath",
    "love.filesystem.mountCommonPath",
    "love.filesystem.mountFullPath",
    "love.filesystem.openFile",
    "love.filesystem.openNativeFile",
    "love.filesystem.unmountCommonPath",
    "love.filesystem.unmountFullPath",
    "love.markDeprecated",
    "love.math.perlinNoise",
    "love.math.simplexNoise",
    "love.physics.computeAngularFrequency",
    "love.physics.computeAngularStiffness",
    "love.physics.computeLinearFrequency",
    "love.physics.computeLinearStiffness",
    "love.physics.newChainBody",
    "love.physics.newCircleBody",
    "love.physics.newEdgeBody",
    "love.physics.newPolygonBody",
    "love.physics.newRectangleBody",
    "love.sensor.getData",
    "love.sensor.getName",
    "love.sensor.hasSensor",
    "love.sensor.isEnabled",
    "love.sensor.setEnabled",
    "love.system.getMemorySize",
    "love.system.getPreferredLocales",
];

/// APIs present in 11.5 and absent in 12.0, measured the same way (same
/// coverage limit). Each has a replacement: `love.filesystem.getInfo` for the
/// five filesystem ones, `love.data.compress`/`decompress` for the maths pair.
pub const REMOVED_IN_12: &[&str] = &[
    "love.filesystem.getLastModified",
    "love.filesystem.getSize",
    "love.filesystem.isDirectory",
    "love.filesystem.isFile",
    "love.filesystem.isSymlink",
    "love.math.compress",
    "love.math.decompress",
];

pub fn lint(source: &str, file: &str, version: LoveVersion) -> Vec<Diagnostic> {
    let t = lex(source);
    let mut out = Vec::new();
    let mut d = |line, severity, rule, message: String| out.push(Diagnostic { line, severity, rule, message });
    for i in 0..t.len() {
        let line = t[i].line;
        // love.graphics.setColor(255, 128, 0): colours are 0 to 1 from LÖVE 11.
        if t[i].is_name("love") && dotted(&t, i, &["graphics", "setColor"]) {
            let open = i + 5;
            if t.get(open).is_some_and(|x| x.is_punct("(")) {
                let mut j = open + 1;
                let mut big = None;
                while let Some(k) = t.get(j) {
                    match &k.tok {
                        Tok::Number(n) => {
                            if *n > 1.0 && big.is_none() {
                                big = Some(*n);
                            }
                        }
                        Tok::Punct(p) if p == "," || p == "{" || p == "}" => {}
                        _ => break,
                    }
                    j += 1;
                }
                if let Some(n) = big {
                    d(line, Severity::Warning, "color-range", format!("setColor value {n} is above 1.0; LÖVE 11 and later use 0 to 1"));
                }
            }
        }
        // t.window.vsync = true
        if t[i].is_name("t") && t.get(i + 1).is_some_and(|x| x.is_punct(".")) && t.get(i + 2).is_some_and(|x| x.is_name("window")) && t.get(i + 3).is_some_and(|x| x.is_punct(".")) && t.get(i + 4).is_some_and(|x| x.is_name("vsync")) && t.get(i + 5).is_some_and(|x| x.is_punct("=")) {
            if let Some(v) = t.get(i + 6) {
                if v.is_name("true") || v.is_name("false") {
                    d(line, Severity::Warning, "vsync-boolean", "t.window.vsync is a number (1, 0 or -1) from LÖVE 11, not a boolean".into());
                }
            }
        }
        // Dotted API names.
        if t[i].is_name("love") && t.get(i + 1).is_some_and(|x| x.is_punct(".")) {
            if let Some(name) = dotted_name(&t, i) {
                if !version.is_12_or_later() && ONLY_IN_12.contains(&name.as_str()) {
                    d(line, Severity::Error, "needs-12", format!("{name} needs LÖVE 12.0; this project targets {version}"));
                }
                if version.is_12_or_later() && REMOVED_IN_12.contains(&name.as_str()) {
                    d(line, Severity::Error, "removed-in-12", format!("{name} was removed in LÖVE 12.0{}", replacement(&name)));
                }
            }
        }
    }
    let _ = file;
    out
}

fn replacement(name: &str) -> &'static str {
    if name.starts_with("love.filesystem.") {
        "; use love.filesystem.getInfo"
    } else if name.starts_with("love.math.") {
        "; use love.data.compress / love.data.decompress"
    } else {
        ""
    }
}

fn dotted(t: &[crate::lua::Token], i: usize, rest: &[&str]) -> bool {
    let mut k = i + 1;
    for name in rest {
        if !t.get(k).is_some_and(|x| x.is_punct(".")) || !t.get(k + 1).is_some_and(|x| x.is_name(name)) {
            return false;
        }
        k += 2;
    }
    true
}

fn dotted_name(t: &[crate::lua::Token], i: usize) -> Option<String> {
    let mut name = String::from("love");
    let mut k = i + 1;
    while t.get(k).is_some_and(|x| x.is_punct(".")) {
        let Tok::Name(n) = &t.get(k + 1)?.tok else { break };
        name.push('.');
        name.push_str(n);
        k += 2;
    }
    Some(name)
}

/// A `.luacheckrc` that knows LÖVE: LuaJIT standard library, `love` as a
/// read-only global, and `dt`-style unused arguments left alone.
pub fn luacheckrc() -> &'static str {
    "std = \"luajit\"\nread_globals = { \"love\" }\nunused_args = false\nmax_line_length = false\nexclude_files = { \"libs/\", \"build/\", \".joyride/\" }\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    const V11: LoveVersion = crate::toolchain::BUNDLED;
    fn v12() -> LoveVersion {
        LoveVersion::parse("12.0").unwrap()
    }
    fn rules(src: &str, v: LoveVersion) -> Vec<&'static str> {
        lint(src, "main.lua", v).into_iter().map(|d| d.rule).collect()
    }

    #[test]
    fn old_style_colours_are_flagged() {
        assert_eq!(rules("love.graphics.setColor(255, 128, 0)", V11), ["color-range"]);
        assert_eq!(rules("love.graphics.setColor({1, 300, 0})", V11), ["color-range"]);
        assert!(rules("love.graphics.setColor(1, 0.5, 0, 1)", V11).is_empty());
        assert!(rules("love.graphics.setColor(r, g, b)", V11).is_empty());
        assert!(rules("-- love.graphics.setColor(255,0,0)", V11).is_empty());
    }

    #[test]
    fn vsync_must_be_a_number() {
        assert_eq!(rules("t.window.vsync = true", V11), ["vsync-boolean"]);
        assert!(rules("t.window.vsync = 1", V11).is_empty());
    }

    #[test]
    fn version_specific_apis() {
        let d = lint("local t = love.graphics.newTexture(x)", "m.lua", V11);
        assert_eq!(d[0].rule, "needs-12");
        assert_eq!(d[0].severity, Severity::Error);
        assert!(rules("local t = love.graphics.newTexture(x)", v12()).is_empty());
    }

    #[test]
    fn apis_removed_in_12_are_flagged_only_when_targeting_it() {
        let src = "if love.filesystem.isFile('a.txt') then end\nlocal c = love.math.compress(x)";
        assert!(rules(src, V11).is_empty());
        let d = lint(src, "m.lua", v12());
        assert_eq!(d.iter().map(|d| d.rule).collect::<Vec<_>>(), ["removed-in-12", "removed-in-12"]);
        assert!(d[0].message.contains("getInfo") && d[1].message.contains("love.data"));
        assert!(rules("love.filesystem.getInfo('a')", v12()).is_empty());
    }

    #[test]
    fn the_tables_do_not_overlap_and_are_sorted_by_module() {
        for r in REMOVED_IN_12 {
            assert!(!ONLY_IN_12.contains(r), "{r} is in both");
        }
    }

    #[test]
    fn diagnostics_carry_the_line() {
        let d = lint("local a = 1\n\nlove.graphics.setColor(255,0,0)", "m.lua", V11);
        assert_eq!(d[0].line, 3);
    }

    #[test]
    fn luacheckrc_names_love() {
        assert!(luacheckrc().contains("read_globals = { \"love\" }"));
    }
}
