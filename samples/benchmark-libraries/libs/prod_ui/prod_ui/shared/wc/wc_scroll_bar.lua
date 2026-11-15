local context = select(1, ...)

--[[
	Common scroll bar state and functions.

	Notes:

	* This doesn't handle the actual scrolling action of widgets. For that, please see 'core/wid_shared.lua'
	  for shared functions. The default scroll bar drawing function is in shared/impl_scroll_bar1.lua.

	* Most functions here assume that the client widget has the following methods affixed:

	- self:scrollH()         -> widShared.scrollH
	- self:scrollDeltaH()    -> widShared.scrollDeltaH
	- self:scrollV()         -> widShared.scrollV
	- self:scrollDeltaV()    -> widShared.scrollDeltaV
--]]


local wcScrollBar = {}


local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")


local _mt_bar = {}
_mt_bar.__index = _mt_bar


-- Widgets may implement other press_busy codes (ie for dragging the mouse cursor through a menu).
wcScrollBar.press_busy_codes = {
	["v"] = true, -- vertical thumb (or trough->thumb)
	["v1-pend"] = true, -- vertical button-less, pending repeat action
	["v1-cont"] = true, -- vertical button-less, continuous scroll in update()
	["v2-pend"] = true, -- vertical button-more, pending repeat action
	["v2-cont"] = true, -- vertical button-more, continuous scroll in update()

	["h"] = true, -- horizontal thumb (or trough->thumb)
	["h1-cont"] = true, -- horizontal button-less, pending repeat action
	["h1-pend"] = true, -- horizontal button-less, continuous scrollin update()
	["h2-cont"] = true, -- horizontal button-more, pending repeat action
	["h2-pend"] = true, -- horizontal button-more, continuous scrollin update()
}


--[[
	Scroll button modes:
	* "pend-pend": Initial fixed step, followed by repeated fixed steps
	* "pend-cont": Initial fixed step, followed by continuous scrolling
	* "cont": Continuous scrolling

	"cont" has two shortcomings:
	1) Mouse clicks and releases that occur within the same frame (ie low FPS) will not scroll.

	2) It won't interact well with clients that restrict scroll offsets to a low granularity. Given a large
	   enough step, the scroll buttons may not do anything.
--]]


local code_map_v = {
	["pend-pend"] = {
		["b1"] = "v1-pend",
		["b2"] = "v2-pend"
	},

	["pend-cont"] = {
		["b1"] = "v1-pend",
		["b2"] = "v2-pend"
	},

	["cont"] = {
		["b1"] = "v1-cont",
		["b2"] = "v2-cont"
	}
}


local code_map_h = {
	["pend-pend"] = {
		["b1"] = "h1-pend",
		["b2"] = "h2-pend"
	},

	["pend-cont"] = {
		["b1"] = "h1-pend",
		["b2"] = "h2-pend"
	},

	["cont"] = {
		["b1"] = "h1-cont",
		["b2"] = "h2-cont"
	}
}


