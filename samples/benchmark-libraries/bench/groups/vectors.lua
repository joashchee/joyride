-- 2D vectors. hump.vector makes a new object for every result (operators, reads well); hump.vector-light is plain functions on
-- numbers (no objects at all); batteries.vec2 is an object too, but its main methods work in place (no garbage).

local function close(a, b) return math.abs(a - b) < 1e-9 end
local function pair(x, y, a, b) return close(x, a) and close(y, b) end

return {
  id = "vectors", title = "2D vectors",
  about = "Add, scale, length, normalise, rotate, dot, distance and project: as operators, as functions, or in place.",
  members = {
    { id = "hump.vector", short = "vector", lib = "hump", get = function(_, req) return req("hump.vector") end },
    { id = "vector-light", short = "vec-lt", lib = "hump", get = function(_, req) return req("hump.vector-light") end },
    { id = "batteries.vec2", short = "vec2", lib = "batteries", get = function(m) return m.vec2 end },
    { id = "cpml.vec2", short = "cpml", lib = "cpml", get = function(c) return c.vec2 end },
  },
  rows = {
    { id = "add", label = "Add, scale", kind = "feature", about = "(1,2) + (3,4) = (4,6), and (1,2) * 2 = (2,4).",
      impl = {
        ["hump.vector"] = function(v) local a = v(1, 2) + v(3, 4); local b = v(1, 2) * 2; return pair(a.x, a.y, 4, 6) and pair(b.x, b.y, 2, 4) end,
        ["vector-light"] = function(v)
          local ax, ay = v.add(1, 2, 3, 4); local bx, by = v.mul(2, 1, 2)
          return pair(ax, ay, 4, 6) and pair(bx, by, 2, 4)
        end,
        ["batteries.vec2"] = function(v) local a = v(1, 2) + v(3, 4); local b = v(1, 2) * 2; return pair(a.x, a.y, 4, 6) and pair(b.x, b.y, 2, 4) end,
        ["cpml.vec2"] = function(v) local a = v(1, 2) + v(3, 4); local b = v(1, 2) * 2; return pair(a.x, a.y, 4, 6) and pair(b.x, b.y, 2, 4) end,
      } },
    { id = "length", label = "Length, distance", kind = "feature", about = "|(3,4)| = 5, and the distance from (0,0) to (3,4) is 5.",
      impl = {
        ["hump.vector"] = function(v) return close(v(3, 4):len(), 5) and close(v(0, 0):dist(v(3, 4)), 5) end,
        ["vector-light"] = function(v) return close(v.len(3, 4), 5) and close(v.dist(0, 0, 3, 4), 5) end,
        ["batteries.vec2"] = function(v) return close(v(3, 4):length(), 5) and close(v(0, 0):distance(v(3, 4)), 5) end,
        ["cpml.vec2"] = function(v) return close(v(3, 4):len(), 5) and close(v(0, 0):dist(v(3, 4)), 5) end,
      } },
    { id = "normalise", label = "Normalise", kind = "feature", about = "(3,4) becomes (0.6,0.8); a zero vector does not give NaN.",
      impl = {
        ["hump.vector"] = function(v) local n = v(3, 4):normalized(); local z = v(0, 0):normalized(); return pair(n.x, n.y, 0.6, 0.8) and z.x == z.x end,
        ["vector-light"] = function(v) local x, y = v.normalize(3, 4); local zx = v.normalize(0, 0); return pair(x, y, 0.6, 0.8) and zx == zx end,
        ["batteries.vec2"] = function(v) local n = v(3, 4):normalise_inplace(); local z = v(0, 0):normalise_inplace(); return pair(n.x, n.y, 0.6, 0.8) and z.x == z.x end,
        ["cpml.vec2"] = function(v) local n = v(3, 4):normalize(); local z = v(0, 0):normalize(); return pair(n.x, n.y, 0.6, 0.8) and z.x == z.x end,
      } },
    { id = "rotate", label = "Rotate", kind = "feature", about = "(1,0) turned by a quarter circle is (0,1).",
      impl = {
        ["hump.vector"] = function(v) local r = v(1, 0):rotated(math.pi / 2); return pair(r.x, r.y, 0, 1) end,
        ["vector-light"] = function(v) local x, y = v.rotate(math.pi / 2, 1, 0); return pair(x, y, 0, 1) end,
        ["batteries.vec2"] = function(v) local r = v(1, 0):rotate_inplace(math.pi / 2); return pair(r.x, r.y, 0, 1) end,
        ["cpml.vec2"] = function(v) local r = v(1, 0):rotate(math.pi / 2); return pair(r.x, r.y, 0, 1) end,
      } },
    { id = "dot", label = "Dot, project", kind = "feature", about = "(1,2).(3,4) = 11, and (2,2) projected on the x axis is (2,0).",
      impl = {
        ["hump.vector"] = function(v) local p = v(2, 2):projectOn(v(1, 0)); return close(v(1, 2) * v(3, 4), 11) and pair(p.x, p.y, 2, 0) end,
        ["vector-light"] = function(v) local x, y = v.project(2, 2, 1, 0); return close(v.dot(1, 2, 3, 4), 11) and pair(x, y, 2, 0) end,
        ["batteries.vec2"] = function(v) local p = v(2, 2):vector_projection_inplace(v(1, 0)); return close(v(1, 2):dot(v(3, 4)), 11) and pair(p.x, p.y, 2, 0) end,
        ["cpml.vec2"] = false,   -- has dot but no projection onto a vector
      } },
    { id = "lerp", label = "Interpolate", kind = "feature", about = "Halfway between (1,2) and (3,4) is (2,3).",
      impl = {
        ["hump.vector"] = false,
        ["vector-light"] = false,
        ["batteries.vec2"] = function(v) local r = v(1, 2):lerp_inplace(v(3, 4), 0.5); return pair(r.x, r.y, 2, 3) end,
        ["cpml.vec2"] = function(v) local r = v.lerp(v(1, 2), v(3, 4), 0.5); return pair(r.x, r.y, 2, 3) end,
      } },
    { id = "speed", label = "1000 steps", kind = "speed", unit = "steps/s", units = 1000, about = "Move a point 1000 times: add a velocity, scale by 0.5, normalise, scale back, keep the length.",
      expect = function(len) return len > 0.999 and len < 1.001, len end,
      impl = {
        ["hump.vector"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p = (p + d) * 0.5; p = p:normalized() * 1 end
          return p:len()
        end end,
        ["vector-light"] = function(v) return function()
          local px, py, dx, dy = 0, 0, 1, 2
          for _ = 1, 1000 do
            px, py = v.add(px, py, dx, dy); px, py = v.mul(0.5, px, py); px, py = v.normalize(px, py)
          end
          return v.len(px, py)
        end end,
        ["batteries.vec2"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p:vector_add_inplace(d); p:scalar_mul_inplace(0.5); p:normalise_inplace() end
          return p:length()
        end end,
        ["cpml.vec2"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p = (p + d) * 0.5; p = p:normalize() end
          return p:len()
        end end,
      } },
    { id = "garbage", label = "Garbage made", kind = "memory", lower = true, floor = 1, unit = "KB", about = "Memory allocated by those 1000 steps (what the collector must clean up).",
      expect = function(len) return len > 0.999 and len < 1.001, len end,
      impl = {
        ["hump.vector"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p = (p + d) * 0.5; p = p:normalized() * 1 end
          return p:len()
        end end,
        ["vector-light"] = function(v) return function()
          local px, py, dx, dy = 0, 0, 1, 2
          for _ = 1, 1000 do
            px, py = v.add(px, py, dx, dy); px, py = v.mul(0.5, px, py); px, py = v.normalize(px, py)
          end
          return v.len(px, py)
        end end,
        ["batteries.vec2"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p:vector_add_inplace(d); p:scalar_mul_inplace(0.5); p:normalise_inplace() end
          return p:length()
        end end,
        ["cpml.vec2"] = function(v) return function()
          local p, d = v(0, 0), v(1, 2)
          for _ = 1, 1000 do p = (p + d) * 0.5; p = p:normalize() end
          return p:len()
        end end,
      } },
  },
}
