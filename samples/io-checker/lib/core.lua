-- The IO Checker's state and the work that needs no screen: detecting the platform, probing, loading every asset, building and
-- writing the report. The screens (ui/) draw this state and feed it what the person does. Uses the global `love`.

local model = require("lib.model")
local platform = require("lib.platform")
local probe = require("lib.probe")
local assets = require("lib.assets")
local report = require("lib.report")

local C = { model = model, platform = platform, probe = probe, assets = assets, report = report }

--- A fresh state. Nothing is probed yet (init does that), so a test can build one with a mocked love.
function C.newState()
  return {
    page = "home", sel = {}, input = model.newInput(), idle = model.newIdle(), chord = model.newChord(), frames = model.newFrames(),
    answers = {}, seen = {}, override = nil, reply = "Pick a check.", kind = "hint", joysticks = {},
  }
end

--- What platform.detect needs, read from the running LÖVE.
function C.environment()
  local d, win
  if love.window and love.window.getDesktopDimensions then
    local w, h = love.window.getDesktopDimensions()
    d = { w = w, h = h }
    local ww, wh = love.window.getMode()
    win = { w = ww, h = wh }
  end
  return {
    os = love.system.getOS(), getenv = os.getenv, shim = package.loaded["_jr_shim"], source = love.filesystem.getSource(),
    desktop = d, window = win, jit = rawget(_G, "jit"),
  }
end

--- The version and platform in the form the asset checks want.
function C.assetContext(s)
  local major, minor = love.getVersion()
  return { major = major, minor = minor, id = s.profile.id, jit = rawget(_G, "jit") ~= nil }
end

--- Re-resolve the profile (after the override changes) and re-probe the capabilities.
function C.refresh(s)
  s.profile, s.forced = platform.resolve(s.platform, s.override)
  s.videoInfo = probe.video()
  s.caps = probe.capabilities(s)
end

function C.init(_, s)
  s = s or C.state or C.newState()
  C.state = s
  s.shim = package.loaded["_jr_shim"]
  s.platform = platform.detect(C.environment())
  C.refresh(s)
  local ok, text = pcall(love.filesystem.read, "CREDITS.txt")
  s.creditsText = ok and text or nil
  return s
end

--- Load every asset the way a game would (the checks that need a window are SKIP without one).
function C.runAssets(s)
  local ctx = C.assetContext(s)
  local results = assets.checkAll(ctx, model)
  -- Files nobody listed (the Asset Library put them there): loaded by the loader their extension suggests.
  for _, u in ipairs(assets.unlisted()) do
    local res
    if u.how then res = assets.check(u, ctx, model) else res = { entry = u, status = "SKIP", detail = "unknown file type: nothing here loads it", expect = "maybe" } end
    res.unlisted = true
    results[#results + 1] = res
  end
  s.assetResults = results
  return results
end

function C.runStorage(s)
  s.storage = probe.storage(s.profile)
  return s.storage
end

--- Refresh the list of connected joysticks into s.joysticks (closures over the live objects) and record each one.
function C.pollJoysticks(s)
  s.joysticks = {}
  if not (love.joystick and love.joystick.getJoysticks) then return end
  for _, j in ipairs(love.joystick.getJoysticks()) do
    local nb, na, nh = j:getButtonCount(), j:getAxisCount(), j:getHatCount()
    local id = j:getID()
    s.input:pad(id, { name = j:getName(), guid = j:getGUID(), buttons = nb, axes = na, hats = nh, gamepad = j:isGamepad() })
    s.joysticks[#s.joysticks + 1] = {
      id = id, name = j:getName(), buttons = nb, axes = na, hats = nh, gamepad = j:isGamepad(), obj = j,
      down = function(i) return i <= nb and j:isDown(i) end,
      axis = function(i) return i <= na and j:getAxis(i) or 0 end,
      hat = function() return nh > 0 and j:getHat(1) or "c" end,
    }
  end
end

--- The report from what is known now. `how` goes in its header.
function C.buildReport(s, how)
  if not s.assetResults then C.runAssets(s) end
  if not s.storage then C.runStorage(s) end
  C.pollJoysticks(s)
  return report.build(s, how)
end

--- `love . --report`: the checks that need no person.
function C.headlessReport()
  local s = C.state
  return C.buildReport(s, "love . --report (nothing pressed, shown or played)")
end

return C