--- Makes a scroll bar.
-- @param horizontal When true, the scroll bar is aligned horizontally. When false, it's vertical.
-- @param scr_style The scroll bar style table (options and measurements). If not provided, a default will be used.
-- @param [bar] An existing scroll bar table to update.
-- @return The scroll bar table.
function wcScrollBar.newBar(horizontal, scr_style, bar)
	if bar and getmetatable(bar) ~= _mt_bar then
		error("invalid or corrupt scroll bar")
	end

	horizontal = not not horizontal
	scr_style = scr_style or context.resources.scroll_bar_styles["norm"]

	local self = bar or setmetatable({}, _mt_bar)

	self.active = true -- false when hidden
	self.horizontal = horizontal

	self.bar_size = scr_style.bar_size
	self.button_size = scr_style.button_size
	self.thumb_size_min = scr_style.thumb_size_min
	self.thumb_size_max = scr_style.thumb_size_max

	-- Broad scroll bar shape and position within the client widget.
	self.x, self.y, self.w, self.h = 0, 0, 0, 0

	-- Trough shape and position (relative to the XYWH above).
	if scr_style.has_trough then
		self.tr = self.tr or {x=0, y=0, w=0, h=0}
	else
		self.tr = false
	end

	-- Is true when there is space to render the trough.
	self.trough_valid = false

	-- Thumb.
	if scr_style.has_thumb then
		self.th = self.th or {x=0, y=0, w=0, h=0}
	else
		self.th = false
	end

	-- Is false when the thumb is too big for the trough or when the document is >= the viewport.
	-- Click tests and rendering should only consider the thumb when this is true.
	self.thumb_valid = false

	self.button_mode = scr_style.button_mode

	if not scr_style.has_buttons then
		self.b1, self.b2 = false, false
	else
		-- Button 1 is left or up. Button 2 is right or down.
		self.b1_valid = false
		self.b1 = self.b1 or {x=0, y=0, w=0, h=0}

		self.b2_valid = false
		self.b2 = self.b2 or {x=0, y=0, w=0, h=0}
	end

	-- Internal registers used to position and shape the thumb within the trough.
	-- A max of 0 is shorthand for the thumb not currently being usable, either due to
	-- the document being 100% in view, or the smallest allowed thumb exceeding the trough
	-- length.
	self.pos = 0 -- First part of the viewport in the scrollable document.
	self.len = 0 -- Length of the viewport.
	self.max = 0 -- Length of the entire document.

	-- Offset of mouse pointer when clicking+dragging the thumb.
	self.drag_offset = 0

	local sub_style = horizontal and scr_style.h or scr_style.v

	-- Controls where the scroll bar appears (which side of the widget).
	self.near_side = sub_style.near_side

	-- When true, the scroll bar is hidden when the viewport is large enough to show the entire document.
	self.auto_hide = sub_style.auto_hide

	-- Hover and press states:
	self.hover = false -- false, "trough", "thumb", "b1", "b2"
	self.press = false -- false, "trough", "thumb", "b1", "b2"
	-- (press == "trough" is typically converted to press == "thumb".)

	return self
end


-- * Scroll bar methods *


local function _pointInRect(x, y, rx, ry, rw, rh)
	return x >= rx and y >= ry and x < rx + rw and y < ry + rh
end


function _mt_bar:testPoint(px, py)
	-- Broad check
	if _pointInRect(px, py, self.x, self.y, self.w, self.h) then
		local b1, b2, tr, th = self.b1, self.b2, self.tr, self.th

		-- Check buttons ('more' gets priority over 'less'), then thumb, then trough.
		if self.b2 and self.b2_valid and _pointInRect(px, py, self.x + b2.x, self.y + b2.y, b2.w, b2.h) then
			return "b2"

		elseif b1 and self.b1_valid and _pointInRect(px, py, self.x + b1.x, self.y + b1.y, b1.w, b1.h) then
			return "b1"

		elseif th and self.thumb_valid and _pointInRect(px, py, self.x + th.x, self.y + th.y, th.w, th.h) then
			return "thumb"

		elseif tr and self.trough_valid and _pointInRect(px, py, self.x + tr.x, self.y + tr.y, tr.w, tr.h) then
			return "trough"
		end
	end

	return false
end


-- Thumb measurement requires:
-- * The trough to be active
-- * The 'max' register to be greater than zero (shorthand for invalid state)


local function updateThumbH(self)
	local tr, th = self.tr, self.th

	self.thumb_valid = false

	if th and tr and self.trough_valid and self.max > 0 then
		th.w = wcScrollBar.getThumbLength(self.len, self.max, tr.w, self.thumb_size_min, self.thumb_size_max)
		th.x = tr.x + wcScrollBar.getThumbPosition(self.len, self.pos, self.max, tr.w, th.w)
		th.h = tr.h
		th.y = tr.y

		if self.max > 0 and th.w < tr.w then
			self.thumb_valid = true
		end
	end
end


local function updateThumbV(self)
	local tr, th = self.tr, self.th

	self.thumb_valid = false

	if th and tr and self.trough_valid and self.max > 0 then
		th.w = self.w
		th.x = tr.x
		th.h = wcScrollBar.getThumbLength(self.len, self.max, tr.h, self.thumb_size_min, self.thumb_size_max)
		th.y = tr.y + wcScrollBar.getThumbPosition(self.len, self.pos, self.max, tr.h, th.h)

		if self.max > 0 and th.h < tr.h then
			self.thumb_valid = true
		end
	end
end


