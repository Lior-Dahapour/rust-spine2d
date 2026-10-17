//! The constraint classes of the official API: for each kind a constraint, its pose and its data.
//!
//! Every class holds the skeleton's entity and the constraint's index, and reads or writes the
//! constraint inside the world. The generic helpers below do the lookups, so a class only says
//! which kind it is.
//!
//! Differences from the official API:
//! - The official data classes return `SpineBoneData`, `SpineSlotData` and `SpineAnimation` for
//!   bones, slots and animations. Those methods are left out until those classes exist.
//! - The official data classes share the base class `SpineConstraintData`. An extension class
//!   cannot extend another, so each data class repeats its two methods and a script cannot test
//!   or annotate with the base type.
//! - `update(skeleton, ...)` exists but only logs a warning: the world updates the constraints
//!   itself.

mod ik;
mod path;
mod physics;
mod slider;
mod transform;

use godot::global::godot_warn;
use godot::prelude::*;
use spine2d_core::{
    BoneId, ConstraintBody, ConstraintData, IkConstraint, IkConstraintData, IkConstraintPose,
    PathConstraint, PathConstraintData, PathConstraintPose, PhysicsConstraint,
    PhysicsConstraintData, PhysicsConstraintPose, PoseSet, Slider, SliderData, SliderPose, SlotId,
    TransformConstraint, TransformConstraintData, TransformConstraintPose,
};
use spine2d_ecs::bevy_ecs::change_detection::DetectChangesMut;
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::bone::{SpineBone, SpineBoneData, SpineBonePose};
use super::slot::SpineSlot;
use super::slot_data::SpineSlotData;
use super::PoseKind;
use crate::runtime::store::Owner;
use crate::runtime::world;

pub use ik::{SpineIkConstraint, SpineIkConstraintData};
pub use path::{SpinePathConstraint, SpinePathConstraintData};
pub use physics::{SpinePhysicsConstraint, SpinePhysicsConstraintData};
pub use slider::SpineSlider;
pub use transform::{SpineTransformConstraint, SpineTransformConstraintData};

/// The bones a constraint of this kind constrains, by index. Physics and slider constraints
/// drive one bone or none through their own field, so they list none here.
fn constrained_bones(data: &ConstraintData) -> &[usize] {
    match data {
        ConstraintData::Ik(data) => &data.bones,
        ConstraintData::Transform(data) => &data.bones,
        ConstraintData::Path(data) => &data.bones,
        ConstraintData::Physics(_) | ConstraintData::Slider(_) => &[],
    }
}

/// A handle to one constraint of a skeleton. The logic every constraint and data class shares sits
/// here, so each class only says where its entity and index are and forwards what scripts call.
pub trait ConstraintHandle {
    /// What the constraint belongs to: a skeleton, or for data classes also the stored data.
    fn owner(&self) -> Owner;

    /// The constraint's index in the skeleton data.
    fn index(&self) -> usize;

    /// The constraint's name; empty once the sprite is gone.
    fn constraint_name(&self) -> GString {
        world::owner_data(self.owner(), |data| {
            GString::from(data.constraints[self.index()].name())
        })
        .unwrap_or_default()
    }

    /// Whether the constraint only applies while the active skin includes it.
    fn skin_required(&self) -> bool {
        world::owner_data(self.owner(), |data| {
            data.constraints[self.index()].skin_required()
        })
        .unwrap_or_default()
    }

    /// Whether the constraint takes part in the update; see `Constraint::is_active`.
    fn constraint_active(&self) -> bool {
        let Some(entity) = self.owner().entity() else {
            return false;
        };
        world::skeleton(entity, |skeleton| {
            skeleton.constraint(self.index()).is_active()
        })
        .unwrap_or_default()
    }

    /// Sets the active flag. The skeleton recomputes it from the skin, so it does not last.
    fn set_constraint_active(&self, active: bool) {
        let Some(entity) = self.owner().entity() else {
            return;
        };
        world::skeleton_mut(entity, |skeleton| {
            skeleton.constraint_mut(self.index()).set_active(active);
        });
    }

