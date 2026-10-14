//! `SpineBoneNode`: a `Node2D` child of a `SpineSprite` that follows one of its bones.

use crate::sprite::{SharedPlayer, SpineSprite};
use godot::classes::{INode2D, Node, Node2D};
use godot::prelude::*;
use spine2d_core::{BoneId, BonePose};
use std::rc::Rc;

/// Connects `node` to its parent sprite's `world_transforms_changed` signal, so it can follow its
/// bone after every update. Returns the sprite, or `None` when the parent is not a `SpineSprite`.
/// The node must have a `on_world_transforms_changed` method.
pub(crate) fn attach_to_parent_sprite<T: GodotClass + Inherits<Node>>(
    node: &Gd<T>,
) -> Option<Gd<SpineSprite>> {
    let parent = node.clone().upcast::<Node>().get_parent()?;
    let mut sprite = parent.try_cast::<SpineSprite>().ok()?;
    let callable = Callable::from_object_method(node, "on_world_transforms_changed");
    if !sprite.is_connected("world_transforms_changed", &callable) {
        sprite.connect("world_transforms_changed", &callable);
    }
    Some(sprite)
}

/// Undoes [`attach_to_parent_sprite`].
pub(crate) fn detach_from_sprite<T: GodotClass>(node: &Gd<T>, sprite: Option<Gd<SpineSprite>>) {
    let Some(mut sprite) = sprite.filter(|s| s.is_instance_valid()) else {
        return;
    };
    let callable = Callable::from_object_method(node, "on_world_transforms_changed");
    if sprite.is_connected("world_transforms_changed", &callable) {
        sprite.disconnect("world_transforms_changed", &callable);
    }
}

/// The transform that places a child of a bone node: columns are the bone's x axis, y axis and
/// origin in the sprite's space.
pub(crate) fn bone_transform(pose: &BonePose) -> Transform2D {
    Transform2D::from_cols(
        Vector2::new(pose.a, pose.c),
        Vector2::new(pose.b, pose.d),
        Vector2::new(pose.world_x, pose.world_y),
    )
}

/// Follows a bone of its parent `SpineSprite`: attach weapons, effects or colliders to a bone by
/// making them children of this node.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct SpineBoneNode {
    base: Base<Node2D>,
    /// Name of the bone to follow.
    #[export]
    #[var(set = set_bone_name)]
    bone_name: GString,
    /// The player and bone resolved on the last update; re-resolved when the sprite reloads.
    resolved: Option<(SharedPlayer, BoneId)>,
    connected_to: Option<Gd<SpineSprite>>,
}

#[godot_api]
impl INode2D for SpineBoneNode {
    fn init(base: Base<Node2D>) -> Self {
        SpineBoneNode {
            base,
            bone_name: GString::new(),
            resolved: None,
            connected_to: None,
        }
    }

    #[func(gd_self)]
    fn enter_tree(mut this: Gd<Self>) {
        let Some(sprite) = attach_to_parent_sprite(&this) else {
            return;
        };
        this.bind_mut().connected_to = Some(sprite);
        this.bind_mut().follow();
    }

    #[func(gd_self)]
    fn exit_tree(mut this: Gd<Self>) {
        let sprite = this.bind_mut().connected_to.take();
        detach_from_sprite(&this, sprite);
    }
}

#[godot_api]
impl SpineBoneNode {
    #[func]
    fn set_bone_name(&mut self, name: GString) {
        self.bone_name = name;
        self.resolved = None;
        self.follow();
    }

    /// Called by the sprite after it computed world transforms.
    #[func]
    fn on_world_transforms_changed(&mut self) {
        self.follow();
    }
}

impl SpineBoneNode {
    /// Copies the bone's world transform onto this node.
    fn follow(&mut self) {
        let Some(sprite) = self.connected_to.as_ref().filter(|s| s.is_instance_valid()) else {
            return;
        };
        let Some(player) = sprite.bind().player() else {
            return;
        };
        let stale = self
            .resolved
            .as_ref()
            .is_none_or(|(cached, _)| !Rc::ptr_eq(cached, &player));
        if stale {
            let id = player
                .borrow()
                .skeleton
                .find_bone(&self.bone_name.to_string());
            self.resolved = id.map(|id| (player.clone(), id));
        }
        let Some((player, id)) = &self.resolved else {
            return;
        };
        let pose = *player.borrow().skeleton.bone(*id).applied_pose();
        let transform = bone_transform(&pose);
        self.base_mut().set_transform(transform);
    }
}