local function updateShapesH(self)
	self.trough_valid = false
	self.thumb_valid = false

	local b1, b2, tr, th = self.b1, self.b2, self.tr, self.th

	-- Special case: Compress buttons if they are at least as long as the scroll bar shape.
	local button_check = (b1 and self.button_size or 0) + (self.b2 and self.button_size or 0)
	if button_check >= self.w then
		local shortened_length = math.floor(0.5 + self.w*0.5)
		if b1 then
			b1.x, b1.y, b1.w, b1.h = 0, 0, shortened_length, self.h
		end

		if b2 then
			b2.x, b2.y, b2.w, b2.h = self.w - shortened_length, 0, shortened_length, self.h
		end
	-- Normal positioning.
	else
		local measure = 0

		-- Button 1
		if b1 then
			b1.x, b1.y, b1.w, b1.h = 0, 0, self.button_size, self.h

			measure = measure + self.button_size
		end

		-- Trough and thumb
		if tr then
			tr.x, tr.y = measure, 0
			tr.w = self.w - measure
			if b2 then
				tr.w = tr.w - self.button_size
			end
			tr.h = self.h

			measure = measure + tr.w

			self.trough_valid = true

			updateThumbH(self)
		end

		-- Button 2
		if b2 then
			b2.x, b2.y, b2.w, b2.h = measure, 0, self.button_size, self.h
		end
	end

	self.b1_valid = false
	if self.len < self.max and b1 and b1.w > 0 then
		self.b1_valid = true
	end
	self.b2_valid = false
	if self.len < self.max and b2 and b2.w > 0 then
		self.b2_valid = true
	end
end


local function updateShapesV(self)
	self.trough_valid = false
	self.thumb_valid = false

	local b1, b2, tr, th = self.b1, self.b2, self.tr, self.th

	-- Special case: Compress buttons if they are at least as long as the scroll bar shape.
	local button_check = (b1 and self.button_size or 0) + (b2 and self.button_size or 0)
	if button_check >= self.h then
		local shortened_length = math.floor(0.5 + self.h/2)
		if b1 then
			b1.x, b1.y, b1.w, b1.h = 0, 0, self.w, shortened_length
		end

		if b2 then
			b2.x, b2.y, b2.w, b2.h = 0, self.h - shortened_length, self.w, shortened_length
		end
	-- Normal positioning.
	else
		local measure = 0

		-- Button 1
		if b1 then
			b1.x, b1.y, b1.w, b1.h = 0, 0, self.w, self.button_size

			measure = measure + self.button_size
		end

		-- Trough and thumb
		if tr then
			tr.x, tr.y, tr.w, tr.h = 0, measure, self.w, self.h - measure
			if b2 then
				tr.h = tr.h - self.button_size
			end

			measure = measure + tr.h

			self.trough_valid = true

			updateThumbV(self)
		end

		-- Button 2
		if b2 then
			b2.x, b2.y, b2.w, b2.h = 0, measure, self.w, self.button_size
		end
	end

	self.b1_valid = false
	if self.len < self.max and b1 and b1.h > 0 then
		self.b1_valid = true
	end
	self.b2_valid = false
	if self.len < self.max and b2 and b2.h > 0 then
		self.b2_valid = true
	end
end


local methods = {}


--- Method to make, remake or remove embedded scroll bars.
-- @param hori Horizontal bar (self.scr_h)
-- @param vert Vertical bar (self.scr_v)
function methods:setScrollBars(hori, vert)
	local scr_style = self.skin and self.skin.scr_style or nil

	self.scr_h = (hori) and wcScrollBar.newBar(true, scr_style, self.scr_h) or nil
	self.scr_v = (vert) and wcScrollBar.newBar(false, scr_style, self.scr_v) or nil

	-- If there was a state change, reshape the widget after calling.

	return self
end


function wcScrollBar.setupDef(self)
	uiTable.patch(self, methods, false)
end


function wcScrollBar.refreshScrollBars(self)
	local scr_style = self.skin.scr_style

	if self.scr_h then
		self.scr_h = wcScrollBar.newBar(true, scr_style, self.scr_h)
	end

	if self.scr_v then
		self.scr_v = wcScrollBar.newBar(false, scr_style, self.scr_v)
	end

	return self
end


