-- What the running LÖVE and the device can say about themselves, and the storage checks. Every call is guarded: a missing module
-- or a platform that refuses gives a row that says so, never an error. Uses the global `love` (the tests replace it with a mock).

local model = require("lib.model")
local opt = require("lib.optional")

local P = {}

local function try(f, ...)
  local ok, a, b, c, d = pcall(f, ...)
  if ok then return a, b, c, d end
  return nil
end

local function has(tbl, name) return type(tbl) == "table" and type(tbl[name]) == "function" end

-- ---- system -----------------------------------------------------------------------------------------------------------------------

--- Label/value rows describing the runtime and the machine.
function P.system(ctx)
  local rows = {}
  local function add(label, value) rows[#rows + 1] = { label = label, value = value == nil and "?" or tostring(value) } end
  local major, minor, rev, codename = love.getVersion()
  add("LÖVE", ("%d.%d.%d (%s)"):format(major, minor, rev, codename or "?"))
  add("OS reported", love.system and love.system.getOS() or "?")
  if jit then add("LuaJIT", ("%s, %s/%s"):format(jit.version, jit.os, jit.arch)) else add("Lua", _VERSION .. " (no LuaJIT)") end
  add("Processors", has(love.system, "getProcessorCount") and love.system.getProcessorCount() or "?")
  add("Lua memory", ("%.1f MB"):format(collectgarbage("count") / 1024))
  if has(love.system, "getPowerInfo") then
    local state, pct, secs = love.system.getPowerInfo()
    add("Power", ("%s%s"):format(state or "unknown", pct and (", %d%%"):format(pct) or "") .. (secs and (", %d min"):format(math.floor(secs / 60)) or ""))
  end
  if opt.get("system.getPreferredLocales") then
    local l = try(opt.get("system.getPreferredLocales"))
    add("Locale", l and table.concat(l, ", ") or "?")
  end
  add("Identity", love.filesystem.getIdentity())
  add("Fused", love.filesystem.isFused() and "yes (a .exe or .app with the game inside)" or "no")
  add("Source", love.filesystem.getSource())
  add("Save folder", love.filesystem.getSaveDirectory())
  add("Time zone offset", os.date("%z"))
  if ctx and ctx.shim then add("Shim", ("%s, design %sx%s"):format(ctx.shim.target or "?", tostring(ctx.shim.dw or "-"), tostring(ctx.shim.dh or "-"))) end
  return rows
end

-- ---- video ------------------------------------------------------------------------------------------------------------------------

--- Facts about the window and renderer. Returns a table and the rows for the page.
function P.video()
  local v = {}
  if not (love.window and love.graphics and love.graphics.isActive ~= nil) then
    v.rows = { { label = "Graphics", value = "off (love . --report): no window to check" } }
    return v
  end
  local w, h, flags = love.window.getMode()
  v.w, v.h, v.flags = w, h, flags
  v.dpi = has(love.window, "getDPIScale") and love.window.getDPIScale() or (has(love.window, "getPixelDensity") and love.window.getPixelDensity()) or 1
  v.pw, v.ph = love.graphics.getPixelDimensions()
  v.dw, v.dh = try(love.window.getDesktopDimensions)
  if has(love.window, "getSafeArea") then v.sx, v.sy, v.sw, v.sh = try(love.window.getSafeArea) end
  if has(love.window, "getOrientation") then v.orientation = try(love.window.getOrientation) end
  local name, version, vendor, device = try(love.graphics.getRendererInfo)
  v.renderer = name and ("%s %s, %s %s"):format(name, version or "", vendor or "", device or "") or "?"
  v.limits = try(love.graphics.getSystemLimits) or {}
  v.supported = try(love.graphics.getSupported) or {}
  local rows = v.rows or {}
  v.rows = rows
  local function add(label, value) rows[#rows + 1] = { label = label, value = tostring(value) } end
  add("Window", ("%dx%d points, %dx%d pixels, scale %.2f"):format(w, h, v.pw, v.ph, v.dpi))
  add("Desktop", v.dw and ("%dx%d"):format(v.dw, v.dh) or "?")
  add("Mode", ("%s, vsync %s, %s Hz, display %s"):format(flags.fullscreen and ("fullscreen " .. tostring(flags.fullscreentype)) or "windowed", tostring(flags.vsync), tostring(flags.refreshrate or "?"), tostring(flags.display or 1)))
  add("Resizable", flags.resizable and "yes" or "no")
  add("Safe area", v.sx and ("x %d, y %d, %dx%d"):format(v.sx, v.sy, v.sw, v.sh) or "not reported")
  add("Orientation", v.orientation or "not reported")
  add("Renderer", v.renderer)
  add("Max texture", v.limits.texturesize or "?")
  add("MSAA samples", v.limits.canvasmsaa or v.limits.texturemsaa or "?")
  local feats = {}
  for _, k in ipairs({ "canvas", "shader", "npot", "instancing", "glsl3", "multicanvasformats", "clampzero" }) do
    if v.supported[k] ~= nil then feats[#feats + 1] = k .. (v.supported[k] and "+" or "-") end
  end
  add("Features", table.concat(feats, " "))
  return v
end

-- ---- sound ------------------------------------------------------------------------------------------------------------------------

function P.audio()
  local a = { rows = {} }
  local function add(label, value) a.rows[#a.rows + 1] = { label = label, value = tostring(value) } end
  if not love.audio then
    a.error = "LÖVE's audio module did not start"
    add("Audio", a.error)
    return a
  end
  local dev = has(love.audio, "getPlaybackDevice") and try(love.audio.getPlaybackDevice)
  add("Output", "OpenAL" .. (dev and (", " .. tostring(dev)) or ""))
  add("Volume", ("%.0f%%"):format((try(love.audio.getVolume) or 1) * 100))
  add("Effects", has(love.audio, "isEffectsSupported") and (try(love.audio.isEffectsSupported) and "supported" or "not supported") or "?")
  add("Sources playing", try(love.audio.getActiveSourceCount) or "?")
  add("Max sources", has(love.audio, "getMaxSources") and try(love.audio.getMaxSources) or "?")
  add("Background music", has(love.system, "hasBackgroundMusic") and (love.system.hasBackgroundMusic() and "another app is playing music" or "none") or "not reported")
  return a
end

-- ---- capabilities: one row per thing a platform expects ---------------------------------------------------------------------------

--- Every capability a profile has an expectation for: { key, label, present (true, false or nil when it needs a person), detail }.
--- `human` ones are filled by the person using the page (input record, heard/felt answers); the rest are probed here.
function P.capabilities(s)
  local caps = {}
  local function add(key, label, present, detail, human) caps[#caps + 1] = { key = key, label = label, present = present, detail = detail or "", human = human } end
  local input = s.input

  -- Keyboard: LÖVE always has the module; whether keys arrive is what a person shows.
  add("keyboard", "Keyboard keys arrive", input.keyCount > 0 or nil, input.keyCount > 0 and (input.keyCount .. " different keys") or "press some keys on the Keyboard page", true)
  local m = input.mouse
  add("mouse", "Mouse or pointer", (m.moved or m.presses > 0) or nil, (m.moved or m.presses > 0) and ("moved %s, %d clicks"):format(tostring(m.moved), m.presses) or "move or click on the Pointer page", true)
  add("touch", "Touch screen", input.touch.seen or nil, input.touch.seen and ("%d at once, %d taps"):format(input.touch.max, input.touch.taps) or "touch the screen on the Pointer page", true)

  local pads, accel = 0, 0
  local list = love.joystick and try(love.joystick.getJoysticks) or {}
  for _, j in ipairs(list or {}) do
    if (j:getName() or ""):lower():find("accelerom", 1, true) then accel = accel + 1 else pads = pads + 1 end
  end
  add("gamepad", "Game controller", pads > 0, pads > 0 and (pads .. " connected") or "none connected (plug one in or pair one)")
  local sensors = {}
  local hasSensor = opt.get("sensor.hasSensor")
  if hasSensor then
    for _, k in ipairs({ "accelerometer", "gyroscope" }) do if try(hasSensor, k) then sensors[#sensors + 1] = k end end
  end
  if accel > 0 then sensors[#sensors + 1] = "accelerometer (as a joystick)" end
  add("sensors", "Motion sensors", #sensors > 0, #sensors > 0 and table.concat(sensors, ", ") or (hasSensor and "none found" or "no love.sensor (LÖVE 11.5 has no sensor module)"))
  add("haptics", "Vibration", has(love.system, "vibrate") and (s.answers.vibrate == "felt" and true or (s.answers.vibrate == "not felt" and false or nil)) or false,
      has(love.system, "vibrate") and (s.answers.vibrate or "press X on the Sensors page to buzz") or "love.system.vibrate is missing", true)

  local windowed = love.window ~= nil and love.graphics ~= nil
  local NOWIN = "needs a window (not checked without one)"

  -- Clipboard round trip.
  local clip, cdetail = false, "love.system.setClipboardText is missing"
  if not windowed then clip, cdetail = nil, NOWIN
  elseif has(love.system, "setClipboardText") then
    local ok = pcall(love.system.setClipboardText, "io-checker")
    local back = ok and try(love.system.getClipboardText)
    clip, cdetail = back == "io-checker", back == "io-checker" and "wrote and read back" or ("wrote, read back " .. tostring(back))
  end
  add("clipboard", "Clipboard", clip, cdetail)

  local v = s.videoInfo or {}
  if not windowed then
    add("resizable", "Window can be resized", nil, NOWIN)
    add("fullscreen", "Fullscreen", nil, NOWIN)
    add("safearea", "Safe area reported", nil, NOWIN)
  else
    add("resizable", "Window can be resized", v.flags and v.flags.resizable or false, v.flags and ("flag " .. tostring(v.flags.resizable)) or "no window")
    add("fullscreen", "Fullscreen", has(love.window, "setFullscreen"), has(love.window, "setFullscreen") and (v.flags and v.flags.fullscreen and "on now" or "available") or "love.window.setFullscreen is missing")
    add("safearea", "Safe area reported", has(love.window, "getSafeArea"), v.sx and ("x %d, y %d, %dx%d"):format(v.sx, v.sy, v.sw, v.sh) or "love.window.getSafeArea " .. (has(love.window, "getSafeArea") and "gave nothing" or "is missing"))
  end

  -- Threads: start one, wait for it, read its answer.
  local tok, tdetail = false, "love.thread is not available"
  if love.thread and has(love.thread, "newThread") then
    local ok, err = pcall(function()
      local th = love.thread.newThread("love.thread.getChannel('iochk-thread'):push(42)\n")
      th:start()
      th:wait()
      local got = love.thread.getChannel("iochk-thread"):pop()
      if got ~= 42 then error("the thread's answer was " .. tostring(got), 0) end
    end)
    tok, tdetail = ok, ok and "ran a thread and got its answer" or tostring(err)
  end
  add("threads", "Threads", tok, tdetail)

  local okffi = pcall(require, "ffi")
  add("ffi", "ffi library", okffi, okffi and "loaded" or "not available")
  local pok, pout = false, "io.popen is missing"
  if io.popen then
    local ok, r = pcall(function()
      local p = io.popen("echo io-checker")
      local o = p and p:read("*a") or ""
      if p then p:close() end
      return o
    end)
    pok, pout = ok and r and r:find("io-checker", 1, true) ~= nil, ok and (r and r:gsub("%s+$", "") or "no output") or tostring(r)
  end
  add("popen", "Run a program (io.popen)", pok, pok and "ran echo" or pout)
  local sok = pcall(require, "socket")
  add("network", "Network (luasocket)", sok, sok and "module loaded (no connection is made)" or "not available")
  add("video", "Video (Ogg Theora)", love.video ~= nil, love.video and "love.video present" or "love.video is off or missing")
  add("quit", "Can quit", has(love.event, "quit"), has(love.event, "quit") and "love.event.quit present" or "missing")
  return caps
end

-- ---- storage ------------------------------------------------------------------------------------------------------------------------

local function now() return love.timer and love.timer.getTime() or os.clock() end

--- The checks on LÖVE's own save folder, and on plain io when the platform allows it. Returns rows { status, label, value }.
--- profile is the platform in force (it says whether real file paths are expected).
function P.storage(profile, mb)
  mb = mb or 4
  local rows = {}
  local function add(status, label, value) rows[#rows + 1] = { status = status, label = label, value = tostring(value) } end
  local fs = love.filesystem
  local openFile = fs.openFile or fs.newFile -- 12.0 renamed newFile
  add("INFO", "save folder", fs.getSaveDirectory())
  add("INFO", "identity", fs.getIdentity())

  -- The steps of a game saving: write, read back, append, size, list, make a folder, delete.
  local dir, file = "iochk", "iochk/probe.txt"
  local step
  local ok, err = pcall(function()
    if not fs.createDirectory(dir) then step = "cannot make a folder" return end
    if not fs.write(file, "first\n") then step = "cannot write" return end
    if fs.read(file) ~= "first\n" then step = "read back the wrong text" return end
    if not fs.append(file, "second\n") then step = "cannot append" return end
    local info = fs.getInfo(file)
    if not info or info.size ~= 13 then step = "size is " .. tostring(info and info.size) .. ", not 13" return end
    local names = fs.getDirectoryItems(dir)
    local found = false
    for _, n in ipairs(names) do if n == "probe.txt" then found = true end end
    if not found then step = "the file is not in its folder's listing" return end
    if not fs.remove(file) then step = "cannot delete" return end
    if fs.getInfo(file) then step = "deleted, but still there" return end
    fs.remove(dir)
  end)
  if not ok then step = tostring(err) end
  add(step and "FAIL" or "PASS", "write, read, append, list, delete", step or "ok")

  -- Speed and integrity: mb MB of every byte value, written, read back and compared.
  local block = {}
  for i = 0, 255 do block[#block + 1] = string.char(i) end
  local mib = string.rep(table.concat(block), 4096)
  local path = "iochk-speed.tmp"
  local t0 = now()
  local wok, wres = pcall(function()
    local f, ferr = openFile(path, "w")
    assert(f, ferr)
    for _ = 1, mb do assert(f:write(mib)) end
    f:close()
  end)
  local tw = now() - t0
  if not wok then
    add("FAIL", "write speed", ("could not write %d MB: %s"):format(mb, tostring(wres)))
  else
    add("INFO", "write speed", ("%.1f MB/s (%d MB)"):format(mb * 1.048576 / math.max(tw, 1e-6), mb))
    t0 = now()
    local rok, n, same = pcall(function()
      local f, ferr = openFile(path, "r")
      assert(f, ferr)
      local total, equal = 0, true
      while true do
        local chunk = f:read(#mib)
        if not chunk or #chunk == 0 then break end
        total = total + #chunk
        if chunk ~= mib:sub(1, #chunk) then equal = false end
      end
      f:close()
      return total, equal
    end)
    local tr = now() - t0
    if rok then
      add("INFO", "read speed", ("%.1f MB/s (may come from memory)"):format(mb * 1.048576 / math.max(tr, 1e-6)))
      add((same and n == #mib * mb) and "PASS" or "FAIL", "read back", (same and n == #mib * mb) and "every byte the same" or ("%d of %d bytes, %s"):format(n, #mib * mb, same and "same" or "DIFFERENT"))
    else
      add("FAIL", "read back", tostring(n))
    end
    local info = fs.getInfo(path)
    add((info and info.size == #mib * mb) and "PASS" or "FAIL", "size on disk", ("%s bytes of %d"):format(tostring(info and info.size), #mib * mb))
    fs.remove(path)
  end

  -- Persistence across runs: a counter that goes up each time the app starts (a reload on the web, a relaunch on a phone).
  local counter = "iochk-runs.txt"
  local before = tonumber(fs.read(counter) or "") or 0
  local wrote = fs.write(counter, tostring(before + 1))
  add(wrote and "PASS" or "FAIL", "persistence", wrote and ("this is run %d%s"):format(before + 1, before == 0 and " (the first: start the app again to see it go up)" or " (it went up: saves survive a restart)") or "could not record the run")

  -- Plain io into the save folder and beside the game: only some platforms let a game do this.
  local real = fs.getSaveDirectory()
  local rok = false
  local rdetail = "io.open refused"
  if io.open and real and real ~= "" then
    local p = real .. "/iochk-plain-io.tmp"
    local f = io.open(p, "wb")
    if f then
      local w = f:write("plain io\n")
      f:close()
      local g = io.open(p, "rb")
      local back = g and g:read("*a")
      if g then g:close() end
      os.remove(p)
      rok = w ~= nil and back == "plain io\n"
      rdetail = rok and "wrote and read " .. p or "wrote, but read back " .. tostring(back)
    end
  end
  local status, why = model.judge(profile.realfs, rok)
  add(status, "plain io (real paths)", ("%s: %s"):format(why, rdetail))

  -- A game folder that may be written (PortMaster keeps saves beside the game).
  if profile.id == "portmaster" and fs.getSourceBaseDirectory then
    local base = fs.getSourceBaseDirectory()
    local p = base .. "/.iochk-write.tmp"
    local f = io.open(p, "wb")
    if f then f:write("x"); f:close(); os.remove(p) end
    add(f and "PASS" or "INFO", "game folder writable", ("%s: %s"):format(base, f and "yes" or "no (saves must go in the save folder)"))
  end
  return rows
end

return P
