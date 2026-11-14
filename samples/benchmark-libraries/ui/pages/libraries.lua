-- Every library JoyrIDE ships: whether it loaded on this platform, what that cost, what it is, and which comparison it is in.
local screen = require("ui.screen")
local model = require("lib.model")
local ST = screen.ST

local P = { title = "Every library", parent = "home", foot = "^ v choose  PgUp/PgDn page" }

local function items(app, s)
  local core = app.core
  local cover = {}
  for _, c in ipairs(core.suite.coverage()) do cover[c.name] = c.groups end
  local out = {}
  for _, e in ipairs(core.libload.entries) do
    local rec = core.libload.loaded[e.name]
    local value, style = "not run", ST.dim
    if rec then
      if rec.status == "PASS" then value, style = ("%.2f ms"):format(rec.ms), ST.ok
      elseif rec.status == "SKIP" then value, style = "not tried", ST.dim
      else value, style = "FAILED", ST.error end
    end
    out[#out + 1] = { label = e.name .. (e.kind ~= "lua" and (" [" .. e.kind .. "]") or ""), value = value, style = style, vstyle = style, e = e, rec = rec, groups = cover[e.name] }
  end
  return out
end

function P.draw(app, scr, s)
  local list = items(app, s)
  s.sel.libraries = math.max(1, math.min(s.sel.libraries or 1, #list))
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = 14, title = ("%d libraries"):format(#list), items = list, sel = s.sel.libraries, foot = "" })
  local it = list[s.sel.libraries]
  local e, rec = it.e, it.rec
  scr:box(0, 15, screen.COLS, 7, nil, ST.frame, false)
  scr:text(2, 15, " " .. e.name .. (e.tag and (" " .. e.tag) or "") .. " ", ST.strong)
  local facts = ("%s, %s, %d lines in %d files, %s"):format(e.kind, e.licence, e.loc, e.files, e.rev:sub(1, 7))
  scr:text(2, 16, screen.fit(facts, screen.COLS - 4), ST.hint)
  local lines = screen.wrap(e.summary, screen.COLS - 4)
  for i = 1, math.min(2, #lines) do scr:text(2, 16 + i, lines[i], ST.body) end
  local g = #it.groups > 0 and ("In: " .. table.concat(it.groups, ", ")) or "Stands alone: its own check is under \"Libraries that stand alone\"."
  scr:text(2, 19, screen.fit(g, screen.COLS - 4), ST.dim)
  local r = rec and (rec.status == "PASS" and ("Loaded in %.2f ms, keeps %.1f KB."):format(rec.ms, rec.kb) or rec.detail) or "Run the benchmark to load it."
  scr:text(2, 20, screen.fit(r, screen.COLS - 4), rec and rec.status == "FAIL" and ST.error or ST.dim)
end

function P.act(app, s, a)
  local list = items(app, s)
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.libraries = screen.nav(s.sel.libraries or 1, #list, 12, a) return true end
  return false
end

function P.tap(app, s, col, row)
  local list = items(app, s)
  local per = 12
  local page = math.floor(((s.sel.libraries or 1) - 1) / per)
  local i = page * per + (row - 1)
  if row >= 2 and row <= 13 and list[i] then s.sel.libraries = i return true end
  return false
end

return P
