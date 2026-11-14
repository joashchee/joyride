-- The person's own ratings: 1 to 5 on any cell, kept on the device in `benchmark-ratings.tsv` (one `group/row/member<TAB>rating` line each).
-- They overrule the automatic rating wherever they are set, in the tables, the group scores and the report. Storage is given in, so plain
-- luajit tests it: `store` has read() -> text or nil and write(text) -> true or nil, why.

local rating = require("bench.rating")

local R = {}
R.FILE = "benchmark-ratings.tsv"

function R.new(store)
  local self = { map = {}, store = store, changed = false }

  function self:key(g, r, m) return g .. "/" .. r .. "/" .. m end
  function self:get(g, r, m) return self.map[self:key(g, r, m)] end
  function self:count() local n = 0; for _ in pairs(self.map) do n = n + 1 end; return n end

  --- Sets (or, with nil, clears) a rating; out-of-range values are refused.
  function self:set(g, r, m, v)
    local k = self:key(g, r, m)
    if v == nil then self.map[k] = nil
    else
      v = math.floor(tonumber(v) or 0)
      if v < rating.MANUAL_MIN or v > rating.MANUAL_MAX then return false end
      self.map[k] = v
    end
    self.changed = true
    return true
  end

  --- Step a rating up or down: from nothing, up gives the cell's automatic rating plus one's worth: the caller passes `from`.
  function self:step(g, r, m, delta, from)
    local cur = self:get(g, r, m) or from or 3
    local nxt = math.max(rating.MANUAL_MIN, math.min(rating.MANUAL_MAX, cur + delta))
    self:set(g, r, m, nxt)
    return nxt
  end

  function self:text()
    local keys = {}
    for k in pairs(self.map) do keys[#keys + 1] = k end
    table.sort(keys)
    local out = {}
    for _, k in ipairs(keys) do out[#out + 1] = k .. "\t" .. self.map[k] end
    return table.concat(out, "\n") .. (#out > 0 and "\n" or "")
  end

  function self:load()
    local text = self.store and self.store.read and self.store.read()
    self.map = {}
    for line in (text or ""):gmatch("[^\r\n]+") do
      local k, v = line:match("^([^\t]+)\t(%d)$")
      if k and k:match("^[^/]+/[^/]+/[^/]+$") and tonumber(v) >= rating.MANUAL_MIN and tonumber(v) <= rating.MANUAL_MAX then self.map[k] = tonumber(v) end
    end
    self.changed = false
  end

  function self:save()
    if not (self.store and self.store.write) then return nil, "nowhere to save" end
    local ok, why = self.store.write(self:text())
    if ok then self.changed = false end
    return ok, why
  end

  --- The function the rating code wants: manual(group, row, member).
  function self:lookup() return function(g, r, m) return self:get(g, r, m) end end

  return self
end

--- A store over love.filesystem (the save folder).
function R.loveStore()
  return {
    read = function() return love.filesystem.getInfo(R.FILE) and love.filesystem.read(R.FILE) or nil end,
    write = function(text) return love.filesystem.write(R.FILE, text) end,
  }
end

return R
