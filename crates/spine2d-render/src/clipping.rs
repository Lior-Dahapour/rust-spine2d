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

//! Clips triangles against a clipping attachment's polygon (Sutherland–Hodgman).

use crate::triangulator::Triangulator;
use spine2d_core::attachment::ClippingAttachment;
use spine2d_core::{Skeleton, SlotId};

/// Input/output scratch buffers for [`clip`]. Index 0 is the clip output.
#[derive(Clone, Debug, Default)]
struct ClipBuffers {
    bufs: [Vec<f32>; 2],
}

/// Clips triangles to the area of a clipping attachment.
///
/// Call [`clip_start`](Self::clip_start) when a clipping attachment is reached while drawing, then
/// [`clip_triangles`](Self::clip_triangles) for each attachment after it, and
/// [`clip_end_slot`](Self::clip_end_slot) once the end slot has been drawn.
#[derive(Clone, Debug, Default)]
pub struct SkeletonClipping {
    triangulator: Triangulator,
    clipping_polygon: Vec<f32>,
    // True when the clip shape was split into several convex polygons held by the triangulator;
    // otherwise `clipping_polygon` itself is the only convex polygon.
    decomposed: bool,
    buffers: ClipBuffers,
    clipped_vertices: Vec<f32>,
    clipped_triangles: Vec<u16>,
    clipped_uvs: Vec<f32>,
    inverse_vertices: Vec<f32>,
    /// Index of the slot after which clipping ends (`None` clips to the end), while clipping.
    active_end: Option<Option<usize>>,
    inverse: bool,
}

impl SkeletonClipping {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins clipping with `clip`, drawn by `slot`. Returns the number of clipping polygons
    /// (0 if clipping did not start, eg because it is already active).
    pub fn clip_start(
        &mut self,
        skeleton: &Skeleton,
        slot: SlotId,
        clip: &ClippingAttachment,
    ) -> usize {
        if self.active_end.is_some() {
            return 0;
        }
        let n = clip.vertex.world_vertices_length;
        if n < 6 {
            return 0;
        }
        self.active_end = Some(clip.end_slot);
        self.inverse = clip.inverse;

        self.clipping_polygon.clear();
        self.clipping_polygon.resize(n, 0.0);
        clip.vertex
            .compute_world_vertices(skeleton, slot, 0, n, &mut self.clipping_polygon, 0, 2);
        let convex = make_clockwise(&mut self.clipping_polygon);
        if convex || self.inverse || clip.convex {
            if !convex {
                make_convex(&mut self.clipping_polygon, &mut self.buffers.bufs[0]);
            }
            let (x, y) = (self.clipping_polygon[0], self.clipping_polygon[1]);
            self.clipping_polygon.push(x);
            self.clipping_polygon.push(y);
            self.decomposed = false;
            1
        } else {
            self.triangulator.triangulate(&self.clipping_polygon);
            self.decomposed = true;
            self.triangulator.decompose(&self.clipping_polygon).len()
        }
    }

    /// Ends clipping if `slot` is the end slot of the active clipping attachment.
    pub fn clip_end_slot(&mut self, slot: SlotId) {
        if let Some(Some(end)) = self.active_end {
            if end == slot.0 {
                self.clip_end();
            }
        }
    }

    /// Ends clipping.
    pub fn clip_end(&mut self) {
        if self.active_end.is_none() {
            return;
        }
        self.active_end = None;
    }

    pub fn is_clipping(&self) -> bool {
        self.active_end.is_some()
    }

    pub fn clipped_vertices(&self) -> &[f32] {
        &self.clipped_vertices
    }

    pub fn clipped_triangles(&self) -> &[u16] {
        &self.clipped_triangles
    }

    pub fn clipped_uvs(&self) -> &[f32] {
        &self.clipped_uvs
    }

