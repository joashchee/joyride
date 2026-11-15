-- Under construction.


--[[
A collapsible container. Click on the label bar to expand or collapse.
Does not scroll, except to offset children against viewport 1.


Expanded:

┌─────────────────────┐
│ V Collapsible       │
├─────────────────────┤
│ Some label text     │
│ [X] Checkbox        │
│ [Text input|      ] │
└─────────────────────┘


Collapsed:

┌─────────────────────┐
│ > Collapsible       │
└─────────────────────┘
--]]


local context = select(1, ...)


local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")
local uiScale = require(context.conf.prod_ui_req .. "ui_scale")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")
local wcPipe = context:getLua("shared/wc/wc_pipe")
local widLayout = context:getLua("core/wid_layout")
local widShared = context:getLua("core/wid_shared")


local def = {
	skin_id = "container_collapsible1",
}


local function _getResourceSet(self, skin)
	if not self.enabled then
		return skin.res_disabled

	elseif self.press_busy == "label-bar" then
		return skin.res_pressed

	elseif self.label_bar_hover then
		return skin.res_hovered

	else
		return skin.res_idle
	end
end


local _nm_title_label_side = uiTable.newNamedMapV("TitleLabelSide", "left", "center", "right")


widLayout.setupContainerDef(def)


function def:setLabelSide(side)
	uiAssert.namedMapEval(1, side, _nm_title_label_side)

	if uiTable.setDouble(self, "S_label_side", "label_side", side, self.skin.default_label_side) then
		self:reshape()
	end

	return self
end


function def:getLabelSide()
	return self.S_label_side
end


function def:setText(text)
	uiAssert.type(1, text, "string")

	self.text = text

	self:reshape()

	return self
end


function def:getText()
	return self.text
end


function def:setExpanded(enabled)
	local old_value = not not self.expanded
	self.expanded = not not enabled

	self.block_child_thimble = not self.expanded
	if self.block_child_thimble then
		if context.thimble1 and context.thimble1:nodeHasThisAncestor(self) then
			local wid = context.thimble1:getOpenThimble1Ancestors()
			if wid then
				wid:takeThimble1()
			else
				context:releaseThimble1()
			end
		end
		if context.thimble2 and context.thimble2.nodeHasThisAncestor(self) then
			context:releaseThimble2()
		end
	end

	self.block_child_draw = not self.expanded
	self.block_child_hover = not self.expanded

	if old_value ~= enabled then
		self:reshapeGrowing()
	end
	return self
end


function def:getExpanded()
	return self.expanded
end


function def:evt_initialize()
	self.visible = true
	self.block_child_draw = false
	self.block_child_hover = false
	self.block_child_thimble = false
	self.allow_hover = true
	self.thimble_mode = 1

	widShared.setupViewports(self, 5)
	widLayout.setupLayoutList(self)

	self.S_label_side = false

	self.text = ""
	self.enabled = true -- TODO

	self.label_bar_hover = false

	self.expanded = true

	self.label_side = "center"

	self.press_busy = false -- false, "label-bar"

	self:layoutSetBase("viewport")

	self.thimble_x, self.thimble_y, self.thimble_w, self.thimble_h = 0, 0, 0, 0

	self:skinSetRefs()
	self:skinInstall()
end


local function _getLabelBarHeight(self)
	local skin = self.skin
	local font = skin.font
	--print("font:getHeight()", font:getHeight())
	--print("pad_bar_y1|2", skin.pad_bar_y1, skin.pad_bar_y2)

	return font:getHeight() + skin.pad_bar_y1 + skin.pad_bar_y2
end


function def:evt_getGrowAxisLength(x_axis, cross_length)
	if not x_axis then
		local scale = context.scale
		local skin = self.skin
		local font = skin.font

		local h = _getLabelBarHeight(self)

		if self.expanded then
			for i, child in ipairs(self.LO_list) do
				local len, do_scale = child:evt_getGrowAxisLength(x_axis, cross_length)
				if len then
					local this_scale = do_scale and scale or 1.0
					h = h + len * this_scale
				end
				--print("i", i, "h", h)
			end

			local box = self.skin.box
			local margin = box.margin
			local my1, my2 = self.LO_margin_y1, self.LO_margin_y2
			--print("my1,my2", my1, my2)
			h = h + skin.label_content_space + margin.y1 + margin.y2 + my1 + my2

			--print("final h", h)
		end

		--print("---")

		return h, false
	end
end


function def:reshapeGrowing()
	--print("reshapeGrowing(): start")

	-- Ascending from here to root, stop at the first widget which has a growing layout
	-- and a parent which does not.
	local wid = self
	while wid do
		--print("wid:", wid.id, wid)
		local parent = wid.parent
		if not parent or parent.GE.mode ~= "segment" then
			wid = parent
			break
		end
		wid = parent
	end

	if wid then
		wid:reshape()
	end
end


