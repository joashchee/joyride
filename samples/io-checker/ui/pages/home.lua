-- The list of checks. Each shows what the platform in force expects of it, and a tick once it has been tried.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Checks", foot = "A open  B quit" }

-- id, label, the capability whose expectation it shows, and what counts as tried.
local ITEMS = {
  { "platform", "Platform" },
  { "keyboard", "Keyboard and text", "keyboard" },
  { "pointer", "Mouse and touch", "mouse", "touch" },
  { "gamepad", "Game controllers", "gamepad" },
  { "sensors", "Sensors and vibration", "sensors", "haptics" },
  { "video", "Screen", "fullscreen" },
  { "sound", "Sound" },
  { "storage", "Storage" },
  { "system", "System" },
  { "assets", "Assets: every type LÖVE loads" },
  { "report", "Device report" },
  { "quit", "Quit" },
}

local function tag(s, keys)
  if not keys then return nil end
  local best
  local rank = { need = 3, want = 2, none = 1 }
  for _, k in ipairs(keys) do
    local e = s.profile[k]
    if e and (not best or rank[e] > rank[best]) then best = e end
  end
  return best
end

local function tried(s, id)
  local i = s.input
  if id == "keyboard" then return i.keyCount > 0 end
  if id == "pointer" then return i.mouse.moved or i.touch.seen end
  if id == "gamepad" then
    for _, p in pairs(i.pads) do for _ in pairs(p.buttons) do return true end end
    return false
  end
  if id == "sensors" then return next(i.sensors) ~= nil or s.answers.vibrate ~= nil end
  if id == "video" then return next(s.seen) ~= nil end
  if id == "sound" then return next(s.answers) ~= nil end
  if id == "storage" then return s.storage ~= nil end
  if id == "assets" then return s.assetResults ~= nil end
  if id == "report" then return s.reportFile ~= nil end
  return false
end

local function items(s)
  local out = {}
  for _, it in ipairs(ITEMS) do
    local id, label = it[1], it[2]
    local keys = { it[3], it[4] }
    local e = tag(s, keys)
    local value = e and (e == "none" and "not here" or e) or ""
    local mark = tried(s, id) and "x" or " "
    local v = (id == "quit" or id == "platform") and "" or (value .. " [" .. mark .. "]")
    out[#out + 1] = { label = label, value = v, style = (e == "none") and ST.dim or ST.body }
  end
  return out
end

function P.draw(app, scr, s)
  local list = items(s)
  s.sel.home = s.sel.home or 1
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = #list + 2, title = "What do you want to check?", items = list, sel = s.sel.home, foot = "" })
  local y = #list + 4
  scr:text(1, y, "Running on  " .. s.profile.name .. (s.forced and "  (chosen: not the detected " .. s.platform.name .. ")" or "  (" .. s.platform.certainty .. ")"), ST.strong)
  local lines = screen.wrap(s.profile.note or "", screen.COLS - 4)
  for i = 1, math.min(#lines, 5) do scr:text(2, y + i, lines[i], ST.body) end
  scr:text(1, 20, "need = the platform must have it, want = nice to have,", ST.dim)
  scr:text(1, 21, "not here = not expected. [x] = tried in this run.", ST.dim)
end

local function open(app, s)
  local id = ITEMS[s.sel.home][1]
  if id == "quit" then love.event.quit() return end
  app.go(id)
end

function P.act(app, s, a)
  s.sel.home = s.sel.home or 1
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.home = screen.nav(s.sel.home, #ITEMS, #ITEMS, a) return true end
  if a == "accept" then open(app, s) return true end
  if a == "back" then love.event.quit() return true end
  return false
end

function P.tap(app, s, col, row)
  local i = row - 1
  if col < screen.COLS and i >= 1 and i <= #ITEMS then s.sel.home = i; open(app, s) return true end
  return false
end

return P
