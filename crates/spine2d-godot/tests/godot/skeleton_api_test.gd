extends Node2D
# The whole SpineSkeleton handle: lookups, the arrays, the skin, attachments, the setup pose, the
# placement and the physics time. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://skeleton_api_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func near(a: float, b: float, eps := 0.001) -> bool:
	return absf(a - b) < eps

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	sprite.get_animation_state().set_animation("run")
	await get_tree().process_frame
	await get_tree().process_frame

	var sk := sprite.get_skeleton()
	check("the skeleton knows its data", sk.get_data() == data)
	check("a bone by name", sk.find_bone("head") != null and sk.find_bone("head").get_name() == "head" and sk.find_bone("no such bone") == null)
	check("a slot by name", sk.find_slot("head") != null and sk.find_slot("head").get_name() == "head" and sk.find_slot("no such slot") == null)
	check("the root bone", sk.get_root_bone().get_name() == "root")
	check("every bone, parents first", sk.get_bones().size() == sprite.get_bone_names().size() and sk.get_bones()[0].get_name() == "root")
	check("every slot", sk.get_slots().size() == sprite.get_slot_names().size())
	check("the draw order lists every slot", sk.get_draw_order().size() == sk.get_slots().size() and sk.get_draw_order()[0] is SpineSlot)

	check("only the default skin to begin with", sk.set_skin_by_name("no such skin") == false)
	check("a skin by name", sk.set_skin_by_name("default") and sk.get_skin() != null)
	sk.set_skin(null)
	check("no skin for null", sk.get_skin() == null)
	var skin := data.find_skin("default")
	sk.set_skin(skin)
	check("a skin object", sk.get_skin() != null)

	check("an attachment by slot name", sk.get_attachment_by_slot_name("head", "head") != null and sk.get_attachment_by_slot_name("head", "no such attachment") == null)
	var head_index := sprite.get_slot_names().find("head")
	check("an attachment by slot index", sk.get_attachment_by_slot_index(head_index, "head") != null and sk.get_attachment_by_slot_index(9999, "head") == null)
	check("set_attachment by names", sk.set_attachment("head", "head") and not sk.set_attachment("head", "no such attachment"))
	sk.set_attachment("head", "")
	check("an empty name hides the attachment", sprite.get_slot("head").get_attachment_name() == "")
	sk.set_slots_to_setup_pose()
	check("slots back to the setup pose show the setup attachment", sprite.get_slot("head").get_attachment_name() == "head")

	var pose := sprite.get_bone("head").get_pose()
	var setup_rotation := sprite.get_bone("head").get_data().get_setup_pose().get_rotation()
	pose.set_rotation(setup_rotation + 40.0)
	sk.set_bones_to_setup_pose()
	check("bones back to the setup pose", near(pose.get_rotation(), setup_rotation))
	pose.set_rotation(setup_rotation + 40.0)
	sk.set_to_setup_pose()
	check("everything back to the setup pose", near(pose.get_rotation(), setup_rotation))

	sk.update_world_transform(SpineConstant.Physics_Update)
	check("the world transform can be computed now", not near(sprite.get_bone("head").get_applied_pose().get_a(), 0.0))
	sk.update_world_transform(9)

	var bounds := sk.get_bounds()
	check("bounds of what is drawn, same as the sprite's", bounds.size.x > 0.0 and bounds.size.y > 0.0 and bounds.is_equal_approx(sprite.get_bounds()))

	sk.set_color(Color(0.5, 0.25, 0.75, 1.0))
	check("the color round trips", near(sk.get_color().r, 0.5) and near(sk.get_color().g, 0.25) and near(sprite.get_skeleton_color().b, 0.75))
	sk.set_color(Color.WHITE)
	sk.set_position(Vector2(12.0, -7.0))
	check("the position round trips", near(sk.get_x(), 12.0) and near(sk.get_y(), -7.0))
	sk.set_x(3.0)
	sk.set_y(4.0)
	check("x and y round trip", near(sk.get_x(), 3.0) and near(sk.get_y(), 4.0))
	sk.set_position(Vector2.ZERO)
	sk.set_scale_x(2.0)
	sk.set_scale_y(3.0)
	# As in the reference runtime, the getter of scale y is negated in a y-down world and the setter is not.
	check("the scale round trips", near(sk.get_scale_x(), 2.0) and near(absf(sk.get_scale_y()), 3.0))
	sk.set_scale_x(1.0)
	sk.set_scale_y(1.0)

	sk.set_time(5.0)
	check("the time round trips", near(sk.get_time(), 5.0))
	sk.update(0.5)
	check("update advances the time", near(sk.get_time(), 5.5))
	sk.physics_translate(1.0, 2.0)
	sk.physics_rotate(0.0, 0.0, 10.0)
	check("physics moves are accepted", sk.is_valid())

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
