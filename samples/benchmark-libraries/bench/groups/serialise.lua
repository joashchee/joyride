-- Turning tables into text or bytes and back, for saves and messages. json is the portable text format; lume.serialize and serpent write Lua
-- source; binser and bitser are compact binary formats that keep more types.

local U = require("bench.util")

local function enc(m, v)
  if m.id == "json" then return m.lib.encode(v)
  elseif m.id == "bitser" then return m.lib.dumps(v)
  elseif m.id == "serpent" then return m.lib.dump(v)
  else return m.lib.serialize(v) end
end

local function dec(m, s)
  if m.id == "json" then return m.lib.decode(s)
  elseif m.id == "bitser" then return m.lib.loads(s)
  elseif m.id == "serpent" then return select(2, m.lib.load(s))   -- load returns ok, value
  elseif m.id == "lume" then return m.lib.deserialize(s)
  else return (m.lib.deserialize(s))[1] end   -- binser returns a list of the values, and how many
end

local function rowFor(fn)
  -- impl table where each member runs fn(roundtrip, encode)
  local out = {}
  for _, name in ipairs({ "binser", "bitser", "serpent", "lume", "json" }) do
    out[name] = function(lib)
      local m = { id = name, lib = lib }
      return fn(function(v) return dec(m, enc(m, v)) end, function(v) return enc(m, v) end, name)
    end
  end
  return out
end

local function speedImpl(which)
  local out = {}
  for _, name in ipairs({ "binser", "bitser", "serpent", "lume", "json" }) do
    out[name] = function(lib)
      local m = { id = name, lib = lib }
      local data = U.crowd()
      local text = enc(m, data)
      if which == "encode" then return function() return #enc(m, data) end end
      return function() return U.count(dec(m, text).units) end
    end
  end
  return out
end

return {
  id = "serialise", title = "Serialising tables",
  about = "Save a table to text or bytes and read it back: how much survives, how big, how fast.",
  members = {
    { id = "binser", short = "binser", lib = "binser" },
    { id = "bitser", short = "bitser", lib = "bitser" },
    { id = "serpent", short = "serp", lib = "serpent" },
    { id = "lume", short = "lume", lib = "lume" },
    { id = "json", short = "json", lib = "json" },
  },
  rows = {
    { id = "roundtrip", label = "Round trip", kind = "feature", about = "A save-like record (strings, numbers, booleans, arrays, nested tables) comes back equal.",
      impl = rowFor(function(rt) return U.same(rt(U.record()), U.record()) end) },
    { id = "intkeys", label = "1 and \"1\" apart", kind = "feature", about = "A table with the number key 1 and the string key \"1\" keeps both.",
      impl = rowFor(function(rt)
        local back = rt({ [1] = "num", ["1"] = "str" })
        local ok = back[1] == "num" and back["1"] == "str"
        return ok, ok or "the two keys became one"
      end) },
    { id = "precision", label = "Full precision", kind = "feature", about = "1/3 and 0.1 + 0.2 come back as exactly the same numbers.",
      impl = rowFor(function(rt)
        local back = rt({ a = 1 / 3, b = 0.1 + 0.2, c = 2 ^ 53 })
        local ok = back.a == 1 / 3 and back.b == 0.1 + 0.2 and back.c == 2 ^ 53
        return ok, ok or ("1/3 came back as %.17g"):format(back.a)
      end) },
    { id = "inf", label = "inf and -inf", kind = "feature", about = "math.huge and -math.huge survive (JSON has no such numbers).",
      impl = rowFor(function(rt)
        local back = rt({ a = math.huge, b = -math.huge })
        return back.a == math.huge and back.b == -math.huge
      end) },
    { id = "cycle", label = "Cycles", kind = "feature", about = "A table that contains itself can be saved and comes back containing itself.",
      impl = rowFor(function(rt)
        local t = { n = 1 }; t.self = t
        local back = rt(t)
        return back.n == 1 and back.self == back
      end) },
    { id = "bytes", label = "Binary strings", kind = "feature", about = "A string with a NUL, a quote, a newline and a high byte comes back byte for byte.",
      impl = rowFor(function(rt)
        local s = "a\0b\"\n\255z"
        return rt({ s = s }).s == s
      end) },
    { id = "size", label = "Output size", kind = "value", lower = true, unit = "bytes", floor = 1, about = "Bytes the 200-unit crowd takes once saved.",
      impl = (function()
        local out = {}
        for _, name in ipairs({ "binser", "bitser", "serpent", "lume", "json" }) do
          out[name] = function(lib)
            local m = { id = name, lib = lib }
            local data = U.crowd()
            -- a value row: the number the work returns is the measurement (bytes), not how long it took
            return function() return #enc(m, data) end
          end
        end
        return out
      end)() },
    { id = "encode", label = "Save speed", kind = "speed", unit = "saves/s", units = 1, about = "Serialise the 200-unit crowd.",
      expect = function(n) return n > 100, n end, impl = speedImpl("encode") },
    { id = "decode", label = "Load speed", kind = "speed", unit = "loads/s", units = 1, about = "Read the 200-unit crowd back.",
      expect = function(n) return n == 200, n end, impl = speedImpl("decode") },
  },
}
