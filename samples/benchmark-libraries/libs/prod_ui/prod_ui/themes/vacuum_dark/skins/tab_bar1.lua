-- Work in progress.
return {
	skinner_id = "wimp/tab_bar",

	box = "*boxes/panel",
	tq_px = "*quads/pixel",
	font = "*fonts/p",

	default_icon_side = "left",
	default_text_align = "left",
	default_icon_set_id = "bureau",

	default_scroll_button_mode = "off",

	height_pad_scale = 1.75,

	sl_body = "*slices/list_box_body",

	icon_spacing = 24,

	pad_text_x1 = 12,
	pad_text_x2 = 12,

	scroll_button_size = 32,

	color_body = {1.0, 1.0, 1.0, 1.0},

	box_button = "*boxes/panel",

	res_button_idle = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_icon = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor",
		tq_less = "*quads/arrow2_left",
		tq_more = "*quads/arrow2_right",
		ox = 0,
		oy = 0
	},

	res_button_hovered = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_icon = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor_hover",
		tq_less = "*quads/arrow2_left",
		tq_more = "*quads/arrow2_right",
		ox = 0,
		oy = 0
	},

	res_button_pressed = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_icon = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor_press",
		tq_less = "*quads/arrow2_left",
		tq_more = "*quads/arrow2_right",
		ox = 0,
		oy = 1
	},

	res_tab_idle = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_text = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor"
	},

	res_tab_hovered = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_text = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor_hover"
	},

	res_tab_selected = {
		color_body = {1.0, 1.0, 1.0, 1.0},
		color_text = {1.0, 1.0, 1.0, 1.0},
		slice = "*slices/button_minor_press"
	}
}
