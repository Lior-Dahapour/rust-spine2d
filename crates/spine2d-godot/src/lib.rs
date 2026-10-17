//! Godot 4 extension that plays Spine 4.3 skeletons with the Rust runtime, on one shared ECS
//! world.

mod constants;
mod handles;
mod nodes;
mod resources;
mod runtime;
mod systems;

use godot::classes::ResourceLoader;
use godot::obj::Singleton;
use godot::prelude::*;
use std::cell::RefCell;

struct Spine2dEcsExtension;

thread_local! {
    // Keeps the format loader alive while it is registered with Godot.
    static LOADER: RefCell<Option<Gd<resources::loader::SpineResourceLoader>>> = const { RefCell::new(None) };
}

#[gdextension]
unsafe impl ExtensionLibrary for Spine2dEcsExtension {
    fn on_stage_init(stage: InitStage) {
        if stage == InitStage::Scene {
            let loader = resources::loader::SpineResourceLoader::new_gd();
            ResourceLoader::singleton().add_resource_format_loader(&loader);
            LOADER.with(|slot| *slot.borrow_mut() = Some(loader));
            runtime::world::create();
        }
        if stage == InitStage::MainLoop {
            nodes::driver::add();
        }
    }

    fn on_stage_deinit(stage: InitStage) {
        if stage == InitStage::MainLoop {
            nodes::driver::remove();
        }
        if stage == InitStage::Scene {
            runtime::world::destroy();
            if let Some(loader) = LOADER.with(|slot| slot.borrow_mut().take()) {
                ResourceLoader::singleton().remove_resource_format_loader(&loader);
            }
        }
    }
}
