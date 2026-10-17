extends Node2D
# The transform, path, physics and slider classes, on the core's synthetic skeletons. Every check
# compares against a value written in the fixture JSON, so a wrong field or a swapped getter fails.
# Usage: godot --headless --path . --scene res://constraint_kinds_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func near(a: float, b: float) -> bool:
	return absf(a - b) < 0.0005

# Whether `pose` belongs to `bone`: writes through the pose must show on the bone's applied pose.
func is_bone(pose: SpineBonePose, bone: SpineBone) -> bool:
	var applied := bone.get_applied_pose()
	var before := applied.get_x()
	pose.set_x(before + 1234.5)
	var same := absf(applied.get_x() - (before + 1234.5)) < 0.001
	pose.set_x(before)
	return same

func make_sprite(fixture: String) -> SpineSprite:
	var data := SpineSkeletonDataResource.new()
	data.atlas = load("res://assets/spineboy/spineboy.atlas")
	var file := SpineSkeletonFileResource.new()
	file.source_file = "res://assets/synthetic/%s.json" % fixture
	data.skeleton_file = file
	var sprite := SpineSprite.new()
	sprite.skeleton_data = data
	add_child(sprite)
	return sprite

func _ready() -> void:
	await test_transform()
	await test_path()
	await test_physics()
	await test_slider()
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)

func test_transform() -> void:
	var sprite := make_sprite("transform-options")
	await get_tree().process_frame
	var skeleton := sprite.get_skeleton()
	check("transform: five constraints", skeleton.get_transform_constraints().size() == 5)
	check("transform: an IK lookup does not find it", skeleton.find_ik_constraint("tc1") == null)
	check("transform: unknown name", skeleton.find_transform_constraint("nope") == null)

	var tc1 := skeleton.find_transform_constraint("tc1")
	check("transform: name", tc1.get_data().get_constraint_name() == "tc1")
	check("transform: constrained bones", tc1.get_bones().size() == 1 and is_bone(tc1.get_bones()[0], sprite.get_bone("t1")))
	check("transform: source", tc1.get_source().get_name() == "src")
	var pose := tc1.get_pose()
	check("transform: pose mixes", near(pose.get_mix_rotate(), 1.0) and near(pose.get_mix_x(), 0.8) and near(pose.get_mix_y(), 0.8))
	pose.set_mix_scale_x(0.3)
	pose.set_mix_scale_y(0.4)
	pose.set_mix_shear_y(0.5)
	check("transform: scale and shear mixes round trip", near(pose.get_mix_scale_x(), 0.3) and near(pose.get_mix_scale_y(), 0.4) and near(pose.get_mix_shear_y(), 0.5))
	check("transform: the setup pose keeps the data", near(tc1.get_data().get_setup_pose().get_mix_x(), 0.8))

	check("transform: data bones", tc1.get_data().get_bones().size() == 1 and tc1.get_data().get_bones()[0].get_bone_name() == "t1")
	check("transform: data source", tc1.get_data().get_source().get_bone_name() == "src")
	var tc2 := skeleton.find_transform_constraint("tc2").get_data()
	check("transform: local flags", tc2.is_local_source() and tc2.is_local_target())
	check("transform: tc1 is not local", not tc1.get_data().is_local_source() and not tc1.get_data().is_local_target())
	check("transform: mixes of tc2", near(tc2.get_setup_pose().get_mix_scale_y(), 0.6) and near(tc2.get_setup_pose().get_mix_shear_y(), 0.9))
	var tc3 := skeleton.find_transform_constraint("tc3")
	check("transform: two constrained bones", tc3.get_bones().size() == 2)
	var offsets := tc3.get_data()
	check("transform: offsets", near(offsets.get_offset_rotation(), 15.0) and near(offsets.get_offset_x(), 10.0) and near(offsets.get_offset_y(), -5.0))
	check("transform: data lists both bones in order", tc3.get_data().get_bones().size() == 2 and tc3.get_data().get_bones()[0].get_bone_name() == "t3" and tc3.get_data().get_bones()[1].get_bone_name() == "t4")
	var tc4 := skeleton.find_transform_constraint("tc4").get_data()
	check("transform: scale and shear offsets", near(tc4.get_offset_scale_x(), 0.1) and near(tc4.get_offset_scale_y(), -0.1) and near(tc4.get_offset_shear_y(), 5.0))

	tc1.update(skeleton)
	check("transform: update is a stub that changes nothing", near(tc1.get_pose().get_mix_x(), 0.8))
	tc1.set_source(sprite.get_bone("t2"))
	check("transform: set_source is visible at once", tc1.get_source().get_name() == "t2")
	await get_tree().process_frame
	await get_tree().process_frame
	check("transform: the new source survives the rebuild", tc1.get_source().get_name() == "t2")
	tc1.set_source(null)
	check("transform: a null source is ignored", tc1.get_source().get_name() == "t2")
	check("transform: active", tc1.is_active())
	sprite.free()

