//! Loads a Spine export, plays an animation and prints the draw data the renderer produces.
//!
//! Usage: quickstart <skeleton.json|.skel> <atlas file> <animation>
//!
//! The renderer is engine-agnostic: each `RenderCommand` holds the vertices, UVs, colors and
//! indices of one draw call, plus its blend mode and texture handle.

use spine2d_core::{
    AnimationState, AnimationStateData, Atlas, AtlasAttachmentLoader, AtlasPage, Physics, Skeleton,
    SkeletonBinary, SkeletonJson, TextureLoader,
};
use spine2d_render::SkeletonRenderer;
use std::sync::Arc;

/// Stands in for a real engine: gives every atlas page a handle the draw data will refer to.
struct PageHandles;

impl TextureLoader for PageHandles {
    fn load(&mut self, page: &mut AtlasPage, _path: &str) {
        page.texture = page.index + 1; // an engine would load the image and store its own handle
        page.width = 1024;
        page.height = 1024;
    }

    fn unload(&mut self, _texture: usize) {}
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (skeleton_path, atlas_path, animation) = (&args[1], &args[2], &args[3]);

    let atlas = Atlas::from_file(atlas_path, Some(&mut PageHandles)).expect("read atlas");
    let attachments = AtlasAttachmentLoader::new(&atlas);
    let data = if skeleton_path.ends_with(".json") {
        SkeletonJson::new(&attachments).read_skeleton_data_file(skeleton_path)
    } else {
        SkeletonBinary::new(&attachments).read_skeleton_data_file(skeleton_path)
    };
    let data = Arc::new(data.expect("read skeleton"));

    let mut skeleton = Skeleton::new(data.clone());
    let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data)));
    state
        .set_animation_by_name(0, animation, true)
        .expect("unknown animation");

    let mut renderer = SkeletonRenderer::new();
    for frame in 0..3 {
        let delta = 1.0 / 60.0;
        skeleton.update_time(delta);
        state.update(delta);
        state.apply(&mut skeleton.borrow());
        skeleton.update_world_transform(Physics::Update);

        let commands = renderer.render(&skeleton.borrow_ref());
        println!("frame {frame}: {} draw calls", commands.len());
        for command in commands {
            println!(
                "  texture {} blend {:?}: {} vertices, {} indices",
                command.texture,
                command.blend_mode,
                command.vertex_count(),
                command.indices.len()
            );
        }
    }
}
