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

//! The systems that run a skeleton each frame, one per stage of [`crate::SpineSystems`].
//!
//! Everything shares data only through components and resources. The only clock is the [`Time`]
//! resource, which the engine integration advances once per update.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::query::{Changed, QueryData, With};
use bevy_ecs::system::{Commands, Local, Query, Res, StaticSystemParam, SystemParamItem};
use bevy_time::Time;
use glam::Affine2;
use spine2d_core::skeleton::BorrowedSkeleton;
use spine2d_core::{AnimationState, AnimationStateData, BoneId, Physics, Skeleton, SkeletonData};
use spine2d_render::compute_bounds;
use tracing::warn;

use crate::components::{
    SpineAnimation, SpineBounds, SpineCacheStale, SpineData, SpineDriver, SpineProcess,
    SpineSkeleton, SpineStep,
};
use crate::lod::SpineLod;
use crate::queues::SpineCommand;
use crate::resource::SpineResource;

/// The data stored under `key`. A key with no data (removed from the store) makes the system skip
/// that skeleton; `warned` makes the first one in a system's lifetime log, so a skeleton whose
/// data is gone does not log every frame.
fn data_or_warn<'a, R: SpineResource>(
    source: &'a SystemParamItem<R>,
    key: R::Key,
    warned: &mut bool,
) -> Option<&'a SkeletonData> {
    let data = R::skeleton_data(source, key);
    if data.is_none() && !*warned {
        *warned = true;
        warn!("a skeleton's data is not in the store; skipping the skeleton");
    }
    data
}

/// Rebuilds the update order of the skeletons whose constraints were edited since this system last
/// ran. It matches on [`SpineCacheStale`] alone, with no mode filter, so a skeleton that is paused,
/// manual or skipped by LOD at the time of the edit is still found.
pub fn rebuild_stale_caches<R: SpineResource>(
    source: StaticSystemParam<R>,
    mut warned: Local<bool>,
    mut stale: Query<(&SpineData<R::Key>, &mut SpineSkeleton), Changed<SpineCacheStale>>,
) {
    for (key, mut skeleton) in &mut stale {
        if let Some(data) = data_or_warn::<R>(&source, key.0, &mut warned) {
            skeleton.as_borrow(data).update_cache();
        }
    }
}

/// Applies one command to a skeleton, given its data and mix settings. Unknown animation or skin
/// names are logged and skipped.
pub fn apply_command(
    command: SpineCommand,
    pose: &mut Skeleton,
    state: &mut AnimationState,
    data: &SkeletonData,
    mix_table: &AnimationStateData,
) {
    let mut state = state.as_borrow(mix_table, data);
    let mut pose = pose.as_borrow(data);
    let result = match command {
        SpineCommand::SetAnimation {
            track,
            name,
            looping,
        } => state.set_animation_by_name(track, &name, looping).map(drop),
        SpineCommand::AddAnimation {
            track,
            name,
            looping,
            delay,
        } => state
            .add_animation_by_name(track, &name, looping, delay)
            .map(drop),
        SpineCommand::SetEmptyAnimation {
            track,
            mix_duration,
        } => {
            state.set_empty_animation(track, mix_duration);
            Ok(())
        }
        SpineCommand::AddEmptyAnimation {
            track,
            mix_duration,
            delay,
        } => {
            state.add_empty_animation(track, mix_duration, delay);
            Ok(())
        }
        SpineCommand::ClearTrack { track } => {
            state.clear_track(track);
            Ok(())
        }
        SpineCommand::ClearTracks => {
            state.clear_tracks();
            Ok(())
        }
        SpineCommand::SetSkin(None) => {
            pose.set_skin(None);
            Ok(())
        }
        SpineCommand::SetSkin(Some(name)) => {
            if !pose.set_skin_by_name(&name) {
                warn!("skin {name} not found");
            }
            Ok(())
        }
        SpineCommand::SetupPose => {
            pose.setup_pose();
            Ok(())
        }
    };
    if let Err(error) = result {
        warn!("{error}");
    }
}

