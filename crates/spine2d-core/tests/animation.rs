use spine2d_core::animation::{search, search_step};
use spine2d_core::*;
use std::sync::Arc;

// Spine 4.3 defaults to a y-down world; these tests are written for y-up like the reference's
// headless test.
#[ctor::ctor(unsafe)]
fn init_y_up() {
    spine2d_core::set_y_down(false);
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

fn opts(alpha: f32, from: MixFrom) -> ApplyOptions {
    ApplyOptions {
        alpha,
        from,
        ..Default::default()
    }
}

fn apply(tl: Timeline, sk: &mut Skeleton, time: f32, o: &ApplyOptions) {
    tl.apply(sk, -1.0, time, None, o);
}

/// Bones: root, child. Setup: root rotation 10, x 5, scale 2; child x 1.
fn skeleton() -> Skeleton {
    let mut data = SkeletonData::new();
    let mut root = BoneData::new(0, "root", None);
    root.setup_pose.rotation = 10.0;
    root.setup_pose.x = 5.0;
    root.setup_pose.set_scale(2.0, 2.0);
    let mut child = BoneData::new(1, "child", Some(0));
    child.setup_pose.x = 1.0;
    data.bones = vec![root, child];
    data.slots = vec![
        SlotData::new(0, "s0", BoneId(0)),
        SlotData::new(1, "s1", BoneId(1)),
    ];
    Skeleton::new(Arc::new(data))
}

fn rotate_timeline(frames: &[(f32, f32)]) -> BoneTimeline1 {
    let mut t = BoneTimeline1::new(Bone1Kind::Rotate, frames.len(), 1, 0);
    for (i, &(time, v)) in frames.iter().enumerate() {
        t.set_frame(i, time, v);
        // The last frame stays stepped (the constructor's default), as the reference loaders do.
        if i + 1 < frames.len() {
            t.curve.set_linear(i);
        }
    }
    t
}

#[test]
fn rotate_relative_blends() {
    let tl = Timeline::Bone1(rotate_timeline(&[(0.0, 0.0), (1.0, 90.0)]));
    let rot = |sk: &Skeleton| sk.bone(BoneId(0)).pose.local.rotation;

    // Current: setup(10) + value(45) regardless of current when alpha is 1.
    let mut sk = skeleton();
    sk.bone_mut(BoneId(0)).pose.local.rotation = 99.0;
    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    approx(rot(&sk), 55.0);

    // Half alpha blends from current (10) toward 55.
    let mut sk = skeleton();
    apply(tl.clone(), &mut sk, 0.5, &opts(0.5, MixFrom::Current));
    approx(rot(&sk), 32.5);

    // From setup ignores the current value.
    let mut sk = skeleton();
    sk.bone_mut(BoneId(0)).pose.local.rotation = 99.0;
    apply(tl.clone(), &mut sk, 0.5, &opts(0.5, MixFrom::Setup));
    approx(rot(&sk), 10.0 + 45.0 * 0.5);

    // Additive adds the keyed value to the current value.
    let mut sk = skeleton();
    sk.bone_mut(BoneId(0)).pose.local.rotation = 20.0;
    apply(
        tl,
        &mut sk,
        0.5,
        &ApplyOptions {
            add: true,
            ..opts(1.0, MixFrom::Current)
        },
    );
    approx(rot(&sk), 65.0);
}

#[test]
fn before_first_key_depends_on_from() {
    let tl = Timeline::Bone1(rotate_timeline(&[(0.5, 0.0), (1.0, 90.0)]));
    let run = |from, alpha| {
        let mut sk = skeleton();
        sk.bone_mut(BoneId(0)).pose.local.rotation = 50.0;
        apply(tl.clone(), &mut sk, 0.2, &opts(alpha, from));
        sk.bone(BoneId(0)).pose.local.rotation
    };
    approx(run(MixFrom::Current, 1.0), 50.0);
    approx(run(MixFrom::Setup, 1.0), 10.0);
    approx(run(MixFrom::First, 0.5), 30.0);
}

#[test]
fn stepped_and_bezier_curves() {
    let mut stepped = rotate_timeline(&[(0.0, 0.0), (1.0, 90.0)]);
    stepped.curve.set_stepped(0);
    let mut sk = skeleton();
    apply(
        Timeline::Bone1(stepped),
        &mut sk,
        0.99,
        &opts(1.0, MixFrom::Setup),
    );
    approx(sk.bone(BoneId(0)).pose.local.rotation, 10.0); // still the first key's 0 + setup

    // A bezier with control points on the line y = 90x is exactly linear.
    let mut bez = BoneTimeline1::new(Bone1Kind::Rotate, 2, 1, 0);
    bez.set_frame(0, 0.0, 0.0);
    bez.set_frame(1, 1.0, 90.0);
    bez.curve.set_bezier(
        0,
        0,
        0,
        0.0,
        0.0,
        1.0 / 3.0,
        30.0,
        2.0 / 3.0,
        60.0,
        1.0,
        90.0,
    );
    for t in [0.1, 0.5, 0.77] {
        let mut sk = skeleton();
        apply(
            Timeline::Bone1(bez.clone()),
            &mut sk,
            t,
            &opts(1.0, MixFrom::Setup),
        );
        approx(sk.bone(BoneId(0)).pose.local.rotation, 10.0 + 90.0 * t);
    }

    // An ease-in bezier stays below the line early on.
    let mut ease = BoneTimeline1::new(Bone1Kind::Rotate, 2, 1, 0);
    ease.set_frame(0, 0.0, 0.0);
    ease.set_frame(1, 1.0, 90.0);
    ease.curve
        .set_bezier(0, 0, 0, 0.0, 0.0, 0.5, 0.0, 1.0, 45.0, 1.0, 90.0);
    let mut sk = skeleton();
    apply(
        Timeline::Bone1(ease),
        &mut sk,
        0.25,
        &opts(1.0, MixFrom::Setup),
    );
    assert!(sk.bone(BoneId(0)).pose.local.rotation - 10.0 < 90.0 * 0.25);
}

#[test]
fn translate_pair_and_scale_pair() {
    let mut tr = BoneTimeline2::new(Bone2Kind::Translate, 2, 0, 0);
    tr.set_frame(0, 0.0, 0.0, 0.0);
    tr.set_frame(1, 1.0, 10.0, 20.0);
    tr.curve.set_linear(0);
    let mut sk = skeleton();
    apply(
        Timeline::Bone2(tr),
        &mut sk,
        0.5,
        &opts(1.0, MixFrom::Current),
    );
    let l = sk.bone(BoneId(0)).pose.local;
    approx(l.x, 5.0 + 5.0); // setup x 5 + keyed 5
    approx(l.y, 10.0);

    // Scale keys multiply the setup scale (2).
    let mut sc = BoneTimeline2::new(Bone2Kind::Scale, 2, 0, 0);
    sc.set_frame(0, 0.0, 1.0, 1.0);
    sc.set_frame(1, 1.0, 2.0, 3.0);
    let mut sk = skeleton();
    apply(
        Timeline::Bone2(sc),
        &mut sk,
        1.0,
        &opts(1.0, MixFrom::Setup),
    );
    let l = sk.bone(BoneId(0)).pose.local;
    approx(l.scale_x, 4.0);
    approx(l.scale_y, 6.0);
}

#[test]
fn scale_blend_keeps_sign_when_mixing_in() {
    // Current scale is negative (flipped); keyed positive. Mixing in keeps the current sign...
    let mut sc = BoneTimeline1::new(Bone1Kind::ScaleX, 1, 0, 0);
    sc.set_frame(0, 0.0, 1.0);
    let mut sk = skeleton();
    sk.bone_mut(BoneId(0)).pose.local.scale_x = -2.0;
    apply(
        Timeline::Bone1(sc.clone()),
        &mut sk,
        1.0,
        &opts(0.5, MixFrom::Current),
    );
    // base = |-2| * sign(value=2) = 2; 2 + (2 - 2) * 0.5
    approx(sk.bone(BoneId(0)).pose.local.scale_x, 2.0);

    // ...mixing out keeps magnitude toward the keyed value but the base's sign.
    let mut sk = skeleton();
    sk.bone_mut(BoneId(0)).pose.local.scale_x = -2.0;
    let o = ApplyOptions {
        out: true,
        ..opts(0.5, MixFrom::Current)
    };
    apply(Timeline::Bone1(sc), &mut sk, 1.0, &o);
    // base -2 + (|2| * sign(-2) - -2) * 0.5 = -2
    approx(sk.bone(BoneId(0)).pose.local.scale_x, -2.0);
}

#[test]
fn inactive_bones_are_skipped_and_applied_pose_is_selectable() {
    let mut data = SkeletonData::new();
    let mut b = BoneData::new(0, "root", None);
    b.skin_required = true;
    data.bones = vec![b];
    let mut sk = Skeleton::new(Arc::new(data));
    let tl = Timeline::Bone1(rotate_timeline(&[(0.0, 45.0)]));
    apply(tl, &mut sk, 0.0, &opts(1.0, MixFrom::Setup));
    approx(sk.bone(BoneId(0)).pose.local.rotation, 0.0); // inactive: untouched

    let mut sk = skeleton();
    sk.constrained_bone(BoneId(0)); // applied pose now differs from pose
    let tl = Timeline::Bone1(rotate_timeline(&[(0.0, 45.0)]));
    let o = ApplyOptions {
        applied_pose: true,
        ..opts(1.0, MixFrom::Setup)
    };
    apply(tl, &mut sk, 0.0, &o);
    approx(sk.bone(BoneId(0)).applied_pose().local.rotation, 55.0);
    approx(sk.bone(BoneId(0)).pose.local.rotation, 10.0);
}

#[test]
fn inherit_timeline_sets_mode() {
    let mut t = InheritTimeline::new(2, 1);
    t.set_frame(0, 0.0, Inherit::NoScale);
    t.set_frame(1, 1.0, Inherit::OnlyTranslation);
    let tl = Timeline::Inherit(t);
    let mut sk = skeleton();
    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    assert_eq!(sk.bone(BoneId(1)).pose.local.inherit, Inherit::NoScale);
    apply(tl.clone(), &mut sk, 1.5, &opts(1.0, MixFrom::Current));
    assert_eq!(
        sk.bone(BoneId(1)).pose.local.inherit,
        Inherit::OnlyTranslation
    );
    // Mixing out returns to setup unless from is Current.
    let o = ApplyOptions {
        out: true,
        ..opts(1.0, MixFrom::Setup)
    };
    apply(tl, &mut sk, 1.5, &o);
    assert_eq!(sk.bone(BoneId(1)).pose.local.inherit, Inherit::Normal);
}

#[test]
fn color_timelines() {
    let mut t = ColorTimeline::new(ColorKind::Rgba, 2, 0, 0);
    t.set_frame(0, 0.0, &[0.0, 0.0, 0.0, 0.0]);
    t.set_frame(1, 1.0, &[1.0, 0.5, 0.0, 1.0]);
    t.curve.set_linear(0);
    let tl = Timeline::Color(t);

    let mut sk = skeleton();
    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    let c = sk.slot(SlotId(0)).pose.color;
    assert_eq!((c.r, c.g, c.b, c.a), (0.5, 0.25, 0.0, 0.5));

    // Half alpha from setup (white): setup + (v - setup) * 0.5.
    let mut sk = skeleton();
    apply(tl, &mut sk, 0.5, &opts(0.5, MixFrom::Setup));
    let c = sk.slot(SlotId(0)).pose.color;
    approx(c.r, 0.75);
    approx(c.g, 0.625);
    approx(c.b, 0.5);
    approx(c.a, 0.75);

    // Dark color (rgb2): clamps and writes both colors.
    let mut t = ColorTimeline::new(ColorKind::Rgb2, 1, 0, 1);
    t.set_frame(0, 0.0, &[0.1, 0.2, 0.3, 0.4, 0.5, 2.0]);
    let mut sk = skeleton();
    apply(
        Timeline::Color(t),
        &mut sk,
        0.0,
        &opts(1.0, MixFrom::Current),
    );
    let p = &sk.slot(SlotId(1)).pose;
    assert_eq!((p.color.r, p.color.g, p.color.b), (0.1, 0.2, 0.3));
    assert_eq!(
        (p.dark_color.r, p.dark_color.g, p.dark_color.b),
        (0.4, 0.5, 1.0)
    );

    // Alpha timeline.
    let mut a = AlphaTimeline::new(2, 0, 0);
    a.set_frame(0, 0.0, 0.0);
    a.set_frame(1, 1.0, 1.0);
    a.curve.set_linear(0);
    let mut sk = skeleton();
    apply(
        Timeline::Alpha(a),
        &mut sk,
        0.25,
        &opts(1.0, MixFrom::Current),
    );
    approx(sk.slot(SlotId(0)).pose.color.a, 0.25);
}

fn skeleton_with_attachments() -> (Skeleton, Vec<Arc<Attachment>>) {
    let atts: Vec<Arc<Attachment>> = ["a", "b"]
        .iter()
        .map(|n| Arc::new(Attachment::Point(PointAttachment::new(*n))))
        .collect();
    let mut skin = Skin::new("default");
    skin.set_attachment(0, "a", atts[0].clone());
    skin.set_attachment(0, "b", atts[1].clone());
    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    let mut slot = SlotData::new(0, "s0", BoneId(0));
    slot.attachment_name = "a".into();
    data.slots = vec![slot];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    data.default_skin = Some(skin);
    (Skeleton::new(Arc::new(data)), atts)
}

#[test]
fn attachment_timeline() {
    let (mut sk, atts) = skeleton_with_attachments();
    let mut t = AttachmentTimeline::new(2, 0);
    t.set_frame(0, 0.5, "b");
    t.set_frame(1, 1.0, "");
    let tl = Timeline::Attachment(t);
    let shown = |sk: &Skeleton| sk.slot(SlotId(0)).pose.attachment.clone();

    apply(tl.clone(), &mut sk, 0.7, &opts(1.0, MixFrom::Current));
    assert!(Arc::ptr_eq(&shown(&sk).unwrap(), &atts[1]));
    apply(tl.clone(), &mut sk, 1.2, &opts(1.0, MixFrom::Current));
    assert!(shown(&sk).is_none());

    // Before the first key: nothing for Current, setup attachment otherwise.
    apply(tl.clone(), &mut sk, 0.1, &opts(1.0, MixFrom::Current));
    assert!(shown(&sk).is_none());
    apply(tl.clone(), &mut sk, 0.1, &opts(1.0, MixFrom::Setup));
    assert!(Arc::ptr_eq(&shown(&sk).unwrap(), &atts[0]));
    // Mixing out restores setup the same way.
    apply(tl.clone(), &mut sk, 0.7, &opts(1.0, MixFrom::Current));
    let o = ApplyOptions {
        out: true,
        ..opts(1.0, MixFrom::First)
    };
    apply(tl, &mut sk, 0.7, &o);
    assert!(Arc::ptr_eq(&shown(&sk).unwrap(), &atts[0]));
}

fn mesh_skeleton() -> (Skeleton, Arc<Attachment>) {
    let mut m = MeshAttachment::new("m", Sequence::new(1, false));
    m.vertex.vertices = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
    m.vertex.world_vertices_length = 6;
    let mesh = Arc::new(Attachment::Mesh(m));
    let mut skin = Skin::new("default");
    skin.set_attachment(0, "m", mesh.clone());
    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    let mut slot = SlotData::new(0, "s0", BoneId(0));
    slot.attachment_name = "m".into();
    data.slots = vec![slot];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    data.default_skin = Some(skin);
    (Skeleton::new(Arc::new(data)), mesh)
}

fn deform_timeline(mesh: &Arc<Attachment>) -> DeformTimeline {
    let mut t = DeformTimeline::new(2, 0, 0, mesh.clone());
    t.set_frame(0, 0.0, &[0.0; 6]);
    t.set_frame(1, 1.0, &[2.0; 6]);
    t.curve.set_linear(0);
    t
}

#[test]
fn deform_timeline_interpolates_and_blends() {
    let (mut sk, mesh) = mesh_skeleton();
    let tl = Timeline::Deform(deform_timeline(&mesh));
    let deform = |sk: &Skeleton| sk.slot(SlotId(0)).pose.deform.clone();

    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    assert_eq!(deform(&sk), vec![1.0; 6]);

    // At/after the last key the last frame is used.
    apply(tl.clone(), &mut sk, 5.0, &opts(1.0, MixFrom::Current));
    assert_eq!(deform(&sk), vec![2.0; 6]);

    // alpha 0.5 from current (2) toward the keyed 1.0.
    apply(tl.clone(), &mut sk, 0.5, &opts(0.5, MixFrom::Current));
    assert_eq!(deform(&sk), vec![1.5; 6]);

    // Additive from current adds (keyed - setup vertices) * alpha for unweighted meshes.
    let (mut sk, mesh) = mesh_skeleton();
    sk.slot_mut(SlotId(0)).pose.deform = vec![0.0; 6];
    let tl2 = Timeline::Deform(deform_timeline(&mesh));
    let o = ApplyOptions {
        add: true,
        ..opts(1.0, MixFrom::Current)
    };
    apply(tl2, &mut sk, 5.0, &o);
    // Unweighted add subtracts the setup vertices [0,0,1,0,0,1] from the keyed [2; 6].
    assert_eq!(deform(&sk), vec![2.0, 2.0, 1.0, 2.0, 2.0, 1.0]);
}

#[test]
fn deform_before_first_key() {
    let (mut sk, mesh) = mesh_skeleton();
    let mut t = DeformTimeline::new(2, 0, 0, mesh);
    t.set_frame(0, 0.5, &[2.0; 6]);
    t.set_frame(1, 1.0, &[2.0; 6]);
    t.curve.set_linear(0);
    let tl = Timeline::Deform(t);

    sk.slot_mut(SlotId(0)).pose.deform = vec![4.0; 6];
    apply(tl.clone(), &mut sk, 0.1, &opts(1.0, MixFrom::Current));
    assert_eq!(sk.slot(SlotId(0)).pose.deform, vec![4.0; 6]);
    apply(tl.clone(), &mut sk, 0.1, &opts(1.0, MixFrom::Setup));
    assert!(sk.slot(SlotId(0)).pose.deform.is_empty());

    // From::First blends toward the setup vertices for unweighted meshes.
    sk.slot_mut(SlotId(0)).pose.deform = vec![4.0; 6];
    apply(tl, &mut sk, 0.1, &opts(0.5, MixFrom::First));
    assert_eq!(
        sk.slot(SlotId(0)).pose.deform,
        vec![2.0, 2.0, 2.5, 2.0, 2.0, 2.5]
    );
}

#[test]
fn deform_ignores_unrelated_attachments_and_hits_linked_ones() {
    let (mut sk, mesh) = mesh_skeleton();
    let tl = Timeline::Deform(deform_timeline(&mesh));

    // A linked mesh shows through the same timeline.
    let linked = Arc::new(Attachment::Mesh(
        MeshAttachment::new_linked_mesh(&mesh).unwrap(),
    ));
    sk.slot_mut(SlotId(0)).pose.set_attachment(Some(linked));
    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    assert_eq!(sk.slot(SlotId(0)).pose.deform, vec![1.0; 6]);

    // An unrelated attachment is not deformed (and the timeline is inactive for the slot).
    let other = Arc::new(Attachment::Point(PointAttachment::new("p")));
    sk.slot_mut(SlotId(0)).pose.set_attachment(Some(other));
    apply(tl, &mut sk, 1.0, &opts(1.0, MixFrom::Current));
    assert!(sk.slot(SlotId(0)).pose.deform.is_empty());
}

#[test]
fn sequence_timeline_modes() {
    let mut region = RegionAttachment::new("r", Sequence::new(3, false));
    region.width = 1.0;
    region.height = 1.0;
    region.update_sequence();
    let att = Arc::new(Attachment::Region(region));
    let mut skin = Skin::new("default");
    skin.set_attachment(0, "r", att.clone());
    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    let mut slot = SlotData::new(0, "s0", BoneId(0));
    slot.attachment_name = "r".into();
    data.slots = vec![slot];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    data.default_skin = Some(skin);
    let mut sk = Skeleton::new(Arc::new(data));

    let run = |mode: SequenceMode, start: i32, time: f32| {
        let mut t = SequenceTimeline::new(1, 0, att.clone());
        t.set_frame(0, 0.0, mode, start, 0.1);
        let mut sk = sk.clone();
        apply(
            Timeline::Sequence(t),
            &mut sk,
            time,
            &opts(1.0, MixFrom::Current),
        );
        sk.slot(SlotId(0)).pose.sequence_index
    };
    assert_eq!(run(SequenceMode::Hold, 1, 5.0), 1);
    assert_eq!(run(SequenceMode::Once, 0, 0.15), 1);
    assert_eq!(run(SequenceMode::Once, 0, 5.0), 2); // clamps to the last frame
    assert_eq!(run(SequenceMode::Loop, 0, 0.25), 2);
    assert_eq!(run(SequenceMode::Loop, 0, 0.35), 0);
    assert_eq!(run(SequenceMode::Pingpong, 0, 0.35), 1); // 0 1 2 1 0 1 2 ...
    assert_eq!(run(SequenceMode::OnceReverse, 0, 0.05), 2);
    assert_eq!(run(SequenceMode::OnceReverse, 0, 5.0), 0);
    assert_eq!(run(SequenceMode::LoopReverse, 0, 0.15), 1);

    // Before the first key: reset to -1 unless from is Current.
    let mut t = SequenceTimeline::new(1, 0, att);
    t.set_frame(0, 0.5, SequenceMode::Loop, 0, 0.1);
    sk.slot_mut(SlotId(0)).pose.sequence_index = 2;
    apply(
        Timeline::Sequence(t.clone()),
        &mut sk,
        0.1,
        &opts(1.0, MixFrom::Current),
    );
    assert_eq!(sk.slot(SlotId(0)).pose.sequence_index, 2);
    apply(
        Timeline::Sequence(t),
        &mut sk,
        0.1,
        &opts(1.0, MixFrom::Setup),
    );
    assert_eq!(sk.slot(SlotId(0)).pose.sequence_index, -1);
}

fn skeleton_with_slots(n: usize) -> Skeleton {
    let mut data = SkeletonData::new();
    data.bones = vec![BoneData::new(0, "root", None)];
    data.slots = (0..n)
        .map(|i| SlotData::new(i, format!("s{i}"), BoneId(0)))
        .collect();
    Skeleton::new(Arc::new(data))
}

fn order(sk: &Skeleton, applied: bool) -> Vec<usize> {
    let o = sk.draw_order();
    (if applied { o.applied_pose() } else { &o.pose })
        .iter()
        .map(|s| s.0)
        .collect()
}

#[test]
fn draw_order_timeline() {
    let mut sk = skeleton_with_slots(3);
    let mut t = DrawOrderTimeline::new(2);
    t.set_frame(0, 0.0, Some(&[2, 0, 1]));
    t.set_frame(1, 1.0, None);
    let tl = Timeline::DrawOrder(t);
    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    assert_eq!(order(&sk, false), [2, 0, 1]);
    apply(tl.clone(), &mut sk, 1.5, &opts(1.0, MixFrom::Current));
    assert_eq!(order(&sk, false), [0, 1, 2]); // empty = setup order

    apply(tl.clone(), &mut sk, 0.5, &opts(1.0, MixFrom::Current));
    let o = ApplyOptions {
        out: true,
        ..opts(1.0, MixFrom::Current)
    };
    apply(tl.clone(), &mut sk, 0.5, &o);
    assert_eq!(order(&sk, false), [2, 0, 1]); // out with Current leaves it
    let o = ApplyOptions {
        out: true,
        ..opts(1.0, MixFrom::Setup)
    };
    apply(tl, &mut sk, 0.5, &o);
    assert_eq!(order(&sk, false), [0, 1, 2]);
}

#[test]
fn draw_order_folder_only_moves_folder_slots() {
    let mut sk = skeleton_with_slots(5);
    // Pose starts as the setup order [0,1,2,3,4]; folder = slots 1 and 3.
    let mut t = DrawOrderFolderTimeline::new(1, vec![1, 3], 5);
    t.set_frame(0, 0.0, Some(&[1, 0]));
    let tl = Timeline::DrawOrderFolder(t);
    apply(tl.clone(), &mut sk, 0.0, &opts(1.0, MixFrom::Current));
    assert_eq!(order(&sk, false), [0, 3, 2, 1, 4]);
    let o = ApplyOptions {
        out: true,
        ..opts(1.0, MixFrom::Setup)
    };
    apply(tl, &mut sk, 0.0, &o);
    assert_eq!(order(&sk, false), [0, 1, 2, 3, 4]);
}

fn event(name: &str, time: f32) -> Arc<spine2d_core::Event> {
    Arc::new(spine2d_core::Event::new(
        time,
        Arc::new(EventData::new(name)),
    ))
}

fn event_timeline() -> EventTimeline {
    let mut t = EventTimeline::new(4);
    for (i, (n, time)) in [("a", 0.5), ("b", 1.0), ("c", 1.0), ("d", 2.0)]
        .into_iter()
        .enumerate()
    {
        t.set_frame(i, event(n, time));
    }
    t
}

fn fired(t: &EventTimeline, last: f32, time: f32) -> Vec<String> {
    let mut out = Vec::new();
    Timeline::Event(t.clone()).apply(
        &mut skeleton(),
        last,
        time,
        Some(&mut out),
        &ApplyOptions::default(),
    );
    out.iter().map(|e| e.data.name.to_string()).collect()
}

#[test]
fn event_timeline_fires_in_range() {
    let t = event_timeline();
    assert_eq!(fired(&t, -1.0, 0.7), ["a"]);
    assert_eq!(fired(&t, 0.7, 1.0), ["b", "c"]); // same-time events fire together
    assert_eq!(fired(&t, 0.5, 1.0), ["b", "c"]); // last_time is exclusive
    assert_eq!(fired(&t, 1.0, 2.0), ["d"]);
    assert_eq!(fired(&t, 2.0, 3.0), Vec::<String>::new());
    assert_eq!(fired(&t, -1.0, 0.1), Vec::<String>::new());
    assert_eq!(fired(&t, -1.0, 9.0), ["a", "b", "c", "d"]);
    // No event list: nothing happens and nothing panics.
    Timeline::Event(t).apply(&mut skeleton(), -1.0, 9.0, None, &ApplyOptions::default());
}

#[test]
fn animation_loops_and_fires_across_the_wrap() {
    let t = event_timeline();
    let anim = Animation::new("loop", vec![Timeline::Event(t)], 2.0);
    let mut sk = skeleton();
    let mut events = Vec::new();
    // 1.9 -> 2.3 wraps to 1.9 -> 0.3: fires "d" at 2.0, then restarts and fires nothing before 0.5.
    anim.apply(
        &mut sk,
        1.9,
        2.3,
        true,
        Some(&mut events),
        &ApplyOptions::default(),
    );
    assert_eq!(
        events
            .iter()
            .map(|e| e.data.name.as_str())
            .collect::<Vec<_>>(),
        ["d"]
    );

    events.clear();
    anim.apply(
        &mut sk,
        1.9,
        2.7,
        true,
        Some(&mut events),
        &ApplyOptions::default(),
    );
    assert_eq!(
        events
            .iter()
            .map(|e| e.data.name.as_str())
            .collect::<Vec<_>>(),
        ["d", "a"]
    );
}

#[test]
fn animation_tracks_bones_and_property_ids() {
    let rot = rotate_timeline(&[(0.0, 0.0), (2.0, 10.0)]);
    let mut tr = BoneTimeline2::new(Bone2Kind::Translate, 1, 0, 1);
    tr.set_frame(0, 0.0, 0.0, 0.0);
    let anim = Animation::new("a", vec![Timeline::Bone1(rot), Timeline::Bone2(tr)], 2.0);
    assert_eq!(anim.bones(), [0, 1]);
    assert!(anim.has_timeline(&[Property::Rotate.id(0)]));
    assert!(anim.has_timeline(&[Property::Y.id(1)]));
    assert!(!anim.has_timeline(&[Property::Rotate.id(1), Property::ScaleX.id(0)]));

    let t = Timeline::Bone1(rotate_timeline(&[(0.0, 0.0), (2.0, 10.0)]));
    assert_eq!(
        (t.frame_count(), t.frame_entries(), t.duration()),
        (2, 2, 2.0)
    );
    assert!(t.additive() && !t.instant());
    assert!(Timeline::Event(EventTimeline::new(1)).instant());
}

#[test]
fn animation_apply_wraps_time_when_looping() {
    let anim = Animation::new(
        "a",
        vec![Timeline::Bone1(rotate_timeline(&[
            (0.0, 0.0),
            (1.0, 100.0),
        ]))],
        1.0,
    );
    let mut sk = skeleton();
    anim.apply(&mut sk, 0.0, 2.25, true, None, &opts(1.0, MixFrom::Setup));
    approx(sk.bone(BoneId(0)).pose.local.rotation, 10.0 + 25.0);
    anim.apply(&mut sk, 0.0, 2.25, false, None, &opts(1.0, MixFrom::Setup));
    approx(sk.bone(BoneId(0)).pose.local.rotation, 10.0 + 100.0); // clamps to the last key
}

#[test]
fn search_finds_frame_at_or_before_target() {
    let frames = [0.0, 1.0, 2.0, 3.0];
    assert_eq!(search(&frames, 0.0), 0);
    assert_eq!(search(&frames, 1.5), 1);
    assert_eq!(search(&frames, 3.0), 3);
    assert_eq!(search(&frames, 99.0), 3);
    assert_eq!(search_step(&[0.0, 9.0, 1.0, 9.0], 1.0, 2), 2);
}
