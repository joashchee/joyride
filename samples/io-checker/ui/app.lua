-- The screens: a 48x25 text-mode grid drawn with the VGA font, scaled to the window, driven by every kind of input at once.
-- Keyboard, game controller, mouse and touch all become the same few actions (up, down, left, right, accept, back, pgup, pgdn, x,
-- y), so the same app is usable on a laptop, a handheld, a phone and a web page. The input pages are the exception: they record
-- everything and navigate nothing (see `raw`), and are left by holding Escape, Select+Start, or the bar at the bottom for a second.

local screen = require("ui.screen")
local ST = screen.ST

local App = { hits = {}, page = nil }

local PAGE_IDS = { "home", "platform", "keyboard", "pointer", "gamepad", "sensors", "video", "pattern", "sound", "storage", "system", "assets", "assetview", "report" }

-- ---- actions from devices -----------------------------------------------------------------------------------------------------

local KEYS = {
  up = "up", down = "down", left = "left", right = "right", ["return"] = "accept", kpenter = "accept", space = "accept", z = "accept",
  escape = "back", backspace = "back", x = "back", pageup = "pgup", pagedown = "pgdn", h = "x", c = "x", n = "y", v = "y",
}
local GAMEPAD = { dpup = "up", dpdown = "down", dpleft = "left", dpright = "right", a = "accept", b = "back", x = "x", y = "y", leftshoulder = "pgup", rightshoulder = "pgdn" }
local RAW = { [1] = "accept", [2] = "back", [3] = "x", [4] = "y", [5] = "pgup", [6] = "pgdn" }
local HAT = { u = "up", d = "down", l = "left", r = "right" }

-- ---- the app --------------------------------------------------------------------------------------------------------------------

function App.say(text, kind) local s = App.s; s.reply, s.kind = text, kind or "hint" end

function App.page_() return App.pages[App.s.page] end

function App.go(id)
  local s = App.s
  local old = App.pages[s.page]
  if old and old.leave then old.leave(App, s) end
  s.page = id
  s.chord.t = 0
  s.idle:touch()
  App.overlay = nil
  local p = App.pages[id]
  if p.enter then p.enter(App, s) end
end

--- Run `fn` after the screen has shown `text` (a slow check must not leave a frozen-looking screen).
function App.job(text, fn)
  App.say(text, "warn")
  App.pending = { fn = fn, shown = false }
end

--- One action from any device. Returns true when it was used.
function App.act(a)
  local s = App.s
  local p = App.pages[s.page]
  s.idle:touch()
  if p.raw then return false end
  if p.act and p.act(App, s, a) then return true end
  if a == "back" then
    App.go(p.parent or "home")
    return true
  end
  return false
end

-- ---- layout: the grid scaled into the window, centred ---------------------------------------------------------------------------

function App.layout()
  local w, h = love.graphics.getDimensions()
  local sc = math.min(w / screen.W, h / screen.H)
  if sc >= 1 then sc = math.floor(sc) end
  App.scale = sc
  App.ox = math.floor((w - screen.W * sc) / 2)
  App.oy = math.floor((h - screen.H * sc) / 2)
end

--- Window pixels to a cell (col, row), or nil outside the grid.
function App.cellAt(px, py)
  local x, y = (px - App.ox) / App.scale, (py - App.oy) / App.scale
  if x < 0 or y < 0 or x >= screen.W or y >= screen.H then return nil end
  return math.floor(x / screen.CW), math.floor(y / screen.CH)
end

-- ---- soft keys (rows 23 and 24): the way a mouse or a finger does what the keys do ---------------------------------------------

local SOFT = {
  { "<", "left" }, { "^", "up" }, { "v", "down" }, { ">", "right" }, { "Pg-", "pgup" }, { "Pg+", "pgdn" }, { "OK", "accept" }, { "Back", "back" },
}

