extends Node2D
# SpineSlotData, SpineSlotPose, the pose and data accessors of SpineSlot, the slot of a path
# constraint's data, and SpineBoneLocal. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://slot_data_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func near(a: float, b: float, eps := 0.001) -> bool:
	return absf(a - b) < eps

func make_sprite(data: SpineSkeletonDataResource) -> SpineSprite:
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	return sprite

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var a := make_sprite(data)
	var b := make_sprite(data)
	a.set_animation("run", true, 0)
	await get_tree().process_frame
	await get_tree().process_frame

	var flare := a.get_slot("portal-flare8")
	var slot_data := flare.get_data()
	check("slot data is valid", slot_data.is_valid())
	check("slot data name and index", slot_data.get_name() == "portal-flare8" and a.get_slot_names()[slot_data.get_index()] == "portal-flare8")
	check("slot data blend mode is additive", slot_data.get_blend_mode() == SpineConstant.BlendMode_Additive)
	check("slot data color is the authored tint", near(slot_data.get_color().r, 0xc3 / 255.0, 0.003))
	check("slot data bone data is the slot's bone", slot_data.get_bone_data().get_bone_name() == flare.get_bone().get_name())
	check("the slot's pose is a slot pose", flare.get_pose().is_valid() and flare.get_applied_pose().is_valid())

	var slot_data_b := b.get_slot("portal-flare8").get_data()
	slot_data.set_blend_mode(SpineConstant.BlendMode_Screen)
	check("a blend mode written through one sprite is read through the other", slot_data_b.get_blend_mode() == SpineConstant.BlendMode_Screen)
	slot_data.set_blend_mode(9)
	check("an unknown blend mode is refused", slot_data_b.get_blend_mode() == SpineConstant.BlendMode_Screen)
	slot_data.set_blend_mode(SpineConstant.BlendMode_Additive)

	var attachment_name := slot_data.get_attachment_name()
	slot_data.set_attachment_name("nothing-by-this-name")
	check("the attachment name is shared", slot_data_b.get_attachment_name() == "nothing-by-this-name")
	slot_data.set_attachment_name(attachment_name)

	slot_data.set_color(Color(0.1, 0.2, 0.3, 0.4))
	check("the setup color is shared", near(slot_data_b.get_color().g, 0.2) and near(slot_data_b.get_setup_pose().get_color().a, 0.4))
	slot_data.set_has_dark_color(true)
	slot_data.set_dark_color(Color(0.5, 0.6, 0.7, 1.0))
	check("the setup dark color is shared", slot_data_b.has_dark_color() and near(slot_data_b.get_dark_color().b, 0.7))
	slot_data.set_has_dark_color(false)

	var pose := flare.get_pose()
	pose.set_color(Color(1, 0.5, 0.25, 0.75))
	check("a slot pose color round trips", near(pose.get_color().g, 0.5) and near(flare.get_pose().get_color().b, 0.25))
	check("the other sprite's pose is its own", not near(b.get_slot("portal-flare8").get_pose().get_color().g, 0.5))
	pose.set_has_dark_color(true)
	pose.set_dark_color(Color(0.2, 0.3, 0.4, 1.0))
	check("a slot pose dark color round trips", pose.has_dark_color() and near(pose.get_dark_color().r, 0.2))
	pose.set_sequence_index(3)
	check("the sequence index round trips", pose.get_sequence_index() == 3)
	pose.set_deform(PackedFloat32Array([1.0, 2.0, 3.0]))
	check("the deform round trips", pose.get_deform() == PackedFloat32Array([1.0, 2.0, 3.0]))
	check("the setup pose has no attachment of its own", slot_data.get_setup_pose().get_attachment() == null)

	flare.set_to_setup_pose()
	check("a slot reset to its setup pose uses the shared color", near(flare.get_pose().get_color().g, 0.2) and near(flare.get_pose().get_color().a, 0.4))

	var other_slot_data := a.get_slot("clipping").get_data()
	check("another slot's data is its own", other_slot_data.get_name() == "clipping")

	var local := SpineBoneLocal.new()
	local.set_position(3.0, 4.0)
	local.set_rotation(30.0)
	local.set_scale(2.0, 3.0)
	local.set_shear_x(5.0)
	local.set_shear_y(6.0)
	check("bone local values round trip", near(local.get_x(), 3.0) and near(local.get_y(), 4.0) and near(local.get_rotation(), 30.0) and near(local.get_scale_x(), 2.0) and near(local.get_scale_y(), 3.0) and near(local.get_shear_x(), 5.0) and near(local.get_shear_y(), 6.0))
	local.set_scale_uniform(1.5)
	check("a uniform scale sets both axes", near(local.get_scale_x(), 1.5) and near(local.get_scale_y(), 1.5))
	local.set_inherit(SpineConstant.Inherit_NoScale)
	check("bone local inherit round trips", local.get_inherit() == SpineConstant.Inherit_NoScale)
	local.set_inherit(9)
	check("an unknown inherit is refused", local.get_inherit() == SpineConstant.Inherit_NoScale)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
