-- The Benchmark Libraries' state and the work that needs no screen: detecting the platform, running the suite, building the report.
-- The screens (ui/) draw this state. Uses the global `love`.

local model = require("lib.model")
local platform = require("lib.platform")
local libload = require("lib.libload")
local ratings = require("lib.ratings")
local report = require("lib.report")
local suite = require("bench.suite")

local C = { model = model, platform = platform, report = report, suite = suite, libload = libload }

function C.newState()
  return {
    page = "home", sel = {}, idle = model.newIdle(), chord = model.newChord(), reply = "Run the benchmark, then pick a comparison.", kind = "hint",
    results = suite.newResults(), ran = false, running = nil, col = {},
  }
end

--- What platform.detect needs, read from the running LÖVE (the IO Checker's way).
function C.environment()
  local d, win
  if love.window and love.window.getDesktopDimensions then
    local w, h = love.window.getDesktopDimensions()
    d = { w = w, h = h }
    local ww, wh = love.window.getMode()
    win = { w = ww, h = wh }
  end
  return { os = love.system.getOS(), getenv = os.getenv, shim = package.loaded["_jr_shim"], source = love.filesystem.getSource(), desktop = d, window = win, jit = rawget(_G, "jit") }
end

--- What a benchmark run depends on: whether there is a window, and whether the platform has an FFI.
function C.runEnv(s)
  return { window = love.graphics ~= nil and love.window ~= nil and love.window.isOpen and love.window.isOpen() or false, ffi = s.profile.ffi, threads = s.profile.threads }
end

function C.refresh(s)
  s.profile, s.forced = platform.resolve(s.platform, s.override)
  s.env = C.runEnv(s)
end

function C.init(_, s)
  s = s or C.state or C.newState()
  C.state = s
  s.shim = package.loaded["_jr_shim"]
  s.platform = platform.detect(C.environment())
  C.refresh(s)
  libload.recoverCrash()
  s.ratings = ratings.new(ratings.loveStore())
  s.ratings:load()
  return s
end

--- Start a run that does a few steps a frame (the screen calls `C.pump`); `only` limits it to one group.
function C.start(s, only)
  libload.reset()
  s.results = suite.newResults()
  C.refresh(s)
  s.running = { step = suite.steps(s.results, s.env, only), only = only, t0 = os.clock() }
end

--- Do steps for up to `budget` seconds. Returns true while there is more to do.
function C.pump(s, budget)
  local run = s.running
  if not run then return false end
  local t0 = os.clock()
  repeat
    if not run.step() then
      s.running, s.ran = nil, true
      s.ranSeconds = os.clock() - run.t0
      return false
    end
  until os.clock() - t0 >= budget
  return true
end

--- Everything, now (the headless run).
function C.runAll(s)
  C.start(s)
  while C.pump(s, 1) do end
end

function C.buildReport(s, how)
  if not s.ran then C.runAll(s) end
  return report.build(s, how)
end

function C.headlessReport()
  return C.buildReport(C.state, "love . --report (no window: comparisons that need one are SKIP)")
end

return C
