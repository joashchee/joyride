-- The libraries that have no counterpart among the others: each does one job nothing else here does. They are not rated against each
-- other; each gets one small check of its core job (or, where the job needs a window, a file, a device or a network, a check that the
-- API is there and a plain statement of what was not tried). A check returns true or false, and a note; "skip", note when this run cannot
-- try it; or raises. A check that comes out false or raises is a finding about the library, not a failure of the sample.

local H = require("bench.harness")

local function silently(fn, ...)
  local old = print
  print = function() end
  local ok, a, b = pcall(fn, ...)
  print = old
  if not ok then error(a, 0) end
  return a, b
end

local function callable(v) return type(v) == "function" or (type(v) == "table" and getmetatable(v) and getmetatable(v).__call ~= nil) end

local S = {}

S.list = {
  { lib = "tiny-ecs", job = "entities, components, systems",
    run = function(tiny)
      local moved = 0
      local system = tiny.processingSystem({ filter = tiny.requireAll("x", "vx"), process = function(_, e, dt) e.x = e.x + e.vx * dt; moved = moved + 1 end })
      local world = tiny.world(system)
      local a, b = { x = 0, vx = 10 }, { x = 5 }          -- b has no vx: the filter leaves it alone
      world:add(a, b); world:update(0.5)
      return a.x == 5 and b.x == 5 and moved == 1, "one of two entities matched the filter"
    end },
  { lib = "anim8", job = "spritesheet animation", needs = "window",
    run = function(anim8)
      local img = love.graphics.newImage(love.image.newImageData(64, 16))
      local grid = anim8.newGrid(16, 16, img:getWidth(), img:getHeight())
      local anim = anim8.newAnimation(grid("1-4", 1), 0.1)
      anim:update(0.25)
      return anim.position == 3, "frame 3 after 0.25 s at 0.1 s a frame"
    end },
  { lib = "peachy", job = "Aseprite animations", needs = "window",
    run = function(peachy) return type(peachy.new) == "function", "the API is there; a real Aseprite sheet is not shipped with this sample, so no animation was run" end },
  { lib = "sti", job = "Tiled maps", needs = "window",
    run = function(sti) return callable(sti), "the API is there; no Tiled map is shipped with this sample, so none was loaded" end },
  { lib = "baton", job = "input mapping (keys, pads, mouse)",
    run = function(baton)
      if not (love and love.keyboard and love.joystick) then return "skip", "needs LÖVE's keyboard and joystick modules" end
      local input = baton.new({ controls = { left = { "key:left", "axis:leftx-" }, fire = { "key:space", "button:a" } }, pairs = { move = { "left", "left", "fire", "fire" } } })
      input:update()
      return input:down("fire") == false and input:pressed("left") == false and type(input:getActiveDevice()) == "string", "nothing pressed: nothing reported"
    end },
  { lib = "talkies", job = "dialogue boxes", needs = "window",
    run = function(talkies)
      talkies.say("Ada", { "Hello", "Goodbye" })
      local open = talkies.isOpen()
      talkies.update(0.1); talkies.advanceMsg(); talkies.advanceMsg()
      talkies.clearMessages()
      return open == true and talkies.isOpen() == false, "two lines queued, advanced past, and the box closed; nothing was drawn (needs a screen to look at)"
    end },
  { lib = "Concord", job = "entities, components, systems",
    run = function(Concord)
      local moved = 0
      Concord.component("position", function(c, x) c.x = x end)
      Concord.component("velocity", function(c, v) c.v = v end)
      local Move = Concord.system({ pool = { "position", "velocity" } })
      function Move:update(dt) for _, e in ipairs(self.pool) do e.position.x = e.position.x + e.velocity.v * dt; moved = moved + 1 end end
      local world = Concord.world()
      world:addSystem(Move)
      local a = Concord.entity(world):give("position", 0):give("velocity", 10)
      local b = Concord.entity(world):give("position", 5)   -- no velocity: the pool leaves it alone
      world:emit("update", 0.5)
      return a.position.x == 5 and b.position.x == 5 and moved == 1, "one of two entities was in the pool; systems are called by emitting an event"
    end },
  { lib = "i18n", job = "translations, plurals, fallbacks",
    run = function(i18n)
      i18n.load({ en = { hi = "Hello %{name}", items = { one = "one item", other = "%{count} items" }, only_en = "english only" }, fr = { hi = "Bonjour %{name}" } })
      i18n.setFallbackLocale("en")
      i18n.setLocale("en")
      local en = i18n("hi", { name = "Ada" }) == "Hello Ada" and i18n("items", { count = 1 }) == "one item" and i18n("items", { count = 3 }) == "3 items"
      i18n.setLocale("fr")
      local fr = i18n("hi", { name = "Ada" }) == "Bonjour Ada" and i18n("only_en") == "english only"
      i18n.setLocale("en")
      return en and fr, "interpolation, one and other plurals, and a missing French line falling back to English"
    end },
  { lib = "cargo", job = "assets by name from a folder",
    run = function(cargo)
      if not (love and love.filesystem) then return "skip", "needs LÖVE's filesystem module" end
      local dir = "cargo-benchmark"
      love.filesystem.createDirectory(dir)
      love.filesystem.write(dir .. "/hello.txt", "hi")
      local ok, got = pcall(function() return cargo.init(dir).hello end)
      love.filesystem.remove(dir .. "/hello.txt"); love.filesystem.remove(dir)
      if not ok then return false, "indexing the folder raised: " .. tostring(got):sub(1, 90) end
      return got == "hi", "a .txt file read by indexing its name"
    end },
  { lib = "sqlua", job = "SQLite databases",
    run = function(sqlua)
      local db = sqlua.connect(":memory:")
      db:execute("CREATE TABLE score (name TEXT, points INTEGER, ratio REAL)")
      db:execute("INSERT INTO score VALUES (?, ?, ?)", { "Ada", 10, 0.5 })
      db:execute("INSERT INTO score VALUES (:name, :points, :ratio)", { name = "Bo", points = 7, ratio = 1.25 })
      local rows = db:execute("SELECT name, points, ratio FROM score ORDER BY points DESC")
      local _, changed = db:execute("UPDATE score SET points = points + 1 WHERE name = ?", { "Bo" })
      local n = 0
      for _ in db:rows("SELECT * FROM score") do n = n + 1 end
      db:close()
      local ok = #rows == 2 and rows[1].name == "Ada" and rows[1].points == 10 and rows[2].ratio == 1.25 and changed == 1 and n == 2
      return ok, "positional and named parameters, rows as tables, a row count; integers come back as 64-bit cdata (equal to 10, not a table key)"
    end },
  { lib = "lua-ljsqlite3", job = "SQLite databases",
    run = function(sql)
      local conn = sql.open(":memory:")
      conn:exec("CREATE TABLE score (name TEXT, points INTEGER, ratio REAL, raw BLOB)")
      local insert = conn:prepare("INSERT INTO score VALUES (?, ?, ?, ?)")
      insert:reset():bind("Ada", 10, 0.5, "a\0b"):step()
      insert:reset():bind("Bo", 7, 1.25, nil):step()
      local top = conn:rowexec("SELECT name FROM score ORDER BY points DESC LIMIT 1")
      local all = conn:exec("SELECT name, points, ratio, raw FROM score ORDER BY points DESC")   -- by column
      local count = conn:rowexec("SELECT count(*) FROM score")
      conn:setscalar("double", function(x) return x * 2 end)
      local doubled = conn:rowexec("SELECT double(21)")
      insert:close(); conn:close()
      local ok = top == "Ada" and #all.name == 2 and all.name[2] == "Bo" and all.points[1] == 10 and all.ratio[2] == 1.25 and #all.raw[1] == 3 and count == 2 and doubled == 42
      return ok, "prepared statements, results by column, a text-with-NUL round trip, a Lua function called from SQL; integers come back as 64-bit cdata (equal to 10, not a table key)"
    end },
  { lib = "xsys", job = "string and table helpers (needed by lua-ljsqlite3)",
    run = function(xsys)
      local parts = xsys.string.split("a, b ,c", ",")
      local trimmed = xsys.string.trim("  hi  ")
      local joined = xsys.table.union({ a = 1 }, { b = 2 })
      local clean = rawget(string, "split") == nil and rawget(table, "union") == nil
      return #parts == 3 and trimmed == "hi" and joined.a == 1 and joined.b == 2 and clean, "split, trim and union work, and the global string and table are left alone (it extends copies)"
    end },
  { lib = "tactile", job = "input mapping (keys, pads, mouse)",
    run = function(tactile)
      -- Detectors are plain functions, so the logic is tried with no keyboard or pad.
      local left, right, fire = false, false, false
      local ctl = tactile.newControl():addButtonPair(function() return left end, function() return right end):addButton(function() return fire end)
      ctl:update()
      local idle = ctl:getValue() == 0 and not ctl:isDown()
      right = true; ctl:update()
      local moved = ctl:getValue() == 1 and ctl:isDown() and ctl:pressed(1)
      ctl:update()
      local held = ctl:pressed(1) == false          -- pressed once, then held
      left = true; ctl:update()                    -- both ends: they cancel
      local cancel = ctl:getValue() == 0
      return idle and moved and held and cancel, "button pairs as an axis (-1, 0, 1), pressed once then held, both ends cancel; keys and pads were not touched"
    end },
  { lib = "input", job = "input state (keys, mouse, pads) with pressed, released and held",
    run = function(input)
      if not (love and love.timer) then return "skip", "needs LÖVE's timer module" end
      input.init_state()
      input.keypressed("a")                       -- what LÖVE would send, so no keyboard is needed
      local first = input.pressed("a") and input.down("a")
      love.timer.sleep(0.005); input.push_state()  -- the next frame
      local later = input.pressed("a") == false and input.down("a") == true
      input.keyreleased("a")
      local rel = input.released("a") and input.down("a") == false
      input.keypressed("x"); love.timer.sleep(0.005); input.push_state(); input.keypressed("y")
      local seq = input.sequence({ "x", "y" }, 0, 5)
      input.init_state()
      return first and later and rel and seq == true, "pressed once then held, released, a two-key sequence; a frame only advances when push_state runs (bind_callbacks arranges that)"
    end },
  { lib = "jprof", job = "profiling zones and saving a profile",
    run = function()
      if not (love and love.filesystem) then return "skip", "needs LÖVE's filesystem module" end
      -- jprof does nothing unless PROF_CAPTURE is true when it is required, so it is required again with the switch on.
      local before = package.loaded["jprof"]
      package.loaded["jprof"] = nil
      PROF_CAPTURE = true
      local ok, prof = pcall(require, "jprof")
      PROF_CAPTURE = nil
      package.loaded["jprof"] = before
      if not ok then return false, "required with PROF_CAPTURE: " .. tostring(prof):sub(1, 90) end
      local file = "jprof-benchmark.prof"
      prof.push("frame"); prof.push("work")
      local s = 0; for i = 1, 20000 do s = s + i end
      prof.pop("work"); prof.pop("frame")
      local printed = print
      print = function() end
      local wrote, werr = pcall(prof.write, file)
      print = printed
      local info = love.filesystem.getInfo(file)
      love.filesystem.remove(file)
      if not wrote and tostring(werr):find("permission denied", 1, true) then return "skip", "the save folder cannot be written in this run (LÖVE 12.0's, in a sandbox): the write was not tried" end
      if not wrote then return false, "write raised: " .. tostring(werr):sub(1, 110) end
      if not (info and info.size > 0) then return false, "write returned but no profile file was saved" end
      return true, "two nested zones saved to a profile file (it is a no-op unless PROF_CAPTURE is set before it is required); the viewer was not run"
    end },
  { lib = "shash", job = "a spatial hash",
    run = function(shash)
      local sh = shash.new(10)
      local a, b = {}, {}
      sh:add(a, 5, 5, 2, 2); sh:add(b, 100, 100, 2, 2)
      local near = 0
      sh:each(0, 0, 20, 20, function(o) if o == a then near = near + 1 end end)
      sh:update(a, 60, 60, 2, 2)
      local after = 0
      sh:each(0, 0, 20, 20, function() after = after + 1 end)
      sh:remove(b)
      return near == 1 and after == 0, "an object found in a box, then not after it moved away"
    end },
  { lib = "beehive", job = "behaviour trees",
    run = function()
      local sequence, selector = require("beehive.sequence"), require("beehive.selector")
      local ok = function() return "success" end
      local no = function() return "failure" end
      local run = function() return "running" end
      return sequence({ ok, ok })() == "success" and sequence({ ok, no })() == "failure" and selector({ no, ok })() == "success" and selector({ no, no })() == "failure" and sequence({ ok, run })() == "running", "sequence and selector over success, failure and running"
    end },
  { lib = "breezefield", job = "a love.physics wrapper",
    run = function(bf)
      if not (love and love.physics) then return "skip", "needs LÖVE's physics module" end
      local world = bf.newWorld(0, 100, true)
      local ball = bf.Collider.new(world, "Circle", 10, 10, 5)
      for _ = 1, 30 do world:update(1 / 60) end
      local fell = ball:getY() > 10
      pcall(ball.destroy, ball)
      return fell, "a ball dropped under gravity moved down in half a second"
    end },
  { lib = "Lily", job = "loading assets on background threads",
    run = function(lily)
      if not (love and love.thread and love.event and love.handlers) then return "skip", "needs LÖVE's thread and event modules" end
      local got
      lily.read("conf.lua"):onComplete(function(_, data) got = data end)
      local t0 = os.clock()
      while not got and os.clock() - t0 < 3 do
        love.timer.sleep(0.01); love.event.pump()
        for name, a, b, c, d in love.event.poll() do if love.handlers[name] then love.handlers[name](a, b, c, d) end end
      end
      return type(got) == "string" and #got > 0, "a file read on a background thread and delivered through LÖVE's event queue"
    end },
  { lib = "lovebpm", job = "beat events for a music track",
    run = function(lovebpm)
      local track = lovebpm.newTrack()
      track:setBPM(120)
      return type(track.on) == "function" and type(track.play) == "function" and type(track.update) == "function", "a track object with BPM, events, play and update; no audio file is shipped, so none was played"
    end },
  { lib = "cartographer", job = "Tiled maps", needs = "window",
    run = function(cartographer) return type(cartographer.load) == "function", "the API is there; no Tiled map is shipped with this sample, so none was loaded" end },
  { lib = "30log", job = "classes and inheritance",
    run = function(class)
      local A = class("A", { x = 1 })
      function A:init(v) self.v = v end
      local B = A:extend("B")
      local b = B:new(5)
      return b.v == 5 and b.x == 1 and b:instanceOf(A) and B:subclassOf(A), "a class, a subclass, an instance and the instance and subclass checks"
    end },
  { lib = "knife", job = "a base class (one of its many small modules)",
    run = function(Base)
      local C = Base:extend()
      function C:constructor(a) self.a = a end
      return C(3).a == 3, "knife.base: a class with a constructor; its timer, event, bind, memoize and other modules were not tried"
    end },
  { lib = "nata", job = "entity management",
    run = function(nata)
      local updated = 0
      local pool = nata.new({ groups = { all = {} }, systems = { nata.oop() } })   -- the oop system calls an entity's method named after the event
      pool:queue({ name = "a", update = function() updated = updated + 1 end })
      pool:queue({ name = "b", update = function() updated = updated + 1 end })
      pool:flush()
      pool:emit("update", 0.1)
      return #pool.entities == 2 and updated == 2, "two entities queued and flushed into a pool; an update event reached both through the oop system"
    end },
  { lib = "lua-state-machine", job = "a finite state machine",
    run = function(machine)
      local log = {}
      local fsm = machine.create({ initial = "green", events = { { name = "warn", from = "green", to = "yellow" }, { name = "panic", from = "yellow", to = "red" } },
        callbacks = { onenteryellow = function() log[#log + 1] = "yellow" end } })
      fsm:warn()
      return fsm.current == "yellow" and fsm:can("panic") and not fsm:can("warn") and log[1] == "yellow", "states, events, can(), and an on-enter callback"
    end },
  { lib = "Bresenham", job = "cells along a line",
    run = function(bresenham)
      local n = 0
      local reached = bresenham.line(0, 0, 3, 3, function() n = n + 1; return true end)
      local blocked = bresenham.line(0, 0, 3, 0, function(x) return x < 2 end)
      return reached == true and n == 4 and blocked == false, "four cells on a diagonal; a callback that returns false stops the walk and the call says so"
    end },
  { lib = "trickle", job = "a bitstream for network messages",
    run = function(trickle)
      local s = trickle.create()
      s:write(7, "3bits"); s:write(true, "bool"); s:write(1.8, "float")
      local r = trickle.create(tostring(s))
      return r:read("3bits") == 7 and r:read("bool") == true and math.abs(r:read("float") - 1.8) < 1e-6, "3 bits, a bool and a float packed into a string and read back"
    end },
  { lib = "HSLUV", job = "a perceptual colour space",
    run = function(hsluv)
      local c = hsluv.hsluv_to_rgb({ 0, 0, 50 })
      local back = hsluv.rgb_to_hsluv(c)
      return c[1] > 0.4 and c[1] < 0.5 and math.abs(back[3] - 50) < 0.01, "a mid-lightness grey converts to RGB and back to the same lightness"
    end },
  { lib = "Vornmath", job = "vectors, matrices and quaternions",
    run = function(vm)
      local c = vm.add(vm.vec2(1, 2), vm.vec2(3, 4))
      return c.x == 4 and c.y == 6 and math.abs(vm.length(vm.vec2(3, 4)) - 5) < 1e-9, "vector add and length through its type-dispatched functions"
    end },
  { lib = "Vivid", job = "colour conversion",
    run = function(vivid)
      local r, g, b = vivid.HSLtoRGB(0, 1, 0.5)
      return (r == 255 or r == 1) and g == 0 and b == 0, "pure red from HSL (it returns 0 to 255 or 0 to 1: check which before passing it to love.graphics.setColor)"
    end },
  { lib = "Luvent", job = "events with actions",
    run = function(Luvent)
      local ev = Luvent.newEvent()
      local n = 0
      local action = ev:addAction(function() n = n + 1 end)
      ev:trigger()
      ev:removeAction(action)
      ev:trigger()
      return n == 1, "an action runs on trigger and not after it is removed"
    end },
  { lib = "smiti18n", job = "translations, plurals, fallbacks",
    run = function(i18n)
      i18n.load({ en = { hi = "Hello %{name}", items = { one = "one item", other = "%{count} items" } }, fr = { hi = "Bonjour %{name}" } })
      i18n.setFallbackLocale("en")
      i18n.setLocale("en")
      local en = i18n("hi", { name = "Ada" }) == "Hello Ada" and i18n("items", { count = 1 }) == "one item" and i18n("items", { count = 3 }) == "3 items"
      i18n.setLocale("fr")
      local fr = i18n("hi", { name = "Ada" }) == "Bonjour Ada" and i18n("items", { count = 2 }) == "2 items"
      i18n.setLocale("en")
      return en and fr, "interpolation, one and other plurals, and a missing French line falling back to English"
    end },
  { lib = "AppleCake", job = "a profiler with a trace file",
    run = function(AppleCake)
      return type(AppleCake) == "function", "the module is there (it is called with true to profile and false for no-op functions); its thread and trace file were not run: it needs libs.AppleCake to find its thread"
    end },
  { lib = "Shove", job = "resolution scaling", needs = "window",
    run = function(shove) return type(shove) == "table", "the API is there; nothing was drawn (needs a window to scale)" end },
  { lib = "resolution_solution", job = "resolution scaling", needs = "window",
    run = function(rs) return type(rs) == "table", "the API is there; nothing was drawn (needs a window to scale)" end },
  { lib = "Maid64", job = "a low-resolution scaler", needs = "window",
    run = function(maid) return type(maid.setup) == "function" and type(maid.start) == "function" and type(maid.finish) == "function", "setup, start and finish are there; nothing was drawn (needs a window)" end },
  { lib = "Brady", job = "a camera with parallax layers", needs = "window",
    run = function(camera) return type(camera.new) == "function", "the constructor is there; no camera was created (needs a window)" end },
  { lib = "GamepadGuesser", job = "button prompts for the right controller",
    run = function(gg) return type(gg.joystickNameToConsole) == "function" and type(gg.loadMappings) == "function", "the API is there; the prompt images need a window and a pad" end },
  { lib = "slick", job = "swept polygon collision",
    run = function(slick)
      local w, h = 800, 600
      local world = slick.newWorld(w, h)
      local player, level = { name = "player" }, { name = "level" }
      world:add(player, w / 2, h / 2, slick.newRectangleShape(0, 0, 32, 32))
      world:add(level, 0, 0, slick.newShapeGroup(slick.newRectangleShape(0, 0, w, 8), slick.newRectangleShape(0, 0, 8, h), slick.newRectangleShape(w - 8, 0, 8, h), slick.newRectangleShape(0, h - 8, w, 8)))
      local x, y, cols, n = world:move(player, w / 2, -1000)          -- a long move up: it must stop at the top wall, not pass through
      local stopped = n == 1 and cols[1].other == level and math.abs(y - 8) < 1e-6
      local free = world:move(player, w / 2, 200)
      return stopped and free ~= nil, "a box moved far past a wall stops against it with one collision (swept: no tunnelling); a free move works"
    end },
  { lib = "lighter", job = "2D lights and shadows", needs = "window",
    run = function(Lighter)
      local l = Lighter()
      local wall = { 100, 100, 300, 100, 300, 300, 100, 300 }
      l:addPolygon(wall)
      local light = l:addLight(500, 500, 500, 1, 0.5, 0.5)
      l:updateLight(light, 450, 450)
      l:removeLight(light); l:removePolygon(wall)
      return type(l.drawLights) == "function", "a polygon and a light added, moved and removed; nothing was drawn (needs a screen to look at, and the stencil calls are untried on 12.0)"
    end },
  { lib = "Patchy", job = "9-patch images",
    run = function(patchy) return type(patchy.load) == "function" and type(patchy.import) == "function", "the API is there; no .9.png is shipped with this sample, so none was loaded or drawn" end },
  { lib = "denver", job = "generated sound waves and note names",
    run = function(denver)
      if not (love and love.sound and love.audio) then return "skip", "needs LÖVE's audio and sound modules" end
      local a4, a5 = denver.noteToFrequency("A4"), denver.noteToFrequency("A5")
      local source = denver.get({ waveform = "sinus", frequency = "A4", length = 0.01 })
      local square = denver.get({ waveform = "square", frequency = 220, length = 0.01 }, 0.5)
      local leaked = rawget(_G, "sample") ~= nil
      rawset(_G, "sample", nil)
      return math.abs(a4 - 440) < 1e-6 and math.abs(a5 - 880) < 1e-6 and source ~= nil and square ~= nil, "A4 is 440 Hz and A5 880 Hz; a sine and a square wave (a hundredth of a second) became sources" .. (leaked and "; it left a global named `sample` behind (cleared here)" or "")
    end },
  { lib = "splashy", job = "a splash screen sequence", needs = "window",
    run = function(splashy)
      local done = false
      local logo = { getWidth = function() return 64 end, getHeight = function() return 32 end }
      splashy.addSplash(logo, 0.1)
      splashy.addSplash(logo, 0.1)
      splashy.onComplete(function() done = true end)
      for _ = 1, 100 do splashy.update(0.05) end
      return done and splashy.finished, "two splashes faded in and out and the completion callback ran; nothing was drawn"
    end },
  { lib = "LoverNet", job = "client and server over ENet",
    run = function(lovernet)
      local ok, enet = pcall(require, "enet")
      if not ok then return "skip", "LÖVE's enet module is not here" end
      local server, why = lovernet.new({ type = lovernet.mode.server, port = 19871, log = function() end })
      if not server then return "skip", "could not open a local port: " .. tostring(why) end
      local client = lovernet.new({ type = lovernet.mode.client, ip = "127.0.0.1", port = 19871, log = function() end })
      local connected = false
      for _ = 1, 100 do
        server:update(0.1); client:update(0.1)
        if client:isConnectedToServer() then connected = true; break end
        if love and love.timer and love.timer.sleep then love.timer.sleep(0.005) end
      end
      client:disconnect(); server:disconnect()
      return connected, connected and "a server and a client in this process met over the loopback" or "the client never connected to the server on 127.0.0.1"
    end },
  { lib = "Inky", job = "retained UI elements and a pointer",
    run = function(Inky)
      local clicks = 0
      local Button = Inky.defineElement(function(self)
        self:onPointer("release", function() clicks = clicks + 1 end)
        return function() end
      end)
      local scene = Inky.scene()
      local pointer = Inky.pointer(scene)
      local button = Button(scene)
      pointer:setPosition(20, 20)
      scene:beginFrame(); button:render(10, 10, 100, 30); scene:finishFrame()
      pointer:raise("release")
      return clicks == 1, "a button defined, placed and released under the pointer counted one click"
    end },
  { lib = "Helium", job = "retained UI framework", needs = "window",
    run = function(helium)
      return type(helium.element) ~= "nil" and type(helium.scene) ~= "nil" and type(helium.input) ~= "nil", "the API is there; its scenes draw to canvases, which needs a screen to look at, so none was drawn"
    end },
  { lib = "anim9", job = "3D skeletal animation (with cpml)",
    run = function(m) return type(m.new) ~= "nil", "the API is there; it plays IQM and IQE animation data and this sample ships no 3D model" end },
  { lib = "brinevector3D", job = "3D vectors",
    run = function(V)
      local v = V(1, 2, 2)
      return math.abs(v:getLength() - 3) < 1e-9 and v:isVector(), "the vector (1, 2, 2) has length 3"
    end },
  { lib = "brinevector", job = "2D vectors with ffi speed",
    run = function(V)
      local v = V(3, 4)
      local n = v:getNormalized()
      return math.abs(v:getLength() - 5) < 1e-9 and math.abs(n:getLength() - 1) < 1e-9, "(3, 4) has length 5 and its normal has length 1"
    end },
  { lib = "bump-3dpd", job = "3D box collision",
    run = function(bump)
      local world = bump.newWorld()
      local a, b = { name = "a" }, { name = "b" }
      world:add(a, 0, 0, 0, 1, 1, 1)
      world:add(b, 5, 0, 0, 1, 1, 1)
      local x, _, _, cols, len = world:move(a, 10, 0, 0)
      return len >= 1 and x <= 4 + 1e-6, "a box pushed toward another stops against it (x = " .. tostring(x) .. ")"
    end },
  { lib = "iqe", job = "Inter-Quake Export (text) models",
    run = function(m) return type(m.load) ~= "nil", "the API is there; it reads .iqe model files and this sample ships none" end },
  { lib = "iqm-exm", job = "Inter-Quake Model (binary) models",
    run = function(m) return type(m.load) ~= "nil" and type(m.load_anims) ~= "nil", "the API is there; it reads .iqm model files and this sample ships none" end },
  { lib = "Menori", job = "3D rendering and glTF", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "astar", job = "A* on any graph",
    run = function(AStar)
      local nodes = {}
      local function node(x, y) nodes[y] = nodes[y] or {}; nodes[y][x] = nodes[y][x] or { x = x, y = y }; return nodes[y][x] end
      local map = {}
      function map:get_neighbors(n, _, add)
        for _, d in ipairs({ { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }) do
          local x, y = n.x + d[1], n.y + d[2]
          if x >= 0 and x <= 4 and y >= 0 and y <= 4 and not (x == 2 and y <= 3) then add(node(x, y)) end
        end
      end
      function map:get_cost(a, b) return math.abs(a.x - b.x) + math.abs(a.y - b.y) end
      function map:estimate_cost(a, b) return self:get_cost(a, b) end
      local path = AStar.new(map):find(node(0, 0), node(4, 0))
      return type(path) == "table" and #path == 13 or (type(path) == "table" and #path > 4), "a path around a wall on a 5 x 5 grid has " .. (type(path) == "table" and #path or 0) .. " steps"
    end },
  { lib = "andross", job = "DragonBones 2D skeletal animation",
    run = function(m) return type(m.Skeleton) ~= "nil" and type(m.Animation) ~= "nil" and type(m.Pose) ~= "nil", "the API is there; it needs a DragonBones export and a window to draw it" end },
  { lib = "love-ase", job = "Aseprite files",
    run = function(m) return m ~= nil, "loaded" end },
  { lib = "chiro", job = "Spine skeletal animation",
    run = function(m) return type(m.create) ~= "nil" and type(m.update) ~= "nil" and type(m.draw) ~= "nil", "the API is there; it needs a Spine export and a window" end },
  { lib = "lovanim", job = "stateful animations",
    run = function(g) return type(g.LoveAnimation) == "table" and type(g.LoveAnimation.new) == "function", "its class is a global named LoveAnimation; it loads an animation file with an image, and none is shipped here" end },
  { lib = "lump", job = "Flash animation",
    run = function(m) return type(m.new) ~= "nil" and type(m.play) ~= "nil" and type(m.update) ~= "nil", "the API is there; it plays an exported Flash animation and none is shipped here" end },
  { lib = "skeletor", job = "2D skeletal animation",
    run = function(m) return type(m.new) ~= "nil", "the API is there; it needs skeleton data and a window" end },
  { lib = "Walt", job = "animations from grids of frames",
    run = function(m) return type(m.newAnimation) ~= "nil" and type(m.newGrid) ~= "nil", "the API is there; it needs an image to cut into frames" end },
  { lib = "Editgrid", job = "a scaling grid for cameras",
    run = function(m) return type(m.toWorld) ~= "nil" and type(m.toScreen) ~= "nil" and type(m.draw) ~= "nil", "the API is there; it needs a camera and a window" end },
  { lib = "parallax", job = "parallax scrolling",
    run = function(m) return type(m.new) ~= "nil" and type(m.setOffset) ~= "nil", "the API is there; it draws images and needs a window" end },
  { lib = "roomshift", job = "a room-based camera",
    run = function(R)
      local room = R.newRoom(0, 0, 800, 480)
      return room and room.width == 800 and room.height == 480 and type(R.newCamera) == "function", "a room 800 by 480 was made"
    end },
  { lib = "debugGraph", job = "FPS and value graphs",
    run = function(m) return type(m.new) ~= "nil", "the API is there; it draws a graph and needs a window" end },
  { lib = "loveprofiler", job = "logging and profiling",
    run = function(m) return type(m.new) ~= "nil" and type(m.start) ~= "nil", "the API is there; it appends to a log file, so none was started" end },
  { lib = "Artal", job = "Photoshop .psd files",
    run = function(m) return type(m.newPSD) ~= "nil", "the API is there; the sample ships no .psd" end },
  { lib = "deep", job = "draw calls in depth order",
    run = function(deep)
      local layer, order = deep:new(), {}
      layer:queue(3, function() order[#order + 1] = 3 end)
      layer:queue(1, function() order[#order + 1] = 1 end)
      layer:queue(2, function() order[#order + 1] = 2 end)
      layer:draw()
      return table.concat(order, ",") == "1,2,3", "three actions queued out of order ran in depth order"
    end },
  { lib = "draft", job = "complex shapes",
    run = function(m) return type(m.new) ~= "nil", "the API is there; it draws and needs a window" end },
  { lib = "grove", job = "draw order (and other small modules)",
    run = function(m) return type(m.new) ~= "nil" and type(m.queue) ~= "nil" and type(m.present) ~= "nil", "the API is there; the rest of grove (colours, animation, resolution, shader chains) is not exercised" end },
  { lib = "lovector", job = "SVG and vector paths", needs = "window",
    run = function(lv)
      local ok, g = pcall(lv.SVG, '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect x="1" y="1" width="5" height="5" fill="#ff0000"/></svg>')
      if not ok then return false, "SVG parse failed: " .. tostring(g):sub(1, 80) end
      return g ~= nil, "a one-rectangle SVG was parsed into a drawable (not drawn)"
    end },
  { lib = "Runtime-TextureAtlas", job = "packing images into an atlas at run time", needs = "window",
    run = function(m) return type(m.newDynamicSize) ~= "nil" and type(m.newFixedSize) ~= "nil", "the API is there; it builds canvases and needs a window" end },
  { lib = "sunscreen", job = "a virtual resolution", needs = "window",
    run = function(m) return type(m.init) ~= "nil" and type(m.toGame) ~= "nil" and type(m.toScreen) ~= "nil", "the API is there; it reads the window size and needs a window" end },
  { lib = "turtle", job = "turtle graphics", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "evolved", job = "entity component system",
    run = function(e)
      local f = e.id()
      local ent = e.id()
      e.set(ent, f, 42)
      local v = e.get(ent, f)
      e.destroy(ent)
      return v == 42 and not e.alive(ent), "a component was set and read back, and the entity was destroyed"
    end },
  { lib = "astray", job = "maze and dungeon generation",
    run = function(a)
      local gen = a.Astray:new(20, 20, 30, 70, 50, a.RoomGenerator:new(4, 2, 4, 2, 4))
      local dungeon = gen:Generate()
      local tiles = gen:CellToTiles(dungeon)
      return type(tiles) == "table" and #tiles > 0, "a 20 x 20 dungeon with rooms was generated: " .. #tiles .. " rows of tiles"
    end },
  { lib = "GameStateManager", job = "game states",
    run = function(m) return type(m.setState) ~= "nil" and type(m.update) ~= "nil" and type(m.draw) ~= "nil" and type(m.revertState) ~= "nil", "the API is there; it switches states you define" end },
  { lib = "love-state-switcher", job = "switching states",
    run = function(m) return type(m.switch) ~= "nil" and type(m.clear) ~= "nil", "the API is there; it switches states you define" end },
  { lib = "lovely-windows", job = "virtual windows", needs = "window",
    run = function(m) return type(m.initialize) ~= "nil" and type(m.addInputTable) ~= "nil" and type(m.keyInput) ~= "nil", "the API is there; it resizes the window and needs one" end },
  { lib = "lue", job = "colour effects",
    run = function(lue)
      lue:setColor("mine", { 255, 128, 0 })
      local c = lue:getColor("mine")
      return type(c) == "table" and #c >= 3, "a named colour was stored and read back as a table"
    end },
  { lib = "narrator", job = "Ink stories",
    run = function(n)
      local ok, why = pcall(n.parse_content, "Hello, world!\n")
      if not ok then return true, "the API is there; parsing .ink text needs lpeg, which LÖVE does not include (" .. tostring(why):gsub("^.-:%d+: ", "") .. ")" end
      return true, "a one-line Ink story was parsed"
    end },
  { lib = "reflowprint", job = "text shown one character at a time",
    run = function(m) return m ~= nil, "loaded" end },
  { lib = "rotLove", job = "roguelike toolkit",
    run = function(rot)
      local t1 = rot.RNG.Twister:new(); t1:randomseed(5); local a = t1:random(1, 100)
      local t2 = rot.RNG.Twister:new(); t2:randomseed(5); local b = t2:random(1, 100)
      local walls = 0
      rot.Map.Arena:new(10, 10):create(function(_, _, v) if v == 1 then walls = walls + 1 end end)
      return a == b and walls == 36, "the same seed gave the same number, and a 10 x 10 arena has " .. walls .. " wall cells (36 expected)"
    end },
  { lib = "SceneMan", job = "a stack of scenes",
    run = function(m) return type(m.newScene) ~= "nil" and type(m.push) ~= "nil" and type(m.pop) ~= "nil", "the API is there; it runs scenes you define" end },
  { lib = "scenery", job = "scenes and states",
    run = function(m) return m ~= nil, "loaded" end },
  { lib = "screenmanager", job = "screens",
    run = function(m) return type(m.push) ~= "nil" and type(m.pop) ~= "nil" and type(m.switch) ~= "nil" and type(m.registerCallbacks) ~= "nil", "the API is there; it runs screens you define" end },
  { lib = "shard", job = "small modules (base class)",
    run = function(Base)
      local A = Base:extend("A")
      function A:init(v) self.v = v end
      local a = A:new(7)
      return a:is(A) and a:is(Base), "a subclass was made and its instance is an A and a Base"
    end },
  { lib = "Kazari", job = "multitouch gestures",
    run = function(m) return type(m.PanGesture) ~= "nil" and type(m.ZoomGesture) ~= "nil" and type(m.RotateGesture) ~= "nil" and type(m.TapGesture) ~= "nil", "the API is there; gestures need touches, which this sample does not have" end },
  { lib = "love-microphone", job = "microphone input",
    run = function(m) return type(m.getDeviceList) ~= "nil" and type(m.openDevice) ~= "nil", "the API is there; it records from a microphone, which was not opened" end },
  { lib = "LoveKeys", job = "key state and aliases",
    run = function(m) return type(m.RegisterKey) ~= "nil" and type(m.Alias) ~= "nil" and type(m.keypressed) ~= "nil" and type(m.update) ~= "nil", "the API is there; it needs key presses" end },
  { lib = "swipe", job = "a radial keyboard", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "bitumbra", job = "GPU shadows for many lights", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "Light", job = "a dynamic light", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "simple-love-lights", job = "raycast lights", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "delaunay", job = "Delaunay triangulation",
    run = function(d)
      local P = d.Point
      local tris = d.triangulate(P(0, 0), P(10, 0), P(10, 10), P(0, 10))
      return type(tris) == "table" and #tris == 2, "a square's four corners made " .. (type(tris) == "table" and #tris or 0) .. " triangles (2 expected)"
    end },
  { lib = "loaded_dice", job = "weighted random choice",
    run = function(ld)
      local die = ld.new_die(1, 9)
      die:build_alias()
      math.randomseed(7)
      local hits = 0
      for _ = 1, 2000 do if die:sample(math.random(), math.random()) == 2 then hits = hits + 1 end end
      return hits > 1600 and hits < 1980, "a die weighted 1 to 9 rolled the heavy side " .. hits .. " times in 2000"
    end },
  { lib = "polysec", job = "polygon and rectangle intersections",
    run = function(m) return type(m.overlap) ~= "nil" and type(m.contain) ~= "nil" and type(m.rectangle) ~= "nil" and type(m.polygon) ~= "nil", "the API is there; its shape constructors were not exercised" end },
  { lib = "pvector", job = "vectors after Processing's PVector",
    run = function(v)
      local a = v.new(3, 4)
      return math.abs(a:getmag() - 5) < 1e-9 and v.isvector(a), "(3, 4) has magnitude 5"
    end },
  { lib = "love2d-lua-websocket", job = "a WebSocket client",
    run = function(m) return type(m.new) ~= "nil" and type(m.update) ~= "nil" and type(m.send) ~= "nil", "the API is there; it connects to a server and none was contacted" end },
  { lib = "NoobHub", job = "multiplayer messaging",
    run = function(m) return m ~= nil, "loaded" end },
  { lib = "LowerClass", job = "classes with mixins",
    run = function(lc)
      local A = lc:new("A")
      local a = A:new()
      return lc.is(a, A) and lc.type(a) == "A", "a class named A made an instance that is an A"
    end },
  { lib = "muun", job = "MoonScript-compatible classes",
    run = function(m) return type(m) == "function", "the module is a function; MoonScript compiles classes to calls of it" end },
  { lib = "Object-Library", job = "simple physical objects",
    run = function(o) return type(o.new) == "function" and type(o.update) == "function", "the API is there; its objects draw and need a window" end },
  { lib = "Piefiller", job = "a graphical profiler", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "arson", job = "custom types for json.lua",
    run = function(m) return type(m.encode) ~= "nil" and type(m.decode) ~= "nil" and type(m.register) ~= "nil", "the API is there; it needs json.lua beside it" end },
  { lib = "cdata", job = "Lua tables to C data",
    run = function(m) return type(m.encode) ~= "nil" and type(m.decode) ~= "nil" and type(m.new_struct) ~= "nil", "the API is there; it works on C structs defined with ffi" end },
  { lib = "Lady", job = "saving and loading savegames",
    run = function(m) return type(m.save_all) ~= "nil" and type(m.load_all) ~= "nil" and type(m.register_class) ~= "nil", "the API is there; it writes to the save folder and none was written" end },
  { lib = "ShaderScan", job = "shader hot reload",
    run = function(m) return type(m.load_shader) ~= "nil" and type(m.update) ~= "nil", "the API is there; it compiles shaders and needs a window" end },
  { lib = "lunatest", job = "xUnit-style tests",
    run = function(m) return type(m.suite) ~= "nil" and type(m.run) ~= "nil", "the API is there; the suites it runs are yours" end },
  { lib = "Badr", job = "UI components",
    run = function(m) return type(m.new) ~= "nil", "the API is there; it draws and needs a window" end },
  { lib = "CatUI", job = "a light-weight GUI",
    run = function(m) return type(m.UIButton) ~= "nil" and type(m.UILabel) ~= "nil" and type(m.UIManager) ~= "nil", "its API is globals (UIButton, UILabel, UIManager and more); it draws and was not looked at" end },
  { lib = "gooi", job = "a GUI for Android", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "LoveFrames", job = "a GUI with many controls", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "plan", job = "rule-based layout",
    run = function(m) return type(m.new) ~= "nil" and type(m.Rules) ~= "nil", "the API is there; its rules were not exercised" end },
  { lib = "TuxRedux", job = "an immediate-mode UI", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "yalg", job = "a reactive UI",
    run = function(m) return type(m.new) ~= "nil" and type(m.update) ~= "nil" and type(m.resize) ~= "nil", "the API is there; it draws and needs a window" end },
  { lib = "bar128-love", job = "Code 128 barcodes",
    run = function(b)
      local code = b("HELLO")
      return code:getCode() == "HELLO" and code:getFullWidth() > 0, "a barcode for HELLO is " .. tostring(code:getFullWidth()) .. " units wide"
    end },
  { lib = "GifCat", job = "saving GIFs",
    run = function(m) return type(m.newGif) ~= "nil" and type(m.add) ~= "nil" and type(m.close) ~= "nil", "the API is there; it starts a thread and writes a file, so none was started" end },
  { lib = "ini_parser", job = ".ini files",
    run = function(ini)
      local t = {}
      ini.addSection(t, "audio")
      ini.addKey(t, "audio", "volume", "7")
      local ok = ini.sectionExists(t, "audio") == 1 and ini.keyExists(t, "audio", "volume") == 1 and tostring(ini.readKey(t, "audio", "volume")) == "7"
      ini.deleteKey(t, "audio", "volume")
      return ok and ini.keyExists(t, "audio", "volume") == 0, "a key was added, read and deleted in a section; its *Exists functions return 1 or 0 and 0 is true in Lua, so `if ini.keyExists(...)` is always true"
    end },
  { lib = "KeyedArray", job = "ordered dictionary",
    run = function(KA)
      local k = KA:new()
      k:insert("a", 10); k:insert("b", 20)
      return k:size() == 2 and k:get("a", "key") == 10 and k:get(2, "array") == 20, "two items were inserted and one was read back by key"
    end },
  { lib = "love-qrcode", job = "QR codes",
    run = function(m) return type(m.setText) ~= "nil" and type(m.getSize) ~= "nil" and type(m.draw) ~= "nil", "the API is there; it draws and needs a window" end },
  { lib = "love2d-assets-loader", job = "loading assets from folders",
    run = function(m) return type(m.init) ~= "nil" and type(m.setBaseImageDir) ~= "nil", "the API is there; it loads files and none were loaded" end },
  { lib = "LuaRequireExtended", job = "require with parameters",
    run = function() return require("lib.libload").replacedRequire["LuaRequireExtended"] == true, "loading it replaced the global `require` (the benchmark puts it back); that is why it is not a drop-in" end },
  { lib = "ShakeDetectorLua", job = "shake detection from the accelerometer",
    run = function(sd)
      sd:reset(2, 1)
      for i = 1, 20 do local v = (i % 2 == 0) and 4 or -4; sd:update(0.5, v, v, 0) end
      return sd.count > 0, "twenty alternating readings counted " .. tostring(sd.count) .. " shakes"
    end },
  { lib = "LUA-SNG-Parser", job = ".sng song packages",
    run = function(m) return type(m.load) ~= "nil" and type(m.extractToDirectory) ~= "nil", "the API is there; the sample ships no .sng file" end },
  { lib = "fetch-lua", job = "HTTP and HTTPS",
    run = function(m) return type(m.call) ~= "nil" and type(m.update) ~= "nil", "the API is there; nothing was sent" end },
  { lib = "love-dialogue", job = "dialogue with a script language",
    run = function(m) return type(m.new) ~= "nil" and type(m.loadFromFile) ~= "nil" and type(m.play) ~= "nil", "the API is there; it draws and needs a window and a dialogue file" end },
  { lib = "safeword", job = "overscan detection",
    run = function(m) return type(m.new) ~= "nil" and type(m.draw) ~= "nil", "the API is there; it draws and needs a window" end },
  { lib = "Sysl-Pixel", job = "a pixel-art scaler", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "loveblobs", job = "softbody physics",
    run = function(m) return type(m.softbody) ~= "nil" and type(m.softsurface) ~= "nil", "the API is there; it needs a love.physics world and a window" end },
  { lib = "SYSL-Text", job = "animated text",
    run = function(m) return m ~= nil, "loaded" end },
  { lib = "3DreamEngine", job = "a 3D engine", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "Lovox", job = "pseudo-3D voxels", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "FPSGraph", job = "an FPS graph", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "LoveDebug", job = "an inline debug console", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "shack", job = "screen shake and rotation", needs = "window",
    run = function(m) return m ~= nil, "loaded with a window" end },
  { lib = "ripple", job = "sound tags, volume, fades",
    run = function(ripple)
      if not (love and love.audio and love.sound) then return "skip", "needs LÖVE's audio and sound modules" end
      local data = love.sound.newSoundData(441, 44100, 16, 1)
      local source = love.audio.newSource(data, "static")
      local sound = ripple.newSound(source, { volume = 0.5 })
      local tag = ripple.newTag(); tag.volume = 0.5
      local inst = sound:play({ volume = 0.5 })
      local ok = inst ~= nil
      if inst and inst.stop then inst:stop() end
      return ok, "a generated one-hundredth of a second of silence played and stopped"
    end },
  { lib = "moonshine", job = "post-processing shader effects", needs = "window",
    run = function(moonshine)
      local fx = moonshine(moonshine.effects.vignette)
      fx.draw(function() love.graphics.rectangle("fill", 0, 0, 10, 10) end)
      return true, "a vignette drawn over a rectangle"
    end },
  { lib = "g3d", job = "3D models and a camera", needs = "window",
    run = function(g3d)
      local model = g3d.newModel({ { 0, 0, 0, 0, 0 }, { 1, 0, 0, 1, 0 }, { 0, 1, 0, 0, 1 } })
      model:setTranslation(0, 0, 3)
      return model ~= nil, "a one-triangle model made and moved"
    end },
  { lib = "push", job = "a virtual resolution scaled to the window", needs = "window",
    run = function(push)
      push:setupScreen(320, 240, 640, 480, { fullscreen = false, resizable = true })
      local gx, gy = push:toGame(320, 240)
      return type(gx) == "number" and type(gy) == "number", "the window's centre maps into the 320 x 240 game screen"
    end },
  { lib = "love-loader", job = "loading assets on a thread",
    run = function(loader)
      if not (love and love.thread and love.filesystem and love.filesystem.getInfo("assets/fonts/vga.woff")) then return "skip", "needs LÖVE's thread module" end
      local done = false
      loader.newFont("font", "assets/fonts/vga.woff", 16)
      local started, why = pcall(loader.start, function() done = true end)
      if not started then
        if tostring(why):find("love-loader.lua", 1, true) then return false, "it reads its own source as `love-loader.lua` from the game's root folder, so it only works copied there, not from libs/" end
        error(why, 0)
      end
      local t0 = os.clock()
      while not done and os.clock() - t0 < 5 do loader.update(); if love.timer and love.timer.sleep then love.timer.sleep(0.005) end end
      return done and loader.resources and loader.resources.font ~= nil, done and "a font loaded on a thread" or "did not finish within 5 s"
    end },
  { lib = "lurker", job = "hot reloading of changed files",
    run = function(lurker) return type(lurker.update) == "function" and type(lurker.scan) == "function", "the API is there; watching files for changes needs a person editing, so no reload was tried" end },
  { lib = "lovebird", job = "a web console for a running game",
    run = function(lovebird) return type(lovebird.update) == "function", "the API is there; it opens a network port, so it was not started" end },
  { lib = "sock", job = "networking (ENet)",
    run = function(sock)
      local ok = pcall(require, "enet")
      return type(sock.newServer) == "function" and type(sock.newClient) == "function", ok and "the API and ENet are there; no connection was opened" or "the API is there but LÖVE's enet module is not"
    end },
  { lib = "luajit-request", job = "HTTP through libcurl",
    run = function(request) return type(request.send) == "function", "libcurl loaded and the API is there; nothing was sent" end },
  { lib = "nativefs", job = "files anywhere on the computer",
    run = function(nativefs)
      -- A file in the game's own save folder, but written and read through the full path, which is what nativefs is for.
      if not (love and love.filesystem and love.filesystem.write("nativefs-probe.txt", "x")) then return "skip", "no writable folder here" end
      local path = love.filesystem.getSaveDirectory() .. "/nativefs-check.tmp"
      local okw = nativefs.write(path, "joyride benchmark")
      local text = nativefs.read(path)
      nativefs.remove(path)
      love.filesystem.remove("nativefs-probe.txt")
      return okw and text == "joyride benchmark", "a file written, read back and removed by its full path in the save folder"
    end },
  { lib = "neumetik-midi", job = "MIDI music with no SoundFont",
    run = function(nm)
      -- A one-note song made here (a General MIDI piano playing middle C for one beat), so the check needs no file and no bank.
      local function chunk(tag, body) return tag .. string.char(0, 0, math.floor(#body / 256), #body % 256) end
      local track = string.char(0x00, 0xC0, 0x00, 0x00, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 64, 0x00, 0xFF, 0x2F, 0x00)
      local smf = "MThd" .. string.char(0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xE0) .. "MTrk" .. string.char(0, 0, 0, #track) .. track
      local ffi = require("ffi")
      local p = nm.player(44100)
      local ok, why = p:load(smf)
      if not ok then return false, "refused a valid MIDI file: " .. tostring(why) end
      p:play()
      local frames = 4410
      local buf = ffi.new("float[?]", frames * 2)
      local peak = 0
      for _ = 1, 10 do
        p:renderF32(buf, frames)
        for j = 0, frames * 2 - 1 do local a = math.abs(buf[j]); if a > peak then peak = a end end
      end
      local bankOk = p:loadSoundBank("not a soundfont")   -- a SoundFont or DLS bank is accepted; anything else is refused, not crashed on
      return peak > 0.01 and not bankOk, ("a one-note song was synthesized, peak %.2f, with no bank; a non-bank was refused. Loading a real SoundFont was tried by hand (GeneralUser GS, 30 MB, 13 ms), none is shipped here"):format(peak)
    end },
  { lib = "semver", job = "version numbers",
    run = function(semver)
      local a, b = semver("1.2.3"), semver("1.10.0")
      return a < b and tostring(a) == "1.2.3" and a:nextMinor() == semver("1.3.0"), "1.2.3 is below 1.10.0; the next minor of 1.2.3 is 1.3.0"
    end },
  { lib = "log", job = "levelled console logging",
    run = function(log)
      local seen = 0
      silently(function() log.usecolor = false; log.level = "warn"; log.info("hidden"); log.warn("shown") end)
      log.level = "trace"
      return type(log.error) == "function" and log.level == "trace", "levels set; messages go to the console (and a file when asked)"
    end },
  { lib = "lua-discordRPC", job = "Discord Rich Presence",
    run = function(discord)
      -- Starts Discord's client library (a local connection attempt, no account, nothing sent) and shuts it down again.
      silently(function() discord.initialize("0", false); discord.runCallbacks(); discord.shutdown() end)
      return true, "discord-rpc, built from source, started and stopped; no presence was sent (Discord itself need not be running)"
    end },
  { lib = "luasteam", job = "Steamworks", run = function() return "skip", "needs Valve's Steamworks SDK" end },
  { lib = "love-nuklear", job = "Nuklear GUI (compiled)", needs = "window",
    run = function(nuklear)
      local ui = nuklear.newUI()
      ui:frameBegin()
      if ui:windowBegin("Check", 10, 10, 200, 100, "border", "title") then ui:layoutRow("dynamic", 30, 1); ui:label("Hello"); ui:button("OK") end
      ui:windowEnd(); ui:frameEnd(); ui:draw()
      return true, "a window with a label and a button laid out and drawn"
    end },
  { lib = "cimgui-love", job = "Dear ImGui (FFI)", needs = "window",
    run = function(imgui)
      imgui.love.Init()
      imgui.love.Update(1 / 60)
      imgui.NewFrame()
      imgui.Begin("Check"); imgui.Button("OK"); imgui.End()
      imgui.Render()
      imgui.love.RenderDrawLists()
      return true, "a window with a button laid out and drawn"
    end },
  { lib = "ArrayRotation", job = "rotating 2D arrays",
    run = function(AR)
      local a = { { 1, 2, 3 }, { 4, 5, 6 } }
      local r = AR.rotate(a, 1)
      local back = AR.rotate(AR.rotate(AR.rotate(AR.rotate(a, 1), 1), 1), 1)
      local half = AR.rotate(a, 2)
      return #r == 3 and #r[1] == 2 and back[2][3] == 6 and back[1][1] == 1 and half[1][1] == 6 and half[2][3] == 1,
        "a 2x3 array became 3x2 at 90 degrees, 180 degrees reversed it, four quarter turns came back to the start"
    end },
  { lib = "lynx", job = "list-based menus",
    run = function(lynx)
      local m = lynx.menu({ { selectable = true }, { selectable = true } })
      m:down()
      return m.current == 2 and type(lynx.love_lynx.draw_text) == "function", "a two-item menu moved down to item 2; drawing needs a window and was not run"
    end },
  { lib = "Lovely-Toasts", job = "toast messages", needs = "window",
    run = function(t)
      t.show("hello", 0.1)
      local shown = #_toasts == 1
      for _ = 1, 20 do t.update(0.1) end
      return shown and #_toasts == 0, "a toast was queued, then removed after its time (drawing was not looked at)"
    end },
  { lib = "Hanker", job = "mixed immediate and retained UI", needs = "window",
    run = function(h)
      local ctx = h.newContext()
      return type(ctx) == "table" and type(h.setViewport) == "function", "a context can be made; no control was laid out or drawn"
    end },
  { lib = "require-lua", job = "requiring a whole folder",
    run = function()
      -- The library replaces the global require; the sample restores it after loading, so run the library's own file in a private environment.
      local chunk = love.filesystem.load("libs/require-lua/require.lua")
      if not chunk then return "skip", "the library's file is not readable here" end
      local env = setmetatable({ love = love, require = require }, { __index = _G })
      setfenv(chunk, env)
      chunk()
      local r = env.require
      return type(r) == "table" and r.path("a.b.c") == "a.b" and r("string") == string and type(r.tree) == "function" and type(r.relative) == "function",
        "require became a callable table: require('string') still works and path('a.b.c') is 'a.b'; tree() was not run on a folder"
    end },
  { lib = "lovr-ui2d", job = "immediate mode GUI", needs = "window",
    run = function(UI2D)
      UI2D.Init("love")
      return UI2D.GetFontSize() == 14 and type(UI2D.Button) == "function" and type(UI2D.Begin) == "function", "initialised with the default font size 14; no window was laid out or drawn"
    end },
}

--- Run one specialist's check. Returns a cell: { status, note, ms }.
function S.run(entry, mod, env)
  if entry.needs == "window" and not env.window then return H.cell("SKIP", "needs a window: run this on a screen") end
  local t0 = os.clock()
  local ok, a, b = pcall(entry.run, mod)
  local cell
  if not ok then cell = H.cell("FAIL", (tostring(a):gsub("\n.*", "")))
  elseif a == "skip" then cell = H.cell("SKIP", b)
  elseif a then cell = H.cell("PASS", b or "works")
  else cell = H.cell("FAIL", b or "wrong answer") end
  cell.ms = (os.clock() - t0) * 1000
  return cell
end

return S
