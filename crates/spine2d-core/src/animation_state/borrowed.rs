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

//! A view that pairs an [`AnimationState`] with the mix settings and skeleton data it needs to
//! start animations.
//!
//! Starting an animation reads the mix duration from the [`AnimationStateData`], and starting one
//! by name looks it up in the [`SkeletonData`]. Those methods live on [`BorrowedAnimationState`],
//! so the data is passed once, when the view is made. Everything that does not need the data is
//! called on the state through the view's `Deref`.
//!
//! - [`AnimationState::borrow`] makes a view of a state that holds its mix settings.
//! - `AnimationState::as_borrow` makes a view of a state together with mix settings and skeleton
//!   data the caller keeps. It needs the `external-data` feature, and is for building a runtime
//!   or something similar; see [`AnimationState::detached`].

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::{empty_animation, AnimationState, AnimationStateData, EntryId, StateError};
use crate::animation::Animation;
use crate::skeleton_data::SkeletonData;

/// The mix settings and skeleton data of a view: held by the state, or given by the caller.
enum Data<'a> {
    /// A share of the state's own settings. A share, not a borrow, so a listener that runs while
    /// the view is alive can still start animations on the state.
    Held(Arc<AnimationStateData>),
    Given {
        mix_table: &'a AnimationStateData,
        skeleton_data: &'a SkeletonData,
    },
}

impl Data<'_> {
    fn mix_table(&self) -> &AnimationStateData {
        match self {
            Data::Held(table) => table,
            Data::Given { mix_table, .. } => mix_table,
        }
    }

    fn skeleton_data(&self) -> &SkeletonData {
        match self {
            Data::Held(table) => table.skeleton_data(),
            Data::Given { skeleton_data, .. } => skeleton_data,
        }
    }
}

/// An [`AnimationState`] together with its mix settings and [`SkeletonData`]. See the module
/// documentation.
pub struct BorrowedAnimationState<'a> {
    state: &'a mut AnimationState,
    data: Data<'a>,
}

impl AnimationState {
    /// A view of this state and the mix settings it holds. Panics for a detached state.
    pub fn borrow(&mut self) -> BorrowedAnimationState<'_> {
        let table = self.data().clone();
        BorrowedAnimationState {
            state: self,
            data: Data::Held(table),
        }
    }

    /// A view of this state with mix settings and skeleton data the caller keeps. Pass the data
    /// the skeleton was made from.
    ///
    /// Needs the `external-data` feature.
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    #[cfg_attr(not(feature = "external-data"), allow(dead_code))]
    pub fn as_borrow<'a>(
        &'a mut self,
        mix_table: &'a AnimationStateData,
        skeleton_data: &'a SkeletonData,
    ) -> BorrowedAnimationState<'a> {
        BorrowedAnimationState {
            state: self,
            data: Data::Given {
                mix_table,
                skeleton_data,
            },
        }
    }
}

impl Deref for BorrowedAnimationState<'_> {
    type Target = AnimationState;

    fn deref(&self) -> &AnimationState {
        self.state
    }
}

impl DerefMut for BorrowedAnimationState<'_> {
    fn deref_mut(&mut self) -> &mut AnimationState {
        self.state
    }
}

