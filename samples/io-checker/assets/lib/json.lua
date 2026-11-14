-- A small JSON reader (decode only), so the Assets page can read data/*.json without a dependency. Written for the IO Checker.
local M = {}
local esc = { b = "\b", f = "\f", n = "\n", r = "\r", t = "\t", ['"'] = '"', ["\\"] = "\\", ["/"] = "/" }
local function utf8char(cp) return utf8 and utf8.char(cp) or string.char(cp % 256) end
function M.decode(s)
  local i = 1
  local function ws() i = s:find("%S", i) or #s + 1 end
  local value
  local function str()
    local out = {}
    i = i + 1
    while true do
      local c = s:sub(i, i)
      if c == "" then error("unterminated string at " .. i) end
      if c == '"' then i = i + 1 return table.concat(out) end
      if c == "\\" then
        local n = s:sub(i + 1, i + 1)
        if n == "u" then
          out[#out + 1] = utf8char(tonumber(s:sub(i + 2, i + 5), 16) or 63)
          i = i + 6
        else out[#out + 1] = esc[n] or n i = i + 2 end
      else out[#out + 1] = c i = i + 1 end
    end
  end
  function value()
    ws()
    local c = s:sub(i, i)
    if c == "{" then
      local t = {}
      i = i + 1 ws()
      if s:sub(i, i) == "}" then i = i + 1 return t end
      repeat
        ws()
        local k = str() ws()
        if s:sub(i, i) ~= ":" then error("expected : at " .. i) end
        i = i + 1
        t[k] = value() ws()
        local d = s:sub(i, i) i = i + 1
      until d ~= ","
      return t
    elseif c == "[" then
      local t = {}
      i = i + 1 ws()
      if s:sub(i, i) == "]" then i = i + 1 return t end
      repeat
        t[#t + 1] = value() ws()
        local d = s:sub(i, i) i = i + 1
      until d ~= ","
      return t
    elseif c == '"' then return str()
    elseif s:find("^true", i) then i = i + 4 return true
    elseif s:find("^false", i) then i = i + 5 return false
    elseif s:find("^null", i) then i = i + 4 return nil
    else
      local num = s:match("^-?%d+%.?%d*[eE]?[+-]?%d*", i)
      if not num or num == "" then error("unexpected character at " .. i) end
      i = i + #num
      return tonumber(num)
    end
  end
  local v = value()
  return v
end
return M
