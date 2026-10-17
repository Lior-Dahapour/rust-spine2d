//! `SpineAnimation` and `SpineTimeline`: an animation of the skeleton data and the keyed values it
//! is made of. Both read the shared skeleton data, so they stay current when it is changed.

use godot::classes::RefCounted;
use godot::prelude::*;
use spine2d_core::{
    empty_animation, Animation, ApplyOptions, Bone1Kind, Bone2Kind, ColorKind, Constraint1Kind,
    Event, MixFrom, SkeletonData, Timeline,
};
use std::sync::Arc;

use super::event::SpineEvent;
use super::skeleton::SpineSkeleton;
use crate::runtime::store::Owner;
use crate::runtime::world;

/// The animation `index` of `data`, or the empty animation for `None`.
fn animation_of(data: &SkeletonData, index: Option<usize>) -> Arc<Animation> {
    match index {
        Some(index) => data.animations[index].clone(),
        None => empty_animation(),
    }
}

/// How the arguments of `apply` map to the options of the core.
fn apply_options(
    alpha: f32,
    from_setup: bool,
    add: bool,
    out: bool,
    applied_pose: bool,
) -> ApplyOptions {
    ApplyOptions {
        alpha,
        from: if from_setup {
            MixFrom::Setup
        } else {
            MixFrom::Current
        },
        add,
        out,
        applied_pose,
    }
}

/// Applies `apply` to the skeleton of `skeleton` and appends the events it fired to `events`.
fn apply_to_skeleton(
    skeleton: &Gd<SpineSkeleton>,
    events: &mut VarArray,
    apply: impl FnOnce(&mut spine2d_core::skeleton::BorrowedSkeleton, &mut Vec<Arc<Event>>),
) {
    let entity = skeleton.bind().entity();
    let mut fired = Vec::new();
    if world::skeleton_mut(entity, |skeleton| apply(skeleton, &mut fired)).is_none() {
        return;
    }
    for event in &fired {
        events.push(&SpineEvent::from_core(entity, event).to_variant());
    }
}

/// An animation: a name, a duration and the timelines that pose the skeleton over that time.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineAnimation {
    base: Base<RefCounted>,
    owner: Owner,
    /// Its place in the skeleton data, or `None` for the empty animation.
    index: Option<usize>,
}

impl SpineAnimation {
    /// A handle to animation `index` of `owner`, or to the empty animation for `None`.
    pub fn create(owner: impl Into<Owner>, index: Option<usize>) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineAnimation {
            base,
            owner: owner.into(),
            index,
        })
    }

    /// A handle to `animation`, which is one of the data's animations or the empty one.
    pub fn of(owner: impl Into<Owner>, animation: &Arc<Animation>) -> Option<Gd<Self>> {
        let owner = owner.into();
        let found = world::owner_data(owner, |data| {
            data.animations
                .iter()
                .position(|a| Arc::ptr_eq(a, animation))
        })?;
        match found {
            Some(index) => Some(Self::create(owner, Some(index))),
            None if Arc::ptr_eq(animation, &empty_animation()) => Some(Self::create(owner, None)),
            None => None,
        }
    }

    fn read<R>(&self, f: impl FnOnce(&Animation) -> R) -> Option<R> {
        world::owner_data(self.owner, |data| f(&animation_of(data, self.index)))
    }
}

#[godot_api]
impl SpineAnimation {
    /// False once the sprite this animation came from was reloaded or freed.
    #[func]
    fn is_valid(&self) -> bool {
        world::owner_data(self.owner, |_| ()).is_some()
    }

    #[func]
    fn get_name(&self) -> GString {
        self.read(|animation| GString::from(animation.name()))
            .unwrap_or_default()
    }

    /// The length in seconds.
    #[func]
    fn get_duration(&self) -> f32 {
        self.read(|animation| animation.duration)
            .unwrap_or_default()
    }

