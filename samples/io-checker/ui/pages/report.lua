-- Builds the device report from everything tried so far (the checks nobody ran are SKIP), writes it into the save folder and shows it.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Device report", foot = "A write again  X copy  PgUp/PgDn page" }
local STYLE = { PASS = ST.ok, FAIL = ST.error, INFO = ST.body, SKIP = ST.dim }

local function make(app, s)
  app.job("Writing the report (it loads every asset and tests storage)...", function()
    local r = app.core.buildReport(s, "Device report (interactive)")
    s.reportObj = r
    local path, name = app.core.report.write(r, s.profile.id)
    s.reportFile = path
    local c = r.counts
    if path then app.say(("Written %s (PASS %d FAIL %d SKIP %d)"):format(name, c.PASS, c.FAIL, c.SKIP), c.FAIL == 0 and "ok" or "warn")
    else app.say("Could not write the report: " .. tostring(name), "error") end
  end)
end

function P.enter(app, s)
  s.sel.report = 0
  s.assetResults, s.storage = s.assetResults, s.storage
  make(app, s)
end

local function where(s)
  local id = s.profile.id
  if id == "ios" or id == "ipados" then return "Inside the app's sandbox; copy it with X." end
  if id == "android" then return "Inside the app's storage; copy it with X." end
  if id == "web" then return "In the browser's storage for this page; copy it with X." end
  return s.reportFile or "?"
end

function P.draw(app, scr, s)
  local lines = {}
  if s.reportObj then
    lines[#lines + 1] = { "Saved: " .. where(s), ST.hint }
    for _, l in ipairs(s.reportObj.lines) do
      local status = l:match("^(%u%u%u%u?) ")
      if l:sub(1, 2) == "==" then lines[#lines + 1] = { l, ST.strong }
      elseif status then
        for n, piece in ipairs(screen.wrap(l, screen.COLS - 4)) do lines[#lines + 1] = { (n > 1 and "     " or "") .. piece, STYLE[status] or ST.body } end
      end
    end
    local c = s.reportObj.counts
    lines[#lines + 1] = { ("PASS %d  FAIL %d  INFO %d  SKIP %d"):format(c.PASS, c.FAIL, c.INFO, c.SKIP), c.FAIL == 0 and ST.ok or ST.error }
  else
    lines[1] = { "Writing...", ST.warn }
  end
  s.reportPages = screen.lines(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = "Report", lines = lines, page = s.sel.report })
end

function P.act(app, s, a)
  if a == "accept" then make(app, s) return true end
  if a == "x" then
    if s.reportObj and love.system.setClipboardText then
      love.system.setClipboardText(s.reportObj:text())
      app.say("The report is on the clipboard.", "ok")
    else app.say("No report yet, or no clipboard here.", "warn") end
    return true
  end
  if a == "pgdn" or a == "down" then s.sel.report = math.min((s.reportPages or 1) - 1, s.sel.report + 1) return true end
  if a == "pgup" or a == "up" then s.sel.report = math.max(0, s.sel.report - 1) return true end
  return false
end

return P
