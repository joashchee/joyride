-- Loads each of the project's libraries (libs/<name>/, installed by JoyrIDE from love.toml) in isolation and records what it cost:
-- the time to require it, the memory it keeps, and its size on disk (lib/catalog.lua, written by tools/vendor-libs.py).
--
-- A library is required with only its own folder (and libs/) on the require path, so two libraries that both have a `class.lua` at
-- their root cannot be confused, and what a library requires under a bare name is forgotten afterwards. The catalogue says what to
-- require; whether a library may load at all on this platform is the platform profile's call (ffi, window): a library that needs a
-- binary or a window this run does not have is SKIP, never FAIL.

local catalog = require("lib.catalog")

local L = { entries = catalog, by = {}, loaded = {}, root = "libs" }
for _, e in ipairs(catalog) do L.by[e.name] = e end

-- Libraries that need another one on the path (lurker requires lume by its bare name).
L.deps = { lurker = { "lume" } }

-- Libraries that cannot be required without a window (they build images, fonts or shaders as they load).
L.windowLibs = { g3d = true, push = true, Slab = true, ["cimgui-love"] = true }

-- A library that is known not to load on a LÖVE major version, and why: a finding about the library, not a failure of this project.
-- (Checked against the real LÖVE binaries; the report says INFO, not FAIL.)
L.known = {
  g3d = { [11] = "reads its shader as g3d/g3d.vert from the game's root folder, so it only works copied there, not from libs/", [12] = "reads its shader as g3d/g3d.vert from the game's root folder, so it only works copied there, not from libs/" },
  lurker = { [12] = "calls love.filesystem.isDirectory when it loads, which LÖVE 12.0 removed" },
}

-- How the require path is read and set. Under LÖVE this is love.filesystem's (it works inside a .love and on a phone); plain luajit
-- (the unit tests) uses package.path.
local function getPath()
  if love and love.filesystem and love.filesystem.getRequirePath then return love.filesystem.getRequirePath() end
  return package.path
end
local function setPath(p)
  if love and love.filesystem and love.filesystem.setRequirePath then love.filesystem.setRequirePath(p) else package.path = p end
end
L.getPath, L.setPath = getPath, setPath

