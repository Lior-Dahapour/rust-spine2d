//! Script-facing wrapper objects. Each holds the sprite's shared player plus an index, so creating
//! one allocates no skeleton data, and a stale handle (a disposed track entry) simply stops working.

use crate::attachment::{merge_textures, SpineAttachment};
use crate::sprite::SharedPlayer;
use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::animation_state::{EntryId, TrackEntry};
use spine2d_core::{BoneId, SlotId};

/// An animation playing, or queued to play, on a track.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTrackEntry {
    base: Base<RefCounted>,
    player: SharedPlayer,
    entry: EntryId,
}

impl SpineTrackEntry {
    pub fn create(player: SharedPlayer, entry: EntryId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTrackEntry {
            base,
            player,
            entry,
        })
    }

    fn read<R>(&self, f: impl FnOnce(&TrackEntry) -> R) -> Option<R> {
        let player = self.player.borrow();
        if !player.is_alive() {
            return None;
        }
        player.state.entry(self.entry).map(f)
    }

    fn write(&self, f: impl FnOnce(&mut TrackEntry)) {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return;
        }
        if let Some(entry) = player.state.entry_mut(self.entry) {
            f(entry);
        }
    }
}

#[godot_api]
impl SpineTrackEntry {
    /// False once the entry has been disposed, or its sprite reloaded or freed; every other call
    /// then does nothing.
    #[func]
    fn is_valid(&self) -> bool {
        self.read(|_| ()).is_some()
    }

    #[func]
    fn get_animation_name(&self) -> GString {
        self.read(|e| GString::from(e.animation().name()))
            .unwrap_or_default()
    }

    #[func]
    fn get_track_index(&self) -> i32 {
        self.read(|e| e.track_index() as i32).unwrap_or(-1)
    }

    #[func]
    fn get_loop(&self) -> bool {
        self.read(|e| e.looping).unwrap_or(false)
    }

    #[func]
    fn set_loop(&mut self, value: bool) {
        self.write(|e| e.looping = value);
    }

    /// Adds to the pose instead of replacing it.
    #[func]
    fn get_additive(&self) -> bool {
        self.read(|e| e.additive).unwrap_or(false)
    }

    #[func]
    fn set_additive(&mut self, value: bool) {
        self.write(|e| e.additive = value);
    }

    /// Plays the animation backwards.
    #[func]
    fn get_reverse(&self) -> bool {
        self.read(|e| e.reverse).unwrap_or(false)
    }

    #[func]
    fn set_reverse(&mut self, value: bool) {
        self.write(|e| e.reverse = value);
    }

    #[func]
    fn get_time_scale(&self) -> f32 {
        self.read(|e| e.time_scale).unwrap_or(1.0)
    }

    #[func]
    fn set_time_scale(&mut self, value: f32) {
        self.write(|e| e.time_scale = value);
    }

    /// Blend of this entry with the entries on lower tracks, 0 to 1.
    #[func]
    fn get_alpha(&self) -> f32 {
        self.read(|e| e.alpha).unwrap_or(1.0)
    }

    #[func]
    fn set_alpha(&mut self, value: f32) {
        self.write(|e| e.alpha = value);
    }

    /// Seconds this entry has been running.
    #[func]
    fn get_track_time(&self) -> f32 {
        self.read(|e| e.track_time).unwrap_or(0.0)
    }

    #[func]
    fn set_track_time(&mut self, value: f32) {
        self.write(|e| e.track_time = value);
    }

    /// The animation time this entry is at, in seconds.
    #[func]
    fn get_animation_time(&self) -> f32 {
        self.read(|e| e.animation_time()).unwrap_or(0.0)
    }

    /// Start of the played time range, in animation seconds.
    #[func]
    fn get_animation_start(&self) -> f32 {
        self.read(|e| e.animation_start).unwrap_or(0.0)
    }

    #[func]
    fn set_animation_start(&mut self, value: f32) {
        self.write(|e| e.animation_start = value);
    }

