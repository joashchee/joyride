-- IO Checker. A sample project that ships with JoyrIDE: it works out which of Joyride's platforms it is running on, checks that
-- platform's input, screen, sound and storage, loads every kind of asset LÖVE supports, and writes a device report.
-- `love . --selftest` drives every screen against a mock of LÖVE's drawing, for machines with no GPU (tests/selftest.lua).
-- `love . --report` runs the checks that need no person and no window, prints the report and exits (FAIL gives exit code 1).
local report = false
for _, a in ipairs(arg or {}) do if a == "--report" or a == "--selftest" then report = true end end

function love.conf(t)
  t.identity = "joyride-io-checker"
  t.version = "11.5"
  t.window.title = "IO Checker"
  t.window.width = 768             -- 384x400 text cells' worth of pixels, twice over: 48 columns by 25 rows of 8x16
  t.window.height = 800
  t.window.minwidth = 384
  t.window.minheight = 400
  t.window.resizable = true
  t.window.highdpi = true
  t.window.vsync = 1
  t.accelerometerjoystick = true   -- LÖVE 11.5 on a phone: the accelerometer shows up as a joystick
  t.modules.physics = false        -- the only module not needed
  if report then
    t.window = false               -- no window, no graphics: the headless checks
    t.modules.window = false
    t.modules.graphics = false
  end
end
