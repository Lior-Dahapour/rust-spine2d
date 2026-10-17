//! The one ECS world of the extension, and how the rest of the extension reaches it.
//!
//! Godot calls the extension on one thread, so the world lives in a thread-local cell. Every
//! access is a short closure ([`with`]); nothing may call back into Godot from inside one, because
//! a script reacting to that call would try to open the world again.

use std::cell::RefCell;
use std::time::Duration;

use godot::classes::Node2D;
use godot::global::godot_error;
use godot::obj::Gd;
use spine2d_core::animation_state::BorrowedAnimationState;
use spine2d_core::skeleton::{BorrowedSkeleton, BorrowedSkeletonRef};
use spine2d_core::{AnimationState, AnimationStateData, SkeletonData};
use spine2d_ecs::bevy_ecs::change_detection::{DetectChangesMut, Mut};
use spine2d_ecs::bevy_ecs::component::{Component, Mutable};
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use spine2d_ecs::bevy_ecs::world::World;
use spine2d_ecs::bevy_time::Time;
use spine2d_ecs::systems::{apply_state, update_state};
use spine2d_ecs::{
    add_spine_stage, init_spine_resources, SpineAnimation, SpineCacheStale, SpineData,
    SpinePhysics, SpineProcess, SpineSkeleton, SpineSystems,
};
use thiserror::Error;

use crate::nodes::sprite::SpineSprite;
use crate::resources::callbacks::SpineCallbacks;
use crate::resources::godot_nodes::GodotNodes;
use crate::resources::steps::GodotSteps;
use crate::runtime::store::{GodotSource, Owner, SkeletonDataId, SkeletonStore, StoredSkeleton};
use crate::systems::follow::follow_bones;
use crate::systems::queue::{queue_events, queue_sprites};
use crate::systems::render::materials::{Materials, TextureAlpha};
use crate::systems::render::{render_skeletons, RenderStore, SpineRender};
use crate::systems::steps::apply_godot_steps;

/// Why the world could not be opened.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorldError {
    /// Something is using the world right now, usually a script called back in from a signal that
    /// was emitted while the world was open.
    #[error("the Spine world is already in use")]
    Busy,
    /// The extension has not been initialised yet, or is shutting down.
    #[error("the Spine world does not exist")]
    Missing,
}

/// Which of Godot's two update calls a run of the stages belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The rendered frame, `_process`.
    Process,
    /// The physics tick, `_physics_process`.
    Physics,
}

/// The ECS world and the schedules that advance it, one per stage so that Godot can run a stage,
/// emit signals, and run the next. Each mode has its own set, moving only the skeletons with its
/// marker.
pub struct SpineWorld {
    world: World,
    process: [Schedule; SpineSystems::ALL.len()],
    physics: [Schedule; SpineSystems::ALL.len()],
    /// Draws what changed; run once per frame, just before Godot draws.
    render: Schedule,
}

/// One schedule per stage, moving the skeletons marked `M`. The stages that Godot reacts to also
/// queue what it has to do once they are done.
fn stages<M: Component>() -> [Schedule; SpineSystems::ALL.len()] {
    SpineSystems::ALL.map(|stage| {
        let mut schedule = Schedule::default();
        add_spine_stage::<GodotSource, M>(&mut schedule, stage);
        match stage {
            SpineSystems::Commands => {
                schedule.add_systems((apply_godot_steps, queue_sprites::<M>).chain());
            }
            SpineSystems::Update => {
                schedule.add_systems(queue_sprites::<M>.after(update_state::<M>));
            }
            SpineSystems::Apply => {
                schedule.add_systems(queue_events.after(apply_state::<GodotSource, M>));
                schedule.add_systems(queue_sprites::<M>.after(queue_events));
            }
            SpineSystems::Follow => {
                schedule.add_systems(follow_bones::<M>);
                schedule.add_systems(queue_sprites::<M>.after(follow_bones::<M>));
            }
            SpineSystems::Transforms | SpineSystems::Bounds | SpineSystems::Cleanup => {}
        }
        schedule
    })
}

