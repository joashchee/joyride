//! Running a project with a LÖVE binary and streaming its output (spec 3C).

use crate::trace::{self, Link};
use crate::{err, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub love: PathBuf,
    pub project: PathBuf,
    /// Extra environment, e.g. a simulator's screen size for the shim.
    pub env: Vec<(String, String)>,
}

impl Spec {
    pub fn command(&self) -> Result<Command> {
        if !self.project.join("main.lua").is_file() {
            return err(format!("{} has no main.lua", self.project.display()));
        }
        let mut c = Command::new(&self.love);
        c.arg(&self.project).current_dir(&self.project);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        Ok(c)
    }
}

/// Runs to completion, calling `on_line` for every line of either stream,
/// with a clickable link when the line names a source location. Lines of the
/// two streams arrive in the order they were read, not strictly the order
/// written.
pub fn run(spec: &Spec, mut on_line: impl FnMut(Stream, &str, Option<Link>)) -> Result<ExitStatus> {
    let mut child = spec.command()?.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| crate::Error::Msg(format!("could not start {}: {e}", spec.love.display())))?;
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    for (stream, pipe) in [(Stream::Stdout, child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>)), (Stream::Stderr, child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>))] {
        let Some(pipe) = pipe else { continue };
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(pipe).split(b'\n').flatten() {
                let _ = tx.send((stream, String::from_utf8_lossy(&line).trim_end_matches('\r').to_string()));
            }
        }));
    }
    drop(tx);
    for (stream, line) in rx {
        let l = trace::link(&line);
        on_line(stream, &line, l);
    }
    for r in readers {
        let _ = r.join();
    }
    Ok(child.wait()?)
}

/// Resolves a link's file against the project, only if it stays inside it.
pub fn resolve_link(project: &Path, link: &Link) -> Option<PathBuf> {
    let p = project.join(&link.file);
    let (canon, root) = (p.canonicalize().ok()?, project.canonicalize().ok()?);
    canon.starts_with(root).then_some(canon)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_love(dir: &Path, script: &str) -> PathBuf {
        let p = dir.join("love");
        std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn streams_both_outputs_and_links_errors() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("game");
        std::fs::create_dir_all(proj.join("src")).unwrap();
        std::fs::write(proj.join("main.lua"), "").unwrap();
        std::fs::write(proj.join("src/a.lua"), "").unwrap();
        let love = fake_love(d.path(), "echo hello\necho 'Error: src/a.lua:7: boom' >&2\nexit 3");
        let spec = Spec { love, project: proj.clone(), env: vec![] };
        let mut lines = Vec::new();
        let status = run(&spec, |s, l, k| lines.push((s, l.to_string(), k))).unwrap();
        assert_eq!(status.code(), Some(3));
        assert!(lines.iter().any(|(s, l, k)| *s == Stream::Stdout && l == "hello" && k.is_none()));
        let (_, _, link) = lines.iter().find(|(s, _, _)| *s == Stream::Stderr).unwrap();
        let link = link.clone().unwrap();
        assert_eq!((link.file.as_str(), link.line), ("src/a.lua", 7));
        assert!(resolve_link(&proj, &link).unwrap().ends_with("src/a.lua"));
    }

    #[test]
    fn links_cannot_escape_the_project() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("game")).unwrap();
        std::fs::write(d.path().join("secret.lua"), "").unwrap();
        let l = Link { file: "../secret.lua".into(), line: 1, message: String::new(), span: (0, 0) };
        assert!(resolve_link(&d.path().join("game"), &l).is_none());
    }

    #[test]
    fn needs_a_main_lua_and_a_binary() {
        let d = tempfile::tempdir().unwrap();
        let spec = Spec { love: "/no/such/love".into(), project: d.path().into(), env: vec![] };
        assert!(run(&spec, |_, _, _| {}).is_err());
        std::fs::write(d.path().join("main.lua"), "").unwrap();
        assert!(run(&spec, |_, _, _| {}).is_err());
    }
}