    /// The poses of the constrained bones, in the order the constraint applies them. Like the
    /// constraints themselves, they are the applied poses, the ones constraints write to.
    fn bones(&self) -> Array<Gd<SpineBonePose>> {
        let Some(entity) = self.owner().entity() else {
            return Array::new();
        };
        world::skeleton(entity, |skeleton| {
            constrained_bones(&skeleton.data().constraints[self.index()])
                .iter()
                .map(|&bone| SpineBonePose::create(entity, BoneId(bone), PoseKind::Applied))
                .collect()
        })
        .unwrap_or_default()
    }

    /// The setup data of the constrained bones, in the order the constraint applies them.
    fn bone_data(&self) -> Array<Gd<SpineBoneData>> {
        world::owner_data(self.owner(), |data| {
            constrained_bones(&data.constraints[self.index()])
                .iter()
                .map(|&bone| SpineBoneData::create(self.owner(), BoneId(bone)))
                .collect()
        })
        .unwrap_or_default()
    }
}

/// Finds a kind of constraint inside the live skeleton.
pub trait BodyAccess: Sized {
    /// The constraint of `body`, if `body` is this kind.
    fn in_body(body: &ConstraintBody) -> Option<&Self>;
    /// The constraint of `body` for writing.
    fn in_body_mut(body: &mut ConstraintBody) -> Option<&mut Self>;
}

/// Finds a kind of constraint inside the skeleton data.
pub trait DataAccess: Sized {
    /// The data of `data`, if `data` is this kind.
    fn in_data(data: &ConstraintData) -> Option<&Self>;
    /// The data of `data` for writing.
    fn in_data_mut(data: &mut ConstraintData) -> Option<&mut Self>;
}

/// Finds a kind of pose inside a constraint or its data.
pub trait PoseAccess: Copy + Default + 'static {
    /// The pose set of `body`, if `body` is this kind of constraint.
    fn in_body(body: &ConstraintBody) -> Option<&PoseSet<Self>>;
    /// The pose set of `body` for writing.
    fn in_body_mut(body: &mut ConstraintBody) -> Option<&mut PoseSet<Self>>;
    /// The setup pose of `data`, if `data` is this kind of constraint data.
    fn in_data(data: &ConstraintData) -> Option<&Self>;
    /// The setup pose of `data` for writing.
    fn in_data_mut(data: &mut ConstraintData) -> Option<&mut Self>;
}

/// Implements the three access traits for one kind of constraint.
macro_rules! constraint_kind {
    ($variant:ident: $body:ty, $data:ty, $pose:ty) => {
        impl BodyAccess for $body {
            fn in_body(body: &ConstraintBody) -> Option<&Self> {
                match body {
                    ConstraintBody::$variant(constraint) => Some(constraint),
                    _ => None,
                }
            }

            fn in_body_mut(body: &mut ConstraintBody) -> Option<&mut Self> {
                match body {
                    ConstraintBody::$variant(constraint) => Some(constraint),
                    _ => None,
                }
            }
        }

        impl DataAccess for $data {
            fn in_data(data: &ConstraintData) -> Option<&Self> {
                match data {
                    ConstraintData::$variant(data) => Some(data),
                    _ => None,
                }
            }

            fn in_data_mut(data: &mut ConstraintData) -> Option<&mut Self> {
                match data {
                    ConstraintData::$variant(data) => Some(data),
                    _ => None,
                }
            }
        }

        impl PoseAccess for $pose {
            fn in_body(body: &ConstraintBody) -> Option<&PoseSet<Self>> {
                <$body as BodyAccess>::in_body(body).map(|constraint| &constraint.pose)
            }

            fn in_body_mut(body: &mut ConstraintBody) -> Option<&mut PoseSet<Self>> {
                <$body as BodyAccess>::in_body_mut(body).map(|constraint| &mut constraint.pose)
            }

            fn in_data(data: &ConstraintData) -> Option<&Self> {
                <$data as DataAccess>::in_data(data).map(|data| &data.setup_pose)
            }

            fn in_data_mut(data: &mut ConstraintData) -> Option<&mut Self> {
                <$data as DataAccess>::in_data_mut(data).map(|data| &mut data.setup_pose)
            }
        }
    };
}

