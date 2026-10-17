//! `SpineSlot`: a slot of a sprite's skeleton.

use super::bone::SpineBone;
use super::slot_data::SpineSlotData;
use super::slot_pose::{self, SpineSlotPose};
use super::PoseKind;
use crate::resources::attachment::SpineAttachment;
use crate::runtime::store::Owner;
use crate::runtime::world;
use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::SlotId;
use spine2d_ecs::bevy_ecs::entity::Entity;

/// A slot of the skeleton: holds the shown attachment and its tint.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSlot {
    base: Base<RefCounted>,
    entity: Entity,
    slot: SlotId,
}

impl SpineSlot {
    pub fn create(entity: Entity, slot: SlotId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSlot { base, entity, slot })
    }

    /// The skeleton entity this slot belongs to.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// The slot's index in its skeleton.
    pub fn slot_id(&self) -> SlotId {
        self.slot
    }
}

#[godot_api]
impl SpineSlot {
    /// False once the sprite this slot came from was reloaded or freed. Reads then return zero
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    #[func]
    fn get_name(&self) -> GString {
        world::skeleton(self.entity, |skeleton| {
            GString::from(skeleton.data().slots[self.slot.index()].name.as_str())
        })
        .unwrap_or_default()
    }

    /// The setup description of this slot, shared by every sprite of the skeleton data.
    #[func]
    fn get_data(&self) -> Gd<SpineSlotData> {
        SpineSlotData::create(self.entity, self.slot)
    }

    /// The pose animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpineSlotPose> {
        SpineSlotPose::create(self.entity, self.slot, PoseKind::Pose)
    }

    /// The pose after constraints changed it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpineSlotPose> {
        SpineSlotPose::create(self.entity, self.slot, PoseKind::Applied)
    }

    /// Resets this slot to its setup pose: color, shown attachment and sequence frame.
    #[func]
    fn set_to_setup_pose(&mut self) {
        world::skeleton_mut(self.entity, |skeleton| {
            let data = skeleton.data().slots[self.slot.index()].clone();
            let attachment = skeleton.attachment_by_index(self.slot.index(), &data.attachment_name);
            skeleton.slot_mut(self.slot).setup_pose(&data, attachment);
        });
    }

    /// The bone this slot is attached to, or null once the sprite is gone.
    #[func]
    fn get_bone(&self) -> Option<Gd<SpineBone>> {
        let bone = world::skeleton(self.entity, |skeleton| skeleton.slot(self.slot).bone())?;
        Some(SpineBone::create(self.entity, bone))
    }

    /// Name of the shown attachment, or an empty string.
    #[func]
    fn get_attachment_name(&self) -> GString {
        world::skeleton(self.entity, |skeleton| {
            skeleton
                .slot(self.slot)
                .applied_pose()
                .attachment
                .as_ref()
                .map(|a| GString::from(a.name()))
                .unwrap_or_default()
        })
        .unwrap_or_default()
    }

    /// Shows the attachment called `name` from the active skin (empty hides). False if unknown.
    #[func]
    fn set_attachment(&mut self, name: GString) -> bool {
        world::skeleton_mut(self.entity, |skeleton| {
            let slot_name = skeleton.data().slots[self.slot.index()].name.clone();
            skeleton.set_attachment(&slot_name, &name.to_string())
        })
        .unwrap_or(false)
    }

    /// Shows `attachment` in this slot, whatever skin is active. It is only this sprite's slot that
    /// changes. Returns false if the sprite is gone, or if the attachment uses bones the skeleton
    /// does not have.
    ///
    /// It stays until something replaces it: `set_attachment`, an animation that keys this slot's
    /// attachment, or `SpineSprite.set_slots_to_setup_pose()`. The sprite keeps the attachment's
    /// textures alive until the slots are reset.
    #[func]
    fn set_attachment_object(&mut self, attachment: Gd<SpineAttachment>) -> bool {
        slot_pose::set_attachment_object(
            Owner::Skeleton(self.entity),
            self.slot,
            PoseKind::Pose,
            &attachment,
        )
    }

    /// The attachment this slot shows, or `null` if it shows none.
    #[func]
    fn get_attachment_object(&self) -> Option<Gd<SpineAttachment>> {
        let shown = world::skeleton(self.entity, |skeleton| {
            skeleton.slot(self.slot).applied_pose().attachment.clone()
        })??;
        let textures = world::sprite(self.entity)?.bind().kept_textures();
        Some(SpineAttachment::wrap(shown, textures))
    }

    #[func]
    fn get_color(&self) -> Color {
        world::skeleton(self.entity, |skeleton| {
            let c = skeleton.slot(self.slot).applied_pose().color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
        .unwrap_or_default()
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton
                .slot_mut(self.slot)
                .applied_pose_mut()
                .color
                .set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn has_dark_color(&self) -> bool {
        world::skeleton(self.entity, |skeleton| {
            skeleton.slot(self.slot).applied_pose().has_dark_color
        })
        .unwrap_or(false)
    }

    #[func]
    fn get_dark_color(&self) -> Color {
        world::skeleton(self.entity, |skeleton| {
            let c = skeleton.slot(self.slot).applied_pose().dark_color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
        .unwrap_or_default()
    }

    #[func]
    fn set_dark_color(&mut self, color: Color) {
        world::skeleton_mut(self.entity, |skeleton| {
            let pose = skeleton.slot_mut(self.slot).applied_pose_mut();
            pose.dark_color.set(color.r, color.g, color.b, color.a);
            pose.has_dark_color = true;
        });
    }
}
