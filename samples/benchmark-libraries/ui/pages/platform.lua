-- Which platform is this, how sure are we, and what does it mean for the benchmark? Left and right choose another platform to run as:
-- libraries that need the FFI, threads or a network are then judged against that platform, so a Mac can show what a phone or the web
-- would not run. (The measured numbers are always this machine's own.)
local screen = require("ui.screen")
local ST = screen.ST

local P = { title = "Platform", parent = "home", foot = "< > choose platform  A run again" }

function P.draw(app, scr, s)
  local d = s.platform
  local lines = {}
  lines[#lines + 1] = { ("Detected  %s (%s)"):format(d.name, d.certainty), ST.strong }
  lines[#lines + 1] = { ("Running as  %s%s"):format(s.profile.name, s.forced and "  <- chosen by you" or "  (automatic)"), s.forced and ST.warn or ST.ok }
  lines[#lines + 1] = { "Evidence", ST.hint }
  for _, e in ipairs(d.evidence) do lines[#lines + 1] = { ("  %s: %s"):format(e.label, e.value), ST.body } end
  lines[#lines + 1] = { "" }
  lines[#lines + 1] = { "What this means here", ST.hint }
  local function row(label, expect, text) lines[#lines + 1] = { ("  %-18s %-5s  %s"):format(label, expect or "-", text), ST.body } end
  row("FFI libraries", s.profile.ffi, s.profile.ffi == "none" and "skipped (no FFI)" or "tried")
  row("Threads", s.profile.threads, s.profile.threads == "none" and "love-loader cannot run" or "available")
  row("Network", s.profile.network, s.profile.network == "none" and "sock, lovebird: no sockets" or "available")
  row("Window", s.env.window and "yes" or "no", s.env.window and "GUI and drawing libraries are tried" or "they are skipped")
  lines[#lines + 1] = { "" }
  lines[#lines + 1] = { rawget(_G, "jit") and ("LuaJIT's JIT is " .. (jit.status() and "ON" or "OFF") .. ": speed rows measure that.") or "No LuaJIT: plain Lua, so speed rows are slower.", ST.warn }
  lines[#lines + 1] = { "Speed ratings compare libraries on this machine;", ST.dim }
  lines[#lines + 1] = { "only the same platform's numbers can be compared.", ST.dim }
  s.platformPages = screen.lines(scr, { x = 0, y = 1, w = screen.COLS, h = 21, title = "Platform", lines = lines, page = s.sel.platform or 0 })
  s.sel.platform = math.min(s.sel.platform or 0, s.platformPages - 1)
end

function P.act(app, s, a)
  if a == "left" or a == "right" then
    s.override = app.core.platform.cycle(s.override, a == "right" and 1 or -1)
    app.core.refresh(s)
    s.ran = false
    s.results = app.core.suite.newResults()
    app.core.libload.reset()
    s.sel.platform = 0
    app.say(s.override and ("Running as " .. s.profile.name .. ": run the benchmark again.") or ("Automatic: " .. s.profile.name .. "."), s.override and "warn" or "ok")
    return true
  elseif a == "pgdn" or a == "down" then s.sel.platform = math.min((s.platformPages or 1) - 1, (s.sel.platform or 0) + 1) return true
  elseif a == "pgup" or a == "up" then s.sel.platform = math.max(0, (s.sel.platform or 0) - 1) return true
  elseif a == "accept" then
    app.core.start(s)
    s.afterRun = "platform"
    app.go("run")
    return true
  end
  return false
end

return P