impl SpineWorld {
    fn new() -> Self {
        let mut world = World::new();
        init_spine_resources(&mut world);
        world.insert_non_send(GodotNodes::default());
        world.insert_non_send(SpineCallbacks::default());
        world.insert_non_send(RenderStore::default());
        world.init_resource::<GodotSteps>();
        world.init_resource::<SkeletonStore>();
        world.insert_resource(Materials::default());
        world.insert_resource(TextureAlpha::default());
        let mut render = Schedule::default();
        render.add_systems(render_skeletons);
        SpineWorld {
            world,
            process: stages::<SpineProcess>(),
            physics: stages::<SpinePhysics>(),
            render,
        }
    }

    /// The world, for spawning entities and changing components.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Sets the time the next stages see as this frame's delta, in seconds.
    pub fn advance(&mut self, delta: f32) {
        self.world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(delta.max(0.0)));
    }

    /// Makes `sprite` the node of the skeleton `entity`.
    pub fn add_sprite(&mut self, entity: Entity, sprite: Gd<SpineSprite>) {
        self.world
            .non_send_mut::<GodotNodes>()
            .add_sprite(entity, sprite);
    }

    /// Forgets the sprite of the skeleton `entity`.
    pub fn remove_sprite(&mut self, entity: Entity) {
        self.world
            .non_send_mut::<GodotNodes>()
            .remove_sprite(entity);
    }

    /// The sprite node of the skeleton `entity`.
    pub fn sprite(&self, entity: Entity) -> Option<Gd<SpineSprite>> {
        self.world.non_send::<GodotNodes>().sprite(entity).cloned()
    }

    /// Runs `f` with the drawing state of the skeleton `entity`.
    pub fn render<R>(
        &mut self,
        entity: Entity,
        f: impl FnOnce(&mut SpineRender) -> R,
    ) -> Option<R> {
        self.world
            .non_send_mut::<RenderStore>()
            .get_mut(entity)
            .map(f)
    }

    /// Gives the skeleton `entity` its drawing state.
    pub fn add_render(&mut self, entity: Entity, render: SpineRender) {
        self.world
            .non_send_mut::<RenderStore>()
            .insert(entity, render);
    }

    /// Drops the drawing state of the skeleton `entity`, which frees its meshes.
    pub fn remove_render(&mut self, entity: Entity) {
        self.world.non_send_mut::<RenderStore>().remove(entity);
    }

    /// Records whether the texture with the render server handle `handle` has premultiplied alpha.
    pub fn register_texture_alpha(&mut self, handle: usize, premultiplied: bool) {
        self.world
            .resource_mut::<TextureAlpha>()
            .register(handle, premultiplied);
    }

    /// Asks the skeleton `entity` to advance by `delta` seconds at the next frame, whatever its
    /// mode.
    pub fn step_skeleton(&mut self, entity: Entity, delta: f32) {
        self.world.resource_mut::<GodotSteps>().step(entity, delta);
    }

    /// Tells the render server something changed; see [`RenderStore::ping`].
    pub fn ping_render(&mut self) {
        self.world.non_send_mut::<RenderStore>().ping();
    }

    /// Draws every skeleton that changed since the last call.
    pub fn run_render(&mut self) {
        self.render.run(&mut self.world);
    }

    /// Runs one stage over the skeletons of `frame`.
    pub fn run(&mut self, frame: Frame, stage: SpineSystems) {
        let schedules = match frame {
            Frame::Process => &mut self.process,
            Frame::Physics => &mut self.physics,
        };
        schedules[stage as usize].run(&mut self.world);
    }

    /// Hands the callbacks queued since the last call to `out`, which must be empty.
    pub fn swap_callbacks(&mut self, out: &mut SpineCallbacks) {
        self.world.non_send_mut::<SpineCallbacks>().swap(out);
    }

    /// Makes `node` the one placed by the follower `entity`.
    pub fn add_follower(&mut self, entity: Entity, node: Gd<Node2D>) {
        self.world
            .non_send_mut::<GodotNodes>()
            .add_follower(entity, node);
    }

    /// Forgets the node of the follower `entity` and removes the follower if it still exists.
    pub fn remove_follower(&mut self, entity: Entity) {
        self.world
            .non_send_mut::<GodotNodes>()
            .remove_follower(entity);
        if let Ok(follower) = self.world.get_entity_mut(entity) {
            follower.despawn();
        }
    }
}

