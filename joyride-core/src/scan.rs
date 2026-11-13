//! The auto-porter's static analysis (spec 4B): reads a project's Lua and
//! reports its input use, window configuration and what would break on each
//! target. Read-only; it changes nothing in the project.

use crate::lua::{block_end, lex, Tok, Token};
use std::collections::BTreeSet;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    PortMaster,
    /// iOS, iPadOS and Android share one shim.
    Mobile,
    Web,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// The shim handles it; shown for awareness.
    Info,
    Warn,
    /// Will not work on the target as written.
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub target: Target,
    pub level: Level,
    pub file: String,
    pub line: u32,
    pub rule: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Window {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub resizable: Option<bool>,
    pub highdpi: Option<bool>,
    pub fullscreen: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub files: usize,
    /// Every key name the code reads (`"space"`, `"left"`...).
    pub keys: BTreeSet<String>,
    pub keyboard: bool,
    pub mouse: bool,
    pub touch: bool,
    pub gamepad: bool,
    pub window: Window,
    pub findings: Vec<Finding>,
}

impl Scan {
    pub fn for_target(&self, t: Target) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.target == t)
    }
    pub fn blockers(&self, t: Target) -> usize {
        self.for_target(t).filter(|f| f.level == Level::Blocker).count()
    }
}

/// Scans every `.lua` file under `root` (not `build`, `.git` or `.joyride`).
pub fn scan_project(root: &Path) -> Scan {
    let mut sources = Vec::new();
    let walker = WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !(e.depth() > 0 && e.file_type().is_dir() && matches!(e.file_name().to_str(), Some(".git" | ".joyride" | "build" | "node_modules"))));
    for e in walker.flatten() {
        if e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "lua") {
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                let rel = e.path().strip_prefix(root).unwrap_or(e.path()).components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                sources.push((rel, text));
            }
        }
    }
    scan_sources(&sources)
}

pub fn scan_sources(sources: &[(String, String)]) -> Scan {
    let mut s = Scan { files: sources.len(), ..Default::default() };
    for (file, text) in sources {
        let toks = lex(text);
        scan_tokens(file, &toks, &mut s);
        if file == "conf.lua" {
            read_conf(&toks, &mut s.window);
        }
    }
    // Findings that depend on the whole project.
    if s.keyboard && !s.touch {
        s.findings.push(Finding {
            target: Target::Mobile,
            level: Level::Info,
            file: String::new(),
            line: 0,
            rule: "keyboard-only",
            message: "uses the keyboard but has no touch handlers: the shim adds a touch overlay".into(),
        });
    }
    if s.window.fullscreen == Some(true) {
        s.findings.push(Finding {
            target: Target::Mobile,
            level: Level::Warn,
            file: "conf.lua".into(),
            line: 0,
            rule: "fullscreen-config",
            message: "t.window.fullscreen is true; mobile is always fullscreen, so the window size is ignored".into(),
        });
    }
    s.findings.sort_by(|a, b| (a.target, &a.file, a.line, a.rule).cmp(&(b.target, &b.file, b.line, b.rule)));
    s
}

/// `love . <a> . <b>` at `i`: returns the two names.
fn love_member(t: &[Token], i: usize) -> Option<(&str, &str)> {
    if !t.get(i)?.is_name("love") || !t.get(i + 1)?.is_punct(".") {
        return None;
    }
    let (Tok::Name(a), true) = (&t.get(i + 2)?.tok, t.get(i + 3).is_some_and(|x| x.is_punct("."))) else { return None };
    let Tok::Name(b) = &t.get(i + 4)?.tok else { return None };
    Some((a, b))
}

