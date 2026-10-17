//! `SpineAnimationState`: the tracks of a sprite and the animations queued on them.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::skeleton::SpineSkeleton;
use super::track_entry::SpineTrackEntry;
use crate::runtime::world;

/// The animation state of a sprite. Get it from `SpineSprite.get_animation_state()`.
///
/// The sprite advances and applies it by itself each frame unless its update mode is manual, so a
/// script only needs `update` and `apply` to drive it by hand.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineAnimationState {
    base: Base<RefCounted>,
    entity: Entity,
}

impl SpineAnimationState {
    /// A handle to the animation state of the skeleton `entity`.
    pub fn create(entity: Entity) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineAnimationState { base, entity })
    }
}

#[godot_api]
impl SpineAnimationState {
    /// False once the sprite this state came from was reloaded or freed. Every other call then
    /// does nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::state(self.entity, |_| ()).is_some()
    }

    /// Advances the tracks by `delta` seconds, now. The sprite does this itself each frame unless
    /// its update mode is manual.
    #[func]
    fn update(&mut self, delta: f32) {
        world::state_mut(self.entity, |state| state.update(delta));
    }

    /// Poses `skeleton` from the tracks, now. The skeleton has to be the one of this state's
    /// sprite.
    #[func]
    fn apply(&mut self, skeleton: Option<Gd<SpineSkeleton>>) {
        let Some(skeleton) = skeleton else { return };
        if skeleton.bind().entity() != self.entity {
            godot_warn!("Spine: the skeleton belongs to another sprite; not applied");
            return;
        }
        world::pose_mut(self.entity, |pose, state| state.apply(pose));
    }

    /// Removes all animations from all tracks.
    #[func]
    fn clear_tracks(&mut self) {
        world::state_mut(self.entity, |state| state.clear_tracks());
    }

    /// Removes the animations on `track_id`.
    #[func]
    fn clear_track(&mut self, track_id: i32) {
        world::state_mut(self.entity, |state| {
            state.clear_track(track_id.max(0) as usize)
        });
    }

    /// How many tracks there are; a track exists up to the highest one used.
    #[func]
    fn get_num_tracks(&self) -> i32 {
        world::state(self.entity, |state| state.tracks().len() as i32).unwrap_or_default()
    }

    /// Starts `animation_name` on `track_id`, replacing whatever plays there. Returns its entry,
    /// or null if the animation does not exist.
    #[func]
    fn set_animation(
        &mut self,
        animation_name: GString,
        #[opt(default = true)] looping: bool,
        #[opt(default = 0)] track_id: i32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entry = world::state_mut(self.entity, |state| {
            state.set_animation_by_name(
                track_id.max(0) as usize,
                &animation_name.to_string(),
                looping,
            )
        })?;
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(self.entity, entry)),
            Err(error) => {
                godot_warn!("Spine: {error}");
                None
            }
        }
    }

    /// Queues `animation_name` on `track_id` to start `delay` seconds after the animations
    /// already there end (if `delay` is not positive, relative to when the last one completes).
    /// Returns its entry, or null if the animation does not exist.
    #[func]
    fn add_animation(
        &mut self,
        animation_name: GString,
        #[opt(default = 0.0)] delay: f32,
        #[opt(default = true)] looping: bool,
        #[opt(default = 0)] track_id: i32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entry = world::state_mut(self.entity, |state| {
            state.add_animation_by_name(
                track_id.max(0) as usize,
                &animation_name.to_string(),
                looping,
                delay,
            )
        })?;
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(self.entity, entry)),
            Err(error) => {
                godot_warn!("Spine: {error}");
                None
            }
        }
    }

    /// Mixes `track_id` out to the setup pose over `mix_duration` seconds.
    #[func]
    fn set_empty_animation(
        &mut self,
        track_id: i32,
        mix_duration: f32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entry = world::state_mut(self.entity, |state| {
            state.set_empty_animation(track_id.max(0) as usize, mix_duration)
        })?;
        Some(SpineTrackEntry::create(self.entity, entry))
    }

    /// Queues a mix out to the setup pose on `track_id`.
    #[func]
    fn add_empty_animation(
        &mut self,
        track_id: i32,
        mix_duration: f32,
        delay: f32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entry = world::state_mut(self.entity, |state| {
            state.add_empty_animation(track_id.max(0) as usize, mix_duration, delay)
        })?;
        Some(SpineTrackEntry::create(self.entity, entry))
    }

    /// Mixes every track out to the setup pose.
    #[func]
    fn set_empty_animations(&mut self, mix_duration: f32) {
        world::state_mut(self.entity, |state| {
            state.set_empty_animations(mix_duration)
        });
    }

    /// The entry playing on `track_id`, or null.
    #[func]
    fn get_track(&self, track_id: i32) -> Option<Gd<SpineTrackEntry>> {
        let entry = world::state(self.entity, |state| state.track(track_id.max(0) as usize))??;
        Some(SpineTrackEntry::create(self.entity, entry))
    }

    #[func]
    fn get_time_scale(&self) -> f32 {
        world::state(self.entity, |state| state.time_scale).unwrap_or(1.0)
    }

    #[func]
    fn set_time_scale(&mut self, time_scale: f32) {
        world::state_mut(self.entity, |state| state.time_scale = time_scale);
    }

    /// Makes the events of the animations wait until `enable_queue` instead of being reported
    /// as they happen.
    #[func]
    fn disable_queue(&mut self) {
        world::state_mut(self.entity, |state| state.disable_queue());
    }

    #[func]
    fn enable_queue(&mut self) {
        world::state_mut(self.entity, |state| state.enable_queue());
    }
}
