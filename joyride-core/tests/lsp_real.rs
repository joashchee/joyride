//! The real bundled `lua-language-server`: handshake, then a diagnostic for
//! an undefined global. Skips when the binary was not fetched
//! (`scripts/fetch-toolchains.py`) or cannot run here.

use joyride_core::lsp::{find_love_types, find_server, path_to_uri, settings, Server};
use serde_json::{json, Value};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn luals_initializes_and_reports_a_diagnostic() {
    let tc = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/toolchains");
    let Some(bin) = find_server(Some(&tc)) else {
        eprintln!("skipped: lua-language-server not fetched");
        return;
    };
    let types = find_love_types(Some(&tc));
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    std::fs::write(root.join("main.lua"), "local x = 1\nprint(undefined_thing)\nlove.load = function() end\nlocal _ = love.graphics.newIm\n").unwrap();
    let mut cmd = Command::new(&bin);
    cmd.arg(format!("--logpath={}", root.join("log").display())).arg(format!("--metapath={}", root.join("meta").display()));

    let (tx, rx) = mpsc::channel::<Value>();
    let server = Server::start(cmd, move |m| { let _ = tx.send(serde_json::from_str(&m).unwrap()); }, || {}).unwrap();
    let send = |v: Value| server.send(&v.to_string()).unwrap();
    send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "processId": null, "rootUri": path_to_uri(&root),
        "capabilities": {"workspace": {"configuration": true}, "textDocument": {"publishDiagnostics": {}}},
        "workspaceFolders": [{"uri": path_to_uri(&root), "name": "t"}]}}));

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut got_init = false;
    let mut diag = None;
    while Instant::now() < deadline && diag.is_none() {
        let Ok(m) = rx.recv_timeout(Duration::from_secs(5)) else { continue };
        if m["id"] == 1 && m.get("result").is_some() {
            got_init = true;
            send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
            let uri = path_to_uri(&root.join("main.lua"));
            let text = std::fs::read_to_string(root.join("main.lua")).unwrap();
            send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"lua","version":1,"text":text}}}));
        } else if m["method"] == "workspace/configuration" {
            let n = m["params"]["items"].as_array().map_or(0, |a| a.len());
            send(json!({"jsonrpc":"2.0","id":m["id"],"result": vec![settings(types.as_deref()); n]}));
        } else if m.get("id").is_some() && m.get("method").is_some() {
            send(json!({"jsonrpc":"2.0","id":m["id"],"result":null}));
        } else if m["method"] == "textDocument/publishDiagnostics" {
            let d = m["params"]["diagnostics"].as_array().unwrap().clone();
            if !d.is_empty() { diag = Some(d); }
        }
    }
    assert!(got_init, "no initialize result");
    if types.is_some() {
        // `love.graphics.newIm|` on line 4 (0-based 3), column 29.
        let uri = path_to_uri(&root.join("main.lua"));
        send(json!({"jsonrpc":"2.0","id":7,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":{"line":3,"character":29}}}));
        let mut labels = String::new();
        let end = Instant::now() + Duration::from_secs(30);
        while Instant::now() < end {
            let Ok(m) = rx.recv_timeout(Duration::from_secs(5)) else { continue };
            if m["id"] == 7 {
                labels = m["result"].to_string();
                break;
            }
        }
        assert!(labels.contains("newImage"), "love.graphics completion needs the bundled types: {}", &labels[..labels.len().min(300)]);
    }
    let d = diag.expect("no diagnostics arrived");
    let text = serde_json::to_string(&d).unwrap();
    assert!(text.contains("undefined_thing"), "{text}");
    assert!(!text.contains("global `love`") && !text.contains("'love'"), "love must be a known global: {text}");
    server.stop();
}

/// What the editor's new providers ask for (`src/lib/lsp.ts`): references,
/// prepareRename and rename, documentSymbol and codeAction, answered by the
/// real server, so the client's expectations are measured, not guessed.
#[test]
fn luals_answers_references_rename_symbols_and_code_actions() {
    let tc = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/toolchains");
    let Some(bin) = find_server(Some(&tc)) else {
        eprintln!("skipped: lua-language-server not fetched");
        return;
    };
    let types = find_love_types(Some(&tc));
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    let src = "local function greet(name)\n  return name\nend\nlocal a = greet(1)\nlocal b = greet(2)\nunused_global = 3\n";
    std::fs::write(root.join("main.lua"), src).unwrap();
    let uri = path_to_uri(&root.join("main.lua"));
    let mut cmd = Command::new(&bin);
    cmd.arg(format!("--logpath={}", root.join("log").display())).arg(format!("--metapath={}", root.join("meta").display()));
    let (tx, rx) = mpsc::channel::<Value>();
    let server = Server::start(cmd, move |m| { let _ = tx.send(serde_json::from_str(&m).unwrap()); }, || {}).unwrap();
    let send = |v: Value| server.send(&v.to_string()).unwrap();
    send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "processId": null, "rootUri": path_to_uri(&root),
        "capabilities": {"workspace": {"configuration": true}, "textDocument": {
            "publishDiagnostics": {}, "documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
            "rename": {"prepareSupport": true},
            "codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source"]}}}}},
        "workspaceFolders": [{"uri": path_to_uri(&root), "name": "t"}]}}));

    // Answers one request, serving the server's own requests meanwhile.
    let mut next_id = 100;
    let mut ask = |method: &str, params: Value| -> Value {
        next_id += 1;
        let id = next_id;
        send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let end = Instant::now() + Duration::from_secs(60);
        while Instant::now() < end {
            let Ok(m) = rx.recv_timeout(Duration::from_secs(5)) else { continue };
            if m["id"] == id && m.get("method").is_none() {
                return m;
            } else if m["method"] == "workspace/configuration" {
                let n = m["params"]["items"].as_array().map_or(0, |a| a.len());
                send(json!({"jsonrpc":"2.0","id":m["id"],"result": vec![settings(types.as_deref()); n]}));
            } else if m.get("id").is_some() && m.get("method").is_some() {
                send(json!({"jsonrpc":"2.0","id":m["id"],"result":null}));
            }
        }
        panic!("no answer to {method}");
    };

    let init = ask_initialize(&rx, &send);
    assert!(init, "no initialize result");
    send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"lua","version":1,"text":src}}}));
    let td = json!({"uri": uri});

    // `greet` on line 4 (0-based 3), inside the name.
    let at = json!({"line": 3, "character": 12});
    let refs = ask("textDocument/references", json!({"textDocument": td, "position": at, "context": {"includeDeclaration": true}}));
    assert_eq!(refs["result"].as_array().map(|a| a.len()), Some(3), "declaration and two calls: {refs}");

    let prep = ask("textDocument/prepareRename", json!({"textDocument": td, "position": at}));
    let r = &prep["result"];
    let range = if r["range"].is_object() { &r["range"] } else { r };
    assert_eq!(range["start"]["line"], 3, "prepareRename gives the name's range: {prep}");

    let ren = ask("textDocument/rename", json!({"textDocument": td, "position": at, "newName": "hello"}));
    let edits = ren["result"]["changes"][&uri].as_array().cloned().or_else(|| ren["result"]["documentChanges"][0]["edits"].as_array().cloned());
    assert_eq!(edits.map(|e| e.len()), Some(3), "three places change: {ren}");

    let sym = ask("textDocument/documentSymbol", json!({"textDocument": td}));
    assert!(sym["result"].to_string().contains("greet"), "outline lists the function: {sym}");

    // A quick fix for the undefined-global diagnostic on line 6 may or may
    // not exist in this server version; the request must at least succeed.
    let ca = ask("textDocument/codeAction", json!({"textDocument": td,
        "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 13}},
        "context": {"diagnostics": []}}));
    assert!(ca.get("error").is_none(), "{ca}");
    server.stop();
}

fn ask_initialize(rx: &mpsc::Receiver<Value>, _send: &dyn Fn(Value)) -> bool {
    let end = Instant::now() + Duration::from_secs(60);
    while Instant::now() < end {
        let Ok(m) = rx.recv_timeout(Duration::from_secs(5)) else { continue };
        if m["id"] == 1 && m.get("result").is_some() {
            return true;
        }
    }
    false
}