local function drawSoft(scr, s, p)
  App.hits = {}
  local row = screen.ROWS - 1
  if p.raw then
    local label = "Hold here, or Esc, or Select+Start, to leave"
    local pct = s.chord.t > 0 and s.chord:progress() or 0
    local w = screen.COLS
    local lit = math.floor(w * pct)
    scr:text(0, row, screen.pad(" " .. label, w), ST.key)
    if lit > 0 then scr:text(0, row, screen.pad(" " .. label, w):sub(1, lit), ST.keydown) end
    App.hits[#App.hits + 1] = { x = 0, y = row, w = w, h = 1, leave = true }
    if p.soft then
      local x = 0
      for _, b in ipairs(p.soft) do
        local text = "[" .. b[1] .. "]"
        scr:text(x, row - 1, text, ST.key)
        App.hits[#App.hits + 1] = { x = x, y = row - 1, w = #text, h = 1, fn = b[2] }
        x = x + #text + 1
      end
    end
    return
  end
  local x = 0
  for _, b in ipairs(SOFT) do
    local text = "[" .. b[1] .. "]"
    scr:text(x, row, text, ST.key)
    App.hits[#App.hits + 1] = { x = x, y = row, w = #text, h = 1, action = b[2] }
    x = x + #text + 1
  end
end

--- A tap or click at a cell: soft keys first, then the page's own rows.
function App.tap(col, row)
  local s = App.s
  local p = App.pages[s.page]
  for _, h in ipairs(App.hits) do
    if col >= h.x and col < h.x + h.w and row >= h.y and row < h.y + h.h then
      if h.fn then h.fn(App, s) elseif h.action then App.act(h.action) end
      return true
    end
  end
  if p.tap and not p.raw then return p.tap(App, s, col, row) end
  return false
end

-- ---- drawing -----------------------------------------------------------------------------------------------------------------------

local function drawCells(scr)
  local lg = love.graphics
  local font = App.font
  lg.setFont(font)
  for y = 0, screen.ROWS - 1 do
    local row = scr.cells[y]
    -- Backgrounds in runs.
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
    -- Text in runs of one colour (a cell at a time when the fallback font is not monospaced).
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
  scr:text(1, 0, "IO Checker", ST.title)
  scr:text(13, 0, "> " .. (p.title or s.page), ST.title)
  local who = s.profile.name .. (s.forced and "*" or "")
  scr:text(screen.COLS - #screen.chars(who) - 1, 0, who, ST.title)
end

local function drawReply(scr, s)
  local st = ST[s.kind] or ST.hint
  scr:text(0, 22, screen.pad(" " .. screen.fit(s.reply or "", screen.COLS - 2), screen.COLS), st)
end

function App.render()
  local s = App.s
  local scr = App.scr
  local p = App.pages[s.page]
  scr:clear()
  App.overlay = nil
  p.draw(App, scr, s)
  drawTopBar(scr, s)
  drawReply(scr, s)
  if p.foot then scr:text(0, 23, screen.pad(" " .. p.foot, screen.COLS), ST.dim) end
  drawSoft(scr, s, p)
end

function App.draw()
  local lg = love.graphics
  local s = App.s
  local p = App.pages[s.page]
  if p.fullscreen then
    p.drawFull(App, s)
    return
  end
  App.layout()
  App.render()
  lg.clear(0, 0, 0, 1)
  lg.push()
  lg.translate(App.ox, App.oy)
  lg.scale(App.scale)
  drawCells(App.scr)
  if App.overlay then
    lg.setScissor(App.ox + App.overlay.x * App.scale, App.oy + App.overlay.y * App.scale, App.overlay.w * App.scale, App.overlay.h * App.scale)
    lg.push()
    lg.translate(App.overlay.x, App.overlay.y)
    App.overlay.fn(App.overlay.w, App.overlay.h)
    lg.pop()
    lg.setScissor()
  end
  lg.pop()
  lg.setColor(1, 1, 1, 1)
  if App.pending then App.pending.shown = true end
end

-- ---- install: wire LÖVE's callbacks to it --------------------------------------------------------------------------------------------

function App.install(core)
  App.core = core
  local s = core.state
  App.s = s
  App.scr = screen.new()
  App.scr.ascii = false
  -- The VGA font from the project's own assets (this is the Asset Library's CC BY-SA font at work); the default font if it cannot load.
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
  core.pollJoysticks(s)
  App.go("home")

  function love.draw() App.draw() end

  function love.update(dt)
    s.dt = dt
    local p = App.pages[s.page]
    -- A slow job runs on the frame after its message was drawn.
    if App.pending and App.pending.shown then
      local job = App.pending
      App.pending = nil
      job.fn()
    end
    core.pollJoysticks(s)
    -- The leave chord: Escape, Select+Start (or Back+Start on a mapped pad), or the bar held with a pointer.
    local held = false
    if p.raw then
      held = love.keyboard.isDown("escape") or App.leaveHeld
      for _, j in ipairs(s.joysticks) do
        if j.gamepad and j.obj:isGamepadDown("back") and j.obj:isGamepadDown("start") then held = true end
        if j.buttons >= 14 and j.down(13) and j.down(14) then held = true end
      end
      s.idle:update(dt)
      if s.idle:expired() then App.say("No input for 15 s: back to the list.", "warn"); App.go(p.parent or "home") return end
    end
    if s.chord:update(dt, held) then s.chord.t = 0; App.say("Back."); App.go(p.parent or "home") return end
    if p.update then p.update(App, s, dt) end
  end

  local function touchIdle() s.idle:touch() end

  function love.keypressed(k, _, isrepeat)
    s.input:key(k, isrepeat)
    touchIdle()
    local p = App.pages[s.page]
    if p.key and p.key(App, s, k) then return end
    local a = KEYS[k]
    if a and not p.raw then App.act(a) end
  end
  function love.keyreleased(k) s.input:note("key " .. k .. " up") end
  function love.textinput(t) s.input:addText(t); touchIdle() end

  function love.mousepressed(x, y, b, istouch)
    s.input:mouseButton(b, true, istouch)
    touchIdle()
    local pg = App.pages[s.page]
    if pg.fullscreen and pg.tapPixel then pg.tapPixel(App, s, x) return end
    local col, row = App.cellAt(x, y)
    if col then
      for _, h in ipairs(App.hits) do
        if h.leave and col >= h.x and col < h.x + h.w and row >= h.y then App.leaveHeld = true end
      end
      if not App.leaveHeld then App.tap(col, row) end
    end
  end
  function love.mousereleased(x, y, b, istouch) s.input:mouseButton(b, false, istouch); App.leaveHeld = false end
  function love.mousemoved(x, y) s.input:mouseMoved(x, y) end
  function love.wheelmoved(dx, dy) s.input:wheel(dx, dy); touchIdle() end
  function love.touchpressed(id, x, y, _, _, pressure) s.input:touchDown(id, x, y, pressure); touchIdle() end
  function love.touchmoved(id, x, y) s.input:touchMove(id, x, y); touchIdle() end
  function love.touchreleased(id) s.input:touchUp(id); touchIdle() end

  function love.joystickadded(j) core.pollJoysticks(s); s.input:note("pad added: " .. j:getName()); App.say("Controller connected: " .. j:getName(), "ok"); touchIdle() end
  function love.joystickremoved(j) core.pollJoysticks(s); s.input:note("pad removed"); App.say("Controller removed: " .. j:getName(), "warn") end
  function love.joystickpressed(j, b)
    core.pollJoysticks(s)
    s.input:padPress(j:getID(), b, love.timer.getTime())
    touchIdle()
    if not j:isGamepad() and RAW[b] then App.act(RAW[b]) end
  end
  function love.joystickreleased(j, b) s.input:padRelease(j:getID(), b, love.timer.getTime()) end
  function love.joystickaxis(j, a, v) s.input:padAxis(j:getID(), a, v); if math.abs(v) > 0.5 then touchIdle() end end
  function love.joystickhat(j, h, dir)
    s.input:padHat(j:getID(), h, dir)
    touchIdle()
    if not j:isGamepad() and HAT[dir] then App.act(HAT[dir]) end
  end
  function love.gamepadpressed(j, button)
    local a = GAMEPAD[button]
    if a then App.act(a) end
  end
  function love.resize() App.layout() end
end

-- Exposed so a test can drive the same mapping.
App.KEYS, App.GAMEPAD = KEYS, GAMEPAD

return App
