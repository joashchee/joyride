-- Every group, every library, in order: the whole benchmark as one list of small steps, so a screen can run one step a frame (and
-- show progress) and a headless run can loop through them all. Nothing here draws.

local runner = require("bench.runner")
local rating = require("bench.rating")
local specialists = require("bench.specialists")
local libload = require("lib.libload")

local Suite = {}

-- The comparisons, in the order they are shown.
Suite.ORDER = { "classes", "tweens", "timers", "events", "states", "vectors", "collections", "maths", "strings", "serialise", "printing",
                "collision", "pathfinding", "cameras", "testing", "profiling", "gui" }

Suite.groups, Suite.byId = {}, {}
for _, id in ipairs(Suite.ORDER) do
  local g = require("bench.groups." .. id)
  Suite.groups[#Suite.groups + 1] = g
  Suite.byId[id] = g
end

--- The state a run fills in: results[groupId][rowId][memberId] = cell, specialists[lib] = cell, loads (from libload).
function Suite.newResults() return { groups = {}, specialists = {}, done = 0, total = 0 } end

--- How many steps a whole run is (cells + specialists), for a progress bar.
function Suite.total()
  local n = 0
  for _, g in ipairs(Suite.groups) do n = n + #runner.rows(g) * #g.members end
  return n + #specialists.list
end

--- A function that does one step each call and returns (done, total), or nil when everything has run. `only` limits it to one group id.
function Suite.steps(results, env, only)
  local plan = {}
  for _, g in ipairs(Suite.groups) do
    if not only or only == g.id then
      for _, row in ipairs(runner.rows(g)) do
        for _, m in ipairs(g.members) do plan[#plan + 1] = { g = g, row = row, m = m } end
      end
    end
  end
  if not only then for _, e in ipairs(specialists.list) do plan[#plan + 1] = { spec = e } end end
  results.total, results.done = #plan, 0
  local i, rated = 0, {}
  return function()
    i = i + 1
    local p = plan[i]
    if not p then
      -- Rate every row once all its cells are in.
      return nil
    end
    if p.spec then
      local r = libload.load(p.spec.lib, env)
      local cell
      if r.status == "SKIP" then cell = { status = "SKIP", note = r.detail }
      elseif r.status == "FAIL" then cell = { status = "FAIL", note = "did not load: " .. r.detail }
      else
        local key = "specialists/" .. p.spec.lib
        if libload.crashed[key] then cell = { status = "FAIL", note = "aborted the whole program last time it ran (a C assert inside the library): skipped now" }
        else cell = libload.guard(p.spec.lib, key, function() return libload.withPath(p.spec.lib, function() return specialists.run(p.spec, r.mod, env) end) end) end
      end
      results.specialists[p.spec.lib] = cell
    else
      local gr = results.groups[p.g.id]
      if not gr then gr = {}; results.groups[p.g.id] = gr end
      local rr = gr[p.row.id]
      if not rr then rr = {}; gr[p.row.id] = rr end
      rr[p.m.id] = runner.cell(p.g, p.row, p.m, env)
      if #p.g.members == 0 or next(rr) and (function() local n = 0; for _ in pairs(rr) do n = n + 1 end; return n end)() == #p.g.members then
        rating.rate(p.row, rr)
      end
    end
    results.done = i
    return i, #plan
  end
end

--- Run everything now.
function Suite.runAll(env, only)
  local results = Suite.newResults()
  local step = Suite.steps(results, env, only)
  while step() do end
  return results
end

--- Scores of a group, honouring the person's own ratings: manual(groupId, rowId, memberId) -> 1..5 or nil.
function Suite.scores(group, results, manual)
  local rows = runner.rows(group)
  local order = runner.order(group)
  return rating.groupScores(rows, order, results.groups[group.id] or {}, manual and function(rowId, memberId) return manual(group.id, rowId, memberId) end)
end

--- The libraries each group covers, and which have no group (their own check is in specialists).
function Suite.coverage()
  local inGroup, out = {}, {}
  for _, g in ipairs(Suite.groups) do
    for _, m in ipairs(g.members) do
      inGroup[m.lib] = inGroup[m.lib] or {}
      inGroup[m.lib][#inGroup[m.lib] + 1] = g.id
    end
  end
  for _, e in ipairs(libload.entries) do out[#out + 1] = { name = e.name, groups = inGroup[e.name] or {} } end
  return out
end

return Suite
