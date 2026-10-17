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

use crate::attachment::Attachment;
use crate::bone::BoneId;
use crate::color::Color;
use crate::enums::BlendMode;
use smol_str::SmolStr;
use std::sync::Arc;

/// Index of a slot in `Skeleton::slots()` / `SkeletonData::slots()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub usize);

impl SlotId {
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}

/// A slot's pose: colors, attachment, sequence frame and deform.
#[derive(Clone, Debug)]
pub struct SlotPose {
    pub color: Color,
    pub dark_color: Color,
    pub has_dark_color: bool,
    /// The shown attachment. Use [`SlotPose::set_attachment`] to change it.
    pub attachment: Option<Arc<Attachment>>,
    /// Frame of the attachment's sequence, or -1 to use the sequence's setup index.
    pub sequence_index: i32,
    /// Vertex offsets applied to the attachment's vertices; empty when there is no deform.
    pub deform: Vec<f32>,
}

impl Default for SlotPose {
    fn default() -> Self {
        SlotPose {
            color: Color::WHITE,
            dark_color: Color::new(0.0, 0.0, 0.0, 0.0),
            has_dark_color: false,
            attachment: None,
            sequence_index: 0,
            deform: Vec::new(),
        }
    }
}

impl SlotPose {
    pub fn set(&mut self, other: &SlotPose) {
        self.color = other.color;
        if other.has_dark_color {
            self.dark_color = other.dark_color;
        }
        self.has_dark_color = other.has_dark_color;
        self.attachment = other.attachment.clone();
        self.sequence_index = other.sequence_index;
        self.deform.clear();
        self.deform.extend_from_slice(&other.deform);
    }

    /// Sets the attachment and resets the sequence index to -1.
    ///
    /// The deform is cleared unless the old and new attachments share the same timeline
    /// attachment (see `AttachmentBase::timeline_attachment`).
    pub fn set_attachment(&mut self, attachment: Option<Arc<Attachment>>) {
        let same = match (&self.attachment, &attachment) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        let keep_deform = matches!((&self.attachment, &attachment), (Some(old), Some(new)) if old.timeline_id() == new.timeline_id());
        if !keep_deform {
            self.deform.clear();
        }
        self.attachment = attachment;
        self.sequence_index = -1;
    }
}

/// Setup data for a slot. Shared by all skeleton instances.
#[derive(Clone, Debug)]
pub struct SlotData {
    pub index: usize,
    pub name: SmolStr,
    pub bone: BoneId,
    /// Name of the setup pose attachment (a placeholder name resolved through skins), or empty.
    pub attachment_name: SmolStr,
    pub blend_mode: BlendMode,
    pub visible: bool,
    pub skin_required: bool,
    pub setup_pose: SlotPose,
}

impl SlotData {
    pub fn new(index: usize, name: impl Into<SmolStr>, bone: BoneId) -> Self {
        SlotData {
            index,
            name: name.into(),
            bone,
            attachment_name: SmolStr::default(),
            blend_mode: BlendMode::Normal,
            visible: true,
            skin_required: false,
            setup_pose: SlotPose::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotApplied {
    Pose,
    Constrained,
}

/// A slot instance: draws the attachment of one bone.
#[derive(Clone, Debug)]
pub struct Slot {
    pub(crate) index: usize,
    pub(crate) bone: BoneId,
    /// The unconstrained pose, set by animations and application code.
    pub pose: SlotPose,
    pub(crate) constrained: SlotPose,
    pub(crate) applied: SlotApplied,
    pub(crate) attachment_state: i32,
}

impl Slot {
    pub(crate) fn new(data: &SlotData) -> Slot {
        let mut slot = Slot {
            index: data.index,
            bone: data.bone,
            pose: SlotPose::default(),
            constrained: SlotPose::default(),
            applied: SlotApplied::Pose,
            attachment_state: 0,
        };
        if data.setup_pose.has_dark_color {
            slot.pose.has_dark_color = true;
            slot.constrained.has_dark_color = true;
        }
        slot.setup_pose(data, None);
        slot
    }

    pub fn id(&self) -> SlotId {
        SlotId(self.index)
    }

    /// The bone this slot is attached to.
    pub fn bone(&self) -> BoneId {
        self.bone
    }

    pub fn applied_pose(&self) -> &SlotPose {
        match self.applied {
            SlotApplied::Pose => &self.pose,
            SlotApplied::Constrained => &self.constrained,
        }
    }

    pub fn applied_pose_mut(&mut self) -> &mut SlotPose {
        match self.applied {
            SlotApplied::Pose => &mut self.pose,
            SlotApplied::Constrained => &mut self.constrained,
        }
    }

    /// Resets the pose to the setup pose. `attachment` is the setup attachment resolved by the
    /// skeleton from `data.attachment_name`.
    pub fn setup_pose(&mut self, data: &SlotData, attachment: Option<Arc<Attachment>>) {
        self.pose.color = data.setup_pose.color;
        if self.pose.has_dark_color {
            self.pose.dark_color = data.setup_pose.dark_color;
        }
        self.pose.sequence_index = data.setup_pose.sequence_index;
        if data.attachment_name.is_empty() {
            self.pose.set_attachment(None);
        } else {
            // Forces the change path in set_attachment so deform is cleared and the sequence
            // index reset, even when the same attachment is shown.
            self.pose.attachment = None;
            self.pose.set_attachment(attachment);
        }
    }

    pub(crate) fn reset_constrained(&mut self) {
        self.constrained.set(&self.pose);
    }

    pub(crate) fn constrained(&mut self) {
        self.applied = SlotApplied::Constrained;
    }
}
