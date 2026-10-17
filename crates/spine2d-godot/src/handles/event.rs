//! `SpineEventData` and `SpineEvent`: the description of a user-defined animation event, and one
//! event that fired.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{Event, EventData};
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::CustomEvent;
use std::sync::Arc;

use crate::runtime::store::Owner;
use crate::runtime::world;

/// The defaults of an event type, as authored in the skeleton file: its name, payload and audio.
/// The data is shared by every sprite that uses it, so the setters change it for all of them. The
/// keys of an animation keep the values they were loaded with.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineEventData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpineEventData {
    /// A handle to the data of event `index` of `owner`: a skeleton in the world, or the stored
    /// data.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineEventData {
            base,
            owner: owner.into(),
            index,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&EventData) -> R) -> R {
        world::owner_data(self.owner, |data| f(&data.events[self.index])).unwrap_or_default()
    }

    fn edit(&self, f: impl FnOnce(&mut EventData)) {
        world::edit_data(self.owner, |data| {
            f(Arc::make_mut(&mut data.events[self.index]));
        });
    }
}

#[godot_api]
impl SpineEventData {
    /// False once the sprite this data came from was reloaded or freed. Reads then return zero
    /// values.
    #[func]
    fn is_valid(&self) -> bool {
        world::owner_data(self.owner, |_| ()).is_some()
    }

    #[func]
    fn get_event_name(&self) -> GString {
        self.read(|data| GString::from(data.name.as_str()))
    }

    #[func]
    fn get_int_value(&self) -> i32 {
        self.read(|data| data.int_value)
    }

    #[func]
    fn set_int_value(&mut self, value: i32) {
        self.edit(|data| data.int_value = value);
    }

    #[func]
    fn get_float_value(&self) -> f32 {
        self.read(|data| data.float_value)
    }

    #[func]
    fn set_float_value(&mut self, value: f32) {
        self.edit(|data| data.float_value = value);
    }

    #[func]
    fn get_string_value(&self) -> GString {
        self.read(|data| GString::from(data.string_value.as_str()))
    }

    #[func]
    fn set_string_value(&mut self, value: GString) {
        self.edit(|data| data.string_value = value.to_string().into());
    }

    #[func]
    fn get_audio_path(&self) -> GString {
        self.read(|data| GString::from(data.audio_path.as_str()))
    }

    #[func]
    fn set_audio_path(&mut self, value: GString) {
        self.edit(|data| data.audio_path = value.to_string().into());
    }

    #[func]
    fn get_volume(&self) -> f32 {
        self.read(|data| data.volume)
    }

    #[func]
    fn set_volume(&mut self, value: f32) {
        self.edit(|data| data.volume = value);
    }

    #[func]
    fn get_balance(&self) -> f32 {
        self.read(|data| data.balance)
    }

    #[func]
    fn set_balance(&mut self, value: f32) {
        self.edit(|data| data.balance = value);
    }
}

/// An event that fired while an animation played, with its payload. A script gets it from the
/// `animation_event` signal. It is a copy: changing it changes only this object.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineEvent {
    base: Base<RefCounted>,
    entity: Entity,
    event: CustomEvent,
}

impl SpineEvent {
    /// A copy of `event`, fired by the skeleton `entity`.
    pub fn create(entity: Entity, event: &CustomEvent) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineEvent {
            base,
            entity,
            event: event.clone(),
        })
    }

    /// A copy of the core event `event`, fired by the skeleton `entity`.
    pub fn from_core(entity: Entity, event: &Event) -> Gd<Self> {
        Self::create(
            entity,
            &CustomEvent {
                name: event.data.name.clone(),
                time: event.time,
                int_value: event.int_value,
                float_value: event.float_value,
                string_value: event.string_value.clone(),
                volume: event.volume,
                balance: event.balance,
            },
        )
    }
}

#[godot_api]
impl SpineEvent {
    /// The data of the event's type, or null once the sprite is gone.
    #[func]
    fn get_data(&self) -> Option<Gd<SpineEventData>> {
        let name = &self.event.name;
        let index = world::skeleton(self.entity, |skeleton| {
            skeleton.data().events.iter().position(|e| e.name == *name)
        })??;
        Some(SpineEventData::create(self.entity, index))
    }

    /// The animation time the event was keyed at, in seconds.
    #[func]
    fn get_time(&self) -> f32 {
        self.event.time
    }

    #[func]
    fn get_int_value(&self) -> i32 {
        self.event.int_value
    }

    #[func]
    fn set_int_value(&mut self, value: i32) {
        self.event.int_value = value;
    }

    #[func]
    fn get_float_value(&self) -> f32 {
        self.event.float_value
    }

    #[func]
    fn set_float_value(&mut self, value: f32) {
        self.event.float_value = value;
    }

    #[func]
    fn get_string_value(&self) -> GString {
        GString::from(self.event.string_value.as_str())
    }

    #[func]
    fn set_string_value(&mut self, value: GString) {
        self.event.string_value = value.to_string().into();
    }

    #[func]
    fn get_volume(&self) -> f32 {
        self.event.volume
    }

    #[func]
    fn set_volume(&mut self, value: f32) {
        self.event.volume = value;
    }

    #[func]
    fn get_balance(&self) -> f32 {
        self.event.balance
    }

    #[func]
    fn set_balance(&mut self, value: f32) {
        self.event.balance = value;
    }
}
