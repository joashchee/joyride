-- The mouse and the touch screen: where, which buttons, how many fingers at once. A touch also draws a + in the box.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Mouse and touch", raw = true, parent = "home" }

function P.enter(app, s)
  app.say("Move, click, scroll, touch. Hold the bottom bar to leave.")
end

function P.draw(app, scr, s)
  local i = s.input
  local m, t = i.mouse, i.touch
  scr:box(0, 1, screen.COLS, 7, "Mouse", ST.frame, true)
  local mx, my = love.mouse.getPosition()
  scr:text(2, 2, ("Position %d, %d  (this platform: mouse %s)"):format(mx, my, s.profile.mouse == "none" and "not expected" or s.profile.mouse), ST.body)
  local held = {}
  for b = 1, 5 do if love.mouse.isDown(b) then held[#held + 1] = b end end
  scr:text(2, 3, "Buttons held: " .. (#held > 0 and table.concat(held, " ") or "none") .. ("   clicks %d"):format(m.presses), #held > 0 and ST.ok or ST.body)
  scr:text(2, 4, ("Wheel total %+d   moved: %s"):format(m.wheel, m.moved and "yes" or "no"), m.moved and ST.ok or ST.body)
  local cur = love.mouse.isCursorSupported and love.mouse.isCursorSupported()
  scr:text(2, 5, "Custom cursors: " .. (cur and "supported" or "not supported") .. "   visible: " .. tostring(love.mouse.isVisible()), ST.dim)
  scr:text(2, 6, "Grabbed: " .. tostring(love.mouse.isGrabbed and love.mouse.isGrabbed() or false), ST.dim)

  scr:box(0, 8, screen.COLS, 6, "Touch", ST.frame, true)
  local ids = love.touch and love.touch.getTouches() or {}
  scr:text(2, 9, ("Fingers down now %d   most at once %d   (this platform: touch %s)"):format(#ids, t.max, s.profile.touch == "none" and "not expected" or s.profile.touch), #ids > 0 and ST.ok or ST.body)
  scr:text(2, 10, ("Taps %d   moves %d   pressure up to %.2f"):format(t.taps, t.moves, t.pressureMax), t.seen and ST.ok or ST.body)
  scr:text(2, 11, t.seen and "Touch works." or "No touch yet: tap the screen. (A mouse also taps; it is not counted here.)", t.seen and ST.ok or ST.dim)
  scr:text(2, 12, "LÖVE reports a touch as a mouse press too (istouch); that one is not counted as a mouse.", ST.dim)

  -- A box where every finger and the mouse leave a mark.
  scr:box(0, 14, screen.COLS, 8, "Pad: fingers show as + and the mouse as x", ST.frame)
  local marks = 0
  for _, id in ipairs(ids) do
    local x, y = love.touch.getPosition(id)
    local col, row = app.cellAt(x, y)
    if col and col > 0 and col < screen.COLS - 1 and row > 14 and row < 21 then scr:text(col, row, "+", ST.warn); marks = marks + 1 end
  end
  local col, row = app.cellAt(mx, my)
  if col and col > 0 and col < screen.COLS - 1 and row > 14 and row < 21 then scr:text(col, row, "x", ST.ok) end
  app.say(s.chord.t > 0 and ("Leaving... %d%%"):format(math.floor(s.chord:progress() * 100)) or "Move, click, scroll, touch. Hold the bottom bar to leave.", s.chord.t > 0 and "warn" or "hint")
end

return P