--- Logic for the client's evt_pointerPress(). Detects clicks on embedded scroll bar components and initiates
-- the dragging state. It modifies 'self.press_busy', and state fields within the scroll bar tables.
-- @param self The widget to test and modify.
-- @param x Mouse X position in UI space.
-- @param y Mouse Y position in UI space.
-- @param fixed_step How much to scroll if a button in fixed-step mode is activated.
-- @return True if the scroll is considered activated by the click.
function wcScrollBar.widgetScrollPress(self, x, y, fixed_step)
	-- Don't override existing 'busy' state.
	if self.press_busy then
		return
	end

	-- Check for clicking on scroll bars, and initiate dragging states.
	x, y = self:getRelativePosition(x, y)

	-- Give vertical bar priority in the event of overlap.
	local scr_v = self.scr_v

	if scr_v and scr_v.active then
		local th = scr_v.th
		local test_code = scr_v:testPoint(x, y)

		if test_code then
			scr_v.hover = false
			scr_v.press = test_code

			if test_code == "thumb" then
				self.press_busy = "v"
				scr_v.drag_offset = math.floor(y - th.y)
				--print("NEW DRAG OFFSET (A)", scr_v.drag_offset)
				return true

			elseif test_code == "b1" then
				self.press_busy = code_map_v[scr_v.button_mode][test_code]
				if self.press_busy == "v1-pend" then
					self:scrollDeltaV(-fixed_step)
				end
				return true

			elseif test_code == "b2" then
				self.press_busy = code_map_v[scr_v.button_mode][test_code]
				if self.press_busy == "v2-pend" then
					self:scrollDeltaV(fixed_step)
				end
				return true

			-- Trough
			elseif scr_v.trough_valid then
				-- hand over control to the thumb, but confirm it exists and is validated first.
				if scr_v.th and scr_v.thumb_valid then
					scr_v.press = "thumb"
					self.press_busy = "v"
					scr_v.drag_offset = math.floor(self.vp2.y + th.h / 2)
					--print("NEW DRAG OFFSET (B)", scr_v.drag_offset)
				end

				-- Return positive regardless, to help the client widget with thimble handling.
				return true
			end
		end
	end

	local scr_h = self.scr_h

	if not self.press_busy and scr_h and scr_h.active then
		local th = scr_h.th

		local test_code = scr_h:testPoint(x, y)
		if test_code then
			scr_h.hover = false
			scr_h.press = test_code

			if test_code == "thumb" then
				self.press_busy = "h"
				scr_h.drag_offset = math.floor(x - th.x)
				return true

			elseif test_code == "b1" then
				--self.press_busy = "h1"
				self.press_busy = code_map_h[scr_h.button_mode][test_code]
				if self.press_busy == "h1-pend" then
					self:scrollDeltaH(-fixed_step)
				end
				return true

			elseif test_code == "b2" then
				--self.press_busy = "h2"
				self.press_busy = code_map_h[scr_h.button_mode][test_code]
				if self.press_busy == "h2-pend" then
					self:scrollDeltaH(fixed_step)
				end
				return true

			-- Trough
			elseif scr_h.trough_valid then
				-- hand over control to the thumb, but confirm it exists and is validated first.
				if scr_h.th and scr_h.thumb_valid then
					scr_h.press = "thumb"
					self.press_busy = "h"
					scr_h.drag_offset = math.floor(self.vp2.x + th.w / 2)
				end

				-- Return positive regardless, to help the client widget with thimble handling.
				return true
			end
		end
	end
end


--- Logic for the client's evt_pointerPressRepeat(), which implements repeated 'pend' button motions.
function wcScrollBar.widgetScrollPressRepeat(self, x, y, fixed_step)
	local scr_h, scr_v = self.scr_h, self.scr_v
	local busy_code = self.press_busy

	if scr_v and scr_v.active then
		if busy_code == "v1-pend" then
			if scr_v.b1 then
				if scr_v.button_mode == "pend-cont" then
					self.press_busy = "v1-cont"
				else
					self:scrollDeltaV(-fixed_step)
				end
			end

		elseif busy_code == "v2-pend" then
			if scr_v.b2 then
				if scr_v.button_mode == "pend-cont" then
					self.press_busy = "v2-cont"
				else
					self:scrollDeltaV(fixed_step)
				end
			end
		end
	end

	if scr_h and scr_h.active then
		if busy_code == "h1-pend" then
			if scr_h.b1 then
				if scr_h.button_mode == "pend-cont" then
					self.press_busy = "h1-cont"
				else
					self:scrollDeltaH(-fixed_step)
				end
			end

		elseif busy_code == "h2-pend" then
			if scr_h.b2 then
				if scr_h.button_mode == "pend-cont" then
					self.press_busy = "h2-cont"
				else
					self:scrollDeltaH(fixed_step)
				end
			end
		end
	end
