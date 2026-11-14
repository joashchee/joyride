-- Which platform is this running on, and what does that platform expect of its input, screen, sound and storage?
-- Joyride targets eight: macOS, Windows and Linux (desktop LÖVE), PortMaster (Linux handhelds), iOS, iPadOS, Android and the web
-- (love.js). LÖVE itself only says "OS X", "Windows", "Linux", "Android", "iOS" or "Web", so the rest comes from evidence: the
-- PortMaster launch script's variables, the folder the game sits in, the shim Joyride writes into an auto-ported build, and the
-- screen's size for an iPad. The person can override the answer (the Platform page) to run the checks of another platform.
-- No LÖVE calls here: detect() is given what it needs, so luajit tests it (tests/test_platform.lua).

local P = {}

-- expect: "need" (a missing one fails the check), "want" (noted when missing), "none" (not normally there; present is information).
local function profile(id, name, os, t)
  t.id, t.name, t.os = id, name, os
  return t
end

local DESKTOP = {
  keyboard = "need", mouse = "need", touch = "none", gamepad = "want", sensors = "none", haptics = "none", clipboard = "need",
  resizable = "need", fullscreen = "need", safearea = "none", realfs = "need", threads = "need", ffi = "need", popen = "need",
  video = "need", network = "need", quit = "need",
}
local function with(base, over)
  local t = {}
  for k, v in pairs(base) do t[k] = v end
  for k, v in pairs(over) do t[k] = v end
  return t
end

P.PROFILES = {
  profile("macos", "macOS", "OS X", with(DESKTOP, { note = "Desktop LÖVE. Trackpad touches are not touch events. A pad is optional." })),
  profile("windows", "Windows", "Windows", with(DESKTOP, { note = "Desktop LÖVE with a keyboard, a mouse and optionally a pad. Any folder the person can write is writable." })),
  profile("linux", "Linux", "Linux", with(DESKTOP, { note = "Desktop LÖVE (X11 or Wayland) with a keyboard, a mouse and optionally a pad." })),
  profile("portmaster", "PortMaster", "Linux", with(DESKTOP, {
    keyboard = "want", mouse = "want", gamepad = "need", haptics = "want", clipboard = "want", resizable = "none", network = "want",
    note = "A Linux handheld. The launch script's gptokeyb turns the pad into keys (and Joyride's shim makes a stick-driven mouse), so keys may arrive without a keyboard. The pad itself is the input that must work.",
  })),
  profile("ios", "iOS (iPhone)", "iOS", {
    keyboard = "want", mouse = "none", touch = "need", gamepad = "want", sensors = "want", haptics = "want", clipboard = "need",
    resizable = "none", fullscreen = "need", safearea = "need", realfs = "none", threads = "need", ffi = "need", popen = "none",
    video = "need", network = "want", quit = "none",
    note = "Touch first. Only the app's own sandbox is writable. An app does not quit itself on iOS.",
  }),
  profile("ipados", "iPadOS (iPad)", "iOS", {
    keyboard = "want", mouse = "want", touch = "need", gamepad = "want", sensors = "want", haptics = "none", clipboard = "need",
    resizable = "want", fullscreen = "need", safearea = "need", realfs = "none", threads = "need", ffi = "need", popen = "none",
    video = "need", network = "want", quit = "none",
    note = "Touch first, with a keyboard, a pointer and split-screen windows possible. Only the app's own sandbox is writable.",
  }),
  profile("android", "Android", "Android", {
    keyboard = "want", mouse = "none", touch = "need", gamepad = "want", sensors = "want", haptics = "want", clipboard = "need",
    resizable = "none", fullscreen = "need", safearea = "want", realfs = "none", threads = "need", ffi = "need", popen = "none",
    video = "need", network = "want", quit = "want",
    note = "Touch first. Only the app's own storage is writable. The Back gesture arrives as the escape key.",
  }),
  profile("web", "Web (love.js)", "Web", {
    keyboard = "need", mouse = "need", touch = "want", gamepad = "want", sensors = "none", haptics = "none", clipboard = "want",
    resizable = "want", fullscreen = "want", safearea = "none", realfs = "none", threads = "none", ffi = "none", popen = "none",
    video = "want", network = "none", quit = "none",
    note = "Runs in a browser tab. Audio starts only after a click or key. Saves live in the browser (IndexedDB) and survive a reload only if the page flushes them. Threads, ffi, io.popen and sockets are not there.",
  }),
}

