-- Builds the report from the app's state. Two forms of the same results: the device report (the IO Checker's grammar: `== section ==`, then
-- PASS / FAIL / INFO / SKIP lines, one per library and per cell, so grep and diff work) and a Markdown file of the comparison tables.
--
-- What counts as a FAIL line: a library that was expected to load here and did not, and a specialist's own check that failed. A cell of a
-- comparison that comes out wrong or missing is a RESULT (it is what the benchmark measures), written as INFO with its rating; the
-- summary counts them as findings.

local model = require("lib.model")
local probe = require("lib.sysinfo")
local tables = require("lib.tables")
local runner = require("bench.runner")
local suite = require("bench.suite")
local libload = require("lib.libload")
local rating = require("bench.rating")
local specialists = require("bench.specialists")

local R = {}

local function status(c)
  if c.status == "PASS" then return "PASS" end
  if c.status == "SKIP" then return "SKIP" end
  return "INFO"
end

function R.build(s, how)
  local major, minor, rev = love.getVersion()
  local profile = s.profile
  local r = model.newReport({
    made = os.date("%Y-%m-%d %H:%M:%S"), how = how,
    platform = profile.name .. (s.forced and (" (forced; detected %s)"):format(s.platform.name) or (" (detected, %s)"):format(s.platform.certainty)),
    runtime = model.runtime(major, minor, rev), version = "Benchmark Libraries " .. model.VERSION,
  })

  r:section("platform")
  for _, e in ipairs(s.platform.evidence) do r:line("INFO", e.label, e.value) end
  r:line("INFO", "detected", ("%s (%s)"):format(s.platform.name, s.platform.certainty))
  r:line("INFO", "benchmarks run for", profile.name .. (s.forced and " (chosen by the person, not detected)" or ""))
  r:line("INFO", "what to expect", profile.note or "")
  r:line("INFO", "window", s.env.window and "yes: the windowed checks ran" or "no: checks that need a window are SKIP")
  r:line("INFO", "ffi", s.env.ffi == "none" and "not on this platform: libraries that use it are SKIP" or "available")
  r:line("INFO", "JIT status", rawget(_G, "jit") and (jit.status() and "on" or "off") or "none (plain Lua)")

  r:section("system")
  for _, row in ipairs(probe.system({ shim = s.shim })) do r:line("INFO", row.label, row.value) end

  r:section("libraries")
  local loadFails, loaded, skipped = 0, 0, 0
  for _, e in ipairs(libload.entries) do
    local rec = libload.loaded[e.name]
    if not rec then
      r:line("SKIP", e.name, "not loaded in this run")
    else
      local line = ("%s%s; %d lines"):format(e.kind, e.tag and (", " .. e.tag) or "", e.loc)
      if rec.status == "PASS" then
        r:line("PASS", e.name, ("%s; %.2f ms, %.1f KB kept%s"):format(line, rec.ms, rec.kb, (rec.printed or 0) > 0 and ("; printed %d line(s) to the console when required"):format(rec.printed) or "")); loaded = loaded + 1
      elseif rec.status == "SKIP" then
        r:line("SKIP", e.name, ("%s; %s"):format(line, rec.detail)); skipped = skipped + 1
      else
        local expect = e.kind == "lua" and "need" or (profile.ffi == "none" and "none" or "want")
        local st = model.judge(expect, false)
        if rec.known then st = "INFO"; line = line .. "; known on this LÖVE: " .. rec.known end
        if st == "FAIL" then loadFails = loadFails + 1 end
        r:line(st, e.name, ("%s; did not load: %s"):format(line, rec.detail))
      end
    end
  end
  r:line("INFO", "libraries loaded", ("%d of %d loaded, %d not tried here, %d failed"):format(loaded, #libload.entries, skipped, loadFails))

  local findings, cellsRun, cellsSkipped = 0, 0, 0
  for _, g in ipairs(suite.groups) do
    r:section("compare " .. g.id)
    local info = tables.cells(g, s.results, s.ratings and s.ratings:lookup())
    for _, row in ipairs(runner.rows(g)) do
      for _, m in ipairs(g.members) do
        local i = info[row.id][m.id]
        local c = i.cell
        if c then
          local text = ("%s: %s%s"):format(tables.detail(row, m, i), i.eff and ("rating %d"):format(i.eff) or "no rating", i.best and ", best in this row" or "")
          if c.status == "SKIP" then cellsSkipped = cellsSkipped + 1 else cellsRun = cellsRun + 1 end
          if c.status == "FAIL" or c.status == "NA" then findings = findings + 1 end
          r:line(status(c), ("%s/%s %s"):format(g.id, row.id, m.id), text)
        end
      end
    end
    local libOf = {}
    for _, m in ipairs(g.members) do libOf[m.id] = m.lib end
    for _, sc in ipairs(suite.scores(g, s.results, s.ratings and s.ratings:lookup())) do
      -- "; library X" ends the line so tools/ingest-benchmark.py can tell which curated library a member is
      r:line("INFO", ("%s overall %s"):format(g.id, sc.id), (sc.score and (("%.2f out of 5%s"):format(sc.score, sc.best and " (best)" or "")) or "not rated here") .. "; library " .. libOf[sc.id] .. "; of " .. #g.members)
    end
  end

  r:section("specialists")   -- a check that comes out wrong is INFO (a finding about the library), like a comparison cell
  for _, e in ipairs(specialists.list) do
    local c = s.results.specialists[e.lib]
    if c then r:line(c.status == "PASS" and "PASS" or (c.status == "SKIP" and "SKIP" or "INFO"), e.lib, ("%s: %s%s"):format(e.job, c.note or "", c.ms and (", %.1f ms"):format(c.ms) or "")) end
  end

  r:section("summary")
  r:line("INFO", "comparison cells", ("%d tried, %d not tried here; %d findings (a feature missing or a check that came out wrong: these are the results)"):format(cellsRun, cellsSkipped, findings))
  local mine = s.ratings and s.ratings:count() or 0
  r:line("INFO", "your ratings", mine > 0 and ("%d cells rated by the person; they count in every overall score above"):format(mine) or "none given")
  return r
end

--- The Markdown file: every group's table, then the specialists.
function R.markdown(s)
  local major, minor = love.getVersion()
  local out = { "# Benchmark Libraries", "",
    ("%s on %s, LÖVE %d.%d, %s. Ratings are out of 5; `*` / bold marks the best in a row; `-` means the library has no such feature; `~` means this run could not try it; `[mine]` is a rating given by the person."):format(os.date("%Y-%m-%d"), s.profile.name, major, minor, s.env.window and "with a window" or "no window"), "" }
  for _, g in ipairs(suite.groups) do
    out[#out + 1] = tables.markdown(g, s.results, s.ratings and s.ratings:lookup(), suite.scores(g, s.results, s.ratings and s.ratings:lookup()))
    out[#out + 1] = ""
  end
  out[#out + 1] = "### Libraries that stand alone"
  out[#out + 1] = ""
  out[#out + 1] = "| Library | What it is for | Result |"
  out[#out + 1] = "|---|---|---|"
  for _, e in ipairs(specialists.list) do
    local c = s.results.specialists[e.lib]
    out[#out + 1] = ("| %s | %s | %s |"):format(e.lib, e.job, c and ((c.status == "PASS" and "works" or c.status == "SKIP" and "~ " or "FAIL ") .. (c.status ~= "PASS" and (c.note or "") or "")) or "?")
  end
  return table.concat(out, "\n") .. "\n"
end

--- The files go in the save folder; returns the paths.
function R.write(s, r, platformId)
  local major, minor = love.getVersion()
  local txt, md = model.reportName(platformId, major, minor, "txt"), model.reportName(platformId, major, minor, "md")
  local ok1 = love.filesystem.write(txt, r:text())
  local ok2 = love.filesystem.write(md, R.markdown(s))
  local dir = love.filesystem.getSaveDirectory()
  return ok1 and (dir .. "/" .. txt) or nil, ok2 and (dir .. "/" .. md) or nil
end

return R
