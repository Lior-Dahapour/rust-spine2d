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

//! Attachments: the drawable or queryable things slots show.
//!
//! An [`Attachment`] is immutable once built and shared as `Arc<Attachment>` between skins and
//! slot poses. Per-instance mutable state (deform, sequence frame) lives on the `SlotPose`.

mod mesh;
mod region;
mod simple;
mod vertex;

pub use mesh::{MeshAttachment, MeshError};
pub use region::RegionAttachment;
pub use simple::{BoundingBoxAttachment, ClippingAttachment, PathAttachment, PointAttachment};
pub use vertex::VertexAttachment;

use crate::skeleton::Skeleton;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique identity of an attachment instance. Replaces the pointer identity the reference
/// runtime uses to compare attachments and their timeline attachments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttachmentId(pub u32);

/// Data common to all attachments.
#[derive(Clone, Debug)]
pub struct AttachmentBase {
    pub id: AttachmentId,
    pub name: SmolStr,
    /// The attachment whose timelines (deform, sequence) apply to this one. `None` means itself.
    /// Linked meshes can point at their source mesh to share its timelines.
    pub timeline_attachment: Option<AttachmentId>,
    /// Other slots whose current attachment shares this attachment's timelines.
    pub timeline_slots: SmallVec<[usize; 2]>,
}

impl AttachmentBase {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        let name = name.into();
        debug_assert!(!name.is_empty(), "attachment name must not be empty");
        AttachmentBase {
            id: AttachmentId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            name,
            timeline_attachment: None,
            timeline_slots: SmallVec::new(),
        }
    }

    /// The id of the attachment whose timelines apply to this one.
    pub fn timeline_id(&self) -> AttachmentId {
        self.timeline_attachment.unwrap_or(self.id)
    }
}

#[derive(Clone, Debug)]
pub enum Attachment {
    Region(RegionAttachment),
    Mesh(MeshAttachment),
    BoundingBox(BoundingBoxAttachment),
    Path(PathAttachment),
    Point(PointAttachment),
    Clipping(ClippingAttachment),
}

impl Attachment {
    pub fn base(&self) -> &AttachmentBase {
        match self {
            Attachment::Region(a) => &a.base,
            Attachment::Mesh(a) => &a.vertex.base,
            Attachment::BoundingBox(a) => &a.vertex.base,
            Attachment::Path(a) => &a.vertex.base,
            Attachment::Point(a) => &a.base,
            Attachment::Clipping(a) => &a.vertex.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut AttachmentBase {
        match self {
            Attachment::Region(a) => &mut a.base,
            Attachment::Mesh(a) => &mut a.vertex.base,
            Attachment::BoundingBox(a) => &mut a.vertex.base,
            Attachment::Path(a) => &mut a.vertex.base,
            Attachment::Point(a) => &mut a.base,
            Attachment::Clipping(a) => &mut a.vertex.base,
        }
    }

    pub fn id(&self) -> AttachmentId {
        self.base().id
    }

    pub fn name(&self) -> &str {
        &self.base().name
    }

    pub fn timeline_id(&self) -> AttachmentId {
        self.base().timeline_id()
    }

    /// The vertex data for mesh, path, bounding box and clipping attachments.
    pub fn as_vertex(&self) -> Option<&VertexAttachment> {
        match self {
            Attachment::Mesh(a) => Some(&a.vertex),
            Attachment::BoundingBox(a) => Some(&a.vertex),
            Attachment::Path(a) => Some(&a.vertex),
            Attachment::Clipping(a) => Some(&a.vertex),
            Attachment::Region(_) | Attachment::Point(_) => None,
        }
    }

    pub fn as_region(&self) -> Option<&RegionAttachment> {
        match self {
            Attachment::Region(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_mesh(&self) -> Option<&MeshAttachment> {
        match self {
            Attachment::Mesh(a) => Some(a),
            _ => None,
        }
    }

    /// Whether the bones this attachment is weighted to exist among the first `bone_count` bones.
    /// Attachments without vertices always fit.
    pub fn bones_fit(&self, bone_count: usize) -> bool {
        match self {
            Attachment::Mesh(a) => a.vertex.bones_fit(bone_count),
            Attachment::BoundingBox(a) => a.vertex.bones_fit(bone_count),
            Attachment::Path(a) => a.vertex.bones_fit(bone_count),
            Attachment::Clipping(a) => a.vertex.bones_fit(bone_count),
            Attachment::Region(_) | Attachment::Point(_) => true,
        }
    }

    /// An independent copy with a new id. Timeline info is kept, so the copy shares the
    /// original's timelines. Meshes copy as linked meshes when they are linked.
    pub fn copy(this: &Arc<Attachment>) -> Attachment {
        match &**this {
            Attachment::Region(a) => Attachment::Region(a.copy()),
            Attachment::Mesh(_) => Attachment::Mesh(MeshAttachment::copy(this).expect("is a mesh")),
            Attachment::BoundingBox(a) => Attachment::BoundingBox(a.copy()),
            Attachment::Path(a) => Attachment::Path(a.copy()),
            Attachment::Point(a) => Attachment::Point(a.copy()),
            Attachment::Clipping(a) => Attachment::Clipping(a.copy()),
        }
    }

    /// Whether any active slot currently shows an attachment using this attachment's timelines.
    /// `applied_pose` selects the applied pose rather than the unconstrained pose.
    pub fn is_timeline_active(
        &self,
        skeleton: &Skeleton,
        slot_index: usize,
        applied_pose: bool,
    ) -> bool {
        let timeline_id = self.id();
        let shows = |index: usize| -> bool {
            let slot = &skeleton.slots()[index];
            if !skeleton.bone(slot.bone()).is_active() {
                return false;
            }
            let pose = if applied_pose {
                slot.applied_pose()
            } else {
                &slot.pose
            };
            pose.attachment
                .as_ref()
                .is_some_and(|a| a.timeline_id() == timeline_id)
        };
        shows(slot_index) || self.base().timeline_slots.iter().any(|&i| shows(i))
    }
}
