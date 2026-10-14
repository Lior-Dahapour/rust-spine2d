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

use crate::attachment::Attachment;
use crate::bone::{is_y_down, Bone, BoneId, SkeletonTransform, WorldMatrix};
use crate::color::Color;
use crate::constraint::{self, Constraint, ConstraintBody};
use crate::enums::Physics;
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::{Slot, SlotApplied, SlotId};
use std::sync::Arc;

/// An entry of the flat, topologically sorted update list built by [`Skeleton::update_cache`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// Computes the world transform of the bone's applied pose.
    Bone(BoneId),
    /// Applies the constraint with this index.
    Constraint(usize),
}

/// Objects whose constrained pose must be reset to their pose every update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Posed {
    Bone(BoneId),
    Slot(SlotId),
    Constraint(usize),
}

/// The draw order of slots, with the same pose/constrained/applied scheme as bones and slots.
#[derive(Clone, Debug)]
pub struct DrawOrder {
    pub pose: Vec<SlotId>,
    constrained: Vec<SlotId>,
    applied_constrained: bool,
}

impl DrawOrder {
    fn new(slot_count: usize) -> DrawOrder {
        DrawOrder {
            pose: (0..slot_count).map(SlotId).collect(),
            constrained: Vec::new(),
            applied_constrained: false,
        }
    }

    pub fn setup_pose(&mut self) {
        let n = self.pose.len();
        self.pose.clear();
        self.pose.extend((0..n).map(SlotId));
    }

    /// The order to render in.
    pub fn applied_pose(&self) -> &[SlotId] {
        if self.applied_constrained {
            &self.constrained
        } else {
            &self.pose
        }
    }

    /// The pose vector timelines modify: the applied pose (which is the constrained copy while
    /// constraints modify the draw order) or the unconstrained pose.
    pub(crate) fn pose_vec(&mut self, applied_pose: bool) -> &mut Vec<SlotId> {
        if applied_pose && self.applied_constrained {
            &mut self.constrained
        } else {
            &mut self.pose
        }
    }

    pub(crate) fn set_constrained(&mut self) {
        self.applied_constrained = true;
    }

    fn reset(&mut self) {
        self.constrained.clear();
        self.constrained.extend_from_slice(&self.pose);
    }
}

/// A skeleton instance: the poses of one character, sharing a [`SkeletonData`].
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub(crate) data: Arc<SkeletonData>,
    pub(crate) bones: Vec<Bone>,
    pub(crate) slots: Vec<Slot>,
    pub(crate) draw_order: DrawOrder,
    pub(crate) constraints: Vec<Constraint>,
    pub(crate) update_cache: Vec<Update>,
    reset_cache: Vec<Posed>,
    pub(crate) skin: Option<Arc<Skin>>,
    pub color: Color,
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    scale_y: f32,
    pub wind_x: f32,
    pub wind_y: f32,
    pub gravity_x: f32,
    pub gravity_y: f32,
    pub time: f32,
    pub(crate) update: i32,
}

impl Skeleton {
    pub fn new(data: Arc<SkeletonData>) -> Skeleton {
        let mut bones: Vec<Bone> = Vec::with_capacity(data.bones.len());
        for bone_data in &data.bones {
            let parent = bone_data.parent.map(BoneId);
            bones.push(Bone::new(bone_data, parent));
            if let Some(p) = parent {
                bones[p.0].children.push(BoneId(bone_data.index));
            }
        }
        let slots: Vec<Slot> = data.slots.iter().map(Slot::new).collect();
        let draw_order = DrawOrder::new(slots.len());
        let constraints: Vec<Constraint> = data
            .constraints
            .iter()
            .enumerate()
            .map(|(i, c)| Constraint::new(i, c))
            .collect();

        let mut skeleton = Skeleton {
            data,
            bones,
            slots,
            draw_order,
            constraints,
            update_cache: Vec::new(),
            reset_cache: Vec::new(),
            skin: None,
            color: Color::WHITE,
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            wind_x: 1.0,
            wind_y: 0.0,
            gravity_x: 0.0,
            gravity_y: 1.0,
            time: 0.0,
            update: 0,
        };
        skeleton.setup_pose_slots();
        skeleton.update_cache();
        skeleton
    }

    pub fn data(&self) -> &Arc<SkeletonData> {
        &self.data
    }

    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    pub fn bones_mut(&mut self) -> &mut [Bone] {
        &mut self.bones
    }

    pub fn bone(&self, id: BoneId) -> &Bone {
        &self.bones[id.0]
    }

