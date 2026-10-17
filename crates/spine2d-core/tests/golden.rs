//! Compares this port against the C++ reference runtime, frame by frame.
//!
//! Needs a spine-runtimes checkout (`SPINE_RUNTIMES_DIR`) and the reference headless test built
//! from `spine-cpp/tests/HeadlessTest.cpp` (`SPINE_CPP_HEADLESS` = path to the binary), eg:
//!
//! ```text
//! clang++ -std=c++11 -O1 -w -ffp-contract=off -I$SRC/include -o headless $SRC/src/spine/*.cpp $SRC/tests/HeadlessTest.cpp
//! SPINE_RUNTIMES_DIR=... SPINE_CPP_HEADLESS=./headless cargo test --test golden
//! ```
//!
//! Build the reference with `-ffp-contract=off`: on ARM64 clang otherwise fuses `a * b + c` into
//! fused multiply-adds, which Rust never does, and results then differ in the last bit.
//!
//! By default a handful of skeletons are compared; set `GOLDEN_ALL=1` for every animation of
//! every example.
//!
//! Every bone's local pose and applied world transform, and every slot's pose, is compared.

mod common;
use common::*;
use serde_json::Value;
use spine2d_core::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Replays the reference test's single-animation sampling.
fn run_rust(data: &Arc<SkeletonData>, animation: &str) -> Vec<Frame> {
    let anim = data.find_animation(animation).unwrap().clone();
    let mut sk = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data.clone())));

    state.set_animation(0, anim.clone(), true);
    state.update(0.016);
    state.apply(&mut sk.borrow());
    sk.update_world_transform(Physics::Update);

    sk.setup_pose();
    state.clear_tracks();
    state.set_animation(0, anim.clone(), false);
    state.apply(&mut sk.borrow());
    sk.update_world_transform(Physics::Update);

    let frame_count = (anim.duration * 60.0).ceil() as i32;
    let mut frames = Vec::new();
    for i in 0..=frame_count {
        if i > 0 {
            state.update(1.0 / 60.0);
            state.apply(&mut sk.borrow());
            sk.update_world_transform(Physics::Update);
        }
        frames.push(dump(&sk));
    }
    frames
}

/// The frames of the reference output, as parsed JSON.
fn run_cpp(headless: &Path, skeleton: &Path, atlas: &Path, animation: &str) -> Vec<Value> {
    let out = Command::new(headless)
        .arg(skeleton)
        .arg(atlas)
        .arg(animation)
        .output()
        .expect("run reference");
    assert!(
        out.status.success(),
        "reference failed for {} {animation}",
        skeleton.display()
    );
    parse_frames(&String::from_utf8_lossy(&out.stdout))
}

fn selected(files: Vec<(PathBuf, PathBuf)>) -> Vec<(PathBuf, PathBuf)> {
    // GOLDEN_SKELETON=substring narrows to matching skeleton files (and GOLDEN_ANIMATION to one
    // animation) for debugging.
    if let Ok(filter) = std::env::var("GOLDEN_SKELETON") {
        return files
            .into_iter()
            .filter(|(p, _)| p.to_string_lossy().contains(&filter))
            .collect();
    }
    if std::env::var_os("GOLDEN_ALL").is_some() {
        return files;
    }
    const DEFAULT: [&str; 6] = [
        "spineboy-ess",
        "hero-ess",
        "goblins-ess",
        "dragon-ess",
        "alien-ess",
        "windmill-ess",
    ];
    files
        .into_iter()
        .filter(|(p, _)| {
            DEFAULT
                .iter()
                .any(|d| p.file_stem().is_some_and(|s| s == *d))
        })
        .collect()
}

fn golden(ext: &str) {
    let Some(headless) = std::env::var_os("SPINE_CPP_HEADLESS").map(PathBuf::from) else {
        eprintln!("SPINE_CPP_HEADLESS not set; skipping");
        return;
    };
    if !common::comparable_build() {
        return;
    }
    let files = selected(exports(ext));
    if files.is_empty() {
        eprintln!("SPINE_RUNTIMES_DIR not set; skipping");
        return;
    }
    let mut errors = Vec::new();
    let mut compared = 0;
    for (skeleton, atlas) in &files {
        let data = load(skeleton, atlas);
        for animation in &data.animations {
            if std::env::var("GOLDEN_ANIMATION").is_ok_and(|a| a != animation.name()) {
                continue;
            }
            let cpp = run_cpp(&headless, skeleton, atlas, animation.name());
            let rust = run_rust(&data, animation.name());
            errors.extend(compare(&data.name, animation.name(), &rust, &cpp));
            compared += 1;
        }
        eprintln!("compared {}", data.name);
    }
    eprintln!("{compared} animations compared");
    assert!(
        errors.is_empty(),
        "{} mismatches, first:\n{}",
        errors.len(),
        errors
            .iter()
            .take(15)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn json_matches_reference() {
    golden("json");
}

#[test]
fn binary_matches_reference() {
    golden("skel");
}
