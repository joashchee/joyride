//! Lints that know the project's libraries: what a `require` of one means, what a call into one can and cannot do.
//!
//! They run beside the LÖVE lints ([`crate::lint`]) and are as conservative: a lint that fires is either something JoyrIDE measured about
//! a library at the commit it pins (the findings the Benchmark Libraries sample made, kept here as a short table) or a name so close to
//! one the library does define that it is nearly always a typo. Nothing is run: a library's source is read, never executed.
//!
//! * **version**: requiring a library that does not load on the project's LÖVE version.
//! * **require**: requiring a library by a name that cannot find it (g3d needs `libs.g3d.g3d`).
//! * **pitfall**: a call a library documents badly or gets wrong (HC's `raycast`, a type test hump.class does not have).
//! * **call**: `lib.neWorld(...)` where the library defines `newWorld` and no `neWorld`.

use crate::library::{Entry, Library};
use crate::lint::{Diagnostic, Severity};
use crate::lua::{lex, Tok, Token};
use crate::toolchain::LoveVersion;
use std::collections::BTreeSet;
use std::path::Path;

/// A library required by a name that does not find it, and the name that does.
const WRONG_REQUIRE: &[(&str, &[&str], &str, &str)] = &[
    ("g3d", &["g3d"], "libs.g3d.g3d", "g3d finds its shader from the name it was required by, so `require \"g3d\"` cannot find it"),
    ("neumetik-midi", &["neumetik_midi", "neumetik-midi"], "libs.neumetik-midi.bindings.love.neumetik_midi", "its Lua binding is under bindings/love"),
    ("serpent", &["serpent"], "libs.serpent.src.serpent", "its only file is src/serpent.lua"),
    ("cimgui-love", &["cimgui", "cimgui-love"], "libs.cimgui-love.src", "the Lua side of cimgui-love is its src folder"),
];

struct Pitfall {
    /// The curated library.
    entry: &'static str,
    /// Only when this exact module is required (hump has several), or any of the library's.
    module: Option<&'static str>,
    call: &'static str,
    /// Flag only a call with more than this many arguments.
    more_than: Option<usize>,
    message: &'static str,
}

const PITFALLS: &[Pitfall] = &[
    Pitfall { entry: "HC", module: None, call: "raycast", more_than: None, message: "HC's raycast finds shapes only for rays heading left or up (measured at the pinned commit); for other directions test candidates with shape:intersectionsWithRay" },
    Pitfall { entry: "hump", module: Some("hump.class"), call: "isInstanceOf", more_than: None, message: "hump.class has no type test; keep a field on the class, or use middleclass or classic" },
    Pitfall { entry: "hump", module: Some("hump.class"), call: "is", more_than: None, message: "hump.class has no type test; keep a field on the class, or use middleclass or classic" },
    Pitfall { entry: "batteries", module: None, call: "set_state", more_than: Some(1), message: "state_machine:set_state takes the state's name only (a second argument is a reset flag); it cannot pass arguments to enter" },
];

fn diag(line: u32, severity: Severity, rule: &'static str, message: String) -> Diagnostic {
    Diagnostic { line, severity, rule, message }
}

fn top(m: &str) -> &str {
    m.split('.').next().unwrap_or(m)
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// The curated entry a required module name stands for.
fn entry_for<'a>(lib: &'a Library, module: &str) -> Option<&'a Entry> {
    let m = module.strip_prefix("libs.").unwrap_or(module);
    lib.entries.iter().find(|e| {
        let et = top(&e.module);
        // `libs.g3d.g3d`: the folder is the library's name
        e.module == m || m == e.name || norm(top(m)) == norm(&e.name) || (et.len() > 2 && !["src", "lib", "init"].contains(&et) && norm(top(m)) == norm(et))
    })
}

struct Req {
    module: String,
    line: u32,
    /// The local it is assigned to.
    var: Option<String>,
}

