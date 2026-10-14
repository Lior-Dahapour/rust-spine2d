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

//! Applies animations over time to a skeleton, mixing between them.
//!
//! [`AnimationState`] manages tracks of queued [`TrackEntry`]s. Track entries live in an arena
//! inside the state and are addressed by [`EntryId`] handles: a handle whose entry has been
//! disposed simply returns `None`.

mod data;
mod track_entry;

pub use data::AnimationStateData;
pub use track_entry::TrackEntry;

use crate::animation::{
    search, Animation, ApplyOptions, AttachmentTimeline, Bone1Kind, BoneTimeline1, MixFrom,
    PropertyId, Timeline,
};
use crate::bone::BoneLocal;
use crate::event::Event;
use crate::math::{abs, ceil, fmod, sign};
use crate::skeleton::Skeleton;
use rustc_hash::FxHashMap;
use slotmap::{new_key_type, SlotMap};
use std::fmt;
use std::sync::{Arc, OnceLock};

new_key_type! {
    /// Handle to a [`TrackEntry`] owned by an [`AnimationState`].
    pub struct EntryId;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventType {
    /// An entry started applying.
    Start,
    /// Another entry replaced this one on the track; it may still be mixing out.
    Interrupt,
    /// The entry will never be applied again.
    End,
    /// The entry is about to be disposed.
    Dispose,
    /// The animation completed a loop iteration or reached its end.
    Complete,
    /// A user-defined animation event fired.
    Event,
}

/// Called for animation state events. May freely call back into the state.
pub type Listener =
    Box<dyn FnMut(&mut AnimationState, EventType, EntryId, Option<&Event>) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    AnimationNotFound(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::AnimationNotFound(name) => write!(f, "animation not found: {name}"),
        }
    }
}

impl std::error::Error for StateError {}

// Timeline modes: the low two bits are the `MixFrom`, plus the Hold flag.
const MODE_HOLD: u8 = 4;
const ATTACH_SETUP: i32 = 1;
const ATTACH_RETAIN: i32 = 2;

/// The shared empty animation used by `set_empty_animation` and friends.
pub fn empty_animation() -> Arc<Animation> {
    static EMPTY: OnceLock<Arc<Animation>> = OnceLock::new();
    EMPTY
        .get_or_init(|| Arc::new(Animation::new("<empty>", Vec::new(), 0.0)))
        .clone()
}

#[derive(Clone)]
struct QueueEntry {
    kind: EventType,
    entry: EntryId,
    event: Option<Arc<Event>>,
}

/// Applies animations over time, queues animations for later playback, mixes (crossfading)
/// between animations, and fires events.
pub struct AnimationState {
    data: Arc<AnimationStateData>,
    entries: SlotMap<EntryId, TrackEntry>,
    tracks: Vec<Option<EntryId>>,
    /// Events fired by timelines during the current apply.
    events: Vec<Arc<Event>>,
    queue: Vec<QueueEntry>,
    drain_disabled: bool,
    property_ids: FxHashMap<PropertyId, EntryId>,
    animations_changed: bool,
    listener: Option<Listener>,
    unkeyed_state: i32,
    /// Multiplier for the delta passed to `update`.
    pub time_scale: f32,
    manual_track_entry_disposal: bool,
}

impl AnimationState {
    pub fn new(data: Arc<AnimationStateData>) -> Self {
        AnimationState {
            data,
            entries: SlotMap::with_key(),
            tracks: Vec::new(),
            events: Vec::new(),
            queue: Vec::new(),
            drain_disabled: false,
            property_ids: FxHashMap::default(),
            animations_changed: false,
            listener: None,
            unkeyed_state: 0,
            time_scale: 1.0,
            manual_track_entry_disposal: false,
        }
    }

    pub fn data(&self) -> &Arc<AnimationStateData> {
        &self.data
    }

    /// The entry for a handle, or `None` if it has been disposed.
    pub fn entry(&self, id: EntryId) -> Option<&TrackEntry> {
        self.entries.get(id)
    }

    pub fn entry_mut(&mut self, id: EntryId) -> Option<&mut TrackEntry> {
        self.entries.get_mut(id)
    }

    /// The entry currently on a track (not queued entries).
    pub fn track(&self, track_index: usize) -> Option<EntryId> {
        self.tracks.get(track_index).copied().flatten()
    }

    pub fn tracks(&self) -> &[Option<EntryId>] {
        &self.tracks
    }

    /// Sets a listener called for events of all entries, replacing any existing one.
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

    /// Stops events from being delivered until `enable_queue`; they stay queued.
    pub fn disable_queue(&mut self) {
        self.drain_disabled = true;
    }

    pub fn enable_queue(&mut self) {
        self.drain_disabled = false;
    }

    /// If true, entries are not disposed automatically after their dispose event; the application
    /// must call `dispose_track_entry`.
    pub fn set_manual_track_entry_disposal(&mut self, value: bool) {
        self.manual_track_entry_disposal = value;
    }