thread_local! {
    static WORLD: RefCell<Option<SpineWorld>> = const { RefCell::new(None) };
    /// Data whose last user went away, waiting to be taken out of the store. The user is dropped
    /// from inside the world (a sprite releasing its skeleton), where the store cannot be opened.
    static REMOVED_DATA: RefCell<Vec<SkeletonDataId>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with the drawing state of the skeleton `entity`; `None` if the world is busy or
/// missing, or the entity has none.
pub fn render<R>(entity: Entity, f: impl FnOnce(&mut SpineRender) -> R) -> Option<R> {
    with(|spine| spine.render(entity, f)).ok().flatten()
}

/// Records whether the texture with the render server handle `handle` has premultiplied alpha.
pub fn register_texture_alpha(handle: usize, premultiplied: bool) {
    if let Err(error) = with(|spine| spine.register_texture_alpha(handle, premultiplied)) {
        godot_error!("Spine: {error}");
    }
}

/// Asks the skeleton `entity` to advance by `delta` seconds at the next frame, whatever its mode.
/// Steps asked for before that frame add up into one.
pub fn step(entity: Entity, delta: f32) {
    if let Err(error) = with(|spine| spine.step_skeleton(entity, delta)) {
        godot_error!("Spine: {error}");
    }
}

/// The sprite node of the skeleton `entity`, if the world has one for it.
pub fn sprite(entity: Entity) -> Option<Gd<SpineSprite>> {
    with(|spine| spine.sprite(entity)).ok().flatten()
}

/// Creates the world. Does nothing if it exists.
pub fn create() {
    WORLD.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(SpineWorld::new());
        }
    });
}

/// Drops the world now, instead of leaving it to thread-local destructors at thread exit, which
/// run after Godot has shut down. Anything in it that frees Godot objects must be freed here.
pub fn destroy() {
    // Take it out first so the world is dropped with the cell no longer borrowed.
    let world = WORLD.with(|cell| cell.borrow_mut().take());
    drop(world);
}

/// Runs `f` with the world.
///
/// # Errors
///
/// [`WorldError::Busy`] if the world is already open, [`WorldError::Missing`] if it was not
/// created.
pub fn with<R>(f: impl FnOnce(&mut SpineWorld) -> R) -> Result<R, WorldError> {
    WORLD.with(|cell| {
        let mut slot = cell.try_borrow_mut().map_err(|_| WorldError::Busy)?;
        slot.as_mut().map(f).ok_or(WorldError::Missing)
    })
}

/// Runs `f` with the world for the script-facing code: `None` when the world is missing (the
/// extension is shutting down) or `f` found nothing, and an error in the log when it is busy.
fn access<R>(f: impl FnOnce(&mut SpineWorld) -> Option<R>) -> Option<R> {
    match with(f) {
        Ok(found) => found,
        Err(WorldError::Missing) => None,
        Err(error @ WorldError::Busy) => {
            godot_error!("Spine: {error}");
            None
        }
    }
}

/// Reads component `C` of `entity`. `None` if the entity is gone.
pub fn read<C: Component, R>(entity: Entity, f: impl FnOnce(&C) -> R) -> Option<R> {
    access(|spine| spine.world.get::<C>(entity).map(f))
}

/// Changes component `C` of `entity`. `None` if the entity is gone.
pub fn write<C: Component<Mutability = Mutable>, R>(
    entity: Entity,
    f: impl FnOnce(&mut C) -> R,
) -> Option<R> {
    access(|spine| spine.world.get_mut::<C>(entity).map(|mut c| f(&mut c)))
}

