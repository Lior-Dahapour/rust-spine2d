//! Debug overlay for a sprite: the root, bones, region outlines, mesh wireframes, bounding boxes
//! and clipping polygons, drawn above the batches.

use godot::builtin::{Color, PackedColorArray, PackedVector2Array, Rid, Transform2D, Vector2};
use godot::classes::RenderingServer;
use godot::obj::{Gd, Singleton};
use spine2d_core::attachment::{Attachment, RegionAttachment};
use spine2d_core::skeleton::BorrowedSkeletonRef;
use spine2d_core::Bone;
use spine2d_render::{RenderCommand, SkeletonBounds};

/// Which overlays to draw, and in which colors.
///
/// The paths flag exists for scripts written for the official plugin, which does not draw paths
/// either.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugFlags {
    pub root: bool,
    pub root_color: Color,
    pub bones: bool,
    pub bones_color: Color,
    /// The half width of the shape drawn for a bone.
    pub bones_thickness: f32,
    pub regions: bool,
    pub regions_color: Color,
    pub meshes: bool,
    pub meshes_color: Color,
    pub bounding_boxes: bool,
    pub bounding_boxes_color: Color,
    pub paths: bool,
    pub paths_color: Color,
    pub clipping: bool,
    pub clipping_color: Color,
}

impl Default for DebugFlags {
    fn default() -> Self {
        DebugFlags {
            root: false,
            root_color: Color::from_rgba(1.0, 1.0, 1.0, 0.5),
            bones: false,
            bones_color: Color::from_rgba(1.0, 1.0, 0.0, 0.5),
            bones_thickness: 5.0,
            regions: false,
            regions_color: Color::from_rgba(0.0, 0.0, 1.0, 0.5),
            meshes: false,
            meshes_color: Color::from_rgba(0.0, 0.0, 1.0, 0.5),
            bounding_boxes: false,
            bounding_boxes_color: Color::from_rgba(0.0, 1.0, 0.0, 0.5),
            paths: false,
            paths_color: Color::from_rgba8(0xff, 0x7f, 0x00, 0x77),
            clipping: false,
            clipping_color: Color::from_rgba(0.8, 0.0, 0.0, 0.8),
        }
    }
}

impl DebugFlags {
    /// Whether anything is drawn.
    pub fn any(&self) -> bool {
        self.root
            || self.bones
            || self.regions
            || self.meshes
            || self.bounding_boxes
            || self.clipping
    }
}

/// A canvas item that draws the overlays. It sits above the sprite's batch meshes.
pub struct DebugLayer {
    item: Rid,
    bounds: SkeletonBounds,
}

impl DebugLayer {
    pub fn new(parent_item: Rid) -> Self {
        let mut rs = RenderingServer::singleton();
        let item = rs.canvas_item_create();
        rs.canvas_item_set_parent(item, parent_item);
        rs.canvas_item_set_z_index(item, 1);
        DebugLayer {
            item,
            bounds: SkeletonBounds::new(),
        }
    }

    /// Removes everything drawn so far.
    pub fn clear(&self) {
        RenderingServer::singleton().canvas_item_clear(self.item);
    }

    /// Redraws the overlays selected by `flags` for the current pose.
    pub fn draw(
        &mut self,
        flags: &DebugFlags,
        skeleton: &BorrowedSkeletonRef,
        commands: &[RenderCommand],
    ) {
        let mut rs = RenderingServer::singleton();
        rs.canvas_item_clear(self.item);
        if flags.regions {
            self.draw_regions(&mut rs, flags.regions_color, skeleton);
        }
        if flags.meshes {
            let mut points = PackedVector2Array::new();
            for command in commands {
                for triangle in command.indices.chunks_exact(3) {
                    let at = |i: u16| {
                        Vector2::new(
                            command.positions[i as usize * 2],
                            command.positions[i as usize * 2 + 1],
                        )
                    };
                    let (a, b, c) = (at(triangle[0]), at(triangle[1]), at(triangle[2]));
                    for (from, to) in [(a, b), (b, c), (c, a)] {
                        points.push(from);
                        points.push(to);
                    }
                }
            }
            if !points.is_empty() {
                let mut colors = PackedColorArray::new();
                colors.push(flags.meshes_color);
                rs.canvas_item_add_multiline(self.item, &points, &colors);
            }
        }
        if flags.bounding_boxes {
            self.bounds.update(skeleton, false);
            for polygon in self.bounds.polygons() {
                let points: PackedVector2Array = polygon.vertices[..polygon.count]
                    .chunks_exact(2)
                    .map(|p| Vector2::new(p[0], p[1]))
                    .collect();
                self.add_closed_polyline(&mut rs, points, flags.bounding_boxes_color);
            }
        }
        if flags.clipping {
            self.draw_clipping(&mut rs, flags.clipping_color, skeleton);
        }
        if flags.root {
            if let Some(root) = skeleton.root_bone() {
                let length = skeleton.data().bones[0].length;
                self.draw_bone(
                    &mut rs,
                    root,
                    length,
                    flags.bones_thickness,
                    flags.root_color,
                );
            }
        }
        if flags.bones {
            for (bone, data) in skeleton.bones().iter().zip(&skeleton.data().bones) {
                if bone.is_active() {
                    self.draw_bone(
                        &mut rs,
                        bone,
                        data.length,
                        flags.bones_thickness,
                        flags.bones_color,
                    );
                }
            }
        }
        rs.canvas_item_add_set_transform(self.item, Transform2D::IDENTITY);
    }

