-- Timers: run something later, or on an interval. hump.timer keeps every timer and you call update; batteries.timer is one object per
-- timer with progress and finish callbacks.

return {
  id = "timers", title = "Timers",
  about = "Call something after a delay, every interval, or while time passes; stop it; follow progress.",
  members = {
    { id = "hump.timer", short = "hump", lib = "hump", get = function(_, req) return req("hump.timer") end },
    { id = "batteries.timer", short = "batt", lib = "batteries", get = function(m) return m.timer end },
    { id = "cron", short = "cron", lib = "cron" },
  },
  rows = {
    { id = "after", label = "After a delay", kind = "feature", about = "Call a function once, 0.5 s from now: not before, and once only (0.1 s frames).",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, n = Timer.new(), 0
          T:after(0.5, function() n = n + 1 end)
          for _ = 1, 4 do T:update(0.1) end
          local early = n
          for _ = 1, 10 do T:update(0.1) end
          return early == 0 and n == 1
        end,
        ["batteries.timer"] = function(timer)
          local n = 0
          local t = timer(0.5, nil, function() n = n + 1 end)
          for _ = 1, 4 do t:update(0.1) end
          local early = n
          for _ = 1, 10 do t:update(0.1) end
          return early == 0 and n == 1
        end,
        cron = function(cron)
          local n = 0
          local c = cron.after(0.5, function() n = n + 1 end)
          for _ = 1, 4 do c:update(0.1) end
          local early = n
          for _ = 1, 10 do c:update(0.1) end
          return early == 0 and n == 1
        end,
      } },
    { id = "every", label = "Repeat", kind = "feature", about = "Call a function every 0.5 s: five calls in 2.6 s.",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, n = Timer.new(), 0
          T:every(0.5, function() n = n + 1 end)
          for _ = 1, 26 do T:update(0.1) end
          return n == 5
        end,
        ["batteries.timer"] = function(timer)
          local n, t = 0, nil
          t = timer(0.5, nil, function() n = n + 1; t:reset() end)   -- no repeat of its own: reset it when it finishes
          for _ = 1, 26 do t:update(0.1) end
          return n == 5, "by resetting the timer in its own finish callback"
        end,
        cron = function(cron)
          local n = 0
          local c = cron.every(0.5, function() n = n + 1 end)
          for _ = 1, 26 do c:update(0.1) end
          return n == 5
        end,
      } },
    { id = "count", label = "Repeat N times", kind = "feature", about = "Every 0.2 s, but only three times.",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, n = Timer.new(), 0
          T:every(0.2, function() n = n + 1 end, 3)
          for _ = 1, 30 do T:update(0.1) end
          return n == 3
        end,
        ["batteries.timer"] = false,
        cron = false,
      } },
    { id = "during", label = "Progress", kind = "feature", about = "Be told how far along a 1 s span is on each update, ending at exactly 1.",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, last, calls = Timer.new(), 0, 0
          T:during(1, function(dt, delay) calls = calls + 1 end)
          for _ = 1, 12 do T:update(0.1) end
          return calls >= 10, "reports each frame, not a 0 to 1 fraction (it passes the frame's dt)"
        end,
        ["batteries.timer"] = function(timer)
          local last
          local t = timer(1, function(p) last = p end, function(p) last = p end)
          for _ = 1, 12 do t:update(0.1) end
          return last == 1 and t:expired()
        end,
        cron = false,   -- a clock only calls back when it is done
      } },
    { id = "cancel", label = "Cancel", kind = "feature", about = "Stop a timer that has not fired: it never does.",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, n = Timer.new(), 0
          local h = T:after(0.5, function() n = n + 1 end)
          T:update(0.2); T:cancel(h); for _ = 1, 10 do T:update(0.1) end
          return n == 0
        end,
        ["batteries.timer"] = false,
        cron = false,   -- no cancel: stop calling update on the clock
      } },
    { id = "script", label = "Wait in a script", kind = "feature", about = "Write 'do a, wait 0.5, do b' as one function that pauses.",
      impl = {
        ["hump.timer"] = function(Timer)
          local T, log = Timer.new(), {}
          T:script(function(wait) log[#log + 1] = "a"; wait(0.5); log[#log + 1] = "b" end)
          local early = #log
          for _ = 1, 8 do T:update(0.1) end
          return early == 1 and #log == 2
        end,
        ["batteries.timer"] = false,
        cron = false,
      } },
    { id = "speed", label = "1000 timers", kind = "speed", unit = "timers/s", units = 1000, about = "Create 1000 timers, run them for ten 0.1 s frames, and count how many finished.",
      expect = function(n) return n == 1000, n end,
      impl = {
        ["hump.timer"] = function(Timer) return function()
          local T, n = Timer.new(), 0
          for i = 1, 1000 do T:after(0.5 + (i % 5) * 0.1, function() n = n + 1 end) end
          for _ = 1, 10 do T:update(0.1) end
          return n
        end end,
        ["batteries.timer"] = function(timer) return function()
          local n, ts = 0, {}
          for i = 1, 1000 do ts[i] = timer(0.5 + (i % 5) * 0.1, nil, function() n = n + 1 end) end
          for _ = 1, 10 do for i = 1, 1000 do ts[i]:update(0.1) end end
          return n
        end end,
        cron = function(cron) return function()
          local n, cs = 0, {}
          for i = 1, 1000 do cs[i] = cron.after(0.5 + (i % 5) * 0.1, function() n = n + 1 end) end
          for _ = 1, 10 do for i = 1, 1000 do cs[i]:update(0.1) end end
          return n
        end end,
      } },
  },
}
