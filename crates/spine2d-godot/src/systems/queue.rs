//! The systems that queue callbacks for Godot while a stage runs: which sprites the stage moved and
//! which animation events it reported. They only clone handles; Godot is called after the world
//! is closed.

use spine2d_ecs::bevy_ecs::component::Component;
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::bevy_ecs::query::With;
use spine2d_ecs::bevy_ecs::system::{Local, NonSend, NonSendMut, Query, Res};
use spine2d_ecs::{SpineEvent, SpineEventChannel};

use crate::resources::callbacks::{QueuedEvent, SpineCallbacks};
use crate::resources::godot_nodes::GodotNodes;

/// Queues the sprite of every skeleton marked `M`, which is the set the stage just processed.
pub fn queue_sprites<M: Component>(
    nodes: NonSend<GodotNodes>,
    mut callbacks: NonSendMut<SpineCallbacks>,
    skeletons: Query<Entity, With<M>>,
) {
    for entity in &skeletons {
        if let Some(sprite) = nodes.sprite(entity) {
            callbacks.push_sprite(sprite.clone());
        }
    }
}

/// Queues the animation events reported since the last call, each with its sprite. Events of
/// skeletons without a sprite are dropped.
///
/// TODO: check the performance of an unsafe `Send` wrapper around `Gd` later on. The listener
/// could then send the event with its sprite straight into one channel, and this system would not
/// be needed. The wrapper must only be cloned, moved or dropped off the main thread.
pub fn queue_events(
    channel: Res<SpineEventChannel>,
    nodes: NonSend<GodotNodes>,
    mut callbacks: NonSendMut<SpineCallbacks>,
    mut drained: Local<Vec<(Entity, SpineEvent)>>,
) {
    channel.drain_into(&mut drained);
    for (entity, event) in drained.drain(..) {
        if let Some(sprite) = nodes.sprite(entity) {
            callbacks.push_event(QueuedEvent {
                sprite: sprite.clone(),
                entity,
                event,
            });
        }
    }
}
