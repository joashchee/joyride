-- Benchmark Libraries. A sample project that ships with JoyrIDE: it loads every library JoyrIDE offers, compares the ones that do the
-- same job (a table of features, speed and cost, rated), and writes a report for the platform it runs on.
-- `love . --selftest` drives every screen with no GPU (tests/selftest.lua). `love . --report` runs the benchmark with no window,
-- prints the report and exits (a library that should load and does not gives exit code 1).
local report = false
for _, a in ipairs(arg or {}) do if a == "--report" or a == "--selftest" then report = true end end

function love.conf(t)
  t.identity = "joyride-benchmark-libraries"
  t.version = "11.5"
  t.window.title = "Benchmark Libraries"
  t.window.width = 768             -- 48 columns by 25 rows of 8x16 cells, twice over
  t.window.height = 800
  t.window.minwidth = 384
  t.window.minheight = 400
  t.window.resizable = true
  t.window.highdpi = true
  t.window.vsync = 1
  t.modules.physics = true         -- breezefield (a love.physics wrapper) is tried for real
  if report then
    t.window = false               -- no window, no graphics: the headless run
    t.modules.window = false
    t.modules.graphics = false
  end
end
