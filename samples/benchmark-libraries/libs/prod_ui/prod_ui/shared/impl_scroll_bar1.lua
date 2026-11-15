-- ProdUI: Scroll Bar implementation #1.


local implScrollBar1 = {}


local context = select(1, ...)


local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")



function implScrollBar1.draw(t_data, scr, x, y)
	local shared = t_data.shared

	local tbl_idle = shared.idle
	local tbl_hover = shared.hover
	local tbl_press = shared.press
	local tbl_disabled = shared.disabled

	local xx, yy = x + scr.x, y + scr.y

	local tquad_pixel = t_data.tquad_pixel

	local b1, b2, tr, th = scr.b1, scr.b2, scr.tr, scr.th

	-- Optional body fill. May be useful if there are gaps between the trough and buttons.
	if t_data.render_body then
		love.graphics.setColor(t_data.body_color)
		uiGraphics.quad1x1(tquad_pixel, xx, yy, scr.w, scr.h)
	end

	-- Trough
	if tr and scr.trough_valid then
		love.graphics.setColor(t_data.col_trough)
		uiGraphics.quad1x1(tquad_pixel, xx + tr.x, yy + tr.y, tr.w, tr.h)
	end

	-- Thumb
	if th and scr.thumb_valid then
		local tbl = (scr.press == "thumb") and tbl_press or (scr.hover == "thumb") and tbl_hover or tbl_idle

		local slc_thumb = tbl.slice
		love.graphics.setColor(tbl.col_body)
		uiGraphics.drawSlice(slc_thumb, xx + th.x, yy + th.y, th.w, th.h)
	end

	-- Buttons
	if b1 then
		local tbl = (not scr.b1_valid) and tbl_disabled
			or (scr.press == "b1") and tbl_press
			or (scr.hover == "b1") and tbl_hover
			or tbl_idle

		local slc_button = tbl.slice
		love.graphics.setColor(tbl.col_body)
		uiGraphics.drawSlice(slc_button, xx + b1.x, yy + b1.y, b1.w, b1.h)

		local tq_arrow = scr.horizontal and t_data.tq_arrow_left or t_data.tq_arrow_up
		local breadth = scr.horizontal and b1.h or b1.w
		local pad = math.floor(0.5 + breadth * 0.25)

		love.graphics.setColor(tbl.col_symbol)
		uiGraphics.quadXYWH(
			tq_arrow,
			xx + b1.x + pad,
			yy + b1.y + pad,
			math.max(0, b1.w - pad*2),
			math.max(0, b1.h - pad*2)
		)
	end

	if b2 then
		local tbl = (not scr.b1_valid) and tbl_disabled
			or (scr.press == "b2") and tbl_press
			or (scr.hover == "b2") and tbl_hover
			or tbl_idle

		local slc_button = tbl.slice
		love.graphics.setColor(tbl.col_body)
		uiGraphics.drawSlice(slc_button, xx + b2.x, yy + b2.y, b2.w, b2.h)

		local tq_arrow = scr.horizontal and t_data.tq_arrow_right or t_data.tq_arrow_down
		local breadth = scr.horizontal and b2.h or b2.w
		local pad = math.floor(0.5 + breadth * 0.25)

		love.graphics.setColor(tbl.col_symbol)
		uiGraphics.quadXYWH(
			tq_arrow,
			xx + b2.x + pad,
			yy + b2.y + pad,
			math.max(0, b2.w - pad*2),
			math.max(0, b2.h - pad*2)
		)
	end
end


return implScrollBar1
