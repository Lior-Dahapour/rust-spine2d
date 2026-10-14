use spine2d_core::*;
use std::sync::{Arc, Mutex};

// Spine 4.3 defaults to a y-down world; these tests are written for y-up like the reference's
// headless test.
#[ctor::ctor(unsafe)]
fn init_y_up() {
    spine2d_core::set_y_down(false);
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

fn rotate_anim(name: &str, bone: usize, frames: &[(f32, f32)]) -> Arc<Animation> {
    let mut t = BoneTimeline1::new(Bone1Kind::Rotate, frames.len(), 0, bone);
    for (i, &(time, v)) in frames.iter().enumerate() {
        t.set_frame(i, time, v);
        if i + 1 < frames.len() {
            t.curve.set_linear(i);
        }
    }
    let duration = frames.last().unwrap().0;
    Arc::new(Animation::new(name, vec![Timeline::Bone1(t)], duration))
}

fn event(name: &str, time: f32) -> Arc<Event> {
    Arc::new(Event::new(time, Arc::new(EventData::new(name))))
}

struct Fixture {
    sk: Skeleton,
    state: AnimationState,
    log: Arc<Mutex<Vec<String>>>,
}

fn anim(f: &Fixture, name: &str) -> Arc<Animation> {
    f.state
        .data()
        .skeleton_data()
        .find_animation(name)
        .unwrap()
        .clone()
}

/// Skeleton: root bone (setup rotation 0), one slot with attachments "a" (setup) and "b".
/// Animations:
///   ramp:   rotate 0 -> 90 over 1s
///   hold:   rotate constant 100 over 1s
///   events: event "e1" at 0.5, duration 1s
///   keyb:   attachment "b" at t=0, duration 1s
///   idle:   no timelines, duration 1s
fn fixture(mix: f32) -> Fixture {
    let mut skin = Skin::new("default");
    for n in ["a", "b"] {
        skin.set_attachment(0, n, Arc::new(Attachment::Point(PointAttachment::new(n))));
    }
    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    let mut slot = SlotData::new(0, "s0", BoneId(0));
    slot.attachment_name = "a".into();
    data.slots = vec![slot];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    data.default_skin = Some(skin);

    let mut events = EventTimeline::new(1);
    events.set_frame(0, event("e1", 0.5));
    let mut keyb = AttachmentTimeline::new(1, 0);
    keyb.set_frame(0, 0.0, "b");
    data.animations = vec![
        rotate_anim("ramp", 0, &[(0.0, 0.0), (1.0, 90.0)]),
        rotate_anim("hold", 0, &[(0.0, 100.0), (1.0, 100.0)]),
        Arc::new(Animation::new("events", vec![Timeline::Event(events)], 1.0)),
        Arc::new(Animation::new(
            "keyb",
            vec![Timeline::Attachment(keyb)],
            1.0,
        )),
        Arc::new(Animation::new("idle", vec![], 1.0)),
    ];
    let data = Arc::new(data);

    let mut state_data = AnimationStateData::new(data.clone());
    state_data.default_mix = mix;
    let sk = Skeleton::new(data);
    let mut state = AnimationState::new(Arc::new(state_data));

    let log = Arc::new(Mutex::new(Vec::new()));
    let l = log.clone();
    state.set_listener(move |st, kind, entry, event| {
        let name = st
            .entry(entry)
            .map(|e| e.animation().name().to_string())
            .unwrap_or_else(|| "?".into());
        let line = match event {
            Some(e) => format!("{kind:?} {name} {}", e.data.name),
            None => format!("{kind:?} {name}"),
        };
        l.lock().unwrap().push(line);
    });
    Fixture { sk, state, log }
}

impl Fixture {
    fn step(&mut self, dt: f32) -> bool {
        self.state.update(dt);
        let applied = self.state.apply(&mut self.sk);
        self.sk.update_world_transform(Physics::None);
        applied
    }

    fn rotation(&self) -> f32 {
        self.sk.bone(BoneId(0)).pose.local.rotation
    }

    fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }

    fn attachment(&self) -> String {
        self.sk
            .slot(SlotId(0))
            .pose
            .attachment
            .as_ref()
            .map(|a| a.name().to_string())
            .unwrap_or_default()
    }
}

#[test]
fn plays_loops_and_clamps() {
    let mut f = fixture(0.0);
    let a = anim(&f, "ramp");
    f.state.set_animation(0, a.clone(), false);
    assert!(f.step(0.5));
    approx(f.rotation(), 45.0);
    f.step(2.0);
    approx(f.rotation(), 90.0); // clamps at the last key

    let mut f = fixture(0.0);
    f.state.set_animation(0, a, true);
    f.step(1.25);
    approx(f.rotation(), 22.5);
}

#[test]
fn apply_returns_false_with_no_tracks() {
    let mut f = fixture(0.0);
    assert!(!f.step(0.1));
}