P.BY_ID = {}
for _, p in ipairs(P.PROFILES) do P.BY_ID[p.id] = p end

--- The signals that say "PortMaster" (set by PortMaster's control.txt and the generated launch script).
P.PORTMASTER_ENV = { "CFW_NAME", "DEVICE_NAME", "GPTOKEYB", "controlfolder" }

--- Decide the platform. env = {
---   os = love.system.getOS(), getenv = os.getenv, shim = package.loaded._jr_shim or nil,
---   source = love.filesystem.getSource() (a path), desktop = { w, h } (love.window.getDesktopDimensions),
---   window = { w, h } (love.window.getMode, when there is a window), jit = jit or nil }
--- Returns { id, name, os, certainty = "certain" | "likely", evidence = { { label, value } ... } }.
function P.detect(env)
  local ev = {}
  local function add(label, value) ev[#ev + 1] = { label = label, value = tostring(value) } end
  local getenv = env.getenv or function() return nil end
  local osname = env.os or "?"
  add("love.system.getOS()", osname)
  if env.jit then add("jit", ("%s on %s"):format(env.jit.version or "LuaJIT", (env.jit.os or "?") .. "/" .. (env.jit.arch or "?"))) else add("jit", "none (plain Lua: a love.js build)") end
  local shim = env.shim and env.shim.target
  if shim then add("Joyride shim", shim .. " (this is an auto-ported build)") else add("Joyride shim", "none (run as written)") end

  local function result(id, certainty)
    local p = P.BY_ID[id]
    return { id = id, name = p.name, os = osname, certainty = certainty, evidence = ev }
  end

  local desktopOS = osname == "OS X" or osname == "Windows" or osname == "Linux"
  local function simulated(id)
    add("note", ("this machine runs %s, but the game is Joyride's %s build: a simulated screen, or a build run by hand"):format(osname, shim))
    return result(id, "simulated")
  end

  if osname == "Android" then return result("android", "certain") end
  if osname == "Web" or osname == "Emscripten" then return result("web", "certain") end
  if osname == "iOS" then
    local d = env.desktop
    local short = d and math.min(d.w or 0, d.h or 0) or 0
    add("screen, shortest side", short > 0 and (short .. " points") or "unknown")
    -- iPhones top out at 440 points on the short side, the smallest iPad is 744.
    return result(short >= 600 and "ipados" or "ios", "likely")
  end

  -- PortMaster's own signals (not the shim): the variables its launch script sets, and the folder it installs ports in.
  local real = 0
  if osname == "Linux" then
    for _, name in ipairs(P.PORTMASTER_ENV) do
      local v = getenv(name)
      if v and v ~= "" then real = real + 1; add("env " .. name, v) end
    end
    local src = env.source or ""
    if src:find("/ports/", 1, true) then real = real + 1; add("game folder", src) end
  end
  if real >= 2 then return result("portmaster", "certain") end
  if real == 1 then return result("portmaster", "likely") end

  -- Joyride's shim says which build this is when the OS cannot (a simulated screen on a desktop).
  if shim == "web" then return desktopOS and simulated("web") or result("web", "certain") end
  if shim == "portmaster" then return desktopOS and simulated("portmaster") or result("portmaster", "likely") end
  if shim == "mobile" and desktopOS then
    local w = env.window
    local short = w and math.min(w.w or 0, w.h or 0) or 0
    if short > 0 then add("window, shortest side", short .. " points") end
    return simulated(short >= 600 and "ipados" or "ios")
  end

  if osname == "OS X" then return result("macos", "certain") end
  if osname == "Windows" then return result("windows", "certain") end
  if osname == "Linux" then return result("linux", "certain") end
  -- A LÖVE we do not know: treat as desktop Linux and say so.
  add("note", "unknown OS name; using the Linux desktop checks")
  return result("linux", "likely")
end

--- The profile in force: the override when one is chosen, else what was detected.
function P.resolve(detected, overrideId)
  if overrideId and P.BY_ID[overrideId] then return P.BY_ID[overrideId], overrideId ~= detected.id end
  return P.BY_ID[detected.id], false
end

--- The next id in the list after `id` (nil stands for "auto"), for the Platform page's left and right keys.
function P.cycle(overrideId, step)
  local ids = { false }
  for _, p in ipairs(P.PROFILES) do ids[#ids + 1] = p.id end
  local cur = 1
  for i, id in ipairs(ids) do if id == (overrideId or false) then cur = i end end
  local nxt = ids[(cur - 1 + step) % #ids + 1]
  return nxt or nil
end

return P