    /// End of the played time range, in animation seconds.
    #[func]
    fn get_animation_end(&self) -> f32 {
        self.read(|e| e.animation_end).unwrap_or(0.0)
    }

    #[func]
    fn set_animation_end(&mut self, value: f32) {
        self.write(|e| e.animation_end = value);
    }

    /// Seconds to wait before a queued entry starts.
    #[func]
    fn get_delay(&self) -> f32 {
        self.read(|e| e.delay()).unwrap_or(0.0)
    }

    #[func]
    fn set_delay(&mut self, value: f32) {
        self.write(|e| e.set_delay(value));
    }

    /// Crossfade time from the previous animation on the track.
    #[func]
    fn get_mix_duration(&self) -> f32 {
        self.read(|e| e.mix_duration()).unwrap_or(0.0)
    }

    #[func]
    fn set_mix_duration(&mut self, value: f32) {
        self.write(|e| e.set_mix_duration(value));
    }

    /// True once the animation has completed a loop or reached its end.
    #[func]
    fn is_complete(&self) -> bool {
        self.read(|e| e.is_complete()).unwrap_or(false)
    }

    /// While mixing out, events fire only below this mix fraction (0 to 1).
    #[func]
    fn get_event_threshold(&self) -> f32 {
        self.read(|e| e.event_threshold).unwrap_or(0.0)
    }

    #[func]
    fn set_event_threshold(&mut self, value: f32) {
        self.write(|e| e.event_threshold = value);
    }

    /// Takes the shortest path for rotation mixes.
    #[func]
    fn get_shortest_rotation(&self) -> bool {
        self.read(|e| e.shortest_rotation).unwrap_or(false)
    }

    #[func]
    fn set_shortest_rotation(&mut self, value: bool) {
        self.write(|e| e.shortest_rotation = value);
    }
}

/// A bone of the skeleton. Local values are the pose animations write to; world values are the
/// result after world transforms were computed.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineBone {
    base: Base<RefCounted>,
    player: SharedPlayer,
    bone: BoneId,
}

impl SpineBone {
    pub fn create(player: SharedPlayer, bone: BoneId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineBone { base, player, bone })
    }

    fn local<R>(&self, f: impl FnOnce(&spine2d_core::BoneLocal) -> R) -> R {
        f(&self.player.borrow().skeleton.bone(self.bone).pose.local)
    }

    fn set_local(&self, f: impl FnOnce(&mut spine2d_core::BoneLocal)) {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return;
        }
        let skeleton = &mut player.skeleton;
        f(&mut skeleton.bone_mut(self.bone).pose.local);
    }

    fn world<R>(&self, f: impl FnOnce(&spine2d_core::BonePose) -> R) -> R {
        f(self.player.borrow().skeleton.bone(self.bone).applied_pose())
    }
}

