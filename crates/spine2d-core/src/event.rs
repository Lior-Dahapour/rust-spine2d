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

use smol_str::SmolStr;
use std::sync::Arc;

/// Setup data for an event: its name, audio path and the default values events inherit.
#[derive(Clone, Debug)]
pub struct EventData {
    pub name: SmolStr,
    pub audio_path: SmolStr,
    // The setup pose: default payload for events of this type.
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: SmolStr,
    pub volume: f32,
    pub balance: f32,
}

impl EventData {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        let name = name.into();
        debug_assert!(!name.is_empty(), "event name must not be empty");
        EventData {
            name,
            audio_path: SmolStr::default(),
            int_value: 0,
            float_value: 0.0,
            string_value: SmolStr::default(),
            volume: 0.0,
            balance: 0.0,
        }
    }
}

/// Stores the current pose values for an event. Immutable once on a timeline; the state machine
/// hands out shared references when it fires events.
#[derive(Clone, Debug)]
pub struct Event {
    pub data: Arc<EventData>,
    /// The animation time this event was keyed, in seconds.
    pub time: f32,
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: SmolStr,
    pub volume: f32,
    pub balance: f32,
}

impl Event {
    /// An event with zeroed payload, like the reference constructor. Loaders copy the payload
    /// from the event data's setup pose.
    pub fn new(time: f32, data: Arc<EventData>) -> Self {
        Event {
            data,
            time,
            int_value: 0,
            float_value: 0.0,
            string_value: SmolStr::default(),
            volume: 0.0,
            balance: 0.0,
        }
    }
}
