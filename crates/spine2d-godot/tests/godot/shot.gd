extends Node2D
# Usage: godot --path . -- <dir> <skeleton> <atlas> <animation> <out.png> [frames]
func make(dir: String, skel: String, atlas_name: String) -> SpineSkeletonDataResource:
	var atlas := SpineAtlasResource.new()
	atlas.source_file = "res://assets/%s/%s" % [dir, atlas_name]
	var file := SpineSkeletonFileResource.new()
	file.source_file = "res://assets/%s/%s" % [dir, skel]
	var data := SpineSkeletonDataResource.new()
	data.atlas = atlas
	data.skeleton_file = file
	return data

func _ready() -> void:
	var a := OS.get_cmdline_user_args()
	var data := make(a[0], a[1], a[2])
	print("loaded=", data.is_skeleton_data_loaded(), " animations=", data.get_animation_names())
	var s := SpineSprite.new()
	s.skeleton_data = data
	s.initial_animation = a[3]
	s.position = Vector2(450, 600)
	s.scale = Vector2(float(a[6]) if a.size() > 6 else 0.6, float(a[6]) if a.size() > 6 else 0.6)
	add_child(s)
	var frames := int(a[5]) if a.size() > 5 else 60
	for i in frames:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png(a[4])
	print("stats=", s.get_mesh_stats())
	get_tree().quit()
