-- Plain-LuaJIT tests of the parts that need no LÖVE: platform detection, judging, the input record, timers, tones, the report and the
-- text grid helpers. From the project folder: `luajit tests/unit.lua`. Exits 1 on a failure.
package.path = "./?.lua;" .. package.path

local model = require("lib.model")
local platform = require("lib.platform")
local screen = require("ui.screen")

local failed, n = 0, 0
local function eq(got, want, what)
  n = n + 1
  if got ~= want then failed = failed + 1; print(("FAIL %s: got %s, wanted %s"):format(what, tostring(got), tostring(want))) end
end
local function ok(cond, what) eq(cond and true or false, true, what) end

-- ---- platform detection --------------------------------------------------------------------------------------------------------------
local function env(over)
  local e = { os = "Linux", getenv = function() return nil end, jit = { version = "LuaJIT 2.1", os = "Linux", arch = "x64" } }
  for k, v in pairs(over) do e[k] = v end
  return e
end
local function vars(t) return function(k) return t[k] end end
local function id(over) local d = platform.detect(env(over)); return d.id, d.certainty end

eq(id({ os = "OS X" }), "macos", "macOS")
eq(id({ os = "Windows" }), "windows", "Windows")
eq(id({}), "linux", "Linux")
eq(id({ os = "Android" }), "android", "Android")
eq(id({ os = "Web", jit = false }), "web", "Web by OS name")
eq(id({ os = "iOS", desktop = { w = 390, h = 844 } }), "ios", "iPhone")
eq(id({ os = "iOS", desktop = { w = 820, h = 1180 } }), "ipados", "iPad")
eq(id({ os = "iOS" }), "ios", "iOS with no screen size")
do
  local i, c = id({ getenv = vars({ CFW_NAME = "muOS", DEVICE_NAME = "RG35XX" }) })
  eq(i, "portmaster", "PortMaster from two variables"); eq(c, "certain", "...certain")
  i, c = id({ getenv = vars({ CFW_NAME = "ArkOS" }) })
  eq(i, "portmaster", "PortMaster from one variable"); eq(c, "likely", "...likely")
  i, c = id({ source = "/roms/ports/my-game/gamedata/game.love", getenv = vars({ GPTOKEYB = "/x/gptokeyb2" }) })
  eq(i, "portmaster", "PortMaster from the game folder and a variable"); eq(c, "certain", "...certain")
  i = id({ source = "/home/me/games/thing" })
  eq(i, "linux", "a plain Linux folder")
  i = id({ os = "OS X", getenv = vars({ CFW_NAME = "muOS", DEVICE_NAME = "x" }) })
  eq(i, "macos", "PortMaster variables on a Mac mean nothing")
end
do
  local d = platform.detect(env({ os = "OS X", shim = { target = "portmaster" } }))
  eq(d.id, "portmaster", "the PortMaster build on a Mac is PortMaster"); eq(d.certainty, "simulated", "...simulated")
  d = platform.detect(env({ os = "Linux", shim = { target = "web" } }))
  eq(d.id, "web", "the web build on a desktop"); eq(d.certainty, "simulated", "...simulated")
  d = platform.detect(env({ os = "OS X", shim = { target = "mobile" }, window = { w = 390, h = 844 } }))
  eq(d.id, "ios", "the mobile build in a phone-sized window"); eq(d.certainty, "simulated", "...simulated")
  d = platform.detect(env({ os = "OS X", shim = { target = "mobile" }, window = { w = 1180, h = 820 } }))
  eq(d.id, "ipados", "the mobile build in an iPad-sized window")
  d = platform.detect(env({ os = "Web", jit = false, shim = { target = "web" } }))
  eq(d.certainty, "certain", "web in a browser is certain")
  d = platform.detect(env({ os = "Linux", getenv = vars({ CFW_NAME = "muOS", DEVICE_NAME = "x" }), shim = { target = "portmaster" } }))
  eq(d.certainty, "certain", "PortMaster on its device is certain even with the shim")
  d = platform.detect(env({ os = "Plan 9" }))
  eq(d.id, "linux", "an unknown OS falls back to Linux"); eq(d.certainty, "likely", "...and says it is a guess")
