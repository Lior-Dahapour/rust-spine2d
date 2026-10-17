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

//! Views that pair a [`Skeleton`] with the [`SkeletonData`] it was made from.
//!
//! Most of what a skeleton does needs both its own state and its data: computing world
//! transforms, changing the skin, going back to the setup pose, looking things up by name. Those
//! operations live on the views, [`BorrowedSkeleton`] (exclusive) and [`BorrowedSkeletonRef`]
//! (shared), so the data is passed once, when the view is made, and not to every call.
//!
//! - [`Skeleton::borrow`] and [`Skeleton::borrow_ref`] make a view of a skeleton that holds its
//!   data. The exclusive one moves the data out of the skeleton and puts it back when it is
//!   dropped, so no reference count changes.
//! - `Skeleton::as_borrow` and `Skeleton::as_borrow_ref` make a view of a skeleton together with
//!   data the caller keeps. They need the `external-data` feature, and are for building a runtime
//!   or something similar; see [`Skeleton::detached`].

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::{Posed, Skeleton, Update, DETACHED};
use crate::attachment::Attachment;
use crate::bone::BoneId;
use crate::constraint;
use crate::enums::Physics;
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::{SlotApplied, SlotId};

mod sealed {
    pub trait Sealed {}
    impl Sealed for &crate::skeleton::Skeleton {}
    impl Sealed for &mut crate::skeleton::Skeleton {}
}

/// How a view reaches its skeleton: `&Skeleton` for a shared view, `&mut Skeleton` for an
/// exclusive one. Only those two implement it.
pub trait SkeletonAccess: Deref<Target = Skeleton> + sealed::Sealed {
    /// Gives back the data an exclusive view moved out of the skeleton.
    #[doc(hidden)]
    fn restore(&mut self, data: Arc<SkeletonData>);
}

impl SkeletonAccess for &Skeleton {
    fn restore(&mut self, _data: Arc<SkeletonData>) {
        // A shared view never moves the data out, so there is nothing to give back.
    }
}

impl SkeletonAccess for &mut Skeleton {
    fn restore(&mut self, data: Arc<SkeletonData>) {
        self.data = Some(data);
    }
}

/// The data of a view: moved out of the skeleton, or given by the caller.
enum Data<'a> {
    Held(Option<Arc<SkeletonData>>),
    Given(&'a SkeletonData),
}

impl Data<'_> {
    fn get(&self) -> &SkeletonData {
        match self {
            Data::Held(data) => data.as_ref().expect("the data is only taken when dropped"),
            Data::Given(data) => data,
        }
    }
}

/// A skeleton together with its [`SkeletonData`]. See the module documentation.
///
/// It dereferences to the [`Skeleton`], so everything that does not need the data is called on it
/// directly.
pub struct Borrowed<'a, S: SkeletonAccess> {
    skeleton: S,
    data: Data<'a>,
}

/// An exclusive view of a skeleton and its data: the one that changes the skeleton.
pub type BorrowedSkeleton<'a> = Borrowed<'a, &'a mut Skeleton>;

/// A shared view of a skeleton and its data: the one that only reads, for example to draw.
pub type BorrowedSkeletonRef<'a> = Borrowed<'a, &'a Skeleton>;

