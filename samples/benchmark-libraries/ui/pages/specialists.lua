-- The libraries with no counterpart: each does a job nothing else here does, so there is nothing to compare, only one check of that job.
local screen = require("ui.screen")
local specialists = require("bench.specialists")
local ST = screen.ST

local P = { title = "Stand-alone libraries", parent = "home", foot = "^ v choose  PgUp/PgDn page" }

local function items(app, s)
  local out = {}
  for _, e in ipairs(specialists.list) do
    local c = s.results.specialists[e.lib]
    local value, style = "not run", ST.dim
    if c then
      if c.status == "PASS" then value, style = "works", ST.ok
      elseif c.status == "SKIP" then value, style = "not tried", ST.dim
      else value, style = "finding", ST.warn end
    end
    out[#out + 1] = { label = e.lib, value = value, style = style, vstyle = style, e = e, c = c }
  end
  return out
end

function P.draw(app, scr, s)
  local list = items(app, s)
  s.sel.specialists = math.max(1, math.min(s.sel.specialists or 1, #list))
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = 14, title = "One job each", items = list, sel = s.sel.specialists, foot = "" })
  local it = list[s.sel.specialists]
  scr:box(0, 15, screen.COLS, 7, nil, ST.frame, false)
  scr:text(2, 15, " " .. it.e.lib .. " ", ST.strong)
  scr:text(2, 16, screen.fit(it.e.job .. (it.e.needs == "window" and " (needs a window)" or ""), screen.COLS - 4), ST.hint)
  local lines = screen.wrap(it.c and (it.c.note or "") or "Run the benchmark to try it.", screen.COLS - 4)
  for i = 1, math.min(4, #lines) do scr:text(2, 17 + i, lines[i], it.c and it.c.status == "FAIL" and ST.warn or ST.body) end
end

function P.act(app, s, a)
  local list = items(app, s)
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.specialists = screen.nav(s.sel.specialists or 1, #list, 12, a) return true end
  return false
end

function P.tap(app, s, col, row)
  local list = items(app, s)
  local per = 12
  local page = math.floor(((s.sel.specialists or 1) - 1) / per)
  local i = page * per + (row - 1)
  if row >= 2 and row <= 13 and list[i] then s.sel.specialists = i return true end
  return false
end

return P
