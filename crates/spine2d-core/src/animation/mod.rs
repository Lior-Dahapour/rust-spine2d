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

//! Animations and timelines.
//!
//! An [`Animation`] is a list of [`Timeline`]s that pose a skeleton at a point in time. Timelines
//! are a closed enum rather than a trait hierarchy: dispatch is static, and the state machine can
//! match on variants where the reference runtime uses RTTI.

mod attachment;
mod bone;
mod constraint;
mod curve;
mod draw_order;
mod event;
mod interpolation;
mod slot;
mod timeline;

pub use attachment::{AttachmentTimeline, DeformTimeline, SequenceTimeline};
pub use bone::{Bone1Kind, Bone2Kind, BoneTimeline1, BoneTimeline2, InheritTimeline};
pub use constraint::{
    Constraint1Kind, ConstraintTimeline1, IkConstraintTimeline, PathConstraintMixTimeline,
    PhysicsConstraintResetTimeline, TransformConstraintTimeline,
};
pub use curve::{before_first_key, CurveTimeline, BEZIER, BEZIER_SIZE, LINEAR, STEPPED};
pub use draw_order::{DrawOrderFolderTimeline, DrawOrderTimeline};
pub use event::EventTimeline;
pub use interpolation::Interpolation;
pub use slot::{AlphaTimeline, ColorKind, ColorTimeline};
pub use timeline::Timeline;

use crate::color::Color;
use crate::event::Event;
use crate::math::fmod;
use crate::skeleton::Skeleton;
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use std::sync::Arc;

/// Identifies what a timeline changes: the property in the high 32 bits, the target in the low.
/// Used to find timelines that conflict between animations.
pub type PropertyId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Property {
    Rotate = 0,
    X,
    Y,
    ScaleX,
    ScaleY,
    ShearX,
    ShearY,
    Inherit,
    Rgb,
    Alpha,
    Rgb2,
    Attachment,
    Deform,
    Event,
    DrawOrder,
    IkConstraint,
    TransformConstraint,
    PathConstraintPosition,
    PathConstraintSpacing,
    PathConstraintMix,
    PhysicsConstraintInertia,
    PhysicsConstraintStrength,
    PhysicsConstraintDamping,
    PhysicsConstraintMass,
    PhysicsConstraintWind,
    PhysicsConstraintGravity,
    PhysicsConstraintMix,
    PhysicsConstraintReset,
    Sequence,
    SliderTime,
    SliderMix,
    DrawOrderFolder,
}

impl Property {
    /// The property id for a target index (bone, slot, constraint, ...).
    #[inline]
    pub fn id(self, target: usize) -> PropertyId {
        ((self as u64) << 32) | target as u64
    }
}

/// How a timeline's values combine with the skeleton's current pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixFrom {
    /// Alpha transitions between the current and timeline values; nothing changes before the
    /// first key.
    Current = 0,
    /// Alpha transitions between the setup and timeline values; setup values are used before the
    /// first key and current values are ignored.
    Setup = 1,
    /// Like `Current`, but before the first key the current values transition to setup.
    First = 2,
}

impl MixFrom {
    pub(crate) fn from_bits(bits: u8) -> MixFrom {
        match bits & 3 {
            1 => MixFrom::Setup,
            2 => MixFrom::First,
            _ => MixFrom::Current,
        }
    }
}

/// Parameters shared by [`Animation::apply`] and [`Timeline::apply`].
#[derive(Clone, Copy, Debug)]
pub struct ApplyOptions {
    /// 0 applies setup or current values (depending on `from`), 1 uses timeline values, and
    /// intermediate values interpolate between them.
    pub alpha: f32,
    pub from: MixFrom,
    /// For timelines that support it (`Timeline::additive`), add values to the setup or current
    /// values (depending on `from`) instead of replacing them.
    pub add: bool,
    /// True when the animation is mixing out; used by timelines that change instantly.
    pub out: bool,
    /// True to modify the applied pose, else the unconstrained pose.
    pub applied_pose: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        ApplyOptions {
            alpha: 1.0,
            from: MixFrom::Current,
            add: false,
            out: false,
            applied_pose: false,
        }
    }
}