local function roots(name, plain)
  local t, names = {}, { name }
  for _, d in ipairs(L.deps[name] or {}) do names[#names + 1] = d end
  for _, n in ipairs(names) do
    t[#t + 1] = L.root .. "/" .. n .. "/?.lua"
    t[#t + 1] = L.root .. "/" .. n .. "/?/init.lua"
  end
  t[#t + 1] = L.root .. "/?.lua"
  t[#t + 1] = L.root .. "/?/init.lua"
  return table.concat(t, ";") .. (plain and ";" .. plain or "")
end
L.roots = roots

--- The module names a loaded library owns: its own, and its children (`batteries.class` belongs to `batteries`).
local function owns(entry, key)
  local top = entry.module:match("^[^.]+")
  return key == top or key:sub(1, #top + 1) == top .. "."
end

-- Modules that belong to the runtime, never to a library: loading one lazily must not be undone.
local RUNTIME = { ffi = true, bit = true, enet = true, socket = true, ssl = true, mime = true, utf8 = true, ["string.buffer"] = true, ["table.new"] = true, ["table.clear"] = true }
local function runtime(k) return RUNTIME[k] or k:match("^love") ~= nil or k:match("^jit") ~= nil or k:match("^socket%.") ~= nil end

local ERR_BINARY = { "cannot load", "image not found", "cannot open shared object", "dlopen", "no such file", "could not load", "not found", "undefined symbol", "The specified module could not be found" }
local function looksLikeMissingBinary(msg)
  local m = msg:lower()
  for _, s in ipairs(ERR_BINARY) do if m:find(s:lower(), 1, true) then return true end end
  return false
end

-- The compiled parts, built by tools/build-binaries.sh into bin/ beside the game (never in the repository).
local OS_FILES = {
  ["OS X"] = { nuklear = "nuklear.so", cimgui = "cimgui.dylib", discord = "libdiscord-rpc.dylib" },
}
-- `preload`: a shared library the Lua side opens by its bare name, which is first opened by full path so the bare name then resolves.
L.binaries = {
  ["love-nuklear"] = { key = "nuklear" },
  ["cimgui-love"] = { key = "cimgui" },
  ["lua-discordRPC"] = { key = "discord", preload = true },
}

--- The folder holding the compiled parts: bin/ in the game's own folder (a C module cannot be opened from inside a .love file).
function L.binDir()
  local base = "."
  if love and love.filesystem and love.filesystem.getSource then base = love.filesystem.getSource() end
  return base .. "/bin"
end

--- The full path of a library's compiled part on this OS, or nil when it is not there (or this OS has no recipe for it).
function L.binaryPath(name)
  local b = L.binaries[name]
  local os_ = (love and love.system and love.system.getOS and love.system.getOS()) or "OS X"
  local file = b and OS_FILES[os_] and OS_FILES[os_][b.key]
  if not file then return nil end
  local path = L.binDir() .. "/" .. file
  local f = io.open(path, "rb")
  if f then f:close(); return path end
end

--- Puts bin/ on the C module path; returns the function that puts it back.
local function cpathOn()
  local saved, bin = package.cpath, L.binDir()
  package.cpath = bin .. "/?.so;" .. bin .. "/?.dylib;" .. saved
  return function() package.cpath = saved end
end

--- Whether this run can load a library of this kind. Returns true, or false and why (a SKIP).
function L.canLoad(entry, env)
  env = env or {}
  if entry.name == "luasteam" then return false, "needs Valve's Steamworks SDK, which cannot be redistributed or built here: " .. (entry.native or "") end
  if entry.kind == "native" and not L.binaryPath(entry.name) then return false, "a compiled module that is not built here (tools/build-binaries.sh): " .. (entry.native or "needs its binary") end
  if entry.kind == "ffi" and L.binaries[entry.name] and not L.binaryPath(entry.name) then return false, "its binary is not built here (tools/build-binaries.sh): " .. (entry.native or "") end
  if entry.kind == "ffi" then
    if env.ffi == "none" or not rawget(_G, "jit") then return false, "this platform has no FFI" end
  end
  return true
end

--- Require one library and record the cost. Returns the result record, also kept in L.loaded[name]:
--- { name, status = "PASS" | "FAIL" | "SKIP", detail, ms, kb, mod }
function L.load(name, env)
  local done = L.loaded[name]
  if done then return done end
  local e = L.by[name]
  local res = { name = name, entry = e }
  L.loaded[name] = res
  if not e then res.status, res.detail = "FAIL", "not in the curated list"; return res end
  local ok, why = L.canLoad(e, env)
  if not ok then res.status, res.detail = "SKIP", why; return res end
  -- Its helpers first: a library another one needs is loaded for it, and counted in its own record.
  local before = {}
  for k in pairs(package.loaded) do before[k] = true end
  local saved = getPath()
  setPath(roots(name))
  local cpathOff = cpathOn()
  local bpath = L.binaryPath(name)
  if bpath and L.binaries[name].preload and rawget(_G, "jit") then pcall(require("ffi").load, bpath) end
  collectgarbage(); collectgarbage()
  -- A library that prints when it is required (lurker does) would write into the middle of a report: its output is counted and dropped.
  local realPrint, printed = print, 0
  print = function() printed = printed + 1 end
  local m0, t0 = collectgarbage("count"), os.clock()
  local okr, mod = pcall(require, e.module)
  local t1 = os.clock()
  print = realPrint
  cpathOff()
  res.printed = printed
  collectgarbage(); collectgarbage()
  local m1 = collectgarbage("count")
  setPath(saved)
  for k in pairs(package.loaded) do
    if not before[k] and not owns(e, k) and not runtime(k) and not (L.deps[name] and k == "lume") then package.loaded[k] = nil end
  end
  res.ms, res.kb = (t1 - t0) * 1000, math.max(0, m1 - m0)
  if okr then
    res.status, res.mod, res.detail = "PASS", mod, ("required `%s`"):format(e.module)
    return res
  end
  local msg = tostring(mod):gsub("\n.*", "")
  -- "No window" means no graphics module, or a graphics module with no window open (the self-test's mock has the first and not the second).
  local hasWindow = love and love.graphics ~= nil and not (love.window and love.window.isOpen and not love.window.isOpen())
  local needsBinary = e.native and e.native:find("^Needs") ~= nil
  if e.kind == "ffi" and not bpath and (needsBinary or looksLikeMissingBinary(msg)) then
    res.status, res.detail = "SKIP", "its binary is not here: " .. (e.native or msg)
  elseif not hasWindow and (L.windowLibs[name] or msg:find("graphics", 1, true) or msg:find("window", 1, true) or msg:find("getDimensions", 1, true) or msg:find("getWidth", 1, true) or msg:find("mouse", 1, true) or msg:find("keyboard", 1, true) or msg:find("joystick", 1, true)) then
    res.status, res.detail = "SKIP", "needs a window (" .. msg:sub(1, 60) .. ")"
  else
    res.status, res.detail = "FAIL", msg
    local major = love and love.getVersion and love.getVersion()
    local known = L.known[name] and L.known[name][major]
    if known then res.known = known end
  end
  return res
end

--- Require a module of an already loaded library (hump.timer of hump), with that library's folders on the path. A member's `get`
--- is given this as its second argument.
function L.require(name, module)
  local saved = getPath()
  setPath(roots(name))
  local off = cpathOn()
  local ok, mod = pcall(require, module)
  off()
  setPath(saved)
  if not ok then error(mod, 0) end
  return mod
end

--- Run `fn` with a library's folders on the require path: a library that requires its own parts later, when it is used (moonshine loads
--- an effect the first time it is asked for), needs them then, not only while it loads.
function L.withPath(name, fn)
  local saved = getPath()
  setPath(roots(name))
  local off = cpathOn()
  local ok, a, b = pcall(fn)
  off()
  setPath(saved)
  if not ok then error(a, 0) end
  return a, b
end

-- A compiled library can abort the whole program (a C assert, a segfault) and nothing in Lua can catch that. Before a cell that runs one,
-- its key is written to a marker file in the save folder; a run that comes back and finds the marker knows that cell took the program down
-- last time, and reports it instead of running it again.
L.MARKER = "benchmark-crash.txt"
L.crashed = {}

--- Call once at start: remembers (and clears) the cell a previous run was in when it died.
function L.recoverCrash()
  if not (love and love.filesystem and love.filesystem.getInfo and love.filesystem.getInfo(L.MARKER)) then return end
  local key = love.filesystem.read(L.MARKER)
  love.filesystem.remove(L.MARKER)
  if key and key ~= "" then L.crashed[key] = true end
end

--- Runs fn under a marker when the library is a compiled one (L.binaries). Returns fn's results.
function L.guard(name, key, fn)
  if not (L.binaries[name] and love and love.filesystem and love.filesystem.write) then return fn() end
  love.filesystem.write(L.MARKER, key)
  local a, b = fn()
  love.filesystem.remove(L.MARKER)
  return a, b
end

--- Forget what was loaded (a test, or a re-run).
function L.reset()
  for name, r in pairs(L.loaded) do
    if r.entry then
      for k in pairs(package.loaded) do if owns(r.entry, k) then package.loaded[k] = nil end end
    end
    L.loaded[name] = nil
  end
end

return L
