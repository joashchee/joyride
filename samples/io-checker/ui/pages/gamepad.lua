-- Every controller: its shape, which buttons are down now and which have ever been pressed, each axis live with the range it has
-- reached, the hat, and chatter. Nothing navigates; a pad is leave-able with Select+Start held (or Escape, or the bar).
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Game controllers", raw = true, parent = "home" }

function P.enter(app, s)
  app.core.pollJoysticks(s)
  app.say("Press every button, push every stick. Hold Select+Start to leave.")
end

local function rumble(app, s)
  local n = 0
  for _, j in ipairs(s.joysticks) do
    if j.obj.isVibrationSupported and j.obj:isVibrationSupported() then
      j.obj:setVibration(0.6, 0.6, 0.4)
      n = n + 1
    end
  end
  app.say(n > 0 and ("Rumbling %d controller(s) for 0.4 s."):format(n) or "No connected controller can rumble.", n > 0 and "ok" or "warn")
end

P.soft = { { "Rumble", rumble } }
function P.key(app, s, k) if k == "r" then rumble(app, s) end return false end

function P.draw(app, scr, s)
  local i = s.input
  local fmt = app.core.model.fmt
  local n = #s.joysticks
  scr:box(0, 1, screen.COLS, 21, ("Controllers: %d (this platform: %s)"):format(n, s.profile.gamepad), ST.frame, true)
  if n == 0 then
    local msg = "No game controller is connected. Plug one in or pair one over Bluetooth."
    if s.profile.id == "portmaster" then msg = msg .. " On PortMaster the pad also arrives as KEYS through gptokeyb: look at the Keyboard page." end
    for k, line in ipairs(screen.wrap(msg, screen.COLS - 6)) do scr:text(3, 2 + k, line, ST.warn) end
  end
  local y = 2
  for idx, j in ipairs(s.joysticks) do
    if y > 16 then scr:text(3, 20, ("… and %d more"):format(n - idx + 1), ST.dim) break end
    local p = i.pads[j.id]
    scr:text(2, y, screen.fit(("%d. %s"):format(idx, j.name), screen.COLS - 4), ST.strong)
    scr:text(2, y + 1, ("%d buttons, %d axes, %d hats, SDL mapping %s"):format(j.buttons, j.axes, j.hats, j.gamepad and "yes" or "no"), ST.body)
    local held = {}
    for b = 1, j.buttons do if j.down(b) then held[#held + 1] = b end end
    local missing = i:missing(j.id)
    scr:text(2, y + 2, ("Down: %s"):format(#held > 0 and table.concat(held, " ") or "-"), #held > 0 and ST.ok or ST.body)
    local seen = j.buttons - #missing
    scr:text(2, y + 3, ("Seen %d/%d   never: %s"):format(seen, j.buttons, (#missing > 0 and #missing < j.buttons) and table.concat(missing, " ") or (#missing == 0 and "-" or "all")), #missing == 0 and ST.ok or ST.body)
    local rows = math.min(j.axes, 4)
    for a = 1, rows do
      local v = j.axis(a)
      local r = p and p.axes[a]
      scr:text(2, y + 3 + a, ("A%d %s %s %s"):format(a, screen.axisBar(v, 15), fmt(v), r and ("%s %s"):format(fmt(r.min), fmt(r.max)) or ""), math.abs(v) > 0.5 and ST.warn or ST.body)
    end
    local chat = i:chatterTotal(j.id)
    scr:text(2, y + 4 + rows, ("Hat %s   chatter %d"):format(j.hats > 0 and j.hat() or "none", chat), chat > 0 and ST.error or ST.dim)
    y = y + rows + 6
  end
  app.say(s.chord.t > 0 and ("Leaving... %d%%"):format(math.floor(s.chord:progress() * 100)) or "Press every button, push every stick. Hold Select+Start to leave.", s.chord.t > 0 and "warn" or "hint")
end

return P