func test_path() -> void:
	var sprite := make_sprite("path-options")
	await get_tree().process_frame
	var skeleton := sprite.get_skeleton()
	check("path: five constraints", skeleton.get_path_constraints().size() == 5)
	check("path: unknown name", skeleton.find_path_constraint("nope") == null)

	var cp1 := skeleton.find_path_constraint("cp1")
	check("path: name", cp1.get_data().get_constraint_name() == "cp1")
	check("path: four bones", cp1.get_bones().size() == 4 and is_bone(cp1.get_bones()[0], sprite.get_bone("p1_0")) and is_bone(cp1.get_bones()[3], sprite.get_bone("p1_3")) and not is_bone(cp1.get_bones()[0], sprite.get_bone("p1_1")))
	check("path: slot", cp1.get_slot().get_name() == "p1")
	var pose := cp1.get_pose()
	check("path: pose", near(pose.get_position(), 0.05) and near(pose.get_spacing(), 5.0))
	check("path: mixes", near(pose.get_mix_rotate(), 0.9) and near(pose.get_mix_x(), 1.0) and near(pose.get_mix_y(), 0.9))
	pose.set_position(0.25)
	pose.set_spacing(2.0)
	check("path: position and spacing round trip", near(pose.get_position(), 0.25) and near(pose.get_spacing(), 2.0))
	check("path: the setup pose keeps the data", near(cp1.get_data().get_setup_pose().get_position(), 0.05))

	var path_bones: Array = cp1.get_data().get_bones()
	check("path: data bones", path_bones.size() == 4 and path_bones[0].get_bone_name() == "p1_0" and path_bones[3].get_bone_name() == "p1_3")
	var data1 := cp1.get_data()
	check("path: cp1 modes", data1.get_position_mode() == SpineConstant.PositionMode_Percent and data1.get_spacing_mode() == SpineConstant.SpacingMode_Length and data1.get_rotate_mode() == SpineConstant.RotateMode_Tangent)
	var data2 := skeleton.find_path_constraint("cp2").get_data()
	check("path: cp2 modes", data2.get_position_mode() == SpineConstant.PositionMode_Fixed and data2.get_spacing_mode() == SpineConstant.SpacingMode_Fixed and data2.get_rotate_mode() == SpineConstant.RotateMode_Chain)
	var data3 := skeleton.find_path_constraint("cp3").get_data()
	check("path: cp3 modes", data3.get_spacing_mode() == SpineConstant.SpacingMode_Percent and data3.get_rotate_mode() == SpineConstant.RotateMode_ChainScale and near(data3.get_offset_rotation(), 15.0))
	check("path: cp4 spacing mode", skeleton.find_path_constraint("cp4").get_data().get_spacing_mode() == SpineConstant.SpacingMode_Proportional)
	data1.set_position_mode(0)
	data1.set_spacing_mode(3)
	data1.set_rotate_mode(2)
	data1.set_offset_rotation(99.0)
	check("path: data setters change the shared data", data1.get_position_mode() == 0 and data1.get_spacing_mode() == 3 and data1.get_rotate_mode() == 2 and near(data1.get_offset_rotation(), 99.0))
	data1.set_position_mode(7)
	data1.set_spacing_mode(-1)
	data1.set_rotate_mode(3)
	check("path: an unknown mode is refused", data1.get_position_mode() == 0 and data1.get_spacing_mode() == 3 and data1.get_rotate_mode() == 2)
	data1.set_position_mode(1)
	data1.set_spacing_mode(0)
	data1.set_rotate_mode(0)
	data1.set_offset_rotation(0.0)

	cp1.update(skeleton)
	check("path: update is a stub that changes nothing", near(cp1.get_pose().get_position(), 0.25))
	cp1.set_slot(sprite.get_slot("p2"))
	check("path: set_slot is visible at once", cp1.get_slot().get_name() == "p2")
	await get_tree().process_frame
	await get_tree().process_frame
	check("path: the new slot survives the rebuild", cp1.get_slot().get_name() == "p2")
	cp1.set_slot(null)
	check("path: a null slot is ignored", cp1.get_slot().get_name() == "p2")
	sprite.free()

