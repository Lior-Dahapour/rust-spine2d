//! What the render system needs to know about a sprite's node. The node writes it when something
//! changes, so the system reads components and never asks the node.

use bevy_ecs::component::Component;
use godot::builtin::Rid;
use smallvec::SmallVec;
use spine2d_core::{BlendMode, SlotId};

use crate::systems::render::debug::DebugFlags;

/// A slot node asking for the skeleton's meshes to be cut after its slot, so its content can be
/// drawn in the gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotItem {
    /// The node's canvas item on the render server, which also identifies the node.
    pub item: Rid,
    /// The slot to cut after.
    pub slot: SlotId,
    /// Where Godot would draw the node by its place in the child list, for when its slot draws
    /// nothing.
    pub fallback: i32,
    /// The materials the node gives its slot's meshes, by blend mode; `Rid::Invalid` for none.
    pub materials: [Rid; 4],
}

impl SlotItem {
    /// Whether the node gives its slot's meshes a material of its own.
    pub fn has_materials(&self) -> bool {
        self.materials
            .iter()
            .any(|material| *material != Rid::Invalid)
    }
}

/// The state of a sprite that decides whether, and how, its skeleton is drawn.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SpineView {
    /// The sprite's own canvas item, the parent of the meshes.
    canvas_item: Rid,
    /// The node is in the tree and visible, itself and every ancestor.
    pub(crate) visible: bool,
    /// The render server reports the sprite inside a viewport.
    pub(crate) on_screen: bool,
    /// Skip drawing while the sprite is off screen.
    pub(crate) cull: bool,
    /// Which debug overlays to draw.
    debug: DebugFlags,
    /// The slot nodes of the sprite. Most sprites have none.
    slot_items: SmallVec<[SlotItem; 2]>,
    /// Counts the changes that make Godot forget the draw indices it was given: the slot nodes
    /// changing and the child list changing.
    order_epoch: u32,
    /// The materials the sprite draws its meshes with, by blend mode; `Rid::Invalid` for the
    /// built-in one.
    materials: [Rid; 4],
}

impl SpineView {
    /// A view of a sprite that is on screen.
    pub fn new(canvas_item: Rid, visible: bool, cull: bool, debug: DebugFlags) -> Self {
        SpineView {
            canvas_item,
            visible,
            on_screen: true,
            cull,
            debug,
            slot_items: SmallVec::new(),
            order_epoch: 0,
            materials: [Rid::Invalid; 4],
        }
    }

    /// The material the sprite draws its meshes of `blend_mode` with, if it has its own.
    pub fn material(&self, blend_mode: BlendMode) -> Option<Rid> {
        Some(self.materials[blend_mode as usize]).filter(|material| *material != Rid::Invalid)
    }

    /// Chooses the materials of the sprite, by blend mode.
    pub fn set_materials(&mut self, materials: [Rid; 4]) {
        self.materials = materials;
    }

    /// The sprite's own canvas item.
    pub fn canvas_item(&self) -> Rid {
        self.canvas_item
    }

    /// Which debug overlays to draw.
    pub fn debug(&self) -> &DebugFlags {
        &self.debug
    }

    /// Chooses which debug overlays to draw.
    pub fn set_debug(&mut self, debug: DebugFlags) {
        self.debug = debug;
    }

    /// The slot nodes of the sprite.
    pub fn slot_items(&self) -> &[SlotItem] {
        &self.slot_items
    }

    /// See [`SpineView::reorder`]: changes whenever the draw indices have to be given again.
    pub fn order_epoch(&self) -> u32 {
        self.order_epoch
    }

    /// Adds a slot node, or replaces the entry of the one with the same canvas item.
    pub fn set_slot_item(&mut self, new: SlotItem) {
        match self.slot_items.iter_mut().find(|old| old.item == new.item) {
            Some(old) => *old = new,
            None => self.slot_items.push(new),
        }
        self.reorder();
    }

    /// Removes the slot node with the canvas item `item`, if it has an entry.
    pub fn remove_slot_item(&mut self, item: Rid) {
        self.slot_items.retain(|old| old.item != item);
        self.reorder();
    }

    /// Notes that Godot reset the draw indices, so the render has to give them again.
    pub fn reorder(&mut self) {
        self.order_epoch = self.order_epoch.wrapping_add(1);
    }

    /// Whether the skeleton has to be drawn now.
    pub fn is_drawn(&self) -> bool {
        self.visible && (!self.cull || self.on_screen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_sprite_is_never_drawn() {
        let mut view = SpineView::new(Rid::Invalid, false, false, DebugFlags::default());
        assert!(!view.is_drawn());
        view.on_screen = true;
        assert!(!view.is_drawn());
    }

    #[test]
    fn an_off_screen_sprite_is_drawn_only_when_culling_is_off() {
        let mut view = SpineView::new(Rid::Invalid, true, true, DebugFlags::default());
        view.on_screen = false;
        assert!(!view.is_drawn());

        view.cull = false;
        assert!(view.is_drawn());
    }

    fn slot_item(item: u64, slot: usize) -> SlotItem {
        SlotItem {
            item: Rid::new(item),
            slot: SlotId(slot),
            fallback: 0,
            materials: [Rid::Invalid; 4],
        }
    }

    #[test]
    fn a_slot_item_has_materials_only_when_one_is_set() {
        let mut item = slot_item(1, 0);
        assert!(!item.has_materials());

        item.materials[1] = Rid::new(9);

        assert!(item.has_materials());
    }

    #[test]
    fn a_sprite_has_a_material_for_a_blend_mode_only_when_it_was_set() {
        let mut view = SpineView::new(Rid::Invalid, true, true, DebugFlags::default());
        assert_eq!(view.material(BlendMode::Additive), None);

        view.set_materials([Rid::Invalid, Rid::new(7), Rid::Invalid, Rid::Invalid]);

        assert_eq!(view.material(BlendMode::Additive), Some(Rid::new(7)));
        assert_eq!(view.material(BlendMode::Normal), None);
    }

    #[test]
    fn a_slot_item_is_added_replaced_and_removed_by_its_canvas_item() {
        let mut view = SpineView::new(Rid::Invalid, true, true, DebugFlags::default());

        view.set_slot_item(slot_item(1, 4));
        view.set_slot_item(slot_item(2, 5));
        view.set_slot_item(slot_item(1, 6));

        assert_eq!(view.slot_items().len(), 2);
        assert_eq!(view.slot_items()[0].slot, SlotId(6));
        view.remove_slot_item(Rid::new(1));
        assert_eq!(view.slot_items(), &[slot_item(2, 5)]);
    }

    #[test]
    fn every_change_to_the_slot_items_bumps_the_epoch() {
        let mut view = SpineView::new(Rid::Invalid, true, true, DebugFlags::default());
        let start = view.order_epoch();

        view.set_slot_item(slot_item(1, 4));
        view.reorder();
        view.remove_slot_item(Rid::new(1));

        assert_eq!(view.order_epoch(), start.wrapping_add(3));
    }

    #[test]
    fn a_visible_on_screen_sprite_is_drawn() {
        assert!(SpineView::new(Rid::Invalid, true, true, DebugFlags::default()).is_drawn());
    }
}
