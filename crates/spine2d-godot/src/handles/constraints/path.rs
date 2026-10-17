//! The path constraint classes: `SpinePathConstraint`, its pose and its data.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{
    PathConstraint, PathConstraintData, PathConstraintPose, PositionMode, RotateMode, SlotId,
    SpacingMode,
};
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::{
    edit_data, edit_data_structure, edit_structure, mode_index, own_slot, own_slot_data, read_body,
    read_data, read_pose, write_pose, ConstraintHandle,
};
use crate::handles::bone::{SpineBoneData, SpineBonePose};
use crate::handles::slot::SpineSlot;
use crate::handles::slot_data::SpineSlotData;
use crate::handles::{warn_not_implemented, PoseKind, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// A path constraint of a skeleton: places bones along a path attachment.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePathConstraint {
    base: Base<RefCounted>,
    entity: Entity,
    index: usize,
}

impl SpinePathConstraint {
    /// A handle to constraint `index` of the skeleton `entity`.
    pub fn create(entity: Entity, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePathConstraint {
            base,
            entity,
            index,
        })
    }
}

impl ConstraintHandle for SpinePathConstraint {
    fn owner(&self) -> Owner {
        Owner::Skeleton(self.entity)
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpinePathConstraint {
    /// False once the sprite this constraint came from was reloaded or freed. Reads then return
    /// zero values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    /// Not implemented yet: logs a warning.
    #[func]
    fn update(&mut self, _skeleton: Option<Gd<SpineSkeleton>>) {
        warn_not_implemented("update");
    }

    #[func]
    fn get_data(&self) -> Gd<SpinePathConstraintData> {
        SpinePathConstraintData::create(self.entity, self.index)
    }

    /// The bones placed along the path. Each entry is the bone's applied pose.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBonePose>> {
        self.bones()
    }

    /// The slot that holds the path attachment, or null once the sprite is gone.
    #[func]
    fn get_slot(&self) -> Option<Gd<SpineSlot>> {
        let slot = read_body(self.entity, self.index, |constraint: &PathConstraint| {
            constraint.slot()
        })?;
        Some(SpineSlot::create(self.entity, slot))
    }

    /// Follows the path attachment of `slot`, a slot of the same skeleton, from now on. The
    /// skeleton's update order is rebuilt at the next frame.
    #[func]
    fn set_slot(&mut self, slot: Option<Gd<SpineSlot>>) {
        let Some(slot) = slot.and_then(|slot| own_slot(self.entity, &slot)) else {
            return;
        };
        edit_structure(
            self.entity,
            self.index,
            |constraint: &mut PathConstraint| {
                constraint.set_slot(slot);
            },
        );
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpinePathConstraintPose> {
        SpinePathConstraintPose::create(self.entity, self.index, PoseKind::Pose)
    }

    /// The pose after other constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpinePathConstraintPose> {
        SpinePathConstraintPose::create(self.entity, self.index, PoseKind::Applied)
    }

    /// Whether the constraint takes part in the update. The skeleton sets this from the active
    /// skin.
    #[func]
    fn is_active(&self) -> bool {
        self.constraint_active()
    }

    /// Sets the flag [`is_active`] reports. The skeleton recomputes it whenever the skin changes,
    /// and the update does not read it, so this does not turn the constraint off.
    #[func]
    fn set_active(&mut self, active: bool) {
        self.set_constraint_active(active);
    }
}

/// The values of a path constraint that animations key: position along the path, spacing and the
/// mix of each property.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePathConstraintPose {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
    kind: PoseKind,
}

impl SpinePathConstraintPose {
    /// A handle to the pose `kind` of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePathConstraintPose {
            base,
            owner: owner.into(),
            index,
            kind,
        })
    }

    fn read(&self, f: impl FnOnce(&PathConstraintPose) -> f32) -> f32 {
        read_pose(self.owner, self.index, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut PathConstraintPose)) {
        write_pose(self.owner, self.index, self.kind, f);
    }
}

#[godot_api]
impl SpinePathConstraintPose {
    /// Where along the path the first bone sits; its unit depends on the position mode.
    #[func]
    fn get_position(&self) -> f32 {
        self.read(|pose| pose.position)
    }

    #[func]
    fn set_position(&mut self, value: f32) {
        self.write(|pose| pose.position = value);
    }