    /// Draws the outline and the two triangles of every region attachment shown.
    fn draw_regions(
        &self,
        rs: &mut Gd<RenderingServer>,
        color: Color,
        skeleton: &BorrowedSkeletonRef,
    ) {
        let mut vertices = [0.0f32; 8];
        for &slot_id in skeleton.draw_order().applied_pose() {
            let slot = skeleton.slot(slot_id);
            if !skeleton.bone(slot.bone()).is_active() {
                continue;
            }
            let Some(attachment) = &slot.applied_pose().attachment else {
                continue;
            };
            let Attachment::Region(region) = &**attachment else {
                continue;
            };
            RegionAttachment::compute_world_vertices(
                skeleton,
                slot_id,
                region.offsets(slot.applied_pose()),
                &mut vertices,
                0,
                2,
            );
            let corner = |i: usize| Vector2::new(vertices[i * 2], vertices[i * 2 + 1]);
            let mut lines = PackedVector2Array::new();
            for (from, to) in [(0, 1), (1, 2), (2, 0), (2, 3), (3, 0)] {
                lines.push(corner(from));
                lines.push(corner(to));
            }
            let mut colors = PackedColorArray::new();
            colors.push(color);
            rs.canvas_item_add_multiline(self.item, &lines, &colors);
            let hull: PackedVector2Array = (0..4).map(corner).collect();
            self.add_closed_polyline(rs, hull, color);
        }
    }

    /// Draws the polygon of every clipping attachment shown.
    fn draw_clipping(
        &self,
        rs: &mut Gd<RenderingServer>,
        color: Color,
        skeleton: &BorrowedSkeletonRef,
    ) {
        let mut vertices = Vec::new();
        for &slot_id in skeleton.draw_order().applied_pose() {
            let slot = skeleton.slot(slot_id);
            if !skeleton.bone(slot.bone()).is_active() {
                continue;
            }
            let Some(attachment) = &slot.applied_pose().attachment else {
                continue;
            };
            let Attachment::Clipping(clipping) = &**attachment else {
                continue;
            };
            let count = clipping.vertex.world_vertices_length;
            vertices.clear();
            vertices.resize(count, 0.0);
            clipping.vertex.compute_world_vertices(
                skeleton,
                slot_id,
                0,
                count,
                &mut vertices,
                0,
                2,
            );
            let points: PackedVector2Array = vertices
                .chunks_exact(2)
                .map(|p| Vector2::new(p[0], p[1]))
                .collect();
            self.add_closed_polyline(rs, points, color);
        }
    }

    /// Draws the outline of `points`, joining the last point to the first.
    fn add_closed_polyline(
        &self,
        rs: &mut Gd<RenderingServer>,
        mut points: PackedVector2Array,
        color: Color,
    ) {
        if let Some(first) = points.get(0) {
            points.push(first);
        }
        let mut colors = PackedColorArray::new();
        colors.push(color);
        rs.canvas_item_add_polyline(self.item, &points, &colors);
    }

    /// Draws `bone` as a diamond as long as the bone, or as wide as the bones are thick if it has
    /// no length, placed by its world transform.
    fn draw_bone(
        &self,
        rs: &mut Gd<RenderingServer>,
        bone: &Bone,
        length: f32,
        thickness: f32,
        color: Color,
    ) {
        let pose = bone.applied_pose();
        let transform = Transform2D::from_cols(
            Vector2::new(pose.a, pose.c),
            Vector2::new(pose.b, pose.d),
            Vector2::new(pose.world_x, pose.world_y),
        );
        let length = if length == 0.0 {
            thickness * 2.0
        } else {
            length
        };
        let points = PackedVector2Array::from(&[
            Vector2::new(-thickness, 0.0),
            Vector2::new(0.0, thickness),
            Vector2::new(length, 0.0),
            Vector2::new(0.0, -thickness),
        ]);
        let mut colors = PackedColorArray::new();
        colors.push(color);
        rs.canvas_item_add_set_transform(self.item, transform);
        rs.canvas_item_add_polygon(self.item, &points, &colors);
    }
}
