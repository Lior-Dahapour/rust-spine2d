//! `SpineBone`: a bone of a sprite's skeleton, as the official API names it. The bone's values live
//! in its poses (`SpineBonePose`) and its setup in `SpineBoneData`; like the other handles, these
//! hold only the entity and an index.

mod data;
mod local;
mod pose;

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::BoneId;
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::PoseKind;
use crate::handles::{warn_not_implemented, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;
pub use data::SpineBoneData;
pub use pose::SpineBonePose;
use pose::{
    local_to_world, local_to_world_rotation, parent_to_world, read_pose, rotate_world,
    world_to_local, world_to_local_rotation, world_to_parent, write_pose,
};

/// A bone of the skeleton: its place in the hierarchy, its poses and its setup data.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineBone {
    base: Base<RefCounted>,
    entity: Entity,
    bone: BoneId,
}

impl SpineBone {
    /// A handle to `bone` in the skeleton `entity`.
    pub fn create(entity: Entity, bone: BoneId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineBone { base, entity, bone })
    }

    /// The skeleton entity this bone belongs to.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// The bone's index in its skeleton.
    pub fn bone_id(&self) -> BoneId {
        self.bone
    }
}

#[godot_api]
impl SpineBone {
    /// False once the sprite this bone came from was reloaded or freed. Reads then return zero
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    /// Not implemented yet: logs a warning. The world update calculates the bones in order.
    /// `physics` is one of `SpineConstant.Physics_*`.
    #[func]
    fn update(&mut self, _skeleton: Option<Gd<SpineSkeleton>>, _physics: i32) {
        warn_not_implemented("update");
    }

    /// The setup description of this bone, shared by every sprite of the skeleton file.
    #[func]
    fn get_data(&self) -> Gd<SpineBoneData> {
        SpineBoneData::create(self.entity, self.bone)
    }

    /// The parent bone, or null for the root.
    #[func]
    fn get_parent(&self) -> Option<Gd<SpineBone>> {
        let parent = world::skeleton(self.entity, |skeleton| skeleton.bone(self.bone).parent())??;
        Some(SpineBone::create(self.entity, parent))
    }

    /// The bones that have this bone as their parent.
    #[func]
    fn get_children(&self) -> Array<Gd<SpineBone>> {
        world::skeleton(self.entity, |skeleton| {
            skeleton
                .bone(self.bone)
                .children()
                .iter()
                .map(|&child| SpineBone::create(self.entity, child))
                .collect()
        })
        .unwrap_or_default()
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpineBonePose> {
        SpineBonePose::create(self.entity, self.bone, PoseKind::Pose)
    }

    /// The pose after constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpineBonePose> {
        SpineBonePose::create(self.entity, self.bone, PoseKind::Applied)
    }

    /// Whether the bone takes part in the update. The skeleton sets this from the active skin.
    #[func]
    fn is_active(&self) -> bool {
        world::skeleton(self.entity, |skeleton| skeleton.bone(self.bone).is_active())
            .unwrap_or(false)
    }

