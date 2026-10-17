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

use super::{
    AlphaTimeline, ApplyOptions, AttachmentTimeline, BoneTimeline1, BoneTimeline2, ColorTimeline,
    ConstraintTimeline1, DeformTimeline, DrawOrderFolderTimeline, DrawOrderTimeline, EventTimeline,
    IkConstraintTimeline, InheritTimeline, PathConstraintMixTimeline,
    PhysicsConstraintResetTimeline, PropertyId, SequenceTimeline, TransformConstraintTimeline,
};
use crate::event::Event;
use crate::skeleton::BorrowedSkeleton;
use smallvec::SmallVec;
use std::sync::Arc;

/// A timeline: keyed values that pose part of a skeleton at a point in time.
#[derive(Clone, Debug)]
pub enum Timeline {
    Bone1(BoneTimeline1),
    Bone2(BoneTimeline2),
    Inherit(InheritTimeline),
    Color(ColorTimeline),
    Alpha(AlphaTimeline),
    Attachment(AttachmentTimeline),
    Deform(DeformTimeline),
    Sequence(SequenceTimeline),
    DrawOrder(DrawOrderTimeline),
    DrawOrderFolder(DrawOrderFolderTimeline),
    Event(EventTimeline),
    Ik(IkConstraintTimeline),
    Transform(TransformConstraintTimeline),
    PathMix(PathConstraintMixTimeline),
    Constraint1(ConstraintTimeline1),
    PhysicsReset(PhysicsConstraintResetTimeline),
}

macro_rules! each {
    ($self:expr, $t:ident => $e:expr) => {
        match $self {
            Timeline::Bone1($t) => $e,
            Timeline::Bone2($t) => $e,
            Timeline::Inherit($t) => $e,
            Timeline::Color($t) => $e,
            Timeline::Alpha($t) => $e,
            Timeline::Attachment($t) => $e,
            Timeline::Deform($t) => $e,
            Timeline::Sequence($t) => $e,
            Timeline::DrawOrder($t) => $e,
            Timeline::DrawOrderFolder($t) => $e,
            Timeline::Event($t) => $e,
            Timeline::Ik($t) => $e,
            Timeline::Transform($t) => $e,
            Timeline::PathMix($t) => $e,
            Timeline::Constraint1($t) => $e,
            Timeline::PhysicsReset($t) => $e,
        }
    };
}

impl Timeline {
    /// The keyed floats: `frame_entries()` per frame, the first being the time.
    pub fn frames(&self) -> &[f32] {
        match self {
            Timeline::Bone1(t) => &t.curve.frames,
            Timeline::Bone2(t) => &t.curve.frames,
            Timeline::Color(t) => &t.curve.frames,
            Timeline::Alpha(t) => &t.curve.frames,
            Timeline::Deform(t) => &t.curve.frames,
            Timeline::Ik(t) => &t.curve.frames,
            Timeline::Transform(t) => &t.curve.frames,
            Timeline::PathMix(t) => &t.curve.frames,
            Timeline::Constraint1(t) => &t.curve.frames,
            Timeline::PhysicsReset(t) => &t.frames,
            Timeline::Inherit(t) => &t.frames,
            Timeline::Attachment(t) => &t.frames,
            Timeline::Sequence(t) => &t.frames,
            Timeline::DrawOrder(t) => &t.frames,
            Timeline::DrawOrderFolder(t) => &t.frames,
            Timeline::Event(t) => &t.frames,
        }
    }