    pub fn manual_track_entry_disposal(&self) -> bool {
        self.manual_track_entry_disposal
    }

    /// Frees an entry. Its handle becomes invalid.
    pub fn dispose_track_entry(&mut self, id: EntryId) {
        self.entries.remove(id);
    }

    // --- Update ---------------------------------------------------------------------------------

    /// Advances the time of all entries and fires queued events.
    pub fn update(&mut self, delta: f32) {
        let delta = delta * self.time_scale;
        for i in 0..self.tracks.len() {
            let Some(cur) = self.tracks[i] else {
                continue;
            };
            {
                let c = &mut self.entries[cur];
                c.animation_last = c.next_animation_last;
                c.track_last = c.next_track_last;
            }
            let mut current_delta = delta * self.entries[cur].time_scale;

            if self.entries[cur].delay > 0.0 {
                let c = &mut self.entries[cur];
                c.delay -= current_delta;
                if c.delay > 0.0 {
                    continue;
                }
                current_delta = -c.delay;
                c.delay = 0.0;
                self.animations_changed = true;
            }

            if let Some(next) = self.entries[cur].next {
                // When the next entry's delay is passed, change to it, preserving leftover time.
                let next_time = self.entries[cur].track_last - self.entries[next].delay;
                if next_time >= 0.0 {
                    let cur_scale = self.entries[cur].time_scale;
                    {
                        let n = &mut self.entries[next];
                        n.delay = 0.0;
                        n.track_time += if cur_scale == 0.0 {
                            0.0
                        } else {
                            (next_time / cur_scale + delta) * n.time_scale
                        };
                    }
                    self.entries[cur].track_time += current_delta;
                    self.set_track(i, next, true);
                    let mut n = next;
                    while let Some(from) = self.entries[n].mixing_from {
                        self.entries[n].mix_time += delta;
                        n = from;
                    }
                    continue;
                }
            } else if self.entries[cur].track_last >= self.entries[cur].track_end
                && self.entries[cur].mixed_out
            {
                // Clear the track when there is no next entry and the entry was applied mixed out
                // after the track end time.
                self.tracks[i] = None;
                self.queue_end(cur);
                self.clear_next(cur);
                continue;
            }

            if self.entries[cur].mixing_from.is_some() && self.update_mixing_from(cur, delta) {
                // End mixing from entries once all have completed.
                let from = self.entries[cur].mixing_from.take();
                if let Some(f) = from {
                    self.entries[f].mixing_to = None;
                }
                let mut from = from;
                while let Some(f) = from {
                    self.queue_end(f);
                    from = self.entries[f].mixing_from;
                }
            }

            self.entries[cur].track_time += current_delta;
        }
        self.drain();
    }

    fn update_mixing_from(&mut self, to: EntryId, delta: f32) -> bool {
        let Some(from) = self.entries[to].mixing_from else {
            return true;
        };
        let finished = self.update_mixing_from(from, delta);
        {
            let f = &mut self.entries[from];
            f.animation_last = f.next_animation_last;
            f.track_last = f.next_track_last;
        }

        // The mix is complete and the from entry was applied with it complete, so it is mixed out.
        if self.entries[from].mixed_out
            && self.entries[to].mix_time >= self.entries[to].mix_duration
        {
            // Mixing is complete for all entries before the from entry or the mix is instantaneous.
            if self.entries[from].total_alpha == 0.0 || self.entries[to].mix_duration == 0.0 {
                let before = self.entries[from].mixing_from;
                self.entries[to].mixing_from = before;
                if let Some(b) = before {
                    self.entries[b].mixing_to = Some(to);
                }
                self.queue_end(from);
            }
            return finished;
        }

        let from_scale = self.entries[from].time_scale;
        self.entries[from].track_time += delta * from_scale;
        self.entries[to].mix_time += delta;
        false
    }

    // --- Apply ----------------------------------------------------------------------------------

