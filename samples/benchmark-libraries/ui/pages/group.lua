-- One comparison as a table: a row for each feature (then the cost of having the library), a column for each library, a rating in each
-- cell. The best in a row is marked. Rate any cell yourself with X (up) and Y (down); the group's overall score uses your ratings.
local screen = require("ui.screen")
local tables = require("lib.tables")
local rating = require("bench.rating")
local runner = require("bench.runner")
local ST = screen.ST

local P = { title = "Comparison", parent = "home" }
P.foot = function(app, s) return "< > library  ^ v row  X/Y rate  0 clear  A re-run" end
P.soft = { { "Rate+", function(a) a.act("x") end }, { "Rate-", function(a) a.act("y") end }, { "Clear", function(a) a.act("clear") end }, { "Rerun", function(a) a.act("accept") end } }

local LABEL_W, CELL_W, PER = 18, 7, 9
local VIS = 4   -- members shown at a time: the table is 48 columns wide; a comparison with more scrolls sideways

--- The first member shown, so that the selected column is always on screen.
local function firstShown(at, n)
  local first = at.first or 1
  if at.col < first then first = at.col elseif at.col > first + VIS - 1 then first = at.col - VIS + 1 end
  first = math.max(1, math.min(first, math.max(1, n - VIS + 1)))
  at.first = first
  return first
end

local function group(app, s) return app.core.suite.byId[s.group or "classes"] end

local function cur(s, g)
  s.col[g.id] = s.col[g.id] or { row = 1, col = 1 }
  return s.col[g.id]
end

function P.enter(app, s)
  local g = group(app, s)
  P.title = g.title
  app.say(g.about, "hint")
end

local function cellStyle(info)
  local c = info.cell
  if not c then return ST.dim end
  if info.best then return ST.keydown end
  if c.status == "SKIP" or c.status == "NA" then return ST.dim end
  if c.status == "FAIL" then return ST.error end
  if info.manual then return ST.warn end
  return ST.body
end