/// Gives the skeleton `entity` a [`SpineStep`] of `delta` seconds for the coming frame.
///
/// A skeleton with no mode (`has_mode` false) is made a [`SpineProcess`] skeleton until
/// [`clear_steps`] runs, so the stages of that frame move it, and only by the step. A step from an
/// earlier call that no frame has consumed yet (`earlier`) is added to, so steps add up into one,
/// and it stays manual even though the marker its first call added now looks like a mode.
pub fn begin_step(
    commands: &mut Commands,
    entity: Entity,
    delta: f32,
    has_mode: bool,
    earlier: Option<&SpineStep>,
) {
    let delta = delta + earlier.map_or(0.0, SpineStep::delta);
    let manual = earlier.map_or(!has_mode, SpineStep::is_manual);
    let mut entity = commands.entity(entity);
    if manual {
        entity.insert(SpineProcess);
    }
    entity.insert(SpineStep::new(delta, manual));
}

/// Removes the [`SpineStep`] of the skeletons marked `M` once the frame has moved them, and the
/// marker that only the step added.
pub fn clear_steps<M: Component>(
    mut commands: Commands,
    steps: Query<(Entity, &SpineStep), With<M>>,
) {
    for (entity, step) in &steps {
        let mut entity = commands.entity(entity);
        entity.remove::<SpineStep>();
        if step.is_manual() {
            entity.remove::<M>();
        }
    }
}

/// Advances one skeleton's clock and its animation state by `delta` seconds.
///
/// The animation state scales its own delta; the skeleton's clock (physics) must run at the same
/// speed, so it is scaled here.
pub fn advance_skeleton(pose: &mut Skeleton, state: &mut AnimationState, delta: f32) {
    pose.update_time(delta * state.time_scale);
    state.update(delta);
}

/// Poses one skeleton from its animation tracks.
pub fn pose_skeleton(pose: &mut Skeleton, state: &mut AnimationState, data: &SkeletonData) {
    state.apply(&mut pose.as_borrow(data));
}

/// Computes the world transforms of one skeleton.
pub fn transform_skeleton(pose: &mut Skeleton, data: &SkeletonData) {
    pose.as_borrow(data).update_world_transform(Physics::Update);
}

/// What `update_state` needs of a skeleton.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct Advancing {
    entity: Entity,
    pose: &'static mut SpineSkeleton,
    animation: &'static mut SpineAnimation,
    lod: Option<&'static mut SpineLod>,
    step: Option<&'static SpineStep>,
}

/// Advances the skeletons marked `M` ([`SpineProcess`] or [`SpinePhysics`]) by the frame time.
/// Skeletons with a [`SpineLod`] that skips this frame are left alone, and the later stages leave
/// them alone too.
///
/// [`SpineProcess`]: crate::SpineProcess
/// [`SpinePhysics`]: crate::SpinePhysics
pub fn update_state<M: Component>(
    time: Res<Time>,
    mut frame: Local<u32>,
    mut skeletons: Query<Advancing, With<M>>,
) {
    *frame = frame.wrapping_add(1);
    for mut skeleton in &mut skeletons {
        let elapsed = match skeleton.step {
            Some(step) if step.is_manual() => step.delta(),
            Some(step) => time.delta_secs() + step.delta(),
            None => time.delta_secs(),
        };
        let delta = match skeleton.lod.as_mut() {
            None => Some(elapsed),
            Some(lod) => lod.tick(elapsed, *frame, skeleton.entity),
        };
        if let Some(delta) = delta {
            advance_skeleton(&mut skeleton.pose, &mut skeleton.animation, delta);
        }
    }
}

