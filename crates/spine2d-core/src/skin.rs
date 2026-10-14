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
use crate::color::Color;
use crate::skeleton::Skeleton;
use crate::skeleton_data::SkeletonData;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::sync::Arc;

/// One attachment of a skin, keyed by slot and placeholder name.
#[derive(Clone, Debug)]
pub struct SkinEntry {
    pub slot_index: usize,
    /// The name slots and timelines use to refer to the attachment. May differ from the
    /// attachment's own name.
    pub placeholder: SmolStr,
    pub attachment: Arc<Attachment>,
}

/// A named set of attachments by slot, plus the bones and constraints it requires.
///
/// Entries are kept in buckets per slot index in insertion order, so iteration order matches the
/// reference runtime.
#[derive(Clone, Debug)]
pub struct Skin {
    name: SmolStr,
    buckets: Vec<SmallVec<[SkinEntry; 4]>>,
    /// Indices into `SkeletonData::bones` of bones that are active only with this skin.
    pub bones: Vec<usize>,
    /// Indices into the skeleton's constraints that are active only with this skin.
    pub constraints: Vec<usize>,
    /// Color used by the editor; has no effect at runtime.
    pub color: Color,
}

impl Skin {
    pub fn new(name: impl Into<SmolStr>) -> Skin {
        let name = name.into();
        debug_assert!(!name.is_empty(), "skin name must not be empty");
        Skin {
            name,
            buckets: Vec::new(),
            bones: Vec::new(),
            constraints: Vec::new(),
            color: Color::new(0.99607843, 0.61960787, 0.30980393, 1.0),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Adds an attachment, replacing any existing one for the same slot and placeholder.
    pub fn set_attachment(
        &mut self,
        slot_index: usize,
        placeholder: &str,
        attachment: Arc<Attachment>,
    ) {
        if slot_index >= self.buckets.len() {
            self.buckets.resize_with(slot_index + 1, SmallVec::new);
        }
        let bucket = &mut self.buckets[slot_index];
        match bucket.iter_mut().find(|e| e.placeholder == placeholder) {
            Some(entry) => entry.attachment = attachment,
            None => bucket.push(SkinEntry {
                slot_index,
                placeholder: placeholder.into(),
                attachment,
            }),
        }
    }

    pub fn attachment(&self, slot_index: usize, placeholder: &str) -> Option<&Arc<Attachment>> {
        self.buckets
            .get(slot_index)?
            .iter()
            .find(|e| e.placeholder == placeholder)
            .map(|e| &e.attachment)
    }

    pub(crate) fn attachment_mut(
        &mut self,
        slot_index: usize,
        placeholder: &str,
    ) -> Option<&mut Arc<Attachment>> {
        self.buckets
            .get_mut(slot_index)?
            .iter_mut()
            .find(|e| e.placeholder == placeholder)
            .map(|e| &mut e.attachment)
    }

    pub fn remove_attachment(&mut self, slot_index: usize, placeholder: &str) {
        if let Some(bucket) = self.buckets.get_mut(slot_index) {
            if let Some(i) = bucket.iter().position(|e| e.placeholder == placeholder) {
                bucket.remove(i);
            }
        }
    }

    /// All entries, ordered by slot index then insertion order.
    pub fn entries(&self) -> impl Iterator<Item = &SkinEntry> {
        self.buckets.iter().flatten()
    }

    pub fn entries_for_slot(&self, slot_index: usize) -> impl Iterator<Item = &SkinEntry> {
        self.buckets.get(slot_index).into_iter().flatten()
    }

    /// Adds all attachments, bones and constraints of `other`. Attachments are shared, not copied.
    pub fn add_skin(&mut self, other: &Skin) {
        self.merge_requirements(other);
        for entry in other.entries() {
            self.set_attachment(
                entry.slot_index,
                &entry.placeholder,
                entry.attachment.clone(),
            );
        }
    }

    /// Like [`add_skin`](Self::add_skin) but copies the attachments so they can be modified
    /// independently. Meshes become linked meshes.
    pub fn copy_skin(&mut self, other: &Skin) {
        self.merge_requirements(other);
        for entry in other.entries() {
            let copy = match &*entry.attachment {
                Attachment::Mesh(_) => Attachment::Mesh(
                    crate::attachment::MeshAttachment::new_linked_mesh(&entry.attachment)
                        .expect("is a mesh"),
                ),
                _ => Attachment::copy(&entry.attachment),
            };
            self.set_attachment(entry.slot_index, &entry.placeholder, Arc::new(copy));
        }
    }

    /// Whether every slot, bone and constraint this skin refers to exists in `data`, including the
    /// bones its weighted attachments use. A skin built for another skeleton fails this, and must
    /// not be set on a skeleton of `data`.
    pub fn fits(&self, data: &SkeletonData) -> bool {
        self.buckets.len() <= data.slots.len()
            && self
                .entries()
                .all(|entry| entry.attachment.bones_fit(data.bones.len()))
            && self.bones.iter().all(|&bone| bone < data.bones.len())
            && self
                .constraints
                .iter()
                .all(|&constraint| constraint < data.constraints.len())
    }

    fn merge_requirements(&mut self, other: &Skin) {
        for &bone in &other.bones {
            if !self.bones.contains(&bone) {
                self.bones.push(bone);
            }
        }
        for &constraint in &other.constraints {
            if !self.constraints.contains(&constraint) {
                self.constraints.push(constraint);
            }
        }
    }

    /// Attaches attachments from this skin to slots that show an attachment of `old_skin`.
    /// Used when switching skins.
    pub(crate) fn attach_all(&self, skeleton: &mut Skeleton, old_skin: &Skin) {
        for entry in old_skin.entries() {
            let slot = &skeleton.slots()[entry.slot_index];
            let shows_old = slot
                .pose
                .attachment
                .as_ref()
                .is_some_and(|a| Arc::ptr_eq(a, &entry.attachment));
            if shows_old {
                if let Some(attachment) = self
                    .attachment(entry.slot_index, &entry.placeholder)
                    .cloned()
                {
                    skeleton.slots_mut()[entry.slot_index]
                        .pose
                        .set_attachment(Some(attachment));
                }
            }
        }
    }
}
