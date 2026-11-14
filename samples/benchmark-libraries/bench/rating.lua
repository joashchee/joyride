-- Turns what was measured into ratings, finds the best library in each row, and the best in each group. No LÖVE, no libraries.
--
-- A cell's AUTOMATIC rating is 0 to 5:
--   0  the library has no such feature (NA)         1  it tried and was wrong, or errored (FAIL)
--   feature rows:  5 when it works
--   measured rows: 5 within 10% of the best library in the row, 4 within half of it, 3 within a fifth, 2 within a twentieth, else 1
-- The PERSON can overrule any cell with a rating of their own (1 to 5; saved on the device). The rating that counts is theirs when
-- there is one. SKIP cells (this run could not try them: no window, no FFI) have no rating and are left out of every average.
-- A row's BEST are the members with the top rating, but only when the row tells libraries apart: if every library that was tried
-- got the same rating, nobody is marked. A group's BEST is the member with the highest weighted mean over its rows (NA counts as 0,
-- so a library that offers less scores less; SKIP rows are not counted).

local R = {}

R.MANUAL_MIN, R.MANUAL_MAX = 1, 5

local STEPS = { { 0.9, 5 }, { 0.5, 4 }, { 0.2, 3 }, { 0.05, 2 } }

--- Sets cell.auto on every cell of a row (cells: member id -> cell). Returns the row's cells.
function R.rate(row, cells)
  local best
  local floor = row.floor or 0   -- measurements below this are noise: equal
  if row.kind ~= "feature" then
    for _, c in pairs(cells) do
      if c.status == "PASS" and c.value then
        local v = math.max(c.value, floor)
        if not best or (row.lower and v < best) or (not row.lower and v > best) then best = v end
      end
    end
  end
  for _, c in pairs(cells) do
    if c.status == "NA" then c.auto = 0
    elseif c.status == "FAIL" then c.auto = 1
    elseif c.status == "SKIP" then c.auto = nil
    elseif row.kind == "feature" then c.auto = 5
    else
      local ratio
      if not c.value or not best then ratio = 1
      elseif row.lower then
        local v = math.max(c.value, floor)
        ratio = (v <= 0) and 1 or (best <= 0 and 0 or best / v)
      else ratio = best > 0 and c.value / best or 1 end
      c.auto = 1
      for _, s in ipairs(STEPS) do if ratio >= s[1] then c.auto = s[2]; break end end
    end
  end
  return cells
end

--- The rating that counts: the person's, else the automatic one. nil for a SKIP.
function R.effective(cell, manual)
  if not cell or cell.status == "SKIP" then return nil end
  return manual or cell.auto
end

--- Members that are best in a row. `manualOf(memberId)` gives the person's rating or nil. Empty when nothing sets them apart.
function R.best(row, cells, order, manualOf)
  local top, low, any = -1, 99, false
  local eff = {}
  for _, id in ipairs(order) do
    local e = R.effective(cells[id], manualOf and manualOf(id))
    if e then
      eff[id] = e
      any = true
      if e > top then top = e end
      if e < low then low = e end
    end
  end
  local out = {}
  if not any or top == low or top < 3 then return out end
  for _, id in ipairs(order) do if eff[id] == top then out[id] = true end end
  return out
end

--- Group scores. rows = the group's rows (with .weight, default 1), results[rowId][memberId] = cell, manual(rowId, memberId) = rating or nil.
--- Returns { { id, score (0..5 or nil), counted (rows), best (bool) } ... } in member order. The cost rows count only with the group's own.
function R.groupScores(rows, order, results, manual)
  local out = {}
  local top = -1
  for _, id in ipairs(order) do
    local sum, wsum, n, own = 0, 0, 0, 0
    for _, row in ipairs(rows) do
      local cell = results[row.id] and results[row.id][id]
      local e = R.effective(cell, manual and manual(row.id, id))
      if e then
        local w = row.weight or 1
        sum, wsum, n = sum + e * w, wsum + w, n + 1
        if row.kind ~= "cost" then own = own + 1 end
      end
    end
    -- A library none of whose own rows could be tried (a GUI with no window) is not rated by the costs alone.
    local score = (wsum > 0 and own > 0) and sum / wsum or nil
    out[#out + 1] = { id = id, score = score, counted = n }
    if score and score > top then top = score end
  end
  local lowest = 99
  for _, o in ipairs(out) do if o.score and o.score < lowest then lowest = o.score end end
  for _, o in ipairs(out) do o.best = o.score ~= nil and top > lowest + 1e-9 and o.score >= top - 1e-9 end
  return out
end

--- The stars drawn for a rating: "*****" style, five wide.
function R.stars(v)
  if not v then return "     " end
  v = math.floor(v + 0.5)
  return string.rep("*", v) .. string.rep(".", 5 - v)
end

return R