/// Places `bone` at `transform`, a world transform in skeleton space (the columns are the bone's x
/// axis, y axis and origin), and works out the local pose that makes the next world transform
/// computation give the same placement. The bone's parent is taken as it is now, so a parent that
/// moves in the same frame moves the bone with it.
pub fn set_bone_world_transform(skeleton: &mut BorrowedSkeleton, bone: BoneId, transform: Affine2) {
    let pose = skeleton.bone_mut(bone).applied_pose_mut();
    pose.a = transform.matrix2.x_axis.x;
    pose.c = transform.matrix2.x_axis.y;
    pose.b = transform.matrix2.y_axis.x;
    pose.d = transform.matrix2.y_axis.y;
    pose.world_x = transform.translation.x;
    pose.world_y = transform.translation.y;
    skeleton.update_bone_local_transform(bone);
    let local = skeleton.bone(bone).applied_pose().local;
    skeleton.bone_mut(bone).pose.local = local;
}

/// Places the bones that [`SpineDriver`]s drive on the transforms the drivers hold. It runs after
/// the animations posed the bones and before the world transforms are computed, for the skeletons
/// marked `M` that [`update_state`] advanced.
pub fn drive_bones<R: SpineResource, M: Component>(
    source: StaticSystemParam<R>,
    mut warned: Local<bool>,
    mut skeletons: Query<Transformed<R::Key>, With<M>>,
    drivers: Query<(&SpineDriver, &ChildOf)>,
) {
    for (driver, parent) in &drivers {
        let Ok((key, mut pose, lod)) = skeletons.get_mut(parent.parent()) else {
            continue;
        };
        if !was_updated(lod) {
            continue;
        }
        if let Some(data) = data_or_warn::<R>(&source, key.0, &mut warned) {
            set_bone_world_transform(&mut pose.as_borrow(data), driver.bone(), driver.transform);
        }
    }
}

/// What `apply_state` needs of a skeleton.
type Posed<K> = (
    &'static SpineData<K>,
    &'static mut SpineSkeleton,
    &'static mut SpineAnimation,
    Option<&'static SpineLod>,
);

/// What `update_transforms` needs of a skeleton.
type Transformed<K> = (
    &'static SpineData<K>,
    &'static mut SpineSkeleton,
    Option<&'static SpineLod>,
);

/// Whether the update stage advanced this skeleton this frame.
fn was_updated(lod: Option<&SpineLod>) -> bool {
    lod.is_none_or(SpineLod::is_due)
}

/// Poses the skeletons marked `M` that were advanced by [`update_state`] from their animation
/// tracks.
pub fn apply_state<R: SpineResource, M: Component>(
    source: StaticSystemParam<R>,
    mut warned: Local<bool>,
    mut skeletons: Query<Posed<R::Key>, With<M>>,
) {
    for (key, mut pose, mut animation, lod) in &mut skeletons {
        if !was_updated(lod) {
            continue;
        }
        if let Some(data) = data_or_warn::<R>(&source, key.0, &mut warned) {
            pose_skeleton(&mut pose, &mut animation, data);
        }
    }
}

/// Computes the world transforms of the skeletons marked `M` that were advanced by
/// [`update_state`].
pub fn update_transforms<R: SpineResource, M: Component>(
    source: StaticSystemParam<R>,
    mut warned: Local<bool>,
    mut skeletons: Query<Transformed<R::Key>, With<M>>,
) {
    for (key, mut pose, lod) in &mut skeletons {
        if !was_updated(lod) {
            continue;
        }
        if let Some(data) = data_or_warn::<R>(&source, key.0, &mut warned) {
            transform_skeleton(&mut pose, data);
        }
    }
}

