dofile("tests/lovestub.lua")
package.path = "./?.lua;" .. package.path
local suite = require("bench.suite")
local t0 = os.clock()
local res = suite.runAll({ ffi = "need" })
print(("%d steps in %.1f s"):format(res.done, os.clock() - t0))
for _, g in ipairs(suite.groups) do
  local sc = suite.scores(g, res)
  local t = {}
  for _, s in ipairs(sc) do t[#t + 1] = ("%s %s%s"):format(s.id, s.score and string.format("%.2f", s.score) or "-", s.best and "*" or "") end
  print(g.id, table.concat(t, "  "))
end
for lib, c in pairs(res.specialists) do print(lib, c.status, c.note) end
