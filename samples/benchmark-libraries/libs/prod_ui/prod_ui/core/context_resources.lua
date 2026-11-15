-- To load: local lib = context:getLua("shared/lib")


local contextResources = {}


local context = select(1, ...)


local fontCache = context:getLua("core/res/font_cache")
local pName = require(context.conf.prod_ui_req .. "lib.p_name")
local pString = require(context.conf.prod_ui_req .. "lib.p_string")
local quadSlice = require(context.conf.prod_ui_req .. "graphics.quad_slice")
local resourceCollection = context:getLua("core/res/resource_collection")
local themeAssert = context:getLua("core/res/theme_assert")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiRes = require(context.conf.prod_ui_req .. "ui_res")
local uiScale = require(context.conf.prod_ui_req .. "ui_scale")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")


pName.set(quadSlice._mt_slice, "QuadSlice")


local _assertResolve = uiTable.assertResolve


local _lut_font_ext = uiTable.newNamedMapV("FontFileExtension", ".ttf", ".otf", ".fnt", ".png")
local _lut_font_ext_vec = uiTable.newNamedMapV("VectorFontFileExtension", ".ttf", ".otf")


local _info = {} -- for love.filesystem.getInfo()
local _blank_env = {} -- For setfenv()


local function _validateCollection(model, name)
	local ok, err = uiSchema.validate(model, context.resources[name], "resources." .. name, false)
	if not ok then
		error("Failed to validate a theme resource collection:\n" .. err)
	end
end


-- More context setup


--[[
Important: the first-gen resource tables (boxes, fonts, etc.) must not be overwritten with new tables.
By not overwriting them, far-flung source files may hold direct references without having to perform
repeated lookups through context.resources.
--]]


do
	local _new = resourceCollection.newCollection

	context.resources = {
		boxes = _new("boxes"),
		button_styles = _new("button_styles"),
		card_box_styles = _new("card_box_styles"),
		fonts = _new("fonts"),
		font_defs = _new("font_defs"),
		icons = _new("icons"),
		info = _new("info"),
		pipe_styles = _new("pipe_styles"),
		sash_styles = _new("sash_syles"),
		scroll_bar_data = _new("scroll_bar_data"),
		scroll_bar_styles = _new("scroll_bar_styles"),

		textures = _new("textures"),
		quads = _new("quads"),
		slices = _new("slices"),

		skins = _new("skins")
	}
end


local models = {}


models.box = uiSchema.newModel {
	remaining = uiSchema.newKeysX {
		x1 = {uiAssert.type, "number"},
		x2 = {uiAssert.type, "number"},
		y1 = {uiAssert.type, "number"},
		y2 = {uiAssert.type, "number"}
	}
}


models.buttonStyleRes = uiSchema.newKeysX {
	slice = themeAssert.path.slice,
	color_label = uiAssert.loveColorTuple,
	color_icon = uiAssert.loveColorTuple,
	color_slice = uiAssert.loveColorTuple,
	label_ox = uiAssert.numberNotNan,
	label_oy = uiAssert.numberNotNan,
}


models.buttonStyle = uiSchema.newKeysX {
	box = themeAssert.path.box,
	res_idle = models.buttonStyleRes,
	res_hovered = models.buttonStyleRes,
	res_pressed = models.buttonStyleRes,
	res_disabled = models.buttonStyleRes
}


models.cardBoxStyle = uiSchema.newKeysX {
	card_w = {uiAssert.numberGe, 0},
	card_h = {uiAssert.numberGe, 0},

	-- Spacing between cards.
	spacing_x = {uiAssert.numberGe, 0},
	spacing_y = {uiAssert.numberGe, 0},

	-- Icon and text positions are relative to the card top-left.

	icon_x = {uiAssert.type, "number"},
	icon_y = {uiAssert.type, "number"},
	icon_w = {uiAssert.numberGe, 0},
	icon_h = {uiAssert.numberGe, 0},

	text_x = {uiAssert.type, "number"},
	text_y = {uiAssert.type, "number"},
	text_w = {uiAssert.numberGe, 0},
	text_h = {uiAssert.numberGe, 0},

	text_align_x = {uiAssert.namedMap, uiTheme.named_maps.text_align_x},
	text_align_y = {uiAssert.namedMap, uiTheme.named_maps.text_align_y},
	text_crop = {uiAssert.typeEval, "boolean"},
	font = themeAssert.path.font,

	body_slc = themeAssert.path.sliceEval,

	color_icon = uiAssert.loveColorTuple,
	color_text = uiAssert.loveColorTuple,
	color_body = uiAssert.loveColorTuple
}


