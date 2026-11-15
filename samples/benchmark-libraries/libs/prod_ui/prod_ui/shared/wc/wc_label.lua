--[[
Shared code for handling label text in widgets.

Usage: put these in a widget definition:

At the top scope:
	wcLabel.setupDef(def)

In evt_initialize():
	wcLabel.setupInstance(self)

In the skin installation script:
	wcLabel.refreshFontObject(self.label, skin)

In the skin removal script:
	wcLabel.removeFontObject(self.label)


----


Label modes:
"single": A single line of text.

"single-ul": A single line of text with an optional underline. The text to be underlined is marked
	by two underscores, like the 'b' in:  `foo_b_ar`

"multi": Multi-line text.


Required skin fields:

skin.tq_px: A textured quad of a single white pixel. Used for drawing the underline.

skin.label_defaults = {
	font_id,
	align_x,
	align_y
}


Recommended skin fields:

Default text alignment settings for a widget.


Required widget fields:

Viewport #1: self.vp
--]]


local context = select(1, ...)


local wcLabel = {}


local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiGraphics = require(context.conf.prod_ui_req .. "ui_graphics")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")
local textUtil = require(context.conf.prod_ui_req .. "lib.text_util")
local themeAssert = context:getLua("core/res/theme_assert")


local resources = context.resources
local font_defs = resources.font_defs
local fonts = resources.fonts


local _nm_modes = uiTable.newNamedMapV("LabelMode", "single", "single-ul", "multi")


-- Numeric values from 0-1 that correspond to font rendering alignment constants.
local _align_x_num = {left=0.0, center=0.5, right=1.0, justify=0.0}