fn requires(t: &[Token]) -> Vec<Req> {
    let mut out = Vec::new();
    for (i, tok) in t.iter().enumerate() {
        if !tok.is_name("require") || (i > 0 && (t[i - 1].is_punct(".") || t[i - 1].is_punct(":"))) {
            continue;
        }
        let mut j = i + 1;
        if t.get(j).is_some_and(|x| x.is_punct("(")) {
            j += 1;
        }
        let Some(Tok::Str(m)) = t.get(j).map(|x| &x.tok) else { continue };
        let var = (i >= 2 && t[i - 1].is_punct("=")).then(|| match &t[i - 2].tok { Tok::Name(n) => Some(n.clone()), _ => None }).flatten();
        out.push(Req { module: m.clone(), line: tok.line, var });
    }
    out
}

/// The number of arguments of the call whose `(` is at `open` (top level commas + 1, 0 if empty), or None if it does not close.
fn arg_count(t: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let (mut commas, mut any) = (0usize, false);
    for tok in &t[open..] {
        match &tok.tok {
            Tok::Punct(p) if p == "(" || p == "{" || p == "[" => depth += 1,
            Tok::Punct(p) if p == ")" || p == "}" || p == "]" => {
                depth -= 1;
                if depth == 0 {
                    return Some(if any { commas + 1 } else { 0 });
                }
            }
            Tok::Punct(p) if p == "," && depth == 1 => commas += 1,
            _ if depth >= 1 => any = true,
            _ => {}
        }
    }
    None
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Every name the library's module file mentions after a dot or colon, or before an equals sign: a generous set of what it could define.
fn names_in(source: &str) -> BTreeSet<String> {
    let t = lex(source);
    let mut s = BTreeSet::new();
    for (i, tok) in t.iter().enumerate() {
        if let Tok::Name(n) = &tok.tok {
            let after_dot = i > 0 && (t[i - 1].is_punct(".") || t[i - 1].is_punct(":"));
            let before_eq = t.get(i + 1).is_some_and(|x| x.is_punct("="));
            let before_fn = i > 0 && t[i - 1].is_name("function");
            if after_dot || before_eq || before_fn {
                s.insert(n.clone());
            }
        }
    }
    s
}

/// The source of the module `m` of library `e`, from the project's `libs/`.
fn module_source(root: &Path, e: &Entry, m: &str) -> Option<String> {
    let m = m.strip_prefix("libs.").unwrap_or(m);
    let rel = m.replace('.', "/");
    let n = &e.name;
    for c in [format!("libs/{rel}.lua"), format!("libs/{rel}/init.lua"), format!("libs/{n}/{rel}.lua"), format!("libs/{n}/{rel}/init.lua")] {
        let p = root.join(&c);
        if let Ok(md) = std::fs::metadata(&p) {
            if md.is_file() && md.len() < 512 * 1024 {
                return std::fs::read_to_string(p).ok();
            }
        }
    }
    None
}

/// The library lints for one Lua file.
pub fn lint(root: &Path, source: &str, lib: &Library, version: LoveVersion) -> Vec<Diagnostic> {
    let t = lex(source);
    let reqs = requires(&t);
    let mut out = Vec::new();
    let mut used: Vec<(&Entry, &Req)> = Vec::new();

    for r in &reqs {
        // a wrong name for a library that has a right one
        if let Some((name, _, right, why)) = WRONG_REQUIRE.iter().find(|(_, wrong, _, _)| wrong.contains(&r.module.as_str())) {
            if lib.get(name).is_some() {
                out.push(diag(r.line, Severity::Error, "library-require", format!("Require {name} as `{right}`: {why}")));
                continue;
            }
        }
        if r.module == "love-loader" && root.join("libs/love-loader").is_dir() && !root.join("love-loader.lua").is_file() {
            out.push(diag(r.line, Severity::Warning, "library-require", "love-loader reads its own source as `love-loader.lua` from the game's root folder: copy that one file there, it does not work from libs/".into()));
        }
        if let Some(e) = entry_for(lib, &r.module) {
            if e.known_broken_on(version) {
                out.push(diag(r.line, Severity::Error, "library-version", format!("{} does not load on LÖVE {version}: {}", e.name, e.badge(version).label().trim_start_matches("Does not load: "))));
            }
            used.push((e, r));
        }
    }

    // known pitfalls: a call to `.name(` or `:name(` in a file that requires the library
    for p in PITFALLS {
        if !used.iter().any(|(e, r)| e.name == p.entry && p.module.is_none_or(|m| r.module.strip_prefix("libs.").unwrap_or(&r.module) == m)) {
            continue;
        }
        for i in 1..t.len().saturating_sub(1) {
            if t[i].is_name(p.call) && (t[i - 1].is_punct(".") || t[i - 1].is_punct(":")) && t[i + 1].is_punct("(") {
                if let Some(max) = p.more_than {
                    if arg_count(&t, i + 1).is_none_or(|n| n <= max) {
                        continue;
                    }
                }
                out.push(diag(t[i].line, Severity::Warning, "library-pitfall", p.message.to_string()));
            }
        }
    }

    // a call on a library's module that is a near miss of one it defines
    for (e, r) in &used {
        let Some(var) = &r.var else { continue };
        let Some(src) = module_source(root, e, &r.module) else { continue };
        let names = names_in(&src);
        if names.len() < 3 {
            continue;
        }
        for i in 0..t.len().saturating_sub(3) {
            let Tok::Name(v) = &t[i].tok else { continue };
            if v != var || !(t[i + 1].is_punct(".") || t[i + 1].is_punct(":")) || !t[i + 3].is_punct("(") {
                continue;
            }
            let Tok::Name(call) = &t[i + 2].tok else { continue };
            if i > 0 && (t[i - 1].is_punct(".") || t[i - 1].is_punct(":")) || names.contains(call) {
                continue;
            }
            if let Some(best) = names.iter().filter(|n| n.len() > 2 && edit_distance(n, call) <= 2 && n.len().abs_diff(call.len()) <= 2).min_by_key(|n| edit_distance(n, call)) {
                out.push(diag(t[i + 2].line, Severity::Warning, "library-call", format!("{} has no `{call}`: did you mean `{best}`?", e.name)));
            }
        }
    }
    out.sort_by_key(|d| d.line);
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/benchmark-libraries")
    }

    fn v(s: &str) -> LoveVersion {
        LoveVersion::parse(s).unwrap()
    }

    fn rules(src: &str, ver: &str) -> Vec<(u32, &'static str, String)> {
        lint(&sample(), src, &Library::bundled(), v(ver)).into_iter().map(|d| (d.line, d.rule, d.message)).collect()
    }

    #[test]
    fn a_library_by_a_name_that_cannot_find_it_is_an_error_with_the_name_that_does() {
        let r = rules("local g = require(\"g3d\")\nlocal ok = require \"libs.g3d.g3d\"\nlocal n = require 'neumetik_midi'\n", "11.5");
        assert_eq!(r.iter().map(|x| (x.0, x.1)).collect::<Vec<_>>(), [(1, "library-require"), (3, "library-require")], "{r:?}");
        assert!(r[0].2.contains("libs.g3d.g3d") && r[1].2.contains("bindings.love.neumetik_midi"));
    }

    #[test]
    fn a_library_that_does_not_load_on_the_projects_love_is_an_error_there_only() {
        let src = "local lurker = require 'lurker'\n";
        assert!(rules(src, "11.5").is_empty());
        let r = rules(src, "12.0");
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].0, r[0].1), (1, "library-version"));
        assert!(r[0].2.contains("lurker does not load on LÖVE 12.0") && r[0].2.contains("isDirectory"), "{}", r[0].2);
    }

    #[test]
    fn measured_pitfalls_fire_only_where_the_library_is_required() {
        let with = "local HC = require 'HC'\nlocal w = HC.new()\nw:raycast(0, 0, 1, 0, 100)\n";
        let r = rules(with, "11.5");
        assert_eq!(r.iter().map(|x| (x.0, x.1)).collect::<Vec<_>>(), [(3, "library-pitfall")], "{r:?}");
        assert!(r[0].2.contains("left or up"));
        assert!(rules("local w = {}\nw:raycast(0, 0, 1, 0, 100)\n", "11.5").is_empty(), "another raycast is not HC's");
        // hump has several modules: the type test is only missing from hump.class
        assert_eq!(rules("local C = require 'hump.class'\nlocal a = C{}\nif a:isInstanceOf(C) then end\n", "11.5").len(), 1);
        assert!(rules("local V = require 'hump.vector'\nlocal a = V(1,2)\nif a:isInstanceOf(V) then end\n", "11.5").is_empty());
        // set_state's extra arguments, counted properly
        let b = "local b = require 'batteries'\nlocal sm = b.state_machine({}, 'a')\nsm:set_state('b')\nsm:set_state('b', f(1, 2))\nsm:set_state({1, 2})\n";
        let r = rules(b, "11.5");
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), [4], "{r:?}");
    }

    #[test]
    fn a_near_miss_of_a_name_the_library_defines_is_a_warning_with_the_right_name() {
        let r = rules("local bump = require 'bump'\nlocal w = bump.neWorld(32)\nlocal ok = bump.newWorld(32)\nlocal x = w:add(1, 0, 0, 1, 1)\n", "11.5");
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!((r[0].0, r[0].1), (2, "library-call"));
        assert!(r[0].2.contains("`neWorld`") && r[0].2.contains("`newWorld`"), "{}", r[0].2);
        // Names far from anything are left to the language server: no guessing.
        assert!(rules("local bump = require 'bump'\nbump.completelyUnrelatedThing(1)\n", "11.5").is_empty());
        // Not a library variable: not looked at.
        assert!(rules("local bump = {}\nbump.neWorld(1)\n", "11.5").is_empty());
    }

    #[test]
    fn real_calls_across_every_curated_library_are_never_flagged() {
        // The calls the Benchmark Libraries sample makes on each library are correct: none may be called a typo.
        let mut checked = 0;
        for f in ["bench/groups/classes.lua", "bench/groups/tweens.lua", "bench/groups/collision.lua", "bench/groups/collections.lua", "bench/groups/serialise.lua", "bench/groups/events.lua", "bench/groups/vectors.lua", "bench/groups/maths.lua"] {
            let src = std::fs::read_to_string(sample().join(f)).unwrap();
            let r: Vec<_> = lint(&sample(), &src, &Library::bundled(), v("11.5")).into_iter().filter(|d| d.rule == "library-call").collect();
            assert!(r.is_empty(), "{f}: {r:?}");
            checked += 1;
        }
        assert_eq!(checked, 8);
    }

    #[test]
    fn correct_calls_into_many_libraries_are_never_called_typos() {
        let src = r#"
local lume = require "lume"
local json = require "json"
local binser = require "binser"
local flux = require "flux"
local tween = require "tween"
local bump = require "bump"
local anim8 = require "anim8"
local baton = require "baton"
local semver = require "semver"
local inspect = require "inspect"
local lust = require "lust"
local log = require "log"
print(lume.round(1.5), lume.clamp(2, 0, 1), lume.lerp(0, 1, 0.5), lume.serialize({}), lume.deserialize("{}"), lume.map({}, print), lume.split("a,b", ","), lume.uuid())
print(json.encode({}), json.decode("{}"), binser.serialize(1), binser.deserialize("x"))
flux.to({}, 1, {}); flux.update(0.1); flux.group()
tween.new(1, {}, {}); bump.newWorld(); anim8.newGrid(1, 1, 2, 2); baton.new({}); semver("1.0.0"); inspect({}); lust.describe("x", print); log.info("x")
"#;
        let r: Vec<_> = lint(&sample(), src, &Library::bundled(), v("11.5")).into_iter().filter(|d| d.rule == "library-call").collect();
        assert!(r.is_empty(), "{r:?}");
        // and a typo in each of a few is found
        let typos = "local lume = require 'lume'\nlume.rounnd(1)\nlocal flux = require 'flux'\nflux.updat(1)\nlocal json = require 'json'\njson.encod({})\n";
        assert_eq!(lint(&sample(), typos, &Library::bundled(), v("11.5")).len(), 3);
    }

    #[test]
    fn love_loader_from_libs_is_warned_about_until_its_file_is_at_the_root() {
        let r = rules("local loader = require 'love-loader'\n", "11.5");
        assert!(r.iter().any(|x| x.1 == "library-require" && x.2.contains("root folder")), "{r:?}");
    }
}
