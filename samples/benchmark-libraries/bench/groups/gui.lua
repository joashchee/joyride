-- Immediate-mode GUIs: you call a function for every widget, every frame, and it returns whether it was used. They draw with LÖVE, so
-- every cell here needs a window: a headless run marks them SKIP, a run on a screen tries them. nuklear and cimgui are compiled
-- libraries that are not bundled (SKIP wherever they are not found).

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
        nuklear = function() return "skip", "a compiled module that is not bundled" end,
        cimgui = function() return "skip", "a compiled library that is not bundled" end,
      } },
    { id = "click", label = "Click a button", kind = "feature", about = "Press and release a simulated mouse over a button: the button reports a hit (once).",
      impl = {
        suit = function(suit)
          local hits = 0
          local function run(down)
            suit.updateMouse(20, 20, down)
            suit.layout:reset(10, 10)
            local b = suit.Button("OK", suit.layout:row(100, 30))
            if b.hit then hits = hits + 1 end
          end
          run(false); run(true); run(false); run(false)
          return hits == 1
        end,
        Slab = function() return "skip", "reads the real mouse, which a program cannot press" end,
        nuklear = function() return "skip", "a compiled module that is not bundled" end,
        cimgui = function() return "skip", "a compiled library that is not bundled" end,
      } },
    { id = "toggle", label = "Checkbox, slider", kind = "feature", about = "A simulated click flips a checkbox, and a drag across a slider changes its value.",
      impl = {
        suit = function(suit)
          local box, slider = { checked = false }, { value = 0, min = 0, max = 1 }
          local function run(x, y, down)
            suit.updateMouse(x, y, down); suit.layout:reset(10, 10)
            suit.Checkbox(box, suit.layout:row(100, 20)); suit.Slider(slider, suit.layout:row(100, 20))
          end
          run(15, 15, false); run(15, 15, true); run(15, 15, false)
          run(60, 40, false); run(60, 40, true); run(90, 40, true); run(90, 40, false)
          return box.checked == true and slider.value > 0
        end,
        Slab = function() return "skip", "reads the real mouse, which a program cannot press" end,
        nuklear = function() return "skip", "a compiled module that is not bundled" end,
        cimgui = function() return "skip", "a compiled library that is not bundled" end,
      } },
    { id = "speed", label = "100 buttons", kind = "speed", unit = "widgets/s", units = 100, about = "Lay out and update 100 buttons (not drawn).",
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
        nuklear = false,
        cimgui = false,
      } },
  },
}
