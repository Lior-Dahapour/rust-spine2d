extends Node2D
# The skeleton data resource without any sprite: its finders and lists hand out data handles of the
# stored data, its other getters, the animation mixes, the aliases of the official property names,
# the atlas and file resource calls, and the sprite calls added with them. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://resource_api_test.tscn
var failures := 0
var changed := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func near(a: float, b: float, eps := 0.001) -> bool:
	return absf(a - b) < eps

func _ready() -> void:
	var atlas := SpineAtlasResource.new()
	check("an atlas loads from a file", atlas.load_from_atlas_file("res://assets/spineboy/spineboy.atlas") == OK)
	check("a missing atlas file is an error", SpineAtlasResource.new().load_from_atlas_file("res://nothing.atlas") == ERR_FILE_NOT_FOUND)
	check("the atlas knows its path and pages", atlas.get_source_path() == "res://assets/spineboy/spineboy.atlas" and atlas.get_page_count() >= 1 and atlas.get_textures().size() == atlas.get_page_count())
	check("normal and specular maps are not loaded", atlas.get_normal_maps().is_empty() and atlas.get_specular_maps().is_empty())
	var file := SpineSkeletonFileResource.new()
	check("a skeleton file loads from a path", file.load_from_file("res://assets/spineboy/spineboy-pro.skel") == OK)
	check("a missing skeleton file is an error", SpineSkeletonFileResource.new().load_from_file("res://nothing.skel") == ERR_FILE_NOT_FOUND)

	var data := SpineSkeletonDataResource.new()
	data.skeleton_data_changed.connect(func(): changed += 1)
	data.atlas_res = atlas
	data.skeleton_file_res = file
	check("the official property names are aliases", data.atlas == atlas and data.skeleton_file == file and data.get_atlas_res() == atlas and data.get_skeleton_file_res() == file)
	check("a change of the files is announced", changed == 2)
	check("the data loads without a sprite", data.is_skeleton_data_loaded())

	var run := data.find_animation("run")
	check("an animation by name", run != null and run.get_name() == "run" and near(run.get_duration(), 0.6667, 0.01) and data.find_animation("nope") == null)
	check("every animation", data.get_animations().size() == data.get_animation_names().size() and data.get_animations()[0] is SpineAnimation)
	var head := data.find_bone("head")
	check("a bone by name", head != null and head.get_bone_name() == "head" and head.get_length() > 0.0 and data.find_bone("nope") == null)
	check("every bone, parents first", data.get_bones().size() == 0 or data.get_bones()[0].get_bone_name() == "root")
	check("the parent of a bone", head.get_parent() != null and head.get_parent().is_valid())
	var slot := data.find_slot("portal-flare8")
	check("a slot by name", slot != null and slot.get_blend_mode() == SpineConstant.BlendMode_Additive and data.find_slot("nope") == null)
	check("every slot", data.get_slots().size() == data.get_slot_names().size())
	var footstep := data.find_event("footstep")
	check("an event by name", footstep != null and footstep.get_event_name() == "footstep" and data.find_event("nope") == null)
	check("every event", data.get_events().size() == 1)
	check("an IK constraint by name", data.find_ik_constraint_data("aim-ik") != null and data.find_ik_constraint_data("shoulder") == null)
	check("a transform constraint by name", data.find_transform_constraint_data("shoulder") != null and data.find_transform_constraint_data("aim-ik") == null)
	check("constraint lists by kind", data.get_ik_constraints().size() >= 5 and data.get_transform_constraints().size() >= 5 and data.get_path_constraints().is_empty() and data.get_physics_constraints().is_empty())
	check("every skin", data.get_skins().size() == data.get_skin_names().size() and data.get_skins().size() >= 1)
	check("a default skin", data.get_default_skin() != null and data.get_default_skin().get_name() == "default")

	check("exported information", data.get_fps() > 0.0 and data.get_version().begins_with("4.") and data.get_skeleton_name() != null and data.get_images_path() != null and data.get_audio_path() != null)
	check("the hash is a string", typeof(data.get_hash()) == TYPE_STRING)
	check("the extent", typeof(data.get_x()) == TYPE_FLOAT and typeof(data.get_y()) == TYPE_FLOAT)
	var reference := data.get_reference_scale()
	data.set_reference_scale(reference + 1.0)
	check("the reference scale round trips", near(data.get_reference_scale(), reference + 1.0))
	data.set_reference_scale(reference)

	# A change through a handle of the resource is seen by a sprite.
	var sprite := SpineSprite.new()
	sprite.skeleton_data_res = data
	add_child(sprite)
	await get_tree().process_frame
	check("a sprite takes the data through the official property name", sprite.skeleton_data == data and sprite.skeleton_data_res == data)
	var before := head.get_length()
	head.set_length(before + 5.0)
	check("a length written through the resource is read through a sprite", near(sprite.get_bone("head").get_data().get_length(), before + 5.0))
	head.set_length(before)

	# Mixes.
	data.set_default_mix(0.3)
	check("the default mix round trips", near(data.default_mix, 0.3))
	var mix := SpineAnimationMix.new()
	mix.from = "run"
	mix.to = "jump"
	mix.mix = 0.45
	data.set_animation_mixes([mix])
	var mixes := data.get_animation_mixes()
	check("animation mixes round trip", mixes.size() == 1 and mixes[0].from == "run" and mixes[0].to == "jump" and near(mixes[0].mix, 0.45))
	var state := sprite.get_animation_state()
	state.set_animation("run")
	var jump := state.set_animation("jump", false, 0)
	check("a running sprite uses the mixes", near(jump.get_mix_duration(), 0.45))
	var walk := state.set_animation("walk", false, 0)
	check("and the default mix for other pairs", near(walk.get_mix_duration(), 0.3))
	data.set_animation_mixes([])
	data.set_default_mix(0.0)

	# A change of the data builds the skeleton of a sprite again.
	var old_skeleton := sprite.get_skeleton()
	data.set_scale(0.5)
	for i in 3:
		await get_tree().process_frame
	check("a sprite builds its skeleton again when the data changed", not old_skeleton.is_valid() and sprite.get_skeleton().is_valid())
	data.set_scale(1.0)
	for i in 3:
		await get_tree().process_frame
	state = sprite.get_animation_state()

	# Sprite calls.
	var skin := sprite.new_skin("mine")
	check("a new skin", skin != null and skin.get_name() == "mine")
	var origin := sprite.get_global_bone_transform("head")
	check("a global bone transform", origin.origin.is_finite() and sprite.get_global_bone_transform("nope") == sprite.get_global_transform())
	var moved := Transform2D(0.3, Vector2(200.0, 300.0))
	sprite.set_global_bone_transform("head", moved)
	var back := sprite.get_global_bone_transform("head")
	check("a global bone transform written is read back", back.origin.distance_to(moved.origin) < 0.01)

	var skin_for_slot := data.find_skin("default")
	check("the attachments of a skin for a slot", skin_for_slot.find_attachments_for_slot(sprite.get_slot_names().find("head")).size() >= 1)

	# Materials.
	var material := CanvasItemMaterial.new()
	sprite.normal_material = material
	check("a material round trips", sprite.normal_material == material and sprite.get_normal_material() == material and sprite.additive_material == null)
	sprite.normal_material = null

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
