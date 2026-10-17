//! `SpineSkeleton`: the skeleton of a sprite, as the official API names it. It holds only the
//! entity, so it is as cheap as the other handles. It gives access to the bones, slots and
//! constraints, the skin, the setup pose and the placement of the skeleton.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BoneId, ConstraintData, Physics, SlotId};
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_render::compute_bounds;

use super::bone::SpineBone;
use super::constraints::{
    SpineIkConstraint, SpinePathConstraint, SpinePhysicsConstraint, SpineSlider,
    SpineTransformConstraint,
};
use super::slot::SpineSlot;
use crate::constants::SpineConstant;
use crate::resources::attachment::SpineAttachment;
use crate::resources::skin::SpineSkin;
use crate::resources::SpineSkeletonDataResource;
use crate::runtime::world;

/// The physics mode for the script value `value`, one of `SpineConstant.Physics_*`.
fn physics_from(value: i32) -> Option<Physics> {
    match i64::from(value) {
        SpineConstant::Physics_None => Some(Physics::None),
        SpineConstant::Physics_Reset => Some(Physics::Reset),
        SpineConstant::Physics_Update => Some(Physics::Update),
        SpineConstant::Physics_Pose => Some(Physics::Pose),
        _ => None,
    }
}

/// The bounding rectangle of everything the skeleton of `entity` draws, in its local coordinates,
/// or an empty rectangle if it draws nothing.
pub fn bounds_of(entity: Entity) -> Rect2 {
    let Some(bounds) = world::skeleton(entity, |skeleton| {
        compute_bounds(skeleton, &mut Vec::new(), None)
    }) else {
        return Rect2::default();
    };
    if bounds.width < 0.0 {
        return Rect2::default();
    }
    Rect2::new(
        Vector2::new(bounds.x, bounds.y),
        Vector2::new(bounds.width, bounds.height),
    )
}

/// The skeleton of a sprite: its bones, slots and constraints.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSkeleton {
    base: Base<RefCounted>,
    entity: Entity,
}

impl SpineSkeleton {
    /// A handle to the skeleton of the entity `entity`.
    pub fn create(entity: Entity) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSkeleton { base, entity })
    }

    /// The skeleton entity this handle points at.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// The index of the constraint called `name`, if `keep` accepts its kind.
    fn find_constraint(&self, name: &GString, keep: fn(&ConstraintData) -> bool) -> Option<usize> {
        world::skeleton(self.entity, |skeleton| {
            skeleton
                .find_constraint(&name.to_string())
                .filter(|&index| keep(&skeleton.data().constraints[index]))
        })
        .flatten()
    }

    /// The handles of every constraint whose kind `keep` accepts, in skeleton order.
    fn constraints_of<T: GodotClass>(
        &self,
        keep: fn(&ConstraintData) -> bool,
        create: fn(Entity, usize) -> Gd<T>,
    ) -> Array<Gd<T>> {
        let indices: Vec<usize> = world::skeleton(self.entity, |skeleton| {
            skeleton
                .data()
                .constraints
                .iter()
                .enumerate()
                .filter(|(_, data)| keep(data))
                .map(|(index, _)| index)
                .collect()
        })
        .unwrap_or_default();
        indices
            .into_iter()
            .map(|index| create(self.entity, index))
            .collect()
    }
}

