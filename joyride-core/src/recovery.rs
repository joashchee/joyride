//! Autosave drafts, crash reports and the session marker (`docs/recovery.md`).
//!
//! Nothing here touches the person's source files. An unsaved edit is kept as a
//! *draft* in the app-data folder, keyed by project, and removed the moment the
//! file is saved; after a crash the drafts that still differ from disk are
//! offered back. A crash (a panic, an unhandled error in the window, or an exit
//! that never said goodbye) writes one JSON *report* with a fixed schema, so
//! reports can be collected and ingested (`scripts/ingest-crash.py`). Reports
//! never leave the machine on their own: no telemetry (CLAUDE.md rule 1).

use crate::{err, files, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The report format's version; `scripts/ingest-crash.py` refuses an unknown one.
pub const SCHEMA: u32 = 1;
/// A draft larger than this is not kept (the editor refuses such files anyway).
const MAX_DRAFT_BYTES: usize = 8 * 1024 * 1024;
/// Reports kept in the folder; the oldest go first.
const KEEP_REPORTS: usize = 50;

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `2023-11-14T22:13:20Z` for seconds since the Unix epoch (UTC, no dependency).
pub fn iso(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    Ok(())
}

// ---- Drafts ---------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct DraftFile {
    path: String,
    text: String,
    saved_at: u64,
}

/// An unsaved edit found after a restart.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub path: String,
    pub text: String,
    /// Seconds since the epoch when the draft was last written.
    pub saved_at: u64,
    /// The file no longer exists on disk (restoring creates it again).
    pub missing: bool,
}

/// One project's drafts, in `<app data>/drafts/<hash of the project folder>/`.
pub struct Drafts {
    dir: PathBuf,
}

impl Drafts {
    pub fn for_project(app_data: &Path, root: &Path) -> Drafts {
        let key = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
        Drafts { dir: app_data.join("drafts").join(&key.as_str()[..16]) }
    }

    fn file(&self, rel: &str) -> PathBuf {
        self.dir.join(format!("{}.json", &blake3::hash(rel.as_bytes()).to_hex().as_str()[..32]))
    }

    fn check(rel: &str) -> Result<()> {
        let p = Path::new(rel);
        if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
            return err(format!("`{rel}` is not a path in the project"));
        }
        Ok(())
    }

    /// Keeps `text` as the draft of `rel`, replacing the last one.
    pub fn save(&self, rel: &str, text: &str) -> Result<()> {
        Self::check(rel)?;
        if text.len() > MAX_DRAFT_BYTES {
            return err(format!("`{rel}` is too large to keep a draft of"));
        }
        let body = serde_json::to_vec(&DraftFile { path: rel.to_string(), text: text.to_string(), saved_at: now_secs() })
            .map_err(|e| crate::Error::Msg(e.to_string()))?;
        write_atomic(&self.file(rel), &body)
    }

    /// Forgets the draft of `rel` (the file was saved, or the edit was thrown away).
    pub fn discard(&self, rel: &str) {
        let _ = fs::remove_file(self.file(rel));
    }

    /// Forgets every draft of the project.
    pub fn clear(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    /// The drafts that still differ from what is on disk, oldest first. A draft that
    /// matches its file (it was saved some other way) is removed here.
    pub fn pending(&self, root: &Path) -> Vec<Draft> {
        let Ok(rd) = fs::read_dir(&self.dir) else { return vec![] };
        let mut out = vec![];
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Some(d) = fs::read(&p).ok().and_then(|b| serde_json::from_slice::<DraftFile>(&b).ok()) else {
                let _ = fs::remove_file(&p); // unreadable: nothing to offer
                continue;
            };
            if Self::check(&d.path).is_err() {
                let _ = fs::remove_file(&p);
                continue;
            }
            match files::read_text(root, &d.path) {
                Ok(disk) if disk == d.text => {
                    let _ = fs::remove_file(&p);
                }
                Ok(_) => out.push(Draft { path: d.path, text: d.text, saved_at: d.saved_at, missing: false }),
                Err(_) if !root.join(&d.path).exists() => out.push(Draft { path: d.path, text: d.text, saved_at: d.saved_at, missing: true }),
                // Exists but cannot be read as text (binary, too large): a draft of it means nothing.
                Err(_) => {
                    let _ = fs::remove_file(&p);
                }
            }
        }
        out.sort_by(|a, b| a.saved_at.cmp(&b.saved_at).then_with(|| a.path.cmp(&b.path)));
        out
    }
}