/// Keeps [`SpineBounds`] equal to the drawn area of its skeleton.
pub fn update_bounds(
    mut scratch: Local<Vec<f32>>,
    mut skeletons: Query<(&SpineSkeleton, &mut SpineBounds)>,
) {
    for (pose, mut bounds) in &mut skeletons {
        **bounds = compute_bounds(pose, &mut scratch, None);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;
    use spine2d_core::{Atlas, AtlasAttachmentLoader, SkeletonJson};

    use super::*;
    use crate::queues::{SpineEvent, SpineEventChannel, SpineEventKind};
    use crate::resource::test_support::{TestSource, TestStore};
    use crate::{
        add_spine_systems, init_spine_resources, spawn_skeleton, SpineDriver, SpineFollower,
        SpineInitial, SpinePhysics, SpineProcess, SpineSchedule, SpineStep,
    };

    const SKELETON: &str = r#"{
        "skeleton": { "spine": "4.3.0" },
        "bones": [ { "name": "root" }, { "name": "arm", "parent": "root", "x": 10 } ],
        "slots": [ { "name": "hand", "bone": "arm" } ],
        "events": { "step": { "int": 3, "float": 1.5, "string": "boot", "audio": "step.wav", "volume": 0.5 } },
        "animations": {
            "turn": { "bones": { "root": { "rotate": [ { "value": 0 }, { "time": 1, "value": 90 } ] } } },
            "walk": { "events": [ { "time": 0.5, "name": "step" } ] },
            "idle": {}
        }
    }"#;

    fn skeleton_data() -> SkeletonData {
        let atlas = Atlas::parse("", "", None);
        let attachments = AtlasAttachmentLoader::new(&atlas);
        let mut loader = SkeletonJson::new(&attachments);
        loader
            .read_skeleton_data(SKELETON)
            .expect("the test skeleton is valid")
    }

    /// Spawns a skeleton entity with `initial` applied, putting the test data into the world if it
    /// has none yet.
    fn spawn_with(world: &mut World, initial: &SpineInitial) -> Entity {
        if !world.contains_resource::<TestStore>() {
            TestStore::insert(world, skeleton_data());
        }
        spawn_skeleton::<TestSource>(world, (), initial).expect("the store has the data")
    }

    /// Gives `entity` a step the way an engine integration does.
    fn step(world: &mut World, entity: Entity, delta: f32) {
        let (has_mode, earlier) = {
            let entity = world.entity(entity);
            (
                entity.contains::<SpineProcess>() || entity.contains::<SpinePhysics>(),
                entity.get::<SpineStep>().copied(),
            )
        };
        let mut queue = bevy_ecs::world::CommandQueue::default();
        begin_step(
            &mut Commands::new(&mut queue, world),
            entity,
            delta,
            has_mode,
            earlier.as_ref(),
        );
        queue.apply(world);
    }

    /// A world with every stage in one schedule, like an engine that runs the whole frame at once.
    struct Fixture {
        world: World,
        schedule: Schedule,
    }

    impl Fixture {
        fn new() -> Self {
            spine2d_core::set_y_down(false);
            let mut world = World::new();
            let mut schedule = Schedule::new(SpineSchedule);
            init_spine_resources(&mut world);
            add_spine_systems::<TestSource, SpineProcess>(&mut schedule);
            Fixture { world, schedule }
        }

        /// Spawns a skeleton that moves with the process schedule.
        fn spawn(&mut self) -> Entity {
            let entity = spawn_with(&mut self.world, &SpineInitial::default());
            self.world.entity_mut(entity).insert(SpineProcess);
            entity
        }

        fn advance(&mut self, seconds: f32) {
            self.world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(seconds));
            self.schedule.run(&mut self.world);
        }

        fn push(&mut self, entity: Entity, command: SpineCommand) {
            let store = self
                .world
                .remove_resource::<TestStore>()
                .expect("a skeleton was spawned");
            {
                let mut entity = self.world.entity_mut(entity);
                let (mut pose, mut animation) = entity
                    .get_components_mut::<(&mut SpineSkeleton, &mut SpineAnimation)>()
                    .expect("has a pose and a state");
                apply_command(
                    command,
                    &mut pose,
                    &mut animation,
                    &store.data,
                    &store.table,
                );
            }
            self.world.insert_resource(store);
        }

        fn play(&mut self, entity: Entity, name: &str, looping: bool) {
            self.push(
                entity,
                SpineCommand::SetAnimation {
                    track: 0,
                    name: name.into(),
                    looping,
                },
            );
        }

        fn root_rotation(&self, entity: Entity) -> f32 {
            let pose = self.world.get::<SpineSkeleton>(entity).expect("has a pose");
            pose.bones()[0].applied_pose().world_rotation_x()
        }

        /// Takes the events reported since the last call.
        fn events(&self) -> Vec<(Entity, SpineEvent)> {
            let mut events = Vec::new();
            self.world
                .resource::<SpineEventChannel>()
                .drain_into(&mut events);
            events
        }

        fn kinds(&self) -> Vec<SpineEventKind> {
            self.events()
                .into_iter()
                .map(|(_, event)| event.kind)
                .collect()
        }
    }

    #[test]
    fn spawned_skeletons_have_a_pose_and_their_initial_animation() {
        let mut fixture = Fixture::new();
        let entity = spawn_with(
            &mut fixture.world,
            &SpineInitial {
                animation: Some("turn".into()),
                looping: true,
                ..Default::default()
            },
        );

        let world = &fixture.world;
        assert!(world.get::<SpineSkeleton>(entity).is_some());
        let state = &world.get::<SpineAnimation>(entity).expect("has state");
        let current = state
            .track(0)
            .and_then(|id| state.entry(id))
            .expect("track 0 is playing");
        assert_eq!(current.animation().name(), "turn");
    }

    #[test]
    fn commands_are_applied_in_order() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "idle", true);
        fixture.play(entity, "turn", false);

        fixture.advance(0.0);

        let world = &fixture.world;
        let state = &world.get::<SpineAnimation>(entity).expect("has state");
        let current = state
            .track(0)
            .and_then(|id| state.entry(id))
            .expect("track 0 is playing");
        assert_eq!(current.animation().name(), "turn");
    }

    #[test]
    fn unknown_animation_is_skipped_without_stopping_later_commands() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "missing", true);
        fixture.push(
            entity,
            SpineCommand::SetAnimation {
                track: 1,
                name: "idle".into(),
                looping: true,
            },
        );

        fixture.advance(0.0);

        let state = &fixture
            .world
            .get::<SpineAnimation>(entity)
            .expect("has state")
            .0;
        assert!(state.track(0).is_none());
        assert!(state.track(1).is_some());
    }

    #[test]
    fn a_frame_moves_the_pose_with_the_frame_time() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", false);

        fixture.advance(0.0);
        let start = fixture.root_rotation(entity);
        fixture.advance(0.5);

        assert!((fixture.root_rotation(entity) - start - 45.0).abs() < 0.01);
    }

    #[test]
    fn the_time_scale_slows_the_animation_and_the_skeleton_clock_alike() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", false);
        fixture
            .world
            .get_mut::<SpineAnimation>(entity)
            .expect("has state")
            .0
            .time_scale = 0.5;
        fixture.advance(0.0);

        fixture.advance(0.5);

        let pose = &fixture
            .world
            .get::<SpineSkeleton>(entity)
            .expect("has pose");
        assert!((pose.time - 0.25).abs() < 1e-6);
        assert!((fixture.root_rotation(entity) - 22.5).abs() < 0.01);
    }

    #[test]
    fn skipped_frames_lose_no_animation_time() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        let mut lod = SpineLod::default();
        lod.set_interval(3);
        fixture.world.entity_mut(entity).insert(lod);
        fixture.play(entity, "turn", false);

        for _ in 0..5 {
            fixture.advance(0.1);
        }

        let world = &fixture.world;
        let state = &world.get::<SpineAnimation>(entity).expect("has state");
        let played = state
            .track(0)
            .and_then(|id| state.entry(id))
            .expect("playing")
            .track_time;
        let pending = world.get::<SpineLod>(entity).expect("has state").pending();
        assert!((played + pending - 0.5).abs() < 1e-5);
        assert!(played < 0.5 || pending == 0.0);
    }

    #[test]
    fn a_skipped_skeleton_is_not_posed_either() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        let mut lod = SpineLod::new(200.0, 1000);
        lod.set_interval(1000);
        fixture.world.entity_mut(entity).insert(lod);
        fixture.play(entity, "turn", false);
        fixture.advance(0.0);
        let before = fixture.root_rotation(entity);

        fixture.advance(0.5);

        let lod = fixture.world.get::<SpineLod>(entity).expect("has lod");
        assert!(!lod.is_due());
        assert_eq!(fixture.root_rotation(entity), before);
    }

    #[test]
    fn starting_an_animation_reports_a_start_event_with_its_entry() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", false);

        fixture.advance(0.0);

        let log = fixture.events();
        let (from, event) = log.first().expect("an event was reported");
        assert_eq!(*from, entity);
        assert_eq!(event.kind, SpineEventKind::Start);
        assert_eq!((event.track, event.animation.as_str()), (0, "turn"));
        let state = &fixture
            .world
            .get::<SpineAnimation>(entity)
            .expect("has state")
            .0;
        assert!(state.entry(event.entry).is_some());
    }

    #[test]
    fn finishing_an_animation_reports_complete() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", false);
        fixture.advance(0.0);

        fixture.advance(1.2);

        assert!(fixture.kinds().contains(&SpineEventKind::Complete));
    }

    #[test]
    fn replacing_an_animation_reports_interrupt() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", true);
        fixture.advance(0.1);

        fixture.play(entity, "idle", true);
        fixture.advance(0.0);

        let kinds = fixture.kinds();
        assert!(kinds.contains(&SpineEventKind::Interrupt));
        assert!(kinds.contains(&SpineEventKind::Start));
    }

    #[test]
    fn a_keyed_event_arrives_with_its_payload() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "walk", false);
        fixture.advance(0.0);

        fixture.advance(0.6);

        let custom = fixture
            .kinds()
            .into_iter()
            .find_map(|kind| match kind {
                SpineEventKind::Custom(custom) => Some(custom),
                _ => None,
            })
            .expect("the step event fired");
        assert_eq!(custom.name.as_str(), "step");
        assert_eq!(custom.int_value, 3);
        assert_eq!(custom.float_value, 1.5);
        assert_eq!(custom.string_value.as_str(), "boot");
        assert_eq!(custom.volume, 0.5);
        assert!((custom.time - 0.5).abs() < 1e-5);
    }

    #[test]
    fn drained_events_are_not_reported_again() {
        let mut fixture = Fixture::new();
        let entity = fixture.spawn();
        fixture.play(entity, "turn", true);
        fixture.advance(0.0);
        assert!(!fixture.kinds().is_empty());

        fixture.advance(0.1);

        assert!(fixture.kinds().is_empty());
    }

    #[test]
    fn events_are_attributed_to_their_own_skeleton() {
        let mut fixture = Fixture::new();
        let first = fixture.spawn();
        let second = fixture.spawn();
        fixture.play(second, "turn", true);

        fixture.advance(0.0);

        let log = fixture.events();
        assert!(!log.is_empty());
        assert!(log.iter().all(|(from, _)| *from == second));
        assert_ne!(first, second);
    }

    #[test]
    fn stages_can_be_run_one_at_a_time() {
        use crate::{add_spine_stage, SpineSystems};
        spine2d_core::set_y_down(false);
        let mut world = World::new();
        init_spine_resources(&mut world);
        let run = |stage: SpineSystems, world: &mut World| {
            let mut schedule = Schedule::default();
            add_spine_stage::<TestSource, SpineProcess>(&mut schedule, stage);
            schedule.run(world);
        };
        let entity = spawn_turning(&mut world, Some(SpineProcess));
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.5));
        let rotation = |world: &World| {
            let pose = world.get::<SpineSkeleton>(entity).expect("has a pose");
            pose.bones()[0].applied_pose().world_rotation_x()
        };
        let start = rotation(&world);

        run(SpineSystems::Update, &mut world);
        run(SpineSystems::Apply, &mut world);
        assert_eq!(rotation(&world), start);
        run(SpineSystems::Transforms, &mut world);

        assert!((rotation(&world) - start - 45.0).abs() < 0.01);
    }

    /// Spawns a skeleton turning for a second, with an optional mode marker.
    fn spawn_turning<M: Component>(world: &mut World, marker: Option<M>) -> Entity {
        let entity = spawn_with(
            world,
            &SpineInitial {
                animation: Some("turn".into()),
                ..Default::default()
            },
        );
        if let Some(marker) = marker {
            world.entity_mut(entity).insert(marker);
        }
        entity
    }

    fn rotation_of(world: &World, entity: Entity) -> f32 {
        let pose = world.get::<SpineSkeleton>(entity).expect("has a pose");
        pose.bones()[0].applied_pose().world_rotation_x()
    }

    #[test]
    fn each_schedule_moves_only_the_skeletons_of_its_mode() {
        spine2d_core::set_y_down(false);
        let mut world = World::new();
        init_spine_resources(&mut world);
        let mut process = Schedule::default();
        add_spine_systems::<TestSource, SpineProcess>(&mut process);
        let mut physics = Schedule::default();
        add_spine_systems::<TestSource, SpinePhysics>(&mut physics);
        let in_process = spawn_turning(&mut world, Some(SpineProcess));
        let in_physics = spawn_turning(&mut world, Some(SpinePhysics));
        let manual = spawn_turning::<SpineProcess>(&mut world, None);
        process.run(&mut world);
        let start = rotation_of(&world, in_process);
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.5));

        process.run(&mut world);

        assert!((rotation_of(&world, in_process) - start - 45.0).abs() < 0.01);
        assert_eq!(rotation_of(&world, in_physics), start);
        assert_eq!(rotation_of(&world, manual), start);

        physics.run(&mut world);

        assert!((rotation_of(&world, in_physics) - start - 45.0).abs() < 0.01);
        assert_eq!(rotation_of(&world, manual), start);
    }

    #[test]
    fn nothing_is_spawned_when_the_store_has_no_data() {
        let mut world = World::new();
        init_spine_resources(&mut world);
        let before = world.entities().len();

        let entity = spawn_skeleton::<TestSource>(&mut world, (), &SpineInitial::default());

        assert!(entity.is_none());
        assert_eq!(world.entities().len(), before);
    }

    #[test]
    fn a_driver_places_its_bone_and_leaves_the_others_alone() {
        let mut fixture = Fixture::new();
        let skeleton = fixture.spawn();
        let placement = Affine2::from_translation(glam::Vec2::new(50.0, 60.0));
        let driver = SpineDriver::for_bone(
            &fixture.world.resource::<TestStore>().data,
            "arm",
            placement,
        )
        .expect("the bone exists");
        fixture
            .world
            .spawn((driver, bevy_ecs::hierarchy::ChildOf(skeleton)));

        fixture.advance(0.016);

        let pose = fixture.world.get::<SpineSkeleton>(skeleton).expect("pose");
        let arm = pose.bones()[1].applied_pose();
        let root = pose.bones()[0].applied_pose();
        assert!((arm.world_x - 50.0).abs() < 0.001 && (arm.world_y - 60.0).abs() < 0.001);
        assert!(root.world_x.abs() < 0.001 && root.world_y.abs() < 0.001);
    }

    #[test]
    fn a_manual_skeleton_is_advanced_through_the_per_skeleton_functions() {
        spine2d_core::set_y_down(false);
        let mut world = World::new();
        init_spine_resources(&mut world);
        let manual = spawn_turning::<SpineProcess>(&mut world, None);
        let start = rotation_of(&world, manual);

        let store = world
            .remove_resource::<TestStore>()
            .expect("a skeleton was spawned");
        let mut entity = world.entity_mut(manual);
        let (mut pose, mut animation) = entity
            .get_components_mut::<(&mut SpineSkeleton, &mut SpineAnimation)>()
            .expect("has a pose and a state");
        advance_skeleton(&mut pose, &mut animation, 0.5);
        pose_skeleton(&mut pose, &mut animation, &store.data);
        transform_skeleton(&mut pose, &store.data);
        world.insert_resource(store);

        assert!((rotation_of(&world, manual) - start - 45.0).abs() < 0.01);
    }

    #[test]
    fn a_follower_reports_a_change_only_when_its_bone_moved() {
        let mut fixture = Fixture::new();
        let skeleton = spawn_with(
            &mut fixture.world,
            &SpineInitial {
                animation: Some("turn".into()),
                ..Default::default()
            },
        );
        fixture.world.entity_mut(skeleton).insert(SpineProcess);
        let pose = fixture.world.get::<SpineSkeleton>(skeleton).expect("pose");
        let mut follower =
            SpineFollower::for_bone(pose, &fixture.world.resource::<TestStore>().data, "arm")
                .expect("the bone exists");
        assert!(follower
            .transform()
            .translation
            .abs_diff_eq(glam::Vec2::new(10.0, 0.0), 0.01));
        assert!(!follower.follow(pose));

        fixture.advance(1.0);

        let pose = fixture.world.get::<SpineSkeleton>(skeleton).expect("pose");
        assert!(follower.follow(pose));
        assert!(follower
            .transform()
            .translation
            .abs_diff_eq(glam::Vec2::new(0.0, 10.0), 0.01));
        assert_eq!(follower.name(), "arm");
        assert!(!follower.follow(pose));
    }

    #[test]
    fn a_follower_of_a_slot_follows_the_slots_bone_and_missing_names_have_none() {
        let mut fixture = Fixture::new();
        let skeleton = fixture.spawn();
        let pose = fixture
            .world
            .get::<SpineSkeleton>(skeleton)
            .expect("has a pose");
        let data = &fixture.world.resource::<TestStore>().data;

        let by_slot = SpineFollower::for_slot(pose, data, "hand").expect("the slot exists");

        assert_eq!(by_slot.transform().translation.x, 10.0);
        assert!(SpineFollower::for_slot(pose, data, "arm").is_none());
        assert!(SpineFollower::for_bone(pose, data, "hand").is_none());
    }

    #[test]
    fn steps_move_a_skeleton_no_frame_moves_once_and_summed_and_leave_it_manual() {
        let mut fixture = Fixture::new();
        let manual = spawn_turning::<SpineProcess>(&mut fixture.world, None);
        fixture.advance(0.0);
        let start = rotation_of(&fixture.world, manual);
        step(&mut fixture.world, manual, 0.25);
        step(&mut fixture.world, manual, 0.25);

        fixture.advance(1.0);

        assert!((rotation_of(&fixture.world, manual) - start - 45.0).abs() < 0.01);
        let entity = fixture.world.entity(manual);
        assert!(!entity.contains::<SpineStep>());
        assert!(!entity.contains::<SpineProcess>());
        fixture.advance(1.0);
        assert!((rotation_of(&fixture.world, manual) - start - 45.0).abs() < 0.01);
    }

    #[test]
    fn a_step_adds_to_the_frame_time_of_a_skeleton_that_has_a_mode() {
        let mut fixture = Fixture::new();
        let moving = spawn_turning(&mut fixture.world, Some(SpineProcess));
        fixture.advance(0.0);
        let start = rotation_of(&fixture.world, moving);
        step(&mut fixture.world, moving, 0.25);

        fixture.advance(0.25);

        assert!((rotation_of(&fixture.world, moving) - start - 45.0).abs() < 0.01);
        let entity = fixture.world.entity(moving);
        assert!(!entity.contains::<SpineStep>());
        assert!(entity.contains::<SpineProcess>());
    }
}
