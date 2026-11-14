-- Object orientation: four libraries that make Lua tables behave like classes. The same few tasks, written the way each library wants.
-- (batteries.class is one module of the batteries bundle; hump.class one of hump's.)

return {
  id = "classes", title = "Classes (OOP)",
  about = "Define a class, inherit, call super, test the type, mix in methods.",
  members = {
    { id = "middleclass", short = "mclass", lib = "middleclass" },
    { id = "classic", short = "clsc", lib = "classic" },
    { id = "hump.class", short = "hump", lib = "hump", get = function(_, req) return req("hump.class") end },
    { id = "batteries.class", short = "batt", lib = "batteries", get = function(m) return m.class end },
  },
  rows = {
    { id = "define", label = "Define + new", kind = "feature", about = "A class with a constructor taking arguments; new instances hold them and a method returns them.",
      impl = {
        middleclass = function(c)
          local A = c("A"); function A:initialize(x) self.x = x end; function A:get() return self.x end
          return A:new(7):get() == 7
        end,
        classic = function(C)
          local A = C:extend(); function A:new(x) self.x = x end; function A:get() return self.x end
          return A(7):get() == 7
        end,
        ["hump.class"] = function(C)
          local A = C{ init = function(self, x) self.x = x end, get = function(self) return self.x end }
          return A(7):get() == 7
        end,
        ["batteries.class"] = function(class)
          local A = class({ name = "A" }); function A:new(x) self.x = x end; function A:get() return self.x end
          return A(7):get() == 7
        end,
      } },
    { id = "inherit", label = "Inherit + super", kind = "feature", about = "A subclass overrides a method, calls the parent's version, and inherits the rest.",
      impl = {
        middleclass = function(c)
          local A = c("A"); function A:initialize(x) self.x = x end; function A:who() return "a" end; function A:only() return "base" end
          local B = c("B", A); function B:initialize(x, y) A.initialize(self, x); self.y = y end; function B:who() return "b" .. A.who(self) end
          local o = B:new(1, 2)
          return o.x == 1 and o.y == 2 and o:who() == "ba" and o:only() == "base"
        end,
        classic = function(C)
          local A = C:extend(); function A:new(x) self.x = x end; function A:who() return "a" end; function A:only() return "base" end
          local B = A:extend(); function B:new(x, y) B.super.new(self, x); self.y = y end; function B:who() return "b" .. B.super.who(self) end
          local o = B(1, 2)
          return o.x == 1 and o.y == 2 and o:who() == "ba" and o:only() == "base"
        end,
        ["hump.class"] = function(C)
          local A = C{ init = function(self, x) self.x = x end, who = function() return "a" end, only = function() return "base" end }
          local B = C{ __includes = A, init = function(self, x, y) A.init(self, x); self.y = y end, who = function(self) return "b" .. A.who(self) end }
          local o = B(1, 2)
          return o.x == 1 and o.y == 2 and o:who() == "ba" and o:only() == "base"
        end,
        ["batteries.class"] = function(class)
          local A = class({ name = "A" }); function A:new(x) self.x = x end; function A:who() return "a" end; function A:only() return "base" end
          local B = class({ name = "B", extends = A }); function B:new(x, y) self:super(x); self.y = y end; function B:who() return "b" .. A.who(self) end
          local o = B(1, 2)
          return o.x == 1 and o.y == 2 and o:who() == "ba" and o:only() == "base"
        end,
      } },
    { id = "isa", label = "Type test (is-a)", kind = "feature", about = "Ask an instance whether it is a class or a subclass of it, and not another class.",
      impl = {
        middleclass = function(c)
          local A = c("A"); local B = c("B", A); local X = c("X"); local o = B:new()
          return o:isInstanceOf(A) and o:isInstanceOf(B) and not o:isInstanceOf(X) and B:isSubclassOf(A)
        end,
        classic = function(C)
          local A = C:extend(); local B = A:extend(); local X = C:extend(); local o = B()
          return o:is(A) and o:is(B) and not o:is(X)
        end,
        ["hump.class"] = false,   -- hump.class has no type test
        ["batteries.class"] = function(class)
          local A = class({ name = "A" }); local B = class({ name = "B", extends = A }); local X = class({ name = "X" }); local o = B()
          return o:is(A) and o:is(B) and not o:is(X)
        end,
      } },
    { id = "mixin", label = "Mixins", kind = "feature", about = "Add a table of methods to a class so every instance has them.",
      impl = {
        middleclass = function(c)
          local Fly = { fly = function() return "up" end }
          local A = c("A"); A:include(Fly)
          return A:new():fly() == "up"
        end,
        classic = function(C)
          local Fly = C:extend(); function Fly:fly() return "up" end
          local A = C:extend(); A:implement(Fly)
          return A():fly() == "up"
        end,
        ["hump.class"] = function(C)
          local Fly = C{ fly = function() return "up" end }
          local A = C{ __includes = { Fly } }
          return A():fly() == "up"
        end,
        ["batteries.class"] = function(class)
          local Fly = class({ name = "Fly" }); function Fly:fly() return "up" end
          local A = class({ name = "A", implements = { Fly } })
          return A():fly() == "up"
        end,
      } },
    { id = "tostring", label = "__tostring etc.", kind = "feature", about = "Metamethods defined on the class work on instances (here __tostring and __add).",
      impl = {
        middleclass = function(c)
          local A = c("A"); function A:initialize(n) self.n = n end
          A.__tostring = function(s) return "A" .. s.n end; A.__add = function(a, b) return A:new(a.n + b.n) end
          return tostring(A:new(1) + A:new(2)) == "A3"
        end,
        classic = function(C)
          local A = C:extend(); function A:new(n) self.n = n end
          function A:__tostring() return "A" .. self.n end; function A.__add(a, b) return A(a.n + b.n) end
          return tostring(A(1) + A(2)) == "A3"
        end,
        ["hump.class"] = function(C)
          local A = C{ init = function(self, n) self.n = n end }
          A.__tostring = function(s) return "A" .. s.n end; A.__add = function(a, b) return A(a.n + b.n) end
          return tostring(A(1) + A(2)) == "A3"
        end,
        ["batteries.class"] = function(class)
          local A = class({ name = "A" }); function A:new(n) self.n = n end
          function A:__tostring() return "A" .. self.n end; function A.__add(a, b) return A(a.n + b.n) end
          return tostring(A(1) + A(2)) == "A3"
        end,
      } },
    { id = "new-speed", label = "Create objects", kind = "speed", unit = "objects/s", units = 100, about = "Construct 100 instances of a class whose constructor sets two fields.",
      expect = function(n) return n == 100, n end,
      impl = {
        middleclass = function(c) local A = c("A"); function A:initialize(x, y) self.x, self.y = x, y end
          return function() local o; for i = 1, 100 do o = A:new(i, i) end; return o.x end end,
        classic = function(C) local A = C:extend(); function A:new(x, y) self.x, self.y = x, y end
          return function() local o; for i = 1, 100 do o = A(i, i) end; return o.x end end,
        ["hump.class"] = function(C) local A = C{ init = function(self, x, y) self.x, self.y = x, y end }
          return function() local o; for i = 1, 100 do o = A(i, i) end; return o.x end end,
        ["batteries.class"] = function(class) local A = class({ name = "A" }); function A:new(x, y) self.x, self.y = x, y end
          return function() local o; for i = 1, 100 do o = A(i, i) end; return o.x end end,
      } },
    { id = "call-speed", label = "Method calls", kind = "speed", unit = "calls/s", units = 1000, about = "Call an inherited method 1000 times on an instance two levels down.",
      expect = function(n) return n == 1000, n end,
      impl = {
        middleclass = function(c) local A = c("A"); function A:initialize() self.n = 0 end; function A:inc() self.n = self.n + 1 end
          local o = c("C", c("B", A)):new()
          return function() o.n = 0; for _ = 1, 1000 do o:inc() end; return o.n end end,
        classic = function(C) local A = C:extend(); function A:new() self.n = 0 end; function A:inc() self.n = self.n + 1 end
          local o = A:extend():extend()()
          return function() o.n = 0; for _ = 1, 1000 do o:inc() end; return o.n end end,
        ["hump.class"] = function(C) local A = C{ init = function(self) self.n = 0 end, inc = function(self) self.n = self.n + 1 end }
          local o = C{ __includes = C{ __includes = A } }()
          return function() o.n = 0; for _ = 1, 1000 do o:inc() end; return o.n end end,
        ["batteries.class"] = function(class) local A = class({ name = "A" }); function A:new() self.n = 0 end; function A:inc() self.n = self.n + 1 end
          local o = class({ name = "C", extends = class({ name = "B", extends = A }) })()
          return function() o.n = 0; for _ = 1, 1000 do o:inc() end; return o.n end end,
      } },
  },
}
