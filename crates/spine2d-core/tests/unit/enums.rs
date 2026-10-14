use spine2d_core::enums::{BlendMode, Inherit};

#[test]
fn unknown_names_default() {
    assert_eq!(Inherit::from_name("bogus"), Inherit::Normal);
    assert_eq!(Inherit::from_name("noScale"), Inherit::NoScale);
    assert_eq!(BlendMode::from_name("screen"), BlendMode::Screen);
    assert_eq!(BlendMode::from_name(""), BlendMode::Normal);
}
