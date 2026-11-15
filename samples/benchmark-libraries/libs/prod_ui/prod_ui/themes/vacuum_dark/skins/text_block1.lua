return {
	skinner_id = "wimp/text_block",

	box = "*boxes/panel",

	-- Cursor IDs for hover and press states.
	-- Only used when the text block is configured as a hyperlink.
	cursor_on = "hand",
	cursor_press = "hand",

	color = {1.0, 1.0, 1.0, 1.0},
	color_hyperlink = {0.8, 0.8, 0.2, 1.0},
	color_hyperlink_hover = {0.9, 0.9, 0.9, 1.0},
	color_hyperlink_press = {1.0, 1.0, 1.0, 1.0},

	default_font_id = "p"
}
