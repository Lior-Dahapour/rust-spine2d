extends Node
# SpineConstant: every constant exists under its official name with the official value, and
# nothing else is registered. The values are the ones in the official SpineConstant.h; the one
# addition is SpacingMode_Proportional. Runs headless.
# Usage: godot --headless --path . --scene res://constants_test.tscn
var failures := 0

func check(name: String, ok: bool) -> void:
	print(("PASS " if ok else "FAIL ") + name)
	if not ok: failures += 1

func _ready() -> void:
	var expected := {
		"Inherit_Normal": 0, "Inherit_OnlyTranslation": 1, "Inherit_NoRotationOrReflection": 2,
		"Inherit_NoScale": 3, "Inherit_NoScaleOrReflection": 4,
		"PositionMode_Fixed": 0, "PositionMode_Percent": 1,
		"SpacingMode_Length": 0, "SpacingMode_Fixed": 1, "SpacingMode_Percent": 2, "SpacingMode_Proportional": 3,
		"RotateMode_Tangent": 0, "RotateMode_Chain": 1, "RotateMode_ChainScale": 2,
		"BlendMode_Normal": 0, "BlendMode_Additive": 1, "BlendMode_Multiply": 2, "BlendMode_Screen": 3,
		"ScaleYMode_None": 0, "ScaleYMode_Uniform": 1, "ScaleYMode_Volume": 2,
		"UpdateMode_Process": 0, "UpdateMode_Physics": 1, "UpdateMode_Manual": 2,
		"BoneMode_Follow": 0, "BoneMode_Drive": 1,
		"Physics_None": 0, "Physics_Reset": 1, "Physics_Update": 2, "Physics_Pose": 3,
		"MixInterpolation_Linear": 0, "MixInterpolation_Smooth": 1, "MixInterpolation_SlowFast": 2,
		"MixInterpolation_FastSlow": 3, "MixInterpolation_Circle": 4,
	}
	check("the class is registered", ClassDB.class_exists("SpineConstant"))
	var registered := ClassDB.class_get_integer_constant_list("SpineConstant", true)
	check("exactly the expected constants are registered", registered.size() == expected.size())
	var wrong := []
	for name in expected:
		if not ClassDB.class_has_integer_constant("SpineConstant", name) or ClassDB.class_get_integer_constant("SpineConstant", name) != expected[name]:
			wrong.append(name)
	check("every constant has its official value %s" % str(wrong), wrong.is_empty())
	check("a constant can be written in a script", SpineConstant.Inherit_NoScale == 3 and SpineConstant.Physics_Pose == 3)
	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
