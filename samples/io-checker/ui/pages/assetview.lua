-- One asset, used for real: pictures drawn (the walk strip animated), textures drawn if the GPU can, sound and music played, fonts
-- shown in a sample, video played, the shader applied, scripts run, data shown. Left and right visit the neighbouring assets.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Asset", parent = "assets", foot = "A play/stop  < > neighbour asset  B back" }
local V = {} -- the open asset's live objects
local licences

--- .joyride/assets.toml: the Asset Library's record of each file's licence (not part of an exported game).
local function licenceOf(path)
  if not licences then
    licences = {}
    local text = love.filesystem.read(".joyride/assets.toml")
    if text then
      for block in text:gmatch("%[%[asset%]%](.-)%f[%[%z]") do
        local rec = {}
        for k, v in block:gmatch('(%w+)%s*=%s*"?([^"\n]*)"?') do rec[k] = v end
        if rec.path then licences[rec.path] = rec end
      end
    end
  end
  return licences[path]
end

local function close()
  if V.source then pcall(function() V.source:stop() end) end
  if V.video then pcall(function() V.video:pause() end) end
  love.graphics.setShader()
  V = {}
end

local function lines(text, n)
  local out = {}
  for l in (text or ""):gmatch("[^\r\n]*") do
    if #out >= n then break end
    out[#out + 1] = l
  end
  return out
end

local function open(app, s)
  close()
  local r = s.assetView
  local e = r.entry
  V.t = 0
  V.how = e.how
  local ok, err = pcall(function()
    if r.status == "FAIL" or not e.how then return end
    local lg = love.graphics
    if e.how == "image" and r.obj then
      V.image = lg.newImage(r.obj)
      V.image:setFilter("nearest", "nearest")
      if e.path:find("hero-walk", 1, true) then
        V.frames = {}
        for i = 0, 3 do V.frames[i + 1] = lg.newQuad(i * 160, 0, 160, 128, V.image:getDimensions()) end
      end
    elseif e.how == "image" and not r.obj then
      V.image = lg.newImage(e.path)
    elseif e.how == "texture" and r.obj then
      local ok2, img = pcall(lg.newImage, r.obj)
      if ok2 then V.image = img; V.image:setFilter("nearest", "nearest") else V.gpu = tostring(img) end
    elseif e.how == "audio" then
      V.source = love.audio.newSource(e.path, e.type == "Sound" and "static" or "stream")
    elseif e.how == "font" then
      V.font = lg.newFont(e.path, 24)
    elseif e.how == "bmfont" then
      V.font = lg.newFont(e.path)
    elseif e.how == "imagefont" then
      V.font = lg.newImageFont(e.path, app.core.assets.IMAGEFONT_GLYPHS)
      V.sample = "0123456789 ABCDEF 3.14 1+1-2:0"
    elseif e.how == "video" then
      V.video = lg.newVideo(e.path)
    elseif e.how == "shader" then
      V.shader = lg.newShader(love.filesystem.read(e.path))
      V.image = lg.newImage("assets/images/hero.png")
      V.image:setFilter("nearest", "nearest")
    elseif e.how == "archive" then
      local data = love.filesystem.newFileData(e.path)
      love.filesystem.mount(data, "iochk-view")
      V.members = love.filesystem.getDirectoryItems("iochk-view/pack")
      V.memberText = love.filesystem.read("iochk-view/pack/readme.txt")
      love.filesystem.unmount(data)
    elseif e.how == "lua" then
      V.text = ("The chunk ran and returned: %s"):format(type(r.obj) == "string" and r.obj or type(r.obj))
    else
      V.text = love.filesystem.read(e.path)
    end
  end)
  if not ok then V.error = tostring(err) end
end

function P.enter(app, s) open(app, s) end
function P.leave() close() end

local function neighbour(app, s, step)
  local list = s.assetResults or {}
  local idx = s.sel.assets or 1
  idx = (idx - 1 + step) % #list + 1
  s.sel.assets = idx
  s.assetView = list[idx]
  open(app, s)
end

function P.act(app, s, a)
  if a == "left" then neighbour(app, s, -1) return true end
  if a == "right" then neighbour(app, s, 1) return true end
  if a == "accept" then
    if V.source then
      if V.source:isPlaying() then V.source:stop() else V.source:play() end
    elseif V.video then
      if V.video:isPlaying() then V.video:pause() else V.video:play() end
    end
    return true
  end
  return false
end

function P.update(app, s, dt)
  V.t = (V.t or 0) + dt
end

local STATUS = { PASS = ST.ok, FAIL = ST.error, INFO = ST.warn, SKIP = ST.dim }
local EXPECT = { yes = "should load", no = "is expected to be refused", maybe = "not confirmed on this platform" }

function P.draw(app, scr, s)
  local r = s.assetView
  local e = r.entry
  scr:box(0, 1, screen.COLS, 10, e.type, ST.frame, true)
  scr:text(2, 2, screen.fit(e.path, screen.COLS - 4), ST.strong)
  scr:text(2, 3, ("Loaded with: %s   Expected: %s"):format(e.how or "?", EXPECT[r.expect or "yes"] or "?"), ST.body)
  local detail = screen.wrap(("Result: %s"):format(r.detail or "not run"), screen.COLS - 4)
  for i = 1, math.min(2, #detail) do scr:text(2, 3 + i, detail[i], STATUS[r.status] or ST.body) end
  local lic = licenceOf(e.path)
  if lic then
    scr:text(2, 6, screen.fit(("Licence: %s (%s)"):format(lic.licence or "?", lic.confirmed == "true" and "confirmed" or "NOT confirmed"), screen.COLS - 4), lic.confirmed == "true" and ST.ok or ST.error)
    if lic.author and lic.author ~= "" then scr:text(2, 7, screen.fit("Author: " .. lic.author, screen.COLS - 4), ST.body) end
    if lic.source and lic.source ~= "" then scr:text(2, 8, screen.fit("Source: " .. lic.source, screen.COLS - 4), ST.dim) end
  else
    scr:text(2, 6, "Licence: no .joyride/assets.toml here (it is not exported)", ST.dim)
  end
  scr:text(2, 9, ("Size: %s"):format(love.filesystem.getInfo(e.path) and (love.filesystem.getInfo(e.path).size .. " bytes") or "?"), ST.dim)

  scr:box(0, 11, screen.COLS, 11, "Use it", ST.frame)
  local x, y, w, h = 16, 12 * 16, screen.W - 32, 9 * 16
  if V.error then
    for i, l in ipairs(screen.wrap("Could not use it: " .. V.error, screen.COLS - 4)) do if i <= 8 then scr:text(2, 11 + i, l, ST.error) end end
    return
  end
  if r.status == "FAIL" then scr:text(2, 12, "It did not load, so there is nothing to show.", ST.error) return end
  if r.status == "SKIP" then scr:text(2, 12, r.detail or "Skipped.", ST.dim) return end
  local how = e.how
  if how == "image" or how == "texture" or how == "shader" then
    if V.gpu then
      for i, l in ipairs(screen.wrap("This GPU cannot draw the format (the file itself loaded): " .. V.gpu, screen.COLS - 4)) do if i <= 6 then scr:text(2, 11 + i, l, ST.warn) end end
    elseif V.image then
      app.overlay = { x = x, y = y, w = w, h = h, fn = function(ow, oh)
        local lg = love.graphics
        local iw, ih = V.image:getDimensions()
        lg.setColor(1, 1, 1, 1)
        if V.frames then
          local f = math.floor(V.t * 8) % 4 + 1
          local sc = math.min(ow / 160, oh / 128)
          lg.draw(V.image, V.frames[f], (ow - 160 * sc) / 2, (oh - 128 * sc) / 2, 0, sc, sc)
        else
          local sc = math.min(ow / iw, oh / ih)
          if sc >= 1 then sc = math.floor(sc) end
          if V.shader then
            V.shader:send("tint", { 1, 0.6, 0.2, 0.8 })
            V.shader:send("wobble", 5 * math.sin(V.t * 3))
            lg.setShader(V.shader)
          end
          lg.draw(V.image, (ow - iw * sc) / 2, (oh - ih * sc) / 2, 0, sc, sc)
          lg.setShader()
        end
      end }
    end
  elseif how == "audio" and V.source then
    local playing = V.source:isPlaying()
    local len = V.source:getDuration("seconds") or 0
    local pos = V.source:tell("seconds") or 0
    scr:text(2, 12, playing and "Playing. A stops it." or "Stopped. A plays it.", playing and ST.ok or ST.body)
    if len > 0 then
      local bar = math.floor((screen.COLS - 8) * math.min(1, pos / len))
      scr:text(2, 14, ("%5.1f / %.1f s"):format(pos, len), ST.body)
      scr:text(2, 15, "[" .. string.rep("█", bar) .. string.rep("░", screen.COLS - 8 - bar) .. "]", ST.info)
    end
  elseif (how == "font" or how == "bmfont" or how == "imagefont") and V.font then
    app.overlay = { x = x, y = y, w = w, h = h, fn = function(ow)
      local lg = love.graphics
      lg.setFont(V.font)
      lg.setColor(1, 1, 1, 1)
      lg.printf(V.sample or "The quick brown fox jumps over the lazy dog. 0123456789 !?&", 0, 0, ow, "left")
      lg.setFont(app.font)
    end }
  elseif how == "video" and V.video then
    local playing = V.video:isPlaying()
    scr:text(2, 12, playing and "Playing. A pauses." or "Paused. A plays.", playing and ST.ok or ST.body)
    app.overlay = { x = x, y = y + 32, w = w, h = h - 32, fn = function(ow, oh)
      local lg = love.graphics
      local vw, vh = V.video:getDimensions()
      local sc = math.min(ow / vw, oh / vh)
      lg.setColor(1, 1, 1, 1)
      lg.draw(V.video, (ow - vw * sc) / 2, 0, 0, sc, sc)
    end }
  elseif how == "archive" then
    scr:text(2, 12, "Mounted with love.filesystem.mount; unmounted again.", ST.body)
    scr:text(2, 13, "Members: " .. table.concat(V.members or {}, ", "), ST.strong)
    for i, l in ipairs(screen.wrap(V.memberText or "", screen.COLS - 4)) do if i <= 3 then scr:text(2, 14 + i, l, ST.dim) end end
  elseif V.text then
    local shown = lines(V.text, 8)
    for i, l in ipairs(shown) do scr:text(2, 11 + i, screen.fit(l, screen.COLS - 4), ST.body) end
  end
end

return P