#[godot_api]
impl SpineBone {
    /// False once the sprite this bone came from was reloaded or freed. Reads then return the last
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        self.player.borrow().is_alive()
    }

    #[func]
    fn get_name(&self) -> GString {
        let player = self.player.borrow();
        GString::from(
            player.skeleton.data().bones[self.bone.index()]
                .name
                .as_str(),
        )
    }

    #[func]
    fn get_length(&self) -> f32 {
        self.player.borrow().skeleton.data().bones[self.bone.index()].length
    }

    /// The parent bone, or null for the root.
    #[func]
    fn get_parent(&self) -> Option<Gd<SpineBone>> {
        let parent = self.player.borrow().skeleton.bone(self.bone).parent()?;
        Some(SpineBone::create(self.player.clone(), parent))
    }

    #[func]
    fn is_active(&self) -> bool {
        self.player.borrow().skeleton.bone(self.bone).is_active()
    }

    #[func]
    fn get_x(&self) -> f32 {
        self.local(|l| l.x)
    }

    #[func]
    fn set_x(&mut self, value: f32) {
        self.set_local(|l| l.x = value);
    }

    #[func]
    fn get_y(&self) -> f32 {
        self.local(|l| l.y)
    }

    #[func]
    fn set_y(&mut self, value: f32) {
        self.set_local(|l| l.y = value);
    }

    /// Local rotation in degrees.
    #[func]
    fn get_rotation(&self) -> f32 {
        self.local(|l| l.rotation)
    }

    #[func]
    fn set_rotation(&mut self, value: f32) {
        self.set_local(|l| l.rotation = value);
    }

    #[func]
    fn get_scale_x(&self) -> f32 {
        self.local(|l| l.scale_x)
    }

    #[func]
    fn set_scale_x(&mut self, value: f32) {
        self.set_local(|l| l.scale_x = value);
    }

    #[func]
    fn get_scale_y(&self) -> f32 {
        self.local(|l| l.scale_y)
    }

    #[func]
    fn set_scale_y(&mut self, value: f32) {
        self.set_local(|l| l.scale_y = value);
    }

    #[func]
    fn get_shear_x(&self) -> f32 {
        self.local(|l| l.shear_x)
    }

    #[func]
    fn set_shear_x(&mut self, value: f32) {
        self.set_local(|l| l.shear_x = value);
    }

    #[func]
    fn get_shear_y(&self) -> f32 {
        self.local(|l| l.shear_y)
    }

    #[func]
    fn set_shear_y(&mut self, value: f32) {
        self.set_local(|l| l.shear_y = value);
    }

    /// Sets the local position and rotation back to the setup pose.
    #[func]
    fn set_to_setup_pose(&mut self) {
        let setup = self.player.borrow().skeleton.data().bones[self.bone.index()].setup_pose;
        self.set_local(|l| *l = setup);
    }

    /// Position in the sprite's local space after the last world transform update.
    #[func]
    fn get_world_position(&self) -> Vector2 {
        self.world(|p| Vector2::new(p.world_x, p.world_y))
    }

    /// Rotation of the bone's x axis in the sprite's space, in degrees.
    #[func]
    fn get_world_rotation(&self) -> f32 {
        self.world(|p| p.world_rotation_x())
    }

    #[func]
    fn get_world_scale(&self) -> Vector2 {
        self.world(|p| Vector2::new(p.world_scale_x(), p.world_scale_y()))
    }

    /// Converts a point in the sprite's space to this bone's local space.
    #[func]
    fn world_to_local(&self, point: Vector2) -> Vector2 {
        self.world(|p| {
            let (x, y) = p.world_to_local(point.x, point.y);
            Vector2::new(x, y)
        })
    }

    /// Converts a point in this bone's local space to the sprite's space.
    #[func]
    fn local_to_world(&self, point: Vector2) -> Vector2 {
        self.world(|p| {
            let (x, y) = p.local_to_world(point.x, point.y);
            Vector2::new(x, y)
        })
    }

    /// Moves the bone so it ends up at `position` in the sprite's space, by changing its local
    /// position. Call between `before_world_transforms_change` and the world update.
    #[func]
    fn set_world_position(&mut self, position: Vector2) {
        let parent = self.player.borrow().skeleton.bone(self.bone).parent();
        let local = match parent {
            Some(parent) => {
                let player = self.player.borrow();
                let (x, y) = player
                    .skeleton
                    .bone(parent)
                    .applied_pose()
                    .world_to_local(position.x, position.y);
                Vector2::new(x, y)
            }
            None => position,
        };
        self.set_local(|l| {
            l.x = local.x;
            l.y = local.y;
        });
    }
}

/// A slot of the skeleton: holds the shown attachment and its tint.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSlot {
    base: Base<RefCounted>,
    player: SharedPlayer,
    slot: SlotId,
}

impl SpineSlot {
    pub fn create(player: SharedPlayer, slot: SlotId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSlot { base, player, slot })
    }
}

