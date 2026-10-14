use spine2d_core::color::Color;

#[test]
fn clamps_on_construction() {
    let c = Color::new(2.0, -1.0, 0.5, 1.0);
    assert_eq!((c.r, c.g, c.b), (1.0, 0.0, 0.5));
}

#[test]
fn parses_hex() {
    let c = Color::from_hex("ff000080").unwrap();
    assert_eq!(c.r, 1.0);
    assert!((c.a - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(Color::from_hex("00ff00").unwrap().a, 1.0);
    assert!(Color::from_hex("abc").is_none());
}
