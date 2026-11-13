//! The 11.5 variant of a 12.0 game, for every export.
//!
//! PortMaster has no LÖVE 12.0 runtime, and a 12.0 game may want to ship
//! somewhere running 11.5. A *variant* is built from a staged copy (the
//! project is never changed): the conf is pinned to 11.5, and a generated
//! `_jr_compat.lua` defines the 12.0-only functions the project actually uses,
//! where 11.5 can honour them. Each such function is one of:
//!
//! - **Exact**: 11.5 can do precisely what 12.0 does (`love.filesystem.openFile`
//!   is `newFile` with the mode required; `love.event.restart` is
//!   `love.event.quit("restart")`). Both were compared on the real 11.5 and
//!   12.0 binaries (2026-11-13), including their error messages.
//! - **Reduced**: it works, but gives up something 12.0 has (`love.sensor`: the
//!   variant behaves as a device with no sensors; `love.markDeprecated`: no
//!   notice is printed).
//! - **Unsupported**: no honest emulation exists. The build stops and lists
//!   every use with its file and line, unless the person has listed that API as
//!   accepted (because the game guards it, say). Never defined, so a feature
//!   test such as `if love.system.getMemorySize then` still answers truthfully.

use crate::lint::ONLY_IN_12;
use crate::lua::lex;
use crate::toolchain::LoveVersion;
use std::collections::BTreeSet;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Exact,
    Reduced(&'static str),
    Unsupported(&'static str),
}

/// How the 11.5 variant treats a 12.0-only API.
pub fn support(api: &str) -> Option<Support> {
    if !ONLY_IN_12.contains(&api) {
        return None;
    }
    Some(match api {
        "love.filesystem.openFile" | "love.event.restart" => Support::Exact,
        "love.markDeprecated" => Support::Reduced("deprecation notices are not printed"),
        a if a.starts_with("love.sensor.") => Support::Reduced("behaves as a device with no sensors"),
        "love.graphics.newTexture" => Support::Unsupported("not verified on 11.5, and not emulated"),
        a if a.starts_with("love.physics.") => Support::Unsupported("11.5 has no equivalent with the same arguments"),
        a if a.starts_with("love.filesystem.") => Support::Unsupported("11.5 cannot mount or resolve these paths"),
        a if a.starts_with("love.math.") => Support::Unsupported("11.5's love.math.noise gives different values"),
        _ => Support::Unsupported("11.5 has no equivalent"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub api: String,
    pub file: String,
    pub line: u32,
    pub support: Support,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub uses: Vec<Use>,
}

impl Report {
    /// Uses with no honest emulation, except the APIs the person accepted.
    pub fn blockers(&self, allow: &[String]) -> Vec<&Use> {
        self.uses.iter().filter(|u| matches!(u.support, Support::Unsupported(_)) && !allow.contains(&u.api)).collect()
    }
    pub fn reduced(&self) -> Vec<&Use> {
        self.uses.iter().filter(|u| matches!(u.support, Support::Reduced(_))).collect()
    }
    fn apis(&self) -> BTreeSet<&str> {
        self.uses.iter().filter(|u| !matches!(u.support, Support::Unsupported(_))).map(|u| u.api.as_str()).collect()
    }
}

/// What the person is offered for a project: only a 12.0 (or later) project
/// has an 11.5 variant to make.
pub fn offered(project_version: LoveVersion) -> bool {
    project_version.is_12_or_later()
}

pub fn analyze_sources(sources: &[(String, String)]) -> Report {
    let mut uses = Vec::new();
    for (file, text) in sources {
        let t = lex(text);
        for i in 0..t.len() {
            if !(t[i].is_name("love") && t.get(i + 1).is_some_and(|x| x.is_punct("."))) {
                continue;
            }
            // The longest dotted name starting here, then every prefix that is a known API.
            let mut name = String::from("love");
            let mut k = i + 1;
            while t.get(k).is_some_and(|x| x.is_punct(".")) {
                let Some(crate::lua::Tok::Name(n)) = t.get(k + 1).map(|x| &x.tok) else { break };
                name.push('.');
                name.push_str(n);
                k += 2;
                if let Some(s) = support(&name) {
                    uses.push(Use { api: name.clone(), file: file.clone(), line: t[i].line, support: s });
                    break;
                }
            }
        }
    }
    Report { uses }
}

pub fn analyze_project(root: &Path) -> Report {
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
    analyze_sources(&sources)
}

/// The source of `_jr_compat.lua`: only what this project's uses need.
pub fn polyfill_lua(r: &Report) -> String {
    let need = r.apis();
    let mut s = String::from("-- _jr_compat.lua: generated by JoyrIDE. This LÖVE 12.0 game is built for LÖVE 11.5;\n-- only the 12.0 functions it uses are defined here (see compat.rs for what each does).\n");
    if need.contains("love.filesystem.openFile") {
        s.push_str(OPEN_FILE);
    }
    if need.contains("love.event.restart") {
        s.push_str(RESTART);
    }
    if need.contains("love.markDeprecated") {
        s.push_str(MARK_DEPRECATED);
    }
    if need.iter().any(|a| a.starts_with("love.sensor.")) {
        s.push_str(SENSOR);
    }
    s
}

const OPEN_FILE: &str = r#"
-- 12.0's openFile is newFile with the mode required.
love.filesystem.openFile = function(name, mode)
  if type(mode) ~= "string" then
    error("bad argument #2 to '?' (string expected, got " .. (mode == nil and "no value" or type(mode)) .. ")", 2)
  end
  return love.filesystem.newFile(name, mode)
end
"#;

const MARK_DEPRECATED: &str = r#"
-- Same argument checks as 12.0's, then nothing: 11.5 has no notice to print.
love.markDeprecated = function(_, _, apitype, deptype)
  local api = { "function", "functionvariant", "method", "methodvariant", "callback", "field", "constant", "custom" }
  local dep = { "noreplacement", "replaced", "renamed" }
  local function oneof(v, list)
    for _, x in ipairs(list) do if x == v then return true end end
    return false
  end
  local function quoted(list) return "'" .. table.concat(list, "', '") .. "'" end
  if not oneof(apitype, api) then error("Invalid API type '" .. tostring(apitype) .. "', expected one of: " .. quoted(api), 2) end
  if not oneof(deptype, dep) then error("Invalid deprecation type '" .. tostring(deptype) .. "', expected one of: " .. quoted(dep), 2) end
end
"#;

const RESTART: &str = r#"
love.event.restart = function() love.event.quit("restart") end
"#;

const SENSOR: &str = r#"
-- No sensors on 11.5: every query answers as a device without any.
do
  local kinds = { accelerometer = true, gyroscope = true }
  local function check(kind, level)
    if not kinds[kind] then
      error("Invalid sensor mode '" .. tostring(kind) .. "', expected one of: 'accelerometer', 'gyroscope'", level + 2)
    end
  end
  -- Errors point at the caller of these functions (level 2), as LÖVE's own do.
  local function off(kind) check(kind, 1); error('"' .. kind .. '" sensor is not enabled', 2) end
  love.sensor = love.sensor or {
    hasSensor = function(kind) check(kind, 1); return false end,
    isEnabled = function(kind) check(kind, 1); return false end,
    setEnabled = function(kind) check(kind, 1) end,
    getName = off,
    getData = off,
  }
end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(src: &str) -> Report {
        analyze_sources(&[("main.lua".into(), src.into())])
    }

    #[test]
    fn finds_uses_with_lines_and_classifies_them() {
        let r = rep("local f = love.filesystem.openFile('a', 'r')\n-- love.system.getMemorySize()\nlove.event.restart()\nlocal m = love.system.getMemorySize()\nlove.sensor.hasSensor('gyroscope')\nlove.markDeprecated(1)");
        let got: Vec<(&str, u32)> = r.uses.iter().map(|u| (u.api.as_str(), u.line)).collect();
        assert_eq!(got, [("love.filesystem.openFile", 1), ("love.event.restart", 3), ("love.system.getMemorySize", 4), ("love.sensor.hasSensor", 5), ("love.markDeprecated", 6)]);
        assert_eq!(r.blockers(&[]).len(), 1);
        assert_eq!(r.blockers(&[]) [0].api, "love.system.getMemorySize");
        assert_eq!(r.reduced().len(), 2);
        assert!(r.blockers(&["love.system.getMemorySize".to_string()]).is_empty(), "an accepted API is not a blocker");
    }

    #[test]
    fn plain_11_code_has_nothing_to_do() {
        let r = rep("love.filesystem.newFile('a','r') love.event.quit('restart') love.graphics.draw(x)");
        assert!(r.uses.is_empty());
        assert!(polyfill_lua(&r).lines().count() <= 2, "nothing is defined for a project that uses no 12.0 API");
    }

    #[test]
    fn only_what_is_used_is_defined() {
        let p = polyfill_lua(&rep("love.event.restart()"));
        assert!(p.contains("love.event.restart") && !p.contains("openFile") && !p.contains("sensor"));
        let q = polyfill_lua(&rep("love.sensor.isEnabled('gyroscope')"));
        assert!(q.contains("love.sensor") && !q.contains("restart"));
        // Unsupported APIs are never defined: a feature test must keep answering truthfully.
        let u = polyfill_lua(&rep("if love.system.getMemorySize then x = love.system.getMemorySize() end"));
        assert!(!u.contains("getMemorySize"));
    }

    #[test]
    fn only_12_projects_are_offered_the_variant() {
        assert!(offered(LoveVersion::parse("12.0").unwrap()));
        assert!(!offered(LoveVersion::parse("11.5").unwrap()));
    }

    #[test]
    fn the_polyfill_compiles() {
        let Some(lj) = ["/opt/homebrew/bin/luajit", "/usr/local/bin/luajit", "/usr/bin/luajit"].iter().find(|p| Path::new(p).exists()) else { return };
        let all = rep("love.filesystem.openFile() love.event.restart() love.markDeprecated() love.sensor.hasSensor()");
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("c.lua");
        std::fs::write(&f, polyfill_lua(&all)).unwrap();
        let o = std::process::Command::new(lj).arg("-bl").arg(&f).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
}