    /// Poses the skeleton from the current state of all tracks. Returns true if any entry was
    /// applied.
    pub fn apply(&mut self, skeleton: &mut Skeleton) -> bool {
        if self.animations_changed {
            self.compute_animations_changed();
        }
        let mut applied = false;

        for i in 0..self.tracks.len() {
            let Some(cur) = self.tracks[i] else {
                continue;
            };
            if self.entries[cur].delay > 0.0 {
                continue;
            }
            applied = true;

            // Apply mixing from entries first.
            let mut alpha = self.entries[cur].alpha;
            let mut mix_out = false;
            if self.entries[cur].mixing_from.is_some() {
                alpha *= self.apply_mixing_from(cur, skeleton);
            } else {
                let c = &self.entries[cur];
                if c.track_time >= c.track_end && c.next.is_none() {
                    alpha = 0.0; // Set to setup pose the last time the entry will be applied.
                    mix_out = true;
                }
            }
            self.entries[cur].mixed_out = mix_out;

            // Apply current entry.
            let (
                animation,
                reverse,
                add,
                shortest,
                alpha_attachment_threshold,
                animation_last,
                animation_time,
            ) = {
                let c = &self.entries[cur];
                (
                    c.animation.clone(),
                    c.reverse,
                    c.additive,
                    c.additive || c.shortest_rotation,
                    c.alpha_attachment_threshold,
                    c.animation_last,
                    c.animation_time(),
                )
            };
            let apply_time = if reverse {
                animation.duration - animation_time
            } else {
                animation_time
            };
            let timelines = animation.timelines();
            let unkeyed = self.unkeyed_state;

            if i == 0 && alpha == 1.0 {
                let o = ApplyOptions {
                    alpha,
                    from: MixFrom::Setup,
                    add: false,
                    out: false,
                    applied_pose: false,
                };
                for timeline in timelines {
                    match timeline {
                        Timeline::Attachment(t) => apply_attachment_timeline(
                            t,
                            skeleton,
                            unkeyed,
                            apply_time,
                            MixFrom::Setup,
                            true,
                        ),
                        _ => {
                            let events = if reverse {
                                None
                            } else {
                                Some(&mut self.events)
                            };
                            timeline.apply(skeleton, animation_last, apply_time, events, &o);
                        }
                    }
                }
            } else {
                let timeline_mode = std::mem::take(&mut self.entries[cur].timeline_mode);
                let mut rotations = std::mem::take(&mut self.entries[cur].timelines_rotation);
                let retain_attachments = !mix_out && alpha >= alpha_attachment_threshold;
                let first_frame = !shortest && rotations.len() != timelines.len() << 1;
                if first_frame {
                    rotations.clear();
                    rotations.resize(timelines.len() << 1, 0.0);
                }
                for (ii, timeline) in timelines.iter().enumerate() {
                    let mix_from = MixFrom::from_bits(timeline_mode[ii]);
                    match timeline {
                        Timeline::Bone1(t) if !shortest && t.kind == Bone1Kind::Rotate => {
                            apply_rotate_timeline(
                                t,
                                skeleton,
                                apply_time,
                                alpha,
                                mix_from,
                                &mut rotations,
                                ii << 1,
                                first_frame,
                            );
                        }
                        Timeline::Attachment(t) => {
                            apply_attachment_timeline(
                                t,
                                skeleton,
                                unkeyed,
                                apply_time,
                                mix_from,
                                retain_attachments,
                            );
                        }
                        _ => {
                            let o = ApplyOptions {
                                alpha,
                                from: mix_from,
                                add,
                                out: mix_out,
                                applied_pose: false,
                            };
                            let events = if reverse {
                                None
                            } else {
                                Some(&mut self.events)
                            };
                            timeline.apply(skeleton, animation_last, apply_time, events, &o);
                        }
                    }
                }
                let c = &mut self.entries[cur];
                c.timeline_mode = timeline_mode;
                c.timelines_rotation = rotations;
            }

            if reverse {
                self.events_reverse(&animation, animation_last, animation_time);
            }
            self.queue_events(cur, animation_time);
            self.events.clear();
            let c = &mut self.entries[cur];
            c.next_animation_last = animation_time;
            c.next_track_last = c.track_time;
        }

        // Slots whose attachment was set by an animation that no longer keys them go back to setup.
        let setup_state = self.unkeyed_state + ATTACH_SETUP;
        for i in 0..skeleton.slots.len() {
            if skeleton.slots[i].attachment_state == setup_state {
                let name = &skeleton.data.slots[i].attachment_name;
                let attachment = if name.is_empty() {
                    None
                } else {
                    skeleton.attachment_by_index(i, name)
                };
                skeleton.slots[i].pose.set_attachment(attachment);
            }
        }
        self.unkeyed_state += 2;

        self.drain();
        applied
    }

