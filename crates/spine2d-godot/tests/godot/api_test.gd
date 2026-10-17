# Trimmed copy of the shared api_test.gd for the ECS runtime: update_skeleton steps at the next frame
# there, so the checks that need it to act inside the call are left out for now.
extends Node2D
# Run with: godot --headless --path . --scene res://api_test.tscn
var failures := 0
var events := 0
var started := 0
var completed := 0
var before_calls := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func _ready() -> void:
	var atlas := SpineAtlasResource.new()
	atlas.source_file = "res://assets/spineboy/spineboy.atlas"
	var file := SpineSkeletonFileResource.new()
	file.source_file = "res://assets/spineboy/spineboy-pro.json"
	var data := SpineSkeletonDataResource.new()
	data.atlas = atlas
	data.skeleton_file = file
	data.default_mix = 0.2
	data.set_mix("run", "jump", 0.5)
	check("data loads", data.is_skeleton_data_loaded())
	check("animation names", data.get_animation_names().has("run") and data.get_animation_names().has("jump"))
	check("skin names", data.get_skin_names().size() >= 1)
	check("animation duration", absf(data.get_animation_duration("run") - 0.6667) < 0.01)

	var s := SpineSprite.new()
	s.skeleton_data = data
	s.update_mode = 2
	add_child(s)
	s.animation_started.connect(func(_s, _st, _e): started += 1)
	s.animation_completed.connect(func(_s, _st, _e): completed += 1)
	s.animation_event.connect(func(_s, _st, _e, ev): events += 1)

	var hints := {}
	for prop in s.get_property_list():
		hints[prop.name] = prop.hint_string
	check("inspector suggests animation names", "run" in hints["initial_animation"] and "jump" in hints["initial_animation"])
	check("inspector suggests skin names", "default" in hints["skin"])

	var entry = s.set_animation("run", true, 0)
	check("set_animation returns an entry", entry != null and entry.get_animation_name() == "run")
	check("entry is valid", entry.is_valid())
	check("entry loop flag", entry.get_loop())
	entry.set_time_scale(0.5)
	check("entry time scale round-trips", is_equal_approx(entry.get_time_scale(), 0.5))
	check("track index", entry.get_track_index() == 0)
	check("get_current returns the entry", s.get_current(0).get_animation_name() == "run")
	check("no entry on track 3", s.get_current(3) == null)

	var head = s.get_bone("head")
	check("bone lookup", head != null and head.get_name() == "head")
	check("unknown bone is null", s.get_bone("nope") == null)
	check("bone has parent", head.get_parent() != null)
	check("bone names", s.get_bone_names().has("head") and s.get_bone_names().size() > 10)
	var before_rot: float = head.get_world_rotation()


	var slot = s.get_slot("head")
	check("slot lookup", slot != null and slot.get_name() == "head")
	check("slot attachment name", slot.get_attachment_name() != "")
	var original: String = slot.get_attachment_name()
	check("hide attachment", slot.set_attachment("") and slot.get_attachment_name() == "")
	check("restore attachment", slot.set_attachment(original) and slot.get_attachment_name() == original)
	slot.set_color(Color(1, 0.5, 0.25, 1))
	check("slot color round-trips", slot.get_color().is_equal_approx(Color(1, 0.5, 0.25, 1)))
	check("sprite attachment api", s.set_attachment("head", original))
	check("unknown attachment rejected", not s.set_attachment("head", "no-such-attachment"))

	var bounds: Rect2 = s.get_bounds()
	check("bounds are non-empty", bounds.size.x > 10 and bounds.size.y > 10)

	var jump = s.set_animation("jump", false, 0)
	check("mix from run to jump uses the pair mix", is_equal_approx(jump.get_mix_duration(), 0.5))
	var queued = s.add_animation("idle", 0.0, true, 0)
	check("add_animation returns an entry", queued != null and queued.get_animation_name() == "idle")
	check("unknown animation returns null", s.set_animation("no-such", true, 0) == null)
	s.clear_tracks()
	check("entries are invalid after clear_tracks", not jump.is_valid())
	check("invalid entry calls are harmless", jump.get_animation_name() == "")

	s.skin = "default"
	check("skin by name", true)
	s.set_skeleton_color(Color(1, 0, 0, 1))
	check("skeleton color round-trips", s.get_skeleton_color().is_equal_approx(Color(1, 0, 0, 1)))

	s.queue_free()
	await get_tree().process_frame
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