/// Stores `data` with its `mixes`; `None` if the world is missing or busy.
pub fn register_data(data: SkeletonData, mixes: AnimationStateData) -> Option<SkeletonDataId> {
    access(|spine| {
        let mut store = spine.world.resource_mut::<SkeletonStore>();
        REMOVED_DATA.with_borrow_mut(|removed| {
            for id in removed.drain(..) {
                store.remove(id);
            }
        });
        Some(store.insert(data, mixes))
    })
}

/// Marks the data under `id` as no longer used. It leaves the store the next time data is
/// registered. Safe to call from anywhere, including from inside the world and while the extension
/// shuts down.
pub fn remove_data(id: SkeletonDataId) {
    let _ = REMOVED_DATA.try_with(|removed| removed.borrow_mut().push(id));
}

/// Reads the stored data under `id`. `None` if it is gone.
pub fn data<R>(id: SkeletonDataId, f: impl FnOnce(&StoredSkeleton) -> R) -> Option<R> {
    access(|spine| spine.world.resource::<SkeletonStore>().get(id).map(f))
}

/// Replaces the mix settings stored under `id` with those `make` builds from its data. `None` if
/// the data is gone.
pub fn edit_mixes(
    id: SkeletonDataId,
    make: impl FnOnce(&SkeletonData) -> AnimationStateData,
) -> Option<()> {
    data_mut(id, |stored| stored.mixes = make(&stored.data))
}

/// Changes the stored data under `id`. `None` if it is gone.
pub fn data_mut<R>(id: SkeletonDataId, f: impl FnOnce(&mut StoredSkeleton) -> R) -> Option<R> {
    access(|spine| {
        let mut store = spine.world.resource_mut::<SkeletonStore>();
        store.get_mut(id).map(f)
    })
}

/// The id of the data the skeleton `entity` was made from. `None` if the entity is gone.
pub fn data_id(entity: Entity) -> Option<SkeletonDataId> {
    read::<SpineData<SkeletonDataId>, _>(entity, |key| key.0)
}

/// Reads the skeleton data of `owner`. `None` if the skeleton or the data is gone.
pub fn owner_data<R>(owner: impl Into<Owner>, f: impl FnOnce(&SkeletonData) -> R) -> Option<R> {
    data(owner.into().data_id()?, |stored| f(&stored.data))
}

/// Changes the skeleton data of `owner`, for values that need no rebuild of the skeletons' update
/// order. `None` if the skeleton or the data is gone.
pub fn edit_data(owner: impl Into<Owner>, f: impl FnOnce(&mut SkeletonData)) -> Option<()> {
    data_mut(owner.into().data_id()?, |stored| f(&mut stored.data))
}

/// Changes the skeleton data of `owner` in a way that affects which bones and constraints take
/// part and in what order (a target, a bone, `skin_required`), and flags every skeleton made from
/// that data for a cache rebuild at the next frame. `None` if the skeleton or the data is gone.
pub fn edit_data_structure(
    owner: impl Into<Owner>,
    f: impl FnOnce(&mut SkeletonData),
) -> Option<()> {
    let id = owner.into().data_id()?;
    data_mut(id, |stored| f(&mut stored.data))?;
    mark_cache_stale_for_data(id);
    Some(())
}

/// Flags every skeleton made from the data `id` for a rebuild of its update order. This walks all
/// skeletons, so only structural data changes call it, which are rare.
pub fn mark_cache_stale_for_data(id: SkeletonDataId) {
    access(|spine| {
        let mut skeletons = spine
            .world
            .query::<(&SpineData<SkeletonDataId>, &mut SpineCacheStale)>();
        for (key, mut stale) in skeletons.iter_mut(&mut spine.world) {
            if key.0 == id {
                stale.set_changed();
            }
        }
        Some(())
    });
}

