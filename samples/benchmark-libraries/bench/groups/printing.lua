-- Turning a table into readable text for the console. inspect.lua is the long-standing one; batteries.pretty is the bundle's own.

local U = require("bench.util")

local function has(s, ...) for _, w in ipairs({ ... }) do if not s:find(w, 1, true) then return false end end; return true end

local function viaInspect(f) return function(inspect) return f(function(v, o) return inspect(v, o) end) end end

return {
  id = "printing", title = "Printing tables",
  about = "Readable text for a table: every key, the same order every time, cycles, metatables, depth.",
  members = {
    { id = "inspect", short = "inspct", lib = "inspect" },
    { id = "batteries.pretty", short = "pretty", lib = "batteries", get = function(b) return b.pretty end },
  },
  rows = {
    { id = "nested", label = "Nested table", kind = "feature", about = "The text for a save-like record names every key and value, including the ones nested two deep.",
      impl = {
        inspect = function(inspect) local s = inspect(U.record()); return has(s, "name", "Joyride", "health", "sword", "potion", "12.5") end,
        ["batteries.pretty"] = function(p) local s = p.string(U.record()); return has(s, "name", "Joyride", "health", "sword", "potion", "12.5") end,
      } },
    { id = "order", label = "Stable key order", kind = "feature", about = "{c=3, a=1, b=2} prints a, then b, then c, whatever order Lua stored them in.",
      impl = {
        inspect = function(inspect) local s = inspect({ c = 3, a = 1, b = 2 }); return s:find("a =", 1, true) < s:find("b =", 1, true) and s:find("b =", 1, true) < s:find("c =", 1, true) end,
        ["batteries.pretty"] = function(p) local s = p.string({ c = 3, a = 1, b = 2 }); return s:find("a =", 1, true) < s:find("b =", 1, true) and s:find("b =", 1, true) < s:find("c =", 1, true) end,
      } },
    { id = "cycle", label = "Cycles", kind = "feature", about = "A table that holds itself prints without looping forever.",
      impl = {
        inspect = function(inspect) local t = { n = 1 }; t.me = t; return type(inspect(t)) == "string" end,
        ["batteries.pretty"] = function(p) local t = { n = 1 }; t.me = t; return type(p.string(t)) == "string" end,
      } },
    { id = "meta", label = "Metatables", kind = "feature", about = "A table with a metatable shows it.",
      impl = {
        inspect = function(inspect) return has(inspect(setmetatable({ v = 1 }, { __index = {} })), "metatable") end,
        ["batteries.pretty"] = function(p) local s = p.string(setmetatable({ v = 1 }, { __index = {} })); return has(s, "metatable", "__index"), "it does not print the metatable" end,
      } },
    { id = "depth", label = "Depth limit", kind = "feature", about = "Ask for only the first level of a deeply nested table: the deep part is cut.",
      impl = {
        inspect = function(inspect) local s = inspect({ a = { b = { c = "deep" } } }, { depth = 1 }); return not s:find("deep", 1, true) end,
        ["batteries.pretty"] = false,
      } },
    { id = "speed", label = "Print 200 units", kind = "speed", unit = "prints/s", units = 1, about = "Turn the 200-unit crowd into text.",
      expect = function(n) return n > 1000, n end,
      impl = {
        inspect = function(inspect) local d = U.crowd(); return function() return #inspect(d) end end,
        ["batteries.pretty"] = function(p) local d = U.crowd(); return function() return #p.string(d) end end,
      } },
  },
}
