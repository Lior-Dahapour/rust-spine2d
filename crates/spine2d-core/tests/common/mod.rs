//! Helpers for tests that use the example exports from a spine-runtimes checkout.
//! Set `SPINE_RUNTIMES_DIR` to the checkout; tests skip when it is unset.

use spine2d_core::*;

// Spine 4.3 defaults to a y-down world; these tests are written for y-up like the reference's
// headless test.
#[ctor::ctor(unsafe)]
fn init_y_up_common() {
    spine2d_core::set_y_down(false);
}
use std::path::{Path, PathBuf};

pub fn examples_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPINE_RUNTIMES_DIR")?).join("examples");
    dir.is_dir().then_some(dir)
}

/// Every `*.json` / `*.skel` export under `examples/*/export` paired with an atlas from the same
/// folder (preferring the `-pma` atlas, like the C++ smoke test).
pub fn exports(ext: &str) -> Vec<(PathBuf, PathBuf)> {
    let Some(dir) = examples_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut folders: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path().join("export"))
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();
    for folder in folders {
        let mut files: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        let atlases: Vec<_> = files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "atlas"))
            .cloned()
            .collect();
        let Some(atlas) = atlases
            .iter()
            .find(|p| p.to_string_lossy().contains("pma"))
            .or(atlases.first())
            .cloned()
        else {
            continue;
        };
        for f in files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == ext))
        {
            out.push((f.clone(), atlas.clone()));
        }
    }
    out
}

/// Whether this build can be compared with the reference runtime bit for bit. The reference is an
/// optimized, link-time-optimized build, which merges sine and cosine of the same angle into one
/// call; the Rust port only does that in optimized builds. In debug builds this prints why the
/// comparison is skipped and returns false.
pub fn comparable_build() -> bool {
    if cfg!(debug_assertions) {
        eprintln!("skipping: the golden comparison needs `cargo test --release`");
        return false;
    }
    true
}

/// Mirrors the C++ headless test's texture loader: no texture, fixed 1024x1024 page size.
pub struct HeadlessTextureLoader;

impl TextureLoader for HeadlessTextureLoader {
    fn load(&mut self, page: &mut AtlasPage, _path: &str) {
        page.width = 1024;
        page.height = 1024;
        page.texture = page.index + 1;
    }

    fn unload(&mut self, _texture: usize) {}
}

pub fn load_atlas(path: &Path) -> Atlas {
    Atlas::from_file(path.to_str().unwrap(), Some(&mut HeadlessTextureLoader)).unwrap()
}

// --- Golden comparison support -------------------------------------------------------------------

use serde_json::Value;
use spine2d_core::json::JsonExt;
use std::sync::Arc;

pub struct Frame {
    /// name, local pose [x, y, rotation, scaleX, scaleY, shearX, shearY], applied world
    /// [a, b, c, d, worldX, worldY]
    pub bones: Vec<(String, [f32; 7], [f32; 6])>,
    /// name, attachment, sequence index, color
    pub slots: Vec<(String, Option<String>, i32, [f32; 4])>,
}

pub fn dump(sk: &Skeleton) -> Frame {
    Frame {
        bones: sk
            .bones()
            .iter()
            .map(|b| {
                let l = b.pose.local;
                let a = b.applied_pose();
                (
                    sk.data().bones[b.id().index()].name.to_string(),
                    [
                        l.x, l.y, l.rotation, l.scale_x, l.scale_y, l.shear_x, l.shear_y,
                    ],
                    [a.a, a.b, a.c, a.d, a.world_x, a.world_y],
                )
            })
            .collect(),
        slots: sk
            .slots()
            .iter()
            .map(|s| {
                let p = &s.pose;
                (
                    sk.data().slots[s.id().index()].name.to_string(),
                    p.attachment.as_ref().map(|a| a.name().to_string()),
                    p.sequence_index,
                    [p.color.r, p.color.g, p.color.b, p.color.a],
                )
            })
            .collect(),
    }
}

pub fn parse_frames(text: &str) -> Vec<Value> {
    let mut frames = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("=== ANIMATION FRAME") {
            frames.push(
                serde_json::from_str(lines.next().expect("frame json")).expect("valid frame json"),
            );
        }
    }
    frames
}

