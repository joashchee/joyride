-- Six test tones, generated in code (nothing to load): the left channel, the right, both, a low one, a sweep and a quiet one.
-- Play one, then say whether it was heard. The answers go in the device report.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Sound", foot = "A play  X heard  Y silent  < > volume" }
local tones, source = {}, nil

local function play(app, s)
  local model = app.core.model
  local t = model.TONES[s.sel.sound]
  if not (love.audio and love.sound) then app.say("No sound: LÖVE's audio module did not start.", "error") return end
  if not tones[t.id] then
    local n = model.toneFrames(t)
    local sd = love.sound.newSoundData(n, model.RATE, 16, 2)
    for k = 0, n - 1 do
      local l, r = model.toneSample(t, k)
      sd:setSample(k * 2, l)
      sd:setSample(k * 2 + 1, r)
    end
    tones[t.id] = sd
  end
  if source then source:stop() end
  local ok, err = pcall(function()
    source = love.audio.newSource(tones[t.id], "static")
    source:play()
  end)
  if not ok then app.say("Could not play: " .. tostring(err), "error") s.audioError = true return end
  s.answers[t.id] = "played"
  app.say(("Playing %s. Heard it? X yes, Y no."):format(t.label))
end

function P.draw(app, scr, s)
  local model = app.core.model
  local items = {}
  local label = { heard = "heard", silent = "SILENT", played = "played" }
  for i, t in ipairs(model.TONES) do
    local a = s.answers[t.id]
    items[i] = { label = t.label, value = a and label[a] or "-", vstyle = a == "silent" and ST.error or (a == "heard" and ST.ok or nil) }
  end
  s.sel.sound = s.sel.sound or 1
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = #items + 2, title = "Test tones", items = items, sel = s.sel.sound, foot = "" })
  local top = #items + 3
  scr:box(0, top, screen.COLS, 22 - top, "The audio output", ST.frame)
  local a = app.core.probe.audio()
  local y = top + 1
  for _, r in ipairs(a.rows) do
    scr:text(2, y, screen.fit(("%-17s %s"):format(r.label, r.value), screen.COLS - 4), a.error and ST.error or ST.body)
    y = y + 1
  end
  scr:text(2, y + 1, "Try the speaker, then headphones. On the web, sound", ST.dim)
  scr:text(2, y + 2, "starts only after a click or key: press Play once.", ST.dim)
end

function P.act(app, s, a)
  local n = #app.core.model.TONES
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.sound = screen.nav(s.sel.sound or 1, n, n, a) return true end
  if a == "accept" then play(app, s) return true end
  if a == "x" or a == "y" then
    local t = app.core.model.TONES[s.sel.sound]
    s.answers[t.id] = a == "x" and "heard" or "silent"
    app.say(("%s: %s."):format(t.label, a == "x" and "heard" or "silent"), a == "x" and "ok" or "warn")
    return true
  end
  if (a == "left" or a == "right") and love.audio then
    local v = math.max(0, math.min(1, love.audio.getVolume() + (a == "right" and 0.1 or -0.1)))
    love.audio.setVolume(v)
    app.say(("Volume %d%%"):format(math.floor(v * 100 + 0.5)))
    return true
  end
  return false
end

function P.tap(app, s, col, row)
  local i = row - 1
  if i >= 1 and i <= #app.core.model.TONES then
    if s.sel.sound == i then play(app, s) else s.sel.sound = i end
    return true
  end
  return false
end

return P
