-- The asset types LÖVE loads, one or more files of each, and a check that loads each the way a game would. This is what exercises
-- Joyride's Asset Library: every file under assets/ is one the library can track (kind, licence, hash in .joyride/assets.toml),
-- and the checks prove LÖVE can use it on the platform that is running.
--
-- A check returns ok, detail. "yes" / "no" / "maybe" is what the platform and LÖVE version are expected to do with the file:
--   a "no" that refuses is a PASS (LÖVE 11.5 is meant to refuse WebP and QOI), a "maybe" is only noted.
-- Checks that need a window (Image, Font, Shader, Video drawn) are skipped when the graphics module is off (love . --report).

local A = {}

-- type: the LÖVE asset family (the Assets page groups by it); how: the loader below.
A.LIST = {
  { path = "assets/images/hero.png", type = "Image", how = "image", note = "Painter sprite" },
  { path = "assets/images/hero.jpg", type = "Image", how = "image" },
  { path = "assets/images/hero.bmp", type = "Image", how = "image" },
  { path = "assets/images/hero.tga", type = "Image", how = "image" },
  { path = "assets/images/hero.hdr", type = "Image", how = "image", web = "maybe", note = "Radiance HDR: float pixels" },
  { path = "assets/textures/scanline.exr", type = "Image", how = "image", web = "maybe", note = "OpenEXR (scanline; a tiled one aborts LÖVE)" },
  { path = "assets/images/hero.webp", type = "Image", how = "image", only12 = true, note = "LÖVE 12.0 only" },
  { path = "assets/images/hero.qoi", type = "Image", how = "image", only12 = true, note = "LÖVE 12.0 only" },
  { path = "assets/images/hero-walk.png", type = "Spritesheet", how = "image", note = "4 frames of the walk clip" },
  { path = "assets/images/tiles.png", type = "Tileset", how = "image", note = "4 tiles of 64x64" },
  { path = "assets/textures/bc1.dds", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/bc3.dds", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/bc7.dds", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/etc2.ktx", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/etc1.pkm", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/pvrtc.pvr", type = "Compressed texture", how = "texture" },
  { path = "assets/textures/astc4.astc", type = "Compressed texture", how = "texture" },
  { path = "assets/audio/jingle.wav", type = "Sound", how = "audio" },
  { path = "assets/audio/jingle.ogg", type = "Sound", how = "audio" },
  { path = "assets/audio/jingle.mp3", type = "Sound", how = "audio" },
  { path = "assets/audio/jingle.flac", type = "Sound", how = "audio" },
  { path = "assets/audio/theme.mid", type = "Music (MIDI)", how = "audio", webMaybe = true },
  { path = "assets/audio/protracker.mod", type = "Music (tracker)", how = "audio", webMaybe = true },
  { path = "assets/audio/fasttracker2.xm", type = "Music (tracker)", how = "audio", webMaybe = true },
  { path = "assets/audio/impulsetracker.it", type = "Music (tracker)", how = "audio", webMaybe = true },
  { path = "assets/audio/screamtracker.s3m", type = "Music (tracker)", how = "audio", webMaybe = true },
  { path = "assets/fonts/vga.woff", type = "Font (TrueType)", how = "font", note = "also this app's own font" },
  { path = "assets/fonts/SourceCodePro-Regular.otf", type = "Font (OpenType)", how = "font", note = "CFF outlines" },
  { path = "assets/fonts/corpus.ttf", type = "Font (TrueType)", how = "font" },
  { path = "assets/fonts/corpus.ttc", type = "Font (collection)", how = "font" },
  { path = "assets/fonts/vga-bmfont.fnt", type = "Font (BMFont)", how = "bmfont", companions = { "assets/fonts/vga-bmfont.png" }, note = "its page, vga-bmfont.png, sits beside it" },
  { path = "assets/fonts/hex-imagefont.png", type = "Font (image)", how = "imagefont", note = "a 3x5 hex font" },
  { path = "assets/video/clip.ogv", type = "Video", how = "video", web = "maybe", note = "Ogg Theora, no sound" },
  { path = "assets/shaders/tint.glsl", type = "Shader", how = "shader", note = "vertex and pixel stage in one file" },
  { path = "assets/lib/json.lua", type = "Script", how = "lua" },
  { path = "assets/lib/hello.luac", type = "Script (bytecode)", how = "lua", bytecode = true, note = "LuaJIT bytecode" },
  { path = "assets/data/config.json", type = "Data", how = "json" },
  { path = "assets/data/walk.clip.json", type = "Data", how = "json", note = "Animator clip" },
  { path = "assets/data/levels.csv", type = "Data", how = "csv" },
  { path = "assets/data/strings.toml", type = "Data", how = "toml" },
  { path = "assets/data/map.tmx", type = "Data", how = "xml", note = "Tiled map" },
  { path = "assets/data/tiles.tsx", type = "Data", how = "xml", note = "Tiled tileset" },
  { path = "assets/data/world.ldtk", type = "Data", how = "json", note = "LDtk world" },
  { path = "assets/data/notes.txt", type = "Data", how = "text" },
  { path = "assets/data/gamecontrollerdb.txt", type = "Gamepad mappings", how = "mappings", web = "maybe" },
  { path = "assets/archives/pack.zip", type = "Archive", how = "archive", note = "mounted with love.filesystem.mount" },
}

--- What to expect of an entry: "yes", "no" or "maybe". ctx = { major, minor, id (platform id), jit (bool) }.
function A.expect(e, ctx)
  if e.only12 and (ctx.major or 0) < 12 then return "no" end
  if e.bytecode then
    if not ctx.jit then return "no" end
    return "maybe" -- bytecode is tied to the LuaJIT build that wrote it
  end
  if ctx.id == "web" then
    if e.web then return e.web end
    if e.webMaybe then return "maybe" end
  end
  return "yes"
end

--- The checks that need the graphics module.
local NEEDS_WINDOW = { shader = true }

-- ---- loaders: return ok, detail (or nil, reason when skipped) -------------------------------------------------------------------

local function read(path) return love.filesystem.read(path) end

local function size(path)
  local i = love.filesystem.getInfo(path)
  return i and i.size or 0
end

local L = {}

function L.image(e)
  local d = love.image.newImageData(e.path)
  local w, h = d:getDimensions()
  local fmt = d.getFormat and d:getFormat() or "?"
  return true, ("%dx%d %s, %d bytes"):format(w, h, tostring(fmt), size(e.path)), d
end

function L.texture(e)
  local c = love.image.newCompressedData(e.path)
  return true, ("%s %dx%d, %d mip level(s)"):format(tostring(c:getFormat()), c:getWidth(), c:getHeight(), c:getMipmapCount()), c
end

-- True when the first `seconds` of the decoder hold any sound at all (peak above a whisper).
local function hasSound(path, seconds)
  local d = love.sound.newDecoder(path)
  local want = d:getSampleRate() * d:getChannelCount() * (d:getBitDepth() / 8) * seconds
  local got = 0
  while got < want do
    local chunk = d:decode()
    if not chunk then break end
    for i = 0, chunk:getSampleCount() * chunk:getChannelCount() - 1 do
      if math.abs(chunk:getSample(i)) > 0.001 then return true end
    end
    got = got + chunk:getSize()
  end
  return false
end

function L.audio(e)
  local d = love.sound.newDecoder(e.path)
  local dur = d.getDuration and d:getDuration() or -1
  local detail = ("%d Hz, %d ch, %d bit%s"):format(d:getSampleRate(), d:getChannelCount(), d:getBitDepth(), dur and dur >= 0 and (", %.1f s"):format(dur) or "")
  -- MIDI is rendered from a GUS patch set (timidity.cfg) that desktop LOVE does not bring. Without one
  -- libmodplug prints "can not open .../timidity.cfg" on the console and falls back to plain built-in
  -- tones (measured on macOS, LOVE 11.5: audible, peak 0.73). A platform that gives silence is flagged.
  if e.type == "Music (MIDI)" and not hasSound(e.path, 10) then e.silent = true end
  return true, detail, d
end

function L.font(e)
  local r = love.font.newRasterizer(e.path, 16, "normal", 1) -- dpiscale 1: the same numbers with or without a window
  return true, ("%d glyphs, height %d, ascent %d"):format(r:getGlyphCount(), r:getHeight(), r:getAscent()), r
end

function L.bmfont(e)
  local r = love.font.newRasterizer(e.path)
  return true, ("%d glyphs, height %d (page image read from beside the .fnt)"):format(r:getGlyphCount(), r:getHeight()), r
end

-- The glyphs of assets/fonts/hex-imagefont.png, in order.
A.IMAGEFONT_GLYPHS = "0123456789ABCDEF.:-+ "

function L.imagefont(e)
  local r = love.font.newImageRasterizer(love.image.newImageData(e.path), A.IMAGEFONT_GLYPHS, 0, 1)
  return true, ("%d glyphs, height %d"):format(r:getGlyphCount(), r:getHeight()), r
end

function L.video(e)
  local v = love.video.newVideoStream(e.path)
  return true, ("Theora stream, %d bytes"):format(size(e.path)), v
end

function L.shader(e)
  if not (love.graphics and love.graphics.newShader) then return nil, "needs a window (the graphics module is off)" end
  local sh = love.graphics.newShader(read(e.path))
  return true, "compiled for " .. tostring((love.graphics.getRendererInfo and select(1, love.graphics.getRendererInfo())) or "this GPU"), sh
end

function L.lua(e)
  local chunk, err = love.filesystem.load(e.path)
  if not chunk then error(err, 0) end
  local v = chunk()
  return true, ("ran, returned %s"):format(type(v) == "table" and "a module" or type(v) == "string" and ('"' .. v .. '"') or type(v)), v
end

local function json(e)
  return require("assets.lib.json").decode(read(e.path))
end

function L.json(e)
  local v = json(e)
  local n = 0
  for _ in pairs(v) do n = n + 1 end
  return true, ("%d top-level keys, %d bytes"):format(n, size(e.path)), v
end

function L.csv(e)
  local rows, cols = 0, 0
  for line in read(e.path):gmatch("[^\r\n]+") do
    rows = rows + 1
    local c = 1
    for _ in line:gsub('"[^"]*"', "X"):gmatch(",") do c = c + 1 end
    if c > cols then cols = c end
  end
  if rows < 2 then error("no data rows", 0) end
  return true, ("%d rows, %d columns"):format(rows, cols)
end

function L.toml(e)
  local tables, keys = 0, 0
  for line in read(e.path):gmatch("[^\r\n]+") do
    if line:match("^%s*%[[%w_.%-]+%]%s*$") then tables = tables + 1 elseif line:match("^%s*[%w_%-]+%s*=") then keys = keys + 1 end
  end
  if keys == 0 then error("no key = value lines", 0) end
  return true, ("%d tables, %d keys"):format(tables, keys)
end

function L.xml(e)
  local text = read(e.path)
  local root = text:match("<([%a_][%w_]*)[%s>]", (text:find("?>", 1, true) or 0) + 1)
  if not root then error("no root element", 0) end
  local _, opens = text:gsub("<[%a_]", "")
  return true, ("root <%s>, %d elements"):format(root, opens)
end

function L.text(e)
  local t = read(e.path)
  local _, lines = t:gsub("\n", "")
  return true, ("%d lines, %d bytes"):format(lines, #t)
end

function L.mappings(e)
  if not (love.joystick and love.joystick.loadGamepadMappings) then error("love.joystick.loadGamepadMappings is missing", 0) end
  love.joystick.loadGamepadMappings(e.path)
  return true, "mappings accepted by SDL"
end

function L.archive(e)
  local data = love.filesystem.newFileData(e.path)
  local mp = "iochk-pack"
  local ok = love.filesystem.mount(data, mp)
  if not ok then error("love.filesystem.mount refused the archive", 0) end
  local names = love.filesystem.getDirectoryItems(mp .. "/pack")
  local txt = love.filesystem.read(mp .. "/pack/readme.txt")
  love.filesystem.unmount(data)
  if #names == 0 or not txt then error("mounted, but nothing was inside", 0) end
  return true, ("mounted; %d member(s), read readme.txt"):format(#names)
end

--- Check one entry. Returns { entry, status, detail, ms, obj }. status: PASS FAIL INFO SKIP. model supplies the judging.
function A.check(e, ctx, model)
  local expect = A.expect(e, ctx)
  local loader = L[e.how]
  if not loader then return { entry = e, status = "FAIL", detail = "no loader for " .. tostring(e.how), expect = expect } end
  if NEEDS_WINDOW[e.how] and not (love.graphics and love.graphics.newShader) then
    return { entry = e, status = "SKIP", detail = "needs a window (run without --report)", expect = expect }
  end
  local t0 = love.timer and love.timer.getTime() or os.clock()
  local ok, a, b, c = pcall(loader, e)
  local ms = ((love.timer and love.timer.getTime() or os.clock()) - t0) * 1000
  local loaded, detail, obj
  if ok then loaded, detail, obj = a, b, c else loaded, detail = false, tostring(a):gsub("^.-:%d+: ", "") end
  if ok and loaded == nil then
    return { entry = e, status = "SKIP", detail = detail, ms = ms, expect = expect }
  end
  local status = model.judgeLoad(expect, loaded)
  if status == "PASS" and not loaded then detail = "refused, as expected: " .. tostring(detail) end
  if status == "INFO" and loaded then detail = tostring(detail) .. " (loaded, though this build is not expected to)" end
  if status == "INFO" and not loaded then detail = "refused (not confirmed on this platform): " .. tostring(detail) end
  if status == "FAIL" then detail = "refused: " .. tostring(detail) end
  if e.silent and status == "PASS" then
    status = "INFO"
    detail = tostring(detail) .. "; but SILENT: no instrument patches (timidity.cfg) here, so the notes are not heard"
  end
  return { entry = e, status = status, detail = detail, ms = ms, obj = obj, expect = expect }
end

--- Check every entry; returns the results in order.
function A.checkAll(ctx, model)
  local out = {}
  for _, e in ipairs(A.LIST) do out[#out + 1] = A.check(e, ctx, model) end
  return out
end

--- The type families in order of first appearance, each with its entries.
function A.groups()
  local order, by = {}, {}
  for _, e in ipairs(A.LIST) do
    if not by[e.type] then by[e.type] = {}; order[#order + 1] = e.type end
    by[e.type][#by[e.type] + 1] = e
  end
  return order, by
end

-- Extensions the library tracks, by family (asset-core/src/kind.rs), to guess a file nobody listed.
local GUESS = {
  png = "image", jpg = "image", jpeg = "image", bmp = "image", tga = "image", hdr = "image", webp = "image", qoi = "image",
  dds = "texture", ktx = "texture", pkm = "texture", pvr = "texture", astc = "texture",
  wav = "audio", ogg = "audio", oga = "audio", mp3 = "audio", flac = "audio", mid = "audio", midi = "audio",
  mod = "audio", xm = "audio", it = "audio", s3m = "audio",
  ttf = "font", otf = "font", woff = "font", ttc = "font", fnt = "bmfont",
  ogv = "video", glsl = "shader", frag = "shader", vert = "shader",
  lua = "lua", luac = "lua", json = "json", ldtk = "json", csv = "csv", toml = "toml", tmx = "xml", tsx = "xml", txt = "text",
  zip = "archive", love = "archive",
}

--- Files under assets/ that the list above does not name, with a guessed loader (nil when the extension is unknown).
--- This is how a file dropped into the project by the Asset Library shows up here without editing the code.
function A.unlisted()
  local known = {}
  for _, e in ipairs(A.LIST) do
    known[e.path] = true
    for _, c in ipairs(e.companions or {}) do known[c] = true end -- a file another asset needs (a BMFont's page picture)
  end
  local out = {}
  local function walk(dir)
    for _, name in ipairs(love.filesystem.getDirectoryItems(dir)) do
      local p = dir .. "/" .. name
      local info = love.filesystem.getInfo(p)
      if info and info.type == "directory" then walk(p)
      elseif info and not known[p] and not name:match("^%.") then
        local ext = name:match("%.([%w]+)$")
        local how = ext and GUESS[ext:lower()]
        out[#out + 1] = { path = p, type = "Unlisted", how = how, note = how and "found by extension" or "unknown extension" }
      end
    end
  end
  if love.filesystem.getInfo("assets", "directory") then walk("assets") end
  return out
end

return A
