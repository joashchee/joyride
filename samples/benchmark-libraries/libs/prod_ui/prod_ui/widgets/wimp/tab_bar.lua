-- Work in progress.


--[[
A horizontal bar of tabs, which represents a set of resources that are being viewed
or edited by the user.

One tab may be selected at a time.

 The selected tab
        │
┌───────┴───────┐
┌───┬═════════════┬─────────────┬─────────────┬───┬───┐
│ < │ [I] Title1  │ [I] Title2  │ [I] Title3  │ > │ v │ <-- Optional menu of tabs
└───┴═════════════┴─────────────┴─────────────┴───┴───┘
  ^    ^                                        ^
  |    |                                        Optional scroll button
  |    Optional icons
  Optional scroll button
--]]


local context = select(1, ...)


local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiDummy = require(context.conf.prod_ui_req .. "ui_dummy")
local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")
local uiScale = require(context.conf.prod_ui_req .. "ui_scale")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")
local wcIconsAndText = context:getLua("shared/wc/wc_icons_and_text")
local wcMenu = context:getLua("shared/wc/wc_menu")
local widShared = context:getLua("core/wid_shared")


local _nm_scroll_button_mode = uiTable.newNamedMapV("TabScrollButtonMode", "off", "on", "conditional")
-- ^ or false to use the skin default


local def = {
	skin_id = "tab_bar1",

	user_callbacks = uiTable.newLutV(
		"cb_action",
		"cb_action2",
		"cb_action3",
		"cb_select",
		"cb_dropped"
	)
}


wcIconsAndText.attachMethods(def)
wcMenu.attachMenuMethods(def)
widShared.scrollSetMethods(def)


local _arrange_tb = wcMenu.arrangers["list-lr"]
function def:arrangeItems(first, last)
	_arrange_tb(self, self.vp, true, first, last, 0, 0)
end


-- * Scroll helpers *


def.getInBounds = wcMenu.getItemInBoundsX
def.selectionInView = wcMenu.selectionInView


-- * Spatial selection *


def.getItemAtPoint = wcMenu.widgetGetItemAtPointH -- (self, px, py, first, last)
def.trySelectItemAtPoint = wcMenu.widgetTrySelectItemAtPoint -- (self, x, y, first, last)


-- * Selection movement *


def.movePrev = wcMenu.widgetMovePrev
def.moveNext = wcMenu.widgetMoveNext
def.moveFirst = wcMenu.widgetMoveFirst
def.moveLast = wcMenu.widgetMoveLast


-- Widget:cb_action(item, item_i)
-- Called when user left-clicks on the widget.
-- @param item The current selected item, or nil if no item is selected.
-- @param item_i Index of the current selected item, or zero if no item is selected.
def.cb_action = uiDummy.func


-- Widget:cb_action2(item, item_i)
-- Called when the user right-clicks on the widget.
-- @param item The current selected item, or nil if no item is selected.
-- @param item_i Index of the current selected item, or zero if no item is selected.
def.cb_action2 = uiDummy.func


-- Widget:cb_action3(item, item_i)
-- Called when the user middle-clicks on the widget.
-- @param item The current selected item, or nil if no item is selected.
-- @param item_i Index of the current selected item, or zero if no item is selected.
def.cb_action3 = uiDummy.func


-- Widget:cb_select(item, item_i)
-- Called when there is a change in the selected item.
-- @param item The current selected item, or nil if no item is selected.
-- @param item_i Index of the current selected item, or zero if no item is selected.
def.cb_select = uiDummy.func
-- XXX This may not be firing when going from a selected item to nothing selected.


-- Widget:cb_dropped(drop_state)
-- Called when the mouse drags and drops something onto this widget.
-- @param drop_state A DropState table that describes the nature of the drag-and-drop action.
-- @return true to clear the DropState and to stop event bubbling.
def.cb_dropped = uiDummy.func


function def:setTabWidth(min, max)
	uiAssert.numberGeEval(1, min, 0)
	uiAssert.numberGeEval(2, max, 0)

	self.tab_w_min = min or false
	self.tab_w_max = max or self.tab_w_min

	return self
end


function def:getTabWidth()
	return self.tab_w_min, self.tab_w_max
end


function def:setScrollButtonMode(mode)
	uiAssert.namedMapEval(1, mode, _nm_scroll_button_mode)

	self.sb_mode = mode

	return self
end


function def:getScrollButtonMode()
	return self.sb_mode
end


local function _shapeItem(self, item)
	local skin = self.skin
	local font = skin.font

	local w = skin.pad_text_x1 + skin.pad_text_x2

	local tab_min = self.tab_w_min or 0
	local tab_max = self.tab_w_max or math.huge
	if tab_min == tab_max then
		w = w + tab_min
	else
		w = w + math.max(tab_min, math.min(tab_max, font:getWidth(item.text)))
	end

	item.w = w
	item.h = self.vp.h