    /// Sets the length in seconds, for every sprite made from the same skeleton data. The empty
    /// animation cannot be changed.
    #[func]
    fn set_duration(&mut self, duration: f32) {
        let Some(index) = self.index else {
            godot_warn!("Spine: the empty animation cannot be changed");
            return;
        };
        world::edit_data(self.owner, |data| {
            Arc::make_mut(&mut data.animations[index]).duration = duration;
        });
    }

    #[func]
    fn get_timelines(&self) -> VarArray {
        let count = self
            .read(|animation| animation.timelines().len())
            .unwrap_or_default();
        (0..count)
            .map(|timeline| SpineTimeline::create(self.owner, self.index, timeline).to_variant())
            .collect()
    }

    /// Whether the animation has a timeline for every property id in `ids`.
    #[func]
    fn has_timeline(&self, ids: VarArray) -> bool {
        let ids: Vec<u64> = ids
            .iter_shared()
            .filter_map(|id| id.try_to::<i64>().ok())
            .map(|id| id as u64)
            .collect();
        self.read(|animation| animation.has_timeline(&ids))
            .unwrap_or_default()
    }

    /// Poses `skeleton` from this animation at `time`. `last_time` is the time of the last call,
    /// or -1 the first time. Events it fires are appended to `events` as `SpineEvent`s.
    ///
    /// The official plugin's version of this call is broken and fills nothing into `events`.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        skeleton: Option<Gd<SpineSkeleton>>,
        last_time: f32,
        time: f32,
        looping: bool,
        mut events: VarArray,
        alpha: f32,
        from_setup: bool,
        add: bool,
        out: bool,
        applied_pose: bool,
    ) {
        let Some(skeleton) = skeleton else { return };
        let options = apply_options(alpha, from_setup, add, out, applied_pose);
        let index = self.index;
        apply_to_skeleton(&skeleton, &mut events, |view, fired| {
            let animation = animation_of(view.data(), index);
            animation.apply(view, last_time, time, looping, Some(fired), &options);
        });
    }
}

/// One timeline of an animation: keyed values for one property, or a few related ones.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineTimeline {
    base: Base<RefCounted>,
    owner: Owner,
    animation: Option<usize>,
    timeline: usize,
}

impl SpineTimeline {
    fn create(owner: Owner, animation: Option<usize>, timeline: usize) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineTimeline {
            base,
            owner,
            animation,
            timeline,
        })
    }

    fn read<R>(&self, f: impl FnOnce(&Timeline) -> R) -> Option<R> {
        world::owner_data(self.owner, |data| {
            let animation = animation_of(data, self.animation);
            animation.timelines().get(self.timeline).map(f)
        })
        .flatten()
    }
}

