//! Applies the steps that nodes and scripts queued. It is the first thing a frame does, so a step
//! is what makes a manual skeleton move in that same frame.

use spine2d_ecs::bevy_ecs::query::Has;
use spine2d_ecs::bevy_ecs::system::{Commands, Query, ResMut};
use spine2d_ecs::systems::begin_step;
use spine2d_ecs::{SpinePhysics, SpineProcess, SpineStep};

use crate::resources::steps::GodotSteps;

/// Drains [`GodotSteps`]. Returns at once when nothing is queued, which is nearly every frame,
/// without touching a single skeleton.
pub fn apply_godot_steps(
    mut commands: Commands,
    mut queue: ResMut<GodotSteps>,
    modes: Query<(Has<SpineProcess>, Has<SpinePhysics>, Option<&SpineStep>)>,
) {
    if queue.is_empty() {
        return;
    }
    for (entity, delta) in queue.drain() {
        if let Ok((process, physics, earlier)) = modes.get(entity) {
            begin_step(&mut commands, entity, delta, process || physics, earlier);
        }
    }
}