end


function wcScrollBar.widgetProcessHover(self, mx, my)
	local skip = false

	local scr_v = self.scr_v
	if scr_v then
		scr_v.hover = scr_v:testPoint(mx, my)

		if scr_v.hover then
			skip = true
		end
	end

	local scr_h = self.scr_h
	if not skip and scr_h then
		scr_h.hover = scr_h:testPoint(mx, my)
	end
end


--- Logic for 'evt_pointerHoverOff()', which just turns off the hover state.
-- @param self The client widget.
function wcScrollBar.widgetClearHover(self)
	local scr_h = self.scr_h
	if scr_h then
		scr_h.hover = false
	end

	local scr_v = self.scr_v
	if scr_v then
		scr_v.hover = false
	end
end


--- Logic for 'evt_pointerUnpress()', which just turns off the press state.
-- @param self The client widget.
function wcScrollBar.widgetClearPress(self)
	local scr_h = self.scr_h
	if scr_h then
		scr_h.press = false
	end

	local scr_v = self.scr_v
	if scr_v then
		scr_v.press = false
	end
end


--- Logic for 'evt_update()' that controls scrolling while the mouse presses and moves. Some methods and
--  variables for handling scrolling must be present in the widget for this to work.
-- @param self The client widget.
-- @param mx Mouse X, relative to widget top-left.
-- @param my Mouse Y, relative to widget top-left.
-- @param button_step If holding a less/more button, how far it should scroll on this frame.
-- @return true if an action was taken, false if not.
function wcScrollBar.widgetDragLogic(self, mx, my, button_step)
	local mode = self.press_busy

	if mode == "v" then
		local scr_v = self.scr_v
		if scr_v and scr_v.active then
			local th, tr = scr_v.th, scr_v.tr
			if th and tr then
				local scroll_y = wcScrollBar.getDocumentPosition(self.doc_h, tr.h, my - tr.y - scr_v.drag_offset, th.h, self.vp.h)

				self:scrollV(scroll_y, true)
				return true
			end
		end

	elseif mode == "h" then
		local scr_h = self.scr_h
		if scr_h and scr_h.active then
			local th, tr = scr_h.th, scr_h.tr
			if th and tr then
				local scroll_x = wcScrollBar.getDocumentPosition(self.doc_w, tr.w, mx - tr.x - scr_h.drag_offset, th.w, self.vp.w)

				self:scrollH(scroll_x, true)
				return true
			end
		end

	elseif mode == "v1-cont" then
		local scr_v = self.scr_v
		if scr_v and scr_v.active and scr_v.b1 then
			self:scrollDeltaV(-button_step)
			return true
		end

	elseif mode == "v2-cont" then
		local scr_v = self.scr_v
		if scr_v and scr_v.active and scr_v.b2 then
			self:scrollDeltaV(button_step)
			return true
		end

	elseif mode == "h1-cont" then
		local scr_h = self.scr_h
		if scr_h and scr_h.active and scr_h.b1 then
			self:scrollDeltaH(-button_step)
			return true
		end

	elseif mode == "h2-cont" then
		local scr_h = self.scr_h
		if scr_h and scr_h.active and scr_h.b2 then
			self:scrollDeltaH(button_step)
			return true
		end
	end
end


