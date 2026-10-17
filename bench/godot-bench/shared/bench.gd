extends Node2D
# Frame-time benchmark that runs unchanged against the official spine-godot extension and spine2d.
# Usage: <game> -- <asset dir> <skeleton file> <atlas file> <animation> <count> <frames> <scale>
# The engine updates and draws the sprites by itself (no per-frame script calls), vsync is off.

const WARMUP_FRAMES := 120
# A scene averaging more than this after SLOW_CHECK_FRAMES measured frames is cut short: the result
# is reported as "slower than this".
const SLOW_MS := 100.0
const SLOW_CHECK_FRAMES := 20
const VIEW_WIDTH := 860.0
const VIEW_HEIGHT := 560.0

const PROBE_BONES := ["tank-root", "wheel-mid-center", "tread", "tread2", "tread10", "tread20", "tread30"]

# Every scene beyond the plain ones below is driven by a script that only uses the API the official
# plugin has, so both plugins run exactly the same calls. The choices come from a generator with a
# fixed seed, so both get the same sequence.
#
# The "churn" scene: every frame a script moves sprites, starts, queues and clears animations,
# changes time scales and replaces a few sprites.
# The "mixed" asset directory spawns these five skeletons in turn: name, first animation, and the
# sprite scale relative to the one given on the command line (made for spineboy).
const MIXED := [
	["spineboy", "run", 1.0], ["alien", "run", 1.0], ["raptor", "walk", 0.75],
	["tank", "drive", 0.5], ["coin", "animation", 1.0]]
# The "constraints" scene spawns the same five kinds and, every frame, a script looks constraints up
# by name and reads and writes their pose.
const CONSTRAINT_ACCESSES := 300
const CONSTRAINTS := {
	"spineboy": [["ik", "aim-ik"], ["ik", "front-leg-ik"], ["ik", "rear-leg-ik"],
		["transform", "shoulder"], ["transform", "aim-head-transform"]],
	"alien": [["transform", "jaw"], ["transform", "eye"]],
	"raptor": [["ik", "front-leg-ik"], ["ik", "back-leg-ik"], ["ik", "stirrup"]],
	"tank": [["ik", "cannon-ik"], ["path", "treads-path"], ["transform", "wheel-big-transform"]],
	"coin": [],
}
# The "materials" scene: half the sprites start with a custom material per blend mode; every frame
# a few get one, lose it, or swap it for another one. The cost is in the draw calls the engine
# needs for the different materials.
const MATERIAL_CHANGES := 40
# The "skins" scene (mix-and-match): every frame a few sprites get a new skin built from the base
# skin and random accessories.
const SKIN_CHANGES := 10
const SKIN_ACCESSORIES := 3
# The "nodes" scene: every sprite has a bone node following its head, one driving its front hand,
# and a slot node on its gun; the driving one is moved by the script every frame.
# The "events" scene: every sprite listens to its animation events and the script counts them.
const SCRIPTED_SCENES := ["churn", "constraints", "materials", "skins", "nodes", "events"]
const CHURN_MOVES := 150
const CHURN_SET := 30
const CHURN_QUEUE := 30
const CHURN_EMPTY := 10
const CHURN_CLEAR := 5
const CHURN_TIME_SCALE := 10
const CHURN_REPLACE := 3

# Both plugins register the same classes, so the plugin is told apart by a property only spine2d has.
var _official := not ClassDB.class_get_property_list("SpineSprite").any(
	func(property): return property.name == "cull_offscreen")
var _first_sprite: Node
var _scene := ""
var _churn := false
var _constraints := false
var _materials: Array[Material] = []
var _skin_names: Array[String] = []
var _accessory_names: Array[String] = []
var _drivers: Array[Node2D] = []
var _events := 0
var _rng := RandomNumberGenerator.new()
var _sprites: Array[Node2D] = []
# One entry per skeleton kind: the data, its first animation, all its animations and its scale.
var _kinds: Array[Dictionary] = []
var _columns: int
var _step_x: float
var _step_y: float


func _ready() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	Engine.max_fps = 0
	var args := OS.get_cmdline_user_args()
	if args.size() < 7:
		push_error("usage: -- <asset dir> <skeleton> <atlas> <animation> <count> <frames> <scale>")
		get_tree().quit(2)
		return
	var count := int(args[4])
	var frames := int(args[5])
	_scene = args[3]
	_churn = _scene == "churn"
	_constraints = _scene == "constraints"
	_make_materials()
	_rng.seed = 12345
	_load_kinds(args)
	_collect_accessories()
	_spawn(count)
	var warmup := int(args[8]) if args.size() > 8 else WARMUP_FRAMES
	for i in warmup:
		_churn_step()
		await get_tree().process_frame
	if args.size() > 7:
		await _save_shots(args[7])
	await _measure(count, frames, args[0])
	get_tree().quit()


func _load_kinds(args: PackedStringArray) -> void:
	var scale := float(args[6])
	if args[0] == "mixed":
		for entry in MIXED:
			_add_kind(entry[0], entry[1], scale * entry[2])
		return
	var animation := args[3]
	if _scene in SCRIPTED_SCENES:
		animation = "walk" if _scene == "skins" else "run"
	_add_kind(args[0], animation, scale, args[1], args[2])


