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

fn two_bone(root: BoneLocal, child: BoneLocal) -> Skeleton {
    let mut data = SkeletonData::new();
    let mut r = BoneData::new(0, "root", None);
    r.setup_pose = root;
    let mut c = BoneData::new(1, "child", Some(0));
    c.setup_pose = child;
    data.bones = vec![r, c];
    data.slots = vec![SlotData::new(0, "slot", BoneId(1))];
    Skeleton::new(Arc::new(data))
}

#[test]
fn child_translation_follows_parent_rotation() {
    let root = BoneLocal {
        rotation: 90.0,
        ..Default::default()
    };
    let child = BoneLocal {
        x: 10.0,
        ..Default::default()
    };
    let mut sk = two_bone(root, child);
    sk.update_world_transform(Physics::None);
    let c = sk.bone(BoneId(1)).applied_pose();
    approx(c.world_x, 0.0);
    approx(c.world_y, 10.0);
    approx(c.world_rotation_x(), 90.0);
}

#[test]
fn skeleton_position_and_scale_apply_to_root() {
    let root = BoneLocal {
        x: 1.0,
        y: 2.0,
        ..Default::default()
    };
    let mut sk = two_bone(root, BoneLocal::default());
    sk.set_position(100.0, 50.0);
    sk.set_scale(2.0, 3.0);
    sk.update_world_transform(Physics::None);
    let r = sk.bone(BoneId(0)).applied_pose();
    approx(r.world_x, 102.0);
    approx(r.world_y, 56.0);
    approx(r.world_scale_x(), 2.0);
    approx(r.world_scale_y(), 3.0); // Y axis column is (cos 90 * sx, sin 90 * sy)
}

#[test]
fn inherit_modes_ignore_what_they_should() {
    let root = BoneLocal {
        rotation: 45.0,
        scale_x: 2.0,
        scale_y: 2.0,
        ..Default::default()
    };

    let mut child = BoneLocal {
        rotation: 10.0,
        ..Default::default()
    };
    child.inherit = Inherit::OnlyTranslation;
    let mut sk = two_bone(root, child);
    sk.update_world_transform(Physics::None);
    let c = sk.bone(BoneId(1)).applied_pose();
    approx(c.world_rotation_x(), 10.0);
    approx(c.world_scale_x(), 1.0);

    child.inherit = Inherit::NoRotationOrReflection;
    let mut sk = two_bone(root, child);
    sk.update_world_transform(Physics::None);
    let c = sk.bone(BoneId(1)).applied_pose();
    approx(c.world_rotation_x(), 10.0);
    approx(c.world_scale_x(), 2.0); // scale is still inherited

    child.inherit = Inherit::NoScale;
    let mut sk = two_bone(root, child);
    sk.update_world_transform(Physics::None);
    let c = sk.bone(BoneId(1)).applied_pose();
    approx(c.world_rotation_x(), 55.0); // rotation is inherited
    approx(c.world_scale_x(), 1.0);
}

#[test]
fn world_to_local_round_trips_for_every_inherit_mode() {
    let modes = [
        Inherit::Normal,
        Inherit::OnlyTranslation,
        Inherit::NoRotationOrReflection,
        Inherit::NoScale,
        Inherit::NoScaleOrReflection,
    ];
    for mode in modes {
        for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.5, -0.75)] {
            let root = BoneLocal {
                x: 3.0,
                y: -2.0,
                rotation: 33.0,
                scale_x: 1.5,
                scale_y: 0.8,
                shear_x: 5.0,
                ..Default::default()
            };
            let child = BoneLocal {
                x: 7.0,
                y: 4.0,
                rotation: -20.0,
                scale_x: 0.9,
                scale_y: 1.2,
                shear_y: 8.0,
                inherit: mode,
                ..Default::default()
            };
            let mut sk = two_bone(root, child);
            sk.set_scale(sx, sy);
            sk.set_position(5.0, 6.0);
            sk.update_world_transform(Physics::None);

            let before = *sk.bone(BoneId(1)).applied_pose();
            // Recompute local from world, then world from that local: must reproduce the world.
            sk.update_bone_local_transform(BoneId(1));
            sk.update_bone_world_transform(BoneId(1));
            let after = *sk.bone(BoneId(1)).applied_pose();
            for (name, x, y) in [
                ("a", before.a, after.a),
                ("b", before.b, after.b),
                ("c", before.c, after.c),
                ("d", before.d, after.d),
                ("wx", before.world_x, after.world_x),
                ("wy", before.world_y, after.world_y),
            ] {
                assert!(
                    (x - y).abs() < 1e-3,
                    "{mode:?} scale=({sx},{sy}) {name}: {x} != {y}"
                );
            }
        }
    }
}

#[test]
fn local_matches_original_for_normal_inherit() {
    let root = BoneLocal {
        rotation: 30.0,
        scale_x: 2.0,
        ..Default::default()
    };
    let child = BoneLocal {
        x: 4.0,
        y: 1.0,
        rotation: 15.0,
        scale_x: 1.5,
        scale_y: 0.5,
        shear_y: 10.0,
        ..Default::default()
    };
    let mut sk = two_bone(root, child);
    sk.update_world_transform(Physics::None);
    sk.update_bone_local_transform(BoneId(1));
    let l = sk.bone(BoneId(1)).applied_pose().local;
    approx(l.x, 4.0);
    approx(l.y, 1.0);
    approx(l.scale_x, 1.5);
    approx(l.scale_y, 0.5);
    approx(l.rotation, 15.0);
    approx(l.shear_y, 10.0);
}

#[test]
fn update_cache_orders_parents_first_and_skips_inactive() {
    let mut data = SkeletonData::new();
    // Defined child-first in index terms isn't possible (parents precede children), so check a chain
    // plus a skin-required bone that stays inactive without a skin.
    data.bones = vec![
        BoneData::new(0, "root", None),
        BoneData::new(1, "a", Some(0)),
        BoneData::new(2, "b", Some(1)),
        {
            let mut b = BoneData::new(3, "skinned", Some(0));
            b.skin_required = true;
            b
        },
    ];
    let sk = Skeleton::new(Arc::new(data));
    assert_eq!(
        sk.update_cache_entries(),
        &[
            skeleton::Update::Bone(BoneId(0)),
            skeleton::Update::Bone(BoneId(1)),
            skeleton::Update::Bone(BoneId(2))
        ]
    );
    assert!(!sk.bone(BoneId(3)).is_active());
}

#[test]
fn modify_world_marks_descendants_for_recompute() {
    let mut sk = two_bone(
        BoneLocal::default(),
        BoneLocal {
            x: 10.0,
            ..Default::default()
        },
    );
    sk.update_world_transform(Physics::None);
    // Move the root in world space, then let descendants recompute.
    sk.bone_mut(BoneId(0)).applied_pose_mut().world_x = 5.0;
    sk.modify_bone_world(BoneId(0));
    sk.update_bone_world_transform(BoneId(1));
    approx(sk.bone(BoneId(1)).applied_pose().world_x, 15.0);
}

#[test]
fn affine_conversion_matches_matrix() {
    let mut sk = two_bone(
        BoneLocal {
            rotation: 90.0,
            x: 1.0,
            y: 2.0,
            ..Default::default()
        },
        BoneLocal::default(),
    );
    sk.update_world_transform(Physics::None);
    let m = sk
        .bone(BoneId(0))
        .applied_pose()
        .world_matrix()
        .to_affine2();
    let p = m.transform_point2(glam::Vec2::new(1.0, 0.0));
    approx(p.x, 1.0);
    approx(p.y, 3.0);
}