fn scan_tokens(file: &str, t: &[Token], s: &mut Scan) {
    let add = |s: &mut Scan, target, level, line, rule, message: String| {
        s.findings.push(Finding { target, level, file: file.to_string(), line, rule, message });
    };
    let mut i = 0;
    while i < t.len() {
        let line = t[i].line;
        // require "ffi" / require("ffi")
        if t[i].is_name("require") {
            let arg = if t.get(i + 1).is_some_and(|x| x.is_punct("(")) { t.get(i + 2) } else { t.get(i + 1) };
            if let Some(Token { tok: Tok::Str(m), .. }) = arg {
                if m == "ffi" {
                    add(s, Target::Web, Level::Blocker, line, "ffi", "require(\"ffi\") is not available in love.js".into());
                }
            }
        }
        if t[i].is_name("io") && t.get(i + 1).is_some_and(|x| x.is_punct(".")) && t.get(i + 2).is_some_and(|x| x.is_name("popen")) {
            add(s, Target::Web, Level::Blocker, line, "popen", "io.popen blocks and is unavailable in love.js".into());
        }
        if t[i].is_name("os") && t.get(i + 1).is_some_and(|x| x.is_punct(".")) && t.get(i + 2).is_some_and(|x| x.is_name("execute")) {
            add(s, Target::Web, Level::Blocker, line, "execute", "os.execute is unavailable in love.js".into());
        }
        if let Tok::Str(text) = &t[i].tok {
            if let Some(v) = glsl_version(text) {
                if v >= 130 {
                    add(s, Target::PortMaster, Level::Warn, line, "shader-version", format!("shader asks for #version {v}; handhelds run OpenGL ES 2.0/3.0"));
                }
            }
        }
        if let Some((module, member)) = love_member(t, i) {
            match (module, member) {
                ("keyboard", _) => s.keyboard = true,
                ("mouse", m) => {
                    s.mouse = true;
                    add(s, Target::PortMaster, Level::Info, line, "mouse", format!("love.mouse.{m}: no mouse on handhelds; the shim adds a right-stick cursor"));
                }
                ("touch", _) => s.touch = true,
                ("joystick", _) => s.gamepad = true,
                ("thread", "newThread") => add(s, Target::Web, Level::Blocker, line, "thread", "love.thread.newThread is unavailable in love.js".into()),
                ("graphics", "getWidth" | "getHeight" | "getDimensions") => add(
                    s,
                    Target::PortMaster,
                    Level::Info,
                    line,
                    "screen-size",
                    format!("love.graphics.{member}() depends on the screen; the shim reports the design size"),
                ),
                ("window", "setFullscreen") => add(s, Target::Mobile, Level::Info, line, "set-fullscreen", "love.window.setFullscreen has no effect on mobile".into()),
                _ => {}
            }
            if module == "graphics" && matches!(member, "getWidth" | "getHeight" | "getDimensions") {
                add(s, Target::Mobile, Level::Info, line, "screen-size", format!("love.graphics.{member}() depends on the screen; the shim reports the design size"));
            }
            if module == "keyboard" && (member == "isDown") {
                collect_strings_in_call(t, i + 5, &mut s.keys);
            }
        }
        // function love.keypressed(key) ... / love.mousepressed / love.touch* / love.gamepad*
        if t[i].is_name("function") && t.get(i + 1).is_some_and(|x| x.is_name("love")) && t.get(i + 2).is_some_and(|x| x.is_punct(".")) {
            if let Some(Token { tok: Tok::Name(h), .. }) = t.get(i + 3) {
                match h.as_str() {
                    "keypressed" | "keyreleased" => {
                        s.keyboard = true;
                        if let Some(Token { tok: Tok::Name(param), .. }) = t.get(i + 5) {
                            collect_compared(t, i, param, &mut s.keys);
                        }
                    }
                    "mousepressed" | "mousereleased" | "mousemoved" => {
                        s.mouse = true;
                        add(s, Target::PortMaster, Level::Info, line, "mouse", format!("love.{h} handler: no mouse on handhelds; the shim adds a right-stick cursor"));
                    }
                    "touchpressed" | "touchreleased" | "touchmoved" => s.touch = true,
                    "gamepadpressed" | "gamepadreleased" | "gamepadaxis" | "joystickpressed" | "joystickadded" => s.gamepad = true,
                    _ => {}
                }
            }
        }
        i += 1;
    }
}