/// Reads the skeleton of `entity` together with its data.
pub fn skeleton<R>(entity: Entity, f: impl FnOnce(&BorrowedSkeletonRef) -> R) -> Option<R> {
    access(|spine| {
        let store = spine.world.resource::<SkeletonStore>();
        let entity = spine.world.get_entity(entity).ok()?;
        let pose = entity.get::<SpineSkeleton>()?;
        let key = entity.get::<SpineData<SkeletonDataId>>()?;
        let stored = store.get(key.0)?;
        Some(f(&pose.as_borrow_ref(&stored.data)))
    })
}

/// Changes the skeleton of `entity` together with its data.
pub fn skeleton_mut<R>(entity: Entity, f: impl FnOnce(&mut BorrowedSkeleton) -> R) -> Option<R> {
    access(|spine| {
        spine
            .world
            .resource_scope(|world, store: Mut<SkeletonStore>| {
                let mut entity = world.get_entity_mut(entity).ok()?;
                let (mut pose, key) = entity
                    .get_components_mut::<(&mut SpineSkeleton, &SpineData<SkeletonDataId>)>()
                    .ok()?;
                let stored = store.get(key.0)?;
                let result = f(&mut pose.as_borrow(&stored.data));
                Some(result)
            })
    })
}

/// Reads the animation state of `entity`. It needs no data, so it is the state itself.
pub fn state<R>(entity: Entity, f: impl FnOnce(&AnimationState) -> R) -> Option<R> {
    read::<SpineAnimation, _>(entity, |animation| f(animation))
}

/// Changes the animation state of `entity` together with its mix settings and skeleton data.
pub fn state_mut<R>(entity: Entity, f: impl FnOnce(&mut BorrowedAnimationState) -> R) -> Option<R> {
    access(|spine| {
        spine
            .world
            .resource_scope(|world, store: Mut<SkeletonStore>| {
                let mut entity = world.get_entity_mut(entity).ok()?;
                let (mut animation, key) = entity
                    .get_components_mut::<(&mut SpineAnimation, &SpineData<SkeletonDataId>)>()
                    .ok()?;
                let stored = store.get(key.0)?;
                let result = f(&mut animation.as_borrow(&stored.mixes, &stored.data));
                Some(result)
            })
    })
}

/// Changes the skeleton of `entity` and hands `f` its [`SpineCacheStale`] marker, which `f` flags
/// with `set_changed()` only when it changed what a constraint refers to. One entity lookup serves
/// both components.
pub fn constraints_mut<R>(
    entity: Entity,
    f: impl FnOnce(&mut BorrowedSkeleton, &mut Mut<SpineCacheStale>) -> R,
) -> Option<R> {
    access(|spine| {
        spine
            .world
            .resource_scope(|world, store: Mut<SkeletonStore>| {
                let mut entity = world.get_entity_mut(entity).ok()?;
                let (mut pose, mut stale, key) = entity
                    .get_components_mut::<(
                        &mut SpineSkeleton,
                        &mut SpineCacheStale,
                        &SpineData<SkeletonDataId>,
                    )>()
                    .ok()?;
                let stored = store.get(key.0)?;
                let result = f(&mut pose.as_borrow(&stored.data), &mut stale);
                Some(result)
            })
    })
}

/// Changes the skeleton and the animation state of `entity` together, with the skeleton's data.
pub fn pose_mut<R>(
    entity: Entity,
    f: impl FnOnce(&mut BorrowedSkeleton, &mut AnimationState) -> R,
) -> Option<R> {
    access(|spine| {
        spine
            .world
            .resource_scope(|world, store: Mut<SkeletonStore>| {
                let mut entity = world.get_entity_mut(entity).ok()?;
                let (mut pose, mut animation, key) = entity
                    .get_components_mut::<(
                        &mut SpineSkeleton,
                        &mut SpineAnimation,
                        &SpineData<SkeletonDataId>,
                    )>()
                    .ok()?;
                let stored = store.get(key.0)?;
                let result = f(&mut pose.as_borrow(&stored.data), &mut animation);
                Some(result)
            })
    })
}

