extends Node2D
# Needs the extension built with `--features profiling` for the pose/render/upload columns.
# Usage: godot --path . --scene res://bench.tscn -- <dir> <skeleton> <atlas> <animation> <count> <frames> [scale]
func _ready() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	var a := OS.get_cmdline_user_args()
	var atlas := SpineAtlasResource.new()
	atlas.source_file = "res://assets/%s/%s" % [a[0], a[2]]
	var file := SpineSkeletonFileResource.new()
	file.source_file = "res://assets/%s/%s" % [a[0], a[1]]
	var data := SpineSkeletonDataResource.new()
	data.atlas = atlas
	data.skeleton_file = file
	var count := int(a[4])
	var frames := int(a[5])
	var sc := float(a[6]) if a.size() > 6 else 0.2
	var sprites: Array[SpineSprite] = []
	for i in count:
		var s := SpineSprite.new()
		s.skeleton_data = data
		s.initial_animation = a[3]
		s.cull_offscreen = a.size() <= 7 or a[7] != "0"
		s.update_mode = 2 # manual: this script drives the updates so it can time them
		s.position = Vector2(60 + (i % 20) * 42, 120 + (i / 20) * 70)
		s.scale = Vector2(sc, sc)
		add_child(s)
		sprites.append(s)
	for i in 30: # warm up
		for s in sprites: s.update_skeleton(1.0 / 60.0)
		await get_tree().process_frame
	for s in sprites: s.take_timings()
	var update_us := 0.0
	var frame_us := 0.0
	var t_last := Time.get_ticks_usec()
	for i in frames:
		var t0 := Time.get_ticks_usec()
		for s in sprites: s.update_skeleton(1.0 / 60.0)
		update_us += Time.get_ticks_usec() - t0
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frame_us += now - t_last
		t_last = now
	var pose := 0.0; var render := 0.0; var upload := 0.0; var rebuilds := 0; var detaches := 0; var index_uploads := 0
	for s in sprites:
		var t := s.take_timings(); pose += t.pose_us; render += t.render_us; upload += t.upload_us
		var st := s.get_mesh_stats(); rebuilds += st.rebuilds; detaches += st.detaches; index_uploads += st.index_uploads
	print("RESULT count=%d update_total_us/frame=%.1f (per skeleton %.2f) | pose=%.2f render=%.2f upload=%.2f us/skeleton | engine frame=%.1f us | draw_calls=%d | rebuilds=%d detaches=%d index_uploads=%d" % [
		count, update_us / frames, update_us / frames / count, pose / count, render / count, upload / count, frame_us / frames,
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME), rebuilds, detaches, index_uploads])
	get_tree().quit()
