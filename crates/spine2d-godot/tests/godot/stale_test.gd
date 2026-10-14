extends Node2D
# Handles a script keeps (bones, slots, track entries) must not keep a replaced or freed sprite
# on screen. Needs a window: it counts the draw calls the render server reports.
var failures := 0
var boy: SpineSkeletonDataResource
var alien: SpineSkeletonDataResource

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

func draw_calls() -> int:
	await RenderingServer.frame_post_draw
	return int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))

func make(dir: String, skel: String, atlas_name: String) -> SpineSkeletonDataResource:
	var d := SpineSkeletonDataResource.new()
	d.atlas = load("res://assets/%s/%s" % [dir, atlas_name])
	d.skeleton_file = load("res://assets/%s/%s" % [dir, skel])
	return d

func spawn(data: SpineSkeletonDataResource, animation: String) -> SpineSprite:
	var s := SpineSprite.new()
	s.skeleton_data = data
	s.initial_animation = animation
	s.position = Vector2(450, 600)
	s.scale = Vector2(0.5, 0.5)
	add_child(s)
	return s

# Swaps a sprite from spineboy to the alien and returns the draw calls afterwards.
func swap_and_count(hold: bool) -> Array:
	var s := spawn(boy, "run")
	await frames(15)
	var held := []
	if hold:
		held = [s.get_bone("head"), s.get_slot("head"), s.get_current(0)]
	s.skeleton_data = alien
	s.initial_animation = "run"
	await frames(15)
	var calls := await draw_calls()
	return [s, calls, held]

func _ready() -> void:
	boy = make("spineboy", "spineboy-pro.skel", "spineboy.atlas")
	alien = make("alien", "alien-pro.skel", "alien.atlas")
	await frames(5)
	var base := await draw_calls()

	var control := await swap_and_count(false)
	control[0].queue_free()
	await frames(5)

	var held_run := await swap_and_count(true)
	var sprite: SpineSprite = held_run[0]
	var bone: SpineBone = held_run[2][0]
	var slot: SpineSlot = held_run[2][1]
	var entry: SpineTrackEntry = held_run[2][2]
	check("no ghost: same draw calls as when nothing is held", held_run[1] == control[1])
	check("held bone reports invalid after the reload", not bone.is_valid())
	check("held slot reports invalid after the reload", not slot.is_valid())
	check("held track entry reports invalid after the reload", not entry.is_valid())
	bone.set_rotation(45.0)
	slot.set_color(Color.RED)
	check("writing through dead handles is harmless", not slot.set_attachment("anything"))
	check("sprite's own handles are valid", sprite.get_bone("head") != null and sprite.get_bone("head").is_valid())

	sprite.queue_free()
	await frames(5)
	check("freed sprite leaves nothing drawn", await draw_calls() == base)
	check("handles are invalid after the sprite is freed", not bone.is_valid())

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
