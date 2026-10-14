#[allow(dead_code)]
mod common;
use common::*;
use spine2d_core::*;

#[test]
fn loads_every_example_json() {
    let files = exports("json");
    if files.is_empty() {
        eprintln!("SPINE_RUNTIMES_DIR not set; skipping");
        return;
    }
    let mut failures = Vec::new();
    for (json, atlas) in &files {
        let atlas = load_atlas(atlas);
        let loader = AtlasAttachmentLoader::new(&atlas);
        let mut reader = SkeletonJson::new(&loader);
        match reader.read_skeleton_data_file(json.to_str().unwrap()) {
            Ok(data) => {
                // The data must be usable: build a skeleton and pose it.
                let data = std::sync::Arc::new(data);
                let mut sk = Skeleton::new(data.clone());
                sk.update_world_transform(Physics::None);
                eprintln!(
                    "ok  {:<40} bones={:<3} slots={:<3} skins={:<2} anims={:<3} constraints={} warnings={}",
                    data.name,
                    data.bones.len(),
                    data.slots.len(),
                    data.skins.len(),
                    data.animations.len(),
                    data.constraints.len(),
                    reader.warnings().len()
                );
            }
            Err(e) => failures.push(format!("{}: {e}", json.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "failed to load:\n{}",
        failures.join("\n")
    );
}
