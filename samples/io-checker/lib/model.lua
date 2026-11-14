-- The IO Checker's pure logic: no LÖVE calls, so plain luajit tests it (tests/test_model.lua).
-- The input record (what every device did), the leave chord and idle timer, the frame-time statistics of the motion test, the test
-- tones, the video patterns, how a result is judged against what a platform expects, and the device report's text.

local M = {}

M.VERSION = "1.0"
M.ZONE = 0.5          -- an axis counts as "pushed" past this
M.LEAVE_HOLD = 1.0    -- seconds the leave gesture must be held
M.IDLE_LIMIT = 15     -- seconds without input before an input page goes back to the list by itself
M.IDLE_WARN = 10
M.CHATTER = 0.03      -- a press this soon (s) after the same button's release is chatter (a bouncing contact)
M.LOG_LINES = 6
M.RATE = 22050        -- the test tones' sample rate

function M.fmt(v) return string.format("%+.2f", v or 0) end

--- "12.0.0" style runtime name from LÖVE's version numbers.
function M.runtime(major, minor, revision)
  if major then return ("LÖVE %d.%d.%d"):format(major, minor or 0, revision or 0) end
  return "LÖVE ?"
end

--- A string cut to n characters (bytes, ASCII is what the screen shows), with an ellipsis when it was cut.
function M.fit(s, n)
  s = tostring(s or "")
  if #s <= n then return s end
  if n <= 1 then return s:sub(1, n) end
  return s:sub(1, n - 1) .. "~"
end

function M.gb(bytes)
  if not bytes then return "?" end
  if bytes >= 1073741824 then return ("%.1f GB"):format(bytes / 1073741824) end
  if bytes >= 1048576 then return ("%.0f MB"):format(bytes / 1048576) end
  return ("%d KB"):format(math.floor(bytes / 1024))
end

-- ---- timers ------------------------------------------------------------------------------------------------------------------

--- Leave chord: held() must stay true for `hold` seconds in a row.
function M.newChord(hold)
  local c = { hold = hold or M.LEAVE_HOLD, t = 0 }
  function c:update(dt, held)
    if held then self.t = self.t + dt else self.t = 0 end
    return self.t >= self.hold
  end
  function c:progress() return math.min(1, self.t / self.hold) end
  return c
end

--- No input for `limit` seconds: an input page goes back by itself, so a device with a dead Back cannot trap anyone.
function M.newIdle(limit, warn)
  local idle = { limit = limit or M.IDLE_LIMIT, warn = warn or M.IDLE_WARN, t = 0 }
  function idle:touch() self.t = 0 end
  function idle:update(dt) self.t = self.t + dt end
  function idle:remaining() return math.max(0, self.limit - self.t) end
  function idle:warning() return self:remaining() <= self.warn end
  function idle:expired() return self.t >= self.limit end
  return idle
end

-- ---- the input record --------------------------------------------------------------------------------------------------------

