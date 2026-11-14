-- Events: parts of a game that do not know each other talk through named messages. hump.signal and batteries.pubsub.

return {
  id = "events", title = "Events and messages",
  about = "Register a listener, send a message with arguments, remove the listener, listen to everything.",
  members = {
    { id = "hump.signal", short = "signal", lib = "hump", get = function(_, req) return req("hump.signal") end },
    { id = "batteries.pubsub", short = "pubsub", lib = "batteries", get = function(b) return b.pubsub end },
  },
  rows = {
    { id = "emit", label = "Send + arguments", kind = "feature", about = "Two listeners on \"hit\" both get (3, 4) when it is sent; a different event reaches neither.",
      impl = {
        ["hump.signal"] = function(Signal)
          local s, sum = Signal.new(), 0
          s:register("hit", function(a, b) sum = sum + a + b end); s:register("hit", function(a, b) sum = sum + a * b end)
          s:emit("hit", 3, 4); s:emit("miss", 100, 100)
          return sum == 7 + 12
        end,
        ["batteries.pubsub"] = function(pubsub)
          local p, sum = pubsub(), 0
          p:subscribe("hit", function(a, b) sum = sum + a + b end); p:subscribe("hit", function(a, b) sum = sum + a * b end)
          p:publish("hit", 3, 4); p:publish("miss", 100, 100)
          return sum == 7 + 12
        end,
      } },
    { id = "remove", label = "Remove a listener", kind = "feature", about = "After it is removed, a listener hears nothing more.",
      impl = {
        ["hump.signal"] = function(Signal)
          local s, n = Signal.new(), 0
          local f = s:register("x", function() n = n + 1 end); s:emit("x"); s:remove("x", f); s:emit("x")
          return n == 1
        end,
        ["batteries.pubsub"] = function(pubsub)
          local p, n = pubsub(), 0
          local f = function() n = n + 1 end; p:subscribe("x", f); p:publish("x"); p:unsubscribe("x", f); p:publish("x")
          return n == 1
        end,
      } },
    { id = "once", label = "Listen once", kind = "feature", about = "A listener that hears the first message only.",
      impl = {
        ["hump.signal"] = false,
        ["batteries.pubsub"] = function(pubsub)
          local p, n = pubsub(), 0
          p:subscribe_once("x", function() n = n + 1 end); p:publish("x"); p:publish("x")
          return n == 1
        end,
      } },
    { id = "wild", label = "Wildcards", kind = "feature", about = "Hear every message whose name starts with \"player.\", or all of them.",
      impl = {
        ["hump.signal"] = function(Signal)
          local s, n = Signal.new(), 0
          local f = function() n = n + 1 end
          s:register("player.hit", f); s:register("player.die", f); s:register("enemy.hit", f)
          s:emitPattern("^player%."); return n == 2, "by pattern at send time (it finds the matching names), not at listen time"
        end,
        ["batteries.pubsub"] = function(pubsub)
          local p, n = pubsub(), 0
          p:subscribe("everything", function() n = n + 1 end)
          p:publish("player.hit"); p:publish("enemy.hit"); return n == 2, "one catch-all (\"everything\"), no partial patterns"
        end,
      } },
    { id = "reenter", label = "Change mid-send", kind = "feature", about = "A listener may add or remove listeners while a message is being sent without breaking it.",
      impl = {
        ["hump.signal"] = function(Signal)
          local s, n = Signal.new(), 0
          local late = function() n = n + 100 end
          s:register("x", function() n = n + 1; s:register("x", late) end)
          s:emit("x")
          return n >= 1
        end,
        ["batteries.pubsub"] = function(pubsub)
          local p, n = pubsub(), 0
          local late = function() n = n + 100 end
          p:subscribe("x", function() n = n + 1; p:subscribe("x", late) end)
          p:publish("x")
          return n == 1, "the late listener waits until the send is over"
        end,
      } },
    { id = "speed", label = "10000 sends", kind = "speed", unit = "sends/s", units = 10000, about = "Send a message to five listeners, 10000 times.",
      expect = function(n) return n == 50000, n end,
      impl = {
        ["hump.signal"] = function(Signal) local s, n = Signal.new(), 0
          for _ = 1, 5 do s:register("x", function() n = n + 1 end) end
          return function() n = 0; for _ = 1, 10000 do s:emit("x") end; return n end end,
        ["batteries.pubsub"] = function(pubsub) local p, n = pubsub(), 0
          for _ = 1, 5 do p:subscribe("x", function() n = n + 1 end) end
          return function() n = 0; for _ = 1, 10000 do p:publish("x") end; return n end end,
      } },
  },
}
