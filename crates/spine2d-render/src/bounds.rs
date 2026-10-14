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

//! Skeleton bounds: the AABB of what is drawn, and hit-testing against bounding boxes.

use crate::clipping::SkeletonClipping;
use spine2d_core::attachment::{Attachment, RegionAttachment};
use spine2d_core::Skeleton;
use std::sync::Arc;

/// An axis-aligned rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

/// Computes the AABB of the skeleton's visible region and mesh attachments in world space.
/// Pass a clipper to take clipping attachments into account. If nothing is drawn, the result is
/// `x = y = f32::MAX` and a negative size, like the reference.
///
/// `vertex_buffer` is scratch space for world vertices. Pass the same vector on every call to
/// avoid reallocating it; its contents on return are unspecified.
pub fn compute_bounds(
    skeleton: &Skeleton,
    vertex_buffer: &mut Vec<f32>,
    mut clipper: Option<&mut SkeletonClipping>,
) -> Bounds {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, -f32::MAX, -f32::MAX);

    for &slot_id in skeleton.draw_order().applied_pose() {
        let slot = skeleton.slot(slot_id);
        if !skeleton.bone(slot.bone()).is_active() {
            continue;
        }
        let Some(attachment) = slot.applied_pose().attachment.clone() else {
            if let Some(c) = clipper.as_deref_mut() {
                c.clip_end_slot(slot_id);
            }
            continue;
        };
        let triangles: Option<&[u16]> = match &*attachment {
            Attachment::Region(region) => {
                vertex_buffer.clear();
                vertex_buffer.resize(8, 0.0);
                RegionAttachment::compute_world_vertices(
                    skeleton,
                    slot_id,
                    region.offsets(slot.applied_pose()),
                    vertex_buffer,
                    0,
                    2,
                );
                Some(&QUAD_INDICES)
            }
            Attachment::Mesh(mesh) => {
                let length = mesh.vertex.world_vertices_length;
                vertex_buffer.clear();
                vertex_buffer.resize(length, 0.0);
                mesh.vertex.compute_world_vertices(
                    skeleton,
                    slot_id,
                    0,
                    length,
                    vertex_buffer,
                    0,
                    2,
                );
                Some(&mesh.triangles)
            }
            Attachment::Clipping(clip) => {
                if let Some(c) = clipper.as_deref_mut() {
                    c.clip_end_slot(slot_id);
                    c.clip_start(skeleton, slot_id, clip);
                    continue;
                }
                None
            }
            _ => None,
        };

        if let Some(triangles) = triangles {
            let mut vertices: &[f32] = vertex_buffer;
            if let Some(c) = clipper.as_deref_mut() {
                if c.is_clipping() && c.clip_triangles(vertices, triangles) {
                    vertices = c.clipped_vertices();
                }
            }
            for v in vertices.chunks_exact(2) {
                min_x = min_x.min(v[0]);
                min_y = min_y.min(v[1]);
                max_x = max_x.max(v[0]);
                max_y = max_y.max(v[1]);
            }
        }
        if let Some(c) = clipper.as_deref_mut() {
            c.clip_end_slot(slot_id);
        }
    }
    if let Some(c) = clipper {
        c.clip_end();
    }
    Bounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

/// A bounding box's polygon in world space.
#[derive(Clone, Debug, Default)]
pub struct Polygon {
    /// `x, y` pairs.
    pub vertices: Vec<f32>,
    /// Number of floats in use in `vertices`.
    pub count: usize,
}

