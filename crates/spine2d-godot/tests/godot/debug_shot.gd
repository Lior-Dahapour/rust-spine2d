extends Node2D
func _ready() -> void:
	var atlas := SpineAtlasResource.new()
	atlas.source_file = "res://assets/spineboy/spineboy.atlas"
	var file := SpineSkeletonFileResource.new()
	file.source_file = "res://assets/spineboy/spineboy-pro.json"
	var data := SpineSkeletonDataResource.new()
	data.atlas = atlas
	data.skeleton_file = file
	var s := SpineSprite.new()
	s.skeleton_data = data
	s.initial_animation = "idle"
	s.position = Vector2(450, 620)
	s.scale = Vector2(0.9, 0.9)
	s.debug_bones = true
	s.debug_bounding_boxes = true
	s.debug_meshes = true
	var follower := SpineBoneNode.new()
	follower.bone_name = "muzzle"
	var marker := Polygon2D.new()
	marker.polygon = PackedVector2Array([Vector2(-8, -8), Vector2(8, -8), Vector2(8, 8), Vector2(-8, 8)])
	marker.color = Color(1, 0, 0, 0.9)
	follower.add_child(marker)
	s.add_child(follower)
	add_child(s)
	for i in 40:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png(OS.get_cmdline_user_args()[0])
	print("hit_box_at_head=", s.get_bounding_box_at(s.get_bone("head").get_world_position()), " bone_names=", s.get_bone_names().size())
	get_tree().quit()
