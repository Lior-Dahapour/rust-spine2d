extends Node2D
# SpineBoneNode: the properties of the official node, finding its sprite and bone, and drive mode,
# where the bone takes the transform of the node. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://bone_node_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	sprite.update_mode = 2
	add_child(sprite)
	sprite.get_animation_state().set_animation("idle")
	await get_tree().process_frame

	var node := SpineBoneNode.new()
	node.bone_name = "head"
	sprite.add_child(node)
	await get_tree().process_frame
	check("a bone node defaults to follow mode and enabled", node.bone_mode == SpineConstant.BoneMode_Follow and node.enabled)
	node.bone_mode = 7
	check("an unknown mode is refused", node.bone_mode == SpineConstant.BoneMode_Follow)

	# Follow: the node goes to the bone.
	sprite.get_skeleton().update_world_transform(SpineConstant.Physics_Update)
	sprite.update_skeleton(0.016)
	await get_tree().process_frame
	await get_tree().process_frame
	var bone_origin := sprite.get_global_bone_transform("head").origin
	check("in follow mode the node sits on the bone", node.global_position.distance_to(bone_origin) < 0.5)
	node.enabled = false
	node.global_position = Vector2(-500.0, -500.0)
	sprite.update_skeleton(0.016)
	await get_tree().process_frame
	await get_tree().process_frame
	check("a disabled node stays where it is", node.global_position.distance_to(Vector2(-500.0, -500.0)) < 0.01)
	node.enabled = true

	# Drive: the bone goes to the node.
	node.bone_mode = SpineConstant.BoneMode_Drive
	var target := Vector2(321.0, 123.0)
	node.global_position = target
	sprite.update_skeleton(0.016)
	await get_tree().process_frame
	await get_tree().process_frame
	# As in the official plugin the bone is placed against the world transform of its parent from the
	# frame before, so it lands within a pixel or two of the node while the parent still moves.
	check("in drive mode the bone takes the node's place", sprite.get_global_bone_transform("head").origin.distance_to(target) < 3.0)
	node.bone_mode = SpineConstant.BoneMode_Follow

	check("debug color defaults to the official one", node.get_debug_color().is_equal_approx(Color8(0xff, 0, 0, 0x77)))
	check("debug thickness defaults to 5", node.get_debug_thickness() == 5.0)
	node.set_debug_color(Color.GREEN)
	node.set_debug_thickness(9.0)
	check("debug color and thickness read back", node.get_debug_color() == Color.GREEN and node.get_debug_thickness() == 9.0)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
