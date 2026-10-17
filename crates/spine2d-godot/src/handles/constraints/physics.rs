//! The physics constraint classes: `SpinePhysicsConstraint`, its pose and its data.

use godot::classes::RefCounted;
use godot::global::godot_warn;
use godot::prelude::*;
use spine2d_core::{
    BoneId, PhysicsConstraint, PhysicsConstraintData, PhysicsConstraintPose, ScaleYMode,
};
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::{
    edit_body, edit_data, edit_structure, mode_index, own_bone_pose, read_body, read_data,
    read_pose, write_pose, ConstraintHandle,
};
use crate::handles::bone::{SpineBoneData, SpineBonePose};
use crate::handles::{warn_not_implemented, PoseKind, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// A physics constraint of a skeleton: a spring-like response of a bone to movement, wind and
/// gravity. It has no active flag in the official API, so it has none here.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePhysicsConstraint {
    base: Base<RefCounted>,
    entity: Entity,
    index: usize,
}

impl SpinePhysicsConstraint {
    /// A handle to constraint `index` of the skeleton `entity`.
    pub fn create(entity: Entity, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePhysicsConstraint {
            base,
            entity,
            index,
        })
    }
}

impl ConstraintHandle for SpinePhysicsConstraint {
    fn owner(&self) -> Owner {
        Owner::Skeleton(self.entity)
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpinePhysicsConstraint {
    /// False once the sprite this constraint came from was reloaded or freed. Reads then return
    /// zero values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    /// Not implemented yet: logs a warning. `physics` is one of `SpineConstant.Physics_*`.
    #[func]
    fn update(&mut self, _skeleton: Option<Gd<SpineSkeleton>>, _physics: i32) {
        warn_not_implemented("update");
    }

    #[func]
    fn get_data(&self) -> Gd<SpinePhysicsConstraintData> {
        SpinePhysicsConstraintData::create(self.entity, self.index)
    }

    /// The applied pose of the bone the simulation moves, or null once the sprite is gone.
    #[func]
    fn get_bone(&self) -> Option<Gd<SpineBonePose>> {
        let bone = read_body(self.entity, self.index, |constraint: &PhysicsConstraint| {
            constraint.bone()
        })?;
        Some(SpineBonePose::create(self.entity, bone, PoseKind::Applied))
    }

    /// Moves the bone `bone` belongs to, a bone of the same skeleton, from now on. The skeleton's
    /// update order is rebuilt at the next frame.
    #[func]
    fn set_bone(&mut self, bone: Option<Gd<SpineBonePose>>) {
        let Some(bone) = bone.and_then(|pose| own_bone_pose(self.entity, &pose)) else {
            return;
        };
        edit_structure(
            self.entity,
            self.index,
            |constraint: &mut PhysicsConstraint| constraint.set_bone(bone),
        );
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpinePhysicsConstraintPose> {
        SpinePhysicsConstraintPose::create(self.entity, self.index, PoseKind::Pose)
    }

    /// The pose after other constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpinePhysicsConstraintPose> {
        SpinePhysicsConstraintPose::create(self.entity, self.index, PoseKind::Applied)
    }

    /// Restarts the simulation from the bone's current pose. `skeleton` must be the skeleton the
    /// constraint belongs to; its clock is where the simulation restarts.
    #[func]
    fn reset(&mut self, skeleton: Option<Gd<SpineSkeleton>>) {
        let Some(skeleton) = skeleton else { return };
        if skeleton.bind().entity() != self.entity {
            godot_warn!("Spine: the skeleton is not the one this constraint belongs to");
            return;
        }
        let Some(time) = world::skeleton(self.entity, |skeleton| skeleton.time) else {
            return;
        };
        edit_body(
            self.entity,
            self.index,
            |constraint: &mut PhysicsConstraint| constraint.reset(time),
        );
    }

    /// Moves the simulation as if the skeleton moved by `x`, `y`, so the bone does not react to
    /// the move.
    #[func]
    fn translate(&mut self, x: f32, y: f32) {
        edit_body(
            self.entity,
            self.index,
            |constraint: &mut PhysicsConstraint| constraint.translate(x, y),
        );
    }

    /// Rotates the simulation around `x`, `y` by `degrees`, as if the skeleton rotated.
    #[func]
    fn rotate(&mut self, x: f32, y: f32, degrees: f32) {
        edit_body(
            self.entity,
            self.index,
            |constraint: &mut PhysicsConstraint| constraint.rotate(x, y, degrees),
        );
    }
}

/// The values of a physics constraint that animations key: how the simulated bone responds.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePhysicsConstraintPose {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
    kind: PoseKind,
}

impl SpinePhysicsConstraintPose {
    /// A handle to the pose `kind` of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePhysicsConstraintPose {
            base,
            owner: owner.into(),
            index,
            kind,
        })
    }

    fn read(&self, f: impl FnOnce(&PhysicsConstraintPose) -> f32) -> f32 {
        read_pose(self.owner, self.index, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut PhysicsConstraintPose)) {
        write_pose(self.owner, self.index, self.kind, f);
    }
}

#[godot_api]
impl SpinePhysicsConstraintPose {
    /// How much of the skeleton's movement the bone keeps.
    #[func]
    fn get_inertia(&self) -> f32 {
        self.read(|pose| pose.inertia)
    }

