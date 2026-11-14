-- What LÖVE and the device say about themselves: runtime, machine, window, renderer, audio.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "System", foot = "PgUp/PgDn page  B back" }

function P.enter(app, s) s.sel.system = 0; s.videoInfo = app.core.probe.video() end

function P.draw(app, scr, s)
  local lines = {}
  local function section(name, rows)
    lines[#lines + 1] = { name, ST.hint }
    for _, r in ipairs(rows) do lines[#lines + 1] = { ("%-12s %s"):format(screen.fit(r.label, 12), r.value), ST.body } end
  end
  section("LÖVE and the machine", app.core.probe.system({ shim = s.shim }))
  section("Window and renderer", (s.videoInfo or {}).rows or {})
  section("Audio", app.core.probe.audio().rows)
  s.systemPages = screen.lines(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = "System", lines = lines, page = s.sel.system })
end

function P.act(app, s, a)
  if a == "pgdn" or a == "down" then s.sel.system = math.min((s.systemPages or 1) - 1, s.sel.system + 1) return true end
  if a == "pgup" or a == "up" then s.sel.system = math.max(0, s.sel.system - 1) return true end
  return false
end

return P
