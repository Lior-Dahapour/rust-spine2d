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

use super::{Attachment, VertexAttachment};
use crate::color::Color;
use crate::sequence::Sequence;
use crate::texture_region::TextureRegion;
use smol_str::SmolStr;
use std::sync::Arc;
use thiserror::Error;

/// Why mesh data was refused by [`MeshAttachment::from_arrays`].
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MeshError {
    #[error("a mesh needs at least 3 vertices, got {0}")]
    TooFewVertices(usize),
    #[error("a mesh can have at most 65535 vertices, got {0}")]
    TooManyVertices(usize),
    #[error("the UV array has {0} values; it needs 2 per vertex")]
    OddUvCount(usize),
    #[error("the triangle array has {0} values; it needs 3 per triangle")]
    PartialTriangle(usize),
    #[error("triangle value {value} at position {at} is not one of the {vertex_count} vertices")]
    TriangleOutOfRange {
        at: usize,
        value: u16,
        vertex_count: usize,
    },
    #[error("the hull has {hull} vertices but the mesh has {vertex_count}")]
    HullTooLarge { hull: usize, vertex_count: usize },
    #[error("the vertex array has {len} values; unweighted needs {expected}, or use the weighted layout")]
    VertexCountMismatch { len: usize, expected: usize },
    #[error("weighted vertex {vertex} is malformed: {reason}")]
    BadWeightedVertex { vertex: usize, reason: &'static str },
    #[error("the weighted vertex data describes {found} vertices but the UVs have {expected}")]
    WeightedVertexCountMismatch { found: usize, expected: usize },
    #[error("the {array} array contains a value that is not a finite number")]
    NotFinite { array: &'static str },
}

/// A textured, optionally weighted triangle mesh.
#[derive(Clone, Debug)]
pub struct MeshAttachment {
    pub vertex: VertexAttachment,
    pub sequence: Sequence,
    /// UVs in the 0..1 space of the original (unpacked) image.
    pub region_uvs: Vec<f32>,
    pub triangles: Vec<u16>,
    /// Number of floats in the hull (2 per vertex).
    pub hull_length: usize,
    pub path: SmolStr,
    pub color: Color,
    source_mesh: Option<Arc<Attachment>>,
    pub edges: Vec<u16>,
    pub width: f32,
    pub height: f32,
}

impl MeshAttachment {
    pub fn new(name: impl Into<SmolStr>, sequence: Sequence) -> Self {
        MeshAttachment {
            vertex: VertexAttachment::new(name),
            sequence,
            region_uvs: Vec::new(),
            triangles: Vec::new(),
            hull_length: 0,
            path: SmolStr::default(),
            color: Color::WHITE,
            source_mesh: None,
            edges: Vec::new(),
            width: 0.0,
            height: 0.0,
        }
    }

    /// Builds a mesh from raw arrays, checking them so the renderer can trust them.
    ///
    /// * `region_uvs`: 2 values per vertex, 0 to 1 across the image of `region`.
    /// * `triangles`: 3 vertex numbers per triangle.
    /// * `vertices`: unweighted, 2 values per vertex (`x, y` in the slot bone's space). Weighted,
    ///   Spine's layout: per vertex the number of bones, then for each bone its number, `x`, `y`
    ///   and weight. It is weighted when the length differs from 2 values per vertex.
    /// * `hull_vertices`: how many of the first vertices form the outline.
    ///
    /// Bone numbers are only checked to be whole and non-negative; whether they exist is for
    /// [`crate::Skin::fits`] to say, because the mesh does not know its skeleton.
    pub fn from_arrays(
        name: impl Into<SmolStr>,
        region: Arc<TextureRegion>,
        region_uvs: &[f32],
        triangles: &[u16],
        vertices: &[f32],
        hull_vertices: usize,
    ) -> Result<MeshAttachment, MeshError> {
        if !region_uvs.len().is_multiple_of(2) {
            return Err(MeshError::OddUvCount(region_uvs.len()));
        }
        let vertex_count = region_uvs.len() / 2;
        if vertex_count < 3 {
            return Err(MeshError::TooFewVertices(vertex_count));
        }
        if vertex_count > usize::from(u16::MAX) {
            return Err(MeshError::TooManyVertices(vertex_count));
        }
        if !triangles.len().is_multiple_of(3) {
            return Err(MeshError::PartialTriangle(triangles.len()));
        }
        if let Some((at, &value)) = triangles
            .iter()
            .enumerate()
            .find(|&(_, &value)| usize::from(value) >= vertex_count)
        {
            return Err(MeshError::TriangleOutOfRange {
                at,
                value,
                vertex_count,
            });
        }
        if hull_vertices > vertex_count {
            return Err(MeshError::HullTooLarge {
                hull: hull_vertices,
                vertex_count,
            });
        }
        if region_uvs.iter().any(|v| !v.is_finite()) {
            return Err(MeshError::NotFinite { array: "uvs" });
        }
        if vertices.iter().any(|v| !v.is_finite()) {
            return Err(MeshError::NotFinite { array: "vertices" });
        }

        let mut mesh = MeshAttachment::new(name, Sequence::new(1, false));
        mesh.sequence.regions[0] = Some(region);
        if vertices.len() < region_uvs.len() {
            // Even a weighted vertex takes more values than an unweighted one.
            return Err(MeshError::VertexCountMismatch {
                len: vertices.len(),
                expected: region_uvs.len(),
            });
        }
        if vertices.len() == region_uvs.len() {
            mesh.vertex.vertices = vertices.to_vec();
        } else {
            let (bones, weights) = weighted_layout(vertices, vertex_count)?;
            mesh.vertex.bones = bones;
            mesh.vertex.vertices = weights;
        }
        mesh.vertex.world_vertices_length = vertex_count * 2;
        mesh.region_uvs = region_uvs.to_vec();
        mesh.triangles = triangles.to_vec();
        mesh.hull_length = hull_vertices * 2;
        mesh.update_sequence();
        Ok(mesh)
    }

    /// The mesh this linked mesh takes its geometry from, if any.
    pub fn source_mesh(&self) -> Option<&Arc<Attachment>> {
        self.source_mesh.as_ref()
    }

    /// Makes this a linked mesh: copies the geometry of `source`, which must be a mesh.
    /// Does nothing for other attachment kinds.
    pub fn set_source_mesh(&mut self, source: Option<Arc<Attachment>>) {
        if let Some(src) = source.as_deref().and_then(Attachment::as_mesh) {
            self.vertex.bones = src.vertex.bones.clone();
            self.vertex.vertices = src.vertex.vertices.clone();
            self.region_uvs = src.region_uvs.clone();
            self.triangles = src.triangles.clone();
            self.hull_length = src.hull_length;
            self.vertex.world_vertices_length = src.vertex.world_vertices_length;
            self.edges = src.edges.clone();
            self.width = src.width;
            self.height = src.height;
            self.source_mesh = source;
        } else if source.is_none() {
            self.source_mesh = None;
        }
    }

    /// Recomputes the sequence's UVs. Call after changing region UVs or the sequence's regions.
    pub fn update_sequence(&mut self) {
        let region_uvs = &self.region_uvs;
        self.sequence.update_mesh(region_uvs.len(), |region, uvs| {
            Self::compute_uvs(region, region_uvs, uvs)
        });
    }

    /// Computes atlas-space `uvs` from `region_uvs` for a region, handling packed rotation.
    pub fn compute_uvs(region: Option<&TextureRegion>, region_uvs: &[f32], uvs: &mut [f32]) {
        let n = uvs.len();
        let (u, v, width, height);
        match region {
            Some(region) if region.atlas.is_some() => {
                let r = region.atlas.as_ref().unwrap();
                let (tw, th) = (r.page_width as f32, r.page_height as f32);
                let (ow, oh) = (r.original_width as f32, r.original_height as f32);
                let (mut ru, mut rv) = (region.u, region.v);
                match r.degrees {
                    90 => {
                        ru -= (oh - r.offset_y - r.packed_width as f32) / tw;
                        rv -= (ow - r.offset_x - r.packed_height as f32) / th;
                        let (w, h) = (oh / tw, ow / th);
                        for i in (0..n).step_by(2) {
                            uvs[i] = ru + region_uvs[i + 1] * w;
                            uvs[i + 1] = rv + (1.0 - region_uvs[i]) * h;
                        }
                        return;
                    }
                    180 => {
                        ru -= (ow - r.offset_x - r.packed_width as f32) / tw;
                        rv -= r.offset_y / th;
                        let (w, h) = (ow / tw, oh / th);
                        for i in (0..n).step_by(2) {
                            uvs[i] = ru + (1.0 - region_uvs[i]) * w;
                            uvs[i + 1] = rv + (1.0 - region_uvs[i + 1]) * h;
                        }
                        return;
                    }
                    270 => {
                        ru -= r.offset_y / tw;
                        rv -= r.offset_x / th;
                        let (w, h) = (oh / tw, ow / th);
                        for i in (0..n).step_by(2) {
                            uvs[i] = ru + (1.0 - region_uvs[i + 1]) * w;
                            uvs[i + 1] = rv + region_uvs[i] * h;
                        }
                        return;
                    }
                    _ => {
                        ru -= r.offset_x / tw;
                        rv -= (oh - r.offset_y - r.packed_height as f32) / th;
                        u = ru;
                        v = rv;
                        width = ow / tw;
                        height = oh / th;
                    }
                }
            }
            None => {
                u = 0.0;
                v = 0.0;
                width = 1.0;
                height = 1.0;
            }
            Some(region) => {
                u = region.u;
                v = region.v;
                width = region.u2 - u;
                height = region.v2 - v;
            }
        }
        for i in (0..n).step_by(2) {
            uvs[i] = u + region_uvs[i] * width;
            uvs[i + 1] = v + region_uvs[i + 1] * height;
        }
    }

    /// A full, independent copy. For a linked mesh this is a new linked mesh of the same source.
    pub(crate) fn copy(this: &Arc<Attachment>) -> Option<MeshAttachment> {
        let mesh = this.as_mesh()?;
        if mesh.source_mesh.is_some() {
            return Self::new_linked_mesh(this);
        }
        let mut copy = MeshAttachment::new(mesh.vertex.base.name.clone(), mesh.sequence.clone());
        copy.path = mesh.path.clone();
        copy.color = mesh.color;
        mesh.vertex.copy_to(&mut copy.vertex);
        copy.region_uvs = mesh.region_uvs.clone();
        copy.triangles = mesh.triangles.clone();
        copy.hull_length = mesh.hull_length;
        copy.edges = mesh.edges.clone();
        copy.width = mesh.width;
        copy.height = mesh.height;
        Some(copy)
    }

    /// A new mesh that shares the geometry and timelines of `this` (or of its source, if `this`
    /// is itself linked). Returns `None` if `this` is not a mesh.
    pub fn new_linked_mesh(this: &Arc<Attachment>) -> Option<MeshAttachment> {
        let mesh = this.as_mesh()?;
        let mut copy = MeshAttachment::new(mesh.vertex.base.name.clone(), mesh.sequence.clone());
        copy.vertex.base.timeline_attachment = Some(mesh.vertex.base.timeline_id());
        copy.path = mesh.path.clone();
        copy.color = mesh.color;
        copy.set_source_mesh(Some(
            mesh.source_mesh.clone().unwrap_or_else(|| this.clone()),
        ));
        copy.update_sequence();
        Some(copy)
    }
}

/// Splits Spine's weighted vertex layout (per vertex: bone count, then bone number, x, y, weight
/// for each bone) into the runtime's bone list and `x, y, weight` list.
fn weighted_layout(
    data: &[f32],
    expected_vertices: usize,
) -> Result<(Vec<usize>, Vec<f32>), MeshError> {
    let mut bones = Vec::with_capacity(expected_vertices * 3);
    let mut weights = Vec::with_capacity(data.len());
    let (mut at, mut vertex) = (0, 0);
    while at < data.len() {
        let bad = |reason| MeshError::BadWeightedVertex { vertex, reason };
        let count =
            whole_number(data[at]).ok_or_else(|| bad("the bone count is not a whole number"))?;
        if count == 0 {
            return Err(bad("it has no bones"));
        }
        at += 1;
        if data.len() < at + count * 4 {
            return Err(bad("the data ends before all its bones"));
        }
        bones.push(count);
        for _ in 0..count {
            let bone = whole_number(data[at])
                .ok_or_else(|| bad("a bone number is not a whole non-negative number"))?;
            bones.push(bone);
            weights.extend_from_slice(&data[at + 1..at + 4]);
            at += 4;
        }
        vertex += 1;
    }
    if vertex != expected_vertices {
        return Err(MeshError::WeightedVertexCountMismatch {
            found: vertex,
            expected: expected_vertices,
        });
    }
    Ok((bones, weights))
}

/// `value` as a count or index, if it is a whole number from 0 up.
fn whole_number(value: f32) -> Option<usize> {
    (value >= 0.0 && value.fract() == 0.0 && value <= 16_777_216.0).then_some(value as usize)
}