    /// Clips triangles whose vertices are `x, y` pairs. Results are in
    /// [`clipped_vertices`](Self::clipped_vertices) and [`clipped_triangles`](Self::clipped_triangles).
    /// Returns true if any triangle was clipped.
    pub fn clip_triangles(&mut self, vertices: &[f32], triangles: &[u16]) -> bool {
        self.clipped_vertices.clear();
        self.clipped_triangles.clear();
        let mut index: u16 = 0;

        if self.inverse {
            let polygon = std::mem::take(&mut self.clipping_polygon);
            for tri in triangles.chunks_exact(3) {
                let p = |i: u16| {
                    (
                        vertices[(i as usize) << 1],
                        vertices[((i as usize) << 1) + 1],
                    )
                };
                let ((x1, y1), (x2, y2), (x3, y3)) = (p(tri[0]), p(tri[1]), p(tri[2]));
                self.clip_inverse(x1, y1, x2, y2, x3, y3, &polygon);
                let mut offset = 0;
                while offset < self.inverse_vertices.len() {
                    let polygon_size = self.inverse_vertices[offset] as usize;
                    offset += 1;
                    let vertex_count = (polygon_size >> 1) as u16;
                    self.clipped_vertices
                        .extend_from_slice(&self.inverse_vertices[offset..offset + polygon_size]);
                    for ii in 1..vertex_count - 1 {
                        self.clipped_triangles.extend_from_slice(&[
                            index,
                            index + ii,
                            index + ii + 1,
                        ]);
                    }
                    index += vertex_count;
                    offset += polygon_size;
                }
            }
            self.clipping_polygon = polygon;
            return true;
        }

        let polygons: &[Vec<f32>] = if self.decomposed {
            self.triangulator.polygons()
        } else {
            std::slice::from_ref(&self.clipping_polygon)
        };
        let mut clipped = false;
        for tri in triangles.chunks_exact(3) {
            let p = |i: u16| {
                (
                    vertices[(i as usize) << 1],
                    vertices[((i as usize) << 1) + 1],
                )
            };
            let ((x1, y1), (x2, y2), (x3, y3)) = (p(tri[0]), p(tri[1]), p(tri[2]));
            for polygon in polygons {
                if clip(&mut self.buffers, x1, y1, x2, y2, x3, y3, polygon) {
                    let output = &self.buffers.bufs[0];
                    let output_len = output.len();
                    if output_len == 0 {
                        continue;
                    }
                    clipped = true;
                    let output_count = (output_len >> 1) as u16;
                    self.clipped_vertices.extend_from_slice(output);
                    for ii in 1..output_count - 1 {
                        self.clipped_triangles.extend_from_slice(&[
                            index,
                            index + ii,
                            index + ii + 1,
                        ]);
                    }
                    index += output_count;
                } else {
                    self.clipped_vertices
                        .extend_from_slice(&[x1, y1, x2, y2, x3, y3]);
                    self.clipped_triangles
                        .extend_from_slice(&[index, index + 1, index + 2]);
                    index += 3;
                    break;
                }
            }
        }
        clipped
    }

