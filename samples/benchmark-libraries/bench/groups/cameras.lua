-- Cameras: turn screen positions into world positions and back, with zoom and rotation. Both libraries ask LÖVE for the window size
-- when they are made; with no window (a headless run) the runner gives them a pretend 800 x 600 one and says so in each cell's note.

local U = require("bench.util")
local close = U.close

-- The screen's size as the libraries see it (a real window, or the runner's pretend 800 x 600), and its centre.
local function centre() local w, h = love.graphics.getWidth(), love.graphics.getHeight(); return w / 2, h / 2 end

return {
  id = "cameras", title = "Cameras",
  about = "Screen to world and back with zoom and rotation; keep the view inside the world; follow a target.",
  stubWindow = { 800, 600 },
  members = {
    { id = "gamera", short = "gamera", lib = "gamera" },
    { id = "hump.camera", short = "camera", lib = "hump", get = function(_, req) return req("hump.camera") end },
  },
  rows = {
    { id = "centre", label = "Screen centre", kind = "feature", about = "A camera looking at (1000,1000): the middle of the screen is world (1000,1000).",
      impl = {
        gamera = function(g) local c = g.new(0, 0, 2000, 2000); local cx, cy = centre(); c:setPosition(1000, 1000); local x, y = c:toWorld(cx, cy); return close(x, 1000) and close(y, 1000) end,
        ["hump.camera"] = function(cam) local cx, cy = centre(); local c = cam(1000, 1000); local x, y = c:worldCoords(cx, cy); return close(x, 1000) and close(y, 1000) end,
      } },
    { id = "zoom", label = "Zoom", kind = "feature", about = "At zoom 2, 100 screen pixels right of the centre is 50 world units right.",
      impl = {
        gamera = function(g) local c = g.new(0, 0, 2000, 2000); c:setPosition(1000, 1000); c:setScale(2); local cx, cy = centre(); local x, y = c:toWorld(cx + 100, cy); return close(x, 1050) and close(y, 1000) end,
        ["hump.camera"] = function(cam) local c = cam(1000, 1000); c:zoomTo(2); local cx, cy = centre(); local x, y = c:worldCoords(cx + 100, cy); return close(x, 1050) and close(y, 1000) end,
      } },
    { id = "rotate", label = "Rotate", kind = "feature", about = "A quarter turn swaps the axes: 100 pixels right of centre becomes 100 world units up or down.",
      impl = {
        gamera = function(g) local c = g.new(0, 0, 2000, 2000); c:setPosition(1000, 1000); c:setAngle(math.pi / 2); local cx, cy = centre(); local x, y = c:toWorld(cx + 100, cy); return close(x, 1000, 1e-6) and close(math.abs(y - 1000), 100, 1e-6) end,
        ["hump.camera"] = function(cam) local c = cam(1000, 1000); c:rotateTo(math.pi / 2); local cx, cy = centre(); local x, y = c:worldCoords(cx + 100, cy); return close(x, 1000, 1e-6) and close(math.abs(y - 1000), 100, 1e-6) end,
      } },
    { id = "roundtrip", label = "There and back", kind = "feature", about = "At zoom 1.5 and a turn of 0.3, a world point to the screen and back is the same point.",
      impl = {
        gamera = function(g)
          local c = g.new(0, 0, 2000, 2000); c:setPosition(900, 700); c:setScale(1.5); c:setAngle(0.3)
          local sx, sy = c:toScreen(950, 720); local wx, wy = c:toWorld(sx, sy); return close(wx, 950, 1e-6) and close(wy, 720, 1e-6)
        end,
        ["hump.camera"] = function(cam)
          local c = cam(900, 700); c:zoomTo(1.5); c:rotateTo(0.3)
          local sx, sy = c:cameraCoords(950, 720); local wx, wy = c:worldCoords(sx, sy); return close(wx, 950, 1e-6) and close(wy, 720, 1e-6)
        end,
      } },
    { id = "bounds", label = "World bounds", kind = "feature", about = "Asked to look far outside a 2000 x 2000 world, the camera keeps its view inside the world.",
      impl = {
        gamera = function(g)
          local c = g.new(0, 0, 2000, 2000); c:setPosition(-500, -500); local x, y = c:toWorld(0, 0)
          return x >= -1e-9 and y >= -1e-9
        end,
        ["hump.camera"] = false,
      } },
    { id = "follow", label = "Follow a target", kind = "feature", about = "Chase a moving target with a smoothing function: the camera ends up on it.",
      impl = {
        gamera = false,
        ["hump.camera"] = function(cam)
          local c = cam(0, 0)
          for _ = 1, 200 do c:lockPosition(100, 50, function(dx, dy) return dx * 0.3, dy * 0.3 end) end
          local x, y = c:position(); return close(x, 100, 0.01) and close(y, 50, 0.01)
        end,
      } },
    { id = "speed", label = "10000 conversions", kind = "speed", unit = "points/s", units = 10000, about = "Convert 10000 screen points to world points at zoom 1.5 with a turn.",
      expect = function(v) return v == v, v end,
      impl = {
        gamera = function(g) local c = g.new(0, 0, 2000, 2000); c:setPosition(900, 700); c:setScale(1.5); c:setAngle(0.3)
          return function() local sx = 0; for i = 1, 10000 do local x = c:toWorld(i % 800, i % 600); sx = sx + x end; return sx end end,
        ["hump.camera"] = function(cam) local c = cam(900, 700); c:zoomTo(1.5); c:rotateTo(0.3)
          return function() local sx = 0; for i = 1, 10000 do local x = c:worldCoords(i % 800, i % 600); sx = sx + x end; return sx end end,
      } },
  },
}
