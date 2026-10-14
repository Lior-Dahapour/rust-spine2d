use spine2d_core::sequence::Sequence;
use spine2d_core::slot::SlotPose;

#[test]
fn path_pads_frame_numbers() {
    let mut s = Sequence::new(3, true);
    s.start = 1;
    s.digits = 3;
    assert_eq!(s.path("walk", 0), "walk001");
    assert_eq!(s.path("walk", 11), "walk012");
    s.digits = 1;
    assert_eq!(s.path("walk", 11), "walk12");
    assert_eq!(Sequence::new(1, false).path("walk", 5), "walk");
}

#[test]
fn resolve_index_uses_setup_and_clamps() {
    let mut s = Sequence::new(3, false);
    s.setup_index = 1;
    let mut pose = SlotPose {
        sequence_index: -1,
        ..SlotPose::default()
    };
    assert_eq!(s.resolve_index(&pose), 1);
    pose.sequence_index = 9;
    assert_eq!(s.resolve_index(&pose), 2);
    pose.sequence_index = 0;
    assert_eq!(s.resolve_index(&pose), 0);
}

#[test]
fn clone_gets_new_id() {
    let s = Sequence::new(1, false);
    assert_ne!(s.id(), s.clone().id());
}
