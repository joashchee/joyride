-- Finding a way across a grid. jumper knows grids and has several algorithms (A*, Dijkstra, jump point search); batteries.pathfind is one
-- general A* over any graph: you supply the neighbours; lua-star and Luafinding are small A* modules that ask you whether a cell is open
-- (lua-star can be told to move in four directions only, Luafinding always moves in eight).

local H = require("bench.harness")

-- A map is rows of text: # is a wall. Returns the rows as numbers (0 walkable, 1 wall) and its width and height.
local function parse(rows)
  local m = {}
  for y, row in ipairs(rows) do m[y] = {}; for x = 1, #row do m[y][x] = row:sub(x, x) == "#" and 1 or 0 end end
  return m, #rows[1], #rows
end

-- The shortest 4-way step count by plain breadth-first search: the answer every library must agree with.
local function bfs(m, w, h, sx, sy, gx, gy)
  local dist, q, qi = { [sy * 1000 + sx] = 0 }, { { sx, sy } }, 1
  while qi <= #q do
    local x, y = q[qi][1], q[qi][2]; qi = qi + 1
    if x == gx and y == gy then return dist[y * 1000 + x] end
    for _, d in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
      local nx, ny = x + d[1], y + d[2]
      if m[ny] and m[ny][nx] == 0 and not dist[ny * 1000 + nx] then dist[ny * 1000 + nx] = dist[y * 1000 + x] + 1; q[#q + 1] = { nx, ny } end
    end
  end
  return nil
end

local ROOM = { "........", ".######.", "........", ".######.", "........" }          -- a zigzag: 5 rows
local WALLED = { "...#....", "...#....", "...#....", "...#...." }                      -- no way across
local function crowdMap()
  local rnd, rows = H.rng(11), {}
  for y = 1, 40 do
    local r = {}
    for x = 1, 40 do r[x] = (rnd() < 0.2 and not (x == 1 and y == 1) and not (x == 40 and y == 40)) and "#" or "." end
    rows[y] = table.concat(r)
  end
  return rows
end

local function openFn(rows) return function(x, y) return rows[y] ~= nil and rows[y]:sub(x, x) == "." end end

-- lua-star: positions are tables with x and y, the result is the list of cells including the start (so steps = #path - 1), or false.
local function luastarPath(ls, rows, sx, sy, gx, gy, noDiagonal)
  local p = ls:find(#rows[1], #rows, { x = sx, y = sy }, { x = gx, y = gy }, openFn(rows), false, noDiagonal)
  return p and (#p - 1) or nil
end

-- Luafinding: positions are vectors (its own), the result is the list of cells including the start, or nil. It always moves in eight directions.
local function luafindingPath(mods, rows, sx, sy, gx, gy)
  local open = openFn(rows)
  local f = mods.LF(mods.Vector(sx, sy), mods.Vector(gx, gy), function(v) return open(v.x, v.y) end)
  local p = f:GetPath()
  return p and (#p - 1) or nil
end

local function jumperPath(mods, rows, mode, algo, sx, sy, gx, gy)
  local m = parse(rows)
  local finder = mods.Pathfinder(mods.Grid(m), algo or "ASTAR", 0)
  finder:setMode(mode or "ORTHOGONAL")
  local p = finder:getPath(sx, sy, gx, gy)
  if not p then return nil end
  local n = 0
  for _ in p:nodes() do n = n + 1 end
  return n - 1, p:getLength()
end

local function batteriesPath(pathfind, rows, sx, sy, gx, gy, weight)
  local m, w, h = parse(rows)
  local function id(x, y) return (y - 1) * w + x end
  local function xy(n) return (n - 1) % w + 1, math.floor((n - 1) / w) + 1 end
  local path = pathfind({
    start = id(sx, sy),
    is_goal = function(n) return n == id(gx, gy) end,
    neighbours = function(n)
      local x, y = xy(n); local out = {}
      for _, d in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
        local nx, ny = x + d[1], y + d[2]
        if m[ny] and m[ny][nx] == 0 then out[#out + 1] = id(nx, ny) end
      end
      return out
    end,
    distance = weight and function(a, b) local x, y = xy(b); return weight(x, y) end or nil,
    heuristic = function(n) local x, y = xy(n); return math.abs(gx - x) + math.abs(gy - y) end,
  })
  return path and (#path - 1) or nil, path
end

return {
  id = "pathfinding", title = "Pathfinding",
  about = "The shortest way across a grid with walls, no way at all, and a 40 by 40 maze.",
  members = {
    { id = "jumper", short = "jumper", lib = "jumper", get = function(P, req) return { Pathfinder = P, Grid = req("jumper.grid") } end },
    { id = "batteries.pathfind", short = "pathf", lib = "batteries", get = function(b) return b.pathfind end },
    { id = "lua-star", short = "lstar", lib = "Lua-star" },
    { id = "luafinding", short = "lfind", lib = "Luafinding", get = function(LF, req) return { LF = LF, Vector = req("vector") } end },
  },
  rows = {
    { id = "shortest", label = "Shortest path", kind = "feature", about = "Across the zigzag room from the top left to the bottom right: as many steps as a breadth-first search finds.",
      impl = {
        jumper = function(mods)
          local m, w, h = parse(ROOM)
          local want = bfs(m, w, h, 1, 1, w, h)
          local steps = jumperPath(mods, ROOM, "ORTHOGONAL", "ASTAR", 1, 1, w, h)
          return steps == want, ("%s steps, expected %s"):format(tostring(steps), tostring(want))
        end,
        ["batteries.pathfind"] = function(pf)
          local m, w, h = parse(ROOM)
          local want = bfs(m, w, h, 1, 1, w, h)
          local steps = batteriesPath(pf, ROOM, 1, 1, w, h)
          return steps == want, ("%s steps, expected %s"):format(tostring(steps), tostring(want))
        end,
        ["lua-star"] = function(ls)
          local m, w, h = parse(ROOM)
          local want = bfs(m, w, h, 1, 1, w, h)
          local steps = luastarPath(ls, ROOM, 1, 1, w, h, true)
          return steps == want, ("%s steps, expected %s (told to move in four directions)"):format(tostring(steps), tostring(want))
        end,
        luafinding = function(mods)
          local m, w, h = parse(ROOM)
          local want = bfs(m, w, h, 1, 1, w, h)
          local steps = luafindingPath(mods, ROOM, 1, 1, w, h)
          return steps == want, ("%s steps, expected %s: it always moves in eight directions, there is no four-way mode"):format(tostring(steps), tostring(want))
        end,
      } },
    { id = "none", label = "No way across", kind = "feature", about = "A wall from top to bottom: no path, and no error.",
      impl = {
        jumper = function(mods) return jumperPath(mods, WALLED, "ORTHOGONAL", "ASTAR", 1, 1, 8, 4) == nil end,
        ["batteries.pathfind"] = function(pf)
          local steps = batteriesPath(pf, WALLED, 1, 1, 8, 4)
          return steps == nil or steps == false
        end,
        ["lua-star"] = function(ls) return luastarPath(ls, WALLED, 1, 1, 8, 4, true) == nil end,
        luafinding = function(mods) return luafindingPath(mods, WALLED, 1, 1, 8, 4) == nil end,
      } },
    { id = "diagonal", label = "Diagonal moves", kind = "feature", about = "Eight-way movement across an open 5 by 5 room: 4 diagonal steps, not 8 square ones.",
      impl = {
        jumper = function(mods)
          local rows = { ".....", ".....", ".....", ".....", "....." }
          local steps = jumperPath(mods, rows, "DIAGONAL", "ASTAR", 1, 1, 5, 5)
          return steps == 4
        end,
        ["batteries.pathfind"] = function(pf)
          local W = 5
          local function id(x, y) return (y - 1) * W + x end
          local function xy(n) return (n - 1) % W + 1, math.floor((n - 1) / W) + 1 end
          local path = pf({ start = id(1, 1), is_goal = function(n) return n == id(5, 5) end,
            neighbours = function(n)
              local x, y = xy(n); local out = {}
              for dx = -1, 1 do for dy = -1, 1 do
                local nx, ny = x + dx, y + dy
                if (dx ~= 0 or dy ~= 0) and nx >= 1 and nx <= W and ny >= 1 and ny <= W then out[#out + 1] = id(nx, ny) end
              end end
              return out
            end,
            distance = function(a, b) local ax, ay = xy(a); local bx, by = xy(b); return math.sqrt((ax - bx) ^ 2 + (ay - by) ^ 2) end,
            heuristic = function(n) local x, y = xy(n); return math.sqrt((5 - x) ^ 2 + (5 - y) ^ 2) end })
          return path and #path - 1 == 4, "you write the neighbour list yourself"
        end,
        ["lua-star"] = function(ls)
          local rows = { ".....", ".....", ".....", ".....", "....." }
          return luastarPath(ls, rows, 1, 1, 5, 5, false) == 4
        end,
        luafinding = function(mods)
          local rows = { ".....", ".....", ".....", ".....", "....." }
          return luafindingPath(mods, rows, 1, 1, 5, 5) == 4
        end,
      } },
    { id = "cost", label = "Terrain costs", kind = "feature", about = "Mud (cost 5) in the short way round makes the long way (cost 1) cheaper: the path avoids the mud.",
      impl = {
        jumper = false,
        ["batteries.pathfind"] = function(pf)
          -- a 3-wide room: the middle row is mud, the top row is clear (longer way round)
          local rows = { "...", "...", "..." }
          local steps = batteriesPath(pf, rows, 1, 2, 3, 2, function(x, y) return (x == 2 and y == 2) and 5 or 1 end)
          return steps == 4, ("%s steps (a straight line is 2 steps through the mud)"):format(tostring(steps))
        end,
        ["lua-star"] = false,
        luafinding = false,
      } },
    { id = "jps", label = "Jump point search", kind = "feature", about = "A faster algorithm for open grids, picked by name.",
      impl = {
        jumper = function(mods)
          local rows = { "........", "........", "........", "........" }
          local _, len = jumperPath(mods, rows, "DIAGONAL", "JPS", 1, 1, 8, 4)   -- JPS lists only the turning points, so use the length
          return len ~= nil and len >= 7 and len < 9
        end,
        ["batteries.pathfind"] = false,
        ["lua-star"] = false,
        luafinding = false,
      } },
    { id = "maze", label = "40 by 40 maze", kind = "speed", unit = "paths/s", units = 1, about = "Corner to corner across a seeded 40 by 40 grid with one wall in five.",
      expect = function(n) return n ~= nil and n > 0, n end,
      impl = {
        jumper = function(mods) local rows = crowdMap(); return function() return (jumperPath(mods, rows, "ORTHOGONAL", "ASTAR", 1, 1, 40, 40)) end end,
        ["batteries.pathfind"] = function(pf) local rows = crowdMap(); return function() return (batteriesPath(pf, rows, 1, 1, 40, 40)) end end,
        ["lua-star"] = function(ls) local rows = crowdMap(); return function() return luastarPath(ls, rows, 1, 1, 40, 40, true) end end,
        luafinding = function(mods) local rows = crowdMap(); return function() return luafindingPath(mods, rows, 1, 1, 40, 40) end end,
      } },
  },
}
