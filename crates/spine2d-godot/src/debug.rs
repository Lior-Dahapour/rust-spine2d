//! Debug overlay for a sprite: bones, bounding boxes and mesh wireframes, drawn above the batches.

use godot::builtin::{Color, PackedColorArray, PackedVector2Array, Rid, Vector2};
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use spine2d_core::Skeleton;
use spine2d_render::{RenderCommand, SkeletonBounds};

const BONE_COLOR: Color = Color::from_rgba(0.9, 0.8, 0.1, 1.0);
const JOINT_COLOR: Color = Color::from_rgba(0.9, 0.2, 0.2, 1.0);
const BOX_COLOR: Color = Color::from_rgba(0.3, 0.6, 1.0, 1.0);
const MESH_COLOR: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.35);

/// Which overlays to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebugFlags {
    pub bones: bool,
    pub bounding_boxes: bool,
    pub meshes: bool,
}

impl DebugFlags {
    pub fn any(self) -> bool {
        self.bones || self.bounding_boxes || self.meshes
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
    pub fn draw(&mut self, flags: DebugFlags, skeleton: &Skeleton, commands: &[RenderCommand]) {
        let mut rs = RenderingServer::singleton();
        rs.canvas_item_clear(self.item);
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
                colors.push(MESH_COLOR);
                rs.canvas_item_add_multiline(self.item, &points, &colors);
            }
        }
        if flags.bounding_boxes {
            self.bounds.update(skeleton, false);
            for polygon in self.bounds.polygons() {
                let mut points: PackedVector2Array = polygon.vertices[..polygon.count]
                    .chunks_exact(2)
                    .map(|p| Vector2::new(p[0], p[1]))
                    .collect();
                if let Some(first) = points.get(0) {
                    points.push(first);
                }
                let mut colors = PackedColorArray::new();
                colors.push(BOX_COLOR);
                rs.canvas_item_add_polyline(self.item, &points, &colors);
            }
        }
        if flags.bones {
            for (bone, data) in skeleton.bones().iter().zip(&skeleton.data().bones) {
                let pose = bone.applied_pose();
                let from = Vector2::new(pose.world_x, pose.world_y);
                let to = Vector2::new(
                    pose.world_x + pose.a * data.length,
                    pose.world_y + pose.c * data.length,
                );
                rs.canvas_item_add_line(self.item, from, to, BONE_COLOR);
                rs.canvas_item_add_circle(self.item, from, 3.0, JOINT_COLOR);
            }
        }
    }
}

impl Drop for DebugLayer {
    fn drop(&mut self) {
        RenderingServer::singleton().free_rid(self.item);
    }
}
