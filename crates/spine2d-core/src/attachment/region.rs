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

use super::AttachmentBase;
use crate::color::Color;
use crate::math::{cos_deg, sin_deg};
use crate::sequence::Sequence;
use crate::skeleton::Skeleton;
use crate::slot::{SlotId, SlotPose};
use crate::texture_region::TextureRegion;
use smol_str::SmolStr;
use std::sync::Arc;

// Vertex order inside the 8-float offsets/uvs arrays.
const BLX: usize = 0;
const BLY: usize = 1;
const ULX: usize = 2;
const ULY: usize = 3;
const URX: usize = 4;
const URY: usize = 5;
const BRX: usize = 6;
const BRY: usize = 7;

/// A textured quad positioned relative to the slot's bone.
#[derive(Clone, Debug)]
pub struct RegionAttachment {
    pub base: AttachmentBase,
    pub sequence: Sequence,
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub width: f32,
    pub height: f32,
    pub path: SmolStr,
    pub color: Color,
}

impl RegionAttachment {
    pub fn new(name: impl Into<SmolStr>, sequence: Sequence) -> Self {
        RegionAttachment {
            base: AttachmentBase::new(name),
            sequence,
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            width: 0.0,
            height: 0.0,
            path: SmolStr::default(),
            color: Color::WHITE,
        }
    }

    /// A region attachment that draws one texture region, `width` by `height` skeleton units,
    /// centered on its slot's bone. The UVs and vertex offsets are ready to use.
    pub fn with_region(
        name: impl Into<SmolStr>,
        region: Arc<TextureRegion>,
        width: f32,
        height: f32,
    ) -> Self {
        let mut sequence = Sequence::new(1, false);
        sequence.regions[0] = Some(region);
        let mut attachment = RegionAttachment::new(name, sequence);
        attachment.width = width;
        attachment.height = height;
        attachment.update_sequence();
        attachment
    }

    /// Recomputes the sequence's UVs and vertex offsets. Call after changing x, y, scale,
    /// rotation, width, height or the sequence's regions.
    pub fn update_sequence(&mut self) {
        let (x, y, sx, sy, rot, w, h) = (
            self.x,
            self.y,
            self.scale_x,
            self.scale_y,
            self.rotation,
            self.width,
            self.height,
        );
        self.sequence.update_region(|region, offset, uvs| {
            Self::compute_uvs(region, x, y, sx, sy, rot, w, h, offset, uvs);
        });
    }

    /// The 8 vertex offsets (bottom-left, upper-left, upper-right, bottom-right) for the pose's
    /// sequence frame.
    pub fn offsets(&self, pose: &SlotPose) -> &[f32] {
        self.sequence.offsets(self.sequence.resolve_index(pose))
    }

    /// Writes 4 world vertices (8 floats at `stride`) to `world_vertices`. `vertex_offsets` is the
    /// result of [`offsets`](Self::offsets).
    pub fn compute_world_vertices(
        skeleton: &Skeleton,
        slot: SlotId,
        vertex_offsets: &[f32],
        world_vertices: &mut [f32],
        mut offset: usize,
        stride: usize,
    ) {
        let bone = skeleton.bone(skeleton.slot(slot).bone()).applied_pose();
        let (x, y) = (bone.world_x, bone.world_y);
        let (a, b, c, d) = (bone.a, bone.b, bone.c, bone.d);
        for (ox, oy) in [(BRX, BRY), (BLX, BLY), (ULX, ULY), (URX, URY)] {
            let (offset_x, offset_y) = (vertex_offsets[ox], vertex_offsets[oy]);
            world_vertices[offset] = offset_x * a + offset_y * b + x;
            world_vertices[offset + 1] = offset_x * c + offset_y * d + y;
            offset += stride;
        }
    }

    /// Computes the vertex `offset`s and `uvs` (8 floats each) for a region.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_uvs(
        region: Option<&TextureRegion>,
        x: f32,
        y: f32,
        scale_x: f32,
        scale_y: f32,
        rotation: f32,
        width: f32,
        height: f32,
        offset: &mut [f32],
        uvs: &mut [f32],
    ) {
        let mut local_x2 = width / 2.0;
        let mut local_y2 = height / 2.0;
        let mut local_x = -local_x2;
        let mut local_y = -local_y2;
        let mut rotated = false;
        if let Some(r) = region.and_then(|r| r.atlas.as_ref()) {
            let (ow, oh) = (r.original_width as f32, r.original_height as f32);
            local_x += r.offset_x / ow * width;
            local_y += r.offset_y / oh * height;
            if r.degrees == 90 {
                rotated = true;
                local_x2 -= (ow - r.offset_x - r.packed_height as f32) / ow * width;
                local_y2 -= (oh - r.offset_y - r.packed_width as f32) / oh * height;
            } else {
                local_x2 -= (ow - r.offset_x - r.packed_width as f32) / ow * width;
                local_y2 -= (oh - r.offset_y - r.packed_height as f32) / oh * height;
            }
        }
        local_x *= scale_x;
        local_y *= scale_y;
        local_x2 *= scale_x;
        local_y2 *= scale_y;
        let cos = cos_deg(rotation);
        let sin = sin_deg(rotation);
        let local_x_cos = local_x * cos + x;
        let local_x_sin = local_x * sin;
        let local_y_cos = local_y * cos + y;
        let local_y_sin = local_y * sin;
        let local_x2_cos = local_x2 * cos + x;
        let local_x2_sin = local_x2 * sin;
        let local_y2_cos = local_y2 * cos + y;
        let local_y2_sin = local_y2 * sin;
        offset[BLX] = local_x_cos - local_y_sin;
        offset[BLY] = local_y_cos + local_x_sin;
        offset[ULX] = local_x_cos - local_y2_sin;
        offset[ULY] = local_y2_cos + local_x_sin;
        offset[URX] = local_x2_cos - local_y2_sin;
        offset[URY] = local_y2_cos + local_x2_sin;
        offset[BRX] = local_x2_cos - local_y_sin;
        offset[BRY] = local_y_cos + local_x2_sin;

        match region {
            None => {
                uvs[BLX] = 0.0;
                uvs[BLY] = 0.0;
                uvs[ULX] = 0.0;
                uvs[ULY] = 1.0;
                uvs[URX] = 1.0;
                uvs[URY] = 1.0;
                uvs[BRX] = 1.0;
                uvs[BRY] = 0.0;
            }
            Some(r) => {
                uvs[BLX] = r.u2;
                uvs[ULY] = r.v2;
                uvs[URX] = r.u;
                uvs[BRY] = r.v;
                if rotated {
                    uvs[BLY] = r.v;
                    uvs[ULX] = r.u2;
                    uvs[URY] = r.v2;
                    uvs[BRX] = r.u;
                } else {
                    uvs[BLY] = r.v2;
                    uvs[ULX] = r.u;
                    uvs[URY] = r.v;
                    uvs[BRX] = r.u2;
                }
            }
        }
    }

    pub(crate) fn copy(&self) -> RegionAttachment {
        let mut copy = RegionAttachment::new(self.base.name.clone(), self.sequence.clone());
        copy.base.timeline_attachment = Some(self.base.timeline_id());
        copy.base.timeline_slots = self.base.timeline_slots.clone();
        copy.path = self.path.clone();
        copy.x = self.x;
        copy.y = self.y;
        copy.scale_x = self.scale_x;
        copy.scale_y = self.scale_y;
        copy.rotation = self.rotation;
        copy.width = self.width;
        copy.height = self.height;
        copy.color = self.color;
        copy
    }
}
