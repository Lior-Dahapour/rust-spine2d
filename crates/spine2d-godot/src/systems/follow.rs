//! The nodes that follow bones: creating and removing their follower entities in the world, and
//! placing the nodes once the world has computed where they go.

use godot::classes::Node2D;
use godot::prelude::*;
use spine2d_core::skeleton::BorrowedSkeletonRef;
use spine2d_core::SkeletonData;
use spine2d_ecs::bevy_ecs::component::Component;
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::bevy_ecs::hierarchy::ChildOf;
use spine2d_ecs::bevy_ecs::query::With;
use spine2d_ecs::bevy_ecs::system::{NonSend, NonSendMut, Query};
use spine2d_ecs::glam::{Affine2, Vec2};
use spine2d_ecs::{SpineData, SpineDriver, SpineFollower, SpineSkeleton};

use crate::resources::callbacks::{QueuedTransform, SpineCallbacks};
use crate::resources::godot_nodes::GodotNodes;
use crate::runtime::store::{SkeletonDataId, SkeletonStore};
use crate::runtime::world;

/// Places the followers of the skeletons marked `M` on their bones, and queues the nodes of the
/// ones whose bone moved for [`trigger_transform_changes`].
pub fn follow_bones<M: Component>(
    nodes: NonSend<GodotNodes>,
    mut callbacks: NonSendMut<SpineCallbacks>,
    skeletons: Query<&SpineSkeleton, With<M>>,
    mut followers: Query<(Entity, &mut SpineFollower, &ChildOf)>,
) {
    for (entity, mut follower, parent) in &mut followers {
        let Ok(skeleton) = skeletons.get(parent.parent()) else {
            continue;
        };
        if !follower.follow(skeleton) {
            continue;
        }
        if let Some(node) = nodes.follower(entity) {
            callbacks.push_transform(node.clone(), follower.transform());
        }
    }
}

/// Spawns a follower for `node` as a child of the skeleton entity `parent` and places the node on
/// it at once. `make` builds the follower from the skeleton; the result is `None` when the
/// skeleton is gone or `make` finds no such bone.
pub fn spawn<T: GodotClass + Inherits<Node2D>>(
    parent: Entity,
    node: &Gd<T>,
    make: impl FnOnce(&BorrowedSkeletonRef) -> Option<SpineFollower>,
) -> Option<Entity> {
    let mut node = node.clone().upcast::<Node2D>();
    let spawned = world::with(|spine| {
        let follower = {
            let world = spine.world_mut();
            let skeleton = world.get::<SpineSkeleton>(parent)?;
            let key = world.get::<SpineData<SkeletonDataId>>(parent)?;
            let stored = world.resource::<SkeletonStore>().get(key.0)?;
            make(&skeleton.as_borrow_ref(&stored.data))?
        };
        let placement = follower.transform();
        let entity = spine.world_mut().spawn((follower, ChildOf(parent))).id();
        spine.add_follower(entity, node.clone());
        Some((entity, placement))
    })
    .ok()
    .flatten()?;
    node.set_transform(to_transform(spawned.1));
    Some(spawned.0)
}

/// Spawns a driver as a child of the skeleton entity `parent`: the bone it names takes the
/// placement it holds, every frame. `make` builds the driver from the skeleton data; the result is
/// `None` when the skeleton is gone or `make` finds no such bone.
pub fn spawn_driver(
    parent: Entity,
    make: impl FnOnce(&SkeletonData) -> Option<SpineDriver>,
) -> Option<Entity> {
    world::with(|spine| {
        let driver = {
            let world = spine.world_mut();
            let key = world.get::<SpineData<SkeletonDataId>>(parent)?;
            let stored = world.resource::<SkeletonStore>().get(key.0)?;
            make(&stored.data)?
        };
        Some(spine.world_mut().spawn((driver, ChildOf(parent))).id())
    })
    .ok()
    .flatten()
}

/// Gives the driver `driver` a new placement for the bone it drives.
pub fn drive(driver: Entity, placement: Affine2) {
    world::write::<SpineDriver, _>(driver, |driver| driver.transform = placement);
}

/// The bone placement for a Godot transform: the inverse of the placement of a follower node,
/// y axis included.
pub fn from_transform(transform: Transform2D) -> Affine2 {
    Affine2::from_cols(
        Vec2::new(transform.a.x, transform.a.y),
        Vec2::new(-transform.b.x, -transform.b.y),
        Vec2::new(transform.origin.x, transform.origin.y),
    )
}

/// Removes a follower and forgets its node. Does nothing if the follower is already gone, which
/// is the case when its skeleton was removed first.
pub fn despawn(follower: Entity) {
    // The world is gone while the extension shuts down; there is nothing to remove then.
    let _ = world::with(|spine| spine.remove_follower(follower));
}

/// Places the nodes whose bones moved. The world must be closed: a script reacting to the new
/// transform can call back into its sprite. A handler must not `free()` a node of the queue; see
/// the warning in [`crate::systems::signals`].
pub fn trigger_transform_changes(queue: &mut [QueuedTransform]) {
    for QueuedTransform { node, placement } in queue {
        node.set_transform(to_transform(*placement));
    }
}

/// The Godot transform for a bone placement: columns are the bone's x axis, y axis and origin.
/// The skeleton is y-down, so the bone's local space is y-up and its y axis points up; a node's
/// local space is y-down, so the axis is turned around, as the official plugin does.
fn to_transform(placement: Affine2) -> Transform2D {
    let [x, y, origin] = [
        placement.matrix2.x_axis,
        placement.matrix2.y_axis,
        placement.translation,
    ];
    Transform2D::from_cols(
        Vector2::new(x.x, x.y),
        Vector2::new(-y.x, -y.y),
        Vector2::new(origin.x, origin.y),
    )
}
