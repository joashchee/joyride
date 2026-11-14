-- Motion sensors (LÖVE 12.0's love.sensor; on 11.5 a phone's accelerometer is a joystick) and vibration.
local screen = require("ui.screen")
local opt = require("lib.optional")
local ST = screen.ST

local P = { title = "Sensors and vibration", raw = true, parent = "home" }
local KINDS = { "accelerometer", "gyroscope" }

local function vibrate(app, s)
  if love.system.vibrate then
    pcall(love.system.vibrate, 0.4)
    s.answers.vibrate = "buzzed"
    app.say("Buzzing for 0.4 s. Did you feel it? [Felt] or [Not felt].", "warn")
  else
    app.say("love.system.vibrate is missing on this platform.", "error")
  end
end
local function felt(app, s) s.answers.vibrate = "felt"; app.say("Vibration felt.", "ok") end
local function notfelt(app, s) s.answers.vibrate = "not felt"; app.say("Vibration not felt.", "warn") end
P.soft = { { "Buzz", vibrate }, { "Felt", felt }, { "Not felt", notfelt } }
function P.key(app, s, k)
  if k == "v" then vibrate(app, s) return true end
  return false
end

function P.enter(app, s)
  if opt.get("sensor") then
    for _, k in ipairs(KINDS) do
      if opt.call("sensor.hasSensor", k) then pcall(opt.get("sensor.setEnabled"), k, true) end
    end
  end
  app.say("Tilt and turn the device. Hold the bottom bar to leave.")
end

function P.leave()
  if opt.get("sensor") then for _, k in ipairs(KINDS) do pcall(opt.get("sensor.setEnabled"), k, false) end end
end

function P.update(app, s)
  if opt.get("sensor") then
    for _, k in ipairs(KINDS) do
      if opt.call("sensor.hasSensor", k) and opt.call("sensor.isEnabled", k) then
        local x, y, z = opt.call("sensor.getData", k)
        if x then s.input:sensor(k, x, y, z) end
      end
    end
  end
  -- 11.5 on a phone: the accelerometer is a joystick named "Accelerometer".
  for _, j in ipairs(s.joysticks) do
    if j.name:lower():find("accelerom", 1, true) and j.axes >= 3 then s.input:sensor("accelerometer (joystick)", j.axis(1), j.axis(2), j.axis(3)) end
  end
end

function P.draw(app, scr, s)
  scr:box(0, 1, screen.COLS, 21, ("Sensors (this platform: %s)"):format(s.profile.sensors), ST.frame, true)
  local y = 2
  local any = false
  for kind, r in pairs(s.input.sensors) do
    any = true
    scr:text(2, y, kind, ST.strong)
    local f = app.core.model.fmt
    scr:text(2, y + 1, ("now %s %s %s"):format(f(r.last[1]), f(r.last[2]), f(r.last[3])), ST.ok)
    scr:text(2, y + 2, ("min %s %s %s"):format(f(r.min[1]), f(r.min[2]), f(r.min[3])), ST.body)
    scr:text(2, y + 3, ("max %s %s %s   %d readings"):format(f(r.max[1]), f(r.max[2]), f(r.max[3]), r.n), ST.body)
    y = y + 5
  end
  if not any then
    local why = opt.get("sensor") and "This device reports no accelerometer or gyroscope." or "LÖVE 11.5 has no love.sensor (it arrives in 12.0). A phone's accelerometer would show as a joystick on the Game controllers page."
    for k, line in ipairs(screen.wrap(why, screen.COLS - 6)) do scr:text(3, y + k - 1, line, ST.warn) end
    y = y + 4
  end
  scr:text(2, 14, "Vibration", ST.strong)
  scr:text(2, 15, ("love.system.vibrate: %s   last answer: %s"):format(love.system.vibrate and "present" or "missing", s.answers.vibrate or "none"), ST.body)
  scr:text(2, 16, "A desktop has none; a phone should buzz.", ST.dim)
  scr:text(2, 17, ("Power: %s"):format(love.system.getPowerInfo and (love.system.getPowerInfo() or "?") or "?"), ST.dim)
  app.say(s.chord.t > 0 and ("Leaving... %d%%"):format(math.floor(s.chord:progress() * 100)) or "Tilt and turn the device. Hold the bottom bar to leave.", s.chord.t > 0 and "warn" or "hint")
end

return P
