extends Node2D
# SpineBoneNode and SpineSlotNode keep their position on the bone, through animation, renaming,
# leaving and re-entering the tree, reloading the skeleton and manual updates. Runs headless.
# Usage: godot --headless --path . --scene res://follow_test.tscn
var failures := 0
var data: SpineSkeletonDataResource
var sprite: SpineSprite

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

func on_bone(node: Node2D, bone: String) -> bool:
	var want := sprite.get_bone(bone).get_world_position()
	return node.position.distance_to(want) < 0.01

func _ready() -> void:
	data = SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	sprite = SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	sprite.set_animation("run", true, 0)
	var head := SpineBoneNode.new()
	head.bone_name = "head"
	sprite.add_child(head)
	var gun := SpineSlotNode.new()
	gun.slot_name = "gun"
	sprite.add_child(gun)
	await frames(5)

	check("bone node sits on its bone", on_bone(head, "head"))
	check("slot node sits on its slot's bone", gun.position.distance_to(sprite.get_slot("gun").get_bone().get_world_position()) < 0.01)
	var before := head.position
	await frames(10)
	check("bone node moves with the animation", head.position != before and on_bone(head, "head"))

	var want_axes := sprite.get_global_bone_transform("head")
	var got_axes := head.global_transform
	check("bone node is not mirrored", got_axes.determinant() > 0.0)
	check("bone node axes match get_global_bone_transform",
		got_axes.x.distance_to(want_axes.x) < 0.01 and got_axes.y.distance_to(want_axes.y) < 0.01)

	head.bone_name = "muzzle"
	await frames(2)
	check("renaming the bone moves the node to the new bone", on_bone(head, "muzzle"))
	head.bone_name = "no-such-bone"
	var kept := head.position
	await frames(2)
	check("an unknown bone leaves the node where it was", head.position == kept)
	head.bone_name = "head"
	await frames(2)

	sprite.remove_child(head)
	await frames(3)
	var away := head.position
	await frames(3)
	check("a node outside the tree is left alone", head.position == away)
	sprite.add_child(head)
	await frames(3)
	check("a node back in the tree follows again", on_bone(head, "head"))

	sprite.skeleton_data = data
	await frames(5)
	check("nodes follow the rebuilt skeleton", on_bone(head, "head"))
	sprite.set_animation("run", true, 0)

	sprite.update_mode = 2
	await frames(2)
	var parked := head.position
	await frames(5)
	check("a manual sprite does not move its nodes by itself", head.position == parked)
	sprite.update_skeleton(0.2)
	await frames(2)
	check("update_skeleton places the nodes at the next frame", head.position != parked and on_bone(head, "head"))

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
