-- Finding out what is slow. profile.lua hooks every call and reports per function; batteries.measure times and counts a function you
-- hand it, many times over.

local function work() local s = 0; for i = 1, 20000 do s = s + i end; return s end

return {
  id = "profiling", title = "Timing and profiling",
  about = "Time a function, count the memory it uses, find which functions the time went to.",
  members = {
    { id = "profile", short = "prof", lib = "profile" },
    { id = "batteries.measure", short = "meas", lib = "batteries", get = function(b) return b.measure end },
  },
  rows = {
    { id = "time", label = "Time a function", kind = "feature", about = "How long a small function takes: a number, above zero.",
      impl = {
        profile = function(P)
          P.reset(); P.start(); for _ = 1, 3 do work() end; P.stop()
          local report = P.report(10)
          return type(report) == "string" and report:find("work", 1, true) ~= nil, "a table of functions with times"
        end,
        ["batteries.measure"] = function(m) local mean = m.time_taken(work, 10, 2); return type(mean) == "number" and mean >= 0 end,
      } },
    { id = "perfn", label = "Which function", kind = "feature", about = "Run a program of three functions and be told which of them took the time.",
      impl = {
        profile = function(P)
          local function cheap() return 1 end
          local function costly() local s = 0; for i = 1, 50000 do s = s + i end; return s end
          local function program() for _ = 1, 20 do cheap() end; costly() end
          P.reset(); P.start(); program(); P.stop()
          local top = P.query(1)
          return top and top[1] and top[1][2] ~= nil
        end,
        ["batteries.measure"] = false,
      } },
    { id = "memory", label = "Memory used", kind = "feature", about = "How much memory a function allocates per run.",
      impl = {
        profile = false,
        ["batteries.measure"] = function(m) local mean = m.memory_taken(function() local t = {}; for i = 1, 100 do t[i] = {} end end, 20, 2); return type(mean) == "number" and mean > 0 end,
      } },
    { id = "stats", label = "Mean, min, max", kind = "feature", about = "Many runs summarised as a mean, a fastest and a slowest.",
      impl = {
        profile = false,
        ["batteries.measure"] = function(m) local mean, min, max = m.time_taken(work, 20, 2); return mean and min and max and min <= mean and mean <= max end,
      } },
    { id = "overhead", label = "Cost of measuring", kind = "speed", unit = "runs/s", units = 1, about = "How many times a 20000-step function runs per second while measuring is switched on (higher is cheaper).",
      expect = function(v) return v == 200010000, v end,
      impl = {
        profile = function(P) return function()
          P.reset(); P.start(); local v = work(); P.stop()
          return v
        end end,
        ["batteries.measure"] = function(m) return function()
          local v; m.time_taken(function() v = work() end, 1, 0)
          return v
        end end,
      } },
  },
}
