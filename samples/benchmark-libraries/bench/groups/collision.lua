-- Collision. bump moves boxes through a world and stops them at walls (swept: a fast box cannot jump a thin wall). HC keeps shapes of
-- any convex kind and tells you how far to push them apart. batteries.intersect is bare functions, no world, no broad phase.

local H = require("bench.harness")
local U = require("bench.util")

-- The same 300 boxes every run: a seeded scatter over a 1000 x 1000 field. Fractional edges, so no two boxes merely touch (libraries
-- disagree on whether touching is overlapping, and that is not what this row measures).
local function boxes()
  local rnd, t = H.rng(5), {}
  for i = 1, 300 do t[i] = { x = H.int(rnd, 0, 960) + 0.25, y = H.int(rnd, 0, 960) + 0.25, w = H.int(rnd, 10, 40) + 0.5, h = H.int(rnd, 10, 40) + 0.5 } end
  return t
end
local function overlapping(a, b) return a.x < b.x + b.w and b.x < a.x + a.w and a.y < b.y + b.h and b.y < a.y + a.h end
local REFERENCE
local function reference()
  if not REFERENCE then
    local bs, n = boxes(), 0
    for i = 1, #bs do for j = i + 1, #bs do if overlapping(bs[i], bs[j]) then n = n + 1 end end end
    REFERENCE = n
  end
  return REFERENCE
end

