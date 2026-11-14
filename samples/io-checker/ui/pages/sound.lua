-- Six test tones, generated in code (nothing to load): the left channel, the right, both, a low one, a sweep and a quiet one.
-- Play one, then say whether it was heard. The answers go in the device report.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Sound" }
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

local function answer(app, s, heard)
  local t = app.core.model.TONES[s.sel.sound]
  if s.answers[t.id] == nil then app.say("Play it first (Enter, or the Play button).", "warn") return end
  s.answers[t.id] = heard and "heard" or "silent"
  app.say(("%s: %s."):format(t.label, heard and "heard" or "SILENT"), heard and "ok" or "warn")
  s.sel.sound = s.sel.sound % #app.core.model.TONES + 1 -- on to the next tone
end

-- On screen for a mouse or a finger; on a keyboard Y and N (or H) answer, on a pad X and Y.
P.soft = {
  { "Play", function(app, s) play(app, s) end },
  { "Heard", function(app, s) answer(app, s, true) end },
  { "Silent", function(app, s) answer(app, s, false) end },
}

function P.key(app, s, k, isrepeat)
  if isrepeat then return false end
  if k == "y" or k == "h" then answer(app, s, true) return true end
  if k == "n" then answer(app, s, false) return true end
  return false
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
  scr:text(2, y, "1. Play a tone   2. Did you hear it?   Heard or Silent", ST.strong)
  scr:text(2, y + 1, "Keyboard: Enter plays, Y heard, N silent. Pad: A plays,", ST.dim)
  scr:text(2, y + 2, "X heard, Y silent. Mouse or touch: the buttons below.", ST.dim)
  scr:text(2, y + 3, "On the web, sound starts after a click or key.", ST.dim)
end

function P.act(app, s, a)
  local n = #app.core.model.TONES
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.sound = screen.nav(s.sel.sound or 1, n, n, a) return true end
  if a == "accept" then play(app, s) return true end
  if a == "x" or a == "y" then answer(app, s, a == "x") return true end
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