    /// Applies the entries being mixed out into `to`, recursively; returns the mix of `to`.
    fn apply_mixing_from(&mut self, to: EntryId, skeleton: &mut Skeleton) -> f32 {
        let from = self.entries[to]
            .mixing_from
            .expect("caller checked mixing_from");
        let from_mix = if self.entries[from].mixing_from.is_some() {
            self.apply_mixing_from(from, skeleton)
        } else {
            1.0
        };

        let (mix, to_alpha, to_mix_duration) = {
            let t = &self.entries[to];
            (t.mix(), t.alpha, t.mix_duration)
        };
        let (
            animation,
            f_alpha,
            event_threshold,
            mix_attachment_threshold,
            mix_draw_order_threshold,
            alpha_attachment_threshold,
        ) = {
            let f = &self.entries[from];
            (
                f.animation.clone(),
                f.alpha,
                f.event_threshold,
                f.mix_attachment_threshold,
                f.mix_draw_order_threshold,
                f.alpha_attachment_threshold,
            )
        };
        let a = f_alpha * from_mix;
        let keep = 1.0 - mix * to_alpha;
        let alpha_mix = a * (1.0 - mix);
        let alpha_hold = if keep > 0.0 { alpha_mix / keep } else { a };

        let timelines = animation.timelines();
        let timeline_mode = std::mem::take(&mut self.entries[from].timeline_mode);
        let timeline_hold_mix = std::mem::take(&mut self.entries[from].timeline_hold_mix);
        let mut rotations = std::mem::take(&mut self.entries[from].timelines_rotation);
        let retain_attachments = mix < mix_attachment_threshold;
        let draw_order = mix < mix_draw_order_threshold;
        let (add, reverse) = (self.entries[from].additive, self.entries[from].reverse);
        let shortest = add || self.entries[from].shortest_rotation;
        let first_frame = !shortest && rotations.len() != timelines.len() << 1;
        if first_frame {
            rotations.clear();
            rotations.resize(timelines.len() << 1, 0.0);
        }
        let animation_last = self.entries[from].animation_last;
        let animation_time = self.entries[from].animation_time();
        let apply_time = if reverse {
            animation.duration - animation_time
        } else {
            animation_time
        };
        let fire_events = !reverse && mix < event_threshold;
        let unkeyed = self.unkeyed_state;

        let mut total_alpha = 0.0;
        for (i, timeline) in timelines.iter().enumerate() {
            let mode = timeline_mode[i];
            let mix_from = MixFrom::from_bits(mode);
            let alpha = if mode & MODE_HOLD != 0 {
                match timeline_hold_mix[i] {
                    None => alpha_hold,
                    Some(hold_mix) => alpha_hold * (1.0 - self.entries[hold_mix].mix()),
                }
            } else {
                if !draw_order
                    && matches!(timeline, Timeline::DrawOrder(_))
                    && mix_from == MixFrom::Current
                {
                    continue;
                }
                alpha_mix
            };
            total_alpha += alpha;

            match timeline {
                Timeline::Bone1(t) if !shortest && t.kind == Bone1Kind::Rotate => {
                    apply_rotate_timeline(
                        t,
                        skeleton,
                        apply_time,
                        alpha,
                        mix_from,
                        &mut rotations,
                        i << 1,
                        first_frame,
                    );
                }
                Timeline::Attachment(t) => {
                    apply_attachment_timeline(
                        t,
                        skeleton,
                        unkeyed,
                        apply_time,
                        mix_from,
                        retain_attachments && alpha >= alpha_attachment_threshold,
                    );
                }
                _ => {
                    let out = !draw_order
                        || !matches!(timeline, Timeline::DrawOrder(_))
                        || mix_from == MixFrom::Current;
                    let o = ApplyOptions {
                        alpha,
                        from: mix_from,
                        add,
                        out,
                        applied_pose: false,
                    };
                    let events = if fire_events {
                        Some(&mut self.events)
                    } else {
                        None
                    };
                    timeline.apply(skeleton, animation_last, apply_time, events, &o);
                }
            }
        }
        {
            let f = &mut self.entries[from];
            f.total_alpha = total_alpha;
            f.timeline_mode = timeline_mode;
            f.timeline_hold_mix = timeline_hold_mix;
            f.timelines_rotation = rotations;
        }

        if reverse && mix < event_threshold {
            self.events_reverse(&animation, animation_last, animation_time);
        }
        if to_mix_duration > 0.0 {
            self.queue_events(from, animation_time);
        }
        self.events.clear();
        let f = &mut self.entries[from];
        f.next_animation_last = animation_time;
        f.next_track_last = f.track_time;
        f.mixed_out = mix == 1.0;
        mix
    }

    /// Queues the events fired during this apply, around the animation's completion.
    fn queue_events(&mut self, entry: EntryId, animation_time: f32) {
        let e = &self.entries[entry];
        let (animation_start, animation_end) = (e.animation_start, e.animation_end);
        let duration = animation_end - animation_start;
        let reverse = e.reverse;
        let mut split = if duration != 0.0 {
            fmod(e.track_last, duration)
        } else {
            0.0
        };
        if reverse {
            split = duration - split;
        }

        // Queue events before complete.
        let n = self.events.len();
        let mut i = 0;
        while i < n {
            let event = &self.events[i];
            if (event.time < split) != reverse {
                break;
            }
            if event.time >= animation_start && event.time <= animation_end {
                let event = event.clone();
                self.queue_event(entry, event);
            }
            i += 1;
        }

        // Queue complete if completed a loop iteration or the animation.
        let e = &self.entries[entry];
        let complete = if e.looping {
            if duration == 0.0 {
                true
            } else {
                let cycles = (e.track_time / duration) as i32;
                cycles > 0 && cycles > (e.track_last / duration) as i32
            }
        } else {
            animation_time >= animation_end && e.animation_last < animation_end
        };
        if complete {
            self.queue_complete(entry);
        }

        // Queue events after complete.
        while i < n {
            let event = &self.events[i];
            if event.time >= animation_start && event.time <= animation_end {
                let event = event.clone();
                self.queue_event(entry, event);
            }
            i += 1;
        }
    }

