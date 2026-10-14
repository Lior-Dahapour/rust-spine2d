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

use super::{AttachmentBase, VertexAttachment};
use crate::bone::BonePose;
use crate::color::Color;
use crate::math::{atan2_deg, cos, sin, DEG_RAD};
use smol_str::SmolStr;

/// A path for path constraints. Vertices are in the slot bone's space.
#[derive(Clone, Debug)]
pub struct PathAttachment {
    pub vertex: VertexAttachment,
    /// Curve lengths, excluding the closing curve for closed paths.
    pub lengths: Vec<f32>,
    pub closed: bool,
    pub constant_speed: bool,
    pub color: Color,
}

impl PathAttachment {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        PathAttachment {
            vertex: VertexAttachment::new(name),
            lengths: Vec::new(),
            closed: false,
            constant_speed: false,
            color: Color::default(),
        }
    }

    pub(crate) fn copy(&self) -> PathAttachment {
        let mut copy = PathAttachment::new(self.vertex.base.name.clone());
        self.vertex.copy_to(&mut copy.vertex);
        copy.lengths = self.lengths.clone();
        copy.closed = self.closed;
        copy.constant_speed = self.constant_speed;
        copy
    }
}

/// A polygon used for hit detection.
#[derive(Clone, Debug)]
pub struct BoundingBoxAttachment {
    pub vertex: VertexAttachment,
    pub color: Color,
}

impl BoundingBoxAttachment {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        BoundingBoxAttachment {
            vertex: VertexAttachment::new(name),
            color: Color::new(0.38, 0.94, 0.0, 1.0),
        }
    }

    pub(crate) fn copy(&self) -> BoundingBoxAttachment {
        let mut copy = BoundingBoxAttachment::new(self.vertex.base.name.clone());
        self.vertex.copy_to(&mut copy.vertex);
        copy
    }
}

/// A polygon that clips the slots drawn between its slot and `end_slot`.
#[derive(Clone, Debug)]
pub struct ClippingAttachment {
    pub vertex: VertexAttachment,
    /// Index of the last slot clipped, or `None` to clip to the end of the draw order.
    pub end_slot: Option<usize>,
    pub convex: bool,
    pub inverse: bool,
    pub color: Color,
}

impl ClippingAttachment {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        ClippingAttachment {
            vertex: VertexAttachment::new(name),
            end_slot: None,
            convex: false,
            inverse: false,
            color: Color::default(),
        }
    }

    pub(crate) fn copy(&self) -> ClippingAttachment {
        let mut copy = ClippingAttachment::new(self.vertex.base.name.clone());
        self.vertex.copy_to(&mut copy.vertex);
        copy.end_slot = self.end_slot;
        copy.convex = self.convex;
        copy.inverse = self.inverse;
        copy
    }
}

/// A point with a rotation, positioned relative to the slot's bone.
#[derive(Clone, Debug)]
pub struct PointAttachment {
    pub base: AttachmentBase,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub color: Color,
}

impl PointAttachment {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        PointAttachment {
            base: AttachmentBase::new(name),
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            color: Color::new(0.9451, 0.9451, 0.0, 1.0),
        }
    }

    /// The world position of the point for the given bone pose.
    pub fn compute_world_position(&self, bone: &BonePose) -> (f32, f32) {
        (
            self.x * bone.a + self.y * bone.b + bone.world_x,
            self.x * bone.c + self.y * bone.d + bone.world_y,
        )
    }

    /// The world rotation in degrees of the point for the given bone pose.
    pub fn compute_world_rotation(&self, bone: &BonePose) -> f32 {
        let r = self.rotation * DEG_RAD;
        let (cosine, sine) = (cos(r), sin(r));
        let x = cosine * bone.a + sine * bone.b;
        let y = cosine * bone.c + sine * bone.d;
        atan2_deg(y, x)
    }

    pub(crate) fn copy(&self) -> PointAttachment {
        let mut copy = PointAttachment::new(self.base.name.clone());
        copy.base.timeline_attachment = Some(self.base.timeline_id());
        copy.base.timeline_slots = self.base.timeline_slots.clone();
        copy.x = self.x;
        copy.y = self.y;
        copy.rotation = self.rotation;
        copy.color = self.color;
        copy
    }
}
