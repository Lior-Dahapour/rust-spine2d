extends Node2D
var failures := 0
func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func _ready() -> void:
	var atlas = load("res://assets/spineboy/spineboy.atlas")
	check("atlas loads via load()", atlas is SpineAtlasResource and atlas.get_page_count() == 1)
	var skel = load("res://assets/spineboy/spineboy-pro.skel")
	check("binary skeleton loads via load()", skel is SpineSkeletonFileResource and skel.is_binary and skel.bytes.size() > 100)
	var json = load("res://assets/spineboy/spineboy-pro.spine-json")
	check("spine-json loads via load()", json is SpineSkeletonFileResource and not json.is_binary)
	var deps := ResourceLoader.get_dependencies("res://assets/spineboy/spineboy.atlas")
	check("atlas reports its texture as a dependency", deps.size() == 1 and "spineboy.png" in deps[0])
	for f in [skel, json]:
		var data := SpineSkeletonDataResource.new()
		data.atlas = atlas
		data.skeleton_file = f
		check("data from %s" % ("binary" if f.is_binary else "json"), data.is_skeleton_data_loaded() and data.get_animation_names().has("run"))
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
