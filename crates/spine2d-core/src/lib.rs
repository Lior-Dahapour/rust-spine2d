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

//! Engine-agnostic core of the Spine 4.3 runtime, ported from `spine-cpp`.
//!
//! # Architecture
//!
//! The C++ runtime is a graph of pointers. Here the graph is flattened into index-based ids
//! ([`BoneId`], [`SlotId`]) into `Vec`s owned by [`Skeleton`], while the immutable setup data
//! ([`SkeletonData`]) is shared through an `Arc`.
//!
//! Every bone and slot has a setup pose (in data), a `pose` (written by animations and
//! application code), a `constrained` copy (modified by constraints), and an *applied* pose that
//! selects between the two. Use `applied_pose()` for rendering.
//!
//! # What is here
//!
//! math, color, enums, bones (including the full world/local transform math), slots, skeleton
//! data, events, attachments, skins, animations with every timeline, `AnimationState`, all five
//! constraints (IK, transform, path, physics, slider), the atlas parser, and the JSON and binary
//! skeleton loaders.
//!
//! The port is checked against the C++ reference runtime frame by frame; see the golden tests in
//! `tests/` (`golden.rs`, `golden_driver.rs`) and `tests/cpp/build.sh`.

pub mod animation;
pub mod animation_state;
pub mod atlas;
pub mod atlas_attachment_loader;
pub mod attachment;
pub mod attachment_loader;
pub mod bone;
pub mod color;
pub mod constraint;
pub mod constraint_data;
pub mod enums;
pub mod event;
pub mod json;
pub mod loader;
pub mod math;
pub mod sequence;
pub mod skeleton;
pub mod skeleton_data;
pub mod skin;
pub mod slot;
pub mod texture_region;

pub use animation::{
    AlphaTimeline, Animation, ApplyOptions, AttachmentTimeline, Bone1Kind, Bone2Kind,
    BoneTimeline1, BoneTimeline2, ColorKind, ColorTimeline, Constraint1Kind, ConstraintTimeline1,
    CurveTimeline, DeformTimeline, DrawOrderFolderTimeline, DrawOrderTimeline, EventTimeline,
    IkConstraintTimeline, InheritTimeline, Interpolation, MixFrom, PathConstraintMixTimeline,
    PhysicsConstraintResetTimeline, Property, PropertyId, SequenceTimeline, Timeline,
    TransformConstraintTimeline,
};
pub use animation_state::{
    empty_animation, AnimationState, AnimationStateData, EntryId, EventType, Listener, StateError,
    TrackEntry,
};
pub use atlas::{Atlas, AtlasPage, AtlasRegion, TextureLoader};
pub use atlas_attachment_loader::AtlasAttachmentLoader;
pub use attachment::{
    Attachment, AttachmentBase, AttachmentId, BoundingBoxAttachment, ClippingAttachment,
    MeshAttachment, PathAttachment, PointAttachment, RegionAttachment, VertexAttachment,
};
pub use attachment_loader::AttachmentLoader;
pub use bone::{
    is_y_down, set_y_down, Bone, BoneData, BoneId, BoneLocal, BonePose, SkeletonTransform,
    WorldMatrix,
};
pub use color::Color;
pub use constraint::{
    Constraint, ConstraintBody, IkConstraint, PathConstraint, PhysicsConstraint, PoseSet, Slider,
    TransformConstraint,
};
pub use constraint_data::{
    BoneProperty, ConstraintData, FromProperty, IkConstraintData, IkConstraintPose,
    PathConstraintData, PathConstraintPose, PhysicsConstraintData, PhysicsConstraintPose,
    PositionMode, RotateMode, ScaleYMode, SliderData, SliderPose, SpacingMode, ToProperty,
    TransformConstraintData, TransformConstraintPose,
};
pub use enums::{BlendMode, Inherit, Physics};
pub use event::{Event, EventData};
pub use loader::{LoadError, SkeletonBinary, SkeletonJson};
pub use sequence::{Sequence, SequenceMode};
pub use skeleton::Skeleton;
pub use skeleton_data::SkeletonData;
pub use skin::{Skin, SkinEntry};
pub use slot::{Slot, SlotData, SlotId, SlotPose};
pub use smol_str::SmolStr;
pub use texture_region::{AtlasRegionInfo, TextureRegion};
