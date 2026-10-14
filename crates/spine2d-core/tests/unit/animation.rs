use spine2d_core::animation::{search_step, Property};

fn linear_search(frames: &[f32], target: f32, step: usize) -> usize {
    let mut i = step;
    while i < frames.len() {
        if frames[i] > target {
            return i - step;
        }
        i += step;
    }
    frames.len() - step
}

#[test]
fn binary_search_matches_reference_linear_scan() {
    for step in [1usize, 2, 3, 8] {
        for count in [1usize, 2, 7, 8, 9, 20, 64] {
            // Includes duplicate frame times.
            let frames: Vec<f32> = (0..count * step)
                .map(|i| {
                    if i % step == 0 {
                        (i / step / 2) as f32
                    } else {
                        -1.0
                    }
                })
                .collect();
            for k in 0..(count + 2) * 4 {
                let target = k as f32 * 0.25 - 0.5;
                if target < frames[0] {
                    continue;
                }
                assert_eq!(
                    search_step(&frames, target, step),
                    linear_search(&frames, target, step),
                    "step={step} n={count} t={target}"
                );
            }
        }
    }
}

#[test]
fn property_ids_pack_property_and_target() {
    assert_eq!(Property::Rotate.id(5), 5);
    assert_eq!(Property::X.id(5), (1u64 << 32) | 5);
}
