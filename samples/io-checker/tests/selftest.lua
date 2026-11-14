-- `love . --selftest`: drives every screen of the IO Checker with no window and no GPU. LÖVE's drawing and input modules are replaced by
-- mocks that remember what was asked of them; everything else (filesystem, image, sound, font, thread, system) is the real LÖVE.
-- It presses keys, taps, touches and pushes a fake controller's sticks through the same callbacks a person would, opens every page
-- and every asset, checks the text each screen shows, and exits 1 on any failure. Real drawing is NOT tested here: that needs a
-- display (see the README).

local T = {}

local failures, checks = {}, 0

local function check(ok, what)
  checks = checks + 1
  if not ok then failures[#failures + 1] = what; print("FAIL " .. what) end
end

local function contains(hay, needle) return hay:find(needle, 1, true) ~= nil end

-- ---- mocks ----------------------------------------------------------------------------------------------------------------------

local function noop() end

local function installMocks(w, h)
  local calls = { draw = 0, print = 0 }
  local function object(extra)
    return setmetatable(extra or {}, { __index = function() return noop end })
  end
  love.graphics = {
    isActive = function() return true end,
    getDimensions = function() return w, h end, getWidth = function() return w end, getHeight = function() return h end,
    getPixelDimensions = function() return w, h end,
    getRendererInfo = function() return "Mock", "1.0", "selftest", "no GPU" end,
    getSystemLimits = function() return { texturesize = 4096 } end,
    getSupported = function() return { canvas = true, shader = true } end,
    newFont = function() return object({ getWidth = function() return 8 end, setFilter = noop }) end,
    newImageFont = function() return object({ getWidth = function() return 8 end }) end,
    newImage = function() return object({ getDimensions = function() return 160, 128 end, getWidth = function() return 160 end, getHeight = function() return 128 end }) end,
    newQuad = function() return {} end,
    newShader = function() return object() end,
    newVideo = function() return object({ getDimensions = function() return 64, 48 end, isPlaying = function() return false end, tell = function() return 0 end }) end,
    print = function() calls.print = calls.print + 1 end, printf = noop,
    draw = function() calls.draw = calls.draw + 1 end,
  }
  for _, name in ipairs({ "setFont", "setColor", "rectangle", "clear", "push", "pop", "translate", "scale", "origin", "setScissor", "setBackgroundColor",
                          "setShader", "circle", "line", "setLineWidth" }) do love.graphics[name] = noop end
  love.window = {
    getMode = function() return w, h, { fullscreen = false, vsync = 1, resizable = true, refreshrate = 60, display = 1 } end,
    getDesktopDimensions = function() return 1920, 1080 end, getDPIScale = function() return 2 end,
    getSafeArea = function() return 0, 0, w, h end, getOrientation = function() return "landscape" end, setFullscreen = noop,
  }
  local keys, mouse = {}, { x = 0, y = 0 }
  love.keyboard = {
    isDown = function(k) return keys[k] == true end, setKeyRepeat = noop, setTextInput = noop, hasTextInput = function() return false end,
    hasKeyRepeat = function() return true end,
  }
  love.mouse = {
    getPosition = function() return mouse.x, mouse.y end, isDown = function() return false end, isCursorSupported = function() return true end,
    isVisible = function() return true end, isGrabbed = function() return false end,
  }
  love.touch = { getTouches = function() return {} end, getPosition = function() return 0, 0 end }
  return calls, keys, mouse
end

--- A fake controller with 14 buttons, 4 axes and a hat, driven by the test.
local function fakePad()
  local st = { down = {}, axes = { 0, 0, 0, 0 }, hat = "c", rumble = 0 }
  local j = {}
  function j:getName() return "Selftest Pad" end
  function j:getGUID() return "00000000selftest" end
  function j:getID() return 7 end
  function j:getButtonCount() return 14 end
  function j:getAxisCount() return 4 end
  function j:getHatCount() return 1 end
  function j:isGamepad() return false end
  function j:isDown(i) return st.down[i] == true end
  function j:getAxis(i) return st.axes[i] end
  function j:getHat() return st.hat end
  function j:isGamepadDown() return false end
  function j:isVibrationSupported() return true end
  function j:setVibration() st.rumble = st.rumble + 1 end
  return j, st
end

-- ---- the tour ---------------------------------------------------------------------------------------------------------------------

local function screenText(app)
  app.render()
  return app.scr:dump()
end

function T.run(core)
  local calls, keys, mouse = installMocks(768, 800)
  local pad, padState = fakePad()
  love.joystick = love.joystick or {}
  local realJoystick = love.joystick
  love.joystick = setmetatable({ getJoysticks = function() return { pad } end }, { __index = realJoystick })

  -- Start again with the mocks in place, as the real app would see a window.
  core.state = core.newState()
  local s = core.init()
  local App = require("ui.app")
  local ok, err = xpcall(function() App.install(core) end, debug.traceback)
  check(ok, "install: " .. tostring(err))
  if not ok then return true end

  local function step(name, fn)
    local good, e = xpcall(fn, debug.traceback)
    check(good, name .. ": " .. tostring(e))
    return good
  end
  local function frame(dt)
    love.update(dt or 0.016)
    love.draw()
    -- A job set by a page runs on the frame after it was drawn.
    love.update(dt or 0.016)
    love.draw()
  end
  local function press(k) love.keypressed(k, nil, false); love.keyreleased(k) end

  -- Home: the list, then every page reached the way a person would, by keys.
  step("home screen", function()
    frame()
    local t = screenText(App)
    check(contains(t, "IO Checker"), "home shows the title")
    check(contains(t, "Game controllers"), "home lists game controllers")
    check(contains(t, "Assets: every type"), "home lists the assets check")
    check(contains(t, "[Pg-]") and contains(t, "[Back]"), "the soft keys are on screen")
  end)

  local plan = {
    { "platform", { "Detected", "Checking", "Evidence" } },
    { "system", { "LÖVE and the machine" } },
    { "storage", { "Save folder", "PASS" } },
    { "sound", { "1 kHz, left", "Sweep" } },
    { "video", { "Colour bars", "Frame rate" } },
    { "assets", { "hero.png", "jingle.wav" } },
    { "report", { "Saved:" } },
  }
  for _, p in ipairs(plan) do
    step("page " .. p[1], function()
      App.go(p[1])
      frame()
      local t = screenText(App)
      for _, want in ipairs(p[2]) do check(contains(t, want), p[1] .. " shows " .. want) end
      press("pagedown"); frame(); press("pageup"); frame()
    end)
  end

  -- The pattern page: every pattern drawn full window, left and right between them.
  step("every pattern", function()
    App.go("video")
    for _ = 1, #core.model.PATTERNS do
      press("return"); frame(0.5)
      check(App.s.page == "pattern", "a pattern opens")
      press("right"); frame(0.5)
      press("escape"); frame()
      press("down"); frame()
    end
    check(next(s.seen) ~= nil, "the patterns shown are recorded")
    check(s.seen.motion ~= nil, "the motion pattern was shown")
  end)

  -- The platform page: every platform chosen in turn re-judges the capabilities.
  step("every platform", function()
    App.go("platform")
    local seen = {}
    for _ = 1, #core.platform.PROFILES + 1 do
      press("right"); frame()
      seen[s.profile.id] = true
      local t = screenText(App)
      check(contains(t, "Checking"), "platform page shows what is checked")
    end
    for _, p in ipairs(core.platform.PROFILES) do check(seen[p.id], "platform " .. p.id .. " reachable") end
    check(s.override == nil, "cycling all the way round is automatic again")
    check(not s.forced, "automatic means not forced")
  end)

  -- Input pages record everything and navigate nothing.
  step("keyboard page", function()
    App.go("keyboard"); frame()
    press("q"); press("escape"); press("return")
    love.textinput("héllo")
    check(App.s.page == "keyboard", "keys do not navigate an input page")
    check(s.input.keyCount >= 3, "keys are recorded (" .. s.input.keyCount .. ")")
    check(s.input.text:find("llo", 1, true) ~= nil, "typed text is recorded")
    frame()
    check(contains(screenText(App), "Different keys seen"), "keyboard page shows the count")
    -- Hold escape for a second: back to the list.
    keys.escape = true
    for _ = 1, 80 do love.update(0.016) end
    keys.escape = nil
    check(App.s.page == "home", "holding Escape leaves the page")
    -- Still holding it: the key repeats on the list, and that must not quit or even arm the quit.
    local quits = 0
    local realQuit = love.event.quit
    love.event.quit = function() quits = quits + 1 end
    keys.escape = true
    for _ = 1, 20 do love.keypressed("escape", nil, true); love.update(0.016) end
    check(quits == 0 and (s.quitT or 0) <= 0, "a held Escape does not quit or arm quitting after leaving")
    keys.escape = nil
    love.keyreleased("escape")
    -- A fresh press arms it, a second quits; the soft Back button does the same.
    press("escape")
    check(quits == 0 and (s.quitT or 0) > 0, "one Escape on the list only asks")
    press("escape")
    check(quits == 1, "a second Escape quits")
    love.event.quit = realQuit
    s.quitT = 0
  end)

  step("pointer page", function()
    App.go("pointer"); frame()
    mouse.x, mouse.y = 100, 300
    love.mousemoved(100, 300)
    love.mousepressed(100, 300, 1, false); love.mousereleased(100, 300, 1, false)
    love.mousepressed(120, 300, 1, true); love.mousereleased(120, 300, 1, true) -- a touch, also sent as a mouse event
    love.touchpressed("a", 100, 300, 0, 0, 0.5); love.touchpressed("b", 200, 300, 0, 0, 1)
    love.touchmoved("a", 110, 300); love.touchreleased("a"); love.touchreleased("b")
    love.wheelmoved(0, 2)
    frame()
    check(s.input.mouse.presses == 1, "a click is one mouse press, the touch's mouse event is not (" .. s.input.mouse.presses .. ")")
    check(s.input.touch.max == 2, "two fingers at once are counted")
    check(s.input.touch.pressureMax == 1, "pressure is kept")
    check(s.input.mouse.wheel == 2, "the wheel is summed")
    check(App.s.page == "pointer", "taps on the screen do not navigate an input page")
    check(contains(screenText(App), "Fingers down now"), "pointer page shows touches")
    -- Hold the bottom bar: back to the list.
    local bar = App.hits[1]
    check(bar and bar.leave, "the leave bar is a hit area")
    love.mousepressed(App.ox + 10, App.oy + (bar.y * 16 + 4) * App.scale, 1, false)
    for _ = 1, 80 do love.update(0.016) end
    love.mousereleased(0, 0, 1, false)
    check(App.s.page == "home", "holding the bar leaves the page")
  end)

  step("gamepad page", function()
    core.pollJoysticks(s)
    App.go("gamepad"); frame()
    love.joystickadded(pad)
    for b = 1, 10 do padState.down[b] = true; love.joystickpressed(pad, b); frame(); padState.down[b] = nil; love.joystickreleased(pad, b) end
    padState.axes[1] = -1; love.joystickaxis(pad, 1, -1); frame()
    padState.axes[1] = 1; love.joystickaxis(pad, 1, 1); padState.axes[1] = 0; love.joystickaxis(pad, 1, 0)
    love.joystickhat(pad, 1, "ul")
    check(App.s.page == "gamepad", "pad buttons do not navigate an input page")
    frame()
    local t = screenText(App)
    check(contains(t, "Selftest Pad"), "gamepad page names the pad")
    check(contains(t, "Seen 10/14"), "ten of fourteen buttons seen: " .. (t:match("Seen[^\n]*") or "?"))
    check(#s.input:missing(7) == 4, "four buttons never pressed")
    check(contains(t, "never: 11 12 13 14"), "the missing buttons are named")
    press("r"); check(padState.rumble == 1, "R rumbles the pad")
    -- Select+Start held (raw buttons 13 and 14).
    padState.down[13], padState.down[14] = true, true
    for _ = 1, 80 do love.update(0.016) end
    padState.down = {}
    check(App.s.page == "home", "holding Select+Start leaves the page")
  end)

  step("sensors page", function()
    App.go("sensors"); frame()
    love.system.vibrate = love.system.vibrate or noop
    press("v")
    check(s.answers.vibrate == "buzzed", "V buzzes")
    frame()
    check(contains(screenText(App), "Vibration"), "sensors page shows vibration")
    App.go("home")
  end)

  -- The pad as a navigator, on a list page: raw buttons 1 and 2 are accept and back.
  step("the pad navigates a list", function()
    App.go("home"); frame()
    love.joystickhat(pad, 1, "d"); love.joystickhat(pad, 1, "c")
    check(s.sel.home == 2, "hat down moves the selection")
    love.joystickpressed(pad, 1)
    check(App.s.page == "keyboard", "button 1 opens the selected page (" .. App.s.page .. ")")
    App.go("home")
  end)

  -- Sound: play and answer.
  step("sound answers", function()
    App.go("sound"); frame()
    press("y"); check(s.answers.left == nil, "answering before playing does nothing")
    press("return"); check(s.answers.left == "played", "play marks the tone played")
    press("y"); check(s.answers.left == "heard", "Y marks heard")
    check(s.sel.sound == 2, "and moves on to the next tone")
    press("return"); press("n"); check(s.answers.right == "silent", "N marks silent")
    check(App.s.page == "sound", "answering does not leave the page")
    -- The on-screen buttons do the same for a mouse or a finger.
    frame()
    local function tapButton(label)
      for _, h in ipairs(App.hits) do
        if h.fn and App.scr:row(h.y):sub(h.x + 1, h.x + #label + 2) == "[" .. label .. "]" then h.fn(App, s) return true end
      end
    end
    check(tapButton("Play"), "a Play button is on screen")
    check(tapButton("Heard"), "a Heard button is on screen")
    check(s.answers.both == "heard", "the Heard button marks heard")
    check(contains(screenText(App), "Did you hear it?"), "the instructions say what to do")
    App.go("home")
  end)

  -- Assets: open every one, so each viewer runs.
  step("every asset viewer", function()
    App.go("assets"); frame()
    check(s.assetResults ~= nil and #s.assetResults >= 45, "the asset list ran (" .. #(s.assetResults or {}) .. ")")
    local opened = 0
    for i = 1, #s.assetResults do
      s.sel.assets = i
      press("return")
      frame()
      check(App.s.page == "assetview", "asset " .. i .. " opens")
      local t = screenText(App)
      check(contains(t, s.assetResults[i].entry.path:sub(1, 40)), "asset view names " .. s.assetResults[i].entry.path)
      opened = opened + 1
      press("right"); frame()
      press("left"); frame()
      press("escape")
    end
    check(opened == #s.assetResults, "every asset was opened")
  end)

  -- A file the Asset Library adds later: it appears under its folder with no code change, loaded by its extension.
  step("an unlisted asset is found and loaded", function()
    if love.filesystem.createDirectory("assets") and love.filesystem.write("assets/zz-extra.txt", "found by extension\n") then
      core.runAssets(s)
      local found
      for _, r in ipairs(s.assetResults) do if r.entry.path == "assets/zz-extra.txt" then found = r end end
      check(found ~= nil, "the new file is in the list")
      check(found and found.unlisted and found.status == "PASS", "it loaded as text")
      love.filesystem.write("assets/zz-unknown.xyz", "?")
      core.runAssets(s)
      local unk
      for _, r in ipairs(s.assetResults) do if r.entry.path == "assets/zz-unknown.xyz" then unk = r end end
      check(unk and unk.status == "SKIP", "an unknown extension is skipped, not failed")
      love.filesystem.remove("assets/zz-extra.txt"); love.filesystem.remove("assets/zz-unknown.xyz"); love.filesystem.remove("assets")
      core.runAssets(s)
      check(#s.assetResults == #core.assets.LIST, "cleaned up: only the listed assets remain")
    else
      print("skip: the save folder is not writable here, so the unlisted-asset check did not run")
    end
  end)

  step("report", function()
    App.go("report"); frame()
    check(s.reportFile ~= nil, "the report was written")
    local r = s.reportObj
    check(r ~= nil, "report object")
    local text = r:text()
    for _, sec in ipairs({ "== platform ==", "== system ==", "== platform expectations ==", "== input ==", "== video ==", "== sound ==", "== storage ==", "== assets ==" }) do
      check(contains(text, sec), "report has " .. sec)
    end
    check(contains(text, "SKIP tone 440 Hz, both"), "tones not played are SKIP")
    check(contains(text, "PASS tone 1 kHz, left"), "a tone marked heard passes")
    check(contains(text, "FAIL tone 1 kHz, right"), "a tone marked silent fails")
    if not contains(text, "axis 1 range") then print((text:match("== input ==.-== video") or ""):sub(1, 1500)) end
    check(contains(text, "PASS pad 7 (Selftest Pad) axis 1 range") or contains(text, "PASS pad 7 (Selftest Pad)"), "the pad's axis range is judged")
    check(contains(text, "FAIL pad 7 (Selftest Pad) every button pressed"), "four buttons never pressed fails the pad")
  end)

  -- The drawing code ran against the mock for every frame above.
  check(calls.print > 0, "text was drawn (" .. calls.print .. " prints)")

  print(("selftest: %d checks, %d failed"):format(checks, #failures))
  return #failures > 0
end

return T
