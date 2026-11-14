-- A 48 by 25 grid of text cells, 8 by 16 pixels each, in the 16 VGA colours (the ANSIapps look). Everything the app shows is put in
-- this grid first, so a test can read the screen as text; app.lua draws it with the VGA font. No LÖVE calls here.

local S = {}

S.COLS, S.ROWS, S.CW, S.CH = 48, 25, 8, 16
S.W, S.H = S.COLS * S.CW, S.ROWS * S.CH

-- The 16 VGA colours as 0..1 RGB.
S.RGB = {
  [0] = { 0, 0, 0 }, { 0, 0, 2 / 3 }, { 0, 2 / 3, 0 }, { 0, 2 / 3, 2 / 3 }, { 2 / 3, 0, 0 }, { 2 / 3, 0, 2 / 3 }, { 2 / 3, 1 / 3, 0 }, { 2 / 3, 2 / 3, 2 / 3 },
  { 1 / 3, 1 / 3, 1 / 3 }, { 1 / 3, 1 / 3, 1 }, { 1 / 3, 1, 1 / 3 }, { 1 / 3, 1, 1 }, { 1, 1 / 3, 1 / 3 }, { 1, 1 / 3, 1 }, { 1, 1, 1 / 3 }, { 1, 1, 1 },
}

-- Named foreground/background pairs, each with good contrast.
S.ST = {
  body = { 7, 0 }, strong = { 15, 0 }, hint = { 11, 0 }, ok = { 10, 0 }, warn = { 14, 0 }, error = { 12, 0 }, dim = { 8, 0 },
  bar = { 0, 7 }, title = { 15, 1 }, sel = { 0, 7 }, key = { 0, 3 }, keydown = { 15, 2 }, frame = { 9, 0 }, info = { 11, 0 },
}

local ASCII_BOX = { ["─"] = "-", ["│"] = "|", ["┌"] = "+", ["┐"] = "+", ["└"] = "+", ["┘"] = "+", ["═"] = "=", ["║"] = "|", ["╔"] = "+", ["╗"] = "+", ["╚"] = "+", ["╝"] = "+", ["█"] = "#", ["░"] = ".", ["▒"] = ":", ["▓"] = "#", ["…"] = "~", ["·"] = "." }

