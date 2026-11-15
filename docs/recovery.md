# Autosave, crash recovery, error log and crash reports

Nothing here sends anything anywhere (CLAUDE.md rule 1). Everything is a file on the person's own machine, in JoyrIDE's own folders, never in the project (rule 4).

## Drafts (autosave for recovery)

- Every change to an open file starts an 800 ms timer; when it fires, the file's text is written as a **draft** in `<app data>/drafts/<project hash>/` (`joyride_core::recovery::Drafts`, atomic write, one JSON file per path, replaced each time). A file that is clean again (saved, or undone back to the saved text) has its draft removed.
- The project's own files are written only by a save: Cmd+S, or the optional **Save changed files automatically** setting (off by default, 2 s after typing stops, never formats).
- Drafts are flushed at once when the window is asked to close and before another project opens, so quitting with unsaved files (the person answers "Quit without saving?") still leaves them recoverable.
- At the next open of the project, drafts that still differ from the file on disk are offered in the **Recovery** panel: Restore (puts the text into the file's tab as an unsaved, undoable edit; a file deleted since is created empty first), Discard, or close the panel to decide later. A draft equal to its file is dropped silently. Closing a tab on purpose ("Close without saving?") discards its draft.
- Drafts are per project (hash of the folder path), refuse paths outside the project, and keep at most 8 MB a file.

## The error log

`joyride.log` in the OS log folder (macOS `~/Library/Logs/com.ansiapps.joyride/`), written by `tauri-plugin-log`, 1 MB a file, the last three kept. Rust `log::` lines, panics, and every uncaught error or rejected promise in the window (`src/main.tsx` to `report_frontend_error`) land in it. Settings > Diagnostics > Show error log opens the folder.

## Crash reports

One JSON file per event in `<app data>/crash-reports/` (the newest 50 kept), named `<UTC time>-<kind>-<signature>.json`. Three kinds:

| kind | when |
|---|---|
| `panic` | a Rust panic. The hook (installed first thing in `run()`) writes the report, then lets the default handler run; a release build then aborts. A panic before the app's folders are known writes to `<temp>/joyride-crash-reports/`. |
| `frontend-error` | an uncaught error or unhandled rejection in the window; the first 5 a session are written, the rest only logged. |
| `unclean-exit` | the last session left its `session.json` marker behind: a crash outside Rust, a force quit or a power cut. Written at the next start, from that session's log. The window says so once. |

Fields (`schema` 1; `scripts/ingest-crash.py` refuses another): `schema`, `kind`, `id`, `time` (UTC ISO 8601), `app`, `version`, `os`, `arch`, `summary`, `location` (`file:line:col` or the window's source), `thread`, `backtrace`, `signature`, `sessionSecs`, `logTail` (the last 80 log lines).

- **`signature`** groups the same fault: a hash of kind, file of the location and the summary with digit runs removed, so line numbers and ids do not split it.
- **Privacy:** the home folder (so the user name) is replaced with `~` in the summary, location, backtrace and log lines. File contents and project names are never included. A release build is stripped, so its backtrace has addresses, not names; keep the build's symbols to read them.

## Ingesting

```
python3 -I scripts/ingest-crash.py <report.json | folder> ...      # faults grouped by signature, most frequent first
python3 -I scripts/ingest-crash.py --check <paths>                 # fail on a malformed report or unknown schema
python3 -I scripts/ingest-crash.py --show <signature> <paths>      # the newest report of one fault, in full
```

A person attaches a report to an issue (Settings > Diagnostics > Show crash reports, or Copy newest report); nothing is uploaded by the app.

## Tests

`cargo test --offline --manifest-path joyride-core/Cargo.toml recovery` (drafts, redaction, signatures, reports, the log tail, the session marker); `cargo test --offline --manifest-path src-tauri/Cargo.toml recovery` (a real panic through the hook writes a redacted report).

**Not yet seen in a window:** the Recovery panel, the Settings rows, a real crash and restart, and a native crash leaving the marker.
