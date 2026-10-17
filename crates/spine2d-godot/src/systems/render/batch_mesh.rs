//! A GPU mesh that is created once and then updated in place every frame.
//!
//! Each [`MeshBatch`] draws one [`BatchGroup`] of the renderer's output: a run of attachments that
//! share a texture, blend mode and tint. Geometry is written straight from the renderer's
//! sub-commands into the mesh's own byte buffers, so there is no intermediate Godot array and no
//! merge copy. Positions go in with one bulk copy per sub-command, and indices are compared and
//! copied as whole slices.

use bytemuck::{cast_slice, Pod};
use godot::builtin::{
    Aabb, PackedByteArray, PackedColorArray, PackedFloat32Array, PackedInt32Array,
    PackedVector2Array, Rid, Variant, Vector3,
};
use godot::classes::rendering_server::{ArrayCustomFormat, ArrayFormat, ArrayType, PrimitiveType};
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use godot::prelude::*;
use spine2d_render::{BatchGroup, RenderCommand};

/// Floats per light-color value stored per vertex when two-color tinting is on (RGBA).
const LIGHT_FLOATS: usize = 4;
/// Floats per dark-color value stored per vertex when two-color tinting is on (RGB).
const DARK_FLOATS: usize = 3;
/// Bytes of one 2D vertex position in the vertex buffer.
const POSITION_BYTES: usize = 8;
/// Fraction of the box size added on every side when the custom bounds have to grow, so a moving
/// skeleton does not update them every frame.
const BOUNDS_MARGIN: f32 = 0.25;

/// Geometry capacity and byte layout of the surface currently in the mesh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Layout {
    vertex_capacity: usize,
    index_capacity: usize,
    two_color: bool,
    attribute_stride: usize,
    index_stride: usize,
    uv_offset: usize,
    color_offset: usize,
    dark_offset: usize,
    light_offset: usize,
}

/// Counters for how often the expensive paths run; used by the benchmarks and reported in the log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchStats {
    /// Times the mesh was created or re-created because it was outgrown.
    pub rebuilds: u64,
    /// Times writing into a buffer made Godot copy it first (the array was still shared).
    pub detaches: u64,
    /// Times the index region was uploaded.
    pub index_uploads: u64,
    /// Times the vertex (position) region was uploaded.
    pub vertex_uploads: u64,
    /// Times the attribute (uv and color) region was uploaded.
    pub attribute_uploads: u64,
}

impl BatchStats {
    pub fn add(&mut self, other: BatchStats) {
        self.rebuilds += other.rebuilds;
        self.detaches += other.detaches;
        self.index_uploads += other.index_uploads;
        self.vertex_uploads += other.vertex_uploads;
        self.attribute_uploads += other.attribute_uploads;
    }
}

/// One persistent mesh drawn through its own canvas item.
pub struct MeshBatch {
    canvas_item: Rid,
    mesh: Rid,
    layout: Layout,
    vertex_bytes: PackedByteArray,
    attribute_bytes: PackedByteArray,
    index_bytes: PackedByteArray,
    /// Reused staging buffers for the offset indices of one upload.
    indices_u16: Vec<u16>,
    indices_u32: Vec<u32>,
    /// Number of indices that were non-padding in the last upload.
    uploaded_index_count: usize,
    /// The custom bounds last given to the render server.
    uploaded_bounds: Option<Aabb>,
    /// The exact bounds of the geometry drawn by the last upload; `None` while hidden.
    drawn_bounds: Option<Rect2>,
    /// Texture and material the canvas item currently draws with.
    drawn_texture: Rid,
    drawn_material: Rid,
    visible: bool,
    /// Position among the sprite's other canvas items; see [`set_draw_index`](Self::set_draw_index).
    draw_index: i32,
    /// False when the render server returned no usable buffers (eg the headless dummy renderer);
    /// the batch then skips drawing instead of writing out of bounds.
    usable: bool,
    stats: BatchStats,
}

impl MeshBatch {
    /// Creates an empty batch whose canvas item is a child of `parent_item`.
    pub fn new(parent_item: Rid) -> Self {
        let mut rs = RenderingServer::singleton();
        let canvas_item = rs.canvas_item_create();
        rs.canvas_item_set_parent(canvas_item, parent_item);
        MeshBatch {
            canvas_item,
            mesh: Rid::Invalid,
            layout: Layout::default(),
            vertex_bytes: PackedByteArray::new(),
            attribute_bytes: PackedByteArray::new(),
            index_bytes: PackedByteArray::new(),
            indices_u16: Vec::new(),
            indices_u32: Vec::new(),
            uploaded_index_count: 0,
            uploaded_bounds: None,
            drawn_bounds: None,
            drawn_texture: Rid::Invalid,
            drawn_material: Rid::Invalid,
            visible: true,
            draw_index: 0,
            usable: true,
            stats: BatchStats::default(),
        }
    }

