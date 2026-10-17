extends Node2D
# The animation signals reach the sprite they belong to: started, completed on loops, user events,
# and only for the sprite that plays. Runs headless on the process frames.
# Usage: godot --headless --path . --scene res://events_test.tscn
var failures := 0
var started := 0
var completed := 0
var events := 0
var idle_signals := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var playing := SpineSprite.new()
	var idle := SpineSprite.new()
	playing.skeleton_data = data
	idle.skeleton_data = data
	add_child(playing)
	add_child(idle)

	playing.animation_started.connect(func(_s, _st, _e): started += 1)
	playing.animation_completed.connect(func(_s, _st, _e): completed += 1)
	playing.animation_event.connect(func(_s, _st, _e, _ev): events += 1)
	idle.animation_started.connect(func(_s, _st, _e): idle_signals += 1)
	idle.animation_completed.connect(func(_s, _st, _e): idle_signals += 1)
	idle.animation_event.connect(func(_s, _st, _e, _ev): idle_signals += 1)

	playing.set_animation("run", true, 0)
	await get_tree().create_timer(2.5).timeout

	check("started signal fired", started >= 1)
	check("completed signal fired on loops", completed >= 2)
	check("user events fired", events >= 1)
	check("a sprite that plays nothing gets no animation signals", idle_signals == 0)
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