#[test]
fn fires_start_event_complete_end_dispose_in_order() {
    let mut f = fixture(0.0);
    let id = f.state.set_animation_by_name(0, "events", false).unwrap();
    assert_eq!(f.take_log(), ["Start events"]);

    f.step(0.6);
    assert_eq!(f.take_log(), ["Event events e1"]);
    f.step(0.6);
    assert_eq!(f.take_log(), ["Complete events"]);

    f.state.clear_track(0);
    assert_eq!(f.take_log(), ["End events", "Dispose events"]);
    // The disposed entry's handle is now invalid rather than dangling.
    assert!(f.state.entry(id).is_none());
    assert!(f.state.track(0).is_none());
}

#[test]
fn loop_fires_complete_each_iteration() {
    let mut f = fixture(0.0);
    f.state.set_animation_by_name(0, "events", true).unwrap();
    f.take_log();
    f.step(0.6);
    f.step(0.6); // wraps: complete, then the event again after restart? (0.5 already passed)
    let log = f.take_log();
    assert_eq!(log, ["Event events e1", "Complete events"]);
    f.step(0.6); // 1.2 -> 1.8: second iteration's event fires
    assert_eq!(f.take_log(), ["Event events e1"]);
}

#[test]
fn queued_animation_starts_after_the_current_completes() {
    let mut f = fixture(0.0);
    let ramp = f.state.set_animation_by_name(0, "ramp", false).unwrap();
    let hold = f
        .state
        .add_animation_by_name(0, "hold", false, 0.0)
        .unwrap();
    // delay = previous track_complete (1.0) - mix (0).
    approx(f.state.entry(hold).unwrap().delay(), 1.0);
    assert_eq!(f.state.entry(ramp).unwrap().next(), Some(hold));
    f.take_log();

    f.step(0.5);
    assert_eq!(f.state.track(0), Some(ramp));
    f.step(0.5);
    f.step(0.0);
    assert_eq!(f.state.track(0), Some(hold));
    let log = f.take_log();
    assert!(
        log.contains(&"Start hold".to_string()) && log.contains(&"Interrupt ramp".to_string()),
        "{log:?}"
    );
    approx(f.rotation(), 100.0);
}

#[test]
fn mixing_holds_the_outgoing_value_and_blends() {
    let mut f = fixture(0.0);
    let mut sd = AnimationStateData::new(f.sk.data().clone());
    assert!(sd.set_mix("ramp", "hold", 1.0));
    assert!(!sd.set_mix("ramp", "nope", 1.0));
    f.state = AnimationState::new(Arc::new(sd));

    let ramp = anim(&f, "ramp");
    let hold = anim(&f, "hold");
    f.state.set_animation(0, ramp, false);
    f.step(0.5);
    approx(f.rotation(), 45.0);

    let to = f.state.set_animation(0, hold, false);
    approx(f.state.entry(to).unwrap().mix_duration(), 1.0);
    f.step(0.5);
    // ramp is held at its end (90) while hold (100) mixes in at 50%.
    approx(f.rotation(), 95.0);
    f.step(0.5);
    approx(f.rotation(), 100.0);
    f.step(0.1); // the finished mix ends the outgoing entry
    assert!(f.state.entry(to).unwrap().mixing_from().is_none());
}

#[test]
fn mix_interpolation_shapes_the_blend() {
    let mut f = fixture(1.0);
    let ramp = anim(&f, "ramp");
    let hold = anim(&f, "hold");
    f.state.set_animation(0, ramp, false);
    f.step(1.0); // ramp ends at 90
    let to = f.state.set_animation(0, hold, false);
    f.state.entry_mut(to).unwrap().mix_interpolation = Interpolation::SlowFast;
    f.step(0.5);
    // mix = 0.5^2 = 0.25 -> 90 + (100 - 90) * 0.25
    approx(f.rotation(), 92.5);
}

#[test]
fn empty_animation_mixes_out_to_setup_and_clears_the_track() {
    let mut f = fixture(0.0);
    let ramp = anim(&f, "ramp");
    f.state.set_animation(0, ramp, false);
    f.step(0.75);
    approx(f.rotation(), 67.5);

    let empty = f.state.set_empty_animation(0, 0.5);
    assert!(f.state.entry(empty).unwrap().is_empty_animation());
    f.step(0.25);
    // ramp has reached 90 and is half mixed out toward setup (0).
    approx(f.rotation(), 45.0);
    f.step(0.25);
    approx(f.rotation(), 0.0);
    f.step(0.1);
    f.step(0.1);
    assert!(
        f.state.track(0).is_none(),
        "track clears once the empty entry passes its end"
    );
}

#[test]
fn reverse_plays_backward() {
    let mut f = fixture(0.0);
    let ramp = anim(&f, "ramp");
    let e = f.state.set_animation(0, ramp, false);
    f.state.entry_mut(e).unwrap().reverse = true;
    f.step(0.25);
    approx(f.rotation(), 67.5); // time 0.25 reversed = 0.75 of the ramp
}

#[test]
fn delay_postpones_application() {
    let mut f = fixture(0.0);
    let ramp = anim(&f, "ramp");
    f.state.add_animation(0, ramp, false, 2.0);
    assert!(!f.step(1.0));
    approx(f.rotation(), 0.0);
    assert!(f.step(1.5)); // 0.5s of the delay's leftover plays
    approx(f.rotation(), 45.0);
}

