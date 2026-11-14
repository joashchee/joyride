-- String helpers Lua's standard library leaves out: split, trim, prefix tests, templates, wrapping. lume against batteries.stringx.

return {
  id = "strings", title = "String helpers",
  about = "Split, trim, test prefixes, fill templates, wrap and title-case.",
  members = {
    { id = "lume", short = "lume", lib = "lume" },
    { id = "stringx", short = "strx", lib = "batteries", get = function(b) return b.stringx end },
  },
  rows = {
    { id = "split", label = "Split", kind = "feature", about = "\"a,b,,c\" on a comma is a, b, an empty string, c.",
      impl = {
        lume = function(l) local t = l.split("a,b,,c", ","); return #t == 4 and t[3] == "" and t[4] == "c" end,
        stringx = function(s) local t = s.split("a,b,,c", ","); return #t == 4 and t[3] == "" and t[4] == "c" end,
      } },
    { id = "trim", label = "Trim", kind = "feature", about = "Spaces and tabs off both ends; the inside untouched.",
      impl = {
        lume = function(l) return l.trim("  \t a b \n ") == "a b" end,
        stringx = function(s) return s.trim("  \t a b \n ") == "a b" end,
      } },
    { id = "prefix", label = "Prefix / suffix", kind = "feature", about = "\"hello\" starts with \"he\" and ends with \"lo\", not the other way round.",
      impl = {
        lume = false,
        stringx = function(s) return s.starts_with("hello", "he") and s.ends_with("hello", "lo") and not s.starts_with("hello", "lo") end,
      } },
    { id = "template", label = "Fill a template", kind = "feature", about = "\"Hi {name}, you are {age}\" with a table of values.",
      impl = {
        lume = function(l) return l.format("Hi {name}, you are {age}", { name = "Bo", age = 7 }) == "Hi Bo, you are 7" end,
        stringx = function(s) return s.apply_template("Hi $name, you are ${age}", { name = "Bo", age = 7 }) == "Hi Bo, you are 7" end,
      } },
    { id = "wrap", label = "Word wrap", kind = "feature", about = "Wrap \"aaa bbb ccc\" at 7 columns: three lines.",
      impl = {
        lume = function(l) local out = l.wordwrap("aaa bbb ccc", 7); local n = 0; for _ in out:gmatch("[^\n]+") do n = n + 1 end; return n == 3 end,
        stringx = false,
      } },
    { id = "title", label = "Title case", kind = "feature", about = "\"hello big world\" becomes \"Hello Big World\".",
      impl = {
        lume = false,
        stringx = function(s) return s.title_case("hello big world") == "Hello Big World" end,
      } },
    { id = "contains", label = "Contains", kind = "feature", about = "A plain-text search (no patterns): \"a.c\" is in \"xa.cx\" but not in \"abc\".",
      impl = {
        lume = false,
        stringx = function(s) return s.contains("xa.cx", "a.c") and not s.contains("abc", "a.c") end,
      } },
    { id = "speed", label = "Split 1000 fields", kind = "speed", unit = "fields/s", units = 1000, about = "Split a string of 1000 comma-separated numbers.",
      expect = function(n) return n == 1000, n end,
      impl = {
        lume = function(l) local parts = {}; for i = 1, 1000 do parts[i] = tostring(i) end; local line = table.concat(parts, ",")
          return function() return #l.split(line, ",") end end,
        stringx = function(s) local parts = {}; for i = 1, 1000 do parts[i] = tostring(i) end; local line = table.concat(parts, ",")
          return function() return #s.split(line, ",") end end,
      } },
  },
}
