// Spine Runtimes License Agreement
// Last updated April 5, 2025. Replaces all prior versions.
//
// Copyright (c) 2013-2025, Esoteric Software LLC
//
// Integration of the Spine Runtimes into software or otherwise creating
// derivative works of the Spine Runtimes is permitted under the terms and
// conditions of Section 2 of the Spine Editor License Agreement:
// http://esotericsoftware.com/spine-editor-license
//
// Otherwise, it is permitted to integrate the Spine Runtimes into software
// or otherwise create derivative works of the Spine Runtimes (collectively,
// "Products"), provided that each user of the Products must obtain their own
// Spine Editor license and redistribution of the Products in any form must
// include this license and copyright notice.
//
// THE SPINE RUNTIMES ARE PROVIDED BY ESOTERIC SOFTWARE LLC "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL ESOTERIC SOFTWARE LLC BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES,
// BUSINESS INTERRUPTION, OR LOSS OF USE, DATA, OR PROFITS) HOWEVER CAUSED AND
// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
// THE SPINE RUNTIMES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Unofficial Rust port of the Spine 4.3 runtime (spine-cpp). Not an official Spine runtime.

//! Constraint instances: IK, transform, path, physics and slider constraints.
//!
//! Setup data lives in [`ConstraintData`]; a [`Skeleton`] owns one
//! [`Constraint`] per data entry. Like bones, a constraint has a `pose` (set by animations and
//! application code), a `constrained` copy and an applied pose that selects between them.

mod ik;
mod path;
mod physics;
mod slider;
mod transform;

pub use path::PathConstraint;
pub use physics::PhysicsConstraint;
pub use slider::Slider;
pub use transform::{from_value, to_apply, to_mix};

use crate::bone::BoneId;
use crate::constraint_data::*;
use crate::enums::Physics;
use crate::skeleton::Skeleton;
use crate::skeleton_data::SkeletonData;

/// A pose with its constrained copy and which of the two is applied.
#[derive(Clone, Debug)]
pub struct PoseSet<P> {
    /// The unconstrained pose, set by animations and application code.
    pub pose: P,
    pub(crate) constrained: P,
    pub(crate) use_constrained: bool,
}

impl<P: Copy + Default> PoseSet<P> {
    pub(crate) fn new(setup: P) -> Self {
        PoseSet {
            pose: setup,
            constrained: P::default(),
            use_constrained: false,
        }
    }

    /// The pose to use: `pose`, or the constrained copy if another constraint modifies it.
    pub fn applied(&self) -> &P {
        if self.use_constrained {
            &self.constrained
        } else {
            &self.pose
        }
    }

    pub fn applied_mut(&mut self) -> &mut P {
        if self.use_constrained {
            &mut self.constrained
        } else {
            &mut self.pose
        }
    }

    /// The pose animations modify: the applied pose if `applied_pose`, else the unconstrained pose.
    pub(crate) fn target(&mut self, applied_pose: bool) -> &mut P {
        if applied_pose {
            self.applied_mut()
        } else {
            &mut self.pose
        }
    }

    pub(crate) fn reset_constrained(&mut self) {
        self.constrained = self.pose;
    }

    pub(crate) fn set_constrained(&mut self) {
        self.use_constrained = true;
    }

    pub(crate) fn set_unconstrained(&mut self) {
        self.use_constrained = false;
    }

    pub fn is_pose_equal_to_applied(&self) -> bool {
        !self.use_constrained
    }
}

#[derive(Clone, Debug)]
pub struct IkConstraint {
    pub pose: PoseSet<IkConstraintPose>,
    pub(crate) target: BoneId,
}

impl IkConstraint {
    /// The bone the constrained bones reach for.
    pub fn target(&self) -> BoneId {
        self.target
    }

    /// Makes the constrained bones reach for `bone`. Call [`update_cache`](crate::skeleton::Borrowed::update_cache) afterwards so
    /// `bone` is updated before the constraint; `bone` must not depend on the constrained bones.
    pub fn set_target(&mut self, bone: BoneId) {
        self.target = bone;
    }
}

#[derive(Clone, Debug)]
pub struct TransformConstraint {
    pub pose: PoseSet<TransformConstraintPose>,
    pub(crate) source: BoneId,
}

impl TransformConstraint {
    /// The bone the constrained bones copy from.
    pub fn source(&self) -> BoneId {
        self.source
    }

    /// Makes the constrained bones copy from `bone`. Call [`update_cache`](crate::skeleton::Borrowed::update_cache) afterwards so
    /// `bone` is updated before the constraint; `bone` must not depend on the constrained bones.
    pub fn set_source(&mut self, bone: BoneId) {
        self.source = bone;
    }
}

/// The variant-specific part of a [`Constraint`].
#[derive(Clone, Debug)]
pub enum ConstraintBody {
    Ik(IkConstraint),
    Transform(TransformConstraint),
    Path(PathConstraint),
    Physics(PhysicsConstraint),
    Slider(Slider),
}

/// A constraint instance within a skeleton.
#[derive(Clone, Debug)]
pub struct Constraint {
    pub(crate) index: usize,
    pub(crate) active: bool,
    pub body: ConstraintBody,
}