models.pipeStyle = uiSchema.newKeysX {
	-- legs, horizontal and vertical
	l_h = themeAssert.path.quad,
	l_v = themeAssert.path.quad,

	-- joints (right-angle connectors)
	j_bl = themeAssert.path.quad,
	j_br = themeAssert.path.quad,
	j_tl = themeAssert.path.quad,
	j_tr = themeAssert.path.quad,

	-- terminators (end-caps)
	t_b = themeAssert.path.quad,
	t_l = themeAssert.path.quad,
	t_r = themeAssert.path.quad,
	t_t = themeAssert.path.quad,

	-- Offsets that shorten legs relative to joints and terminators. These
	-- are relative to the quad 'ox' and 'oy' offsets; refer to the theme's
	-- 'base_data.lua'.
	pad_x = uiAssert.numberNotNan,
	pad_y = uiAssert.numberNotNan,

	-- Offsets to shorten pipes that function as horizontal or vertical separators.
	sep_t = uiAssert.numberNotNan,
	sep_b = uiAssert.numberNotNan,
	sep_l = uiAssert.numberNotNan,
	sep_r = uiAssert.numberNotNan,
}


models.quad = uiSchema.newKeysX {
	x = uiAssert.numberNotNan,
	y = uiAssert.numberNotNan,
	w = uiAssert.numberNotNan,
	h = uiAssert.numberNotNan,

	ox = uiAssert.numberNotNan,
	oy = uiAssert.numberNotNan,

	texture = {uiAssert.loveTypes, "Canvas", "Image", "Texture"},
	quad = {uiAssert.loveType, "Quad"},
	blend_mode = {uiAssert.namedMap, uiTheme.named_maps.BlendMode},
	alpha_mode = {uiAssert.namedMap, uiTheme.named_maps.BlendAlphaMode}
}


models.quadSlice = uiSchema.newModel {
	reject_unhandled = true,

	metatable = {uiAssert.tableWithMetatable, quadSlice._mt_slice},

	keys = {
		x = uiAssert.numberNotNan,
		y = uiAssert.numberNotNan,
		w = uiAssert.numberNotNan,
		h = uiAssert.numberNotNan,
		w1 = uiAssert.numberNotNan,
		h1 = uiAssert.numberNotNan,
		w2 = uiAssert.numberNotNan,
		h2 = uiAssert.numberNotNan,
		w3 = uiAssert.numberNotNan,
		h3 = uiAssert.numberNotNan,
		iw = uiAssert.numberNotNan,
		ih = uiAssert.numberNotNan,

		mirror_h = {uiAssert.type, "boolean"},
		mirror_v = {uiAssert.type, "boolean"},

		quads =	uiSchema.newModel {
			array_len = 9,
			array = {uiAssert.loveType, "Quad"}
		}
	}
}


models.slice = uiSchema.newKeysX {
	tex_quad = themeAssert.path.quad,
	texture = {uiAssert.loveTypes, "Canvas", "Image", "Texture"},
	blend_mode = {uiAssert.namedMap, uiTheme.named_maps.BlendMode},
	alpha_mode = {uiAssert.namedMap, uiTheme.named_maps.BlendAlphaMode},

	slice = models.quadSlice
}


models.scrollBarDataSharedRes = uiSchema.newKeysX {
	slice = themeAssert.path.slice,
	col_body = uiAssert.loveColorTuple,
	col_symbol = uiAssert.loveColorTuple
}