function def:evt_reshapePre()
	-- Viewport #1 is the content area.
	-- Viewport #2 is the background panel.
	-- Viewport #3 is the (clickable) label bar.
	-- Viewport #4 is for the arrow graphic in the label bar.
	-- Viewport #5 is for the text in the label bar.
	-- Margin shrinks the content area.

	local skin = self.skin
	local box = skin.box
	local vp, vp2, vp3, vp4, vp5 = self.vp, self.vp2, self.vp3, self.vp4, self.vp5
	local font = skin.font

	vp2:set(0, 0, self.w, self.h)
	print("self.h", self.h)

	if self.expanded then
		vp2:copy(vp)
		vp:split(vp3, "top", _getLabelBarHeight(self))
		vp:reduceT(box.margin)
		vp:reduceTop(skin.label_content_space)
	else
		vp2:copy(vp3)
		vp:set(0, 0, 0, 0)
	end
	print("vp3.h", vp3.h)
	print("_getLabelBarHeight(self)", _getLabelBarHeight(self))

	-- Arrow graphic area
	vp4.y, vp4.h = vp3.y, vp3.h
	vp4.x = vp3.y + skin.pad_arrow_x1
	vp4.w = vp4.h

	-- Text area
	vp5.h = font:getHeight()
	vp5.y = math.floor(vp3.y + (vp3.h - vp5.h) / 2)
	vp5.x = vp4.x + vp4.w + skin.pad_arrow_x2 + skin.label_pad_x1
	vp5.w = vp3.w - (vp5.x + skin.label_pad_x2)

	widLayout.resetLayoutSpace(self)
	widShared.scrollDirect(self, -vp.x, -vp.y)

	-- Place the thimble glow over the label bar, not the whole widget.
	self.thimble_x, self.thimble_y, self.thimble_w, self.thimble_h = vp3.x, vp3.y, vp3.w, vp3.h
end


function def:evt_thimbleAction(targ, key, scancode, isrepeat)
	if self == targ then
		if self.enabled then
			self:setExpanded(not self:getExpanded())

			return true -- Stop bubbling.
		end
	end
end


function def:evt_pointerHover(targ, x, y, dx, dy)
	x, y = self:getRelativePosition(x, y)

	self.label_bar_hover = self.vp3:pointOverlap(x, y)
	self.cursor_hover = self.label_bar_hover and self.skin.cursor_on
end


function def:evt_pointerHoverOff(targ, x, y, dx, dy)
	self.label_bar_hover = false
	self.cursor_hover = nil
end


function def:evt_pointerPress(targ, x, y, button, istouch, presses)
	if self == targ then
		if button == context.mouse_pressed_button then
			-- Clicked on label bar?
			local xx, yy = self:getRelativePosition(x, y)
			if self.vp3:pointOverlap(xx, yy) then
				if button <= 3 then
					self:tryTakeThimble1()
				end
				if button == 1 then
					self.press_busy = "label-bar"
				end

				self.cursor_press = self.skin.cursor_press

				return true

			-- Otherwise: try directing thimble1 to the container's UI Frame ancestor.
			elseif button <= 3 then
				self.press_busy = false
				local wid = self:getUiFrame()
				if wid then
					wid:tryTakeThimble1()
				end
			end
		end
	end
end


function def:evt_pointerUnpress(targ, x, y, button, istouch, presses)
	if self == targ then
		if button == context.mouse_pressed_button then
			if button == 1 and self.press_busy == "label-bar" then
				-- Unpressed over label bar?
				local xx, yy = self:getRelativePosition(x, y)
				if self.vp3:pointOverlap(xx, yy) then
					self:setExpanded(not self:getExpanded())
				end

				self.cursor_press = nil
			end
			if button <= 3 then
				self.press_busy = false
			end
		end
	end
end


function def:evt_destroy(targ)
	if self == targ then
		widShared.removeViewports(self, 5)
	end
end


local themeAssert = context:getLua("core/res/theme_assert")


def.default_skinner = {
	--validate = uiSchema.newKeysX { TODO


	--transform = function(scale, skin)


	install = function(self, skinner, skin)
		uiTheme.skinnerCopyMethods(self, skinner)

		uiTable.updateDouble(self, "S_label_side", "label_side", skin.default_label_side)
	end,


	remove = function(self, skinner, skin)
		uiTheme.skinnerClearData(self)
	end,


	--refresh = function(self, skinner, skin)
	--update = function(self, skinner, skin, dt)


	render = function(self, ox, oy)
		local vp, vp2, vp3, vp4, vp5 = self.vp, self.vp2, self.vp3, self.vp4, self.vp5
		local skin = self.skin
		local res = _getResourceSet(self, skin)

		love.graphics.push("all")

		-- Background panel
		love.graphics.setColor(skin.background_color)
		uiGraphics.drawSlice(skin.background_slice, vp2.x, vp2.y, vp2.w, vp2.h)

		-- Label bar
		uiGraphics.intersectScissor(ox + self.x + vp3.x, oy + self.y + vp3.y, vp3.w, vp3.h)

		love.graphics.setColor(res.bar_color)
		uiGraphics.drawSlice(res.bar_slice, vp3.x, vp3.y, vp3.w, vp3.h)

		love.graphics.setColor(res.arrow_color)
		local q_arrow = self.expanded and res.arrow_quad_expanded or res.arrow_quad_closed
		uiGraphics.quadShrinkOrCenterXYWH(q_arrow, vp4.x, vp4.y, vp4.w, vp4.h)

		love.graphics.setColor(res.label_text_color)
		love.graphics.setFont(skin.font)
		love.graphics.print(self.text, vp5.x, vp5.y)

		love.graphics.pop()

		love.graphics.push("all")

		love.graphics.pop()
	end,


	--renderLast = function(self, ox, oy)


	renderThimble = widShared.renderThimbleAtRect
}


return def