    /// Like [`clip_triangles`](Self::clip_triangles) but also interpolates UVs. `vertices` has
    /// `stride` floats per vertex (x, y first); `uvs` has 2 floats per vertex. Results include
    /// [`clipped_uvs`](Self::clipped_uvs).
    pub fn clip_triangles_with_uvs(
        &mut self,
        vertices: &[f32],
        triangles: &[u16],
        uvs: &[f32],
        stride: usize,
    ) -> bool {
        self.clipped_vertices.clear();
        self.clipped_triangles.clear();
        self.clipped_uvs.clear();
        let mut index: u16 = 0;

        if self.inverse {
            let polygon = std::mem::take(&mut self.clipping_polygon);
            for tri in triangles.chunks_exact(3) {
                let (t0, t1, t2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                let (x1, y1) = (vertices[t0 * stride], vertices[t0 * stride + 1]);
                let (x2, y2) = (vertices[t1 * stride], vertices[t1 * stride + 1]);
                let (x3, y3) = (vertices[t2 * stride], vertices[t2 * stride + 1]);
                self.clip_inverse(x1, y1, x2, y2, x3, y3, &polygon);
                let nn = self.inverse_vertices.len();
                if nn == 0 {
                    continue;
                }
                let (u1, v1) = (uvs[t0 << 1], uvs[(t0 << 1) + 1]);
                let (u2, v2) = (uvs[t1 << 1], uvs[(t1 << 1) + 1]);
                let (u3, v3) = (uvs[t2 << 1], uvs[(t2 << 1) + 1]);
                let d0 = y2 - y3;
                let d1 = x3 - x2;
                let d2 = x1 - x3;
                let d4 = y3 - y1;
                let d = 1.0 / (d0 * d2 + d1 * (y1 - y3));
                let mut offset = 0;
                while offset < nn {
                    let polygon_size = self.inverse_vertices[offset] as usize;
                    offset += 1;
                    let vertex_count = (polygon_size >> 1) as u16;
                    for ii in (0..polygon_size).step_by(2) {
                        let x = self.inverse_vertices[offset + ii];
                        let y = self.inverse_vertices[offset + ii + 1];
                        self.clipped_vertices.push(x);
                        self.clipped_vertices.push(y);
                        let c0 = x - x3;
                        let c1 = y - y3;
                        let a = (d0 * c0 + d1 * c1) * d;
                        let b = (d4 * c0 + d2 * c1) * d;
                        let c = 1.0 - a - b;
                        self.clipped_uvs.push(u1 * a + u2 * b + u3 * c);
                        self.clipped_uvs.push(v1 * a + v2 * b + v3 * c);
                    }
                    for ii in 1..vertex_count - 1 {
                        self.clipped_triangles.extend_from_slice(&[
                            index,
                            index + ii,
                            index + ii + 1,
                        ]);
                    }
                    index += vertex_count;
                    offset += polygon_size;
                }
            }
            self.clipping_polygon = polygon;
            return true;
        }

        let polygons: &[Vec<f32>] = if self.decomposed {
            self.triangulator.polygons()
        } else {
            std::slice::from_ref(&self.clipping_polygon)
        };
        let mut clipped = false;
        for tri in triangles.chunks_exact(3) {
            let t = tri[0] as usize;
            let (x1, y1) = (vertices[t * stride], vertices[t * stride + 1]);
            let (u1, v1) = (uvs[t << 1], uvs[(t << 1) + 1]);
            let t = tri[1] as usize;
            let (x2, y2) = (vertices[t * stride], vertices[t * stride + 1]);
            let (u2, v2) = (uvs[t << 1], uvs[(t << 1) + 1]);
            let t = tri[2] as usize;
            let (x3, y3) = (vertices[t * stride], vertices[t * stride + 1]);
            let (u3, v3) = (uvs[t << 1], uvs[(t << 1) + 1]);
            let (mut d0, mut d1, mut d2, mut d4, mut d) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for polygon in polygons {
                if clip(&mut self.buffers, x1, y1, x2, y2, x3, y3, polygon) {
                    let output = &self.buffers.bufs[0];
                    let output_len = output.len();
                    if output_len == 0 {
                        continue;
                    }
                    clipped = true;
                    let mut output_count = (output_len >> 1) as u16;
                    if d == 0.0 {
                        d0 = y2 - y3;
                        d1 = x3 - x2;
                        d2 = x1 - x3;
                        d4 = y3 - y1;
                        d = 1.0 / (d0 * d2 - d1 * d4);
                    }
                    for ii in (0..output_len).step_by(2) {
                        let x = output[ii];
                        let y = output[ii + 1];
                        self.clipped_vertices.push(x);
                        self.clipped_vertices.push(y);
                        let c0 = x - x3;
                        let c1 = y - y3;
                        let a = (d0 * c0 + d1 * c1) * d;
                        let b = (d4 * c0 + d2 * c1) * d;
                        let c = 1.0 - a - b;
                        self.clipped_uvs.push(u1 * a + u2 * b + u3 * c);
                        self.clipped_uvs.push(v1 * a + v2 * b + v3 * c);
                    }
                    output_count -= 1;
                    for ii in 1..output_count {
                        self.clipped_triangles.extend_from_slice(&[
                            index,
                            index + ii,
                            index + ii + 1,
                        ]);
                    }
                    index += output_count + 1;
                } else {
                    self.clipped_vertices
                        .extend_from_slice(&[x1, y1, x2, y2, x3, y3]);
                    self.clipped_uvs
                        .extend_from_slice(&[u1, v1, u2, v2, u3, v3]);
                    self.clipped_triangles
                        .extend_from_slice(&[index, index + 1, index + 2]);
                    index += 3;
                    break;
                }
            }
        }
        clipped
    }

    /// Clips a triangle to the outside of a convex clockwise polygon, writing the fragments to
    /// `inverse_vertices` as `[count, x, y, x, y, ...]` runs.
    #[allow(clippy::too_many_arguments)]
    fn clip_inverse(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        polygon: &[f32],
    ) {
        self.inverse_vertices.clear();
        self.inverse_vertices.reserve(polygon.len() * 3);
        let last = polygon.len() - 4;
        let bufs = &mut self.buffers.bufs;
        let mut a = std::mem::take(&mut bufs[0]);
        let mut b = std::mem::take(&mut bufs[1]);
        // `input` is `a` (the clip output) when the polygon length mod 4 is >= 2, else `b`.
        let mut input_is_a = polygon.len() % 4 >= 2;
        {
            let input = if input_is_a { &mut a } else { &mut b };
            input.clear();
            input.extend_from_slice(&[x1, y1, x2, y2, x3, y3, x1, y1]);
        }
        (if input_is_a { &mut b } else { &mut a }).clear();

        let mut i = 0;
        loop {
            let (input, output) = if input_is_a {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            let edge_x = polygon[i];
            let edge_y = polygon[i + 1];
            let ex = edge_x - polygon[i + 2];
            let ey = edge_y - polygon[i + 3];
            let output_start = output.len();
            let fragment_start = self.inverse_vertices.len();
            self.inverse_vertices.push(0.0);
            let mut x1 = input[0];
            let mut y1 = input[1];
            let mut s1 = ey * (edge_x - x1) - ex * (edge_y - y1);
            let nn = input.len() - 2;
            let mut ii = 2;
            while ii <= nn {
                let x2 = input[ii];
                let y2 = input[ii + 1];
                let s2 = ey * (edge_x - x2) - ex * (edge_y - y2);
                if s1 > 0.0 {
                    if s2 > 0.0 {
                        output.push(x2);
                        output.push(y2);
                    } else {
                        let ix = x2 - x1;
                        let iy = y2 - y1;
                        let t = s1 / (ix * ey - iy * ex);
                        if (0.0..=1.0).contains(&t) {
                            let cx = x1 + ix * t;
                            let cy = y1 + iy * t;
                            output.push(cx);
                            output.push(cy);
                            self.inverse_vertices.extend_from_slice(&[cx, cy, x2, y2]);
                        } else {
                            output.push(x2);
                            output.push(y2);
                        }
                    }
                } else if s2 > 0.0 {
                    let ix = x2 - x1;
                    let iy = y2 - y1;
                    let t = s1 / (ix * ey - iy * ex);
                    if (0.0..=1.0).contains(&t) {
                        let cx = x1 + ix * t;
                        let cy = y1 + iy * t;
                        self.inverse_vertices.push(cx);
                        self.inverse_vertices.push(cy);
                        output.push(cx);
                        output.push(cy);
                        output.push(x2);
                        output.push(y2);
                    } else {
                        output.push(x2);
                        output.push(y2);
                    }
                } else {
                    self.inverse_vertices.push(x2);
                    self.inverse_vertices.push(y2);
                }
                x1 = x2;
                y1 = y2;
                s1 = s2;
                ii += 2;
            }

            let fragment_size = self.inverse_vertices.len() - fragment_start - 1;
            if fragment_size >= 6 {
                self.inverse_vertices[fragment_start] = fragment_size as f32;
            } else {
                self.inverse_vertices.truncate(fragment_start);
            }

            if output_start == output.len() {
                break;
            }
            let (ox, oy) = (output[0], output[1]);
            output.push(ox);
            output.push(oy);
            if i == last {
                break;
            }
            // The output becomes the input of the next edge.
            input_is_a = !input_is_a;
            let (_, new_output) = if input_is_a {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            new_output.clear();
            i += 2;
        }
        self.buffers.bufs[0] = a;
        self.buffers.bufs[1] = b;
    }
}

/// Clips the input triangle against the convex, clockwise clipping area. If the triangle lies
/// entirely within the clipping area, false is returned. The clipping area must duplicate the
/// first vertex at the end of the vertices list. The clipped polygon (if any) is left in
/// `buffers.bufs[0]`.
#[allow(clippy::too_many_arguments)]
fn clip(
    buffers: &mut ClipBuffers,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    polygon: &[f32],
) -> bool {
    let mut clipped = false;
    let mut a = std::mem::take(&mut buffers.bufs[0]);
    let mut b = std::mem::take(&mut buffers.bufs[1]);
    // `a` is the original output buffer. The input is `a` when the polygon length mod 4 >= 2.
    let mut input_is_a = polygon.len() % 4 >= 2;
    {
        let input = if input_is_a { &mut a } else { &mut b };
        input.clear();
        input.extend_from_slice(&[x1, y1, x2, y2, x3, y3, x1, y1]);
    }
    (if input_is_a { &mut b } else { &mut a }).clear();
    let last = polygon.len() - 4;

    let mut i = 0;
    loop {
        let (input, output) = if input_is_a {
            (&mut a, &mut b)
        } else {
            (&mut b, &mut a)
        };
        let edge_x = polygon[i];
        let edge_y = polygon[i + 1];
        let ex = edge_x - polygon[i + 2];
        let ey = edge_y - polygon[i + 3];
        let output_start = output.len();
        let mut x1 = input[0];
        let mut y1 = input[1];
        let mut s1 = ey * (edge_x - x1) - ex * (edge_y - y1);
        // Each remaining input vertex pairs with the previous one; the last equals the first.
        for point in input[2..].chunks_exact(2) {
            let (x2, y2) = (point[0], point[1]);
            let s2 = ey * (edge_x - x2) - ex * (edge_y - y2);
            if s1 > 0.0 && s2 > 0.0 {
                output.extend_from_slice(&[x2, y2]);
            } else if s1 > 0.0 || s2 > 0.0 {
                let ix = x2 - x1;
                let iy = y2 - y1;
                let t = s1 / (ix * ey - iy * ex);
                if (0.0..=1.0).contains(&t) {
                    output.extend_from_slice(&[x1 + ix * t, y1 + iy * t]);
                    clipped = true;
                    if s2 > 0.0 {
                        output.extend_from_slice(&[x2, y2]);
                    }
                } else {
                    output.extend_from_slice(&[x2, y2]);
                }
            } else {
                clipped = true;
            }
            x1 = x2;
            y1 = y2;
            s1 = s2;
        }

        if output_start == output.len() {
            // The triangle is completely outside this edge: nothing remains.
            a.clear();
            buffers.bufs[0] = a;
            buffers.bufs[1] = b;
            return true;
        }
        let (ox, oy) = (output[0], output[1]);
        output.push(ox);
        output.push(oy);
        if i == last {
            break;
        }
        input_is_a = !input_is_a;
        let (_, new_output) = if input_is_a {
            (&mut a, &mut b)
        } else {
            (&mut b, &mut a)
        };
        new_output.clear();
        i += 2;
    }

    // The result is in the buffer that was last written (the one that is not `input`).
    let output_is_a = !input_is_a;
    if !output_is_a {
        // Result is in `b`; copy it into the original output buffer `a` without the repeated
        // closing vertex.
        a.clear();
        let n = b.len() - 2;
        a.extend_from_slice(&b[..n]);
    } else {
        let n = a.len() - 2;
        a.truncate(n);
    }
    buffers.bufs[0] = a;
    buffers.bufs[1] = b;
    clipped
}

/// Reverses the polygon to clockwise winding if needed. Returns true if the polygon is convex.
fn make_clockwise(polygon: &mut [f32]) -> bool {
    let n = polygon.len();
    let v = &mut *polygon;
    let mut no_cw = true;
    let mut no_ccw = true;
    let mut area = 0.0f32;
    let mut prev_x = v[n - 2];
    let mut prev_y = v[n - 1];
    let mut curr_x = v[0];
    let mut curr_y = v[1];
    let mut i = 2;
    while i < n {
        let next_x = v[i];
        let next_y = v[i + 1];
        area += curr_x * next_y - next_x * curr_y;
        let cross = (curr_x - prev_x) * (next_y - curr_y) - (curr_y - prev_y) * (next_x - curr_x);
        no_ccw &= cross <= 0.0;
        no_cw &= cross >= 0.0;
        prev_x = curr_x;
        prev_y = curr_y;
        curr_x = next_x;
        curr_y = next_y;
        i += 2;
    }
    area += curr_x * v[1] - v[0] * curr_y;
    let cross = (curr_x - prev_x) * (v[1] - curr_y) - (curr_y - prev_y) * (v[0] - curr_x);
    no_ccw &= cross <= 0.0;
    no_cw &= cross >= 0.0;

    if area >= 0.0 {
        let last_x = n - 2;
        let half = n >> 1;
        let mut i = 0;
        while i < half {
            let (x, y) = (v[i], v[i + 1]);
            let other = last_x - i;
            v[i] = v[other];
            v[i + 1] = v[other + 1];
            v[other] = x;
            v[other + 1] = y;
            i += 2;
        }
        return no_cw;
    }
    no_ccw
}

/// Replaces the polygon by its convex hull (Andrew's monotone chain). `scratch` holds the sorted
/// points.
fn make_convex(polygon: &mut Vec<f32>, scratch: &mut Vec<f32>) {
    let n = polygon.len();
    scratch.clear();
    scratch.extend_from_slice(polygon);
    let sorted = scratch;
    // Insertion sort by x, then y.
    let mut i = 2;
    while i < n {
        let x = sorted[i];
        let y = sorted[i + 1];
        let mut p = i as isize - 2;
        while p >= 0
            && (sorted[p as usize] > x || (sorted[p as usize] == x && sorted[p as usize + 1] > y))
        {
            sorted[p as usize + 2] = sorted[p as usize];
            sorted[p as usize + 3] = sorted[p as usize + 1];
            p -= 2;
        }
        sorted[(p + 2) as usize] = x;
        sorted[(p + 3) as usize] = y;
        i += 2;
    }

    // The reference writes up to a point past the polygon's size (it has spare capacity there).
    polygon.resize(n + 4, 0.0);
    let v = &mut *polygon;
    v[0] = sorted[0];
    v[1] = sorted[1];
    v[2] = sorted[2];
    v[3] = sorted[3];
    let mut s = 4usize;
    let mut i = 4;
    while i < n {
        let x = sorted[i];
        let y = sorted[i + 1];
        while (v[s - 2] - v[s - 4]) * (y - v[s - 3]) - (v[s - 1] - v[s - 3]) * (x - v[s - 4]) >= 0.0
        {
            s -= 2;
            if s == 2 {
                break;
            }
        }
        v[s] = x;
        v[s + 1] = y;
        i += 2;
        s += 2;
    }
    v[s] = sorted[n - 4];
    v[s + 1] = sorted[n - 3];
    let t = s;
    s += 2;
    let mut i = n as isize - 6;
    while i >= 0 {
        let iu = i as usize;
        let x = sorted[iu];
        let y = sorted[iu + 1];
        while (v[s - 2] - v[s - 4]) * (y - v[s - 3]) - (v[s - 1] - v[s - 3]) * (x - v[s - 4]) >= 0.0
        {
            s -= 2;
            if s == t {
                break;
            }
        }
        v[s] = x;
        v[s + 1] = y;
        i -= 2;
        s += 2;
    }
    polygon.truncate(s - 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_clockwise_reverses_ccw_polygons_and_reports_convexity() {
        // Counter-clockwise square (y up) becomes clockwise; convex.
        let mut sq = vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
        assert!(make_clockwise(&mut sq));
        assert_eq!(sq, [0.0, 10.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.0]);
        // A concave arrow reports non-convex.
        let mut arrow = vec![0.0, 0.0, 10.0, 5.0, 0.0, 10.0, 3.0, 5.0];
        assert!(!make_clockwise(&mut arrow));
    }

    #[test]
    fn make_convex_computes_the_hull() {
        let mut poly = vec![0.0, 0.0, 10.0, 5.0, 0.0, 10.0, 3.0, 5.0];
        let mut scratch = Vec::new();
        make_convex(&mut poly, &mut scratch);
        // The reflex vertex (3, 5) is dropped.
        assert_eq!(poly.len(), 6);
        assert!(!poly.chunks(2).any(|p| p == [3.0, 5.0]));
    }

    #[test]
    fn clip_cuts_a_triangle_to_a_square() {
        // Square 0..10 clockwise with the first vertex repeated.
        let square = [0.0, 10.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.0, 0.0, 10.0];
        let mut bufs = ClipBuffers::default();
        // Triangle fully inside: not clipped.
        assert!(!clip(&mut bufs, 2.0, 2.0, 8.0, 2.0, 5.0, 8.0, &square));
        // Triangle poking out the right side: clipped, and all output stays inside.
        assert!(clip(&mut bufs, 5.0, 2.0, 20.0, 2.0, 5.0, 8.0, &square));
        let out = &bufs.bufs[0];
        assert!(out.len() >= 6);
        assert!(out
            .chunks(2)
            .all(|p| p[0] <= 10.0 + 1e-4 && p[0] >= -1e-4 && p[1] >= -1e-4 && p[1] <= 10.0 + 1e-4));
        // Entirely outside: nothing left.
        assert!(clip(&mut bufs, 20.0, 2.0, 30.0, 2.0, 25.0, 8.0, &square));
        assert!(bufs.bufs[0].is_empty());
    }
}