    /// Sets the flag [`is_active`] reports. The skeleton recomputes it whenever the skin changes,
    /// and the update does not read it, so this does not turn the bone off.
    #[func]
    fn set_active(&mut self, active: bool) {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.bone_mut(self.bone).set_active(active);
        });
    }

    /// Converts a point in the sprite's space to this bone's local space.
    #[func]
    fn world_to_local(&self, world_position: Vector2) -> Vector2 {
        world_to_local(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            world_position,
        )
    }

    /// Converts a point in the sprite's space to the local space of this bone's parent.
    #[func]
    fn world_to_parent(&self, world_position: Vector2) -> Vector2 {
        world_to_parent(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            world_position,
        )
    }

    /// Converts a point in this bone's local space to the sprite's space.
    #[func]
    fn local_to_world(&self, local_position: Vector2) -> Vector2 {
        local_to_world(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            local_position,
        )
    }

    /// Converts a point in the local space of this bone's parent to the sprite's space.
    #[func]
    fn parent_to_world(&self, local_position: Vector2) -> Vector2 {
        parent_to_world(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            local_position,
        )
    }

    /// Converts a rotation in the sprite's space to a rotation relative to this bone, in degrees.
    #[func]
    fn world_to_local_rotation(&self, world_rotation: f32) -> f32 {
        world_to_local_rotation(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            world_rotation,
        )
    }

    /// Converts a rotation relative to this bone to a rotation in the sprite's space, in degrees.
    #[func]
    fn local_to_world_rotation(&self, local_rotation: f32) -> f32 {
        local_to_world_rotation(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            local_rotation,
        )
    }

    /// Rotates the bone by `degrees` in the sprite's space.
    #[func]
    fn rotate_world(&mut self, degrees: f32) {
        rotate_world(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            degrees,
        );
    }

    /// The bone's local pose as a transform: position, rotation, scale and shear of its axes.
    #[func]
    fn get_transform(&self) -> Transform2D {
        read_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Pose,
            |pose| {
                let local = &pose.local;
                let rotation_x = (local.rotation + local.shear_x).to_radians();
                let rotation_y = (local.rotation + 90.0 + local.shear_y).to_radians();
                Transform2D::from_cols(
                    Vector2::new(rotation_x.cos(), rotation_x.sin()) * local.scale_x,
                    Vector2::new(rotation_y.cos(), rotation_y.sin()) * local.scale_y,
                    Vector2::new(local.x, local.y),
                )
            },
        )
        .unwrap_or_default()
    }

    /// Sets the bone's local position, rotation and scale from `local_transform`. Shear is left as
    /// it is.
    #[func]
    fn set_transform(&mut self, local_transform: Transform2D) {
        let scale = local_transform.scale();
        write_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Pose,
            |pose| {
                pose.local.x = local_transform.origin.x;
                pose.local.y = local_transform.origin.y;
                pose.local.rotation = local_transform.rotation().to_degrees();
                pose.local.scale_x = scale.x;
                pose.local.scale_y = scale.y;
            },
        );
    }
}

/// Methods this plugin adds to the official ones, for scripts that only need a bone's name or
/// where it is.
#[godot_api(secondary)]
impl SpineBone {
    #[func]
    fn get_name(&self) -> GString {
        world::skeleton(self.entity, |skeleton| {
            GString::from(skeleton.data().bones[self.bone.index()].name.as_str())
        })
        .unwrap_or_default()
    }

    #[func]
    pub fn get_length(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| {
            skeleton.data().bones[self.bone.index()].length
        })
        .unwrap_or_default()
    }

    /// Position in the sprite's space after the last world transform update.
    #[func]
    fn get_world_position(&self) -> Vector2 {
        read_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            |pose| Vector2::new(pose.world_x, pose.world_y),
        )
        .unwrap_or_default()
    }

    /// Rotation of the bone's x axis in the sprite's space, in degrees.
    #[func]
    fn get_world_rotation(&self) -> f32 {
        read_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            |pose| pose.world_rotation_x(),
        )
        .unwrap_or_default()
    }

    #[func]
    fn get_world_scale(&self) -> Vector2 {
        read_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            |pose| Vector2::new(pose.world_scale_x(), pose.world_scale_y()),
        )
        .unwrap_or_default()
    }

    /// Moves the bone so it ends up at `position` in the sprite's space, by changing its local
    /// position. Call between `before_world_transforms_change` and the world update.
    #[func]
    fn set_world_position(&mut self, position: Vector2) {
        let local = world_to_parent(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Applied,
            position,
        );
        write_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Pose,
            |pose| {
                pose.local.x = local.x;
                pose.local.y = local.y;
            },
        );
    }

    /// Sets the local pose back to the setup pose.
    #[func]
    fn set_to_setup_pose(&mut self) {
        let Some(setup) = world::skeleton(self.entity, |skeleton| {
            skeleton.data().bones[self.bone.index()].setup_pose
        }) else {
            return;
        };
        write_pose(
            Owner::Skeleton(self.entity),
            self.bone,
            PoseKind::Pose,
            |pose| {
                pose.local = setup;
            },
        );
    }
}
