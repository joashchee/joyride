return {
	skinner_id = "base/slider_bar",

	box = "*boxes/button",
	tq_px = "*quads/pixel",

	default_height = 40,

	-- NOTE: this default skin isn't really designed to accommodate labels.
	label_spacing = 0,
	label_placement = "left",

	trough_breadth = 1,
	trough_breadth2 = 4,

	trough_click_anywhere = true,

	thumb_w = 24,
	thumb_h = 24,

	thumb_ox = 12,
	thumb_oy = 12,

	trough_ext = 12,

	cursor_on = "hand",
	cursor_press = "hand",

	label_defaults = {
		font_id = "p",
		align_x = "center",
		align_y = "middle"
	},

	res_idle = {
		tq_thumb = "*quads/slider_thumb1", -- XXX make some variations for hover, press, disabled
		sl_trough_active = "*slices/slider_trough_active",
		sl_trough_empty = "*slices/slider_trough_empty",

		color_label = {1.0, 1.0, 1.0, 1.0},

		label_ox = 0,
		label_oy = 0,
	},

	res_hovered = {
		tq_thumb = "*quads/slider_thumb1",
		sl_trough_active = "*slices/slider_trough_active",
		sl_trough_empty = "*slices/slider_trough_empty",

		color_label = {1.0, 1.0, 1.0, 1.0},

		label_ox = 0,
		label_oy = 0,
	},

	res_pressed = {
		tq_thumb = "*quads/slider_thumb1",
		sl_trough_active = "*slices/slider_trough_active",
		sl_trough_empty = "*slices/slider_trough_empty",

		color_label = {0.9, 0.9, 0.9, 1.0},

		label_ox = 0,
		label_oy = 0,
	},

	res_disabled = {
		tq_thumb = "*quads/slider_thumb1",
		sl_trough_active = "*slices/slider_trough_active",
		sl_trough_empty = "*slices/slider_trough_empty",

		color_label = {0.5, 0.5, 0.5, 1.0},

		label_ox = 0,
		label_oy = 0,
	},
}