    /// Collects the events a reversed animation fires between `animation_last` and
    /// `animation_time`.
    fn events_reverse(&mut self, animation: &Animation, animation_last: f32, animation_time: f32) {
        let duration = animation.duration;
        let from = duration - animation_last;
        let to = duration - animation_time;
        for timeline in animation.timelines() {
            let Timeline::Event(t) = timeline else {
                continue;
            };
            let frames = &t.frames;
            let push = |events: &mut Vec<Arc<Event>>, i: usize| {
                if let Some(e) = &t.events[i] {
                    events.push(e.clone());
                }
            };
            if from >= to {
                for (ii, &time) in frames.iter().enumerate() {
                    if time < to {
                        continue;
                    }
                    if time >= from {
                        break;
                    }
                    push(&mut self.events, ii);
                }
            } else {
                for (ii, &time) in frames.iter().enumerate() {
                    if time >= from {
                        break;
                    }
                    push(&mut self.events, ii);
                }
                let start = frames.iter().position(|&t| t >= to).unwrap_or(frames.len());
                for ii in start..frames.len() {
                    push(&mut self.events, ii);
                }
            }
        }
    }

    // --- Tracks ---------------------------------------------------------------------------------

    /// Removes all tracks, ending their entries.
    pub fn clear_tracks(&mut self) {
        let old = self.drain_disabled;
        self.drain_disabled = true;
        for i in 0..self.tracks.len() {
            self.clear_track(i);
        }
        self.tracks.clear();
        self.drain_disabled = old;
        self.drain();
    }

    /// Removes a track, ending its entry (and everything mixing into or queued after it).
    pub fn clear_track(&mut self, track_index: usize) {
        let Some(current) = self.track(track_index) else {
            return;
        };
        self.queue_end(current);
        self.clear_next(current);

        let mut entry = current;
        while let Some(from) = self.entries[entry].mixing_from {
            self.queue_end(from);
            self.entries[entry].mixing_from = None;
            self.entries[entry].mixing_to = None;
            entry = from;
        }

        let index = self.entries[current].track_index;
        self.tracks[index] = None;
        self.drain();
    }