#[godot_api]
impl SpineSkeleton {
    /// False once the sprite this skeleton came from was reloaded or freed.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    /// Computes the world transforms of the bones now and applies the constraints. `physics` is
    /// one of `SpineConstant.Physics_*`. The sprite does this itself each frame.
    #[func]
    fn update_world_transform(&mut self, physics: i32) {
        let Some(physics) = physics_from(physics) else {
            godot_warn!("Spine: {physics} is not a physics mode (0 to 3)");
            return;
        };
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.update_world_transform(physics)
        });
    }

    /// Puts bones, constraints, slots and the draw order back to the setup pose.
    #[func]
    fn set_to_setup_pose(&mut self) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.setup_pose());
    }

    /// Puts bones and constraints back to the setup pose.
    #[func]
    fn set_bones_to_setup_pose(&mut self) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.setup_pose_bones());
    }

    /// Puts slots and the draw order back to the setup pose.
    #[func]
    fn set_slots_to_setup_pose(&mut self) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.setup_pose_slots());
    }

    /// The bone called `bone_name`, or null.
    #[func]
    fn find_bone(&self, bone_name: GString) -> Option<Gd<SpineBone>> {
        let id = world::skeleton(self.entity, |skeleton| {
            skeleton.find_bone(&bone_name.to_string())
        })??;
        Some(SpineBone::create(self.entity, id))
    }

    /// The slot called `slot_name`, or null.
    #[func]
    fn find_slot(&self, slot_name: GString) -> Option<Gd<SpineSlot>> {
        let id = world::skeleton(self.entity, |skeleton| {
            skeleton.find_slot(&slot_name.to_string())
        })??;
        Some(SpineSlot::create(self.entity, id))
    }

    /// The root bone, or null.
    #[func]
    fn get_root_bone(&self) -> Option<Gd<SpineBone>> {
        world::skeleton(self.entity, |skeleton| !skeleton.bones().is_empty())?
            .then(|| SpineBone::create(self.entity, BoneId(0)))
    }

    /// Every bone, parents before children.
    #[func]
    fn get_bones(&self) -> VarArray {
        let count = world::skeleton(self.entity, |skeleton| skeleton.bones().len());
        (0..count.unwrap_or_default())
            .map(|bone| SpineBone::create(self.entity, BoneId(bone)).to_variant())
            .collect()
    }

    /// Every slot, in setup order.
    #[func]
    fn get_slots(&self) -> VarArray {
        let count = world::skeleton(self.entity, |skeleton| skeleton.slots().len());
        (0..count.unwrap_or_default())
            .map(|slot| SpineSlot::create(self.entity, SlotId(slot)).to_variant())
            .collect()
    }

    /// Every slot, in the order they are drawn now.
    #[func]
    fn get_draw_order(&self) -> VarArray {
        let order: Vec<SlotId> = world::skeleton(self.entity, |skeleton| {
            skeleton.draw_order().applied_pose().to_vec()
        })
        .unwrap_or_default();
        order
            .into_iter()
            .map(|slot| SpineSlot::create(self.entity, slot).to_variant())
            .collect()
    }

    /// The skeleton data this skeleton was made from.
    #[func]
    fn get_data(&self) -> Option<Gd<SpineSkeletonDataResource>> {
        world::sprite(self.entity)?.bind().skeleton_data()
    }

    /// The active skin, or null when only the default skin is used.
    #[func]
    fn get_skin(&self) -> Option<Gd<SpineSkin>> {
        let skin = world::skeleton(self.entity, |skeleton| skeleton.skin().cloned())??;
        Some(SpineSkin::wrap(skin))
    }

    /// Shows the skin called `skin_name`. Like `Skeleton.setSkin` in Spine this does not reset the
    /// slots. Returns false and changes nothing if there is no such skin.
    #[func]
    fn set_skin_by_name(&mut self, skin_name: GString) -> bool {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.set_skin_by_name(&skin_name.to_string())
        })
        .unwrap_or(false)
    }

    /// Shows `new_skin`, or only the default skin for null. Like `Skeleton.setSkin` in Spine this
    /// does not reset the slots.
    #[func]
    fn set_skin(&mut self, new_skin: Option<Gd<SpineSkin>>) {
        let Some(mut sprite) = world::sprite(self.entity) else {
            return;
        };
        match new_skin {
            Some(skin) => {
                sprite.bind_mut().set_custom_skin(skin);
            }
            None => world::skeleton_mut(self.entity, |skeleton| skeleton.set_skin(None))
                .unwrap_or_default(),
        }
    }

    /// The attachment `attachment_name` of the slot called `slot_name`, searching the active skin
    /// and then the default skin, or null.
    #[func]
    fn get_attachment_by_slot_name(
        &self,
        slot_name: GString,
        attachment_name: GString,
    ) -> Option<Gd<SpineAttachment>> {
        let attachment = world::skeleton(self.entity, |skeleton| {
            skeleton.attachment(&slot_name.to_string(), &attachment_name.to_string())
        })??;
        let textures = world::sprite(self.entity)?.bind().kept_textures();
        Some(SpineAttachment::wrap(attachment, textures))
    }

    /// Like `get_attachment_by_slot_name`, with the slot given by its index.
    #[func]
    fn get_attachment_by_slot_index(
        &self,
        slot_index: i32,
        attachment_name: GString,
    ) -> Option<Gd<SpineAttachment>> {
        let slot_index = usize::try_from(slot_index).ok()?;
        let attachment = world::skeleton(self.entity, |skeleton| {
            (slot_index < skeleton.slots().len())
                .then(|| skeleton.attachment_by_index(slot_index, &attachment_name.to_string()))
                .flatten()
        })??;
        let textures = world::sprite(self.entity)?.bind().kept_textures();
        Some(SpineAttachment::wrap(attachment, textures))
    }

    /// Shows the attachment `attachment_name` in the slot called `slot_name` (an empty name hides
    /// it). Returns false if either name is unknown.
    #[func]
    fn set_attachment(&mut self, slot_name: GString, attachment_name: GString) -> bool {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.set_attachment(&slot_name.to_string(), &attachment_name.to_string())
        })
        .unwrap_or(false)
    }

    /// The bounding rectangle of everything drawn, in the skeleton's local coordinates.
    #[func]
    fn get_bounds(&self) -> Rect2 {
        bounds_of(self.entity)
    }

    /// The tint applied to the whole skeleton.
    #[func]
    fn get_color(&self) -> Color {
        world::skeleton(self.entity, |skeleton| {
            let c = skeleton.color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
        .unwrap_or(Color::WHITE)
    }

    #[func]
    fn set_color(&mut self, color: Color) {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.color.set(color.r, color.g, color.b, color.a);
        });
    }

    #[func]
    fn set_position(&mut self, position: Vector2) {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.set_position(position.x, position.y)
        });
    }

    #[func]
    fn get_x(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| skeleton.x).unwrap_or_default()
    }

    #[func]
    fn set_x(&mut self, x: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.x = x);
    }

    #[func]
    fn get_y(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| skeleton.y).unwrap_or_default()
    }

    #[func]
    fn set_y(&mut self, y: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.y = y);
    }

    #[func]
    fn get_scale_x(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| skeleton.scale_x).unwrap_or(1.0)
    }

    #[func]
    fn set_scale_x(&mut self, scale_x: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.scale_x = scale_x);
    }

    #[func]
    fn get_scale_y(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| skeleton.scale_y()).unwrap_or(1.0)
    }

    #[func]
    fn set_scale_y(&mut self, scale_y: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.set_scale_y(scale_y));
    }

    /// The time the physics constraints run on, in seconds.
    #[func]
    fn get_time(&self) -> f32 {
        world::skeleton(self.entity, |skeleton| skeleton.time).unwrap_or_default()
    }

    #[func]
    fn set_time(&mut self, time: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.time = time);
    }

    /// Advances the time the physics constraints run on by `delta` seconds. The sprite does this
    /// itself each frame.
    #[func]
    fn update(&mut self, delta: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.update_time(delta));
    }

    /// Moves the physics simulation as if the skeleton moved by `x`, `y`, so physics constraints
    /// don't react to the movement.
    #[func]
    fn physics_translate(&mut self, x: f32, y: f32) {
        world::skeleton_mut(self.entity, |skeleton| skeleton.physics_translate(x, y));
    }

    /// Rotates the physics simulation by `degrees` around `x`, `y`.
    #[func]
    fn physics_rotate(&mut self, x: f32, y: f32, degrees: f32) {
        world::skeleton_mut(self.entity, |skeleton| {
            skeleton.physics_rotate(x, y, degrees)
        });
    }

    /// The IK constraint called `constraint_name`, or null.
    #[func]
    fn find_ik_constraint(&self, constraint_name: GString) -> Option<Gd<SpineIkConstraint>> {
        let index = self.find_constraint(&constraint_name, is_ik)?;
        Some(SpineIkConstraint::create(self.entity, index))
    }

    /// The transform constraint called `constraint_name`, or null.
    #[func]
    fn find_transform_constraint(
        &self,
        constraint_name: GString,
    ) -> Option<Gd<SpineTransformConstraint>> {
        let index = self.find_constraint(&constraint_name, is_transform)?;
        Some(SpineTransformConstraint::create(self.entity, index))
    }

    /// The path constraint called `constraint_name`, or null.
    #[func]
    fn find_path_constraint(&self, constraint_name: GString) -> Option<Gd<SpinePathConstraint>> {
        let index = self.find_constraint(&constraint_name, is_path)?;
        Some(SpinePathConstraint::create(self.entity, index))
    }

    /// The physics constraint called `constraint_name`, or null.
    #[func]
    fn find_physics_constraint(
        &self,
        constraint_name: GString,
    ) -> Option<Gd<SpinePhysicsConstraint>> {
        let index = self.find_constraint(&constraint_name, is_physics)?;
        Some(SpinePhysicsConstraint::create(self.entity, index))
    }

    /// The slider called `slider_name`, or null.
    #[func]
    fn find_slider(&self, slider_name: GString) -> Option<Gd<SpineSlider>> {
        let index = self.find_constraint(&slider_name, is_slider)?;
        Some(SpineSlider::create(self.entity, index))
    }

    /// Every IK constraint, in skeleton order.
    #[func]
    fn get_ik_constraints(&self) -> Array<Gd<SpineIkConstraint>> {
        self.constraints_of(is_ik, SpineIkConstraint::create)
    }

    /// Every transform constraint, in skeleton order.
    #[func]
    fn get_transform_constraints(&self) -> Array<Gd<SpineTransformConstraint>> {
        self.constraints_of(is_transform, SpineTransformConstraint::create)
    }

    /// Every path constraint, in skeleton order.
    #[func]
    fn get_path_constraints(&self) -> Array<Gd<SpinePathConstraint>> {
        self.constraints_of(is_path, SpinePathConstraint::create)
    }

    /// Every slider, in skeleton order.
    #[func]
    fn get_sliders(&self) -> Array<Gd<SpineSlider>> {
        self.constraints_of(is_slider, SpineSlider::create)
    }
}

fn is_ik(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Ik(_))
}

fn is_transform(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Transform(_))
}

fn is_path(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Path(_))
}

fn is_physics(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Physics(_))
}

fn is_slider(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Slider(_))
}
