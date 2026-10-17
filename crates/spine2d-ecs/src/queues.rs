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

//! How a skeleton is controlled and observed.
//!
//! A [`SpineCommand`] changes what a skeleton plays; how an engine queues commands is up to it.
//! What a skeleton did comes back as [`SpineEvent`]s in the world's [`SpineEventChannel`], which
//! the engine integration drains.

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use crossbeam_channel::{Receiver, Sender};
use smol_str::SmolStr;
use spine2d_core::{AnimationState, EntryId, Event, EventType};

/// A change to what a skeleton plays. Applied in order, before the animation advances.
#[derive(Clone, Debug, PartialEq)]
pub enum SpineCommand {
    /// Plays an animation now, replacing whatever is on the track.
    SetAnimation {
        track: usize,
        name: SmolStr,
        looping: bool,
    },
    /// Plays an animation after the ones already on the track, `delay` seconds after the last one
    /// ends (a delay of 0 or less is relative to when the last one would complete).
    AddAnimation {
        track: usize,
        name: SmolStr,
        looping: bool,
        delay: f32,
    },
    /// Mixes the track out to nothing over `mix_duration` seconds.
    SetEmptyAnimation { track: usize, mix_duration: f32 },
    /// Mixes the track out to nothing after the queued animations.
    AddEmptyAnimation {
        track: usize,
        mix_duration: f32,
        delay: f32,
    },
    /// Stops the track at once, leaving the pose as it is.
    ClearTrack { track: usize },
    /// Stops every track at once.
    ClearTracks,
    /// Shows a skin by name, or the default skin for `None`.
    SetSkin(Option<SmolStr>),
    /// Resets bones and slots to the setup pose.
    SetupPose,
}

/// What happened to an animation.
#[derive(Clone, Debug, PartialEq)]
pub enum SpineEventKind {
    /// The animation started playing.
    Start,
    /// Another animation replaced it; it may still be mixing out.
    Interrupt,
    /// It will never be applied again.
    End,
    /// It is about to be freed.
    Dispose,
    /// It finished a loop, or reached its end.
    Complete,
    /// A user-defined event keyed in the animation fired.
    Custom(CustomEvent),
}

/// The payload of a user-defined animation event.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomEvent {
    pub name: SmolStr,
    /// The animation time the event was keyed at, in seconds.
    pub time: f32,
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: SmolStr,
    pub volume: f32,
    pub balance: f32,
}

/// An animation event on a skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct SpineEvent {
    pub kind: SpineEventKind,
    /// The animation entry the event belongs to. It stops resolving once the entry is disposed.
    pub entry: EntryId,
    /// The track the animation plays on, read when the event fired.
    pub track: usize,
    /// Name of the animation the event belongs to, read when the event fired.
    pub animation: SmolStr,
}

impl SpineEvent {
    /// Builds an event from what the animation state reports. Returns `None` for a custom event
    /// that arrives without its payload.
    fn from_state(
        state: &AnimationState,
        kind: EventType,
        entry: EntryId,
        event: Option<&Event>,
    ) -> Option<Self> {
        let kind = match kind {
            EventType::Start => SpineEventKind::Start,
            EventType::Interrupt => SpineEventKind::Interrupt,
            EventType::End => SpineEventKind::End,
            EventType::Dispose => SpineEventKind::Dispose,
            EventType::Complete => SpineEventKind::Complete,
            EventType::Event => {
                let event = event?;
                SpineEventKind::Custom(CustomEvent {
                    name: event.data.name.clone(),
                    time: event.time,
                    int_value: event.int_value,
                    float_value: event.float_value,
                    string_value: event.string_value.clone(),
                    volume: event.volume,
                    balance: event.balance,
                })
            }
        };
        let (track, animation) = state.entry(entry).map_or_else(
            || (0, SmolStr::default()),
            |e| (e.track_index(), SmolStr::new(e.animation().name())),
        );
        Some(SpineEvent {
            kind,
            entry,
            track,
            animation,
        })
    }
}

/// The one channel every skeleton in the world reports its events through. Both ends are
/// `Send + Sync`, so skeletons can be animated from any thread.
#[derive(Resource, Debug)]
pub struct SpineEventChannel {
    sender: Sender<(Entity, SpineEvent)>,
    receiver: Receiver<(Entity, SpineEvent)>,
}

impl Default for SpineEventChannel {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        SpineEventChannel { sender, receiver }
    }
}

impl SpineEventChannel {
    /// Makes `state` report its events as coming from `entity`, replacing any listener it has.
    pub(crate) fn listen(&self, state: &mut AnimationState, entity: Entity) {
        let sender = self.sender.clone();
        state.set_listener(move |state, kind, entry, event| {
            if let Some(event) = SpineEvent::from_state(state, kind, entry, event) {
                // The receiver lives as long as the world; a send can only fail while it is torn
                // down, when nobody wants the event.
                let _ = sender.send((entity, event));
            }
        });
    }

    /// Moves every event reported since the last call into `out`, oldest first.
    ///
    /// Nothing else removes events, so an integration has to call this regularly or they pile up.
    /// Keep `out` between calls and clear it after use, so it stops allocating once it has grown
    /// to the size of a frame's events.
    pub fn drain_into(&self, out: &mut Vec<(Entity, SpineEvent)>) {
        out.extend(self.receiver.try_iter());
    }
}
