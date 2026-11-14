-- Scratch runner under plain luajit: lua tests/try.lua <group>
dofile("tests/lovestub.lua")package.path = "./?.lua;" .. package.path
local libload = require("lib.libload")
local runner = require("bench.runner")
local rating = require("bench.rating")
local g = require("bench.groups." .. (arg[1] or "classes"))
local res = runner.group(g, { ffi = "need" })
local order = runner.order(g)
for _, row in ipairs(runner.rows(g)) do
  local t = {}
  for _, id in ipairs(order) do local c = res[row.id][id]; t[#t + 1] = ("%s=%s%s/%s"):format(id, c.status, c.value and ("(" .. string.format("%.3g", c.value) .. ")") or "", tostring(c.auto)) end
  print(row.id, table.concat(t, "  "))
  for _, id in ipairs(order) do local c = res[row.id][id]; if c.status == "FAIL" or c.status == "SKIP" then print("   ", id, c.note) end end
end
for _, s in ipairs(rating.groupScores(runner.rows(g), order, res)) do print(s.id, s.score, s.best) end
