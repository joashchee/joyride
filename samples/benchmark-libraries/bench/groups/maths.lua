-- The small maths every game writes by hand: clamp, round, lerp, smoothing, wrapping, angles. lume against batteries.mathx.

local U = require("bench.util")
local close = U.close

return {
  id = "maths", title = "Maths helpers",
  about = "Clamp, round, interpolate, smooth, ping-pong, wrap, distance and angles.",
  members = {
    { id = "lume", short = "lume", lib = "lume" },
    { id = "mathx", short = "mathx", lib = "batteries", get = function(b) return b.mathx end },
  },
  rows = {
    { id = "clamp", label = "Clamp", kind = "feature", about = "5 into 0..3 is 3; -1 is 0; 2 stays 2.",
      impl = {
        lume = function(l) return l.clamp(5, 0, 3) == 3 and l.clamp(-1, 0, 3) == 0 and l.clamp(2, 0, 3) == 2 end,
        mathx = function(m) return m.clamp(5, 0, 3) == 3 and m.clamp(-1, 0, 3) == 0 and m.clamp(2, 0, 3) == 2 end,
      } },
    { id = "round", label = "Round", kind = "feature", about = "2.5 rounds to 3, -2.5 to -3 (away from zero), and 1.234 to one decimal is 1.2.",
      impl = {
        lume = function(l) return l.round(2.5) == 3 and l.round(-2.5) == -3 and close(l.round(1.234, 0.1), 1.2) end,
        mathx = function(m) return m.round(2.5) == 3 and m.round(-2.5) == -3 and close(m.to_precision(1.234, 1), 1.2) end,
      } },
    { id = "lerp", label = "Lerp", kind = "feature", about = "Halfway from 0 to 10 is 5; t = 0 and t = 1 give the ends exactly.",
      impl = {
        lume = function(l) return l.lerp(0, 10, 0.5) == 5 and l.lerp(2, 9, 0) == 2 and l.lerp(2, 9, 1) == 9 end,
        mathx = function(m) return m.lerp(0, 10, 0.5) == 5 and m.lerp(2, 9, 0) == 2 and m.lerp(2, 9, 1) == 9 end,
      } },
    { id = "smooth", label = "Smoothstep", kind = "feature", about = "An eased 0..10 is 5 in the middle and slower than linear at 0.25 (below 2.5).",
      impl = {
        lume = function(l) return close(l.smooth(0, 10, 0.5), 5) and l.smooth(0, 10, 0.25) < 2.5 end,
        mathx = function(m) return close(m.smoothstep(0.5) * 10, 5) and m.smoothstep(0.25) * 10 < 2.5 end,
      } },
    { id = "pingpong", label = "Ping-pong", kind = "feature", about = "A value that runs up then back down: 0 at 0 and 2, 1 at 1.",
      impl = {
        lume = function(l) return close(l.pingpong(0), 0) and close(l.pingpong(1), 1) and close(l.pingpong(2), 0) end,
        mathx = function(m) return close(m.pingpong(0), 0) and close(m.pingpong(0.5), 1) and close(m.pingpong(1), 0) end,
      } },
    { id = "wrap", label = "Wrap", kind = "feature", about = "5 wrapped into 0..3 is 2; -1 is 2.",
      impl = {
        lume = false,
        mathx = function(m) return m.wrap(5, 0, 3) == 2 and m.wrap(-1, 0, 3) == 2 end,
      } },
    { id = "remap", label = "Remap a range", kind = "feature", about = "5 from 0..10 to 0..100 is 50.",
      impl = {
        lume = false,
        mathx = function(m) return close(m.remap_range(5, 0, 10, 0, 100), 50) end,
      } },
    { id = "distance", label = "Distance, angle", kind = "feature", about = "From (0,0) to (3,4) is 5; the angle to (1,1) is a quarter-pi.",
      impl = {
        lume = function(l) return close(l.distance(0, 0, 3, 4), 5) and close(l.angle(0, 0, 1, 1), math.pi / 4) end,
        mathx = function(m) return close(m.distance(0, 0, 3, 4), 5) and close(m.angle_difference(0, math.pi / 4), math.pi / 4) end,
      } },
    { id = "sign", label = "Sign", kind = "feature", about = "-3 is -1, 4 is 1.",
      impl = {
        lume = function(l) return l.sign(-3) == -1 and l.sign(4) == 1 end,
        mathx = function(m) return m.sign(-3) == -1 and m.sign(4) == 1 end,
      } },
    { id = "speed", label = "10000 clamp+lerp", kind = "speed", unit = "ops/s", units = 10000, about = "Clamp and interpolate 10000 numbers.",
      expect = function(v) return v > 0, v end,
      impl = {
        lume = function(l) local clamp, lerp = l.clamp, l.lerp; return function()
          local s = 0; for i = 1, 10000 do s = s + lerp(0, 1, clamp(i / 5000, 0, 1)) end; return s end end,
        mathx = function(m) local clamp, lerp = m.clamp, m.lerp; return function()
          local s = 0; for i = 1, 10000 do s = s + lerp(0, 1, clamp(i / 5000, 0, 1)) end; return s end end,
      } },
  },
}
