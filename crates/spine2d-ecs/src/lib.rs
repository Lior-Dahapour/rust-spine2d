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

//! Engine-independent ECS side of the spine2d Spine 4.3 runtime.
//!
//! An engine integration creates skeletons with [`spawn_skeleton`], advances
//! [`bevy_time::Time`] once per update and runs the stages of [`SpineSystems`]. The systems apply
//! queued [`SpineCommand`]s and animate the skeletons, and report what the animations did as
//! [`SpineEvent`]s, which the integration drains from the [`SpineEventChannel`]. Nothing here
//! knows about meshes, transforms or textures; each engine draws the poses its own way.
//!
//! The `bevy_ecs` and `bevy_time` crates are re-exported, so an integration uses exactly the
//! versions this crate was built with.
//!
//! Run all stages in one schedule with [`add_spine_systems`], or one stage at a time with
//! [`add_spine_stage`] when the engine has to call back into scripts between stages.

pub mod components;
pub mod lod;
pub mod queues;
pub mod resource;
pub mod spawn;
pub mod systems;

pub use bevy_ecs;
pub use bevy_time;
pub use glam;

use bevy_ecs::component::Component;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel, SystemSet};
use bevy_ecs::world::World;
use bevy_time::Time;

pub use components::{
    SpineAnimation, SpineBounds, SpineCacheStale, SpineData, SpineDriver, SpineFollower,
    SpineInitial, SpinePhysics, SpineProcess, SpineSkeleton, SpineStep,
};
pub use lod::SpineLod;
pub use queues::{CustomEvent, SpineCommand, SpineEvent, SpineEventChannel, SpineEventKind};
pub use resource::SpineResource;
pub use spawn::spawn_skeleton;

/// The stages a skeleton goes through each frame, in order.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub enum SpineSystems {
    /// Applies queued [`SpineCommand`]s.
    Commands,
    /// Rebuilds the update order of skeletons whose constraints were edited, advances the time of
    /// animations, and decides which skeletons this frame processes.
    Update,
    /// Poses skeletons from their animations.
    Apply,
    /// Computes world transforms.
    Transforms,
    /// Places the [`SpineFollower`]s on their bones. No system of this crate runs here: each
    /// engine adds its own, because moving an engine object differs between engines.
    Follow,
    /// Updates [`SpineBounds`].
    Bounds,
    /// Ends what only lasts one frame: the [`SpineStep`]s.
    Cleanup,
}

impl SpineSystems {
    /// Every stage, in the order they run.
    pub const ALL: [SpineSystems; 7] = [
        SpineSystems::Commands,
        SpineSystems::Update,
        SpineSystems::Apply,
        SpineSystems::Transforms,
        SpineSystems::Follow,
        SpineSystems::Bounds,
        SpineSystems::Cleanup,
    ];
}

/// Label for a schedule that runs only the Spine systems, for integrations that own their own
/// world instead of living inside an engine's schedules.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, ScheduleLabel)]
pub struct SpineSchedule;

/// Inserts the resources the systems need: the clock and the event channel.
/// Existing ones are kept.
pub fn init_spine_resources(world: &mut World) {
    world.init_resource::<Time>();
    world.init_resource::<SpineEventChannel>();
}

/// Adds the systems of one stage to `schedule`.
///
/// `R` is how the engine stores skeleton data (see [`SpineResource`]), and `M` is the marker that
/// picks which skeletons the `Update`, `Apply` and `Transforms` stages move: [`SpineProcess`] or
/// [`SpinePhysics`]. The other stages ignore it.
///
/// Stages that run in separate schedules need the world to apply deferred changes in between,
/// which running a schedule does.
pub fn add_spine_stage<R: SpineResource, M: Component>(
    schedule: &mut Schedule,
    stage: SpineSystems,
) {
    match stage {
        // Queuing commands is how an engine reaches its skeletons, so each engine adds its own.
        SpineSystems::Commands => schedule,
        SpineSystems::Update => schedule.add_systems(
            (
                systems::rebuild_stale_caches::<R>,
                systems::update_state::<M>,
            )
                .chain()
                .in_set(stage),
        ),
        SpineSystems::Apply => schedule.add_systems(systems::apply_state::<R, M>.in_set(stage)),
        SpineSystems::Transforms => schedule.add_systems(
            (
                systems::drive_bones::<R, M>,
                systems::update_transforms::<R, M>,
            )
                .chain()
                .in_set(stage),
        ),
        // Placing followers means moving an engine object, so each engine adds its own system.
        SpineSystems::Follow => schedule,
        SpineSystems::Bounds => schedule.add_systems(systems::update_bounds.in_set(stage)),
        SpineSystems::Cleanup => schedule.add_systems(systems::clear_steps::<M>.in_set(stage)),
    };
}

