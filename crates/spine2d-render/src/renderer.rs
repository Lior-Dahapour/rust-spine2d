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

//! Turns a posed skeleton into batched, engine-agnostic draw commands.

use crate::clipping::SkeletonClipping;
use spine2d_core::attachment::Attachment;
use spine2d_core::skeleton::BorrowedSkeletonRef;
use spine2d_core::{BlendMode, SlotId};

/// One draw call: a batch of triangles sharing a texture, blend mode and tint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderCommand {
    /// World positions, 2 floats per vertex.
    pub positions: Vec<f32>,
    /// Texture coordinates, 2 floats per vertex.
    pub uvs: Vec<f32>,
    /// Light color per vertex, packed as `0xAARRGGBB`.
    pub colors: Vec<u32>,
    /// Dark color per vertex, packed as `0xFFRRGGBB` (`0xFF000000` when the slot has none).
    pub dark_colors: Vec<u32>,
    pub indices: Vec<u16>,
    pub blend_mode: BlendMode,
    /// The renderer's texture handle (the `renderer_object` of the attachment's texture region).
    pub texture: usize,
}

impl RenderCommand {
    pub fn vertex_count(&self) -> usize {
        self.colors.len()
    }

    fn clear(&mut self) {
        self.positions.clear();
        self.uvs.clear();
        self.colors.clear();
        self.dark_colors.clear();
        self.indices.clear();
    }

    fn first_color(&self) -> u32 {
        self.colors.first().copied().unwrap_or(0)
    }

    fn first_dark_color(&self) -> u32 {
        self.dark_colors.first().copied().unwrap_or(0)
    }
}

/// A run of consecutive sub-commands that [`SkeletonRenderer::render`] merges into one command:
/// same texture, blend mode and tint. Backends that upload geometry themselves can walk a group's
/// sub-commands and write straight into their own buffers instead of using the merged command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchGroup {
    /// Index of the group's first sub-command in [`Batches::commands`].
    pub start: usize,
    /// One past the index of the group's last sub-command.
    pub end: usize,
    /// Total vertices over the group's sub-commands.
    pub vertex_count: usize,
    /// Total indices over the group's sub-commands.
    pub index_count: usize,
    pub blend_mode: BlendMode,
    pub texture: usize,
    /// Light color of the group (`0xAARRGGBB`); the same for every vertex in the group.
    pub color: u32,
    /// Dark color of the group (`0xFFRRGGBB`); the same for every vertex in the group.
    pub dark_color: u32,
}

/// Where a slot sits among the batch groups, reported for the slots a caller asked to split at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotMarker {
    pub slot: SlotId,
    /// How many groups are drawn before this slot's place in the draw order. Anything the caller
    /// draws for the slot goes after group `groups_before - 1` and before group `groups_before`.
    pub groups_before: usize,
    /// Index of the first sub-command drawn after the slot; groups never straddle it.
    sub_position: usize,
}

/// The unmerged output of [`SkeletonRenderer::render_batches`].
#[derive(Clone, Copy, Debug)]
pub struct Batches<'a> {
    /// One command per drawn attachment, in draw order. Indices are relative to each command.
    pub commands: &'a [RenderCommand],
    /// How `commands` merge into draw calls.
    pub groups: &'a [BatchGroup],
    /// The slots passed as split points, in draw order. Empty for [`SkeletonRenderer::render_batches`].
    pub markers: &'a [SlotMarker],
}

impl<'a> Batches<'a> {
    /// The sub-commands that make up `group`.
    pub fn group_commands(&self, group: &BatchGroup) -> &'a [RenderCommand] {
        &self.commands[group.start..group.end]
    }
}

/// Builds [`RenderCommand`]s for skeletons. Keep one around and reuse it: it keeps its buffers
/// between calls, so steady-state rendering allocates nothing.
#[derive(Clone, Debug)]
pub struct SkeletonRenderer {
    clipping: SkeletonClipping,
    world_vertices: Vec<f32>,
    /// Unbatched commands, one per drawn attachment; recycled between calls.
    sub_commands: Vec<RenderCommand>,
    sub_count: usize,
    /// Merged commands handed to the caller; recycled between calls like `sub_commands`.
    batched: Vec<RenderCommand>,
    batched_count: usize,
    groups: Vec<BatchGroup>,
    markers: Vec<SlotMarker>,
}

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

