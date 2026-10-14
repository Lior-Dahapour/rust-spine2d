extends Node2D
# A weighted mesh rebuilt from the numbers in the example's JSON through create_mesh_from_atlas
# must look exactly like the original attachment, while the skeleton animates. The JSON is read
# here, in the test; the extension never sees a file format. Needs a window.
# Usage: godot --path . --scene res://rebuilt_mesh_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

# Every mesh of the skin `skin_name` in the JSON, as {slot, placeholder, json}.
func meshes_in(document: Dictionary, skin_name: String) -> Array:
	var found := []
	for skin in document["skins"]:
		if skin["name"] != skin_name:
			continue
		for slot_name in skin["attachments"]:
			for placeholder in skin["attachments"][slot_name]:
				var entry: Dictionary = skin["attachments"][slot_name][placeholder]
				if entry.get("type", "region") == "mesh":
					found.append({"slot": slot_name, "placeholder": placeholder, "json": entry})
	return found

# Declares the bones and constraints that `from` requires, as the real skin does.
func add_requirements(skin: SpineSkin, from: SpineSkin) -> void:
	for bone in from.get_bones():
		skin.add_bone(bone)
	for constraint in from.get_constraints():
		skin.add_constraint(constraint)

func make_sprite(data: SpineSkeletonDataResource) -> SpineSprite:
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	sprite.update_mode = 2 # manual, so the sprites advance by exactly the same steps
	sprite.cull_offscreen = false # a hidden sprite counts as off screen and would skip drawing
	sprite.initial_animation = "dance"
	sprite.position = Vector2(450, 640)
	sprite.scale = Vector2(0.9, 0.9)
	add_child(sprite)
	return sprite

func step(sprite: SpineSprite, times: int) -> void:
	for i in times:
		sprite.update_skeleton(1.0 / 30.0)

func snapshot(sprite: SpineSprite, others: Array) -> Image:
	for other in others:
		other.visible = false
	sprite.visible = true
	await frames(2)
	await RenderingServer.frame_post_draw
	return get_viewport().get_texture().get_image()

func differing_pixels(a: Image, b: Image) -> int:
	var count := 0
	for y in a.get_height():
		for x in a.get_width():
			var p := a.get_pixel(x, y)
			var q := b.get_pixel(x, y)
			if absf(p.r - q.r) > 0.02 or absf(p.g - q.g) > 0.02 or absf(p.b - q.b) > 0.02 or absf(p.a - q.a) > 0.02:
				count += 1
	return count

func visible_pixels(image: Image) -> int:
	var background := image.get_pixel(5, 5)
	var count := 0
	for y in range(0, image.get_height(), 2):
		for x in range(0, image.get_width(), 2):
			var p := image.get_pixel(x, y)
			if absf(p.r - background.r) > 0.05 or absf(p.g - background.g) > 0.05 or absf(p.b - background.b) > 0.05:
				count += 1
	return count

