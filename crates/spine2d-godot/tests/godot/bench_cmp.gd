extends Node2D
# Compares spine2d-godot with the official spine-godot extension using the same scene. Detects which
# plugin is loaded. Usage: godot --path . --scene res://bench_cmp.tscn -- <dir> <skeleton.skel> <atlas> <animation> <count> <frames> <scale>
func _ready() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	var a := OS.get_cmdline_user_args()
	var official := ClassDB.class_exists("SpineAnimationState")
	var base := "res://assets/%s/" % a[0]
	var count := int(a[4])
	var frames := int(a[5])
	var sc := float(a[6])
	var data
	if official:
		var atlas := SpineAtlasResource.new()
		atlas.load_from_atlas_file(base + a[2])
		var file := SpineSkeletonFileResource.new()
		file.load_from_file(base + a[1])
		data = SpineSkeletonDataResource.new()
		data.atlas_res = atlas
		data.skeleton_file_res = file
	else:
		data = SpineSkeletonDataResource.new()
		data.atlas = load(base + a[2])
		data.skeleton_file = load(base + a[1])
	var cols := int(ceil(sqrt(count * 1.5)))
	var rows := int(ceil(float(count) / cols))
	var dx := 860.0 / cols
	var dy := 560.0 / rows
	var sprites: Array = []
	for i in count:
		var s := SpineSprite.new()
		if official:
			s.skeleton_data_res = data
			s.update_mode = 2
		else:
			s.skeleton_data = data
			s.update_mode = 2
			s.cull_offscreen = false # the official plugin has no culling: compare like with like
		s.position = Vector2(20 + (i % cols) * dx + dx * 0.5, 80 + (i / cols) * dy + dy)
		s.scale = Vector2(sc, sc)
		add_child(s)
		if official:
			s.get_animation_state().set_animation(a[3], true, 0)
		else:
			s.initial_animation = a[3]
		sprites.append(s)
	for i in 40: # warm up
		for s in sprites: s.update_skeleton(1.0 / 60.0)
		await get_tree().process_frame
	var script_us := 0.0
	var frame_us := 0.0
	var t_last := Time.get_ticks_usec()
	for i in frames:
		var t0 := Time.get_ticks_usec()
		for s in sprites: s.update_skeleton(1.0 / 60.0)
		script_us += Time.get_ticks_usec() - t0
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frame_us += now - t_last
		t_last = now
	print("RESULT %s count=%d | update_skeleton=%.1f us/skeleton | whole frame=%.1f us (%.2f us/skeleton) | draw_calls=%d | mem=%.0f MB" % [
		"official" if official else "ours", count, script_us / frames / count, frame_us / frames, frame_us / frames / count,
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME), Performance.get_monitor(Performance.MEMORY_STATIC) / 1048576.0])
	get_tree().quit()
