//! The Godot nodes the world has to reach but cannot hold as components, because Godot objects are
//! not `Send`. Entities are the keys; every lookup happens inside the world, and only handles
//! leave it.

use godot::classes::Node2D;
use godot::obj::Gd;
use spine2d_ecs::bevy_ecs::entity::{Entity, EntityHashMap};

use crate::nodes::sprite::SpineSprite;

/// A non-send resource of the world.
#[derive(Default)]
pub struct GodotNodes {
    /// The sprite node of each skeleton entity.
    sprites: EntityHashMap<Gd<SpineSprite>>,
    /// The node each follower entity places.
    followers: EntityHashMap<Gd<Node2D>>,
}

impl GodotNodes {
    /// Makes `sprite` the node of the skeleton `entity`.
    pub fn add_sprite(&mut self, entity: Entity, sprite: Gd<SpineSprite>) {
        self.sprites.insert(entity, sprite);
    }

    /// Forgets the sprite of the skeleton `entity`.
    pub fn remove_sprite(&mut self, entity: Entity) {
        self.sprites.remove(&entity);
    }

    /// The sprite node of the skeleton `entity`.
    pub fn sprite(&self, entity: Entity) -> Option<&Gd<SpineSprite>> {
        self.sprites.get(&entity)
    }

    /// Makes `node` the one placed by the follower `entity`.
    pub fn add_follower(&mut self, entity: Entity, node: Gd<Node2D>) {
        self.followers.insert(entity, node);
    }

    /// Forgets the node of the follower `entity`.
    pub fn remove_follower(&mut self, entity: Entity) {
        self.followers.remove(&entity);
    }

    /// The node the follower `entity` places.
    pub fn follower(&self, entity: Entity) -> Option<&Gd<Node2D>> {
        self.followers.get(&entity)
    }
}
