//! Where the skeleton data of the extension lives.
//!
//! Every loaded skeleton data resource registers its data here, with the mix settings of its
//! animations, and gets a [`SkeletonDataId`] back. Skeletons in the world hold only that id, so
//! the data exists once however many sprites use it, and a change to it is seen by all of them.

use slotmap::{new_key_type, SlotMap};
use spine2d_core::{AnimationStateData, SkeletonData};
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::bevy_ecs::resource::Resource;
use spine2d_ecs::bevy_ecs::system::{Res, SystemParam, SystemParamItem};
use spine2d_ecs::SpineResource;

use super::world;

new_key_type! {
    /// Handle to the data of a [`SkeletonStore`] entry.
    pub struct SkeletonDataId;
}

/// What a handle to skeleton data belongs to: the data of a skeleton in the world, or the stored
/// data itself, which is what the skeleton data resource hands out.
///
/// Handles that only read or edit data work with either. The ones that also need a live skeleton
/// (the current poses) exist only for the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Skeleton(Entity),
    Data(SkeletonDataId),
}

impl From<Entity> for Owner {
    fn from(entity: Entity) -> Owner {
        Owner::Skeleton(entity)
    }
}

impl From<SkeletonDataId> for Owner {
    fn from(id: SkeletonDataId) -> Owner {
        Owner::Data(id)
    }
}

impl Owner {
    /// The skeleton, if this is the data of one.
    pub fn entity(self) -> Option<Entity> {
        match self {
            Owner::Skeleton(entity) => Some(entity),
            Owner::Data(_) => None,
        }
    }

    /// The id of the stored data, or `None` if the skeleton is gone.
    pub fn data_id(self) -> Option<SkeletonDataId> {
        match self {
            Owner::Skeleton(entity) => world::data_id(entity),
            Owner::Data(id) => Some(id),
        }
    }
}

/// A skeleton data and the mix settings of its animations.
pub struct StoredSkeleton {
    pub data: SkeletonData,
    pub mixes: AnimationStateData,
}

/// All skeleton data of the extension, by [`SkeletonDataId`].
#[derive(Resource, Default)]
pub struct SkeletonStore(SlotMap<SkeletonDataId, StoredSkeleton>);

impl SkeletonStore {
    /// Stores `data` with its `mixes` and returns the id to find them by.
    pub fn insert(&mut self, data: SkeletonData, mixes: AnimationStateData) -> SkeletonDataId {
        self.0.insert(StoredSkeleton { data, mixes })
    }

    /// The entry under `id`, or `None` if it was removed.
    pub fn get(&self, id: SkeletonDataId) -> Option<&StoredSkeleton> {
        self.0.get(id)
    }

    /// The entry under `id` for changing, or `None` if it was removed.
    pub fn get_mut(&mut self, id: SkeletonDataId) -> Option<&mut StoredSkeleton> {
        self.0.get_mut(id)
    }

    /// Removes the entry under `id`. Skeletons that still use it are skipped by the systems.
    pub fn remove(&mut self, id: SkeletonDataId) {
        self.0.remove(id);
    }
}

/// How the shared systems find the data of a skeleton in the [`SkeletonStore`].
#[derive(SystemParam)]
pub struct GodotSource<'w> {
    store: Res<'w, SkeletonStore>,
}

impl SpineResource for GodotSource<'static> {
    type Key = SkeletonDataId;

    fn skeleton_data<'a>(
        resource: &'a SystemParamItem<Self>,
        key: SkeletonDataId,
    ) -> Option<&'a SkeletonData> {
        resource.store.get(key).map(|stored| &stored.data)
    }

    fn mix_table<'a>(
        resource: &'a SystemParamItem<Self>,
        key: SkeletonDataId,
    ) -> Option<&'a AnimationStateData> {
        resource.store.get(key).map(|stored| &stored.mixes)
    }
}