// ---- Reports ----------------------------------------------------------------

/// One crash or error report. `docs/recovery.md` describes every field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: u32,
    /// `panic`, `frontend-error` or `unclean-exit`.
    pub kind: String,
    pub id: String,
    /// UTC, ISO 8601.
    pub time: String,
    pub app: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub summary: String,
    pub location: Option<String>,
    pub thread: Option<String>,
    pub backtrace: Option<String>,
    /// Groups the same fault across reports: digits and addresses are left out of it.
    pub signature: String,
    /// How long the session had run (when known).
    pub session_secs: Option<u64>,
    /// The last lines of the log, home folder removed.
    pub log_tail: Vec<String>,
}

/// A report as listed, without its backtrace and log.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportInfo {
    pub file: String,
    pub kind: String,
    pub time: String,
    pub summary: String,
    pub signature: String,
}

/// Replaces the person's home folder (and so their user name) with `~`.
pub fn redact(text: &str, home: Option<&str>) -> String {
    let Some(h) = home.map(|h| h.trim_end_matches(['/', '\\'])).filter(|h| h.len() > 1) else { return text.to_string() };
    let mut t = text.replace(h, "~");
    let slashed = h.replace('\\', "/");
    if slashed != h {
        t = t.replace(&slashed, "~");
    }
    t
}

/// The same fault gives the same signature: digit runs (lines, ids, addresses) are dropped first.
pub fn signature(kind: &str, location: Option<&str>, summary: &str) -> String {
    let squash = |s: &str| {
        let mut o = String::new();
        let mut last_digit = false;
        for c in s.chars() {
            let d = c.is_ascii_digit();
            if !(d && last_digit) {
                o.push(if d { '#' } else { c });
            }
            last_digit = d;
        }
        o
    };
    let loc = location.map(|l| l.rsplit_once(':').map(|(a, _)| a).unwrap_or(l)); // file:line:col -> file:line
    let loc = loc.map(|l| l.rsplit_once(':').map(|(a, _)| a).unwrap_or(l)); // -> file
    let key = format!("{kind}|{}|{}", loc.unwrap_or(""), squash(summary));
    blake3::hash(key.as_bytes()).to_hex().as_str()[..12].to_string()
}

impl Report {
    /// A report for `now`, filled with what is the same for every kind.
    pub fn new(kind: &str, version: &str, summary: String, location: Option<String>) -> Report {
        let t = now_secs();
        let signature = signature(kind, location.as_deref(), &summary);
        Report {
            schema: SCHEMA,
            kind: kind.to_string(),
            id: format!("{}-{kind}-{signature}", iso(t).replace([':', '-'], "")),
            time: iso(t),
            app: "JoyrIDE".into(),
            version: version.to_string(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            summary,
            location,
            thread: None,
            backtrace: None,
            signature,
            session_secs: None,
            log_tail: vec![],
        }
    }
}

/// Writes the report into `dir` (created if needed) and prunes the oldest beyond 50.
pub fn write_report(dir: &Path, r: &Report) -> Result<PathBuf> {
    let body = serde_json::to_vec_pretty(r).map_err(|e| crate::Error::Msg(e.to_string()))?;
    let mut path = dir.join(format!("{}.json", r.id));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("{}-{n}.json", r.id));
    }
    write_atomic(&path, &body)?;
    prune(dir);
    Ok(path)
}

fn prune(dir: &Path) {
    let mut names: Vec<_> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".json")).collect())
        .unwrap_or_default();
    names.sort(); // names start with the UTC time
    while names.len() > KEEP_REPORTS {
        let _ = fs::remove_file(dir.join(names.remove(0)));
    }
}

