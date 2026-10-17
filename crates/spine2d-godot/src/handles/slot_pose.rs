//! `SpineSlotPose`: the colors, shown attachment, sequence frame and deform of a slot, in one of
//! its poses.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{SlotId, SlotPose};

use super::PoseKind;
use crate::resources::attachment::SpineAttachment;
use crate::runtime::store::Owner;
use crate::runtime::world;

/// Reads the pose `kind` of `slot`. `None` once the sprite is gone.
pub fn read_pose<R>(
    owner: Owner,
    slot: SlotId,
    kind: PoseKind,
    f: impl FnOnce(&SlotPose) -> R,
) -> Option<R> {
    if kind == PoseKind::Setup {
        return world::owner_data(owner, |data| f(&data.slots[slot.index()].setup_pose));
    }
    world::skeleton(owner.entity()?, |skeleton| {
        if kind == PoseKind::Pose {
            f(&skeleton.slot(slot).pose)
        } else {
            f(skeleton.slot(slot).applied_pose())
        }
    })
}

/// Changes the pose `kind` of `slot`. The setup pose belongs to the shared skeleton data, so
/// changing it changes it for every sprite made from the data; they use it the next time they
/// apply their setup pose.
pub fn write_pose(owner: Owner, slot: SlotId, kind: PoseKind, f: impl FnOnce(&mut SlotPose)) {
    if kind == PoseKind::Setup {
        world::edit_data(owner, |data| f(&mut data.slots[slot.index()].setup_pose));
        return;
    }
    let Some(entity) = owner.entity() else {
        return;
    };
    world::skeleton_mut(entity, |skeleton| {
        let slot = skeleton.slot_mut(slot);
        f(if kind == PoseKind::Applied {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        });
    });
}

/// Shows `attachment` in the pose `kind` of `slot`, whatever skin is active. Returns false if the
/// sprite is gone, if the attachment uses bones the skeleton does not have, or if `kind` is the
/// setup pose, which names its attachment instead of holding one.
pub fn set_attachment_object(
    owner: Owner,
    slot: SlotId,
    kind: PoseKind,
    attachment: &Gd<SpineAttachment>,
) -> bool {
    let Some(entity) = owner.entity() else {
        return false;
    };
    if kind == PoseKind::Setup {
        godot_warn!(
            "Spine: the setup pose names its attachment; use SpineSlotData.set_attachment_name"
        );
        return false;
    }
    let attachment = attachment.bind();
    let Some(fits) = world::skeleton(entity, |skeleton| {
        attachment.attachment().bones_fit(skeleton.bones().len())
    }) else {
        return false;
    };
    if !fits {
        godot_warn!(
            "Spine: attachment '{}' uses bones this skeleton does not have",
            attachment.attachment().name()
        );
        return false;
    }
    if let Some(mut sprite) = world::sprite(entity) {
        sprite.bind_mut().keep_textures(attachment.textures());
    }
    write_pose(owner, slot, kind, |pose| {
        pose.set_attachment(Some(attachment.attachment().clone()));
    });
    true
}

/// One pose of a slot. Get it from `SpineSlot.get_pose()` (what animations and scripts set),
/// `SpineSlot.get_applied_pose()` (after constraints) or `SpineSlotData.get_setup_pose()`.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSlotPose {
    base: Base<RefCounted>,
    owner: Owner,
    slot: SlotId,
    kind: PoseKind,
}

impl SpineSlotPose {
    /// A handle to the pose `kind` of `slot` of `owner`. Only the setup pose exists without a
    /// skeleton.
    pub fn create(owner: impl Into<Owner>, slot: SlotId, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSlotPose {
            base,
            owner: owner.into(),
            slot,
            kind,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&SlotPose) -> R) -> R {
        read_pose(self.owner, self.slot, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut SlotPose)) {
        write_pose(self.owner, self.slot, self.kind, f);
    }
}

#[godot_api]
impl SpineSlotPose {
    /// False once the sprite this pose came from was reloaded or freed. Reads then return zero
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::owner_data(self.owner, |_| ()).is_some()
    }

    #[func]
    fn get_color(&self) -> Color {
        self.read(|pose| Color::from_rgba(pose.color.r, pose.color.g, pose.color.b, pose.color.a))
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        self.write(|pose| {
            pose.color.set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn get_dark_color(&self) -> Color {
        self.read(|pose| {
            let c = pose.dark_color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
    }

    /// Sets the dark color of the two color tint. Call `set_has_dark_color` too, or this does not
    /// show.
    #[func]
    fn set_dark_color(&mut self, color: Color) {
        self.write(|pose| {
            pose.dark_color.set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn has_dark_color(&self) -> bool {
        self.read(|pose| pose.has_dark_color)
    }

    #[func]
    fn set_has_dark_color(&mut self, has_dark_color: bool) {
        self.write(|pose| pose.has_dark_color = has_dark_color);
    }

    /// The attachment this pose shows, or `null`. The setup pose names its attachment instead, so
    /// it always returns `null`.
    #[func]
    fn get_attachment(&self) -> Option<Gd<SpineAttachment>> {
        let shown = self.read(|pose| pose.attachment.clone())?;
        let textures = world::sprite(self.owner.entity()?)?.bind().kept_textures();
        Some(SpineAttachment::wrap(shown, textures))
    }

    /// Shows `attachment`, whatever skin is active. See `SpineSlot.set_attachment_object`.
    #[func]
    fn set_attachment(&mut self, attachment: Option<Gd<SpineAttachment>>) {
        match attachment {
            Some(attachment) => {
                set_attachment_object(self.owner, self.slot, self.kind, &attachment);
            }
            None => self.write(|pose| pose.set_attachment(None)),
        }
    }

    /// The frame of the attachment's sequence, or -1 to use the sequence's setup frame.
    #[func]
    fn get_sequence_index(&self) -> i32 {
        self.read(|pose| pose.sequence_index)
    }

    #[func]
    fn set_sequence_index(&mut self, sequence_index: i32) {
        self.write(|pose| pose.sequence_index = sequence_index);
    }

    /// The vertex offsets applied to the attachment's vertices; empty when there is no deform.
    #[func]
    fn get_deform(&self) -> PackedFloat32Array {
        self.read(|pose| PackedFloat32Array::from(pose.deform.as_slice()))
    }

    #[func]
    fn set_deform(&mut self, deform: PackedFloat32Array) {
        self.write(|pose| pose.deform = deform.to_vec());
    }
}