    pub fn stats(&self) -> BatchStats {
        self.stats
    }

    /// The exact bounds of what this batch drew last, in the sprite's coordinates; `None` if it is
    /// hidden.
    pub fn drawn_bounds(&self) -> Option<Rect2> {
        self.drawn_bounds
    }

    /// Sets where this batch draws among its siblings (batches and slot nodes). Items with a lower
    /// index draw first. Does nothing when the index is unchanged.
    pub fn set_draw_index(&mut self, index: i32) {
        if self.draw_index != index {
            RenderingServer::singleton().canvas_item_set_draw_index(self.canvas_item, index);
            self.draw_index = index;
        }
    }

    /// Hides or shows the batch without touching its buffers.
    pub fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.drawn_bounds = None;
        }
        if self.visible != visible {
            RenderingServer::singleton().canvas_item_set_visible(self.canvas_item, visible);
            self.visible = visible;
        }
    }

    /// Uploads `group` (made of `commands`) into the mesh and points the canvas item at `texture`
    /// and the material chosen by `material`, which is told whether the batch is two-color.
    ///
    /// A batch that has been two-color stays two-color: switching back would rebuild the mesh
    /// every time a dark tint flips between black and a color.
    pub fn upload(
        &mut self,
        group: &BatchGroup,
        commands: &[RenderCommand],
        wants_two_color: bool,
        texture: Rid,
        material: impl FnOnce(bool) -> Rid,
    ) {
        if group.vertex_count == 0 || group.index_count == 0 {
            self.set_visible(false);
            return;
        }
        let two_color = wants_two_color || self.layout.two_color;
        let material = material(two_color);
        let needs_rebuild = self.mesh == Rid::Invalid
            || group.vertex_count > self.layout.vertex_capacity
            || group.index_count > self.layout.index_capacity
            || two_color != self.layout.two_color;
        let mut redraw = false;
        if needs_rebuild {
            self.rebuild(group, two_color);
            redraw = true;
        }
        if !self.usable {
            self.set_visible(false);
            return;
        }
        let bounds = self.write_vertices(group, commands);
        self.write_indices(group, commands);
        self.set_visible(true);
        self.drawn_bounds = Some(Rect2::new(
            Vector2::new(bounds.position.x, bounds.position.y),
            Vector2::new(bounds.size.x, bounds.size.y),
        ));

        let mut rs = RenderingServer::singleton();
        if let Some(bounds) = self.grown_bounds(bounds) {
            rs.mesh_set_custom_aabb(self.mesh, bounds);
            self.uploaded_bounds = Some(bounds);
        }
        if texture != self.drawn_texture {
            self.drawn_texture = texture;
            redraw = true;
        }
        if material != self.drawn_material {
            rs.canvas_item_set_material(self.canvas_item, material);
            self.drawn_material = material;
        }
        if redraw {
            rs.canvas_item_clear(self.canvas_item);
            rs.canvas_item_add_mesh_ex(self.canvas_item, self.mesh)
                .texture(texture)
                .done();
        }
    }

    /// The custom bounds to send to the render server, or `None` if the ones it has still contain
    /// `needed`. New bounds get a margin so a moving skeleton rarely triggers an update.
    fn grown_bounds(&self, needed: Aabb) -> Option<Aabb> {
        if self
            .uploaded_bounds
            .is_some_and(|current| current.encloses(needed))
        {
            return None;
        }
        let margin = needed.size * BOUNDS_MARGIN;
        Some(Aabb::new(
            needed.position - margin,
            needed.size + margin * 2.0,
        ))
    }

    /// Writes positions, UVs and tint colors of the group's sub-commands and uploads the regions
    /// that changed. Returns the bounds of the positions.
    fn write_vertices(&mut self, group: &BatchGroup, commands: &[RenderCommand]) -> Aabb {
        let l = self.layout;
        let vertex_ptr = self.vertex_bytes.as_slice().as_ptr();
        let attribute_ptr = self.attribute_bytes.as_slice().as_ptr();
        let vertices = self.vertex_bytes.as_mut_slice();
        let attributes = self.attribute_bytes.as_mut_slice();
        // A changed address means Godot copied the array because something still shared it.
        self.stats.detaches += (vertices.as_ptr() != vertex_ptr) as u64
            + (attributes.as_ptr() != attribute_ptr) as u64;

        let color = color_bytes(group.color);
        let light: [u8; LIGHT_FLOATS * 4] = bytemuck::cast(color_floats(group.color));
        let dark_floats = color_floats(group.dark_color);
        let dark: [u8; DARK_FLOATS * 4] =
            bytemuck::cast([dark_floats[0], dark_floats[1], dark_floats[2]]);
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let (mut vertices_changed, mut attributes_changed) = (false, false);
        let mut vertex = 0usize;
        for command in commands {
            let count = command.positions.len() / 2;
            if count == 0 {
                continue;
            }
            // Positions are tightly packed 2D vectors: one compare and copy per sub-command.
            let target = &mut vertices[vertex * POSITION_BYTES..(vertex + count) * POSITION_BYTES];
            let positions: &[u8] = cast_slice(&command.positions);
            if target != positions {
                target.copy_from_slice(positions);
                vertices_changed = true;
            }
            for p in command.positions.chunks_exact(2) {
                min_x = min_x.min(p[0]);
                max_x = max_x.max(p[0]);
                min_y = min_y.min(p[1]);
                max_y = max_y.max(p[1]);
            }
            let records =
                &mut attributes[vertex * l.attribute_stride..(vertex + count) * l.attribute_stride];
            for (record, uv) in records
                .chunks_exact_mut(l.attribute_stride)
                .zip(command.uvs.chunks_exact(2))
            {
                attributes_changed |=
                    set_bytes(&mut record[l.uv_offset..l.uv_offset + 8], cast_slice(uv));
                if l.two_color {
                    // Godot modulates COLOR before vertex(); the real tint travels in custom attributes.
                    attributes_changed |=
                        set_bytes(&mut record[l.color_offset..l.color_offset + 4], &[255; 4]);
                    attributes_changed |= set_bytes(
                        &mut record[l.light_offset..l.light_offset + LIGHT_FLOATS * 4],
                        &light,
                    );
                    attributes_changed |= set_bytes(
                        &mut record[l.dark_offset..l.dark_offset + DARK_FLOATS * 4],
                        &dark,
                    );
                } else {
                    attributes_changed |=
                        set_bytes(&mut record[l.color_offset..l.color_offset + 4], &color);
                }
            }
            vertex += count;
        }
        debug_assert_eq!(vertex, group.vertex_count);

        let mut rs = RenderingServer::singleton();
        // A fresh surface already holds zeros, so its first upload must always go through.
        if vertices_changed || self.uploaded_index_count == 0 {
            rs.mesh_surface_update_vertex_region(self.mesh, 0, 0, &self.vertex_bytes);
            self.stats.vertex_uploads += 1;
        }
        if attributes_changed || self.uploaded_index_count == 0 {
            rs.mesh_surface_update_attribute_region(self.mesh, 0, 0, &self.attribute_bytes);
            self.stats.attribute_uploads += 1;
        }
        Aabb::new(
            Vector3::new(min_x, min_y, 0.0),
            Vector3::new(max_x - min_x, max_y - min_y, 0.0),
        )
    }

    /// Writes the group's indices (offset per sub-command) and zero-pads what the last upload used
    /// beyond this group's length. Uploads only when something changed.
    fn write_indices(&mut self, group: &BatchGroup, commands: &[RenderCommand]) {
        let stride = self.layout.index_stride;
        let index_ptr = self.index_bytes.as_slice().as_ptr();
        let bytes = self.index_bytes.as_mut_slice();
        self.stats.detaches += (bytes.as_ptr() != index_ptr) as u64;

        let changed = if stride == 2 {
            fill_indices(&mut self.indices_u16, commands);
            copy_if_different(
                bytes,
                cast_slice(&self.indices_u16),
                self.uploaded_index_count * stride,
            )
        } else {
            fill_indices(&mut self.indices_u32, commands);
            copy_if_different(
                bytes,
                cast_slice(&self.indices_u32),
                self.uploaded_index_count * stride,
            )
        };
        self.uploaded_index_count = group.index_count;
        if changed {
            RenderingServer::singleton().mesh_surface_update_index_region(
                self.mesh,
                0,
                0,
                &self.index_bytes,
            );
            self.stats.index_uploads += 1;
        }
    }

    /// Creates a fresh surface big enough for `group`, growing geometrically when an earlier
    /// surface was outgrown (eg animated clipping).
    fn rebuild(&mut self, group: &BatchGroup, two_color: bool) {
        let vertex_capacity = grow(self.layout.vertex_capacity, group.vertex_count);
        let index_capacity = grow(self.layout.index_capacity, group.index_count);
        let mut rs = RenderingServer::singleton();
        if self.mesh != Rid::Invalid {
            rs.free_rid(self.mesh);
        }
        self.mesh = rs.mesh_create();
        self.stats.rebuilds += 1;

        let mut arrays = VarArray::new();
        for _ in 0..ArrayType::MAX.ord() {
            arrays.push(&Variant::nil());
        }
        let mut vertices = PackedVector2Array::new();
        vertices.resize(vertex_capacity);
        let mut uvs = PackedVector2Array::new();
        uvs.resize(vertex_capacity);
        let mut colors = PackedColorArray::new();
        colors.resize(vertex_capacity);
        let mut indices = PackedInt32Array::new();
        indices.resize(index_capacity);
        arrays.set(ArrayType::VERTEX.ord() as usize, &vertices.to_variant());
        arrays.set(ArrayType::TEX_UV.ord() as usize, &uvs.to_variant());
        arrays.set(ArrayType::COLOR.ord() as usize, &colors.to_variant());
        arrays.set(ArrayType::INDEX.ord() as usize, &indices.to_variant());

        let mut format = ArrayFormat::FLAG_USE_DYNAMIC_UPDATE;
        if two_color {
            let mut dark = PackedFloat32Array::new();
            dark.resize(vertex_capacity * DARK_FLOATS);
            let mut light = PackedFloat32Array::new();
            light.resize(vertex_capacity * LIGHT_FLOATS);
            arrays.set(ArrayType::CUSTOM0.ord() as usize, &dark.to_variant());
            arrays.set(ArrayType::CUSTOM1.ord() as usize, &light.to_variant());
            let custom0 =
                (ArrayCustomFormat::RGB_FLOAT.ord() as u64) << ArrayFormat::CUSTOM0_SHIFT.ord();
            let custom1 =
                (ArrayCustomFormat::RGBA_FLOAT.ord() as u64) << ArrayFormat::CUSTOM1_SHIFT.ord();
            format = ArrayFormat::from_ord(format.ord() | custom0 | custom1);
        }
        rs.mesh_add_surface_from_arrays_ex(self.mesh, PrimitiveType::TRIANGLES, &arrays)
            .compress_format(format)
            .done();

        let surface = rs.mesh_get_surface(self.mesh, 0);
        let surface_format = ArrayFormat::from_ord(
            surface
                .get("format")
                .and_then(|v| v.try_to::<i64>().ok())
                .unwrap_or(0) as u64,
        );
        let count = vertex_capacity as i32;
        let offset = |array: ArrayType| {
            rs.mesh_surface_get_format_offset(surface_format, count, array.ord()) as usize
        };
        let vertex_stride =
            rs.mesh_surface_get_format_vertex_stride(surface_format, count) as usize;
        self.layout = Layout {
            vertex_capacity,
            index_capacity,
            two_color,
            attribute_stride: rs.mesh_surface_get_format_attribute_stride(surface_format, count)
                as usize,
            index_stride: rs.mesh_surface_get_format_index_stride(surface_format, count) as usize,
            uv_offset: offset(ArrayType::TEX_UV),
            color_offset: offset(ArrayType::COLOR),
            dark_offset: if two_color {
                offset(ArrayType::CUSTOM0)
            } else {
                0
            },
            light_offset: if two_color {
                offset(ArrayType::CUSTOM1)
            } else {
                0
            },
        };
        self.vertex_bytes = surface
            .get("vertex_data")
            .and_then(|v| v.try_to().ok())
            .unwrap_or_default();
        self.attribute_bytes = surface
            .get("attribute_data")
            .and_then(|v| v.try_to().ok())
            .unwrap_or_default();
        self.index_bytes = surface
            .get("index_data")
            .and_then(|v| v.try_to().ok())
            .unwrap_or_default();
        self.uploaded_index_count = 0;
        self.uploaded_bounds = None;
        self.drawn_texture = Rid::Invalid;
        let l = &self.layout;
        // The fast paths above assume tightly packed 2D positions and in-bounds attribute records.
        self.usable = vertex_stride == POSITION_BYTES
            && l.attribute_stride > 0
            && matches!(l.index_stride, 2 | 4)
            && l.uv_offset + 8 <= l.attribute_stride
            && l.color_offset + 4 <= l.attribute_stride
            && (!two_color
                || (l.light_offset + LIGHT_FLOATS * 4 <= l.attribute_stride
                    && l.dark_offset + DARK_FLOATS * 4 <= l.attribute_stride))
            && self.vertex_bytes.len() >= vertex_capacity * POSITION_BYTES
            && self.attribute_bytes.len() >= vertex_capacity * l.attribute_stride
            && self.index_bytes.len() >= index_capacity * l.index_stride;
    }
}

