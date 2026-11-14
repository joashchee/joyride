-- IO Checker: the entry point. lib/core.lua holds the state and the checks, ui/ the screens.
local core = require("lib.core")

function love.load(args)
  core.init(args)
  for _, a in ipairs(args or {}) do
    if a == "--selftest" then
      local failed = require("tests.selftest").run(core)
      love.event.quit(failed and 1 or 0)
      return
    end
  end
  if not love.graphics then
    -- `love . --report`: no window. Run what needs no person, print it, leave with 1 if anything failed.
    local r = core.headlessReport()
    io.write(r:text())
    -- Also keep it: in LÖVE's save folder for this project, and say where (on stderr, so a redirected report stays clean).
    local path = core.report.write(r, core.state.profile.id)
    io.stderr:write(path and ("Report file: " .. path .. "\n") or "The report could not be written to the save folder.\n")
    love.event.quit(#r.fails > 0 and 1 or 0)
    return
  end
  require("ui.app").install(core)
end
