//! `SpineSlotNode`: a `Node2D` child of a `SpineSprite` that draws at one slot's place in the
//! skeleton's draw order.

use crate::bone_node::{attach_to_parent_sprite, bone_transform, detach_from_sprite};
use crate::sprite::{SharedPlayer, SpineSprite};
use godot::classes::{INode2D, Node2D, RenderingServer};
use godot::obj::Singleton;
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyHintInfo, PropertyInfo};
use spine2d_core::SlotId;
use std::rc::Rc;

/// Draws its children between the slots of its parent `SpineSprite`: everything under this node
/// appears in front of the slots drawn before the chosen slot and behind the ones drawn after it,
/// and follows the bone the slot is attached to. Use it for a weapon that has to pass behind an
/// arm, or an effect that belongs between two layers.
///
/// Every slot node splits the sprite's meshes at its slot, which costs one extra draw call.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct SpineSlotNode {
    base: Base<Node2D>,
    /// Name of the slot to draw at.
    #[export]
    #[var(set = set_slot_name)]
    slot_name: GString,
    /// The player and slot resolved on an earlier frame; dropped when the sprite reloads.
    resolved: Option<(SharedPlayer, SlotId)>,
    connected_to: Option<Gd<SpineSprite>>,
    /// The draw index last given to the render server; `None` when Godot owns it.
    applied_index: Option<i32>,
}

#[godot_api]
impl INode2D for SpineSlotNode {
    fn init(base: Base<Node2D>) -> Self {
        SpineSlotNode {
            base,
            slot_name: GString::new(),
            resolved: None,
            connected_to: None,
            applied_index: None,
        }
    }

    fn on_validate_property(&self, property: &mut PropertyInfo) {
        if property.property_name != "slot_name" {
            return;
        }
        let names = self
            .base()
            .get_parent()
            .and_then(|parent| parent.try_cast::<SpineSprite>().ok())
            .and_then(|sprite| sprite.bind().skeleton_data())
            .map(|mut data| data.bind_mut().get_slot_names());
        let list: Vec<String> = names
            .unwrap_or_default()
            .to_vec()
            .iter()
            .map(|name| name.to_string())
            .collect();
        property.hint_info = PropertyHintInfo {
            hint: PropertyHint::ENUM_SUGGESTION,
            hint_string: GString::from(list.join(",").as_str()),
        };
    }

    #[func(gd_self)]
    fn enter_tree(mut this: Gd<Self>) {
        // Godot gave the item its tree index; ours has to be applied again.
        this.bind_mut().applied_index = None;
        let Some(mut sprite) = attach_to_parent_sprite(&this) else {
            return;
        };
        this.bind_mut().connected_to = Some(sprite.clone());
        sprite.bind_mut().register_slot_node(this.clone());
        this.bind_mut().follow();
    }

    #[func(gd_self)]
    fn exit_tree(mut this: Gd<Self>) {
        let sprite = this.bind_mut().connected_to.take();
        if let Some(mut sprite) = sprite.clone().filter(|s| s.is_instance_valid()) {
            sprite.bind_mut().unregister_slot_node(&this);
        }
        detach_from_sprite(&this, sprite);
    }
}

#[godot_api]
impl SpineSlotNode {
    #[func]
    fn set_slot_name(&mut self, name: GString) {
        self.slot_name = name;
        self.resolved = None;
        // The sprite has to redraw, and it must not be touched while this node is borrowed.
        self.base_mut().call_deferred("on_slot_changed", &[]);
    }

    #[func(gd_self)]
    fn on_slot_changed(mut this: Gd<Self>) {
        let sprite = this.bind().connected_to.clone();
        if let Some(mut sprite) = sprite.filter(|s| s.is_instance_valid()) {
            sprite.bind_mut().refresh_slot_nodes();
        }
        this.bind_mut().follow();
    }

    /// Called by the sprite after it computed world transforms.
    #[func]
    fn on_world_transforms_changed(&mut self) {
        self.follow();
    }
}

impl SpineSlotNode {
    /// The slot this node draws at in `player`, looked up by name and remembered until the sprite
    /// reloads.
    pub fn resolve(&mut self, player: &SharedPlayer) -> Option<SlotId> {
        let stale = self
            .resolved
            .as_ref()
            .is_none_or(|(cached, _)| !Rc::ptr_eq(cached, player));
        if stale {
            let id = player
                .borrow()
                .skeleton
                .find_slot(&self.slot_name.to_string());
            self.resolved = id.map(|id| (player.clone(), id));
        }
        self.resolved.as_ref().map(|(_, id)| *id)
    }

    /// Draws this node at `index` among the sprite's canvas items; `None` hands the position back
    /// to Godot (its place in the child list). Does nothing when nothing changed.
    pub fn set_draw_index(&mut self, index: Option<i32>) {
        if self.applied_index == index {
            return;
        }
        let item = self.base().get_canvas_item();
        let target = index.unwrap_or_else(|| self.base().get_index());
        RenderingServer::singleton().canvas_item_set_draw_index(item, target);
        self.applied_index = index;
    }

    /// Forgets the applied draw index, because Godot reset it (a sibling was added or moved).
    pub fn forget_draw_index(&mut self) {
        self.applied_index = None;
    }

    /// Copies the world transform of the slot's bone onto this node.
    fn follow(&mut self) {
        let Some(sprite) = self.connected_to.as_ref().filter(|s| s.is_instance_valid()) else {
            return;
        };
        let Some(player) = sprite.bind().player() else {
            return;
        };
        let Some(id) = self.resolve(&player) else {
            return;
        };
        let player = player.borrow();
        let bone = player.skeleton.slot(id).bone();
        let pose = *player.skeleton.bone(bone).applied_pose();
        drop(player);
        self.base_mut().set_transform(bone_transform(&pose));
    }
}
