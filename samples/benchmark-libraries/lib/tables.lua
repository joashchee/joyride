-- How a group's results are shown as tables, in text and in Markdown: one place, so the screen, the report and the Markdown file agree
-- on what a cell says and which library is marked best.

local rating = require("bench.rating")
local runner = require("bench.runner")
local model = require("lib.model")

local T = {}

--- What one cell holds, worked out once: { cell, eff (the rating that counts), auto, manual, best (bool), text }.
--- `manual(groupId, rowId, memberId)` is the person's rating or nil.
function T.cells(group, results, manual)
  local rows = runner.rows(group)
  local order = runner.order(group)
  local gr = results.groups[group.id] or {}
  local out = {}
  for _, row in ipairs(rows) do
    local cells = gr[row.id] or {}
    local best = rating.best(row, cells, order, function(m) return manual and manual(group.id, row.id, m) end)
    out[row.id] = {}
    for _, m in ipairs(order) do
      local c = cells[m]
      local man = manual and manual(group.id, row.id, m)
      out[row.id][m] = { cell = c, auto = c and c.auto, manual = man, eff = c and rating.effective(c, man), best = best[m] == true }
    end
  end
  return out
end

--- The short text of a cell for a table: "5", "5*" (best), "-" (no such feature), "~" (not tried here), "1" for a fail; a person's own
--- rating is followed by "m" ("4m").
function T.short(info)
  local c = info.cell
  if not c then return "?" end
  if c.status == "SKIP" then return "~" end
  if c.status == "NA" then return "-" end
  local s = tostring(info.eff or "?")
  if info.manual then s = s .. "m" end
  if info.best then s = s .. "*" end
  return s
end

--- A measured value for people: "12.3M ops/s".
function T.value(c)
  if not c or c.value == nil then return nil end
  return model.si(c.value) .. (c.unit and (" " .. c.unit) or "")
end

--- One sentence about a cell for the detail line.
function T.detail(row, member, info)
  local c = info.cell
  if not c then return "not run yet" end
  local bits = {}
  if c.status == "PASS" then bits[#bits + 1] = c.value and (T.value(c)) or "works"
  elseif c.status == "FAIL" then bits[#bits + 1] = "FAIL"
  elseif c.status == "NA" then bits[#bits + 1] = "no such feature"
  else bits[#bits + 1] = "not tried" end
  if c.note and c.note ~= "works" and c.note ~= "measured" and c.status ~= "NA" then bits[#bits + 1] = c.note end
  return table.concat(bits, ": ")
end

--- A group as a Markdown table. Best cells are bold; the rating is out of 5.
function T.markdown(group, results, manual, scores)
  local order = runner.order(group)
  local info = T.cells(group, results, manual)
  local lines = {}
  lines[#lines + 1] = ("### %s"):format(group.title)
  lines[#lines + 1] = ""
  lines[#lines + 1] = group.about
  lines[#lines + 1] = ""
  local head, rule = { "Feature" }, { "---" }
  for _, m in ipairs(group.members) do head[#head + 1] = m.id; rule[#rule + 1] = ":-:" end
  lines[#lines + 1] = "| " .. table.concat(head, " | ") .. " |"
  lines[#lines + 1] = "|" .. table.concat(rule, "|") .. "|"
  for _, row in ipairs(runner.rows(group)) do
    local cells = { row.label }
    for _, m in ipairs(order) do
      local i = info[row.id][m]
      local c = i.cell
      local s
      if not c then s = "?"
      elseif c.status == "SKIP" then s = "~"
      elseif c.status == "NA" then s = "-"
      else
        s = tostring(i.eff or "?")
        if c.status == "PASS" and c.value then s = s .. " (" .. T.value(c) .. ")" end
        if c.status == "FAIL" then s = s .. " (FAIL)" end
        if i.manual then s = s .. " [mine]" end
        if i.best then s = "**" .. s .. "**" end
      end
      cells[#cells + 1] = s
    end
    lines[#lines + 1] = "| " .. table.concat(cells, " | ") .. " |"
  end
  local sc = { "**Overall**" }
  for _, s in ipairs(scores) do
    local t = s.score and string.format("%.2f", s.score) or "~"
    sc[#sc + 1] = s.best and ("**" .. t .. " best**") or t
  end
  lines[#lines + 1] = "| " .. table.concat(sc, " | ") .. " |"
  return table.concat(lines, "\n")
end

return T
