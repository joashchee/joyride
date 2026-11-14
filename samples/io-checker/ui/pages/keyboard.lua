-- Every key and every character, recorded; nothing navigates.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Keyboard and text", raw = true, parent = "home" }

function P.enter(app, s)
  s.input:note("keyboard page")
  if love.keyboard.setTextInput then pcall(love.keyboard.setTextInput, true) end
  app.say("Press keys; type text. Hold Escape for a second to leave.")
end

function P.leave()
  if love.keyboard.setTextInput then pcall(love.keyboard.setTextInput, false) end
end

function P.draw(app, scr, s)
  local i = s.input
  scr:box(0, 1, screen.COLS, 21, "Keys", ST.frame, true)
  local expect = s.profile.keyboard
  scr:text(2, 2, ("This platform: keyboard is %s."):format(expect == "none" and "not expected" or expect), ST.hint)
  scr:text(2, 4, ("Different keys seen: %d"):format(i.keyCount), i.keyCount > 0 and ST.ok or ST.body)
  local down = {}
  for _, k in ipairs({ "lshift", "rshift", "lctrl", "rctrl", "lalt", "ralt", "lgui", "rgui" }) do
    if love.keyboard.isDown(k) then down[#down + 1] = k end
  end
  scr:text(2, 5, "Modifiers held: " .. (#down > 0 and table.concat(down, " ") or "none"))
  local names = {}
  for k in pairs(i.keys) do names[#names + 1] = k end
  table.sort(names)
  local wrapped = screen.wrap(#names > 0 and table.concat(names, " ") or "none yet", screen.COLS - 6)
  scr:text(2, 7, "Keys seen:", ST.strong)
  for n = 1, math.min(#wrapped, 6) do scr:text(3, 7 + n, wrapped[n], ST.body) end
  scr:text(2, 15, "Typed text (love.textinput):", ST.strong)
  scr:text(3, 16, i.text ~= "" and screen.fit(i.text, screen.COLS - 6) or "(nothing typed yet)", i.text ~= "" and ST.ok or ST.dim)
  local ti = love.keyboard.hasTextInput and love.keyboard.hasTextInput()
  scr:text(2, 18, "Text input is " .. (ti and "on (an on-screen keyboard may show)" or "off"), ST.dim)
  scr:text(2, 19, "Key repeat is " .. (love.keyboard.hasKeyRepeat() and "on" or "off"), ST.dim)
  scr:text(2, 20, "Last events:", ST.dim)
  for n, e in ipairs(i.log) do if n <= 1 then scr:text(15, 20, screen.fit(e, 30), ST.dim) end end
  app.say(s.chord.t > 0 and ("Leaving... %d%%"):format(math.floor(s.chord:progress() * 100)) or (s.idle:warning() and ("No input: back in %d s"):format(math.ceil(s.idle:remaining())) or "Press keys; type text. Hold Escape to leave."), s.chord.t > 0 and "warn" or "hint")
end

return P
