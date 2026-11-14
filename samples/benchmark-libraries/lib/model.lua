-- The Benchmark Libraries' pure logic that is not about libraries: the report text, the leave chord, the idle timer. No LÖVE calls, so
-- plain luajit tests it. (The report grammar is the IO Checker's, which is r3i6nos's: `== section ==`, then PASS / FAIL / INFO / SKIP lines.)

local M = {}

M.VERSION = "1.0"
M.LEAVE_HOLD = 1.0   -- seconds the leave gesture must be held
M.IDLE_LIMIT = 15

--- "LÖVE 11.5.0" from LÖVE's version numbers.
function M.runtime(major, minor, revision)
  if major then return ("LÖVE %d.%d.%d"):format(major, minor or 0, revision or 0) end
  return "LÖVE ?"
end

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

function M.newIdle(limit)
  local idle = { limit = limit or M.IDLE_LIMIT, t = 0 }
  function idle:touch() self.t = 0 end
  function idle:update(dt) self.t = self.t + dt end
  function idle:expired() return self.t >= self.limit end
  return idle
end

--- A number for people: 1234567 -> "1.2M", 12345 -> "12k", 123 -> "123", 0.5 -> "0.50".
function M.si(v)
  if v == nil then return "?" end
  local a = math.abs(v)
  if a >= 1e9 then return ("%.1fG"):format(v / 1e9) end
  if a >= 1e6 then return ("%.1fM"):format(v / 1e6) end
  if a >= 1e4 then return ("%.0fk"):format(v / 1e3) end
  if a >= 1e3 then return ("%.1fk"):format(v / 1e3) end
  if a >= 100 then return ("%.0f"):format(v) end
  if a >= 10 then return ("%.1f"):format(v) end
  return ("%.2f"):format(v)
end

--- How a library's load is judged on a platform. expect: "need" (a failure is a FAIL), "want", "none" (not expected to work here).
function M.judge(expect, ok)
  if expect == "none" then return ok and "INFO" or "SKIP" end
  if ok then return "PASS" end
  return expect == "need" and "FAIL" or "INFO"
end

--- Report builder (the IO Checker's grammar).
function M.newReport(head)
  local r = { lines = {}, counts = { PASS = 0, FAIL = 0, INFO = 0, SKIP = 0 }, fails = {} }
  local function add(s) r.lines[#r.lines + 1] = s end
  add("# Benchmark Libraries report")
  for _, k in ipairs({ "made", "how", "platform", "runtime", "version" }) do
    if head[k] then add(("# %s: %s"):format(k, head[k])) end
  end
  function r:section(name) add(""); add("== " .. name .. " ==") end
  function r:raw(text) add(text) end
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

--- One platform and runtime per file, so builds never overwrite each other.
function M.reportName(platformId, major, minor, ext)
  return ("benchmark-libraries-%s-%s.%s"):format(platformId, major and ("%d.%d"):format(major, minor or 0) or "unknown", ext or "txt")
end

return M