    #[func]
    fn set_inertia(&mut self, value: f32) {
        self.write(|pose| pose.inertia = value);
    }

    /// How hard the bone is pulled back to its rest pose.
    #[func]
    fn get_strength(&self) -> f32 {
        self.read(|pose| pose.strength)
    }

    #[func]
    fn set_strength(&mut self, value: f32) {
        self.write(|pose| pose.strength = value);
    }

    /// How quickly the motion dies down.
    #[func]
    fn get_damping(&self) -> f32 {
        self.read(|pose| pose.damping)
    }

    #[func]
    fn set_damping(&mut self, value: f32) {
        self.write(|pose| pose.damping = value);
    }

    /// One divided by the mass: a larger value makes the bone lighter.
    #[func]
    fn get_mass_inverse(&self) -> f32 {
        self.read(|pose| pose.mass_inverse)
    }

    #[func]
    fn set_mass_inverse(&mut self, value: f32) {
        self.write(|pose| pose.mass_inverse = value);
    }

    #[func]
    fn get_wind(&self) -> f32 {
        self.read(|pose| pose.wind)
    }

    #[func]
    fn set_wind(&mut self, value: f32) {
        self.write(|pose| pose.wind = value);
    }

    #[func]
    fn get_gravity(&self) -> f32 {
        self.read(|pose| pose.gravity)
    }

    #[func]
    fn set_gravity(&mut self, value: f32) {
        self.write(|pose| pose.gravity = value);
    }

    /// How much of the simulation is applied, from 0 (none) to 1 (all).
    #[func]
    fn get_mix(&self) -> f32 {
        self.read(|pose| pose.mix)
    }

    #[func]
    fn set_mix(&mut self, value: f32) {
        self.write(|pose| pose.mix = value);
    }
}

/// The setup values of a physics constraint, from the skeleton data. The data is shared by every
/// sprite that uses it, so the setters change it for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpinePhysicsConstraintData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpinePhysicsConstraintData {
    /// A handle to the data of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpinePhysicsConstraintData {
            base,
            owner: owner.into(),
            index,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&PhysicsConstraintData) -> R) -> R {
        read_data(self.owner, self.index, f).unwrap_or_default()
    }
}

impl ConstraintHandle for SpinePhysicsConstraintData {
    fn owner(&self) -> Owner {
        self.owner
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpinePhysicsConstraintData {
    #[func]
    fn get_constraint_name(&self) -> GString {
        self.constraint_name()
    }

    #[func]
    fn is_skin_required(&self) -> bool {
        self.skin_required()
    }

    /// The bone the simulation moves.
    #[func]
    fn get_bone(&self) -> Option<Gd<SpineBoneData>> {
        let bone = self.read(|data| data.bone);
        Some(SpineBoneData::create(self.owner, BoneId(bone)))
    }

    /// How much of the bone's x movement is simulated.
    #[func]
    fn get_x(&self) -> f32 {
        self.read(|data| data.x)
    }

    /// How much of the bone's y movement is simulated.
    #[func]
    fn get_y(&self) -> f32 {
        self.read(|data| data.y)
    }

    /// How much of the bone's rotation is simulated.
    #[func]
    fn get_rotate(&self) -> f32 {
        self.read(|data| data.rotate)
    }

    /// How much of the bone's x scale is simulated.
    #[func]
    fn get_scale_x(&self) -> f32 {
        self.read(|data| data.scale_x)
    }

    /// How much of the bone's x shear is simulated.
    #[func]
    fn get_shear_x(&self) -> f32 {
        self.read(|data| data.shear_x)
    }

    /// The largest distance the simulated bone may move from its rest pose.
    #[func]
    fn get_limit(&self) -> f32 {
        self.read(|data| data.limit)
    }

    /// The simulation's time step in seconds.
    #[func]
    fn get_step(&self) -> f32 {
        self.read(|data| data.step)
    }

    /// How the constraint scales the Y axis when stretching: one of `SpineConstant.ScaleYMode_*`.
    #[func]
    fn get_scale_y_mode(&self) -> i32 {
        self.read(|data| data.scale_y_mode as i32)
    }

    #[func]
    fn set_scale_y_mode(&mut self, mode: i32) {
        let Some(mode) = mode_index(mode, 3) else {
            return;
        };
        edit_data::<PhysicsConstraintData>(self.owner, self.index, |data| {
            data.scale_y_mode = ScaleYMode::from_index(mode);
        });
    }

    #[func]
    fn is_inertia_global(&self) -> bool {
        self.read(|data| data.inertia_global)
    }

    #[func]
    fn is_strength_global(&self) -> bool {
        self.read(|data| data.strength_global)
    }

    #[func]
    fn is_damping_global(&self) -> bool {
        self.read(|data| data.damping_global)
    }

    #[func]
    fn is_mass_global(&self) -> bool {
        self.read(|data| data.mass_global)
    }

    #[func]
    fn is_wind_global(&self) -> bool {
        self.read(|data| data.wind_global)
    }

    #[func]
    fn is_gravity_global(&self) -> bool {
        self.read(|data| data.gravity_global)
    }

    #[func]
    fn is_mix_global(&self) -> bool {
        self.read(|data| data.mix_global)
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpinePhysicsConstraintPose> {
        SpinePhysicsConstraintPose::create(self.owner, self.index, PoseKind::Setup)
    }
}
