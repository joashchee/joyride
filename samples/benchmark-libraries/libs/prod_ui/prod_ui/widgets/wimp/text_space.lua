local context = select(1, ...)


local themeAssert = context:getLua("core/res/theme_assert")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")


local def = {
	skin_id = "text_space1"
}


function def:setFont(id)
	themeAssert.id.fontEval(1, id)

	self.ref_font:setId(id)

	--self:reshape()

	return self
end


function def:getFont()
	return self.ref_font:getId()
end


function def:setSpacing(space)
	uiAssert.numberNotNan(1, space)

	self.space = math.max(space, 0.0)

	--self:reshape()

	return self
end


function def:getSpacing()
	return self.space
end


function def:evt_initialize()
	self.space = 1.0

	self.ref_font = context.resources.fonts:newReference()

	self:skinSetRefs()
	self:skinInstall()
end


function def:evt_getGrowAxisLength(x_axis, cross_length)
	if not x_axis then
		local font = self.ref_font.object
		return math.floor(self.space * font:getHeight() * font:getLineHeight()), false
	end
end


-- function def:evt_reshapePre()


def.default_skinner = {
	validate = uiSchema.newKeysX {
		skinner_id = {uiAssert.type, "string"},

		default_font_id = themeAssert.id.font
	},


	--transform = function(scale, skin)


	install = function(self, skinner, skin)
		uiTheme.skinnerCopyMethods(self, skinner)
		self.ref_font:setDefaultId(skin.default_font_id)
	end,


	remove = function(self, skinner, skin)
		uiTheme.skinnerClearData(self)
		self.ref_font:clear()
	end,


	--refresh = function(self, skinner, skin)
	--update = function(self, skinner, skin, dt)
	--render = function(self, ox, oy)
	--renderLast = function(self, ox, oy) end,
	--renderThimble = function(self, ox, oy)
}


return def