/// Index of the frame at or before `target` in a one-float-per-frame `frames` array.
/// `target` must be at or after the first frame; the result is the last frame index otherwise
/// equal to the reference runtime's linear scan.
#[inline]
pub fn search(frames: &[f32], target: f32) -> usize {
    search_step(frames, target, 1)
}

/// Like [`search`] for frames with `step` floats each; returns the float index of the frame.
#[inline]
pub fn search_step(frames: &[f32], target: f32, step: usize) -> usize {
    let n = frames.len() / step;
    // Linear is faster for the short timelines that dominate in practice.
    if n <= 8 {
        for k in 1..n {
            if frames[k * step] > target {
                return (k - 1) * step;
            }
        }
        return (n - 1) * step;
    }
    // First k in 1..n with frames[k] > target, else n.
    let (mut lo, mut hi) = (1, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if frames[mid * step] > target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    (lo - 1) * step
}

/// Stores a list of timelines to animate a skeleton's pose over time.
#[derive(Clone, Debug)]
pub struct Animation {
    name: SmolStr,
    timelines: Vec<Timeline>,
    timeline_ids: FxHashSet<PropertyId>,
    bones: Vec<usize>,
    /// The duration in seconds, usually the highest frame time of all timelines. Used to know
    /// when the animation has completed and when it loops.
    pub duration: f32,
    /// The color of the animation in the editor; has no runtime effect.
    pub color: Color,
}

impl Animation {
    pub fn new(name: impl Into<SmolStr>, timelines: Vec<Timeline>, duration: f32) -> Animation {
        let mut animation = Animation {
            name: name.into(),
            timelines: Vec::new(),
            timeline_ids: FxHashSet::default(),
            bones: Vec::new(),
            duration,
            color: Color::WHITE,
        };
        animation.set_timelines(timelines);
        animation
    }

    /// Like [`new`](Self::new) but with an explicit list of affected bone indices, in the order
    /// the exporter listed them (including bones that have no timelines).
    pub fn new_with_bones(
        name: impl Into<SmolStr>,
        timelines: Vec<Timeline>,
        duration: f32,
        bones: Vec<usize>,
    ) -> Animation {
        let mut animation = Animation::new(name, timelines, duration);
        animation.bones = bones;
        animation
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn timelines(&self) -> &[Timeline] {
        &self.timelines
    }

    /// Indices of the skeleton bones affected by this animation.
    pub fn bones(&self) -> &[usize] {
        &self.bones
    }

    /// Replaces the timelines and recomputes the affected bone indices and property ids.
    pub fn set_timelines(&mut self, timelines: Vec<Timeline>) {
        self.timeline_ids.clear();
        self.bones.clear();
        for timeline in &timelines {
            self.timeline_ids.extend(timeline.property_ids());
            if let Some(bone) = timeline.bone_index() {
                if !self.bones.contains(&bone) {
                    self.bones.push(bone);
                }
            }
        }
        self.timelines = timelines;
    }

    /// True if this animation contains a timeline with any of the given property ids.
    pub fn has_timeline(&self, ids: &[PropertyId]) -> bool {
        ids.iter().any(|id| self.timeline_ids.contains(id))
    }

    /// Applies the animation's timelines to the skeleton.
    ///
    /// `last_time` is the last time this animation was applied; discrete timelines trigger all
    /// keys between `last_time` (exclusive) and `time` (inclusive). Pass -1 the first time to
    /// trigger frame 0. If `loop_` is true, times beyond the duration wrap. Fired events are
    /// appended to `events`.
    pub fn apply(
        &self,
        skeleton: &mut Skeleton,
        mut last_time: f32,
        mut time: f32,
        loop_: bool,
        mut events: Option<&mut Vec<Arc<Event>>>,
        options: &ApplyOptions,
    ) {
        if loop_ && self.duration != 0.0 {
            time = fmod(time, self.duration);
            if last_time > 0.0 {
                last_time = fmod(last_time, self.duration);
            }
        }
        for timeline in &self.timelines {
            timeline.apply(skeleton, last_time, time, events.as_deref_mut(), options);
        }
    }
}