/// The reports in `dir`, newest first.
pub fn list_reports(dir: &Path) -> Vec<ReportInfo> {
    let mut out: Vec<ReportInfo> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if !name.ends_with(".json") {
                        return None;
                    }
                    let r: Report = serde_json::from_slice(&fs::read(e.path()).ok()?).ok()?;
                    Some(ReportInfo { file: name, kind: r.kind, time: r.time, summary: r.summary, signature: r.signature })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.file.cmp(&a.file));
    out
}

/// A report's text, by its file name in `dir` (a name only: nothing outside the folder).
pub fn read_report(dir: &Path, file: &str) -> Result<String> {
    if file.is_empty() || file.contains(['/', '\\']) || !file.ends_with(".json") {
        return err("not a report");
    }
    Ok(fs::read_to_string(dir.join(file))?)
}

/// The last `max_lines` lines of a log file, read from its last 64 KB, home folder removed.
pub fn log_tail(path: &Path, max_lines: usize, home: Option<&str>) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(path) else { return vec![] };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(64 * 1024);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(from)).is_err() || f.read_to_end(&mut buf).is_err() {
        return vec![];
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if from > 0 && !lines.is_empty() {
        lines.remove(0); // a cut line
    }
    let skip = lines.len().saturating_sub(max_lines);
    lines[skip..].iter().map(|l| redact(l, home)).collect()
}

// ---- Session marker -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub pid: u32,
    pub started: u64,
    pub version: String,
}

fn marker(dir: &Path) -> PathBuf {
    dir.join("session.json")
}

/// Marks this session as running. If the last one left its marker behind, it never
/// exited cleanly (a crash, a kill, a power cut) and its record is returned.
pub fn begin_session(dir: &Path, version: &str) -> Option<Session> {
    let prev = fs::read(marker(dir)).ok().and_then(|b| serde_json::from_slice::<Session>(&b).ok());
    let me = Session { pid: std::process::id(), started: now_secs(), version: version.to_string() };
    if let Ok(b) = serde_json::to_vec(&me) {
        let _ = write_atomic(&marker(dir), &b);
    }
    prev
}

