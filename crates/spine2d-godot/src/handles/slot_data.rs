//! `SpineSlotData`: the setup description of a slot, shared by every sprite of a skeleton file.
//!
//! The setters change the shared skeleton data, so every sprite made from it sees the new value.
//! A running skeleton uses it the next time it applies its setup pose.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BlendMode, SlotData, SlotId};

use super::bone::SpineBoneData;
use super::constraints::mode_index;
use super::slot_pose::SpineSlotPose;
use super::PoseKind;
use crate::runtime::store::Owner;
use crate::runtime::world;

/// The setup values of a slot: its name, bone, colors, attachment name and blend mode, as authored
/// in the skeleton file. The data is shared by every sprite that uses it, so the setters change it
/// for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSlotData {
    base: Base<RefCounted>,
    owner: Owner,
    slot: SlotId,
}

impl SpineSlotData {
    /// A handle to the data of `slot` of `owner`: a skeleton in the world, or the stored data.
    pub fn create(owner: impl Into<Owner>, slot: SlotId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSlotData {
            base,
            owner: owner.into(),
            slot,
        })
    }

    /// What this handle belongs to.
    pub fn owner(&self) -> Owner {
        self.owner
    }

    /// The slot this handle describes.
    pub fn slot_id(&self) -> SlotId {
        self.slot
    }

    fn read<R: Default>(&self, f: impl FnOnce(&SlotData) -> R) -> R {
        world::owner_data(self.owner, |data| f(&data.slots[self.slot.index()])).unwrap_or_default()
    }

    fn edit(&self, f: impl FnOnce(&mut SlotData)) {
        world::edit_data(self.owner, |data| f(&mut data.slots[self.slot.index()]));
    }
}

#[godot_api]
impl SpineSlotData {
    /// False once the sprite this data came from was reloaded or freed. Reads then return zero
    /// values.
    #[func]
    fn is_valid(&self) -> bool {
        world::owner_data(self.owner, |_| ()).is_some()
    }

    /// The slot's index in the skeleton, in setup draw order.
    #[func]
    fn get_index(&self) -> i32 {
        self.slot.index() as i32
    }

    #[func]
    fn get_name(&self) -> GString {
        self.read(|data| GString::from(data.name.as_str()))
    }

    /// The data of the bone the slot is attached to.
    #[func]
    fn get_bone_data(&self) -> Option<Gd<SpineBoneData>> {
        let bone = self.read(|data| Some(data.bone))?;
        Some(SpineBoneData::create(self.owner, bone))
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpineSlotPose> {
        SpineSlotPose::create(self.owner, self.slot, PoseKind::Setup)
    }

    #[func]
    fn get_color(&self) -> Color {
        self.read(|data| {
            let c = data.setup_pose.color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        self.edit(|data| {
            data.setup_pose
                .color
                .set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn get_dark_color(&self) -> Color {
        self.read(|data| {
            let c = data.setup_pose.dark_color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
    }

    #[func]
    fn set_dark_color(&mut self, color: Color) {
        self.edit(|data| {
            data.setup_pose
                .dark_color
                .set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn has_dark_color(&self) -> bool {
        self.read(|data| data.setup_pose.has_dark_color)
    }

    #[func]
    fn set_has_dark_color(&mut self, has_dark_color: bool) {
        self.edit(|data| data.setup_pose.has_dark_color = has_dark_color);
    }

    /// The name of the attachment the slot shows in the setup pose, or an empty string.
    #[func]
    fn get_attachment_name(&self) -> GString {
        self.read(|data| GString::from(data.attachment_name.as_str()))
    }

    #[func]
    fn set_attachment_name(&mut self, name: GString) {
        self.edit(|data| data.attachment_name = name.to_string().into());
    }

    /// How the slot blends with what is behind it: one of `SpineConstant.BlendMode_*`.
    #[func]
    fn get_blend_mode(&self) -> i32 {
        self.read(|data| data.blend_mode as i32)
    }

    #[func]
    fn set_blend_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 4) else {
            return;
        };
        self.edit(|data| data.blend_mode = BlendMode::from_index(usize::from(mode)));
    }
}