--- Everything every device did since the app started: the checklist of an input test. Times are seconds (love.timer).
function M.newInput()
  local r = {
    keys = {}, keyCount = 0, text = "", mods = {},
    mouse = { moved = false, x = 0, y = 0, buttons = {}, wheel = 0, presses = 0, cursor = nil },
    touch = { max = 0, now = {}, taps = 0, seen = false, pressureMax = 0, moves = 0 },
    pads = {},     -- by joystick id: { name, guid, buttons = {n -> presses}, down, axes = {a -> {min,max}}, hats = {}, last = {n -> release time}, chatter = {n -> count}, gamepad }
    sensors = {},  -- by sensor type: { n = readings, min = {x,y,z}, max = {x,y,z}, last = {x,y,z} }
    log = {},
  }
  function r:note(line)
    self.log[#self.log + 1] = line
    if #self.log > M.LOG_LINES then table.remove(self.log, 1) end
  end
  function r:key(k, isrepeat)
    if not self.keys[k] then self.keyCount = self.keyCount + 1 end
    self.keys[k] = (self.keys[k] or 0) + (isrepeat and 0 or 1)
    self:note("key " .. k)
  end
  function r:addText(t)
    self.text = (self.text .. t):sub(-40)
  end
  function r:mouseMoved(x, y) self.mouse.moved = true; self.mouse.x, self.mouse.y = x, y end
  function r:mouseButton(b, down, istouch)
    if istouch then return end -- a touch is also reported as the mouse; it is counted as a touch
    self.mouse.buttons[b] = (self.mouse.buttons[b] or 0) + (down and 1 or 0)
    if down then self.mouse.presses = self.mouse.presses + 1 end
    self:note(("mouse %s %s"):format(tostring(b), down and "DOWN" or "UP"))
  end
  function r:wheel(dx, dy) self.mouse.wheel = self.mouse.wheel + (dy or 0) + (dx or 0); self:note(("wheel %+d %+d"):format(dx or 0, dy or 0)) end
  function r:touchDown(id, x, y, pressure)
    self.touch.seen = true
    self.touch.now[id] = { x = x, y = y }
    local n = 0
    for _ in pairs(self.touch.now) do n = n + 1 end
    if n > self.touch.max then self.touch.max = n end
    self.touch.taps = self.touch.taps + 1
    if pressure and pressure > self.touch.pressureMax then self.touch.pressureMax = pressure end
    self:note(("touch down (%d now)"):format(n))
  end
  function r:touchMove(id, x, y) self.touch.now[id] = { x = x, y = y }; self.touch.moves = self.touch.moves + 1 end
  function r:touchUp(id) self.touch.now[id] = nil; self:note("touch up") end

  --- A joystick's record, made on first sight. info = { name, guid, buttons, axes, hats, gamepad }.
  function r:pad(id, info)
    local p = self.pads[id]
    if not p then
      p = { name = info.name, guid = info.guid, nbuttons = info.buttons or 0, naxes = info.axes or 0, nhats = info.hats or 0, gamepad = info.gamepad,
            buttons = {}, last = {}, chatter = {}, axes = {}, hats = {}, rumble = nil }
      self.pads[id] = p
    end
    return p
  end
  function r:padPress(id, b, t)
    local p = self.pads[id]
    if not p then return end
    p.buttons[b] = (p.buttons[b] or 0) + 1
    if p.last[b] and t and t - p.last[b] < M.CHATTER then p.chatter[b] = (p.chatter[b] or 0) + 1 end
    self:note(("pad %s button %s DOWN"):format(tostring(id), tostring(b)))
  end
  function r:padRelease(id, b, t)
    local p = self.pads[id]
    if p then p.last[b] = t end
  end
  function r:padAxis(id, a, v)
    local p = self.pads[id]
    if not p then return end
    local rr = p.axes[a]
    if not rr then rr = { min = v, max = v }; p.axes[a] = rr end
    if v < rr.min then rr.min = v end
    if v > rr.max then rr.max = v end
  end
  function r:padHat(id, h, dir)
    local p = self.pads[id]
    if not p then return end
    p.hats[dir] = (p.hats[dir] or 0) + 1
    self:note(("pad %s hat %s %s"):format(tostring(id), tostring(h), dir))
  end
  --- Buttons of a pad (1..nbuttons) that were never pressed.
  function r:missing(id)
    local p, out = self.pads[id], {}
    if not p then return out end
    for b = 1, p.nbuttons do if not p.buttons[b] then out[#out + 1] = b end end
    return out
  end
  function r:chatterTotal(id)
    local n = 0
    for _, c in pairs((self.pads[id] or { chatter = {} }).chatter) do n = n + c end
    return n
  end
  function r:sensor(kind, x, y, z)
    local s = self.sensors[kind]
    if not s then s = { n = 0, min = { x, y, z }, max = { x, y, z } }; self.sensors[kind] = s end
    s.n = s.n + 1
    s.last = { x, y, z }
    local v = { x, y, z }
    for i = 1, 3 do
      if v[i] < s.min[i] then s.min[i] = v[i] end
      if v[i] > s.max[i] then s.max[i] = v[i] end
    end
  end
  return r
end

--- Which side of the zone an axis value is on: -1, 0 or 1.
function M.zone(v)
  if v <= -M.ZONE then return -1 end
  if v >= M.ZONE then return 1 end
  return 0
end

-- ---- the motion test ---------------------------------------------------------------------------------------------------------

--- Frame-time statistics: add(dt) each frame; stats() has frames, mean, worst, fps and how many frames ran over 1.5 times the mean.
function M.newFrames()
  local f = { dts = {} }
  function f:reset() self.dts = {} end
  function f:add(dt)
    if dt and dt > 0 then self.dts[#self.dts + 1] = dt end
  end
  function f:stats()
    local n = #self.dts
    if n < 30 then return nil end
    local sum, worst = 0, 0
    for _, d in ipairs(self.dts) do sum = sum + d; if d > worst then worst = d end end
    local mean = sum / n
    local late = 0
    for _, d in ipairs(self.dts) do if d > mean * 1.5 then late = late + 1 end end
    return { frames = n, mean = mean, worst = worst, fps = 1 / mean, late = late }
  end
  return f
end

function M.framesLine(st)
  if not st then return "not measured: show the Motion pattern for a few seconds" end
  return ("%.1f fps, mean %.1f ms, worst %.1f ms, %d of %d frames late"):format(st.fps, st.mean * 1000, st.worst * 1000, st.late, st.frames)
end

-- ---- test tones and video patterns ---------------------------------------------------------------------------------------------

M.TONES = {
  { id = "left", label = "1 kHz, left", hz = 1000, left = 1, right = 0, secs = 1.0 },
  { id = "right", label = "1 kHz, right", hz = 1000, left = 0, right = 1, secs = 1.0 },
  { id = "both", label = "1 kHz, both", hz = 1000, left = 1, right = 1, secs = 1.0 },
  { id = "low", label = "440 Hz, both", hz = 440, left = 1, right = 1, secs = 1.0 },
  { id = "sweep", label = "Sweep 100 Hz to 8 kHz", sweep = { 100, 8000 }, left = 1, right = 1, secs = 2.0 },
  { id = "quiet", label = "1 kHz at 10%", hz = 1000, left = 0.1, right = 0.1, secs = 1.0 },
}

function M.toneFrames(t) return math.floor(t.secs * M.RATE) end

--- Sample k (0-based) of tone t: left, right in -1..1, with a 20 ms fade in and out so nothing clicks.
function M.toneSample(t, k)
  local n = M.toneFrames(t)
  local sec = k / M.RATE
  local phase
  if t.sweep then
    local f0, f1 = t.sweep[1], t.sweep[2]
    phase = 2 * math.pi * (f0 * sec + (f1 - f0) * sec * sec / (2 * t.secs))
  else
    phase = 2 * math.pi * t.hz * sec
  end
  local fade = math.min(1, k / (0.02 * M.RATE), (n - 1 - k) / (0.02 * M.RATE))
  local v = math.sin(phase) * 0.5 * math.max(0, fade)
  return v * t.left, v * t.right
end

M.PATTERNS = {
  { id = "bars", label = "Colour bars and the 16 VGA colours", look = "Eight bars in order, then sixteen blocks, each clearly different." },
  { id = "white", label = "White", look = "Even white. Dark dots are stuck pixels; patches are backlight bleed." },
  { id = "red", label = "Red", look = "Even red. Any pixel that is not red is stuck." },
  { id = "green", label = "Green", look = "Even green." },
  { id = "blue", label = "Blue", look = "Even blue." },
  { id = "black", label = "Black", look = "Even black. Bright dots are stuck; glow is backlight bleed." },
  { id = "ramps", label = "Grey, red, green and blue ramps", look = "Smooth ramps. Visible steps mean fewer colour bits." },
  { id = "grid", label = "Checker, grid, edges and corners", look = "Left: a 1-pixel checker (should look flat grey). Right: a 10-pixel grid. A white frame on all four edges and red corners: if one is cut off, the screen overscans or the window is cropped." },
  { id = "safe", label = "Safe area and circle", look = "The circle must be round (square pixels). The inner box is the safe area: notches and home bars stay outside it." },
  { id = "motion", label = "Motion and frame rate", look = "A bar moving right and a row of boxes lit one a frame. A gap in a photo of the row is a dropped frame." },
}

-- ---- what a platform expects --------------------------------------------------------------------------------------------------

--- Judge one probed fact against what the platform expects. expect is "need" (a missing one fails), "want" (a missing one is only
--- noted), or "none" (not expected here; present is just information).
--- Returns status ("PASS" "FAIL" "INFO"), and a short reason.
function M.judge(expect, present)
  if expect == "need" then
    if present then return "PASS", "present, as this platform needs" end
    return "FAIL", "missing, and this platform needs it"
  elseif expect == "want" then
    if present then return "PASS", "present" end
    return "INFO", "not found (optional on this platform)"
  end
  if present then return "INFO", "present, though this platform does not usually have it" end
  return "PASS", "absent, as expected here"
end

--- Judge a load of an asset against whether the platform and LÖVE are expected to load it: "yes", "no" or "maybe".
function M.judgeLoad(expect, ok)
  if expect == "yes" then return ok and "PASS" or "FAIL" end
  if expect == "no" then return ok and "INFO" or "PASS" end
  return ok and "PASS" or "INFO" -- "maybe": loading is good news, refusing is only noted
end

-- ---- the device report ---------------------------------------------------------------------------------------------------------

--- Report builder. The grammar is r3i6nos's docs/DEVICE-REPORTS.md: `== section ==` headers, then `STATUS label: value` lines with
--- STATUS one of PASS, FAIL, INFO, SKIP. A header block says what made it.
function M.newReport(head)
  local r = { lines = {}, counts = { PASS = 0, FAIL = 0, INFO = 0, SKIP = 0 }, fails = {} }
  local function add(s) r.lines[#r.lines + 1] = s end
  add("# IO Checker device report")
  for _, k in ipairs({ "made", "how", "platform", "runtime", "version" }) do
    if head[k] then add(("# %s: %s"):format(k, head[k])) end
  end
  function r:section(name) add(""); add("== " .. name .. " ==") end
  function r:line(status, label, value)
    self.counts[status] = (self.counts[status] or 0) + 1
    if status == "FAIL" then self.fails[#self.fails + 1] = label end
    add(("%-4s %s: %s"):format(status, label, tostring(value)))
  end
  function r:check(ok, label, value) self:line(ok and "PASS" or "FAIL", label, value) end
  function r:text()
    return table.concat(self.lines, "\n") .. ("\n\n# PASS %d  FAIL %d  INFO %d  SKIP %d\n"):format(self.counts.PASS, self.counts.FAIL, self.counts.INFO, self.counts.SKIP)
  end
  return r
end

--- The report's file name: one a platform and runtime, so builds never overwrite each other.
function M.reportName(platformId, major, minor)
  return ("io-checker-report-%s-%s.txt"):format(platformId, major and ("%d.%d"):format(major, minor or 0) or "unknown")
end

return M