    pub fn bone_mut(&mut self, id: BoneId) -> &mut Bone {
        &mut self.bones[id.0]
    }

    pub fn root_bone(&self) -> Option<&Bone> {
        self.bones.first()
    }

    pub fn find_bone(&self, name: &str) -> Option<BoneId> {
        if name.is_empty() {
            return None;
        }
        self.data
            .bones
            .iter()
            .position(|b| b.name == name)
            .map(BoneId)
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut [Slot] {
        &mut self.slots
    }

    pub fn slot(&self, id: SlotId) -> &Slot {
        &self.slots[id.0]
    }

    pub fn slot_mut(&mut self, id: SlotId) -> &mut Slot {
        &mut self.slots[id.0]
    }

    pub fn find_slot(&self, name: &str) -> Option<SlotId> {
        if name.is_empty() {
            return None;
        }
        self.data
            .slots
            .iter()
            .position(|s| s.name == name)
            .map(SlotId)
    }

    pub fn draw_order(&self) -> &DrawOrder {
        &self.draw_order
    }

    pub fn draw_order_mut(&mut self) -> &mut DrawOrder {
        &mut self.draw_order
    }

    pub fn update_cache_entries(&self) -> &[Update] {
        &self.update_cache
    }

    /// Scale Y, negated when the world is y-down.
    pub fn scale_y(&self) -> f32 {
        self.scale_y * if is_y_down() { -1.0 } else { 1.0 }
    }

    pub fn set_scale_y(&mut self, scale_y: f32) {
        self.scale_y = scale_y;
    }

    pub fn set_scale(&mut self, scale_x: f32, scale_y: f32) {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub(crate) fn transform(&self) -> SkeletonTransform {
        SkeletonTransform {
            x: self.x,
            y: self.y,
            scale_x: self.scale_x,
            scale_y: self.scale_y(),
            update: self.update,
        }
    }

    /// Rebuilds the update and reset caches. Call after changing bone/constraint activity, for
    /// example when the skin changes.
    pub fn update_cache(&mut self) {
        self.update_cache.clear();
        self.reset_cache.clear();

        self.draw_order.applied_constrained = false;
        for slot in &mut self.slots {
            slot.applied = SlotApplied::Pose;
        }

        for (bone, data) in self.bones.iter_mut().zip(&self.data.bones) {
            bone.sorted = data.skin_required;
            bone.active = !bone.sorted;
            bone.unconstrained();
        }

        if let Some(skin) = self.skin.clone() {
            for &skin_bone in &skin.bones {
                let mut current = Some(BoneId(skin_bone));
                while let Some(id) = current {
                    let bone = &mut self.bones[id.0];
                    bone.sorted = false;
                    bone.active = true;
                    current = bone.parent;
                }
            }
        }

        let data = self.data.clone();
        for c in &mut self.constraints {
            c.set_unconstrained();
        }
        for i in 0..self.constraints.len() {
            let skin_has = self
                .skin
                .as_ref()
                .is_some_and(|s| s.constraints.contains(&i));
            let active = constraint::is_source_active(self, &self.constraints[i])
                && (!data.constraints[i].skin_required() || skin_has);
            self.constraints[i].active = active;
            if active {
                constraint::sort(self, &data, i);
            }
        }

        for i in 0..self.bones.len() {
            self.sort_bone(BoneId(i));
        }
    }

    /// Adds a bone (after its parent) to the update cache, unless it is already sorted or inactive.
    pub(crate) fn sort_bone(&mut self, id: BoneId) {
        let bone = &self.bones[id.0];
        if bone.sorted || !bone.active {
            return;
        }
        if let Some(parent) = bone.parent {
            self.sort_bone(parent);
        }
        self.bones[id.0].sorted = true;
        self.update_cache.push(Update::Bone(id));
    }

    /// Marks the children of a bone as unsorted again (recursively through sorted ones), so
    /// they are re-added to the update cache after a constraint modifying the bone.
    pub(crate) fn sort_reset_children(&mut self, id: BoneId) {
        for i in 0..self.bones[id.0].children.len() {
            let child = self.bones[id.0].children[i];
            if self.bones[child.0].active {
                if self.bones[child.0].sorted {
                    self.sort_reset_children(child);
                }
                self.bones[child.0].sorted = false;
            }
        }
    }

    /// Marks a bone as modified by a constraint: it will use its constrained pose, which is
    /// reset from the pose at the start of every update.
    pub fn constrained_bone(&mut self, id: BoneId) {
        let bone = &mut self.bones[id.0];
        if bone.is_pose_equal_to_applied() {
            bone.constrained();
            self.reset_cache.push(Posed::Bone(id));
        }
    }

    pub fn constrained_slot(&mut self, id: SlotId) {
        let slot = &mut self.slots[id.0];
        if slot.applied == SlotApplied::Pose {
            slot.constrained();
            self.reset_cache.push(Posed::Slot(id));
        }
    }

    /// Marks a constraint's pose as modified by another constraint (a slider's animation).
    pub fn constrained_constraint(&mut self, index: usize) {
        if self.constraints[index].is_pose_equal_to_applied() {
            self.constraints[index].set_constrained();
            self.reset_cache.push(Posed::Constraint(index));
        }
    }

    /// Computes the world transforms of all active bones and applies constraints.
    pub fn update_world_transform(&mut self, physics: Physics) {
        self.update = self.update.wrapping_add(1);

        if self.draw_order.applied_constrained {
            self.draw_order.reset();
        }
        for i in 0..self.reset_cache.len() {
            match self.reset_cache[i] {
                Posed::Bone(id) => self.bones[id.0].reset_constrained(),
                Posed::Slot(id) => self.slots[id.0].reset_constrained(),
                Posed::Constraint(i) => self.constraints[i].reset_constrained(),
            }
        }

        let data = self.data.clone();
        for i in 0..self.update_cache.len() {
            match self.update_cache[i] {
                Update::Bone(id) => self.update_bone(id, physics),
                Update::Constraint(c) => constraint::update(self, &data, c, physics),
            }
        }
    }

    fn update_bone(&mut self, id: BoneId, _physics: Physics) {
        let sk = self.transform();
        if self.bones[id.0].applied_pose().world != sk.update {
            self.update_bone_world_transform(id);
        }
    }

    /// Computes the world transform of the bone's applied pose from its parent's applied pose.
    /// Child bones are not updated.
    pub fn update_bone_world_transform(&mut self, id: BoneId) {
        let sk = self.transform();
        let parent = self.parent_world(id);
        self.bones[id.0]
            .applied_pose_mut()
            .update_world_transform(&sk, parent);
    }

    /// Computes the bone's local transform from its world transform.
    pub fn update_bone_local_transform(&mut self, id: BoneId) {
        let sk = self.transform();
        let parent = self.parent_world(id);
        self.bones[id.0]
            .applied_pose_mut()
            .update_local_transform(&sk, parent);
    }

    /// Calls `update_bone_local_transform` if the world transform was modified by a constraint
    /// and the local transform no longer matches. Call after `update_world_transform` before
    /// reading the applied local pose.
    pub fn validate_bone_local_transform(&mut self, id: BoneId) {
        if self.bones[id.0].applied_pose().local_stale == self.update {
            self.update_bone_local_transform(id);
        }
    }

    /// Call before modifying the applied local pose of a bone, so descendants recompute.
    pub fn modify_bone_local(&mut self, id: BoneId) {
        let update = self.update;
        if self.bones[id.0].applied_pose().local_stale == update {
            self.update_bone_local_transform(id);
        }
        self.bones[id.0].applied_pose_mut().world = 0;
        self.reset_world(id, update);
    }

    /// Call after modifying the applied world transform of a bone, so descendants recompute.
    pub fn modify_bone_world(&mut self, id: BoneId) {
        let update = self.update;
        let pose = self.bones[id.0].applied_pose_mut();
        pose.local_stale = update;
        pose.world = update;
        self.reset_world(id, update);
    }

    fn reset_world(&mut self, id: BoneId, update: i32) {
        for i in 0..self.bones[id.0].children.len() {
            let child = self.bones[id.0].children[i];
            let pose = self.bones[child.0].applied_pose();
            if pose.world == update {
                if pose.local_stale == update {
                    self.update_bone_local_transform(child);
                }
                self.bones[child.0].applied_pose_mut().world = 0;
                self.reset_world(child, update);
            }
        }
    }

    fn parent_world(&self, id: BoneId) -> Option<WorldMatrix> {
        self.bones[id.0]
            .parent
            .map(|p| self.bones[p.0].applied_pose().world_matrix())
    }

    /// Sets bones and constraints to their setup pose.
    pub fn setup_pose_bones(&mut self) {
        for (bone, data) in self.bones.iter_mut().zip(&self.data.bones) {
            bone.setup_pose(data);
        }
        for (c, data) in self.constraints.iter_mut().zip(&self.data.constraints) {
            c.setup_pose(data);
        }
    }

    pub fn setup_pose_slots(&mut self) {
        self.draw_order.setup_pose();
        let data = self.data.clone();
        for (i, slot_data) in data.slots.iter().enumerate() {
            let attachment = self.attachment_by_index(i, &slot_data.attachment_name);
            self.slots[i].setup_pose(slot_data, attachment);
        }
    }

    pub fn setup_pose(&mut self) {
        self.setup_pose_bones();
        self.setup_pose_slots();
    }

    /// The active skin, or `None` to use only the default skin.
    pub fn skin(&self) -> Option<&Arc<Skin>> {
        self.skin.as_ref()
    }

    /// Sets the active skin by name. Does nothing if the name is empty or no skin has it.
    /// Returns whether a skin was found.
    pub fn set_skin_by_name(&mut self, name: &str) -> bool {
        match self.data.find_skin(name).cloned() {
            Some(skin) => {
                self.set_skin(Some(skin));
                true
            }
            None => false,
        }
    }

    /// Sets the active skin and rebuilds the update cache.
    ///
    /// If there was an old skin, attachments from it that slots currently show are replaced by
    /// the same placeholder from the new skin, if it has one. Otherwise slots with a setup pose
    /// attachment name show the new skin's attachment for it.
    pub fn set_skin(&mut self, new_skin: Option<Arc<Skin>>) {
        let same = match (&self.skin, &new_skin) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        if let Some(new) = &new_skin {
            if let Some(old) = self.skin.clone() {
                new.attach_all(self, &old);
            } else {
                let data = self.data.clone();
                for (slot, slot_data) in self.slots.iter_mut().zip(&data.slots) {
                    if slot_data.attachment_name.is_empty() {
                        continue;
                    }
                    if let Some(attachment) =
                        new.attachment(slot_data.index, &slot_data.attachment_name)
                    {
                        slot.pose.set_attachment(Some(attachment.clone()));
                    }
                }
            }
        }
        self.skin = new_skin;
        self.update_cache();
    }

    /// Finds an attachment by slot name and placeholder, searching the active skin and then the
    /// default skin.
    pub fn attachment(&self, slot_name: &str, placeholder: &str) -> Option<Arc<Attachment>> {
        let slot = self.data.find_slot(slot_name)?;
        self.attachment_by_index(slot.index, placeholder)
    }

    /// Like [`attachment`](Self::attachment) but by slot index.
    pub fn attachment_by_index(
        &self,
        slot_index: usize,
        placeholder: &str,
    ) -> Option<Arc<Attachment>> {
        if placeholder.is_empty() {
            return None;
        }
        if let Some(attachment) = self
            .skin
            .as_ref()
            .and_then(|s| s.attachment(slot_index, placeholder))
        {
            return Some(attachment.clone());
        }
        self.data
            .default_skin
            .as_ref()?
            .attachment(slot_index, placeholder)
            .cloned()
    }

    /// Sets the slot's attachment by placeholder name; an empty placeholder clears it. Does
    /// nothing if the slot or attachment is not found. Returns whether the change was applied.
    pub fn set_attachment(&mut self, slot_name: &str, placeholder: &str) -> bool {
        let Some(slot_id) = self.find_slot(slot_name) else {
            return false;
        };
        let attachment = if placeholder.is_empty() {
            None
        } else {
            match self.attachment_by_index(slot_id.0, placeholder) {
                Some(a) => Some(a),
                None => return false,
            }
        };
        self.slots[slot_id.0].pose.set_attachment(attachment);
        true
    }

    /// Advances the skeleton's time, used by physics.
    pub fn update_time(&mut self, delta: f32) {
        self.time += delta;
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    pub fn constraint(&self, index: usize) -> &Constraint {
        &self.constraints[index]
    }

    pub fn constraint_mut(&mut self, index: usize) -> &mut Constraint {
        &mut self.constraints[index]
    }

    pub fn find_constraint(&self, name: &str) -> Option<usize> {
        self.data.find_constraint(name)
    }

    /// Moves the physics simulation as if the skeleton moved by `x`, `y`, so physics constraints
    /// don't react to the movement.
    pub fn physics_translate(&mut self, x: f32, y: f32) {
        for c in &mut self.constraints {
            if let ConstraintBody::Physics(p) = &mut c.body {
                p.translate(x, y);
            }
        }
    }

    /// Rotates the physics simulation around `x`, `y` as if the skeleton rotated by `degrees`.
    pub fn physics_rotate(&mut self, x: f32, y: f32, degrees: f32) {
        for c in &mut self.constraints {
            if let ConstraintBody::Physics(p) = &mut c.body {
                p.rotate(x, y, degrees);
            }
        }
    }
}
