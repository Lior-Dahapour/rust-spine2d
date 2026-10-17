//! What the world asks Godot to do once it is closed. The stages queue these while the world is
//! open, holding cloned node handles, and the frame acts on them afterwards: a script reacting to a
//! signal or a transform can then call back into the world.
//!
//! There is one small vector per kind, not one vector of an enum: an enum slot is as big as its
//! largest variant, and a sprite is queued once per stage for every skeleton.

use godot::classes::Node2D;
use godot::obj::Gd;
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::glam::Affine2;
use spine2d_ecs::SpineEvent;

use crate::nodes::sprite::SpineSprite;

/// A sprite the stage processed, which gets the update signals of that stage.
pub struct QueuedSprite {
    /// The node to emit on.
    pub sprite: Gd<SpineSprite>,
}

/// An animation event, with the sprite it belongs to.
pub struct QueuedEvent {
    /// The node to emit on.
    pub sprite: Gd<SpineSprite>,
    /// The skeleton the event came from.
    pub entity: Entity,
    /// What happened.
    pub event: SpineEvent,
}

/// A bone or slot node and the placement its follower computed.
pub struct QueuedTransform {
    /// The node to place.
    pub node: Gd<Node2D>,
    /// Where its bone put it.
    pub placement: Affine2,
}

/// The callbacks queued since the frame last took them. It is a non-send resource of the world,
/// and the driver keeps a second one that the frame swaps with it.
#[derive(Default)]
pub struct SpineCallbacks {
    sprites: Vec<QueuedSprite>,
    events: Vec<QueuedEvent>,
    transforms: Vec<QueuedTransform>,
}

impl SpineCallbacks {
    /// Queues a sprite for the update signals.
    pub fn push_sprite(&mut self, sprite: Gd<SpineSprite>) {
        self.sprites.push(QueuedSprite { sprite });
    }

    /// Queues an animation event.
    pub fn push_event(&mut self, event: QueuedEvent) {
        self.events.push(event);
    }

    /// Queues a node to place.
    pub fn push_transform(&mut self, node: Gd<Node2D>, placement: Affine2) {
        self.transforms.push(QueuedTransform { node, placement });
    }

    /// The queued sprites, to emit on.
    pub fn sprites_mut(&mut self) -> &mut [QueuedSprite] {
        &mut self.sprites
    }

    /// The queued animation events, to emit.
    pub fn events_mut(&mut self) -> &mut [QueuedEvent] {
        &mut self.events
    }

    /// The queued nodes to place.
    pub fn transforms_mut(&mut self) -> &mut [QueuedTransform] {
        &mut self.transforms
    }

    /// Whether no sprite is queued. Only the stages that queue sprites are asked this.
    pub fn no_sprites(&self) -> bool {
        self.sprites.is_empty()
    }

    /// Forgets everything queued, keeping the allocations.
    pub fn clear(&mut self) {
        self.sprites.clear();
        self.events.clear();
        self.transforms.clear();
    }

    /// Swaps the contents with `other`, which must be empty, so neither side allocates.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_queue_has_nothing() {
        let queue = SpineCallbacks::default();

        assert!(queue.no_sprites());
        assert!(queue.events.is_empty());
        assert!(queue.transforms.is_empty());
    }

    #[test]
    fn swapping_moves_the_contents_and_keeps_the_empty_side_empty() {
        let mut full = SpineCallbacks::default();
        full.sprites.reserve(8);
        let capacity = full.sprites.capacity();
        let mut out = SpineCallbacks::default();

        full.swap(&mut out);

        assert_eq!(out.sprites.capacity(), capacity);
        assert!(full.no_sprites());
    }
}