end


function def:addItem(text, pos, icon_id)
	uiAssert.type(1, text, "string")
	uiAssert.integerEval(2, pos, "number")
	uiAssert.typeEval(3, icon_id, "string")

	local items = self.MN_items

	local item = {}

	item.selectable = true
	item.marked = false -- multi-select

	item.text = text
	item.icon_id = icon_id
	item.tq_icon = wcIconsAndText.getIconQuad(self.icon_set_id, item.icon_id) or false

	item.x, item.y = 0, 0
	_shapeItem(self, item)

	pos = pos or #items + 1

	if pos < 1 or pos > #items + 1 then
		error("position is out of range")
	end

	table.insert(items, pos, item)

	self:arrangeItems(pos, #items)

	return item
end


function def:removeItem(item_t)
	uiAssert.type(1, item_t, "table")

	local item_i = self:menuGetItemIndex(item_t)

	local removed_item = self:removeItemByIndex(item_i)
	return removed_item
end


function def:removeItemByIndex(item_i)
	uiAssert.numberNotNan(1, item_i)

	local items = self.MN_items
	local removed_item = items[item_i]
	if not removed_item then
		error("no item to remove at index: " .. tostring(item_i))
	end

	table.remove(items, item_i)

	wcMenu.removeItemIndexCleanup(self, item_i)

	self:arrangeItems(item_i, #items)

	return removed_item
end


function def:setSelection(item_t)
	uiAssert.type(1, item_t, "table")

	local item_i = self:menuGetItemIndex(item_t)
	self:setSelectionByIndex(item_i)
end


function def:setSelectionByIndex(item_i)
	uiAssert.integerGe(1, item_i, 0)

	local old_index = self.MN_index
	self:menuSetSelectedIndex(item_i)
	if old_index ~= self.MN_index then
		self:cb_select(self.MN_items[self.MN_index], self.MN_index)
	end
	self:selectionInView()
end


function def:evt_initialize()
	self.visible = true
	self.allow_hover = true
	self.thimble_mode = 0

	widShared.setupDoc(self)
	widShared.setupScroll(self, -1, -1)
	widShared.setupViewports(self, 6)

	self.tab_w_min = false
	self.tab_w_max = false

	-- sb == scroll button.
	-- Scroll buttons are considered "off" when their associated viewports are zero width.
	self.sb_mode = false -- _nm_scroll_button_mode

	self.button_hover = false -- false, "less", "more"
	self.button_press = false -- false, "less", "more"

	self.press_busy = false

	wcIconsAndText.setupInstance(self)
	wcMenu.setup(self, nil, true, true) -- with mark and drag+drop state

	self.MN_wrap_selection = false

	-- State flags.
	self.enabled = true

	self:skinSetRefs()
	self:skinInstall()
end


function def:evt_getGrowAxisLength(x_axis, cross_length)
	if not x_axis then
		if self.hidden then
			return 0, false
		else
			local skin = self.skin
			return math.floor(skin.font:getHeight() * skin.height_pad_scale), false
		end
	end
end


function def:evt_reshapePre()
	-- Viewport #1 is the main tab area.
	-- Viewport #2 is the region which accepts mouse clicks.
	-- Viewport #3 is the "scroll less" button area.
	-- Viewport #4 is the "scroll more" button area.
	-- Viewport #5: "scroll less" arrow space.
	-- Viewport #6: "scroll more" arrow space.

	local skin = self.skin
	local vp, vp2, vp3, vp4, vp5, vp6 = self.vp, self.vp2, self.vp3, self.vp4, self.vp5, self.vp6
	local box, box_button = skin.box, skin.box_button

	vp:set(0, 0, self.w, self.h)
	vp:reduceT(box.border)

	-- 'Okay-to-click' rectangle.
	vp:copy(vp2)
	vp:reduceT(box.margin)

	for i, item in ipairs(self.MN_items) do
		_shapeItem(self, item)
	end

	self:arrangeItems()

	local sb_size = skin.scroll_button_size
	local scroll_overflow
	local last_item = self.MN_items[#self.MN_items]
	if last_item and last_item.x + last_item.w > vp.w - sb_size*2 then
		scroll_overflow = true
	end

	local sb_mode = self.sb_mode or skin.default_scroll_button_mode

	if sb_mode == "on" or (scroll_overflow and sb_mode == "conditional") then
		vp:split(vp3, "left", sb_size)
		vp:split(vp4, "right", sb_size)

		vp3:copy(vp5)
		vp5:reduceT(box_button.border)
		vp5:reduceT(box_button.margin)

		vp4:copy(vp6)
		vp6:reduceT(box_button.border)
		vp6:reduceT(box_button.margin)
	else
		vp3:set(0, 0, 0, 0)
		vp4:set(0, 0, 0, 0)
		vp5:set(0, 0, 0, 0)
		vp6:set(0, 0, 0, 0)
	end

	self:scrollClampViewport()
	self:selectionInView()

	self:cacheUpdate(true)
end


--- Updates cached display state.
function def:cacheUpdate(refresh_dimensions)
	if refresh_dimensions then
		self.doc_w, self.doc_h = wcMenu.getCombinedItemDimensions(self.MN_items)
	end
end


-- Tab bars cannot hold the thimble, so they don't get keyboard input events directly.
--function def:evt_keyPressed(targ, key, scancode, isrepeat)


function def:evt_pointerHover(targ, mouse_x, mouse_y, mouse_dx, mouse_dy)
	if self == targ then
		local mx, my = self:getRelativePosition(mouse_x, mouse_y)

		local hover_ok = false
		self.button_hover = false

		if self.vp3:pointOverlap(mx, my) then
			self.button_hover = "less"

		elseif self.vp4:pointOverlap(mx, my) then
			self.button_hover = "more"

		elseif self.vp2:pointOverlap(mx, my) then
			mx, my = mx + self.scr_x, my + self.scr_y

			-- Update item hover
			local i, item = self:getItemAtPoint(mx, my, math.max(1, self.MN_items_first), math.min(#self.MN_items, self.MN_items_last))

			if item and item.selectable then
				-- Un-hover any existing hovered item
				self.MN_item_hover = item

				hover_ok = true
			end
		end

		if self.MN_item_hover and not hover_ok then
			self.MN_item_hover = false
		end
	end
end


function def:evt_pointerHoverOff(targ, mouse_x, mouse_y, mouse_dx, mouse_dy)
	if self == targ then
		self.MN_item_hover = false

		self.button_hover = false
	end
end


function def:evt_pointerPress(targ, x, y, button, istouch, presses)
	if self == targ
	and self.enabled
	and button == context.mouse_pressed_button
	then
		local mx, my = self:getRelativePosition(x, y)

		if self.vp2:pointOverlap(mx, my) then
			if self.vp3:pointOverlap(mx, my) then
				self.button_press = "less"

			elseif self.vp4:pointOverlap(mx, my) then
				self.button_press = "more"

			else
				mx = mx + self.scr_x
				my = my + self.scr_y

				local item_i, item_t = wcMenu.checkItemIntersect(self, mx, my, button)

				if item_t and item_t.selectable then
					local old_index = self.MN_index
					local old_item = self.MN_items[old_index]

					-- Buttons 1, 2 and 3 all select an item.
					-- Only button 1 updates the item mark state.
					if button <= 3 then
						wcMenu.widgetSelectItemByIndex(self, item_i)
						self.MN_mouse_clicked_item = item_t

						if button == 1 then
							wcMenu.pointerPressButton1(self, item_t, old_index)
						end

						if old_item ~= item_t then
							self:cb_select(item_t, item_i)
						end
					end

					-- Clicking Button 1 invokes action 1.
					if button == 1 then
						self.press_busy = "menu-drag"
						self:cb_action(item_t, item_i)

					-- Button 2 clicks invoke action 2.
					elseif button == 2 then
						self:cb_action2(item_t, item_i)

					-- Button 3 -> action 3...
					elseif button == 3 then
						self:cb_action3(item_t, item_i)
					end
				end
			end
		end
	end
end


function def:evt_pointerDrag(targ, mouse_x, mouse_y, mouse_dx, mouse_dy)
	if self == targ and self.press_busy == "menu-drag" then
		wcMenu.menuPointerDragLogic(self, mouse_x, mouse_y)
	end
end


function def:evt_pointerUnpress(targ, x, y, button, istouch, presses)
	if self == targ
	and self.enabled
	and button == context.mouse_pressed_button
	then
		self.press_busy = false

		self.button_press = false
	end
end


function def:evt_pointerWheel(targ, x, y)
	if self == targ then
		-- Direct Y motions to horizontal scroll.
		-- TODO: maybe there should be a separate function for this...
		if widShared.checkScrollWheelScroll(self, y, 0) then
			self:cacheUpdate(false)

			return true -- Stop bubbling.
		end
	end
end


function def:evt_pointerDragDestRelease(targ, x, y, button, istouch, presses)
	if self == targ then
		return wcMenu.dragDropReleaseLogic(self)
	end
end


function def:evt_update(dt)
	dt = math.min(dt, 1.0)

	local scr_x_old, scr_y_old = self.scr_x, self.scr_y
	local needs_update = false

	-- Clear click-sequence item.
	if self.MN_mouse_clicked_item and context.cseq_widget ~= self then
		self.MN_mouse_clicked_item = false
	end

	-- TODO: rate scaling, better integration with existing scroll code...
	if self.button_press == "less" then
		self:scrollDeltaH(-dt * 256)

	elseif self.button_press == "more" then
		self:scrollDeltaH(dt * 256)
	end

	-- Handle update-time drag-scroll.
	if self.MN_drag_scroll
	and self.press_busy == "menu-drag"
	and widShared.dragToScroll(self, dt)
	then
		needs_update = true
	end

	self:scrollUpdate(dt)

	-- Force a cache update if the external scroll position is different.
	if scr_x_old ~= self.scr_x or scr_y_old ~= self.scr_y then
		needs_update = true
	end

	if needs_update then
		self:cacheUpdate(false)
	end
end


function def:evt_destroy(targ)
	if self == targ then
		widShared.removeViewports(self, 6)
	end
end


local themeAssert = context:getLua("core/res/theme_assert")


def.default_skinner = {
	--validate = uiSchema.newKeysX { -- TODO
	--transform = function(scale, skin) -- TODO


	install = function(self, skinner, skin)
		uiTheme.skinnerCopyMethods(self, skinner)

		-- Update shapes, positions, and icons of any existing items
		for i, item in ipairs(self.MN_items) do
			item.tq_icon = wcIconsAndText.getIconQuad(self.icon_set_id, item.icon_id)
			_shapeItem(self, item)
		end

		self:arrangeItems()
	end,


	remove = function(self, skinner, skin)
		uiTheme.skinnerClearData(self)
	end,


	--refresh = function(self, skinner, skin)
	--update = function(self, skinner, skin, dt)


	render = function(self, ox, oy)
		local skin = self.skin
		local vp, vp2, vp3, vp4, vp5, vp6 = self.vp, self.vp2, self.vp3, self.vp4, self.vp5, self.vp6
		local items = self.MN_items

		love.graphics.push("all")

		love.graphics.setColor(skin.color_body)
		uiGraphics.drawSlice(skin.sl_body, 0, 0, self.w, self.h)

		if vp3.w > 0 then
			local res = self.button_press == "less" and skin.res_button_pressed
				or self.button_hover == "less" and skin.res_button_hovered
				or skin.res_button_idle

			love.graphics.setColor(res.color_body)
			uiGraphics.drawSlice(res.slice, vp3.x, vp3.y, vp3.w, vp3.h)

			love.graphics.setColor(res.color_icon)
			uiGraphics.quadShrinkOrCenterSmallestAxisXYWH(res.tq_less, vp5.x + res.ox, vp5.y + res.oy, vp5.w, vp5.h)
		end

		if vp4.w > 0 then
			local res = self.button_press == "more" and skin.res_button_pressed
				or self.button_hover == "more" and skin.res_button_hovered
				or skin.res_button_idle

			love.graphics.setColor(res.color_body)
			uiGraphics.drawSlice(res.slice, vp4.x, vp4.y, vp4.w, vp4.h)

			love.graphics.setColor(res.color_icon)
			uiGraphics.quadShrinkOrCenterSmallestAxisXYWH(res.tq_more, vp6.x + res.ox, vp6.y + res.oy, vp6.w, vp6.h)
		end

		local scis_x, scis_y = ox + self.x + vp.x, oy + self.y + vp.y
		uiGraphics.intersectScissor(scis_x, scis_y, vp.w, vp.h)

		love.graphics.translate(-self.scr_x, -self.scr_y)

		local first = math.max(self.MN_items_first, 1)
		local last = math.min(self.MN_items_last, #items)

		local text_y = math.floor((vp.h - skin.font:getHeight()) * 0.5)
		for i = first, last do
			local item = items[i]

			love.graphics.push("all")

			local res = (i == self.MN_index) and skin.res_tab_selected
				or item == self.MN_item_hover and skin.res_tab_hovered
				or skin.res_tab_idle

			love.graphics.setColor(res.color_body)
			uiGraphics.drawSlice(res.slice, item.x, item.y, item.w, item.h)

			--uiGraphics.intersectScissor(scis_x - self.scr_x + item.x, scis_y - self.scr_y + item.y, item.w, item.h)
			love.graphics.setColor(res.color_text)
			love.graphics.setFont(skin.font)
			love.graphics.print(item.text, item.x + skin.pad_text_x1, item.y + text_y)

			love.graphics.pop()
		end

		love.graphics.pop()
	end,


	--renderLast = function(self, ox, oy) end,


	renderThimble = wcMenu.renderThimbleNoSelection
}


return def
