//! Builds a small LÖVE 11.5 demo game as a love.js page, to try a web template
//! in a real browser:
//!
//!   cargo run --example web_demo -- <love.js src folder> <out folder> [--compat]
//!
//! The demo shows the Lua runtime (`_VERSION`, LuaJIT or not), a frame counter,
//! the last key pressed, and a counter saved with `love.filesystem` that goes up
//! each time the page is opened, which tests that saves persist.

use joyride_core::pipeline::{build_web, Build};
use std::path::PathBuf;

const MAIN: &str = r#"
local opened, last_key, t = 0, "(none)", 0
local save = "opened.txt"
function love.load()
  if love.filesystem.getInfo(save) then opened = (tonumber(love.filesystem.read(save)) or 0) end
  opened = opened + 1
  love.filesystem.write(save, tostring(opened))
  love.graphics.setFont(love.graphics.newFont(18))
end
function love.update(dt) t = t + dt end
function love.keypressed(k) last_key = k end
function love.draw()
  love.graphics.setColor(0.1, 0.6, 0.9)
  love.graphics.rectangle("fill", 20 + (t * 60) % 400, 20, 40, 40)
  love.graphics.setColor(1, 1, 1)
  local jit_info = (type(jit) == "table" and jit.version) or "no jit"
  love.graphics.print("Lua: " .. _VERSION .. "  (" .. jit_info .. ")", 20, 90)
  local major, minor = love.getVersion()
  love.graphics.print("LÖVE " .. major .. "." .. minor, 20, 120)
  love.graphics.print("Opened " .. opened .. " time(s)  [reload the page: it should go up]", 20, 150)
  love.graphics.print("Last key: " .. last_key .. "   time: " .. string.format("%.1f", t), 20, 180)
  love.graphics.print("FPS: " .. love.timer.getFPS(), 20, 210)
  local w, h = love.graphics.getDimensions()
  love.graphics.print("Screen: " .. w .. "x" .. h, 20, 240)
end
"#;

const CONF: &str = "function love.conf(t)\n  t.window.width = 640\n  t.window.height = 480\n  t.window.title = 'JoyrIDE web demo'\nend\n";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: web_demo <love.js src folder> <out folder> [--compat]");
        std::process::exit(2);
    }
    let (template, out) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    let compat = args.iter().any(|a| a == "--compat");
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("Web Demo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.lua"), MAIN).unwrap();
    std::fs::write(root.join("conf.lua"), CONF).unwrap();
    std::fs::write(root.join("love.toml"), "[project]\nname = 'Web Demo'\nbundle_id = 'dev.joyride.webdemo'\nversion = '1.0.0'\nlove_version = '11.5'\n").unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let page = build_web(&Build { root: &root, out_dir: &out, gate: None, compat: None }, &template, compat).unwrap();
    println!("{}", page.display());
}
