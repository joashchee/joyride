-- Shows the benchmark running: a few steps every frame, so the screen keeps drawing and nothing looks frozen.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Running", foot = "Back stops after this step" }

function P.enter(app, s) app.say("Running. Each library is tried in turn.", "warn") end

function P.draw(app, scr, s)
  local r = s.results
  local total = math.max(1, r.total)
  local pct = r.done / total
  scr:box(0, 1, screen.COLS, 8, "Progress", ST.frame, true)
  local w = screen.COLS - 6
  local lit = math.floor(w * pct)
  scr:text(3, 3, string.rep("█", lit) .. string.rep("░", w - lit), ST.ok)
  scr:text(3, 5, ("%d of %d steps  (%d%%)"):format(r.done, r.total, math.floor(pct * 100)), ST.strong)
  scr:text(3, 7, s.running and (s.running.only and ("Just: " .. s.running.only) or "Every comparison and every library") or "Done", ST.body)
  scr:text(1, 10, "Each cell times a small, fixed piece of work with the", ST.dim)
  scr:text(1, 11, "same data for every library. It runs on this machine,", ST.dim)
  scr:text(1, 12, "so a laptop, a phone and a handheld give different numbers.", ST.dim)
end

function P.update(app, s)
  if s.running then
    if not app.core.pump(s, 0.008) then
      app.say(("Done in %.1f s."):format(s.ranSeconds or 0), "ok")
      app.go(s.afterRun or "home")
    end
  end
end

function P.act(app, s, a)
  if a == "back" then s.running = nil; s.ran = s.results.done > 0; app.say("Stopped.", "warn"); app.go("home") return true end
  return true
end

return P
