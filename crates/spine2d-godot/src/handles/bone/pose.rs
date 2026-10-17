//! `SpineBonePose`: the local values and the world transform of a bone, in one of its poses.

use godot::classes::RefCounted;
use godot::global::godot_warn;
use godot::prelude::*;
use spine2d_core::{BoneId, BonePose, Inherit};

use crate::constants::SpineConstant;
use crate::handles::{warn_not_implemented, PoseKind, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// Reads the pose `kind` of `bone`. `None` once the sprite is gone. The setup pose has no world
/// transform, so its world values read as zero.
pub fn read_pose<R>(
    owner: Owner,
    bone: BoneId,
    kind: PoseKind,
    f: impl FnOnce(&BonePose) -> R,
) -> Option<R> {
    if kind == PoseKind::Setup {
        return world::owner_data(owner, |data| {
            let mut pose = BonePose::default();
            pose.set_local(&data.bones[bone.index()].setup_pose);
            f(&pose)
        });
    }
    world::skeleton(owner.entity()?, |skeleton| {
        if kind == PoseKind::Pose {
            f(&skeleton.bone(bone).pose)
        } else {
            f(skeleton.bone(bone).applied_pose())
        }
    })
}

/// Changes the pose `kind` of `bone`. The setup pose belongs to the shared skeleton data, so
/// changing it changes it for every sprite made from the data; they use it the next time they
/// apply their setup pose.
pub fn write_pose(owner: Owner, bone: BoneId, kind: PoseKind, f: impl FnOnce(&mut BonePose)) {
    if kind == PoseKind::Setup {
        world::edit_data(owner, |data| {
            let local = &mut data.bones[bone.index()].setup_pose;
            let mut pose = BonePose::default();
            pose.set_local(local);
            f(&mut pose);
            *local = pose.local;
        });
        return;
    }
    let Some(entity) = owner.entity() else {
        return;
    };
    world::skeleton_mut(entity, |skeleton| {
        let bone = skeleton.bone_mut(bone);
        f(if kind == PoseKind::Applied {
            bone.applied_pose_mut()
        } else {
            &mut bone.pose
        });
    });
}

/// Converts a point from the sprite's space to the local space of `bone`.
pub fn world_to_local(owner: Owner, bone: BoneId, kind: PoseKind, point: Vector2) -> Vector2 {
    read_pose(owner, bone, kind, |pose| {
        let (x, y) = pose.world_to_local(point.x, point.y);
        Vector2::new(x, y)
    })
    .unwrap_or_default()
}

/// Converts a point from the local space of `bone` to the sprite's space.
pub fn local_to_world(owner: Owner, bone: BoneId, kind: PoseKind, point: Vector2) -> Vector2 {
    read_pose(owner, bone, kind, |pose| {
        let (x, y) = pose.local_to_world(point.x, point.y);
        Vector2::new(x, y)
    })
    .unwrap_or_default()
}

/// Converts a point from the sprite's space to the local space of the parent of `bone`; the point
/// is returned as it is for the root bone.
pub fn world_to_parent(owner: Owner, bone: BoneId, kind: PoseKind, point: Vector2) -> Vector2 {
    match parent_of(owner, bone) {
        Some(parent) => world_to_local(owner, parent, kind, point),
        None => point,
    }
}

/// Converts a point from the local space of the parent of `bone` to the sprite's space; the point
/// is returned as it is for the root bone.
pub fn parent_to_world(owner: Owner, bone: BoneId, kind: PoseKind, point: Vector2) -> Vector2 {
    match parent_of(owner, bone) {
        Some(parent) => local_to_world(owner, parent, kind, point),
        None => point,
    }
}

/// Converts a rotation in the sprite's space to a rotation relative to `bone`, in degrees.
pub fn world_to_local_rotation(owner: Owner, bone: BoneId, kind: PoseKind, rotation: f32) -> f32 {
    read_pose(owner, bone, kind, |pose| {
        pose.world_to_local_rotation(rotation)
    })
    .unwrap_or_default()
}

/// Converts a rotation relative to `bone` to a rotation in the sprite's space, in degrees.
pub fn local_to_world_rotation(owner: Owner, bone: BoneId, kind: PoseKind, rotation: f32) -> f32 {
    read_pose(owner, bone, kind, |pose| {
        pose.local_to_world_rotation(rotation)
    })
    .unwrap_or_default()
}

/// Rotates `bone` by `degrees` in the sprite's space.
pub fn rotate_world(owner: Owner, bone: BoneId, kind: PoseKind, degrees: f32) {
    write_pose(owner, bone, kind, |pose| pose.rotate_world(degrees));
}

fn parent_of(owner: Owner, bone: BoneId) -> Option<BoneId> {
    world::owner_data(owner, |data| data.bones[bone.index()].parent.map(BoneId)).flatten()
}

/// The inherit mode for the script value `value`, one of `SpineConstant.Inherit_*`.
pub(super) fn inherit_from(value: i32) -> Option<Inherit> {
    match i64::from(value) {
        SpineConstant::Inherit_Normal => Some(Inherit::Normal),
        SpineConstant::Inherit_OnlyTranslation => Some(Inherit::OnlyTranslation),
        SpineConstant::Inherit_NoRotationOrReflection => Some(Inherit::NoRotationOrReflection),
        SpineConstant::Inherit_NoScale => Some(Inherit::NoScale),
        SpineConstant::Inherit_NoScaleOrReflection => Some(Inherit::NoScaleOrReflection),
        _ => None,
    }
}

/// One pose of a bone: the local values animations key, and the world transform computed from
/// them. Get it from `SpineBone.get_pose()` (what animations and scripts set),
/// `SpineBone.get_applied_pose()` (after constraints) or `SpineBoneData.get_setup_pose()`.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineBonePose {
    base: Base<RefCounted>,
    owner: Owner,
    bone: BoneId,
    kind: PoseKind,
}

