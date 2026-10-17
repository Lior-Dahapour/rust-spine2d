//! `SpineBoneData`: the setup description of a bone, shared by every sprite of a skeleton file.
//!
//! The setters change the shared skeleton data, so every sprite made from it sees the new value.
//! A running skeleton uses it the next time it applies its setup pose; `set_skin_required` also
//! makes every such skeleton rebuild its update order at the next frame.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BoneData, BoneId};

use super::pose::SpineBonePose;
use crate::handles::PoseKind;
use crate::runtime::store::Owner;
use crate::runtime::world;

/// The setup values of a bone: its name, length, parent, color and setup pose, as authored in the
/// skeleton file. The data is shared by every sprite that uses it, so the setters change it for
/// all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineBoneData {
    base: Base<RefCounted>,
    owner: Owner,
    bone: BoneId,
}

impl SpineBoneData {
    /// A handle to the data of `bone` of `owner`: a skeleton in the world, or the stored data.
    pub fn create(owner: impl Into<Owner>, bone: BoneId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineBoneData {
            base,
            owner: owner.into(),
            bone,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&BoneData) -> R) -> R {
        world::owner_data(self.owner, |data| f(&data.bones[self.bone.index()])).unwrap_or_default()
    }

    /// Changes the bone's data; sprites keep their update order.
    fn edit(&self, f: impl FnOnce(&mut BoneData)) {
        world::edit_data(self.owner, |data| f(&mut data.bones[self.bone.index()]));
    }

    /// Changes the bone's data in a way that affects the update order, which every sprite made
    /// from the data rebuilds at the next frame.
    fn edit_structure(&self, f: impl FnOnce(&mut BoneData)) {
        world::edit_data_structure(self.owner, |data| f(&mut data.bones[self.bone.index()]));
    }

    /// What this handle belongs to.
    pub fn owner(&self) -> Owner {
        self.owner
    }

    /// The bone this handle describes.
    pub fn bone_id(&self) -> BoneId {
        self.bone
    }
}

#[godot_api]
impl SpineBoneData {
    /// False once the sprite this data came from was reloaded or freed. Reads then return zero
    /// values.
    #[func]
    fn is_valid(&self) -> bool {
        world::owner_data(self.owner, |_| ()).is_some()
    }

    /// The bone's index in the skeleton, parents before children.
    #[func]
    fn get_index(&self) -> i32 {
        self.read(|data| data.index as i32)
    }

    #[func]
    fn get_bone_name(&self) -> GString {
        self.read(|data| GString::from(data.name.as_str()))
    }

    /// The data of the parent bone, or null for the root.
    #[func]
    fn get_parent(&self) -> Option<Gd<SpineBoneData>> {
        let parent = self.read(|data| data.parent);
        Some(SpineBoneData::create(self.owner, BoneId(parent?)))
    }

    #[func]
    fn get_length(&self) -> f32 {
        self.read(|data| data.length)
    }

    #[func]
    fn set_length(&mut self, length: f32) {
        self.edit(|data| data.length = length);
    }

    /// The pose the bone starts from. Its world values read as zero.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpineBonePose> {
        SpineBonePose::create(self.owner, self.bone, PoseKind::Setup)
    }

    /// Whether the bone only takes part while the active skin includes it.
    #[func]
    fn is_skin_required(&self) -> bool {
        self.read(|data| data.skin_required)
    }

    #[func]
    fn set_skin_required(&mut self, skin_required: bool) {
        self.edit_structure(|data| data.skin_required = skin_required);
    }

    /// The color the Spine editor draws the bone with.
    #[func]
    fn get_color(&self) -> Color {
        self.read(|data| Color::from_rgba(data.color.r, data.color.g, data.color.b, data.color.a))
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        self.edit(|data| data.color = spine2d_core::Color::new(color.r, color.g, color.b, color.a));
    }

    /// The bone's icon name in the Spine editor; empty if it was not exported.
    #[func]
    fn get_icon(&self) -> GString {
        self.read(|data| GString::from(data.icon.as_str()))
    }

    /// Whether the Spine editor shows the bone.
    #[func]
    fn is_visible(&self) -> bool {
        self.read(|data| data.visible)
    }

    #[func]
    fn set_visible(&mut self, visible: bool) {
        self.edit(|data| data.visible = visible);
    }
}