--- Logic for the client that positions scroll bars and implements auto-hide. The client's viewport #1 needs to be
--	set to a default size and position, from which the space will be carved out for the scroll bars.
--	Scroll bar shapes must then be updated.
-- @param self The widget to modify.
function wcScrollBar.arrangeScrollBars(self)
	local vp, scr_h, scr_v = self.vp, self.scr_h, self.scr_v

	-- Determine auto-hide flags for scroll bars.
	-- as*1 -> Content exceeds view on this axis.
	-- as*2 -> Content exceeds reduced view (if the bar on the other axis would be present)
	local asv1, asv2, ash1, ash2

	if scr_v and scr_v.auto_hide then
		asv1 = (self.doc_h > vp.h) and true or false
		asv2 = (self.doc_h > vp.h - scr_v.bar_size) and true or false
	end
	if scr_h and scr_h.auto_hide then
		ash1 = (self.doc_w > vp.w) and true or false
		ash2 = (self.doc_w > vp.w - scr_h.bar_size) and true or false
	end

	if scr_v then
		if scr_v.auto_hide then
			if asv1 or (ash1 and asv2) then
				scr_v.active = true
			else
				scr_v.active = false
			end
		end

		scr_v.w = scr_v.bar_size
		scr_v.h = vp.h

		scr_v.x = scr_v.near_side and vp.x + vp.w - scr_v.w or vp.x
		scr_v.y = vp.y

		-- If active, reduce viewport
		if scr_v.active then
			vp.w = vp.w - scr_v.w
			if not scr_v.near_side then
				vp.x = vp.x + scr_v.w
			end
		end
	end

	if scr_h then
		if scr_h.auto_hide then
			if ash1 or (asv1 and ash2) then
				scr_h.active = true
			else
				scr_h.active = false
			end
		end

		scr_h.w = vp.w
		scr_h.h = scr_h.bar_size

		scr_h.x = vp.x
		scr_h.y = scr_h.near_side and vp.y + vp.h - scr_h.h or vp.y

		-- If active, reduce viewport
		if scr_h.active then
			vp.h = vp.h - scr_h.h
			if not scr_h.near_side then
				vp.y = vp.y + scr_h.h
			end
		end
	end

	-- Make some adjustments when both scroll bars are active.
	if scr_h and scr_h.active and scr_v and scr_v.active then
		scr_v.h = scr_v.h - scr_h.h
		if not scr_h.near_side then
			scr_v.y = scr_v.y + scr_h.h
		end
		-- (The horizontal bar should already be handled).

		--[[
		This leaves a small, non-functional rectangle in the corner where the two scroll bars touch.
		You can put a resize sensor here or another kind of control button. The hover clipping should
		prevent clicking through to child widgets in the content container.
		--]]
	end
end


--- Logic that updates scroll bar component shapes.
function wcScrollBar.updateScrollBarShapes(self)
	local scr_h, scr_v = self.scr_h, self.scr_v
	if scr_h then
		updateShapesH(scr_h)
	end
	if scr_v then
		updateShapesV(scr_v)
	end
end


--- Sets a scroll bar's internal registers: Unworkable numbers cause all values to be set to zero, which is a shorthand
-- for the thumb not being in a valid state.
-- @param scr The scroll bar table.
-- @param pos The first bit of the visible viewport.
-- @param len The length of the viewport on the scrolling axis.
-- @param max The document length, where the upper bound of pos is max - len.
local function updateRegisters(scr, pos, len, max)
	-- This needs to happen after viewport + content resizing.
	if max <= 0 or pos + len > max then
		scr.pos = 0
		scr.len = 0
		scr.max = 0
	else
		scr.pos = pos
		scr.len = len
		scr.max = max
	end
end


--- Updates a widget's built-in scroll registers.
function wcScrollBar.updateScrollState(self)
	local scr_h, scr_v = self.scr_h, self.scr_v

	if scr_v then
		local vp = self.vp
		updateRegisters(scr_v, math.floor(0.5 + vp.y + self.scr_y), vp.h, self.doc_h)
	end
	if scr_h then
		local vp = self.vp
		updateRegisters(scr_h, math.floor(0.5 + vp.x + self.scr_x), vp.w, self.doc_w)
	end
end


-- * Common scroll bar render methods *


function wcScrollBar.drawScrollBarsHV(self, data_scroll)
	local scr_h = self.scr_h
	local scr_v = self.scr_v

	if scr_h and scr_h.active then
		self.impl_scroll_bar.draw(data_scroll, self.scr_h, 0, 0)
	end
	if scr_v and scr_v.active then
		self.impl_scroll_bar.draw(data_scroll, self.scr_v, 0, 0)
	end
end


function wcScrollBar.drawScrollBarsH(self, data_scroll)
	local scr_h = self.scr_h

	if scr_h and scr_h.active then
		self.impl_scroll_bar.draw(data_scroll, self.scr_h, 0, 0)
	end
end