func _collect_accessories() -> void:
	if _scene != "skins":
		return
	for skin in _kinds[0].data.get_skins():
		var skin_name: String = skin.get_name()
		if skin_name.begins_with("accessories/"):
			_accessory_names.append(skin_name)


func _add_kind(
		asset: String, animation: String, sprite_scale: float,
		skeleton := "", atlas := "") -> void:
	var base := "res://assets/%s/" % asset
	var data := _load_data(
		base + (skeleton if skeleton != "" else asset + "-pro.skel"),
		base + (atlas if atlas != "" else asset + ".atlas"))
	var names := []
	if _churn:
		for candidate in data.get_animations():
			names.append(candidate.get_name())
	_kinds.append({"asset": asset, "data": data, "animation": animation, "names": names,
		"scale": sprite_scale})


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


func _spawn(count: int) -> void:
	_columns = int(ceil(sqrt(count * 1.5)))
	var rows := int(ceil(float(count) / _columns))
	_step_x = VIEW_WIDTH / _columns
	_step_y = VIEW_HEIGHT / rows
	for i in count:
		var sprite := _make_sprite(i)
		add_child(sprite)
		_sprites.append(sprite)
		if i == 0:
			_first_sprite = sprite
		if _official:
			sprite.get_animation_state().set_animation(
				_kinds[i % _kinds.size()].animation, true, 0)
		_decorate(sprite, i)


func _make_sprite(index: int) -> Node2D:
	var kind := _kinds[index % _kinds.size()]
	var sprite := SpineSprite.new()
	if _official:
		sprite.skeleton_data_res = kind.data
	else:
		sprite.skeleton_data = kind.data
		sprite.cull_offscreen = false # the official plugin cannot cull, keep the work equal
		sprite.initial_animation = kind.animation
	sprite.position = Vector2(
		20 + (index % _columns) * _step_x + _step_x * 0.5,
		80 + (index / _columns) * _step_y + _step_y)
	sprite.scale = Vector2(kind.scale, kind.scale)
	return sprite


func _make_materials() -> void:
	for mode in [CanvasItemMaterial.BLEND_MODE_MIX, CanvasItemMaterial.BLEND_MODE_ADD,
			CanvasItemMaterial.BLEND_MODE_MUL, CanvasItemMaterial.BLEND_MODE_PREMULT_ALPHA]:
		var material := CanvasItemMaterial.new()
		material.blend_mode = mode
		_materials.append(material)


# Per-scene setup of a sprite that is already in the tree.
func _decorate(sprite: Node2D, index: int) -> void:
	match _scene:
		"materials":
			if index % 2 == 0:
				_set_material(sprite, _materials[index % _materials.size()])
		"events":
			sprite.animation_event.connect(_on_animation_event)
		"nodes":
			var head := SpineBoneNode.new()
			head.bone_name = "head"
			sprite.add_child(head)
			var gun := SpineSlotNode.new()
			gun.slot_name = "gun"
			sprite.add_child(gun)
			var hand := SpineBoneNode.new()
			hand.bone_name = "front-fist"
			hand.bone_mode = 1
			sprite.add_child(hand)
			_drivers.append(hand)


func _on_animation_event(_sprite, _state, _entry, _event) -> void:
	_events += 1


func _set_material(sprite: Node2D, material: Material) -> void:
	sprite.set_normal_material(material)
	sprite.set_additive_material(material)
	sprite.set_multiply_material(material)
	sprite.set_screen_material(material)


func _random_index() -> int:
	return _rng.randi_range(0, _sprites.size() - 1)


# A random animation of the skeleton kind at `index`.
func _random_animation(index: int) -> String:
	var names: Array = _kinds[index % _kinds.size()].names
	return names[_rng.randi_range(0, names.size() - 1)]


# One frame of scripted changes; does nothing outside the churn scene.
func _churn_step() -> void:
	match _scene:
		"constraints":
			_constraint_step()
			return
		"materials":
			_material_step()
			return
		"skins":
			_skin_step()
			return
		"nodes":
			_node_step()
			return
	if not _churn or _sprites.is_empty():
		return
	for i in CHURN_MOVES:
		_sprites[_random_index()].position += Vector2(
			_rng.randf_range(-3.0, 3.0), _rng.randf_range(-3.0, 3.0))
	for i in CHURN_SET:
		var index := _random_index()
		_sprites[index].get_animation_state().set_animation(_random_animation(index), true, 0)
	for i in CHURN_QUEUE:
		var index := _random_index()
		_sprites[index].get_animation_state().add_animation(_random_animation(index), 0.0, true, 0)
	for i in CHURN_EMPTY:
		_sprites[_random_index()].get_animation_state().set_empty_animation(0, 0.2)
	for i in CHURN_CLEAR:
		_sprites[_random_index()].get_animation_state().clear_track(0)
	for i in CHURN_TIME_SCALE:
		_sprites[_random_index()].get_animation_state().set_time_scale(_rng.randf_range(0.5, 1.5))
	for i in CHURN_REPLACE:
		var index := _random_index()
		var old := _sprites[index]
		var replacement := _make_sprite(index)
		replacement.position = old.position
		old.queue_free()
		add_child(replacement)
		if _official:
			replacement.get_animation_state().set_animation(_kinds[index % _kinds.size()].animation, true, 0)
		_sprites[index] = replacement