impl Skeleton {
    /// An exclusive view of this skeleton and the data it holds. The data is moved out of the
    /// skeleton while the view lives and put back when it is dropped. Panics for a
    /// detached skeleton, which holds no data.
    #[allow(clippy::should_implement_trait)]
    pub fn borrow(&mut self) -> BorrowedSkeleton<'_> {
        let data = self.data.take().expect(DETACHED);
        Borrowed {
            skeleton: self,
            data: Data::Held(Some(data)),
        }
    }

    /// A shared view of this skeleton and the data it holds. Panics for a detached
    /// skeleton, which holds no data.
    pub fn borrow_ref(&self) -> BorrowedSkeletonRef<'_> {
        Borrowed {
            skeleton: self,
            data: Data::Given(Self::data_of(&self.data)),
        }
    }

    /// An exclusive view of this skeleton and `data`, which the caller keeps. Needs the
    /// `external-data` feature.
    ///
    /// **This is for building a runtime or something similar**: see [`Skeleton::detached`].
    /// `data` must be the data the skeleton was made from.
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    pub fn as_borrow<'a>(&'a mut self, data: &'a SkeletonData) -> BorrowedSkeleton<'a> {
        Borrowed {
            skeleton: self,
            data: Data::Given(data),
        }
    }

    /// A shared view of this skeleton and `data`, which the caller keeps. Needs the
    /// `external-data` feature.
    ///
    /// **This is for building a runtime or something similar**: see [`Skeleton::detached`].
    /// `data` must be the data the skeleton was made from.
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    #[cfg_attr(not(feature = "external-data"), allow(dead_code))]
    pub fn as_borrow_ref<'a>(&'a self, data: &'a SkeletonData) -> BorrowedSkeletonRef<'a> {
        Borrowed {
            skeleton: self,
            data: Data::Given(data),
        }
    }

    /// Finds an attachment by slot index and placeholder, searching the active skin and then the
    /// default skin of `skeleton_data`. The views' `attachment_by_index` is this, with the data
    /// they hold.
    pub(crate) fn find_attachment(
        &self,
        skeleton_data: &SkeletonData,
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
        skeleton_data
            .default_skin
            .as_ref()?
            .attachment(slot_index, placeholder)
            .cloned()
    }
}

impl<S: SkeletonAccess> Deref for Borrowed<'_, S> {
    type Target = Skeleton;

    fn deref(&self) -> &Skeleton {
        &self.skeleton
    }
}

impl<S: SkeletonAccess + DerefMut> DerefMut for Borrowed<'_, S> {
    fn deref_mut(&mut self) -> &mut Skeleton {
        &mut self.skeleton
    }
}

impl<S: SkeletonAccess> Drop for Borrowed<'_, S> {
    fn drop(&mut self) {
        if let Data::Held(data) = &mut self.data {
            if let Some(data) = data.take() {
                self.skeleton.restore(data);
            }
        }
    }
}

/// What a view can do with the skeleton and its data without changing either.
impl<S: SkeletonAccess> Borrowed<'_, S> {
    /// The data this view pairs the skeleton with.
    pub fn data(&self) -> &SkeletonData {
        self.data.get()
    }

    /// Finds a bone by name.
    pub fn find_bone(&self, name: &str) -> Option<BoneId> {
        self.data().find_bone(name).map(|bone| BoneId(bone.index))
    }

    /// Finds a slot by name.
    pub fn find_slot(&self, name: &str) -> Option<SlotId> {
        self.data().find_slot(name).map(|slot| SlotId(slot.index))
    }

    /// Finds a constraint by name; the result indexes [`Skeleton::constraint`].
    pub fn find_constraint(&self, name: &str) -> Option<usize> {
        self.data().find_constraint(name)
    }

    /// Finds an attachment by slot name and placeholder, searching the active skin and then the
    /// default skin.
    pub fn attachment(&self, slot_name: &str, placeholder: &str) -> Option<Arc<Attachment>> {
        let slot = self.data().find_slot(slot_name)?;
        self.attachment_by_index(slot.index, placeholder)
    }

    /// Like [`attachment`](Self::attachment) but by slot index.
    pub fn attachment_by_index(
        &self,
        slot_index: usize,
        placeholder: &str,
    ) -> Option<Arc<Attachment>> {
        self.skeleton
            .find_attachment(self.data(), slot_index, placeholder)
    }
}

