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

use super::{search, Property, PropertyId};
use crate::event::Event;
use smallvec::{smallvec, SmallVec};
use std::sync::Arc;

/// Fires events at discrete times.
#[derive(Clone, Debug)]
pub struct EventTimeline {
    pub frames: Vec<f32>,
    pub events: Vec<Option<Arc<Event>>>,
}

impl EventTimeline {
    pub fn new(frame_count: usize) -> Self {
        EventTimeline {
            frames: vec![0.0; frame_count],
            events: vec![None; frame_count],
        }
    }

    pub fn set_frame(&mut self, frame: usize, event: Arc<Event>) {
        self.frames[frame] = event.time;
        self.events[frame] = Some(event);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::Event.id(0)]
    }

    /// Appends the events keyed between `last_time` (exclusive) and `time` (inclusive).
    pub(crate) fn apply(&self, last_time: f32, time: f32, events: Option<&mut Vec<Arc<Event>>>) {
        let Some(events) = events else {
            return;
        };
        let frame_count = self.frames.len();
        if frame_count == 0 {
            return;
        }
        let mut last_time = last_time;
        if last_time > time {
            // Looped: fire the events after last_time up to the end, then continue from the start.
            self.fire(last_time, f32::MAX, events);
            last_time = -1.0;
        } else if last_time >= self.frames[frame_count - 1] {
            return; // Last time is after the last frame.
        }
        self.fire(last_time, time, events);
    }

    fn fire(&self, last_time: f32, time: f32, events: &mut Vec<Arc<Event>>) {
        let frame_count = self.frames.len();
        if last_time >= self.frames[frame_count - 1] || time < self.frames[0] {
            return;
        }
        let mut i;
        if last_time < self.frames[0] {
            i = 0;
        } else {
            i = search(&self.frames, last_time) + 1;
            let frame_time = self.frames[i];
            // Fire multiple events keyed at the same time.
            while i > 0 && self.frames[i - 1] == frame_time {
                i -= 1;
            }
        }
        while i < frame_count && time >= self.frames[i] {
            if let Some(event) = &self.events[i] {
                events.push(event.clone());
            }
            i += 1;
        }
    }
}
