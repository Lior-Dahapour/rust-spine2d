extends Node2D
# Attachments made from script: create_region (textures built in memory), their setters and the
# rules for editing them. Needs a window: it reads pixels.
# Usage: godot --path . --scene res://attachment_test.tscn [-- <screenshot dir>]
var failures := 0
var data: SpineSkeletonDataResource
var sprite: SpineSprite
var last_slot: String
var shots := ""

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

func texture_of(left: Color, right: Color) -> ImageTexture:
	var image := Image.create(64, 64, false, Image.FORMAT_RGBA8)
	for x in 64:
		for y in 64:
			image.set_pixel(x, y, left if x < 32 else right)
	return ImageTexture.create_from_image(image)

func is_color(c: Color, want: Color) -> bool:
	return absf(c.r - want.r) < 0.06 and absf(c.g - want.g) < 0.06 and absf(c.b - want.b) < 0.06

# Window pixel under the bone of the gun slot, plus an offset in skeleton units.
func pixel_near_bone(offset: Vector2 = Vector2.ZERO) -> Color:
	await RenderingServer.frame_post_draw
	var bone := sprite.get_slot(last_slot).get_bone()
	var at := sprite.to_global(bone.get_world_position() + offset)
	return get_viewport().get_texture().get_image().get_pixelv(at)

# Shows `attachment` in the gun slot and waits for it to appear.
func show_attachment(attachment: SpineAttachment) -> void:
	var skin := SpineSkin.create("test")
	skin.set_attachment(data.get_slot_index(last_slot), "box", attachment)
	sprite.set_custom_skin(skin)
	sprite.get_slot(last_slot).set_attachment("box")
	await frames(3)

# Whether the object behind `reference` still exists. The check lives in its own function because
# GDScript keeps the temporaries of a function alive until their stack slot is reused, which would
# keep the very object being tested.
func alive(reference: WeakRef) -> bool:
	return reference.get_ref() != null

# An attachment and a weak reference to its texture. The texture itself is only held by the
# attachment: this function's own reference is gone when it returns.
func make_with_weak_texture() -> Array:
	var texture := texture_of(Color.RED, Color.RED)
	var attachment := SpineAttachment.create_region("life", texture, Rect2(), Vector2(40, 40))
	return [attachment, weakref(texture)]

# Makes an attachment, sets it on `slot` and returns a weak reference to its texture. Nothing but
# the sprite keeps the attachment once this returns.
func set_new_attachment_on(slot: SpineSlot) -> WeakRef:
	var made := make_with_weak_texture()
	slot.set_attachment_object(made[0])
	return made[1]

func save_shot(name: String) -> void:
	if shots == "":
		return
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png("%s/%s.png" % [shots, name])

