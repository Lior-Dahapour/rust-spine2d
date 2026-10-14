//! Self-contained checks of constraint behavior (no reference binary needed).

use spine2d_core::*;
use std::sync::Arc;

#[ctor::ctor(unsafe)]
fn init_y_up() {
    spine2d_core::set_y_down(false);
}

fn approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "{a} != {b}");
}

fn bone(index: usize, name: &str, parent: Option<usize>, x: f32, y: f32, length: f32) -> BoneData {
    let mut b = BoneData::new(index, name, parent);
    b.setup_pose.x = x;
    b.setup_pose.y = y;
    b.length = length;
    b
}

/// root, upper arm (length 100), forearm (length 100) and an IK target.
fn ik_skeleton(target: (f32, f32), mix: f32) -> Skeleton {
    let mut data = SkeletonData::new();
    data.bones = vec![
        bone(0, "root", None, 0.0, 0.0, 0.0),
        bone(1, "upper", Some(0), 0.0, 0.0, 100.0),
        bone(2, "fore", Some(1), 100.0, 0.0, 100.0),
        bone(3, "target", Some(0), target.0, target.1, 0.0),
    ];
    let mut ik = IkConstraintData::new("ik");
    ik.bones = vec![1, 2];
    ik.target = 3;
    ik.setup_pose = IkConstraintPose {
        mix,
        softness: 0.0,
        bend_direction: 1,
        compress: false,
        stretch: false,
    };
    data.constraints = vec![ConstraintData::Ik(ik)];
    Skeleton::new(Arc::new(data))
}

fn tip(sk: &Skeleton) -> (f32, f32) {
    let fore = sk.bone(BoneId(2)).applied_pose();
    fore.local_to_world(100.0, 0.0)
}

#[test]
fn two_bone_ik_reaches_a_reachable_target() {
    for target in [(120.0, 80.0), (-90.0, 60.0), (30.0, -150.0), (199.0, 0.0)] {
        let mut sk = ik_skeleton(target, 1.0);
        sk.update_world_transform(Physics::None);
        let (x, y) = tip(&sk);
        approx(x, target.0, 0.05);
        approx(y, target.1, 0.05);
    }
}

#[test]
fn ik_with_zero_mix_leaves_the_pose_alone() {
    let mut sk = ik_skeleton((120.0, 80.0), 0.0);
    sk.update_world_transform(Physics::None);
    let (x, y) = tip(&sk);
    approx(x, 200.0, 1e-3);
    approx(y, 0.0, 1e-3);
    // The pose can be changed at runtime through the constraint.
    if let ConstraintBody::Ik(ik) = &mut sk.constraint_mut(0).body {
        ik.pose.pose.mix = 1.0;
    }
    sk.update_world_transform(Physics::None);
    let (x, y) = tip(&sk);
    approx(x, 120.0, 0.05);
    approx(y, 80.0, 0.05);
}

#[test]
fn unreachable_target_straightens_the_chain_toward_it() {
    let mut sk = ik_skeleton((400.0, 0.0), 1.0);
    sk.update_world_transform(Physics::None);
    let (x, y) = tip(&sk);
    approx(x, 200.0, 0.05);
    approx(y, 0.0, 0.05);
}

#[test]
fn transform_constraint_copies_world_rotation() {
    let mut data = SkeletonData::new();
    let mut source = bone(1, "source", Some(0), 10.0, 0.0, 0.0);
    source.setup_pose.rotation = 40.0;
    data.bones = vec![
        bone(0, "root", None, 0.0, 0.0, 0.0),
        source,
        bone(2, "follower", Some(0), 0.0, 50.0, 0.0),
    ];
    let mut tc = TransformConstraintData::new("copy");
    tc.bones = vec![2];
    tc.source = 1;
    tc.setup_pose.mix_rotate = 1.0;
    let mut from = FromProperty::new(BoneProperty::Rotate);
    let mut to = ToProperty::new(BoneProperty::Rotate);
    to.max = 1.0;
    from.to.push(to);
    tc.properties.push(from);
    data.constraints = vec![ConstraintData::Transform(tc)];
    let mut sk = Skeleton::new(Arc::new(data));
    sk.update_world_transform(Physics::None);
    approx(
        sk.bone(BoneId(2)).applied_pose().world_rotation_x(),
        40.0,
        1e-3,
    );
    // The unconstrained pose is untouched.
    approx(sk.bone(BoneId(2)).pose.local.rotation, 0.0, 0.0);
    assert!(!sk.bone(BoneId(2)).is_pose_equal_to_applied());
}

#[test]
fn skin_required_constraints_follow_the_skin() {
    let mut sk = ik_skeleton((120.0, 80.0), 1.0);
    let mut data = (**sk.data()).clone();
    if let ConstraintData::Ik(ik) = &mut data.constraints[0] {
        ik.skin_required = true;
    }
    let mut skin = Skin::new("with-ik");
    skin.constraints = vec![0];
    let skin = Arc::new(skin);
    data.skins.push(skin.clone());
    sk = Skeleton::new(Arc::new(data));

    assert!(!sk.constraint(0).is_active());
    sk.update_world_transform(Physics::None);
    approx(tip(&sk).0, 200.0, 1e-3); // inactive: chain stays straight

    sk.set_skin(Some(skin));
    assert!(sk.constraint(0).is_active());
    sk.update_world_transform(Physics::None);
    approx(tip(&sk).0, 120.0, 0.05);

    sk.set_skin(None);
    assert!(!sk.constraint(0).is_active());
}

#[test]
fn constraints_are_found_by_name_and_reset_with_setup_pose() {
    let mut sk = ik_skeleton((120.0, 80.0), 1.0);
    assert_eq!(sk.find_constraint("ik"), Some(0));
    assert_eq!(sk.find_constraint("nope"), None);
    if let ConstraintBody::Ik(ik) = &mut sk.constraint_mut(0).body {
        ik.pose.pose.mix = 0.25;
    }
    sk.setup_pose();
    match &sk.constraint(0).body {
        ConstraintBody::Ik(ik) => assert_eq!(ik.pose.pose.mix, 1.0),
        _ => unreachable!(),
    }
}

#[test]
fn physics_translate_does_not_panic_without_physics_constraints() {
    let mut sk = ik_skeleton((1.0, 1.0), 1.0);
    sk.physics_translate(10.0, 5.0);
    sk.physics_rotate(0.0, 0.0, 30.0);
    sk.update_world_transform(Physics::Update);
}
