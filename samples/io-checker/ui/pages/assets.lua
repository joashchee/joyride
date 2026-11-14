-- Every asset type LÖVE loads, one list. Each is loaded the way a game would and judged against what this platform and LÖVE version
-- are expected to do; open one to see it drawn, played, shown in its font or run. Files in assets/ that nobody listed appear at the
-- end, guessed by extension: that is how a file the Asset Library adds shows up without any code change.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Assets", foot = "A open  X test all  PgUp/PgDn page" }
local TAG = { PASS = { "ok", ST.ok }, FAIL = { "FAIL", ST.error }, INFO = { "note", ST.warn }, SKIP = { "skip", ST.dim } }

--- Every row the list shows: the results in manifest order, then the files nobody listed.
local function rows(s) return s.assetResults or {} end

local function run(app, s)
  app.job("Loading every asset...", function()
    app.core.runAssets(s)
    local c = { PASS = 0, FAIL = 0, INFO = 0, SKIP = 0 }
    for _, r in ipairs(s.assetResults) do c[r.status] = c[r.status] + 1 end
    app.say(("%d loaded as expected, %d failed, %d noted, %d skipped."):format(c.PASS, c.FAIL, c.INFO, c.SKIP), c.FAIL == 0 and "ok" or "error")
  end)
end

function P.enter(app, s)
  s.sel.assets = s.sel.assets or 1
  if not s.assetResults then run(app, s) end
end

function P.draw(app, scr, s)
  local list = rows(s)
  local items = {}
  for i, r in ipairs(list) do
    local tag = TAG[r.status] or TAG.INFO
    local e = r.entry
    local name = e.path:gsub("^assets/", "")
    items[i] = { label = ("%s: %s"):format(screen.fit(e.type, 18), name), value = tag[1], vstyle = tag[2] }
  end
  if #items == 0 then items[1] = { label = "Loading..." } end
  s.sel.assets = math.max(1, math.min(s.sel.assets or 1, #items))
  screen.list(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = ("Assets (%d)"):format(#list), items = items, sel = s.sel.assets, foot = "" })
  s.assetPer = 19
end

function P.act(app, s, a)
  local list = rows(s)
  if a == "up" or a == "down" or a == "pgup" or a == "pgdn" then s.sel.assets = screen.nav(s.sel.assets or 1, #list, 19, a) return true end
  if a == "x" then run(app, s) return true end
  if a == "accept" and list[s.sel.assets] then s.assetView = list[s.sel.assets]; app.go("assetview") return true end
  return false
end

function P.tap(app, s, col, row)
  local list = rows(s)
  local per = 19
  local page = math.floor(((s.sel.assets or 1) - 1) / per)
  local idx = page * per + (row - 1)
  if row >= 2 and row <= 20 and list[idx] then
    s.sel.assets = idx
    s.assetView = list[idx]
    app.go("assetview")
    return true
  end
  return false
end

return P