constraint_kind!(Ik: IkConstraint, IkConstraintData, IkConstraintPose);
constraint_kind!(
    Transform: TransformConstraint,
    TransformConstraintData,
    TransformConstraintPose
);
constraint_kind!(Path: PathConstraint, PathConstraintData, PathConstraintPose);
constraint_kind!(
    Physics: PhysicsConstraint,
    PhysicsConstraintData,
    PhysicsConstraintPose
);
constraint_kind!(Slider: Slider, SliderData, SliderPose);

/// Reads constraint `index` as a `B`. `None` if it is another kind or the sprite is gone.
pub fn read_body<B: BodyAccess, R>(
    entity: Entity,
    index: usize,
    f: impl FnOnce(&B) -> R,
) -> Option<R> {
    world::skeleton(entity, |skeleton| {
        B::in_body(&skeleton.constraint(index).body).map(f)
    })
    .flatten()
}

/// Changes constraint `index` as a `B`, for values that need no rebuild of the update order.
pub fn edit_body<B: BodyAccess>(entity: Entity, index: usize, f: impl FnOnce(&mut B)) {
    world::skeleton_mut(entity, |skeleton| {
        if let Some(constraint) = B::in_body_mut(&mut skeleton.constraint_mut(index).body) {
            f(constraint);
        }
    });
}

/// Changes what constraint `index` refers to (a target, source, slot or bone) and flags the
/// skeleton's update order for a rebuild at the next frame.
pub fn edit_structure<B: BodyAccess>(entity: Entity, index: usize, f: impl FnOnce(&mut B)) {
    world::constraints_mut(entity, |skeleton, stale| {
        if let Some(constraint) = B::in_body_mut(&mut skeleton.constraint_mut(index).body) {
            f(constraint);
            stale.set_changed();
        }
    });
}

/// Reads the data of constraint `index` as a `D`. `None` if it is another kind or the sprite is
/// gone.
pub fn read_data<D: DataAccess, R>(
    owner: Owner,
    index: usize,
    f: impl FnOnce(&D) -> R,
) -> Option<R> {
    world::owner_data(owner, |data| D::in_data(&data.constraints[index]).map(f)).flatten()
}

/// Changes the data of constraint `index` as a `D`, for values that need no rebuild of the update
/// order. Every sprite made from the same skeleton data sees the change.
pub fn edit_data<D: DataAccess>(owner: Owner, index: usize, f: impl FnOnce(&mut D)) {
    world::edit_data(owner, |data| {
        if let Some(constraint) = D::in_data_mut(&mut data.constraints[index]) {
            f(constraint);
        }
    });
}

/// Changes what the data of constraint `index` refers to (a target or a bone), and flags every
/// skeleton made from the same data for a rebuild of its update order at the next frame.
pub fn edit_data_structure<D: DataAccess>(owner: Owner, index: usize, f: impl FnOnce(&mut D)) {
    world::edit_data_structure(owner, |data| {
        if let Some(constraint) = D::in_data_mut(&mut data.constraints[index]) {
            f(constraint);
        }
    });
}

/// Reads the pose `kind` of constraint `index`. `None` once the sprite is gone.
pub fn read_pose<P: PoseAccess, R>(
    owner: Owner,
    index: usize,
    kind: PoseKind,
    f: impl FnOnce(&P) -> R,
) -> Option<R> {
    if kind == PoseKind::Setup {
        return world::owner_data(owner, |data| P::in_data(&data.constraints[index]).map(f))
            .flatten();
    }
    world::skeleton(owner.entity()?, |skeleton| {
        let set = P::in_body(&skeleton.constraint(index).body)?;
        Some(f(if kind == PoseKind::Pose {
            &set.pose
        } else {
            set.applied()
        }))
    })
    .flatten()
}