models.scrollBarDataShared = uiSchema.newKeysX {
	idle = models.scrollBarDataSharedRes,
	hover = models.scrollBarDataSharedRes,
	press = models.scrollBarDataSharedRes,
	disabled = models.scrollBarDataSharedRes
}


models.scrollBarData = uiSchema.newKeysX {
	tquad_pixel = themeAssert.path.quad,
	tq_arrow_down = themeAssert.path.quad,
	tq_arrow_up = themeAssert.path.quad,
	tq_arrow_left = themeAssert.path.quad,
	tq_arrow_right = themeAssert.path.quad,

	-- This might be helpful if the buttons and trough do not fit snugly into the scroll bar's rectangular body.
	render_body = {uiAssert.typeEval, "boolean"},

	body_color = uiAssert.loveColorTuple,
	col_trough = uiAssert.loveColorTuple,

	-- In this implementation, the thumb and buttons share slices and colors for idle, hover and press states.
	shared = models.scrollBarDataShared
}


local _nm_scroll_button_mode = uiTable.newNamedMapV("ScrollButtonMode", "pend-pend", "pend-cont", "cont")


models.scrollBarSubStyle = uiSchema.newKeysX {
	near_side = {uiAssert.typeEval, "boolean"},
	auto_hide = {uiAssert.typeEval, "boolean"}
}


models.scrollBarStyle = uiSchema.newKeysX {
	has_buttons = {uiAssert.typeEval, "boolean"},
	has_trough = {uiAssert.typeEval, "boolean"},
	has_thumb = {uiAssert.typeEval, "boolean"},

	bar_size = {uiAssert.integerGeEval, 0},
	button_size = {uiAssert.integerGeEval, 0},
	thumb_size_min = {uiAssert.integerGeEval, 0},
	thumb_size_max = {uiAssert.integerGeEval, 0},

	button_mode = {uiAssert.namedMap, _nm_scroll_button_mode},

	v = models.scrollBarSubStyle,
	h = models.scrollBarSubStyle
}


models.thimbleInfo = uiSchema.newKeysX {
	-- Common details for drawing a rectangular thimble glow.
	mode = {uiAssert.namedMap, uiTheme.named_maps.DrawMode},
	color = uiAssert.colorTuple,
	line_style = {uiAssert.namedMap, uiTheme.named_maps.LineStyle},
	line_width = {uiAssert.integerGe, 0},
	line_join = {uiAssert.namedMap, uiTheme.named_maps.LineJoin},
	corner_rx = {uiAssert.numberGe, 0},
	corner_ry = {uiAssert.numberGe, 0},

	-- Pushes the thimble outline out from the widget rectangle.
	-- This is overridden if the widget contains 'self.thimble_x(|y|w|h)'.
	outline_pad = {uiAssert.integerGe, 0},

	segments = {uiAssert.numberGeEval, 0}
}


models.sashStyleRes = uiSchema.newKeysX {
	slc_lr = themeAssert.path.slice,
	slc_tb = themeAssert.path.slice,

	col_body = uiAssert.loveColorTuple
}


models.sashStyle = uiSchema.newKeysX {
	-- Width of tall sashes; height of wide sashes.
	breadth_half = {uiAssert.integerGe, 0},

	-- Reduces the intersection box when checking for the mouse *entering* a sash.
	-- NOTE: overly large values will make the sash unclickable.
	contract_x = {uiAssert.integerGe, 0},
	contract_y = {uiAssert.integerGe, 0},

	-- Increases the intersection box when checking for the mouse *leaving* a sash.
	-- NOTES:
	-- * Overly large values will prevent the user from clicking on widgets that
	--   are descendants of the container.
	-- * The expansion does not go beyond the container's body.
	expand_x = {uiAssert.integerGe, 0},
	expand_y = {uiAssert.integerGe, 0},

	-- To apply a graphical margin to a sash mosaic, please bake the margin into the texture.

	cursor_hover_h = {uiAssert.typeEval, "string"},
	cursor_hover_v = {uiAssert.typeEval, "string"},
	cursor_drag_h = {uiAssert.typeEval, "string"},
	cursor_drag_v = {uiAssert.typeEval, "string"},

	res_idle = models.sashStyleRes,
	res_hovered = models.sashStyleRes,
	res_pressed = models.sashStyleRes,
	res_disabled = models.sashStyleRes
}


