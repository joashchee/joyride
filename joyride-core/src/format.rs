//! Formatting Lua with the bundled StyLua (spec 3B): text in, text out over
//! stdin, so nothing is written until the editor applies the result.

use crate::{err, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `stylua/<arch>/stylua` in the toolchains folder, this machine's
/// architecture first. `JOYRIDE_STYLUA` overrides, for development.
pub fn find_stylua(toolchains: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JOYRIDE_STYLUA").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let base = toolchains?.join("stylua");
    let order = if cfg!(target_arch = "aarch64") { ["arm64", "x64"] } else { ["x64", "arm64"] };
    let exe = if cfg!(windows) { "stylua.exe" } else { "stylua" };
    order.iter().map(|a| base.join(a).join(exe)).find(|p| p.is_file())
}

/// Formats `text`. A project's own `stylua.toml` or `.stylua.toml` decides the
/// style; without one, two-space indents (as the editor uses). A syntax error
/// is returned as the message StyLua gives, and nothing changes.
pub fn format_lua(stylua: &Path, project: &Path, text: &str) -> Result<String> {
    run(stylua, project, text, None)
}

/// Formats only the bytes `start..end` of `text` (UTF-8 byte offsets, as
/// StyLua counts them); the rest of the file comes back untouched. StyLua
/// formats whole statements, so the changed part can be wider than the range.
pub fn format_lua_range(stylua: &Path, project: &Path, text: &str, start: usize, end: usize) -> Result<String> {
    if start > end || end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return err(format!("range {start}..{end} is not inside the text ({} bytes)", text.len()));
    }
    run(stylua, project, text, Some((start, end)))
}

fn run(stylua: &Path, project: &Path, text: &str, range: Option<(usize, usize)>) -> Result<String> {
    let mut cmd = Command::new(stylua);
    cmd.current_dir(project).args(["--search-parent-directories"]);
    if let Some((a, b)) = range {
        cmd.args(["--range-start", &a.to_string(), "--range-end", &b.to_string()]);
    }
    cmd.arg("-");
    if !["stylua.toml", ".stylua.toml"].iter().any(|f| project.join(f).is_file()) {
        cmd.args(["--indent-type", "Spaces", "--indent-width", "2"]);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| crate::Error::Msg(format!("could not start StyLua: {e}")))?;
    let mut stdin = child.stdin.take().unwrap();
    let input = text.to_string();
    // Written on a thread: a large file would fill the pipe while we wait for output.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let out = child.wait_with_output()?;
    let _ = writer.join();
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        return err(msg.trim().trim_start_matches("error: ").to_string());
    }
    String::from_utf8(out.stdout).or_else(|_| err("StyLua returned text that is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin() -> Option<PathBuf> {
        find_stylua(Some(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/toolchains")))
    }

    #[test]
    fn formats_with_two_spaces_by_default_and_reports_syntax_errors() {
        let Some(b) = bin() else { return };
        let d = tempfile::tempdir().unwrap();
        let out = format_lua(&b, d.path(), "local   x=1\nif x then\nprint( x )\nend\n").unwrap();
        assert_eq!(out, "local x = 1\nif x then\n  print(x)\nend\n");
        let e = format_lua(&b, d.path(), "local = = 1").unwrap_err().to_string();
        assert!(!e.is_empty());
        assert_eq!(format_lua(&b, d.path(), "").unwrap(), "");
    }

    #[test]
    fn formats_only_the_selected_statements() {
        let Some(b) = bin() else { return };
        let d = tempfile::tempdir().unwrap();
        // Non-ASCII before the range: byte offsets are not character offsets.
        let text = "local   s=\"é\"\nlocal   a=1\nlocal   b=2\n";
        let start = text.find("local   a").unwrap();
        let end = text.find("local   b").unwrap() - 1;
        let out = format_lua_range(&b, d.path(), text, start, end).unwrap();
        assert_eq!(out, "local   s=\"é\"\nlocal a = 1\nlocal   b=2\n", "only the middle line changes");
        assert!(format_lua_range(&b, d.path(), text, 3, 999).is_err());
        let inside = text.find('é').unwrap() + 1;
        assert!(format_lua_range(&b, d.path(), text, inside, end).is_err(), "inside a character is refused");
    }

    #[test]
    fn a_project_stylua_toml_wins() {
        let Some(b) = bin() else { return };
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("stylua.toml"), "indent_type = \"Tabs\"\n").unwrap();
        assert_eq!(format_lua(&b, d.path(), "if x then\nprint(x)\nend\n").unwrap(), "if x then\n\tprint(x)\nend\n");
    }

    #[test]
    fn a_large_file_does_not_deadlock() {
        let Some(b) = bin() else { return };
        let d = tempfile::tempdir().unwrap();
        let big = "local a = 1\n".repeat(200_000);
        assert_eq!(format_lua(&b, d.path(), &big).unwrap().len(), big.len());
    }

    #[test]
    fn a_missing_binary_is_an_error() {
        assert!(format_lua(Path::new("/no/such/stylua"), Path::new("/tmp"), "x").is_err());
    }
}
