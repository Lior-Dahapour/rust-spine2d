extends Node2D
# The IK constraint classes: lookup through get_skeleton(), pose reads and writes, retargeting that
# the next frames pick up, the read-only setup pose and stale handles. Runs headless.
# Usage: godot --headless --path . --scene res://constraint_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

# Whether `pose` belongs to `bone`: writes through the pose must show on the bone's applied pose.
func is_bone(pose: SpineBonePose, bone: SpineBone) -> bool:
	var applied := bone.get_applied_pose()
	var before := applied.get_x()
	pose.set_x(before + 1234.5)
	var same := absf(applied.get_x() - (before + 1234.5)) < 0.001
	pose.set_x(before)
	return same

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	sprite.set_animation("run", true, 0)
	await get_tree().process_frame

	var skeleton := sprite.get_skeleton()
	check("get_skeleton returns a valid skeleton", skeleton != null and skeleton.is_valid())
	check("spineboy has 7 IK constraints", skeleton.get_ik_constraints().size() == 7)
	check("an unknown name finds nothing", skeleton.find_ik_constraint("nope") == null)
	check("a transform constraint is not an IK constraint", skeleton.find_ik_constraint("shoulder") == null)

	var ik := skeleton.find_ik_constraint("front-leg-ik")
	check("find_ik_constraint finds front-leg-ik", ik != null and ik.is_valid())
	var constraint_data := ik.get_data()
	check("data name", constraint_data.get_constraint_name() == "front-leg-ik")
	check("data is not skin required", not constraint_data.is_skin_required())
	check("a leg chain has two bones", ik.get_bones().size() == 2)
	var thigh_pose: SpineBonePose = ik.get_bones()[0]
	var belongs_to_a_bone := false
	for bone_name in sprite.get_bone_names():
		if is_bone(thigh_pose, sprite.get_bone(bone_name)): belongs_to_a_bone = true
	check("the constrained bones are bone poses that write through to their bones", belongs_to_a_bone)
	var data_bones: Array = constraint_data.get_bones()
	check("the data lists the same two bones", data_bones.size() == 2 and data_bones[1].get_parent().get_bone_name() == data_bones[0].get_bone_name())
	check("the data's bones are the live constraint's bones", is_bone(ik.get_bones()[0], sprite.get_bone(data_bones[0].get_bone_name())) and is_bone(ik.get_bones()[1], sprite.get_bone(data_bones[1].get_bone_name())))
	check("the data's target is the live target by default", constraint_data.get_target().get_bone_name() == ik.get_target().get_name())
	constraint_data.set_target(sprite.get_bone("root").get_data())
	check("the data's set_target changes the shared data", constraint_data.get_target().get_bone_name() == "root")
	constraint_data.set_target(sprite.get_bone(ik.get_target().get_name()).get_data())
	check("target is a bone", ik.get_target() != null and ik.get_target().get_name() != "")
	check("scale y mode is in range", constraint_data.get_scale_y_mode() >= 0 and constraint_data.get_scale_y_mode() <= 2)
	check("the constraint is active", ik.is_active())

	var pose := ik.get_pose()
	pose.set_mix(0.25)
	pose.set_softness(3.0)
	pose.set_bend_direction(-1)
	pose.set_compress(true)
	pose.set_stretch(true)
	check("mix round trips", is_equal_approx(pose.get_mix(), 0.25))
	check("softness round trips", is_equal_approx(pose.get_softness(), 3.0))
	check("bend direction round trips", pose.get_bend_direction() == -1)
	check("compress and stretch round trip", pose.get_compress() and pose.get_stretch())
	check("a second pose handle sees the same values", is_equal_approx(ik.get_pose().get_mix(), 0.25))

	var setup := constraint_data.get_setup_pose()
	var setup_mix := setup.get_mix()
	setup.set_mix(0.9)
	check("the setup pose is shared data: a write changes it", is_equal_approx(setup.get_mix(), 0.9))
	setup.set_mix(setup_mix)
	var scale_y_mode := constraint_data.get_scale_y_mode()
	constraint_data.set_scale_y_mode((scale_y_mode + 1) % 3)
	check("data setters change the shared data", constraint_data.get_scale_y_mode() == (scale_y_mode + 1) % 3)
	constraint_data.set_scale_y_mode(scale_y_mode)

	var other := sprite.get_bone("root")
	var original_target := ik.get_target().get_name()
	ik.set_target(other)
	check("set_target is visible at once", ik.get_target().get_name() == "root")
	await get_tree().process_frame
	await get_tree().process_frame
	check("the retarget survives the frames that rebuild the cache", ik.get_target().get_name() == "root")
	ik.set_target(sprite.get_bone(original_target))
	await get_tree().process_frame
	check("the target can be set back", ik.get_target().get_name() == original_target)
	ik.set_target(null)
	check("a null target is ignored", ik.get_target().get_name() == original_target)

	var mix_before := ik.get_pose().get_mix()
	ik.update(skeleton)
	check("update is a stub that changes nothing", is_equal_approx(ik.get_pose().get_mix(), mix_before))

	var applied := ik.get_applied_pose()
	check("the applied pose is readable", applied.get_bend_direction() == 1 or applied.get_bend_direction() == -1)

	var other_sprite := SpineSprite.new()
	other_sprite.skeleton_data = data
	add_child(other_sprite)
	await get_tree().process_frame
	ik.set_target(other_sprite.get_bone("root"))
	check("a bone of another sprite is refused", ik.get_target().get_name() == original_target)

	sprite.free()
	await get_tree().process_frame
	check("handles go stale with the sprite", not ik.is_valid() and not skeleton.is_valid())
	check("stale reads return zero values", ik.get_pose().get_mix() == 0.0 and ik.get_bones().size() == 0)
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
