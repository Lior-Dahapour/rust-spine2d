extends Node2D
# Records every signal of three sprites while a fixed script of animation calls runs, and writes
# the lines to a file so the logs of two runtimes can be diffed.
# Usage: godot --headless --fixed-fps 60 --path . --scene res://sequence_test.tscn -- out=<file>
const LAST_FRAME := 400
var lines := PackedStringArray()
var sprites := {}

func _ready() -> void:
	sprites["boy"] = _make("spineboy", "spineboy-pro", 0.2)
	sprites["alien"] = _make("alien", "alien-pro", 0.1)
	sprites["raptor"] = _make("raptor", "raptor-pro", 0.1)
	sprites["boy"].skeleton_data.set_mix("run", "jump", 0.5)

func _make(folder: String, file: String, mix: float) -> SpineSprite:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/%s/%s.atlas" % [folder, folder])
	data.skeleton_file = load("res://assets/%s/%s.skel" % [folder, file])
	data.default_mix = mix
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	var id := folder
	for signal_name in ["animation_started", "animation_interrupted", "animation_ended",
			"animation_disposed", "animation_completed"]:
		sprite.connect(signal_name, func(_sprite, _state, entry): _log(id, signal_name, _entry(entry)))
	sprite.animation_event.connect(func(_sprite, _state, entry, event): _log(id, "animation_event", _entry(entry) + " " + _event(event)))
	for signal_name in ["before_animation_state_update", "before_animation_state_apply",
			"before_world_transforms_change", "world_transforms_changed"]:
		sprite.connect(signal_name, func(_sprite): _log(id, signal_name, ""))
	return sprite

func _entry(entry) -> String:
	return "%s track=%d time=%.4f" % [entry.get_animation_name(), entry.get_track_index(), entry.get_track_time()]

func _event(event: SpineEvent) -> String:
	return "balance=%s,float_value=%s,int_value=%s,name=%s,string_value=%s,time=%s,volume=%s" % [
		event.get_balance(), event.get_float_value(), event.get_int_value(),
		event.get_data().get_event_name(), event.get_string_value(), event.get_time(),
		event.get_volume()]

func _log(id: String, signal_name: String, args: String) -> void:
	lines.append("%d|%s|%s|%s" % [Engine.get_process_frames(), id, signal_name, args])

func _process(_delta: float) -> void:
	var frame := Engine.get_process_frames()
	var boy: SpineSprite = sprites["boy"]
	var alien: SpineSprite = sprites["alien"]
	var raptor: SpineSprite = sprites["raptor"]
	var alien_names: PackedStringArray = alien.skeleton_data.get_animation_names()
	var raptor_names: PackedStringArray = raptor.skeleton_data.get_animation_names()
	match frame:
		1:
			boy.set_animation("run", true, 0)
			boy.set_animation("shoot", false, 1)
			alien.set_animation(alien_names[0], true, 0)
			raptor.set_animation(raptor_names[0], true, 0)
		40:
			boy.add_animation("jump", 0.0, false, 0)
		50:
			alien.set_animation(alien_names[1], false, 0)
			alien.add_animation(alien_names[0], 0.0, true, 0)
		60:
			boy.add_animation("idle", 0.0, true, 0)
		70:
			raptor.set_empty_animations(0.3)
		90:
			boy.set_empty_animation(1, 0.2)
		100:
			boy.add_empty_animation(1, 0.2, 0.5)
			raptor.set_animation(raptor_names[raptor_names.size() - 1], false, 0)
		120:
			boy.set_animation("walk", true, 0)
		150:
			boy.clear_track(1)
		160:
			boy.time_scale = 0.5
		200:
			boy.time_scale = 1.0
		220:
			boy.set_animation("death", false, 0)
		260:
			boy.clear_tracks()
		270:
			boy.set_animation("idle", true, 0)
		LAST_FRAME:
			_finish()

func _finish() -> void:
	var path := ""
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("out="): path = arg.substr(4)
	var file := FileAccess.open(path, FileAccess.WRITE)
	file.store_string("\n".join(lines) + "\n")
	file.close()
	print("RESULT lines=%d" % lines.size())
	get_tree().quit()
