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

//! The components that make up a skeleton entity.
//!
//! Create an entity with [`crate::spawn_skeleton`], which adds the pose, the animation state and
//! the command queue. Components only hold data; the systems in [`crate::systems`] do the work.

use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::Component;
use glam::{Affine2, Vec2};
use smol_str::SmolStr;
use spine2d_core::{AnimationState, BoneId, BonePose, Skeleton, SkeletonData, SlotId};
use spine2d_render::Bounds;

/// What a skeleton does when it first appears, passed to [`crate::spawn_skeleton`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpineInitial {
    /// Name of the skin to show. The default skin is used when `None`.
    pub skin: Option<SmolStr>,
    /// Name of an animation to start on track 0.
    pub animation: Option<SmolStr>,
    /// Whether that animation loops.
    pub looping: bool,
}

/// Where the data of a skeleton is stored: the key a [`crate::SpineResource`] looks it up with.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpineData<K: Send + Sync + 'static>(pub K);

/// The live skeleton of an entity: its bones, slots and current pose.
#[derive(Component, Debug, Deref, DerefMut)]
pub struct SpineSkeleton(pub(crate) Skeleton);

/// The animation state of an entity: its tracks, queued animations and mixing.
#[derive(Component, Deref, DerefMut)]
pub struct SpineAnimation(pub(crate) AnimationState);

/// The bounding box of what the skeleton draws, in skeleton space. Add it to a skeleton entity to
/// have it kept up to date; skeletons without it pay nothing.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Deref, DerefMut)]
pub struct SpineBounds(pub(crate) Bounds);

/// Present on every skeleton from its spawn. An engine flags it with `set_changed()` after it
/// changes what a constraint refers to (its target, source, slot or bone), and the next frame
/// rebuilds the skeleton's update order once, however many edits there were. See
/// [`crate::systems::rebuild_stale_caches`].
///
/// Changed-detection fires for a component in the frame it is added too, so each new skeleton
/// rebuilds once on its first frame.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpineCacheStale;

/// Marks skeletons that advance with the engine's process frame. A skeleton with neither this nor
/// [`SpinePhysics`] is manual: no schedule moves it, and the integration advances it itself with
/// [`crate::systems::advance_skeleton`] and the functions next to it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpineProcess;

/// Marks skeletons that advance with the engine's physics tick.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpinePhysics;

/// Makes a child entity of a skeleton follow one of its bones.
///
/// Spawn it with [`bevy_ecs::hierarchy::ChildOf`] pointing at the skeleton entity. What to do with
/// the placement is up to the engine: its system in the `Follow` stage calls
/// [`SpineFollower::follow`] and moves the engine's own object when that reports a change. A
/// skeleton nobody follows pays nothing.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SpineFollower {
    name: SmolStr,
    bone: BoneId,
    transform: Affine2,
}

impl SpineFollower {
    /// A follower of the bone called `name`, or `None` if the skeleton has no such bone. `data` is
    /// the data `skeleton` was made from.
    pub fn for_bone(skeleton: &Skeleton, data: &SkeletonData, name: &str) -> Option<Self> {
        let bone = BoneId(data.find_bone(name)?.index);
        Some(Self::new(name, bone, skeleton))
    }

    /// A follower of the bone the slot called `name` is attached to, or `None` if the skeleton has
    /// no such slot. `data` is the data `skeleton` was made from.
    pub fn for_slot(skeleton: &Skeleton, data: &SkeletonData, name: &str) -> Option<Self> {
        let bone = skeleton.slot(SlotId(data.find_slot(name)?.index)).bone();
        Some(Self::new(name, bone, skeleton))
    }

    fn new(name: &str, bone: BoneId, skeleton: &Skeleton) -> Self {
        SpineFollower {
            name: name.into(),
            bone,
            transform: bone_transform(skeleton.bone(bone).applied_pose()),
        }
    }

    /// The bone or slot name this follower was made for.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The bone's world transform in skeleton space as of the last `Follow` stage: the columns are
    /// the bone's x axis, y axis and origin.
    pub fn transform(&self) -> Affine2 {
        self.transform
    }

    /// Reads the bone's current transform from `skeleton`. Returns whether it changed.
    pub fn follow(&mut self, skeleton: &Skeleton) -> bool {
        let placed = bone_transform(skeleton.bone(self.bone).applied_pose());
        let moved = self.transform != placed;
        self.transform = placed;
        moved
    }
}

/// Makes a child entity of a skeleton drive one of its bones: the bone takes the placement of
/// this component, as the engine's own object moves.
///
/// Spawn it with [`bevy_ecs::hierarchy::ChildOf`] pointing at the skeleton entity and change
/// [`SpineDriver::transform`] whenever the object moves. The `drive_bones` system places the bone
/// in the `Transforms` stage, after the animation posed it and before the world transforms are
/// computed, so the animation's own pose of that bone is replaced.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SpineDriver {
    bone: BoneId,
    /// The world transform the bone takes, in skeleton space: the columns are the bone's x axis,
    /// y axis and origin, as [`SpineFollower::transform`] gives them.
    pub transform: Affine2,
}

impl SpineDriver {
    /// A driver of the bone called `name`, or `None` if the data has no such bone.
    pub fn for_bone(data: &SkeletonData, name: &str, transform: Affine2) -> Option<Self> {
        let bone = BoneId(data.find_bone(name)?.index);
        Some(SpineDriver { bone, transform })
    }

    /// The bone this driver places.
    pub fn bone(&self) -> BoneId {
        self.bone
    }
}

fn bone_transform(pose: &BonePose) -> Affine2 {
    Affine2::from_cols(
        Vec2::new(pose.a, pose.c),
        Vec2::new(pose.b, pose.d),
        Vec2::new(pose.world_x, pose.world_y),
    )
}

/// A step a skeleton takes at this frame because one was requested with
/// [`crate::systems::begin_step`]. Removed at the end of the frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpineStep {
    delta: f32,
    /// The skeleton has no mode of its own: [`SpineProcess`] was added for this frame, and the
    /// frame time does not move it.
    manual: bool,
}

impl SpineStep {
    pub fn new(delta: f32, manual: bool) -> Self {
        SpineStep { delta, manual }
    }

    /// Seconds to advance, on top of the frame time unless the step is manual.
    pub fn delta(&self) -> f32 {
        self.delta
    }

    /// Whether the skeleton moves only by this step.
    pub fn is_manual(&self) -> bool {
        self.manual
    }
}