models.boxes_collection = uiSchema.newModel {
	remaining = models.box
}


models.button_styles_collection = uiSchema.newModel {
	remaining = models.buttonStyle
}


models.icons = uiSchema.newModel {
	remaining = themeAssert.path.quad
}


models.icons_collection = uiSchema.newModel {
	remaining = models.icons
}


models.sash_styles_collection = uiSchema.newModel {
	remaining = models.sashStyle
}


models.scroll_bar_data_collection = uiSchema.newModel {
	remaining = models.scrollBarData
}


models.scroll_bar_styles_collection = uiSchema.newModel {
	remaining = models.scrollBarStyle
}


models.card_box_styles_collection = uiSchema.newModel {
	remaining = models.cardBoxStyle
}


local function _scaleBox(box, scale)
	for k, v in pairs(box) do
		for k2, v2 in pairs(v) do
			if type(v2) == "number" then
				v[k2] = math.max(0, math.floor(v2 * scale))
			end
		end
	end
end


local function _scaleButtonStyleRes(res, scale)
	uiScale.fieldInteger(scale, res, "label_ox")
	uiScale.fieldInteger(scale, res, "label_oy")
end


local function _scaleButtonStyle(bs, scale)
	_scaleButtonStyleRes(bs.res_idle, scale)
	_scaleButtonStyleRes(bs.res_hovered, scale)
	_scaleButtonStyleRes(bs.res_pressed, scale)
	_scaleButtonStyleRes(bs.res_disabled, scale)
end


local function _scaleCardBoxStyle(cbs, scale)
	uiScale.fieldInteger(scale, cbs, "card_w", 0)
	uiScale.fieldInteger(scale, cbs, "card_h", 0)

	uiScale.fieldInteger(scale, cbs, "spacing_x", 0)
	uiScale.fieldInteger(scale, cbs, "spacing_y", 0)

	uiScale.fieldInteger(scale, cbs, "icon_x", 0)
	uiScale.fieldInteger(scale, cbs, "icon_y", 0)
	uiScale.fieldInteger(scale, cbs, "icon_w", 0)
	uiScale.fieldInteger(scale, cbs, "icon_h", 0)

	uiScale.fieldInteger(scale, cbs, "text_x", 0)
	uiScale.fieldInteger(scale, cbs, "text_y", 0)
	uiScale.fieldInteger(scale, cbs, "text_w", 0)
	uiScale.fieldInteger(scale, cbs, "text_h", 0)
end


local function _scalePipeStyle(ps, scale, tex_scale)
	uiScale.fieldInteger(tex_scale, ps, "pad_x", 0)
	uiScale.fieldInteger(tex_scale, ps, "pad_y", 0)

	uiScale.fieldInteger(scale, ps, "sep_t", 0)
	uiScale.fieldInteger(scale, ps, "sep_b", 0)
	uiScale.fieldInteger(scale, ps, "sep_l", 0)
	uiScale.fieldInteger(scale, ps, "sep_r", 0)
end


local function _scaleScrollBarStyle(sbs, scale)
	uiScale.fieldInteger(scale, sbs, "bar_size", 0)
	uiScale.fieldInteger(scale, sbs, "button_size", 0)
	uiScale.fieldInteger(scale, sbs, "thumb_size_min", 0)
	uiScale.fieldInteger(scale, sbs, "thumb_size_max", 0)
end


