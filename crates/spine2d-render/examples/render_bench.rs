//! Render benchmark mirroring benches/cpp/bench.cpp exactly.
//!
//! Usage: render_bench <skeleton.json|.skel> <atlas> <animation> <frames>
//! Prints: render_ns=<per frame> bounds_ns=<per frame> hit_ns=<per frame> checksum=<value>

use spine2d_core::{
    AnimationState, AnimationStateData, Atlas, AtlasAttachmentLoader, AtlasPage, Physics, Skeleton,
    SkeletonBinary, SkeletonData, SkeletonJson, TextureLoader,
};
use spine2d_render::{compute_bounds, SkeletonBounds, SkeletonClipping, SkeletonRenderer};
use std::sync::Arc;
use std::time::Instant;

struct Headless;

impl TextureLoader for Headless {
    fn load(&mut self, page: &mut AtlasPage, _path: &str) {
        page.width = 1024;
        page.height = 1024;
        page.texture = page.index + 1;
    }

    fn unload(&mut self, _texture: usize) {}
}

fn load(path: &str, atlas: &Atlas) -> SkeletonData {
    let loader = AtlasAttachmentLoader::new(atlas);
    if path.ends_with(".json") {
        SkeletonJson::new(&loader)
            .read_skeleton_data_file(path)
            .expect("load json")
    } else {
        SkeletonBinary::new(&loader)
            .read_skeleton_data_file(path)
            .expect("load binary")
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, atlas_path, animation_name) = (&args[1], &args[2], &args[3]);
    let frames: usize = args[4].parse().expect("frame count");
    spine2d_core::set_y_down(false);
    let atlas = Atlas::from_file(atlas_path, Some(&mut Headless)).expect("atlas");
    let data = Arc::new(load(path, &atlas));
    let animation = data
        .find_animation(animation_name)
        .expect("animation")
        .clone();

    let mut skeleton = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data)));
    state.set_animation(0, animation, true);
    let (mut renderer, mut clipper, mut bounds) = (
        SkeletonRenderer::new(),
        SkeletonClipping::new(),
        SkeletonBounds::new(),
    );
    let mut scratch = Vec::new();
    let dt = 1.0f32 / 60.0;
    let (mut render_ns, mut bounds_ns, mut hit_ns, mut checksum) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);

    // The first 600 frames warm up and are not timed.
    for i in -600..frames as i32 {
        skeleton.update_time(dt);
        state.update(dt);
        state.apply(&mut skeleton);
        skeleton.update_world_transform(Physics::Update);

        let t0 = Instant::now();
        let commands = renderer.render(&skeleton);
        let t1 = Instant::now();
        let area = compute_bounds(&skeleton, &mut scratch, Some(&mut clipper));
        let t2 = Instant::now();
        bounds.update(&skeleton, true);
        let t3 = Instant::now();
        if i < 0 {
            continue;
        }
        render_ns += (t1 - t0).as_secs_f64() * 1e9;
        bounds_ns += (t2 - t1).as_secs_f64() * 1e9;
        hit_ns += (t3 - t2).as_secs_f64() * 1e9;
        for command in commands {
            checksum += (command.vertex_count() + command.indices.len()) as f64;
            for &p in &command.positions {
                checksum += f64::from(p) * 0.001;
            }
        }
        checksum += f64::from(area.x + area.y + area.width + area.height);
    }
    let n = frames as f64;
    println!(
        "render_ns={:.1} bounds_ns={:.1} hit_ns={:.1} checksum={checksum:.3}",
        render_ns / n,
        bounds_ns / n,
        hit_ns / n
    );
}