/// Changes the pose `kind` of constraint `index`. The setup pose belongs to the shared skeleton
/// data, so changing it changes it for every sprite made from the data; they use it the next time
/// they apply their setup pose.
pub fn write_pose<P: PoseAccess>(
    owner: Owner,
    index: usize,
    kind: PoseKind,
    f: impl FnOnce(&mut P),
) {
    if kind == PoseKind::Setup {
        world::edit_data(owner, |data| {
            if let Some(pose) = P::in_data_mut(&mut data.constraints[index]) {
                f(pose);
            }
        });
        return;
    }
    let Some(entity) = owner.entity() else {
        return;
    };
    world::skeleton_mut(entity, |skeleton| {
        let body = &mut skeleton.constraint_mut(index).body;
        if let Some(set) = P::in_body_mut(body) {
            f(if kind == PoseKind::Applied {
                set.applied_mut()
            } else {
                &mut set.pose
            });
        }
    });
}

/// The mode index for the script value `value` when there are `count` modes. Logs a warning and
/// returns `None` for a value that is no mode.
pub fn mode_index(value: i32, count: u8) -> Option<u8> {
    let index = u8::try_from(value).ok().filter(|&index| index < count);
    if index.is_none() {
        godot_warn!("Spine: {value} is not a valid mode; not changed");
    }
    index
}

/// The id of the bone `data` describes if it comes from the same skeleton data as the skeleton
/// `entity`; logs a warning and returns `None` for the bone data of another skeleton data.
pub fn own_bone_data(owner: Owner, data: &Gd<SpineBoneData>) -> Option<BoneId> {
    let data = data.bind();
    if data.owner().data_id() == owner.data_id() {
        return Some(data.bone_id());
    }
    godot_warn!("Spine: the bone data belongs to another skeleton data");
    None
}

/// The id of the slot `data` describes if it comes from the same skeleton data as the skeleton
/// `entity`; logs a warning and returns `None` for the slot data of another skeleton data.
pub fn own_slot_data(owner: Owner, data: &Gd<SpineSlotData>) -> Option<SlotId> {
    let data = data.bind();
    if data.owner().data_id() == owner.data_id() {
        return Some(data.slot_id());
    }
    godot_warn!("Spine: the slot data belongs to another skeleton data");
    None
}

/// The id of `bone` if it belongs to the skeleton `entity`; logs a warning and returns `None` for
/// a bone of another skeleton.
pub fn own_bone(entity: Entity, bone: &Gd<SpineBone>) -> Option<BoneId> {
    let bone = bone.bind();
    if bone.entity() == entity {
        return Some(bone.bone_id());
    }
    godot_warn!("Spine: the bone belongs to another skeleton");
    None
}

/// The id of the bone `pose` belongs to if it is in the skeleton `entity`; logs a warning and
/// returns `None` for a pose of another skeleton.
pub fn own_bone_pose(entity: Entity, pose: &Gd<SpineBonePose>) -> Option<BoneId> {
    let pose = pose.bind();
    if pose.owner() == Owner::Skeleton(entity) {
        return Some(pose.bone_id());
    }
    godot_warn!("Spine: the bone pose belongs to another skeleton");
    None
}

/// The id of `slot` if it belongs to the skeleton `entity`; logs a warning and returns `None` for
/// a slot of another skeleton.
pub fn own_slot(entity: Entity, slot: &Gd<SpineSlot>) -> Option<SlotId> {
    let slot = slot.bind();
    if slot.entity() == entity {
        return Some(slot.slot_id());
    }
    godot_warn!("Spine: the slot belongs to another skeleton");
    None
}
