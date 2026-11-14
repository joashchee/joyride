-- Immediate-mode GUIs: you call a function for every widget, every frame, and it returns whether it was used. They draw with LÖVE, so
-- every cell here needs a window: a headless run marks them SKIP, a run on a screen tries them. nuklear and cimgui are compiled
-- libraries built by tools/build-binaries.sh into bin/ (SKIP wherever that has not been run).

local function frame(fn) local ok, e = pcall(fn); if not ok then error(e, 0) end; return true end

return {
  id = "gui", title = "Immediate-mode GUI",
  about = "Build a frame of widgets, click a button with a simulated mouse, count how many widgets a second.",
  needs = "window",
  members = {
    { id = "suit", short = "suit", lib = "suit" },
    { id = "Slab", short = "Slab", lib = "Slab" },
    { id = "nuklear", short = "nuklr", lib = "love-nuklear" },
    { id = "cimgui", short = "cimgui", lib = "cimgui-love" },
  },
  rows = {
    { id = "frame", label = "One frame", kind = "feature", about = "Lay out a label, a button, a checkbox and a slider, and draw the frame, without an error.",
      impl = {
        suit = function(suit)
          suit.layout:reset(10, 10, 4, 4)
          local state = { checked = false }; local slider = { value = 0.5, min = 0, max = 1 }
          suit.Label("Benchmark", suit.layout:row(120, 20))
          suit.Button("OK", suit.layout:row(120, 24))
          suit.Checkbox(state, suit.layout:row(120, 20))
          suit.Slider(slider, suit.layout:row(120, 20))
          suit.draw()
          return true
        end,
        Slab = function(Slab)
          if not Slab._benchmarkStarted then Slab.Initialize({}); Slab._benchmarkStarted = true end
          Slab.Update(1 / 60)
          Slab.BeginWindow("benchmark", { Title = "Benchmark", X = 10, Y = 10, W = 200, H = 150 })
          Slab.Text("Benchmark"); Slab.Button("OK"); Slab.CheckBox(false, "tick")
          Slab.EndWindow()
          Slab.Draw()
          return true
        end,
        nuklear = function(nuklear)
          local ui = nuklear.newUI()
          ui:frameBegin()
          if ui:windowBegin("benchmark", 10, 10, 200, 150, "border", "title") then
            ui:layoutRow("dynamic", 24, 1); ui:label("Benchmark"); ui:button("OK")
          end
          ui:windowEnd(); ui:frameEnd(); ui:draw()
          return true
        end,
        cimgui = function(imgui)
          if not imgui._benchmarkStarted then imgui.love.Init(); imgui._benchmarkStarted = true end
          imgui.love.Update(1 / 60); imgui.NewFrame()
          imgui.Begin("benchmark"); imgui.Text("Benchmark"); imgui.Button("OK"); imgui.End()
          imgui.Render(); imgui.love.RenderDrawLists()
          return true
        end,
      } },
    { id = "click", label = "Click a button", kind = "feature", about = "Press and release a simulated mouse over a button: the button reports a hit (once).",
      impl = {
        suit = function(suit)
          local hits = 0
          local function run(down)
            suit.enterFrame()            -- a frame begins (it reads the real mouse, so the simulated one goes in after)
            suit.updateMouse(20, 20, down)
            suit.layout:reset(10, 10)
            local b = suit.Button("OK", suit.layout:row(100, 30))
            if b.hit then hits = hits + 1 end
          end
          run(false); run(true); run(false); run(false)
          return hits == 1
        end,
        Slab = function() return "skip", "its own mouse module is not driven by this benchmark" end,
        nuklear = function(nuklear)
          -- nuklear's button reports the click on the frame after the release; input goes in between frames
          local ui, hits = nuklear.newUI(), 0
          local function frame()
            ui:frameBegin()
            if ui:windowBegin("click", 10, 10, 200, 100, "border") then ui:layoutRow("dynamic", 30, 1); if ui:button("OK") then hits = hits + 1 end end
            ui:windowEnd(); ui:frameEnd(); ui:draw()
          end
          frame(); ui:mousemoved(100, 30, 0, 0, false); frame(); ui:mousepressed(100, 30, 1, false); frame(); ui:mousereleased(100, 30, 1, false); frame(); frame()
          return hits == 1
        end,
        cimgui = function(imgui)
          if not imgui._benchmarkStarted then imgui.love.Init(); imgui._benchmarkStarted = true end
          local io, hits, cx, cy = imgui.GetIO(), 0, 0, 0
          local function frame()
            imgui.love.Update(1 / 60); imgui.NewFrame()
            imgui.SetNextWindowPos(imgui.ImVec2_Float(10, 10)); imgui.SetNextWindowSize(imgui.ImVec2_Float(240, 160))
            imgui.Begin("bench-click", nil, 0)
            if imgui.Button("OK") then hits = hits + 1 end
            local mn, mx = imgui.GetItemRectMin(), imgui.GetItemRectMax(); cx, cy = (mn.x + mx.x) / 2, (mn.y + mx.y) / 2
            imgui.End(); imgui.Render(); imgui.love.RenderDrawLists()
          end
          -- events go straight to ImGui (imgui.love's own handlers ignore a mouse the window has not focused)
          frame(); io:AddMousePosEvent(cx, cy); frame(); io:AddMouseButtonEvent(0, true); frame(); io:AddMouseButtonEvent(0, false); frame(); frame()
          io:AddMousePosEvent(-10000, -10000); frame()
          return hits == 1
        end,
      } },
    { id = "toggle", label = "Checkbox, slider", kind = "feature", about = "A simulated click flips a checkbox, and a drag across a slider changes its value.",
      impl = {
        suit = function(suit)
          local box, slider = { checked = false }, { value = 0, min = 0, max = 1 }
          local function run(x, y, down)
            suit.enterFrame(); suit.updateMouse(x, y, down); suit.layout:reset(10, 10)
            suit.Checkbox(box, suit.layout:row(100, 20)); suit.Slider(slider, suit.layout:row(100, 20))
          end
          run(15, 15, false); run(15, 15, true); run(15, 15, false)
          run(60, 40, false); run(60, 40, true); run(90, 40, true); run(90, 40, false)
          return box.checked == true and slider.value > 0
        end,
        Slab = function() return "skip", "its own mouse module is not driven by this benchmark" end,
        nuklear = function(nuklear)
          local ui, box, slider = nuklear.newUI(), { value = false }, { value = 0 }
          local function frame()
            ui:frameBegin()
            if ui:windowBegin("toggle", 10, 10, 200, 100, "border") then
              ui:layoutRow("dynamic", 30, 1); ui:checkbox("tick", box)
              ui:layoutRow("dynamic", 30, 1); ui:slider(0, slider, 1, 0.05)
            end
            ui:windowEnd(); ui:frameEnd(); ui:draw()
          end
          frame(); ui:mousemoved(30, 30, 0, 0, false); frame(); ui:mousepressed(30, 30, 1, false); frame(); ui:mousereleased(30, 30, 1, false); frame(); frame()
          -- a slider moves only when the press starts on its knob (at the left end while it is 0)
          frame(); ui:mousemoved(20, 60, 0, 0, false); frame(); ui:mousepressed(20, 60, 1, false); frame(); ui:mousemoved(190, 60, 0, 0, false); frame()
          ui:mousereleased(190, 60, 1, false); frame(); frame()
          return box.value == true and slider.value > 0.5
        end,
        cimgui = function(imgui)
          if not imgui._benchmarkStarted then imgui.love.Init(); imgui._benchmarkStarted = true end
          local ffi = require("ffi")
          local io, box, val = imgui.GetIO(), ffi.new("bool[1]"), ffi.new("float[1]")
          local bx, by, sx, sy, sw = 0, 0, 0, 0, 0
          local function frame()
            imgui.love.Update(1 / 60); imgui.NewFrame()
            imgui.SetNextWindowPos(imgui.ImVec2_Float(10, 10)); imgui.SetNextWindowSize(imgui.ImVec2_Float(240, 160))
            imgui.Begin("bench-toggle", nil, 0)
            imgui.Checkbox("tick", box)
            local mn, mx = imgui.GetItemRectMin(), imgui.GetItemRectMax(); bx, by = mn.x + 8, (mn.y + mx.y) / 2
            imgui.SliderFloat("##s", val, 0, 1)
            mn, mx = imgui.GetItemRectMin(), imgui.GetItemRectMax(); sx, sy, sw = mn.x, (mn.y + mx.y) / 2, mx.x - mn.x
            imgui.End(); imgui.Render(); imgui.love.RenderDrawLists()
          end
          frame(); io:AddMousePosEvent(bx, by); frame(); io:AddMouseButtonEvent(0, true); frame(); io:AddMouseButtonEvent(0, false); frame(); frame()
          io:AddMousePosEvent(sx + 5, sy); frame(); io:AddMouseButtonEvent(0, true); frame(); io:AddMousePosEvent(sx + sw - 5, sy); frame()
          io:AddMouseButtonEvent(0, false); frame(); frame()
          io:AddMousePosEvent(-10000, -10000); frame()
          return box[0] == true and val[0] > 0.5
        end,
      } },
    { id = "speed", label = "100 buttons", kind = "speed", unit = "widgets/s", units = 100, about = "Lay out and update 100 buttons (nuklear must also draw: that is how its frame ends).",
      expect = function(v) return v == 100, v end,
      impl = {
        suit = function(suit) return function()
          suit.updateMouse(-1, -1, false); suit.layout:reset(0, 0, 2, 2)
          local n = 0
          for _ = 1, 100 do suit.Button("x", suit.layout:row(40, 14)); n = n + 1 end
          return n
        end end,
        Slab = function(Slab) return function()
          if not Slab._benchmarkStarted then Slab.Initialize({}); Slab._benchmarkStarted = true end
          Slab.Update(1 / 60)
          Slab.BeginWindow("speed", { X = 0, Y = 0, W = 400, H = 600 })
          local n = 0
          for _ = 1, 100 do Slab.Button("x"); n = n + 1 end
          Slab.EndWindow()
          return n
        end end,
        nuklear = function(nuklear) local ui = nuklear.newUI(); return function()
          ui:frameBegin()
          local n = 0
          if ui:windowBegin("speed", 0, 0, 400, 600, "border") then
            ui:layoutRow("dynamic", 14, 1)
            for _ = 1, 100 do ui:button("x"); n = n + 1 end
          end
          ui:windowEnd(); ui:frameEnd()
          ui:draw()   -- a nuklear frame is only over after draw: a second frame without it aborts the program (a C assert)
          return n
        end end,
        cimgui = function(imgui) return function()
          if not imgui._benchmarkStarted then imgui.love.Init(); imgui._benchmarkStarted = true end
          imgui.love.Update(1 / 60); imgui.NewFrame()
          imgui.Begin("speed")
          local n = 0
          for _ = 1, 100 do imgui.Button("x"); n = n + 1 end
          imgui.End(); imgui.Render()
          return n
        end end,
      } },
  },
}
