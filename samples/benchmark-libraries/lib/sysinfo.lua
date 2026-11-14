-- Facts about the machine that explain a result: LÖVE, LuaJIT (a benchmark means little without knowing whether the JIT was on),
-- processors, memory, where files go. Uses the global `love`.

local P = {}

local function has(t, name) return type(t) == "table" and type(t[name]) == "function" end

function P.system(ctx)
  local rows = {}
  local function add(label, value) rows[#rows + 1] = { label = label, value = value == nil and "?" or tostring(value) } end
  local major, minor, rev, codename = love.getVersion()
  add("LÖVE", ("%d.%d.%d (%s)"):format(major, minor, rev, codename or "?"))
  add("OS reported", love.system and love.system.getOS() or "?")
  if rawget(_G, "jit") then
    add("LuaJIT", ("%s, %s/%s, JIT %s"):format(jit.version, jit.os, jit.arch, jit.status() and "on" or "off"))
  else add("Lua", _VERSION .. " (no LuaJIT: no JIT, no FFI)") end
  add("Processors", has(love.system, "getProcessorCount") and love.system.getProcessorCount() or "?")
  add("Lua memory", ("%.1f MB"):format(collectgarbage("count") / 1024))
  if has(love.system, "getPowerInfo") then
    local state, pct = love.system.getPowerInfo()
    add("Power", ("%s%s"):format(state or "unknown", pct and (", %d%%"):format(pct) or "") .. " (a laptop on battery runs slower)")
  end
  if love.filesystem then
    add("Identity", love.filesystem.getIdentity())
    add("Fused", love.filesystem.isFused() and "yes" or "no")
    add("Source", love.filesystem.getSource())
    add("Save folder", love.filesystem.getSaveDirectory())
  end
  if ctx and ctx.shim then add("Shim", ("%s"):format(ctx.shim.target or "?")) end
  return rows
end

return P