    /// Sets the current animation of a track, discarding queued animations. The previous
    /// animation mixes out if there is one.
    pub fn set_animation(
        &mut self,
        track_index: usize,
        animation: Arc<Animation>,
        looping: bool,
    ) -> EntryId {
        let mut interrupt = true;
        let mut current = self.expand_to_index(track_index);
        if let Some(cur) = current {
            let never_applied_same = {
                let c = &self.entries[cur];
                c.next_track_last == -1.0 && Arc::ptr_eq(&c.animation, &animation)
            };
            if never_applied_same {
                // Don't mix from an entry that was never applied.
                self.tracks[track_index] = self.entries[cur].mixing_from;
                self.queue_interrupt(cur);
                self.queue_end(cur);
                self.clear_next(cur);
                current = self.entries[cur].mixing_from;
                interrupt = false;
            } else {
                self.clear_next(cur);
            }
        }
        let entry = self.new_track_entry(track_index, animation, looping, current);
        self.set_track(track_index, entry, interrupt);
        self.drain();
        entry
    }

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
        let mut last = self.expand_to_index(track_index);
        if let Some(mut l) = last {
            while let Some(n) = self.entries[l].next {
                l = n;
            }
            last = Some(l);
        }
        let entry = self.new_track_entry(track_index, animation, looping, last);
        match last {
            None => {
                self.set_track(track_index, entry, true);
                self.drain();
                if delay < 0.0 {
                    delay = 0.0;
                }
            }
            Some(l) => {
                self.entries[l].next = Some(entry);
                self.entries[entry].previous = Some(l);
                if delay <= 0.0 {
                    delay = (delay + self.entries[l].track_complete()
                        - self.entries[entry].mix_duration)
                        .max(0.0);
                }
            }
        }
        self.entries[entry].delay = delay;
        entry
    }

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
        let e = &mut self.entries[entry];
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
        let e = &mut self.entries[entry];
        if delay <= 0.0 {
            e.delay = (e.delay + e.mix_duration - mix_duration).max(0.0);
        }
        e.mix_duration = mix_duration;
        e.track_end = mix_duration;
        entry
    }

    /// Sets an empty animation on every track that has an entry.
    pub fn set_empty_animations(&mut self, mix_duration: f32) {
        let old = self.drain_disabled;
        self.drain_disabled = true;
        for i in 0..self.tracks.len() {
            if self.tracks[i].is_some() {
                self.set_empty_animation(i, mix_duration);
            }
        }
        self.drain_disabled = old;
        self.drain();
    }

    /// Sets an entry's mix duration. If `delay` is 0 or negative it is computed relative to the
    /// previous entry's completion, as in `add_animation`.
    pub fn set_mix_duration(&mut self, entry: EntryId, mix_duration: f32, mut delay: f32) {
        let previous_complete = self
            .entries
            .get(entry)
            .and_then(|e| e.previous)
            .map(|p| self.entries[p].track_complete());
        let Some(e) = self.entries.get_mut(entry) else {
            return;
        };
        e.mix_duration = mix_duration;
        if delay <= 0.0 {
            delay = match previous_complete {
                None => 0.0,
                Some(complete) => (delay + complete - mix_duration).max(0.0),
            };
        }
        e.delay = delay;
    }

    /// True if the entry's next entry is ready to take over.
    pub fn is_next_ready(&self, entry: EntryId) -> bool {
        self.entries.get(entry).is_some_and(|e| {
            e.next
                .is_some_and(|n| e.next_track_last - self.entries[n].delay >= 0.0)
        })
    }

    fn find_animation(&self, name: &str) -> Result<Arc<Animation>, StateError> {
        self.data
            .skeleton_data()
            .find_animation(name)
            .cloned()
            .ok_or_else(|| StateError::AnimationNotFound(name.to_string()))
    }

    fn set_track(&mut self, index: usize, current: EntryId, interrupt: bool) {
        let from = self.expand_to_index(index);
        self.tracks[index] = Some(current);
        self.entries[current].previous = None;

        if let Some(from) = from {
            self.entries[from].next = None;
            if interrupt {
                self.queue_interrupt(from);
            }
            self.entries[current].mixing_from = Some(from);
            self.entries[from].mixing_to = Some(current);
            self.entries[current].mix_time = 0.0;
            let f = &mut self.entries[from];
            f.timelines_rotation.clear(); // Reset rotation for mixing out, in case entry was mixed in.
            f.mixed_out = false; // Reset in case it was applied past its track end.
        }
        self.queue_start(current); // Triggers animations_changed.
    }

    /// Grows the tracks to include `index`; returns the entry already there.
    fn expand_to_index(&mut self, index: usize) -> Option<EntryId> {
        if index < self.tracks.len() {
            return self.tracks[index];
        }
        self.tracks.resize(index + 1, None);
        None
    }

    fn new_track_entry(
        &mut self,
        track_index: usize,
        animation: Arc<Animation>,
        looping: bool,
        last: Option<EntryId>,
    ) -> EntryId {
        let mix_duration = match last {
            None => 0.0,
            Some(l) => self
                .data
                .mix(self.entries[l].animation.name(), animation.name()),
        };
        let animation_end = animation.duration;
        self.entries.insert(TrackEntry {
            animation,
            previous: None,
            next: None,
            mixing_from: None,
            mixing_to: None,
            track_index,
            looping,
            additive: false,
            reverse: false,
            shortest_rotation: false,
            mixed_out: false,
            event_threshold: 0.0,
            mix_attachment_threshold: 0.0,
            alpha_attachment_threshold: 0.0,
            mix_draw_order_threshold: 0.0,
            animation_start: 0.0,
            animation_end,
            animation_last: -1.0,
            next_animation_last: -1.0,
            delay: 0.0,
            track_time: 0.0,
            track_last: -1.0,
            next_track_last: -1.0, // -1 signifies an entry that wasn't applied yet.
            track_end: f32::MAX,
            time_scale: 1.0,
            alpha: 1.0,
            mix_time: 0.0,
            mix_duration,
            total_alpha: 0.0,
            mix_interpolation: Default::default(),
            timeline_mode: Vec::new(),
            timeline_hold_mix: Vec::new(),
            timelines_rotation: Vec::new(),
            listener: None,
        })
    }

    fn clear_next(&mut self, entry: EntryId) {
        let mut next = self.entries[entry].next;
        while let Some(n) = next {
            self.queue_dispose(n);
            next = self.entries[n].next;
        }
        self.entries[entry].next = None;
    }

    // --- Hold computation -----------------------------------------------------------------------

    fn compute_animations_changed(&mut self) {
        self.animations_changed = false;
        for i in 0..self.tracks.len() {
            let Some(track) = self.tracks[i] else {
                continue;
            };
            if self.entries[track].delay > 0.0 {
                continue;
            }
            let mut entry = track;
            while let Some(from) = self.entries[entry].mixing_from {
                entry = from;
            }
            loop {
                self.compute_hold(entry, track);
                match self.entries[entry].mixing_to {
                    Some(to) => entry = to,
                    None => break,
                }
            }
        }
        self.property_ids.clear();
    }

    /// Decides, for each timeline of `entry`, whether it blends from setup, the current pose or
    /// the first key, and whether the pose is held while a later entry mixes in.
    fn compute_hold(&mut self, entry: EntryId, track: EntryId) {
        let animation = self.entries[entry].animation.clone();
        let timelines = animation.timelines();
        let add = self.entries[entry].additive;
        let to = self.entries[entry].mixing_to;

        let mut modes = std::mem::take(&mut self.entries[entry].timeline_mode);
        let mut hold_mix = std::mem::take(&mut self.entries[entry].timeline_hold_mix);
        modes.clear();
        modes.resize(timelines.len(), 0);
        hold_mix.clear();
        hold_mix.resize(timelines.len(), None);

        for (i, timeline) in timelines.iter().enumerate() {
            let ids = timeline.property_ids();
            let mix_from = self.timeline_mix_from(track, timeline, &ids);
            if add && timeline.additive() {
                modes[i] = mix_from;
                continue;
            }
            // Hold if the next entry will overwrite this property.
            let holds = match to {
                None => false,
                Some(to) => {
                    let t = &self.entries[to];
                    !(timeline.instant()
                        || (t.additive && timeline.additive())
                        || !t.animation.has_timeline(&ids))
                }
            };
            if !holds {
                modes[i] = mix_from;
            } else {
                modes[i] = mix_from | MODE_HOLD;
                // Find the next entry that doesn't overwrite this property. Its mix fades out the
                // hold, instead of it ending abruptly.
                let mut next = self.entries[to.expect("holds implies to")].mixing_to;
                while let Some(n) = next {
                    let t = &self.entries[n];
                    if (t.additive && timeline.additive()) || !t.animation.has_timeline(&ids) {
                        hold_mix[i] = Some(n);
                        break;
                    }
                    next = t.mixing_to;
                }
            }
        }
        let e = &mut self.entries[entry];
        e.timeline_mode = modes;
        e.timeline_hold_mix = hold_mix;
    }

    fn timeline_mix_from(&mut self, track: EntryId, timeline: &Timeline, ids: &[PropertyId]) -> u8 {
        let mut mix_from = MixFrom::Setup as u8;
        let mut i = 0;
        while i < ids.len() {
            match self.property_ids.entry(ids[i]) {
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(track);
                }
                std::collections::hash_map::Entry::Occupied(o) => {
                    if *o.get() != track {
                        for &id in &ids[i + 1..] {
                            self.property_ids.entry(id).or_insert(track);
                        }
                        return MixFrom::Current as u8;
                    }
                    mix_from = MixFrom::First as u8;
                }
            }
            i += 1;
        }
        if matches!(timeline, Timeline::DrawOrderFolder(_)) {
            if let Some(&first) = self
                .property_ids
                .get(&crate::animation::DrawOrderTimeline::property_id())
            {
                return if first != track {
                    MixFrom::Current as u8
                } else {
                    MixFrom::First as u8
                };
            }
        }
        mix_from
    }

    // --- Event queue ----------------------------------------------------------------------------

    fn queue_start(&mut self, entry: EntryId) {
        self.queue.push(QueueEntry {
            kind: EventType::Start,
            entry,
            event: None,
        });
        self.animations_changed = true;
    }

    fn queue_interrupt(&mut self, entry: EntryId) {
        self.queue.push(QueueEntry {
            kind: EventType::Interrupt,
            entry,
            event: None,
        });
    }

    fn queue_end(&mut self, entry: EntryId) {
        self.queue.push(QueueEntry {
            kind: EventType::End,
            entry,
            event: None,
        });
        self.animations_changed = true;
    }

    fn queue_dispose(&mut self, entry: EntryId) {
        self.queue.push(QueueEntry {
            kind: EventType::Dispose,
            entry,
            event: None,
        });
    }

    fn queue_complete(&mut self, entry: EntryId) {
        self.queue.push(QueueEntry {
            kind: EventType::Complete,
            entry,
            event: None,
        });
    }

    fn queue_event(&mut self, entry: EntryId, event: Arc<Event>) {
        self.queue.push(QueueEntry {
            kind: EventType::Event,
            entry,
            event: Some(event),
        });
    }

    /// Delivers all queued events to listeners and empties the queue. Listeners may queue more
    /// events, which are delivered in the same drain.
    fn drain(&mut self) {
        if self.drain_disabled {
            return;
        }
        self.drain_disabled = true;
        let mut i = 0;
        // Don't cache the length so listeners can queue their own events (eg, call set_animation
        // from a Complete listener).
        while i < self.queue.len() {
            let q = self.queue[i].clone();
            i += 1;
            match q.kind {
                EventType::Start | EventType::Interrupt | EventType::Complete => {
                    self.invoke(q.kind, q.entry, None)
                }
                EventType::End => {
                    self.invoke(EventType::End, q.entry, None);
                    self.invoke(EventType::Dispose, q.entry, None);
                    if !self.manual_track_entry_disposal {
                        self.dispose_track_entry(q.entry);
                    }
                }
                EventType::Dispose => {
                    self.invoke(EventType::Dispose, q.entry, None);
                    if !self.manual_track_entry_disposal {
                        self.dispose_track_entry(q.entry);
                    }
                }
                EventType::Event => self.invoke(EventType::Event, q.entry, q.event.as_deref()),
            }
        }
        self.queue.clear();
        self.drain_disabled = false;
    }

    /// Calls the entry's listener, then the state's. Listeners are taken out while running so
    /// they can receive `&mut self`.
    fn invoke(&mut self, kind: EventType, entry: EntryId, event: Option<&Event>) {
        if let Some(mut listener) = self.entries.get_mut(entry).and_then(|e| e.listener.take()) {
            listener(self, kind, entry, event);
            if let Some(e) = self.entries.get_mut(entry) {
                if e.listener.is_none() {
                    e.listener = Some(listener);
                }
            }
        }
        if let Some(mut listener) = self.listener.take() {
            listener(self, kind, entry, event);
            if self.listener.is_none() {
                self.listener = Some(listener);
            }
        }
    }
}