/// A clean exit: the marker goes.
pub fn end_session(dir: &Path) {
    let _ = fs::remove_file(marker(dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (p, t) in files {
            fs::write(d.path().join(p), t).unwrap();
        }
        d
    }

    #[test]
    fn iso_is_utc() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn a_draft_comes_back_until_the_file_matches_it() {
        let app = tempfile::tempdir().unwrap();
        let proj = project(&[("main.lua", "print(1)\n")]);
        let d = Drafts::for_project(app.path(), proj.path());
        d.save("main.lua", "print(2)\n").unwrap();
        let p = d.pending(proj.path());
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].path.as_str(), p[0].text.as_str(), p[0].missing), ("main.lua", "print(2)\n", false));
        // Saved some other way: the draft means nothing and is dropped.
        fs::write(proj.path().join("main.lua"), "print(2)\n").unwrap();
        assert!(d.pending(proj.path()).is_empty());
        assert!(d.pending(proj.path()).is_empty());
    }

    #[test]
    fn saving_a_draft_replaces_it_and_discard_removes_it() {
        let app = tempfile::tempdir().unwrap();
        let proj = project(&[("a.lua", "x"), ("b.lua", "y")]);
        let d = Drafts::for_project(app.path(), proj.path());
        d.save("a.lua", "x1").unwrap();
        d.save("a.lua", "x2").unwrap();
        d.save("b.lua", "y1").unwrap();
        let p = d.pending(proj.path());
        assert_eq!(p.len(), 2);
        assert_eq!(p.iter().find(|x| x.path == "a.lua").unwrap().text, "x2");
        d.discard("a.lua");
        assert_eq!(d.pending(proj.path()).len(), 1);
        d.clear();
        assert!(d.pending(proj.path()).is_empty());
    }

    #[test]
    fn a_draft_of_a_deleted_file_is_kept_and_marked() {
        let app = tempfile::tempdir().unwrap();
        let proj = project(&[]);
        let d = Drafts::for_project(app.path(), proj.path());
        d.save("gone.lua", "work").unwrap();
        let p = d.pending(proj.path());
        assert_eq!(p.len(), 1);
        assert!(p[0].missing);
    }

    #[test]
    fn projects_do_not_share_drafts() {
        let app = tempfile::tempdir().unwrap();
        let (p1, p2) = (project(&[("m.lua", "")]), project(&[("m.lua", "")]));
        Drafts::for_project(app.path(), p1.path()).save("m.lua", "one").unwrap();
        assert!(Drafts::for_project(app.path(), p2.path()).pending(p2.path()).is_empty());
        assert_eq!(Drafts::for_project(app.path(), p1.path()).pending(p1.path()).len(), 1);
    }

    #[test]
    fn a_draft_cannot_name_a_path_outside_the_project() {
        let app = tempfile::tempdir().unwrap();
        let proj = project(&[]);
        let d = Drafts::for_project(app.path(), proj.path());
        for bad in ["../x.lua", "/etc/passwd", "", "a/../../b"] {
            assert!(d.save(bad, "x").is_err(), "{bad}");
        }
    }

    #[test]
    fn redact_removes_the_home_folder() {
        assert_eq!(redact("open /Users/ann/game/main.lua failed", Some("/Users/ann")), "open ~/game/main.lua failed");
        assert_eq!(redact(r"C:\Users\ann\g.lua and C:/Users/ann/h.lua", Some(r"C:\Users\ann")), "~\\g.lua and ~/h.lua");
        assert_eq!(redact("nothing", Some("/")), "nothing");
        assert_eq!(redact("nothing", None), "nothing");
    }

    #[test]
    fn the_signature_ignores_numbers_but_not_the_fault() {
        let a = signature("panic", Some("src/run.rs:41:9"), "index 7 out of range for length 3");
        let b = signature("panic", Some("src/run.rs:52:13"), "index 12 out of range for length 40");
        let c = signature("panic", Some("src/run.rs:41:9"), "called `Option::unwrap()` on a `None` value");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 12);
    }

    #[test]
    fn reports_are_written_listed_newest_first_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Report::new("panic", "0.2.0", "boom".into(), Some("a.rs:1:1".into()));
        r.backtrace = Some("bt".into());
        let path = write_report(dir.path(), &r).unwrap();
        let back: Report = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.schema, SCHEMA);
        // The same id written twice does not overwrite.
        let again = write_report(dir.path(), &r).unwrap();
        assert_ne!(path, again);
        assert_eq!(list_reports(dir.path()).len(), 2);
        for i in 0..60 {
            let mut x = r.clone();
            x.id = format!("2099{i:04}-panic-x");
            write_report(dir.path(), &x).unwrap();
        }
        let l = list_reports(dir.path());
        assert_eq!(l.len(), KEEP_REPORTS);
        assert!(l[0].file.starts_with("2099"));
        assert!(read_report(dir.path(), &l[0].file).unwrap().contains("\"schema\""));
        assert!(read_report(dir.path(), "../x.json").is_err());
        assert!(read_report(dir.path(), "session.txt").is_err());
    }

    #[test]
    fn the_log_tail_keeps_the_last_lines_without_the_home_folder() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("joyride.log");
        let text: String = (0..300).map(|i| format!("line {i} /Users/ann/x\n")).collect();
        fs::write(&log, text).unwrap();
        let t = log_tail(&log, 5, Some("/Users/ann"));
        assert_eq!(t.len(), 5);
        assert_eq!(t[4], "line 299 ~/x");
        assert!(log_tail(&dir.path().join("none.log"), 5, None).is_empty());
    }

    #[test]
    fn a_marker_left_behind_means_the_last_session_did_not_exit() {
        let dir = tempfile::tempdir().unwrap();
        assert!(begin_session(dir.path(), "0.2.0").is_none()); // the first run
        end_session(dir.path());
        assert!(begin_session(dir.path(), "0.2.0").is_none()); // a clean exit
        // No end_session: the next start sees it.
        let prev = begin_session(dir.path(), "0.3.0").unwrap();
        assert_eq!(prev.version, "0.2.0");
        assert_eq!(prev.pid, std::process::id());
    }
}
