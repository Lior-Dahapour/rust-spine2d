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

use super::{AnimationState, EntryId, EventType, Listener};
use crate::animation::{Animation, Interpolation};
use crate::event::Event;
use crate::math::fmod;
use std::sync::Arc;

/// State for the playback of one animation on a track: its time, mixing and blending settings.
///
/// Entries are owned by an [`AnimationState`] and addressed by [`EntryId`]. Fields that are safe
/// to change at any time are public; the rest are managed by the state.
pub struct TrackEntry {
    pub(crate) animation: Arc<Animation>,
    pub(crate) previous: Option<EntryId>,
    pub(crate) next: Option<EntryId>,
    pub(crate) mixing_from: Option<EntryId>,
    pub(crate) mixing_to: Option<EntryId>,
    pub(crate) track_index: usize,
    /// If true, the animation repeats when it reaches the end.
    pub looping: bool,
    /// If true, timelines that support it add to the pose instead of replacing it.
    pub additive: bool,
    /// If true, the animation plays backward.
    pub reverse: bool,
    /// If true, rotation mixes take the shortest path instead of preserving direction. Implied
    /// by `additive`.
    pub shortest_rotation: bool,
    pub(crate) mixed_out: bool,
    /// While mixing out, events are fired only when the mix is below this fraction (0..1).
    pub event_threshold: f32,
    /// While mixing out, attachment timelines are applied while the mix is below this fraction.
    pub mix_attachment_threshold: f32,
    /// Attachment timelines are applied while alpha is at least this.
    pub alpha_attachment_threshold: f32,
    /// While mixing out, draw order timelines are applied while the mix is below this fraction.
    pub mix_draw_order_threshold: f32,
    /// The animation time range to play, in seconds.
    pub animation_start: f32,
    pub animation_end: f32,
    pub(crate) animation_last: f32,
    pub(crate) next_animation_last: f32,
    pub(crate) delay: f32,
    /// Seconds this entry has been running (scaled by `time_scale`).
    pub track_time: f32,
    pub(crate) track_last: f32,
    pub(crate) next_track_last: f32,
    /// The track time at which this entry's track is cleared if nothing follows it.
    pub track_end: f32,
    pub time_scale: f32,
    /// Blends this entry with the entries below it on lower tracks (0..1).
    pub alpha: f32,
    pub mix_time: f32,
    pub(crate) mix_duration: f32,
    pub(crate) total_alpha: f32,
    pub mix_interpolation: Interpolation,
    pub(crate) timeline_mode: Vec<u8>,
    pub(crate) timeline_hold_mix: Vec<Option<EntryId>>,
    pub(crate) timelines_rotation: Vec<f32>,
    pub(crate) listener: Option<Listener>,
}

impl TrackEntry {
    pub fn animation(&self) -> &Arc<Animation> {
        &self.animation
    }

    pub fn track_index(&self) -> usize {
        self.track_index
    }

    /// The entry on the same track that played before this one, if this one is queued.
    pub fn previous(&self) -> Option<EntryId> {
        self.previous
    }

    /// The entry queued to play after this one.
    pub fn next(&self) -> Option<EntryId> {
        self.next
    }

    /// The entry that is being mixed out while this one mixes in.
    pub fn mixing_from(&self) -> Option<EntryId> {
        self.mixing_from
    }

    /// The entry that this one is being mixed out to.
    pub fn mixing_to(&self) -> Option<EntryId> {
        self.mixing_to
    }

    /// Seconds to wait before this entry starts applying. Negative values are ignored.
    pub fn delay(&self) -> f32 {
        self.delay
    }

    pub fn set_delay(&mut self, delay: f32) {
        if delay >= 0.0 {
            self.delay = delay;
        }
    }

    pub fn animation_last(&self) -> f32 {
        self.animation_last
    }

    /// Sets the last animation time; events fire for keys after this time.
    pub fn set_animation_last(&mut self, animation_last: f32) {
        self.animation_last = animation_last;
        self.next_animation_last = animation_last;
    }

    /// The animation time for the current track time, honoring looping and start/end.
    pub fn animation_time(&self) -> f32 {
        if !self.looping {
            return (self.track_time + self.animation_start).min(self.animation_end);
        }
        let duration = self.animation_end - self.animation_start;
        if duration == 0.0 {
            return self.animation_start;
        }
        fmod(self.track_time, duration) + self.animation_start
    }

    /// True if the animation has played to its end (or completed a loop).
    pub fn is_complete(&self) -> bool {
        self.track_time >= self.animation_end - self.animation_start
    }

    pub fn mix_duration(&self) -> f32 {
        self.mix_duration
    }

    pub fn set_mix_duration(&mut self, mix_duration: f32) {
        self.mix_duration = mix_duration;
    }

    /// The track time at which the animation will next complete (a loop iteration or the end),
    /// or the next update if already complete.
    pub fn track_complete(&self) -> f32 {
        let duration = self.animation_end - self.animation_start;
        if duration != 0.0 {
            if self.looping {
                return duration * (1.0 + (self.track_time / duration) as i32 as f32);
            }
            if self.track_time < duration {
                return duration;
            }
        }
        self.track_time
    }

    /// True if this entry has been applied at least once.
    pub fn was_applied(&self) -> bool {
        self.next_track_last != -1.0
    }

    pub fn is_empty_animation(&self) -> bool {
        Arc::ptr_eq(&self.animation, &super::empty_animation())
    }

    /// Clears the stored rotation directions so the next mix picks the shortest path again.
    pub fn reset_rotation_directions(&mut self) {
        self.timelines_rotation.clear();
    }

    /// Sets a listener called for this entry's events, replacing any existing one.
    pub fn set_listener(
        &mut self,
        listener: impl FnMut(&mut AnimationState, EventType, EntryId, Option<&Event>)
            + Send
            + Sync
            + 'static,
    ) {
        self.listener = Some(Box::new(listener));
    }

    pub fn clear_listener(&mut self) {
        self.listener = None;
    }

    /// The mix progress (0..1) after applying `mix_interpolation`.
    pub(crate) fn mix(&self) -> f32 {
        if self.mix_duration == 0.0 {
            return 1.0;
        }
        let mix = self.mix_time / self.mix_duration;
        if mix >= 1.0 {
            return 1.0;
        }
        if self.mix_interpolation.is_linear() {
            return mix;
        }
        self.mix_interpolation.apply(mix).clamp(0.0, 1.0)
    }
}

impl std::fmt::Debug for TrackEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackEntry")
            .field("animation", &self.animation.name())
            .field("track_index", &self.track_index)
            .field("track_time", &self.track_time)
            .field("looping", &self.looping)
            .finish_non_exhaustive()
    }
}