local function _multiLineWrap(self, label, vp, font)
	local _, lines = font:getWrap(label, vp.w)
	label.h = math.floor(0.5 + font:getHeight() * font:getLineHeight() * #lines)
	label.w_old = vp.w
end


function wcLabel.refreshFontObject(label, skin)
	label.ref_font:setDefaultId(skin.label_defaults.font_id)
		:refresh()
end


function wcLabel.removeFontObject(label)
	label.ref_font:clear()
end


function wcLabel.newTextLabel()
	local label = {
		-- Whether the label is single-line, single-with-underline, or multi-line.
		mode = "single",

		text = "",

		-- (processed) text with underline markers removed
		text_pro = "",

		-- When not specified, the widget provides fallbacks through its skin table.
		align_x = false, -- false, "left", "center", "right", "justify"
		align_y = false, -- false, "top", "middle", "bottom"

		-- (multi) text height when wrapping.
		h = false,

		-- (multi) The old wrap-limit for the text. Indicates if the label is in need of an
		-- update while reshaping.
		w_old = false,

		-- (multi) Height of the wrapped text. Used for vertical centering.
		text_h = 0,

		-- (single-ul) 'not ul_x' means that the underline is not in use.
		ul_x = false,

		-- (single-ul) The amount to offset the underline when "center" or
		-- "right" horizontal alignment is active.
		ul_ox = 0,
		ul_w = 0,

		-- Underline height is determined by font defs in the theme.

		ref_font = fonts:newReference()
			:setDefaultId(resources.info.misc.default_label_font_id)
	}

	return label
end


local function _calculateUnderlineOffset(self, label, vp, font)
	-- Valid only for "single-ul" mode.
	local align_x = label.align_x or self.skin.label_defaults.align_x
	label.ul_ox = math.floor((vp.w - font:getWidth(label.text_pro)) * _align_x_num[align_x])
end


--- Update a label during widget reshaping, if necessary.
function wcLabel.reshapeLabel(self)
	local label = self.label
	local font = label.ref_font.object

	if label.mode == "single-ul" then
		_calculateUnderlineOffset(self, label, self.vp, font)

	elseif label.mode == "multi" then
		-- Need to refresh wrapped dimensions?
		local vp = self.vp
		if vp.w ~= label.w_old then
			_multiLineWrap(self, label, vp, font)
		end
	end

	-- Nothing to do for `single`.
end


--- Draws a widget's label text.
-- @param self The widget.
-- @param skin The widget's skin.
-- @param c_text The text color (table).
-- @param c_ul The underline color (table, or false/nil to use the text color).
-- @param label_ox Text X offset.
-- @param label_oy Text Y offset. (ie for inset button states)
-- @param ox Scissor X offset.
-- @param oy Scissor Y offset.
function wcLabel.render(self, skin, c_text, c_ul, label_ox, label_oy, ox, oy)
	local vp = self.vp
	local label = self.label
	local label_defaults = skin.label_defaults
	local ref_font = label.ref_font
	local font = ref_font.object
	local font_def = font_defs[ref_font.id or ref_font.default_id]
	local align_x = label.align_x or label_defaults.align_x
	local align_y = label.align_y or label_defaults.align_y

	love.graphics.push("all")

	-- Prevent text from spilling out of Viewport #1.
	uiGraphics.intersectScissor(ox + self.x + vp.x, oy + self.y + vp.y, vp.w, vp.h)

	local height = wcLabel.getLabelHeight(label)
	local text_y
	if align_y == "top" then
		text_y = math.floor(vp.y)

	elseif align_y == "bottom" then
		text_y = math.floor(vp.y + vp.h - height)

	else -- "middle"
		text_y = math.floor(vp.y + ((vp.h - height) * 0.5))
	end

	-- underline
	if label.ul_x then
		local tq_px = skin.tq_px
		love.graphics.setColor(c_ul or c_text)
		local ul_y = textUtil.getUnderlineOffset(font, font_def.underline_height)
		uiGraphics.quadXYWH(
			tq_px,
			vp.x + label_ox + label.ul_x + label.ul_ox,
			text_y + ul_y + label_oy,
			label.ul_w,
			font_def.underline_height
		)
	end

	-- label text
	love.graphics.setFont(font)
	love.graphics.setColor(c_text)
	love.graphics.printf(label.text_pro, vp.x + label_ox, text_y + label_oy, vp.w, align_x)

	love.graphics.pop()
end


local methods = {}


function wcLabel.setupDef(def)
	uiTable.patch(def, methods)
end


function wcLabel.setupInstance(self)
	self.label = wcLabel.newTextLabel()
end


function wcLabel.getLabelHeight(label)
	if label.h then
		return label.h
	else
		local font = label.ref_font.object
		return math.floor(0.5 + font:getHeight() * font:getLineHeight())
	end
end


function methods:setLabelFont(id)
	local label = self.label
	label.ref_font:setId(id)

	wcLabel.refreshFontObject(label, self.skin)

	return self
end


function methods:getLabelFont()
	return self.label.ref_font:getId()
end


function methods:setLabelAlign(align_x, align_y)
	uiAssert.namedMapEval(1, align_x, uiTheme.named_maps.label_align_x)
	uiAssert.namedMapEval(2, align_y, uiTheme.named_maps.label_align_y)

	local label = self.label
	label.align_x = align_x or false
	label.align_y = align_y or false

	return self
end


function methods:getLabelAlign()
	local label = self.label

	return label.align_x, label.align_y
end


function methods:setLabelAlignX(align_x)
	uiAssert.namedMapEval(1, align_x, uiTheme.named_maps.label_align_x)

	self.label.align_x = align_x or false

	return self
end


function methods:getLabelAlignX()
	return self.label.align_x
end


function methods:setLabelAlignY(align_y)
	uiAssert.namedMapEval(1, align_y, uiTheme.named_maps.label_align_y)

	self.label.align_y = align_y or false

	return self
end


function methods:getLabelAlignY()
	return self.label.align_y
end


function methods:setLabelMode(mode)
	uiAssert.namedMapEval(1, mode, _nm_modes)

	local label = self.label

	label.mode = mode

	if mode ~= "multi" then
		label.h = false
	end

	self:reshape()

	return self
end


function methods:getLabelMode()
	return self.label.mode
end


--- Sets a widget's label text.
--@param self The widget to update.
--@param text The text to assign to the label.
function methods:setLabelText(text)
	local label = self.label

	uiAssert.type(1, text, "string")

	label.text = text
	label.text_pro = text

	if label.mode == "single-ul" then
		-- Process underline.
		local font = label.ref_font.object
		local temp_str
		temp_str, label.ul_x, label.ul_w = textUtil.processUnderline(text, font)

		_calculateUnderlineOffset(self, label, self.vp, font)

		if temp_str then
			label.text_pro = temp_str
		end

	elseif self.label_mode == "multi" then
		_multiLineWrap(self, label, self.vp, label.ref_font.object)
	end

	self:reshape()

	return self
end


function methods:getLabelText()
	return self.label.text
end


return wcLabel
