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

//! Creating a skeleton entity.

use bevy_ecs::entity::Entity;
use bevy_ecs::system::SystemState;
use bevy_ecs::world::World;
use spine2d_core::{AnimationState, Physics, Skeleton};
use tracing::warn;

use crate::components::{SpineAnimation, SpineCacheStale, SpineData, SpineInitial, SpineSkeleton};
use crate::queues::SpineEventChannel;
use crate::resource::SpineResource;

/// Spawns a skeleton entity: its pose and its animation state, with the
/// initial skin and animation applied, reporting its events through the world's
/// [`SpineEventChannel`].
///
/// The data of the skeleton is read through `R` from the world, under `key`, and stays where it
/// is: the entity gets a [`SpineData`] with the key and a pose and state that hold no data. Add a
/// mode marker ([`crate::SpineProcess`] or [`crate::SpinePhysics`]) to the returned entity to have
/// a schedule move it. Unknown skin or animation names are logged and skipped.
///
/// Returns `None`, and spawns nothing, if `R` has no data or mix settings under `key`. The world
/// needs the resources from [`crate::init_spine_resources`].
pub fn spawn_skeleton<R: SpineResource>(
    world: &mut World,
    key: R::Key,
    initial: &SpineInitial,
) -> Option<Entity> {
    let mut params = SystemState::<R>::new(world);
    let source = params.get_mut(world).ok()?;
    let (data, mix_table) = (R::skeleton_data(&source, key)?, R::mix_table(&source, key)?);

    let mut pose = Skeleton::detached(data);
    let mut state = AnimationState::detached();
    let mut view = state.as_borrow(mix_table, data);
    if let Some(skin) = &initial.skin {
        if !pose.as_borrow(data).set_skin_by_name(skin) {
            warn!("skin {skin} not found");
        }
    }
    if let Some(animation) = &initial.animation {
        if let Err(error) = view.set_animation_by_name(0, animation, initial.looping) {
            warn!("{error}");
        }
    }
    drop(view);
    pose.as_borrow(data).setup_pose();
    state.apply(&mut pose.as_borrow(data));
    pose.as_borrow(data).update_world_transform(Physics::Reset);
    drop(source);

    let entity = world.spawn_empty().id();
    world
        .resource::<SpineEventChannel>()
        .listen(&mut state, entity);
    world.entity_mut(entity).insert((
        SpineData(key),
        SpineSkeleton(pose),
        SpineAnimation(state),
        SpineCacheStale,
    ));
    Some(entity)
}
