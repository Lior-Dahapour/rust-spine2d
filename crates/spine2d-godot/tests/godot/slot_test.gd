extends Node2D
# SpineSlotNode: children of a slot node draw between the slots of the skeleton, in slot order,
# whatever the order of the nodes in the tree. Needs a window: it counts draw calls and reads pixels.
# Usage: godot --path . --scene res://slot_test.tscn [-- <screenshot dir>]
var failures := 0
var data: SpineSkeletonDataResource
var sprite: SpineSprite
var shots: String = ""

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

func draw_calls() -> int:
	await RenderingServer.frame_post_draw
	return int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))

func slot_node(slot: String, color: Color) -> SpineSlotNode:
	var node := SpineSlotNode.new()
	node.slot_name = slot
	var rect := ColorRect.new()
	rect.color = color
	rect.position = Vector2(-3000, -3000)
	rect.size = Vector2(6000, 6000)
	node.add_child(rect)
	return node

# A child that draws nothing, to count only the splits a slot node causes.
func empty_slot_node(slot: String) -> SpineSlotNode:
	var node := SpineSlotNode.new()
	node.slot_name = slot
	return node

# A pixel on the jacket, and one in empty space. The test rectangles are huge, so where a node's
# bone is does not matter; only what is drawn above and below it does.
func pixel_at(x: int, y: int) -> Color:
	await RenderingServer.frame_post_draw
	return get_viewport().get_texture().get_image().get_pixel(x, y)

func pixel_at_body() -> Color:
	return await pixel_at(440, 450)

func pixel_in_space() -> Color:
	return await pixel_at(100, 100)

func is_color(c: Color, want: Color) -> bool:
	return absf(c.r - want.r) < 0.05 and absf(c.g - want.g) < 0.05 and absf(c.b - want.b) < 0.05

func save_shot(name: String) -> void:
	if shots == "":
		return
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png("%s/%s.png" % [shots, name])

func fresh_sprite() -> SpineSprite:
	if sprite:
		sprite.free()
	sprite = SpineSprite.new()
	sprite.skeleton_data = data
	sprite.position = Vector2(450, 600)
	sprite.scale = Vector2(0.5, 0.5)
	add_child(sprite)
	await frames(5)
	return sprite

func _ready() -> void:
	var args := OS.get_cmdline_user_args()
	shots = args[0] if args.size() > 0 else ""
	data = SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	await fresh_sprite()
	var names := sprite.get_slot_names()
	var first := names[0]
	var middle := names[names.size() / 2]
	var third := "head"
	var last := names[names.size() - 1]
	print("slots: ", names.size(), " first=", first, " middle=", middle, " third=", third, " last=", last)

	var base := await draw_calls()
	check("baseline draws something", base >= 1)

	var one := empty_slot_node(middle)
	sprite.add_child(one)
	await frames(3)
	var split := await draw_calls()
	check("a slot node splits the meshes (+1 draw call)", split == base + 1)

	var two := empty_slot_node(third)
	sprite.add_child(two)
	await frames(3)
	check("a second slot node at another slot splits again (+2)", await draw_calls() == base + 2)

	var same := empty_slot_node(third)
	sprite.add_child(same)
	await frames(3)
	check("a node at an already split slot adds no draw call", await draw_calls() == base + 2)

	var bad := empty_slot_node("no-such-slot")
	sprite.add_child(bad)
	await frames(3)
	check("an unknown slot name splits nothing", await draw_calls() == base + 2)

	two.slot_name = last
	await frames(3)
	check("a split stays while another node holds the slot", await draw_calls() == base + 2)
	sprite.remove_child(same)
	same.free()
	await frames(3)
	check("the last node leaving a slot removes its split (+1 left)", await draw_calls() == base + 1)

	sprite.remove_child(one)
	one.free()
	await frames(3)
	check("moving to the last slot adds no split", await draw_calls() == base)
	sprite.remove_child(two)
	sprite.remove_child(bad)
	two.free()
	bad.free()
	await frames(3)
	check("with no slot nodes the draw calls match the baseline", await draw_calls() == base)

	# Stacking: the rectangles cover the whole window.
	await fresh_sprite()
	var plain := await pixel_at_body()
	check("the body pixel is part of the body", not is_color(plain, Color(0.3, 0.3, 0.3)) and not is_color(plain, Color.MAGENTA))
	var front := slot_node(last, Color.MAGENTA)
	sprite.add_child(front)
	await frames(3)
	var in_front := await pixel_at_body()
	await save_shot("slot_front")
	check("a node at the last slot draws over the body", is_color(in_front, Color.MAGENTA))
	front.slot_name = first
	await frames(3)
	var behind := await pixel_at_body()
	await save_shot("slot_behind")
	check("a node at the first slot draws behind the body", is_color(behind, plain))

	# Slot order wins over tree order: the node added second belongs lower in the stack.
	front.slot_name = last
	var below := slot_node(first, Color.RED)
	sprite.add_child(below)
	await frames(3)
	check("slot order wins over tree order", is_color(await pixel_in_space(), Color.MAGENTA))
	sprite.move_child(front, 1)
	sprite.move_child(below, 0)
	await frames(3)
	check("reordering the children keeps the slot order", is_color(await pixel_in_space(), Color.MAGENTA))
	var late := Node2D.new()
	sprite.add_child(late)
	sprite.move_child(late, 0)
	await frames(3)
	check("adding a sibling keeps the slot order", is_color(await pixel_in_space(), Color.MAGENTA))

	# The node follows its slot's bone.
	var bone_node := empty_slot_node(middle)
	sprite.add_child(bone_node)
	await frames(3)
	var bone := sprite.get_bone(sprite.get_slot(middle).get_bone().get_name())
	check("a slot node follows its slot's bone", bone_node.position.distance_to(bone.get_world_position()) < 0.01)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
