use spine2d_core::animation::Interpolation;

#[test]
fn curves_hit_the_endpoints_and_midpoint() {
    for i in [
        Interpolation::Linear,
        Interpolation::Smooth,
        Interpolation::SlowFast,
        Interpolation::FastSlow,
        Interpolation::Circle,
    ] {
        assert!((i.apply(0.0)).abs() < 1e-6, "{i:?}");
        assert!((i.apply(1.0) - 1.0).abs() < 1e-6, "{i:?}");
    }
    assert_eq!(Interpolation::Smooth.apply(0.5), 0.5);
    assert!((Interpolation::Circle.apply(0.5) - 0.5).abs() < 1e-6);
    assert_eq!(Interpolation::SlowFast.apply(0.5), 0.25);
    assert_eq!(Interpolation::FastSlow.apply(0.5), 0.75);
    assert_eq!(Interpolation::Linear.apply_between(10.0, 20.0, 0.25), 12.5);
    assert_eq!(Interpolation::Custom(|a| a * 2.0).apply(0.25), 0.5);
}