#[godot_api]
impl SpineSlot {
    /// False once the sprite this slot came from was reloaded or freed. Reads then return the last
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        self.player.borrow().is_alive()
    }

    #[func]
    fn get_name(&self) -> GString {
        let player = self.player.borrow();
        GString::from(
            player.skeleton.data().slots[self.slot.index()]
                .name
                .as_str(),
        )
    }

    /// The bone this slot is attached to.
    #[func]
    fn get_bone(&self) -> Gd<SpineBone> {
        let bone = self.player.borrow().skeleton.slot(self.slot).bone();
        SpineBone::create(self.player.clone(), bone)
    }

    /// Name of the shown attachment, or an empty string.
    #[func]
    fn get_attachment_name(&self) -> GString {
        let player = self.player.borrow();
        player
            .skeleton
            .slot(self.slot)
            .applied_pose()
            .attachment
            .as_ref()
            .map(|a| GString::from(a.name()))
            .unwrap_or_default()
    }

    /// Shows the attachment called `name` from the active skin (empty hides). False if unknown.
    #[func]
    fn set_attachment(&mut self, name: GString) -> bool {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return false;
        }
        let slot_name = player.skeleton.data().slots[self.slot.index()].name.clone();
        player
            .skeleton
            .set_attachment(&slot_name, &name.to_string())
    }

    /// Shows `attachment` in this slot, whatever skin is active. It is only this sprite's slot that
    /// changes. Returns false if the sprite is gone, or if the attachment uses bones the skeleton
    /// does not have.
    ///
    /// It stays until something replaces it: `set_attachment`, an animation that keys this slot's
    /// attachment, or `SpineSprite.set_slots_to_setup_pose()`. The sprite keeps the attachment's
    /// textures alive until the slots are reset.
    #[func]
    fn set_attachment_object(&mut self, attachment: Gd<SpineAttachment>) -> bool {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return false;
        }
        let attachment = attachment.bind();
        if !attachment
            .attachment()
            .bones_fit(player.skeleton.bones().len())
        {
            godot::global::godot_warn!(
                "Spine: attachment '{}' uses bones this skeleton does not have",
                attachment.attachment().name()
            );
            return false;
        }
        merge_textures(&mut player.kept_textures, attachment.textures());
        player.skeleton.slots_mut()[self.slot.index()]
            .pose
            .set_attachment(Some(attachment.attachment().clone()));
        true
    }

    /// The attachment this slot shows, or `null` if it shows none.
    #[func]
    fn get_attachment_object(&self) -> Option<Gd<SpineAttachment>> {
        let player = self.player.borrow();
        let shown = player
            .skeleton
            .slot(self.slot)
            .applied_pose()
            .attachment
            .clone()?;
        Some(SpineAttachment::wrap(shown, player.kept_textures.clone()))
    }

    #[func]
    fn get_color(&self) -> Color {
        let c = self
            .player
            .borrow()
            .skeleton
            .slot(self.slot)
            .applied_pose()
            .color;
        Color::from_rgba(c.r, c.g, c.b, c.a)
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return;
        }
        player
            .skeleton
            .slot_mut(self.slot)
            .applied_pose_mut()
            .color
            .set(color.r, color.g, color.b, color.a);
    }

    #[func]
    fn has_dark_color(&self) -> bool {
        self.player
            .borrow()
            .skeleton
            .slot(self.slot)
            .applied_pose()
            .has_dark_color
    }

    #[func]
    fn get_dark_color(&self) -> Color {
        let c = self
            .player
            .borrow()
            .skeleton
            .slot(self.slot)
            .applied_pose()
            .dark_color;
        Color::from_rgba(c.r, c.g, c.b, c.a)
    }

    #[func]
    fn set_dark_color(&mut self, color: Color) {
        let mut player = self.player.borrow_mut();
        if !player.is_alive() {
            return;
        }
        let pose = player.skeleton.slot_mut(self.slot).applied_pose_mut();
        pose.dark_color.set(color.r, color.g, color.b, color.a);
        pose.has_dark_color = true;
    }
}
