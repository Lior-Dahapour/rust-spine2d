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

//! Lets skeletons that are small on screen update less often.
//!
//! Add [`SpineLod`] to a skeleton to opt in. An engine integration measures the skeleton's height
//! on screen and calls [`SpineLod::fit_height`]; the smaller it is, the fewer frames it is
//! animated on. Time is accumulated across the skipped frames, so animations keep their speed.
//! The skeletons that update on a given frame are spread out by entity, so the work stays even.
//! Skeletons without a [`SpineLod`] update every frame.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

/// Opts a skeleton in to updating less often when it is small on screen, and tracks how it is
/// doing so: the current interval, the time not yet used, and whether this frame updates it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpineLod {
    full_rate_height: f32,
    max_interval: u32,
    interval: u32,
    accumulated: f32,
    due: bool,
}

impl Default for SpineLod {
    fn default() -> Self {
        SpineLod::new(200.0, 4)
    }
}

impl SpineLod {
    /// A skeleton that updates every frame from `full_rate_height` pixels tall on screen, and at
    /// most `max_interval` frames apart however small it gets.
    pub fn new(full_rate_height: f32, max_interval: u32) -> Self {
        SpineLod {
            full_rate_height,
            max_interval: max_interval.max(1),
            interval: 1,
            accumulated: 0.0,
            due: true,
        }
    }

    /// The on-screen height, in pixels, at or above which the skeleton updates every frame.
    pub fn full_rate_height(&self) -> f32 {
        self.full_rate_height
    }

    /// The longest gap allowed between updates, in frames.
    pub fn max_interval(&self) -> u32 {
        self.max_interval
    }

    /// How many frames pass between updates right now.
    pub fn interval(&self) -> u32 {
        self.interval
    }

    /// Whether the current frame updates the skeleton. Meaningful after the update stage.
    pub fn is_due(&self) -> bool {
        self.due
    }

    /// Sets how many frames pass between updates, within `1..=max_interval`.
    pub fn set_interval(&mut self, interval: u32) {
        self.interval = interval.clamp(1, self.max_interval);
    }

    /// Sets the interval for a skeleton that is `height` pixels tall on screen.
    pub fn fit_height(&mut self, height: f32) {
        let interval = if height <= 0.0 {
            self.max_interval
        } else {
            (self.full_rate_height / height).round() as u32
        };
        self.set_interval(interval);
    }

    /// Adds a frame's time. Returns the time to advance by if the skeleton updates on `frame`,
    /// and `None` if it skips it.
    pub(crate) fn tick(&mut self, delta: f32, frame: u32, entity: Entity) -> Option<f32> {
        self.accumulated += delta;
        self.due = frame
            .wrapping_add(entity.index_u32())
            .is_multiple_of(self.interval);
        self.due.then(|| std::mem::take(&mut self.accumulated))
    }

    /// The time that has been added but not yet used by an update.
    #[cfg(test)]
    pub(crate) fn pending(&self) -> f32 {
        self.accumulated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("a valid index")
    }

    fn interval_at(height: f32) -> u32 {
        let mut lod = SpineLod::default();
        lod.fit_height(height);
        lod.interval()
    }

    #[test]
    fn large_skeletons_update_every_frame() {
        assert_eq!(interval_at(400.0), 1);
        assert_eq!(interval_at(200.0), 1);
    }

    #[test]
    fn smaller_skeletons_update_less_often_up_to_the_limit() {
        assert_eq!(interval_at(100.0), 2);
        assert_eq!(interval_at(10.0), 4);
        assert_eq!(interval_at(0.0), 4);
    }

    #[test]
    fn an_interval_is_kept_within_one_and_the_maximum() {
        let mut lod = SpineLod::new(200.0, 3);
        lod.set_interval(0);
        assert_eq!(lod.interval(), 1);
        lod.set_interval(10);
        assert_eq!(lod.interval(), 3);
    }

    #[test]
    fn skipped_frames_keep_their_time_for_the_next_update() {
        let mut lod = SpineLod::default();
        lod.set_interval(3);
        let ticks: Vec<_> = (0..6)
            .map(|frame| lod.tick(0.1, frame, entity(0)))
            .collect();
        assert_eq!(ticks[0], Some(0.1));
        assert_eq!(ticks[1], None);
        assert_eq!(ticks[2], None);
        assert!((ticks[3].expect("due") - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_tick_records_whether_the_skeleton_is_due() {
        let mut lod = SpineLod::default();
        lod.set_interval(2);
        lod.tick(0.1, 0, entity(0));
        assert!(lod.is_due());
        lod.tick(0.1, 1, entity(0));
        assert!(!lod.is_due());
    }

    #[test]
    fn entities_update_on_different_frames() {
        let mut first = SpineLod::default();
        let mut second = SpineLod::default();
        first.set_interval(2);
        second.set_interval(2);
        assert!(first.tick(0.1, 0, entity(0)).is_some());
        assert!(second.tick(0.1, 0, entity(1)).is_none());
    }
}