impl SpineBonePose {
    /// A handle to the pose `kind` of `bone` of `owner`. Only the setup pose exists without a
    /// skeleton.
    pub fn create(owner: impl Into<Owner>, bone: BoneId, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineBonePose {
            base,
            owner: owner.into(),
            bone,
            kind,
        })
    }

    /// What this pose belongs to.
    pub fn owner(&self) -> Owner {
        self.owner
    }

    /// The index of the bone this pose belongs to.
    pub fn bone_id(&self) -> BoneId {
        self.bone
    }

    fn read<R: Default>(&self, f: impl FnOnce(&BonePose) -> R) -> R {
        read_pose(self.owner, self.bone, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut BonePose)) {
        write_pose(self.owner, self.bone, self.kind, f);
    }
}

#[godot_api]
impl SpineBonePose {
    /// Not implemented yet: logs a warning.
    #[func]
    fn update_world_transform(&mut self, _skeleton: Option<Gd<SpineSkeleton>>) {
        warn_not_implemented("update_world_transform");
    }

    /// Not implemented yet: logs a warning.
    #[func]
    fn update_local_transform(&mut self, _skeleton: Option<Gd<SpineSkeleton>>) {
        warn_not_implemented("update_local_transform");
    }

    #[func]
    fn get_x(&self) -> f32 {
        self.read(|pose| pose.local.x)
    }

    #[func]
    fn set_x(&mut self, value: f32) {
        self.write(|pose| pose.local.x = value);
    }

    #[func]
    fn get_y(&self) -> f32 {
        self.read(|pose| pose.local.y)
    }

    #[func]
    fn set_y(&mut self, value: f32) {
        self.write(|pose| pose.local.y = value);
    }

    /// Local rotation in degrees.
    #[func]
    fn get_rotation(&self) -> f32 {
        self.read(|pose| pose.local.rotation)
    }

    #[func]
    fn set_rotation(&mut self, value: f32) {
        self.write(|pose| pose.local.rotation = value);
    }

    #[func]
    fn get_scale_x(&self) -> f32 {
        self.read(|pose| pose.local.scale_x)
    }

    #[func]
    fn set_scale_x(&mut self, value: f32) {
        self.write(|pose| pose.local.scale_x = value);
    }

    #[func]
    fn get_scale_y(&self) -> f32 {
        self.read(|pose| pose.local.scale_y)
    }

    #[func]
    fn set_scale_y(&mut self, value: f32) {
        self.write(|pose| pose.local.scale_y = value);
    }

    #[func]
    fn get_shear_x(&self) -> f32 {
        self.read(|pose| pose.local.shear_x)
    }

    #[func]
    fn set_shear_x(&mut self, value: f32) {
        self.write(|pose| pose.local.shear_x = value);
    }

    #[func]
    fn get_shear_y(&self) -> f32 {
        self.read(|pose| pose.local.shear_y)
    }

    #[func]
    fn set_shear_y(&mut self, value: f32) {
        self.write(|pose| pose.local.shear_y = value);
    }