impl Drop for MeshBatch {
    fn drop(&mut self) {
        let mut rs = RenderingServer::singleton();
        if self.mesh != Rid::Invalid {
            rs.free_rid(self.mesh);
        }
        rs.free_rid(self.canvas_item);
    }
}

/// Fills `out` with the group's indices, each sub-command's indices offset by the vertices before it.
fn fill_indices<T>(out: &mut Vec<T>, commands: &[RenderCommand])
where
    T: Pod + From<u16> + std::ops::Add<Output = T>,
{
    out.clear();
    let mut vertex_offset = T::from(0);
    for command in commands {
        out.extend(
            command
                .indices
                .iter()
                .map(|&index| T::from(index) + vertex_offset),
        );
        vertex_offset = vertex_offset + T::from(command.vertex_count() as u16);
    }
}

/// Copies `new` into `target` if it differs. Returns whether it did.
fn set_bytes(target: &mut [u8], new: &[u8]) -> bool {
    if target == new {
        return false;
    }
    target.copy_from_slice(new);
    true
}

/// Copies `new` to the front of `target` if it differs from what is there, and zeroes the bytes up
/// to `previously_used` that `new` no longer covers. Returns whether anything changed.
fn copy_if_different(target: &mut [u8], new: &[u8], previously_used: usize) -> bool {
    let mut changed = false;
    let head = &mut target[..new.len()];
    if head != new {
        head.copy_from_slice(new);
        changed = true;
    }
    if previously_used > new.len() {
        target[new.len()..previously_used].fill(0);
        changed = true;
    }
    changed
}

