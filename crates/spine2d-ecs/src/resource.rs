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

//! Where the systems find the data of a skeleton.
//!
//! A skeleton and its animation state hold no [`SkeletonData`]: every system looks it up through
//! a [`SpineResource`] and passes it to the skeleton's `as_borrow`. How the data is stored is up
//! to the engine, so a Godot integration can keep it in a slot map and a Bevy one in its asset
//! storage, and the systems here stay the same.

use bevy_ecs::system::{SystemParam, SystemParamItem};
use spine2d_core::{AnimationStateData, SkeletonData};

/// A system parameter that gives the systems the data a skeleton was made from.
///
/// An engine integration implements it on a [`SystemParam`] that reads its own storage, for
/// example one over `Res<Assets<SkeletonData>>`. The methods take the parameter's item, the
/// value a running system receives, not the parameter type.
pub trait SpineResource: SystemParam + 'static {
    /// What a skeleton stores to find its data: a slot map key, an asset handle, and so on.
    type Key: Copy + Send + Sync + 'static;

    /// The data of the skeleton stored under `key`, or `None` if it is not (or no longer) there.
    fn skeleton_data<'a>(
        resource: &'a SystemParamItem<Self>,
        key: Self::Key,
    ) -> Option<&'a SkeletonData>;

    /// The mix settings stored under `key`, or `None` if they are not (or no longer) there.
    fn mix_table<'a>(
        resource: &'a SystemParamItem<Self>,
        key: Self::Key,
    ) -> Option<&'a AnimationStateData>;
}

/// A [`SpineResource`] over one skeleton, for the tests of this crate.
#[cfg(test)]
pub(crate) mod test_support {
    use bevy_ecs::resource::Resource;
    use bevy_ecs::system::{Res, SystemParam};
    use bevy_ecs::world::World;
    use spine2d_core::{AnimationStateData, SkeletonData};

    use super::SpineResource;

    /// The data of the one skeleton a test world has.
    #[derive(Resource)]
    pub(crate) struct TestStore {
        pub(crate) data: SkeletonData,
        pub(crate) table: AnimationStateData,
    }

    impl TestStore {
        /// Puts `data` and an empty mix table into `world`.
        pub(crate) fn insert(world: &mut World, data: SkeletonData) {
            world.insert_resource(TestStore {
                data,
                table: AnimationStateData::detached(),
            });
        }
    }

    #[derive(SystemParam)]
    pub(crate) struct TestSource<'w> {
        store: Res<'w, TestStore>,
    }

    impl SpineResource for TestSource<'static> {
        type Key = ();

        fn skeleton_data<'a>(
            resource: &'a bevy_ecs::system::SystemParamItem<Self>,
            _key: (),
        ) -> Option<&'a SkeletonData> {
            Some(&resource.store.data)
        }

        fn mix_table<'a>(
            resource: &'a bevy_ecs::system::SystemParamItem<Self>,
            _key: (),
        ) -> Option<&'a AnimationStateData> {
            Some(&resource.store.table)
        }
    }
}
