//! Golden tests that need more than the reference's own headless test provides: they advance the
//! skeleton's time and move the skeleton (so physics constraints simulate), and they cover
//! hand-written fixtures that exercise constraint options the example exports don't use.
//!
//! Needs `SPINE_RUNTIMES_DIR` (a spine-runtimes checkout) and `SPINE_CPP_DRIVER` (the compiled
//! `tests/cpp/driver.cpp`, see its header). Both are optional; the tests skip without them.

mod common;
use common::*;
use spine2d_core::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

const DT: f32 = 1.0 / 60.0;

/// How the animation is played.
#[derive(Clone, Copy, Debug)]
struct Options {
    looping: bool,
    additive: bool,
    alpha: f32,
}

fn driver() -> Option<PathBuf> {
    std::env::var_os("SPINE_CPP_DRIVER").map(PathBuf::from)
}

fn run_cpp(
    driver: &Path,
    skeleton: &Path,
    atlas: Option<&Path>,
    animation: Option<&str>,
    frames: usize,
    o: Options,
) -> Vec<serde_json::Value> {
    let out = Command::new(driver)
        .arg(skeleton)
        .arg(atlas.map_or("-".into(), |a| a.to_string_lossy().into_owned()))
        .arg(animation.unwrap_or("-"))
        .arg(frames.to_string())
        .arg(if o.looping { "1" } else { "0" })
        .arg(DT.to_string())
        .arg(if o.additive { "1" } else { "0" })
        .arg(o.alpha.to_string())
        .output()
        .expect("run driver");
    assert!(
        out.status.success(),
        "driver failed for {}: {}",
        skeleton.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_frames(&String::from_utf8_lossy(&out.stdout))
}

/// Mirrors `driver.cpp`: moves the skeleton, advances time, plays the animation.
fn run_rust(
    data: &Arc<SkeletonData>,
    animation: Option<&str>,
    frames: usize,
    o: Options,
) -> Vec<Frame> {
    let mut sk = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data.clone())));
    if let Some(name) = animation {
        let entry = state.set_animation_by_name(0, name, o.looping).unwrap();
        let e = state.entry_mut(entry).unwrap();
        e.additive = o.additive;
        e.alpha = o.alpha;
    }
    let mut out = Vec::new();
    for i in 0..frames {
        sk.set_position(
            (i as f32 * 0.15).sin() * 80.0,
            (i as f32 * 0.11).cos() * 30.0,
        );
        sk.update_time(DT);
        state.update(DT);
        state.apply(&mut sk.borrow());
        sk.update_world_transform(if i == 0 {
            Physics::Reset
        } else {
            Physics::Update
        });
        out.push(dump(&sk));
    }
    out
}

fn check(
    driver: &Path,
    skeleton: &Path,
    atlas: Option<&Path>,
    data: &Arc<SkeletonData>,
    frames: usize,
    o: Options,
    errors: &mut Vec<String>,
) {
    let mut animations: Vec<Option<String>> = vec![None];
    animations.extend(data.animations.iter().map(|a| Some(a.name().to_string())));
    for animation in animations {
        let cpp = run_cpp(driver, skeleton, atlas, animation.as_deref(), frames, o);
        let rust = run_rust(data, animation.as_deref(), frames, o);
        let label = format!("{} {o:?}", animation.as_deref().unwrap_or("setup"));
        errors.extend(compare(&data.name, &label, &rust, &cpp));
    }
}

fn report(errors: Vec<String>) {
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

/// The example skeletons that use physics constraints, simulated for two seconds with the
/// skeleton moving.
fn physics_examples(ext: &str) {
    let Some(driver) = driver() else {
        eprintln!("SPINE_CPP_DRIVER not set; skipping");
        return;
    };
    if !common::comparable_build() {
        return;
    }
    let mut errors = Vec::new();
    let wanted = [
        "owl-pro",
        "cloud-pot",
        "celestial-circus-pro",
        "food-app-delivery-pro",
    ];
    for (skeleton, atlas) in exports(ext) {
        let stem = skeleton.file_stem().unwrap().to_string_lossy().into_owned();
        if !wanted.contains(&stem.as_str()) {
            continue;
        }
        let data = load(&skeleton, &atlas);
        assert!(
            data.constraints
                .iter()
                .any(|c| matches!(c, ConstraintData::Physics(_))),
            "{stem} should have physics"
        );
        check(
            &driver,
            &skeleton,
            Some(&atlas),
            &data,
            120,
            Options {
                looping: true,
                additive: false,
                alpha: 1.0,
            },
            &mut errors,
        );
        eprintln!("compared {stem}");
    }
    report(errors);
}

#[test]
fn physics_json_matches_reference() {
    physics_examples("json");
}

#[test]
fn physics_binary_matches_reference() {
    physics_examples("skel");
}

/// Hand-written skeletons covering constraint options the example exports don't use. They have
/// no images, so they load against an empty atlas.
#[test]
fn synthetic_fixtures_match_reference() {
    let Some(driver) = driver() else {
        eprintln!("SPINE_CPP_DRIVER not set; skipping");
        return;
    };
    if !common::comparable_build() {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let atlas = Atlas::parse("", "", None);
    let mut errors = Vec::new();
    for file in files {
        let loader = AtlasAttachmentLoader::new(&atlas);
        let mut reader = SkeletonJson::new(&loader);
        let data = Arc::new(
            reader
                .read_skeleton_data_file(file.to_str().unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", file.display())),
        );
        let combos = [
            (false, false, 1.0),
            (true, false, 1.0),
            (true, true, 1.0),
            (true, false, 0.6),
            (true, true, 0.6),
        ];
        for (looping, additive, alpha) in combos {
            check(
                &driver,
                &file,
                None,
                &data,
                150,
                Options {
                    looping,
                    additive,
                    alpha,
                },
                &mut errors,
            );
        }
        eprintln!("compared {}", data.name);
    }
    report(errors);
}
