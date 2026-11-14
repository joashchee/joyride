-- Checking things in code. lust is a small test framework (describe, it, expect); batteries.assert is a set of checks that raise an
-- error with a message. Both stop a program that is wrong; only lust also organises and counts tests.

local function raises(f, ...) local ok, e = pcall(f, ...); return not ok, tostring(e) end

return {
  id = "testing", title = "Assertions and tests",
  about = "Check a value, fail with a message that says what was wrong, group checks into tests.",
  members = {
    { id = "lust", short = "lust", lib = "lust" },
    { id = "batteries.assert", short = "assert", lib = "batteries", get = function(b) return b.assert end },
    { id = "luaunit", short = "lunit", lib = "luaunit" },
  },
  rows = {
    { id = "equal", label = "Equal passes", kind = "feature", about = "Checking that 2 + 2 is 4 raises nothing.",
      impl = {
        lust = function(lust) return not raises(function() lust.expect(2 + 2).to.equal(4) end) end,
        ["batteries.assert"] = function(a) return not raises(function() a:equal(2 + 2, 4) end) end,
        luaunit = function(lu) return not raises(function() lu.assertEquals(2 + 2, 4) end) end,
      } },
    { id = "message", label = "Failure says why", kind = "feature", about = "Checking that 1 is 2 raises an error whose text mentions both numbers.",
      impl = {
        lust = function(lust) local bad, e = raises(function() lust.expect(1).to.equal(2) end); return bad and e:find("1", 1, true) and e:find("2", 1, true) end,
        ["batteries.assert"] = function(a) local bad, e = raises(function() a:equal(1, 2) end); return bad and e:find("1", 1, true) and e:find("2", 1, true) end,
        luaunit = function(lu) local bad, e = raises(function() lu.assertEquals(1, 2) end); return bad and e:find("1", 1, true) and e:find("2", 1, true) end,
      } },
    { id = "type", label = "Type check", kind = "feature", about = "A number is not a string, and the error says so.",
      impl = {
        lust = function(lust) local bad, e = raises(function() lust.expect(1).to.be.a("string") end); return bad and e:find("string", 1, true) end,
        ["batteries.assert"] = function(a) local bad, e = raises(function() a:type(1, "string") end); return bad and e:find("string", 1, true) end,
        luaunit = function(lu) local bad, e = raises(function() lu.assertIsString(1) end); return bad and e:find("string", 1, true) end,
      } },
    { id = "deep", label = "Compare tables", kind = "feature", about = "Two tables with the same contents are equal.",
      impl = {
        lust = function(lust) return not raises(function() lust.expect({ a = { 1, 2 } }).to.equal({ a = { 1, 2 } }) end) end,
        ["batteries.assert"] = function(a)
          local bad = raises(function() a:equal({ a = { 1, 2 } }, { a = { 1, 2 } }) end)
          return not bad, "compares tables by identity: equal contents are not enough"
        end,
        luaunit = function(lu) return not raises(function() lu.assertEquals({ a = { 1, 2 } }, { a = { 1, 2 } }) end) end,
      } },
    { id = "suite", label = "Describe / it", kind = "feature", about = "Group checks into named tests and run them: a passing and a failing one are told apart.",
      impl = {
        lust = function(lust)
          local passes, errors = lust.passes, lust.errors
          local oldPrint = print
          print = function() end   -- lust writes its report with print
          local ok = pcall(function()
            lust.describe("benchmark", function()
              lust.it("passes", function() lust.expect(1).to.equal(1) end)
              lust.it("fails", function() lust.expect(1).to.equal(2) end)
            end)
          end)
          print = oldPrint
          lust.level = 0
          local good = ok and lust.passes == passes + 1 and lust.errors == errors + 1
          lust.passes, lust.errors = passes, errors   -- leave its counters as they were
          return good, "its report goes to the console"
        end,
        ["batteries.assert"] = false,
        luaunit = function(lu)
          local runner = lu.LuaUnit.new()
          runner:setOutputType("NIL")   -- no report: the counts are read back
          pcall(runner.runSuiteByInstancesNoCmdLineParsing, runner, { { "TestBench", { testPasses = function() lu.assertEquals(1, 1) end, testFails = function() lu.assertEquals(1, 2) end } } })
          return runner.result.successCount == 1 and runner.result.failureCount == 1, "a suite is a table of test functions; its run and runSuiteByInstances read the program's command line (a game started with --report prints luaunit's usage), so the check uses runSuiteByInstancesNoCmdLineParsing"
        end,
      } },
    { id = "hooks", label = "Before / after", kind = "feature", about = "Setup and teardown run around each test.",
      impl = {
        lust = function(lust)
          local n = 0
          local oldPrint = print; print = function() end
          pcall(function()
            lust.describe("hooks", function()
              lust.before(function() n = n + 1 end)
              lust.it("one", function() end); lust.it("two", function() end)
            end)
          end)
          print = oldPrint; lust.level = 0
          return n == 2
        end,
        ["batteries.assert"] = false,
        luaunit = function(lu)
          local n = 0
          local runner = lu.LuaUnit.new(); runner:setOutputType("NIL")
          pcall(runner.runSuiteByInstancesNoCmdLineParsing, runner, { { "TestHooks", { setUp = function() n = n + 1 end, testOne = function() end, testTwo = function() end } } })
          return n == 2
        end,
      } },
    { id = "speed", label = "10000 checks", kind = "speed", unit = "checks/s", units = 10000, about = "Run a passing equality check 10000 times.",
      expect = function(n) return n == 10000, n end,
      impl = {
        lust = function(lust) return function() local n = 0; for i = 1, 10000 do lust.expect(i).to.equal(i); n = n + 1 end; return n end end,
        ["batteries.assert"] = function(a) return function() local n = 0; for i = 1, 10000 do a:equal(i, i); n = n + 1 end; return n end end,
        luaunit = function(lu) return function() local n = 0; for i = 1, 10000 do lu.assertEquals(i, i); n = n + 1 end; return n end end,
      } },
  },
}
