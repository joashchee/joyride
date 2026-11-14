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
L.windowLibs = { g3d = true, push = true, Slab = true }

-- A library that is known not to load on a LÖVE major version, and why: a finding about the library, not a failure of this project.
-- (Checked against the real LÖVE binaries; the report says INFO, not FAIL.)
L.known = {
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

--- Whether this run can load a library of this kind. Returns true, or false and why (a SKIP).
function L.canLoad(entry, env)
  env = env or {}
  if entry.kind == "native" then return false, "a compiled module that is not bundled here: " .. (entry.native or "needs its binary") end
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
  collectgarbage(); collectgarbage()
  -- A library that prints when it is required (lurker does) would write into the middle of a report: its output is counted and dropped.
  local realPrint, printed = print, 0
  print = function() printed = printed + 1 end
  local m0, t0 = collectgarbage("count"), os.clock()
  local okr, mod = pcall(require, e.module)
  local t1 = os.clock()
  print = realPrint
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
  if e.kind == "ffi" and (needsBinary or looksLikeMissingBinary(msg)) then
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
  local ok, mod = pcall(require, module)
  setPath(saved)
  if not ok then error(mod, 0) end
  return mod
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
