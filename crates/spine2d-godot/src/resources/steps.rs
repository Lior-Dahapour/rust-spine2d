//! The manual steps requested for skeletons, in one queue for the whole world. Godot calls us one
//! at a time from one thread, so there is nothing to gain from a queue per skeleton, and the usual
//! case is that nothing is waiting at all.

use spine2d_ecs::bevy_ecs::entity::{Entity, EntityHashMap};
use spine2d_ecs::bevy_ecs::resource::Resource;

/// What is waiting to be applied at the next `Commands` stage.
#[derive(Resource, Default)]
pub struct GodotSteps {
    steps: EntityHashMap<f32>,
}

impl GodotSteps {
    /// Asks the skeleton `entity` to advance by `delta` seconds at the next frame, whatever its
    /// mode. Steps requested before that frame add up into one.
    pub fn step(&mut self, entity: Entity, delta: f32) {
        *self.steps.entry(entity).or_insert(0.0) += delta;
    }

    /// Whether nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Removes and returns the summed step of each skeleton that has one.
    pub fn drain(&mut self) -> impl Iterator<Item = (Entity, f32)> + '_ {
        self.steps.drain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("a valid index")
    }

    #[test]
    fn a_new_queue_is_empty() {
        assert!(GodotSteps::default().is_empty());
    }

    #[test]
    fn steps_of_one_skeleton_add_up_and_others_stay_apart() {
        let mut queue = GodotSteps::default();
        queue.step(entity(1), 0.25);
        queue.step(entity(2), 1.0);
        queue.step(entity(1), 0.25);

        let mut steps: Vec<_> = queue.drain().collect();
        steps.sort_by_key(|(entity, _)| entity.index());

        assert_eq!(steps, vec![(entity(1), 0.5), (entity(2), 1.0)]);
        assert!(queue.is_empty());
    }
}
