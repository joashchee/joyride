local context = select(1, ...)


local themeAssert = context:getLua("core/res/theme_assert")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")
local uiScale = require(context.conf.prod_ui_req .. "ui_scale")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")
local wcButtonStyle = context:getLua("shared/wc/wc_button_style")
local wcGraphic = context:getLua("shared/wc/wc_graphic")
local wcLabel = context:getLua("shared/wc/wc_label")


return {
	validate = uiSchema.newKeysX {
		skinner_id = {uiAssert.type, "string"},

		tq_px = themeAssert.path.quad,

		default_button_style = themeAssert.id.buttonStyle,

		default_height = {uiAssert.numberGe, 0}, -- unscaled

		-- Cursor IDs for hover and press states.
		cursor_on = {uiAssert.types, "nil", "string"},
		cursor_press = {uiAssert.types, "nil", "string"},

		label_defaults = themeAssert.models.default_label_settings,

		-- A default graphic to use if the widget doesn't provide one.
		-- TODO
		-- graphic

		quad_align_x = {uiAssert.namedMap, uiTheme.named_maps.quad_align_x},
		quad_align_y = {uiAssert.namedMap, uiTheme.named_maps.quad_align_y},

		-- Placement of graphic in relation to text labels.
		graphic_placement = {uiAssert.namedMap, uiTheme.named_maps.graphic_placement},

		-- How much space to assign the graphic when not using "overlay" placement.
		graphic_spacing = {uiAssert.numberGe, 0},
	},


	transform = function(scale, skin)
		uiScale.fieldInteger(scale, skin, "graphic_spacing")
	end,


	install = function(self, skinner, skin)
		uiTheme.skinnerCopyMethods(self, skinner)
		wcLabel.refreshFontObject(self.label, skin)
		wcButtonStyle.updateButtonStyle(self)
	end,


	remove = function(self, skinner, skin)
		uiTheme.skinnerClearData(self)
		wcLabel.removeFontObject(self.label)
		wcButtonStyle.clearButtonStyle(self)
	end,


	--refresh = function (self, skinner, skin)
	--update = function(self, skinner, skin, dt)


	render = function(self, ox, oy)
		local skin = self.skin
		local label = self.label
		local button_style = self.ref_button_style.object
		local res = uiTheme.pickButtonResource(self, button_style)

		local slc_body = res.slice
		love.graphics.setColor(res.color_slice)
		uiGraphics.drawSlice(slc_body, 0, 0, self.w, self.h)

		local graphic = self.graphic or skin.graphic
		if graphic then
			wcGraphic.render(self, graphic, skin, res.color_quad, res.label_ox, res.label_oy, ox, oy)
		end

		if label and label.mode then
			wcLabel.render(self, skin, res.color_label, res.color_label_ul, res.label_ox, res.label_oy, ox, oy)
		end
	end,


	--renderLast = function(self, ox, oy) end,
	--renderThimble = function(self, ox, oy) end,
}
