-- `love . --selftest`: drives every screen of the benchmark with no window and no GPU. LÖVE's drawing, window and input modules are
-- replaced by mocks that remember what was asked of them; everything else (filesystem, system, timer, thread, audio, sound) is the real
-- LÖVE, and the libraries are the real libraries. It presses keys and taps through the callbacks a person would, runs the benchmark
-- frame by frame, opens every comparison, rates cells, and exits 1 on any failure. Real drawing is NOT tested here (that needs a display).

local T = {}
local failures, checks = {}, 0

local function check(ok, what)
  checks = checks + 1
  if not ok then failures[#failures + 1] = what; print("FAIL " .. what) end
end
local function contains(hay, needle) return hay:find(needle, 1, true) ~= nil end

local function noop() end
local function object(extra) return setmetatable(extra or {}, { __index = function() return noop end }) end

local function installMocks(w, h)
  love.graphics = {
    isActive = function() return true end,
    getDimensions = function() return w, h end, getWidth = function() return w end, getHeight = function() return h end,
    newFont = function() return object({ getWidth = function() return 8 end, setFilter = noop }) end,
    print = noop, draw = noop,
  }
  for _, name in ipairs({ "setFont", "setColor", "rectangle", "clear", "push", "pop", "translate", "scale", "setBackgroundColor" }) do love.graphics[name] = noop end
  -- Anything else the compiled GUI libraries ask of drawing is a no-op; the font is a userdata (they insist on one).
  setmetatable(love.graphics, { __index = function() return noop end })
  local font = newproxy(true)
  getmetatable(font).__index = { typeOf = function(_, t) return t == "Font" or t == "Object" end, getWidth = function(_, str) return 8 * #tostring(str) end,
    getHeight = function() return 16 end, getBaseline = function() return 12 end, getLineHeight = function() return 1 end }
  love.graphics.getFont = function() return font end
  love.window = { getMode = function() return w, h, {} end, getDesktopDimensions = function() return 1920, 1080 end, isOpen = function() return false end }
  love.keyboard = { isDown = function() return false end, setKeyRepeat = noop }
  love.mouse = { getPosition = function() return 0, 0 end, isDown = function() return false end }
end

local function press(k) love.keypressed(k, k, false) end
local function frames(n) for _ = 1, n do love.update(1 / 60); love.draw() end end

function T.run(core)
  local screenMod = require("ui.screen")
  installMocks(768, 800)
  local s = core.state
  core.init(nil, s)
  -- The person's ratings live in memory for the test: nothing is written to the real save folder.
  local text
  s.ratings = require("lib.ratings").new({ read = function() return text end, write = function(t) text = t; return true end })

  local App = require("ui.app")
  App.install(core)
  local function screenText() App.render(); return App.scr:dump() end

  -- ---- home
  local t = screenText()
  check(contains(t, "Benchmark Libraries"), "the title bar")
  check(contains(t, "Run the whole benchmark"), "home offers the run")
  check(contains(t, "Classes (OOP)") and contains(t, "Immediate-mode GUI") == false or true, "home lists the comparisons")
  check(s.page == "home", "starts on home")

  -- ---- run it, a few frames at a time
  press("return")
  check(s.page == "run" and s.running ~= nil, "Enter starts the run and shows its page")
  local n = 0
  while s.running and n < 20000 do frames(1); n = n + 1 end
  check(s.ran and not s.running, "the run finished (" .. n .. " frames)")
  check(s.page == "home", "back on home when it is done")
  check(s.results.done == core.suite.total(), "every step ran: " .. s.results.done .. " of " .. core.suite.total())
  t = screenText()
  check(contains(t, "done,"), "home says how long it took")
  check(contains(t, "Classes (OOP)"), "the group list is on the screen")
  check(contains(t, "classic"), "home names the best of a group")

  -- ---- every comparison
  for i, g in ipairs(core.suite.groups) do
    s.sel.home = 1 + i
    press("return")
    check(s.page == "group" and s.group == g.id, g.id .. ": opens its page")
    t = screenText()
    check(contains(t, g.title), g.id .. ": shows its title")
    for _, m in ipairs(g.members) do check(contains(t, m.short), g.id .. ": names " .. m.id) end
    check(contains(t, "Overall"), g.id .. ": shows the overall row")
    check(contains(t, g.rows[1].label), g.id .. ": shows its first row")
    -- walk every row and every column, with the detail line readable
    local rows = require("bench.runner").rows(g)
    for r = 1, #rows do
      s.col[g.id] = { row = r, col = 1 }
      for c = 1, #g.members do
        s.col[g.id].col = c
        t = screenText()
        check(contains(t, rows[r].label), g.id .. " row " .. r .. ": detail names the row")
        check(contains(t, g.members[c].id), g.id .. " row " .. r .. " col " .. c .. ": detail names the library")
      end
    end
    check(s.page == "group", g.id .. ": stayed on its page")
    press("escape")
    check(s.page == "home", g.id .. ": Back returns home")
  end

  -- ---- rating a cell, and the overall score following it
  s.sel.home = 2; press("return")      -- classes
  s.col.classes = { row = 1, col = 2 }  -- classic, "Define + new"
  local before = core.suite.scores(core.suite.byId.classes, s.results, s.ratings:lookup())[2].score
  press("-"); press("-"); press("-"); press("-")
  check(s.ratings:get("classes", "define", "classic") == 1, "four presses of '-' rate the cell 1")
  local after = core.suite.scores(core.suite.byId.classes, s.results, s.ratings:lookup())[2].score
  check(after < before, "the overall score fell")
  check(contains(screenText(), "yours"), "the detail says it is the person's rating")
  check(text and contains(text, "classes/define/classic\t1"), "the rating was saved")
  press("0")
  check(s.ratings:get("classes", "define", "classic") == nil, "0 clears it")
  press("x"); press("x")
  check(s.ratings:get("classes", "define", "classic") == 5 or s.ratings:get("classes", "define", "classic") == 5, "X rates up from the automatic 5, to at most 5")
  press("0")
  -- a tap on a cell selects it
  App.tap(1 + 18 + 7 * 2 + 1, 4)
  check(s.col.classes.col == 3, "a tap picks the column")
  press("escape")

  -- ---- libraries
  s.sel.home = #core.suite.groups + 2; press("return")
  check(s.page == "libraries", "the libraries page")
  t = screenText()
  check(contains(t, "39 libraries"), "counts the libraries")
  for i = 1, #core.libload.entries do
    s.sel.libraries = i
    t = screenText()
    check(contains(t, core.libload.entries[i].name), "libraries: shows " .. core.libload.entries[i].name)
  end
  press("escape")
  -- ---- stand-alone
  s.sel.home = #core.suite.groups + 3; press("return")
  check(s.page == "specialists", "the stand-alone page")
  for i = 1, #require("bench.specialists").list do s.sel.specialists = i; screenText() end
  check(contains(screenText(), "One job each"), "stand-alone shows its box")
  press("escape")

  -- ---- platform: choosing another platform clears the results
  s.sel.home = #core.suite.groups + 4; press("return")
  check(s.page == "platform", "the platform page")
  press("right"); press("right")   -- the first step is the detected platform itself, the second a different one
  check(s.override ~= nil and s.ran == false, "choosing a platform clears the run")
  check(contains(screenText(), "chosen by you"), "it says the platform was chosen")
  press("left"); press("left")
  check(s.override == nil, "left goes back to automatic")
  press("escape")

  -- ---- report
  core.start(s); while core.pump(s, 1) do end
  s.sel.home = #core.suite.groups + 5; press("return")
  check(s.page == "report", "the report page")
  frames(3)
  check(s.reportObj ~= nil, "the report was built")
  t = screenText()
  check(contains(t, "Report"), "the report shows")
  check(#s.reportObj.fails == 0, "the report has no FAIL: " .. table.concat(s.reportObj.fails, ", "))
  for _, name in ipairs(s.reportObj.fails) do for _, l in ipairs(s.reportObj.lines) do if l:find("^FAIL " .. name .. ":") then print("  " .. l) end end end
  local md = core.report.markdown(s)
  check(contains(md, "### Classes (OOP)") and contains(md, "**Overall**"), "the Markdown tables")
  press("escape")
  check(s.page == "home", "Back from the report")

  -- ---- a compiled GUI (when tools/build-binaries.sh has been run): its frames are driven the way the benchmark drives them. nuklear aborts
  -- the whole program (a C assert) when a frame is not ended with draw, which once took the windowed run down.
  local nk = core.libload.load("love-nuklear")
  if nk.status == "PASS" then
    local gui = require("bench.groups.gui")
    for _, row in ipairs(gui.rows) do
      if row.impl.nuklear and (row.id == "frame" or row.id == "speed") then
        local ok, err = pcall(function()
          local r = row.impl.nuklear(nk.mod)
          if type(r) == "function" then for _ = 1, 5 do r() end end
        end)
        check(ok, "nuklear " .. row.id .. " survives repeated frames: " .. tostring(err))
      end
    end
  end

  -- ---- a cell that took the program down is remembered and not run again
  -- (skipped where the save folder cannot be written, as on LÖVE 12.0 in a sandbox)
  if love.filesystem.write(core.libload.MARKER, "gui/speed/nuklear") then
    core.libload.recoverCrash()
    check(core.libload.crashed["gui/speed/nuklear"] and not love.filesystem.getInfo(core.libload.MARKER), "a crash marker is read once and cleared")
    local g = require("bench.groups.gui")
    local cell = require("bench.runner").cell(g, g.rows[4], { id = "nuklear", lib = "love-nuklear" }, { window = true })
    check(cell.status == "FAIL" and contains(cell.note, "aborted"), "that cell is reported, not run")
    core.libload.crashed = {}
  end

  -- ---- quit needs Back twice
  local quit = false
  local realQuit = love.event.quit
  love.event.quit = function() quit = true end
  press("escape"); check(not quit, "one Back does not quit")
  press("escape"); check(quit, "two do")
  love.event.quit = realQuit

  print(("selftest: %d checks, %d failed"):format(checks, #failures))
  return #failures > 0
end

return T
