--[[
return {
	path = "default",
	size = 14,
	underline_height = 1
}
--]]


-- [[
-- XXX: Test symbol substitution in single-line text boxes
return {
	path = "%resources%/font_data/noto_sans/NotoSans-Regular.ttf",
	size = 14,
	underline_height = 1,
	fallbacks = {
		{
			path = "%resources%/font_data/noto_sans/NotoSansSymbols-Regular.ttf",
			size = 14
		},
		{
			path = "%resources%/font_data/noto_sans/NotoSansSymbols2-Regular.ttf",
			size = 14
		}
	}
}
--]]
