-- Benchmark Libraries: the entry point. lib/core.lua holds the state, bench/ the comparisons, ui/ the screens.
love.filesystem.setRequirePath("?.lua;?/init.lua")   -- the project's own modules; each library gets its own path when it is loaded (lib/libload.lua)
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
    -- `love . --report`: no window. Run the benchmark, print the report, keep it, leave with 1 if a library failed that should not.
    local r = core.headlessReport()
    io.write(r:text())
    local txt, md = core.report.write(core.state, r, core.state.profile.id)
    io.stderr:write(txt and ("Report file: " .. txt .. "\n") or "The report could not be written to the save folder.\n")
    io.stderr:write(md and ("Tables file: " .. md .. "\n") or "")
    love.event.quit(#r.fails > 0 and 1 or 0)
    return
  end
  require("ui.app").install(core)
end
