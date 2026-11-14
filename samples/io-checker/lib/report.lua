-- Builds the device report from the app's state. Grammar: r3i6nos's docs/DEVICE-REPORTS.md (== section ==, then PASS / FAIL / INFO /
-- SKIP lines). Nothing here draws or reads input; it only turns what was probed and what the person did into lines, so the same
-- code writes the report from the interactive app and from `love . --report` (nothing pressed: those lines are SKIP).

local model = require("lib.model")
local probe = require("lib.probe")

local R = {}

local function gamepadRows(r, s)
  local any = false
  for id, p in pairs(s.input.pads) do
    any = true
    local name = ("pad %s (%s)"):format(tostring(id), p.name or "?")
    r:line("INFO", name, ("%d buttons, %d axes, %d hats, SDL gamepad mapping: %s, GUID %s"):format(p.nbuttons, p.naxes, p.nhats, p.gamepad and "yes" or "no", p.guid or "?"))
    local pressed = 0
    for _ in pairs(p.buttons) do pressed = pressed + 1 end
    if pressed == 0 then
      r:line("SKIP", name .. " buttons", "none pressed on the Gamepad page")
    else
      local missing = s.input:missing(id)
      r:check(#missing == 0, name .. " every button pressed", #missing == 0 and (p.nbuttons .. " of " .. p.nbuttons) or ("never pressed: " .. table.concat(missing, " ")))
      local chat = s.input:chatterTotal(id)
      r:check(chat == 0, name .. " no chatter", chat == 0 and "none" or (chat .. " presses within 30 ms of a release (a bouncing contact)"))
    end
    for a = 1, p.naxes do
      local rr = p.axes[a]
      if not rr or (rr.max - rr.min) < 0.2 then
        r:line("SKIP", ("%s axis %d"):format(name, a), "not moved")
      else
        r:check(rr.min <= -0.9 and rr.max >= 0.9, ("%s axis %d range"):format(name, a), ("%s to %s (goal past -0.90 and +0.90)"):format(model.fmt(rr.min), model.fmt(rr.max)))
      end
    end
    if next(p.hats) then
      local d = {}
      for dir, n in pairs(p.hats) do d[#d + 1] = dir .. "x" .. n end
      table.sort(d)
      r:line("INFO", name .. " hat directions", table.concat(d, " "))
    end
  end
  -- Rest position, read now: hands off.
  for _, j in ipairs(s.joysticks or {}) do
    local bad = {}
    for a = 1, j.axes do
      local v = j.axis(a)
      if math.abs(v) > 0.10 then bad[#bad + 1] = ("%d (%s)"):format(a, model.fmt(v)) end
    end
    r:check(#bad == 0, ("%s axes at rest"):format(j.name), #bad == 0 and "all within 0.10 of centre" or ("off centre: " .. table.concat(bad, ", ") .. " (hands off the sticks when the report is made)"))
  end
  if not any and #(s.joysticks or {}) == 0 then r:line("SKIP", "game controllers", "none connected") end
end

--- s = the app's state. how = a short phrase for the header ("Device report (interactive)" or "love . --report").
function R.build(s, how)
  local major, minor, rev = love.getVersion()
  local profile = s.profile
  local r = model.newReport({
    made = os.date("%Y-%m-%d %H:%M:%S"), how = how,
    platform = profile.name .. (s.forced and (" (forced; detected %s)"):format(s.platform.name) or (" (detected, %s)"):format(s.platform.certainty)),
    runtime = model.runtime(major, minor, rev), version = "IO Checker " .. model.VERSION,
  })

  r:section("platform")
  for _, e in ipairs(s.platform.evidence) do r:line("INFO", e.label, e.value) end
  r:line("INFO", "detected", ("%s (%s)"):format(s.platform.name, s.platform.certainty))
  r:line("INFO", "checks run for", profile.name .. (s.forced and " (chosen by the person, not detected)" or ""))
  r:line("INFO", "what to expect", profile.note or "")

  r:section("system")
  for _, row in ipairs(probe.system({ shim = s.shim })) do r:line("INFO", row.label, row.value) end

  r:section("platform expectations")
  for _, c in ipairs(s.caps) do
    local expect = profile[c.key]
    if expect then
      if c.present == nil then
        r:line("SKIP", c.label, ("%s (this platform: %s)"):format(c.detail, expect))
      else
        local status, why = model.judge(expect, c.present)
        r:line(status, c.label, ("%s; %s (this platform: %s)"):format(c.detail, why, expect))
      end
    end
  end

  r:section("input")
  local inp = s.input
  r:line(inp.keyCount > 0 and "INFO" or "SKIP", "keyboard", inp.keyCount > 0 and ("%d different keys seen"):format(inp.keyCount) or "no key pressed")
  r:line(inp.mouse.moved and "INFO" or "SKIP", "mouse", inp.mouse.moved and ("moved; %d clicks, wheel %+d"):format(inp.mouse.presses, inp.mouse.wheel) or "not moved")
  r:line(inp.touch.seen and "INFO" or "SKIP", "touch", inp.touch.seen and ("%d at once at most, %d taps, %d moves, pressure up to %.2f"):format(inp.touch.max, inp.touch.taps, inp.touch.moves, inp.touch.pressureMax) or "no touch seen")
  for kind, sen in pairs(inp.sensors) do r:line("INFO", "sensor " .. kind, ("%d readings; last %s"):format(sen.n, table.concat({ model.fmt(sen.last[1]), model.fmt(sen.last[2]), model.fmt(sen.last[3]) }, " "))) end
  r:line(inp.text ~= "" and "INFO" or "SKIP", "text input", inp.text ~= "" and ("typed: " .. inp.text) or "nothing typed")
  gamepadRows(r, s)

  r:section("video")
  for _, row in ipairs((s.videoInfo or { rows = {} }).rows) do r:line("INFO", row.label, row.value) end
  local st = s.frames:stats()
  if st then r:check(st.fps >= 55 and st.late <= st.frames / 100, "motion", model.framesLine(st) .. " (goal 55+ fps, under 1% late)")
  else r:line("SKIP", "motion", model.framesLine(nil)) end
  local looked = {}
  for _, pat in ipairs(model.PATTERNS) do if s.seen[pat.id] then looked[#looked + 1] = pat.id end end
  r:line("INFO", "patterns shown", #looked > 0 and table.concat(looked, " ") or "none")
  r:line("INFO", "how they looked", "not judged by a program: say in words what was wrong, if anything")

  r:section("sound")
  local a = probe.audio()
  for _, row in ipairs(a.rows) do r:line("INFO", row.label, row.value) end
  r:check(love.audio ~= nil and not a.error, "output opened", a.error or "OpenAL started")
  for _, t in ipairs(model.TONES) do
    local ans = s.answers[t.id]
    if ans == "heard" then r:line("PASS", "tone " .. t.label, "heard")
    elseif ans == "silent" then r:line("FAIL", "tone " .. t.label, "marked silent")
    else r:line("SKIP", "tone " .. t.label, ans == "played" and "played, not answered" or "not played") end
  end

  r:section("storage")
  for _, row in ipairs(s.storage or probe.storage(profile)) do r:line(row.status, row.label, row.value) end

  r:section("assets")
  local counts = {}
  for _, res in ipairs(s.assetResults or {}) do
    local e = res.entry
    r:line(res.status, e.path, ("%s%s: %s%s"):format(e.type, res.unlisted and " (not in the list; found by extension)" or "", res.detail or "", res.ms and (", %.1f ms"):format(res.ms) or ""))
    counts[e.type] = counts[e.type] or { n = 0, ok = 0 }
    counts[e.type].n = counts[e.type].n + 1
    if res.status == "PASS" then counts[e.type].ok = counts[e.type].ok + 1 end
  end
  local types = {}
  for t in pairs(counts) do types[#types + 1] = t end
  table.sort(types)
  for _, t in ipairs(types) do r:line("INFO", "type " .. t, ("%d of %d loaded as expected"):format(counts[t].ok, counts[t].n)) end
  if s.creditsText then r:line("INFO", "CREDITS.txt", ("%d bytes, written by Joyride's export"):format(#s.creditsText))
  else r:line("INFO", "CREDITS.txt", "not in this build (Joyride's export adds it when assets need credit)") end
  return r
end

--- Writes the report into the save folder; returns the full path (or nil, why).
function R.write(r, platformId)
  local major, minor = love.getVersion()
  local name = model.reportName(platformId, major, minor)
  local ok, err = love.filesystem.write(name, r:text())
  if not ok then return nil, tostring(err) end
  return love.filesystem.getSaveDirectory() .. "/" .. name, name
end

return R
