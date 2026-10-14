//! Separate binary: `set_y_down` is process-wide, so it must not share a process with other tests.
use spine2d_core::*;
use std::sync::Arc;

#[test]
fn y_down_flips_skeleton_scale_y() {
    let mut data = SkeletonData::new();
    let mut root = BoneData::new(0, "root", None);
    root.setup_pose.y = 2.0;
    data.bones = vec![root];
    let mut sk = Skeleton::new(Arc::new(data));

    // Spine 4.3's default, matching the reference.
    assert!(is_y_down());
    set_y_down(true);
    sk.update_world_transform(Physics::None);
    assert!((sk.bone(BoneId(0)).applied_pose().world_y + 2.0).abs() < 1e-5);
    assert_eq!(sk.scale_y(), -1.0);
}
