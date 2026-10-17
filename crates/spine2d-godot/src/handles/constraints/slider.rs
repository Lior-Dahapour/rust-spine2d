//! The slider classes: `SpineSlider`, its pose and its data. A slider is a constraint that plays
//! an animation driven by a bone property or by a time value.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{BoneId, Slider, SliderData, SliderPose};
use spine2d_ecs::bevy_ecs::entity::Entity;

use super::{
    edit_data, edit_data_structure, edit_structure, own_bone, own_bone_data, read_body, read_data,
    read_pose, write_pose, ConstraintHandle,
};
use crate::handles::bone::{SpineBone, SpineBoneData};
use crate::handles::{warn_not_implemented, PoseKind, SpineAnimation, SpineSkeleton};
use crate::runtime::store::Owner;
use crate::runtime::world;

/// A slider of a skeleton: plays an animation driven by a bone property or by a time value.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSlider {
    base: Base<RefCounted>,
    entity: Entity,
    index: usize,
}

impl SpineSlider {
    /// A handle to constraint `index` of the skeleton `entity`.
    pub fn create(entity: Entity, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSlider {
            base,
            entity,
            index,
        })
    }
}

impl ConstraintHandle for SpineSlider {
    fn owner(&self) -> Owner {
        Owner::Skeleton(self.entity)
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineSlider {
    /// False once the sprite this slider came from was reloaded or freed. Reads then return zero
    /// values and writes do nothing.
    #[func]
    fn is_valid(&self) -> bool {
        world::skeleton(self.entity, |_| ()).is_some()
    }

    /// Not implemented yet: logs a warning. `physics` is one of `SpineConstant.Physics_*`.
    #[func]
    fn update(&mut self, _skeleton: Option<Gd<SpineSkeleton>>, _physics: i32) {
        warn_not_implemented("update");
    }

    #[func]
    fn get_data(&self) -> Gd<SpineSliderData> {
        SpineSliderData::create(self.entity, self.index)
    }

    /// The bone whose property drives the animation, or null when the pose time drives it.
    #[func]
    fn get_bone(&self) -> Option<Gd<SpineBone>> {
        let bone = read_body(self.entity, self.index, |slider: &Slider| slider.bone())??;
        Some(SpineBone::create(self.entity, bone))
    }

    /// Drives the animation from a property of `bone`, a bone of the same skeleton, from now on.
    /// The skeleton's update order is rebuilt at the next frame.
    #[func]
    fn set_bone(&mut self, bone: Option<Gd<SpineBone>>) {
        let Some(bone) = bone.and_then(|bone| own_bone(self.entity, &bone)) else {
            return;
        };
        edit_structure(self.entity, self.index, |slider: &mut Slider| {
            slider.set_bone(Some(bone));
        });
    }

    /// The pose that animations and scripts set.
    #[func]
    fn get_pose(&self) -> Gd<SpineSliderPose> {
        SpineSliderPose::create(self.entity, self.index, PoseKind::Pose)
    }

    /// The pose after other constraints modified it; the one the skeleton is drawn with.
    #[func]
    fn get_applied_pose(&self) -> Gd<SpineSliderPose> {
        SpineSliderPose::create(self.entity, self.index, PoseKind::Applied)
    }

    /// Whether the slider takes part in the update. The skeleton sets this from the active skin.
    #[func]
    fn is_active(&self) -> bool {
        self.constraint_active()
    }

    /// Sets the flag [`is_active`] reports. The skeleton recomputes it whenever the skin changes,
    /// and the update does not read it, so this does not turn the slider off.
    #[func]
    fn set_active(&mut self, active: bool) {
        self.set_constraint_active(active);
    }
}

/// The values of a slider that animations key: the time its animation plays at, and the mix.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSliderPose {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
    kind: PoseKind,
}

impl SpineSliderPose {
    /// A handle to the pose `kind` of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize, kind: PoseKind) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSliderPose {
            base,
            owner: owner.into(),
            index,
            kind,
        })
    }

    fn read(&self, f: impl FnOnce(&SliderPose) -> f32) -> f32 {
        read_pose(self.owner, self.index, self.kind, f).unwrap_or_default()
    }

    fn write(&self, f: impl FnOnce(&mut SliderPose)) {
        write_pose(self.owner, self.index, self.kind, f);
    }
}