impl Constraint {
    pub(crate) fn new(index: usize, data: &ConstraintData) -> Constraint {
        let body = match data {
            ConstraintData::Ik(d) => ConstraintBody::Ik(IkConstraint {
                pose: PoseSet::new(d.setup_pose),
                target: BoneId(d.target),
            }),
            ConstraintData::Transform(d) => ConstraintBody::Transform(TransformConstraint {
                pose: PoseSet::new(d.setup_pose),
                source: BoneId(d.source),
            }),
            ConstraintData::Path(d) => ConstraintBody::Path(PathConstraint::new(d)),
            ConstraintData::Physics(d) => ConstraintBody::Physics(PhysicsConstraint::new(d)),
            ConstraintData::Slider(d) => ConstraintBody::Slider(Slider::new(d)),
        };
        Constraint {
            index,
            active: true,
            body,
        }
    }

    /// Index into `SkeletonData::constraints` (and `Skeleton::constraints`).
    pub fn index(&self) -> usize {
        self.index
    }

    /// False when skin-required and the active skin doesn't include this constraint, or when its
    /// source is inactive.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Sets the flag [`Constraint::is_active`] reports. The update loop does not read it: the
    /// skeleton recomputes it from the skin in `update_cache`, so a value set here lasts only
    /// until the next rebuild.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub(crate) fn setup_pose(&mut self, data: &ConstraintData) {
        match (&mut self.body, data) {
            (ConstraintBody::Ik(c), ConstraintData::Ik(d)) => c.pose.pose = d.setup_pose,
            (ConstraintBody::Transform(c), ConstraintData::Transform(d)) => {
                c.pose.pose = d.setup_pose
            }
            (ConstraintBody::Path(c), ConstraintData::Path(d)) => c.pose.pose = d.setup_pose,
            (ConstraintBody::Physics(c), ConstraintData::Physics(d)) => c.pose.pose = d.setup_pose,
            (ConstraintBody::Slider(c), ConstraintData::Slider(d)) => c.pose.pose = d.setup_pose,
            _ => {}
        }
    }

    pub(crate) fn reset_constrained(&mut self) {
        match &mut self.body {
            ConstraintBody::Ik(c) => c.pose.reset_constrained(),
            ConstraintBody::Transform(c) => c.pose.reset_constrained(),
            ConstraintBody::Path(c) => c.pose.reset_constrained(),
            ConstraintBody::Physics(c) => c.pose.reset_constrained(),
            ConstraintBody::Slider(c) => c.pose.reset_constrained(),
        }
    }

    pub(crate) fn set_constrained(&mut self) {
        match &mut self.body {
            ConstraintBody::Ik(c) => c.pose.set_constrained(),
            ConstraintBody::Transform(c) => c.pose.set_constrained(),
            ConstraintBody::Path(c) => c.pose.set_constrained(),
            ConstraintBody::Physics(c) => c.pose.set_constrained(),
            ConstraintBody::Slider(c) => c.pose.set_constrained(),
        }
    }

    pub(crate) fn set_unconstrained(&mut self) {
        match &mut self.body {
            ConstraintBody::Ik(c) => c.pose.set_unconstrained(),
            ConstraintBody::Transform(c) => c.pose.set_unconstrained(),
            ConstraintBody::Path(c) => c.pose.set_unconstrained(),
            ConstraintBody::Physics(c) => c.pose.set_unconstrained(),
            ConstraintBody::Slider(c) => c.pose.set_unconstrained(),
        }
    }

    pub fn is_pose_equal_to_applied(&self) -> bool {
        match &self.body {
            ConstraintBody::Ik(c) => c.pose.is_pose_equal_to_applied(),
            ConstraintBody::Transform(c) => c.pose.is_pose_equal_to_applied(),
            ConstraintBody::Path(c) => c.pose.is_pose_equal_to_applied(),
            ConstraintBody::Physics(c) => c.pose.is_pose_equal_to_applied(),
            ConstraintBody::Slider(c) => c.pose.is_pose_equal_to_applied(),
        }
    }
}

/// Whether the constraint's source (target bone, slot's bone, ...) is active.
pub(crate) fn is_source_active(sk: &Skeleton, constraint: &Constraint) -> bool {
    let bone_active = |b: BoneId| sk.bones[b.0].active;
    match &constraint.body {
        ConstraintBody::Ik(c) => bone_active(c.target),
        ConstraintBody::Transform(c) => bone_active(c.source),
        ConstraintBody::Path(c) => bone_active(sk.slots[c.slot.0].bone),
        ConstraintBody::Physics(c) => bone_active(c.bone),
        ConstraintBody::Slider(c) => c.bone.is_none_or(bone_active),
    }
}

/// Adds the constraint to the skeleton's update cache and marks what it modifies.
pub(crate) fn sort(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    match &sk.constraints[index].body {
        ConstraintBody::Ik(_) => ik::sort(sk, data, index),
        ConstraintBody::Transform(_) => transform::sort(sk, data, index),
        ConstraintBody::Path(_) => path::sort(sk, data, index),
        ConstraintBody::Physics(_) => physics::sort(sk, index),
        ConstraintBody::Slider(_) => slider::sort(sk, data, index),
    }
}

/// Applies the constraint.
pub(crate) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize, physics: Physics) {
    match &sk.constraints[index].body {
        ConstraintBody::Ik(_) => ik::update(sk, data, index),
        ConstraintBody::Transform(_) => transform::update(sk, data, index),
        ConstraintBody::Path(_) => path::update(sk, data, index),
        ConstraintBody::Physics(_) => physics::update(sk, data, index, physics),
        ConstraintBody::Slider(_) => slider::update(sk, data, index),
    }
}
