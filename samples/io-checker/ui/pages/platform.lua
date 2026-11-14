-- Which platform is this, how sure are we, and what does it expect? Left and right choose another platform to check as.
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Platform", foot = "< > choose platform  A re-probe  B back" }

local ORDER = {
  { "keyboard", "Keyboard" }, { "mouse", "Mouse or pointer" }, { "touch", "Touch screen" }, { "gamepad", "Game controller" },
  { "sensors", "Motion sensors" }, { "haptics", "Vibration" }, { "clipboard", "Clipboard" }, { "resizable", "Resizable window" },
  { "fullscreen", "Fullscreen" }, { "safearea", "Safe area" }, { "realfs", "Real file paths" }, { "threads", "Threads" },
  { "ffi", "ffi library" }, { "popen", "io.popen" }, { "video", "Video (Theora)" }, { "network", "Network (luasocket)" }, { "quit", "Can quit" },
}

local function cap(s, key)
  for _, c in ipairs(s.caps) do if c.key == key then return c end end
end

function P.draw(app, scr, s)
  local d = s.platform
  local lines = {}
  lines[#lines + 1] = { ("Detected  %s (%s)"):format(d.name, d.certainty), ST.strong }
  lines[#lines + 1] = { ("Checking  %s%s"):format(s.profile.name, s.forced and "  <- chosen by you" or "  (automatic)"), s.forced and ST.warn or ST.ok }
  lines[#lines + 1] = { "Evidence", ST.hint }
  for _, e in ipairs(d.evidence) do lines[#lines + 1] = { ("  %s: %s"):format(e.label, e.value), ST.body } end
  lines[#lines + 1] = { "" }
  lines[#lines + 1] = { "What this platform expects, and what is here now", ST.hint }
  for _, o in ipairs(ORDER) do
    local expect = s.profile[o[1]]
    local c = cap(s, o[1])
    local now, st = "-", ST.dim
    if c then
      if c.present == nil then now, st = "untried", ST.dim
      else
        local verdict = app.core.model.judge(expect, c.present)
        now = c.present and "yes" or "no"
        st = verdict == "FAIL" and ST.error or (verdict == "PASS" and ST.ok or ST.warn)
      end
    end
    lines[#lines + 1] = { ("  %-18s %-5s  %s"):format(o[2], expect or "-", now), st }
  end
  s.platformPages = screen.lines(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = "Platform", lines = lines, page = s.sel.platform or 0 })
  s.sel.platform = math.min(s.sel.platform or 0, s.platformPages - 1)
end

function P.act(app, s, a)
  if a == "left" or a == "right" then
    s.override = app.core.platform.cycle(s.override, a == "right" and 1 or -1)
    app.core.refresh(s)
    s.assetResults, s.storage = nil, nil
    s.sel.platform = 0
    app.say(s.override and ("Checking as " .. s.profile.name .. ".") or ("Automatic: " .. s.profile.name .. "."), s.override and "warn" or "ok")
    return true
  elseif a == "pgdn" or a == "down" then s.sel.platform = math.min((s.platformPages or 1) - 1, (s.sel.platform or 0) + 1) return true
  elseif a == "pgup" or a == "up" then s.sel.platform = math.max(0, (s.sel.platform or 0) - 1) return true
  elseif a == "accept" then
    app.core.refresh(s)
    app.say("Probed again.", "ok")
    return true
  end
  return false
end

return P
