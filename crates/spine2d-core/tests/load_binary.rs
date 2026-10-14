#[allow(dead_code)]
mod common;
use common::*;
use spine2d_core::*;

#[test]
fn loads_every_example_binary() {
    let files = exports("skel");
    if files.is_empty() {
        eprintln!("SPINE_RUNTIMES_DIR not set; skipping");
        return;
    }
    let mut failures = Vec::new();
    for (skel, atlas) in &files {
        let atlas = load_atlas(atlas);
        let loader = AtlasAttachmentLoader::new(&atlas);
        let mut reader = SkeletonBinary::new(&loader);
        match reader.read_skeleton_data_file(skel.to_str().unwrap()) {
            Ok(data) => {
                let data = std::sync::Arc::new(data);
                let mut sk = Skeleton::new(data.clone());
                sk.update_world_transform(Physics::None);
                eprintln!(
                    "ok  {:<40} bones={:<3} slots={:<3} skins={:<2} anims={:<3} constraints={} warnings={}",
                    data.name, data.bones.len(), data.slots.len(), data.skins.len(), data.animations.len(), data.constraints.len(), reader.warnings().len()
                );
            }
            Err(e) => failures.push(format!("{}: {e}", skel.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "failed to load:\n{}",
        failures.join("\n")
    );
}

#[test]
fn truncated_and_garbage_input_is_an_error_not_a_panic() {
    let atlas = Atlas::parse("", "", None);
    let loader = AtlasAttachmentLoader::new(&atlas);
    let mut reader = SkeletonBinary::new(&loader);
    assert!(reader.read_skeleton_data(&[]).is_err());
    assert!(reader.read_skeleton_data(&[0; 16]).is_err());
    assert!(reader.read_skeleton_data(&[0xff; 64]).is_err());

    // Truncating a real file at every length must never panic.
    if let Some((skel, _)) = exports("skel")
        .into_iter()
        .find(|(p, _)| p.to_string_lossy().contains("spineboy-pro"))
    {
        let bytes = std::fs::read(skel).unwrap();
        for len in (0..bytes.len()).step_by(97) {
            assert!(
                reader.read_skeleton_data(&bytes[..len]).is_err(),
                "len {len}"
            );
        }
    }
}
