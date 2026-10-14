extends Node2D
# Frame-time benchmark that runs unchanged against the official spine-godot extension and spine2d.
# Usage: <game> -- <asset dir> <skeleton file> <atlas file> <animation> <count> <frames> <scale>
# The engine updates and draws the sprites by itself (no per-frame script calls), vsync is off.

const WARMUP_FRAMES := 120
const VIEW_WIDTH := 860.0
const VIEW_HEIGHT := 560.0

const PROBE_BONES := ["tank-root", "wheel-mid-center", "tread", "tread2", "tread10", "tread20", "tread30"]

var _official := ClassDB.class_exists("SpineAnimationState")
var _first_sprite: Node


func _ready() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	Engine.max_fps = 0
	var args := OS.get_cmdline_user_args()
	if args.size() < 7:
		push_error("usage: -- <asset dir> <skeleton> <atlas> <animation> <count> <frames> <scale>")
		get_tree().quit(2)
		return
	var base := "res://assets/%s/" % args[0]
	var count := int(args[4])
	var frames := int(args[5])
	var data := _load_data(base + args[1], base + args[2])
	_spawn(data, args[3], count, float(args[6]))
	var warmup := int(args[8]) if args.size() > 8 else WARMUP_FRAMES
	for i in warmup:
		await get_tree().process_frame
	if args.size() > 7:
		await _save_shots(args[7])
	await _measure(count, frames, args[0])
	get_tree().quit()


func _load_data(skeleton_path: String, atlas_path: String) -> Resource:
	var data := SpineSkeletonDataResource.new()
	if _official:
		var atlas := SpineAtlasResource.new()
		atlas.load_from_atlas_file(atlas_path)
		var file := SpineSkeletonFileResource.new()
		file.load_from_file(skeleton_path)
		data.atlas_res = atlas
		data.skeleton_file_res = file
	else:
		data.atlas = load(atlas_path)
		data.skeleton_file = load(skeleton_path)
	return data


func _spawn(data: Resource, animation: String, count: int, sprite_scale: float) -> void:
	var columns := int(ceil(sqrt(count * 1.5)))
	var rows := int(ceil(float(count) / columns))
	var step_x := VIEW_WIDTH / columns
	var step_y := VIEW_HEIGHT / rows
	for i in count:
		var sprite := SpineSprite.new()
		if _official:
			sprite.skeleton_data_res = data
		else:
			sprite.skeleton_data = data
			sprite.cull_offscreen = false # the official plugin cannot cull, keep the work equal
		sprite.position = Vector2(
			20 + (i % columns) * step_x + step_x * 0.5, 80 + (i / columns) * step_y + step_y)
		sprite.scale = Vector2(sprite_scale, sprite_scale)
		add_child(sprite)
		if i == 0:
			_first_sprite = sprite
		if _official:
			sprite.get_animation_state().set_animation(animation, true, 0)
		else:
			sprite.initial_animation = animation


# Optional check, only when a path prefix is given as the eighth argument (the ninth then sets the
# warm-up length): two screenshots taken 20 frames apart, to confirm that both plugins draw the same
# thing and keep animating. Run with `--fixed-fps 60` so both are at the same animation time.
func _save_shots(prefix: String) -> void:
	for suffix in ["a", "b"]:
		await RenderingServer.frame_post_draw
		get_viewport().get_texture().get_image().save_png("%s-%s.png" % [prefix, suffix])
		if suffix == "a":
			_probe()
		for i in 20:
			await get_tree().process_frame


# Prints the world position and rotation of a few bones of the first sprite, so the poses of the two
# plugins can be compared as numbers.
func _probe() -> void:
	for bone_name in PROBE_BONES:
		var bone = _first_sprite.get_skeleton().find_bone(bone_name) if _official \
			else _first_sprite.get_bone(bone_name)
		if bone == null:
			print("PROBE %s none" % bone_name)
			continue
		var x := 0.0
		var y := 0.0
		var rotation := 0.0
		if _official:
			var pose = bone.get_applied_pose()
			x = pose.get_world_x()
			y = pose.get_world_y()
			rotation = pose.get_world_rotation_x()
		else:
			var position: Vector2 = bone.get_world_position()
			x = position.x
			y = position.y
			rotation = bone.get_world_rotation()
		print("PROBE %s x=%.4f y=%.4f rot=%.4f" % [bone_name, x, y, rotation])


func _measure(count: int, frames: int, scene: String) -> void:
	var frame_us := PackedFloat64Array()
	var last := Time.get_ticks_usec()
	for i in frames:
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frame_us.append(now - last)
		last = now
	var sorted := frame_us.duplicate()
	sorted.sort()
	var total := 0.0
	for value in frame_us:
		total += value
	var mean_ms := total / frames / 1000.0
	var line := ("RESULT plugin=%s scene=%s count=%d frames=%d mean_ms=%.3f median_ms=%.3f "
		+ "p99_ms=%.3f fps=%.1f draw_calls=%d primitives=%d renderer=%s build=%s focused=%s")
	print(line % [
		"official" if _official else "ours", scene, count, frames, mean_ms,
		sorted[frames >> 1] / 1000.0, sorted[int(frames * 0.99)] / 1000.0, 1000.0 / mean_ms,
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		RenderingServer.get_current_rendering_method(),
		"debug" if OS.is_debug_build() else "release",
		str(DisplayServer.window_is_focused())])