#[godot_api]
impl SpineSliderPose {
    /// The time of the animation the slider applies, when no bone drives it.
    #[func]
    fn get_time(&self) -> f32 {
        self.read(|pose| pose.time)
    }

    #[func]
    fn set_time(&mut self, value: f32) {
        self.write(|pose| pose.time = value);
    }

    /// How much of the animation is applied, from 0 (none) to 1 (all).
    #[func]
    fn get_mix(&self) -> f32 {
        self.read(|pose| pose.mix)
    }

    #[func]
    fn set_mix(&mut self, value: f32) {
        self.write(|pose| pose.mix = value);
    }
}

/// The setup values of a slider, from the skeleton data. The data is shared by every sprite that
/// uses it, so the setters change it for all of them.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSliderData {
    base: Base<RefCounted>,
    owner: Owner,
    index: usize,
}

impl SpineSliderData {
    /// A handle to the data of constraint `index` of `owner`.
    pub fn create(owner: impl Into<Owner>, index: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSliderData {
            base,
            owner: owner.into(),
            index,
        })
    }

    fn read<R: Default>(&self, f: impl FnOnce(&SliderData) -> R) -> R {
        read_data(self.owner, self.index, f).unwrap_or_default()
    }
}

impl ConstraintHandle for SpineSliderData {
    fn owner(&self) -> Owner {
        self.owner
    }

    fn index(&self) -> usize {
        self.index
    }
}

#[godot_api]
impl SpineSliderData {
    #[func]
    fn get_constraint_name(&self) -> GString {
        self.constraint_name()
    }

    #[func]
    fn is_skin_required(&self) -> bool {
        self.skin_required()
    }

    /// The animation the slider drives, or null if it has none.
    #[func]
    fn get_animation(&self) -> Option<Gd<SpineAnimation>> {
        let animation = self.read(|data| data.animation.clone())?;
        SpineAnimation::of(self.owner, &animation)
    }

    /// The bone whose property drives the animation, or null when the pose time drives it.
    #[func]
    fn get_bone(&self) -> Option<Gd<SpineBoneData>> {
        let bone = self.read(|data| data.bone);
        Some(SpineBoneData::create(self.owner, BoneId(bone?)))
    }

    #[func]
    fn set_bone(&mut self, bone: Option<Gd<SpineBoneData>>) {
        let bone = match bone {
            Some(bone) => match own_bone_data(self.owner, &bone) {
                Some(bone) => Some(bone.index()),
                None => return,
            },
            None => None,
        };
        edit_data_structure::<SliderData>(self.owner, self.index, |data| data.bone = bone);
    }

    /// Whether the animation is added to the pose instead of replacing it.
    #[func]
    fn get_additive(&self) -> bool {
        self.read(|data| data.additive)
    }

    #[func]
    fn set_additive(&mut self, value: bool) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.additive = value;
        });
    }

    /// Whether the animation loops.
    #[func]
    fn get_loop(&self) -> bool {
        self.read(|data| data.loop_)
    }

    #[func]
    fn set_loop(&mut self, value: bool) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.loop_ = value;
        });
    }

    /// How many seconds the animation time moves per unit the bone property changes.
    #[func]
    fn get_scale(&self) -> f32 {
        self.read(|data| data.scale)
    }

    #[func]
    fn set_scale(&mut self, value: f32) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.scale = value;
        });
    }

    /// The animation time, in seconds, at the starting value of the bone property.
    #[func]
    fn get_offset(&self) -> f32 {
        self.read(|data| data.offset)
    }

    #[func]
    fn set_offset(&mut self, value: f32) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.offset = value;
        });
    }

    /// The maximum stored in the skeleton data for the driving property. The update does not read
    /// it.
    #[func]
    fn get_max(&self) -> f32 {
        self.read(|data| data.max)
    }

    #[func]
    fn set_max(&mut self, value: f32) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.max = value;
        });
    }

    /// Whether the bone property is read from the bone's local pose instead of its world pose.
    #[func]
    fn get_local(&self) -> bool {
        self.read(|data| data.local)
    }

    #[func]
    fn set_local(&mut self, value: bool) {
        edit_data::<SliderData>(self.owner, self.index, |data| {
            data.local = value;
        });
    }

    /// The setup values the pose starts from.
    #[func]
    fn get_setup_pose(&self) -> Gd<SpineSliderPose> {
        SpineSliderPose::create(self.owner, self.index, PoseKind::Setup)
    }
}
