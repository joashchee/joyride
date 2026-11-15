--[[
Button style state (how it looks).

For shared button code, see: wc_button.lua
For label code, see: wc_label.lua

----

Usage: put these in a widget definition:

At the top scope:
	wcButtonStyle.setupDef(def)

In evt_initialize(), before the skin is installed:
	wcButtonStyle.setupInstance(self)

In the skin installation script:
	wcButtonStyle.updateButtonStyle(self)

In the skin removal script:
	wcButtonStyle.clearButtonStyle(self)

In the widget skin, put a ButtonStyle ID here:
	skin.default_button_style
--]]


local context = select(1, ...)


local wcButtonStyle = {}


local methods = {}


local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")


local button_styles = context.resources.button_styles


function wcButtonStyle.setupDef(def)
	uiTable.patch(def, methods)
end


function wcButtonStyle.setupInstance(self)
	self.ref_button_style = button_styles:newReference()
end


function methods:setButtonStyle(id)
	if id then
		button_styles:assertObject(id)
	end

	self.ref_button_style:setId(id)

	wcButtonStyle.updateButtonStyle(self)

	self:reshape()

	return self
end


function methods:getButtonStyle()
	return self.ref_button_style:getId()
end


function wcButtonStyle.updateButtonStyle(self)
	self.ref_button_style:setDefaultId(self.skin.default_button_style)
end


function wcButtonStyle.clearButtonStyle(self)
	self.ref_button_style:clear()
end


return wcButtonStyle
