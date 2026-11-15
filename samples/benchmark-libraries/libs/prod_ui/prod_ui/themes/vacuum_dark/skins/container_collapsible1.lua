return {
	skinner_id = "base/container_collapsible",

	box = "*boxes/container_collapsible",

	font = "*fonts/p",

	cursor_on = "hand",
	cursor_press = "hand",

	default_label_side = "center",

	label_content_space = 4,

	pad_arrow_x1 = 8,
	pad_arrow_x2 = 8,
	pad_bar_y1 = 8,
	pad_bar_y2 = 8,

	label_pad_far = 16,

	label_pad_x1 = 0,
	label_pad_x2 = 0,
	label_pad_y = 0,

	background_slice = "*slices/container_body1",
	background_color = {1, 1, 1, 1},

	res_idle = {
		bar_slice = "*slices/header_button_idle",
		bar_color = {1, 1, 1, 1},
		label_text_color = {0.9, 0.9, 0.9, 1},
		arrow_quad_expanded = "*quads/arrow2_right",
		arrow_quad_closed = "*quads/arrow2_down",
		arrow_color = {1.0, 1.0, 1.0, 1.0},
		label_ox = 0,
		label_oy = 0
	},

	res_hovered = {
		bar_slice = "*slices/header_button_hover",
		bar_color = {1, 1, 1, 1},
		label_text_color = {0.9, 0.9, 0.9, 1},
		arrow_quad_expanded = "*quads/arrow2_right",
		arrow_quad_closed = "*quads/arrow2_down",
		arrow_color = {1.0, 1.0, 1.0, 1.0},
		label_ox = 0,
		label_oy = 0
	},

	res_pressed = {
		bar_slice = "*slices/header_button_press",
		bar_color = {1, 1, 1, 1},
		label_text_color = {0.9, 0.9, 0.9, 1},
		arrow_quad_expanded = "*quads/arrow2_right",
		arrow_quad_closed = "*quads/arrow2_down",
		arrow_color = {1.0, 1.0, 1.0, 1.0},
		label_ox = 0,
		label_oy = 0
	},

	res_disabled = {
		bar_slice = "*slices/header_button_disabled",
		bar_color = {1, 1, 1, 1},
		label_text_color = {0.9, 0.9, 0.9, 1},
		arrow_quad_expanded = "*quads/arrow2_right",
		arrow_quad_closed = "*quads/arrow2_down",
		arrow_color = {1.0, 1.0, 1.0, 1.0},
		label_ox = 0,
		label_oy = 0
	},
}