#[cfg(test)]
mod tests {
    use spine2d_core::{Atlas, AtlasAttachmentLoader, SkeletonJson};
    use spine2d_ecs::{spawn_skeleton as spawn, SpineInitial};

    use super::*;

    fn skeleton_data() -> SkeletonData {
        let atlas = Atlas::parse("", "", None);
        let attachments = AtlasAttachmentLoader::new(&atlas);
        SkeletonJson::new(&attachments)
            .read_skeleton_data(
                r#"{ "skeleton": { "spine": "4.3.0" }, "bones": [ { "name": "root" } ],
                    "animations": { "idle": {} } }"#,
            )
            .expect("the test skeleton is valid")
    }

    /// A skeleton in neither mode, which only moves when stepped.
    #[derive(Component)]
    struct Manual;

    fn spawn_skeleton(spine: &mut SpineWorld, marker: impl Component) -> Entity {
        let id = spine
            .world_mut()
            .resource_mut::<SkeletonStore>()
            .insert(skeleton_data(), AnimationStateData::detached());
        let entity = spawn::<GodotSource>(spine.world_mut(), id, &SpineInitial::default())
            .expect("the data was just stored");
        spine.world_mut().entity_mut(entity).insert(marker);
        entity
    }

    #[test]
    fn the_world_is_missing_until_created_and_after_destroyed() {
        destroy();
        assert_eq!(with(|_| ()), Err(WorldError::Missing));

        create();
        assert_eq!(with(|_| ()), Ok(()));

        destroy();
        assert_eq!(with(|_| ()), Err(WorldError::Missing));
    }

    #[test]
    fn opening_the_world_inside_itself_reports_busy_instead_of_panicking() {
        create();

        let inner = with(|_| with(|_| ())).expect("the outer access works");

        assert_eq!(inner, Err(WorldError::Busy));
        destroy();
    }

    #[test]
    fn the_commands_stage_drains_the_queued_steps() {
        create();
        let drained = with(|spine| {
            let entity = spawn_skeleton(spine, SpineProcess);
            spine.step_skeleton(entity, 0.5);
            spine.run(Frame::Process, SpineSystems::Commands);
            spine.world_mut().resource::<GodotSteps>().is_empty()
        })
        .expect("the world exists");

        assert!(drained);
        destroy();
    }

    #[test]
    fn a_step_moves_a_manual_skeleton_in_the_next_frame() {
        create();
        let entity = with(|spine| {
            let entity = spawn_skeleton(spine, Manual);
            spine.step_skeleton(entity, 0.5);
            spine.advance(0.016);
            for stage in [SpineSystems::Commands, SpineSystems::Update] {
                spine.run(Frame::Process, stage);
            }
            entity
        })
        .expect("the world exists");

        assert_eq!(skeleton(entity, |s| s.time), Some(0.5));
        destroy();
    }

    #[test]
    fn each_frame_moves_only_the_skeletons_of_its_mode() {
        create();
        let (process, physics) = with(|spine| {
            let process = spawn_skeleton(spine, SpineProcess);
            let physics = spawn_skeleton(spine, SpinePhysics);
            spine.advance(0.5);
            spine.run(Frame::Process, SpineSystems::Update);
            (process, physics)
        })
        .expect("the world exists");

        let time = |entity| skeleton(entity, |skeleton| skeleton.time).expect("has a pose");

        assert_eq!(time(process), 0.5);
        assert_eq!(time(physics), 0.0);
        destroy();
    }

    #[test]
    fn accessors_find_nothing_for_a_despawned_entity() {
        create();
        let entity = with(|spine| spawn_skeleton(spine, SpineProcess)).expect("the world exists");
        assert!(state(entity, |_| ()).is_some());

        with(|spine| spine.world_mut().despawn(entity)).expect("the world exists");

        assert!(state(entity, |_| ()).is_none());
        assert!(pose_mut(entity, |_, _| ()).is_none());
        destroy();
    }
}