func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	shots = args[0] if args.size() > 0 else ""
	data = SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	sprite = SpineSprite.new()
	sprite.skeleton_data = data
	sprite.position = Vector2(450, 400)
	sprite.scale = Vector2(0.5, 0.5)
	add_child(sprite)
	await frames(3)
	# The gun slot is drawn after everything except the muzzle flash, which is hidden at rest and
	# uses additive blending, so a normal-blend attachment on the gun is not covered or tinted.
	last_slot = "gun"
	check("the gun slot exists", data.get_slot_index(last_slot) >= 0)

	var red := texture_of(Color.RED, Color.RED)
	var split := texture_of(Color.RED, Color.BLUE)

	var box := SpineAttachment.create_region("box", red, Rect2(), Vector2(40, 40))
	check("create_region makes a region attachment", box != null and box.get_type() == "region" and box.get_attachment_name() == "box")
	check("a rectangle outside the texture is refused", SpineAttachment.create_region("bad", red, Rect2(40, 0, 40, 40)) == null)
	check("an empty name is refused", SpineAttachment.create_region("", red) == null)

	await show_attachment(box)
	await save_shot("attachment_region")
	check("the image is drawn at its slot's bone", is_color(await pixel_near_bone(), Color.RED))
	check("it is only as big as its size", not is_color(await pixel_near_bone(Vector2(60, 0)), Color.RED))
	check("an attachment that is in a skin cannot be edited", not box.set_position(Vector2(10, 10)))

	# An attachment's position is in the axes of its slot's bone, which is rotated.
	var bone := sprite.get_slot(last_slot).get_bone()
	var along_bone := Vector2.RIGHT.rotated(deg_to_rad(bone.get_world_rotation())) * 100.0
	var moved := SpineAttachment.create_region("moved", red, Rect2(), Vector2(40, 40))
	check("a new attachment can be moved", moved.set_position(Vector2(100, 0)))
	await show_attachment(moved)
	check("set_position moves it along the bone", is_color(await pixel_near_bone(along_bone), Color.RED))
	check("and it is no longer at the bone", not is_color(await pixel_near_bone(), Color.RED))

	var part := SpineAttachment.create_region("part", split, Rect2(32, 0, 32, 64), Vector2(40, 40))
	await show_attachment(part)
	check("rect picks the right half of the texture", is_color(await pixel_near_bone(), Color.BLUE))

	var tinted := SpineAttachment.create_region("tinted", texture_of(Color.WHITE, Color.WHITE), Rect2(), Vector2(40, 40))
	check("set_color tints a region", tinted.set_color(Color.GREEN) and tinted.set_rotation(45.0) and tinted.set_scale(Vector2(1.5, 1.5)) and tinted.set_size(Vector2(50, 50)))
	await show_attachment(tinted)
	check("the tint shows", is_color(await pixel_near_bone(), Color.GREEN))

	var plain_copy := box.copy()
	check("a copy can be edited", plain_copy.set_position(Vector2(5, 5)))

	var mesh_source := data.find_skin("default").get_attachments()
	var mesh: SpineAttachment = null
	for entry in mesh_source:
		if entry.get_attachment().get_type() == "mesh":
			mesh = entry.get_attachment()
			break
	check("only region attachments can be edited", mesh != null and not mesh.copy().set_position(Vector2.ONE))

	# Meshes. A quad of 60 units around the bone, unweighted and weighted to the gun's own bone.
	var quad_uvs := PackedFloat32Array([0, 0, 1, 0, 1, 1, 0, 1])
	var quad_triangles := PackedInt32Array([0, 1, 2, 2, 3, 0])
	var corners := [Vector2(-30, -30), Vector2(30, -30), Vector2(30, 30), Vector2(-30, 30)]
	var plain_vertices := PackedFloat32Array()
	for c in corners:
		plain_vertices.append_array([c.x, c.y])
	var mesh_unweighted := SpineAttachment.create_mesh("quad", red, quad_uvs, quad_triangles, plain_vertices, 4)
	check("create_mesh makes a mesh", mesh_unweighted != null and mesh_unweighted.get_type() == "mesh")
	await show_attachment(mesh_unweighted)
	await save_shot("attachment_mesh")
	check("an unweighted mesh is drawn at the bone", is_color(await pixel_near_bone(), Color.RED))

	var gun_bone := data.get_bone_index(sprite.get_slot(last_slot).get_bone().get_name())
	var weighted_vertices := PackedFloat32Array()
	for c in corners:
		weighted_vertices.append_array([1, gun_bone, c.x, c.y, 1.0])
	var mesh_weighted := SpineAttachment.create_mesh("quad", red, quad_uvs, quad_triangles, weighted_vertices, 4)
	check("a weighted mesh is accepted", mesh_weighted != null)
	await show_attachment(mesh_weighted)
	check("a weighted mesh is drawn on its bone", is_color(await pixel_near_bone(), Color.RED))

	var missing_bone := PackedFloat32Array()
	for c in corners:
		missing_bone.append_array([1, 999, c.x, c.y, 1.0])
	var mesh_missing := SpineAttachment.create_mesh("quad", red, quad_uvs, quad_triangles, missing_bone, 4)
	check("a bone number that does not exist is only caught when shown", mesh_missing != null)
	var bad_skin := SpineSkin.create("bad")
	bad_skin.set_attachment(data.get_slot_index(last_slot), "box", mesh_missing)
	check("a skin with such a mesh is refused", not sprite.set_custom_skin(bad_skin))

	check("triangles past the last vertex are refused", SpineAttachment.create_mesh("m", red, quad_uvs, PackedInt32Array([0, 1, 7]), plain_vertices, 4) == null)
	check("a negative triangle value is refused", SpineAttachment.create_mesh("m", red, quad_uvs, PackedInt32Array([0, 1, -1]), plain_vertices, 4) == null)
	check("vertices that do not match the uvs are refused", SpineAttachment.create_mesh("m", red, quad_uvs, quad_triangles, PackedFloat32Array([0, 0, 1, 1]), 4) == null)
	check("a negative hull is refused", SpineAttachment.create_mesh("m", red, quad_uvs, quad_triangles, plain_vertices, -1) == null)

	# A linked mesh takes a mesh of the skeleton and gives it another image.
	# Use a mesh that its slot shows at rest, so the linked mesh shows up where it replaces it.
	sprite.set_slots_to_setup_pose()
	await frames(2)
	var slot_names := sprite.get_slot_names()
	var parent: SpineAttachment = null
	var parent_slot := -1
	var parent_name := ""
	for entry in data.find_skin("default").get_attachments():
		var shown_at_rest := sprite.get_slot(slot_names[entry.get_slot_index()]).get_attachment_name()
		if entry.get_attachment().get_type() == "mesh" and shown_at_rest == entry.get_attachment().get_attachment_name():
			parent = entry.get_attachment()
			parent_slot = entry.get_slot_index()
			parent_name = entry.get_name()
			break
	var linked := SpineAttachment.create_linked_mesh("recolored", parent, red)
	check("create_linked_mesh makes a mesh", linked != null and linked.get_type() == "mesh" and linked.get_attachment_name() == "recolored")
	check("a region cannot be the parent of a linked mesh", SpineAttachment.create_linked_mesh("x", box, red) == null)
	var recolor := SpineSkin.create("recolor")
	recolor.set_attachment(parent_slot, parent_name, linked)
	check("a linked mesh fits the skeleton", sprite.set_custom_skin(recolor))
	sprite.set_slots_to_setup_pose()
	await frames(3)
	await save_shot("attachment_linked")
	var shown := sprite.get_slot(slot_names[parent_slot]).get_attachment_name()
	check("the slot shows the linked mesh", shown == "recolored")

	# Alpha mode: one half-transparent red pixel is drawn differently depending on whether the
	# texture is declared premultiplied (rgb already scaled by alpha) or not.
	var half_image := Image.create(8, 8, false, Image.FORMAT_RGBA8)
	half_image.fill(Color(0.5, 0.0, 0.0, 0.5))
	var half_texture := ImageTexture.create_from_image(half_image)
	var straight := SpineAttachment.create_region("straight", half_texture, Rect2(), Vector2(40, 40), false)
	await show_attachment(straight)
	var straight_red := (await pixel_near_bone()).r
	var premultiplied := SpineAttachment.create_region("pma", half_texture, Rect2(), Vector2(40, 40), true)
	await show_attachment(premultiplied)
	var premultiplied_red := (await pixel_near_bone()).r
	check("the premultiplied flag changes how the texture is blended", premultiplied_red > straight_red + 0.1)

	# Lifetime: the texture lives as long as something shows or holds its attachment.
	var made := make_with_weak_texture()
	var weak: WeakRef = made[1]
	check("the attachment keeps its texture alive", alive(weak))
	var kept_skin := SpineSkin.create("kept")
	kept_skin.set_attachment(data.get_slot_index(last_slot), "box", made[0])
	made = []
	check("a skin keeps it alive after the attachment variable is gone", alive(weak))
	check("the sprite accepts the skin", sprite.set_custom_skin(kept_skin))
	sprite.get_slot(last_slot).set_attachment("box")
	kept_skin = null
	await frames(3)
	check("the sprite keeps it alive after the skin variable is gone", alive(weak))
	check("and it is still drawn", is_color(await pixel_near_bone(), Color.RED))
	var other := SpineSkin.create("other")
	other.set_attachment(data.get_slot_index(last_slot), "other", SpineAttachment.create_region("o", red, Rect2(), Vector2(40, 40)))
	sprite.set_custom_skin(other)
	await frames(2)
	check("a switched-away skin's texture stays while the slot may still show it", alive(weak))
	sprite.set_slots_to_setup_pose()
	check("after set_slots_to_setup_pose the old texture is freed", not alive(weak))

	var dropped := make_with_weak_texture()
	var dropped_weak: WeakRef = dropped[1]
	dropped = []
	check("an attachment nobody keeps frees its texture", not alive(dropped_weak))

	# Setting an attachment straight on a slot.
	sprite.set_slots_to_setup_pose()
	var direct := SpineAttachment.create_region("direct", red, Rect2(), Vector2(40, 40))
	var gun := sprite.get_slot(last_slot)
	check("set_attachment_object accepts an attachment", gun.set_attachment_object(direct))
	await frames(3)
	check("the slot reports it", gun.get_attachment_object() != null and gun.get_attachment_object().get_attachment_name() == "direct" and gun.get_attachment_name() == "direct")
	check("and it is drawn", is_color(await pixel_near_bone(), Color.RED))
	var plain_skin := SpineSkin.create("plain")
	check("switching skins leaves it alone", sprite.set_custom_skin(plain_skin))
	await frames(3)
	check("it is still drawn after the skin switch", is_color(await pixel_near_bone(), Color.RED))
	check("set_attachment by name replaces it", gun.set_attachment("gun") and gun.get_attachment_name() == "gun")
	gun.set_attachment_object(direct)
	sprite.set_slots_to_setup_pose()
	check("set_slots_to_setup_pose drops it", gun.get_attachment_name() == "gun")
	check("an attachment with a bone the skeleton lacks is refused", not gun.set_attachment_object(mesh_missing))
	check("the slot still shows what it showed", gun.get_attachment_name() == "gun")
	var direct_weak := set_new_attachment_on(gun)
	check("the sprite keeps the texture of a directly set attachment", alive(direct_weak))
	sprite.set_slots_to_setup_pose()
	check("and frees it after the reset", not alive(direct_weak))

	var unknown := SpineAttachment.create_region_from_atlas("a", SpineAtlasResource.new(), "head")
	check("an empty atlas gives no attachment", unknown == null)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
