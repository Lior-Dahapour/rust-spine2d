extends Node2D
# The data setters change the skeleton data every sprite of a resource shares: a write through one
# sprite's data handle is seen through another's, setup pose writes show when a sprite applies its
# setup pose, and a structural change makes every sprite rebuild its update order. Runs headless.
# Usage: godot --headless --path . --scene res://data_setters_test.tscn
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
	await get_tree().process_frame
	await get_tree().process_frame

	var head_a := a.get_bone("head")
	var head_b := b.get_bone("head")
	var data_a := head_a.get_data()
	var data_b := head_b.get_data()

	var length := data_a.get_length()
	data_a.set_length(length + 10.0)
	check("a length written through one sprite is read through the other", near(data_b.get_length(), length + 10.0))
	data_a.set_length(length)

	var color := data_a.get_color()
	data_a.set_color(Color(0.25, 0.5, 0.75, 1.0))
	check("a color written through one sprite is read through the other", near(data_b.get_color().r, 0.25) and near(data_b.get_color().g, 0.5) and near(data_b.get_color().b, 0.75))
	data_a.set_color(color)

	var visible := data_a.is_visible()
	data_a.set_visible(not visible)
	check("visible is shared", data_b.is_visible() == not visible)
	data_a.set_visible(visible)

	var setup_rotation := data_a.get_setup_pose().get_rotation()
	data_a.get_setup_pose().set_rotation(setup_rotation + 20.0)
	check("a setup pose written through one sprite is read through the other", near(data_b.get_setup_pose().get_rotation(), setup_rotation + 20.0))
	head_b.set_to_setup_pose()
	check("a sprite that applies its setup pose uses the changed data", near(head_b.get_pose().get_rotation(), setup_rotation + 20.0))
	data_a.get_setup_pose().set_rotation(setup_rotation)

	check("head starts active in both sprites", head_a.is_active() and head_b.is_active())
	data_a.set_skin_required(true)
	check("skin required is shared", data_b.is_skin_required())
	await get_tree().process_frame
	await get_tree().process_frame
	check("a structural change rebuilds every sprite: the bone is inactive in both", not head_a.is_active() and not head_b.is_active())
	data_a.set_skin_required(false)
	await get_tree().process_frame
	await get_tree().process_frame
	check("changing it back rebuilds again: the bone is active in both", head_a.is_active() and head_b.is_active())

	var ik := a.get_skeleton().find_ik_constraint("aim-ik")
	var ik_data := ik.get_data()
	var ik_data_b := b.get_skeleton().find_ik_constraint("aim-ik").get_data()
	var mode := ik_data.get_scale_y_mode()
	ik_data.set_scale_y_mode((mode + 1) % 3)
	check("an IK mode written through one sprite is read through the other", ik_data_b.get_scale_y_mode() == (mode + 1) % 3)
	ik_data.set_scale_y_mode(mode)
	var target := ik_data.get_target().get_bone_name()
	ik_data.set_target(a.get_bone("root").get_data())
	check("the IK target is shared", ik_data_b.get_target().get_bone_name() == "root")
	ik_data.set_target(a.get_bone(target).get_data())
	check("the IK target is restored", ik_data_b.get_target().get_bone_name() == target)

	var other := SpineSprite.new()
	var other_data := SpineSkeletonDataResource.new()
	other_data.atlas = load("res://assets/spineboy/spineboy.atlas")
	other_data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	other.skeleton_data = other_data
	add_child(other)
	await get_tree().process_frame
	ik_data.set_target(other.get_bone("root").get_data())
	check("bone data of another skeleton data is refused", ik_data.get_target().get_bone_name() == target)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