    /// The distance between bones along the path; its unit depends on the spacing mode.
    #[func]
    fn get_spacing(&self) -> f32 {
        self.read(|pose| pose.spacing)
    }

    #[func]
    fn set_spacing(&mut self, value: f32) {
        self.write(|pose| pose.spacing = value);
    }

    #[func]
    fn get_mix_rotate(&self) -> f32 {
        self.read(|pose| pose.mix_rotate)
    }

    #[func]
    fn set_mix_rotate(&mut self, value: f32) {
        self.write(|pose| pose.mix_rotate = value);
    }

    #[func]
    fn get_mix_x(&self) -> f32 {
        self.read(|pose| pose.mix_x)
    }

    #[func]
    fn set_mix_x(&mut self, value: f32) {
        self.write(|pose| pose.mix_x = value);
    }

    #[func]
    fn get_mix_y(&self) -> f32 {
        self.read(|pose| pose.mix_y)
    }

    #[func]
    fn set_mix_y(&mut self, value: f32) {
        self.write(|pose| pose.mix_y = value);
    }
}

/// The setup values of a path constraint, from the skeleton data. The data is shared by every
/// sprite that uses it, so the setters change it for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePathConstraintData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpinePathConstraintData {
    /// A handle to the data of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePathConstraintData {
            base,
            owner: owner.into(),
            index,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&PathConstraintData) -> R) -> R {
        read_data(self.owner, self.index, f).unwrap_or_default()
    }
}

impl ConstraintHandle for SpinePathConstraintData {
    fn owner(&self) -> Owner {
        self.owner
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpinePathConstraintData {
    #[func]
    fn get_constraint_name(&self) -> GString {
        self.constraint_name()
    }

    #[func]
    fn is_skin_required(&self) -> bool {
        self.skin_required()
    }

    /// The bones placed along the path.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBoneData>> {
        self.bone_data()
    }

    /// The data of the slot whose path attachment the bones follow.
    #[func]
    fn get_slot(&self) -> Option<Gd<SpineSlotData>> {
        let slot = self.read(|data| Some(data.slot))?;
        Some(SpineSlotData::create(self.owner, SlotId(slot)))
    }

    /// Sets the slot whose path attachment the bones follow. The slot data has to come from the
    /// same skeleton data.
    #[func]
    fn set_slot(&mut self, slot: Option<Gd<SpineSlotData>>) {
        let Some(slot) = slot else {
            godot_warn!("Spine: a path constraint needs a slot; not changed");
            return;
        };
        let Some(slot) = own_slot_data(self.owner, &slot) else {
            return;
        };
        edit_data_structure::<PathConstraintData>(self.owner, self.index, |data| {
            data.slot = slot.index();
        });
    }

    /// How the position is measured: one of `SpineConstant.PositionMode_*`.
    #[func]
    fn get_position_mode(&self) -> i32 {
        self.read(|data| data.position_mode as i32)
    }

    #[func]
    fn set_position_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 2) else {
            return;
        };
        edit_data::<PathConstraintData>(self.owner, self.index, |data| {
            data.position_mode = PositionMode::from_index(mode);
        });
    }

    /// How the spacing is measured: one of `SpineConstant.SpacingMode_*`.
    #[func]
    fn get_spacing_mode(&self) -> i32 {
        self.read(|data| data.spacing_mode as i32)
    }

    #[func]
    fn set_spacing_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 4) else {
            return;
        };
        edit_data::<PathConstraintData>(self.owner, self.index, |data| {
            data.spacing_mode = SpacingMode::from_index(mode);
        });
    }

    /// How the bones rotate along the path: one of `SpineConstant.RotateMode_*`.
    #[func]
    fn get_rotate_mode(&self) -> i32 {
        self.read(|data| data.rotate_mode as i32)
    }

    #[func]
    fn set_rotate_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 3) else {
            return;
        };
        edit_data::<PathConstraintData>(self.owner, self.index, |data| {
            data.rotate_mode = RotateMode::from_index(mode);
        });
    }

    #[func]
    fn get_offset_rotation(&self) -> f32 {
        self.read(|data| data.offset_rotation)
    }

    #[func]
    fn set_offset_rotation(&mut self, value: f32) {
        edit_data::<PathConstraintData>(self.owner, self.index, |data| {
            data.offset_rotation = value;
        });
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpinePathConstraintPose> {
        SpinePathConstraintPose::create(self.owner, self.index, PoseKind::Setup)
    }
}