return {
  id = "collision", title = "Collision",
  about = "Do boxes, circles and polygons touch; stop a mover at a wall; find every overlap in a crowd.",
  members = {
    { id = "bump", short = "bump", lib = "bump" },
    { id = "HC", short = "HC", lib = "HC" },
    { id = "intersect", short = "intsct", lib = "batteries", get = function(b) return { I = b.intersect, vec2 = b.vec2 } end },
  },
  rows = {
    { id = "rect", label = "Box vs box", kind = "feature", about = "(0,0,10,10) overlaps (5,5,10,10) and does not overlap (20,20,5,5).",
      impl = {
        bump = function(bump)
          local w = bump.newWorld(32); local a, b, c = {}, {}, {}
          w:add(b, 5, 5, 10, 10); w:add(c, 20, 20, 5, 5)
          local items = w:queryRect(0, 0, 10, 10)
          return #items == 1 and items[1] == b
        end,
        HC = function(HC)
          local h = HC.new(32); local a = h:rectangle(0, 0, 10, 10)
          return a:collidesWith(h:rectangle(5, 5, 10, 10)) and not a:collidesWith(h:rectangle(20, 20, 5, 5))
        end,
        intersect = function(x)
          local v = x.vec2
          return x.I.aabb_aabb_overlap(v(5, 5), v(5, 5), v(10, 10), v(5, 5)) and not x.I.aabb_aabb_overlap(v(5, 5), v(5, 5), v(22.5, 22.5), v(2.5, 2.5))
        end,
      } },
    { id = "circle", label = "Circle vs circle", kind = "feature", about = "Radius 5 at (0,0) overlaps radius 5 at (6,0), not at (11,0).",
      impl = {
        bump = false,
        HC = function(HC)
          local h = HC.new(32); local a = h:circle(0, 0, 5)
          return a:collidesWith(h:circle(6, 0, 5)) and not a:collidesWith(h:circle(11, 0, 5))
        end,
        intersect = function(x)
          local v = x.vec2
          return x.I.circle_circle_overlap(v(0, 0), 5, v(6, 0), 5) and not x.I.circle_circle_overlap(v(0, 0), 5, v(11, 0), 5)
        end,
      } },
    { id = "point", label = "Point in box", kind = "feature", about = "(9,5) is inside (0,0,10,10); (11,5) is not.",
      impl = {
        bump = function(bump)
          local w = bump.newWorld(32); local a = {}; w:add(a, 0, 0, 10, 10)
          return #w:queryPoint(9, 5) == 1 and #w:queryPoint(11, 5) == 0
        end,
        HC = function(HC)
          local h = HC.new(32); local a = h:rectangle(0, 0, 10, 10)
          return a:contains(9, 5) and not a:contains(11, 5)
        end,
        intersect = function(x)
          local v = x.vec2
          return x.I.aabb_point_overlap(v(5, 5), v(5, 5), v(9, 5)) and not x.I.aabb_point_overlap(v(5, 5), v(5, 5), v(11, 5))
        end,
      } },
    { id = "polygon", label = "Polygon", kind = "feature", about = "A triangle (0,0) (10,0) (5,10) holds (5,2) and not (0,9).",
      impl = {
        bump = false,
        HC = function(HC)
          local h = HC.new(32); local tri = h:polygon(0, 0, 10, 0, 5, 10)
          return tri:contains(5, 2) and not tri:contains(0, 9)
        end,
        intersect = function(x)
          local v = x.vec2; local poly = { v(0, 0), v(10, 0), v(5, 10) }
          return x.I.point_in_poly(v(5, 2), poly) and not x.I.point_in_poly(v(0, 9), poly)
        end,
      } },
    { id = "separate", label = "Push apart", kind = "feature", about = "Two overlapping boxes (0,0,10,10) and (8,0,10,10): the way out is 2 along x.",
      impl = {
        bump = function(bump)
          local w = bump.newWorld(32); local a, b = {}, {}; w:add(a, 0, 0, 10, 10); w:add(b, 8, 0, 10, 10)
          local x = w:move(a, 0, 0)
          return x == -2
        end,
        HC = function(HC)
          local h = HC.new(32); local a = h:rectangle(0, 0, 10, 10); local b = h:rectangle(8, 0, 10, 10)
          local hit, dx, dy = a:collidesWith(b)
          return hit and math.abs(math.abs(dx) - 2) < 1e-9 and math.abs(dy) < 1e-9
        end,
        intersect = function(x)
          local v = x.vec2
          local msv = x.I.aabb_aabb_collide(v(5, 5), v(5, 5), v(13, 5), v(5, 5))
          return msv and math.abs(math.abs(msv.x) - 2) < 1e-9 and math.abs(msv.y) < 1e-9
        end,
      } },
    { id = "sweep", label = "No tunnelling", kind = "feature", about = "A box moved 100 in one step towards a thin wall at x = 50 stops at the wall instead of passing through.",
      impl = {
        bump = function(bump)
          local w = bump.newWorld(32); local body, wall = {}, {}; w:add(body, 0, 0, 10, 10); w:add(wall, 50, 0, 2, 10)
          local x = w:move(body, 100, 0)
          return x == 40
        end,
        HC = false,
        intersect = false,
      } },
    { id = "ray", label = "Ray cast", kind = "feature", about = "A ray along the x axis hits the box (0,0,10,10), whichever way it travels (left to right and right to left).",
      impl = {
        bump = function(bump)
          local w = bump.newWorld(32); local a = {}; w:add(a, 0, 0, 10, 10)
          return #w:querySegment(-5, 5, 15, 5) == 1 and #w:querySegment(15, 5, -5, 5) == 1
        end,
        HC = function(HC)
          local h = HC.new(32); h:rectangle(0, 0, 10, 10)
          local right, left = next(h:raycast(-5, 5, 1, 0, 100)), next(h:raycast(15, 5, -1, 0, 100))
          local ok = right ~= nil and left ~= nil
          return ok, ok or "raycast finds the box only for rays heading left or up (its search box is built the wrong way round for the others)"
        end,
        intersect = false,
      } },
    { id = "crowd", label = "Find all overlaps", kind = "speed", unit = "boxes/s", units = 300, about = "Count the overlapping pairs among 300 boxes: with a world and a grid, or (batteries) with a plain double loop.",
      expect = function(n) return n == reference(), ("%s pairs, expected %s"):format(tostring(n), tostring(reference())) end,
      impl = {
        bump = function(bump) local bs = boxes(); return function()
          local w = bump.newWorld(64); local items = {}
          for i, b in ipairs(bs) do items[i] = { id = i }; w:add(items[i], b.x, b.y, b.w, b.h) end
          local n = 0
          for i, b in ipairs(bs) do
            for _, o in ipairs(w:queryRect(b.x, b.y, b.w, b.h)) do if o.id > i then n = n + 1 end end
          end
          return n
        end end,
        HC = function(HC) local bs = boxes(); return function()
          local h = HC.new(64); local shapes, ids = {}, {}
          for i, b in ipairs(bs) do shapes[i] = h:rectangle(b.x, b.y, b.w, b.h); ids[shapes[i]] = i end
          local n = 0
          for i, s in ipairs(shapes) do
            for other in pairs(h:collisions(s)) do if ids[other] > i then n = n + 1 end end
          end
          return n
        end end,
        intersect = function(x) local bs = boxes(); local v = x.vec2; local pos, hs = {}, {}
          for i, b in ipairs(bs) do pos[i] = v(b.x + b.w / 2, b.y + b.h / 2); hs[i] = v(b.w / 2, b.h / 2) end
          return function()
            local n = 0
            for i = 1, #bs do for j = i + 1, #bs do if x.I.aabb_aabb_overlap(pos[i], hs[i], pos[j], hs[j]) then n = n + 1 end end end
            return n
          end
        end,
      } },
  },
}
