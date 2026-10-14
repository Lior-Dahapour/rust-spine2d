extends Node2D
# Skin composition: SpineSkin, find_skin, set_skins, set_custom_skin. Runs headless; with a
# directory argument (and a window) it also saves screenshots of two outfits.
# Usage: godot --path . --scene res://skin_test.tscn [-- <screenshot dir>]
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func frames(n: int) -> void:
	for i in n: await get_tree().process_frame

# What every slot shows, as one string.
func look(sprite: SpineSprite) -> String:
	var parts := PackedStringArray()
	for name in sprite.get_slot_names():
		parts.append("%s=%s" % [name, sprite.get_slot(name).get_attachment_name()])
	return ",".join(parts)

func outfit(data: SpineSkeletonDataResource, names: Array) -> SpineSkin:
	var skin := SpineSkin.create("outfit")
	for n in names:
		skin.add_skin(data.find_skin(n))
	return skin

func _ready() -> void:
	var shots := OS.get_cmdline_user_args()
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/mix-and-match/mix-and-match.atlas")
	data.skeleton_file = load("res://assets/mix-and-match/mix-and-match-pro.skel")
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	sprite.initial_animation = "idle"
	sprite.position = Vector2(450, 640)
	sprite.scale = Vector2(0.9, 0.9)
	add_child(sprite)
	await frames(3)

	check("find_skin returns null for an unknown name", data.find_skin("no-such-skin") == null)
	check("SpineSkin.create refuses an empty name", SpineSkin.create("") == null)
	var base := data.find_skin("skin-base")
	check("find_skin returns the named skin", base != null and base.get_name() == "skin-base")
	var base_count := base.get_attachment_count()
	check("a skin has attachments", base_count > 0)

	var girl := ["skin-base", "nose/short", "eyelids/girly", "eyes/violet", "hair/pink", "clothes/dress-blue", "legs/boots-pink"]
	var boy := ["skin-base", "nose/long", "eyelids/semiclosed", "eyes/green", "hair/short-red", "clothes/hoodie-orange", "legs/pants-jeans", "legs/boots-red"]

	var custom := outfit(data, girl)
	check("combining skins collects their attachments", custom.get_attachment_count() > base_count)
	check("combining never changes the skeleton data's skins", data.find_skin("skin-base").get_attachment_count() == base_count)

	var before := look(sprite)
	check("set_custom_skin accepts a skin of this skeleton", sprite.set_custom_skin(custom))
	var girl_look := look(sprite)
	check("the custom skin changes what the slots show", girl_look != before)
	if shots.size() > 0:
		await frames(20)
		await RenderingServer.frame_post_draw
		get_viewport().get_texture().get_image().save_png(shots[0] + "/skin_girl.png")

	check("set_skins accepts names", sprite.set_skins(PackedStringArray(boy)))
	var boy_after_girl := look(sprite)
	check("another outfit looks different", boy_after_girl != girl_look)
	if shots.size() > 0:
		await frames(20)
		await RenderingServer.frame_post_draw
		get_viewport().get_texture().get_image().save_png(shots[0] + "/skin_boy.png")

	# Switching skins does not reset the slots; set_slots_to_setup_pose does. The clean result is
	# what a sprite that never wore the girl's outfit shows.
	var fresh := SpineSprite.new()
	fresh.skeleton_data = data
	add_child(fresh)
	await frames(2)
	check("a fresh sprite accepts the boy's outfit", fresh.set_skins(PackedStringArray(boy)))
	var boy_clean := look(fresh)
	sprite.set_slots_to_setup_pose()
	check("after set_slots_to_setup_pose the sprite looks like one that never wore the old outfit", look(sprite) == boy_clean)
	fresh.queue_free()

	check("set_skins with the girl's names matches set_custom_skin", sprite.set_skins(PackedStringArray(girl)))
	sprite.set_slots_to_setup_pose()
	check("the girl's outfit looks the same as at first", look(sprite) == girl_look)
	check("an unknown name is refused", not sprite.set_skins(PackedStringArray(["skin-base", "hat/none"])))
	check("a refused call changes nothing", look(sprite) == girl_look)
	sprite.set_skins(PackedStringArray(["skin-base", "hair/blue", "hair/pink"]))
	sprite.set_slots_to_setup_pose()
	var pink_wins := look(sprite)
	sprite.set_skins(PackedStringArray(["skin-base", "hair/pink"]))
	sprite.set_slots_to_setup_pose()
	check("a later name replaces an earlier one", look(sprite) == pink_wins)

	var copy := SpineSkin.create("copy")
	copy.copy_skin(data.find_skin("hair/pink"))
	check("copy_skin copies the attachments", copy.get_attachment_count() == data.find_skin("hair/pink").get_attachment_count())

	# Attachments: lookup, entries, names, copy, remove.
	var hair := data.find_skin("hair/pink")
	var entries := hair.get_attachments()
	check("get_attachments lists the entries", entries.size() == hair.get_attachment_count() and entries.size() > 0)
	var entry: SpineSkinEntry = entries[0]
	var found := hair.get_attachment(entry.get_slot_index(), entry.get_name())
	check("get_attachment finds an entry's attachment", found != null and found.get_attachment_name() == entry.get_attachment().get_attachment_name())
	check("find_names_for_slot lists the entry's name", entry.get_name() in hair.find_names_for_slot(entry.get_slot_index()))
	check("an attachment has a type", found.get_type() in ["region", "mesh"])
	check("get_attachment is null for an unknown name", hair.get_attachment(entry.get_slot_index(), "no-such-attachment") == null)
	check("slot and bone numbers come from the data", data.get_slot_index("no-such-slot") == -1 and data.get_slot_index(sprite.get_slot_names()[0]) == 0 and data.get_bone_index("no-such-bone") == -1 and data.get_bone_index(sprite.get_bone_names()[0]) == 0)

	var mix := SpineSkin.create("mix")
	mix.set_attachment(entry.get_slot_index(), entry.get_name(), found)
	check("set_attachment adds an attachment", mix.get_attachment_count() == 1)
	var duplicate := found.copy()
	mix.set_attachment(entry.get_slot_index(), "copy", duplicate)
	check("a copy can go in under another name", mix.get_attachment_count() == 2 and mix.find_names_for_slot(entry.get_slot_index()).size() == 2)
	mix.remove_attachment(entry.get_slot_index(), "copy")
	check("remove_attachment removes it", mix.get_attachment_count() == 1 and mix.get_attachment(entry.get_slot_index(), "copy") == null)
	check("changing a skin does not change the data's skin", data.find_skin("hair/pink").get_attachment_count() == hair.get_attachment_count())
	check("a skin made of one attachment can be shown", sprite.set_custom_skin(mix))

	var other := SpineSkeletonDataResource.new()
	other.atlas = load("res://assets/spineboy/spineboy.atlas")
	other.skeleton_file = load("res://assets/spineboy/spineboy-pro.skel")
	var boy_sprite := SpineSprite.new()
	boy_sprite.skeleton_data = other
	add_child(boy_sprite)
	await frames(2)
	check("a skin from a bigger skeleton is refused", not boy_sprite.set_custom_skin(custom))
	check("a plain skin name still works afterwards", boy_sprite.set_skins(PackedStringArray(["default"])))

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
