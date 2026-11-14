-- The screens: a 48x25 text-mode grid drawn with the VGA font, scaled to the window, driven by every kind of input at once.
-- Keyboard, game controller, mouse and touch all become the same few actions (up, down, left, right, accept, back, pgup, pgdn, x, y),
-- so the same app is usable on a laptop, a handheld, a phone and a web page. (The grid, the actions and the soft keys are the IO
-- Checker's: this is the same kind of app.)

local screen = require("ui.screen")
local ST = screen.ST

local App = { hits = {}, page = nil }

local PAGE_IDS = { "home", "run", "group", "libraries", "specialists", "platform", "report" }

local KEYS = {
  up = "up", down = "down", left = "left", right = "right", ["return"] = "accept", kpenter = "accept", space = "accept", z = "accept",
  escape = "back", backspace = "back", pageup = "pgup", pagedown = "pgdn", x = "x", ["="] = "x", ["+"] = "x", kp_plus = "x", c = "y",
  ["-"] = "y", kp_minus = "y", v = "y", delete = "clear", ["0"] = "clear",
}
local GAMEPAD = { dpup = "up", dpdown = "down", dpleft = "left", dpright = "right", a = "accept", b = "back", x = "x", y = "y", leftshoulder = "pgup", rightshoulder = "pgdn", back = "clear" }
local RAW = { [1] = "accept", [2] = "back", [3] = "x", [4] = "y", [5] = "pgup", [6] = "pgdn" }
local HAT = { u = "up", d = "down", l = "left", r = "right" }

function App.say(text, kind) local s = App.s; s.reply, s.kind = text, kind or "hint" end

function App.go(id)
  local s = App.s
  local old = App.pages[s.page]
  if old and old.leave then old.leave(App, s) end
  s.page = id
  s.idle:touch()
  local p = App.pages[id]
  if p.enter then p.enter(App, s) end
end

--- Run `fn` after the screen has shown `text` (a slow step must not leave a frozen-looking screen).
function App.job(text, fn)
  App.say(text, "warn")
  App.pending = { fn = fn, shown = false }
end

--- One action from any device. Returns true when it was used.
function App.act(a)
  local s = App.s
  local p = App.pages[s.page]
  s.idle:touch()
  if p.act and p.act(App, s, a) then return true end
  if a == "back" then
    App.go(p.parent or "home")
    return true
  end
  return false
end

-- ---- layout -----------------------------------------------------------------------------------------------------------------

function App.layout()
  local w, h = love.graphics.getDimensions()
  local sc = math.min(w / screen.W, h / screen.H)
  if sc >= 1 then sc = math.floor(sc) end
  App.scale = sc
  App.ox = math.floor((w - screen.W * sc) / 2)
  App.oy = math.floor((h - screen.H * sc) / 2)
end

function App.cellAt(px, py)
  local x, y = (px - App.ox) / App.scale, (py - App.oy) / App.scale
  if x < 0 or y < 0 or x >= screen.W or y >= screen.H then return nil end
  return math.floor(x / screen.CW), math.floor(y / screen.CH)
end

-- ---- soft keys (rows 23 and 24): the way a mouse or a finger does what the keys do ----------------------------------------------

local SOFT = { { "<", "left" }, { "^", "up" }, { "v", "down" }, { ">", "right" }, { "Pg-", "pgup" }, { "Pg+", "pgdn" }, { "OK", "accept" }, { "Back", "back" } }

local function drawSoft(scr, s, p)
  App.hits = {}
  local row = screen.ROWS - 1
  if p.soft then
    local bx = 0
    scr:text(0, row - 1, string.rep(" ", screen.COLS), ST.body)
    for _, b in ipairs(p.soft) do
      local text = "[" .. b[1] .. "]"
      scr:text(bx, row - 1, text, ST.key)
      App.hits[#App.hits + 1] = { x = bx, y = row - 1, w = #text, h = 1, fn = b[2] }
      bx = bx + #text + 1
    end
  end
  local x = 0
  for _, b in ipairs(SOFT) do
    local text = "[" .. b[1] .. "]"
    scr:text(x, row, text, ST.key)
    App.hits[#App.hits + 1] = { x = x, y = row, w = #text, h = 1, action = b[2] }
    x = x + #text + 1
  end
end

function App.tap(col, row)
  local s = App.s
  local p = App.pages[s.page]
  for _, h in ipairs(App.hits) do
    if col >= h.x and col < h.x + h.w and row >= h.y and row < h.y + h.h then
      if h.fn then h.fn(App, s) elseif h.action then App.act(h.action) end
      return true
    end
  end
  if p.tap then return p.tap(App, s, col, row) end
  return false
end

-- ---- drawing ----------------------------------------------------------------------------------------------------------------

local function drawCells(scr)
  local lg = love.graphics
  lg.setFont(App.font)
  for y = 0, screen.ROWS - 1 do
    local row = scr.cells[y]
    local x = 0
    while x < screen.COLS do
      local bg = row[x][3]
      local x2 = x
      while x2 + 1 < screen.COLS and row[x2 + 1][3] == bg do x2 = x2 + 1 end
      if bg ~= 0 then
        local c = screen.RGB[bg]
        lg.setColor(c[1], c[2], c[3], 1)
        lg.rectangle("fill", x * screen.CW, y * screen.CH, (x2 - x + 1) * screen.CW, screen.CH)
      end
      x = x2 + 1
    end
    x = 0
    while x < screen.COLS do
      local fg = row[x][2]
      local x2, buf = x, { row[x][1] }
      if App.mono then
        while x2 + 1 < screen.COLS and row[x2 + 1][2] == fg do x2 = x2 + 1; buf[#buf + 1] = row[x2][1] end
      end
      local text = table.concat(buf)
      if text:find("%S") then
        local c = screen.RGB[fg]
        lg.setColor(c[1], c[2], c[3], 1)
        lg.print(text, x * screen.CW, y * screen.CH)
      end
      x = x2 + 1
    end
  end
end

local function drawTopBar(scr, s)
  local p = App.pages[s.page]
  scr:fill(0, 0, screen.COLS, 1, ST.title)
  scr:text(1, 0, "Benchmark Libraries", ST.title)
  scr:text(22, 0, "> " .. (p.title or s.page), ST.title)
  local who = s.profile.name .. (s.forced and "*" or "")
  scr:text(screen.COLS - #screen.chars(who) - 1, 0, who, ST.title)
end

local function drawReply(scr, s)
  scr:text(0, 22, screen.pad(" " .. screen.fit(s.reply or "", screen.COLS - 2), screen.COLS), ST[s.kind] or ST.hint)
end

function App.render()
  local s = App.s
  local scr = App.scr
  local p = App.pages[s.page]
  scr:clear()
  p.draw(App, scr, s)
  drawTopBar(scr, s)
  drawReply(scr, s)
  local foot = type(p.foot) == "function" and p.foot(App, s) or p.foot
  if foot then scr:text(0, 23, screen.pad(" " .. foot, screen.COLS), ST.dim) end
  drawSoft(scr, s, p)
end

function App.draw()
  local lg = love.graphics
  App.layout()
  App.render()
  lg.clear(0, 0, 0, 1)
  lg.push()
  lg.translate(App.ox, App.oy)
  lg.scale(App.scale)
  drawCells(App.scr)
  lg.pop()
  lg.setColor(1, 1, 1, 1)
  if App.pending then App.pending.shown = true end
end

-- ---- install: wire LÖVE's callbacks to it -------------------------------------------------------------------------------------------

function App.install(core)
  App.core = core
  local s = core.state
  App.s = s
  App.scr = screen.new()
  local ok, font = pcall(love.graphics.newFont, "assets/fonts/vga.woff", 16, "mono", 1)
  if ok and font then
    font:setFilter("nearest", "nearest")
    App.font, App.mono = font, true
  else
    App.font, App.mono, App.scr.ascii = love.graphics.newFont(16), false, true
    s.fontError = tostring(font)
  end
  App.pages = {}
  for _, id in ipairs(PAGE_IDS) do App.pages[id] = require("ui.pages." .. id) end
  love.graphics.setBackgroundColor(0, 0, 0)
  love.keyboard.setKeyRepeat(true)
  App.go("home")

  function love.draw() App.draw() end

  function love.update(dt)
    s.dt = dt
    local p = App.pages[s.page]
    if App.pending and App.pending.shown then
      local job = App.pending
      App.pending = nil
      job.fn()
    end
    if p.update then p.update(App, s, dt) end
  end

  function love.keypressed(k, _, isrepeat)
    s.idle:touch()
    local p = App.pages[s.page]
    if p.key and p.key(App, s, k, isrepeat) then return end
    local a = KEYS[k]
    if a then
      if isrepeat and (a == "back" or a == "accept" or a == "x" or a == "y" or a == "clear") then return end
      App.act(a)
    end
  end

  function love.mousepressed(x, y, b)
    s.idle:touch()
    local col, row = App.cellAt(x, y)
    if col then App.tap(col, row) end
  end

  function love.joystickpressed(j, b)
    s.idle:touch()
    if not j:isGamepad() and RAW[b] then App.act(RAW[b]) end
  end
  function love.joystickhat(j, h, dir)
    if not j:isGamepad() and HAT[dir] then App.act(HAT[dir]) end
  end
  function love.gamepadpressed(j, button)
    local a = GAMEPAD[button]
    if a then App.act(a) end
  end
  function love.resize() App.layout() end
end

App.KEYS, App.GAMEPAD = KEYS, GAMEPAD

return App
