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
use crate::skeleton::Skeleton;
use crate::slot::SlotId;
use smol_str::SmolStr;

/// Vertex data shared by mesh, path, bounding box and clipping attachments.
///
/// Unweighted: `bones` is empty and `vertices` holds `x, y` pairs in the slot bone's space.
/// Weighted: `bones` holds, per vertex, the influence count followed by that many bone indices,
/// and `vertices` holds `x, y, weight` per influence.
#[derive(Clone, Debug)]
pub struct VertexAttachment {
    pub base: AttachmentBase,
    pub bones: Vec<usize>,
    pub vertices: Vec<f32>,
    /// Number of floats in the world vertices output (2 per vertex).
    pub world_vertices_length: usize,
}

impl VertexAttachment {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        VertexAttachment {
            base: AttachmentBase::new(name),
            bones: Vec::new(),
            vertices: Vec::new(),
            world_vertices_length: 0,
        }
    }

    /// Whether every bone this attachment is weighted to is one of the first `bone_count` bones,
    /// and the bone list is well formed (an influence count, then that many bone indices, per
    /// vertex). Unweighted attachments always fit.
    pub fn bones_fit(&self, bone_count: usize) -> bool {
        let mut at = 0;
        while at < self.bones.len() {
            let count = self.bones[at];
            let end = at + 1 + count;
            let Some(influences) = self.bones.get(at + 1..end) else {
                return false;
            };
            if influences.iter().any(|&bone| bone >= bone_count) {
                return false;
            }
            at = end;
        }
        true
    }

    /// Copies the vertex data and timeline info into `other`, leaving its name and id.
    pub(crate) fn copy_to(&self, other: &mut VertexAttachment) {
        other.bones = self.bones.clone();
        other.vertices = self.vertices.clone();
        other.world_vertices_length = self.world_vertices_length;
        other.base.timeline_attachment = Some(self.base.timeline_id());
        other.base.timeline_slots = self.base.timeline_slots.clone();
    }

    /// Transforms this attachment's vertices to world coordinates.
    ///
    /// `start` and `count` are in floats (2 per vertex). Results are written to `world_vertices`
    /// starting at `offset`, advancing `stride` floats per vertex. If the slot has deform, it is
    /// applied.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_world_vertices(
        &self,
        skeleton: &Skeleton,
        slot: SlotId,
        start: usize,
        count: usize,
        world_vertices: &mut [f32],
        offset: usize,
        stride: usize,
    ) {
        let slot = skeleton.slot(slot);
        let count = offset + (count >> 1) * stride;
        let deform = &slot.applied_pose().deform;
        let mut vertices = &self.vertices;
        let bones = &self.bones;

        if bones.is_empty() {
            if !deform.is_empty() {
                vertices = deform;
            }
            let bone = skeleton.bone(slot.bone()).applied_pose();
            let (x, y) = (bone.world_x, bone.world_y);
            let (a, b, c, d) = (bone.a, bone.b, bone.c, bone.d);
            let (mut vv, mut w) = (start, offset);
            while w < count {
                let (vx, vy) = (vertices[vv], vertices[vv + 1]);
                world_vertices[w] = vx * a + vy * b + x;
                world_vertices[w + 1] = vx * c + vy * d + y;
                vv += 2;
                w += stride;
            }
            return;
        }

        let (mut v, mut skip) = (0usize, 0usize);
        let mut i = 0;
        while i < start {
            let n = bones[v];
            v += n + 1;
            skip += n;
            i += 2;
        }

        let mut w = offset;
        let mut b = skip * 3;
        let mut f = skip << 1;
        while w < count {
            let (mut wx, mut wy) = (0.0f32, 0.0f32);
            let n = bones[v];
            v += 1;
            let end = v + n;
            while v < end {
                let bone = skeleton.bone(crate::bone::BoneId(bones[v])).applied_pose();
                let (mut vx, mut vy) = (vertices[b], vertices[b + 1]);
                if !deform.is_empty() {
                    vx += deform[f];
                    vy += deform[f + 1];
                    f += 2;
                }
                let weight = vertices[b + 2];
                wx += (vx * bone.a + vy * bone.b + bone.world_x) * weight;
                wy += (vx * bone.c + vy * bone.d + bone.world_y) * weight;
                v += 1;
                b += 3;
            }
            world_vertices[w] = wx;
            world_vertices[w + 1] = wy;
            w += stride;
        }
    }
}