func _ready() -> void:
	var document: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://assets/mix-and-match/mix-and-match-pro.json"))
	var atlas: SpineAtlasResource = load("res://assets/mix-and-match/mix-and-match.atlas")
	var data := SpineSkeletonDataResource.new()
	data.atlas = atlas
	data.skeleton_file = load("res://assets/mix-and-match/mix-and-match-pro.skel")

	var meshes := meshes_in(document, "clothes/dress-blue")
	check("the outfit has meshes to rebuild", meshes.size() > 0)
	var weighted := false
	for mesh in meshes:
		var json: Dictionary = mesh["json"]
		weighted = weighted or json["vertices"].size() != json["uvs"].size()
	check("and at least one is weighted to bones", weighted)

	# Rebuild every mesh of the outfit from its numbers and put them in a skin of their own.
	var rebuilt := SpineSkin.create("rebuilt")
	var original := SpineSkin.create("original")
	var original_skin := data.find_skin("clothes/dress-blue")
	var rebuilt_all := true
	for mesh in meshes:
		var json: Dictionary = mesh["json"]
		var slot := data.get_slot_index(mesh["slot"])
		var region_name: String = json.get("path", json.get("name", mesh["placeholder"]))
		var made := SpineAttachment.create_mesh_from_atlas(
			json.get("name", mesh["placeholder"]), atlas, region_name,
			PackedFloat32Array(json["uvs"]), PackedInt32Array(json["triangles"]),
			PackedFloat32Array(json["vertices"]), int(json.get("hull", 0)))
		if made == null:
			rebuilt_all = false
			continue
		rebuilt.set_attachment(slot, mesh["placeholder"], made)
		original.set_attachment(slot, mesh["placeholder"], original_skin.get_attachment(slot, mesh["placeholder"]))
	check("every mesh could be rebuilt from its numbers", rebuilt_all)

	check("the outfit's skin requires bones and constraints of its own", original_skin.get_bones().size() > 0 and original_skin.get_constraints().size() > 0)
	var bare := SpineSkin.create("bare")
	for mesh in meshes:
		var slot_b := data.get_slot_index(mesh["slot"])
		bare.set_attachment(slot_b, mesh["placeholder"], rebuilt.get_attachment(slot_b, mesh["placeholder"]))
	add_requirements(original, original_skin)
	add_requirements(rebuilt, original_skin)
	var copies := SpineSkin.create("copies")
	for mesh in meshes:
		var slot_i := data.get_slot_index(mesh["slot"])
		copies.set_attachment(slot_i, mesh["placeholder"], original_skin.get_attachment(slot_i, mesh["placeholder"]).copy())
	var a := make_sprite(data)
	var b := make_sprite(data)
	var c := make_sprite(data)
	var d := make_sprite(data)
	check("the original skin fits", a.set_custom_skin(original))
	check("the rebuilt skin fits", b.set_custom_skin(rebuilt))
	add_requirements(copies, original_skin)
	check("the copied skin fits", c.set_custom_skin(copies))
	check("the skin without requirements fits", d.set_custom_skin(bare))
	for mesh in meshes:
		a.get_slot(mesh["slot"]).set_attachment(mesh["placeholder"])
		b.get_slot(mesh["slot"]).set_attachment(mesh["placeholder"])
		c.get_slot(mesh["slot"]).set_attachment(mesh["placeholder"])
		d.get_slot(mesh["slot"]).set_attachment(mesh["placeholder"])

	var all_equal := true
	var copies_equal := true
	var bare_collapses := true
	var drew_something := true
	for round in 3:
		# A hidden sprite is not redrawn, so all are visible while they step.
		a.visible = true
		b.visible = true
		c.visible = true
		d.visible = true
		step(a, 12)
		step(b, 12)
		step(c, 12)
		step(d, 12)
		var image_a := await snapshot(a, [b, c, d])
		var image_b := await snapshot(b, [a, c, d])
		var image_c := await snapshot(c, [a, b, d])
		var image_d := await snapshot(d, [a, b, c])
		if round == 0 and OS.get_cmdline_user_args().size() > 0:
			image_a.save_png(OS.get_cmdline_user_args()[0] + "/rebuilt_original.png")
			image_b.save_png(OS.get_cmdline_user_args()[0] + "/rebuilt_rebuilt.png")
		var different := differing_pixels(image_a, image_b)
		var copy_different := differing_pixels(image_a, image_c)
		var bare_different := differing_pixels(image_a, image_d)
		drew_something = drew_something and visible_pixels(image_a) > 500
		print("round ", round, ": rebuilt ", different, ", copy ", copy_different, " differing pixels")
		# Godot's JSON parser and the runtime's own read some numbers one unit in the last place
		# apart, which can move an edge pixel; a different mesh would differ in thousands.
		all_equal = all_equal and different <= 20
		copies_equal = copies_equal and copy_different == 0
		bare_collapses = bare_collapses and (round == 0 or bare_different > 1000)
	check("the outfit is drawn", drew_something)
	check("the rebuilt meshes look the same as the originals while dancing", all_equal)
	check("copies of the originals look exactly like them while dancing", copies_equal)
	check("without the required bones and constraints the mesh looks different", bare_collapses)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
