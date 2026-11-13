//! Runs the generated shims under LuaJIT against a mocked `love`, simulating
//! frames and input, so their behaviour is tested without a display. (The
//! real-LÖVE run is `tests/e2e_love.rs`.) Skipped when luajit is absent.

use joyride_core::{export, overlay, scan};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn luajit() -> Option<PathBuf> {
    ["/opt/homebrew/bin/luajit", "/usr/local/bin/luajit", "/usr/bin/luajit"].iter().map(PathBuf::from).find(|p| p.exists())
}

/// A mock `love` for a 1280x720 screen, then the staged game, then `checks`.
fn harness(stage: &Path, checks: &str) -> String {
    format!(
        r#"
local log = {{}}
local function noop() end
local fakejs = {{ isGamepad = function() return true end, axes = {{ rightx = 0, righty = 0 }}, down = {{}},
  getGamepadAxis = function(self, a) return self.axes[a] or 0 end,
  isGamepadDown = function(self, b) return self.down[b] == true end }}
love = {{
  graphics = {{
    getDimensions = function() return 1280, 720 end, getWidth = function() return 1280 end, getHeight = function() return 720 end,
    newCanvas = function(w, h) return {{ w = w, h = h, setFilter = noop }} end,
    setCanvas = function(c) log[#log + 1] = "setCanvas:" .. tostring(c and c.w or "screen") end,
    clear = noop, setColor = noop, rectangle = noop, circle = noop, polygon = noop, print = noop, setLineWidth = noop,
    draw = function(c, x, y, r, sx) log[#log + 1] = string.format("blit:%d,%d,%g", x, y, sx) end,
  }},
  mouse = {{ getPosition = function() return 640, 360 end, isDown = function() return false end, setVisible = noop }},
  keyboard = {{ isDown = function() return false end }},
  window = {{ getSafeArea = function() return 0, 0, 1280, 720 end }},
  joystick = {{ getJoysticks = function() return {{ fakejs }} end }},
  event = {{ quit = function() log[#log + 1] = "quit" end }},
  filesystem = {{ write = function() return true end }},
  thread = {{ newThread = function() return {{}} end }},
}}
package.path = "{stage}/?.lua;" .. package.path
GAME = {{}}
dofile("{stage}/main.lua")
local jr = require("_jr_shim")
local function has(prefix) for _, l in ipairs(log) do if l:sub(1, #prefix) == prefix then return true end end return false end
{checks}
print("OK")
"#,
        stage = stage.display()
    )
}

const GAME: &str = r#"
function love.draw() love.graphics.rectangle("fill", 0, 0, 10, 10) end
function love.update(dt) GAME.dt = dt end
function love.keypressed(k) GAME.key = k end
function love.keyreleased(k) GAME.released = k end
function love.mousepressed(x, y, b) GAME.press = { x, y, b } end
function love.mousemoved(x, y, dx, dy) GAME.moved = { x, y, dx, dy } end
function love.touchpressed(id, x, y) GAME.touch = { id, x, y } end
function love.gamepadpressed(j, b) GAME.pad = b end
"#;

fn build(target: scan::Target, game: &str, conf: &str) -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("game");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.lua"), game).unwrap();
    std::fs::write(root.join("conf.lua"), conf).unwrap();
    let s = scan::scan_project(&root);
    overlay::write(&root, &overlay::generate(target, &s)).unwrap();
    let stage = d.path().join("stage");
    overlay::stage(&root, target, &stage, &BTreeMap::new()).unwrap();
    export::love_archive(&stage, &d.path().join("g.love")).unwrap(); // also proves it zips
    (d, stage)
}

fn run(lj: &Path, stage: &Path, checks: &str) {
    let script = stage.parent().unwrap().join("harness.lua");
    std::fs::write(&script, harness(stage, checks)).unwrap();
    let out = Command::new(lj).arg(&script).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success() && text.contains("OK"), "luajit failed:\n{text}");
}

const CONF: &str = "function love.conf(t) t.window.width = 640 t.window.height = 480 end";

#[test]
fn portmaster_scales_letterboxes_and_drives_a_virtual_mouse() {
    let Some(lj) = luajit() else { return };
    let (_d, stage) = build(scan::Target::PortMaster, GAME, CONF);
    run(
        &lj,
        &stage,
        r#"
assert(love.graphics.getWidth() == 640 and love.graphics.getHeight() == 480, "design size")
local w, h = love.graphics.getDimensions(); assert(w == 640 and h == 480)
love.draw()
-- 1280x720 screen, 640x480 design, integer scale: 1x, centred at 320,120.
assert(has("setCanvas:640") and has("blit:320,120,1") and has("setCanvas:screen"), table.concat(log, " "))
-- The virtual cursor starts at the middle of the design screen, whatever the real mouse says.
local mx, my = love.mouse.getPosition(); assert(mx == 320 and my == 240, mx .. "," .. my)
-- Right stick right for 0.1 s moves it by 640*0.6*0.1 = 38.4 and tells the game.
fakejs.axes.rightx = 1
love.update(0.1)
local x = love.mouse.getPosition(); assert(math.abs(x - 358.4) < 0.001, x)
assert(GAME.dt == 0.1 and GAME.moved and math.abs(GAME.moved[1] - 358.4) < 0.001 and math.abs(GAME.moved[3] - 38.4) < 0.001)
-- A clicks the left button at the cursor; the game still gets its own gamepad event.
love.gamepadpressed(fakejs, "a")
assert(GAME.press and GAME.press[3] == 1 and math.abs(GAME.press[1] - 358.4) < 0.001 and GAME.pad == "a")
assert(love.mouse.isDown(1) == true)
love.gamepadreleased(fakejs, "a"); assert(love.mouse.isDown(1) == false)
-- A real mouse (touch screens, USB) is ignored in virtual mode.
love.mousepressed(10, 10, 1, false, 1); assert(GAME.press[1] ~= 10)
-- Select + Start quits.
fakejs.down.back = true
love.gamepadpressed(fakejs, "start")
assert(has("quit"))
-- The game cannot hide the cursor we drew.
love.mouse.setVisible(false)
"#,
    );
}

#[test]
fn mobile_scales_and_maps_touch_and_the_pad_presses_keys() {
    let Some(lj) = luajit() else { return };
    // The game reads the keyboard, not touch, so the shim adds a pad (keys: space).
    let game = "function love.draw() end\nfunction love.keypressed(k) GAME.key = k end\nfunction love.keyreleased(k) GAME.released = k end\nfunction love.update() end\nfunction love.mousepressed(x,y,b) GAME.press = {x,y,b} end\nfunction love.keypressed(k) if k == 'space' then GAME.key = k end end";
    let (_d, stage) = build(scan::Target::Mobile, game, CONF);
    run(
        &lj,
        &stage,
        r#"
love.draw()
-- Not integer: 1.5x, centred horizontally at 160,0.
assert(has("blit:160,0,1.5"), table.concat(log, " "))
-- Real mouse at the screen centre is the design centre.
local mx, my = love.mouse.getPosition(); assert(mx == 320 and my == 240, mx .. "," .. my)
-- A click maps back into design space.
love.mousepressed(310, 150, 1, false, 1)
assert(math.abs(GAME.press[1] - 100) < 0.001 and math.abs(GAME.press[2] - 100) < 0.001, GAME.press[1] .. "," .. GAME.press[2])
-- The A button is at (1153.3, 545.8); touching it presses the game's key, and holds it down.
love.touchpressed(1, 1153.3 / 1280, 545.8 / 720, 0, 0, 1)
assert(GAME.key == "space", tostring(GAME.key))
assert(love.keyboard.isDown("space") == true)
love.touchreleased(1, 1153.3 / 1280, 545.8 / 720)
assert(GAME.released == "space" and love.keyboard.isDown("space") == false)
-- Touches elsewhere go to the game's own handler (none here), not the pad.
GAME.key = nil
love.touchpressed(2, 0.5, 0.1, 0, 0, 1)
assert(GAME.key == nil)
"#,
    );
}

#[test]
fn web_stubs_what_a_browser_cannot_do() {
    let Some(lj) = luajit() else { return };
    let game = "local ok = pcall(require, 'ffi')\nGAME.ffi_ok = ok\nfunction love.draw() end";
    let (_d, stage) = build(scan::Target::Web, game, CONF);
    run(
        &lj,
        &stage,
        r#"
assert(GAME.ffi_ok == false, "require('ffi') should fail softly")
assert(io.popen("ls") == nil)
assert(os.execute("ls") == nil)
assert(pcall(love.thread.newThread, "x") == false)
assert(jr.dirty == false)
love.filesystem.write("save.dat", "x")
assert(jr.dirty == true)
"#,
    );
}