--- Split a UTF-8 string into characters (LuaJIT has no utf8 library).
function S.chars(str)
  local out = {}
  for ch in tostring(str):gmatch("[%z\1-\127\194-\244][\128-\191]*") do out[#out + 1] = ch end
  return out
end

function S.new()
  local scr = { ascii = false, cells = {} }
  function scr:clear(st)
    st = st or S.ST.body
    for y = 0, S.ROWS - 1 do
      local row = {}
      for x = 0, S.COLS - 1 do row[x] = { " ", st[1], st[2] } end
      self.cells[y] = row
    end
  end
  --- Put text at a cell; stops at the right edge. Returns the next column.
  function scr:text(x, y, str, st)
    st = st or S.ST.body
    if y < 0 or y >= S.ROWS then return x end
    for _, ch in ipairs(S.chars(str)) do
      if x >= S.COLS then break end
      if x >= 0 then
        if self.ascii and ASCII_BOX[ch] then ch = ASCII_BOX[ch] end
        local c = self.cells[y][x]
        c[1], c[2], c[3] = ch, st[1], st[2]
      end
      x = x + 1
    end
    return x
  end
  function scr:fill(x, y, w, h, st, ch)
    for yy = y, y + h - 1 do for xx = x, x + w - 1 do self:text(xx, yy, ch or " ", st or S.ST.body) end end
  end
  --- A frame with an optional title; the inside is cleared.
  function scr:box(x, y, w, h, title, st, double)
    st = st or S.ST.frame
    local c = double and { "╔", "╗", "╚", "╝", "═", "║" } or { "┌", "┐", "└", "┘", "─", "│" }
    self:fill(x, y, w, h, S.ST.body)
    self:text(x, y, c[1] .. string.rep(c[5], w - 2) .. c[2], st)
    self:text(x, y + h - 1, c[3] .. string.rep(c[5], w - 2) .. c[4], st)
    for yy = y + 1, y + h - 2 do self:text(x, yy, c[6], st); self:text(x + w - 1, yy, c[6], st) end
    if title and title ~= "" then self:text(x + 2, y, " " .. title .. " ", S.ST.strong) end
  end
  --- One row of the screen as plain text (for tests).
  function scr:row(y)
    local t = {}
    for x = 0, S.COLS - 1 do t[#t + 1] = self.cells[y][x][1] end
    return table.concat(t)
  end
  function scr:dump()
    local t = {}
    for y = 0, S.ROWS - 1 do t[#t + 1] = (self:row(y):gsub("%s+$", "")) end
    return table.concat(t, "\n")
  end
  scr:clear()
  return scr
end

--- Cut a string to n characters, with an ellipsis when it was cut.
function S.fit(str, n)
  local ch = S.chars(str)
  if #ch <= n then return str end
  if n <= 1 then return table.concat(ch, "", 1, n) end
  return table.concat(ch, "", 1, n - 1) .. "…"
end

--- Pad or cut to exactly n characters.
function S.pad(str, n)
  local ch = S.chars(str)
  if #ch >= n then return table.concat(ch, "", 1, n) end
  return str .. string.rep(" ", n - #ch)
end

--- Word-wrap text into lines of at most w characters.
function S.wrap(str, w)
  local out = {}
  for para in (tostring(str) .. "\n"):gmatch("(.-)\n") do
    local line = ""
    for word in para:gmatch("%S+") do
      if #line == 0 then line = word
      elseif #S.chars(line) + 1 + #S.chars(word) <= w then line = line .. " " .. word
      else out[#out + 1] = line; line = word end
      while #S.chars(line) > w do
        out[#out + 1] = table.concat(S.chars(line), "", 1, w)
        line = table.concat(S.chars(line), "", w + 1)
      end
    end
    out[#out + 1] = line
  end
  return out
end

-- ---- widgets ------------------------------------------------------------------------------------------------------------------------

--- A framed list. items = { { label, value, style } }. Shows the page the selection is on; returns how many rows per page.
--- opts = { x, y, w, h, title, items, sel, foot }
function S.list(scr, o)
  scr:box(o.x, o.y, o.w, o.h, o.title, S.ST.frame, true)
  local per = o.h - 2
  local page = math.floor((o.sel - 1) / per)
  local pages = math.max(1, math.ceil(#o.items / per))
  for i = 1, per do
    local idx = page * per + i
    local it = o.items[idx]
    if not it then break end
    local sel = idx == o.sel
    local st = sel and S.ST.sel or (it.style or S.ST.body)
    local inner = o.w - 4
    local value = it.value and tostring(it.value) or ""
    local vw = #S.chars(value)
    local lw = inner - (vw > 0 and vw + 1 or 0)
    scr:text(o.x + 1, o.y + i, S.pad(" " .. S.fit(it.label, lw - 1), inner + 2), st)
    if vw > 0 then scr:text(o.x + o.w - 2 - vw, o.y + i, value .. " ", sel and S.ST.sel or (it.vstyle or st)) end
  end
  local foot = o.foot or ""
  if pages > 1 then foot = ("Page %d/%d  "):format(page + 1, pages) .. foot end
  if foot ~= "" then scr:text(o.x + o.w - 2 - #S.chars(foot) - 1, o.y + o.h - 1, " " .. foot .. " ", S.ST.dim) end
  return per
end

--- Paged plain lines inside a frame. lines = { { text, style } or "text" }. Returns the page count.
function S.lines(scr, o)
  scr:box(o.x, o.y, o.w, o.h, o.title, S.ST.frame, o.double)
  local per = o.h - 2
  local pages = math.max(1, math.ceil(#o.lines / per))
  local page = math.min(o.page or 0, pages - 1)
  for i = 1, per do
    local ln = o.lines[page * per + i]
    if not ln then break end
    if type(ln) == "string" then ln = { ln } end
    scr:text(o.x + 2, o.y + i, S.fit(ln[1], o.w - 4), ln[2] or S.ST.body)
  end
  if pages > 1 then
    local foot = (" Page %d/%d "):format(page + 1, pages)
    scr:text(o.x + o.w - 2 - #foot, o.y + o.h - 1, foot, S.ST.dim)
  end
  return pages, per
end

--- The selection after an action: up and down wrap, a page up or down stops at the ends.
function S.nav(sel, count, per, a)
  if count <= 0 then return 1 end
  if a == "up" then return (sel - 2) % count + 1 end
  if a == "down" then return sel % count + 1 end
  if a == "pgup" then return math.max(1, sel - per) end
  if a == "pgdn" then return math.min(count, sel + per) end
  return sel
end

--- "[=====|     ]" bar for an axis value in -1..1, w characters inside the brackets.
function S.axisBar(v, w)
  local pos = math.floor((math.max(-1, math.min(1, v)) + 1) / 2 * (w - 1) + 0.5)
  local mid = math.floor((w - 1) / 2)
  local t = {}
  for i = 0, w - 1 do
    if i == pos then t[#t + 1] = "█"
    elseif i == mid then t[#t + 1] = "│"
    else t[#t + 1] = "─" end
  end
  return "[" .. table.concat(t) .. "]"
end

return S
