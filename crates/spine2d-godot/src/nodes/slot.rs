//! `SpineSlotNode`: a `Node2D` child of a `SpineSprite` that draws at one slot's place in the
//! skeleton's draw order.

use crate::nodes::bone::parent_entity;
use crate::nodes::sprite::SpineSprite;
use crate::runtime::world;
use crate::systems::follow;
use crate::systems::render::view::{SlotItem, SpineView};
use godot::classes::{INode2D, Material, Node2D};
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyHintInfo, PropertyInfo};
use spine2d_core::SlotId;
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::SpineFollower;

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
    /// The skeleton and slot resolved on an earlier frame; dropped when the sprite reloads.
    resolved: Option<(Entity, SlotId)>,
    /// The entity in the world that tracks the slot's bone for this node.
    follower: Option<Entity>,
    /// The skeleton whose view holds this node's slot item, while the node is in the tree.
    attached: Option<Entity>,
    /// The material for the meshes of this node's slot that blend normally.
    #[export_group(name = "Materials")]
    #[export]
    #[var(set = set_normal_material)]
    normal_material: Option<Gd<Material>>,
    /// The material for the meshes of this node's slot that blend additively.
    #[export]
    #[var(set = set_additive_material)]
    additive_material: Option<Gd<Material>>,
    /// The material for the meshes of this node's slot that blend by multiplying.
    #[export]
    #[var(set = set_multiply_material)]
    multiply_material: Option<Gd<Material>>,
    /// The material for the meshes of this node's slot that blend as screen.
    #[export]
    #[var(set = set_screen_material)]
    screen_material: Option<Gd<Material>>,
}

#[godot_api]
impl INode2D for SpineSlotNode {
    fn init(base: Base<Node2D>) -> Self {
        SpineSlotNode {
            base,
            slot_name: GString::new(),
            resolved: None,
            follower: None,
            attached: None,
            normal_material: None,
            additive_material: None,
            multiply_material: None,
            screen_material: None,
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
        let entity = parent_entity(&this);
        this.bind_mut().attach_to(entity);
    }

    #[func(gd_self)]
    fn exit_tree(mut this: Gd<Self>) {
        this.bind_mut().attach_to(None);
    }
}

#[godot_api]
impl SpineSlotNode {
    #[func]
    fn set_normal_material(&mut self, material: Option<Gd<Material>>) {
        self.normal_material = material;
        self.refresh_order();
    }

    #[func]
    fn set_additive_material(&mut self, material: Option<Gd<Material>>) {
        self.additive_material = material;
        self.refresh_order();
    }

    #[func]
    fn set_multiply_material(&mut self, material: Option<Gd<Material>>) {
        self.multiply_material = material;
        self.refresh_order();
    }

    #[func]
    fn set_screen_material(&mut self, material: Option<Gd<Material>>) {
        self.screen_material = material;
        self.refresh_order();
    }

    #[func]
    fn set_slot_name(&mut self, name: GString) {
        self.slot_name = name;
        self.resolved = None;
        if let Some(entity) = self.attached {
            self.attach_to(Some(entity));
        }
    }
}

impl SpineSlotNode {
    /// The slot this node draws at in the skeleton `entity`, looked up by name and remembered
    /// until the sprite reloads.
    fn resolve(&mut self, entity: Entity) -> Option<SlotId> {
        let stale = self.resolved.is_none_or(|(cached, _)| cached != entity);
        if stale {
            let id = world::skeleton(entity, |skeleton| {
                skeleton.find_slot(&self.slot_name.to_string())
            })
            .flatten();
            self.resolved = id.map(|id| (entity, id));
        }
        self.resolved.map(|(_, id)| id)
    }

    /// Puts this node in the skeleton `entity`: it follows the bone of its slot and asks the
    /// render to cut the meshes after the slot. `None` takes it out again. Called when the node
    /// enters or leaves the tree and by the sprite when it builds a new skeleton.
    pub fn attach_to(&mut self, entity: Option<Entity>) {
        if let Some(old) = self.follower.take() {
            follow::despawn(old);
        }
        if let Some(old) = self.attached.take() {
            let item = self.base().get_canvas_item();
            world::write::<SpineView, _>(old, |view| view.remove_slot_item(item));
        }
        let Some(entity) = entity else { return };
        let name = self.slot_name.to_string();
        self.follower = follow::spawn(entity, &self.to_gd(), |skeleton| {
            SpineFollower::for_slot(skeleton, skeleton.data(), &name)
        });
        self.attached = Some(entity);
        self.publish(entity);
    }

    /// Gives the render this node's canvas item, slot and place in the child list. A slot the
    /// skeleton does not have leaves the node an ordinary child.
    fn publish(&mut self, entity: Entity) {
        let Some(slot) = self.resolve(entity) else {
            return;
        };
        let item = SlotItem {
            item: self.base().get_canvas_item(),
            slot,
            fallback: self.base().get_index(),
            materials: [
                &self.normal_material,
                &self.additive_material,
                &self.multiply_material,
                &self.screen_material,
            ]
            .map(|material| material.as_ref().map_or(Rid::Invalid, |m| m.get_rid())),
        };
        world::write::<SpineView, _>(entity, |view| view.set_slot_item(item));
    }

    /// Publishes this node again, because the child list changed and Godot reset its draw index.
    pub fn refresh_order(&mut self) {
        if let Some(entity) = self.attached {
            self.publish(entity);
        }
    }
}
