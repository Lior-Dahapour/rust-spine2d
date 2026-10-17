//! The IK constraint classes: `SpineIkConstraint`, its pose and its data.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BoneId, IkConstraint, IkConstraintData, IkConstraintPose, ScaleYMode};
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::{
    edit_data, edit_data_structure, edit_structure, mode_index, own_bone, own_bone_data, read_body,
    read_data, read_pose, write_pose, ConstraintHandle,
};
use crate::handles::bone::{SpineBone, SpineBoneData, SpineBonePose};
use crate::handles::{warn_not_implemented, PoseKind, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// An IK constraint of a skeleton: bends one or two bones so the tip reaches a target bone.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineIkConstraint {
    base: Base<RefCounted>,
    entity: Entity,
    index: usize,
}

impl SpineIkConstraint {
    /// A handle to constraint `index` of the skeleton `entity`.
    pub fn create(entity: Entity, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineIkConstraint {
            base,
            entity,
            index,
        })
    }
}

impl ConstraintHandle for SpineIkConstraint {
    fn owner(&self) -> Owner {
        Owner::Skeleton(self.entity)
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineIkConstraint {
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
    fn get_data(&self) -> Gd<SpineIkConstraintData> {
        SpineIkConstraintData::create(self.entity, self.index)
    }

    /// The constrained bones: one, or two for a two-bone chain. Each entry is the bone's applied pose.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBonePose>> {
        self.bones()
    }

    /// The bone the constrained bones reach for, or null once the sprite is gone.
    #[func]
    fn get_target(&self) -> Option<Gd<SpineBone>> {
        let target = read_body(self.entity, self.index, |ik: &IkConstraint| ik.target())?;
        Some(SpineBone::create(self.entity, target))
    }

    /// Makes the constrained bones reach for `target`, a bone of the same skeleton. The skeleton's
    /// update order is rebuilt at the next frame.
    #[func]
    fn set_target(&mut self, target: Option<Gd<SpineBone>>) {
        let Some(target) = target.and_then(|bone| own_bone(self.entity, &bone)) else {
            return;
        };
        edit_structure(self.entity, self.index, |ik: &mut IkConstraint| {
            ik.set_target(target);
        });
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpineIkConstraintPose> {
        SpineIkConstraintPose::create(self.entity, self.index, PoseKind::Pose)
    }

    /// The pose after other constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpineIkConstraintPose> {
        SpineIkConstraintPose::create(self.entity, self.index, PoseKind::Applied)
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

/// The values of an IK constraint that animations key: mix, softness, bend direction, compress and
/// stretch.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineIkConstraintPose {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
    kind: PoseKind,
}

impl SpineIkConstraintPose {
    /// A handle to the pose `kind` of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineIkConstraintPose {
            base,
            owner: owner.into(),
            index,
            kind,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&IkConstraintPose) -> R) -> R {
        read_pose(self.owner, self.index, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut IkConstraintPose)) {
        write_pose(self.owner, self.index, self.kind, f);
    }
}

#[godot_api]
impl SpineIkConstraintPose {
    /// How much of the IK result is applied, from 0 (none) to 1 (all).
    #[func]
    fn get_mix(&self) -> f32 {
        self.read(|pose| pose.mix)
    }

    #[func]
    fn set_mix(&mut self, value: f32) {
        self.write(|pose| pose.mix = value);
    }

    /// For a two-bone chain, how far the reach is eased as the chain straightens.
    #[func]
    fn get_softness(&self) -> f32 {
        self.read(|pose| pose.softness)
    }

    #[func]
    fn set_softness(&mut self, value: f32) {
        self.write(|pose| pose.softness = value);
    }

    /// Which way a two-bone chain bends: 1 or -1.
    #[func]
    fn get_bend_direction(&self) -> i32 {
        self.read(|pose| pose.bend_direction)
    }

    #[func]
    fn set_bend_direction(&mut self, value: i32) {
        self.write(|pose| pose.bend_direction = value);
    }

    /// Whether a one-bone chain shortens to reach a target that is too close.
    #[func]
    fn get_compress(&self) -> bool {
        self.read(|pose| pose.compress)
    }

    #[func]
    fn set_compress(&mut self, value: bool) {
        self.write(|pose| pose.compress = value);
    }

    /// Whether the chain lengthens to reach a target that is too far.
    #[func]
    fn get_stretch(&self) -> bool {
        self.read(|pose| pose.stretch)
    }

    #[func]
    fn set_stretch(&mut self, value: bool) {
        self.write(|pose| pose.stretch = value);
    }
}

/// The setup values of an IK constraint, from the skeleton data. The data is shared by every
/// sprite that uses it, so the setters change it for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineIkConstraintData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpineIkConstraintData {
    /// A handle to the data of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineIkConstraintData {
            base,
            owner: owner.into(),
            index,
        })
    }
}

impl ConstraintHandle for SpineIkConstraintData {
    fn owner(&self) -> Owner {
        self.owner
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineIkConstraintData {
    #[func]
    fn get_constraint_name(&self) -> GString {
        self.constraint_name()
    }

    #[func]
    fn is_skin_required(&self) -> bool {
        self.skin_required()
    }

    /// The constrained bones: one, or two for a two-bone chain.
    #[func]
    fn get_bones(&self) -> Array<Gd<SpineBoneData>> {
        self.bone_data()
    }

    /// The bone the constrained bones reach for by default.
    #[func]
    fn get_target(&self) -> Option<Gd<SpineBoneData>> {
        let target = read_data(self.owner, self.index, |data: &IkConstraintData| {
            data.target
        })?;
        Some(SpineBoneData::create(self.owner, BoneId(target)))
    }

    #[func]
    fn set_target(&mut self, target: Option<Gd<SpineBoneData>>) {
        let Some(target) = target else {
            godot_warn!("Spine: an IK constraint needs a target bone; not changed");
            return;
        };
        let Some(bone) = own_bone_data(self.owner, &target) else {
            return;
        };
        edit_data_structure::<IkConstraintData>(self.owner, self.index, |data| {
            data.target = bone.index();
        });
    }

    /// How the constraint scales the Y axis when stretching: one of `SpineConstant.ScaleYMode_*`.
    #[func]
    fn get_scale_y_mode(&self) -> i32 {
        read_data(self.owner, self.index, |data: &IkConstraintData| {
            data.scale_y_mode as i32
        })
        .unwrap_or_default()
    }

    #[func]
    fn set_scale_y_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 3) else {
            return;
        };
        edit_data::<IkConstraintData>(self.owner, self.index, |data| {
            data.scale_y_mode = ScaleYMode::from_index(mode);
        });
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpineIkConstraintPose> {
        SpineIkConstraintPose::create(self.owner, self.index, PoseKind::Setup)
    }
}
