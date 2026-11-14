# A scene with a NeumetikMusic node named Music, and this script on its root.
extends Node

@onready var music: NeumetikMusic = $Music

func _ready() -> void:
	var problem := music.load_file("res://overworld.mid")
	if problem != "":
		push_error(problem)
		return
	music.looping = true
	music.play()
	music.set_instrument(15, 0, 11)  # a vibraphone for notes over the music

func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_accept"):
		if music.is_playing(): music.pause()
		else: music.play()
	elif event is InputEventKey and event.pressed and event.keycode == KEY_F:
		music.fade_to(0.0 if music.get_volume() > 0.5 else 1.0, 1.5)
	elif event is InputEventKey and event.pressed and event.keycode == KEY_M:
		music.set_channel_muted(9, true)
	elif event is InputEventKey and event.pressed and event.keycode == KEY_N:
		music.note_on(15, 72, 110)