local function _scaleSashStyle(s, scale)
	uiScale.fieldInteger(scale, s, "breadth_half", 0)
	uiScale.fieldInteger(scale, s, "contract_x", 0)
	uiScale.fieldInteger(scale, s, "contract_y", 0)
	uiScale.fieldInteger(scale, s, "expand_x", 0)
	uiScale.fieldInteger(scale, s, "expand_y", 0)
end


local function _getFontTypeFromPath(path)
	if path == "default" then
		return "vector"
	end

	local ext = uiTheme.named_maps.font_type[path:sub(-4)]
	local font_type = ext
	if not font_type then
		error("invalid font extension: " .. tostring(ext))
	end

	return font_type
end


local function _getFontInfoTable(tbl, k)
	local font_info = tbl[k]
	if not font_info then
		error("expected font-info table")
	end
	return font_info
end


local function _checkFontInfo(theme_fonts, k)
	local font_info = _getFontInfoTable(theme_fonts, k)
	uiAssert.type(nil, font_info.path, "string")

	local font_type = _getFontTypeFromPath(font_info.path)
	if font_type == "vector" then
		uiAssert.type(nil, font_info.size, "number")

	elseif font_type == "bmfont" then
		uiAssert.types(nil, font_info.image_path, "nil", "string")

	elseif font_type == "imagefont" then
		uiAssert.type(nil, font_info.glyphs, "string")
		uiAssert.types(nil, font_info.extraspacing, "nil", "number")
	end

	uiAssert.type(nil, font_info.underline_height, "number")

	uiAssert.types(nil, font_info.fallbacks, "nil", "table")
	if font_info.fallbacks then
		for i, fb in ipairs(font_info.fallbacks) do

			uiAssert.type(nil, fb.path, "string")
			if font_type == "vector" then
				uiAssert.type(nil, fb.size, "number")
			end
		end
	end
end


local function _scaleFontInfo(theme_fonts, k, scale)
	local font_info = _getFontInfoTable(theme_fonts, k)

	local font_type = _getFontTypeFromPath(font_info.path)
	if font_type == "vector" then
		uiScale.fieldInteger(scale, font_info, "size", 0, uiTheme.settings.max_font_size)
	end

	uiScale.fieldInteger(scale, font_info, "underline_height", 1)

	-- Do not scale ImageFont extraspacing.

	if font_info.fallbacks then
		for i, fb in ipairs(font_info.fallbacks) do
			if font_type == "vector" then
				uiScale.fieldInteger(scale, fb, "size", 0, uiTheme.settings.max_font_size)
			end
		end
	end
end


local methods = {}
contextResources.methods = methods


function methods:resetResources()
	for k, v in pairs(self.resources) do
		if type(v) == "table" then
			uiTable.clearAll(v)
		end
	end
end


function methods:interpolatePath(path)
	uiAssert.type(1, path, "string")

	return (path:gsub(pString.ptn_percent, self.path_symbols))
end


-- If 'ta' is a table, then deep-copy its key-value pairs to table 'tb'.
local function _deepCopyFields(ta, tb)
	if type(tb) ~= "table" then
		error("the destination does not exist, or it is not a table.")
	end
	if type(ta) == "table" then
		for k, v in pairs(ta) do
			tb[k] = uiTable.deepCopy(v)
		end
	end
end


