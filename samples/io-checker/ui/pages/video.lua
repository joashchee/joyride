-- The list of full-screen patterns, and what the screen says about itself. A pattern is drawn in real pixels (see pattern.lua).
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Screen", foot = "A show  < > next  B back" }

function P.draw(app, scr, s)
  local model = app.core.model
  local items = {}
  for i, p in ipairs(model.PATTERNS) do items[i] = { label = p.label, value = s.seen[p.id] and "[x]" or "[ ]" } end
  s.sel.video = s.sel.video or 1
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = #items + 2, title = "Patterns", items = items, sel = s.sel.video, foot = "" })
  local top = #items + 3
  scr:box(0, top, screen.COLS, 22 - top, "This screen", ST.frame)
  local rows = { { label = "Frame rate", value = model.framesLine(s.frames:stats()) } }
  for _, r in ipairs((s.videoInfo or {}).rows or {}) do rows[#rows + 1] = r end
  local y = top + 1
  for _, r in ipairs(rows) do
    if y >= 21 then break end
    scr:text(2, y, screen.fit(("%-11s %s"):format(screen.fit(r.label, 11), r.value), screen.COLS - 4), ST.body)
    y = y + 1
  end
end

function P.act(app, s, a)
  local n = #app.core.model.PATTERNS
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.video = screen.nav(s.sel.video or 1, n, n, a) return true end
  if a == "accept" then app.go("pattern") return true end
  return false
end

function P.tap(app, s, col, row)
  local i = row - 1
  if i >= 1 and i <= #app.core.model.PATTERNS then s.sel.video = i; app.go("pattern") return true end
  return false
end

return P
