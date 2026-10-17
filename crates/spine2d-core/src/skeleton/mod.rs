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
use crate::constraint::{Constraint, ConstraintBody};
use crate::enums::Physics;
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::{Slot, SlotApplied, SlotId};
use std::sync::Arc;

mod borrowed;

pub use borrowed::{Borrowed, BorrowedSkeleton, BorrowedSkeletonRef, SkeletonAccess};

/// An entry of the flat, topologically sorted update list built by [`update_cache`](crate::skeleton::Borrowed::update_cache).
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

/// What a method panics with when it needs the data a skeleton does not hold.
const DETACHED: &str = "this skeleton holds no data; use `as_borrow` (see `Skeleton::detached`)";

/// A skeleton instance: the poses of one character, sharing a [`SkeletonData`].
#[derive(Clone, Debug)]
pub struct Skeleton {
    /// The data this skeleton shares. `None` for a [`Skeleton::detached`] skeleton, and while
    /// an exclusive view of it lives, which has moved the data out.
    pub(crate) data: Option<Arc<SkeletonData>>,
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
    /// A skeleton that holds a share of `data`. This is the one to use, unless you are building a
    /// runtime: see `Skeleton::detached`, which needs the `external-data` feature.
    pub fn new(data: Arc<SkeletonData>) -> Skeleton {
        let mut skeleton = Self::detached(&data);
        skeleton.data = Some(data);
        skeleton
    }

    /// A skeleton that holds no [`SkeletonData`]: whoever calls it keeps the data and passes it to
    /// the `_with` methods (`update_world_transform_with`, `set_skin_with`, and so on).
    ///
    /// Needs the `external-data` feature.
    ///
    /// **This is for building a runtime or something similar**, where the data lives in one place
    /// (a store, an ECS resource) and many skeletons use it without each holding a share of an
    /// `Arc`. Anyone else should use [`Skeleton::new`], which holds its data and needs none of
    /// this. The methods without `_with`, and [`Skeleton::data`], panic on a detached skeleton
    /// because they have no data to read, so always pass the same data the skeleton was made from.
    /// To find a bone, slot, skin or constraint by name, ask the data: [`SkeletonData::find_bone`],
    /// [`SkeletonData::find_slot`], [`SkeletonData::find_skin`] and [`SkeletonData::find_constraint`].
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    pub fn detached(data: &SkeletonData) -> Skeleton {
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
            data: None,
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
        let mut view = skeleton.as_borrow(data);
        view.setup_pose_slots();
        view.update_cache();
        drop(view);
        skeleton
    }

    pub fn data(&self) -> &Arc<SkeletonData> {
        Self::data_of(&self.data)
    }

    /// The data held in `field`. Taking the field, not the skeleton, lets a caller keep borrowing
    /// the skeleton's other fields while it reads the data.
    pub(crate) fn data_of(field: &Option<Arc<SkeletonData>>) -> &Arc<SkeletonData> {
        field.as_ref().expect(DETACHED)
    }

    /// Finds a bone by name.
    pub fn find_bone(&self, name: &str) -> Option<BoneId> {
        self.borrow_ref().find_bone(name)
    }

    /// Finds a slot by name.
    pub fn find_slot(&self, name: &str) -> Option<SlotId> {
        self.borrow_ref().find_slot(name)
    }

    /// Finds a constraint by name and returns its index.
    pub fn find_constraint(&self, name: &str) -> Option<usize> {
        self.borrow_ref().find_constraint(name)
    }

    /// Rebuilds the update and reset caches. Call after changing bone/constraint activity, for
    /// example when the skin changes.
    pub fn update_cache(&mut self) {
        self.borrow().update_cache();
    }

    /// Computes the world transforms of all active bones and applies constraints.
    pub fn update_world_transform(&mut self, physics: Physics) {
        self.borrow().update_world_transform(physics);
    }

    /// Sets bones and constraints to their setup pose.
    pub fn setup_pose_bones(&mut self) {
        self.borrow().setup_pose_bones();
    }

    /// Sets slots and the draw order to their setup pose.
    pub fn setup_pose_slots(&mut self) {
        self.borrow().setup_pose_slots();
    }

    /// Sets bones, constraints, slots and the draw order to their setup pose.
    pub fn setup_pose(&mut self) {
        self.borrow().setup_pose();
    }

    /// Sets the active skin by name. Does nothing if the name is empty or no skin has it.
    pub fn set_skin_by_name(&mut self, name: &str) -> bool {
        self.borrow().set_skin_by_name(name)
    }

    /// Sets the active skin, or `None` to use only the default skin.
    pub fn set_skin(&mut self, new_skin: Option<Arc<Skin>>) {
        self.borrow().set_skin(new_skin);
    }

    /// Finds an attachment by slot name and placeholder, searching the active skin and then the
    /// default skin.
    pub fn attachment(&self, slot_name: &str, placeholder: &str) -> Option<Arc<Attachment>> {
        self.borrow_ref().attachment(slot_name, placeholder)
    }

    /// Like [`attachment`](Self::attachment) but by slot index.
    pub fn attachment_by_index(
        &self,
        slot_index: usize,
        placeholder: &str,
    ) -> Option<Arc<Attachment>> {
        self.borrow_ref()
            .attachment_by_index(slot_index, placeholder)
    }

    /// Sets the slot's attachment by placeholder name; an empty placeholder clears it. Does
    /// nothing if the slot or attachment is not found. Returns whether the change was applied.
    pub fn set_attachment(&mut self, slot_name: &str, placeholder: &str) -> bool {
        self.borrow().set_attachment(slot_name, placeholder)
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

    /// The active skin, or `None` to use only the default skin.
    pub fn skin(&self) -> Option<&Arc<Skin>> {
        self.skin.as_ref()
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