local function _initTexture(texture, metadata)
	local tex_info = {texture=texture}

	if metadata then
		if metadata.config then
			for k, v in pairs(metadata.config) do
				tex_info[k] = v
			end
		end
	end

	uiTable.assignIfNil(tex_info, "alpha_mode", "alphamultiply")
	uiTable.assignIfNil(tex_info, "blend_mode", "alpha")
	uiTable.assignIfNil(tex_info, "filter_mag", "linear")
	uiTable.assignIfNil(tex_info, "filter_min", "linear")
	uiTable.assignIfNil(tex_info, "wrap_h", "clamp")
	uiTable.assignIfNil(tex_info, "wrap_v", "clamp")

	texture:setFilter(tex_info.filter_min, tex_info.filter_mag)
	texture:setWrap(tex_info.wrap_h, tex_info.wrap_v)

	if metadata then
		if metadata.quads then
			tex_info.quads = {}
			for k, v in pairs(metadata.quads) do
				tex_info.quads[k] = {
					x = v.x,
					y = v.y,
					w = v.w,
					h = v.h,
					ox = v.ox,
					oy = v.oy,
					texture = tex_info.texture,
					quad = love.graphics.newQuad(v.x, v.y, v.w, v.h, tex_info.texture),
					blend_mode = tex_info.blend_mode,
					alpha_mode = tex_info.alpha_mode,
				}
			end
		end

		if metadata.slices then
			tex_info.slices = {}
			for k, v in pairs(metadata.slices) do
				local base_tq = tex_info.quads[k]
				if not base_tq then
					error("missing base texture+quad pair for 9-Slice: " .. tostring(k))
				end

				local tex_slice = {
					tex_quad = base_tq,
					texture = base_tq.texture,
					blend_mode = tex_info.blend_mode,
					alpha_mode = tex_info.alpha_mode,

					slice = quadSlice.newSlice(
						base_tq.x + v.x, base_tq.y + v.y,
						v.w1, v.h1,
						v.w2, v.h2,
						v.w3, v.h3,
						base_tq.texture:getDimensions()
					)
				}

				-- If specified, attach a starting draw function.
				if v.draw_fn_id then
					local draw_fn = quadSlice.draw_functions[v.draw_fn_id]
					if not draw_fn then
						error("in 'quadSlice.draw_functions', cannot find function with ID: " .. tostring(v.draw_fn_id)) -- XXX: print in binary
					end

					tex_slice.slice.drawFromParams = draw_fn
				end

				-- If specified, set the initial state of each tile.
				-- 'tiles_state' is an array of bools. Only indexes 1-9 with true or
				-- false are considered. All other values are ignored (so they will default
				-- to being enabled).
				if v.tiles_state then
					for i = 1, 9 do
						if type(v.tiles_state[i]) == "boolean" then
							tex_slice.slice:setTileEnabled(i, false)
						end
					end
				end

				tex_info.slices[k] = tex_slice
			end
		end
	end

	return tex_info
end


--- Loads a PNG, and an optional .lua metadata file of the same name.
local function _loadTextureFiles(path)
	path = context:interpolatePath(path)

	local info = uiRes.assertGetInfo(path, "file", _info)

	local tex = love.graphics.newImage(path)
	local metadata
	local path_lua = path:sub(1, -5) .. ".lua"

	local info2 = love.filesystem.getInfo(path_lua, "file", _info)
	if info2 then
		local chunk = uiRes.assertLoad(path_lua)
		metadata = chunk()
	end
	-- TODO: check config fields

	return tex, metadata
end


local function _applyReferences(t, resources)
	for k, v in pairs(t) do
		if type(v) == "table" then
			_applyReferences(v, resources)

		elseif type(v) == "string" and v:sub(1, 1) == "*" then
			t[k] = _assertResolve(resources, v:sub(2))
		end
	end
end


