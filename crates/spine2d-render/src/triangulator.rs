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

//! Triangulation and convex decomposition of simple polygons, used for clipping.

/// Triangulates polygons into triangles and decomposes triangulations into convex polygons.
#[derive(Clone, Debug, Default)]
pub struct Triangulator {
    indices: Vec<i32>,
    is_concave: Vec<bool>,
    triangles: Vec<i32>,
    convex_polygons: Vec<Vec<f32>>,
    convex_indices: Vec<Vec<i32>>,
    // Vectors from earlier `decompose` calls, kept so their allocations are reused.
    polygon_pool: Vec<Vec<f32>>,
    indices_pool: Vec<Vec<i32>>,
}

impl Triangulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Triangulates the polygon given as `x, y` pairs by ear clipping. Returns triangle vertex
    /// indices (3 per triangle).
    pub fn triangulate(&mut self, vertices: &[f32]) -> &[i32] {
        let mut vertex_count = (vertices.len() >> 1) as i32;
        self.indices.clear();
        self.indices.extend(0..vertex_count);
        self.is_concave.clear();
        for i in 0..vertex_count {
            let c = is_concave(i, vertex_count, vertices, &self.indices);
            self.is_concave.push(c);
        }
        self.triangles.clear();
        self.triangles
            .reserve((vertex_count - 2).max(0) as usize * 3);

        while vertex_count > 3 {
            // Find an ear.
            let mut previous = vertex_count - 1;
            let mut i = 0;
            let mut next = 1;
            loop {
                if !self.is_concave[i as usize] {
                    let p1 = (self.indices[previous as usize] << 1) as usize;
                    let p2 = (self.indices[i as usize] << 1) as usize;
                    let p3 = (self.indices[next as usize] << 1) as usize;
                    let (p1x, p1y) = (vertices[p1], vertices[p1 + 1]);
                    let (p2x, p2y) = (vertices[p2], vertices[p2 + 1]);
                    let (p3x, p3y) = (vertices[p3], vertices[p3 + 1]);
                    let mut ear = true;
                    let mut ii = if next + 1 < vertex_count { next + 1 } else { 0 };
                    while ii != previous {
                        if self.is_concave[ii as usize] {
                            let v = (self.indices[ii as usize] << 1) as usize;
                            let (vx, vy) = (vertices[v], vertices[v + 1]);
                            if positive_area(p3x, p3y, p1x, p1y, vx, vy)
                                && positive_area(p1x, p1y, p2x, p2y, vx, vy)
                                && positive_area(p2x, p2y, p3x, p3y, vx, vy)
                            {
                                ear = false;
                                break;
                            }
                        }
                        ii += 1;
                        if ii == vertex_count {
                            ii = 0;
                        }
                    }
                    if ear {
                        break;
                    }
                }

                if next == 0 {
                    loop {
                        if !self.is_concave[i as usize] {
                            break;
                        }
                        i -= 1;
                        if i <= 0 {
                            break;
                        }
                    }
                    previous = if i > 0 { i - 1 } else { vertex_count - 1 };
                    next = if i + 1 < vertex_count { i + 1 } else { 0 };
                    break;
                }

                previous = i;
                i = next;
                next += 1;
                if next == vertex_count {
                    next = 0;
                }
            }

            self.triangles.push(self.indices[previous as usize]);
            self.triangles.push(self.indices[i as usize]);
            self.triangles.push(self.indices[next as usize]);
            self.indices.remove(i as usize);
            self.is_concave.remove(i as usize);
            vertex_count -= 1;

            let previous_index = if i > 0 { i - 1 } else { vertex_count - 1 };
            let next_index = if i < vertex_count { i } else { 0 };
            self.is_concave[previous_index as usize] =
                is_concave(previous_index, vertex_count, vertices, &self.indices);
            self.is_concave[next_index as usize] =
                is_concave(next_index, vertex_count, vertices, &self.indices);
        }

        if vertex_count == 3 {
            self.triangles.push(self.indices[2]);
            self.triangles.push(self.indices[0]);
            self.triangles.push(self.indices[1]);
        }
        &self.triangles
    }

    /// Merges the triangles of the last [`triangulate`](Self::triangulate) call, which must have
    /// been given the same `vertices`, into convex polygons. Each returned polygon is `x, y`
    /// pairs with the first vertex repeated at the end. The polygons stay valid, and are also
    /// available from [`polygons`](Self::polygons), until the next call to `decompose`.
    pub fn decompose(&mut self, vertices: &[f32]) -> &[Vec<f32>] {
        // Last call's polygons go back to the pools; `append` moves the vectors without freeing.
        self.polygon_pool.append(&mut self.convex_polygons);
        self.indices_pool.append(&mut self.convex_indices);
        let triangles = std::mem::take(&mut self.triangles);
        let mut convex_polygons = std::mem::take(&mut self.convex_polygons);
        let mut convex_indices = std::mem::take(&mut self.convex_indices);
        let mut polygon_indices = self.indices_pool.pop().unwrap_or_default();
        polygon_indices.clear();
        let mut polygon = self.polygon_pool.pop().unwrap_or_default();
        polygon.clear();
        let mut fan_base_index = -1i32;
        let mut last_winding = 0;

        let mut i = 0;
        while i < triangles.len() {
            let t1 = triangles[i] << 1;
            let t2 = triangles[i + 1] << 1;
            let t3 = triangles[i + 2] << 1;
            let (x1, y1) = (vertices[t1 as usize], vertices[t1 as usize + 1]);
            let (x2, y2) = (vertices[t2 as usize], vertices[t2 as usize + 1]);
            let (x3, y3) = (vertices[t3 as usize], vertices[t3 as usize + 1]);
            i += 3;

            // If the base of the last triangle is the same as this triangle, check if they form a convex polygon.
            if fan_base_index == t1 {
                let o = polygon.len() - 4;
                let p = &polygon;
                if winding(p[o], p[o + 1], p[o + 2], p[o + 3], x3, y3) == last_winding
                    && winding(x3, y3, p[0], p[1], p[2], p[3]) == last_winding
                {
                    polygon.push(x3);
                    polygon.push(y3);
                    polygon_indices.push(t3);
                    continue;
                }
            }

            // Otherwise make this triangle the new base.
            if !polygon.is_empty() {
                let next = self.polygon_pool.pop().unwrap_or_default();
                convex_polygons.push(std::mem::replace(&mut polygon, next));
                let next = self.indices_pool.pop().unwrap_or_default();
                convex_indices.push(std::mem::replace(&mut polygon_indices, next));
            }
            polygon.clear();
            polygon.extend_from_slice(&[x1, y1, x2, y2, x3, y3]);
            polygon_indices.clear();
            polygon_indices.extend_from_slice(&[t1, t2, t3]);
            last_winding = winding(x1, y1, x2, y2, x3, y3);
            fan_base_index = t1;
        }
        if polygon.is_empty() {
            self.polygon_pool.push(polygon);
            self.indices_pool.push(polygon_indices);
        } else {
            convex_polygons.push(polygon);
            convex_indices.push(polygon_indices);
        }

        // Go through the list of polygons and try to merge the remaining triangles with the found triangle fans.
        let n = convex_polygons.len();
        for i in 0..n {
            if convex_indices[i].is_empty() {
                continue;
            }
            let first_index = convex_indices[i][0];
            let mut last_index = *convex_indices[i].last().unwrap();
            let o = convex_polygons[i].len() - 4;
            let (mut prev_prev_x, mut prev_prev_y, mut prev_x, mut prev_y) = (
                convex_polygons[i][o],
                convex_polygons[i][o + 1],
                convex_polygons[i][o + 2],
                convex_polygons[i][o + 3],
            );
            let (first_x, first_y, second_x, second_y) = (
                convex_polygons[i][0],
                convex_polygons[i][1],
                convex_polygons[i][2],
                convex_polygons[i][3],
            );
            let polygon_winding =
                winding(prev_prev_x, prev_prev_y, prev_x, prev_y, first_x, first_y);

            let mut ii: i64 = 0;
            while (ii as usize) < n {
                let iu = ii as usize;
                if iu == i {
                    ii += 1;
                    continue;
                }
                if convex_indices[iu].len() != 3 {
                    ii += 1;
                    continue;
                }
                let other_first = convex_indices[iu][0];
                let other_second = convex_indices[iu][1];
                let other_last = convex_indices[iu][2];
                let other_len = convex_polygons[iu].len();
                let (x3, y3) = (
                    convex_polygons[iu][other_len - 2],
                    convex_polygons[iu][other_len - 1],
                );
                if other_first != first_index || other_second != last_index {
                    ii += 1;
                    continue;
                }
                if winding(prev_prev_x, prev_prev_y, prev_x, prev_y, x3, y3) == polygon_winding
                    && winding(x3, y3, first_x, first_y, second_x, second_y) == polygon_winding
                {
                    convex_polygons[iu].clear();
                    convex_indices[iu].clear();
                    convex_polygons[i].push(x3);
                    convex_polygons[i].push(y3);
                    convex_indices[i].push(other_last);
                    last_index = other_last;
                    prev_prev_x = prev_x;
                    prev_prev_y = prev_y;
                    prev_x = x3;
                    prev_y = y3;
                    ii = 0;
                    continue;
                }
                ii += 1;
            }
        }

        // Remove empty polygons that resulted from the merge step above, and close the rest.
        for i in (0..convex_polygons.len()).rev() {
            if convex_polygons[i].is_empty() {
                self.polygon_pool.push(convex_polygons.remove(i));
                self.indices_pool.push(convex_indices.remove(i));
            } else {
                let (x, y) = (convex_polygons[i][0], convex_polygons[i][1]);
                convex_polygons[i].push(x);
                convex_polygons[i].push(y);
            }
        }
        self.triangles = triangles;
        self.convex_polygons = convex_polygons;
        self.convex_indices = convex_indices;
        &self.convex_polygons
    }

    /// The polygons from the last [`decompose`](Self::decompose) call.
    pub fn polygons(&self) -> &[Vec<f32>] {
        &self.convex_polygons
    }
}

