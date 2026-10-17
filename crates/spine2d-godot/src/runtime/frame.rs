//! One Godot frame: the stages of the world run over the skeletons of one mode, and what each stage
//! queued is acted on in between.
//!
//! The world is closed whenever Godot is called, so a script connected to a signal can freely call
//! back into its sprite, its bones and its animation state.

use godot::global::godot_error;
use spine2d_ecs::SpineSystems;

use crate::resources::callbacks::SpineCallbacks;
use crate::runtime::world::{self, Frame};
use crate::systems::follow::trigger_transform_changes;
use crate::systems::signals::{emit, emit_events};

/// Runs the stages once for every sprite moved by `frame`, `delta` seconds after the last one.
/// `queue` is the driver's buffer for what each stage queued; it is empty after.
///
/// Scripts connected to the signals must use `queue_free()`, never `free()`, on sprites and bone
/// nodes; see the warning in [`crate::systems::signals`].
///
/// The queued steps are applied first, because a queued step is what makes a manual sprite move this frame.
pub fn run(frame: Frame, delta: f32, queue: &mut SpineCallbacks) {
    queue.clear();
    phase(frame, Some(delta), &[SpineSystems::Commands], queue);
    if queue.no_sprites() {
        return;
    }
    // Skeletons move this frame, so there will be something to draw.
    if let Err(error) = world::with(|spine| spine.ping_render()) {
        godot_error!("Spine: {error}");
    }
    emit(queue.sprites_mut(), "before_animation_state_update");
    queue.clear();

    phase(frame, None, &[SpineSystems::Update], queue);
    emit(queue.sprites_mut(), "before_animation_state_apply");
    queue.clear();

    phase(frame, None, &[SpineSystems::Apply], queue);
    emit_events(queue.events_mut());
    emit(queue.sprites_mut(), "before_world_transforms_change");
    queue.clear();

    phase(
        frame,
        None,
        &[SpineSystems::Transforms, SpineSystems::Follow],
        queue,
    );
    trigger_transform_changes(queue.transforms_mut());
    emit(queue.sprites_mut(), "world_transforms_changed");
    queue.clear();

    phase(frame, None, &[SpineSystems::Cleanup], queue);
    queue.clear();
}

/// Runs `stages` for `frame` in one world borrow, after setting the clock to `delta` if given, and
/// moves what they queued into `queue`.
fn phase(frame: Frame, delta: Option<f32>, stages: &[SpineSystems], queue: &mut SpineCallbacks) {
    let ran = world::with(|spine| {
        if let Some(delta) = delta {
            spine.advance(delta);
        }
        for stage in stages {
            spine.run(frame, *stage);
        }
        spine.swap_callbacks(queue);
    });
    if let Err(error) = ran {
        godot_error!("Spine: {error}");
    }
}
