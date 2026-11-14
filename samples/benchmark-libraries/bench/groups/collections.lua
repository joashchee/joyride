-- Working on arrays and tables: map, filter, reduce, find, copy, merge. lume is one file of everything; batteries splits it into
-- functional and tablex (the two together are what is compared).

local U = require("bench.util")

local function seq(n) local t = {}; for i = 1, n do t[i] = i end; return t end
local function J(t) return table.concat(t, ",") end
local function even(x) return x % 2 == 0 end
local function double(x) return x * 2 end

return {
  id = "collections", title = "Arrays and tables",
  about = "Map, filter, reduce, search, copy and merge: lume against batteries' functional and tablex.",
  members = {
    { id = "lume", short = "lume", lib = "lume" },
    { id = "batteries", short = "batt", lib = "batteries", get = function(b) return { f = b.functional, t = b.tablex } end },
  },
  rows = {
    { id = "map", label = "map", kind = "feature", about = "{1,2,3} doubled is {2,4,6}.",
      impl = {
        lume = function(l) return J(l.map({ 1, 2, 3 }, double)) == "2,4,6" end,
        batteries = function(b) return J(b.f.map({ 1, 2, 3 }, double)) == "2,4,6" end,
      } },
    { id = "filter", label = "filter", kind = "feature", about = "Keep the even numbers of 1 to 6: {2,4,6}.",
      impl = {
        lume = function(l) return J(l.filter(seq(6), even)) == "2,4,6" end,
        batteries = function(b) return J(b.f.filter(seq(6), even)) == "2,4,6" end,
      } },
    { id = "reduce", label = "reduce", kind = "feature", about = "Add up 1 to 10: 55.",
      impl = {
        lume = function(l) return l.reduce(seq(10), function(a, x) return a + x end) == 55 end,
        batteries = function(b) return b.f.reduce(seq(10), 0, function(a, x) return a + x end) == 55 end,
      } },
    { id = "find", label = "find a value", kind = "feature", about = "The position of \"b\" in {a,b,c} is 2; a missing value gives nil.",
      impl = {
        lume = function(l) return l.find({ "a", "b", "c" }, "b") == 2 and l.find({ "a" }, "z") == nil end,
        batteries = function(b) return b.t.index_of({ "a", "b", "c" }, "b") == 2 and b.t.index_of({ "a" }, "z") == nil end,
      } },
    { id = "slice", label = "slice", kind = "feature", about = "Elements 2 to 3 of {1,2,3,4}: {2,3}.",
      impl = {
        lume = function(l) return J(l.slice({ 1, 2, 3, 4 }, 2, 3)) == "2,3" end,
        batteries = false,
      } },
    { id = "count", label = "count / any / all", kind = "feature", about = "How many are even; whether any is, whether all are.",
      impl = {
        lume = function(l) return l.count(seq(6), even) == 3 and l.any(seq(6), even) and not l.all(seq(6), even) end,
        batteries = function(b) return b.f.count(seq(6), even) == 3 and b.f.any(seq(6), even) and not b.f.all(seq(6), even) end,
      } },
    { id = "unique", label = "Remove duplicates", kind = "feature", about = "{1,1,2,3,3} without repeats: {1,2,3}.",
      impl = {
        lume = function(l) return J(l.set({ 1, 1, 2, 3, 3 })) == "1,2,3" end,
        batteries = function(b) return J(b.t.dedupe({ 1, 1, 2, 3, 3 })) == "1,2,3" end,
      } },
    { id = "partition", label = "Group / partition", kind = "feature", about = "Split 1 to 6 into evens and odds in one call.",
      impl = {
        lume = false,
        batteries = function(b) local a, c = b.f.partition(seq(6), even); return J(a) == "2,4,6" and J(c) == "1,3,5" end,
      } },
    { id = "zip", label = "Zip", kind = "feature", about = "Combine {1,2} with {a,b} element by element.",
      impl = {
        lume = false,
        batteries = function(b) local z = b.f.zip({ 1, 2 }, { "a", "b" }); return #z == 4 and z[1] == 1 and z[2] == "a" and z[4] == "b", "interleaves into one flat list, not pairs" end,
      } },
    { id = "shuffle", label = "Shuffle", kind = "feature", about = "A shuffled 1 to 20 holds the same numbers, and a seeded stream gives the same order twice.",
      impl = {
        lume = function(l)
          local function once() math.randomseed(7); return J(l.shuffle(seq(20))) end
          local a, b = once(), once()
          local sorted = {}; for n in a:gmatch("%d+") do sorted[#sorted + 1] = tonumber(n) end; table.sort(sorted)
          return J(sorted) == J(seq(20)) and a == b
        end,
        batteries = function(b)
          local H = require("bench.harness")
          local function once() local rnd = H.rng(7); return J(b.t.shuffle(seq(20), { random = function(_, lo, hi) return H.int(rnd, lo, hi) end })) end
          local x, y = once(), once()
          local sorted = {}; for n in x:gmatch("%d+") do sorted[#sorted + 1] = tonumber(n) end; table.sort(sorted)
          return J(sorted) == J(seq(20)) and x == y, "takes its own random function, so it is repeatable without touching math.random"
        end,
      } },
    { id = "clone", label = "Copy (shallow)", kind = "feature", about = "A copy has the same entries and is a different table; inner tables are shared.",
      impl = {
        lume = function(l) local a = { 1, { 2 } }; local c = l.clone(a); return c ~= a and c[2] == a[2] and c[1] == 1 end,
        batteries = function(b) local a = { 1, { 2 } }; local c = b.t.shallow_copy(a); return c ~= a and c[2] == a[2] and c[1] == 1 end,
      } },
    { id = "deepcopy", label = "Copy (deep)", kind = "feature", about = "A deep copy shares nothing, including a cycle.",
      impl = {
        lume = false,
        batteries = function(b) local a = { 1, { 2 } }; a.me = a; local c = b.t.deep_copy(a); return c[2] ~= a[2] and c[2][1] == 2 and c.me == c end,
      } },
    { id = "merge", label = "Merge", kind = "feature", about = "Put the fields of {b=2} into {a=1}: both present, the first table changed, and the second left alone.",
      impl = {
        lume = function(l) local a, b = { a = 1 }, { b = 2 }; local r = l.extend(a, b); return r == a and a.a == 1 and a.b == 2 and b.a == nil end,
        batteries = function(b) local a, c = { a = 1 }, { b = 2 }; local r = b.t.shallow_overlay(a, c); return (r == a or a.b == 2) and a.a == 1 and a.b == 2 and c.a == nil end,
      } },
    { id = "keys", label = "Keys", kind = "feature", about = "The keys of {a=1,b=2}, in any order.",
      impl = {
        lume = function(l) local k = l.keys({ a = 1, b = 2 }); table.sort(k); return J(k) == "a,b" end,
        batteries = function(b) local k = b.t.keys({ a = 1, b = 2 }); table.sort(k); return J(k) == "a,b" end,
      } },
    { id = "pipeline", label = "map+filter+reduce", kind = "speed", unit = "elements/s", units = 1000, about = "Double 1000 numbers, keep the multiples of 4, add them.",
      expect = function(v) return v == 501000, v end,
      impl = {
        lume = function(l) local data = seq(1000); return function()
          local m = l.map(data, double); local f = l.filter(m, function(x) return x % 4 == 0 end)
          return l.reduce(f, function(a, x) return a + x end)
        end end,
        batteries = function(b) local data = seq(1000); return function()
          local m = b.f.map(data, double); local f = b.f.filter(m, function(x) return x % 4 == 0 end)
          return b.f.reduce(f, 0, function(a, x) return a + x end)
        end end,
      } },
    { id = "copy-speed", label = "Copy 1000", kind = "speed", unit = "elements/s", units = 1000, about = "Shallow-copy a 1000-element array.",
      expect = function(n) return n == 1000 end,
      impl = {
        lume = function(l) local data = seq(1000); return function() return #l.clone(data) end end,
        batteries = function(b) local data = seq(1000); return function() return #b.t.shallow_copy(data) end end,
      } },
  },
}