/// The name of the timeline class of the reference runtime that `timeline` corresponds to.
fn type_name(timeline: &Timeline) -> &'static str {
    match timeline {
        Timeline::Bone1(t) => match t.kind {
            Bone1Kind::Rotate => "RotateTimeline",
            Bone1Kind::X => "TranslateXTimeline",
            Bone1Kind::Y => "TranslateYTimeline",
            Bone1Kind::ScaleX => "ScaleXTimeline",
            Bone1Kind::ScaleY => "ScaleYTimeline",
            Bone1Kind::ShearX => "ShearXTimeline",
            Bone1Kind::ShearY => "ShearYTimeline",
        },
        Timeline::Bone2(t) => match t.kind {
            Bone2Kind::Translate => "TranslateTimeline",
            Bone2Kind::Scale => "ScaleTimeline",
            Bone2Kind::Shear => "ShearTimeline",
        },
        Timeline::Inherit(_) => "InheritTimeline",
        Timeline::Color(t) => match t.kind {
            ColorKind::Rgba => "RGBATimeline",
            ColorKind::Rgb => "RGBTimeline",
            ColorKind::Rgba2 => "RGBA2Timeline",
            ColorKind::Rgb2 => "RGB2Timeline",
        },
        Timeline::Alpha(_) => "AlphaTimeline",
        Timeline::Attachment(_) => "AttachmentTimeline",
        Timeline::Deform(_) => "DeformTimeline",
        Timeline::Sequence(_) => "SequenceTimeline",
        Timeline::DrawOrder(_) => "DrawOrderTimeline",
        Timeline::DrawOrderFolder(_) => "DrawOrderFolderTimeline",
        Timeline::Event(_) => "EventTimeline",
        Timeline::Ik(_) => "IkConstraintTimeline",
        Timeline::Transform(_) => "TransformConstraintTimeline",
        Timeline::PathMix(_) => "PathConstraintMixTimeline",
        Timeline::Constraint1(t) => match t.kind {
            Constraint1Kind::PathPosition => "PathConstraintPositionTimeline",
            Constraint1Kind::PathSpacing => "PathConstraintSpacingTimeline",
            Constraint1Kind::SliderTime => "SliderTimeline",
            Constraint1Kind::SliderMix => "SliderMixTimeline",
            Constraint1Kind::PhysicsInertia => "PhysicsConstraintInertiaTimeline",
            Constraint1Kind::PhysicsStrength => "PhysicsConstraintStrengthTimeline",
            Constraint1Kind::PhysicsDamping => "PhysicsConstraintDampingTimeline",
            Constraint1Kind::PhysicsMass => "PhysicsConstraintMassTimeline",
            Constraint1Kind::PhysicsWind => "PhysicsConstraintWindTimeline",
            Constraint1Kind::PhysicsGravity => "PhysicsConstraintGravityTimeline",
            Constraint1Kind::PhysicsMix => "PhysicsConstraintMixTimeline",
        },
        Timeline::PhysicsReset(_) => "PhysicsConstraintResetTimeline",
    }
}

#[godot_api]
impl SpineTimeline {
    /// False once the sprite this timeline came from was reloaded or freed.
    #[func]
    fn is_valid(&self) -> bool {
        self.read(|_| ()).is_some()
    }

    /// The name of the timeline class, for example `RotateTimeline`.
    #[func]
    fn get_type(&self) -> GString {
        self.read(|timeline| GString::from(type_name(timeline)))
            .unwrap_or_default()
    }

    /// How many floats one key takes in `get_frames()`; the first is the time.
    #[func]
    fn get_frame_entries(&self) -> i32 {
        self.read(|timeline| timeline.frame_entries() as i32)
            .unwrap_or_default()
    }

    /// How many keys there are.
    #[func]
    fn get_frame_count(&self) -> i32 {
        self.read(|timeline| timeline.frame_count() as i32)
            .unwrap_or_default()
    }

    /// The keyed floats, `get_frame_entries()` per key.
    #[func]
    fn get_frames(&self) -> VarArray {
        self.read(|timeline| timeline.frames().iter().map(|f| f.to_variant()).collect())
            .unwrap_or_default()
    }

    /// The time of the last key, in seconds.
    #[func]
    fn get_duration(&self) -> f32 {
        self.read(|timeline| timeline.duration())
            .unwrap_or_default()
    }

    /// The ids of the properties this timeline changes.
    #[func]
    fn get_property_ids(&self) -> VarArray {
        self.read(|timeline| {
            timeline
                .property_ids()
                .iter()
                .map(|id| (*id as i64).to_variant())
                .collect()
        })
        .unwrap_or_default()
    }

    /// Poses `skeleton` from this timeline at `time`. Events it fires are appended to `events` as
    /// `SpineEvent`s.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        skeleton: Option<Gd<SpineSkeleton>>,
        last_time: f32,
        time: f32,
        mut events: VarArray,
        alpha: f32,
        from_setup: bool,
        add: bool,
        out: bool,
        applied_pose: bool,
    ) {
        let Some(skeleton) = skeleton else { return };
        let options = apply_options(alpha, from_setup, add, out, applied_pose);
        let (animation, timeline) = (self.animation, self.timeline);
        apply_to_skeleton(&skeleton, &mut events, |view, fired| {
            let animation = animation_of(view.data(), animation);
            if let Some(timeline) = animation.timelines().get(timeline) {
                timeline.apply(view, last_time, time, Some(fired), &options);
            }
        });
    }
}