function P.draw(app, scr, s)
  local g = group(app, s)
  local suite = app.core.suite
  local manual = s.ratings and s.ratings:lookup()
  local rows = runner.rows(g)
  local info = tables.cells(g, s.results, manual)
  local scores = suite.scores(g, s.results, manual)
  local at = cur(s, g)
  at.row = math.max(1, math.min(at.row, #rows)); at.col = math.max(1, math.min(at.col, #g.members))

  local first = firstShown(at, #g.members)
  local last = math.min(#g.members, first + VIS - 1)
  local pages = math.max(1, math.ceil(#rows / PER))
  local page = math.floor((at.row - 1) / PER)
  scr:box(0, 1, screen.COLS, 14, g.title, ST.frame, true)
  -- header
  local x0 = 1 + LABEL_W
  scr:text(2, 2, screen.pad("Feature", LABEL_W - 1), ST.hint)
  for i = first, last do scr:text(x0 + (i - first) * CELL_W, 2, screen.pad(" " .. g.members[i].short, CELL_W), ST.hint) end
  for k = 1, PER do
    local idx = page * PER + k
    local row = rows[idx]
    if not row then break end
    local y = 2 + k
    local sel = idx == at.row
    scr:text(2, y, screen.pad(screen.fit(row.label, LABEL_W - 1), LABEL_W - 1), sel and ST.sel or (row.kind == "cost" and ST.dim or ST.body))
    for i = first, last do
      local m = g.members[i]
      local inf = info[row.id][m.id]
      local text = tables.short(inf)
      local pad = math.floor((CELL_W - #text) / 2)
      local st = (sel and at.col == i) and ST.sel or cellStyle(inf)
      scr:text(x0 + (i - first) * CELL_W, y, string.rep(" ", pad) .. text .. string.rep(" ", CELL_W - pad - #text), st)
    end
  end
  -- overall
  scr:text(2, 13, screen.pad("Overall (of 5)", LABEL_W - 1), ST.strong)
  for i = first, last do
    local sc = scores[i]
    local text = sc.score and string.format("%.1f", sc.score) or "~"
    if sc.best then text = text .. "*" end
    local pad = math.floor((CELL_W - #text) / 2)
    scr:text(x0 + (i - first) * CELL_W, 13, string.rep(" ", pad) .. text .. string.rep(" ", CELL_W - pad - #text), sc.best and ST.keydown or ST.strong)
  end
  if #g.members > VIS then scr:text(2, 14, (" Libraries %d-%d of %d "):format(first, last, #g.members), ST.dim) end
  if pages > 1 then scr:text(screen.COLS - 14, 14, (" Page %d/%d "):format(page + 1, pages), ST.dim) end

  -- detail
  local row, m = rows[at.row], g.members[at.col]
  local inf = info[row.id][m.id]
  scr:box(0, 15, screen.COLS, 7, nil, ST.frame, false)
  scr:text(2, 15, " " .. screen.fit(row.label, 30) .. " ", ST.strong)
  local about = screen.wrap(row.about or "", screen.COLS - 4)
  for i = 1, math.min(2, #about) do scr:text(2, 15 + i, about[i], ST.body) end
  local res = screen.wrap(m.id .. ": " .. tables.detail(row, m, inf), screen.COLS - 4)
  for i = 1, math.min(2, #res) do scr:text(2, 17 + i, res[i], (inf.cell and inf.cell.status == "FAIL") and ST.error or ST.hint) end
  local c = inf.cell
  local line
  if not c then line = "Not run yet: press A."
  elseif c.status == "SKIP" then line = "Not tried here, so it has no rating."
  else
    line = ("Rating %s%s%s"):format(inf.eff or "-", inf.manual and (" (yours; automatic %s)"):format(tostring(inf.auto or "-")) or " (automatic)", inf.best and ", best in this row" or "")
  end
  scr:text(2, 20, screen.fit(line, screen.COLS - 4), inf.manual and ST.warn or ST.dim)
end

local function save(app, s)
  local ok, why = s.ratings:save()
  if not ok then app.say("Your rating could not be saved: " .. tostring(why), "error") return false end
  return true
end

function P.act(app, s, a)
  local g = group(app, s)
  local rows = runner.rows(g)
  local at = cur(s, g)
  if a == "up" then at.row = (at.row - 2) % #rows + 1 return true end
  if a == "down" then at.row = at.row % #rows + 1 return true end
  if a == "left" then at.col = (at.col - 2) % #g.members + 1 return true end
  if a == "right" then at.col = at.col % #g.members + 1 return true end
  if a == "pgup" then at.row = math.max(1, at.row - PER) return true end
  if a == "pgdn" then at.row = math.min(#rows, at.row + PER) return true end
  local row, m = rows[at.row], g.members[at.col]
  local cell = s.results.groups[g.id] and s.results.groups[g.id][row.id] and s.results.groups[g.id][row.id][m.id]
  if a == "x" or a == "y" then
    if not cell then app.say("Run it first (A).", "warn") return true end
    local now = s.ratings:get(g.id, row.id, m.id)
    local v = s.ratings:step(g.id, row.id, m.id, a == "x" and 1 or -1, now or cell.auto or 3)
    if save(app, s) then app.say(("Your rating for %s / %s: %d."):format(row.label, m.id, v), "ok") end
    return true
  end
  if a == "clear" then
    if s.ratings:get(g.id, row.id, m.id) then s.ratings:set(g.id, row.id, m.id, nil); if save(app, s) then app.say("Your rating removed: the automatic one counts.", "ok") end
    else app.say("You had not rated this one.", "hint") end
    return true
  end
  if a == "accept" then
    app.core.start(s, g.id)
    s.afterRun = "group"
    app.go("run")
    return true
  end
  return false
end

function P.tap(app, s, col, row)
  local g = group(app, s)
  local at = cur(s, g)
  local rows = runner.rows(g)
  local page = math.floor((at.row - 1) / PER)
  if row >= 3 and row <= 11 then
    local idx = page * PER + (row - 2)
    if rows[idx] then
      at.row = idx
      local c = (at.first or 1) + math.floor((col - (1 + LABEL_W)) / CELL_W)
      if c >= (at.first or 1) and c <= #g.members and c < (at.first or 1) + VIS then at.col = c end
      return true
    end
  end
  return false
end

return P