func test_physics() -> void:
	var sprite := make_sprite("physics-options")
	await get_tree().process_frame
	var skeleton := sprite.get_skeleton()
	check("physics: seven constraints, found by name", skeleton.find_physics_constraint("ph6") != null and skeleton.find_physics_constraint("ph7") == null)
	check("physics: an IK lookup does not find it", skeleton.find_ik_constraint("ph0") == null)

	var ph0 := skeleton.find_physics_constraint("ph0")
	check("physics: name and bone", ph0.get_data().get_constraint_name() == "ph0" and is_bone(ph0.get_bone(), sprite.get_bone("p0")))
	var data := ph0.get_data()
	check("physics: x, y and limit", near(data.get_x(), 1.0) and near(data.get_y(), 1.0) and near(data.get_limit(), 80.0))
	check("physics: default step is 1/60", near(data.get_step(), 1.0 / 60.0))
	check("physics: step of 30 fps", near(skeleton.find_physics_constraint("ph1").get_data().get_step(), 1.0 / 30.0))
	var setup := data.get_setup_pose()
	check("physics: setup pose", near(setup.get_inertia(), 0.7) and near(setup.get_strength(), 60.0) and near(setup.get_damping(), 0.8))
	check("physics: setup mass, wind, gravity, mix", near(setup.get_mass_inverse(), 1.0 / 1.5) and near(setup.get_wind(), 0.2) and near(setup.get_gravity(), 0.6) and near(setup.get_mix(), 1.0))
	var pose := ph0.get_pose()
	pose.set_inertia(0.1)
	pose.set_strength(11.0)
	pose.set_damping(0.2)
	pose.set_mass_inverse(3.0)
	pose.set_wind(4.0)
	pose.set_gravity(5.0)
	pose.set_mix(0.5)
	check("physics: pose round trips", near(pose.get_inertia(), 0.1) and near(pose.get_strength(), 11.0) and near(pose.get_damping(), 0.2) and near(pose.get_mass_inverse(), 3.0) and near(pose.get_wind(), 4.0) and near(pose.get_gravity(), 5.0) and near(pose.get_mix(), 0.5))
	check("physics: the applied pose is readable", ph0.get_applied_pose().get_mix() >= 0.0)

	check("physics: data bone", data.get_bone().get_bone_name() == "p0" and skeleton.find_physics_constraint("ph3").get_data().get_bone().get_bone_name() == "p3")
	var ph1 := skeleton.find_physics_constraint("ph1").get_data()
	check("physics: ph1 rotate", near(ph1.get_rotate(), 1.0))
	var ph2 := skeleton.find_physics_constraint("ph2").get_data()
	check("physics: ph2 shear and wind flag", near(ph2.get_shear_x(), 0.8) and ph2.is_wind_global() and not ph2.is_gravity_global())
	var ph3 := skeleton.find_physics_constraint("ph3").get_data()
	check("physics: ph3 scale", near(ph3.get_scale_x(), 0.5) and ph3.get_scale_y_mode() == SpineConstant.ScaleYMode_Uniform and ph3.is_gravity_global())
	var ph4 := skeleton.find_physics_constraint("ph4").get_data()
	check("physics: ph4 flags", ph4.get_scale_y_mode() == SpineConstant.ScaleYMode_Volume and ph4.is_mass_global() and ph4.is_inertia_global() and not ph4.is_strength_global())
	var ph5 := skeleton.find_physics_constraint("ph5").get_data()
	check("physics: ph5 flags", ph5.is_mix_global() and ph5.is_strength_global() and ph5.is_damping_global() and not ph5.is_wind_global())
	ph5.set_scale_y_mode(2)
	check("physics: the data setter changes the shared data", ph5.get_scale_y_mode() == 2)
	ph5.set_scale_y_mode(9)
	check("physics: an unknown mode is refused", ph5.get_scale_y_mode() == 2)
	ph5.set_scale_y_mode(0)

	ph0.update(skeleton, SpineConstant.Physics_Update)
	check("physics: update is a stub that changes nothing", near(ph0.get_pose().get_inertia(), 0.1))
	ph0.translate(10.0, 5.0)
	ph0.rotate(0.0, 0.0, 30.0)
	ph0.reset(skeleton)
	var other := make_sprite("physics-options")
	ph0.reset(other.get_skeleton())
	ph0.reset(null)
	await get_tree().process_frame
	check("physics: translate, rotate and reset keep the sprite running", ph0.is_valid())
	ph0.set_bone(sprite.get_bone("p3").get_applied_pose())
	check("physics: set_bone is visible at once", is_bone(ph0.get_bone(), sprite.get_bone("p3")))
	await get_tree().process_frame
	await get_tree().process_frame
	check("physics: the new bone survives the rebuild", is_bone(ph0.get_bone(), sprite.get_bone("p3")))
	ph0.set_bone(other.get_bone("p1").get_applied_pose())
	check("physics: a bone pose of another sprite is refused", is_bone(ph0.get_bone(), sprite.get_bone("p3")))
	ph0.set_bone(null)
	check("physics: a null bone is ignored", is_bone(ph0.get_bone(), sprite.get_bone("p3")))
	other.free()
	sprite.free()