/// Collects the skeleton's bounding box attachments in world space for hit-testing.
///
/// Call [`update`](Self::update) after posing the skeleton, then query.
#[derive(Clone, Debug, Default)]
pub struct SkeletonBounds {
    bounding_boxes: Vec<Arc<Attachment>>,
    polygons: Vec<Polygon>,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl SkeletonBounds {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recomputes the polygons of the bounding boxes shown by active slots. If `update_aabb`, also
    /// the combined AABB.
    pub fn update(&mut self, skeleton: &Skeleton, update_aabb: bool) {
        self.bounding_boxes.clear();
        let mut polygons = std::mem::take(&mut self.polygons);
        let mut used = 0;
        for slot in skeleton.slots() {
            if !skeleton.bone(slot.bone()).is_active() {
                continue;
            }
            let Some(attachment) = slot.applied_pose().attachment.clone() else {
                continue;
            };
            let Attachment::BoundingBox(bb) = &*attachment else {
                continue;
            };
            let count = bb.vertex.world_vertices_length;
            if used == polygons.len() {
                polygons.push(Polygon::default());
            }
            let polygon = &mut polygons[used];
            polygon.count = count;
            if polygon.vertices.len() < count {
                polygon.vertices.resize(count, 0.0);
            }
            bb.vertex.compute_world_vertices(
                skeleton,
                slot.id(),
                0,
                count,
                &mut polygon.vertices,
                0,
                2,
            );
            self.bounding_boxes.push(attachment.clone());
            used += 1;
        }
        polygons.truncate(used);
        self.polygons = polygons;

        if update_aabb {
            self.aabb_compute();
        } else {
            // The reference uses FLT_MIN (the smallest *positive* float) here.
            self.min_x = f32::MIN_POSITIVE;
            self.min_y = f32::MIN_POSITIVE;
            self.max_x = f32::MAX;
            self.max_y = f32::MAX;
        }
    }

    fn aabb_compute(&mut self) {
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        // The reference starts the maxima at FLT_MIN (the smallest positive float), so bounds
        // lying entirely at negative coordinates report a max of ~0.
        let (mut max_x, mut max_y) = (f32::MIN_POSITIVE, f32::MIN_POSITIVE);
        for polygon in &self.polygons {
            for v in polygon.vertices[..polygon.count].chunks_exact(2) {
                min_x = min_x.min(v[0]);
                min_y = min_y.min(v[1]);
                max_x = max_x.max(v[0]);
                max_y = max_y.max(v[1]);
            }
        }
        self.min_x = min_x;
        self.min_y = min_y;
        self.max_x = max_x;
        self.max_y = max_y;
    }

    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    pub fn polygons(&self) -> &[Polygon] {
        &self.polygons
    }

    pub fn bounding_boxes(&self) -> &[Arc<Attachment>] {
        &self.bounding_boxes
    }

    pub fn polygon(&self, attachment: &Arc<Attachment>) -> Option<&Polygon> {
        let i = self
            .bounding_boxes
            .iter()
            .position(|a| Arc::ptr_eq(a, attachment))?;
        self.polygons.get(i)
    }

    /// Whether the point is inside the combined AABB. Requires `update(.., true)`.
    pub fn aabb_contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Whether the segment intersects the combined AABB. Requires `update(.., true)`.
    pub fn aabb_intersects_segment(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let (min_x, min_y, max_x, max_y) = (self.min_x, self.min_y, self.max_x, self.max_y);
        if (x1 <= min_x && x2 <= min_x)
            || (y1 <= min_y && y2 <= min_y)
            || (x1 >= max_x && x2 >= max_x)
            || (y1 >= max_y && y2 >= max_y)
        {
            return false;
        }
        let m = (y2 - y1) / (x2 - x1);
        let mut y = m * (min_x - x1) + y1;
        if y > min_y && y < max_y {
            return true;
        }
        y = m * (max_x - x1) + y1;
        if y > min_y && y < max_y {
            return true;
        }
        let mut x = (min_y - y1) / m + x1;
        if x > min_x && x < max_x {
            return true;
        }
        x = (max_y - y1) / m + x1;
        x > min_x && x < max_x
    }

    /// Whether this AABB overlaps another's.
    pub fn aabb_intersects_skeleton(&self, other: &SkeletonBounds) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
    }

    /// Whether the point is inside the polygon (even-odd rule).
    pub fn polygon_contains_point(polygon: &Polygon, x: f32, y: f32) -> bool {
        let vertices = &polygon.vertices;
        let nn = polygon.count;
        let mut prev_index = nn - 2;
        let mut inside = false;
        let mut ii = 0;
        while ii < nn {
            let vertex_y = vertices[ii + 1];
            let prev_y = vertices[prev_index + 1];
            if (vertex_y < y && prev_y >= y) || (prev_y < y && vertex_y >= y) {
                let vertex_x = vertices[ii];
                if vertex_x
                    + (y - vertex_y) / (prev_y - vertex_y) * (vertices[prev_index] - vertex_x)
                    < x
                {
                    inside = !inside;
                }
            }
            prev_index = ii;
            ii += 2;
        }
        inside
    }

    /// The first bounding box containing the point.
    pub fn contains_point(&self, x: f32, y: f32) -> Option<&Arc<Attachment>> {
        self.polygons
            .iter()
            .position(|p| Self::polygon_contains_point(p, x, y))
            .map(|i| &self.bounding_boxes[i])
    }

    /// The first bounding box intersecting the segment.
    pub fn intersects_segment(
        &self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) -> Option<&Arc<Attachment>> {
        self.polygons
            .iter()
            .position(|p| Self::polygon_intersects_segment(p, x1, y1, x2, y2))
            .map(|i| &self.bounding_boxes[i])
    }

    /// Whether the segment intersects any edge of the polygon.
    pub fn polygon_intersects_segment(
        polygon: &Polygon,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) -> bool {
        let vertices = &polygon.vertices;
        let nn = polygon.count;
        let width12 = x1 - x2;
        let height12 = y1 - y2;
        let det1 = x1 * y2 - y1 * x2;
        let mut x3 = vertices[nn - 2];
        let mut y3 = vertices[nn - 1];
        let mut ii = 0;
        while ii < nn {
            let x4 = vertices[ii];
            let y4 = vertices[ii + 1];
            let det2 = x3 * y4 - y3 * x4;
            let width34 = x3 - x4;
            let height34 = y3 - y4;
            let det3 = width12 * height34 - height12 * width34;
            let x = (det1 * width34 - width12 * det2) / det3;
            if ((x >= x3 && x <= x4) || (x >= x4 && x <= x3))
                && ((x >= x1 && x <= x2) || (x >= x2 && x <= x1))
            {
                let y = (det1 * height34 - height12 * det2) / det3;
                if ((y >= y3 && y <= y4) || (y >= y4 && y <= y3))
                    && ((y >= y1 && y <= y2) || (y >= y2 && y <= y1))
                {
                    return true;
                }
            }
            x3 = x4;
            y3 = y4;
            ii += 2;
        }
        false
    }
}
