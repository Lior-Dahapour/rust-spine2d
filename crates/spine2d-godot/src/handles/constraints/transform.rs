//! The transform constraint classes: `SpineTransformConstraint`, its pose and its data.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BoneId, TransformConstraint, TransformConstraintData, TransformConstraintPose};
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::{
    edit_structure, own_bone, read_body, read_data, read_pose, write_pose, ConstraintHandle,
};
use crate::handles::bone::{SpineBone, SpineBoneData, SpineBonePose};
use crate::handles::{warn_not_implemented, PoseKind, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// A transform constraint of a skeleton: copies properties of a source bone onto other bones.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTransformConstraint {
    base: Base<RefCounted>,
    entity: Entity,
    index: usize,
}

impl SpineTransformConstraint {
    /// A handle to constraint `index` of the skeleton `entity`.
    pub fn create(entity: Entity, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTransformConstraint {
            base,
            entity,
            index,
        })
    }
}

impl ConstraintHandle for SpineTransformConstraint {
    fn owner(&self) -> Owner {
        Owner::Skeleton(self.entity)
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineTransformConstraint {
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
    fn get_data(&self) -> Gd<SpineTransformConstraintData> {
        SpineTransformConstraintData::create(self.entity, self.index)
    }

    /// The bones the constraint moves. Each entry is the bone's applied pose.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBonePose>> {
        self.bones()
    }

    /// The bone the properties are copied from, or null once the sprite is gone.
    #[func]
    fn get_source(&self) -> Option<Gd<SpineBone>> {
        let source = read_body(
            self.entity,
            self.index,
            |constraint: &TransformConstraint| constraint.source(),
        )?;
        Some(SpineBone::create(self.entity, source))
    }

    /// Copies the properties from `source`, a bone of the same skeleton, from now on. The
    /// skeleton's update order is rebuilt at the next frame.
    #[func]
    fn set_source(&mut self, source: Option<Gd<SpineBone>>) {
        let Some(source) = source.and_then(|bone| own_bone(self.entity, &bone)) else {
            return;
        };
        edit_structure(
            self.entity,
            self.index,
            |constraint: &mut TransformConstraint| constraint.set_source(source),
        );
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpineTransformConstraintPose> {
        SpineTransformConstraintPose::create(self.entity, self.index, PoseKind::Pose)
    }

    /// The pose after other constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpineTransformConstraintPose> {
        SpineTransformConstraintPose::create(self.entity, self.index, PoseKind::Applied)
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

/// The values of a transform constraint that animations key: how much of each property is mixed.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTransformConstraintPose {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
    kind: PoseKind,
}

impl SpineTransformConstraintPose {
    /// A handle to the pose `kind` of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTransformConstraintPose {
            base,
            owner: owner.into(),
            index,
            kind,
        })
    }

    fn read(&self, f: impl FnOnce(&TransformConstraintPose) -> f32) -> f32 {
        read_pose(self.owner, self.index, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut TransformConstraintPose)) {
        write_pose(self.owner, self.index, self.kind, f);
    }
}

#[godot_api]
impl SpineTransformConstraintPose {
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

    #[func]
    fn get_mix_scale_x(&self) -> f32 {
        self.read(|pose| pose.mix_scale_x)
    }

    #[func]
    fn set_mix_scale_x(&mut self, value: f32) {
        self.write(|pose| pose.mix_scale_x = value);
    }

    #[func]
    fn get_mix_scale_y(&self) -> f32 {
        self.read(|pose| pose.mix_scale_y)
    }

    #[func]
    fn set_mix_scale_y(&mut self, value: f32) {
        self.write(|pose| pose.mix_scale_y = value);
    }

    #[func]
    fn get_mix_shear_y(&self) -> f32 {
        self.read(|pose| pose.mix_shear_y)
    }

    #[func]
    fn set_mix_shear_y(&mut self, value: f32) {
        self.write(|pose| pose.mix_shear_y = value);
    }
}

/// The setup values of a transform constraint, from the skeleton data. The data is shared by every
/// sprite that uses it, so the setters change it for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTransformConstraintData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpineTransformConstraintData {
    /// A handle to the data of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTransformConstraintData {
            base,
            owner: owner.into(),
            index,
        })
    }

    fn offset(&self, which: usize) -> f32 {
        read_data(self.owner, self.index, |data: &TransformConstraintData| {
            data.offsets[which]
        })
        .unwrap_or_default()
    }

    fn flag(&self, f: impl FnOnce(&TransformConstraintData) -> bool) -> bool {
        read_data(self.owner, self.index, f).unwrap_or_default()
    }
}

impl ConstraintHandle for SpineTransformConstraintData {
    fn owner(&self) -> Owner {
        self.owner
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineTransformConstraintData {
    #[func]
    fn get_constraint_name(&self) -> GString {
        self.constraint_name()
    }

    #[func]
    fn is_skin_required(&self) -> bool {
        self.skin_required()
    }

    /// The bones the constraint moves.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBoneData>> {
        self.bone_data()
    }

    /// The bone the properties are copied from.
    #[func]
    fn get_source(&self) -> Option<Gd<SpineBoneData>> {
        let source = read_data(self.owner, self.index, |data: &TransformConstraintData| {
            data.source
        })?;
        Some(SpineBoneData::create(self.owner, BoneId(source)))
    }

    #[func]
    fn get_offset_rotation(&self) -> f32 {
        self.offset(TransformConstraintData::ROTATION)
    }

    #[func]
    fn get_offset_x(&self) -> f32 {
        self.offset(TransformConstraintData::X)
    }

    #[func]
    fn get_offset_y(&self) -> f32 {
        self.offset(TransformConstraintData::Y)
    }

    #[func]
    fn get_offset_scale_x(&self) -> f32 {
        self.offset(TransformConstraintData::SCALE_X)
    }

    #[func]
    fn get_offset_scale_y(&self) -> f32 {
        self.offset(TransformConstraintData::SCALE_Y)
    }

    #[func]
    fn get_offset_shear_y(&self) -> f32 {
        self.offset(TransformConstraintData::SHEAR_Y)
    }

    /// Whether the source bone's local values are copied instead of its world values.
    #[func]
    fn is_local_source(&self) -> bool {
        self.flag(|data| data.local_source)
    }

    /// Whether the values are applied to the constrained bones' local pose.
    #[func]
    fn is_local_target(&self) -> bool {
        self.flag(|data| data.local_target)
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpineTransformConstraintPose> {
        SpineTransformConstraintPose::create(self.owner, self.index, PoseKind::Setup)
    }
}