/// What an exclusive view can do: everything that changes the skeleton using its data.
impl<S: SkeletonAccess + DerefMut> Borrowed<'_, S> {
    /// The skeleton and the data as two separate borrows, for code that changes the one while it
    /// reads the other.
    pub(crate) fn parts(&mut self) -> (&mut Skeleton, &SkeletonData) {
        (&mut self.skeleton, self.data.get())
    }

    /// A shared view of what this view holds, for as long as it is not changed.
    pub fn as_ref(&self) -> BorrowedSkeletonRef<'_> {
        Borrowed {
            skeleton: &self.skeleton,
            data: Data::Given(self.data.get()),
        }
    }

    /// Rebuilds the update and reset caches. Call after changing bone/constraint activity, for
    /// example when the skin changes.
    pub fn update_cache(&mut self) {
        let (this, skeleton_data) = self.parts();
        this.update_cache.clear();
        this.reset_cache.clear();

        this.draw_order.applied_constrained = false;
        for slot in &mut this.slots {
            slot.applied = SlotApplied::Pose;
        }

        for (bone, data) in this.bones.iter_mut().zip(&skeleton_data.bones) {
            bone.sorted = data.skin_required;
            bone.active = !bone.sorted;
            bone.unconstrained();
        }

        if let Some(skin) = this.skin.clone() {
            for &skin_bone in &skin.bones {
                let mut current = Some(BoneId(skin_bone));
                while let Some(id) = current {
                    let bone = &mut this.bones[id.0];
                    bone.sorted = false;
                    bone.active = true;
                    current = bone.parent;
                }
            }
        }

        for c in &mut this.constraints {
            c.set_unconstrained();
        }
        for i in 0..this.constraints.len() {
            let skin_has = this
                .skin
                .as_ref()
                .is_some_and(|s| s.constraints.contains(&i));
            let active = constraint::is_source_active(this, &this.constraints[i])
                && (!skeleton_data.constraints[i].skin_required() || skin_has);
            this.constraints[i].active = active;
            if active {
                constraint::sort(this, skeleton_data, i);
            }
        }

        for i in 0..this.bones.len() {
            this.sort_bone(BoneId(i));
        }
    }

    /// Computes the world transforms of all active bones and applies constraints.
    pub fn update_world_transform(&mut self, physics: Physics) {
        let (this, skeleton_data) = self.parts();
        this.update = this.update.wrapping_add(1);

        if this.draw_order.applied_constrained {
            this.draw_order.reset();
        }
        for i in 0..this.reset_cache.len() {
            match this.reset_cache[i] {
                Posed::Bone(id) => this.bones[id.0].reset_constrained(),
                Posed::Slot(id) => this.slots[id.0].reset_constrained(),
                Posed::Constraint(i) => this.constraints[i].reset_constrained(),
            }
        }

        for i in 0..this.update_cache.len() {
            match this.update_cache[i] {
                Update::Bone(id) => this.update_bone(id, physics),
                Update::Constraint(c) => constraint::update(this, skeleton_data, c, physics),
            }
        }
    }

    /// Sets bones and constraints to their setup pose.
    pub fn setup_pose_bones(&mut self) {
        let (this, skeleton_data) = self.parts();
        for (bone, data) in this.bones.iter_mut().zip(&skeleton_data.bones) {
            bone.setup_pose(data);
        }
        for (c, data) in this.constraints.iter_mut().zip(&skeleton_data.constraints) {
            c.setup_pose(data);
        }
    }

    /// Sets slots and the draw order to their setup pose.
    pub fn setup_pose_slots(&mut self) {
        let (this, skeleton_data) = self.parts();
        this.draw_order.setup_pose();
        for (i, slot_data) in skeleton_data.slots.iter().enumerate() {
            let attachment = this.find_attachment(skeleton_data, i, &slot_data.attachment_name);
            this.slots[i].setup_pose(slot_data, attachment);
        }
    }

    /// Sets bones, constraints, slots and the draw order to their setup pose.
    pub fn setup_pose(&mut self) {
        self.setup_pose_bones();
        self.setup_pose_slots();
    }

    /// Sets the active skin by name. Does nothing if the name is empty or no skin has it.
    /// Returns whether a skin was found.
    pub fn set_skin_by_name(&mut self, name: &str) -> bool {
        match self.data().find_skin(name).cloned() {
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
        let (this, skeleton_data) = self.parts();
        let same = match (&this.skin, &new_skin) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        if let Some(new) = &new_skin {
            if let Some(old) = this.skin.clone() {
                new.attach_all(this, &old);
            } else {
                for (slot, slot_data) in this.slots.iter_mut().zip(&skeleton_data.slots) {
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
        this.skin = new_skin;
        self.update_cache();
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
}
