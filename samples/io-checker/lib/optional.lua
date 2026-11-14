-- APIs that exist only in some LÖVE versions (love.sensor and love.system.getPreferredLocales arrive in 12.0), looked up by name at run
-- time. The project targets LÖVE 11.5, which is bundled with JoyrIDE; asking for a name that is not there gives nil, never an error,
-- so the same code is right on 11.5 and 12.0. (Writing `love.sensor.hasSensor` literally would be a lint error in an 11.5 project.)
local O = {}

--- The value at a dotted path under `love` ("sensor", "system.vibrate"), or nil when any part is missing.
function O.get(path)
  local v = love
  for part in path:gmatch("[^.]+") do
    if type(v) ~= "table" then return nil end
    v = v[part]
  end
  return v
end

--- Call a function by path if it exists; returns its results, or nothing.
function O.call(path, ...)
  local f = O.get(path)
  if type(f) == "function" then return f(...) end
end

return O