/// Adds every stage to `schedule`, chained in order, moving the skeletons marked `M`. The world
/// needs the resources from [`init_spine_resources`].
pub fn add_spine_systems<R: SpineResource, M: Component>(schedule: &mut Schedule) {
    for stage in SpineSystems::ALL {
        add_spine_stage::<R, M>(schedule, stage);
    }
    schedule.configure_sets(
        (
            SpineSystems::Commands,
            SpineSystems::Update,
            SpineSystems::Apply,
            SpineSystems::Transforms,
            SpineSystems::Follow,
            SpineSystems::Bounds,
            SpineSystems::Cleanup,
        )
            .chain(),
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::change_detection::DetectChangesMut;
    use bevy_ecs::entity::Entity;
    use bevy_ecs::world::World;
    use bevy_time::Time;
    use spine2d_core::{Atlas, AtlasAttachmentLoader, SkeletonJson};

    use super::*;
    use crate::resource::test_support::{TestSource, TestStore};

    #[test]
    fn a_plain_world_and_schedule_animate_a_spawned_skeleton() {
        spine2d_core::set_y_down(false);
        let atlas = Atlas::parse("", "", None);
        let attachments = AtlasAttachmentLoader::new(&atlas);
        let data = SkeletonJson::new(&attachments)
            .read_skeleton_data(
                r#"{ "skeleton": { "spine": "4.3.0" }, "bones": [ { "name": "root" } ] }"#,
            )
            .expect("the test skeleton is valid");
        let mut world = World::new();
        init_spine_resources(&mut world);
        TestStore::insert(&mut world, data);
        let mut schedule = Schedule::new(SpineSchedule);
        add_spine_systems::<TestSource, SpineProcess>(&mut schedule);
        let entity = spawn_skeleton::<TestSource>(&mut world, (), &SpineInitial::default())
            .expect("the store has the data");
        world.entity_mut(entity).insert(SpineProcess);

        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        schedule.run(&mut world);

        let time = world
            .get::<SpineSkeleton>(entity)
            .expect("spawned with a pose")
            .0
            .time;
        assert!(time > 0.0);
        assert!(world.get::<SpineBounds>(entity).is_none());
    }

    const CONSTRAINT_JSON: &str = r#"{
        "skeleton": { "spine": "4.3.0" },
        "bones": [
            { "name": "root" },
            { "name": "upper", "parent": "root", "length": 100 },
            { "name": "fore", "parent": "upper", "x": 100, "length": 100 },
            { "name": "target", "parent": "root", "x": 120, "y": 80 }
        ],
        "constraints": [
            { "type": "ik", "name": "ik", "bones": ["upper", "fore"], "target": "target" }
        ]
    }"#;

    /// A world with one process-mode skeleton that has an IK constraint, before its first frame.
    fn constrained_skeleton() -> (World, Schedule, Entity) {
        spine2d_core::set_y_down(false);
        let atlas = Atlas::parse("", "", None);
        let attachments = AtlasAttachmentLoader::new(&atlas);
        let data = SkeletonJson::new(&attachments)
            .read_skeleton_data(CONSTRAINT_JSON)
            .expect("the test skeleton is valid");
        let mut world = World::new();
        init_spine_resources(&mut world);
        TestStore::insert(&mut world, data);
        let mut schedule = Schedule::new(SpineSchedule);
        add_spine_systems::<TestSource, SpineProcess>(&mut schedule);
        let entity = spawn_skeleton::<TestSource>(&mut world, (), &SpineInitial::default())
            .expect("the store has the data");
        world.entity_mut(entity).insert(SpineProcess);
        (world, schedule, entity)
    }

    fn run_frame(world: &mut World, schedule: &mut Schedule) {
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        schedule.run(world);
    }

    /// Turns the constraint off. Only `update_cache` turns it back on, so it shows a rebuild.
    fn deactivate_constraint(world: &mut World, entity: Entity) {
        world
            .get_mut::<SpineSkeleton>(entity)
            .expect("spawned with a pose")
            .constraint_mut(0)
            .set_active(false);
    }

    fn constraint_is_active(world: &World, entity: Entity) -> bool {
        world
            .get::<SpineSkeleton>(entity)
            .expect("spawned with a pose")
            .constraint(0)
            .is_active()
    }

    #[test]
    fn a_spawned_skeleton_carries_the_cache_marker() {
        let (world, _, entity) = constrained_skeleton();

        assert!(world.get::<SpineCacheStale>(entity).is_some());
    }

    #[test]
    fn a_new_skeleton_rebuilds_its_cache_on_its_first_frame() {
        let (mut world, mut schedule, entity) = constrained_skeleton();
        deactivate_constraint(&mut world, entity);

        run_frame(&mut world, &mut schedule);

        assert!(constraint_is_active(&world, entity));
    }

    #[test]
    fn an_unflagged_marker_causes_no_rebuild() {
        let (mut world, mut schedule, entity) = constrained_skeleton();
        run_frame(&mut world, &mut schedule);
        deactivate_constraint(&mut world, entity);

        run_frame(&mut world, &mut schedule);

        assert!(!constraint_is_active(&world, entity));
    }

    #[test]
    fn a_flagged_marker_rebuilds_the_cache_once() {
        let (mut world, mut schedule, entity) = constrained_skeleton();
        run_frame(&mut world, &mut schedule);
        deactivate_constraint(&mut world, entity);
        world
            .get_mut::<SpineCacheStale>(entity)
            .expect("spawned with the marker")
            .set_changed();

        run_frame(&mut world, &mut schedule);
        assert!(constraint_is_active(&world, entity));

        deactivate_constraint(&mut world, entity);
        run_frame(&mut world, &mut schedule);
        assert!(!constraint_is_active(&world, entity));
    }
}
