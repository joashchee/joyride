-- Timing, a seeded random stream and the cell record. Everything a benchmark workload needs and nothing about any library.
--
-- Workloads are deterministic: the random stream is this file's own (a 32-bit LCG in integer-safe arithmetic), never math.random, so
-- two runs, two machines and LÖVE 11.5 and 12.0 do the same work. Time is os.clock (CPU time), memory is Lua's own count.

local H = {}

--- A seeded stream of numbers in [0, 1). Same seed, same numbers, everywhere.
function H.rng(seed)
  local s = (seed or 1) % 2147483647
  if s <= 0 then s = s + 2147483646 end
  return function()
    s = (s * 48271) % 2147483647   -- Park-Miller; the product stays below 2^53, so doubles hold it exactly
    return (s - 1) / 2147483646
  end
end

--- Integer in [lo, hi] from a stream.
function H.int(rnd, lo, hi) return lo + math.floor(rnd() * (hi - lo + 1)) end

H.MIN_TIME = 0.02   -- seconds a timing batch must run for, so the clock's grain does not matter
H.REPS = 3          -- batches per measurement: the fastest is kept (the others had interruptions)

--- Calls `work` over and over and returns calls per second (times `units`, the operations one call does). Also returns the value of
--- the last call, so a caller can check the answer was right. A work function that errors raises here.
function H.rate(work, units, opts)
  opts = opts or {}
  local minTime, reps = opts.minTime or H.MIN_TIME, opts.reps or H.REPS
  local clock = os.clock
  local last = work()                 -- one call to warm up, and to get the answer
  local n = 1
  -- Find a batch size that takes at least minTime.
  while true do
    local t0 = clock()
    for _ = 1, n do work() end
    local dt = clock() - t0
    if dt >= minTime or n >= 1e7 then break end
    n = dt <= 0 and n * 10 or math.max(n + 1, math.min(n * 10, math.ceil(n * minTime / dt * 1.2)))
  end
  local best = 0
  for _ = 1, reps do
    local t0 = clock()
    for _ = 1, n do work() end
    local dt = clock() - t0
    if dt > 0 then best = math.max(best, n * (units or 1) / dt) end
  end
  return best, last
end

--- Memory (KB) that one call of `work` allocates: the count before and after, with the collector stopped so nothing is freed
--- in between. The first call is made before, so lazily built tables are not counted.
function H.alloc(work)
  work()
  collectgarbage(); collectgarbage()
  collectgarbage("stop")
  local m0 = collectgarbage("count")
  local v = work()
  local m1 = collectgarbage("count")
  collectgarbage("restart")
  return math.max(0, m1 - m0), v
end

--- A cell: what one library did on one row.
---   status: "PASS" (did it, correctly), "FAIL" (tried, wrong or errored), "NA" (the library has no such feature), "SKIP" (this run cannot try it)
---   value/unit: a measurement; note: one line for the person
function H.cell(status, note, value, unit)
  return { status = status, note = note, value = value, unit = unit }
end

--- Runs one row for one member. `L` is what the member's `get` returned, `row` the row definition, `impl` this member's function.
--- feature rows: impl(L) returns true (or true, note) when the feature works.
--- speed rows:   impl(L) returns a work function; each call does `row.units` operations and returns a value row.expect accepts.
--- value rows:   impl(L) returns a work function whose result is the number itself (bytes, a count); lower or higher is better (row.lower).
--- memory rows:  same as speed, but the value is KB allocated by one call (lower is better).
function H.run(row, impl, L)
  if impl == nil or impl == false then return H.cell("NA", "no such feature") end
  if row.kind == "feature" then
    local ok, a, b = pcall(impl, L)
    if not ok then return H.cell("FAIL", (tostring(a):gsub("\n.*", ""))) end
    if a == "skip" then return H.cell("SKIP", b or "not tried here") end   -- the adapter says this run cannot try it
    local note = type(b) == "string" and b or nil
    if a then return H.cell("PASS", note or "works") end
    return H.cell("FAIL", note or "wrong answer")
  end
  local ok, work = pcall(impl, L)
  if not ok then return H.cell("FAIL", (tostring(work):gsub("\n.*", ""))) end
  if type(work) ~= "function" then return H.cell("FAIL", "no work function") end
  local okv, val, got
  if row.kind == "value" then
    -- the work's own result is the measurement (a size, a count)
    okv, val = pcall(work)
    if okv and type(val) ~= "number" then okv, val = false, "the work did not return a number" end
    if not okv then return H.cell("FAIL", (tostring(val):gsub("\n.*", ""))) end
    return H.cell("PASS", "measured", val, row.unit)
  end
  if row.kind == "speed" then
    okv, val, got = pcall(H.rate, work, row.units or 1)
  else
    okv, val, got = pcall(H.alloc, work)
  end
  if not okv then return H.cell("FAIL", (tostring(val):gsub("\n.*", ""))) end
  if row.expect then
    local good, why = row.expect(got)
    if not good then return H.cell("FAIL", "wrong answer: " .. tostring(why or got)) end
  end
  return H.cell("PASS", row.kind == "speed" and "measured" or "measured", val, row.unit)
end

return H