#[test]
fn additive_and_alpha_blend_with_lower_tracks() {
    let mut f = fixture(0.0);
    let hold = anim(&f, "hold");
    let ramp = anim(&f, "ramp");
    f.state.set_animation(0, hold, false); // 100
    let top = f.state.set_animation(1, ramp, false);
    f.state.entry_mut(top).unwrap().alpha = 0.5;
    f.step(1.0);
    // Track 1 blends from the pose track 0 left (100) toward 90 at half alpha.
    approx(f.rotation(), 95.0);

    let mut f = fixture(0.0);
    let base = anim(&f, "hold");
    let add = anim(&f, "ramp");
    f.state.set_animation(0, base, false);
    let top = f.state.set_animation(1, add, false);
    f.state.entry_mut(top).unwrap().additive = true;
    f.step(0.5);
    approx(f.rotation(), 145.0); // 100 + 45
}

#[test]
fn rotation_mix_takes_the_shortest_path() {
    let mut f = fixture(0.0);
    let low = rotate_anim("r350", 0, &[(0.0, 350.0), (1.0, 350.0)]);
    let high = rotate_anim("r10", 0, &[(0.0, 10.0), (1.0, 10.0)]);
    f.state.set_animation(0, low, false);
    let top = f.state.set_animation(1, high, false);
    f.state.entry_mut(top).unwrap().alpha = 0.5;
    f.step(0.5);
    // 350 -> 10 is +20 degrees, not -340: half way is 360, not 180.
    approx(f.rotation(), 360.0);
}

#[test]
fn unkeyed_attachments_return_to_setup_after_mixing_out() {
    let mut f = fixture(1.0);
    let keyb = anim(&f, "keyb");
    let idle = anim(&f, "idle");
    f.state.set_animation(0, keyb, true);
    f.step(0.1);
    assert_eq!(f.attachment(), "b");

    f.state.set_animation(0, idle, true);
    f.step(0.1);
    // keyb mixes out without retaining its attachment, and nothing else keys the slot.
    assert_eq!(f.attachment(), "a");
}

#[test]
fn listeners_can_reenter_the_state() {
    let mut f = fixture(0.0);
    let ramp = anim(&f, "ramp");
    let hold = anim(&f, "hold");
    f.state.set_animation(0, ramp, false);
    let log = f.log.clone();
    f.state.set_listener(move |st, kind, entry, _| {
        if kind == EventType::Complete {
            // Queue another animation from inside the callback.
            let next = st.entry(entry).map(|e| e.animation().clone());
            if next.is_some_and(|a| a.name() == "ramp") {
                st.set_animation(0, hold.clone(), false);
            }
        }
        if kind == EventType::Start {
            let name = st.entry(entry).unwrap().animation().name().to_string();
            log.lock().unwrap().push(format!("start {name}"));
        }
    });
    f.step(1.5);
    f.step(0.1);
    assert!(f.log.lock().unwrap().contains(&"start hold".to_string()));
    approx(f.rotation(), 100.0);
}

#[test]
fn entry_listener_receives_only_its_events() {
    let mut f = fixture(0.0);
    let hits = Arc::new(Mutex::new(Vec::new()));
    let h = hits.clone();
    let e = f.state.set_animation_by_name(0, "events", false).unwrap();
    f.state
        .entry_mut(e)
        .unwrap()
        .set_listener(move |_, kind, _, event| {
            h.lock()
                .unwrap()
                .push((kind, event.map(|e| e.data.name.to_string())));
        });
    f.step(0.6);
    f.step(0.6);
    assert_eq!(
        *hits.lock().unwrap(),
        [
            (EventType::Event, Some("e1".to_string())),
            (EventType::Complete, None)
        ]
    );
}

#[test]
fn clear_tracks_and_manual_disposal() {
    let mut f = fixture(0.0);
    f.state.set_manual_track_entry_disposal(true);
    let e = f.state.set_animation_by_name(0, "ramp", false).unwrap();
    f.state.clear_tracks();
    // With manual disposal the entry survives its dispose event until freed.
    assert!(f.state.entry(e).is_some());
    f.state.dispose_track_entry(e);
    assert!(f.state.entry(e).is_none());
}

#[test]
fn unknown_animation_is_an_error() {
    let mut f = fixture(0.0);
    assert_eq!(
        f.state.set_animation_by_name(0, "nope", false),
        Err(StateError::AnimationNotFound("nope".into()))
    );
    assert!(f
        .state
        .add_animation_by_name(0, "nope", false, 0.0)
        .is_err());
}

#[test]
fn state_data_mix_lookup() {
    let f = fixture(0.0);
    let mut sd = AnimationStateData::new(f.sk.data().clone());
    sd.default_mix = 0.2;
    assert!(sd.set_mix("ramp", "hold", 0.7));
    assert_eq!(sd.mix("ramp", "hold"), 0.7);
    assert_eq!(sd.mix("hold", "ramp"), 0.2);
    sd.clear();
    assert_eq!(sd.mix("ramp", "hold"), 0.0);
}
