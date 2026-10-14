extends Node2D
var failures := 0
func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

# The render server reports visibility some frames after a move; wait for it instead of guessing.
func wait_for_screen_state(s: SpineSprite, on_screen: bool) -> void:
	for i in 300:
		if s.is_on_screen() == on_screen: return
		await get_tree().process_frame

func _ready() -> void:
	var data = SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var s := SpineSprite.new()
	s.skeleton_data = data
	s.initial_animation = "run"
	s.position = Vector2(450, 600)
	s.scale = Vector2(0.5, 0.5)
	add_child(s)
	await frames(20)
	check("on screen at first", s.is_on_screen())
	var before = s.get_mesh_stats().vertex_uploads
	await frames(30)
	check("uploads while visible", s.get_mesh_stats().vertex_uploads - before >= 20)

	s.position = Vector2(9000, 9000)
	await wait_for_screen_state(s, false)
	check("reported off screen", not s.is_on_screen())
	var uploads = s.get_mesh_stats().vertex_uploads
	var time_before: float = s.get_current(0).get_track_time()
	await frames(30)
	check("animation keeps running while culled", s.get_current(0).get_track_time() > time_before + 0.2)
	check("no uploads while culled", s.get_mesh_stats().vertex_uploads == uploads)

	s.position = Vector2(450, 600)
	await wait_for_screen_state(s, true)
	check("reported on screen again", s.is_on_screen())
	uploads = s.get_mesh_stats().vertex_uploads
	await frames(30)
	check("uploads resume", s.get_mesh_stats().vertex_uploads - uploads >= 20)

	s.cull_offscreen = false
	s.position = Vector2(9000, 9000)
	await frames(10)
	uploads = s.get_mesh_stats().vertex_uploads
	await frames(30)
	check("culling can be turned off", s.get_mesh_stats().vertex_uploads - uploads >= 20)
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