# A few sprites get a material, lose it or swap it.
func _material_step() -> void:
	for i in MATERIAL_CHANGES:
		var sprite = _sprites[_random_index()]
		var pick := _rng.randi_range(0, _materials.size())
		_set_material(sprite, null if pick == _materials.size() else _materials[pick])


# A few sprites get a skin made of the base skin and some random accessories.
func _skin_step() -> void:
	for i in SKIN_CHANGES:
		var sprite = _sprites[_random_index()]
		var data = _kinds[0].data
		var skin = sprite.new_skin("custom")
		skin.add_skin(data.find_skin("skin-base"))
		for j in SKIN_ACCESSORIES:
			skin.add_skin(data.find_skin(_accessory_names[_rng.randi_range(0, _accessory_names.size() - 1)]))
		var skeleton = sprite.get_skeleton()
		skeleton.set_skin(skin)
		skeleton.set_slots_to_setup_pose()


# The driving bone nodes move a little every frame.
func _node_step() -> void:
	var t := Time.get_ticks_msec() / 1000.0
	for node in _drivers:
		node.position = Vector2(sin(t) * 20.0, cos(t) * 20.0)


# One frame of constraint access: look up a constraint of a random sprite by name, read its pose and
# write a new value back.
func _constraint_step() -> void:
	for i in CONSTRAINT_ACCESSES:
		var index := _random_index()
		var list: Array = CONSTRAINTS[_kinds[index % _kinds.size()].asset]
		var value := _rng.randf_range(0.2, 1.0)
		if list.is_empty():
			continue
		var entry: Array = list[_rng.randi_range(0, list.size() - 1)]
		var skeleton = _sprites[index].get_skeleton()
		match entry[0]:
			"ik":
				var pose = skeleton.find_ik_constraint(entry[1]).get_pose()
				pose.set_mix(minf(pose.get_mix(), 1.0) * 0.5 + value * 0.5)
			"transform":
				var pose = skeleton.find_transform_constraint(entry[1]).get_pose()
				pose.set_mix_rotate(minf(pose.get_mix_rotate(), 1.0) * 0.5 + value * 0.5)
			"path":
				var pose = skeleton.find_path_constraint(entry[1]).get_pose()
				pose.set_position(fposmod(pose.get_position() + value * 0.01, 1.0))


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


func _mean_ms(frame_us: PackedFloat64Array) -> float:
	var total := 0.0
	for value in frame_us:
		total += value
	return total / frame_us.size() / 1000.0


# A cheap check that the scene really did its work, so a number is never taken from a scene that
# silently did nothing.
func _check() -> bool:
	match _scene:
		"events":
			return _events > 0
		"skins":
			return _sprites.any(func(sprite): return sprite.get_skeleton().get_skin() != null \
				and sprite.get_skeleton().get_skin().get_name() == "custom")
		"nodes":
			var sprite = _sprites[0]
			var bone_position: Vector2 = sprite.get_global_bone_transform("head").origin
			for child in sprite.get_children():
				if child is SpineBoneNode and child.bone_name == "head":
					return child.global_position.distance_to(bone_position) < 20.0
			return false
		"materials":
			return _sprites.any(func(sprite): return sprite.get_normal_material() != null)
	return true


func _measure(count: int, frames: int, scene: String) -> void:
	var frame_us := PackedFloat64Array()
	var last := Time.get_ticks_usec()
	var slow := 0
	for i in frames:
		_churn_step()
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frame_us.append(now - last)
		last = now
		if frame_us.size() == SLOW_CHECK_FRAMES and _mean_ms(frame_us) > SLOW_MS:
			slow = 1
			frames = frame_us.size()
			break
	var check_ok := _check()
	var sorted := frame_us.duplicate()
	sorted.sort()
	var total := 0.0
	for value in frame_us:
		total += value
	var mean_ms := total / frames / 1000.0
	var line := ("RESULT plugin=%s scene=%s count=%d frames=%d mean_ms=%.3f median_ms=%.3f "
		+ "p99_ms=%.3f fps=%.1f draw_calls=%d primitives=%d renderer=%s build=%s focused=%s events=%d slow=%d check=%d")
	print(line % [
		"official" if _official else "ours", scene, count, frames, mean_ms,
		sorted[frames >> 1] / 1000.0, sorted[int(frames * 0.99)] / 1000.0, 1000.0 / mean_ms,
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		RenderingServer.get_current_rendering_method(),
		"debug" if OS.is_debug_build() else "release",
		str(DisplayServer.window_is_focused()), _events, slow, int(check_ok)])
