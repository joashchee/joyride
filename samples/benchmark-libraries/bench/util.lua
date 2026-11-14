-- Small helpers the groups share: deep comparison and the data they all use. No libraries here.

local U = {}

--- Deep equality of two plain values (tables compared by content, numbers exactly, NaN equals NaN).
function U.same(a, b, seen)
  if a ~= a and b ~= b then return true end
  if type(a) ~= type(b) then return false end
  if type(a) ~= "table" then return a == b end
  seen = seen or {}
  if seen[a] == b then return true end
  seen[a] = b
  for k, v in pairs(a) do if not U.same(v, b[k], seen) then return false end end
  for k in pairs(b) do if a[k] == nil then return false end end
  return true
end

function U.close(a, b, eps) return math.abs(a - b) <= (eps or 1e-9) end

--- Number of keys in a table.
function U.count(t) local n = 0; for _ in pairs(t) do n = n + 1 end; return n end

--- A settings-and-save-like record every serialiser should manage: only string keys in objects, arrays with no holes.
function U.record()
  return {
    name = "Joyride", version = 3, ratio = 0.75, enabled = true, off = false,
    tags = { "lua", "love", "games" },
    player = { x = 12.5, y = -3, health = 100, inventory = { { id = "sword", n = 1 }, { id = "potion", n = 7 } } },
    levels = { 1, 2, 3, 5, 8, 13, 21, 34 },
  }
end

--- A larger record for speed rows: 200 small objects.
function U.crowd()
  local t = {}
  for i = 1, 200 do t[i] = { id = i, name = "unit" .. i, x = i * 1.5, y = -i, alive = i % 2 == 0, tags = { "a", "b" } } end
  return { units = t }
end

return U
