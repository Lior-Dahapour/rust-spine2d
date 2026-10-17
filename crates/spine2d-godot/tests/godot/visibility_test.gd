extends Node2D
# A hidden sprite, or one under a hidden ancestor, is not drawn; showing it draws again. Needs a
# window: it counts the mesh uploads, which headless rendering does not make.
# Usage: godot --path . --scene res://visibility_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

func uploads(s: SpineSprite) -> int:
	return s.get_mesh_stats().vertex_uploads

func _ready() -> void:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	data.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var holder := Node2D.new()
	add_child(holder)
	var s := SpineSprite.new()
	s.skeleton_data = data
	s.initial_animation = "run"
	s.position = Vector2(450, 600)
	s.scale = Vector2(0.5, 0.5)
	holder.add_child(s)
	await frames(20)

	var before := uploads(s)
	await frames(30)
	check("uploads while visible", uploads(s) - before >= 20)

	s.visible = false
	await frames(10)
	var hidden := uploads(s)
	await frames(30)
	check("no uploads while hidden", uploads(s) == hidden)

	s.visible = true
	await frames(5)
	var shown := uploads(s)
	await frames(30)
	check("uploads resume when shown", uploads(s) - shown >= 20)

	holder.visible = false
	await frames(10)
	hidden = uploads(s)
	await frames(30)
	check("no uploads under a hidden ancestor", uploads(s) == hidden)

	holder.visible = true
	await frames(5)
	shown = uploads(s)
	await frames(30)
	check("uploads resume when the ancestor is shown", uploads(s) - shown >= 20)

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