/// True if both print identically with six decimals, like the reference does.
fn same(rust: f32, cpp: &Value) -> bool {
    let expected = cpp.as_f64().unwrap();
    let actual: f64 = format!("{rust:.6}").parse().unwrap();
    // `-0.000000` and `0.000000` are the same number.
    actual == expected
}

pub fn compare(name: &str, animation: &str, rust: &[Frame], cpp: &[Value]) -> Vec<String> {
    let mut errors = Vec::new();
    if rust.len() != cpp.len() {
        return vec![format!(
            "{name}/{animation}: {} frames vs {} in the reference",
            rust.len(),
            cpp.len()
        )];
    }
    const LOCAL: [&str; 7] = ["x", "y", "rotation", "scaleX", "scaleY", "shearX", "shearY"];
    const WORLD: [&str; 6] = ["a", "b", "c", "d", "worldX", "worldY"];
    for (i, (r, c)) in rust.iter().zip(cpp).enumerate() {
        let cbones = c
            .get_ci("bones")
            .unwrap()
            .children()
            .map(|(_, b)| b)
            .collect::<Vec<_>>();
        if cbones.len() != r.bones.len() {
            errors.push(format!("{name}/{animation} frame {i}: bone count"));
            continue;
        }
        for (bname, local, world, cb) in r
            .bones
            .iter()
            .zip(&cbones)
            .map(|((n, l, w), cb)| (n, l, w, cb))
        {
            if cb.str("name") != Some(bname) {
                errors.push(format!(
                    "{name}/{animation} frame {i}: bone name {bname} vs {:?}",
                    cb.str("name")
                ));
                continue;
            }
            let pose = cb.get_ci("pose").unwrap();
            for (k, key) in LOCAL.iter().enumerate() {
                if !same(local[k], pose.get_ci(key).unwrap()) {
                    errors.push(format!(
                        "{name}/{animation} frame {i} bone {bname} pose.{key}: {} vs {}",
                        local[k],
                        pose.get_ci(key).unwrap()
                    ));
                }
            }
            {
                let applied = cb.get_ci("appliedPose").unwrap();
                for (k, key) in WORLD.iter().enumerate() {
                    if !same(world[k], applied.get_ci(key).unwrap()) {
                        errors.push(format!("{name}/{animation} frame {i} bone {bname} applied.{key}: {} ({:08x}) vs {}", world[k], world[k].to_bits(), applied.get_ci(key).unwrap()));
                    }
                }
            }
        }
        let cslots = c
            .get_ci("slots")
            .unwrap()
            .children()
            .map(|(_, s)| s)
            .collect::<Vec<_>>();
        for ((sname, attachment, sequence, color), cs) in r.slots.iter().zip(&cslots) {
            let pose = cs.get_ci("pose").unwrap();
            let c_attachment = pose.get_ci("attachment").and_then(|a| a.as_str());
            if attachment.as_deref() != c_attachment {
                errors.push(format!("{name}/{animation} frame {i} slot {sname} attachment: {attachment:?} vs {c_attachment:?}"));
            }
            if *sequence != pose.int("sequenceIndex", 0) {
                errors.push(format!(
                    "{name}/{animation} frame {i} slot {sname} sequenceIndex: {sequence} vs {}",
                    pose.int("sequenceIndex", 0)
                ));
            }
            let c_color = pose.get_ci("color").unwrap();
            for (k, key) in ["r", "g", "b", "a"].iter().enumerate() {
                if !same(color[k], c_color.get_ci(key).unwrap()) {
                    errors.push(format!(
                        "{name}/{animation} frame {i} slot {sname} color.{key}: {} vs {}",
                        color[k],
                        c_color.get_ci(key).unwrap()
                    ));
                }
            }
        }
        if errors.len() > 20 {
            break;
        }
    }
    errors
}

/// Loads a `.json` or `.skel` skeleton with the atlas.
pub fn load(skeleton: &std::path::Path, atlas_path: &std::path::Path) -> Arc<SkeletonData> {
    let atlas = load_atlas(atlas_path);
    let loader = AtlasAttachmentLoader::new(&atlas);
    let path = skeleton.to_str().unwrap();
    let data = if path.ends_with(".json") {
        SkeletonJson::new(&loader).read_skeleton_data_file(path)
    } else {
        SkeletonBinary::new(&loader).read_skeleton_data_file(path)
    };
    Arc::new(data.unwrap_or_else(|e| panic!("{path}: {e}")))
}
