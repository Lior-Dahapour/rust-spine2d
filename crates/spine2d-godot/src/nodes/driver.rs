//! The hidden node that runs the world once per Godot frame.
//!
//! The extension adds one to the scene root when the main loop starts and removes it when the main
//! loop ends. It advances the skeletons in `_process` and `_physics_process`, and draws the ones
//! that changed just before Godot draws the frame. It keeps only the buffer that carries what the
//! stages queued for Godot: the world lives in [`crate::runtime::world`], so every node can reach it
//! directly.

use std::cell::RefCell;

use godot::classes::node::InternalMode;
use godot::classes::{Engine, INode, Node, RenderingServer, SceneTree};
use godot::global::godot_error;
use godot::obj::{NewAlloc, Singleton};
use godot::prelude::*;

use crate::resources::callbacks::SpineCallbacks;
use crate::runtime::frame;
use crate::runtime::world;
use crate::runtime::world::Frame;

/// Advances every skeleton in the world once per frame, at the default process priority.
///
/// The driver sits in the root before the main scene, so it ticks before scene scripts: a call a
/// script makes in `_process` takes effect, and its signals fire, in the next frame.
#[derive(GodotClass)]
#[class(base=Node, tool, init)]
pub struct SpineDriver {
    base: Base<Node>,
    /// What the stages of a frame queued for Godot, kept between frames so it allocates once.
    queue: SpineCallbacks,
}

#[godot_api]
impl INode for SpineDriver {
    fn ready(&mut self) {
        self.base_mut().set_physics_process(true);
        // Everything that changed this frame is in by the time Godot is about to draw.
        let callable = Callable::from_object_method(&self.to_gd(), "render_frame");
        RenderingServer::singleton().connect("frame_pre_draw", &callable);
    }

    fn process(&mut self, delta: f64) {
        frame::run(Frame::Process, delta as f32, &mut self.queue);
    }

    fn physics_process(&mut self, delta: f64) {
        frame::run(Frame::Physics, delta as f32, &mut self.queue);
    }
}

#[godot_api]
impl SpineDriver {
    /// Draws the skeletons that changed. Connected to the render server's `frame_pre_draw`.
    #[func]
    fn render_frame(&mut self) {
        if let Err(error) = world::with(|spine| spine.run_render()) {
            godot_error!("Spine: {error}");
        }
    }
}

thread_local! {
    static DRIVER: RefCell<Option<Gd<SpineDriver>>> = const { RefCell::new(None) };
}

/// Adds the driver to the scene root, hidden from the user's scene as an internal child. Does
/// nothing if one is already added.
pub fn add() {
    if DRIVER.with(|slot| slot.borrow().is_some()) {
        return;
    }
    let Some(mut root) = Engine::singleton()
        .get_main_loop()
        .and_then(|main_loop| main_loop.try_cast::<SceneTree>().ok())
        .and_then(|tree| tree.get_root())
    else {
        godot_error!("Spine: there is no scene root to attach the driver to");
        return;
    };
    let driver = SpineDriver::new_alloc();
    root.add_child_ex(&driver)
        .internal(InternalMode::FRONT)
        .done();
    DRIVER.with(|slot| *slot.borrow_mut() = Some(driver));
}

/// Frees the driver. The tree may have freed it already while shutting down.
pub fn remove() {
    let Some(driver) = DRIVER.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    if driver.is_instance_valid() {
        let callable = Callable::from_object_method(&driver, "render_frame");
        let mut server = RenderingServer::singleton();
        if server.is_connected("frame_pre_draw", &callable) {
            server.disconnect("frame_pre_draw", &callable);
        }
        driver.free();
    }
}
