//! Running `lua-language-server` for the editor (spec 1): the Language Server
//! Protocol's `Content-Length` framing, finding the bundled binary, and a
//! child process whose messages are handed to a callback. The editor side
//! (Monaco providers) is `src/lib/lsp.ts`; this module only moves bytes, so
//! it is tested with a fake server.

use crate::{err, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Largest message accepted from a server; a bigger header is a broken or
/// hostile peer, not a document.
const MAX_MESSAGE: usize = 64 * 1024 * 1024;

/// Wraps a JSON message in its `Content-Length` header.
pub fn frame(json: &str) -> Vec<u8> {
    let mut v = format!("Content-Length: {}\r\n\r\n", json.len()).into_bytes();
    v.extend_from_slice(json.as_bytes());
    v
}

/// Reads one framed message; `Ok(None)` at a clean end of the stream.
pub fn read_message(r: &mut impl BufRead) -> Result<Option<String>> {
    let mut len: Option<usize> = None;
    let mut seen_any = false;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return if seen_any { err("the language server's stream ended inside a header") } else { Ok(None) };
        }
        seen_any = true;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = Some(v.trim().parse().map_err(|_| crate::Error::Msg(format!("bad Content-Length {v:?}")))?);
            }
        }
    }
    let Some(len) = len else { return err("a language server message had no Content-Length") };
    if len > MAX_MESSAGE {
        return err(format!("a language server message of {len} bytes is too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(Some(String::from_utf8_lossy(&body).into_owned()))
}

/// The `file://` URI of an absolute path (what the protocol uses for documents).
pub fn path_to_uri(p: &Path) -> String {
    let mut s = String::from("file://");
    let raw = p.to_string_lossy().replace('\\', "/");
    if !raw.starts_with('/') {
        s.push('/');
    }
    for b in raw.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => s.push(b as char),
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// `lua-language-server` inside the toolchains folder: `lua-language-server/<arch>/bin/lua-language-server`
/// for this machine's architecture, then the other one (Rosetta runs x64 on
/// Apple silicon). `JOYRIDE_LUALS` overrides, for development.
pub fn find_server(toolchains: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JOYRIDE_LUALS").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let base = toolchains?.join("lua-language-server");
    let order = if cfg!(target_arch = "aarch64") { ["arm64", "x64"] } else { ["x64", "arm64"] };
    let exe = if cfg!(windows) { "lua-language-server.exe" } else { "lua-language-server" };
    order.iter().map(|a| base.join(a).join("bin").join(exe)).find(|p| p.is_file())
}

/// The LuaCATS definitions of LÖVE 11.5 that ship inside the bundled
/// language server (`meta/3rd/love2d/library`, generated from love-api; MIT,
/// as the server is), for the same architectures [`find_server`] tries. A
/// 12.0 project has no 12.0 types yet: it gets these, so 12-only functions
/// show as unknown fields (the lints say which are 12-only).
pub fn find_love_types(toolchains: Option<&Path>) -> Option<PathBuf> {
    let base = toolchains?.join("lua-language-server");
    ["arm64", "x64"].iter().map(|a| base.join(a).join("meta/3rd/love2d/library")).find(|p| p.join("love.lua").is_file())
}

/// A running language server. Messages from it arrive on a background thread
/// through the callback given to [`Server::start`]; [`Server::send`] writes
/// one to it. Dropping it, or [`Server::stop`], kills the process.
pub struct Server {
    child: Arc<Mutex<Child>>,
    stdin: Mutex<Option<std::process::ChildStdin>>,
}

impl Server {
    /// Starts `command` (already holding its arguments). `on_message` gets
    /// each JSON message; `on_end` runs once when the server's output ends.
    pub fn start(mut command: Command, on_message: impl Fn(String) + Send + 'static, on_end: impl FnOnce() + Send + 'static) -> Result<Server> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| crate::Error::Msg(format!("could not start the language server: {e}")))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| crate::Error::Msg("no stdout from the language server".into()))?;
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            // A malformed frame ends the session; the editor then reports the
            // server as stopped rather than reading garbage.
            while let Ok(Some(m)) = read_message(&mut r) {
                on_message(m);
            }
            on_end();
        });
        Ok(Server { child: Arc::new(Mutex::new(child)), stdin: Mutex::new(stdin) })
    }

    /// Sends one JSON message.
    pub fn send(&self, json: &str) -> Result<()> {
        let mut g = self.stdin.lock().unwrap();
        let Some(w) = g.as_mut() else { return err("the language server is not running") };
        w.write_all(&frame(json))?;
        w.flush()?;
        Ok(())
    }

    pub fn stop(&self) {
        self.stdin.lock().unwrap().take();
        let mut c = self.child.lock().unwrap();
        let _ = c.kill();
        let _ = c.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The settings the editor answers `workspace/configuration` with, so the
/// project needs no `.luarc.json` of ours (the user's source is never
/// mutated; a `.luarc.json` the project has still wins, as LuaLS reads it).
pub fn settings(love_types: Option<&Path>) -> serde_json::Value {
    let mut lib = Vec::new();
    if let Some(t) = love_types {
        lib.push(t.to_string_lossy().into_owned());
    }
    serde_json::json!({
        "runtime": { "version": "LuaJIT" },
        "diagnostics": { "globals": ["love"] },
        "workspace": { "library": lib, "checkThirdParty": false },
        "telemetry": { "enable": false },
    })
}

/// Reads exactly what `r` holds into messages (for tests and tools).
pub fn read_all(mut r: impl Read) -> Result<Vec<String>> {
    let mut b = BufReader::new(&mut r);
    let mut out = Vec::new();
    while let Some(m) = read_message(&mut b)? {
        out.push(m);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_with_multibyte_text() {
        let msgs = ["{\"a\":1}", "{\"t\":\"LÖVE ✓\"}"];
        let bytes: Vec<u8> = msgs.iter().flat_map(|m| frame(m)).collect();
        assert!(String::from_utf8_lossy(&bytes).contains("Content-Length: 17"), "length counts bytes, not chars");
        assert_eq!(read_all(&bytes[..]).unwrap(), msgs);
    }

    #[test]
    fn extra_headers_and_case_are_accepted() {
        let raw = b"content-length: 2\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}";
        assert_eq!(read_all(&raw[..]).unwrap(), ["{}"]);
    }

    #[test]
    fn bad_frames_are_errors_not_panics() {
        assert!(read_all(&b"X: 1\r\n\r\n{}"[..]).is_err(), "no length");
        assert!(read_all(&b"Content-Length: nope\r\n\r\n"[..]).is_err());
        assert!(read_all(&b"Content-Length: 99999999999\r\n\r\n"[..]).is_err(), "too large");
        assert!(read_all(&b"Content-Length: 10\r\n\r\n{}"[..]).is_err(), "short body");
        assert!(read_all(&b"Content-Length: 2"[..]).is_err(), "cut header");
        assert!(read_all(&b""[..]).unwrap().is_empty());
    }

    #[test]
    fn uris_escape_what_they_must() {
        assert_eq!(path_to_uri(Path::new("/Users/a b/Löve#1/main.lua")), "file:///Users/a%20b/L%C3%B6ve%231/main.lua");
        assert_eq!(path_to_uri(Path::new("C:\\g\\main.lua")), "file:///C:/g/main.lua");
    }

    #[test]
    fn settings_name_love_and_luajit() {
        let s = settings(Some(Path::new("/t/love-api")));
        assert_eq!(s["runtime"]["version"], "LuaJIT");
        assert_eq!(s["workspace"]["library"][0], "/t/love-api");
        assert_eq!(s["diagnostics"]["globals"][0], "love");
    }

    #[cfg(unix)]
    #[test]
    fn a_server_gets_what_is_sent_and_stops() {
        use std::sync::mpsc;
        // `cat` is a perfect echo server: framed bytes in, the same out.
        let (tx, rx) = mpsc::channel();
        let (end_tx, end_rx) = mpsc::channel();
        let s = Server::start(Command::new("cat"), move |m| tx.send(m).unwrap(), move || end_tx.send(()).unwrap()).unwrap();
        s.send("{\"id\":1}").unwrap();
        s.send("{\"id\":2,\"x\":\"é\"}").unwrap();
        let t = std::time::Duration::from_secs(5);
        assert_eq!(rx.recv_timeout(t).unwrap(), "{\"id\":1}");
        assert_eq!(rx.recv_timeout(t).unwrap(), "{\"id\":2,\"x\":\"é\"}");
        s.stop();
        end_rx.recv_timeout(t).expect("the end callback runs when the server stops");
        assert!(s.send("{}").is_err(), "sending after stop is an error");
    }

    #[test]
    fn love_types_are_found_only_where_they_exist() {
        let d = tempfile::tempdir().unwrap();
        assert!(find_love_types(Some(d.path())).is_none());
        let lib = d.path().join("lua-language-server/x64/meta/3rd/love2d/library");
        std::fs::create_dir_all(&lib).unwrap();
        assert!(find_love_types(Some(d.path())).is_none(), "an empty folder is not the types");
        std::fs::write(lib.join("love.lua"), "").unwrap();
        assert_eq!(find_love_types(Some(d.path())).unwrap(), lib);
        assert!(find_love_types(None).is_none());
    }

    #[test]
    fn a_missing_server_is_an_error() {
        assert!(Server::start(Command::new("/no/such/server"), |_| {}, || {}).is_err());
        assert!(find_server(Some(Path::new("/no/such"))).is_none() || std::env::var_os("JOYRIDE_LUALS").is_some());
    }
}