    /// How the bone inherits its parent's transform: one of `SpineConstant.Inherit_*`.
    #[func]
    fn get_inherit(&self) -> i32 {
        self.read(|pose| pose.local.inherit as i32)
    }

    #[func]
    fn set_inherit(&mut self, value: i32) {
        let Some(inherit) = inherit_from(value) else {
            godot_warn!("Spine: {value} is not an inherit mode (0 to 4)");
            return;
        };
        self.write(|pose| pose.local.inherit = inherit);
    }

    /// The `a` entry of the world matrix: x axis, x component.
    #[func]
    fn get_a(&self) -> f32 {
        self.read(|pose| pose.a)
    }

    #[func]
    fn set_a(&mut self, value: f32) {
        self.write(|pose| pose.a = value);
    }

    /// The `b` entry of the world matrix: y axis, x component.
    #[func]
    fn get_b(&self) -> f32 {
        self.read(|pose| pose.b)
    }

    #[func]
    fn set_b(&mut self, value: f32) {
        self.write(|pose| pose.b = value);
    }

    /// The `c` entry of the world matrix: x axis, y component.
    #[func]
    fn get_c(&self) -> f32 {
        self.read(|pose| pose.c)
    }

    #[func]
    fn set_c(&mut self, value: f32) {
        self.write(|pose| pose.c = value);
    }

    /// The `d` entry of the world matrix: y axis, y component.
    #[func]
    fn get_d(&self) -> f32 {
        self.read(|pose| pose.d)
    }

    #[func]
    fn set_d(&mut self, value: f32) {
        self.write(|pose| pose.d = value);
    }

    /// X position in the sprite's space after the last world transform update.
    #[func]
    fn get_world_x(&self) -> f32 {
        self.read(|pose| pose.world_x)
    }

    #[func]
    fn set_world_x(&mut self, value: f32) {
        self.write(|pose| pose.world_x = value);
    }

    /// Y position in the sprite's space after the last world transform update.
    #[func]
    fn get_world_y(&self) -> f32 {
        self.read(|pose| pose.world_y)
    }

    #[func]
    fn set_world_y(&mut self, value: f32) {
        self.write(|pose| pose.world_y = value);
    }

    /// Rotation of the bone's x axis in the sprite's space, in degrees.
    #[func]
    fn get_world_rotation_x(&self) -> f32 {
        self.read(|pose| pose.world_rotation_x())
    }

    /// Rotation of the bone's y axis in the sprite's space, in degrees.
    #[func]
    fn get_world_rotation_y(&self) -> f32 {
        self.read(|pose| pose.world_rotation_y())
    }

    #[func]
    fn get_world_scale_x(&self) -> f32 {
        self.read(|pose| pose.world_scale_x())
    }

    #[func]
    fn get_world_scale_y(&self) -> f32 {
        self.read(|pose| pose.world_scale_y())
    }

    /// Converts a point in the sprite's space to this bone's local space.
    #[func]
    fn world_to_local(&self, world_position: Vector2) -> Vector2 {
        world_to_local(self.owner, self.bone, self.kind, world_position)
    }

    /// Converts a point in this bone's local space to the sprite's space.
    #[func]
    fn local_to_world(&self, local_position: Vector2) -> Vector2 {
        local_to_world(self.owner, self.bone, self.kind, local_position)
    }

    /// Converts a point in the sprite's space to the local space of this bone's parent.
    #[func]
    fn world_to_parent(&self, world_position: Vector2) -> Vector2 {
        world_to_parent(self.owner, self.bone, self.kind, world_position)
    }

    /// Converts a point in the local space of this bone's parent to the sprite's space.
    #[func]
    fn parent_to_world(&self, local_position: Vector2) -> Vector2 {
        parent_to_world(self.owner, self.bone, self.kind, local_position)
    }

    #[func]
    fn world_to_local_rotation(&self, world_rotation: f32) -> f32 {
        world_to_local_rotation(self.owner, self.bone, self.kind, world_rotation)
    }

    #[func]
    fn local_to_world_rotation(&self, local_rotation: f32) -> f32 {
        local_to_world_rotation(self.owner, self.bone, self.kind, local_rotation)
    }

    /// Rotates the bone by `degrees` in the sprite's space.
    #[func]
    fn rotate_world(&mut self, degrees: f32) {
        rotate_world(self.owner, self.bone, self.kind, degrees);
    }
}