fn glsl_version(text: &str) -> Option<u32> {
    let i = text.find("#version")?;
    text[i + 8..].trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// String literals inside the parentheses of the call whose `(` is at `open`
/// (or just after a name).
fn collect_strings_in_call(t: &[Token], mut i: usize, out: &mut BTreeSet<String>) {
    if !t.get(i).is_some_and(|x| x.is_punct("(")) {
        return;
    }
    let mut depth = 0;
    while let Some(tok) = t.get(i) {
        match &tok.tok {
            Tok::Punct(p) if p == "(" => depth += 1,
            Tok::Punct(p) if p == ")" => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            Tok::Str(k) if depth >= 1 => {
                out.insert(k.clone());
            }
            _ => {}
        }
        i += 1;
    }
}

/// Inside the function starting at `func`, strings compared to `param`.
fn collect_compared(t: &[Token], func: usize, param: &str, out: &mut BTreeSet<String>) {
    let end = block_end(t, func);
    for k in func..end {
        let is_param = |x: &Token| x.is_name(param);
        if t[k].is_punct("==") || t[k].is_punct("~=") {
            if let (Some(a), Some(b)) = (k.checked_sub(1).and_then(|p| t.get(p)), t.get(k + 1)) {
                match (&a.tok, &b.tok) {
                    (_, Tok::Str(v)) if is_param(a) => {
                        out.insert(v.clone());
                    }
                    (Tok::Str(v), _) if is_param(b) => {
                        out.insert(v.clone());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn read_conf(t: &[Token], w: &mut Window) {
    for i in 0..t.len().saturating_sub(6) {
        if t[i].is_name("t") && t[i + 1].is_punct(".") && t[i + 2].is_name("window") && t[i + 3].is_punct(".") && t[i + 5].is_punct("=") {
            let Tok::Name(field) = &t[i + 4].tok else { continue };
            let num = |t: &Token| if let Tok::Number(n) = t.tok { Some(n) } else { None };
            let boolean = |t: &Token| if t.is_name("true") { Some(true) } else if t.is_name("false") { Some(false) } else { None };
            let v = &t[i + 6];
            match field.as_str() {
                "width" => w.width = num(v),
                "height" => w.height = num(v),
                "resizable" => w.resizable = boolean(v),
                "highdpi" => w.highdpi = boolean(v),
                "fullscreen" => w.fullscreen = boolean(v),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(files: &[(&str, &str)]) -> Scan {
        scan_sources(&files.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>())
    }
    fn rules(s: &Scan, t: Target) -> Vec<&'static str> {
        s.for_target(t).map(|f| f.rule).collect()
    }

    #[test]
    fn collects_keys_from_isdown_and_keypressed() {
        let s = scan(&[(
            "main.lua",
            r#"
            function love.update(dt)
              if love.keyboard.isDown("left", "a") then x = x - 1 end
              if love.keyboard.isDown('right') then x = x + 1 end
            end
            function love.keypressed(key, scancode)
              if key == "space" then jump() elseif "escape" == key then love.event.quit() end
              if other == "nokey" then end
            end
            -- love.keyboard.isDown("commented")
            "#,
        )]);
        assert_eq!(s.keys.iter().cloned().collect::<Vec<_>>(), ["a", "escape", "left", "right", "space"]);
        assert!(s.keyboard && !s.mouse && !s.touch);
    }

    #[test]
    fn reads_the_window_from_conf() {
        let s = scan(&[("conf.lua", "function love.conf(t)\n t.window.width = 1280\n t.window.height=720 t.window.resizable = true\n t.window.fullscreen = true\nend")]);
        assert_eq!(s.window, Window { width: Some(1280.0), height: Some(720.0), resizable: Some(true), highdpi: None, fullscreen: Some(true) });
        assert!(rules(&s, Target::Mobile).contains(&"fullscreen-config"));
    }

    #[test]
    fn web_blockers() {
        let s = scan(&[("main.lua", "local ffi = require('ffi')\nlocal t = love.thread.newThread('x')\nio.popen('ls')\nos.execute('ls')\nlocal s = \"io.popen os.execute\"")]);
        assert_eq!(rules(&s, Target::Web), ["ffi", "thread", "popen", "execute"]);
        assert_eq!(s.blockers(Target::Web), 4);
        assert_eq!(s.blockers(Target::Mobile), 0);
    }

    #[test]
    fn handheld_and_mobile_findings() {
        let s = scan(&[(
            "main.lua",
            "function love.mousepressed(x,y,b) end\nlocal w = love.graphics.getWidth()\nlocal sh = love.graphics.newShader([[#version 330\nvoid main(){}]])",
        )]);
        let pm = rules(&s, Target::PortMaster);
        assert!(pm.contains(&"mouse") && pm.contains(&"screen-size") && pm.contains(&"shader-version"));
        assert!(s.mouse);
        // Keyboard-free: no touch-overlay note. Add a keyboard use and there is one.
        assert!(!rules(&s, Target::Mobile).contains(&"keyboard-only"));
        let s2 = scan(&[("main.lua", "function love.keypressed(k) end")]);
        assert!(rules(&s2, Target::Mobile).contains(&"keyboard-only"));
        let s3 = scan(&[("main.lua", "function love.keypressed(k) end\nfunction love.touchpressed() end")]);
        assert!(!rules(&s3, Target::Mobile).contains(&"keyboard-only"));
    }

    #[test]
    fn gamepad_and_old_shaders_are_fine() {
        let s = scan(&[("main.lua", "function love.gamepadpressed(j,b) end\nlocal sh = love.graphics.newShader('#version 100\\nvoid main(){}')")]);
        assert!(s.gamepad);
        assert!(!rules(&s, Target::PortMaster).contains(&"shader-version"));
    }

    #[test]
    fn scans_a_directory_skipping_build_and_git() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("build")).unwrap();
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("main.lua"), "love.keyboard.isDown('x')").unwrap();
        std::fs::write(d.path().join("src/a.lua"), "io.popen('x')").unwrap();
        std::fs::write(d.path().join("build/b.lua"), "os.execute('x')").unwrap();
        let s = scan_project(d.path());
        assert_eq!(s.files, 2);
        assert_eq!(s.for_target(Target::Web).next().unwrap().file, "src/a.lua");
        assert_eq!(s.blockers(Target::Web), 1);
    }
}