    /// Floats per frame.
    pub fn frame_entries(&self) -> usize {
        match self {
            Timeline::Bone1(t) => t.curve.frame_entries(),
            Timeline::Bone2(t) => t.curve.frame_entries(),
            Timeline::Color(t) => t.curve.frame_entries(),
            Timeline::Alpha(t) => t.curve.frame_entries(),
            Timeline::Deform(t) => t.curve.frame_entries(),
            Timeline::Ik(t) => t.curve.frame_entries(),
            Timeline::Transform(t) => t.curve.frame_entries(),
            Timeline::PathMix(t) => t.curve.frame_entries(),
            Timeline::Constraint1(t) => t.curve.frame_entries(),
            Timeline::Inherit(_) => 2,
            Timeline::Sequence(_) => 3,
            Timeline::Attachment(_)
            | Timeline::DrawOrder(_)
            | Timeline::DrawOrderFolder(_)
            | Timeline::Event(_)
            | Timeline::PhysicsReset(_) => 1,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames().len() / self.frame_entries()
    }

    /// The time of the last frame.
    pub fn duration(&self) -> f32 {
        let frames = self.frames();
        frames[frames.len() - self.frame_entries()]
    }

    /// True if this timeline supports additive blending.
    pub fn additive(&self) -> bool {
        match self {
            Timeline::Bone1(_)
            | Timeline::Bone2(_)
            | Timeline::Deform(_)
            | Timeline::Transform(_) => true,
            Timeline::Constraint1(t) => t.kind.additive(),
            _ => false,
        }
    }

    /// True if this timeline sets values instantly and does not interpolate between frames.
    pub fn instant(&self) -> bool {
        matches!(
            self,
            Timeline::Inherit(_)
                | Timeline::Attachment(_)
                | Timeline::Sequence(_)
                | Timeline::DrawOrder(_)
                | Timeline::DrawOrderFolder(_)
                | Timeline::Event(_)
                | Timeline::PhysicsReset(_)
        )
    }

    /// What this timeline changes; see [`Property::id`](super::Property::id).
    pub fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        each!(self, t => t.property_ids())
    }

    /// The skeleton bone index this timeline targets, for bone timelines.
    pub fn bone_index(&self) -> Option<usize> {
        match self {
            Timeline::Bone1(t) => Some(t.bone),
            Timeline::Bone2(t) => Some(t.bone),
            Timeline::Inherit(t) => Some(t.bone),
            _ => None,
        }
    }

    /// The skeleton slot index this timeline targets, for slot timelines.
    pub fn slot_index(&self) -> Option<usize> {
        match self {
            Timeline::Color(t) => Some(t.slot),
            Timeline::Alpha(t) => Some(t.slot),
            Timeline::Attachment(t) => Some(t.slot),
            Timeline::Deform(t) => Some(t.slot),
            Timeline::Sequence(t) => Some(t.slot),
            _ => None,
        }
    }

    /// The constraint index this timeline targets, for constraint timelines that target one.
    pub fn constraint_index(&self) -> Option<usize> {
        match self {
            Timeline::Ik(t) => Some(t.constraint),
            Timeline::Transform(t) => Some(t.constraint),
            Timeline::PathMix(t) => Some(t.constraint),
            Timeline::Constraint1(t) => t.constraint,
            Timeline::PhysicsReset(t) => t.constraint,
            _ => None,
        }
    }

    /// True for physics timelines that apply to every physics constraint.
    pub fn is_global_physics(&self) -> bool {
        match self {
            Timeline::Constraint1(t) => t.kind.is_physics() && t.constraint.is_none(),
            Timeline::PhysicsReset(t) => t.constraint.is_none(),
            _ => false,
        }
    }

    /// Applies this timeline to the skeleton.
    ///
    /// `last_time` is the last time this timeline was applied; discrete timelines trigger all keys
    /// between `last_time` (exclusive) and `time` (inclusive). Pass -1 the first time to trigger
    /// frame 0. Fired events are appended to `events`, which may be `None` to ignore them.
    pub fn apply(
        &self,
        skeleton: &mut BorrowedSkeleton,
        last_time: f32,
        time: f32,
        events: Option<&mut Vec<Arc<Event>>>,
        options: &ApplyOptions,
    ) {
        let (skeleton, skeleton_data) = skeleton.parts();
        match self {
            Timeline::Bone1(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Bone2(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Inherit(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Color(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Alpha(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Attachment(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Deform(t) => t.apply(skeleton, time, options),
            Timeline::Sequence(t) => t.apply(skeleton, time, options),
            Timeline::DrawOrder(t) => t.apply(skeleton, time, options),
            Timeline::DrawOrderFolder(t) => t.apply(skeleton, time, options),
            Timeline::Event(t) => t.apply(last_time, time, events),
            Timeline::Ik(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Transform(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::PathMix(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::Constraint1(t) => t.apply(skeleton, skeleton_data, time, options),
            Timeline::PhysicsReset(t) => t.apply(skeleton, last_time, time),
        }
    }
}
