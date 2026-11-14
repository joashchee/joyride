-- Game states and state machines: menu, play, pause. hump.gamestate is a stack of state tables you switch between;
-- batteries.state_machine holds named states and moves between them by name (or by a state's own return value).

return {
  id = "states", title = "Game states",
  about = "Switch between states with enter and leave callbacks, pass arguments, push and pop, transition from inside a state.",
  members = {
    { id = "hump.gamestate", short = "gstate", lib = "hump", get = function(_, req) return req("hump.gamestate") end },
    { id = "batteries.state_machine", short = "sm", lib = "batteries", get = function(b) return b.state_machine end },
    { id = "roomy", short = "roomy", lib = "roomy" },
  },
  rows = {
    { id = "switch", label = "Enter / leave", kind = "feature", about = "Switching calls the old state's leave, then the new state's enter, and once each.",
      impl = {
        ["hump.gamestate"] = function(GS)
          local log, A, B = {}, GS.new(), GS.new()
          function A:enter() log[#log + 1] = "A+" end; function A:leave() log[#log + 1] = "A-" end
          function B:enter() log[#log + 1] = "B+" end; function B:leave() log[#log + 1] = "B-" end
          GS.switch(A); GS.switch(B)
          return table.concat(log, " ") == "A+ A- B+"
        end,
        ["batteries.state_machine"] = function(SM)
          local log = {}
          local sm = SM({ A = { enter = function() log[#log + 1] = "A+" end, exit = function() log[#log + 1] = "A-" end },
                          B = { enter = function() log[#log + 1] = "B+" end, exit = function() log[#log + 1] = "B-" end } }, "A")
          sm:set_state("B")
          return table.concat(log, " ") == "A+ A- B+"
        end,
        roomy = function(roomy)
          local log, m = {}, roomy.new()
          local function scene(n) return { enter = function() log[#log + 1] = n .. "+" end, leave = function() log[#log + 1] = n .. "-" end } end
          m:enter(scene("A")); m:enter(scene("B"))
          return table.concat(log, " ") == "A+ A- B+"
        end,
      } },
    { id = "args", label = "Pass arguments", kind = "feature", about = "Hand values to the next state when switching to it.",
      impl = {
        ["hump.gamestate"] = function(GS)
          local got, A = nil, GS.new()
          function A:enter(prev, a, b) got = a + b end
          GS.switch(A, 3, 4)
          return got == 7
        end,
        ["batteries.state_machine"] = false,   -- set_state(name) takes no arguments: enter gets the machine itself
        roomy = function(roomy)
          local got, m = nil, roomy.new()
          m:enter({ enter = function(_, _, a, b) got = a + b end }, 3, 4)
          return got == 7
        end,
      } },
    { id = "update", label = "Update current", kind = "feature", about = "Calling update runs only the current state's update.",
      impl = {
        ["hump.gamestate"] = function(GS)
          local a, b, A, B = 0, 0, GS.new(), GS.new()
          function A:update() a = a + 1 end; function B:update() b = b + 1 end
          GS.switch(A); GS.update(0.1); GS.switch(B); GS.update(0.1); GS.update(0.1)
          return a == 1 and b == 2
        end,
        ["batteries.state_machine"] = function(SM)
          local a, b = 0, 0
          local sm = SM({ A = { update = function() a = a + 1 end }, B = { update = function() b = b + 1 end } }, "A")
          sm:update(0.1); sm:set_state("B"); sm:update(0.1); sm:update(0.1)
          return a == 1 and b == 2
        end,
        roomy = function(roomy)
          local a, b, m = 0, 0, roomy.new()
          local A, B = { update = function() a = a + 1 end }, { update = function() b = b + 1 end }
          m:enter(A); m:emit("update"); m:enter(B); m:emit("update"); m:emit("update")
          return a == 1 and b == 2
        end,
      } },
    { id = "stack", label = "Push and pop", kind = "feature", about = "Push a pause state over the game, then pop back: the game's state is current again.",
      impl = {
        ["hump.gamestate"] = function(GS)
          local game, pause = GS.new(), GS.new()
          GS.switch(game); GS.push(pause); local on = GS.current() == pause; GS.pop()
          return on and GS.current() == game
        end,
        ["batteries.state_machine"] = false,
        roomy = function(roomy)
          local seen, m = {}, roomy.new()
          local game = { update = function() seen[#seen + 1] = "game" end }
          local pause = { update = function() seen[#seen + 1] = "pause" end }
          m:enter(game); m:push(pause); m:emit("update"); m:pop(); m:emit("update")
          return table.concat(seen, " ") == "pause game"
        end,
      } },
    { id = "self", label = "Self-transition", kind = "feature", about = "A state's update can ask for the next state itself, and it happens.",
      impl = {
        ["hump.gamestate"] = function(GS)
          local B, A = GS.new(), GS.new()
          function A:update() GS.switch(B) end
          GS.switch(A); GS.update(0.1)
          return GS.current() == B, "by calling switch from inside update"
        end,
        ["batteries.state_machine"] = function(SM)
          local sm = SM({ A = { update = function() return "B" end }, B = {} }, "A")
          sm:update(0.1)
          return sm:in_state("B"), "by returning the next state's name"
        end,
        roomy = function(roomy)
          local m, hit = roomy.new(), false
          local B = { update = function() hit = true end }
          m:enter({ update = function() m:enter(B) end }); m:emit("update"); m:emit("update")
          return hit, "by calling enter from inside update"
        end,
      } },
    { id = "query", label = "Ask where we are", kind = "feature", about = "Find out the current state, by name or by table.",
      impl = {
        ["hump.gamestate"] = function(GS) local A = GS.new(); GS.switch(A); return GS.current() == A end,
        ["batteries.state_machine"] = function(SM) local sm = SM({ A = {}, B = {} }, "A"); return sm:in_state("A") and not sm:in_state("B") and sm:has_state("B") end,
        roomy = false,   -- no way to ask which scene is current (the list is private)
      } },
    { id = "speed", label = "20000 switches", kind = "speed", unit = "switches/s", units = 20000, about = "Switch between two states 20000 times, enter doing a little work.",
      expect = function(n) return n == 20000, n end,
      impl = {
        ["hump.gamestate"] = function(GS)
          local n, A, B = 0, GS.new(), GS.new()
          function A:enter() n = n + 1 end; function B:enter() n = n + 1 end
          return function() n = 0; for k = 1, 20000 do GS.switch(k % 2 == 1 and B or A) end; return n end
        end,
        ["batteries.state_machine"] = function(SM)
          local n = 0
          local sm = SM({ A = { enter = function() n = n + 1 end }, B = { enter = function() n = n + 1 end } }, "A")
          return function() n = 0; for k = 1, 20000 do sm:set_state(k % 2 == 1 and "B" or "A") end; return n end
        end,
        roomy = function(roomy)
          local n, m = 0, roomy.new()
          local A, B = { enter = function() n = n + 1 end }, { enter = function() n = n + 1 end }
          return function() n = 0; for k = 1, 20000 do m:enter(k % 2 == 1 and B or A) end; return n end
        end,
      } },
  },
}
