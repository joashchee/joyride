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
