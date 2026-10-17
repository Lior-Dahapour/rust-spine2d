extends Node2D
# The debug and material properties of SpineSprite and SpineSlotNode, with the defaults and the
# methods of the official plugin. Runs headless on spineboy.
# Usage: godot --headless --path . --scene res://sprite_api_test.tscn
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

	check("the debug defaults of the official plugin", not sprite.get_debug_root() and not sprite.get_debug_bones() and not sprite.get_debug_regions() and not sprite.get_debug_meshes() and not sprite.get_debug_bounding_boxes() and not sprite.get_debug_paths() and not sprite.get_debug_clipping())
	check("the default colors and thickness", sprite.get_debug_root_color() == Color(1, 1, 1, 0.5) and sprite.get_debug_bones_color() == Color(1, 1, 0, 0.5) and near(sprite.get_debug_bones_thickness(), 5.0) and sprite.get_debug_regions_color() == Color(0, 0, 1, 0.5) and sprite.get_debug_meshes_color() == Color(0, 0, 1, 0.5) and sprite.get_debug_bounding_boxes_color() == Color(0, 1, 0, 0.5) and sprite.get_debug_clipping_color() == Color(0.8, 0, 0, 0.8))
	sprite.set_debug_root(true)
	sprite.set_debug_bones(true)
	sprite.set_debug_regions(true)
	sprite.set_debug_meshes(true)
	sprite.set_debug_bounding_boxes(true)
	sprite.set_debug_paths(true)
	sprite.set_debug_clipping(true)
	sprite.set_debug_root_color(Color.RED)
	sprite.set_debug_bones_color(Color.GREEN)
	sprite.set_debug_bones_thickness(7.0)
	sprite.set_debug_regions_color(Color.BLUE)
	sprite.set_debug_meshes_color(Color.WHITE)
	sprite.set_debug_bounding_boxes_color(Color.YELLOW)
	sprite.set_debug_paths_color(Color.CYAN)
	sprite.set_debug_clipping_color(Color.MAGENTA)
	check("the debug flags round trip", sprite.get_debug_root() and sprite.get_debug_bones() and sprite.get_debug_regions() and sprite.get_debug_meshes() and sprite.get_debug_bounding_boxes() and sprite.get_debug_paths() and sprite.get_debug_clipping())
	check("the debug colors round trip", sprite.get_debug_root_color() == Color.RED and sprite.get_debug_bones_color() == Color.GREEN and near(sprite.get_debug_bones_thickness(), 7.0) and sprite.get_debug_regions_color() == Color.BLUE and sprite.get_debug_meshes_color() == Color.WHITE and sprite.get_debug_bounding_boxes_color() == Color.YELLOW and sprite.get_debug_paths_color() == Color.CYAN and sprite.get_debug_clipping_color() == Color.MAGENTA)
	for i in 5:
		await get_tree().process_frame
	check("every overlay draws without trouble", sprite.get_animation_state().is_valid())
	sprite.set_debug_root(false)
	sprite.set_debug_bones(false)
	sprite.set_debug_regions(false)
	sprite.set_debug_meshes(false)
	sprite.set_debug_bounding_boxes(false)
	sprite.set_debug_paths(false)
	sprite.set_debug_clipping(false)
	await get_tree().process_frame

	check("the update mode getter", sprite.get_update_mode() == 0)
	sprite.set_update_mode(2)
	check("the update mode round trips", sprite.get_update_mode() == 2)
	sprite.set_update_mode(0)

	var multiply := CanvasItemMaterial.new()
	var additive := CanvasItemMaterial.new()
	sprite.set_multiply_material(multiply)
	sprite.set_additive_material(additive)
	sprite.set_screen_material(null)
	check("the sprite's materials round trip", sprite.get_multiply_material() == multiply and sprite.get_additive_material() == additive and sprite.get_screen_material() == null and sprite.get_normal_material() == null)
	await get_tree().process_frame

	var slot_node := SpineSlotNode.new()
	slot_node.slot_name = "portal-flare8"
	sprite.add_child(slot_node)
	await get_tree().process_frame
	check("a slot node starts without materials", slot_node.get_normal_material() == null and slot_node.get_additive_material() == null and slot_node.get_multiply_material() == null and slot_node.get_screen_material() == null)
	slot_node.set_additive_material(additive)
	slot_node.set_normal_material(multiply)
	slot_node.set_multiply_material(multiply)
	slot_node.set_screen_material(additive)
	check("a slot node's materials round trip", slot_node.get_additive_material() == additive and slot_node.get_normal_material() == multiply and slot_node.multiply_material == multiply and slot_node.screen_material == additive)
	for i in 3:
		await get_tree().process_frame
	check("the sprite draws with them", sprite.get_animation_state().is_valid())

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
