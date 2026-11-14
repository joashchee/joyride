-- Tweening: change numbers over time. tween.lua is objects you update yourself; flux and hump.timer keep the tweens for you.

local function near(a, b) return math.abs(a - b) < 1e-9 end
local function steps(update, n, dt) for _ = 1, n do update(dt) end end   -- n frames of dt seconds

return {
  id = "tweens", title = "Tweening",
  about = "Move a number to a target over time, with easing, callbacks and stopping.",
  members = {
    { id = "tween", short = "tween", lib = "tween" },
    { id = "flux", short = "flux", lib = "flux" },
    { id = "hump.timer", short = "hump", lib = "hump", get = function(_, req) return req("hump.timer") end },
  },
  rows = {
    { id = "linear", label = "Tween a number", kind = "feature", about = "Tween x from 0 to 10 over one second, linearly: 5 after half a second, 10 at the end.",
      impl = {
        tween = function(tween)
          local s = { x = 0 }; local t = tween.new(1, s, { x = 10 }, "linear")
          t:update(0.5); local mid = s.x; local done = t:update(0.5)
          return near(mid, 5) and near(s.x, 10) and done == true
        end,
        flux = function(flux)
          local f = flux.group(); local s = { x = 0 }; f:to(s, 1, { x = 10 }):ease("linear")
          f:update(0.5); local mid = s.x; f:update(0.5)
          return near(mid, 5) and near(s.x, 10)
        end,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { x = 0 }; T:tween(1, s, { x = 10 }, "linear")
          T:update(0.5); local mid = s.x; T:update(0.5)
          return near(mid, 5) and near(s.x, 10)
        end,
      } },
    { id = "ease", label = "Named easing", kind = "feature", about = "Ask for an in-out cubic curve by name: symmetric (5 at the half), and exactly 10 at the end.",
      impl = {
        tween = function(tween)
          local s = { x = 0 }; local t = tween.new(1, s, { x = 10 }, "inOutCubic")
          t:update(0.5); local mid = s.x; t:update(0.5)
          return near(mid, 5) and near(s.x, 10)
        end,
        flux = function(flux)
          local f = flux.group(); local s = { x = 0 }; f:to(s, 1, { x = 10 }):ease("cubicinout")
          f:update(0.5); local mid = s.x; f:update(0.5)
          return near(mid, 5) and near(s.x, 10)
        end,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { x = 0 }; T:tween(1, s, { x = 10 }, "in-out-cubic")
          T:update(0.5); local mid = s.x; T:update(0.5)
          return near(mid, 5) and near(s.x, 10)
        end,
      } },
    { id = "nested", label = "Nested fields", kind = "feature", about = "Tween a field inside a sub-table (pos.x) without touching its sibling (pos.y).",
      impl = {
        tween = function(tween)
          local s = { pos = { x = 0, y = 3 } }; local t = tween.new(1, s, { pos = { x = 10 } }, "linear"); t:update(1)
          local ok = near(s.pos.x, 10) and s.pos.y == 3
          return ok, ok or "x arrives, but the sibling pos.y is gone once the tween ends (its final copy replaces the sub-table)"
        end,
        flux = false,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { pos = { x = 0, y = 3 } }; T:tween(1, s, { pos = { x = 10 } }, "linear"); T:update(1)
          return near(s.pos.x, 10) and s.pos.y == 3
        end,
      } },
    { id = "done", label = "Completion", kind = "feature", about = "Know when the tween has finished (a callback, or a result from update) and have it fire once.",
      impl = {
        tween = function(tween)
          local s = { x = 0 }; local t = tween.new(1, s, { x = 1 }); local n = 0
          for _ = 1, 4 do if t:update(0.5) then n = n + 1 end end
          return n >= 1, "update returns true when finished (and keeps returning true: count it yourself)"
        end,
        flux = function(flux)
          local f = flux.group(); local s = { x = 0 }; local n = 0
          f:to(s, 1, { x = 1 }):oncomplete(function() n = n + 1 end); for _ = 1, 4 do f:update(0.5) end
          return n == 1
        end,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { x = 0 }; local n = 0
          T:tween(1, s, { x = 1 }, "linear", function() n = n + 1 end); for _ = 1, 4 do T:update(0.5) end
          return n == 1
        end,
      } },
    { id = "chain", label = "Delay and chain", kind = "feature", about = "Wait half a second, tween a, then (when it ends) tween b: a moves only after the delay. Run in 0.05 s frames.",
      impl = {
        tween = false,
        flux = function(flux)
          local f = flux.group(); local s = { a = 0, b = 0 }
          f:to(s, 1, { a = 1 }):delay(0.5):ease("linear"):after(s, 1, { b = 1 }):ease("linear")
          steps(function(dt) f:update(dt) end, 5, 0.05); local still = s.a == 0   -- 0.25 s: still waiting
          steps(function(dt) f:update(dt) end, 15, 0.05); local moved = s.a > 0.4 and s.a < 0.9 and s.b == 0   -- 1.0 s: a is half way
          steps(function(dt) f:update(dt) end, 30, 0.05)   -- 2.5 s: a has ended, b has started
          return still and moved and near(s.a, 1) and s.b > 0
        end,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { a = 0, b = 0 }
          T:after(0.5, function()
            T:tween(1, s, { a = 1 }, "linear", function() T:tween(1, s, { b = 1 }, "linear") end)
          end)
          steps(function(dt) T:update(dt) end, 5, 0.05); local still = s.a == 0
          steps(function(dt) T:update(dt) end, 15, 0.05); local moved = s.a > 0.4 and s.a < 0.9 and s.b == 0
          steps(function(dt) T:update(dt) end, 30, 0.05)
          return still and moved and near(s.a, 1) and s.b > 0
        end,
      } },
    { id = "stop", label = "Stop a tween", kind = "feature", about = "Cancel a running tween: the value stays where it was and nothing more happens.",
      impl = {
        tween = false,
        flux = function(flux)
          local f = flux.group(); local s = { x = 0 }; local tw = f:to(s, 1, { x = 10 }):ease("linear")
          f:update(0.5); tw:stop(); f:update(0.5)
          return near(s.x, 5)
        end,
        ["hump.timer"] = function(Timer)
          local T = Timer.new(); local s = { x = 0 }; local h = T:tween(1, s, { x = 10 }, "linear")
          T:update(0.5); T:cancel(h); T:update(0.5)
          return near(s.x, 5)
        end,
      } },
    { id = "many", label = "Run 100 tweens", kind = "speed", unit = "tweens/s", units = 100, about = "Create 100 tweens and run each to the end in ten updates.",
      expect = function(v) return near(v, 100 * 5), v end,
      impl = {
        tween = function(tween) return function()
          local subjects, ts = {}, {}
          for i = 1, 100 do subjects[i] = { x = 0 }; ts[i] = tween.new(1, subjects[i], { x = 10 }, "linear") end
          for _ = 1, 10 do for i = 1, 100 do ts[i]:update(0.1) end end
          local sum = 0; for i = 1, 100 do sum = sum + subjects[i].x end; return sum / 2
        end end,
        flux = function(flux) return function()
          local f = flux.group(); local subjects = {}
          for i = 1, 100 do subjects[i] = { x = 0 }; f:to(subjects[i], 1, { x = 10 }):ease("linear") end
          for _ = 1, 10 do f:update(0.1) end
          local sum = 0; for i = 1, 100 do sum = sum + subjects[i].x end; return sum / 2
        end end,
        ["hump.timer"] = function(Timer) return function()
          local T = Timer.new(); local subjects = {}
          for i = 1, 100 do subjects[i] = { x = 0 }; T:tween(1, subjects[i], { x = 10 }, "linear") end
          for _ = 1, 10 do T:update(0.1) end
          local sum = 0; for i = 1, 100 do sum = sum + subjects[i].x end; return sum / 2
        end end,
      } },
  },
}
