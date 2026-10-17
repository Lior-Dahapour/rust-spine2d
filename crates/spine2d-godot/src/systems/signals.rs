//! The signals of a sprite: the update hooks and the animation events, with the arguments of the
//! official plugin: the sprite first, then (for animation events) its animation state and track
//! entry. Each takes what the world
//! reported and calls Godot, so the world must be closed when they run: a script connected to a
//! signal can call back into its sprite, its bones and its animation state.
//!
//! WARNING: a handler must never free a sprite or a bone node with `free()`. Use `queue_free()`,
//! which frees it after the frame. The queue holds a handle for every sprite of the stage, so a
//! node freed in the middle of the loop is still in it, and the next signal on that handle panics.

use godot::prelude::*;
use spine2d_ecs::SpineEventKind;

use crate::handles::{SpineAnimationState, SpineEvent, SpineTrackEntry};
use crate::resources::callbacks::{QueuedEvent, QueuedSprite};

/// Emits a signal on every queued sprite, with the sprite as its only argument.
pub fn emit(queue: &mut [QueuedSprite], signal: &str) {
    for queued in queue {
        let sprite = queued.sprite.to_variant();
        queued.sprite.emit_signal(signal, &[sprite]);
    }
}

/// Emits the animation signal of each queued event on the sprite it came from. Each carries the
/// sprite, its animation state and the track entry, and `animation_event` also the event.
pub fn emit_events(queue: &mut [QueuedEvent]) {
    for QueuedEvent {
        sprite,
        entity,
        event,
    } in queue
    {
        let sprite_arg = sprite.to_variant();
        let state = SpineAnimationState::create(*entity).to_variant();
        let entry = SpineTrackEntry::create(*entity, event.entry).to_variant();
        match &event.kind {
            SpineEventKind::Custom(custom) => {
                let event = SpineEvent::create(*entity, custom).to_variant();
                sprite.emit_signal("animation_event", &[sprite_arg, state, entry, event])
            }
            SpineEventKind::Start => {
                sprite.emit_signal("animation_started", &[sprite_arg, state, entry])
            }
            SpineEventKind::Interrupt => {
                sprite.emit_signal("animation_interrupted", &[sprite_arg, state, entry])
            }
            SpineEventKind::End => {
                sprite.emit_signal("animation_ended", &[sprite_arg, state, entry])
            }
            SpineEventKind::Dispose => {
                sprite.emit_signal("animation_disposed", &[sprite_arg, state, entry])
            }
            SpineEventKind::Complete => {
                sprite.emit_signal("animation_completed", &[sprite_arg, state, entry])
            }
        };
    }
}