function methods:applyTheme(theme)
	uiAssert.type(1, theme, "table")

	local resources = self.resources
	local scale = self.scale

	self:resetResources()
	self.theme_id = false
	fontCache.clear()

	if not theme then
		return
	end

	local tex_scale = math.max(1, context:getTextureScale())

	self.theme_id = theme.info.theme_ids[1]
	if type(self.theme_id) ~= "string" then
		error("invalid Theme ID")
	end

	-- TODO: support loading a directory of images as one group (like an uncompiled atlas).

	-- textures, quads, slices
	if theme.textures then
		for k, tex_info in pairs(theme.textures) do
			local tex, meta = _loadTextureFiles(tex_info.path)
			local tex_tbl = _initTexture(tex, meta)

			resources.textures[k] = tex_tbl

			-- Directly add quads, QuadSlices to the resource tables.
			-- Duplicate names are an error.
			if tex_tbl.quads then
				for k2, v in pairs(tex_tbl.quads) do
					if resources.quads[k2] then
						error("duplicate quad name: " .. k2)
					end
					resources.quads[k2] = v
				end
			end

			if tex_tbl.slices then
				for k2, v in pairs(tex_tbl.slices) do
					if resources.slices[k2] then
						error("duplicate QuadSlice name: " .. k2)
					end
					resources.slices[k2] = v
				end
			end
		end
	end

	if theme.fonts then
		for k in pairs(theme.fonts) do
			_checkFontInfo(theme.fonts, k)
			_scaleFontInfo(theme.fonts, k, scale)

			resources.font_defs[k] = theme.fonts[k]
		end

		for k, font_info in pairs(theme.fonts) do
			fontCache.createFontObjects(font_info)
		end

		for k, font_info in pairs(theme.fonts) do
			fontCache.setFallbacks(font_info)
			resources.fonts[k] = fontCache.getFont(font_info)
		end
	end
	fontCache.clear()

	if theme.boxes then
		_deepCopyFields(theme.boxes, resources.boxes)
		_validateCollection(models.boxes_collection, "boxes")
		for k, box in pairs(resources.boxes) do
			_scaleBox(box, scale)
		end
	end

	if theme.icons then
		_deepCopyFields(theme.icons, resources.icons)
	end

	if theme.info then
		_deepCopyFields(theme.info, resources.info)

		-- TODO
	end

	if theme.button_styles then
		_deepCopyFields(theme.button_styles, resources.button_styles)
		_validateCollection(models.button_styles_collection, "button_styles")

		for k, bs in pairs(resources.button_styles) do
			_scaleButtonStyle(bs, scale)
		end
	end

	if theme.card_box_styles then
		_deepCopyFields(theme.card_box_styles, resources.card_box_styles)
		_validateCollection(models.card_box_styles_collection, "card_box_styles")

		for k, cbs in pairs(resources.card_box_styles) do
			_scaleCardBoxStyle(cbs, scale)
		end
	end

	if theme.pipe_styles then
		_deepCopyFields(theme.pipe_styles, resources.pipe_styles)

		for k, ps in pairs(resources.pipe_styles) do
			_scalePipeStyle(ps, scale, tex_scale)
		end
	end

	if theme.sash_styles then
		_deepCopyFields(theme.sash_styles, resources.sash_styles)
	end

	if theme.scroll_bar_data then
		_deepCopyFields(theme.scroll_bar_data, resources.scroll_bar_data)
		_validateCollection(models.scroll_bar_data_collection, "scroll_bar_data")
	end

	if theme.scroll_bar_styles then
		_deepCopyFields(theme.scroll_bar_styles, resources.scroll_bar_styles)
		_validateCollection(models.scroll_bar_styles_collection, "scroll_bar_styles")

		for k, v in pairs(resources.scroll_bar_styles) do
			_scaleScrollBarStyle(v, scale)
		end
	end

	if theme.skins then
		for k, v in pairs(theme.skins) do
			resources.skins[k] = uiTable.deepCopy(v)
		end
	end

	_validateCollection(models.icons_collection, "icons")

	_validateCollection(models.sash_styles_collection, "sash_styles")
	for k, v in pairs(resources.sash_styles) do
		_scaleSashStyle(v, scale)
	end

	local skin_errors = {}

	for k, v in pairs(resources.skins) do
		local skinner_id = v.skinner_id
		local skinner = context.skinners[skinner_id]
		if not skinner then
			error(tostring(k) .. ": no skinner with ID: " .. tostring(skinner_id))
		end

		if skinner.validate then
			local ok, err = uiSchema.validate(skinner.validate, v, tostring(k))
			if not ok then
				table.insert(skin_errors, err)

			elseif skinner.transform then
				skinner.transform(scale, v)
			end
		end
	end

	if #skin_errors > 0 then
		error("failed to validate skin data:\n" .. table.concat(skin_errors, "\n…\n"))
	end

	_applyReferences(resources, resources)

	collectgarbage("collect")
	collectgarbage("collect")
end


return contextResources
