//! Benchmark mirroring benches/cpp/bench.cpp exactly.
//!
//! Usage: bench <skeleton.json|.skel> <atlas> <animation> <frames> <loads>
//! Prints: load_us=<per load> ctor_us=<per Skeleton> frame_ns=<per frame> checksum=<value>

use spine2d_core::*;
use std::sync::Arc;
use std::time::Instant;

struct Headless;
impl TextureLoader for Headless {
    fn load(&mut self, page: &mut AtlasPage, _path: &str) {
        page.width = 1024;
        page.height = 1024;
    }
    fn unload(&mut self, _texture: usize) {}
}

fn load(path: &str, atlas: &Atlas) -> SkeletonData {
    let loader = AtlasAttachmentLoader::new(atlas);
    if path.ends_with(".json") {
        SkeletonJson::new(&loader)
            .read_skeleton_data_file(path)
            .unwrap()
    } else {
        SkeletonBinary::new(&loader)
            .read_skeleton_data_file(path)
            .unwrap()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, atlas_path, animation_name) = (&args[1], &args[2], &args[3]);
    let frames: usize = args[4].parse().unwrap();
    let loads: usize = args[5].parse().unwrap();
    set_y_down(false);
    let atlas = Atlas::from_file(atlas_path, Some(&mut Headless)).unwrap();

    let t0 = Instant::now();
    for _ in 0..loads {
        std::hint::black_box(load(path, &atlas));
    }
    let load_us = t0.elapsed().as_secs_f64() * 1e6 / loads as f64;

    let data = Arc::new(load(path, &atlas));
    let animation = data
        .find_animation(animation_name)
        .expect("animation")
        .clone();

    let t0 = Instant::now();
    for _ in 0..loads * 10 {
        std::hint::black_box(Skeleton::new(data.clone()));
    }
    let ctor_us = t0.elapsed().as_secs_f64() * 1e6 / (loads * 10) as f64;

    let mut skeleton = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data.clone())));
    state.set_animation(0, animation, true);
    let dt = 1.0f32 / 60.0;
    let step = |skeleton: &mut Skeleton, state: &mut AnimationState| {
        skeleton.update_time(dt);
        state.update(dt);
        state.apply(skeleton);
        skeleton.update_world_transform(Physics::Update);
    };
    for _ in 0..600 {
        step(&mut skeleton, &mut state);
    }
    let t0 = Instant::now();
    for _ in 0..frames {
        step(&mut skeleton, &mut state);
    }
    let frame_ns = t0.elapsed().as_secs_f64() * 1e9 / frames as f64;

    let checksum: f64 = skeleton
        .bones()
        .iter()
        .map(|b| {
            let p = b.applied_pose();
            p.world_x as f64 + 2.0 * p.world_y as f64 + 0.5 * p.a as f64
        })
        .sum();
    println!(
        "load_us={load_us:.1} ctor_us={ctor_us:.2} frame_ns={frame_ns:.1} checksum={checksum:.4}"
    );
}
