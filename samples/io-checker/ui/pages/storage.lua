-- Write, read, append, list and delete in the save folder; write and read back 4 MB byte for byte; check the run counter that proves
-- saves survive a restart; and try plain io where the platform allows it.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Storage", foot = "A test again  PgUp/PgDn page  B back" }
local STYLE = { PASS = ST.ok, FAIL = ST.error, INFO = ST.body, SKIP = ST.dim }

local function run(app, s)
  app.job("Testing storage (a few seconds)...", function()
    local rows = app.core.runStorage(s)
    local bad = 0
    for _, r in ipairs(rows) do if r.status == "FAIL" then bad = bad + 1 end end
    app.say(bad == 0 and "Storage passed." or ("%d check(s) failed: see the list."):format(bad), bad == 0 and "ok" or "error")
  end)
end

function P.enter(app, s)
  s.sel.storage = 0
  if not s.storage then run(app, s) end
end

function P.draw(app, scr, s)
  local lines = {}
  for _, r in ipairs(s.storage or {}) do
    lines[#lines + 1] = { ("%-4s %s"):format(r.status, r.label), STYLE[r.status] }
    for _, l in ipairs(screen.wrap(r.value, screen.COLS - 9)) do lines[#lines + 1] = { "     " .. l, ST.dim } end
  end
  if #lines == 0 then lines[1] = { "Testing...", ST.warn } end
  s.storagePages = screen.lines(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = "Save folder: " .. screen.fit(love.filesystem.getIdentity(), 28), lines = lines, page = s.sel.storage })
end

function P.act(app, s, a)
  if a == "accept" then run(app, s) return true end
  if a == "pgdn" or a == "down" then s.sel.storage = math.min((s.storagePages or 1) - 1, s.sel.storage + 1) return true end
  if a == "pgup" or a == "up" then s.sel.storage = math.max(0, s.sel.storage - 1) return true end
  return false
end

return P
