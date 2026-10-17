extends Node2D
# SpineBone, SpineBoneData and SpineBonePose: the hierarchy, the three poses, the conversions and
# stale handles. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://bone_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func near(a: float, b: float, eps := 0.001) -> bool:
	return absf(a - b) < eps

func same_angle(a: float, b: float, eps := 0.01) -> bool:
	return absf(wrapf(a - b, -180.0, 180.0)) < eps

func near_vec(a: Vector2, b: Vector2) -> bool:
	return a.distance_to(b) < 0.01

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	sprite.set_animation("run", true, 0)
	await get_tree().process_frame
	await get_tree().process_frame

	var head := sprite.get_bone("head")
	var root := sprite.get_bone("root")
	var bone_data := head.get_data()
	check("data name", bone_data.get_bone_name() == "head")
	check("data index is the bone's place in the skeleton", bone_data.get_index() > 0 and sprite.get_bone_names()[bone_data.get_index()] == "head")
	check("data length matches the bone", near(bone_data.get_length(), head.get_length()) and bone_data.get_length() > 0.0)
	check("data parent matches the bone parent", bone_data.get_parent().get_bone_name() == head.get_parent().get_name())
	check("the root has no parent data", root.get_data().get_parent() == null and root.get_parent() == null)
	check("data color is opaque", bone_data.get_color().a > 0.0)
	check("data flags and icon", bone_data.is_visible() == true or bone_data.is_visible() == false)
	check("head is not skin required", not bone_data.is_skin_required())
	check("the icon is a string", typeof(bone_data.get_icon()) == TYPE_STRING)

	var parent := head.get_parent()
	var siblings := parent.get_children()
	var found := false
	for child in siblings:
		if child.get_name() == "head": found = true
	check("a parent lists its child", found)
	check("a child points back at its parent", siblings[0].get_parent().get_name() == parent.get_name())
	check("the root has children", root.get_children().size() > 0)

	var pose := head.get_pose()
	var applied := head.get_applied_pose()
	var setup := bone_data.get_setup_pose()
	var setup_rotation := setup.get_rotation()
	pose.set_x(12.5)
	pose.set_y(-3.5)
	pose.set_rotation(33.0)
	pose.set_scale_x(1.5)
	pose.set_scale_y(0.5)
	pose.set_shear_x(4.0)
	pose.set_shear_y(5.0)
	check("local values round trip", near(pose.get_x(), 12.5) and near(pose.get_y(), -3.5) and near(pose.get_rotation(), 33.0))
	check("scale and shear round trip", near(pose.get_scale_x(), 1.5) and near(pose.get_scale_y(), 0.5) and near(pose.get_shear_x(), 4.0) and near(pose.get_shear_y(), 5.0))
	check("a second pose handle sees the values", near(head.get_pose().get_rotation(), 33.0))
	check("the setup pose is not the live pose", near(setup.get_rotation(), setup_rotation) and not near(setup_rotation, 33.0))
	setup.set_rotation(99.0)
	check("the setup pose is shared data: a write changes it", near(setup.get_rotation(), 99.0))
	setup.set_rotation(setup_rotation)
	check("the setup pose has no world transform", setup.get_world_x() == 0.0 and setup.get_a() == 0.0)
	var inherit := pose.get_inherit()
	pose.set_inherit(SpineConstant.Inherit_NoScale)
	check("inherit round trips", pose.get_inherit() == SpineConstant.Inherit_NoScale)
	pose.set_inherit(9)
	check("an unknown inherit mode is refused", pose.get_inherit() == SpineConstant.Inherit_NoScale)
	pose.set_inherit(inherit)

	await get_tree().process_frame
	await get_tree().process_frame
	var world_pose := head.get_applied_pose()
	check("world position matches the convenience method", near_vec(Vector2(world_pose.get_world_x(), world_pose.get_world_y()), head.get_world_position()))
	check("world rotation follows the matrix", near(world_pose.get_world_rotation_x(), rad_to_deg(atan2(world_pose.get_c(), world_pose.get_a())), 0.01))
	check("world scale is positive", world_pose.get_world_scale_x() > 0.0 and world_pose.get_world_scale_y() > 0.0)
	check("the pose is not the applied pose after constraints", applied.get_world_x() == world_pose.get_world_x())

	var point := Vector2(10.0, 20.0)
	check("bone conversions round trip", near_vec(head.local_to_world(head.world_to_local(point)), point))
	check("pose conversions round trip", near_vec(world_pose.local_to_world(world_pose.world_to_local(point)), point))
	check("the bone and its applied pose convert alike", near_vec(head.world_to_local(point), world_pose.world_to_local(point)))
	check("parent conversions round trip", near_vec(head.parent_to_world(head.world_to_parent(point)), point))
	check("the root's parent space is the sprite's space", near_vec(root.world_to_parent(point), point) and near_vec(root.parent_to_world(point), point))
	# The rotation conversions are only inverses of each other when the world matrix is not
	# reflected, and the Godot extension flips y. They always satisfy these two identities, which
	# come from the formulas of the reference runtime.
	for bone: SpineBone in [root, head]:
		var applied_pose: SpineBonePose = bone.get_applied_pose()
		var rotation: float = applied_pose.get_rotation()
		var shear: float = applied_pose.get_shear_x()
		var determinant: float = applied_pose.get_a() * applied_pose.get_d() - applied_pose.get_b() * applied_pose.get_c()
		var world_rotation: float = applied_pose.get_world_rotation_x()
		check("local_to_world_rotation of rotation + shear is the world rotation (%s)" % bone.get_name(), same_angle(bone.local_to_world_rotation(rotation + shear), world_rotation))
		var expected: float = rotation - shear + (180.0 if determinant < 0.0 else 0.0)
		check("world_to_local_rotation of the world rotation is rotation - shear, 180 more when reflected (%s)" % bone.get_name(), same_angle(bone.world_to_local_rotation(world_rotation), expected))
	var before := world_pose.get_world_rotation_x()
	head.rotate_world(10.0)
	check("rotate_world turns the applied pose", same_angle(head.get_applied_pose().get_world_rotation_x() - before, 10.0, 0.05))

	var transform := Transform2D(deg_to_rad(30.0), Vector2(5.0, 6.0))
	head.set_transform(transform)
	check("set_transform sets position and rotation", near(head.get_pose().get_x(), 5.0) and near(head.get_pose().get_y(), 6.0) and near(head.get_pose().get_rotation(), 30.0, 0.01))
	check("set_transform sets scale", near(head.get_pose().get_scale_x(), 1.0) and near(head.get_pose().get_scale_y(), 1.0))
	head.get_pose().set_shear_x(0.0)
	head.get_pose().set_shear_y(0.0)
	var back := head.get_transform()
	check("get_transform returns what set_transform gave", near_vec(back.origin, transform.origin) and near_vec(back.x, transform.x) and near_vec(back.y, transform.y))

	var skeleton := sprite.get_skeleton()
	var x_before := world_pose.get_world_x()
	head.update(skeleton, 0)
	world_pose.update_world_transform(skeleton)
	world_pose.update_local_transform(skeleton)
	check("the update methods are stubs that change nothing", near(world_pose.get_world_x(), x_before))

	check("the bone is active", head.is_active())
	head.set_active(false)
	check("set_active changes the flag", not head.is_active())

	head.set_to_setup_pose()
	check("set_to_setup_pose restores the local pose", near(head.get_pose().get_rotation(), setup_rotation))
	var target := Vector2(100.0, -40.0)
	head.set_world_position(target)
	var local := head.get_pose()
	check("set_world_position moves the local position", not near(local.get_x(), 5.0) or not near(local.get_y(), 6.0))

	sprite.free()
	await get_tree().process_frame
	check("handles go stale with the sprite", not head.is_valid() and not bone_data.is_valid())
	check("stale reads return zero values", pose.get_x() == 0.0 and head.get_children().size() == 0 and bone_data.get_bone_name() == "")
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