end
ok(#platform.detect(env({ os = "OS X" })).evidence >= 3, "evidence is listed")

-- Every platform Joyride targets has a profile with every expectation filled in.
local KEYS = { "keyboard", "mouse", "touch", "gamepad", "sensors", "haptics", "clipboard", "resizable", "fullscreen", "safearea", "realfs", "threads", "ffi", "popen", "video", "network", "quit" }
eq(#platform.PROFILES, 8, "eight platforms")
for _, p in ipairs(platform.PROFILES) do
  for _, k in ipairs(KEYS) do
    ok(p[k] == "need" or p[k] == "want" or p[k] == "none", p.id .. "." .. k .. " is need, want or none")
  end
  ok(p.note and #p.note > 20, p.id .. " has a note")
end
eq(platform.BY_ID.portmaster.gamepad, "need", "a handheld needs its pad")
eq(platform.BY_ID.ios.touch, "need", "a phone needs touch")
eq(platform.BY_ID.macos.touch, "none", "a Mac has no touch events")
eq(platform.BY_ID.web.threads, "none", "the web has no threads")

local d = platform.detect(env({ os = "OS X" }))
local pr, forced = platform.resolve(d, nil)
eq(pr.id, "macos", "resolve: automatic"); eq(forced, false, "...not forced")
pr, forced = platform.resolve(d, "web")
eq(pr.id, "web", "resolve: forced"); eq(forced, true, "...forced")
pr, forced = platform.resolve(d, "macos")
eq(forced, false, "choosing what was detected is not forcing")
local cur, seen = nil, {}
for _ = 1, 9 do cur = platform.cycle(cur, 1); seen[#seen + 1] = cur or "auto" end
eq(seen[1], "macos", "cycle starts at the first platform"); eq(seen[8], "web", "...reaches the last"); eq(seen[9], "auto", "...and comes back to automatic")
eq(platform.cycle(nil, -1), "web", "cycling back from automatic reaches the last")

-- ---- judging -------------------------------------------------------------------------------------------------------------------------
eq((model.judge("need", true)), "PASS", "need + present"); eq((model.judge("need", false)), "FAIL", "need + absent")
eq((model.judge("want", true)), "PASS", "want + present"); eq((model.judge("want", false)), "INFO", "want + absent")
eq((model.judge("none", true)), "INFO", "none + present"); eq((model.judge("none", false)), "PASS", "none + absent")
eq(model.judgeLoad("yes", true), "PASS", "expected load, loaded"); eq(model.judgeLoad("yes", false), "FAIL", "expected load, refused")
eq(model.judgeLoad("no", false), "PASS", "expected refusal, refused"); eq(model.judgeLoad("no", true), "INFO", "expected refusal, loaded")
eq(model.judgeLoad("maybe", true), "PASS", "maybe, loaded"); eq(model.judgeLoad("maybe", false), "INFO", "maybe, refused")

-- ---- the input record ------------------------------------------------------------------------------------------------------------------
local r = model.newInput()
r:key("a"); r:key("a"); r:key("b", true)
eq(r.keyCount, 2, "two different keys")
r:mouseButton(1, true, false); r:mouseButton(1, false, false); r:mouseButton(1, true, true)
eq(r.mouse.presses, 1, "a touch's mouse event is not a click")
r:touchDown("a", 1, 1, 0.4); r:touchDown("b", 2, 2, 0.9); r:touchUp("a")
eq(r.touch.max, 2, "two fingers at once"); eq(r.touch.pressureMax, 0.9, "pressure kept")
r:pad(1, { name = "P", buttons = 4, axes = 2, hats = 0 })
r:padPress(1, 1, 1.000); r:padRelease(1, 1, 1.100); r:padPress(1, 1, 1.110)
eq(r:chatterTotal(1), 1, "a press 10 ms after a release is chatter")
r:padPress(1, 2, 5)
eq(#r:missing(1), 2, "two of four buttons never pressed")
r:padAxis(1, 1, -1); r:padAxis(1, 1, 0.95)
eq(r.pads[1].axes[1].min, -1, "axis minimum"); eq(r.pads[1].axes[1].max, 0.95, "axis maximum")
r:sensor("accelerometer", 0, 0, 1); r:sensor("accelerometer", 0.5, 0, 1)
eq(r.sensors.accelerometer.max[1], 0.5, "sensor maximum"); eq(r.sensors.accelerometer.n, 2, "sensor readings")
for i = 1, 20 do r:note("e" .. i) end
eq(#r.log, model.LOG_LINES, "the event log is capped")
eq(model.zone(0.6), 1, "zone high"); eq(model.zone(-0.6), -1, "zone low"); eq(model.zone(0.2), 0, "zone centre")

-- ---- timers and the motion test ---------------------------------------------------------------------------------------------------------
local c = model.newChord(1)
eq(c:update(0.5, true), false, "chord not yet"); eq(c:update(0.6, true), true, "chord after a second")
c:update(0.1, false); eq(c.t, 0, "chord resets when let go")
local idle = model.newIdle(15, 10)
idle:update(4); eq(idle:warning(), false, "no warning at 4 s"); idle:update(2); eq(idle:warning(), true, "warning from 6 s")
idle:update(9); eq(idle:expired(), true, "expired at 15 s"); idle:touch(); eq(idle:expired(), false, "touch resets")
local f = model.newFrames()
eq(f:stats(), nil, "no stats before 30 frames")
for i = 1, 100 do f:add(i == 50 and 0.05 or 1 / 60) end
local st = f:stats()
eq(st.late, 1, "one late frame"); ok(st.fps > 55 and st.fps < 61, "fps near 60"); ok(st.worst > 0.049, "worst frame")
ok(model.framesLine(nil):find("not measured", 1, true), "framesLine with no data")

-- ---- tones ------------------------------------------------------------------------------------------------------------------------------
for _, t in ipairs(model.TONES) do
  local nframes = model.toneFrames(t)
  local l0, r0 = model.toneSample(t, 0)
  eq(l0, 0, t.id .. " starts silent (fade in)"); eq(r0, 0, t.id .. " right starts silent")
  local peak = 0
  for k = 0, nframes - 1, 7 do
    local l, rr = model.toneSample(t, k)
    peak = math.max(peak, math.abs(l), math.abs(rr))
  end
  ok(peak <= 0.5 + 1e-9, t.id .. " never clips")
end
do
  local left, right = model.TONES[1], model.TONES[2]
  local _, rr = model.toneSample(left, 5000)
  eq(rr, 0, "the left tone is silent on the right")
  local l2 = model.toneSample(right, 5000)
  eq(l2, 0, "the right tone is silent on the left")
end

-- ---- the report ---------------------------------------------------------------------------------------------------------------------------
local rep = model.newReport({ made = "now", how = "test", platform = "X" })
rep:section("a"); rep:line("INFO", "one", "1"); rep:check(true, "two", "2"); rep:check(false, "three", "3"); rep:line("SKIP", "four", "4")
local text = rep:text()
ok(text:find("== a ==", 1, true), "section header"); ok(text:find("PASS two: 2", 1, true), "PASS line"); ok(text:find("FAIL three: 3", 1, true), "FAIL line")
ok(text:find("# PASS 1  FAIL 1  INFO 1  SKIP 1", 1, true), "counts")
eq(rep.fails[1], "three", "failures are listed")
eq(model.reportName("web", 11, 5), "io-checker-report-web-11.5.txt", "report name")

-- ---- the grid ------------------------------------------------------------------------------------------------------------------------------
local scr = screen.new()
scr:text(0, 0, "héllo")
eq(scr:row(0):sub(1, 7), "héllo ", "UTF-8 text lands one character a cell")
scr:text(46, 1, "abcdef")
eq(scr:row(1):sub(-2), "ab", "text stops at the right edge")
eq(#screen.chars("日本語"), 3, "chars splits UTF-8")
eq(screen.fit("abcdefghij", 5), "abcd…", "fit cuts with an ellipsis"); eq(screen.fit("abc", 5), "abc", "fit leaves short text")
eq(screen.pad("ab", 4), "ab  ", "pad")
local w = screen.wrap("one two three four five six", 9)
eq(w[1], "one two", "wrap line 1"); eq(w[2], "three", "wrap line 2")
eq(#screen.wrap(string.rep("x", 25), 10), 3, "wrap breaks a long word")
eq(screen.nav(1, 5, 3, "up"), 5, "nav wraps up"); eq(screen.nav(5, 5, 3, "down"), 1, "nav wraps down")
eq(screen.nav(2, 10, 4, "pgdn"), 6, "nav page down"); eq(screen.nav(9, 10, 4, "pgdn"), 10, "nav page down stops at the end")
eq(screen.nav(3, 10, 4, "pgup"), 1, "nav page up stops at the start")
local bar = screen.axisBar(-1, 11)
eq(#screen.chars(bar), 13, "axis bar width"); eq(screen.chars(bar)[2], "█", "axis bar at the far left")
eq(screen.chars(screen.axisBar(1, 11))[12], "█", "axis bar at the far right")
scr:clear()
screen.list(scr, { x = 0, y = 0, w = 20, h = 5, title = "L", items = { { label = "one" }, { label = "two", value = "ok" }, { label = "three" }, { label = "four" } }, sel = 4, foot = "" })
ok(scr:dump():find("four", 1, true), "a list shows the page the selection is on")
ok(not scr:dump():find("one", 1, true), "...and not the first page")
local ascii = screen.new(); ascii.ascii = true; ascii:box(0, 0, 4, 3, nil)
eq(ascii:row(0):sub(1, 4), "+--+", "ASCII box when the font has no box characters")

print(("unit: %d checks, %d failed"):format(n, failed))
os.exit(failed > 0 and 1 or 0)