impl BorrowedAnimationState<'_> {
    fn find_animation(&self, name: &str) -> Result<Arc<Animation>, StateError> {
        self.data
            .skeleton_data()
            .find_animation(name)
            .cloned()
            .ok_or_else(|| StateError::AnimationNotFound(name.to_string()))
    }

    /// Sets the current animation of a track, discarding queued animations. The previous
    /// animation mixes out if there is one.
    pub fn set_animation(
        &mut self,
        track_index: usize,
        animation: Arc<Animation>,
        looping: bool,
    ) -> EntryId {
        let state = &mut *self.state;
        let mut interrupt = true;
        let mut current = state.expand_to_index(track_index);
        if let Some(cur) = current {
            let never_applied_same = {
                let c = &state.entries[cur];
                c.next_track_last == -1.0 && Arc::ptr_eq(&c.animation, &animation)
            };
            if never_applied_same {
                // Don't mix from an entry that was never applied.
                state.tracks[track_index] = state.entries[cur].mixing_from;
                state.queue_interrupt(cur);
                state.queue_end(cur);
                state.clear_next(cur);
                current = state.entries[cur].mixing_from;
                interrupt = false;
            } else {
                state.clear_next(cur);
            }
        }
        let entry = state.new_track_entry(
            self.data.mix_table(),
            track_index,
            animation,
            looping,
            current,
        );
        state.set_track(track_index, entry, interrupt);
        state.drain();
        entry
    }

    /// Like [`set_animation`](Self::set_animation), finding the animation by name.
    pub fn set_animation_by_name(
        &mut self,
        track_index: usize,
        name: &str,
        looping: bool,
    ) -> Result<EntryId, StateError> {
        let animation = self.find_animation(name)?;
        Ok(self.set_animation(track_index, animation, looping))
    }

    /// Queues an animation to play after the current and queued ones. `delay` is the seconds to
    /// wait; if 0 or negative it's relative to the previous entry's completion minus the mix
    /// duration.
    pub fn add_animation(
        &mut self,
        track_index: usize,
        animation: Arc<Animation>,
        looping: bool,
        mut delay: f32,
    ) -> EntryId {
        let state = &mut *self.state;
        let mut last = state.expand_to_index(track_index);
        if let Some(mut l) = last {
            while let Some(n) = state.entries[l].next {
                l = n;
            }
            last = Some(l);
        }
        let entry =
            state.new_track_entry(self.data.mix_table(), track_index, animation, looping, last);
        match last {
            None => {
                state.set_track(track_index, entry, true);
                state.drain();
                if delay < 0.0 {
                    delay = 0.0;
                }
            }
            Some(l) => {
                state.entries[l].next = Some(entry);
                state.entries[entry].previous = Some(l);
                if delay <= 0.0 {
                    delay = (delay + state.entries[l].track_complete()
                        - state.entries[entry].mix_duration)
                        .max(0.0);
                }
            }
        }
        state.entries[entry].delay = delay;
        entry
    }

    /// Like [`add_animation`](Self::add_animation), finding the animation by name.
    pub fn add_animation_by_name(
        &mut self,
        track_index: usize,
        name: &str,
        looping: bool,
        delay: f32,
    ) -> Result<EntryId, StateError> {
        let animation = self.find_animation(name)?;
        Ok(self.add_animation(track_index, animation, looping, delay))
    }

    /// Sets an empty animation for a track, discarding queued animations, mixing out the current
    /// animation over `mix_duration`.
    pub fn set_empty_animation(&mut self, track_index: usize, mix_duration: f32) -> EntryId {
        let entry = self.set_animation(track_index, empty_animation(), false);
        let e = &mut self.state.entries[entry];
        e.mix_duration = mix_duration;
        e.track_end = mix_duration;
        entry
    }

    /// Adds an empty animation to be played after the current or queued animations.
    pub fn add_empty_animation(
        &mut self,
        track_index: usize,
        mix_duration: f32,
        delay: f32,
    ) -> EntryId {
        let entry = self.add_animation(track_index, empty_animation(), false, delay);
        let e = &mut self.state.entries[entry];
        if delay <= 0.0 {
            e.delay = (e.delay + e.mix_duration - mix_duration).max(0.0);
        }
        e.mix_duration = mix_duration;
        e.track_end = mix_duration;
        entry
    }

    /// Sets an empty animation on every track that has an entry.
    pub fn set_empty_animations(&mut self, mix_duration: f32) {
        let old = self.state.drain_disabled;
        self.state.drain_disabled = true;
        for i in 0..self.state.tracks.len() {
            if self.state.tracks[i].is_some() {
                self.set_empty_animation(i, mix_duration);
            }
        }
        self.state.drain_disabled = old;
        self.state.drain();
    }
}
