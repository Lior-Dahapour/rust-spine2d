//! `SpineSkin`: a set of attachments scripts can combine, for equipment and mix-and-match.

use crate::attachment::{merge_textures, SpineAttachment, SpineSkinEntry, Textures};
use godot::classes::{RefCounted, Texture2D};
use godot::global::godot_warn;
use godot::prelude::*;
use spine2d_core::Skin;
use std::sync::Arc;

/// A skin of a skeleton, or a new skin to fill with the attachments of others.
///
/// Get one from `SpineSkeletonDataResource.find_skin()` or make an empty one with
/// `SpineSkin.create()`, combine them with `add_skin()`, and show the result with
/// `SpineSprite.set_custom_skin()`. Changing a skin never changes the skeleton data it came from.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSkin {
    base: Base<RefCounted>,
    skin: Arc<Skin>,
    /// The textures of the attachments in this skin, kept alive while the skin exists.
    textures: Textures,
}

impl SpineSkin {
    /// Wraps a skin of the core runtime.
    pub fn wrap(skin: Arc<Skin>) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSkin {
            base,
            skin,
            textures: Textures::new(),
        })
    }

    /// The wrapped skin.
    pub fn skin(&self) -> &Arc<Skin> {
        &self.skin
    }

    /// The textures the skin's attachments draw.
    pub fn textures(&self) -> &[Gd<Texture2D>] {
        &self.textures
    }
}

#[godot_api]
impl SpineSkin {
    /// Makes an empty skin called `name`, or `null` if the name is empty.
    #[func]
    fn create(name: GString) -> Option<Gd<SpineSkin>> {
        if name.is_empty() {
            godot_warn!("Spine: a skin needs a name");
            return None;
        }
        Some(Self::wrap(Arc::new(Skin::new(name.to_string()))))
    }

    #[func]
    fn get_name(&self) -> GString {
        GString::from(self.skin.name())
    }

    /// Number of attachments in the skin.
    #[func]
    fn get_attachment_count(&self) -> i32 {
        self.skin.entries().count() as i32
    }

    /// Adds every attachment of `other` (and the bones and constraints it needs) to this skin,
    /// replacing attachments for the same slot and name. The attachments are shared with `other`.
    #[func]
    fn add_skin(&mut self, other: Gd<SpineSkin>) {
        let other = other.bind();
        Arc::make_mut(&mut self.skin).add_skin(&other.skin);
        merge_textures(&mut self.textures, &other.textures);
    }

    /// Like `add_skin`, but copies the attachments, so they can be changed without affecting
    /// `other`.
    #[func]
    fn copy_skin(&mut self, other: Gd<SpineSkin>) {
        let other = other.bind();
        Arc::make_mut(&mut self.skin).copy_skin(&other.skin);
        merge_textures(&mut self.textures, &other.textures);
    }

    /// Puts `attachment` into the skin for the slot with number `slot_index` (see
    /// `SpineSkeletonDataResource.get_slot_index`), under the name `placeholder`, replacing any
    /// attachment there. The attachment is shared, not copied.
    #[func]
    fn set_attachment(
        &mut self,
        slot_index: i32,
        placeholder: GString,
        attachment: Gd<SpineAttachment>,
    ) {
        let Some(slot_index) = valid_slot_index(slot_index) else {
            return;
        };
        if placeholder.is_empty() {
            godot_warn!("Spine: an attachment needs a name in the skin");
            return;
        }
        let attachment = attachment.bind();
        Arc::make_mut(&mut self.skin).set_attachment(
            slot_index,
            &placeholder.to_string(),
            attachment.attachment().clone(),
        );
        merge_textures(&mut self.textures, attachment.textures());
    }

    /// The attachment under `placeholder` for the slot, or `null`.
    #[func]
    fn get_attachment(&self, slot_index: i32, placeholder: GString) -> Option<Gd<SpineAttachment>> {
        let slot_index = valid_slot_index(slot_index)?;
        let attachment = self.skin.attachment(slot_index, &placeholder.to_string())?;
        Some(SpineAttachment::wrap(
            attachment.clone(),
            self.textures.clone(),
        ))
    }

    /// Removes the attachment under `placeholder` for the slot, if there is one.
    #[func]
    fn remove_attachment(&mut self, slot_index: i32, placeholder: GString) {
        let Some(slot_index) = valid_slot_index(slot_index) else {
            return;
        };
        Arc::make_mut(&mut self.skin).remove_attachment(slot_index, &placeholder.to_string());
    }

    /// The names of the attachments the skin has for the slot, in insertion order.
    #[func]
    fn find_names_for_slot(&self, slot_index: i32) -> PackedStringArray {
        let Some(slot_index) = valid_slot_index(slot_index) else {
            return PackedStringArray::new();
        };
        self.skin
            .entries_for_slot(slot_index)
            .map(|entry| GString::from(entry.placeholder.as_str()))
            .collect()
    }

    /// Every entry of the skin as a `SpineSkinEntry`: slot number, name and attachment.
    #[func]
    fn get_attachments(&self) -> Array<Gd<SpineSkinEntry>> {
        self.skin
            .entries()
            .map(|entry| {
                let attachment =
                    SpineAttachment::wrap(entry.attachment.clone(), self.textures.clone());
                SpineSkinEntry::create(entry.slot_index, &entry.placeholder, attachment)
            })
            .collect()
    }

    /// The numbers of the bones that are only active while this skin is: skin-specific bones such
    /// as the control bones of a skirt. See `SpineSkeletonDataResource.get_bone_index`.
    #[func]
    fn get_bones(&self) -> PackedInt32Array {
        self.skin.bones.iter().map(|&bone| bone as i32).collect()
    }

    /// The numbers of the constraints that are only active while this skin is. See
    /// `SpineSkeletonDataResource.get_constraint_index`.
    #[func]
    fn get_constraints(&self) -> PackedInt32Array {
        self.skin
            .constraints
            .iter()
            .map(|&constraint| constraint as i32)
            .collect()
    }

    /// Declares that the bone with number `bone_index` is only active while this skin is. Needed
    /// when attachments built from arrays are weighted to such bones; without it the bone stays
    /// inactive and the mesh collapses. Skins taken from the skeleton already have theirs.
    #[func]
    fn add_bone(&mut self, bone_index: i32) {
        let Ok(bone) = usize::try_from(bone_index) else {
            godot_warn!("Spine: bone index {bone_index} is negative");
            return;
        };
        let skin = Arc::make_mut(&mut self.skin);
        if !skin.bones.contains(&bone) {
            skin.bones.push(bone);
        }
    }

    /// Declares that the constraint with number `constraint_index` is only active while this skin
    /// is. See `add_bone`.
    #[func]
    fn add_constraint(&mut self, constraint_index: i32) {
        let Ok(constraint) = usize::try_from(constraint_index) else {
            godot_warn!("Spine: constraint index {constraint_index} is negative");
            return;
        };
        let skin = Arc::make_mut(&mut self.skin);
        if !skin.constraints.contains(&constraint) {
            skin.constraints.push(constraint);
        }
    }
}

/// A slot number from a script, or `None` (with a warning) if it is negative.
fn valid_slot_index(slot_index: i32) -> Option<usize> {
    let index = usize::try_from(slot_index).ok();
    if index.is_none() {
        godot_warn!("Spine: slot index {slot_index} is negative");
    }
    index
}
