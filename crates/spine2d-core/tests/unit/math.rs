use spine2d_core::math::{clamp, fmod, sign};

#[test]
fn fmod_keeps_sign_of_dividend() {
    assert_eq!(fmod(-5.0, 3.0), -2.0);
    assert_eq!(fmod(5.0, 3.0), 2.0);
}

#[test]
fn clamp_and_sign() {
    assert_eq!(clamp(2.0, 0.0, 1.0), 1.0);
    assert_eq!(clamp(-2.0, 0.0, 1.0), 0.0);
    assert_eq!(sign(-3.0), -1.0);
    assert_eq!(sign(0.0), 0.0);
}
