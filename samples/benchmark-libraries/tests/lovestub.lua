-- LÖVE also provides utf8; plain luajit has none (enough of it for a library to be required).
package.preload.utf8 = package.preload.utf8 or function()
  return { charpattern = "[%z\1-\127\194-\244][\128-\191]*", len = function(s) return #s end, offset = function(_, n) return n end,
           char = function(...) return string.char(...) end, codepoint = function(s, i) return s:byte(i or 1) end }
end

-- A minimal `love` for plain luajit, only what the libraries read when they are required or used headless. Real runs use LÖVE's own.
love = love or {
  handlers = {}, _stub = true,
  timer = { getTime = function() return os.clock() end },
  system = { getOS = function() return "OS X" end },
  filesystem = {}, getVersion = function() return 11, 5, 0, "Mysterious Mysteries" end,
}
