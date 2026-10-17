//! A skeleton made with `Skeleton::detached`, driven only through the `_with` methods, behaves
//! exactly like one that holds its data: same lookups, same skins, same poses, bit for bit.

#[allow(dead_code)]
mod common;

use spine2d_core::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[ctor::ctor(unsafe)]
fn init_y_up() {
    spine2d_core::set_y_down(false);
}

const DT: f32 = 1.0 / 60.0;

fn fixtures() -> Vec<(String, Arc<SkeletonData>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("the fixture folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let atlas = Atlas::parse("", "", None);
    files
        .into_iter()
        .map(|file| {
            let loader = AtlasAttachmentLoader::new(&atlas);
            let data = SkeletonJson::new(&loader)
                .read_skeleton_data_file(file.to_str().expect("a utf-8 path"))
                .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
            (file.display().to_string(), Arc::new(data))
        })
        .collect()
}

/// Fails unless both skeletons have the same world matrices, local poses, shown attachments and
/// draw order.
fn assert_same(held: &Skeleton, detached: &Skeleton, context: &str) {
    for (i, (a, b)) in held.bones().iter().zip(detached.bones()).enumerate() {
        let (a, b) = (a.applied_pose(), b.applied_pose());
        assert_eq!(
            [a.a, a.b, a.c, a.d, a.world_x, a.world_y],
            [b.a, b.b, b.c, b.d, b.world_x, b.world_y],
            "{context}: world matrix of bone {i}"
        );
        assert_eq!(a.local, b.local, "{context}: local pose of bone {i}");
    }
    for (i, (a, b)) in held.slots().iter().zip(detached.slots()).enumerate() {
        let name = |slot: &Slot| {
            slot.applied_pose()
                .attachment
                .as_ref()
                .map(|attachment| attachment.name().to_string())
        };
        assert_eq!(name(a), name(b), "{context}: attachment of slot {i}");
    }
    let order = |skeleton: &Skeleton| -> Vec<usize> {
        skeleton
            .draw_order()
            .applied_pose()
            .iter()
            .map(|slot| slot.0)
            .collect()
    };
    assert_eq!(order(held), order(detached), "{context}: draw order");
}

/// Drives a held and a detached skeleton through the same lookups, skins, animations and setup
/// pose resets, and fails at the first difference.
fn assert_detached_matches_held(file: &str, data: &Arc<SkeletonData>) {
    let mut held = Skeleton::new(Arc::clone(data));
    let mut detached = Skeleton::detached(data);
    assert_same(&held, &detached, &format!("{file}: constructed"));

    for bone in &data.bones {
        assert_eq!(
            held.find_bone(&bone.name),
            data.find_bone(&bone.name).map(|bone| BoneId(bone.index))
        );
    }
    for slot in &data.slots {
        assert_eq!(
            held.find_slot(&slot.name),
            data.find_slot(&slot.name).map(|slot| SlotId(slot.index))
        );
        assert_eq!(
            held.attachment(&slot.name, &slot.attachment_name)
                .map(|a| a.name().to_string()),
            data.find_slot(&slot.name)
                .and_then(|found| {
                    detached
                        .as_borrow_ref(data)
                        .attachment_by_index(found.index, &slot.attachment_name)
                })
                .map(|a| a.name().to_string()),
        );
    }
    for constraint in &data.constraints {
        assert_eq!(
            held.find_constraint(constraint.name()),
            data.find_constraint(constraint.name())
        );
    }
    assert_eq!(held.find_bone("no such bone"), None);
    assert!(data.find_bone("no such bone").is_none());

    for skin in &data.skins {
        let name = skin.name().to_string();
        assert!(held.set_skin_by_name(&name));
        detached
            .as_borrow(data)
            .set_skin(data.find_skin(&name).cloned());
        assert_same(&held, &detached, &format!("{file}: skin {name}"));
    }
    assert!(!held.set_skin_by_name("no such skin"));
    assert!(data.find_skin("no such skin").is_none());

    let names: Vec<String> = data
        .animations
        .iter()
        .map(|a| a.name().to_string())
        .collect();
    let mut held_mix = AnimationStateData::new(Arc::clone(data));
    let mut detached_mix = AnimationStateData::detached();
    if let [first, second, ..] = names.as_slice() {
        assert!(held_mix.set_mix(first, second, 0.25));
        assert!(detached_mix.set_mix_with(data, first, second, 0.25));
        assert_eq!(held_mix.mix(first, second), detached_mix.mix(first, second));
    }
    assert!(!held_mix.set_mix("no such animation", "no such animation", 1.0));
    assert!(!detached_mix.set_mix_with(data, "no such animation", "no such animation", 1.0));
    let mut held_state = AnimationState::new(Arc::new(held_mix));
    let mut detached_state = AnimationState::detached();

    assert!(held_state
        .set_animation_by_name(0, "no such animation", true)
        .is_err());
    assert!(data.find_animation("no such animation").is_none());

    for name in &names {
        held_state
            .set_animation_by_name(0, name, true)
            .expect("an animation");
        detached_state.as_borrow(&detached_mix, data).set_animation(
            0,
            data.find_animation(name).expect("an animation").clone(),
            true,
        );
        if let Some(other) = names.first() {
            held_state
                .add_animation_by_name(1, other, false, 0.1)
                .expect("an animation");
            detached_state.as_borrow(&detached_mix, data).add_animation(
                1,
                data.find_animation(other).expect("an animation").clone(),
                false,
                0.1,
            );
        }
        for frame in 0..90 {
            held.update_time(DT);
            detached.update_time(DT);
            held_state.update(DT);
            detached_state.update(DT);
            held_state.apply(&mut held.borrow());
            detached_state.apply(&mut detached.as_borrow(data));
            held.update_world_transform(Physics::Update);
            detached
                .as_borrow(data)
                .update_world_transform(Physics::Update);
            assert_same(&held, &detached, &format!("{file}: {name}, frame {frame}"));
        }
        held.setup_pose();
        detached.as_borrow(data).setup_pose();
        assert_same(&held, &detached, &format!("{file}: {name}, setup pose"));
    }
    held.update_cache();
    detached.as_borrow(data).update_cache();
    assert_same(&held, &detached, &format!("{file}: update cache"));
}

#[test]
fn a_detached_skeleton_behaves_like_one_that_holds_its_data() {
    for (file, data) in fixtures() {
        assert_detached_matches_held(&file, &data);
    }
}

/// The same check on the Spine example skeletons, which have real skins. Needs
/// `SPINE_RUNTIMES_DIR`, a checkout of spine-runtimes, like the golden tests.
#[test]
fn a_detached_skeleton_behaves_like_a_held_one_on_the_example_skeletons() {
    let Some(root) = std::env::var_os("SPINE_RUNTIMES_DIR").map(PathBuf::from) else {
        eprintln!("SPINE_RUNTIMES_DIR not set; skipping");
        return;
    };
    let mut checked = 0;
    for entry in std::fs::read_dir(root.join("examples"))
        .expect("the examples folder")
        .flatten()
    {
        let export = entry.path().join("export");
        let Ok(files) = std::fs::read_dir(&export) else {
            continue;
        };
        for file in files.flatten().map(|f| f.path()) {
            let is_skeleton = file.to_string_lossy().ends_with("-pro.json")
                || file.to_string_lossy().ends_with("-ess.json");
            if !is_skeleton {
                continue;
            }
            let atlas = export.join(format!("{}.atlas", entry.file_name().to_string_lossy()));
            if !atlas.exists() {
                continue;
            }
            let data = common::load(&file, &atlas);
            assert_detached_matches_held(&file.display().to_string(), &data);
            checked += 1;
        }
    }
    assert!(checked > 0, "no example skeleton was found");
    eprintln!("checked {checked} example skeletons");
}

#[test]
fn a_skeleton_made_with_new_still_holds_its_data() {
    let (_, data) = fixtures().remove(0);
    let skeleton = Skeleton::new(Arc::clone(&data));
    assert!(Arc::ptr_eq(skeleton.data(), &data));
}

#[test]
#[should_panic(expected = "holds no data")]
fn a_detached_skeleton_panics_when_asked_without_the_data() {
    let (_, data) = fixtures().remove(0);
    Skeleton::detached(&data).update_world_transform(Physics::None);
}

#[test]
#[should_panic(expected = "hold no data")]
fn detached_mix_settings_panic_when_asked_without_the_data() {
    let mut mix = AnimationStateData::detached();
    mix.set_mix("a", "b", 1.0);
}
