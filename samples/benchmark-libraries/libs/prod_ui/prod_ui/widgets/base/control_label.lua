--[[
A rewrite of the label widget.
--]]


local context = select(1, ...)


local pMath = require(context.conf.prod_ui_req .. "lib.p_math")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")
local uiScale = require(context.conf.prod_ui_req .. "ui_scale")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")
local wcLabel = context:getLua("shared/wc/wc_label")
local widShared = context:getLua("core/wid_shared")


local fonts = context.resources.fonts


local _lerp = pMath.lerp


local _nm_text_align_x = uiTheme.named_maps.text_align_x
local _nm_text_align_y = uiTheme.named_maps.text_align_y


local def = {
	skin_id = "control_label1",
}


wcLabel.setupDef(def)


function def:setEnabled(enabled)
	local old_enabled = self.enabled

	self.enabled = not not enabled

	if self.enabled ~= old_enabled then
		self:reshape()
	end

	return self
end


function def:getEnabled()
	return self.enabled
end


function def:evt_initialize()
	self.visible = true

	widShared.setupViewports(self, 1)

	wcLabel.setupInstance(self)

	self:skinSetRefs()
	self:skinInstall()

	-- Affects visual style.
	self.enabled = true
end


local dummy = {}


function def:evt_getGrowAxisLength(x_axis, cross_length)
	if not x_axis then
		local border = self.skin.box.border
		local border_y = border.y1 + border.y2
		return border_y + wcLabel.getLabelHeight(self.label), false
	end
end


function def:evt_reshapePre()
	local skin = self.skin
	local vp = self.vp

	vp:set(0, 0, self.w, self.h)
	vp:reduceT(skin.box.border)

	wcLabel.reshapeLabel(self)

	return true
end


function def:evt_destroy(targ)
	if self == targ then
		widShared.removeViewports(self, 1)
	end
end


local themeAssert = context:getLua("core/res/theme_assert")


def.default_skinner = {
	validate = uiSchema.newKeysX {
		skinner_id = {uiAssert.type, "string"},

		box = themeAssert.path.box,
		tq_px = themeAssert.path.quad,

		label_defaults = themeAssert.models.default_label_settings,

		color_active = uiAssert.loveColorTuple,
		color_disabled = uiAssert.loveColorTuple
	},


	--transform = function(scale, skin)


	install = function(self, skinner, skin)
		uiTheme.skinnerCopyMethods(self, skinner)
		wcLabel.refreshFontObject(self.label, skin)
	end,


	remove = function(self, skinner, skin)
		uiTheme.skinnerClearData(self)
		wcLabel.removeFontObject(self.label)
	end,


	--refresh = function (self, skinner, skin)
	--update = function(self, skinner, skin, dt)


	render = function(self, ox, oy)
		local skin = self.skin
		local label = self.label
		local font = self.font
		local vp = self.vp

		love.graphics.push("all")

		local color = self.enabled and skin.color_active or skin.color_disabled

		if label.mode then
			wcLabel.render(self, skin, color, color, 0, 0, ox, oy)
		end

		love.graphics.pop()
	end,


	--renderLast = function(self, ox, oy) end,
	--renderThimble = function(self, ox, oy) end,
}


return def