impl Default for SkeletonRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SkeletonRenderer {
    pub fn new() -> Self {
        SkeletonRenderer {
            clipping: SkeletonClipping::new(),
            world_vertices: Vec::new(),
            sub_commands: Vec::new(),
            sub_count: 0,
            batched: Vec::new(),
            batched_count: 0,
            groups: Vec::new(),
            markers: Vec::new(),
        }
    }

    /// Renders the skeleton's applied pose in draw order. Consecutive attachments with the same
    /// texture, blend mode and colors are merged into one command. The returned commands are valid
    /// until the next call.
    pub fn render(&mut self, skeleton: &BorrowedSkeletonRef) -> &[RenderCommand] {
        self.build_sub_commands(skeleton, &[]);
        self.compute_groups();
        self.merge_groups();
        &self.batched[..self.batched_count]
    }

    /// Like [`render`](Self::render) but stops before merging: returns the per-attachment
    /// sub-commands and the groups that would be merged. Valid until the next call.
    #[inline]
    pub fn render_batches(&mut self, skeleton: &BorrowedSkeletonRef) -> Batches<'_> {
        self.render_batches_split(skeleton, &[])
    }

    /// Like [`render_batches`](Self::render_batches), but no group straddles the place in the draw
    /// order right after a slot with `split_slots[slot index] == true`, so the caller can draw
    /// something of its own there. [`Batches::markers`] says which groups come before each such
    /// slot, even when the slot draws nothing. Slots past the end of `split_slots` are not split.
    pub fn render_batches_split(
        &mut self,
        skeleton: &BorrowedSkeletonRef,
        split_slots: &[bool],
    ) -> Batches<'_> {
        self.build_sub_commands(skeleton, split_slots);
        self.compute_groups();
        Batches {
            commands: &self.sub_commands[..self.sub_count],
            groups: &self.groups,
            markers: &self.markers,
        }
    }

    fn build_sub_commands(&mut self, skeleton: &BorrowedSkeletonRef, split_slots: &[bool]) {
        let skeleton_data = skeleton.data();
        self.sub_count = 0;
        self.markers.clear();
        // The marker for a slot is placed when the next slot starts (or the loop ends), because
        // the loop body leaves early in many places.
        let mut pending_marker: Option<SlotId> = None;

        for &slot_id in skeleton.draw_order().applied_pose() {
            if let Some(slot) = pending_marker.take() {
                self.push_marker(slot);
            }
            if split_slots.get(slot_id.0).copied().unwrap_or(false) {
                pending_marker = Some(slot_id);
            }
            let slot = skeleton.slot(slot_id);
            let pose = slot.applied_pose();
            let Some(attachment) = &pose.attachment else {
                self.clipping.clip_end_slot(slot_id);
                continue;
            };

            // Early out if the slot color is 0 or the bone is not active.
            let is_clip = matches!(&**attachment, Attachment::Clipping(_));
            if (pose.color.a == 0.0 || !skeleton.bone(slot.bone()).is_active()) && !is_clip {
                self.clipping.clip_end_slot(slot_id);
                continue;
            }

            enum Source<'a> {
                Quad,
                Mesh(&'a [u16]),
            }
            let (attachment_color, texture, source, uvs_of): (_, usize, Source, (usize, bool));
            let mut vertices_count: usize;
            match &**attachment {
                Attachment::Region(region) => {
                    if region.color.a == 0.0 {
                        self.clipping.clip_end_slot(slot_id);
                        continue;
                    }
                    let sequence_index = region.sequence.resolve_index(pose);
                    texture = region
                        .sequence
                        .region(sequence_index)
                        .map_or(0, |r| r.renderer_object);
                    self.world_vertices.clear();
                    self.world_vertices.resize(8, 0.0);
                    spine2d_core::attachment::RegionAttachment::compute_world_vertices(
                        skeleton,
                        slot_id,
                        region.sequence.offsets(sequence_index),
                        &mut self.world_vertices,
                        0,
                        2,
                    );
                    vertices_count = 4;
                    attachment_color = region.color;
                    source = Source::Quad;
                    uvs_of = (sequence_index, true);
                }
                Attachment::Mesh(mesh) => {
                    if mesh.color.a == 0.0 {
                        self.clipping.clip_end_slot(slot_id);
                        continue;
                    }
                    let sequence_index = mesh.sequence.resolve_index(pose);
                    texture = mesh
                        .sequence
                        .region(sequence_index)
                        .map_or(0, |r| r.renderer_object);
                    let length = mesh.vertex.world_vertices_length;
                    self.world_vertices.clear();
                    self.world_vertices.resize(length, 0.0);
                    mesh.vertex.compute_world_vertices(
                        skeleton,
                        slot_id,
                        0,
                        length,
                        &mut self.world_vertices,
                        0,
                        2,
                    );
                    vertices_count = length >> 1;
                    attachment_color = mesh.color;
                    source = Source::Mesh(&mesh.triangles);
                    uvs_of = (sequence_index, false);
                }
                Attachment::Clipping(clip) => {
                    self.clipping.clip_start(skeleton, slot_id, clip);
                    continue;
                }
                _ => continue,
            }

            // Colors, packed like the reference (truncating each channel to 8 bits).
            let sk = skeleton.color;
            let channel = |s: f32, sl: f32, a: f32| (s * sl * a * 255.0) as u8 as u32;
            let (r, g, b, a) = (
                channel(sk.r, pose.color.r, attachment_color.r),
                channel(sk.g, pose.color.g, attachment_color.g),
                channel(sk.b, pose.color.b, attachment_color.b),
                channel(sk.a, pose.color.a, attachment_color.a),
            );
            let color = (a << 24) | (r << 16) | (g << 8) | b;
            let mut dark_color = 0xff00_0000u32;
            if pose.has_dark_color {
                let d = pose.dark_color;
                dark_color |= (((d.r * 255.0) as u8 as u32) << 16)
                    | (((d.g * 255.0) as u8 as u32) << 8)
                    | ((d.b * 255.0) as u8 as u32);
            }

            let sequence = match &**attachment {
                Attachment::Region(r) => &r.sequence,
                Attachment::Mesh(m) => &m.sequence,
                _ => unreachable!(),
            };
            let uvs = sequence.uvs(uvs_of.0);
            let indices: &[u16] = match source {
                Source::Quad => &QUAD_INDICES,
                Source::Mesh(t) => t,
            };

            let blend_mode = skeleton_data.slots[slot_id.0].blend_mode;
            let cmd = take_command(&mut self.sub_commands, &mut self.sub_count);
            cmd.blend_mode = blend_mode;
            cmd.texture = texture;

            if self.clipping.is_clipping() {
                self.clipping
                    .clip_triangles_with_uvs(&self.world_vertices, indices, uvs, 2);
                vertices_count = self.clipping.clipped_vertices().len() >> 1;
                cmd.positions
                    .extend_from_slice(self.clipping.clipped_vertices());
                cmd.uvs.extend_from_slice(self.clipping.clipped_uvs());
                cmd.indices
                    .extend_from_slice(self.clipping.clipped_triangles());
            } else {
                cmd.positions
                    .extend_from_slice(&self.world_vertices[..vertices_count << 1]);
                cmd.uvs.extend_from_slice(&uvs[..vertices_count << 1]);
                cmd.indices.extend_from_slice(indices);
            }
            cmd.colors.resize(vertices_count, color);
            cmd.dark_colors.resize(vertices_count, dark_color);
            self.clipping.clip_end_slot(slot_id);
        }
        if let Some(slot) = pending_marker {
            self.push_marker(slot);
        }
        self.clipping.clip_end();
    }

    fn push_marker(&mut self, slot: SlotId) {
        self.markers.push(SlotMarker {
            slot,
            groups_before: 0,
            sub_position: self.sub_count,
        });
    }

    /// Finds the runs of consecutive compatible sub-commands.
    fn compute_groups(&mut self) {
        let commands = &self.sub_commands[..self.sub_count];
        self.groups.clear();
        // Markers are in draw order. Those before the first command have no groups before them.
        let mut next_marker = 0;
        if commands.is_empty() {
            self.markers.iter_mut().for_each(|m| m.groups_before = 0);
            return;
        }
        while self
            .markers
            .get(next_marker)
            .is_some_and(|m| m.sub_position == 0)
        {
            self.markers[next_marker].groups_before = 0;
            next_marker += 1;
        }
        let mut first = 0;
        let mut start = 0;
        let mut num_vertices = commands[0].vertex_count();
        let mut num_indices = commands[0].indices.len();
        let mut i = 1;
        while i <= commands.len() {
            let cmd = commands.get(i);
            if let Some(c) = cmd {
                if c.vertex_count() == 0 && c.indices.is_empty() {
                    i += 1;
                    continue;
                }
            }
            let f = &commands[first];
            let split_here = self
                .markers
                .get(next_marker)
                .is_some_and(|m| m.sub_position <= i);
            let compatible = !split_here
                && cmd.is_some_and(|c| {
                    c.texture == f.texture
                        && c.blend_mode == f.blend_mode
                        && c.first_color() == f.first_color()
                        && c.first_dark_color() == f.first_dark_color()
                        && num_indices + c.indices.len() < 0xffff
                });
            if compatible {
                let c = cmd.unwrap();
                num_vertices += c.vertex_count();
                num_indices += c.indices.len();
            } else {
                self.groups.push(BatchGroup {
                    start,
                    end: i,
                    vertex_count: num_vertices,
                    index_count: num_indices,
                    blend_mode: f.blend_mode,
                    texture: f.texture,
                    color: f.first_color(),
                    dark_color: f.first_dark_color(),
                });
                while let Some(marker) = self
                    .markers
                    .get_mut(next_marker)
                    .filter(|m| m.sub_position <= i)
                {
                    marker.groups_before = self.groups.len();
                    next_marker += 1;
                }
                if i == commands.len() {
                    break;
                }
                first = i;
                start = i;
                num_vertices = commands[i].vertex_count();
                num_indices = commands[i].indices.len();
            }
            i += 1;
        }
    }

    /// Copies each group's sub-commands into one merged command.
    fn merge_groups(&mut self) {
        self.batched_count = 0;
        for group in &self.groups {
            let out = take_command(&mut self.batched, &mut self.batched_count);
            out.blend_mode = group.blend_mode;
            out.texture = group.texture;
            out.positions.reserve(group.vertex_count * 2);
            out.uvs.reserve(group.vertex_count * 2);
            out.colors.reserve(group.vertex_count);
            out.dark_colors.reserve(group.vertex_count);
            out.indices.reserve(group.index_count);
            let mut offset = 0u16;
            for c in &self.sub_commands[group.start..group.end] {
                out.positions.extend_from_slice(&c.positions);
                out.uvs.extend_from_slice(&c.uvs);
                out.colors.extend_from_slice(&c.colors);
                out.dark_colors.extend_from_slice(&c.dark_colors);
                out.indices
                    .extend(c.indices.iter().map(|&ix| ix.wrapping_add(offset)));
                offset = offset.wrapping_add(c.vertex_count() as u16);
            }
        }
    }
}

/// Returns a cleared recycled command, growing the pool if needed.
fn take_command<'a>(pool: &'a mut Vec<RenderCommand>, count: &mut usize) -> &'a mut RenderCommand {
    if *count == pool.len() {
        pool.push(RenderCommand::default());
    }
    let cmd = &mut pool[*count];
    cmd.clear();
    *count += 1;
    cmd
}