/// Applies an attachment timeline, tracking which slots animations key so unkeyed slots can be
/// reset to setup afterwards.
fn apply_attachment_timeline(
    t: &AttachmentTimeline,
    skeleton: &mut Skeleton,
    unkeyed_state: i32,
    time: f32,
    from: MixFrom,
    retain: bool,
) {
    let slot_index = t.slot;
    if !skeleton.bones[skeleton.slots[slot_index].bone.0].active {
        return;
    }
    if !retain && skeleton.slots[slot_index].attachment_state == unkeyed_state + ATTACH_RETAIN {
        return;
    }
    let mut setup = time < t.frames[0];
    let mut name: &str = "";
    if !setup {
        name = &t.attachment_names[search(&t.frames, time)];
        setup = !retain && name.is_empty();
    }
    if setup {
        if from == MixFrom::Current {
            return;
        }
        name = &skeleton.data.slots[slot_index].attachment_name;
    }
    let attachment = if name.is_empty() {
        None
    } else {
        skeleton.attachment_by_index(slot_index, name)
    };
    let slot = &mut skeleton.slots[slot_index];
    slot.pose.set_attachment(attachment);
    if retain {
        slot.attachment_state = unkeyed_state + ATTACH_RETAIN;
    } else if !setup {
        slot.attachment_state = unkeyed_state + ATTACH_SETUP;
    }
}