function wcScrollBar.drawScrollBarsV(self, data_scroll)
	local scr_v = self.scr_v

	if scr_v and scr_v.active then
		self.impl_scroll_bar.draw(data_scroll, self.scr_v, 0, 0)
	end
end


-- * Standalone scroll bar calculations *


--[[
Terminology:

Thumb: the movable block that shows where the viewport is located within the document. On systems with mouse cursors,
you can typically (though not always) click and drag the thumb to scroll the viewport. There is usually a minimum
thumb size to ensure that it's easy to see and click, and in some games, it's always the same size. Therefore, don't
count on this being an accurate indication of the document size.

Trough: the area in which the thumb can be moved. For these functions, this excludes any additional buttons on the far
edges.

Document: An axis-aligned rectangle (format: XYWH) that represents the scrollable area, not including margins.
--]]


--- Gets the length of the scroll bar thumb.
-- @param viewport_len Length of the viewport.
-- @param doc_len Length of the scrollable document area. Must be greater than zero.
-- @param trough_len Length of the scroll bar trough.
-- @param thumb_min Minimum permitted thumb size.
-- @param thumb_max Maximum permitted thumb size.
function wcScrollBar.getThumbLength(viewport_len, doc_len, trough_len, thumb_min, thumb_max)
	return math.max(thumb_min, math.min(trough_len, math.min(thumb_max, math.floor(viewport_len / doc_len * trough_len))))
end


--- Gets the document scroll position from the thumb position (or any arbitrary location) within the trough, clamped from 0 to trough length minus thumb length. (If thumb length isn't applicable, pass in zero.)
-- @param doc_len Length of the document's scrollable area.
-- @param trough_len Length of the scroll bar trough.
-- @param pos Position of the thumb (or other arbitrary) position within the trough.
-- @param thumb_len Length of the scroll bar thumb.
-- @param viewport_len Length of the visible content viewport.
-- @return Floored and clamped scroll position in the document that corresponds to this position in the trough.
function wcScrollBar.getDocumentPosition(doc_len, trough_len, pos, thumb_len, viewport_len)
	local doc_shortened = doc_len - viewport_len
	local scroll_shortened = trough_len - thumb_len

	if scroll_shortened <= 0 then -- avoid div/0
		return 0
	end

	return math.max(0, math.min(doc_shortened, math.floor(pos / scroll_shortened * doc_shortened)))
end


--- Gets the scroll bar thumb position from the current scroll offset within the document.
-- @param scroll_pos Scroll offset of the viewport (left or top side) within the document, in the range of 0 to doc_len - thumb_len.
function wcScrollBar.getThumbPosition(viewport_len, scroll_pos, doc_len, trough_len, thumb_len)
	local doc_shortened = doc_len - viewport_len
	local scroll_shortened = trough_len - thumb_len

	if doc_shortened <= 0 then -- avoid div/0
		return 0
	end

	return math.floor(math.max(0, math.min(scroll_shortened, 0.5 + scroll_pos / doc_shortened * scroll_shortened)))
end


-- * Debug *


local function _debugDrawBar(scr)
	if scr and scr.active then
		local b1, b2, tr, th = scr.b1, scr.b2, scr.tr, scr.th

		if tr then
			love.graphics.setColor(1, 1, 1, 0.5)
			love.graphics.rectangle("fill", scr.x + tr.x, scr.y + tr.y, tr.w, tr.h)
		end
		if th then
			love.graphics.setColor(1, 0, 0, 0.5)
			love.graphics.rectangle("fill", scr.x + th.x, scr.y + th.y, th.w, th.h)
		end
		if b1 then
			love.graphics.setColor(0, 1, 0, 0.5)
			love.graphics.rectangle("fill", scr.x + b1.x, scr.y + b1.y, b1.w, b1.h)
		end
		if b2 then
			love.graphics.setColor(0, 0, 1, 0.5)
			love.graphics.rectangle("fill", scr.x + b2.x, scr.y + b2.y, b2.w, b2.h)
		end
	end
end


--- Debug-render code for built-in scroll bars. It assumes that (x0,y0) in the transformation state is the widget's
--  top-left point.
-- @param self The client widget.
function wcScrollBar.debugRender(self)
	local scr_h, scr_v = self.scr_h, self.scr_v

	_debugDrawBar(self.scr_h)
	_debugDrawBar(self.scr_v)
end


return wcScrollBar
