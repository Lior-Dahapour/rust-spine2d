//! `SpineTrackEntry`: an animation playing, or queued to play, on a track.

use super::animation::SpineAnimation;
use crate::constants::SpineConstant;
use crate::runtime::world;
use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::animation_state::{EntryId, TrackEntry};
use spine2d_core::Interpolation;
use spine2d_ecs::bevy_ecs::entity::Entity;

/// An animation playing, or queued to play, on a track.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTrackEntry {
    base: Base<RefCounted>,
    entity: Entity,
    entry: EntryId,
}

impl SpineTrackEntry {
    pub fn create(entity: Entity, entry: EntryId) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTrackEntry {
            base,
            entity,
            entry,
        })
    }

    fn read<R>(&self, f: impl FnOnce(&TrackEntry) -> R) -> Option<R> {
        world::state(self.entity, |state| state.entry(self.entry).map(f)).flatten()
    }

    fn write(&self, f: impl FnOnce(&mut TrackEntry)) {
        world::state_mut(self.entity, |state| {
            if let Some(entry) = state.entry_mut(self.entry) {
                f(entry);
            }
        });
    }

    /// A handle to the entry `other` of the same sprite, if there is one.
    fn neighbor(&self, other: impl FnOnce(&TrackEntry) -> Option<EntryId>) -> Option<Gd<Self>> {
        let other = self.read(other)??;
        Some(Self::create(self.entity, other))
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

    /// The animation this entry plays, or null once the entry is gone.
    #[func]
    fn get_animation(&self) -> Option<Gd<SpineAnimation>> {
        let animation = self.read(|e| e.animation().clone())?;
        SpineAnimation::of(self.entity, &animation)
    }

    /// The entry queued before this one on the track, or null.
    #[func]
    fn get_previous(&self) -> Option<Gd<SpineTrackEntry>> {
        self.neighbor(|e| e.previous())
    }

    /// The entry queued after this one on the track, or null.
    #[func]
    fn get_next(&self) -> Option<Gd<SpineTrackEntry>> {
        self.neighbor(|e| e.next())
    }

    /// The entry this one is mixing from, or null.
    #[func]
    fn get_mixing_from(&self) -> Option<Gd<SpineTrackEntry>> {
        self.neighbor(|e| e.mixing_from())
    }

    /// The entry this one is mixing out to, or null.
    #[func]
    fn get_mixing_to(&self) -> Option<Gd<SpineTrackEntry>> {
        self.neighbor(|e| e.mixing_to())
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

    /// While mixing, attachments change only below this mix fraction (0 to 1).
    #[func]
    fn get_mix_attachment_threshold(&self) -> f32 {
        self.read(|e| e.mix_attachment_threshold).unwrap_or(0.0)
    }

    #[func]
    fn set_mix_attachment_threshold(&mut self, value: f32) {
        self.write(|e| e.mix_attachment_threshold = value);
    }

    /// While mixing, the draw order changes only above this mix fraction (0 to 1).
    #[func]
    fn get_mix_draw_order_threshold(&self) -> f32 {
        self.read(|e| e.mix_draw_order_threshold).unwrap_or(0.0)
    }

    #[func]
    fn set_mix_draw_order_threshold(&mut self, value: f32) {
        self.write(|e| e.mix_draw_order_threshold = value);
    }

    /// Attachment keys apply only above this alpha (0 to 1).
    #[func]
    fn get_alpha_attachment_threshold(&self) -> f32 {
        self.read(|e| e.alpha_attachment_threshold).unwrap_or(0.0)
    }

    #[func]
    fn set_alpha_attachment_threshold(&mut self, value: f32) {
        self.write(|e| e.alpha_attachment_threshold = value);
    }

    /// The animation time the entry was last applied at, in seconds.
    #[func]
    fn get_animation_last(&self) -> f32 {
        self.read(|e| e.animation_last()).unwrap_or(0.0)
    }

    #[func]
    fn set_animation_last(&mut self, value: f32) {
        self.write(|e| e.set_animation_last(value));
    }

    /// Seconds into the mix from the previous animation.
    #[func]
    fn get_mix_time(&self) -> f32 {
        self.read(|e| e.mix_time).unwrap_or(0.0)
    }

    #[func]
    fn set_mix_time(&mut self, value: f32) {
        self.write(|e| e.mix_time = value);
    }

    /// The track time at which the entry ends and is removed.
    #[func]
    fn get_track_end(&self) -> f32 {
        self.read(|e| e.track_end).unwrap_or(0.0)
    }

    #[func]
    fn set_track_end(&mut self, value: f32) {
        self.write(|e| e.track_end = value);
    }

    /// The track time at which the animation will next complete.
    #[func]
    fn get_track_complete(&self) -> f32 {
        self.read(|e| e.track_complete()).unwrap_or(0.0)
    }

    /// Whether the entry has been applied at least once.
    #[func]
    fn was_applied(&self) -> bool {
        self.read(|e| e.was_applied()).unwrap_or(false)
    }

    /// Forgets the rotation directions of the mix, so the next one takes the shortest way again.
    #[func]
    fn reset_rotation_directions(&mut self) {
        self.write(|e| e.reset_rotation_directions());
    }

    /// Sets the mix duration and the delay together, as `set_mix_duration` in the reference
    /// runtime does when it is given a delay: a delay of 0 or less is computed from the previous
    /// entry's completion.
    #[func]
    fn set_mix_duration_and_delay(&mut self, mix_duration: f32, delay: f32) {
        world::state_mut(self.entity, |state| {
            state.set_mix_duration(self.entry, mix_duration, delay)
        });
    }

    /// The curve of the mix: one of `SpineConstant.MixInterpolation_*`.
    #[func]
    fn get_mix_interpolation(&self) -> i32 {
        self.read(|e| match e.mix_interpolation {
            Interpolation::Smooth => SpineConstant::MixInterpolation_Smooth,
            Interpolation::SlowFast => SpineConstant::MixInterpolation_SlowFast,
            Interpolation::FastSlow => SpineConstant::MixInterpolation_FastSlow,
            Interpolation::Circle => SpineConstant::MixInterpolation_Circle,
            Interpolation::Linear | Interpolation::Custom(_) => {
                SpineConstant::MixInterpolation_Linear
            }
        } as i32)
            .unwrap_or_default()
    }

    #[func]
    fn set_mix_interpolation(&mut self, mix_interpolation: i32) {
        let interpolation = match i64::from(mix_interpolation) {
            SpineConstant::MixInterpolation_Linear => Interpolation::Linear,
            SpineConstant::MixInterpolation_Smooth => Interpolation::Smooth,
            SpineConstant::MixInterpolation_SlowFast => Interpolation::SlowFast,
            SpineConstant::MixInterpolation_FastSlow => Interpolation::FastSlow,
            SpineConstant::MixInterpolation_Circle => Interpolation::Circle,
            _ => {
                godot_warn!("Spine: {mix_interpolation} is not a mix interpolation (0 to 4)");
                return;
            }
        };
        self.write(|e| e.mix_interpolation = interpolation);
    }
}