func test_slider() -> void:
	var sprite := make_sprite("slider-options")
	await get_tree().process_frame
	var skeleton := sprite.get_skeleton()
	check("slider: four sliders", skeleton.get_sliders().size() == 4)
	check("slider: an IK constraint is not a slider", skeleton.find_slider("ikS") == null)
	check("slider: the transform lookup skips sliders", skeleton.find_transform_constraint("wave") == null)
	check("slider: the IK constraint of the file is found", skeleton.get_ik_constraints().size() == 1 and skeleton.get_transform_constraints().size() == 1)

	var wave := skeleton.find_slider("wave")
	check("slider: name and bone", wave.get_data().get_constraint_name() == "wave" and wave.get_bone().get_name() == "driver")
	var data := wave.get_data()
	check("slider: wave data", near(data.get_scale(), 0.01) and near(data.get_max(), 100.0) and not data.get_additive() and not data.get_loop() and not data.get_local())
	check("slider: data bone", data.get_bone().get_bone_name() == "driver")
	data.set_bone(sprite.get_bone("dw").get_data())
	check("slider: the data's set_bone changes the shared data", data.get_bone().get_bone_name() == "dw")
	data.set_bone(sprite.get_bone("driver").get_data())
	var plocal := skeleton.find_slider("plocal").get_data()
	check("slider: plocal flags", plocal.get_local() and plocal.get_additive() and not plocal.get_loop())
	var pulse := skeleton.find_slider("pulse").get_data()
	check("slider: pulse flags", pulse.get_local() and pulse.get_additive() and pulse.get_loop())
	check("slider: setup mix", near(wave.get_data().get_setup_pose().get_mix(), 1.0) and near(plocal.get_setup_pose().get_mix(), 0.5))

	var timed := skeleton.find_slider("timed")
	check("slider: a time driven slider has no bone data", timed.get_data().get_bone() == null)
	check("slider: a time driven slider has no bone", timed.get_bone() == null)
	check("slider: its setup time", near(timed.get_data().get_setup_pose().get_time(), 0.5) and near(timed.get_data().get_setup_pose().get_mix(), 0.6))
	var pose := timed.get_pose()
	pose.set_time(0.25)
	pose.set_mix(0.9)
	check("slider: pose round trips", near(pose.get_time(), 0.25) and near(pose.get_mix(), 0.9))
	data.set_additive(true)
	data.set_loop(true)
	data.set_scale(5.0)
	data.set_offset(5.0)
	data.set_max(5.0)
	data.set_local(true)
	check("slider: data setters change the shared data", data.get_additive() and data.get_loop() and near(data.get_scale(), 5.0) and near(data.get_offset(), 5.0) and near(data.get_max(), 5.0) and data.get_local())
	data.set_additive(false)
	data.set_loop(false)
	data.set_scale(0.01)
	data.set_offset(0.0)
	data.set_max(100.0)
	data.set_local(false)

	wave.update(skeleton, SpineConstant.Physics_Update)
	check("slider: update is a stub that changes nothing", near(wave.get_pose().get_mix(), 1.0))
	wave.set_bone(sprite.get_bone("dw"))
	check("slider: set_bone is visible at once", wave.get_bone().get_name() == "dw")
	await get_tree().process_frame
	await get_tree().process_frame
	check("slider: the new bone survives the rebuild", wave.get_bone().get_name() == "dw")
	timed.set_bone(sprite.get_bone("driver"))
	check("slider: a time driven slider can get a bone", timed.get_bone().get_name() == "driver")
	check("slider: active", wave.is_active())
	sprite.free()
