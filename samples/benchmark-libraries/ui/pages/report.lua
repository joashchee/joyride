-- Builds the report from the benchmark (running it first if it has not been), writes the text report and the Markdown tables into the
-- save folder and shows the text.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Report", parent = "home", foot = "A write again  X copy text  C copy tables  PgUp/PgDn" }
local STYLE = { PASS = ST.ok, FAIL = ST.error, INFO = ST.body, SKIP = ST.dim }

local function make(app, s)
  if not s.ran then
    app.core.start(s)
    s.afterRun = "report"
    app.go("run")
    return
  end
  app.job("Writing the report...", function()
    local r = app.core.buildReport(s, "Report (interactive)")
    s.reportObj = r
    local txt, md = app.core.report.write(s, r, s.profile.id)
    s.reportFile, s.tablesFile = txt, md
    local c = r.counts
    if txt then app.say(("Written (PASS %d FAIL %d INFO %d SKIP %d)"):format(c.PASS, c.FAIL, c.INFO, c.SKIP), c.FAIL == 0 and "ok" or "warn")
    else app.say("Could not write the report.", "error") end
  end)
end

function P.enter(app, s)
  s.sel.report = 0
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
        for n, piece in ipairs(screen.wrap(l, screen.COLS - 4)) do
          if n <= 2 then lines[#lines + 1] = { (n > 1 and "     " or "") .. piece, STYLE[status] or ST.body } end
        end
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
  if a == "accept" then s.ran = false; make(app, s) return true end
  if a == "x" or a == "y" then
    if s.reportObj and love.system.setClipboardText then
      love.system.setClipboardText(a == "x" and s.reportObj:text() or app.core.report.markdown(s))
      app.say(a == "x" and "The report text is on the clipboard." or "The tables (Markdown) are on the clipboard.", "ok")
    else app.say("No report yet, or no clipboard here.", "warn") end
    return true
  end
  if a == "pgdn" or a == "down" then s.sel.report = math.min((s.reportPages or 1) - 1, s.sel.report + 1) return true end
  if a == "pgup" or a == "up" then s.sel.report = math.max(0, s.sel.report - 1) return true end
  return false
end

return P
