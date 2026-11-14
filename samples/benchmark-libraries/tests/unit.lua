-- Plain-luajit tests of everything that does not need LÖVE: `luajit tests/unit.lua` (from the project folder). They load the real libraries
-- from libs/ with a minimal stand-in for `love`, so they also pin what each library does at the commit this project ships.

dofile("tests/lovestub.lua")
package.path = "./?.lua;" .. package.path

local passed, failed = 0, 0
local function check(ok, what)
  if ok then passed = passed + 1 else failed = failed + 1; print("FAIL " .. what) end
end
local function eq(a, b, what) check(a == b, ("%s: got %s, want %s"):format(what, tostring(a), tostring(b))) end

local H = require("bench.harness")
local rating = require("bench.rating")
local model = require("lib.model")
local ratings = require("lib.ratings")
local libload = require("lib.libload")
local runner = require("bench.runner")
local suite = require("bench.suite")
local specialists = require("bench.specialists")
local U = require("bench.util")

-- ---- harness ------------------------------------------------------------------------------------------------------------------------

do
  local a, b = H.rng(42), H.rng(42)
  local same = true
  for _ = 1, 100 do if a() ~= b() then same = false end end
  check(same, "the same seed gives the same stream")
  local c, d = H.rng(1), H.rng(2)
  check(c() ~= d(), "different seeds differ")
  local r, lo, hi = H.rng(9), 1, 0
  for _ = 1, 1000 do local v = r(); lo = math.min(lo or v, v); hi = math.max(hi, v) end
  check(lo >= 0 and hi < 1, "the stream stays in [0, 1)")
  local i = H.int(H.rng(3), 5, 8)
  check(i >= 5 and i <= 8 and i == math.floor(i), "H.int is an integer in range")
  -- The first values are fixed forever, so a result on one machine means the same work as on another.
  local f = H.rng(1)
  eq(math.floor(f() * 1e6), 22, "first value of seed 1 (Park-Miller)")

  local rate, last = H.rate(function() return 7 end, 10, { minTime = 0.002, reps = 2 })
  check(rate > 0 and last == 7, "H.rate gives a positive rate and the work's answer")
  local kb = H.alloc(function() local t = {}; for k = 1, 100 do t[k] = {} end; return #t end)
  check(kb > 1, "H.alloc sees what a call allocates")
  local zero = H.alloc(function() return 1 end)
  check(zero < 1, "H.alloc sees nothing for no allocation")

  eq(H.run({ kind = "feature" }, nil, nil).status, "NA", "no impl is NA")
  eq(H.run({ kind = "feature" }, false, nil).status, "NA", "impl false is NA")
  eq(H.run({ kind = "feature" }, function() return true end, nil).status, "PASS", "true is PASS")
  eq(H.run({ kind = "feature" }, function() return false, "why" end, nil).note, "why", "false carries its note")
  eq(H.run({ kind = "feature" }, function() error("boom") end, nil).status, "FAIL", "an error is FAIL")
  eq(H.run({ kind = "feature" }, function() return "skip", "no window" end, nil).status, "SKIP", "skip is SKIP")
  eq(H.run({ kind = "feature" }, function() return true, true end, nil).note, "works", "a non-string note is ignored")
  eq(H.run({ kind = "speed", units = 1, expect = function() return false, "nope" end }, function() return function() return 1 end end, nil).status, "FAIL", "a wrong answer in a speed row is FAIL")
  eq(H.run({ kind = "value" }, function() return function() return 42 end end, nil).value, 42, "a value row takes the number")
end

-- ---- rating -----------------------------------------------------------------------------------------------------------------------------

do
  local speed = { kind = "speed" }
  local cells = { a = H.cell("PASS", "", 1000), b = H.cell("PASS", "", 600), c = H.cell("PASS", "", 300), d = H.cell("PASS", "", 100), e = H.cell("PASS", "", 10), f = H.cell("FAIL", "x"), g = H.cell("NA"), h = H.cell("SKIP") }
  rating.rate(speed, cells)
  eq(cells.a.auto, 5, "the fastest is 5"); eq(cells.b.auto, 4, "60% of the best is 4"); eq(cells.c.auto, 3, "30% is 3")
  eq(cells.d.auto, 2, "10% is 2"); eq(cells.e.auto, 1, "1% is 1"); eq(cells.f.auto, 1, "FAIL is 1"); eq(cells.g.auto, 0, "NA is 0"); eq(cells.h.auto, nil, "SKIP has none")
  local lower = { kind = "memory", lower = true, floor = 1 }
  local m = { a = H.cell("PASS", "", 0.1), b = H.cell("PASS", "", 0.5), c = H.cell("PASS", "", 50) }
  rating.rate(lower, m)
  eq(m.a.auto, 5, "below the floor is the best"); eq(m.b.auto, 5, "both below the floor tie"); eq(m.c.auto, 1, "50 against a floor of 1 is 1")
  local feat = { kind = "feature" }
  local f = { a = H.cell("PASS"), b = H.cell("FAIL"), c = H.cell("NA") }
  rating.rate(feat, f)
  eq(f.a.auto, 5, "a working feature is 5")

  local order = { "a", "b", "c" }
  local best = rating.best(feat, f, order)
  check(best.a and not best.b and not best.c, "the one that works is best")
  local all = { a = H.cell("PASS"), b = H.cell("PASS") }
  rating.rate(feat, all)
  check(next(rating.best(feat, all, { "a", "b" })) == nil, "nobody is best when all are equal")
  local low = { a = H.cell("FAIL"), b = H.cell("NA") }
  rating.rate(feat, low)
  check(next(rating.best(feat, low, { "a", "b" })) == nil, "a best of 1 is not 'best'")
  -- the person's rating overrules
  local best2 = rating.best(feat, all, { "a", "b" }, function(id) return id == "b" and 2 or nil end)
  check(best2.a and not best2.b, "a person's lower rating moves the best")
  eq(rating.effective(all.a, 3), 3, "manual counts"); eq(rating.effective(all.a, nil), 5, "automatic otherwise"); eq(rating.effective(H.cell("SKIP"), 4), nil, "skip has no rating")

  local rows = { { id = "r1", kind = "feature" }, { id = "r2", kind = "feature" }, { id = "c1", kind = "cost", weight = 0.5 } }
  local res = { r1 = { a = { status = "PASS", auto = 5 }, b = { status = "NA", auto = 0 } }, r2 = { a = { status = "PASS", auto = 5 }, b = { status = "PASS", auto = 5 } }, c1 = { a = { status = "PASS", auto = 1 }, b = { status = "PASS", auto = 5 } } }
  local sc = rating.groupScores(rows, { "a", "b" }, res)
  check(math.abs(sc[1].score - (5 + 5 + 0.5) / 2.5) < 1e-9, "a weighted mean")
  check(math.abs(sc[2].score - (0 + 5 + 2.5) / 2.5) < 1e-9, "NA counts as 0")
  check(sc[1].best and not sc[2].best, "the higher mean is best")
  local only = rating.groupScores({ { id = "r1", kind = "feature" }, { id = "c1", kind = "cost" } }, { "a" }, { r1 = { a = { status = "SKIP" } }, c1 = { a = { status = "PASS", auto = 5 } } })
  eq(only[1].score, nil, "costs alone give no score")
  local tie = rating.groupScores({ { id = "r", kind = "feature" } }, { "a", "b" }, { r = { a = { status = "PASS", auto = 4 }, b = { status = "PASS", auto = 4 } } })
  check(not tie[1].best and not tie[2].best, "a tie marks nobody")
  eq(rating.stars(3), "***..", "stars")
end

-- ---- the person's ratings ----------------------------------------------------------------------------------------------------------------------

do
  local text
  local store = { read = function() return text end, write = function(t) text = t; return true end }
  local r = ratings.new(store)
  check(r:set("classes", "isa", "classic", 4), "a rating of 4 is accepted")
  check(not r:set("classes", "isa", "classic", 6), "6 is refused"); check(not r:set("classes", "isa", "classic", 0), "0 is refused")
  eq(r:get("classes", "isa", "classic"), 4, "it is kept")
  eq(r:step("classes", "isa", "classic", 1), 5, "step up"); eq(r:step("classes", "isa", "classic", 1), 5, "stops at 5")
  eq(r:step("g", "r", "m", -1, 1), 1, "stops at 1")
  r:save()
  local again = ratings.new(store); again:load()
  eq(again:get("classes", "isa", "classic"), 5, "saved and loaded"); eq(again:count(), 2, "both saved")
  again:set("classes", "isa", "classic", nil)
  eq(again:get("classes", "isa", "classic"), nil, "cleared")
  text = "a/b/c\t3\nbroken line\nx/y\t2\na/b/d\t9\n"
  again:load()
  eq(again:count(), 1, "bad lines are ignored when loading")
end

-- ---- model -----------------------------------------------------------------------------------------------------------------------------------

do
  eq(model.si(1234567), "1.2M", "si millions"); eq(model.si(12345), "12k", "si ten thousands"); eq(model.si(0.5), "0.50", "si small")
  eq(model.judge("need", true), "PASS", "need and present"); eq(model.judge("need", false), "FAIL", "need and missing"); eq(model.judge("none", false), "SKIP", "not expected")
  local chord = model.newChord(1)
  check(not chord:update(0.5, true) and chord:update(0.6, true), "the chord fires after the hold")
  local r = model.newReport({ made = "now" })
  r:line("PASS", "a", "b"); r:line("FAIL", "c", "d")
  check(r:text():find("PASS a: b", 1, true) and #r.fails == 1, "report lines")
  eq(model.reportName("macos", 11, 5, "md"), "benchmark-libraries-macos-11.5.md", "report name")
end

-- ---- the catalogue and the vendored libraries ------------------------------------------------------------------------------------------------

do
  local names = {}
  for _, e in ipairs(libload.entries) do
    names[e.name] = true
    check(e.module and e.rev and #e.rev == 40, e.name .. " has a module and a pinned commit")
  end
  eq(#libload.entries, 70, "all 70 curated libraries are in the catalogue")
  local lock = io.open("love.lock"):read("*a")
  for _, e in ipairs(libload.entries) do
    check(lock:find('commit = "' .. e.rev .. '"', 1, true), e.name .. " is locked at its pinned commit in love.lock")
    local f = io.open("libs/" .. e.name .. "/.joyride-commit")
    check(f and f:read("*l") == e.rev, e.name .. " is vendored at its pinned commit")
    if f then f:close() end
  end
  -- no library of Valve's SDK, no binaries
  local p = io.popen("find libs -name '*.dylib' -o -name '*.so' -o -name '*.dll' -o -name '*.a' -o -iname 'steam_api*' 2>/dev/null")
  eq(p:read("*a"), "", "no compiled files in libs/"); p:close()
end

-- ---- every group is well formed ------------------------------------------------------------------------------------------------------------------

do
  local KINDS = { feature = true, speed = true, memory = true, value = true }
  for _, g in ipairs(suite.groups) do
    check(g.id and g.title and g.about and #g.members >= 2, g.id .. ": id, title, about, at least two members")
    local ids = {}
    for _, m in ipairs(g.members) do
      check(libload.by[m.lib], g.id .. "/" .. m.id .. ": a library that exists")
      check(m.short and #m.short <= 6, g.id .. "/" .. m.id .. ": a short name of at most 6 letters")
      check(not ids[m.id], g.id .. "/" .. m.id .. ": unique"); ids[m.id] = true
    end
    check(#g.members <= 8, g.id .. ": at most eight members (the table shows four at a time and scrolls sideways)")
    local rids = {}
    for _, row in ipairs(g.rows) do
      check(KINDS[row.kind], g.id .. "/" .. row.id .. ": a known kind")
      check(not rids[row.id], g.id .. "/" .. row.id .. ": unique row"); rids[row.id] = true
      check(row.label and #row.label <= 17, g.id .. "/" .. row.id .. ": a label of at most 17 characters")
      check(row.about and #row.about > 10, g.id .. "/" .. row.id .. ": says what it tests")
      for k in pairs(row.impl) do check(ids[k], g.id .. "/" .. row.id .. ": impl for " .. k .. " is a member") end
      for _, m in ipairs(g.members) do
        if row.impl[m.id] == nil then check(false, g.id .. "/" .. row.id .. ": no word (a function, or false) for " .. m.id) end
      end
      if row.kind ~= "feature" then check(row.unit or row.kind == "speed", g.id .. "/" .. row.id .. ": a unit") end
      if row.kind == "speed" then check(row.unit and row.units, g.id .. "/" .. row.id .. ": a unit and how many operations a call does") end
    end
  end
  -- every library is somewhere: in a group, or stand-alone
  local inSpec = {}
  for _, e in ipairs(specialists.list) do inSpec[e.lib] = true; check(libload.by[e.lib], e.lib .. ": a specialist for a library that exists") end
  for _, c in ipairs(suite.coverage()) do
    check(#c.groups > 0 or inSpec[c.name], c.name .. " is in a comparison or has its own check")
  end
end

-- ---- the real run: what each library does at its pinned commit ----------------------------------------------------------------------------------------

do
  local results = suite.runAll({ ffi = "need", window = false })
  eq(results.done, suite.total(), "every step ran")
  local function cell(g, r, m) return results.groups[g][r][m] end
  local function status(g, r, m) return cell(g, r, m).status end
  -- findings that are about the libraries themselves (they would change if a library were updated)
  eq(status("classes", "isa", "hump.class"), "NA", "hump.class has no is-a test")
  eq(status("classes", "isa", "classic"), "PASS", "classic can test its type")
  eq(status("tweens", "nested", "tween"), "FAIL", "tween.lua drops the sibling field of a nested tween")
  eq(status("tweens", "nested", "hump.timer"), "PASS", "hump.timer keeps it")
  eq(status("serialise", "cycle", "binser"), "PASS", "binser keeps cycles")
  eq(status("serialise", "cycle", "json"), "FAIL", "json refuses cycles")
  eq(status("serialise", "precision", "binser"), "PASS", "binser keeps full precision")
  eq(status("serialise", "precision", "json"), "FAIL", "json loses digits")
  eq(status("collision", "sweep", "bump"), "PASS", "bump does not tunnel")
  eq(status("collision", "sweep", "HC"), "NA", "HC does not sweep")
  eq(status("collision", "ray", "HC"), "FAIL", "HC's raycast misses rays heading right or down")
  eq(status("collision", "crowd", "bump"), "PASS", "bump finds every overlap")
  eq(status("collision", "crowd", "HC"), "PASS", "HC finds every overlap")
  eq(status("pathfinding", "shortest", "jumper"), "PASS", "jumper finds the shortest path")
  eq(status("pathfinding", "shortest", "batteries.pathfind"), "PASS", "batteries.pathfind finds the shortest path")
  eq(status("states", "args", "batteries.state_machine"), "NA", "state_machine passes no arguments")
  eq(status("printing", "meta", "batteries.pretty"), "FAIL", "pretty does not show metatables")
  eq(status("gui", "frame", "suit"), "SKIP", "GUI needs a window")
  -- every comparison cell that ran has a rating, and every group has scores
  for _, g in ipairs(suite.groups) do
    for _, row in ipairs(runner.rows(g)) do
      for _, m in ipairs(g.members) do
        local c = results.groups[g.id][row.id][m.id]
        check(c ~= nil, g.id .. "/" .. row.id .. "/" .. m.id .. " has a cell")
        if c and c.status ~= "SKIP" then check(c.auto ~= nil, g.id .. "/" .. row.id .. "/" .. m.id .. " is rated") end
        if c and c.status == "PASS" and row.kind ~= "feature" then check(type(c.value) == "number", g.id .. "/" .. row.id .. "/" .. m.id .. " has a measurement") end
      end
    end
  end
  local sc = suite.scores(suite.byId.classes, results)
  check(sc[2].best and sc[2].id == "classic", "classic scores best among the classes (and the person's rating can change that)")
  local manual = function(g, r, m) if g == "classes" and m == "classic" then return 1 end end
  local sc2 = suite.scores(suite.byId.classes, results, manual)
  check(not sc2[2].best, "rating every classic cell 1 takes the crown")
  -- the stand-alone checks
  eq(results.specialists["tiny-ecs"].status, "PASS", "tiny-ecs does its job")
  eq(results.specialists["semver"].status, "PASS", "semver does its job")
  -- its compiled library is built by tools/build-binaries.sh (git-ignored): the check runs where it was built and is SKIP where it was not
  local built = io.open("bin/libneumetik_midi.dylib") ~= nil
  eq(results.specialists["neumetik-midi"].status, built and "PASS" or "SKIP", "neumetik-midi synthesizes a song where its library is built")
  eq(results.specialists["anim8"].status, "SKIP", "anim8 needs a window")
  eq(results.specialists["luasteam"].status, "SKIP", "luasteam is compiled")
  -- a library that prints when required is quiet and counted
  eq(libload.loaded["lurker"].printed, 1, "lurker's greeting is counted, not printed")
  -- isolation: nothing a library required under a bare name is left behind
  check(package.loaded["class"] == nil and package.loaded["vector"] == nil, "bare helper names are forgotten after a library loads")
  check(libload.getPath() == package.path, "the require path is put back")
end

print(("unit: %d passed, %d failed"):format(passed, failed))
os.exit(failed == 0 and 0 or 1)