fn is_concave(index: i32, vertex_count: i32, vertices: &[f32], indices: &[i32]) -> bool {
    let previous = (indices[if index > 0 {
        index - 1
    } else {
        vertex_count - 1
    } as usize]
        << 1) as usize;
    let current = (indices[index as usize] << 1) as usize;
    let next = (indices[if index + 1 < vertex_count {
        index + 1
    } else {
        0
    } as usize]
        << 1) as usize;
    !positive_area(
        vertices[previous],
        vertices[previous + 1],
        vertices[current],
        vertices[current + 1],
        vertices[next],
        vertices[next + 1],
    )
}

fn positive_area(p1x: f32, p1y: f32, p2x: f32, p2y: f32, p3x: f32, p3y: f32) -> bool {
    p1x * (p3y - p2y) + p2x * (p1y - p3y) + p3x * (p2y - p1y) >= 0.0
}

fn winding(p1x: f32, p1y: f32, p2x: f32, p2y: f32, p3x: f32, p3y: f32) -> i32 {
    if p1x * (p3y - p2y) + p2x * (p1y - p3y) + p3x * (p2y - p1y) >= 0.0 {
        1
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(vertices: &[f32], tri: &[i32]) -> f32 {
        tri.chunks(3)
            .map(|t| {
                let p = |i: i32| (vertices[(i * 2) as usize], vertices[(i * 2 + 1) as usize]);
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                ((b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1)).abs() / 2.0
            })
            .sum()
    }

    fn covered_area(polygons: &[Vec<f32>]) -> f32 {
        polygons
            .iter()
            .map(|p| {
                let n = p.len() / 2 - 1;
                let twice_area: f32 = (0..n)
                    .map(|i| p[i * 2] * p[i * 2 + 3] - p[i * 2 + 2] * p[i * 2 + 1])
                    .sum();
                twice_area.abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn triangulates_convex_and_concave_polygons_without_losing_area() {
        let mut t = Triangulator::new();
        // Polygons are clockwise (y up), like the clipper makes them.
        let square = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 10.0, 0.0];
        assert_eq!(area(&square, t.triangulate(&square)), 100.0);
        // An L shape (concave): 200 + 100 = 300 area.
        let l = [
            0.0, 0.0, 0.0, 20.0, 10.0, 20.0, 10.0, 10.0, 20.0, 10.0, 20.0, 0.0,
        ];
        let tri = t.triangulate(&l).to_vec();
        assert_eq!(tri.len(), (6 - 2) * 3);
        assert_eq!(area(&l, &tri), 300.0);
        // A triangle stays one triangle.
        let one = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0];
        assert_eq!(t.triangulate(&one).len(), 3);
    }

    #[test]
    fn decompose_yields_closed_convex_polygons_covering_the_shape() {
        let mut t = Triangulator::new();
        let l = [
            0.0, 0.0, 0.0, 20.0, 10.0, 20.0, 10.0, 10.0, 20.0, 10.0, 20.0, 0.0,
        ];
        t.triangulate(&l);
        let polys = t.decompose(&l);
        assert!(polys.len() >= 2 && polys.len() <= 4);
        assert_eq!(covered_area(polys), 300.0);
    }

    #[test]
    fn decompose_gives_the_same_polygons_when_its_buffers_are_reused() {
        let mut t = Triangulator::new();
        let l = [
            0.0, 0.0, 0.0, 20.0, 10.0, 20.0, 10.0, 10.0, 20.0, 10.0, 20.0, 0.0,
        ];
        let square = [0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 10.0, 0.0];
        t.triangulate(&l);
        let first = t.decompose(&l).to_vec();
        t.triangulate(&square);
        assert_eq!(covered_area(t.decompose(&square)), 100.0);
        t.triangulate(&l);
        assert_eq!(t.decompose(&l), first.as_slice());
    }
}
