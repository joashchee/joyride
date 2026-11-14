-- Runs a group: loads each member's library, runs every row for every member, and rates the cells. A row can be run on its own
-- (the interface does one cell per frame so a slow group never freezes the screen).

local H = require("bench.harness")
local rating = require("bench.rating")
local libload = require("lib.libload")

local Run = {}

--- The costs of having the library: the same three rows in every group, from the library's own load (lib/libload.lua) and its
--- size on disk. Whole-library figures: a bundle (batteries) costs all of it, even if one module is wanted.
Run.COST_ROWS = {
  { id = "cost-time", label = "Load time", kind = "cost", field = "ms", unit = "ms", lower = true, weight = 0.34, about = "Time to require the library (CPU, once)." },
  { id = "cost-mem", label = "Memory kept", kind = "cost", field = "kb", unit = "KB", lower = true, weight = 0.34, about = "Memory the library still holds after loading, after a full collection." },
  { id = "cost-size", label = "Code size", kind = "cost", field = "loc", unit = "lines", lower = true, weight = 0.34, about = "Lines of Lua in the library as installed." },
}

local function memberLoad(group, m, env)
  local r = libload.load(m.lib, env)
  return r
end

--- One cost cell for a member.
local function costCell(row, m, env)
  local r = memberLoad(nil, m, env)
  if r.status == "SKIP" then return H.cell("SKIP", r.detail) end
  if r.status == "FAIL" then return H.cell("FAIL", r.detail) end
  local v
  if row.field == "loc" then v = r.entry.loc elseif row.field == "ms" then v = r.ms else v = r.kb end
  return H.cell("PASS", "whole library", v, row.unit)
end

-- A group that asks LÖVE for the window's size when it builds something (cameras) gets a pretend window when there is none. The cell says so.
local function withStubWindow(group, fn)
  if not group.stubWindow or (love and love.graphics) then return fn(false) end
  local w, h = group.stubWindow[1], group.stubWindow[2]
  local made = false
  love = love or {}
  made = true
  love.graphics = { getWidth = function() return w end, getHeight = function() return h end, getDimensions = function() return w, h end }
  local ok, a = pcall(fn, true)
  love.graphics = nil
  if not ok then error(a, 0) end
  return a
end

local cellNow

--- The cell for one (row, member), run now.
function Run.cell(group, row, m, env)
  return withStubWindow(group, function(stubbed)
    local c = cellNow(group, row, m, env)
    if stubbed and c.status ~= "SKIP" and row.kind ~= "cost" then c.note = (c.note and (c.note .. "; ") or "") .. ("measured with a pretend %dx%d window"):format(group.stubWindow[1], group.stubWindow[2]) end
    return c
  end)
end

cellNow = function(group, row, m, env)
  if row.kind == "cost" then return costCell(row, m, env) end
  local r = memberLoad(group, m, env)
  if r.status == "SKIP" then return H.cell("SKIP", r.detail) end
  if r.status == "FAIL" then return H.cell("FAIL", "did not load: " .. r.detail) end
  if (group.needs == "window" or row.needs == "window") and not env.window then return H.cell("SKIP", "needs a window: run this on a screen") end
  local impl = row.impl and row.impl[m.id]
  local obj = r.mod
  if m.get then
    local ok, v = pcall(m.get, r.mod, function(module) return libload.require(m.lib, module) end)
    if not ok or v == nil then return H.cell("FAIL", "the library has no " .. m.id) end
    obj = v
  end
  return H.run(row, impl, obj)
end

--- The rows of a group with the cost rows after its own.
function Run.rows(group)
  local rows = {}
  for _, r in ipairs(group.rows) do rows[#rows + 1] = r end
  for _, r in ipairs(Run.COST_ROWS) do rows[#rows + 1] = r end
  return rows
end

function Run.order(group)
  local o = {}
  for _, m in ipairs(group.members) do o[#o + 1] = m.id end
  return o
end

--- Run a whole group. Returns results[rowId][memberId] = cell (rated).
function Run.group(group, env, onCell)
  local results = {}
  for _, row in ipairs(Run.rows(group)) do
    local cells = {}
    for _, m in ipairs(group.members) do
      cells[m.id] = Run.cell(group, row, m, env)
      if onCell then onCell(row, m) end
    end
    rating.rate(row, cells)
    results[row.id] = cells
  end
  return results
end

return Run
