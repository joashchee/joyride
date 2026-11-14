-- A full-window test pattern in real pixels (the grid is not used): a panel test has to light every pixel in a known colour.
-- Left and right (or a tap on either half) change the pattern; accept or back (or a tap in the middle) returns.
local screen = require("ui.screen")

local P = { title = "Pattern", fullscreen = true, parent = "video" }

local SOLID = { white = { 1, 1, 1 }, red = { 1, 0, 0 }, green = { 0, 1, 0 }, blue = { 0, 0, 1 }, black = { 0, 0, 0 } }
local BARS = { { 1, 1, 1 }, { 1, 1, 0 }, { 0, 1, 1 }, { 0, 1, 0 }, { 1, 0, 1 }, { 1, 0, 0 }, { 0, 0, 1 }, { 0, 0, 0 } }
local motion = { x = 0, n = 0 }
local checker

local function rect(r, g, b, x, y, w, h)
  love.graphics.setColor(r, g, b, 1)
  love.graphics.rectangle("fill", x, y, w, h)
end

local function show(app, s, i)
  local n = #app.core.model.PATTERNS
  s.sel.video = (i - 1) % n + 1
  local p = app.core.model.PATTERNS[s.sel.video]
  s.seen[p.id] = true
  if p.id == "motion" then s.frames:reset(); motion.x, motion.n = 0, 0 end
end

function P.enter(app, s) show(app, s, s.sel.video or 1) end

function P.leave(app, s)
  local st = s.frames:stats()
  if st then app.say("Measured: " .. app.core.model.framesLine(st), (st.fps >= 55) and "ok" or "warn") end
end

function P.update(app, s, dt)
  if app.core.model.PATTERNS[s.sel.video].id == "motion" then s.frames:add(dt) end
end

function P.act(app, s, a)
  if a == "left" or a == "up" or a == "pgup" then show(app, s, s.sel.video - 1) return true end
  if a == "right" or a == "down" or a == "pgdn" then show(app, s, s.sel.video + 1) return true end
  if a == "accept" or a == "back" then app.go("video") return true end
  return false
end

--- A tap: left third back a pattern, right third forward, the middle leaves.
function P.tapPixel(app, s, x)
  local w = love.graphics.getWidth()
  if x < w / 3 then return P.act(app, s, "left") end
  if x > w * 2 / 3 then return P.act(app, s, "right") end
  return P.act(app, s, "back")
end

function P.drawFull(app, s)
  local lg = love.graphics
  local model = app.core.model
  local p = model.PATTERNS[s.sel.video]
  local W, H = lg.getDimensions()
  lg.origin()
  lg.setScissor()
  lg.clear(0, 0, 0, 1)
  if SOLID[p.id] then
    local c = SOLID[p.id]
    lg.clear(c[1], c[2], c[3], 1)
  elseif p.id == "bars" then
    for i, c in ipairs(BARS) do rect(c[1], c[2], c[3], (i - 1) * W / 8, 0, math.ceil(W / 8), H * 2 / 3) end
    for i = 0, 15 do
      local c = screen.RGB[i]
      rect(c[1], c[2], c[3], i * W / 16, H * 2 / 3, math.ceil(W / 16), H / 3)
    end
  elseif p.id == "ramps" then
    for x = 0, W - 1 do
      local v = x / math.max(1, W - 1)
      rect(v, v, v, x, 0, 1, H / 4); rect(v, 0, 0, x, H / 4, 1, H / 4); rect(0, v, 0, x, H / 2, 1, H / 4); rect(0, 0, v, x, H * 3 / 4, 1, H / 4)
    end
  elseif p.id == "grid" then
    if not checker or checker:getWidth() ~= math.floor(W / 2) then
      local d = love.image.newImageData(math.max(1, math.floor(W / 2)), H)
      for y = 0, H - 1 do for x = 0, d:getWidth() - 1 do
        local on = (x + y) % 2 == 0 and 1 or 0
        d:setPixel(x, y, on, on, on, 1)
      end end
      checker = lg.newImage(d)
      checker:setFilter("nearest", "nearest")
    end
    lg.setColor(1, 1, 1, 1)
    lg.draw(checker, 0, 0)
    for x = math.floor(W / 2), W - 1, 10 do rect(0.5, 0.5, 0.5, x, 0, 1, H) end
    for y = 0, H - 1, 10 do rect(0.5, 0.5, 0.5, math.floor(W / 2), y, W, 1) end
    rect(1, 1, 1, 0, 0, W, 1); rect(1, 1, 1, 0, H - 1, W, 1); rect(1, 1, 1, 0, 0, 1, H); rect(1, 1, 1, W - 1, 0, 1, H)
    rect(1, 0, 0, 0, 0, 4, 4); rect(1, 0, 0, W - 4, 0, 4, 4); rect(1, 0, 0, 0, H - 4, 4, 4); rect(1, 0, 0, W - 4, H - 4, 4, 4)
  elseif p.id == "safe" then
    lg.clear(0.1, 0.1, 0.2, 1)
    local sx, sy, sw, sh = 0, 0, W, H
    if love.window.getSafeArea then sx, sy, sw, sh = love.window.getSafeArea() end
    local scale = love.window.getDPIScale and love.window.getDPIScale() or 1
    sx, sy, sw, sh = sx * scale, sy * scale, sw * scale, sh * scale
    if sx == 0 and sy == 0 and sw >= W and sh >= H then sx, sy, sw, sh = W * 0.05, H * 0.05, W * 0.9, H * 0.9 end -- nothing reported: show 5%
    lg.setColor(0.2, 0.6, 0.2, 1)
    lg.rectangle("line", sx + 1, sy + 1, sw - 2, sh - 2)
    lg.setColor(1, 1, 1, 1)
    local r = math.min(W, H) * 0.35
    lg.setLineWidth(3)
    lg.circle("line", W / 2, H / 2, r)
    lg.line(W / 2 - r, H / 2, W / 2 + r, H / 2)
    lg.line(W / 2, H / 2 - r, W / 2, H / 2 + r)
    lg.setLineWidth(1)
  elseif p.id == "motion" then
    motion.n = motion.n + 1
    motion.x = (motion.x + 8) % W
    rect(1, 1, 1, motion.x, 0, 16, H - 40)
    for i = 0, 59 do rect(0.33, 0.33, 0.33, i * 10 + 20, H - 28, 8, 16) end
    rect(1, 1, 0, (motion.n % 60) * 10 + 20, H - 28, 8, 16)
    local st = s.frames:stats()
    lg.setColor(1, 1, 1, 1)
    lg.setFont(app.font)
    lg.print(st and ("%.1f fps"):format(st.fps) or "measuring...", 8, 8)
  end
  lg.setColor(1, 1, 1, 1)
  -- The pattern's own name for a moment, so it is clear what is being looked at; tap the middle to leave.
  if (s.patternShown or 0) ~= s.sel.video then s.patternShown = s.sel.video; s.patternT = 2 end
  s.patternT = (s.patternT or 0) - (s.dt or 0)
  if s.patternT > 0 and p.id ~= "motion" then
    lg.setFont(app.font)
    local text = p.label .. "  (< > change, middle or Esc: leave)"
    local tw = app.font:getWidth(text)
    lg.setColor(0, 0, 0, 0.7)
    lg.rectangle("fill", 0, 0, tw + 16, 24)
    lg.setColor(1, 1, 1, 1)
    lg.print(text, 8, 4)
  end
end

return P
