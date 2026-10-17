//! `SpineBoneNode`: a `Node2D` child of a `SpineSprite` that follows one of its bones, or drives
//! it.

use crate::constants::SpineConstant;
use crate::handles::SpineBone;
use crate::nodes::sprite::SpineSprite;
use crate::systems::follow;
use godot::classes::notify::CanvasItemNotification;
use godot::classes::{INode2D, Node, Node2D};
use godot::prelude::*;
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::glam::Affine2;
use spine2d_ecs::{SpineDriver, SpineFollower};

/// The `SpineSprite` that `node` is a child of, if its parent is one.
pub(crate) fn parent_sprite<T: GodotClass + Inherits<Node>>(
    node: &Gd<T>,
) -> Option<Gd<SpineSprite>> {
    node.clone().upcast::<Node>().get_parent()?.try_cast().ok()
}

/// The skeleton entity of the `SpineSprite` that `node` is a child of, if it has one.
pub(crate) fn parent_entity<T: GodotClass + Inherits<Node>>(node: &Gd<T>) -> Option<Entity> {
    parent_sprite(node)?.bind().entity()
}

/// Follows a bone of its parent `SpineSprite`: attach weapons, effects or colliders to a bone by
/// making them children of this node. In drive mode it is the other way round: the bone takes
/// the transform of this node, every frame, after the animation posed it.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct SpineBoneNode {
    base: Base<Node2D>,
    /// Name of the bone to follow.
    #[export]
    #[var(set = set_bone_name)]
    bone_name: GString,
    /// Whether the node follows the bone (0) or drives it (1).
    #[export(enum = (Follow = 0, Drive = 1))]
    #[var(set = set_bone_mode)]
    bone_mode: i32,
    /// Whether the node follows or drives its bone at all.
    #[export]
    #[var(set = set_enabled)]
    enabled: bool,
    /// Only kept so scripts written for the official plugin read back what they set.
    debug_color: Color,
    debug_thickness: f32,
    /// The skeleton entity of the parent sprite, while the node is in the tree.
    parent: Option<Entity>,
    /// The entity in the world that tracks the bone for this node.
    follower: Option<Entity>,
}

#[godot_api]
impl INode2D for SpineBoneNode {
    fn init(base: Base<Node2D>) -> Self {
        SpineBoneNode {
            base,
            bone_name: GString::new(),
            bone_mode: SpineConstant::BoneMode_Follow as i32,
            enabled: true,
            debug_color: Color::from_rgba8(0xff, 0x00, 0x00, 0x77),
            debug_thickness: 5.0,
            parent: None,
            follower: None,
        }
    }

    fn on_notification(&mut self, what: CanvasItemNotification) {
        if !matches!(what, CanvasItemNotification::TRANSFORM_CHANGED) {
            return;
        }
        if let Some(driver) = self.follower {
            let placement = self.placement();
            follow::drive(driver, placement);
        }
    }

    #[func(gd_self)]
    fn enter_tree(mut this: Gd<Self>) {
        let parent = parent_entity(&this);
        this.bind_mut().attach_to(parent);
    }

    #[func(gd_self)]
    fn exit_tree(mut this: Gd<Self>) {
        this.bind_mut().attach_to(None);
    }
}

#[godot_api]
impl SpineBoneNode {
    #[func]
    fn set_bone_name(&mut self, name: GString) {
        self.bone_name = name;
        self.attach_to(self.parent);
    }

    #[func]
    fn set_bone_mode(&mut self, bone_mode: i32) {
        if i64::from(bone_mode) != SpineConstant::BoneMode_Follow
            && i64::from(bone_mode) != SpineConstant::BoneMode_Drive
        {
            godot_warn!("Spine: {bone_mode} is not a bone mode (0 or 1)");
            return;
        }
        self.bone_mode = bone_mode;
        self.attach_to(self.parent);
    }

    #[func]
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.attach_to(self.parent);
    }

    #[func]
    fn get_debug_color(&self) -> Color {
        self.debug_color
    }

    #[func]
    fn set_debug_color(&mut self, color: Color) {
        godot_warn!("Spine: SpineBoneNode.debug_color has no effect, the node draws nothing; use the sprite's debug_bones");
        self.debug_color = color;
    }

    #[func]
    fn get_debug_thickness(&self) -> f32 {
        self.debug_thickness
    }

    #[func]
    fn set_debug_thickness(&mut self, thickness: f32) {
        godot_warn!("Spine: SpineBoneNode.debug_thickness has no effect, the node draws nothing; use the sprite's debug_bones_thickness");
        self.debug_thickness = thickness;
    }

    /// The bone this node follows or drives, or null if the node is hidden or has no sprite or
    /// no such bone.
    #[func]
    fn find_bone(&self) -> Option<Gd<SpineBone>> {
        if !self.base().is_visible_in_tree() {
            return None;
        }
        let sprite = self.find_sprite()?;
        let bone = sprite.bind().get_bone(self.bone_name.clone());
        bone
    }

    /// The `SpineSprite` this node is under, or null.
    #[func]
    fn find_sprite(&self) -> Option<Gd<SpineSprite>> {
        let mut parent = self.base().get_parent();
        while let Some(node) = parent {
            if let Ok(sprite) = node.clone().try_cast::<SpineSprite>() {
                return Some(sprite);
            }
            parent = node.get_parent();
        }
        None
    }
}

impl SpineBoneNode {
    /// Where this node is in the skeleton's space: its global transform seen from the sprite.
    fn placement(&self) -> Affine2 {
        let node = self.base().get_global_transform();
        let relative = match self.find_sprite() {
            Some(sprite) => sprite.get_global_transform().affine_inverse() * node,
            None => node,
        };
        follow::from_transform(relative)
    }

    /// Puts this node in the skeleton `parent`, replacing what it had before: a follower that
    /// moves the node with its bone, or in drive mode a driver that moves the bone with the node.
    /// `None` takes it out again. Called when the node enters or leaves the tree, when its
    /// settings change and by the sprite when it builds a new skeleton.
    pub fn attach_to(&mut self, parent: Option<Entity>) {
        if let Some(old) = self.follower.take() {
            follow::despawn(old);
        }
        self.base_mut().set_notify_transform(false);
        self.parent = parent;
        let Some(parent) = parent else { return };
        if !self.enabled {
            return;
        }
        if i64::from(self.bone_mode) == SpineConstant::BoneMode_Drive {
            let name = self.bone_name.to_string();
            let placement = self.placement();
            self.follower =
                follow::spawn_driver(parent, |data| SpineDriver::for_bone(data, &name, placement));
            // Godot tells the node when it moves, and the driver is told where it went.
            self.base_mut().set_notify_transform(true);
            return;
        }
        let name = self.bone_name.to_string();
        self.follower = follow::spawn(parent, &self.to_gd(), |skeleton| {
            SpineFollower::for_bone(skeleton, skeleton.data(), &name)
        });
    }
}
