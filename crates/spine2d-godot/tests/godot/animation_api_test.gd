extends Node2D
# SpineAnimationState, SpineAnimation, SpineTimeline, SpineEvent, SpineEventData and the track
# entry methods that point at them, plus the arguments of the animation signals. Runs headless on
# spineboy.
# Usage: godot --headless --path . --scene res://animation_api_test.tscn
var failures := 0
var started_args_ok := false
var event_seen: SpineEvent = null
var event_state_ok := false

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
	sprite.animation_started.connect(func(s, st, e): started_args_ok = s == sprite and st is SpineAnimationState and e is SpineTrackEntry)
	sprite.animation_event.connect(func(_s, st, _e, ev):
		event_seen = ev
		event_state_ok = st is SpineAnimationState)
	await get_tree().process_frame

	var state := sprite.get_animation_state()
	check("the sprite has an animation state", state != null and state.is_valid())
	check("no tracks before an animation", state.get_num_tracks() == 0)
	var run := state.set_animation("run")
	check("set_animation defaults to looping on track 0", run != null and run.get_loop() and run.get_track_index() == 0)
	check("a track exists now", state.get_num_tracks() == 1 and state.get_track(0).get_animation_name() == "run")
	check("an unknown animation gives null", state.set_animation("no such animation") == null)
	var shoot := state.set_animation("shoot", false, 1)
	check("another track", state.get_num_tracks() == 2 and shoot.get_track_index() == 1 and not shoot.get_loop())
	check("the time scale round trips", near(state.get_time_scale(), 1.0))
	state.set_time_scale(0.5)
	check("a changed time scale is read back", near(state.get_time_scale(), 0.5) and near(sprite.time_scale, 0.5))
	state.set_time_scale(1.0)

	var animation := run.get_animation()
	check("the entry's animation", animation != null and animation.get_name() == "run" and near(animation.get_duration(), 0.6667, 0.01))
	var timelines := animation.get_timelines()
	check("an animation has timelines", timelines.size() > 0)
	var rotate = null
	for timeline in timelines:
		if timeline.get_type() == "RotateTimeline": rotate = timeline
	check("a rotate timeline is among them", rotate != null)
	var frames = rotate.get_frames()
	check("frames agree with the entry and key counts", frames.size() == rotate.get_frame_entries() * rotate.get_frame_count() and rotate.get_frame_count() > 1)
	check("the duration is the last key time", near(rotate.get_duration(), frames[frames.size() - rotate.get_frame_entries()]))
	var ids = rotate.get_property_ids()
	check("a rotate timeline has one property id", ids.size() == 1 and typeof(ids[0]) == TYPE_INT)
	check("the animation has a timeline for that property", animation.has_timeline(ids))
	check("it has none for an unknown property", not animation.has_timeline([123456789]))

	var duration := animation.get_duration()
	animation.set_duration(duration + 1.0)
	check("the animation duration is shared data", near(data.get_animation_duration("run"), duration + 1.0))
	animation.set_duration(duration)

	var next := state.add_animation("jump", 0.0, false, 0)
	check("the queued entry follows the current one", next.get_previous() != null and next.get_previous().get_animation_name() == "run" and run.get_next().get_animation_name() == "jump")
	var walk := state.set_animation("walk", true, 0)
	check("a replaced entry is mixed from", walk.get_mixing_from() != null and walk.get_mixing_from().get_animation_name() == "run")
	check("the mixed from entry points back", walk.get_mixing_from().get_mixing_to().get_animation_name() == "walk")

	walk.set_mix_attachment_threshold(0.3)
	walk.set_mix_draw_order_threshold(0.4)
	walk.set_alpha_attachment_threshold(0.2)
	walk.set_animation_last(0.1)
	walk.set_mix_time(0.05)
	walk.set_track_end(9.0)
	check("the thresholds and times round trip", near(walk.get_mix_attachment_threshold(), 0.3) and near(walk.get_mix_draw_order_threshold(), 0.4) and near(walk.get_alpha_attachment_threshold(), 0.2) and near(walk.get_animation_last(), 0.1) and near(walk.get_mix_time(), 0.05) and near(walk.get_track_end(), 9.0))
	walk.set_mix_interpolation(SpineConstant.MixInterpolation_Circle)
	check("the mix interpolation round trips", walk.get_mix_interpolation() == SpineConstant.MixInterpolation_Circle)
	walk.set_mix_interpolation(99)
	check("an unknown interpolation is refused", walk.get_mix_interpolation() == SpineConstant.MixInterpolation_Circle)
	walk.set_mix_duration_and_delay(0.4, 0.1)
	check("the mix duration is set", near(walk.get_mix_duration(), 0.4))
	check("a fresh entry was not applied yet and completes after a full run", not walk.was_applied() and walk.get_track_complete() > 0.0)
	walk.reset_rotation_directions()

	var empty := state.set_empty_animation(2, 0.2)
	check("an empty animation entry", empty != null and empty.get_track_index() == 2 and empty.get_animation().get_name() == "<empty>")
	empty.get_animation().set_duration(5.0)
	check("a change to the empty animation is refused", near(empty.get_animation().get_duration(), 0.0))
	var queued_empty := state.add_empty_animation(2, 0.3, 0.1)
	check("a queued empty animation", queued_empty != null and near(queued_empty.get_mix_duration(), 0.3))
	state.set_empty_animations(0.1)
	state.clear_track(2)
	check("a cleared track has no entry", state.get_track(2) == null)
	state.clear_tracks()
	check("cleared tracks leave no entry", state.get_track(0) == null and state.get_track(1) == null)

	state.set_animation("run", true, 0)
	await get_tree().process_frame
	await get_tree().process_frame
	check("the started signal carries the sprite, the state and the entry", started_args_ok)
	await get_tree().create_timer(1.5).timeout
	check("the animation event signal carries a SpineEvent", event_seen != null and event_state_ok)
	check("the event's data is its type", event_seen.get_data().get_event_name() == "footstep")
	check("the event has a time and a volume", event_seen.get_time() > 0.0 and event_seen.get_volume() >= 0.0)
	var type_data := event_seen.get_data()
	var type_int := type_data.get_int_value()
	type_data.set_int_value(type_int + 7)
	check("an event type's payload is shared data", event_seen.get_data().get_int_value() == type_int + 7)
	type_data.set_int_value(type_int)
	event_seen.set_int_value(42)
	check("an event is a copy: changing it changes only it", event_seen.get_int_value() == 42 and event_seen.get_data().get_int_value() == type_int)

	var manual := SpineSprite.new()
	manual.skeleton_data = data
	manual.update_mode = 2
	add_child(manual)
	await get_tree().process_frame
	var manual_state := manual.get_animation_state()
	manual_state.set_animation("run", true, 0)
	var track_time := manual_state.get_track(0).get_track_time()
	manual_state.update(0.25)
	check("update advances the state at once", near(manual_state.get_track(0).get_track_time(), track_time + 0.25))
	var hip := manual.get_bone("hip")
	var setup := hip.get_data().get_setup_pose()
	manual_state.apply(manual.get_skeleton())
	var pose := hip.get_pose()
	check("apply poses the skeleton at once", not (near(pose.get_x(), setup.get_x()) and near(pose.get_y(), setup.get_y()) and near(pose.get_rotation(), setup.get_rotation())))
	manual_state.apply(sprite.get_skeleton())
	check("a skeleton of another sprite is refused without harm", manual_state.is_valid())

	var run_animation := manual_state.get_track(0).get_animation()
	var fired := []
	run_animation.apply(manual.get_skeleton(), 0.0, 0.6, false, fired, 1.0, true, false, false, false)
	check("Animation.apply collects the events it fired", fired.size() >= 1 and fired[0] is SpineEvent and fired[0].get_data().get_event_name() == "footstep")
	var timeline_events := []
	for timeline in run_animation.get_timelines():
		if timeline.get_type() == "EventTimeline":
			timeline.apply(manual.get_skeleton(), 0.0, 0.6, timeline_events, 1.0, true, false, false, false)
	check("Timeline.apply collects events too", timeline_events.size() == fired.size())

	print("RESULT failures=%d" % failures)
	get_tree().quit(failures)