/// The first surface is sized exactly; later ones double so a growing batch rebuilds rarely.
fn grow(capacity: usize, count: usize) -> usize {
    match capacity {
        0 => count,
        c if c >= count => c,
        c => count.max(c.saturating_mul(2)),
    }
}

/// `0xAARRGGBB` as straight RGBA bytes.
fn color_bytes(packed: u32) -> [u8; 4] {
    [
        (packed >> 16) as u8,
        (packed >> 8) as u8,
        packed as u8,
        (packed >> 24) as u8,
    ]
}

/// `0xAARRGGBB` as straight RGBA floats in 0..=1.
fn color_floats(packed: u32) -> [f32; 4] {
    color_bytes(packed).map(|b| f32::from(b) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grow_is_exact_first_then_doubles() {
        assert_eq!(grow(0, 10), 10);
        assert_eq!(grow(10, 8), 10);
        assert_eq!(grow(10, 11), 20);
        assert_eq!(grow(10, 50), 50);
    }

    #[test]
    fn packed_colors_unpack_to_rgba() {
        assert_eq!(color_bytes(0x80ff4020), [0xff, 0x40, 0x20, 0x80]);
        assert_eq!(color_floats(0xff000000), [0.0, 0.0, 0.0, 1.0]);
    }

    fn command(vertices: usize, indices: &[u16]) -> RenderCommand {
        RenderCommand {
            positions: vec![0.0; vertices * 2],
            uvs: vec![0.0; vertices * 2],
            colors: vec![0; vertices],
            dark_colors: vec![0; vertices],
            indices: indices.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn indices_are_offset_by_the_vertices_before_them() {
        let commands = [command(3, &[0, 1, 2]), command(4, &[0, 1, 2, 2, 3, 0])];
        let mut out: Vec<u16> = Vec::new();
        fill_indices(&mut out, &commands);
        assert_eq!(out, [0, 1, 2, 3, 4, 5, 5, 6, 3]);
    }

    #[test]
    fn set_bytes_reports_whether_it_wrote() {
        let mut target = [1u8, 2, 3];
        assert!(!set_bytes(&mut target, &[1, 2, 3]));
        assert!(set_bytes(&mut target, &[1, 9, 3]));
        assert_eq!(target, [1, 9, 3]);
    }

    #[test]
    fn copy_reports_changes_and_zero_pads_the_tail() {
        let mut target = [1u8, 2, 3, 4, 5, 6];
        assert!(
            !copy_if_different(&mut target, &[1, 2, 3], 3),
            "identical data is not a change"
        );
        assert!(copy_if_different(&mut target, &[9, 2, 3], 3));
        assert_eq!(target, [9, 2, 3, 4, 5, 6]);
        assert!(
            copy_if_different(&mut target, &[9, 2], 5),
            "shrinking pads the dropped tail with zeros"
        );
        assert_eq!(target, [9, 2, 0, 0, 0, 6]);
    }
}