/// Applies a rotate timeline, mixing between rotations using the direction of the shortest route
/// on the first frame while detecting crosses. `rotations[i]` stores the total rotation and
/// `rotations[i + 1]` the last diff.
#[allow(clippy::too_many_arguments)]
fn apply_rotate_timeline(
    t: &BoneTimeline1,
    skeleton: &mut Skeleton,
    time: f32,
    alpha: f32,
    from: MixFrom,
    rotations: &mut [f32],
    i: usize,
    first_frame: bool,
) {
    if first_frame {
        rotations[i] = 0.0;
    }
    if alpha == 1.0 {
        let o = ApplyOptions {
            alpha: 1.0,
            from,
            add: false,
            out: false,
            applied_pose: false,
        };
        t.apply(skeleton, time, &o);
        return;
    }
    let bone = &mut skeleton.bones[t.bone];
    if !bone.active {
        return;
    }
    let setup: &BoneLocal = &skeleton.data.bones[t.bone].setup_pose;
    let pose = &mut bone.pose.local;
    let (r1, r2);
    if time < t.curve.frames[0] {
        match from {
            MixFrom::Setup => {
                pose.rotation = setup.rotation;
                return;
            }
            MixFrom::Current => return,
            MixFrom::First => {}
        }
        r1 = pose.rotation;
        r2 = setup.rotation;
    } else {
        r1 = if from == MixFrom::Setup {
            setup.rotation
        } else {
            pose.rotation
        };
        r2 = setup.rotation + t.curve.curve_value(time);
    }

    let mut diff = r2 - r1;
    diff -= ceil(diff / 360.0 - 0.5) * 360.0;
    let total;
    if diff == 0.0 {
        let last = rotations[i];
        total = sign(last) * ceil(abs(last) / 360.0 - 0.5) * 360.0;
        rotations[i] = total;
    } else {
        let (last_total, last_diff) = if first_frame {
            (0.0, diff)
        } else {
            (rotations[i], rotations[i + 1])
        };
        let loops = last_total - fmod(last_total, 360.0);
        let mut tot = diff + loops;
        let current = diff >= 0.0;
        let mut dir = last_total >= 0.0;
        if abs(last_diff) <= 90.0 && sign(last_diff) != sign(diff) {
            if abs(last_total - loops) > 180.0 {
                tot += 360.0 * sign(last_total);
                dir = current;
            } else if loops != 0.0 {
                tot -= 360.0 * sign(last_total);
            } else {
                dir = current;
            }
        }
        if dir != current {
            tot += 360.0 * sign(last_total);
        }
        rotations[i] = tot;
        total = tot;
    }
    rotations[i + 1] = diff;
    pose.rotation = r1 + total * alpha;
}
