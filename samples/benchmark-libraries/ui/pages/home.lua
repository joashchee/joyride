-- The list: run the benchmark, then open a comparison, the libraries, the stand-alone libraries, the platform or the report.
local screen = require("ui.screen")
local tables = require("lib.tables")
local ST = screen.ST

local P = { title = "Compare", foot = "Enter open  Esc twice to quit" }

local function entries(app, s)
  local core = app.core
  local out = {}
  local suite = core.suite
  out[#out + 1] = { id = "run", label = s.ran and "Run the whole benchmark again" or "Run the whole benchmark", value = s.ran and ("done, %.1f s"):format(s.ranSeconds or 0) or "not run", style = s.ran and ST.body or ST.warn }
  for _, g in ipairs(suite.groups) do
    local value, style = "", ST.dim
    local have = s.results.groups[g.id]
    if have then
      local best, top = nil, 0
      for _, sc in ipairs(suite.scores(g, s.results, s.ratings and s.ratings:lookup())) do
        if sc.best then best = best and (best .. "+" .. sc.id) or sc.id end
        top = math.max(top, sc.score or 0)
      end
      if best then value, style = screen.fit(best, 20) .. " " .. string.format("%.1f", top), ST.ok
      else
        local rated = false
        for _, sc in ipairs(suite.scores(g, s.results, s.ratings and s.ratings:lookup())) do if sc.score then rated = true end end
        value, style = rated and "no clear best" or "needs a window", rated and ST.body or ST.dim
      end
    else value = "not run" end
    out[#out + 1] = { id = "group:" .. g.id, label = g.title .. (g.needs == "window" and " (window)" or ""), value = value, style = style }
  end
  local loaded, total = 0, #core.libload.entries
  for _, rec in pairs(core.libload.loaded) do if rec.status == "PASS" then loaded = loaded + 1 end end
  out[#out + 1] = { id = "libraries", label = "Every library", value = s.ran and ("%d of %d loaded"):format(loaded, total) or ("%d libraries"):format(total) }
  local ok, n = 0, 0
  for _, c in pairs(s.results.specialists) do n = n + 1; if c.status == "PASS" then ok = ok + 1 end end
  out[#out + 1] = { id = "specialists", label = "Libraries that stand alone", value = n > 0 and ("%d of %d work here"):format(ok, n) or "not run" }
  out[#out + 1] = { id = "platform", label = "Platform" }
  out[#out + 1] = { id = "report", label = "Report and tables" }
  out[#out + 1] = { id = "quit", label = "Quit" }
  return out
end

function P.draw(app, scr, s)
  local list = entries(app, s)
  s.sel.home = s.sel.home or 1
  s.sel.home = math.min(s.sel.home, #list)
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = 19, title = "Libraries that do the same job, side by side", items = list, sel = s.sel.home, foot = "" })
  scr:text(1, 20, s.env.window and "A window is open: every comparison can run." or "No window: comparisons that draw are not tried.", ST.dim)
  scr:text(1, 21, ("Rating out of 5; * marks the best; your own ratings count."):sub(1, screen.COLS - 2), ST.dim)
end

local function open(app, s)
  local list = entries(app, s)
  local e = list[s.sel.home]
  if not e then return end
  if e.id == "quit" then love.event.quit() return end
  if e.id == "run" then
    app.core.start(s)
    s.afterRun = "home"
    app.go("run")
    return
  end
  local gid = e.id:match("^group:(.+)$")
  if gid then
    s.group = gid
    if s.results.groups[gid] then app.go("group")
    else
      app.core.start(s, gid)
      s.afterRun = "group"
      app.go("run")
    end
    return
  end
  app.go(e.id)
end

function P.act(app, s, a)
  local list = entries(app, s)
  s.sel.home = s.sel.home or 1
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.home = screen.nav(s.sel.home, #list, 17, a) return true end
  if a == "accept" then open(app, s) return true end
  if a == "back" then
    if (s.quitT or 0) > 0 then love.event.quit() return true end
    s.quitT = 3
    app.say("Press Back again to quit.", "warn")
    return true
  end
  return false
end

function P.update(app, s, dt)
  if (s.quitT or 0) > 0 then
    s.quitT = s.quitT - dt
    if s.quitT <= 0 then app.say("Pick a comparison.") end
  end
end

function P.tap(app, s, col, row)
  local list = entries(app, s)
  local per = 17
  local page = math.floor((s.sel.home - 1) / per)
  local i = page * per + (row - 1 - 0)
  if col < screen.COLS and row >= 2 and row <= 18 and list[i] then s.sel.home = i; open(app, s) return true end
  return false
end

return P
