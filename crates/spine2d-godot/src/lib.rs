//! Godot 4 extension that plays Spine 4.3 skeletons with the Rust runtime.

mod api;
mod attachment;
mod batch_mesh;
mod bone_node;
mod debug;
mod loader;
mod materials;
mod resources;
mod skin;
mod slot_node;
mod sprite;

use godot::classes::ResourceLoader;
use godot::obj::Singleton;
use godot::prelude::*;
use std::cell::RefCell;

struct Spine2dExtension;

thread_local! {
    // Keeps the format loader alive while it is registered with Godot.
    static LOADER: RefCell<Option<Gd<loader::SpineResourceLoader>>> = const { RefCell::new(None) };
}

#[gdextension]
unsafe impl ExtensionLibrary for Spine2dExtension {
    fn on_stage_init(stage: InitStage) {
        if stage == InitStage::Scene {
            let loader = loader::SpineResourceLoader::new_gd();
            ResourceLoader::singleton().add_resource_format_loader(&loader);
            LOADER.with(|slot| *slot.borrow_mut() = Some(loader));
        }
    }

    fn on_stage_deinit(stage: InitStage) {
        if stage == InitStage::Scene {
            if let Some(loader) = LOADER.with(|slot| slot.borrow_mut().take()) {
                ResourceLoader::singleton().remove_resource_format_loader(&loader);
            }
            materials::free_all();
        }
    }
}
